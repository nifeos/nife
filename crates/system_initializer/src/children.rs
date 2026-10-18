//! The builders: the thin shapes over the ABI that make one child each (a grant, a caretaker, the
//! network stack, a graphical terminal session, the terminal), and `reclaim`. Moved out of `lib.rs`
//! unchanged but for visibility by milestone 843 (the system-initializer crate root is split along
//! its seams), under §279 (the system-initializer crate root splits into private modules).
//!
//! Name: provisional (milestone 843's lane, 2026-10-11 UTC).

use super::*;

// -------------------------------------------------------------------------------------------
// The thin shapes over the ABI. The loader itself is `supervision_protocol`'s, which is the tree's
// only one since milestone 96.
// -------------------------------------------------------------------------------------------

/// **Build the caretaker a directory grant is delivered by**, and hand back its endpoint.
///
/// `fs_subtree_caretaker` for a directory, or `fs_nameset_caretaker` with its name set on a page
/// from the job's region when the shell sent one (milestone 205 (how a foreign program is told what
/// to do)'s designation half, `spawnproto::NAMESET_BIT`). A set that arrives for a program that does
/// not hear words is refused with the grant.
///
/// **A program that hears words gets the rights its endowed manifest allows, and no more** (§170
/// clause 4): the shell asked with the rights of the note it read, and this clamps them to the
/// manifest decided here, which for bytes nobody vouched for is read-only
/// (`grant_plan::UNVOUCHED_STD_MANIFEST`).
///
/// **Out of line on purpose.** `spawn_service`'s frame is on the stack under every activation, and
/// an install of `uptime` runs `package_archive`'s parser beneath it; in a debug build that path
/// sat within about 400 bytes of the progenitor's 32 KiB stack (kernel `INIT_STACK_PAGES`) until
/// these locals were moved here. CI found it on 2026-09-27 as a data abort one word below the
/// stack's lowest page.
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
    let words = match manifest.map(|m| m.arg) {
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
    let hears_words = manifest.is_some_and(|m| m.arg.hears_words());
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

/// **Build a `fs_subtree_caretaker` for one directory grant and hand back the narrowed endpoint**
/// (milestone 31 phase 3, DECISIONS §92).
///
/// This is the whole of what "the command line is a grant expression" needed and did not have. The
/// FS service's unit of authority is a *directory* (§27) and `rm rmtree/rm-keep` says less than that,
/// so the narrowing is a **caretaker**: a process that holds the file service, descends once into the
/// granted directory asking for exactly the granted rights, and serves the same contract on an
/// endpoint of its own. The program then holds that endpoint and **nothing that names the FS
/// server**, so "it cannot reach a sibling directory" is a property of its capability table rather than of a
/// branch it is trusted to take.
///
/// `region` is the client's region, and everything here comes out of it; see
/// [`DIR_JOB_REGION_PAGES`] for why that is the lifetime rule rather than a convenience.
/// `care_words` are the caretaker's three `START` words exactly as the shell packed them.
///
/// # The handshake is what makes this safe to call from the progenitor
///
/// The progenitor has no second thread: it is this loop, and a `RECEIVE` that never completes is a machine that
/// never takes another command. So the readiness endpoint is not an optimization, it is the thing
/// that bounds this call, and `fs_subtree_caretaker` answers `DESCENT_REFUSED` rather than trapping
/// precisely so that `rm nosuchdir/x` costs a refusal instead of the prompt.
///
/// # BUGS
///
/// **A caretaker that dies before it answers still parks the progenitor.** The handshake covers the refusal a
/// person can cause by typing a name that is not there; it does not cover an image whose caretaker
/// faults on its own stack, because a corpse sends nothing. Nothing in the ABI offers a receive with
/// a deadline, and giving the progenitor one would mean a second thread inside the process this system's whole
/// design keeps small. The exposure is a build defect rather than an input, and it is the same one
/// `kernel::user::fs_service::wait_for_caretaker` has carried since milestone 47.
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
    // The shared page, and the name set read-only where `fs_nameset_caretaker` reads it (milestone
    // 205), when there is one.
    let maps = [
        (FS_CLIENT_PAGE_VA, fs.page, abi::address_space::MAP_RW),
        (
            NAMESET_PAGE_VA,
            set.unwrap_or(0),
            abi::address_space::MAP_RO,
        ),
    ];
    // Its whole authority, and reading these three lines is reading it: the file service to
    // attenuate, the endpoint it will serve, and one place to say it is ready. No untyped, no clock,
    // no terminal, and nothing that could name another process.
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
    // **Deliberately unsupervised** (`fault: None`). A caretaker built into the client's region is
    // already collected by the client's reap, and giving it a fault slot on `deaths` would put a
    // second death message on that endpoint for a thread whose region the first reap had already
    // taken away: `job_undertaker` would then be asked to collect a tid the scheduler no longer
    // knows, which it reads as the kernel contradicting itself and traps on.
    let child = built.ok()?;
    let started = start_child(child, care_words.0, care_words.1, care_words.2);
    if !started {
        cap_delete(ready);
        cap_delete(narrow_ep);
        return None;
    }
    // The one bounded wait. `READY` means the descent succeeded and everything the client can reach
    // it will reach through the handle that one request minted.
    let (verdict, _, _) = receive(ready);
    cap_delete(ready);
    if verdict == filesystem_contract::READY {
        Some(narrow_ep)
    } else {
        // A refused descent. The caretaker has already exited; the endpoint goes back, and the
        // caller answers the prompt without building the program.
        cap_delete(narrow_ep);
        None
    }
}

/// **Give a failed job's region back, retrying while something in it can still run.**
///
/// A `DESTROY` of a region holding a live thread is refused with §16's kill armed, and one
/// preemption later the retry succeeds; that is `sched::reclaim_region`'s documented contract and it
/// is why the shell's `^C` escalation is a loop. It reaches this path when a directory grant's
/// caretaker was built and the program behind it was not: the caretaker is parked in `RECEIVE`, the
/// endpoint sweep wakes it, and a single attempt would leave [`DIR_JOB_REGION_PAGES`] spoken for
/// until the machine stops. That is the *out of memory* path, which is exactly where a leak hurts
/// most.
///
/// Bounded and silent on failure, which is not the same call `job_undertaker` makes and the
/// difference is who is running: that program's whole job is collecting, so giving up there means a
/// leak nobody will see and trapping is the loud answer. This is the spawn service, and a trap here
/// takes the prompt down over one command's memory. The pool is bounded and renewable, so a region
/// this could not reclaim costs later commands and does not end them.
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
/// **One preemption is enough and sixty-four attempts do not reliably buy one**, which is a
/// different claim from the one above and the measurement is not this crate's. `login_test_client`
/// carried this exact loop, over the same refusal, and it was measured on 2026-08-27 (see that
/// program's `destroy_with_retry` and notes/load-sensitive-assertions.md): a `yield_now` that finds
/// work on this core returns in about 130 us, so sixty-four of them can elapse in 8 ms, where the
/// preemption being waited for is a whole tick period (10 ms) away. Whether the count covers the
/// tick is a scheduling outcome the host decides, and at roughly 2x oversubscription it stopped
/// covering it in about one run in three. That client now waits on the property with a clock; this
/// loop was left as it is, deliberately, because the two failures are not the same size: a test
/// client that gives up early fails a run, and this gives up quietly and strands
/// [`DIR_JOB_REGION_PAGES`] until the machine stops. **The failure has never been observed here**,
/// and it would be invisible if it happened, which is the argument for a lane rather than for a
/// number: this wants the same clock-bounded wait and a way to notice when it expires.
const RECLAIM_ATTEMPTS: usize = 64;

/// Carve `pages` off `ut` into a new child untyped we can delegate (milestone 31). The SPLIT grants
/// full rights on the child, including GRANT, so a memory budget can be handed on. The error code
/// `supervision_protocol` returns is for the dropped-authority proof, which this crate takes from the
/// raw `invoke` instead, so it is dropped here.
/// **Build `net_stack` from the NIC trio and wait for its DHCP lease** (milestone 590
/// (provisional)). Returns `(stack, lease, name server)`: the full-rights endpoint its clients
/// `CALL`, which this process keeps, the IPv4 address `net_stack` reported, big-endian in the low
/// 32 bits, and the lease's name server in `socket_contract::lease`'s encoding.
///
/// The endowment is `kernel::user::virtio_service::wire_net_server`'s, slot for slot, because that
/// is the one every network test proves: report `WRITE` (0), interrupt `READ` (1), transport
/// `WRITE` (2), heap budget `WRITE` (3), client endpoint `READ` (4), the DMA page at
/// [`NET_DMA_VA`], and `socket_contract::NO_LISTEN_GRANT` as its third argument. The caller
/// deletes the trio; this deletes everything else it made except the endpoint it returns.
///
/// Failures trap, as the entropy build's do: every step here is a retype or a build out of this
/// process's own budget, and one failing means the boot's arithmetic is wrong, which is a defect to
/// see rather than a feature to go without.
pub(super) fn build_net_stack(ut: u64, program: &elf::Elf, g: &BootEndowment) -> (u64, u64, u64) {
    // SAFETY: `invoke` traps to the kernel, which validates the capability and the method.
    let mapped = unsafe {
        invoke(
            g.virtio_net_dma,
            abi::page_frame::MAP,
            NET_DMA_PEEK_VA,
            1,
            ut,
        )
    } == 0;
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
            // §255 (each socket is its own capability): slot 7, our endpoint again with `WRITE |
            // GRANT`, which each socket's capability is minted from; slot 8, its own address
            // space, so `CLOSE` can take a socket's page back. `socket_contract::stack_slots`.
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
    // `net_stack`'s first and only message on this endpoint: the lease, sent with a blocking
    // `send`, so it enters its serve loop only once this receive has taken it.
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

/// **What the boot says about its network** (milestones 590 (the booted system starts its network
/// stack) and 892): the lease `net_stack` reported, so the line is evidence rather than an
/// intention, and the server the resolver asks, which is where every name a program at this prompt
/// asks will go. Silent on a boot with no network. Split out of [`boot`](crate::boot) to keep it
/// under its length ratchet.
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

/// **Build `domain_name_resolver` on the booted system's stack** (milestone 892 (a std program at the
/// prompt holds the network and a resolver)), told to ask `server`, the name server the DHCP
/// lease named, on port 53 over UDP (and over TCP when a reply is truncated). Returns its unbadged
/// endpoint, which this process keeps for the life of the boot: it mints each resolving job's
/// badged copy from it, and sends the grant and the forget through it.
///
/// The endowment is the kernel harness's (`name_resolver_tests::start_resolver`), slot for slot:
/// its endpoint `READ` (0), the stack `WRITE` (1), a budget (2), the entropy service `WRITE` (3),
/// and its own address space `WRITE` (4) for a forget's `UNMAP`. What differs is the server: the
/// harness names the runners' peer, and this names whatever the lease said.
///
/// Failures trap, [`build_net_stack`]'s reason.
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
                (2, budget, abi::rights::SPEND_ONLY),
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

/// **What a `graphical_terminal` session is made of, measured, and whether this boot can launch one**:
/// [`boot`]'s verdict (milestone 632 (provisional)). `held` means the kernel granted a gpu and
/// the table vouches for both `gpu_driver` and `display_terminal`, so the spawn service keeps the
/// gpu's four (and the keyboard's three when `keyboard` is also true, which additionally needs its
/// grant and a vouched `keyboard_driver`) for every session it builds (milestone 715
/// (provisional): until then the shell held them). A grant no session can be built from is
/// released at once, rather than carried, which is the posture every kernel grant here already
/// takes.
///
/// Never inlined, so the verdict's temporaries stay out of [`boot`]'s frame (see the comment where
/// `boot` calls this).
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
    /// the boot (milestone 715 (provisional)). Meaningful only when `held`.
    pub(super) gpu: [u64; 4],
    /// The keyboard's three, likewise. Meaningful only when `keyboard`.
    pub(super) kbd: [u64; 3],
    driver: Option<elf::Elf<'static>>,
    terminal: Option<elf::Elf<'static>>,
    kbd_driver: Option<elf::Elf<'static>>,
}

/// The session's line discipline, which is the boot's own pair of programs: the supervisor that
/// builds `line_editor`, and the measured `line_editor` bytes it is handed as a blob (the
/// 2026-09-27 ruling that made `terminal_supervisor` what builds `line_editor`). A session's
/// discipline is therefore the boot's discipline, built twice from one measurement.
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

/// **One `graphical_terminal` session's region, in pages** (milestone 632 (provisional)): everything the
/// session is made of, so the session job's one reap ends the drivers with it, DECISIONS §92's
/// shape. Bounded from the constants rather than measured, which is the BUGS entry this comment
/// owes:
///
/// - four program-shaped children (`gpu_driver`, `display_terminal`, `keyboard_driver` on the arm
///   that builds it, and the `graphical_terminal` program itself): [`JOB_REGION_PAGES`] bounds each, being
///   sized for exactly one archived program's image, twelve stack pages, TCB, address space and
///   tables;
/// - the discipline: the supervisor, bounded by [`JOB_REGION_PAGES`] as above, plus the
///   discipline's own image it is handed as a blob (one more [`JOB_REGION_PAGES`]' worth) and
///   `line_editor::component::supervisor::BUDGET_PAGES` (112, sized for the two instances a live
///   swap needs, which a session's control-less wiring never asks for but the constant does not
///   know that);
/// - two [`GRAPHICAL_MAP_BUDGET_PAGES`] map budgets, the driver's and the terminal's;
/// - the endpoints and pair pages this wiring retypes: nine at the most.
///
/// That sums to four hundred fifty-seven, and the count was wrong. **Measured 2026-10-03 (UTC)**
/// by bisecting this constant under `script/swish-check`'s graphical launch (then `--graphical`), keyboard arm: aarch64
/// needs 490 to 493 pages and riscv64 494 to 495. At the counted 464 the session's last child
/// (`keyboard_driver`, or at 481-489 the session program itself) could not be built, the launch
/// answered `could not spawn`, and CI's graphical leg failed. Five hundred twenty-eight is the
/// riscv64 measurement plus about seven percent. Bisected on those two ISAs only: x86_64 has no
/// graphical leg (milestone 192 (a keyboard on real silicon)'s x86_64 half is unbuilt), so its need
/// is unmeasured and ungated, and that leg's arrival must re-bisect this. It comes out of the job
/// pool ([`JOBS_BUDGET_PAGES`], six hundred seventy-two), once, for the life of the session:
/// while a session runs, plain native jobs still spawn from the 144 left, and a `std` job or a
/// second session is refused, which the shell reports as `could not spawn`.
pub(super) const GRAPHICAL_TERMINAL_SESSION_PAGES: u64 = 528;

/// Drop every capability named, freeing the slots. A builder's means, spent once the children
/// hold their own narrowed copies: on the success path after the last build, and on every
/// failure path after the thing they wired was built or failed, for the ordinary spawn path's
/// own reason (a failed spawn does not cost this capability table a slot for the rest of the
/// boot).
fn drop_caps(caps: &[u64]) {
    for c in caps {
        cap_delete(*c);
    }
}

/// One `graphical_terminal` launch's everything: the resources the session is built from and with. A record
/// rather than a parameter list, because [`build_graphical_terminal_session`] and its builder take the same
/// ten facts twice, and a reader holding either call site should not have to count positions.
pub(super) struct GraphicalTerminalLaunch<'a> {
    /// The spawn service's own untyped: the loader's scratch tables for every build come from
    /// here, everything the children are made of from [`Self::region`].
    pub(super) own_ut: u64,
    /// The session's job region, split by the caller. [`build_graphical_terminal_session`] owns its disposal.
    pub(super) region: u64,
    /// The supervision endpoint every job is born under. The session program's fault slot, so its
    /// one reap sweeps the drivers with it.
    pub(super) deaths: u64,
    /// The boot discipline's endpoint: the UART arm's keystroke source (milestone 632
    /// (provisional)), the spawn service's only copy.
    pub(super) boot_terminal: u64,
    /// The shell's result endpoint, the session program's slot 0.
    pub(super) result_ep: u64,
    /// The gpu's four, the spawn service's own (milestone 715 (provisional)): lent, never consumed.
    pub(super) gpu: &'a [u64; 4],
    /// The keyboard's three, when this boot has one; lent likewise.
    pub(super) kbd: Option<&'a [u64; 3]>,
    /// The `graphical_terminal` program itself, measured.
    pub(super) program: &'a elf::Elf<'a>,
}

/// **Build a whole `graphical_terminal` session and start it** (milestone 632 (provisional)): `gpu_driver`
/// and `display_terminal` on the gpu's four capabilities (lent from this process's own), then either the session's own
/// line discipline with `keyboard_driver` behind it (the device arm, `kbd` is `Some`) or nothing
/// but the screen, with the boot line discipline's endpoint handed to the program for raw reads
/// (the UART arm, milestone 192 (a keyboard on real silicon)'s option A at launch). Everything
/// the kernel's own wiring gave the driver and terminal pair
/// (`kernel::user::display_service::start_terminal`'s slot-for-slot shape, which
/// milestone 600 (provisional) already moved into userspace for the boot) and nothing else, so
/// neither program can tell who built it or when.
///
/// **Every one-time report is taken here, and in the order the drivers send it**:
/// `UP`, `TERM_UP`, the driver's one `FLUSHED`, and `KEYBOARD_UP`. The middle two of those are
/// the three-week hang of milestone 177's whole second flush: a report nobody receives parks its
/// sender for ever and the failure is total silence, which is exactly the property
/// `script/swish-check`'s graphical launches exist to catch, now at launch rather than at boot.
///
/// Everything is carved from `region` (the session's job region) except the loader's own scratch
/// tables (`own_ut`). **The session program is born supervised on `deaths`, exactly as any job
/// is**, and the drivers and the discipline are deliberately unsupervised for the reason
/// `build_caretaker`'s own comment records: they share the session's region, the program's reap
/// sweeps that region, and a death message for a thread the sweep already collected would have
/// `job_undertaker` trap on a tid the scheduler no longer knows. One reap, of the one job, ends
/// the session's drivers with it. The device capabilities are the spawn service's own and are
/// **not** deleted here (milestone 715 (provisional)): each driver gets a narrowed copy, and the
/// originals stay for the next session. This process's capability to the region goes back with the
/// build's other means: since §32 the reap is a method on the supervision endpoint, so nothing
/// holds a capability to a live session's memory.
///
/// `Err(())` rather than a trap, all the way down: a session that cannot come up is a refused
/// spawn (the shell says "could not spawn" and the prompt continues), where a boot that cannot
/// come up is a dead machine. The failure path reclaims the region, bounded and silent as a
/// failed ordinary spawn's is; children the sweep woke but would not collect are recorded in
/// this module's BUGS.
///
/// Never inlined, so neither its locals nor the parsed images sit in [`boot`]'s frame, which
/// lives as long as the spawn service does.
#[inline(never)]
pub(super) fn build_graphical_terminal_session(
    l: &GraphicalTerminalLaunch,
    s: &GraphicalTerminalCaps,
    d: &SessionDiscipline,
) -> Result<(), ()> {
    let built = graphical_terminal_session_children(l, s, d);
    // The region cap was only ever the means of building. A session that came up is collected
    // by its job's reap; one that did not is reclaimed here rather than left for a death that
    // will never come, the same disposal the ordinary path gives a failed spawn. Bounded and
    // silent, for that path's own reason: a region this could not take back costs later
    // commands and does not end them.
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
    // The keyboard's driver is checked before anything is retyped: the request announced a
    // keyboard, so a verdict without its program is a refusal, not a session built half-endowed.
    if kbd.is_some() && s.kbd_driver.is_none() {
        return Err(());
    }
    // --- the driver: the confined transport, the interrupt, the display endpoint's serving half,
    // a map budget and the whole DMA run. `components/src/gpu_driver.rs`'s slots 0-5. ---
    let (display, driver_report) = start_gpu_driver(l, gpu_program)?;

    // --- the terminal: `painter`'s authority plus the endpoint it serves and the page an
    // application writes into. `components/src/display_terminal.rs`'s slots 0-5. ---
    let (term, out) = start_display_terminal(l, disp_program, display, driver_report)?;

    // --- the keystroke source, and it decides the program's arm. With a keyboard (its three
    // caps came with the gpu's four): the session's own discipline, `terminal_supervisor`
    // building `line_editor` in `MODE_DISPLAY`, with `keyboard_driver`
    // `CALL`ing it in `MODE_DIRECT`. Without one: no discipline and no driver, and the program
    // reads the boot's own discipline raw, over the UART, echoing to the screen itself.
    // `components/src/graphical_terminal.rs`'s slots 0-2 and its `x0` are the contract. ---
    let arm = match kbd {
        Some(k) => start_keyboard_arm(l, &s.kbd_driver, d, k, (term, out))?,
        // **Arm 1 gets the raw half of the boot discipline, not the discipline** (milestone 709
        // (a graphical terminal session on the no-keyboard arm holds only the raw half of the boot
        // discipline)). A copy of `boot_terminal` badged `proto::RAW_ONLY_BADGE`, which the
        // discipline answers `OPERATION_RAWMODE` and `OPERATION_READRAW` on and refuses everything else, so the
        // session cannot `OPERATION_BYTES` a line the boot shell then runs with the shell's authority.
        // Minted per launch and deleted below once the session holds its narrowed copy; one slot
        // for the length of the build, under arm 0's peak (its discipline, pages and report).
        None => {
            // SAFETY: the syscall; the kernel checks `GRANT` on `boot_terminal` and that it is
            // unbadged, and refuses a zero badge.
            let raw = unsafe {
                invoke(
                    boot_terminal,
                    abi::rendezvous::BADGE,
                    proto::RAW_ONLY_BADGE,
                    0,
                    0,
                )
            };
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

/// Build and start `gpu_driver` from the session's region and wait for its `UP`. On success the
/// display endpoint and the driver's report endpoint, which the terminal phase needs next.
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

/// Build and start `display_terminal`, wait for its `TERM_UP` and then the driver's `FLUSHED`.
/// Consumes the display and report endpoints; on success the terminal's endpoint and write page.
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

/// Arm 0: build the session's own line discipline and start `keyboard_driver` behind it, waiting
/// for its `KEYBOARD_UP`. On failure `term` and `out` are dropped with everything made here.
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
    // `s.kbd_driver` was checked before anything was retyped, so this is the
    // same `Some` the guard saw.
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
    // --- the session program itself, born supervised exactly as any job is (`deaths` in the
    // reserved fault slot, where `START` reads it and clears it), so its one reap, by
    // `job_undertaker`, sweeps the drivers with it. Slot 0 is its answer word; slot 1 prints to
    // the screen; slot 2 owns the keystrokes. The maps are one or two pages: the write page
    // always, the line-delivery page only on the discipline arm, where there is a discipline to
    // deliver lines. ---
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
    // The wiring's means are spent: the children hold their own narrowed copies, ours were only
    // the way the wires got strung. `keys_ep` is one of them on both arms (arm 0's session
    // discipline, arm 1's badged copy). `boot_terminal` itself is *not*: it is this process's only
    // copy, held for the next session this boot launches.
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

/// **Build the terminal: `terminal_supervisor`, which builds `line_editor`** (milestone 23 (a
/// capability-routed component OS with live replacement), calef's ruling of 2026-09-27). `caps` is
/// in `line_editor::component::supervisor`'s slot order after its budget: the terminal endpoint
/// with every right (it delegates `READ` and sends `OPERATION_QUIESCE`), the output sink, the three pages,
/// and the console's reply endpoint in console mode. Copies, not moves: this process frees its own
/// as it always did.
///
/// No swap endpoint is passed: the trigger (milestone 198 (a package manager, and the trivial
/// install)'s installer activating a new build) is not built, so nothing on this boot asks the
/// supervisor to swap. `terminal_supervisor`'s `BUGS`.
/// `ut` is where the loader's own scratch tables come from; `budget` is the region the
/// supervisor's map budget is split from. The same region at boot (the root, before the giveaway),
/// two different ones inside a `graphical_terminal` session (milestone 632 (provisional): `own_ut` for the
/// tables, the session's job region for everything the supervisor is made of, so its reclaim
/// returns it).
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
    must_ok(start_child(sup, mode, td_bytes.len() as u64, 0));
    cap_delete(budget_pages);
}

pub(super) fn memory_region_split(ut: u64, pages: u64) -> Result<u64, ()> {
    supervision_protocol::memory_region_split(ut, pages).map_err(|_| ())
}
