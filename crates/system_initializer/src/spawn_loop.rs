//! The spawn service: `spawn_service`, the loop `boot` ends in, which serves the shell's `run`
//! requests forever, with the caretaker, channel and file-service shapes it carries. Its design is
//! notes/system-initializer.md, "The spawn service".
//!
//! Name: provisional (milestone 843's lane, 2026-10-11 UTC).

use super::*;

/// **The two caretakers a directory grant is built from**: `fs_subtree_caretaker` for a directory
/// and everything under it, `fs_nameset_caretaker` for named entries of one directory (milestone
/// 205). `None` when the archive did not carry it, or it did not measure. Name: provisional.
pub(super) struct Caretakers {
    pub(super) subtree: Option<elf::Elf<'static>>,
    pub(super) nameset: Option<elf::Elf<'static>>,
}

/// The archive entry a spawnable program is loaded from: its own name, for every program.
pub(super) fn archive_name(p: Program) -> Option<&'static str> {
    Some(p.name())
}

/// **Place `cap`, narrowed to `rights`, at `slot` of an unstarted child's table** (`CAP_INSERT` with
/// an explicit target, which the ABI numbers from one). `false` if the slot was taken or the right
/// could not be granted.
pub(super) fn place_at(tcb: u64, cap: u64, rights: u64, slot: u64) -> bool {
    user_mode_runtime::thread_control_block_capability_insert(tcb, cap, rights, slot + 1) >= 0
}

// The slot the run-unvouched capability is placed in sits just under the kernel's reserved fault
// slot; `grant_plan` states the number without depending on `abi`, so the relation is held here.
const _: () = assert!(spawnproto::RUN_UNVOUCHED_SLOT == abi::fault::FAULT_EP_SLOT - 1);

/// Turn a `receive_cap` slot into `Some(slot)`, or `None` if the message carried no capability.
pub(super) fn opt_cap(slot: u64) -> Option<u64> {
    if slot == abi::rendezvous::NO_CAP {
        None
    } else {
        Some(slot)
    }
}

/// Everything the spawn service holds for its whole life, so the loop's signature says what the progenitor's
/// remaining authority *is*: two channels, one supervision endpoint it only ever delegates from, and
/// two bounded budgets. The root construction budget is deliberately not in here; it is gone.
pub(super) struct Channels {
    /// READ: the shell's `run` requests arrive here.
    pub(super) spawn_ep: u64,
    /// WRITE: a child's answer channel, and our own spawn-failed sentinel.
    pub(super) result_ep: u64,
    /// GRANT: placed `READ` in every job's reserved fault slot, so `job_undertaker` collects it.
    pub(super) deaths: u64,
    /// Our own scratch budget: page tables for the loader's scratch window, and nothing else.
    pub(super) own_ut: u64,
    /// The job pool. One region per job, split off here and returned here when the job is reaped.
    pub(super) jobs_ut: u64,
    /// The image pool, [`IMAGE_POOL_PAGES`]: a file run by its path, and a package's staging.
    pub(super) images_ut: u64,
    /// READ on the wall clock, endowed to a child whose manifest declares one (DECISIONS §43).
    pub(super) clock_page: u64,
    /// READ on the inert-configuration page, endowed to a child whose manifest declares
    /// [`grant_plan::Manifest::configuration`] (DECISIONS §111 (inert configuration is a read-only page)). [`clock_page`](Channels::clock_page)'s
    /// twin in every respect: unconditional, read-only, and handed on only where the manifest asks.
    pub(super) config_page: u64,
    /// WRITE-delegable: the endpoint the terminal's sink adapter serves, which is where a declared
    /// second stream goes when the command line named no file for it (DECISIONS §67). `None` when
    /// this initial ramdisk carried no adapter, and then a declaring child simply gets no second
    /// stream. This is authority to *print*, and nothing else: the adapter holds the terminal, we
    /// do not.
    pub(super) term_sink: Option<u64>,
    /// **The file service and the page its clients share with it** (milestone 31 phase 3), or `None`
    /// on a boot with no disk. `WRITE | GRANT` on the endpoint: this is the directory capability a
    /// `fs_subtree_caretaker` attenuates, and the only reason the progenitor keeps it past the boot.
    /// The shell's copy carries no `GRANT`, so the progenitor is the only process here that can
    /// build a caretaker.
    pub(super) fs: Option<Fs>,
    /// **The second mounted tree's service and its client-window pool** (milestone 834 (ioping on nife and Linux)), or `None`
    /// on a boot without one. [`fs`](Channels::fs)'s shape and reason: a directory grant the shell
    /// names through `/disk` is built from this, badged per job
    /// (`grant_plan::spawnproto::in_mounted_tree`).
    pub(super) disk: Option<Fs>,
    /// **A client view of the entropy service** (milestone 111 (endow a child with entropy)),
    /// endowed `WRITE`, the right to `CALL`, to a child whose manifest declares
    /// [`grant_plan::Manifest::entropy`]; the progenitor keeps the full-rights capability it
    /// retyped and never receives on it. `None` on a boot that built no entropy service (no
    /// virtio-rng device, no `entropy` program in the initial ramdisk, or a service that did not
    /// answer `READY`), and then a declaring child gets an empty [`grant_plan::ENTROPY_SLOT`] and
    /// says so rather than drawing predictable bytes. The shell holds none, since nothing it does
    /// as a builtin is unpredictable.
    pub(super) entropy: Option<u64>,
    /// **The network stack's client endpoint** (milestone 590 (provisional)), endowed to a child
    /// whose manifest declares [`grant_plan::Manifest::network`]. [`entropy`](Channels::entropy)'s
    /// shape exactly: the progenitor keeps the full-rights capability it retyped and never receives
    /// on it, and places `WRITE` for a declaring child. `None` on a boot with no NIC or no
    /// `net_stack`, and then a declaring child holds an empty [`grant_plan::NETWORK_SLOT`] and says
    /// so. The shell holds none, for `entropy`'s reason: nothing it does as a builtin reaches the
    /// network.
    pub(super) network: Option<u64>,
    /// **The name resolver's unbadged endpoint** (milestone 892 (a std program at the prompt holds
    /// the network and a resolver)), or `None` on a boot that started none. Never received on: the
    /// spawn service mints each resolving job a badged copy and sends the grant and, at the job's
    /// reap, the forget through it ([`SpawnEndpoint::resolver_for`]).
    pub(super) resolver: Option<u64>,
    /// **The reboot object** (milestone 805, DECISIONS §251), endowed `WRITE` to a child whose
    /// manifest declares [`grant_plan::Manifest::reboot`], and to nothing else. `None` only on a
    /// kernel that did not grant it, and then a declaring child holds an empty
    /// [`grant_plan::REBOOT_SLOT`] and says so. The shell holds none: restarting the machine is not
    /// something a builtin does.
    pub(super) reboot: Option<u64>,
    /// **The image's package catalog**, measured (milestone 198 rung 3a's installer): one
    /// `<stem> <digest>` line per package the image vouches for (`package_archive::CATALOG`).
    /// Empty when the archive carried none or the table refused it, and then every
    /// `package install` is refused as not cataloged.
    pub(super) catalogue: &'static str,
    /// **The image's index sources**, measured (milestone 801 (packages over the internet)):
    /// `package_index::SOURCES_ENTRY`, copied to a package manager holding the index store. Empty
    /// when the archive carried none or the table refused it.
    pub(super) index_sources: &'static str,
    /// **READ on the run-unvouched endpoint** (DECISIONS §219 gate D2): the only receive right on
    /// it. The boot shell and `login` hold `WRITE` at `spawnproto::RUN_UNVOUCHED_SLOT`, and `login`
    /// passes `WRITE` to each session it builds. A request that sets `spawnproto::Wiring::
    /// run_unvouched` is followed by one `SEND` on it, which this process takes with a `RECEIVE`;
    /// arriving is the proof, because nothing but a holder can send here.
    ///
    /// One slot for the life of the boot, retyped after `login`'s build, the boot's capability peak,
    /// so it is not held across it; `script/swish-check`'s `capability slots:` line says whether
    /// that held.
    pub(super) run_unvouched: u64,
    /// **The boot line discipline's endpoint, `WRITE | GRANT`** (milestone 632 (provisional)):
    /// held for one delivery, a `graphical_terminal` session launched with no virtio keyboard, whose program
    /// reads the boot discipline raw over the UART and echoes to its own screen. Never received
    /// on; the file service pair's shape, and the comment at `term_out`'s delete records why the
    /// shell could not hold this instead. Unused on every boot that launches no such session.
    pub(super) boot_terminal: u64,
    /// **The display devices' verdict and the session's measured images** (milestone 632
    /// (provisional)): `held` is false on every boot whose shell got no gpu grants, and then a
    /// `graphical_terminal` request is refused below. The shell, not this process, holds the caps between
    /// boot and launch.
    pub(super) screen: GraphicalTerminalCaps,
}

/// The file service, as the progenitor holds it for the life of the boot.
///
/// A struct rather than two `Option<u64>` fields because the two are one fact: a boot either has a
/// filesystem or it does not, and `Some(ep)` with `None` page is not a state that can exist.
#[derive(Clone, Copy)]
pub(super) struct Fs {
    /// The service endpoint, `WRITE | GRANT`. The directory capability, rooted at the image root.
    pub(super) ep: u64,
    /// The frame every client of that service maps to trade bytes with it.
    pub(super) page: u64,
}

/// The spawn service loop: serve the shell's `run` requests forever, as the ELF loader the shell
/// directs. A spawned program holds only what the shell endowed, so it reaches nothing its command
/// line did not name. A normal job is built in a region of the job pool and supervised on `deaths`;
/// an interruptible one is built from the shell's own job untyped and is not supervised here
/// (§24 (interrupting the foreground process)). See notes/system-initializer.md, "The spawn
/// service".
///
/// `spawnable.progs` is indexed by [`Program::id`], so it is [`grant_plan::PROGRAM_COUNT`] long: a
/// variant added to `grant_plan` without a slot here would be an out-of-bounds read.
pub(super) fn spawn_service(
    c: Channels,
    spawnable: &crate::boot_steps::Spawnable,
    care: Caretakers,
    discipline: SessionDiscipline,
    fs_scoped: bool,
) -> ! {
    let spawn_ep = c.spawn_ep;
    let resolver = c.resolver;
    let files = c.fs.map(|f| f.ep);
    let mut service = Service {
        c,
        progs: &spawnable.progs,
        absent: &spawnable.absent,
        care,
        discipline,
        fs_scoped,
        fs_mapped: false,
        disk_mapped: false,
        spawn: SpawnEndpoint::new(spawn_ep, files, resolver),
    };
    loop {
        service.serve();
    }
}

/// What the spawn service loop carries from one request to the next: the progenitor's remaining
/// authority, the boot's measured images, and the two pieces of state a request can change.
struct Service<'a> {
    c: Channels,
    progs: &'a [Option<elf::Elf<'a>>; grant_plan::PROGRAM_COUNT],
    absent: &'a [bool; grant_plan::PROGRAM_COUNT],
    care: Caretakers,
    discipline: SessionDiscipline,
    fs_scoped: bool,
    /// Whether the file page is mapped here yet, for the installed-programs table (first image
    /// request).
    fs_mapped: bool,
    /// Whether the second mounted tree's pool is mapped here yet, for zeroing a window between jobs.
    disk_mapped: bool,
    /// The spawn endpoint, its reaped messages, and the windows they free (milestone 685 (a job is
    /// finished when its memory is back)).
    spawn: SpawnEndpoint,
}

/// A directory grant's two data messages: the caretaker's words, then the child's.
type GrantWords = ((u64, u64, u64), (u64, u64, u64));

/// One spawn request as read off the endpoint, in protocol order: its words, its data messages,
/// its frames, and every capability the shell delegated with it.
struct Request {
    w0: u64,
    badge: u64,
    arg: u64,
    mem_pages: u64,
    wiring: spawnproto::Wiring,
    interruptible: bool,
    prog: Option<Program>,
    grant: Option<GrantWords>,
    frames: Option<Frames>,
    args_seen: Option<u64>,
    set_seen: Option<u64>,
    delegated: Delegated,
    presented: bool,
}

/// The capabilities the shell delegated with a request, each `None` when it was not promised or
/// did not arrive.
struct Delegated {
    job_ut: Option<u64>,
    job_fr: Option<u64>,
    sink: Option<u64>,
    source: Option<u64>,
    diagnostics: Option<u64>,
    screen: Option<u64>,
    budget: Option<u64>,
    machine_page: Option<u64>,
}

/// What a request resolved to before anything is built: the one manifest, the word a refusal
/// answers with, the image's parsed bytes when it is one, and the verb read from its argv.
struct Resolved {
    manifest: Option<grant_plan::Manifest>,
    failure: u64,
    image_elf: Option<elf::Elf<'static>>,
    /// The digest of an image the live generation vouched for, which its zone grants are keyed on
    /// (milestone 892).
    image_digest: Option<measured_boot::Digest>,
    verb: [u8; grant_plan::MAX_VERB],
    verb_len: Option<usize>,
}

impl Resolved {
    /// The verb the line's argv carried, if it carried one.
    fn verb(&self) -> Option<&[u8]> {
        self.verb_len.map(|n| &self.verb[..n])
    }
}

/// The authorities a manifest declares that nothing on a command line can designate.
#[derive(Clone, Copy)]
struct Wants {
    clock: bool,
    domain: bool,
    config: bool,
    entropy: bool,
    network: bool,
    machine: bool,
    share: bool,
    reboot: bool,
    sync: bool,
    console_input: bool,
}

impl Wants {
    /// Read each declaration from the program's own manifest, never from the request: a person
    /// designates none of these, so no bit on the wire carries them. See
    /// notes/system-initializer/the-spawn-service.md, "What a manifest declares that no command line can".
    fn of(manifest: Option<grant_plan::Manifest>) -> Wants {
        let wants_clock = manifest.is_some_and(|m| m.clock);
        let wants_domain = manifest.is_some_and(|m| m.domain);
        let wants_config = manifest.is_some_and(|m| m.configuration);
        let wants_entropy = manifest.is_some_and(|m| m.entropy);
        let wants_network = manifest.is_some_and(|m| m.network);
        let wants_machine = manifest.is_some_and(|m| m.machine);
        let wants_share = manifest.is_some_and(|m| m.share);
        // `grant_plan::image_can_carry` keeps this off every image, so only the built-in `reboot`.
        let wants_reboot = manifest.is_some_and(|m| m.reboot);
        let wants_sync = manifest.is_some_and(|m| m.sync);
        Wants {
            clock: wants_clock,
            domain: wants_domain,
            config: wants_config,
            entropy: wants_entropy,
            network: wants_network,
            machine: wants_machine,
            share: wants_share,
            reboot: wants_reboot,
            sync: wants_sync,
            console_input: manifest.is_some_and(|m| m.console_input),
        }
    }
}

/// A package manager's installer and reply endpoints, its catalog page, and whether each of the
/// two that its manifest declared failed.
type PackageParts = (Option<(u64, u64)>, Option<u64>, bool, bool);

/// What a `std` child is placed beyond [`Wants`]: its argv page, installer endpoints, catalog page,
/// resolver badges, index sources page, standard input and the line's input stream.
type StdParts = (
    Option<u64>,
    Option<(u64, u64)>,
    Option<u64>,
    Resolvers,
    Option<u64>,
    Option<u64>,
    Option<u64>,
);

/// The capabilities a normal job's directory grant and sync-only grant were wired from: its label,
/// its window, the bound or caretaker channel, and what the child is handed in slot 0 and slot 8.
struct DirWiring {
    label: u64,
    window: Option<u64>,
    bound: Option<Fs>,
    channel: Option<Fs>,
    narrowed: Option<u64>,
    dir_failed: bool,
    sync_ep: Option<u64>,
    sync_failed: bool,
    /// Whether `narrowed` is the machine's index store rather than a directory the line named
    /// (milestone 801 (packages over the internet)).
    index_store: bool,
    /// Whether `window` is the second mounted tree's (milestone 834) rather than the file
    /// service's, which is the pool it goes back to.
    mounted: bool,
}

/// A native child's endowment: its in-order capabilities, its named slots, and its mappings.
struct NativeEndowment {
    in_order: [(u64, u64); 6],
    n: usize,
    placed_buf: [(u64, u64, u64); 8],
    placed_n: usize,
    map_buf: [(u64, u64, u64); 4],
    map_n: usize,
}

/// What a normal job holds that the progenitor must give back or delete once it has started (or
/// failed to): everything [`Service::finish_job`] tidies.
struct JobLeftovers {
    dir: DirWiring,
    installer: Option<(u64, u64)>,
    /// The badged copies of the resolver's endpoint the child was placed, one per zone (milestone
    /// 892).
    resolvers: Resolvers,
    /// The badged copy of the terminal endpoint the child was placed as its stdin (milestone 892).
    stdin: Option<u64>,
    catalog_page: Option<u64>,
    /// The child's copy of the image's index sources (milestone 801).
    sources_page: Option<u64>,
    args_page: Option<u64>,
    region: Option<u64>,
    labeled: Option<u64>,
}

impl Service<'_> {
    /// Serve one request on the spawn endpoint, whatever its shape, to its answer.
    fn serve(&mut self) {
        let Channels {
            result_ep,
            own_ut,
            images_ut,
            fs,
            ..
        } = self.c;
        let (w0, w1, w2, badge) = self.spawn.request();
        // An installer endpoint's request (§270 (a package manager holds an installer endpoint, not
        // the spawn endpoint)): nothing past this first message is read, and the answer goes to the
        // holder's own reply endpoint, because the shell is reading `result_ep` meanwhile.
        if let Some(label) = spawnproto::installer_label(badge) {
            serve_installer(
                label,
                (w0, w1, w2),
                &mut self.spawn,
                (own_ut, images_ut, fs, self.c.catalogue),
                &mut self.fs_mapped,
            );
            return;
        }
        // An edit to the installed-programs table, not a spawn: asked first, because under this bit
        // no other bit of word 2 means anything.
        if let Some(verb) = spawnproto::activation(w1, w2) {
            let answer = activate(
                verb,
                w0,
                &mut self.spawn,
                &Activating {
                    own_ut,
                    images_ut,
                    fs,
                    catalog: self.c.catalogue,
                    manager: installed_programs::OWNER,
                },
                &mut self.fs_mapped,
            );
            // Only a vouch arrives here, and a vouch's answer names no member and no holder.
            let (r0, r1, r2) = spawnproto::activation_reply(answer.status, answer.live, 0);
            send(result_ep, r0, r1, r2);
            return;
        }
        let req = self.read_request((w0, w1, w2), badge);
        let (image_region, staging) = self.carve_image(&req);

        // Either half alone routes here as well, to be refused rather than half-endowed.
        if req.wiring.graphics || req.prog == Some(Program::GraphicalTerminal) {
            self.launch_graphical_terminal(&req);
            return;
        }

        let job = self.resolve(&req, staging);
        let progs = self.progs;
        let elf = if req.wiring.image {
            job.image_elf.as_ref()
        } else {
            req.prog.and_then(|p| progs[p.id() as usize].as_ref())
        };

        if req.interruptible {
            self.spawn_interruptible(&req, elf, job.failure);
        } else {
            self.spawn_job(&req, &job, elf, image_region, staging);
        }

        // Drop our copies of every delegated capability, built or not: the child and the shell hold
        // their own, and the progenitor's table must not fill across a long session.
        for s in [
            req.delegated.job_ut,
            req.delegated.job_fr,
            req.delegated.sink,
            req.delegated.source,
            req.delegated.diagnostics,
            req.delegated.screen,
            req.delegated.budget,
            req.delegated.machine_page,
        ]
        .into_iter()
        .flatten()
        {
            cap_delete(s);
        }
    }

    /// Read the rest of a spawn request off the endpoint, in the order the shell sent it: the
    /// directory grant's words, the image's frames, the argv and name set, then every capability.
    fn read_request(&mut self, (w0, w1, w2): (u64, u64, u64), badge: u64) -> Request {
        let Channels {
            own_ut,
            run_unvouched,
            ..
        } = self.c;
        let spawn = &mut self.spawn;
        let arg = spawnproto::program_argument(w1);
        let mem_pages = spawnproto::memory_grant_pages(w2);
        let wiring = spawnproto::wiring(w2);
        let interruptible = wiring.interruptible;
        // **Under `IMAGE_BIT` word 0 is a length, not a program** (DECISIONS §219 option D), so no
        // row of `progs` is consulted: the program is whatever the bytes are.
        let prog = if wiring.image {
            None
        } else {
            Program::from_id(spawnproto::prog_id(w0))
        };

        // The directory grant's two data messages come before any capability, and are drained here
        // because the shell has already sent them: left behind, the next command would read them.
        let grant = wiring
            .directory_grant
            .then(|| (spawn.receive(), spawn.receive()));

        // The image's frames, after the data and before every other capability (option D of §219
        // (how the shell names an installed program to the spawner)): mapped now, copied only once
        // the request is read to its end, since a carve may wait for a reap ([`take_frames`]).
        // Any shape but a plain line is drained and refused.
        let frames = wiring
            .image
            .then(|| take_frames(spawn, spawnproto::image_len(w0), own_ut));

        // The argv's frame, then the name set's (milestone 205 (how a foreign program is told what
        // to do)): taken whenever announced, whatever the program, so both sides stay in lockstep.
        let args_seen = if wiring.program_arguments {
            receive_args(spawn, own_ut)
        } else {
            None
        };
        let set_seen = if wiring.nameset {
            receive_args(spawn, own_ut)
        } else {
            None
        };

        let delegated = receive_delegated(spawn, wiring, mem_pages);
        // The run-unvouched proof (§219 gate D2) is the last message of any request that claimed it,
        // taken whether or not this one needs it so the caller's `SEND` never waits.
        let presented = wiring.run_unvouched && {
            receive(run_unvouched);
            true
        };
        Request {
            w0,
            badge,
            arg,
            mem_pages,
            wiring,
            interruptible,
            prog,
            grant,
            frames,
            args_seen,
            set_seen,
            delegated,
            presented,
        }
    }

    /// Carve an image request's child region and staging region from the image pool, and copy its
    /// frames into the staging region. The child's region is a `std` one when the argv bit is set,
    /// sized as the image (`grant_plan::image_region_pages`); see
    /// notes/system-initializer/the-spawn-service.md, "Images".
    fn carve_image(&mut self, req: &Request) -> (Option<u64>, Option<u64>) {
        let Channels {
            own_ut, images_ut, ..
        } = self.c;
        let Request {
            w0,
            wiring,
            interruptible,
            ..
        } = *req;
        let spawn = &mut self.spawn;
        // The child's region first, then the staging region, so staging is the pool's top carve
        // when it is destroyed and its pages go back: a region returns its pages to its parent only
        // when it is the most recent carve (`memory_regions`' `return_to_parent`).
        let image_region = if wiring.image
            && !interruptible
            && (!wiring.directory_grant || wiring.program_arguments)
        {
            spawn.split(
                images_ut,
                grant_plan::image_region_pages(
                    spawnproto::image_pages(spawnproto::image_len(w0)),
                    if wiring.program_arguments {
                        grant_plan::StartupContract::Std
                    } else {
                        grant_plan::StartupContract::Native
                    },
                    JOB_REGION_PAGES,
                ),
            )
        } else {
            None
        };
        let staging = req.frames.as_ref().and_then(|f| {
            stage_frames(
                spawn,
                f,
                spawnproto::image_len(w0),
                own_ut,
                images_ut,
                image_region.is_some(),
            )
        });
        (image_region, staging)
    }

    /// Answer a `graphical_terminal` request: refuse a malformed one or a boot with no display, and
    /// otherwise build the session from its own region of the job pool, with narrowed copies of the
    /// devices this process holds. A session that does not come up is a refused spawn, and its
    /// builder reclaims the region. See notes/system-initializer/the-spawn-service.md.
    fn launch_graphical_terminal(&mut self, req: &Request) {
        let Channels {
            result_ep,
            deaths,
            own_ut,
            jobs_ut,
            boot_terminal,
            ..
        } = self.c;
        let graphical_terminal_caps = &self.c.screen;
        let Request { prog, wiring, .. } = *req;
        // Neither of these is the session's: a `graphical_terminal` manifest forbids both, but a
        // caller could still have set them.
        if let Some(b) = req.delegated.budget {
            cap_delete(b);
        }
        if let Some(p) = req.delegated.machine_page {
            cap_delete(p);
        }
        let answer =
            if prog != Some(Program::GraphicalTerminal) || !wiring.graphics || wiring.keyboard {
                spawnproto::SPAWN_FAILED
            } else if !graphical_terminal_caps.held {
                spawnproto::SPAWN_NO_DISPLAY
            } else {
                let built = match (
                    self.progs[Program::GraphicalTerminal.id() as usize].as_ref(),
                    self.spawn.split(jobs_ut, GRAPHICAL_TERMINAL_SESSION_PAGES),
                ) {
                    (Some(elf), Some(region)) => build_graphical_terminal_session(
                        &GraphicalTerminalLaunch {
                            own_ut,
                            region,
                            deaths,
                            boot_terminal,
                            result_ep,
                            gpu: &graphical_terminal_caps.gpu,
                            kbd: graphical_terminal_caps
                                .keyboard
                                .then_some(&graphical_terminal_caps.kbd),
                            program: elf,
                        },
                        graphical_terminal_caps,
                        &self.discipline,
                    ),
                    _ => Err(()),
                };
                // A session that came up answers for itself; only a failure is ours to say.
                match built {
                    Ok(()) => return,
                    Err(()) => spawnproto::SPAWN_FAILED,
                }
            };
        send(result_ep, answer, 0, 0);
    }

    /// Decide what the request is: vouch for and parse an image, read the line's verb, and settle
    /// the one manifest and the word a refusal answers with. A vouched image is endowed from its
    /// own note, an unvouched one only for a caller that presented the run-unvouched capability and
    /// then with `grant_plan::UNVOUCHED_MANIFEST` (§219 D); a program with a verb table is narrowed
    /// by the line's verb. See notes/system-initializer/the-spawn-service.md, "Images".
    fn resolve(&mut self, req: &Request, staging: Option<u64>) -> Resolved {
        let Channels { own_ut, fs, .. } = self.c;
        let Request {
            w0,
            arg,
            mem_pages,
            wiring,
            prog,
            args_seen,
            presented,
            ..
        } = *req;
        let mut unvouched = false;
        let mut refused_by_manifest = false;
        let mut image_manifest = None;
        let mut image_digest = None;
        let image_elf = staging.and_then(|_| {
            let bytes = staged_image(spawnproto::image_len(w0));
            let (vouch, digest) = vouched(bytes, fs, own_ut, &mut self.fs_mapped);
            unvouched = vouch == grant_plan::Vouch::Unvouched;
            image_digest = (!unvouched).then_some(digest);
            let elf = if !unvouched || presented {
                elf::Elf::parse(bytes).ok()
            } else {
                None
            };
            let elf = elf?;
            match endowed_image(&elf, vouch, arg, mem_pages, wiring.program_arguments) {
                Some(m) => {
                    image_manifest = Some(m);
                    Some(elf)
                }
                None => {
                    refused_by_manifest = true;
                    None
                }
            }
        });
        // The verb is read from the shell's page before the region is split, which an installer
        // holder's manifest sizes, and checked again against the child's copy ([`copy_args`]).
        let mut verb = [0u8; grant_plan::MAX_VERB];
        let verb_len = args_seen.and_then(|theirs| {
            // SAFETY: `receive_args` mapped the shell's frame read-only at `theirs`, one page, and
            // it stays mapped until the shell reclaims its frame after this request's answer.
            let page = unsafe {
                core::slice::from_raw_parts(theirs as *const u8, spawnproto::IMAGE_PAGE as usize)
            };
            let mut read = [0u8; grant_plan::MAX_VERB];
            let w = grant_plan::verb_of_page(page, &mut read)?;
            verb[..w.len()].copy_from_slice(w);
            Some(w.len())
        });
        let manifest = if wiring.image {
            image_manifest
        } else {
            let verb = verb_len.map(|n| &verb[..n]);
            prog.map(|p| grant_plan::narrow_by_verb(&p.manifest(), p.verbs(), verb))
        };
        let failure = if unvouched && !presented {
            spawnproto::SPAWN_UNVOUCHED
        } else if !wiring.image && prog.is_some_and(|p| self.absent[p.id() as usize]) {
            spawnproto::SPAWN_NOT_IN_IMAGE
        } else if refused_by_manifest {
            spawnproto::SPAWN_REFUSED_BY_MANIFEST
        } else {
            spawnproto::SPAWN_FAILED
        };
        Resolved {
            manifest,
            failure,
            image_elf,
            image_digest,
            verb,
            verb_len,
        }
    }

    /// Build and start an interruptible job from the shell's own job untyped, and answer it.
    fn spawn_interruptible(&self, req: &Request, elf: Option<&elf::Elf>, failure: u64) {
        let Channels {
            result_ep, own_ut, ..
        } = self.c;
        // Built from the shell's job untyped with only the job frame mapped: it reports through the
        // frame, and `SPAWN_OK` is the shell's go-ahead to start watching it.
        let built = match (elf, req.delegated.job_ut, req.delegated.job_fr) {
            (Some(e), Some(job), Some(fr)) => build_child(
                own_ut,
                job,
                e,
                &ChildEndowment {
                    maps: &[(CHILD_JOB_PAGE_FRAME_VA, fr, abi::address_space::MAP_RW)],
                    stack_pages: CHILD_STACK_PAGES,
                    ..ChildEndowment::new(Retention::Requester {
                        reason: "the shell suspends its supervised job on ^Z (DECISIONS §309)",
                    })
                },
            )
            .ok(),
            _ => None,
        };
        match built {
            Some(child) => {
                let tcb = child.tcb;
                let ok = start_child(child, 0, req.arg, 0);
                send(
                    result_ep,
                    if ok {
                        spawnproto::SPAWN_OK
                    } else {
                        spawnproto::SPAWN_FAILED
                    },
                    0,
                    0,
                );
                // The TCB is the one message after `SPAWN_OK` (§309 (a supervised job's
                // `ThreadControlBlock` goes to the shell that asked for it)), which the shell takes
                // first, so this `SEND` cannot strand. A job that did not start owes nothing.
                if ok {
                    hand_to_requester(tcb, result_ep);
                } else {
                    forget_thread_control_block(tcb);
                }
            }
            None if failure == spawnproto::SPAWN_NOT_IN_IMAGE => {
                send(result_ep, failure, 0, 0);
            }
            None => {
                send(result_ep, spawnproto::SPAWN_FAILED, 0, 0);
            }
        }
    }

    /// Build, start and answer a normal job: its region, its grants, its endowment in the native or
    /// the `std` layout, then the tidy-up and the answer. A grant the job should hold and could not
    /// be given refuses the spawn. See notes/system-initializer/the-spawn-service.md, "A normal job's
    /// region and grants".
    fn spawn_job(
        &mut self,
        req: &Request,
        job: &Resolved,
        elf: Option<&elf::Elf>,
        image_region: Option<u64>,
        staging: Option<u64>,
    ) {
        let own_ut = self.c.own_ut;
        let manifest = job.manifest;
        let wants = Wants::of(manifest);
        // The region comes before any grant: a directory grant's caretaker is built from the same
        // region as the program it serves (§92 (a caretaker is supervised by the client it serves)).
        let std_layout = manifest.is_some_and(|m| m.startup == grant_plan::StartupContract::Std);
        let wants_installer = manifest.is_some_and(|m| m.installer);
        let region =
            self.split_job_region(req, manifest, image_region, (std_layout, wants_installer));
        // The index store (milestone 801 (packages over the internet), ruling I2) takes the `std`
        // layout's one directory slot, so only a line that named none, on the owner's console.
        let store = manifest
            .and_then(|m| match m.activation_directory {
                true => Some(ActivationGrant::ReadOnly),
                false => m.indexes.rights().map(ActivationGrant::IndexStore),
            })
            .filter(|_| std_layout && !req.wiring.directory_grant && req.badge == 0);
        let dir = self.wire_directory(req, manifest, region, (wants.sync, store));
        let sources_page = match (dir.index_store, region) {
            (true, Some(r)) => copy_text(own_ut, r, self.c.index_sources),
            _ => None,
        };
        let (installer, catalog_page, installer_failed, catalog_failed) = self.package_parts(
            req,
            manifest,
            region,
            (std_layout, wants_installer),
            dir.label,
        );
        // Resolver badges only under `job_undertaker`, whose reap forgets the grants; a
        // screen-narrowed job is reaped by the shell (milestone 892's BUGS).
        let eligible = std_layout && wants.network && req.delegated.screen.is_none();
        let (resolvers, resolver_failed) = if eligible {
            self.resolvers_for(req, job, elf, dir.label)
        } else {
            (NO_RESOLVERS, false)
        };
        // A screen-narrowed job's keystrokes are its session's, so it gets no console input.
        let wants_stdin = std_layout && wants.console_input && req.delegated.screen.is_none();
        let stdin = wants_stdin
            .then(|| badged(self.c.boot_terminal, line_editor::proto::STDIN_BADGE).ok())
            .flatten();
        let stdin_failed = wants_stdin && stdin.is_none();

        let out = self.output(req);
        let native = self.native_endowment(req, manifest, &wants, &dir, out);
        // The argv copy is carved from the job's region so its reclaim frees the page; a copy that
        // could not be made fails the spawn rather than running without the line's words.
        let args_page = match (std_layout, region, req.args_seen) {
            (true, Some(r), Some(theirs)) => copy_args(own_ut, r, theirs, |copy| {
                grant_plan::verb_of_page(copy, &mut [0u8; grant_plan::MAX_VERB]) == job.verb()
            }),
            _ => None,
        };
        let args_failed = std_layout && req.args_seen.is_some() && args_page.is_none();
        let std_parts = match (std_layout, region) {
            (true, Some(r)) => {
                let parts = (
                    args_page,
                    installer,
                    catalog_page,
                    resolvers,
                    sources_page,
                    stdin,
                    req.delegated.source,
                );
                Some(self.std_layout(r, out, &wants, &dir, parts))
            }
            _ => None,
        };
        // `deaths` badged with the job's label (§148 (resolves by asking the kernel)). No label, no
        // build: a job not recognized at its reap would hold its window for the rest of the boot.
        let labeled = if req.delegated.screen.is_none() {
            badged(self.c.deaths, dir.label).ok()
        } else {
            None
        };
        let fault = req.delegated.screen.or(labeled);
        let elf = elf.filter(|_| {
            !dir.dir_failed
                && !dir.sync_failed
                && !args_failed
                && !installer_failed
                && !catalog_failed
                && !resolver_failed
                && !stdin_failed
                && fault.is_some()
        });
        let built = build_job(
            own_ut,
            (elf, region),
            (std_layout, std_parts.as_ref()),
            &native,
            fault,
        );
        // `build_child` copied every segment out of staging, and destroying it now returns its
        // pages, since it is the pool's top carve.
        if let Some(st) = staging {
            supervision_protocol::memory_region_destroy(st);
            cap_delete(st);
        }
        let ok = start_job(built, req, std_layout);
        let leftovers = JobLeftovers {
            dir,
            installer,
            resolvers,
            stdin,
            catalog_page,
            sources_page,
            args_page,
            region,
            labeled,
        };
        self.finish_job(req, ok, job.failure, leftovers);
    }

    /// Split a normal job's region: the image's own, carved before its frames, or one from the
    /// job pool (the image pool for an installer holder) sized by its layout.
    fn split_job_region(
        &mut self,
        req: &Request,
        manifest: Option<grant_plan::Manifest>,
        image_region: Option<u64>,
        (std_layout, wants_installer): (bool, bool),
    ) -> Option<u64> {
        let Channels {
            jobs_ut, images_ut, ..
        } = self.c;
        if req.wiring.image {
            // Split before the frames were taken; see `image_region` above.
            image_region
        } else if let (true, Some(m)) = (std_layout, manifest) {
            self.spawn.split(
                if wants_installer { images_ut } else { jobs_ut },
                grant_plan::named_std_region_pages(&m),
            )
        } else {
            self.spawn.split(
                jobs_ut,
                if req.wiring.directory_grant {
                    DIR_JOB_REGION_PAGES
                } else {
                    JOB_REGION_PAGES
                },
            )
        }
    }

    /// Make a package manager's installer endpoint and reply endpoint, and its copy of the
    /// image's catalog, each only when its manifest declares it; and say whether either failed.
    /// The installer is placed only for the owner's console, in the `std` layout (§270 (a package
    /// manager holds an installer endpoint, not the spawn endpoint)); see
    /// notes/system-initializer/the-spawn-service.md.
    fn package_parts(
        &mut self,
        req: &Request,
        manifest: Option<grant_plan::Manifest>,
        region: Option<u64>,
        (std_layout, wants_installer): (bool, bool),
        label: u64,
    ) -> PackageParts {
        let Channels {
            own_ut, spawn_ep, ..
        } = self.c;
        let wants_catalog = manifest.is_some_and(|m| m.catalog);
        let (installer, catalog_page) = match (std_layout, region) {
            (true, Some(r)) => package_manager_parts(
                r,
                (
                    wants_installer && req.badge == 0 && self.spawn.has_reply_room(),
                    wants_catalog,
                ),
                (spawn_ep, label),
                own_ut,
                self.c.catalogue,
            ),
            _ => (None, None),
        };
        let installer_failed = wants_installer && installer.is_none();
        let catalog_failed = wants_catalog && catalog_page.is_none();
        (installer, catalog_page, installer_failed, catalog_failed)
    }

    /// Wire a normal job's directory grant and sync-only grant: take its label and window, bind a
    /// badge where the server enforces grants itself or build a caretaker otherwise, and say whether
    /// a declared grant failed. The window is held until the job's reap. See
    /// notes/system-initializer/the-spawn-service.md, "A normal job's region and grants".
    fn wire_directory(
        &mut self,
        req: &Request,
        manifest: Option<grant_plan::Manifest>,
        region: Option<u64>,
        (wants_sync, store): (bool, Option<ActivationGrant>),
    ) -> DirWiring {
        let Channels {
            own_ut, fs, disk, ..
        } = self.c;
        let Request {
            wiring,
            grant,
            set_seen,
            ..
        } = *req;
        // A grant in the second mounted tree is a caretaker over its own pool, never a bound badge:
        // that server enforces no subtree.
        if let (true, Some((care_words, _)), Some(tree)) = (wiring.directory_grant, grant, disk)
            && let Some(care_words) = mounted_grant(care_words)
        {
            return self.wire_mounted(req, manifest, region, tree, care_words, wants_sync);
        }
        let spawn = &mut self.spawn;
        let label = spawn.label();
        // Only a server that enforces bindings itself can bind the index store: its path is longer
        // than a caretaker's name.
        let index_store = !wiring.directory_grant && self.fs_scoped && store.is_some();
        // A sync-only grant takes a window too, bound sync-only and held until the reap.
        let window = if (wiring.directory_grant || wants_sync || index_store) && fs.is_some() {
            spawn.until_reaped(|s| s.windows.take(label))
        } else {
            None
        };
        let bound = match (index_store, fs, store, window) {
            (true, Some(f), Some(store), Some(w)) => bound_index_store(
                f,
                (&mut spawn.windows, w),
                own_ut,
                &mut self.fs_mapped,
                store,
            ),
            _ => None,
        };
        let bound = match (wiring.directory_grant && self.fs_scoped, fs, grant, window) {
            _ if bound.is_some() => bound,
            (true, Some(f), Some((care_words, _)), Some(w)) => bound_channel(
                f,
                (&mut spawn.windows, w),
                own_ut,
                &mut self.fs_mapped,
                care_words,
            ),
            _ => None,
        };
        let channel = match (bound, fs, window) {
            (Some(b), ..) => Some(b),
            (None, Some(f), Some(w)) if wiring.directory_grant => {
                job_channel(f, w, own_ut, &mut self.fs_mapped)
            }
            _ => None,
        };
        let narrowed = if let Some(b) = bound {
            match (set_seen, region, grant) {
                // The job holds the file server's own endpoint, badged and bound: no caretaker.
                (None, ..) => Some(b.ep),
                // **A set of names is served over the bound channel**: the badge is bound to the
                // whole directory and the set is what narrows it, so `fs_nameset_caretaker` sits
                // between, told the bound root is its directory (`at_bound_root`).
                (Some(_), Some(r), Some((care_words, _))) => build_grant(
                    own_ut,
                    r,
                    b,
                    &self.care,
                    at_bound_root(care_words),
                    set_seen,
                    manifest,
                ),
                _ => None,
            }
        } else if wiring.directory_grant {
            match (region, channel, grant) {
                (Some(r), Some(ch), Some((care_words, _))) => {
                    build_grant(own_ut, r, ch, &self.care, care_words, set_seen, manifest)
                }
                _ => None,
            }
        } else {
            None
        };
        // A directory grant that produced no capability refuses the spawn: a program told to act on
        // something and holding nothing is the one outcome this model must never trade away.
        let dir_failed = wiring.directory_grant && narrowed.is_none();
        // The window's badge bound by `fs::BIND_SYNC`, so the server answers `SYNC` alone; no page,
        // since a sync carries no bytes. `reboot` must not start unable to sync when there is a
        // filesystem, and with none the slot is left empty.
        let sync_ep = match (wants_sync && self.fs_scoped, fs, window) {
            (true, Some(f), Some(w)) => {
                bind_sync_only(f, &mut spawn.windows, w, own_ut, &mut self.fs_mapped)
            }
            _ => None,
        };
        let sync_failed = wants_sync && fs.is_some() && sync_ep.is_none();
        DirWiring {
            label,
            window,
            bound,
            channel,
            narrowed,
            dir_failed,
            sync_ep,
            sync_failed,
            // The sources page goes with the store alone, not with `activation/`.
            index_store: matches!(store, Some(ActivationGrant::IndexStore(_))) && bound.is_some(),
            mounted: false,
        }
    }

    /// [`Service::wire_directory`] for a grant in the second mounted tree: the job's window of the
    /// disk tree's pool, the disk tree's endpoint badged with it, and a caretaker over that.
    fn wire_mounted(
        &mut self,
        req: &Request,
        manifest: Option<grant_plan::Manifest>,
        region: Option<u64>,
        tree: Fs,
        care_words: (u64, u64, u64),
        wants_sync: bool,
    ) -> DirWiring {
        let own_ut = self.c.own_ut;
        let label = self.spawn.label();
        let window = self.spawn.until_reaped(|s| s.disk_windows.take(label));
        let channel =
            window.and_then(|w| mounted_job_channel(tree, w, own_ut, &mut self.disk_mapped));
        let narrowed = match (region, channel) {
            (Some(r), Some(ch)) => build_grant(
                own_ut,
                r,
                ch,
                &self.care,
                care_words,
                req.set_seen,
                manifest,
            ),
            _ => None,
        };
        DirWiring {
            label,
            window,
            bound: None,
            channel,
            dir_failed: narrowed.is_none(),
            narrowed,
            sync_ep: None,
            // A program that must be able to sync is not started without it, as on the first tree.
            sync_failed: wants_sync && self.c.fs.is_some(),
            index_store: false,
            mounted: true,
        }
    }

    /// Choose a normal job's output and its rights: the delegated sink, the terminal's sink for a
    /// screen-narrowed tail (§106 (an unredirected tail stage's output goes to the screen)), or the
    /// shared result endpoint. The child cannot tell which it holds. See
    /// notes/system-initializer/the-child-layouts.md.
    fn output(&self, req: &Request) -> (u64, u64) {
        let default_screen = self.c.term_sink.filter(|_| req.delegated.screen.is_some());
        (
            req.delegated
                .sink
                .or(default_screen)
                .unwrap_or(self.c.result_ep),
            abi::rights::WRITE,
        )
    }

    /// Lay out a native child's endowment: its in-order slots, its named slots, and its mappings.
    /// Slot 0 is the output, or a directory grant's endpoint with the output at slot 1; then the
    /// clock, the configuration page, the source and the budget. See
    /// notes/system-initializer/the-child-layouts.md, "The native layout".
    fn native_endowment(
        &self,
        req: &Request,
        manifest: Option<grant_plan::Manifest>,
        wants: &Wants,
        dir: &DirWiring,
        out: (u64, u64),
    ) -> NativeEndowment {
        let Channels {
            clock_page,
            config_page,
            ..
        } = self.c;
        let narrowed = dir.narrowed;
        // Sized for what the fields allow: two for a directory grant, one each for the rest.
        let mut caps = [out; 6];
        let mut n = 1usize;
        if let Some(dir_ep) = narrowed {
            caps[0] = (dir_ep, abi::rights::WRITE);
            caps[1] = out;
            n = 2;
        }
        // The clock and configuration page come before the source and budget, so each is slot 1;
        // that holds only while no manifest declares one with an input (notes/pipes.md's BUGS).
        if wants.clock {
            caps[n] = (clock_page, abi::rights::READ);
            n += 1;
        }
        if wants.config {
            caps[n] = (config_page, abi::rights::READ);
            n += 1;
        }
        if let Some(src) = req.delegated.source {
            // `READ` only, so a pipe's reader cannot write back up its own input.
            caps[n] = (src, abi::rights::READ);
            n += 1;
        }
        if let Some(b) = req.delegated.budget {
            // `SPEND_ONLY`, so nothing retyped from it can be lent (milestone 824 (a retype mints no
            // right its budget lacks)); a network holder lends the stack a page per socket from it.
            let rights = if wants.network {
                abi::rights::ALL
            } else {
                abi::rights::SPEND_ONLY
            };
            caps[n] = (b, rights);
            n += 1;
        }
        let (placed_buf, placed_n) = self.named_slots(req, manifest, wants, dir.sync_ep);
        let (map_buf, map_n) = self.native_maps(req, wants, dir);
        NativeEndowment {
            in_order: caps,
            n,
            placed_buf,
            placed_n,
            map_buf,
            map_n,
        }
    }

    /// Place a native child's named slots: the second stream, the domain, entropy, the network,
    /// the machine and share views, reboot and sync, each where its manifest says and with the
    /// narrowest right that serves it. The slots and why each right is the one given are in
    /// notes/system-initializer/the-child-layouts.md, "The native layout".
    fn named_slots(
        &self,
        req: &Request,
        manifest: Option<grant_plan::Manifest>,
        wants: &Wants,
        sync_ep: Option<u64>,
    ) -> ([(u64, u64, u64); 8], usize) {
        let Channels {
            deaths,
            jobs_ut,
            term_sink,
            entropy,
            network,
            reboot,
            ..
        } = self.c;
        let diag_slot = manifest.and_then(|m| m.output.diagnostics_slot());
        // With no `2>` on the line the second stream goes to the terminal's own sink (§67 (a
        // program's second stream is a declaration, not a number)).
        let default_diag = term_sink.filter(|_| diag_slot.is_some());
        let mut placed_buf = [(0u64, 0u64, 0u64); 8];
        let mut placed_n = 0usize;
        if let (Some(ep), Some(slot)) = (req.delegated.diagnostics.or(default_diag), diag_slot) {
            placed_buf[placed_n] = (slot, ep, abi::rights::WRITE);
            placed_n += 1;
        }
        if wants.domain {
            // `ENUMERATE` alone: `READ` would also grant `RECEIVE` and `REAP` on `deaths`.
            placed_buf[placed_n] = (grant_plan::DOMAIN_SLOT, deaths, abi::rights::ENUMERATE);
            placed_n += 1;
        }
        // `WRITE` alone, the right to `CALL`: `READ` would let it take another client's request,
        // and `GRANT` would let it hand the service on. The network's slot is narrowed the same way.
        if let (true, Some(ep)) = (wants.entropy, entropy) {
            placed_buf[placed_n] = (grant_plan::ENTROPY_SLOT, ep, abi::rights::WRITE);
            placed_n += 1;
        }
        if let (true, Some(ep)) = (wants.network, network) {
            placed_buf[placed_n] = (grant_plan::NETWORK_SLOT, ep, abi::rights::WRITE);
            placed_n += 1;
        }
        // The job pool with `ENUMERATE` alone answers `abi::memory_region::USAGE` and nothing that
        // spends, splits or destroys.
        if let (true, Some(page)) = (wants.machine, req.delegated.machine_page) {
            placed_buf[placed_n] = (grant_plan::MACHINE_SLOT, page, abi::rights::READ);
            placed_n += 1;
        }
        if wants.share {
            placed_buf[placed_n] = (grant_plan::SHARE_SLOT, jobs_ut, abi::rights::ENUMERATE);
            placed_n += 1;
        }
        // `WRITE` alone: the child can invoke the reboot object and cannot hand it on.
        if let (true, Some(r)) = (wants.reboot, reboot) {
            placed_buf[placed_n] = (grant_plan::REBOOT_SLOT, r, abi::rights::WRITE);
            placed_n += 1;
        }
        if let Some(f) = sync_ep {
            placed_buf[placed_n] = (grant_plan::SYNC_SLOT, f, abi::rights::WRITE);
            placed_n += 1;
        }
        (placed_buf, placed_n)
    }

    /// List the pages a native child maps: the directory grant's page, the clock, the
    /// configuration page and the machine statistics page, each only when it was asked for. Each is
    /// a page this process holds and only maps, so the child's reclaim leaves it alone.
    fn native_maps(
        &self,
        req: &Request,
        wants: &Wants,
        dir: &DirWiring,
    ) -> ([(u64, u64, u64); 4], usize) {
        let Channels {
            clock_page,
            config_page,
            ..
        } = self.c;
        let clock_map = [(CHILD_CLOCK_VA, clock_page, abi::address_space::MAP_RO)];
        let config_map = [(CHILD_CONFIG_VA, config_page, abi::address_space::MAP_RO)];
        // One page for program, caretaker and file server, sound because every request on both hops
        // is a blocking `CALL`: the program is parked in its own call while the caretaker uses it.
        let dir_map = [(
            FS_CLIENT_PAGE_VA,
            dir.channel.map_or(0, |f| f.page),
            abi::address_space::MAP_RW,
        )];
        let mut map_buf = [(0u64, 0u64, 0u64); 4];
        let mut map_n = 0usize;
        if dir.narrowed.is_some() {
            map_buf[map_n] = dir_map[0];
            map_n += 1;
        }
        if wants.clock {
            map_buf[map_n] = clock_map[0];
            map_n += 1;
        }
        if wants.config {
            map_buf[map_n] = config_map[0];
            map_n += 1;
        }
        if let (true, Some(page)) = (wants.machine, req.delegated.machine_page) {
            map_buf[map_n] = (
                machine_statistics_contract::PAGE_VA,
                page,
                abi::address_space::MAP_RO,
            );
            map_n += 1;
        }
        (map_buf, map_n)
    }

    /// **The resolver badges job `label` is placed**, and whether one it should hold could not be
    /// granted (milestone 892): the table's one zone for a program in it, or an installed image's
    /// granted zones, each asked for by its note (`granted_zones`), at most one per resolver slot.
    fn resolvers_for(
        &mut self,
        req: &Request,
        job: &Resolved,
        elf: Option<&elf::Elf>,
        label: u64,
    ) -> (Resolvers, bool) {
        let mut out = NO_RESOLVERS;
        if let Some(zone) = req.prog.and_then(Program::resolver_zone) {
            let (copy, failed) = self.spawn.resolver_for(label, Some(zone));
            out[0] = copy;
            return (out, failed);
        }
        let (Some(digest), Some(elf)) = (job.image_digest, elf) else {
            return (out, false);
        };
        let Ok(Some(note)) = elf.note(
            capability_manifest_note::OWNER,
            capability_manifest_note::MANIFEST,
        ) else {
            return (out, false);
        };
        let Ok(asked) = capability_manifest_note::zones(note) else {
            return (out, false);
        };
        let Channels { own_ut, fs, .. } = self.c;
        let (spawn, fs_mapped) = (&mut self.spawn, &mut self.fs_mapped);
        let (mut n, mut failed) = (0, false);
        granted_zones((fs, own_ut, fs_mapped), &digest, asked, |zone| {
            if failed || n == out.len() {
                return;
            }
            let (copy, f) = spawn.resolver_for(label, Some(zone));
            out[n] = copy;
            n += 1;
            failed |= f;
        });
        (out, failed)
    }

    /// Lay out a `std` child's endowment in `region` from the same decisions the native one reads.
    fn std_layout(
        &self,
        region: u64,
        out: (u64, u64),
        wants: &Wants,
        dir: &DirWiring,
        (args_page, installer, catalog_page, resolvers, index_sources, stdin, source): StdParts,
    ) -> StdLayout {
        let Channels {
            clock_page,
            config_page,
            entropy,
            network,
            ..
        } = self.c;
        StdLayout::new(
            region,
            out,
            StdGrants {
                dir: dir
                    .narrowed
                    .map(|ep| (ep, dir.channel.map_or(0, |f| f.page))),
                clock: wants.clock.then_some(clock_page),
                config: wants.config.then_some(config_page),
                entropy: entropy.filter(|_| wants.entropy),
                args: args_page,
                network: network.filter(|_| wants.network),
                installer,
                catalog: catalog_page,
                resolvers,
                index_sources,
                stdin,
                source,
            },
        )
    }

    /// Tidy up after a normal job and answer it: delete every capability that was only the means
    /// of building, built or not, give back a window or a region nothing came to hold, and send
    /// what the shell is owed. See notes/system-initializer/the-spawn-service.md, "After the build".
    fn finish_job(&mut self, req: &Request, ok: bool, failure: u64, left: JobLeftovers) {
        let result_ep = self.c.result_ep;
        let JobLeftovers {
            dir,
            installer,
            resolvers,
            stdin,
            catalog_page,
            sources_page,
            args_page,
            region,
            labeled,
        } = left;
        let label = dir.label;
        if let Some(l) = labeled {
            cap_delete(l);
        }
        // The reply endpoint is kept until the reap, filed by label, so an answer finds it. There is
        // room to file it: a spawn that found none made no installer and was refused.
        if let Some((inst, reply)) = installer {
            cap_delete(inst);
            let kept = ok
                && req
                    .prog
                    .is_some_and(|p| self.spawn.hold_reply(label, reply, p.name()));
            if !kept {
                cap_delete(reply);
            }
        }
        for page in [catalog_page, sources_page].into_iter().flatten() {
            cap_delete(page);
        }
        // A job that never started is never reaped, so its resolver grant is forgotten now.
        if !ok {
            self.spawn.forget_resolver(label);
        }
        // A window whose job never started is free now. A job answering on `result_ep` under
        // `job_undertaker` may be waited for: the shell reads it out before its next request
        // ([`ReapsDue`]).
        match (ok, dir.window, dir.mounted) {
            (false, Some(w), false) => self.spawn.release(w),
            (false, Some(w), true) => self.spawn.disk_windows.release(w),
            _ => {}
        }
        if ok && labeled.is_some() && req.delegated.sink.is_none() {
            self.spawn.due.expect(label);
        }
        for endpoint in [dir.narrowed, dir.sync_ep, stdin]
            .into_iter()
            .chain(resolvers)
            .flatten()
        {
            cap_delete(endpoint);
        }
        if let Some(ch) = dir.channel {
            cap_delete(ch.page);
            // A bound channel's endpoint is `narrowed`, already deleted above, unless a caretaker
            // serves a set over it (`at_bound_root`), and then `narrowed` is the caretaker's.
            if dir.narrowed != Some(ch.ep) {
                cap_delete(ch.ep);
            }
        }
        if let Some(page) = args_page {
            cap_delete(page);
        }
        // Nothing holds a capability to a live job's memory: since §32 (a supervisor may collect a
        // corpse) the reap is a supervision-endpoint method. A failed job is reclaimed now.
        if let Some(r) = region {
            if !ok {
                reclaim(r);
            }
            cap_delete(r);
        }
        // A redirected or screen-narrowed child answers elsewhere, so the shell gets one ack here;
        // an unredirected child's own message is the shell's read, and a failure is the sentinel.
        if req.wiring.sink || req.wiring.screen {
            send(
                result_ep,
                if ok { spawnproto::SPAWN_OK } else { failure },
                0,
                0,
            );
        } else if !ok {
            send(result_ep, failure, 0, 0);
        }
        // The shell drains the second stream to `OPERATION_EOF` before reading anything else, and a
        // child never built cannot end it, so the progenitor does.
        if !ok && let Some(ep) = req.delegated.diagnostics {
            send(ep, byte_sink_contract::eof(), 0, 0);
        }
    }
}

/// Receive the capabilities a request delegated, in protocol order (the interrupt pair, sink,
/// source, diagnostics, the screen-narrowed tail's completion endpoint of §106, the `--mem` budget,
/// the machine page), each only if it was promised, so both sides stay in lockstep.
fn receive_delegated(
    spawn: &mut SpawnEndpoint,
    wiring: spawnproto::Wiring,
    mem_pages: u64,
) -> Delegated {
    let interruptible = wiring.interruptible;
    let (job_ut, job_fr) = if interruptible {
        (
            opt_cap(spawn.receive_cap().1),
            opt_cap(spawn.receive_cap().1),
        )
    } else {
        (None, None)
    };
    let sink = if wiring.sink {
        opt_cap(spawn.receive_cap().1)
    } else {
        None
    };
    let source = if wiring.source {
        opt_cap(spawn.receive_cap().1)
    } else {
        None
    };
    let diagnostics = if wiring.diagnostics {
        opt_cap(spawn.receive_cap().1)
    } else {
        None
    };
    let screen = if wiring.screen {
        opt_cap(spawn.receive_cap().1)
    } else {
        None
    };
    let budget = if mem_pages > 0 {
        opt_cap(spawn.receive_cap().1)
    } else {
        None
    };
    let machine_page = if wiring.machine {
        opt_cap(spawn.receive_cap().1)
    } else {
        None
    };
    Delegated {
        job_ut,
        job_fr,
        sink,
        source,
        diagnostics,
        screen,
        budget,
        machine_page,
    }
}

/// Start a normal job's built child with its argument registers: zero for a `std` program, whose
/// words are on its argument page (§170 (how a foreign program is told what to do)), the grant's
/// own words behind a directory grant (`spawnproto::GRANT_WORDS`, forwarded unread), and the line's
/// integer otherwise.
fn start_job(built: Option<supervision_protocol::Child>, req: &Request, std_layout: bool) -> bool {
    match built {
        Some(child) => {
            let (a0, a1, a2) = match req.grant {
                _ if std_layout => (0, 0, 0),
                Some((_, child)) => child,
                None => (0, req.arg, 0),
            };
            start_child(child, a0, a1, a2)
        }
        None => false,
    }
}

/// Build a normal job's child in its region, in the `std` layout when it has one and the native
/// layout otherwise, born supervised on `fault`. `None` when there is no program or region, or the
/// build failed. A screen-narrowed child's `fault` is the shell's own endpoint (§106), which keeps
/// it out of `deaths`'s domain; see notes/system-initializer.md's BUGS.
fn build_job(
    own_ut: u64,
    (elf, region): (Option<&elf::Elf>, Option<u64>),
    (std_layout, std_parts): (bool, Option<&StdLayout>),
    native: &NativeEndowment,
    fault: Option<u64>,
) -> Option<supervision_protocol::Child> {
    let placed: &[(u64, u64, u64)] = &native.placed_buf[..native.placed_n];
    let maps: &[(u64, u64, u64)] = &native.map_buf[..native.map_n];
    match (elf, region) {
        (Some(e), Some(r)) if std_layout => std_parts.and_then(|l| {
            build_child(
                own_ut,
                r,
                e,
                &ChildEndowment {
                    caps: &l.caps,
                    placed: l.placed(),
                    maps: l.maps(),
                    fault,
                    stack_pages: std_runtime_contract::STACK_PAGES,
                    ..ChildEndowment::new(Retention::Nothing)
                },
            )
            .ok()
        }),
        (Some(e), Some(r)) => {
            // The fault slot is last, so a named slot the manifest picked cannot collide with it.
            build_child(
                own_ut,
                r,
                e,
                &ChildEndowment {
                    caps: &native.in_order[..native.n],
                    placed,
                    maps,
                    fault,
                    stack_pages: CHILD_STACK_PAGES,
                    ..ChildEndowment::new(Retention::Nothing)
                },
            )
            .ok()
        }
        _ => None,
    }
}
