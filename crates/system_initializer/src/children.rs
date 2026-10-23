//! The builders: the thin shapes over the ABI that make one child each (a grant, a caretaker, the
//! network stack, a graphical terminal session, the terminal), and `reclaim`. Why caretakers and
//! sessions are built and reclaimed the way they are is in notes/system-initializer/children.md.
//!
//! Name: provisional (milestone 843's lane, 2026-10-11 UTC).

use super::*;

// -------------------------------------------------------------------------------------------
// The thin shapes over the ABI. The loader itself is `supervision_protocol`'s.
// -------------------------------------------------------------------------------------------

/// **Build the caretaker a directory grant is delivered by**, and hand back its endpoint.
///
/// `fs_subtree_caretaker` for a directory, or `fs_nameset_caretaker` with its name set
/// (`set_seen`, `spawnproto::NAMESET_BIT`) on a page from the job's `region`. A set that arrives
/// for a program that does not hear words is refused with the grant, as `None`. For a program that
/// hears words, the rights the shell asked for are clamped to what `manifest` allows (§170 (how a
/// foreign program is told what to do) clause 4), which for unvouched bytes is read-only. Out of
/// line so its locals stay off the spawn service's frame.
#[inline(never)]
pub(super) fn build_grant(
    own_ut: u64,
    region: u64,
    fs: Fs,
    care: &Caretakers,
    care_words: (u64, u64, u64),
    set_seen: Option<u64>,
    manifest: Option<grant_plan::Manifest>,
) -> Option<u64> {
    let words = match manifest.map(|m| m.program_argument) {
        Some(grant_plan::ProgramArgumentDeclaration::Words(g)) => {
            use filesystem_contract::grant as gr;
            let rights = gr::spec_rights(care_words.2) & g.rights();
            (
                care_words.0,
                care_words.1,
                gr::spec(gr::spec_len(care_words.2), rights),
            )
        }
        _ => care_words,
    };
    let hears_words = manifest.is_some_and(|m| m.program_argument.hears_words());
    match set_seen {
        None => care
            .subtree
            .as_ref()
            .and_then(|e| build_caretaker(own_ut, region, e, fs, words, None)),
        Some(theirs) if hears_words => {
            let set = copy_args(own_ut, region, theirs, |_| true)?;
            let ep = care
                .nameset
                .as_ref()
                .and_then(|e| build_caretaker(own_ut, region, e, fs, words, Some(set)));
            cap_delete(set);
            ep
        }
        Some(_) => None,
    }
}

/// **Build a caretaker for one directory grant and hand back its endpoint**, which the program
/// holds instead of anything naming the file server (DECISIONS §92 (a caretaker is supervised by
/// the client it serves)). Everything comes from the client's `region`; `care_words` are the
/// `START` words as the shell packed them. `None` when the descent is refused or the build fails.
/// See notes/system-initializer/children.md.
///
/// # BUGS
///
/// A caretaker that dies before it answers parks the progenitor, since a corpse sends nothing and
/// no receive has a deadline; `kernel::user::fs_service::wait_for_caretaker` carries the same.
pub(super) fn build_caretaker(
    own_ut: u64,
    region: u64,
    care: &elf::Elf,
    fs: Fs,
    care_words: (u64, u64, u64),
    set: Option<u64>,
) -> Option<u64> {
    let narrow_ep = retype_obj(region, abi::objtype::RENDEZVOUS).ok()?;
    let ready = retype_obj(region, abi::objtype::RENDEZVOUS).ok()?;
    let maps = [
        (FS_CLIENT_PAGE_VA, fs.page, abi::address_space::MAP_RW),
        (
            NAMESET_PAGE_VA,
            set.unwrap_or(0),
            abi::address_space::MAP_RO,
        ),
    ];
    // Its whole authority: no untyped, no clock, no terminal, nothing that names another process.
    let built = build_child(
        own_ut,
        region,
        care,
        &ChildEndowment {
            caps: &[
                (fs.ep, abi::rights::WRITE),
                (narrow_ep, abi::rights::READ),
                (ready, abi::rights::WRITE),
            ],
            maps: &maps[..if set.is_some() { 2 } else { 1 }],
            stack_pages: CARETAKER_STACK_PAGES,
            ..ChildEndowment::new(Retention::Nothing)
        },
    );
    // No fault slot: the client's reap collects it, and a second death message would make
    // `job_undertaker` trap on a tid the scheduler no longer knows.
    let child = built.ok()?;
    let started = start_child(child, care_words.0, care_words.1, care_words.2);
    if !started {
        cap_delete(ready);
        cap_delete(narrow_ep);
        return None;
    }
    // The one bounded wait: the caretaker answers `READY` or `DESCENT_REFUSED`, never nothing.
    let (verdict, _, _) = receive(ready);
    cap_delete(ready);
    if verdict == filesystem_contract::READY {
        Some(narrow_ep)
    } else {
        // A refused descent; the caretaker has already exited.
        cap_delete(narrow_ep);
        None
    }
}

/// **Give a failed job's region back, retrying while something in it can still run.**
///
/// A destroy of a region holding a live thread is refused with the kill of §16 (object revocation:
/// reclaim the objects a process built) armed, and one preemption later succeeds
/// (`sched::reclaim_region`). The path is reached when a directory grant's caretaker was built and
/// its program was not. Bounded and silent on failure, unlike `job_undertaker`: a trap here would
/// take the prompt down over one command's memory. See notes/system-initializer/children.md.
pub(super) fn reclaim(region: u64) {
    for _ in 0..RECLAIM_ATTEMPTS {
        if supervision_protocol::memory_region_destroy(region) {
            return;
        }
        user_mode_runtime::yield_now();
    }
}

/// How many times [`reclaim`] retries. Small, because the only resident it ever waits on is a
/// caretaker that has already been woken and doomed, and one preemption is enough.
///
/// # BUGS
///
/// Sixty-four yields do not reliably cover one preemption under load, and a miss strands
/// [`DIR_JOB_REGION_PAGES`] silently; never observed here. The measurement and the fix are in
/// notes/system-initializer/children.md, "How many times `reclaim` retries".
const RECLAIM_ATTEMPTS: usize = 64;

/// **Build `net_stack` from the NIC trio and wait for its DHCP lease.** Returns `(stack, lease,
/// name server)`: the full-rights endpoint its clients `CALL`, which this process keeps, the IPv4
/// address `net_stack` reported, big-endian in the low 32 bits, and the lease's name server in
/// `socket_contract::lease`'s encoding. The endowment is
/// `kernel::user::virtio_service::wire_net_server`'s, slot for slot, with
/// `socket_contract::NO_LISTEN_GRANT`. The caller deletes the trio; this deletes everything else it
/// made but the endpoint. Failures trap: one means the boot's arithmetic is wrong.
pub(super) fn build_net_stack(ut: u64, program: &elf::Elf, g: &BootEndowment) -> (u64, u64, u64) {
    let mapped = map_page_frame(g.virtio_net_dma, NET_DMA_PEEK_VA, true, ut);
    must_ok(mapped);
    // SAFETY: just mapped read/write, one page; the offset is inside it and outside
    // `virtio_net_transport`'s rings and buffers (see `kernel::user::VIRTIO_DMA_PHYS_OFFSET`).
    let direct_memory_access_phys = unsafe {
        core::ptr::read_unaligned(
            (NET_DMA_PEEK_VA as *const u8)
                .add(VIRTIO_DMA_PHYS_OFFSET as usize)
                .cast::<u64>(),
        )
    };
    // Read; the rings are `net_stack`'s from here, so our view of them goes (milestone 95).
    supervision_protocol::give_up_own_page(NET_DMA_PEEK_VA);
    let stack = must(retype_obj(ut, abi::objtype::RENDEZVOUS));
    let report = must(retype_obj(ut, abi::objtype::RENDEZVOUS));
    let budget = must(memory_region_split(ut, NET_STACK_BUDGET_PAGES));
    let child = must(build_child(
        ut,
        ut,
        program,
        &ChildEndowment {
            caps: &[
                (report, abi::rights::WRITE),
                (g.virtio_net_irq, abi::rights::READ),
                (g.virtio_net, abi::rights::WRITE),
                (budget, abi::rights::SPEND_ONLY),
                (stack, abi::rights::READ),
            ],
            // §255 (each socket is its own capability): the endpoint sockets are minted from, and
            // its own address space, so `CLOSE` can take a socket's page back.
            placed: &[
                (
                    socket_contract::stack_slots::MINT,
                    stack,
                    abi::rights::WRITE | abi::rights::GRANT,
                ),
                (
                    socket_contract::stack_slots::OWN_SPACE,
                    supervision_protocol::CHILDS_OWN_SPACE,
                    abi::rights::WRITE,
                ),
            ],
            maps: &[(NET_DMA_VA, g.virtio_net_dma, abi::address_space::MAP_RW)],
            stack_pages: NET_STACK_STACK_PAGES,
            ..ChildEndowment::new(Retention::Nothing)
        },
    ));
    must_ok(start_child(
        child,
        NET_STACK_ROLE_SERVER,
        direct_memory_access_phys,
        socket_contract::NO_LISTEN_GRANT,
    ));
    cap_delete(budget);
    // Its only message here, sent blocking: it serves nobody until this receive takes it.
    let (lease, name_server, _) = receive(report);
    cap_delete(report);
    (stack, lease, name_server)
}

/// **Measure `domain_name_resolver` and build it**, or `None` when the table does not vouch for it. Never
/// inlined, so the measured image stays out of [`boot`](crate::boot)'s frame, which the spawn
/// service runs inside for the life of the boot ([`graphical_terminal_grants`]' reason).
/// `None` too with no stack, no entropy (it asks nothing without unguessable ids), or a lease that
/// named no name server. `network` is `(stack, lease, name server)`, the server in
/// `socket_contract::lease`'s encoding.
#[inline(never)]
pub(super) fn start_domain_name_resolver(
    (fs, table, ut): (&nifefs::Fs<'static>, &str, u64),
    network: Option<(u64, u64, u64)>,
    entropy: Option<u64>,
) -> Option<u64> {
    let (stack, _, name_server) = network?;
    let server = socket_contract::lease::word_ipv4(name_server)?;
    let program = measured(fs, table, "domain_name_resolver").elf?;
    Some(build_domain_name_resolver(
        ut, &program, stack, entropy?, server,
    ))
}

/// **What the boot says about its network**: the lease `net_stack` reported, so the line is
/// evidence rather than an intention, and the name server the resolver asks, or why there is no
/// resolver. Silent on a boot with no network.
pub(super) fn announce_network(term_ep: u64, network: Option<(u64, u64, u64)>, resolver: bool) {
    let Some((_, lease, name_server)) = network else {
        return;
    };
    let mut line = [0u8; 80];
    let head = b"progenitor: network stack up; DHCP leased ";
    line[..head.len()].copy_from_slice(head);
    let mut n = head.len();
    n += render_ipv4(lease as u32, &mut line[n..]);
    line[n] = b'\n';
    announce(term_ep, &line[..n + 1]);
    if resolver {
        let head = b"progenitor: name resolver up; asks ";
        line[..head.len()].copy_from_slice(head);
        let mut n = head.len();
        n += render_ipv4(name_server as u32, &mut line[n..]);
        let tail = b", the lease's name server\n";
        line[n..n + tail.len()].copy_from_slice(tail);
        announce(term_ep, &line[..n + tail.len()]);
    } else {
        announce(
            term_ep,
            b"progenitor: no name resolver (no lease name server, no entropy, or not vouched); \
              programs here resolve no names\n",
        );
    }
}

/// **Build `domain_name_resolver` on the booted system's stack**, told to ask `server`, the
/// lease's name server, on port 53 over UDP (TCP when a reply is truncated). Returns its unbadged
/// endpoint, which this process keeps: each resolving job's badged copy is minted from it, and the
/// grant and the forget are sent through it. The endowment is the kernel harness's
/// (`name_resolver_tests::start_resolver`), slot for slot, but for the server. Failures trap, for
/// [`build_net_stack`]'s reason.
pub(super) fn build_domain_name_resolver(
    ut: u64,
    program: &elf::Elf,
    stack: u64,
    entropy: u64,
    server: [u8; 4],
) -> u64 {
    use domain_name_resolution_contract::endowment;
    let service = must(retype_obj(ut, abi::objtype::RENDEZVOUS));
    let budget = must(memory_region_split(ut, RESOLVER_BUDGET_PAGES));
    let child = must(build_child(
        ut,
        ut,
        program,
        &ChildEndowment {
            placed: &[
                (0, service, abi::rights::READ),
                (1, stack, abi::rights::WRITE),
                // Every right: lending a socket a page retyped from this budget needs `GRANT`.
                (2, budget, abi::rights::ALL),
                (3, entropy, abi::rights::WRITE),
                (
                    endowment::OWN_SPACE_SLOT,
                    supervision_protocol::CHILDS_OWN_SPACE,
                    abi::rights::WRITE,
                ),
            ],
            stack_pages: RESOLVER_STACK_PAGES,
            ..ChildEndowment::new(Retention::Nothing)
        },
    ));
    must_ok(start_child(
        child,
        endowment::server(server),
        endowment::port_and_transport(53, endowment::TRANSPORT_UDP),
        0,
    ));
    cap_delete(budget);
    service
}

/// **What a `graphical_terminal` session is made of, measured, and whether this boot can launch
/// one.** `held` means the kernel granted a gpu and the table vouches for `gpu_driver` and
/// `display_terminal`; `keyboard` also needs the keyboard's grant and a vouched `keyboard_driver`.
/// The spawn service keeps what is held for every session it builds, and a grant no session can be
/// built from is deleted here. Never inlined, so the measured images stay out of [`boot`]'s frame.
#[inline(never)]
pub(super) fn graphical_terminal_grants(
    fs: &nifefs::Fs<'static>,
    table: &str,
    g: &BootEndowment,
) -> GraphicalTerminalCaps {
    let driver = measured(fs, table, "gpu_driver").elf;
    let terminal = measured(fs, table, "display_terminal").elf;
    let held = is_granted(g.gpu) && driver.is_some() && terminal.is_some();
    if is_granted(g.gpu) && !held {
        for c in [g.gpu, g.gpu_irq, g.gpu_dma, g.gpu_surface] {
            cap_delete(c);
        }
    }
    let kbd_driver = measured(fs, table, "keyboard_driver").elf;
    let keyboard = held && is_granted(g.keyboard) && kbd_driver.is_some();
    if is_granted(g.keyboard) && !keyboard {
        for c in [g.keyboard, g.keyboard_irq, g.keyboard_dma] {
            cap_delete(c);
        }
    }
    GraphicalTerminalCaps {
        held,
        keyboard,
        gpu: [g.gpu, g.gpu_irq, g.gpu_dma, g.gpu_surface],
        kbd: [g.keyboard, g.keyboard_irq, g.keyboard_dma],
        driver,
        terminal,
        kbd_driver,
    }
}

/// The spawn service's half of [`graphical_terminal_grants`]' verdict: whether this boot has the
/// display devices, the devices themselves, and the measured images a session is built from.
/// `held` false means no session can be launched: the caps were deleted at boot, and a
/// `graphical_terminal` request is answered [`spawnproto::SPAWN_NO_DISPLAY`].
pub(super) struct GraphicalTerminalCaps {
    pub(super) held: bool,
    pub(super) keyboard: bool,
    /// The gpu's four, in the boot endowment's slots, held by the spawn service for the life of
    /// the boot. Meaningful only when `held`.
    pub(super) gpu: [u64; 4],
    /// The keyboard's three, likewise. Meaningful only when `keyboard`.
    pub(super) kbd: [u64; 3],
    driver: Option<elf::Elf<'static>>,
    terminal: Option<elf::Elf<'static>>,
    kbd_driver: Option<elf::Elf<'static>>,
}

/// The session's line discipline, which is the boot's own pair of programs: the supervisor that
/// builds `line_editor`, and the measured `line_editor` bytes it is handed as a blob. A session's
/// discipline is the boot's discipline, built twice from one measurement.
pub(super) struct SessionDiscipline {
    pub(super) supervisor: elf::Elf<'static>,
    pub(super) discipline: &'static [u8],
}

/// **The budget `gpu_driver` and `display_terminal` each map their frames through**, in pages of
/// page tables. The kernel's test wiring hands each the same (`kernel::user::display_service`'s
/// `MAP_BUDGET_PAGES`, whose doc says why twenty-four); stated here for [`CRED_BUDGET_PAGES`]'s
/// reason, a spawner states the budget it hands.
const GRAPHICAL_MAP_BUDGET_PAGES: u64 = 24;

/// Where `keyboard_driver` finds its DMA page. Must match `components/src/keyboard_driver.rs`'s
/// `DMA_VA`, the same kernel-and-spawner convention [`RNG_DMA_VA`] is.
const KBD_DMA_VA: u64 = address_space_map::pair_page(0x0000_0000_0090_0000);

/// `keyboard_driver.rs`'s own `MODE_DIRECT`: a fixed `CALL` to the terminal endpoint instead of a
/// compositor's ring and doorbell (milestone 177). [`RNG_MODE_VIRTIO`]'s reasoning.
const KBD_MODE_DIRECT: u64 = 1;

/// Where a `graphical_terminal` session's program writes its screen text and reads its lines.
/// `components/src/graphical_terminal.rs`'s own constants; the same band the boot's shell writes into, so a
/// reader of the wiring meets one convention per role rather than one per program.
const SCREEN_OUT_VA: u64 = address_space_map::pair_page(0x0000_0000_00a0_0000);
const SCREEN_IN_VA: u64 = address_space_map::pair_page(0x0000_0000_00a1_0000);

/// **One `graphical_terminal` session's region, in pages**: everything the session is made of, so
/// the session job's one reap ends the drivers with it. Measured on aarch64 and riscv64 under
/// `script/swish-check`'s graphical launch, not counted; it comes out of [`JOBS_BUDGET_PAGES`] for
/// the life of the session. How it was measured is in notes/system-initializer/budgets.md.
///
/// # BUGS
///
/// `x86_64` has no graphical leg, so its need is unmeasured and ungated.
pub(super) const GRAPHICAL_TERMINAL_SESSION_PAGES: u64 = 544;

/// Drop every capability named, freeing the slots: a builder's means, spent once the children hold
/// their own copies or the build failed, so a failed spawn costs no slot for the rest of the boot.
fn drop_caps(caps: &[u64]) {
    for c in caps {
        cap_delete(*c);
    }
}

/// The resources one `graphical_terminal` launch is built from and with, as a record because
/// [`build_graphical_terminal_session`] and its builders all take them.
pub(super) struct GraphicalTerminalLaunch<'a> {
    /// The spawn service's own untyped: the loader's scratch tables for every build come from
    /// here, everything the children are made of from [`Self::region`].
    pub(super) own_ut: u64,
    /// The session's job region, split by the caller. [`build_graphical_terminal_session`] owns its disposal.
    pub(super) region: u64,
    /// The supervision endpoint every job is born under. The session program's fault slot, so its
    /// one reap sweeps the drivers with it.
    pub(super) deaths: u64,
    /// The boot discipline's endpoint: the UART arm's keystroke source, the spawn service's only
    /// copy.
    pub(super) boot_terminal: u64,
    /// The shell's result endpoint, the session program's slot 0.
    pub(super) result_ep: u64,
    /// The gpu's four, the spawn service's own: lent, never consumed.
    pub(super) gpu: &'a [u64; 4],
    /// The keyboard's three, when this boot has one; lent likewise.
    pub(super) kbd: Option<&'a [u64; 3]>,
    /// The `graphical_terminal` program itself, measured.
    pub(super) program: &'a elf::Elf<'a>,
}

/// **Build a whole `graphical_terminal` session and start it**: `gpu_driver` and
/// `display_terminal` on the gpu's four (lent, never deleted), then the session's own line
/// discipline with `keyboard_driver` when `kbd` is `Some`, or a raw view of the boot line
/// discipline when it is not. Everything comes from the session's `region` but the loader's
/// tables, and only the session program is supervised, so its one reap ends the drivers too.
/// `Err(())` is a refused spawn, never a trap, and the region is reclaimed; our capability to it is
/// deleted either way (§32 (a supervisor may collect a corpse without being able to build one)).
/// See notes/system-initializer/children.md.
#[inline(never)]
pub(super) fn build_graphical_terminal_session(
    l: &GraphicalTerminalLaunch,
    s: &GraphicalTerminalCaps,
    d: &SessionDiscipline,
) -> Result<(), ()> {
    let built = graphical_terminal_session_children(l, s, d);
    // A session that came up is collected by its job's reap; one that did not gets no death.
    if built.is_err() {
        reclaim(l.region);
    }
    cap_delete(l.region);
    built
}

/// [`build_graphical_terminal_session`]'s build, separated from it so the region's disposal is written once
/// rather than at every failure site. Never inlined, for the wrapper's own reason.
#[inline(never)]
fn graphical_terminal_session_children(
    l: &GraphicalTerminalLaunch,
    s: &GraphicalTerminalCaps,
    d: &SessionDiscipline,
) -> Result<(), ()> {
    let GraphicalTerminalLaunch {
        boot_terminal, kbd, ..
    } = *l;
    let (Some(gpu_program), Some(disp_program)) = (&s.driver, &s.terminal) else {
        return Err(());
    };
    // Before anything is retyped: a keyboard with no driver refuses the session.
    if kbd.is_some() && s.kbd_driver.is_none() {
        return Err(());
    }
    let (display, driver_report) = start_gpu_driver(l, gpu_program)?;

    let (term, out) = start_display_terminal(l, disp_program, display, driver_report)?;

    let arm = match kbd {
        Some(k) => start_keyboard_arm(l, &s.kbd_driver, d, k, (term, out))?,
        // Only the raw half: a copy badged `proto::RAW_ONLY_BADGE`, so the session cannot send a
        // line the boot shell would run with its own authority.
        None => {
            let raw = user_mode_runtime::badge(boot_terminal, proto::RAW_ONLY_BADGE);
            let Ok(raw) = u64::try_from(raw) else {
                drop_caps(&[term, out]);
                return Err(());
            };
            (1, raw, out, None, term)
        }
    };
    start_session_program(l, arm, (term, out))
}

/// A session's keystroke arm, as the session program is wired from it: `(arm, keys_ep, out_page,
/// in_page, write_ep)`, where `arm` is the program's `x0` and `in_page` exists only on arm 0.
type SessionArm = (u64, u64, u64, Option<u64>, u64);

/// Build and start `gpu_driver` from the session's region, on `components/src/gpu_driver.rs`'s
/// slots 0 to 5, and wait for its `UP`. On success the display endpoint and the driver's report
/// endpoint, which the terminal phase needs next.
fn start_gpu_driver(l: &GraphicalTerminalLaunch, gpu_program: &elf::Elf) -> Result<(u64, u64), ()> {
    let GraphicalTerminalLaunch {
        own_ut,
        region,
        gpu,
        ..
    } = *l;
    let Some(display) = retype_obj(region, abi::objtype::RENDEZVOUS).ok() else {
        return Err(());
    };
    let Some(driver_report) = retype_obj(region, abi::objtype::RENDEZVOUS).ok() else {
        drop_caps(&[display]);
        return Err(());
    };
    let Ok(budget) = memory_region_split(region, GRAPHICAL_MAP_BUDGET_PAGES) else {
        drop_caps(&[display, driver_report]);
        return Err(());
    };
    let Some(driver) = build_child(
        own_ut,
        region,
        gpu_program,
        &ChildEndowment {
            caps: &[
                (driver_report, abi::rights::WRITE),
                (gpu[1], abi::rights::READ),
                (gpu[0], abi::rights::WRITE),
                (display, abi::rights::READ),
                (budget, abi::rights::SPEND_ONLY),
                (gpu[2], abi::rights::READ | abi::rights::WRITE),
            ],
            ..ChildEndowment::new(Retention::Nothing)
        },
    )
    .ok() else {
        drop_caps(&[display, driver_report, budget]);
        return Err(());
    };
    cap_delete(budget);
    // Role 0, the honest driver. No physical address: the run's first page carries it
    // (`abi::virtio::DMA_PHYS_OFFSET`), and this process could not supply one anyway.
    if !start_child(driver, 0, 0, 0) {
        drop_caps(&[display, driver_report]);
        return Err(());
    }
    let (up, _, _) = receive(driver_report);
    if up != graphics_contract::status::UP {
        drop_caps(&[display, driver_report]);
        return Err(());
    }
    Ok((display, driver_report))
}

/// Build and start `display_terminal` on `components/src/display_terminal.rs`'s slots 0 to 5,
/// then wait for its `TERM_UP` and the driver's `FLUSHED`. Consumes the display and report
/// endpoints; on success the terminal's endpoint and write page.
fn start_display_terminal(
    l: &GraphicalTerminalLaunch,
    disp_program: &elf::Elf,
    display: u64,
    driver_report: u64,
) -> Result<(u64, u64), ()> {
    let GraphicalTerminalLaunch {
        own_ut,
        region,
        gpu,
        ..
    } = *l;
    let Some(term_report) = retype_obj(region, abi::objtype::RENDEZVOUS).ok() else {
        drop_caps(&[display, driver_report]);
        return Err(());
    };
    let Some(term) = retype_obj(region, abi::objtype::RENDEZVOUS).ok() else {
        drop_caps(&[display, driver_report, term_report]);
        return Err(());
    };
    let Some(out) = retype_page_frame(region).ok() else {
        drop_caps(&[display, driver_report, term_report, term]);
        return Err(());
    };
    let Ok(budget) = memory_region_split(region, GRAPHICAL_MAP_BUDGET_PAGES) else {
        drop_caps(&[display, driver_report, term_report, term, out]);
        return Err(());
    };
    let Some(terminal) = build_child(
        own_ut,
        region,
        disp_program,
        &ChildEndowment {
            caps: &[
                (term_report, abi::rights::WRITE),
                (display, abi::rights::WRITE),
                (term, abi::rights::READ),
                (budget, abi::rights::SPEND_ONLY),
                (gpu[3], abi::rights::READ | abi::rights::WRITE),
                (out, abi::rights::READ | abi::rights::WRITE),
            ],
            ..ChildEndowment::new(Retention::Nothing)
        },
    )
    .ok() else {
        drop_caps(&[display, driver_report, term_report, term, out, budget]);
        return Err(());
    };
    cap_delete(budget);
    cap_delete(display);
    if !start_child(terminal, video_terminal::status::MODE_DISPLAY, 0, 0) {
        drop_caps(&[driver_report, term_report, term, out]);
        return Err(());
    }
    let (tag, _, _) = receive(term_report);
    if tag != video_terminal::status::TERM_UP {
        drop_caps(&[driver_report, term_report, term, out]);
        return Err(());
    }
    cap_delete(term_report);
    let (tag, _, _) = receive(driver_report);
    if tag != graphics_contract::status::FLUSHED {
        drop_caps(&[driver_report, term, out]);
        return Err(());
    }
    cap_delete(driver_report);
    Ok((term, out))
}

/// Arm 0: build the session's own line discipline (`line_editor` in `MODE_DISPLAY`) and start
/// `keyboard_driver` behind it in `MODE_DIRECT`, waiting for its `KEYBOARD_UP`. On failure `term`
/// and `out` are dropped with everything made here.
fn start_keyboard_arm(
    l: &GraphicalTerminalLaunch,
    kbd_driver: &Option<elf::Elf<'static>>,
    d: &SessionDiscipline,
    k: &[u64; 3],
    (term, out): (u64, u64),
) -> Result<SessionArm, ()> {
    let GraphicalTerminalLaunch { own_ut, region, .. } = *l;
    let Some(session_term) = retype_obj(region, abi::objtype::RENDEZVOUS).ok() else {
        drop_caps(&[term, out]);
        return Err(());
    };
    let Some(session_out) = retype_page_frame(region).ok() else {
        drop_caps(&[term, out, session_term]);
        return Err(());
    };
    let Some(session_in) = retype_page_frame(region).ok() else {
        drop_caps(&[term, out, session_term, session_out]);
        return Err(());
    };
    build_terminal(
        own_ut,
        region,
        &d.supervisor,
        d.discipline,
        LINE_EDITOR_MODE_DISPLAY,
        &[
            (
                session_term,
                abi::rights::READ | abi::rights::WRITE | abi::rights::GRANT,
            ),
            (term, abi::rights::WRITE | abi::rights::GRANT),
            (out, abi::rights::READ | abi::rights::WRITE),
            (session_out, abi::rights::READ),
            (session_in, abi::rights::READ | abi::rights::WRITE),
        ],
    );
    let Some(report) = retype_obj(region, abi::objtype::RENDEZVOUS).ok() else {
        drop_caps(&[term, out, session_term, session_out, session_in]);
        return Err(());
    };
    // The guard in `graphical_terminal_session_children` saw this `Some`.
    let kbd_program = kbd_driver.as_ref().unwrap();
    let Some(driver) = build_child(
        own_ut,
        region,
        kbd_program,
        &ChildEndowment {
            caps: &[
                (report, abi::rights::WRITE),
                (k[1], abi::rights::READ),
                (k[0], abi::rights::WRITE),
                (session_term, abi::rights::WRITE),
            ],
            maps: &[(KBD_DMA_VA, k[2], abi::address_space::MAP_RW)],
            ..ChildEndowment::new(Retention::Nothing)
        },
    )
    .ok() else {
        drop_caps(&[term, out, session_term, session_out, session_in, report]);
        return Err(());
    };
    if !start_child(driver, KBD_MODE_DIRECT, 0, 0) {
        drop_caps(&[term, out, session_term, session_out, session_in, report]);
        return Err(());
    }
    let (tag, _, _) = receive(report);
    if tag != video_terminal::status::KEYBOARD_UP {
        drop_caps(&[term, out, session_term, session_out, session_in, report]);
        return Err(());
    }
    cap_delete(report);
    Ok((0, session_term, session_out, Some(session_in), session_term))
}

/// Build and start the `graphical_terminal` program on its arm's wiring, then drop the wiring's
/// means, whether or not the build worked.
fn start_session_program(
    l: &GraphicalTerminalLaunch,
    (arm, keys_ep, out_page, in_page, write_ep): SessionArm,
    (term, out): (u64, u64),
) -> Result<(), ()> {
    let GraphicalTerminalLaunch {
        own_ut,
        region,
        deaths,
        result_ep,
        program: screen_elf,
        ..
    } = *l;
    // `components/src/graphical_terminal.rs`'s slots 0 to 2; the line page only on arm 0.
    let mut maps = [
        (SCREEN_OUT_VA, out_page, abi::address_space::MAP_RW),
        (0, 0, 0),
    ];
    let n_maps = match in_page {
        Some(p) => {
            maps[1] = (SCREEN_IN_VA, p, abi::address_space::MAP_RO);
            2
        }
        None => 1,
    };
    let Some(session) = build_child(
        own_ut,
        region,
        screen_elf,
        &ChildEndowment {
            caps: &[
                (result_ep, abi::rights::WRITE),
                (write_ep, abi::rights::WRITE),
                (keys_ep, abi::rights::WRITE),
            ],
            maps: &maps[..n_maps],
            fault: Some(deaths),
            ..ChildEndowment::new(Retention::Nothing)
        },
    )
    .ok() else {
        drop_caps(&[term, out, keys_ep]);
        if let Some(p) = in_page {
            drop_caps(&[out_page, p]);
        }
        return Err(());
    };
    // Never `boot_terminal`: it is our only copy, kept for the next session.
    drop_caps(&[term, out, keys_ep]);
    if let Some(p) = in_page {
        drop_caps(&[out_page, p]);
    }
    if !start_child(session, arm, 0, 0) {
        return Err(());
    }
    Ok(())
}

/// Write `addr` (big-endian octets in the low 32 bits) as dotted decimal at the front of `out`;
/// the length. `out` must hold fifteen bytes.
pub(super) fn render_ipv4(addr: u32, out: &mut [u8]) -> usize {
    let mut n = 0;
    for (i, octet) in addr.to_be_bytes().into_iter().enumerate() {
        if i > 0 {
            out[n] = b'.';
            n += 1;
        }
        let mut started = false;
        for div in [100u8, 10, 1] {
            let d = octet / div % 10;
            if d != 0 || started || div == 1 {
                out[n] = b'0' + d;
                n += 1;
                started = true;
            }
        }
    }
    n
}

/// **Build the terminal: `terminal_supervisor`, which builds `line_editor` from `td_bytes`.**
/// `caps` is in `line_editor::component::supervisor`'s slot order after its budget, and is copied,
/// not moved. `ut` is where the loader's tables come from and `budget` is the region the supervisor
/// is made of: the same root at boot, `own_ut` and the job region in a session. No swap endpoint is
/// passed, because nothing yet asks the supervisor to swap (§308 (the installer asks the terminal
/// to swap) is ruled, milestone 694 (the installer asks the terminal to swap) builds it), so it
/// gets `BUDGET_PAGES` and no copy of its own image.
pub(super) fn build_terminal(
    ut: u64,
    budget: u64,
    ts_elf: &elf::Elf,
    td_bytes: &[u8],
    mode: u64,
    caps: &[(u64, u64)],
) {
    use line_editor::component::supervisor as s;
    let budget_pages = must(memory_region_split(budget, s::BUDGET_PAGES));
    let mut all = [(0u64, 0u64); 8];
    all[0] = (budget_pages, abi::rights::ALL);
    all[1..=caps.len()].copy_from_slice(caps);
    let sup = must(build_child(
        ut,
        budget,
        ts_elf,
        &ChildEndowment {
            caps: &all[..=caps.len()],
            blobs: &[(s::ELF_VA, td_bytes)],
            stack_pages: CHILD_STACK_PAGES,
            ..ChildEndowment::new(Retention::Nothing)
        },
    ));
    let Some(lengths) = s::image_lengths(td_bytes.len() as u64, 0, 0) else {
        fail()
    };
    must_ok(start_child(sup, mode, lengths, 0));
    cap_delete(budget_pages);
}

pub(super) fn memory_region_split(ut: u64, pages: u64) -> Result<u64, ()> {
    supervision_protocol::memory_region_split(ut, pages).map_err(|_| ())
}
