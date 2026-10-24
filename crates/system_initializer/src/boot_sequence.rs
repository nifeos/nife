//! The boot sequence: `boot`, which builds the interactive system in numbered steps and then
//! becomes its spawn service, and its last steps (what it says before the prompt, and the
//! undertaker). The building steps and the password helpers are in `boot_steps`. See
//! notes/trusted-init/interactive-boot.md, "What the boot keeps, and what it says before the
//! prompt".
//!
//! Name: provisional (milestone 843's lane, 2026-10-11 UTC).

use super::boot_steps::*;
use super::*;

/// **Build the interactive system and become its spawn service.** Never returns: the last thing it
/// does is park in `RECEIVE` on the shell's spawn channel for the life of the boot.
///
/// `initial_ramdisk_len` is the archive length the kernel passed at entry; `fs_rights` is the
/// `filesystem_contract::dir` rights the file-service endpoint carries, and 0 means this boot
/// attached no disk (so `fs_ep` and `fs_page` are empty). `uart_shape` is the console UART's
/// register shape as the kernel passed it (`abi::uart`), handed on to the console and input
/// drivers. `mounted_tree`, when `Some` and its slots hold something, is the shell's second
/// mounted tree (see [`MountedTreeGrant`]); the real progenitor passes the disk tree's pair.
///
/// # Examples
///
/// A progenitor's whole source, near enough: the endowment is the complete authority of the system
/// about to exist. A grant a boot lacks is an empty slot that `boot` probes, not a failure. The real
/// table is `components/src/progenitor.rs`'s; `no_run`, because it runs only at EL0.
///
/// ```no_run
/// use system_initializer::{BootEndowment, boot};
///
/// fn init(uart_shape: u64, initial_ramdisk_len: u64, fs_rights: u64) -> ! {
///     let endowment = BootEndowment {
///         untyped: 2, uart_dev: 3, uart_irq: 4, clock_page: 5, config_page: 6,
///         fs_ep: 7, fs_page: 8,
///         virtio_rng: 9, virtio_rng_irq: 10, virtio_rng_dma: 11,
///         disp_term_ep: 12, disp_term_page: 13,
///         virtio_net: 15, virtio_net_irq: 16, virtio_net_dma: 17,
///         entropy_ep: 18,
///         gpu_surface: 14, gpu: 19, gpu_irq: 20, gpu_dma: 21,
///         keyboard: 22, keyboard_irq: 23, keyboard_dma: 1,
///         machine_page: 0,
///         kernel_ring: 24, kernel_ring_cursor: 25, kernel_ring_notification: 26,
///         usb_keyboard_attach: 27, own_space: 28,
///         net_stack_ep: 29, net_stack_report: 30, reboot: 31,
///         for_test_roles: &[],
///     };
///     boot(&endowment, initial_ramdisk_len, fs_rights, uart_shape, None)
/// }
/// ```
pub fn boot(
    g: &BootEndowment,
    initial_ramdisk_len: u64,
    fs_rights: u64,
    uart_shape: u64,
    mounted_tree: Option<MountedTreeGrant>,
) -> ! {
    // Probed before anything here allocates, because a slot the kernel left empty is one a later
    // retype may land in.
    let mounted_tree = mounted_tree.filter(|t| is_granted(t.ep) && is_granted(t.pool));
    let (fs, table) = open_archive(g, initial_ramdisk_len);
    let p = read_boot_programs(&fs, table);
    let spawnable = measure_spawnable(&fs, table);

    let ut = g.untyped;
    let devices = start_device_services(g, ut, &fs, table, p.ent_elf.as_ref(), p.net_elf.as_ref());

    // **The two components that have to exist before the progenitor can say anything.** The console writes
    // the UART and the line discipline is its only client, so a refusal of either has no route to a
    // person and this is the one case that stops in silence (see this module's BUGS). Everything
    // else is checked below, after they are running.
    let (Some(con_elf), Some(_), Some(ts_elf)) = (p.con_elf.elf, p.td_elf.elf, p.ts_elf.elf) else {
        fail()
    };
    // Vouched above, so these are the measured bytes.
    let td_bytes = fs.read("line_editor").unwrap_or(&[]);

    // `graphical_terminal_grants` ran in `start_device_services`: the devices' caps sit in their
    // kernel-granted slots until the shell's build places them, and nothing graphical is built here
    // on any boot (milestone 632 (graphics on demand)).
    let term = start_terminal_stack(
        g,
        ut,
        (devices.has_screen, uart_shape),
        &con_elf,
        p.log_elf.as_ref(),
        &ts_elf,
        td_bytes,
    );

    refuse_unvouched(ut, term, &p.in_elf, &p.sh_elf, &p.reaper_elf);
    // Past the refusal, so an absent component still traps the way it always has: a build that did
    // not pack the console's client is a broken build, not an attack, and it has never had a
    // message.
    let (Some(in_elf), Some(sh_elf), Some(reaper_elf)) =
        (p.in_elf.elf, p.sh_elf.elf, p.reaper_elf.elf)
    else {
        fail()
    };

    start_keystroke_sources(g, ut, &in_elf, term.ep, uart_shape);
    let chan = retype_spawn_channel(ut);

    // 4. The shell, holding its own untyped budget so `run --mem N` grants from memory that is the
    // shell's. Its slots, the filesystem's and the clock's among them, are [`build_shell`]'s.
    let sh_budget = must(memory_region_split(ut, SH_BUDGET_PAGES));
    let w = Wiring {
        g,
        ut,
        term,
        with_fs: fs_rights != 0,
    };
    // Meaningless without a first tree, whose page the second shares.
    let mounted_tree = mounted_tree.filter(|_| w.with_fs);
    let (shell, sh_clock_slot) = build_shell(&w, &sh_elf, chan, sh_budget);
    finish_shell_endowment(g, shell.tcb, mounted_tree);

    // Free every boot cap the spawn service does not need, so the capability table has room to
    // build a supervised child. Only the input frame here: `term_ep` is still delegated below (the
    // sink adapter's `WRITE`, the announcements' `CALL`) and `term_out` stages the announcements.
    cap_delete(term.input);
    // The file service stays for the life of the boot: a caretaker must hold it to attenuate a
    // directory grant, and the shell's copy carries no `GRANT` (see the module's note).
    let fs = w.with_fs.then_some(Fs {
        ep: g.fs_ep,
        page: g.fs_page,
    });

    let term_sink = start_sink_adapter(&w, p.sink_elf.as_ref());
    let login = start_login_stack(&w, &p.login, table, devices.entropy.client);
    let kept = give_away_budget(&w, shell.tcb, login.run_unvouched);
    announce_outcomes(
        &w,
        &spawnable,
        p.fs_scoped,
        &devices.entropy,
        devices.network,
        devices.resolver.is_some(),
        &login,
    );

    start_undertaker(kept.own_ut, &reaper_elf, chan);

    // Role 0 (the prompt), and `arg1` is the rights its directory capability carries; a shell told 0
    // holds no directory. `arg2` is the clock slot, which moves with whether this boot had a disk.
    must_ok(start_child(shell, 0, fs_rights, sh_clock_slot));

    spawn_service(
        spawn_channels(
            &w,
            chan,
            kept,
            devices,
            p.sink_elf.is_some().then_some(term_sink),
            (fs, mounted_tree),
            (p.catalog, p.index_sources),
        ),
        &spawnable,
        Caretakers {
            subtree: p.care_elf,
            nameset: p.set_elf,
        },
        // The session's discipline is built from the same supervisor and the same bytes the boot's
        // own was (`ts_elf`/`td_bytes` above), so a session's line editor is the boot's line editor,
        // measured once and built twice.
        SessionDiscipline {
            supervisor: ts_elf,
            discipline: td_bytes,
        },
        p.fs_scoped,
    )
}

/// Say what the boot decided, before the prompt: the measurement, how directory grants are
/// delivered, and the entropy, network and login outcomes carried forward as data. Then give up
/// the shell's output page they were staged through.
#[inline(never)]
fn announce_outcomes(
    w: &Wiring,
    spawnable: &Spawnable,
    fs_scoped: bool,
    entropy: &Entropy,
    network: Option<(u64, u64, u64)>,
    resolver: bool,
    login: &Login,
) {
    let term_ep = w.term.ep;
    let refused = &spawnable.refused[..spawnable.refused_n];
    // What the measurement decided. `script/swish-check` reads this sentence, so a boot that
    // stopped measuring fails the gate instead of passing quietly.
    let mut buf = [0u8; SENTENCE];
    announce(
        term_ep,
        if refused.is_empty() {
            b"progenitor: every program measured against the archive table\n"
        } else {
            sentence(
                &mut buf,
                b"progenitor: measurement refused",
                refused,
                b"; they cannot be spawned\n",
            )
        },
    );
    // How directory grants are delivered (milestone 606 (a directory walk costs what it does on
    // Linux), rulings D and T1), said once for `script/swish-check`: a server whose measured bytes
    // carry the subtree-grants note gets bound badges, and every other boot keeps caretakers.
    if w.with_fs {
        announce(
            term_ep,
            if fs_scoped {
                b"progenitor: the file server enforces subtree grants itself; directory grants get a bound badge\n"
            } else {
                b"progenitor: directory grants get a caretaker\n"
            },
        );
    }
    // The entropy service's outcome, decided early and said here, once the output page is mapped
    // (DECISIONS §120 (a QEMU-only virtio-rng stopgap for the interactive boot)). Silent when there
    // was nothing to build.
    if entropy.ready {
        announce(
            term_ep,
            if entropy.from_instruction {
                &b"progenitor: entropy service up; the kernel built it on the CPU's seed instruction\n"[..]
            } else {
                &b"progenitor: entropy service up; drew real bytes from a virtio-rng device\n"[..]
            },
        );
    }
    // The network stack's outcome and the resolver's, said here for the entropy outcome's reason;
    // [`announce_network`] has both sentences.
    announce_network(term_ep, network, resolver);
    if login.ready {
        announce_login_credentials(term_ep, &login.password);
    }
    // `term_ep` stays and `term_out` does not: a `graphical_terminal` session with no virtio keyboard
    // takes keystrokes from the boot's line discipline, and only this process can grant that
    // endpoint (see the module's note).
    cap_delete(w.term.out);
    // And the window onto the shell's output page, which every line above was staged through and
    // nothing below writes (milestone 95 (an unmap primitive), §249 (a running address space stays
    // nameable)): given up before the shell starts, so this process cannot read what the shell
    // prints or write into it.
    supervision_protocol::give_up_own_page(PROGENITOR_SHELL_OUTPUT_VIRTUAL_ADDRESS);
}

/// Print the generated login credential, once, before the prompt.
#[inline(never)]
fn announce_login_credentials(term_ep: u64, login_password: &[u8; PASSWORD_HEX_LEN]) {
    // Printed once, before the prompt: this process's own copy was wiped once
    // `identity_provisioner` used it. The wording names the evidence, `IDP_RPT_OK`, and claims no
    // more; whether `login` survived is `script/swish-check`'s (see the module's note).
    fn push(buf: &mut [u8; SENTENCE], n: &mut usize, src: &[u8]) {
        for &b in src {
            if *n < SENTENCE {
                buf[*n] = b;
                *n += 1;
            }
        }
    }
    let mut buf = [0u8; SENTENCE];
    let mut n = 0usize;
    push(
        &mut buf,
        &mut n,
        b"progenitor: login credentials provisioned -- identity '",
    );
    push(&mut buf, &mut n, DEMO_IDENTITY);
    push(&mut buf, &mut n, b"' password '");
    push(&mut buf, &mut n, login_password);
    push(&mut buf, &mut n, b"' (shown once; use it now)\n");
    announce(term_ep, &buf[..n]);
}

/// Step 6: build and start `job_undertaker` out of the progenitor's own budget.
#[inline(never)]
fn start_undertaker(own_ut: u64, reaper_elf: &elf::Elf<'static>, chan: SpawnChannel) {
    // 6. The undertaker, out of what is left of our own budget: `READ` on the supervision endpoint,
    // `WRITE` on the result endpoint, and `WRITE` on a copy of the spawn endpoint badged
    // `spawnproto::UNDERTAKER_BADGE` (see the module's note). Slot order is the contract with the
    // program, which names them `DEATHS` (0), `REPORT` (1) and `SPAWN` (2).
    let reaper_spawn = must(badged(chan.spawn_ep, spawnproto::UNDERTAKER_BADGE));
    let reaper = must(build_child(
        own_ut,
        own_ut,
        reaper_elf,
        &ChildEndowment {
            caps: &[
                (chan.deaths, abi::rights::READ),
                (chan.result_ep, abi::rights::WRITE),
                (reaper_spawn, abi::rights::WRITE),
            ],
            stack_pages: CHILD_STACK_PAGES,
            ..ChildEndowment::new(Retention::Nothing)
        },
    ));
    cap_delete(reaper_spawn);
    must_ok(start_child(reaper, 0, 0, 0));
}

/// The channels and services the spawn service is handed: what every step above kept for it.
#[inline(never)]
fn spawn_channels(
    w: &Wiring,
    chan: SpawnChannel,
    kept: Kept,
    devices: DeviceServices,
    term_sink: Option<u64>,
    (fs, mounted_tree): (Option<Fs>, Option<MountedTreeGrant>),
    (catalog, index_sources): (&'static str, &'static str),
) -> Channels {
    Channels {
        spawn_ep: chan.spawn_ep,
        result_ep: chan.result_ep,
        deaths: chan.deaths,
        own_ut: kept.own_ut,
        jobs_ut: kept.jobs_ut,
        images_ut: kept.images_ut,
        clock_page: w.g.clock_page,
        config_page: w.g.config_page,
        // The terminal's sink, if this initial ramdisk carried an adapter to serve it. This is
        // what a declared second stream gets by default (DECISIONS §67 (a program's second
        // stream is a declaration)): the shell names a file with `2>` and otherwise the bytes
        // go straight to the screen, through a process that can do nothing else with them.
        term_sink,
        fs,
        // The second mounted tree's service and pool (milestone 834 (ioping on nife and Linux)), kept
        // for `fs`'s reason: a directory grant inside it is built from this copy.
        disk: mounted_tree.map(|t| Fs {
            ep: t.ep,
            page: t.pool,
        }),
        // The entropy service's client endpoint, kept so a program a person types can be endowed
        // randomness as `date` is a clock. `None` is not a boot failure.
        entropy: devices.entropy.client,
        // The network stack's client endpoint, if this boot built one.
        network: devices.network.map(|(stack, ..)| stack),
        // The resolver's unbadged endpoint, if this boot started one.
        resolver: devices.resolver,
        // The reboot object, kept for the one program that declares it.
        reboot: is_granted(w.g.reboot).then_some(w.g.reboot),
        catalogue: catalog,
        index_sources,
        run_unvouched: kept.run_unvouched,
        // The boot line discipline's endpoint, for a `graphical_terminal` session's UART arm; see
        // where `term_out` alone is deleted. Unused on every boot that builds no session.
        boot_terminal: w.term.ep,
        // The session's measured images and the boot's verdict on the devices. `held` is false on
        // every boot with no gpu, and a `graphical_terminal` request there is refused.
        screen: devices.screen,
    }
}
