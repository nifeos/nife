//! The boot sequence: `boot`, which builds the interactive system in numbered steps and then
//! becomes its spawn service, and the last steps it takes before that (what it says before the
//! prompt, and the undertaker). Moved out of `lib.rs` unchanged but for visibility by milestone 843
//! (the system-initializer crate root is split along its seams), under §279 (the system-initializer
//! crate root splits into private modules). `boot` is now split along its steps, by the
//! long-function paydown lane (2026-10-12 UTC); the building steps and the password helpers are in
//! `boot_steps`.
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
/// about to exist, which is why its fields are named for what they are. A grant a boot lacks (no
/// disk, no rng, no GPU, no NIC) is an empty slot that `boot` probes, not a failure. The real
/// table is `components/src/progenitor.rs`'s. `no_run`, because it only runs at EL0 under
/// `script/swish-check`.
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

    // `graphical_terminal_grants` ran at the top of this function, before the entropy block: the devices'
    // caps sit in their kernel-granted slots until the shell's build places them, and nothing
    // graphical is built here on any boot (milestone 632 (provisional)).
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

    // 4. The shell: prints and reads lines through the terminal, holds the spawn channel, and holds
    // its own untyped budget (slot 3) so `run --mem N` grants from memory that is genuinely the
    // shell's. WRITE lets it SPLIT the budget; GRANT lets it delegate the split to the progenitor. We carve
    // that budget from our own untyped and hand it over the same way we hand any capability.
    //
    // Slot 4 is the filesystem when this boot has one, which is the whole of what `>` and `<` need
    // (milestone 50, notes/pipes.md): the shell resolves a redirection against it and writes the
    // file itself. Narrowed to WRITE, which on an endpoint is the right to CALL, and without GRANT,
    // so the shell can hand it to nobody.
    //
    // **And a read-only clock last** (milestone 86), which is what `time <command>` measures with.
    // It is [`BootEndowment::clock_page`], the same frame this progenitor was granted and hands to a child
    // whose manifest declares a clock; the shell is simply another holder of a narrowed view. `READ`
    // and no `GRANT`, deliberately: the shell can read the wall clock and can hand one to nothing it
    // spawns, so which processes can read the time is still decided by the manifests this crate
    // reads (DECISIONS §43) rather than by anything typed at a prompt.
    //
    // Last in the list, so a boot with no filesystem takes exactly the path it took before this
    // existed. Its slot therefore moves (4 without a disk, 5 with one), which is why the shell is
    // *told* the number in `x2`/`a2` instead of assuming one; see swish.rs's `CLOCK_SLOT`.
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

    // Free every boot cap the spawn service does not need, so the progenitor's 16-slot capability table has room to
    // build a supervised child (which holds a job untyped and a job frame while the loader retypes
    // an address space, frames, and a TCB). The drivers and the shell hold the narrowed copies that matter.
    //
    // **Only the input frame, and the two that stay have a reason each.** `term_ep` is still ours to
    // delegate: the sink adapter below is handed `WRITE` on it, and the drop announcement is a
    // `CALL` on it. `term_out` is where that announcement stages its bytes. Both go back the moment
    // their last use is done, and after that this process holds no way to reach the terminal at all.
    cap_delete(term.input);
    // **The filesystem stays** (milestone 31 phase 3, 2026-08-17). It used to go here, with a
    // comment saying "the day `rm` is reachable from the prompt, the progenitor keeps the endpoint instead,
    // because building a `fs_subtree_caretaker` is its job and not the shell's". This is that day.
    //
    // Two slots held for the life of the boot, and it is worth being precise about what they buy and
    // what they cost. They buy the only delivery mechanism a directory grant has: the caretaker must
    // hold the file service to attenuate it, the shell's copy carries no `GRANT`, and a program
    // spawned without the capability its command line named would be the worst failure this model
    // has. They cost two of the progenitor's sixteen capability table slots, permanently, which takes the spawn service's
    // resting endowment from seven capabilities to nine and its peak from thirteen to fifteen. That
    // peak is the number to watch: it is a directory-granted spawn, and it is one slot from the wall.
    // See `spawn_dir_grant`, which counts it.
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

    // Role 0 (the prompt), and `arg1` is the rights its directory capability carries. A shell told 0
    // holds no directory and says so at every verb that would need one. `arg2` is the clock slot
    // (milestone 86), which moves with whether this boot had a disk, so it is told rather than
    // assumed.
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
    // **And what the measurement decided** (milestone 104), on the same terms as the line above: a
    // claim about what the progenitor refuses is worth what the check behind it is worth, and only the progenitor can
    // run that check. The affirmative line is the load-bearing one. A measured boot's natural bug is
    // for the check to evaporate when the build step does not run, and a boot that says nothing
    // looks exactly like a boot that measured everything, so `script/swish-check` reads this
    // sentence and a boot that stopped measuring fails the gate instead of passing quietly.
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
    // **How directory grants will be delivered on this boot** (milestone 606, rulings D and T1),
    // said once, so `script/swish-check` can hold the boot to it: a server whose own measured bytes
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
    // **The entropy service's own outcome** (DECISIONS §120's 2026-08-26 amendment), said here
    // rather than where it was decided: `entropy_ready` was set long before this process had a
    // terminal at all (the whole reason the build happens first, at the top of `boot`; see
    // that block's own comment), and [`PROGENITOR_SHELL_OUTPUT_VIRTUAL_ADDRESS`] is not mapped into the progenitor's own space until the
    // line above this one first used it, so nothing earlier in `boot` could `announce`
    // regardless. Silent when there was nothing to build (no device, or no `entropy` program in
    // the archive), the same posture the sink adapter and the subtree caretaker already take for a
    // missing component.
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
    // **The network stack's outcome, and the resolver's** (milestones 590 and 892), said here for
    // `entropy_ready`'s reason; [`announce_network`] has both sentences.
    announce_network(term_ep, network, resolver);
    if login.ready {
        announce_login_credentials(term_ep, &login.password);
    }
    // **`term_ep` stays past this point now, and `term_out` does not** (milestone 632
    // (provisional)). A `graphical_terminal` session launched with no virtio keyboard takes its keystrokes
    // from the boot's own line discipline, in raw mode, and the only process that can grant a
    // session that endpoint is this one: the shell's copy carries no `GRANT` (deliberately, so
    // nothing at the prompt can hand the terminal, which also carries `OPERATION_READLINE`, to any
    // program it likes; that widening is exactly what milestone 50's sink adapter exists to
    // avoid), and nobody else holds the discipline's endpoint at all. Keeping it is the file
    // service pair's precedent: the spawn service holds exactly the authority a delivery needs,
    // `WRITE | GRANT` on an endpoint it never receives on. One resting slot, which the table has
    // (the peak is inside the login block above, and this capability was live through it anyway).
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
    // **The generated login credential** (milestone 49's boot-wiring update), said here for
    // [`entropy_ready`]'s own reason: `login_ready` was decided long before this process had a
    // terminal. Printed exactly once, before the prompt, the shape a cloud image's generated
    // first-boot password already takes: this process's own copy was wiped the moment
    // `identity_provisioner` used it (see that block's own comment), so this line is the only
    // place the password exists once it scrolls past. Silent when there is no login stack at all
    // (no entropy, no filesystem, or one of the four programs missing/unvouched), the same posture
    // every other optional boot component already takes.
    //
    // **It used to say `init: login ready` and that was the false half of milestone 233.** The
    // measurement behind this line is `identity_provisioner` answering `IDP_RPT_OK`: a credential
    // store exists and it holds this identity and this secret. That is worth reporting and is kept
    // exactly as it was. What it is not is a statement that the `login` process is alive, and for
    // an unknown number of weeks it was not: this process started it with no way to reach the
    // archive it parsed at `_start`, so it faulted before serving anything while this line went on
    // announcing a service. A reader has no way to tell a sentence's evidence from its wording, so
    // the wording now names the evidence.
    //
    // **Nothing here can honestly claim more, and the claim moved rather than being dropped.**
    // Knowing `login` survived means `login` saying so, which is a message, an endpoint and a wait
    // this process does not have, and a boot that blocks on a child's readiness is a boot that
    // hangs when the child is the thing that is broken. The survival claim lives in
    // `script/swish-check` instead, which fails the boot if the kernel reported killing any user
    // thread; see `swish_check_leg`'s own `no user thread was killed` check.
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
    // 6. The undertaker, out of what is left of our own budget. Three capabilities and nothing
    // else: `READ` on the supervision endpoint, so it can free a job's memory and can never spend
    // it; `WRITE` on the result endpoint, so it can say the one word a job the kernel killed cannot
    // say for itself (milestone 235 (a command that faults hangs the prompt),
    // `grant_plan::spawnproto::JOB_FAULTED`); and `WRITE` on a copy
    // of the spawn endpoint badged `spawnproto::UNDERTAKER_BADGE`, so it can say which job it
    // reaped (milestone 685 (a job is finished when its memory is back), `spawnproto::reaped`).
    // `WRITE` without `GRANT` on both: the right to speak on those channels, not to hand them on.
    //
    // Slot order is the contract with the program, which names them `DEATHS` (0), `REPORT` (1) and
    // `SPAWN` (2). The badged copy is a fresh slot of ours, deleted once the build has copied it.
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
        // **The second mounted tree's service and pool** (milestone 834 (ioping on nife and Linux)), kept for the life of
        // the boot for `fs`'s reason: a directory grant inside it is built from this copy.
        disk: mounted_tree.map(|t| Fs {
            ep: t.ep,
            page: t.pool,
        }),
        // **The client view of the entropy service, if this boot has one** (milestone 111).
        // The same endpoint the login stack drew its password from, kept rather than dropped,
        // so a program a *person* types can be endowed randomness the same way `date` is
        // endowed a clock. `None` here is not a boot failure: it is a `uuid` that says on its
        // second stream that it holds no entropy capability, which is the true sentence.
        entropy: devices.entropy.client,
        // **The stack's client endpoint, if this boot built one** (milestone 590
        // (provisional)), `entropy`'s shape one service over.
        network: devices.network.map(|(stack, ..)| stack),
        // **The resolver's unbadged endpoint, if this boot started one** (milestone 892).
        resolver: devices.resolver,
        // **The reboot object** (milestone 805), kept for the one program that declares it.
        reboot: is_granted(w.g.reboot).then_some(w.g.reboot),
        catalogue: catalog,
        index_sources,
        run_unvouched: kept.run_unvouched,
        // **The boot line discipline's endpoint, for a `graphical_terminal` session's UART arm** (milestone
        // 632 (provisional)); see the comment where `term_out` alone is deleted for why this
        // stays ours. Unused, and unheld, on every boot that builds no session.
        boot_terminal: w.term.ep,
        // **The `graphical_terminal` session's measured images and the boot's verdict on the devices**
        // (milestone 632 (provisional)). `held` is false, and the caps are long deleted, on
        // every boot with no gpu; a `graphical_terminal` request on such a boot is refused below.
        screen: devices.screen,
    }
}
