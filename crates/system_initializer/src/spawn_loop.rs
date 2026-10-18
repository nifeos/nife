//! The spawn service: `spawn_service`, the loop `boot` ends in, which serves the shell's `run`
//! requests forever, with the caretaker, channel and file-service shapes it carries. Moved out of
//! `lib.rs` unchanged but for visibility by milestone 843 (the system-initializer crate root is
//! split along its seams), under §279 (the system-initializer crate root splits into private
//! modules).
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

/// The archive entry a spawnable program is loaded from.
///
/// It answered `None` for `rm` until 2026-08-17, because `rm` is endowed a **directory** capability
/// and the progenitor had deleted the file service during the boot, so there was nothing to attenuate. Keeping
/// the slot empty was the honest answer to that: spawning `rm` with nothing to remove from would be
/// the worst failure this model has, a program told to destroy something, holding nothing, saying
/// nothing. Milestone 31 phase 3 removed the cause rather than the symptom, so the exception is gone
/// and every spawnable program is loaded the same way.
pub(super) fn archive_name(p: Prog) -> Option<&'static str> {
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
    ///
    /// It used to be dropped once the shell held its narrowed copy, with a comment saying why it
    /// would have to come back. This is that day. The shell's copy carries no `GRANT`, so the shell
    /// holds nothing it could hand a caretaker; the progenitor is the only process here that can build one,
    /// which is what made `rm` a refusal at the prompt for six weeks.
    pub(super) fs: Option<Fs>,
    /// **A client view of the entropy service** (milestone 111), endowed to a child whose manifest
    /// declares [`grant_plan::Manifest::entropy`]. `None` on a boot that built no entropy service
    /// (no virtio-rng device, no `entropy` program in the initial ramdisk, or a service that did
    /// not answer `READY`), and then a declaring child is spawned with an empty
    /// [`grant_plan::ENTROPY_SLOT`] and says so on its second stream rather than drawing
    /// predictable bytes.
    ///
    /// [`deaths`](Channels::deaths)'s shape rather than [`clock_page`](Channels::clock_page)'s: an
    /// endpoint the progenitor holds and places a narrowed copy of, not a frame it maps. `WRITE` is what a
    /// child gets, which on an endpoint is the right to `CALL` and nothing more; the progenitor keeps the
    /// full-rights capability it retyped, and never receives on it.
    ///
    /// **The shell does not hold one.** The shell needs no randomness of its own (nothing it does
    /// as a builtin is unpredictable), and endowing it one would be an authority nothing uses,
    /// which is the same call `config_page` and `deaths` already make and the opposite of
    /// `clock_page`, which the shell holds because `time` measures with it.
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
    /// One slot for the life of the boot, and the table had one left (23 of 24; 32 slots since 2026-09-27). It is retyped
    /// after `login`'s build, the peak, so it is not held across it; `script/swish-check`'s
    /// `capability slots:` line is what says whether that held.
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

/// The spawn service loop: serve the shell's `run` requests forever. The progenitor is the ELF loader the
/// shell directs; it inserts only what the shell endows, so a spawned program can reach nothing the
/// command line did not name.
///
/// Two shapes (`grant_plan::spawnproto`). A **normal** job: the shell sends the request and, if
/// `--mem` rode along, one delegated untyped; we split a region off the job pool, build the child in
/// it, endow it the result endpoint (and the budget) plus its supervision endpoint, and start it. A
/// **supervised** (interruptible) job: the shell leads the delegation with a job untyped and a shared
/// job frame; we build the whole child *from that untyped* (so the shell's region owns it and can
/// `DESTROY` it to tear it down, DECISIONS §24), map the job frame in, endow nothing else, start it,
/// and send `SPAWN_OK` once as the shell's go-ahead. The `progs` array is indexed by [`Prog::id`], so
/// it is [`grant_plan::PROG_COUNT`] long: a variant added to `grant_plan` without a slot here would
/// be an out-of-bounds read in the progenitor.
///
/// **Only the normal shape is supervised**, and that is not an oversight. An interruptible job's
/// region belongs to the shell, which tears it down itself on the second `^C` (§24's
/// forcible tier) and after a clean finish; endowing it a supervision endpoint here would put a
/// second party in the teardown path for memory that is not ours, racing the shell's `DESTROY`.
pub(super) fn spawn_service(
    c: Channels,
    progs: &[Option<elf::Elf>; grant_plan::PROG_COUNT],
    care: Caretakers,
    discipline: SessionDiscipline,
    fs_scoped: bool,
) -> ! {
    let spawn_ep = c.spawn_ep;
    let resolver = c.resolver;
    let mut service = Service {
        c,
        progs,
        care,
        discipline,
        fs_scoped,
        // Whether the file page is mapped here yet, for the installed-programs table (first image request).
        fs_mapped: false,
        // The spawn endpoint, its reaped messages, and the windows they free (milestone 685).
        spawn: SpawnEndpoint::new(spawn_ep, resolver),
    };
    loop {
        service.serve();
    }
}

/// What the spawn service loop carries from one request to the next: the progenitor's remaining
/// authority, the boot's measured images, and the two pieces of state a request can change.
struct Service<'a> {
    c: Channels,
    progs: &'a [Option<elf::Elf<'a>>; grant_plan::PROG_COUNT],
    care: Caretakers,
    discipline: SessionDiscipline,
    fs_scoped: bool,
    fs_mapped: bool,
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
    prog: Option<Prog>,
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
}

impl Wants {
    /// Read each declaration from the program's own manifest, never from the request.
    fn of(manifest: Option<grant_plan::Manifest>) -> Wants {
        // Read from the program's own declaration, not from the request: a clock is not something
        // the command line can designate, so there is no bit on the wire for it (`Manifest::clock`).
        let wants_clock = manifest.is_some_and(|m| m.clock);
        // Same reasoning, one authority over (milestone 126): a **process domain** is not something
        // a person designates either. There is no /proc to name and no pid space to scan, so what a
        // program may see is decided here, by which supervision endpoint the progenitor puts in its capability table.
        let wants_domain = manifest.is_some_and(|m| m.domain);
        // `clock`'s twin again: the inert-configuration page is the progenitor's to endow, not something a
        // command line can designate, so there is no bit on the wire for it either
        // (`Manifest::configuration`).
        let wants_config = manifest.is_some_and(|m| m.configuration);
        // `clock`'s family a fourth time, and the first member of it that is an endpoint rather
        // than a page (milestone 111). Randomness is not something a command line designates, so
        // there is no bit on the wire for it either; the program's own declaration is what decides,
        // which is what keeps a program's dependence on unpredictable bytes visible in what it
        // holds. A boot with no entropy service answers `None` here and a declaring child is born
        // with an empty slot, which is the state its second stream exists to report.
        let wants_entropy = manifest.is_some_and(|m| m.entropy);
        // And a fifth (milestone 590 (provisional)): a network is not something a command line
        // designates either.
        let wants_network = manifest.is_some_and(|m| m.network);
        // And the two views milestone 126 added (DECISIONS §225): how the machine is doing, and
        // what this prompt's job budget was spent on. Neither is something a line designates.
        let wants_machine = manifest.is_some_and(|m| m.machine);
        let wants_share = manifest.is_some_and(|m| m.share);
        // And the reboot object (milestone 805, DECISIONS §251), which nothing on a line can name
        // either. `grant_plan::image_can_carry` keeps it off every image, so only the built-in
        // `reboot` gets here with it.
        let wants_reboot = manifest.is_some_and(|m| m.reboot);
        // And a sync-only capability to the file server (milestone 805), which `reboot` declares
        // beside the object so it can sync for itself. Made below, per job, from a window.
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
        }
    }
}

/// A package manager's installer and reply endpoints, its catalog page, and whether each of the
/// two that its manifest declared failed.
type PackageParts = (Option<(u64, u64)>, Option<u64>, bool, bool);

/// What a `std` child is placed beyond [`Wants`]: its argv page, installer endpoints, catalog page,
/// resolver badge and index sources page.
type StdParts = (
    Option<u64>,
    Option<(u64, u64)>,
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
    /// The badged copy of the resolver's endpoint the child was placed (milestone 892).
    resolver: Option<u64>,
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
        // **A request on an installer endpoint** (milestone 809 (the package client becomes a
        // program), DECISIONS §270 (a package manager holds an installer endpoint, not the spawn
        // endpoint)). Install, remove and rollback are served, as the owner's console's would be,
        // and recorded under the holder's name; anything else is refused with nothing read past
        // this first message, so a holder can neither spawn nor vouch. The answer goes to the
        // holder's own reply endpoint, because the shell is reading `result_ep` while it runs.
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
        // **An edit to the installed-programs table, not a spawn** (milestone 198 (a package
        // manager) rung 3a's installer). Asked first, because under this bit no other bit of
        // word 2 means anything. On the spawn endpoint this is the owner's console: `vouch`, and
        // nothing else sends it since `jig`.
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

        // **A `graphical_terminal` spawn** (milestone 632 (provisional)): graphics is launched from the prompt,
        // and this is the launch. The request must be the program and the bit together (`Prog::
        // GraphicalTerminal` *and* [`spawnproto::Wiring::graphics`], and never
        // [`spawnproto::Wiring::keyboard`], which no capability follows since milestone 715);
        // anything else is refused rather than half-endowed. A well-formed request on a boot with
        // no display is answered [`spawnproto::SPAWN_NO_DISPLAY`], so the shell can say so.
        //
        // **The devices are this process's own** (milestone 715 (provisional)): the gpu's four and
        // the keyboard's three stay in the boot endowment's slots for the life of the boot, and
        // each session's drivers get narrowed copies of them, exactly as `boot_terminal` is lent
        // to the UART arm. Until 715 they arrived on this request from the shell's slots, which
        // meant the shell held `GRANT` on every device between launches.
        //
        // The stack this builds is milestone 600 (provisional)'s, moved from the boot into this
        // branch: the session's children are born from one region and the session program is born
        // supervised on `deaths`, so its one reap, by `job_undertaker`, ends the drivers with it.
        //
        // **A session that does not come up is a refused spawn**, not a trap: the shell says
        // "could not spawn", the prompt continues, and the builder reclaims the region itself
        // (`build_graphical_terminal_session` owns that disposal). The boot version of this builder trapped
        // because a boot that cannot come up is a dead machine; a launch that cannot is just a
        // command that failed.
        if req.wiring.graphics || req.prog == Some(Prog::GraphicalTerminal) {
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
            self.spawn_interruptible(&req, elf);
        } else {
            self.spawn_job(&req, &job, elf, image_region, staging);
        }

        // Drop our copies of every delegated cap: the child holds what it needs (the job frame is
        // mapped, the budget and the streams inserted), and the shell holds the originals it kept
        // (the job untyped for teardown, the pipe it minted). This keeps the progenitor's 16-slot capability table from
        // filling across a long session.
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
        let arg = spawnproto::arg(w1);
        let mem_pages = spawnproto::mem_pages(w2);
        let wiring = spawnproto::wiring(w2);
        let interruptible = wiring.interruptible;
        // **Under `IMAGE_BIT` word 0 is a length, not a program** (DECISIONS §219 option D), so no
        // row of `progs` is consulted: the program is whatever the bytes are.
        let prog = if wiring.image {
            None
        } else {
            Prog::from_id(spawnproto::prog_id(w0))
        };

        // **The directory grant's two data messages, before any capability** (milestone 31 phase 3).
        // They are read here rather than inside the branch that uses them because the shell has
        // already sent them: a request that announced them and a progenitor that did not drain them would
        // leave the endpoint holding words the *next* command would read as its own.
        let grant = wiring.dir.then(|| (spawn.receive(), spawn.receive()));

        // **The image's frames, after the data and before every other capability** (§219 D),
        // taken and mapped here and copied once the request has been read to its end
        // ([`take_frames`] says why: a carve may wait for a reap, which is sound only then). A
        // plain line is the only shape the shell sends; anything else is drained and refused.
        let frames = wiring
            .image
            .then(|| take_frames(spawn, spawnproto::image_len(w0), own_ut));

        // **The argv's one frame, after the image's and before every other capability**
        // (milestone 205 (how a foreign program is told what to do), DECISIONS §170; `spawnproto::ARGS_BIT`). Taken on every request that
        // announced it, whatever program it turns out to be for, so both sides stay in lockstep.
        let args_seen = if wiring.args {
            receive_args(spawn, own_ut)
        } else {
            None
        };
        // **And the name set's frame, right after** (milestone 205, `spawnproto::NAMESET_BIT`),
        // read the same way: mapped here, copied into the job's region below, never read again.
        let set_seen = if wiring.nameset {
            receive_args(spawn, own_ut)
        } else {
            None
        };

        let delegated = receive_delegated(spawn, wiring, mem_pages);
        // **The run-unvouched capability, presented** (DECISIONS §219 gate D2): the last message
        // of any request that claimed it, taken whether or not this one turns out to need it, so
        // the caller's `SEND` is never left waiting. It arrived on the endpoint only a holder can
        // send on, which is all it proves and all it has to. See `spawnproto::RUN_UNVOUCHED_BIT`.
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
    /// frames into the staging region.
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
        // **The image's two carves, now that the request is read to its end** (milestone 685).
        // The child's region is split *first*, then the staging region, so the staging region is
        // the top of the image pool when it is destroyed below and its pages go back (a region
        // returns its pages to its parent only when it is the most recent carve,
        // `memory_regions`' `return_to_parent`).
        //
        // **An image that hears words is a `std` program, and gets a `std` region** (milestone 205
        // (how a foreign program is told what to do)). The note that says which is inside the
        // frames, which have not been copied, so the argv bit sizes the region and
        // `endowed_image` checks the note agrees (`grant_plan::image_can_carry` ties the two). A
        // directory grant rides an image only for bytes that hear words (milestone 205): their
        // `std` region is sized to hold the caretaker as well (`grant_plan::STD_REGION_PAGES`).
        //
        // **From the image pool, and as large as the image** (milestone 595): see
        // [`IMAGE_POOL_PAGES`] and `grant_plan::image_region_pages`.
        let image_region = if wiring.image && !interruptible && (!wiring.dir || wiring.args) {
            spawn.split(
                images_ut,
                grant_plan::image_region_pages(
                    spawnproto::image_pages(spawnproto::image_len(w0)),
                    if wiring.args {
                        grant_plan::Runtime::Std
                    } else {
                        grant_plan::Runtime::Native
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
    /// otherwise build the session from its own region of the job pool.
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
        let answer = if prog != Some(Prog::GraphicalTerminal) || !wiring.graphics || wiring.keyboard
        {
            spawnproto::SPAWN_FAILED
        } else if !graphical_terminal_caps.held {
            spawnproto::SPAWN_NO_DISPLAY
        } else {
            let built = match (
                self.progs[Prog::GraphicalTerminal.id() as usize].as_ref(),
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
    /// the one manifest and the word a refusal answers with.
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
        // **Vouched or not** (§219 D), and **what the bytes declare** (milestone 597 (a program
        // carries its manifest in an ELF note), provisional). A hit is endowed with the manifest
        // its own note carries, or `grant_plan::NO_NOTE_MANIFEST` if it carries none: the digest
        // that vouched covers the note, so installing the package installed its manifest. A miss is
        // built only for a caller that presented the run-unvouched capability, and then with
        // `grant_plan::UNVOUCHED_MANIFEST` whatever its note asks (§219: an unvouched note grants
        // nothing). Any other miss is refused with its own word, and bytes whose manifest forbids
        // this request (`grant_plan::image_manifest`, `image_request_fits`) with another.
        let mut unvouched = false;
        let mut refused_by_manifest = false;
        let mut image_manifest = None;
        let image_elf = staging.and_then(|_| {
            let bytes = staged_image(spawnproto::image_len(w0));
            let vouch = vouched(bytes, fs, own_ut, &mut self.fs_mapped);
            unvouched = !vouch;
            let elf = if vouch || presented {
                elf::Elf::parse(bytes).ok()
            } else {
                None
            };
            let elf = elf?;
            match endowed_image(&elf, vouch, arg, mem_pages, wiring.args) {
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
        // **The one manifest everything below reads**, so an image is endowed from what
        // `endowed_image` decided and never from a `Prog` row: `prog` stays `None` for it, and no
        // row's authority can reach it by a path this line does not name.
        // **Narrowed by the line's verb** for a program with a verb table (milestone 809 (the
        // package client becomes a program), option V2): `jig list` is endowed with the catalog
        // alone. The verb is read from the shell's page here, before the region is split (an
        // installer holder's region is sized and pooled from its manifest), and checked against
        // the child's own copy when that is made ([`copy_args`]), so a frame changed between the
        // two reads refuses the spawn rather than endowing for one verb and running another.
        let mut verb = [0u8; grant_plan::MAX_VERB];
        let verb_len = args_seen.and_then(|theirs| {
            // SAFETY: `receive_args` mapped the shell's frame read-only at `theirs`, one page, and
            // it stays mapped until the shell reclaims its frame after this request's answer.
            let page = unsafe {
                core::slice::from_raw_parts(theirs as *const u8, spawnproto::IMAGE_PAGE as usize)
            };
            let w = grant_plan::verb_of_page(page)?;
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
        } else if refused_by_manifest {
            spawnproto::SPAWN_REFUSED_BY_MANIFEST
        } else {
            spawnproto::SPAWN_FAILED
        };
        Resolved {
            manifest,
            failure,
            image_elf,
            verb,
            verb_len,
        }
    }

    /// Build and start an interruptible job from the shell's own job untyped, and answer it.
    fn spawn_interruptible(&self, req: &Request, elf: Option<&elf::Elf>) {
        let Channels {
            result_ep, own_ut, ..
        } = self.c;
        // Build the whole child from the shell's job untyped, mapping the shared job frame; no
        // capabilities in its capability table (it reports through the frame and exits). SPAWN_OK is the
        // go-ahead the shell waits for before it starts watching the frame.
        let built = match (elf, req.delegated.job_ut, req.delegated.job_fr) {
            (Some(e), Some(job), Some(fr)) => build_child(
                own_ut,
                job,
                e,
                &ChildEndowment {
                    maps: &[(CHILD_JOB_PAGE_FRAME_VA, fr, abi::address_space::MAP_RW)],
                    stack_pages: CHILD_STACK_PAGES,
                    ..ChildEndowment::new(Retention::Nothing)
                },
            )
            .ok(),
            _ => None,
        };
        match built {
            Some(child) => {
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
            }
            None => {
                send(result_ep, spawnproto::SPAWN_FAILED, 0, 0);
            }
        }
    }

    /// Build, start and answer a normal job: its region, its grants, its endowment in the native or
    /// the `std` layout, then the tidy-up and the answer.
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
        // **A region of its own, split first now** (milestone 31 phase 3). It used to be split
        // just before the build; a directory grant needs it earlier, because the caretaker is
        // built out of the **same** region as the program it serves. That is DECISIONS §92's
        // decision and §40's mechanism: a child's resources come from its supervisor's region,
        // so one reclaim ends both and the caretaker cannot outlive the grant it carries.
        //
        // **A `std` program's region is bigger, because it is also the heap** (milestone 595
        // (provisional)); see [`grant_plan::STD_REGION_PAGES`] for why one region rather than
        // two. It covers a caretaker as well, so a directory grant does not change its size.
        // An image's region was split before its frames arrived, sized by the argv bit, which
        // `endowed_image` has checked against the note (milestone 205).
        let std_layout = manifest.is_some_and(|m| m.runtime == grant_plan::Runtime::Std);
        // **An installer holder is built from the image pool** (milestone 809): its region
        // carries a package's staging run beside its heap (`grant_plan::named_std_region_pages`
        // says why), and the progenitor's own copy of that package is carved beside it.
        let wants_installer = manifest.is_some_and(|m| m.installer);
        let region =
            self.split_job_region(req, manifest, image_region, (std_layout, wants_installer));
        // **The machine's index store** (milestone 801 (packages over the internet), ruling I2):
        // for a `std` job whose verb holds it, asked on the owner's console (the unbadged spawn
        // endpoint, as the installer is), and whose line named no directory of its own, since the
        // `std` layout has one directory slot. Not granted is not refused: a `jig` without it says
        // what it lacks.
        let index_rights = manifest
            .and_then(|m| m.indexes.rights())
            .filter(|_| std_layout && !req.wiring.dir && req.badge == 0);
        let dir = self.wire_directory(req, manifest, region, (wants.sync, index_rights));
        // **And its copy of the image's index sources**, beside the store and only with it.
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
        // **A resolver badge for the zone the table names** (milestone 892): only for a `std` job
        // holding the network on this line, and under `job_undertaker`, whose reap is what forgets
        // the grant (a screen-narrowed job is reaped by the shell; milestone 892's BUGS). Granted
        // before the child exists, since the resolver deletes a page attached to a badge it does
        // not know yet. A job that should hold one and could not is refused, `dir_failed`'s rule.
        let eligible = std_layout && wants.network && req.delegated.screen.is_none();
        let zone = req.prog.and_then(Prog::resolver_zone).filter(|_| eligible);
        let (resolver, resolver_failed) = self.spawn.resolver_for(dir.label, zone);

        let out = self.output(req);
        let native = self.native_endowment(req, manifest, &wants, &dir, out);
        // **The std layout** (milestone 595 (provisional)): the same authorities, placed where
        // nife's `std` reads them instead of in order. Computed here, beside the native arrays it
        // replaces, so both shapes read from the one set of decisions above (which output, which
        // directory, whether the manifest declared a clock).
        // **The child's own copy of the argv** (§170), carved from the region it is built in so
        // its reclaim frees the page. A `std` program is the only reader; anything else had
        // its frame taken above and hears nothing. A copy that could not be made fails the
        // spawn rather than running the program without the words its line gave it.
        let args_page = match (std_layout, region, req.args_seen) {
            (true, Some(r), Some(theirs)) => copy_args(own_ut, r, theirs, |copy| {
                grant_plan::verb_of_page(copy) == job.verb()
            }),
            _ => None,
        };
        let args_failed = std_layout && req.args_seen.is_some() && args_page.is_none();
        let std_parts = match (std_layout, region) {
            (true, Some(r)) => {
                let parts = (args_page, installer, catalog_page, resolver, sources_page);
                Some(self.std_layout(r, out, &wants, &dir, parts))
            }
            _ => None,
        };
        // **The job's supervision capability, labeled** (milestone 685): `deaths` badged with
        // `label`, which the kernel hands back beside the death (DECISIONS §148 (resolves by
        // asking the kernel)) and `job_undertaker` forwards in its reaped message. A fresh slot,
        // deleted once the build has copied it. A screen-narrowed child is supervised by the
        // shell's endpoint instead and needs none. No label, no build: a job the progenitor
        // could not recognize at its reap would hold its window for the life of the boot.
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
                && fault.is_some()
        });
        let built = build_job(
            own_ut,
            (elf, region),
            (std_layout, std_parts.as_ref()),
            &native,
            fault,
        );
        // **The staged copy is spent once the child is built**: `build_child` copied every
        // segment into the child's own region. Destroying the staging region revokes its
        // mapping here and returns its pages to the job pool (it is the pool's top carve).
        if let Some(st) = staging {
            supervision_protocol::memory_region_destroy(st);
            cap_delete(st);
        }
        let ok = start_job(built, req, std_layout);
        let leftovers = JobLeftovers {
            dir,
            installer,
            resolver,
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
                if req.wiring.dir {
                    DIR_JOB_REGION_PAGES
                } else {
                    JOB_REGION_PAGES
                },
            )
        }
    }

    /// Make a package manager's installer endpoint and reply endpoint, and its copy of the
    /// image's catalog, each only when its manifest declares it; and say whether either failed.
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
        // **The installer endpoint and its reply endpoint** (milestone 809 (the package client
        // becomes a program), DECISIONS §270 (a package manager holds an installer endpoint,
        // not the spawn endpoint)). Placed only for a request on the unbadged spawn endpoint,
        // which the owner's console alone holds, so only the owner grants it; and only in the
        // `std` layout, where `grant_plan` keeps every declaring manifest. The endpoint is a
        // copy of the spawn endpoint badged with this job's label; the reply endpoint is made
        // from the job's own region, so the reap frees it. Either failing refuses the spawn,
        // `dir_failed`'s rule: a package manager started without its installer is told nothing.
        //
        // **The image's catalog, copied into a page of the child's own** (milestone 809), so
        // the reap takes it back. A catalog larger than a page is refused rather than cut.
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
    /// badge or build a caretaker, and say whether a declared grant failed.
    fn wire_directory(
        &mut self,
        req: &Request,
        manifest: Option<grant_plan::Manifest>,
        region: Option<u64>,
        (wants_sync, index_rights): (bool, Option<u64>),
    ) -> DirWiring {
        let Channels { own_ut, fs, .. } = self.c;
        let Request {
            wiring,
            grant,
            set_seen,
            ..
        } = *req;
        let spawn = &mut self.spawn;
        // **The caretaker, built before the program it serves**, because the program's slot 0 is
        // the endpoint this returns. `None` means either that this is not a directory grant or
        // that the delivery failed, and `dir_failed` below is what keeps those apart: a grant
        // that could not be delivered must **not** spawn the program anyway.
        // **The job's own window** (milestone 599): a page of it and the endpoint badged with
        // it, which the caretaker calls with and both the caretaker and the job map. `None`
        // for a job with no directory grant, and for a grant whose channel the kernel refused,
        // which `narrowed` then turns into a refused spawn below.
        //
        // **A bound badge when the server enforces grants itself, a caretaker otherwise**
        // (milestone 606, rulings D and T1). The badge is bound to the grant's directory before
        // the job holds anything, and any failure on the way falls back to the caretaker.
        //
        // **The window is held by this job until `job_undertaker` says it is reaped**
        // (milestone 685): `label` is the name the job's supervision capability carries, and
        // the reaped message carries it back. A pool whose every window is held waits for the
        // reaps that are due, then refuses.
        let label = spawn.label();
        // **The index store** (milestone 801 (packages over the internet)): a window bound to
        // `activation/indexes` with the verb's rights, on a line that named no directory. Only a
        // server that enforces bindings itself can make one, since the path is longer than a
        // caretaker's name; on any other the store is not granted and `jig` says so.
        let index_store = !wiring.dir && self.fs_scoped && index_rights.is_some();
        // A sync-only grant takes a window too (milestone 805): its badge is bound sync-only
        // rather than to a directory, and it is held until the reap like any other.
        let window = if (wiring.dir || wants_sync || index_store) && fs.is_some() {
            spawn.until_reaped(|s| s.windows.take(label))
        } else {
            None
        };
        let bound = match (index_store, fs, index_rights, window) {
            (true, Some(f), Some(rights), Some(w)) => bound_index_store(
                f,
                (&mut spawn.windows, w),
                own_ut,
                &mut self.fs_mapped,
                rights,
            ),
            _ => None,
        };
        let bound = match (wiring.dir && self.fs_scoped, fs, grant, window) {
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
            (None, Some(f), Some(w)) if wiring.dir => {
                job_channel(f, (&mut spawn.windows, w), own_ut, &mut self.fs_mapped)
            }
            _ => None,
        };
        let narrowed = if let Some(b) = bound {
            // The job holds the file server's own endpoint, badged and bound: no caretaker.
            Some(b.ep)
        } else if wiring.dir {
            match (region, channel, grant) {
                (Some(r), Some(ch), Some((care_words, _))) => {
                    build_grant(own_ut, r, ch, &self.care, care_words, set_seen, manifest)
                }
                _ => None,
            }
        } else {
            None
        };
        // A directory grant that produced no capability. The program is not built: it was told
        // to act on something and holds nothing, which is the one outcome this model must never
        // trade away.
        let dir_failed = wiring.dir && narrowed.is_none();
        // **The sync-only capability** (milestone 805, `fs::BIND_SYNC`): the window's badge
        // bound so the file server answers `SYNC` on it and refuses everything else, and the
        // endpoint badged with it. No page: a sync carries no bytes. Only a server that
        // enforces bindings itself can make one, and a job that declared it on a boot with a
        // writable filesystem and got none is refused, `dir_failed`'s rule: `reboot` must not
        // start unable to sync. With no filesystem there is nothing to sync, and the slot is
        // left empty.
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
            index_store: index_store && bound.is_some(),
        }
    }

    /// Choose a normal job's output and its rights: the delegated sink, the terminal's sink for a
    /// screen-narrowed tail, or the shared result endpoint.
    fn output(&self, req: &Request) -> (u64, u64) {
        // **Slot 0 is the output**, and milestone 50 is the whole of what changed here: it is
        // the shared result endpoint unless the shell delegated a sink, in which case the sink
        // goes there instead and the child never learns that anything is different. `>` and the
        // left of a `|` are this line.
        //
        // **Except behind a directory grant**, where the narrowed endpoint takes slot 0 and the
        // output moves to slot 1. That is not a second convention invented here: it is the
        // contract `components/src/rm.rs` already documents and the kernel's `start_granted_dir`
        // already wires, so one program means one thing in a guest test and at the real prompt.
        // The grant goes first because it is the authority the command line named, and the
        // output is what every program gets whether it named anything or not.
        //
        // Slot 1 is the input source when there is one, and otherwise the `--mem` untyped, which
        // is safe only because no manifest declares both today. `grant_plan` is where that stops
        // being true, and the order here is the contract; see notes/pipes.md's BUGS.
        //
        // **Except when the shell narrowed this tail to the screen** (DECISIONS §106): the same
        // default-routing shape as the clock and the diagnostic stream below, applied to slot 0.
        // The shell asked for this by delegating a `screen` completion endpoint rather than by
        // declaring anything the child can read back, so the child's own slot 0 is exactly as
        // opaque to it as a redirected one always was.
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
        // Six rather than five (one more than the tightest bound any manifest shipped today
        // reaches): `dir` takes two, and `clock`, `config`, `source` and `budget` are each one
        // more, so a hypothetical program declaring all of directory, clock, config, source and
        // budget at once would need six. No shipped manifest does (no program declares both
        // `clock` and `config`, or a directory grant alongside either), but the array is sized
        // for what the *fields* allow rather than for what the table happens to contain today,
        // which is `clock_map`'s own ordered-slot debt (notes/pipes.md's `BUGS`) one field over.
        let mut caps = [out; 6];
        let mut n = 1usize;
        if let Some(dir_ep) = narrowed {
            caps[0] = (dir_ep, abi::rights::WRITE);
            caps[1] = out;
            n = 2;
        }
        // **The clock, which nothing on the command line asked for** (milestone 51's wiring).
        // It comes from the manifest rather than from the request, because a person does not
        // designate a clock: `date` declares that it reads one, and the progenitor is the only process
        // here holding a page it could hand over. Before the source and the budget, so `date`'s
        // clock is slot 1, which is unambiguous only because no manifest declares a clock *and*
        // an input. That is the same ordered-slot debt notes/pipes.md's BUGS already records.
        if wants.clock {
            caps[n] = (clock_page, abi::rights::READ);
            n += 1;
        }
        // **The inert-configuration page, `clock`'s twin** (DECISIONS §111). Same reasoning,
        // same ordering rule: before the source and the budget, so `printenv`'s config page is
        // slot 1 exactly as `date`'s clock is, and for the identical reason (no manifest
        // declares `config` alongside an input either, today).
        if wants.config {
            caps[n] = (config_page, abi::rights::READ);
            n += 1;
        }
        if let Some(src) = req.delegated.source {
            // READ only. A pipe's reader must not be able to write back up its own input, which
            // would make a pipeline a two-way channel nobody asked for.
            caps[n] = (src, abi::rights::READ);
            n += 1;
        }
        if let Some(b) = req.delegated.budget {
            // Narrowed to `SPEND_ONLY`: the child may spend it, not lend it, nor since milestone
            // 824 (a retype mints no right its budget lacks) lend anything it retypes from it.
            // Every right for a child holding the network, which lends the stack a page per
            // socket out of this budget (`network_echo_client --mem`), the reason a `std`
            // child's slot 3 carries every right.
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
    /// the machine and share views, reboot and sync, each where its manifest says.
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
        // **The declared second stream, at the slot the manifest names** (DECISIONS §67). Read
        // from the program's own declaration for the same reason the clock is: the shell knows
        // there is one (it minted the endpoint) and only the program knows where it goes. The
        // slot is high and explicit rather than next-in-line, because how many low slots this
        // child gets depends on what else the line granted, and a stream the program probes for
        // by number cannot move under it.
        let diag_slot = manifest.and_then(|m| m.output.diagnostics_slot());
        // **Where the second stream goes when the line did not say.** The shell delegates an
        // endpoint only for a `2>`, because that is the case it has to back a file for. With no
        // operator on the line the destination is the **terminal's own sink**, which is the progenitor's
        // to endow exactly as the clock is: the shell holds nothing it could hand over, and a
        // person does not designate a screen.
        //
        // That is also what keeps a redirected `date`'s complaint off the redirection. The
        // shell drains the output into the file and never sees these bytes at all.
        let default_diag = term_sink.filter(|_| diag_slot.is_some());
        // Either half missing means no second stream reaches the child, and it then says what it
        // has to say in-band, which is what every program did before §67.
        //
        // **Two named slots now, and they are placed the same way for the same reason.** The
        // second is a view over the domain this child is *about to be born into*: `deaths` is
        // the endpoint every job the progenitor spawns for this shell is supervised by, so a viewer handed
        // it sees exactly this shell's jobs, including itself, and nothing else on the machine.
        // The progenitor, the shell, the terminal and the filesystem server are all outside it, which is
        // the confinement claim and is checked by `kernel::user::survey_tests`.
        //
        // **The right is `ENUMERATE`, and it used to be `READ`** (fixed 2026-08-17). `READ` on a
        // supervision endpoint is also what `RECEIVE` and `abi::rendezvous::REAP` take, so the old
        // grant would have let a viewer take a death message out from under `job_undertaker` or
        // collect a corpse, and only the viewer's own source code said it did not. A domain names
        // its members and does not act on them (calef, 2026-08-17); `capability::Rights::ENUMERATE`
        // is what makes that a property of the grant. notes/process-view.md carries the argument.
        let mut placed_buf = [(0u64, 0u64, 0u64); 8];
        let mut placed_n = 0usize;
        if let (Some(ep), Some(slot)) = (req.delegated.diagnostics.or(default_diag), diag_slot) {
            placed_buf[placed_n] = (slot, ep, abi::rights::WRITE);
            placed_n += 1;
        }
        if wants.domain {
            // `ENUMERATE` alone, deliberately. Granting `READ` here would hand a viewer
            // `RECEIVE` and `REAP` on the supervision endpoint as well, which is authority to
            // collect a child rather than to name one. See `Rights::ENUMERATE`.
            placed_buf[placed_n] = (grant_plan::DOMAIN_SLOT, deaths, abi::rights::ENUMERATE);
            placed_n += 1;
        }
        // **The third named slot** (milestone 111), placed for the same reason the other two
        // are: how many low slots a child gets depends on what else the line granted it, and
        // `uuid` reads a fixed number.
        //
        // `WRITE` alone, and the narrowing is the grant rather than a formality. On a
        // rendezvous `WRITE` is the right to `CALL`, so a declaring child may ask the service
        // for bytes; `READ` would additionally let it `RECEIVE`, which is to take another
        // client's request out from under the service, and `GRANT` would let it hand a random
        // source to anything it spawned. Neither is given, so the only thing this capability
        // authorizes is the one thing the manifest declared.
        if let (true, Some(ep)) = (wants.entropy, entropy) {
            placed_buf[placed_n] = (grant_plan::ENTROPY_SLOT, ep, abi::rights::WRITE);
            placed_n += 1;
        }
        // **The fourth named slot** (milestone 590 (provisional)), `entropy`'s narrowing for
        // `entropy`'s reasons: `WRITE` is the right to `CALL` the socket contract, and neither
        // `READ` (taking another client's request off the stack's endpoint) nor `GRANT`
        // (handing the network to a grandchild) rides with it. `unreachable_network_witness`
        // at the prompt is the check that this line runs only for a declaring child.
        if let (true, Some(ep)) = (wants.network, network) {
            placed_buf[placed_n] = (grant_plan::NETWORK_SLOT, ep, abi::rights::WRITE);
            placed_n += 1;
        }
        // **The fifth and sixth named slots** (milestone 126, DECISIONS §225). The machine page
        // carries `READ` and is also mapped below; the job budget carries `ENUMERATE` alone,
        // which answers `abi::memory_region::USAGE` and nothing that spends, splits or
        // destroys. It is the same region this child was split from, so "yours" includes the
        // program asking, the way `ps` lists itself.
        if let (true, Some(page)) = (wants.machine, req.delegated.machine_page) {
            placed_buf[placed_n] = (grant_plan::MACHINE_SLOT, page, abi::rights::READ);
            placed_n += 1;
        }
        if wants.share {
            placed_buf[placed_n] = (grant_plan::SHARE_SLOT, jobs_ut, abi::rights::ENUMERATE);
            placed_n += 1;
        }
        // **The seventh named slot** (milestone 805, DECISIONS §251): the reboot object,
        // `WRITE` alone, so the child can invoke it and cannot hand it on. `GRANT` stays here.
        if let (true, Some(r)) = (wants.reboot, reboot) {
            placed_buf[placed_n] = (grant_plan::REBOOT_SLOT, r, abi::rights::WRITE);
            placed_n += 1;
        }
        // **The eighth** (milestone 805): the sync-only endpoint, `WRITE` alone, the right to
        // `CALL` the file server. Ours is deleted once the child holds its copy.
        if let Some(f) = sync_ep {
            placed_buf[placed_n] = (grant_plan::SYNC_SLOT, f, abi::rights::WRITE);
            placed_n += 1;
        }
        (placed_buf, placed_n)
    }

    /// List the pages a native child maps: the directory grant's page, the clock, the
    /// configuration page and the machine statistics page, each only when it was asked for.
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
        // **The FS contract's shared page, for a program behind a directory grant.** The same
        // frame the caretaker maps and the same frame the FS server maps: one page for all three
        // parties, sound because every request on both hops is a blocking `CALL`, so the client
        // is parked inside its own call for the whole time the caretaker is using it.
        let dir_map = [(
            FS_CLIENT_PAGE_VA,
            dir.channel.map_or(0, |f| f.page),
            abi::address_space::MAP_RW,
        )];
        // The region's own comment lives at the split above, which milestone 31 phase 3 moved
        // earlier so a caretaker could be built out of it. Everything the child is made of comes
        // out of that carve, and a single reclaim frees all of it; the clock frame and the FS
        // page are ours and are only *mapped* into the child, so they are untouched when the
        // region goes.
        // **Every mapping the manifest asked for, in one list** (milestone 126). This was a
        // chain offering at most two extra mappings, whose comment recorded the three-mapping
        // combination it could not express; the machine statistics page made a fourth kind of
        // mapping, and a list closes that debt instead of adding a branch to it. Each entry is a
        // page this progenitor holds and only maps, so the child's reclaim leaves them all alone.
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

    /// Lay out a `std` child's endowment in `region` from the same decisions the native one reads.
    fn std_layout(
        &self,
        region: u64,
        out: (u64, u64),
        wants: &Wants,
        dir: &DirWiring,
        (args_page, installer, catalog_page, resolver, index_sources): StdParts,
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
                resolver,
                index_sources,
            },
        )
    }

    /// Tidy up after a normal job and answer it: delete every capability that was only the means
    /// of building, give back a window or a region nothing came to hold, and send what the shell
    /// is owed.
    fn finish_job(&mut self, req: &Request, ok: bool, failure: u64, left: JobLeftovers) {
        let result_ep = self.c.result_ep;
        let JobLeftovers {
            dir,
            installer,
            resolver,
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
        // The child holds its badged copy; ours goes. The reply endpoint's `WRITE` is kept until
        // the child's reap, filed by its label, which is how an answer finds it; a holder that
        // did not start has its endpoint deleted now. There is room to file it: a spawn that
        // found none made no installer, and was refused above.
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
        // A job that never started is never reaped, so a resolver grant made for it is forgotten
        // now; our badged copy goes with the other endpoints below.
        if !ok {
            self.spawn.forget_resolver(label);
        }
        // **The window and the wait, by whether a job came to hold them** (milestone 685). A
        // window whose job never started is free now, not at a reap that will never come. A
        // job whose whole answer goes to the shell's result endpoint, under `job_undertaker`,
        // is one a later short pool may wait for: the shell reads that answer to its end
        // before it sends another request ([`ReapsDue`]).
        if let (false, Some(w)) = (ok, dir.window) {
            self.spawn.windows.release(w);
        }
        if ok && labeled.is_some() && req.delegated.sink.is_none() {
            self.spawn.due.expect(label);
        }
        // The narrowed endpoint was only ever the means of wiring: the child holds its own copy
        // and the caretaker holds the other end. Dropped whether or not the build worked, so a
        // failed spawn does not cost this capability table a slot for the rest of the boot.
        for endpoint in [dir.narrowed, dir.sync_ep, resolver].into_iter().flatten() {
            cap_delete(endpoint);
        }
        // The window's page and badged endpoint were only the means too: the caretaker and the
        // job each hold their own mapping and copy (milestone 599).
        if let Some(ch) = dir.channel {
            cap_delete(ch.page);
            // A bound channel's endpoint is `narrowed`, already deleted above.
            if dir.bound.is_none() {
                cap_delete(ch.ep);
            }
        }
        // The child holds its own copy of the argv page's capability; ours was only the means.
        if let Some(page) = args_page {
            cap_delete(page);
        }
        // Our capability to the job's region goes back now. It was only ever the means of
        // building: since §32 the reap is a method on the supervision endpoint, so nothing in
        // this system holds a capability to a *live* job's memory. A build or a start that
        // failed leaves nothing running in the region, so we reclaim it here rather than wait
        // for a death that will never come.
        if let Some(r) = region {
            if !ok {
                reclaim(r);
            }
            cap_delete(r);
        }
        // **A redirected child owes the shell no answer**, because its answer is going
        // somewhere else, so the shell has nothing to read and no way to find out that the
        // spawn failed. One ack closes that hole. An unredirected child is unchanged: the
        // child's own message is the shell's single read, and a failure is the sentinel.
        //
        // **A screen-narrowed child owes the same ack, for the same reason** (DECISIONS §106):
        // its output is not going to `result_ep` either, and its completion signal is the fault
        // endpoint above, not this one, so a build failure has to reach the shell here or not
        // at all.
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
        // **A child that was never built cannot end its own second stream**, and the shell
        // drains that stream to `OPERATION_EOF` before it reads anything else, so nothing would ever
        // come back. The progenitor closes it on the child's behalf. It is the same hole `SPAWN_OK`
        // closed for the output side, one stream over.
        if !ok && let Some(ep) = req.delegated.diagnostics {
            send(ep, byte_sink_contract::eof(), 0, 0);
        }
    }
}

/// Receive the capabilities a request delegated, in protocol order, each only if it was promised.
fn receive_delegated(
    spawn: &mut SpawnEndpoint,
    wiring: spawnproto::Wiring,
    mem_pages: u64,
) -> Delegated {
    let interruptible = wiring.interruptible;
    // Receive the delegated caps in protocol order: the interrupt pair first (job untyped, job
    // frame), then the sink, then the source, then the diagnostics, then the screen-narrowed
    // tail's completion endpoint (DECISIONS §106), then any --mem untyped. No promise, no
    // receive, so both sides stay in lockstep.
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
    // **The narrowed tail's completion endpoint** (DECISIONS §106), in the same delegation
    // order as everything else: a fresh capability the shell minted and kept a copy of, so
    // the progenitor installs it as this child's fault target and the shell can `RECEIVE` its exit instead
    // of draining bytes it will no longer see.
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
    // **The machine statistics page, when the session sent it** (milestone 126,
    // `spawnproto::MACHINE_BIT`): the last delegated capability, and deleted with the others
    // below once the child holds its own mapping and slot.
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

/// Start a normal job's built child with its argument registers: zero for a `std` program, the
/// grant's own words behind a directory grant, the line's integer otherwise.
fn start_job(built: Option<supervision_protocol::Child>, req: &Request, std_layout: bool) -> bool {
    match built {
        Some(child) => {
            // **A program behind a directory grant is started with the grant's own three
            // words** rather than with an integer, which is `rm`'s shape: a spec carrying
            // the options and two words of name (`filesystem_contract::grant`). The progenitor forwards what the
            // shell packed and reads none of it; see `spawnproto::GRANT_WORDS`.
            //
            // **A `std` program is started with nothing in its argument registers**: nife's
            // `_start` ignores all three, and `rm`'s grant words would mean nothing to it.
            // Its words reach it on the argument page instead (DECISIONS §170 (how a foreign
            // program is told what to do), milestone 205), placed by `StdLayout`.
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
/// layout otherwise. `None` when there is no program or region, or the build failed.
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
                    // Born supervised exactly as a native job is, so `job_undertaker`
                    // collects it and the region comes back.
                    fault,
                    stack_pages: std_runtime_contract::STACK_PAGES,
                    ..ChildEndowment::new(Retention::Nothing)
                },
            )
            .ok()
        }),
        (Some(e), Some(r)) => {
            // Born supervised: `deaths` goes in the reserved fault slot, where `START` reads
            // it and clears it, so the job cannot forge messages on its own death channel.
            // The declared second stream rides the same named-slot mechanism at the low slot
            // its manifest picked; the two cannot collide, because the fault slot is last.
            build_child(
                own_ut,
                r,
                e,
                &ChildEndowment {
                    caps: &native.in_order[..native.n],
                    placed,
                    maps,
                    // **A screen-narrowed child is supervised by the shell's own fresh
                    // endpoint instead of `deaths`** (DECISIONS §106), so the shell can
                    // `RECEIVE` its exit directly rather than racing the progenitor's reaper for the
                    // same message. Its memory still comes from `region` (unchanged, still
                    // this job pool), and REAP still returns it to this pool regardless of
                    // who holds the supervision endpoint (DECISIONS §26: the reclaimed
                    // region returns to its *builder*, not its supervisor). The trade is
                    // that this child is outside `deaths`'s domain for its short life, so
                    // it will not appear in a concurrent `ps`/`pgrep`; see this function's
                    // BUGS.
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
