//! The steps [`boot`] builds the interactive system in: reading and measuring the archive, the
//! kernel-granted device services, the terminal stack, the shell, the login stack and the budget
//! giveaway, with the password helpers the login stack calls. Each step is never inlined, so its
//! temporaries stay out of `boot`'s frame, which is live for the whole boot because the spawn
//! service runs inside it. The order of the steps is the capability table's budget; see
//! notes/system-initializer/the-boot-order.md.
//!
//! Name: provisional (long-function paydown lane, 2026-10-12 UTC).

use super::*;

/// Draw `out.len()` bytes from the entropy service through `request`, at most
/// [`entropy_contract::MAX_BYTES`] at a time, as `components/src/credentialer.rs`'s `fill` does.
/// `false` when the service could not supply them all; the caller then generates no password.
fn fill_entropy(request: u64, out: &mut [u8]) -> bool {
    let mut done = 0;
    while done < out.len() {
        let want = (out.len() - done).min(entropy_contract::MAX_BYTES as usize);
        let (r0, r1) = call(
            request,
            entropy_contract::req(entropy_contract::GET, want as u64),
            0,
        );
        let Some(n) = entropy_contract::delivered(r0) else {
            return false;
        };
        if n < want {
            return false;
        }
        done += entropy_contract::take(n, r1, &mut out[done..]);
    }
    true
}

/// Hex-encode `bytes` into `out` (`out.len() == 2 * bytes.len()`), lowercase, so a generated
/// password is printable and typeable.
fn hex_password(bytes: &[u8], out: &mut [u8]) {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    for (i, &b) in bytes.iter().enumerate() {
        out[i * 2] = DIGITS[(b >> 4) as usize];
        out[i * 2 + 1] = DIGITS[(b & 0xf) as usize];
    }
}

/// A child's capability grants and page mappings, the two slices a `ChildEndowment` takes as `caps`
/// and `maps`, so the `x86_64` split of the input endowment is one `let` per branch. `'a` ties the
/// slices to the `let`'s block.
type EndowmentSlices<'a> = (&'a [(u64, u64)], &'a [(u64, u64, u64)]);

/// The programs and blobs [`boot`] reads out of the archive in its one measured pass.
pub(super) struct BootPrograms {
    pub(super) catalog: &'static str,
    /// The image's index sources, measured (milestone 801); empty as the catalog is.
    pub(super) index_sources: &'static str,
    pub(super) con_elf: measured_boot::Verdict<'static>,
    pub(super) log_elf: Option<elf::Elf<'static>>,
    pub(super) in_elf: measured_boot::Verdict<'static>,
    pub(super) td_elf: measured_boot::Verdict<'static>,
    pub(super) ts_elf: measured_boot::Verdict<'static>,
    pub(super) sh_elf: measured_boot::Verdict<'static>,
    pub(super) sink_elf: Option<elf::Elf<'static>>,
    pub(super) ent_elf: Option<elf::Elf<'static>>,
    pub(super) net_elf: Option<elf::Elf<'static>>,
    pub(super) login: LoginPrograms,
    pub(super) reaper_elf: measured_boot::Verdict<'static>,
    pub(super) care_elf: Option<elf::Elf<'static>>,
    pub(super) set_elf: Option<elf::Elf<'static>>,
    pub(super) fs_scoped: bool,
}

/// What the login stack is built from: its four programs and the three blobs `login` is handed.
pub(super) struct LoginPrograms {
    cred_elf: Option<elf::Elf<'static>>,
    idp_elf: Option<elf::Elf<'static>>,
    login_elf: Option<elf::Elf<'static>>,
    audit_elf: Option<elf::Elf<'static>>,
    care_blob: &'static [u8],
    keeper_blob: &'static [u8],
    timetable_blob: &'static [u8],
}

/// The programs the shell can spawn, and the names the measurement refused among them.
pub(super) struct Spawnable {
    pub(super) progs: [Option<elf::Elf<'static>>; grant_plan::PROGRAM_COUNT],
    /// Which programs this initial ramdisk has no entry for, so a spawn of one is answered
    /// [`grant_plan::spawnproto::SPAWN_NOT_IN_IMAGE`] rather than as a build failure.
    pub(super) absent: [bool; grant_plan::PROGRAM_COUNT],
    pub(super) refused: [&'static str; grant_plan::PROGRAM_COUNT],
    pub(super) refused_n: usize,
}

/// What the entropy step decided, carried to the announcement and the spawn service.
pub(super) struct Entropy {
    pub(super) ready: bool,
    pub(super) from_instruction: bool,
    pub(super) client: Option<u64>,
}

/// The kernel-granted devices' outcome: the graphical terminal verdict (`screen`, as [`Channels`]
/// names it), whether the firmware's screen has a terminal, entropy, and the network.
pub(super) struct DeviceServices {
    pub(super) screen: GraphicalTerminalCaps,
    pub(super) has_screen: bool,
    pub(super) entropy: Entropy,
    /// `(stack, lease, the lease's name server)`.
    pub(super) network: Option<(u64, u64, u64)>,
    /// The name resolver's unbadged endpoint, if this boot started one (milestone 892).
    pub(super) resolver: Option<u64>,
}

/// The terminal contract's endpoint and the shell's two pages, which the progenitor retypes.
#[derive(Clone, Copy)]
pub(super) struct Terminal {
    pub(super) ep: u64,
    pub(super) out: u64,
    pub(super) input: u64,
}

/// The spawn channel and the supervision endpoint every job is born holding.
#[derive(Clone, Copy)]
pub(super) struct SpawnChannel {
    pub(super) spawn_ep: u64,
    pub(super) result_ep: u64,
    pub(super) deaths: u64,
}

/// What the steps after the terminal share: the endowment, the construction budget, the terminal,
/// and whether this boot has a filesystem.
pub(super) struct Wiring<'g> {
    pub(super) g: &'g BootEndowment,
    pub(super) ut: u64,
    pub(super) term: Terminal,
    pub(super) with_fs: bool,
}

/// The login stack's outcome, said later at the print site, and the run-unvouched endpoint.
pub(super) struct Login {
    pub(super) ready: bool,
    pub(super) password: [u8; PASSWORD_HEX_LEN],
    pub(super) run_unvouched: Option<u64>,
}

/// What the progenitor keeps past the giveaway: the run-unvouched endpoint it placed in the shell
/// and three bounded budgets.
pub(super) struct Kept {
    pub(super) run_unvouched: u64,
    pub(super) own_ut: u64,
    pub(super) jobs_ut: u64,
    pub(super) images_ut: u64,
}

/// Borrow the archive the kernel mapped, give back the loader's scratch pages, and read the
/// measurement table. Returns the archive and the table, which is empty, and so vouches for
/// nothing, when its bytes are not UTF-8. See notes/system-initializer/the-boot-order.md.
#[inline(never)]
pub(super) fn open_archive(
    g: &BootEndowment,
    initial_ramdisk_len: u64,
) -> (nifefs::Fs<'static>, &'static str) {
    let len = initial_ramdisk_len as usize;
    // SAFETY: the kernel mapped `initial_ramdisk_len` bytes of reserved RAM, read-only, at
    // INITIAL_RAMDISK_VA. It is reserved memory that outlives every process, so the borrow is
    // honest for the whole boot.
    let archive: &'static [u8] =
        unsafe { core::slice::from_raw_parts(INITIAL_RAMDISK_VA as *const u8, len) };
    let Ok(fs) = nifefs::Fs::parse(archive) else {
        fail()
    };

    // Before anything is built, so no boot server's page is ever left mapped here.
    if is_granted(g.own_space) {
        supervision_protocol::give_up_scratch_through(g.own_space);
    }

    // Bytes that are not UTF-8 are the empty table, which vouches for nothing.
    let table = fs
        .read(measured_boot::PROGRAM_MEASUREMENTS)
        .and_then(|b| core::str::from_utf8(b).ok())
        .unwrap_or("");
    (fs, table)
}

/// The measured pass: every boot component, the package catalog, the index sources, and the blobs
/// `login` is handed, read and checked against `table` while the archive is still ours. An entry
/// the table refuses reads as absent. What each absence costs is in
/// notes/system-initializer/the-boot-order.md.
#[inline(never)]
pub(super) fn read_boot_programs(fs: &nifefs::Fs<'static>, table: &str) -> BootPrograms {
    // What `package install` checks a digest against; empty vouches for no package.
    let catalog = fs
        .read(package_archive::CATALOG)
        .filter(|bytes| {
            measured_boot::verify_in_manifest(table, package_archive::CATALOG, bytes).is_ok()
        })
        .and_then(|b| core::str::from_utf8(b).ok())
        .unwrap_or("");
    // Where the image says its index is; empty names no source.
    let index_sources = fs
        .read(package_index::SOURCES_ENTRY)
        .filter(|bytes| {
            measured_boot::verify_in_manifest(table, package_index::SOURCES_ENTRY, bytes).is_ok()
        })
        .and_then(|b| core::str::from_utf8(b).ok())
        .unwrap_or("");

    let con_elf = measured(fs, table, "console");
    let log_elf = measured(fs, table, "system_log").elf;
    let in_elf = measured(fs, table, "input");
    let td_elf = measured(fs, table, "line_editor");
    let ts_elf = measured(fs, table, "terminal_supervisor");
    let sh_elf = measured(fs, table, "swish");
    let sink_elf = measured(fs, table, "terminal_sink_caretaker").elf;
    let ent_elf = measured(fs, table, "entropy").elf;
    let net_elf = measured(fs, table, "net_stack").elf;
    // The graphical stack's programs are measured in `graphical_terminal_grants`, not here: three
    // more `elf::Elf`s in `boot`'s frame overflow the progenitor's stack.

    let cred_elf = measured(fs, table, "credentialer").elf;
    let idp_elf = measured(fs, table, "identity_provisioner").elf;
    let login_elf = measured(fs, table, "login").elf;
    let audit_elf = measured(fs, table, "login_audit_receiver").elf;
    let reaper_elf = measured(fs, table, "job_undertaker");
    let care_elf = measured(fs, table, "fs_subtree_caretaker").elf;
    let set_elf = measured(fs, table, "fs_nameset_caretaker").elf;
    // `false` on any doubt, so every doubt ends in a caretaker.
    let fs_scoped = subtree_grants(measured(fs, table, "redoxfs_server").elf.as_ref());
    // `login` cannot read the archive, so it is handed the caretaker's bytes; empty when unvouched,
    // and then `login` denies every login.
    let care_blob: &[u8] = if care_elf.is_some() {
        fs.read("fs_subtree_caretaker").unwrap_or(&[])
    } else {
        &[]
    };
    // Both or neither: without them `login` answers `SCHEDULE` as a plain login.
    let (keeper_blob, timetable_blob): (&[u8], &[u8]) = match (
        measured(fs, table, "user_timetable_keeper").elf.is_some(),
        measured(fs, table, "timetable").elf.is_some(),
        fs.read("user_timetable_keeper"),
        fs.read("timetable"),
    ) {
        (true, true, Some(session), Some(timetable)) => (session, timetable),
        _ => (&[], &[]),
    };
    BootPrograms {
        catalog,
        index_sources,
        con_elf,
        log_elf,
        in_elf,
        td_elf,
        ts_elf,
        sh_elf,
        sink_elf,
        ent_elf,
        net_elf,
        login: LoginPrograms {
            cred_elf,
            idp_elf,
            login_elf,
            audit_elf,
            care_blob,
            keeper_blob,
            timetable_blob,
        },
        reaper_elf,
        care_elf,
        set_elf,
        fs_scoped,
    }
}

/// Measure and parse the programs the shell can spawn, before the giveaway, so the announcement
/// can name the refusals. Returns the programs, the ones the archive has no entry for, and the
/// names the table refused; an entry the archive lacks is never a refusal.
#[inline(never)]
pub(super) fn measure_spawnable(fs: &nifefs::Fs<'static>, table: &str) -> Spawnable {
    let mut progs: [Option<elf::Elf>; grant_plan::PROGRAM_COUNT] = core::array::from_fn(|_| None);
    let mut refused = [""; grant_plan::PROGRAM_COUNT];
    let mut refused_n = 0usize;
    let mut absent = [false; grant_plan::PROGRAM_COUNT];
    for (id, slot) in progs.iter_mut().enumerate() {
        let Some(name) = Program::from_id(id as u64).and_then(archive_name) else {
            continue;
        };
        absent[id] = fs.read(name).is_none();
        let found = measured(fs, table, name);
        if found.unvouched {
            refused[refused_n] = name;
            refused_n += 1;
        }
        *slot = found.elf;
    }
    Spawnable {
        progs,
        absent,
        refused,
        refused_n,
    }
}

/// Free the test roles' grants, then settle every kernel-granted device before the terminal
/// plumbing's peak: the display verdict, the firmware screen, entropy, the network and the name
/// resolver. Every device trio is released here, built or not. See
/// notes/system-initializer/the-boot-order.md, "The boot order is the capability table's budget".
#[inline(never)]
pub(super) fn start_device_services(
    g: &BootEndowment,
    ut: u64,
    fs: &nifefs::Fs<'static>,
    table: &str,
    ent_elf: Option<&elf::Elf<'static>>,
    net_elf: Option<&elf::Elf<'static>>,
) -> DeviceServices {
    // First: only these slots matter, and every peak below is lower without them.
    for &c in g.for_test_roles {
        cap_delete(c);
    }

    let screen = graphical_terminal_grants(fs, table, g);
    let has_screen = is_granted(g.disp_term_ep);
    // Before anything retypes: a retyped object lands in the first free slot, and then this probe
    // could see it instead of the kernel's grant.
    let kernel_built_entropy = is_granted(g.entropy_ep);

    // Entropy and the network before the terminal plumbing is retyped: their trios are kernel
    // grants, and held across the plumbing's six retypes they raise the early peak.
    let entropy = start_entropy(g, ut, ent_elf, kernel_built_entropy);
    let network = start_network(g, ut, net_elf);
    let resolver = start_domain_name_resolver((fs, table, ut), network, entropy.client);
    DeviceServices {
        screen,
        has_screen,
        entropy,
        network,
        resolver,
    }
}

/// Build the entropy service on a virtio-rng device, or take the one the kernel built on the
/// CPU's seed instruction. Neither is a failure when absent. Returns whether entropy is ready,
/// whether it came from the instruction, and the request endpoint, which is kept for the life of
/// the boot: `credentialer` and every child declaring [`grant_plan::Manifest::entropy`] get a view
/// of it. See notes/system-initializer/the-boot-order.md, "The device services".
#[inline(never)]
fn start_entropy(
    g: &BootEndowment,
    ut: u64,
    ent_elf: Option<&elf::Elf<'static>>,
    kernel_built_entropy: bool,
) -> Entropy {
    // No terminal is mapped yet, so the outcome is carried as data and announced later.
    let mut entropy_ready = false;
    let mut entropy_client: Option<u64> = None;
    if let Some(ent_program) = ent_elf {
        entropy_client = start_virtio_entropy(g, ut, ent_program);
        entropy_ready = entropy_client.is_some();
    }
    // Already proven by the kernel, and granted only when the trio was not.
    let mut entropy_from_instruction = false;
    if entropy_client.is_none() && kernel_built_entropy {
        entropy_ready = true;
        entropy_from_instruction = true;
        entropy_client = Some(g.entropy_ep);
    }
    Entropy {
        ready: entropy_ready,
        from_instruction: entropy_from_instruction,
        client: entropy_client,
    }
}

/// Probe the virtio-rng trio, build `entropy` on it and wait for its proof of life. `Some(request)`
/// exactly when the service answered `READY`; every path releases the trio, and every path but
/// that one releases `request`.
#[inline(never)]
fn start_virtio_entropy(
    g: &BootEndowment,
    ut: u64,
    ent_program: &elf::Elf<'static>,
) -> Option<u64> {
    // `NoSuchSlot` on an ungranted slot is the probe; the trio is granted whole or not at all.
    let magic = user_mode_runtime::virtio::read_register(g.virtio_rng, 0);
    if magic >= 0 {
        if map_page_frame(g.virtio_rng_dma, RNG_DMA_PEEK_VA, true, ut) {
            // SAFETY: just mapped read/write, one page, ours alone until entropy is built and
            // holds its own copy of the same frame; `VIRTIO_DMA_PHYS_OFFSET` is inside it and
            // outside entropy's own ring-and-buffer layout (that constant's own doc).
            let direct_memory_access_phys = unsafe {
                core::ptr::read_unaligned(
                    (RNG_DMA_PEEK_VA as *const u8)
                        .add(VIRTIO_DMA_PHYS_OFFSET as usize)
                        .cast::<u64>(),
                )
            };
            // Read; the page is entropy's from here, so our view of it goes (milestone 95).
            supervision_protocol::give_up_own_page(RNG_DMA_PEEK_VA);
            let request = must(retype_obj(ut, abi::objtype::RENDEZVOUS));
            let ready = must(retype_obj(ut, abi::objtype::RENDEZVOUS));
            let entropy = must(build_child(
                ut,
                ut,
                ent_program,
                &ChildEndowment {
                    caps: &[
                        (request, abi::rights::READ),
                        (g.virtio_rng_irq, abi::rights::READ),
                        (g.virtio_rng, abi::rights::WRITE),
                        (ready, abi::rights::WRITE),
                    ],
                    maps: &[(RNG_DMA_VA, g.virtio_rng_dma, abi::address_space::MAP_RW)],
                    stack_pages: CHILD_STACK_PAGES,
                    ..ChildEndowment::new(Retention::Nothing)
                },
            ));
            must_ok(start_child(
                entropy,
                RNG_MODE_VIRTIO,
                direct_memory_access_phys,
                0,
            ));
            cap_delete(g.virtio_rng_irq);
            cap_delete(g.virtio_rng);
            cap_delete(g.virtio_rng_dma);
            // `READY` comes after the service's first bufferful from the device.
            let (verdict, _, _) = receive(ready);
            cap_delete(ready);
            let entropy_ready = verdict == entropy_contract::READY;
            if entropy_ready {
                Some(request)
            } else {
                cap_delete(request);
                None
            }
        } else {
            cap_delete(g.virtio_rng_irq);
            cap_delete(g.virtio_rng);
            cap_delete(g.virtio_rng_dma);
            None
        }
    } else {
        None
    }
}

/// Build `net_stack` on a granted virtio-net device, or take the stack the kernel built on an
/// `e1000e`, and wait for its DHCP lease, which blocks the boot. `Some((endpoint, lease, name
/// server))` when the boot has a network. The NIC trio is released on every path. See
/// notes/system-initializer/the-boot-order.md, "The device services".
#[inline(never)]
fn start_network(
    g: &BootEndowment,
    ut: u64,
    net_elf: Option<&elf::Elf<'static>>,
) -> Option<(u64, u64, u64)> {
    let mut network: Option<(u64, u64, u64)> = None;
    if is_granted(g.virtio_net) {
        if let Some(net_program) = net_elf {
            network = Some(build_net_stack(ut, net_program, g));
        }
        for trio in [g.virtio_net_irq, g.virtio_net, g.virtio_net_dma] {
            cap_delete(trio);
        }
    } else if is_granted(g.net_stack_ep) {
        let (lease, server, _) = receive(g.net_stack_report);
        cap_delete(g.net_stack_report);
        network = Some((g.net_stack_ep, lease, server));
    }
    network
}

/// Retype the terminal plumbing and build what prints: the console, the system log service and
/// the line discipline. The pair is the console's two device facts: whether the kernel wired a
/// screen, and the UART's register shape (`abi::uart`). Returns the terminal endpoint, which the
/// line discipline serves and every keystroke source and the shell hold `WRITE` on
/// (notes/terminal-contract.md), and the shell's two pages.
#[inline(never)]
pub(super) fn start_terminal_stack(
    g: &BootEndowment,
    ut: u64,
    (has_screen, uart_shape): (bool, u64),
    con_elf: &elf::Elf<'static>,
    log_elf: Option<&elf::Elf<'static>>,
    ts_elf: &elf::Elf<'static>,
    td_bytes: &[u8],
) -> Terminal {
    // Retyped with full rights, so narrowed views can be delegated.
    let term_out = must(retype_page_frame(ut)); // shell -> line_editor text and prompts
    let term_in = must(retype_page_frame(ut)); // line_editor -> shell completed lines

    let term_ep = must(retype_obj(ut, abi::objtype::RENDEZVOUS));

    {
        let request = must(retype_obj(ut, abi::objtype::RENDEZVOUS));
        let reply = must(retype_obj(ut, abi::objtype::RENDEZVOUS));
        let con_shared = must(retype_page_frame(ut)); // line_editor -> console text

        start_console(
            g,
            ut,
            con_elf,
            has_screen,
            uart_shape,
            [request, reply, con_shared],
        );
        start_system_log(g, ut, log_elf, request);

        // The console's only client; everyone else prints through the line discipline.
        build_terminal(
            ut,
            ut,
            ts_elf,
            td_bytes,
            LINE_EDITOR_MODE_CONSOLE,
            &[
                (
                    term_ep,
                    abi::rights::READ | abi::rights::WRITE | abi::rights::GRANT,
                ),
                (request, abi::rights::WRITE | abi::rights::GRANT),
                (con_shared, abi::rights::READ | abi::rights::WRITE), // it fills what the console reads
                (term_out, abi::rights::READ),
                (term_in, abi::rights::READ | abi::rights::WRITE),
                (reply, abi::rights::READ | abi::rights::GRANT),
            ],
        );

        // Both children hold their own copies; freed now to lower the shell's peak.
        for c in [request, reply, con_shared] {
            cap_delete(c);
        }
    }
    Terminal {
        ep: term_ep,
        out: term_out,
        input: term_in,
    }
}

/// Build and start the console server on the UART, with the firmware screen's terminal and a
/// batching pair (a notification and a timer) beside it when `has_screen`. The UART is mapped on
/// aarch64 and riscv64 and held as COM1's port range on x86. Slot 2 is the screen's endpoint and
/// slots 3 and 4 the batching pair; the port range lands after them. The screen pair and the
/// batching pair are deleted here once the console holds them. See
/// notes/system-initializer/the-boot-order.md, "The terminal stack and the shell".
#[inline(never)]
fn start_console(
    g: &BootEndowment,
    ut: u64,
    con_elf: &elf::Elf<'static>,
    has_screen: bool,
    uart_shape: u64,
    [request, reply, con_shared]: [u64; 3],
) {
    let mut con_batch = (0, 0);
    if has_screen {
        let notified = must(retype_obj(ut, abi::objtype::NOTIFICATION));
        let timer = must(retype_obj(ut, abi::objtype::TIMER));
        con_batch = (notified, timer);
    }
    let mut con_caps = [(0, 0); 6];
    let mut con_maps = [(0, 0, 0); 3];
    let (mut ncaps, mut nmaps) = (0, 0);
    for cap in [(request, abi::rights::READ), (reply, abi::rights::WRITE)] {
        con_caps[ncaps] = cap;
        ncaps += 1;
    }
    con_maps[nmaps] = (CON_SHARED_VA, con_shared, abi::address_space::MAP_RO);
    nmaps += 1;
    if has_screen {
        con_caps[ncaps] = (g.disp_term_ep, abi::rights::WRITE);
        ncaps += 1;
        // `READ | WRITE` on the notification: WAIT/POLL and the SIGNAL an armed timer makes of
        // it. `WRITE` on the timer: ARM and CANCEL.
        con_caps[ncaps] = (con_batch.0, abi::rights::READ | abi::rights::WRITE);
        ncaps += 1;
        con_caps[ncaps] = (con_batch.1, abi::rights::WRITE);
        ncaps += 1;
        con_maps[nmaps] = (
            CON_SCREEN_OUT_VA,
            g.disp_term_page,
            abi::address_space::MAP_RW,
        );
        nmaps += 1;
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        // mode ignored for a DeviceFrame
        con_maps[nmaps] = (CON_UART_VA, g.uart_dev, abi::address_space::MAP_RO);
        nmaps += 1;
    }
    #[cfg(target_arch = "x86_64")]
    {
        con_caps[ncaps] = (g.uart_dev, abi::rights::WRITE); // COM1's port range, held not mapped
        ncaps += 1;
    }
    let con = must(build_child(
        ut,
        ut,
        con_elf,
        &ChildEndowment {
            caps: &con_caps[..ncaps],
            maps: &con_maps[..nmaps],
            stack_pages: CHILD_STACK_PAGES,
            ..ChildEndowment::new(Retention::Nothing)
        },
    ));
    // Between build and start: before the build there is no thread, and `start_child` deletes our
    // `ThreadControlBlock`.
    if has_screen {
        must_ok(user_mode_runtime::notification_bind(con_batch.0, con.tcb) >= 0);
    }
    // The second word is the UART's register shape (`abi::uart`), which the console needs before
    // its first byte: with the wrong stride it waits on a status bit at the wrong address.
    must_ok(start_child(
        con,
        if has_screen { CONSOLE_MODE_SCREEN } else { 0 },
        uart_shape,
        0,
    ));
    // The binding lives in the objects, not in our copies of them.
    if has_screen {
        cap_delete(g.disp_term_ep);
        cap_delete(g.disp_term_page);
        cap_delete(con_batch.0);
        cap_delete(con_batch.1);
    }
}

/// Build the system log service, draining the kernel's ring into the console through `request`,
/// when the archive has it and the kernel granted the ring, its cursor and its notification. Built
/// right after the console, because until it attaches the kernel prints for itself and can splice
/// with the console's bytes (§175 (where the kernel's own output goes once userspace owns the
/// console), ruling B). The ring's grants are released either way.
#[inline(never)]
fn start_system_log(g: &BootEndowment, ut: u64, log_elf: Option<&elf::Elf<'static>>, request: u64) {
    if let Some(log_program) = log_elf
        && is_granted(g.kernel_ring)
        && is_granted(g.kernel_ring_cursor)
        && is_granted(g.kernel_ring_notification)
    {
        let intake = must(retype_obj(ut, abi::objtype::RENDEZVOUS));
        let timer = must(retype_obj(ut, abi::objtype::TIMER));
        let log = must(build_child(
            ut,
            ut,
            log_program,
            &ChildEndowment {
                caps: &[
                    (intake, abi::rights::READ),
                    (
                        g.kernel_ring_notification,
                        abi::rights::READ | abi::rights::WRITE,
                    ),
                    (timer, abi::rights::WRITE),
                    (request, abi::rights::WRITE),
                ],
                maps: &[
                    (LOG_RING_VA, g.kernel_ring, abi::address_space::MAP_RO),
                    (
                        LOG_CURSOR_VA,
                        g.kernel_ring_cursor,
                        abi::address_space::MAP_RW,
                    ),
                ],
                stack_pages: CHILD_STACK_PAGES,
                ..ChildEndowment::new(Retention::Nothing)
            },
        ));
        // In the window between build and start, as the console's batching notification is.
        must_ok(user_mode_runtime::notification_bind(g.kernel_ring_notification, log.tcb) >= 0);
        must_ok(start_child(log, LOG_MODE_KERNEL, 0, 0));
        // No log writer registers yet, so the intake is not kept (the service's BUGS).
        cap_delete(intake);
        cap_delete(timer);
    }
    // Either way, so none is carried across the login block's peak.
    cap_delete(g.kernel_ring);
    cap_delete(g.kernel_ring_cursor);
    cap_delete(g.kernel_ring_notification);
}

/// Halt, saying why through the console, when `input`, `swish` or `job_undertaker` is not the
/// one measured (§219 (how the shell names an installed program to the spawner)). Runs once the
/// console and the line discipline are up and before anything unmeasured is built; returns only
/// when all three are vouched for.
#[inline(never)]
pub(super) fn refuse_unvouched(
    ut: u64,
    term: Terminal,
    in_elf: &measured_boot::Verdict<'static>,
    sh_elf: &measured_boot::Verdict<'static>,
    reaper_elf: &measured_boot::Verdict<'static>,
) {
    let unvouched: [&str; 3] = [
        if in_elf.unvouched { "input" } else { "" },
        if sh_elf.unvouched { "swish" } else { "" },
        if reaper_elf.unvouched {
            "job_undertaker"
        } else {
            ""
        },
    ];
    if unvouched.iter().any(|n| !n.is_empty()) {
        // The shell's output page, mapped here so the refusal can be read.
        if map_page_frame(term.out, PROGENITOR_SHELL_OUTPUT_VIRTUAL_ADDRESS, true, ut) {
            let mut buf = [0u8; SENTENCE];
            announce(
                term.ep,
                sentence(
                    &mut buf,
                    b"progenitor: cannot vouch for",
                    &unvouched,
                    b"; halting rather than building an unmeasured system\n",
                ),
            );
        }
        fail()
    }
}

/// Build the keystroke sources: the input driver on the UART receive interrupt, and `WRITE` on
/// `term_ep` sent to a USB keyboard driver when the kernel started one. Both feed one line
/// discipline. Then the UART's device and interrupt are deleted, since no later child may get
/// them. See notes/system-initializer/the-boot-order.md, "The terminal stack and the shell".
#[inline(never)]
pub(super) fn start_keystroke_sources(
    g: &BootEndowment,
    ut: u64,
    in_elf: &elf::Elf<'static>,
    term_ep: u64,
    uart_shape: u64,
) {
    #[cfg(not(target_arch = "x86_64"))]
    let (in_caps, in_maps): EndowmentSlices = (
        &[
            (term_ep, abi::rights::WRITE),
            (g.uart_irq, abi::rights::READ),
        ],
        &[(IN_UART_VA, g.uart_dev, abi::address_space::MAP_RO)],
    );
    #[cfg(target_arch = "x86_64")]
    let (in_caps, in_maps): EndowmentSlices = (
        &[
            (term_ep, abi::rights::WRITE),
            (g.uart_irq, abi::rights::READ),
            (g.uart_dev, abi::rights::WRITE), // COM1's port range, read by the TSS I/O bitmap
        ],
        &[],
    );
    let input = must(build_child(
        ut,
        ut,
        in_elf,
        &ChildEndowment {
            caps: in_caps,
            maps: in_maps,
            stack_pages: CHILD_STACK_PAGES,
            ..ChildEndowment::new(Retention::Nothing)
        },
    ));
    must_ok(start_child(input, uart_shape, 0, 0)); // the console's register shape, `abi::uart`

    // The driver receives on its attach endpoint before anything else, so this send cannot hang.
    if is_granted(g.usb_keyboard_attach) {
        must_ok(send_capability(g.usb_keyboard_attach, term_ep, abi::rights::WRITE, 0) >= 0);
        cap_delete(g.usb_keyboard_attach);
    }

    // Before the shell is built, and so that no later child can be handed the UART.
    cap_delete(g.uart_dev);
    cap_delete(g.uart_irq);
}

/// Retype the spawn channel and the supervision endpoint, after the drivers are built: held
/// through the driver builds, they would raise the early peak. The progenitor keeps `deaths` only
/// for its `GRANT`, to place a `READ` view in each job's fault slot; `job_undertaker` receives
/// on it.
#[inline(never)]
pub(super) fn retype_spawn_channel(ut: u64) -> SpawnChannel {
    let spawn_ep = must(retype_obj(ut, abi::objtype::RENDEZVOUS));
    let result_ep = must(retype_obj(ut, abi::objtype::RENDEZVOUS));
    let deaths = must(retype_obj(ut, abi::objtype::RENDEZVOUS));
    SpawnChannel {
        spawn_ep,
        result_ep,
        deaths,
    }
}

/// Build the shell, not yet started, with its terminal, spawn channel, budget, filesystem, clock
/// and configuration page. The file service is slot 4 when the boot has one; the clock is last, so
/// the shell is told its slot. Clock and configuration are `READ` without `GRANT` (§43 (reading the
/// clock is a page, setting it is a page you may write)), and the configuration page is placed at
/// [`grant_plan::SHELL_CONFIGURATION_SLOT`] after the build. Returns the child and the clock's
/// slot. See notes/system-initializer/the-boot-order.md, "The terminal stack and the shell".
#[inline(never)]
pub(super) fn build_shell(
    w: &Wiring,
    sh_elf: &elf::Elf<'static>,
    chan: SpawnChannel,
    sh_budget: u64,
) -> (supervision_protocol::Child, u64) {
    let (g, ut, with_fs) = (w.g, w.ut, w.with_fs);
    let mut sh_caps = [(0u64, 0u64); 6];
    let mut sh_maps = [(0u64, 0u64, 0u64); 5];
    let mut n_caps = 0usize;
    let mut n_maps = 0usize;
    sh_caps[n_caps] = (w.term.ep, abi::rights::WRITE);
    n_caps += 1;
    sh_caps[n_caps] = (chan.spawn_ep, abi::rights::WRITE);
    n_caps += 1;
    sh_caps[n_caps] = (chan.result_ep, abi::rights::READ);
    n_caps += 1;
    // Every right: what the shell retypes carries its budget's rights, and it lends them on.
    sh_caps[n_caps] = (sh_budget, abi::rights::ALL);
    n_caps += 1;
    sh_maps[n_maps] = (SH_OUT_VA, w.term.out, abi::address_space::MAP_RW); // text and prompts
    n_maps += 1;
    sh_maps[n_maps] = (LINE_VA, w.term.input, abi::address_space::MAP_RO); // completed lines
    n_maps += 1;
    // A slice of window 0, never the pool, which would hand the shell every client's window.
    let sh_window = if with_fs {
        window_zero(Fs {
            ep: g.fs_ep,
            page: g.fs_page,
        })
    } else {
        None
    };
    if let Some(zero) = sh_window {
        sh_caps[n_caps] = (zero.ep, abi::rights::WRITE);
        n_caps += 1;
        sh_maps[n_maps] = (SH_FS_VA, zero.page, abi::address_space::MAP_RW);
        n_maps += 1;
    }
    sh_caps[n_caps] = (g.clock_page, abi::rights::READ);
    n_caps += 1;
    sh_maps[n_maps] = (SH_CLOCK_VA, g.clock_page, abi::address_space::MAP_RO);
    n_maps += 1;
    // Mapped for `caps` to read; its capability goes at a named slot after the build, because the
    // list's slots move with what the boot granted.
    sh_maps[n_maps] = (
        grant_plan::SHELL_CONFIGURATION_VIRTUAL_ADDRESS,
        g.config_page,
        abi::address_space::MAP_RO,
    );
    n_maps += 1;
    // `build_child` fills the table from zero, so the clock's slot is the count before it.
    let sh_clock_slot: u64 = n_caps as u64 - 1;
    // Not started: the giveaway's negative control is printed through the shell's output page.
    let shell = must(build_child(
        ut,
        ut,
        sh_elf,
        &ChildEndowment {
            caps: &sh_caps[..n_caps],
            maps: &sh_maps[..n_maps],
            stack_pages: SHELL_STACK_PAGES,
            ..ChildEndowment::new(Retention::Nothing)
        },
    ));
    cap_delete(sh_budget); // our copy; the shell holds its own now
    if let Some(zero) = sh_window {
        cap_delete(zero.page); // the slice was only the means; the shell holds the mapping
    }
    // Before the shell starts, which is what makes its `_start` probe of this slot sound.
    must_ok(place_at(
        shell.tcb,
        g.config_page,
        abi::rights::READ,
        grant_plan::SHELL_CONFIGURATION_SLOT,
    ));
    (shell, sh_clock_slot)
}

/// Hand the shell the machine statistics page (`READ | GRANT`, when [`GRANT_MACHINE_PAGE`]
/// allows), keeping no copy, and the second mounted tree's endpoint (`WRITE`, at
/// [`spawnproto::MOUNTED_TREE_SLOT`]). Runs before the login block, because a page held across it
/// would raise the table's highest peak. The display devices stay with the spawn service; the shell
/// gets none.
#[inline(never)]
pub(super) fn finish_shell_endowment(
    g: &BootEndowment,
    shell_tcb: u64,
    mounted_tree: Option<MountedTreeGrant>,
) {
    if GRANT_MACHINE_PAGE {
        must_ok(place_at(
            shell_tcb,
            g.machine_page,
            abi::rights::READ | abi::rights::GRANT,
            spawnproto::MACHINE_PAGE_SLOT,
        ));
    }
    cap_delete(g.machine_page);
    // Unbadged: the disk tree's window 0 is the frame the shell maps at [`SH_FS_VA`], sound while
    // the shell has one `CALL` in flight. Our copy stays, for the spawn service to badge per job.
    if let Some(t) = mounted_tree {
        must_ok(place_at(
            shell_tcb,
            t.ep,
            abi::rights::WRITE,
            spawnproto::MOUNTED_TREE_SLOT,
        ));
    }
}

/// Build the terminal's sink adapter, which holds `WRITE` on the terminal and serves the sink
/// contract (notes/sink-protocol.md), so a child can be given a place to put bytes without the
/// terminal endpoint, which would also be the keyboard. After the shell, whose build is a peak,
/// and before the giveaway, because it is a system component. Returns the sink endpoint, retyped
/// whether or not an adapter serves it. See notes/system-initializer/the-boot-order.md.
#[inline(never)]
pub(super) fn start_sink_adapter(w: &Wiring, sink_elf: Option<&elf::Elf<'static>>) -> u64 {
    let (ut, term_ep) = (w.ut, w.term.ep);
    let term_sink = must(retype_obj(ut, abi::objtype::RENDEZVOUS));
    if let Some(elf) = sink_elf {
        let adapter = must(build_child(
            ut,
            ut,
            elf,
            &ChildEndowment {
                caps: &[
                    (term_sink, abi::rights::READ),
                    (term_ep, abi::rights::WRITE),
                ],
                stack_pages: CHILD_STACK_PAGES,
                ..ChildEndowment::new(Retention::Nothing)
            },
        ));
        // Safe to start before the shell: it has no client, and no page the announcement uses.
        must_ok(start_child(adapter, 0, 0, 0));
    }
    term_sink
}

/// Build the login stack when this boot has a filesystem, working entropy and all four programs:
/// a generated password, `credentialer`, the provisioned identity, and `login`. Returns whether
/// login is ready, the password to announce, and the run-unvouched endpoint if `login` was given
/// one. Skipped, not faulted, when anything is missing. The order is in
/// notes/system-initializer/the-boot-order.md, "The login stack".
#[inline(never)]
pub(super) fn start_login_stack(
    w: &Wiring,
    p: &LoginPrograms,
    table: &str,
    entropy_client: Option<u64>,
) -> Login {
    let have_login_stack = w.with_fs
        && entropy_client.is_some()
        && p.cred_elf.is_some()
        && p.idp_elf.is_some()
        && p.login_elf.is_some()
        && p.audit_elf.is_some();
    let mut login_ready = false;
    let mut login_password = [0u8; PASSWORD_HEX_LEN];
    // Retyped only after `login`'s build, where this table peaks; see [`Channels::run_unvouched`].
    let mut run_unvouched: Option<u64> = None;
    if !have_login_stack {
        // The entropy endpoint is kept: the spawn service still endows it.
    } else {
        let request = entropy_client.expect("have_login_stack checked entropy_client.is_some()");

        // No unpredictable bits, no password: the block is abandoned rather than weakened.
        let mut password_bytes = [0u8; PASSWORD_BYTES];
        if !fill_entropy(request, &mut password_bytes) {
        } else {
            hex_password(&password_bytes, &mut login_password);
            login_ready = start_credential_services(
                w,
                p,
                table,
                request,
                &login_password,
                &mut run_unvouched,
            );
        }
    }
    Login {
        ready: login_ready,
        password: login_password,
        run_unvouched,
    }
}

/// Build `credentialer`, provision the generated credential through `identity_provisioner`, seal
/// the store whatever provisioning did, and build `login` once `credentialer` reports ready.
/// `true` when provisioning held. `request` stays ours: the spawn service endows it later.
#[inline(never)]
fn start_credential_services(
    w: &Wiring,
    p: &LoginPrograms,
    table: &str,
    request: u64,
    login_password: &[u8; PASSWORD_HEX_LEN],
    run_unvouched: &mut Option<u64>,
) -> bool {
    let ut = w.ut;
    let prov = must(retype_obj(ut, abi::objtype::RENDEZVOUS));
    let verify = must(retype_obj(ut, abi::objtype::RENDEZVOUS));
    let cred_ready = must(retype_obj(ut, abi::objtype::RENDEZVOUS));
    let prov_page = must(retype_page_frame(ut));
    let verify_page = must(retype_page_frame(ut));
    let cred_budget = must(memory_region_split(ut, CRED_BUDGET_PAGES));
    let cred_program = p.cred_elf.as_ref().expect("have_login_stack checked this");
    let cred_child = must(build_child(
        ut,
        ut,
        cred_program,
        &ChildEndowment {
            caps: &[
                (prov, abi::rights::READ),
                (verify, abi::rights::READ),
                (request, abi::rights::WRITE),
                (cred_budget, abi::rights::SPEND_ONLY),
                (cred_ready, abi::rights::WRITE),
            ],
            maps: &[
                (CRED_SVC_PROV_VA, prov_page, abi::address_space::MAP_RW),
                (CRED_SVC_VERIFY_VA, verify_page, abi::address_space::MAP_RW),
            ],
            stack_pages: CRED_STACK_PAGES,
            ..ChildEndowment::new(Retention::Nothing)
        },
    ));
    must_ok(start_child(cred_child, 0, 0, 0));
    cap_delete(cred_budget);

    // `cred_ready` is not read until after the seal below: `credentialer` sends `RPT_READY` only
    // once `SEAL` ends its provisioning phase, so reading it here would never wake.
    let mut login_ready = provision_identity(w, p, prov, prov_page, login_password);
    cap_delete(prov_page);

    // Sealed whatever provisioning did: `VERIFY` answers nothing until the store is sealed.
    call(
        prov,
        credential_contract::req(credential_contract::provision::SEAL, 0, 0),
        0,
    );
    cap_delete(prov);

    // A service that failed at start-up answers too, with its own `0xDEAD_...` word.
    let (cred_rv, _, _) = receive(cred_ready);
    cap_delete(cred_ready);
    if cred_rv != CRED_RPT_READY {
        cap_delete(verify);
        cap_delete(verify_page);
        login_ready = false;
    } else {
        start_login(w, p, table, verify, verify_page, run_unvouched);
    }
    login_ready
}

/// Stage the generated credential in a scratch page, run `identity_provisioner` on it, and wipe
/// the page whatever the outcome. `true` when the provisioner reported success.
#[inline(never)]
fn provision_identity(
    w: &Wiring,
    p: &LoginPrograms,
    prov: u64,
    prov_page: u64,
    login_password: &[u8; PASSWORD_HEX_LEN],
) -> bool {
    let ut = w.ut;
    let req_frame = must(retype_page_frame(ut));
    if !map_page_frame(req_frame, PROVISION_SCRATCH_VA, true, ut) {
        cap_delete(req_frame);
        false
    } else {
        // SAFETY: just mapped read/write, one page, ours alone until it is delegated
        // on below.
        let req_slice = unsafe {
            core::slice::from_raw_parts_mut(
                PROVISION_SCRATCH_VA as *mut u8,
                credential_contract::PAGE,
            )
        };
        req_slice.fill(0);
        let placed = credential_contract::place(
            req_slice,
            DEMO_IDENTITY,
            login_password,
            credential_contract::provision::PUT,
        );
        placed.expect(
            "DEMO_IDENTITY and login_password are both well within credential_contract's \
             bounds by construction; a None here is a logic bug in this file, not a \
             runtime condition",
        );

        let idp_report =
            start_identity_provisioner(w, p, prov, req_frame, prov_page, login_password);
        let (idp_code, _, _) = receive(idp_report);
        cap_delete(idp_report);
        let login_ready = idp_code == IDP_RPT_OK;

        // SAFETY: still the same page this process mapped above, and the only writer
        // between it being staged and this wipe.
        let req_slice = unsafe {
            core::slice::from_raw_parts_mut(
                PROVISION_SCRATCH_VA as *mut u8,
                credential_contract::PAGE,
            )
        };
        credential_contract::wipe(req_slice);
        login_ready
    }
}

/// Build and start `identity_provisioner` on the staged request page, with window 0 mapped in
/// after the build. Returns the endpoint it reports on.
///
/// Window 0's page is sliced only after the build, when `req_frame` has been deleted: the login
/// block is the table's peak, and the slice spends the slot `req_frame` gave back.
#[inline(never)]
fn start_identity_provisioner(
    w: &Wiring,
    p: &LoginPrograms,
    prov: u64,
    req_frame: u64,
    prov_page: u64,
    login_password: &[u8; PASSWORD_HEX_LEN],
) -> u64 {
    let (g, ut) = (w.g, w.ut);
    let idp_report = must(retype_obj(ut, abi::objtype::RENDEZVOUS));
    let idp_program = p.idp_elf.as_ref().expect("have_login_stack checked this");
    let (idp_child, idp_aspace) = must(supervision_protocol::build_child_space(
        ut,
        ut,
        idp_program,
        &ChildEndowment {
            caps: &[
                (prov, abi::rights::WRITE),
                (g.fs_ep, abi::rights::WRITE),
                (idp_report, abi::rights::WRITE),
            ],
            maps: &[
                (IDP_REQ_VA, req_frame, abi::address_space::MAP_RW),
                (IDP_PROV_VA, prov_page, abi::address_space::MAP_RW),
            ],
            stack_pages: CHILD_STACK_PAGES,
            ..ChildEndowment::new(Retention::Nothing)
        },
    ));
    cap_delete(req_frame);
    let idp_page = must(window_page(g.fs_page, 0).ok_or(()));
    let mapped =
        address_space_map_into(idp_aspace, IDP_FS_VA, idp_page, abi::address_space::MAP_RW) == 0;
    cap_delete(idp_page);
    must(mapped.then_some(()).ok_or(()));
    must(supervision_protocol::configure_child(
        idp_child.tcb,
        idp_aspace,
        idp_program.entry(),
    ));
    must_ok(start_child(
        idp_child,
        DEMO_IDENTITY.len() as u64,
        login_password.len() as u64,
        0,
    ));
    idp_report
}

/// Build `login_audit_receiver`, then `login` itself, holding narrowed views of the file
/// service, `credentialer`'s verify endpoint, a construction budget and the terminal (`WRITE |
/// GRANT`, to hand to its first successful caller), and the blobs it builds sessions from. The
/// receiver starts first, so `login`'s blocking audit send always has one. Every wiring
/// capability of ours is deleted once `login` holds its own.
#[inline(never)]
fn start_login(
    w: &Wiring,
    p: &LoginPrograms,
    table: &str,
    verify: u64,
    verify_page: u64,
    run_unvouched: &mut Option<u64>,
) {
    let (g, ut, term_ep) = (w.g, w.ut, w.term.ep);
    let audit = start_login_audit(ut, p);

    let login_request = must(retype_obj(ut, abi::objtype::RENDEZVOUS));
    let login_result = must(retype_obj(ut, abi::objtype::RENDEZVOUS));
    let login_ut = must(memory_region_split(ut, LOGIN_CONSTRUCTION_PAGES));
    let login_program = p.login_elf.as_ref().expect("have_login_stack checked this");
    // Window 0, never the pool: `login` maps what it is handed into every session's caretaker.
    let login_page = must(window_page(g.fs_page, 0).ok_or(()));
    let login_child = must(build_child(
        ut,
        ut,
        login_program,
        &ChildEndowment {
            caps: &[
                (login_request, abi::rights::READ),
                (login_result, abi::rights::WRITE | abi::rights::GRANT),
                (verify, abi::rights::WRITE),
                (g.fs_ep, abi::rights::WRITE | abi::rights::GRANT),
                // `WRITE` alone: we hold only `WRITE | GRANT` on this page, and a writable
                // mapping checks `WRITE`.
                (login_page, abi::rights::WRITE | abi::rights::GRANT),
                (login_ut, abi::rights::ALL),
                (audit, abi::rights::WRITE),
                (term_ep, abi::rights::WRITE | abi::rights::GRANT),
            ],
            maps: &[(LOGIN_CRED_VA, verify_page, abi::address_space::MAP_RW)],
            // No slot cost: `fill_and_map` holds one frame capability at a time.
            blobs: &[
                (login_contract::CARETAKER_ELF_VA, p.care_blob),
                (login_contract::PROGRAM_MEASUREMENTS_VA, table.as_bytes()),
                (login_contract::USER_TIMETABLE_KEEPER_ELF_VA, p.keeper_blob),
                (login_contract::TIMETABLE_ELF_VA, p.timetable_blob),
            ],
            stack_pages: LOGIN_STACK_PAGES,
            ..ChildEndowment::new(Retention::Nothing)
        },
    ));
    place_login_capabilities(w, login_child.tcb, run_unvouched);
    must_ok(start_child(
        login_child,
        p.care_blob.len() as u64,
        table.len() as u64,
        login_contract::schedule_lengths(p.keeper_blob.len() as u64, p.timetable_blob.len() as u64),
    ));
    cap_delete(login_page);
    cap_delete(login_request);
    cap_delete(login_result);
    cap_delete(verify);
    cap_delete(verify_page);
    cap_delete(login_ut);
    cap_delete(audit);
}

/// Build and start `login_audit_receiver`; returns the endpoint `login` sends its audit on.
#[inline(never)]
fn start_login_audit(ut: u64, p: &LoginPrograms) -> u64 {
    let audit = must(retype_obj(ut, abi::objtype::RENDEZVOUS));
    let audit_program = p.audit_elf.as_ref().expect("have_login_stack checked this");
    let audit_child = must(build_child(
        ut,
        ut,
        audit_program,
        &ChildEndowment {
            caps: &[(audit, abi::rights::READ)],
            stack_pages: CHILD_STACK_PAGES,
            ..ChildEndowment::new(Retention::Nothing)
        },
    ));
    must_ok(start_child(audit_child, 0, 0, 0));
    audit
}

/// Place the run-unvouched endpoint (`WRITE | GRANT`, which `login` narrows to `WRITE` for each
/// session, §219 gate D2) and the durable window's page in `login`, after its build and before it
/// starts. After the build because the build is the table's peak and both outlive it.
#[inline(never)]
fn place_login_capabilities(w: &Wiring, login_tcb: u64, run_unvouched: &mut Option<u64>) {
    let (g, ut) = (w.g, w.ut);
    let d2 = *run_unvouched.get_or_insert_with(|| must(retype_obj(ut, abi::objtype::RENDEZVOUS)));
    must_ok(place_at(
        login_tcb,
        d2,
        abi::rights::WRITE | abi::rights::GRANT,
        spawnproto::RUN_UNVOUCHED_SLOT,
    ));
    // The durable sessions' own window, apart from window 0; [`Windows`] never hands it to a job.
    let durable_page = must(window_page(g.fs_page, login_contract::DURABLE_WINDOW).ok_or(()));
    must_ok(place_at(
        login_tcb,
        durable_page,
        abi::rights::WRITE | abi::rights::GRANT,
        login_contract::DURABLE_WINDOW_SLOT,
    ));
    cap_delete(durable_page);
}

/// Place the run-unvouched endpoint in the shell (`WRITE` alone), carve the progenitor's three
/// bounded budgets, give the root away and prove it is gone, aloud. Returns the endpoint and the
/// three budgets. The shell's placement is what makes the boot prompt the owner's console (§221
/// (the boot prompt is the owner's console)), and deleting it makes the prompt refuse every
/// unvouched image. See notes/system-initializer/the-boot-order.md, "The giveaway".
///
/// # BUGS
///
/// - The prompt holds it with no login, which §221's 2026-10-06 amendment rules out (milestone
///   806).
#[inline(never)]
pub(super) fn give_away_budget(w: &Wiring, shell_tcb: u64, run_unvouched: Option<u64>) -> Kept {
    let (ut, term_ep) = (w.ut, w.term.ep);
    let run_unvouched =
        run_unvouched.unwrap_or_else(|| must(retype_obj(ut, abi::objtype::RENDEZVOUS)));
    must_ok(place_at(
        shell_tcb,
        run_unvouched,
        abi::rights::WRITE,
        spawnproto::RUN_UNVOUCHED_SLOT,
    ));

    // The job pool apart from our own budget, so its watermark moves for jobs only: the LIFO
    // return of §16 (object revocation: reclaim the objects a process built).
    let own_ut = must(memory_region_split(ut, PROGENITOR_RESERVED_PAGES));
    let jobs_ut = must(memory_region_split(ut, JOBS_BUDGET_PAGES));
    let images_ut = must(memory_region_split(ut, IMAGE_POOL_PAGES));
    // Given up with `UNMAP` before the shell starts: `PageFrame::REVOKE` would take it from the
    // shell too.
    if !map_page_frame(
        w.term.out,
        PROGENITOR_SHELL_OUTPUT_VIRTUAL_ADDRESS,
        true,
        ut,
    ) {
        fail()
    }
    cap_delete(ut);

    // `NoSuchSlot` (-1), not `NotPermitted` (-3): gone, not narrowed. `script/swish-check` reads
    // the sentence.
    let frame = retype_into_page_frame(ut);
    let object = retype_into_kernel_object(ut, abi::objtype::THREAD_CONTROL_BLOCK);
    announce(
        term_ep,
        if frame == -1 && object == -1 {
            b"progenitor: construction budget dropped; retype answers NoSuchSlot\n"
        } else {
            b"progenitor: construction budget NOT dropped; it can still build\n"
        },
    );
    Kept {
        run_unvouched,
        own_ut,
        jobs_ut,
        images_ut,
    }
}
