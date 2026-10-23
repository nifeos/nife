//! An image request: the executable's bytes as frames the caller owns, the spawn endpoint that
//! carries them, and the vouching that decides whether they run.
//!
//! Name: provisional (milestone 843's lane, 2026-10-11 UTC).

use super::*;

// -------------------------------------------------------------------------------------------
// An image request: the executable's bytes as frames the caller owns (DECISIONS §219 option D,
// ruled 2026-09-26; milestone 198 (a package manager) rung 3a).
// -------------------------------------------------------------------------------------------

/// Where the progenitor maps the file service's client-window pool to read and write the
/// installed-programs table. Mapped once, on first use, with page tables from its own budget.
/// Clear of every other window this process maps (`PROGENITOR_SHELL_OUTPUT_VIRTUAL_ADDRESS`, the
/// peek pages below it, and `supervision_protocol::SCRATCH_WINDOW` at `0x1000_0000`).
pub(super) const ACTIVATION_FS_VA: u64 = address_space_map::pair_page(0x0f40_0000);

/// **Where the progenitor maps the second mounted tree's client-window pool**, to zero a window
/// between two jobs. Past [`ACTIVATION_FS_VA`]'s pool, which is the same size, and
/// below [`IMAGE_STAGING_VA`].
pub(super) const MOUNTED_TREE_POOL_VA: u64 = address_space_map::pair_page(0x0f50_0000);

/// **Where the progenitor keeps its own copy of an image**, [`spawnproto::IMAGE_MAX_PAGES`] pages
/// at most. A fixed window, because every page mapped here comes from a staging region this process
/// destroys once the child is built, and the destroy revokes the mapping (DECISIONS §13
/// (capability revocation and untyped reclamation)), so the next request finds it empty. The page
/// tables behind it come from `own_ut` and are reused.
const IMAGE_STAGING_VA: u64 = address_space_map::pair_page(0x0f80_0000);

// The largest image's copy stays inside the pair band, clear of the loader's scratch window at its
// top.
const _: () = assert!(
    IMAGE_STAGING_VA + spawnproto::IMAGE_MAX_PAGES * spawnproto::IMAGE_PAGE
        <= address_space_map::PAIR_PAGES.end
);

/// **Take the argv's frame off the spawn endpoint and map it where this process can read it**
/// (DECISIONS §170 (how a foreign program is told what to do),
/// `spawnproto::PROGRAM_ARGUMENTS_BIT`). Returns the address, or `None` if the caller sent no frame
/// or it would not map. The capability goes at once, as an image frame's does ([`take_frames`],
/// [`stage_frames`]): the mapping outlives it, and the slot is what is scarce.
///
/// The shell keeps its own mapping of this frame, so it is read once, by [`copy_args`], before the
/// child exists; nothing reads it after.
pub(super) fn receive_args(spawn: &mut SpawnEndpoint, own_ut: u64) -> Option<u64> {
    let frame = opt_cap(spawn.receive_cap().1)?;
    // Revoked when the shell reclaims its frame; the scratch window reuses the page on a later lap.
    let theirs = supervision_protocol::map_scratch(frame, false, own_ut).ok();
    cap_delete(frame);
    theirs
}

/// **Copy the argv at `theirs` into a page carved from the child's `region`**, and return that
/// page's capability for [`StdLayout`] to place. §170 ruled that the bytes carry no authority, and
/// `std` refuses a page that does not parse whole on its own side, so the copy is read for one
/// thing only, by `agrees`: since milestone 809 (the package client becomes a program) a verb
/// chooses among a manifest's grants, so an argv's copy must carry the verb the endowment was
/// narrowed by, or it is refused and the spawn with it. A name set's copy is checked for nothing.
pub(super) fn copy_args(
    own_ut: u64,
    region: u64,
    theirs: u64,
    agrees: impl FnOnce(&[u8]) -> bool,
) -> Option<u64> {
    let page = retype_page_frame(region).ok()?;
    // Fresh from the child's region, so reaping the child takes this mapping back.
    let Ok(ours) = supervision_protocol::map_scratch(page, true, own_ut) else {
        cap_delete(page);
        return None;
    };
    // SAFETY: both pages are mapped, `theirs` read-only by `receive_args` and `ours` read/write
    // just above, one page each, at two different pages of the scratch window.
    unsafe {
        core::ptr::copy_nonoverlapping(
            theirs as *const u8,
            ours as *mut u8,
            spawnproto::IMAGE_PAGE as usize,
        );
    }
    // SAFETY: `ours` is mapped read/write, one page, and was just written.
    let copied =
        unsafe { core::slice::from_raw_parts(ours as *const u8, spawnproto::IMAGE_PAGE as usize) };
    let same_verb = agrees(copied);
    // Written, and the child's from here: give our window onto it up (milestone 95, §249). Without
    // a capability to our own space this does nothing, and the region's reap takes it back as before.
    supervision_protocol::give_up_own_page(ours);
    if !same_verb {
        cap_delete(page);
        return None;
    }
    Some(page)
}

/// **The spawn endpoint, as the spawn loop reads it.** The shell sends unbadged, `job_undertaker`
/// sends `spawnproto::reaped` under [`spawnproto::UNDERTAKER_BADGE`], and an installer holder under
/// [`spawnproto::installer_badge`]. Every receive takes reaped messages out of the way and keeps
/// another sender's first message, and a short pool waits for the reaps it is owed
/// ([`split`](SpawnEndpoint::split)); see notes/system-initializer/children.md.
///
/// # BUGS
///
/// - A job that answers and then does not exit holds that wait, and `^C` cannot reach it.
/// - [`STASHED`] first messages at once; one more is dropped, and its sender's follow-ups refused.
pub(super) struct SpawnEndpoint {
    ep: u64,
    /// The file service's unbadged endpoint, which a reap's `UNBIND` goes to, or `None` on a boot
    /// with no disk (and then no window is ever bound).
    files: Option<u64>,
    /// The file service's client windows, each held until its job is reaped and unbound.
    pub(super) windows: Windows,
    /// The second mounted tree's client windows (milestone 834), held the same way.
    pub(super) disk_windows: Windows,
    /// The finished jobs whose reap a short pool may wait for.
    pub(super) due: ReapsDue,
    /// First messages, with their badges, from senders this loop was not serving when they arrived;
    /// the next requests, oldest first.
    stashed: [Option<(u64, u64, u64, u64)>; STASHED],
    /// The badge of the sender whose request is in hand: `0` for the shell.
    serving: u64,
    /// **The request in hand came from an installer holder that has since died** (milestone 809):
    /// its reaped message arrived mid-request. Every receive then answers at once with nothing,
    /// so the request unwinds instead of waiting for frames nobody will send.
    pub(super) abandoned: bool,
    /// The endpoint each live installer holder's answers go back on, by its job's label
    /// (`grant_plan::INSTALLER_REPLY_SLOT`). Ours is `WRITE`; the holder's is `READ`. Deleted at
    /// the holder's reap.
    replies: [Option<(u64, u64, &'static str)>; INSTALLER_HOLDERS],
    /// The label the next job's supervision capability carries (DECISIONS §148 (resolves by asking
    /// the kernel)). Never `0`, which is "unlabeled".
    next_label: u64,
    /// **The name resolver's unbadged endpoint** (milestone 892 (a std program at the prompt holds
    /// the network and a resolver)), or `None` on a boot that started none.
    pub(super) resolver: Option<u64>,
    /// The label of the job each resolver badge is granted to: badge `i + 1` for entry `i`, so a
    /// badge names its entry and no two live jobs share one. Forgotten at the job's reap, which is
    /// when nothing holds the badge any more.
    resolving: [Option<u64>; domain_name_resolution_contract::GRANTS_MAX],
}

/// How many first messages [`SpawnEndpoint`] keeps from senders it is not serving.
const STASHED: usize = 4;

/// **How many installer holders may be alive at once** (milestone 809): one reply endpoint each.
/// A spawn that would be one more is refused. `jig` is a foreground job, so one is the usual
/// number and four is a pipeline of them.
const INSTALLER_HOLDERS: usize = 4;

impl SpawnEndpoint {
    pub(super) fn new(ep: u64, files: Option<u64>, resolver: Option<u64>) -> Self {
        Self {
            ep,
            files,
            windows: job_windows(),
            disk_windows: mounted_job_windows(),
            due: ReapsDue::new(),
            stashed: [None; STASHED],
            serving: 0,
            abandoned: false,
            replies: [None; INSTALLER_HOLDERS],
            next_label: 1,
            resolver,
            resolving: [None; domain_name_resolution_contract::GRANTS_MAX],
        }
    }

    /// **Grant job `label` a resolver badge for `zone`** (milestone 892), and return a fresh badged
    /// copy of the resolver's endpoint for the caller to place and delete. `None` with no resolver,
    /// with every badge held by a live job, or if the kernel refused the copy (and then the grant is
    /// forgotten at once).
    ///
    /// Name: provisional, milestone 892's lane (#2011), 2026-10-12 (UTC).
    /// The pull request records no other name considered.
    pub(super) fn grant_resolver(&mut self, label: u64, zone: &str) -> Option<u64> {
        let ep = self.resolver?;
        let entry = self.resolving.iter().position(Option::is_none)?;
        let badge = entry as u32 + 1;
        for (w0, w1) in domain_name_resolution_contract::grant_messages(badge, zone.as_bytes())? {
            send(ep, w0, w1, 0);
        }
        self.resolving[entry] = Some(label);
        match badged(ep, badge as u64) {
            Ok(copy) => Some(copy),
            Err(()) => {
                self.forget_resolver(label);
                None
            }
        }
    }

    /// **A resolver badge for `zone`, when the job may hold one** (milestone 892): the badged copy,
    /// and whether a job that should hold one is going without, which refuses its spawn. A badge
    /// every live job holds may be one a finished job's due reap frees, as a window may, so this
    /// waits for the reaps that are due before it gives up. `zone` is `None` for a job the table
    /// names no zone for or that is not eligible; on a boot with no resolver nothing is owed.
    pub(super) fn resolver_for(&mut self, label: u64, zone: Option<&str>) -> (Option<u64>, bool) {
        let Some(zone) = zone.filter(|_| self.resolver.is_some()) else {
            return (None, false);
        };
        let copy = self.until_reaped(|s| s.grant_resolver(label, zone));
        (copy, copy.is_none())
    }

    /// **Withdraw job `label`'s resolver grants**, if it has any: the resolver denies its badge from
    /// now on and gives up the page it attached. At the job's reap, or at once for a job that never
    /// started.
    ///
    /// Name: provisional, milestone 892's lane (#2011), 2026-10-12 (UTC).
    /// The pull request records no other name considered.
    pub(super) fn forget_resolver(&mut self, label: u64) {
        let Some(ep) = self.resolver else {
            return;
        };
        // Every badge the job holds: one per zone it was granted (milestone 892's slots 9 and 22
        // to 28).
        for (entry, held) in self.resolving.iter_mut().enumerate() {
            if *held != Some(label) {
                continue;
            }
            if let Some((w0, w1)) =
                domain_name_resolution_contract::forget_message(entry as u32 + 1)
            {
                send(ep, w0, w1, 0);
            }
            *held = None;
        }
    }

    /// Keep a first message from a sender not being served; `false` if there is no room.
    fn stash(&mut self, m: (u64, u64, u64, u64)) -> bool {
        match self.stashed.iter_mut().find(|s| s.is_none()) {
            Some(slot) => {
                *slot = Some(m);
                true
            }
            None => false,
        }
    }

    /// Whether anything is kept for later.
    fn any_stashed(&self) -> bool {
        self.stashed.iter().any(Option::is_some)
    }

    /// File the reply endpoint for the installer holder labeled `label`; `false` if all
    /// [`INSTALLER_HOLDERS`] are taken.
    pub(super) fn hold_reply(&mut self, label: u64, ep: u64, manager: &'static str) -> bool {
        match self.replies.iter_mut().find(|r| r.is_none()) {
            Some(slot) => {
                *slot = Some((label, ep, manager));
                true
            }
            None => false,
        }
    }

    /// Whether another installer holder may be started: a reply endpoint has somewhere to be filed.
    pub(super) fn has_reply_room(&self) -> bool {
        self.replies.iter().any(Option::is_none)
    }

    /// The reply endpoint of the installer holder labeled `label`, and the name its rows are
    /// recorded under, if it is alive.
    pub(super) fn reply_for(&self, label: u64) -> Option<(u64, &'static str)> {
        self.replies
            .iter()
            .flatten()
            .find(|(l, ..)| *l == label)
            .map(|&(_, ep, m)| (ep, m))
    }

    /// A fresh label for the next job.
    pub(super) fn label(&mut self) -> u64 {
        let l = self.next_label;
        self.next_label = l.checked_add(1).unwrap_or(1);
        l
    }

    /// Job `label` is reaped: its memory is back in its pool, and its window's grant is taken back
    /// and the window is free. An installer holder's reply endpoint goes with it, and a request it
    /// was in the middle of is abandoned.
    fn reaped(&mut self, label: u64) {
        if let Some(w) = self.windows.reaped(label) {
            unbind(self.files, &mut self.windows, w);
        }
        self.disk_windows.reaped(label);
        self.due.reaped(label);
        self.forget_resolver(label);
        for r in &mut self.replies {
            if let Some((l, ep, _)) = *r
                && l == label
            {
                cap_delete(ep);
                *r = None;
            }
        }
        if spawnproto::installer_label(self.serving) == Some(label) {
            self.abandoned = true;
        }
    }

    /// The first message of the next request, and the badge it came with: `0` for the shell,
    /// [`spawnproto::installer_badge`] for an installer holder. Every later receive in the request
    /// takes only that sender's messages.
    pub(super) fn request(&mut self) -> (u64, u64, u64, u64) {
        // Kept in arrival order from the front ([`stash`](SpawnEndpoint::stash) fills the first
        // free entry), so the oldest is at 0 and taking it is a rotation.
        let m = match self.stashed[0].take() {
            Some(m) => {
                self.stashed.rotate_left(1);
                m
            }
            None => loop {
                let (w0, w1, w2, badge) = receive_badged(self.ep);
                if badge == spawnproto::UNDERTAKER_BADGE {
                    self.reaped(w0);
                    continue;
                }
                break (w0, w1, w2, badge);
            },
        };
        // Set only once the message is in hand: a reaped message taken while waiting for it is
        // for the request before, whose holder may be the one that just died, and must not mark
        // this one abandoned.
        self.serving = m.3;
        self.abandoned = false;
        m
    }

    /// The served sender's next plain message, with every reaped message before it taken out of
    /// the way and any other sender's first message kept. `(0, 0, 0)` once the request is
    /// [`abandoned`](SpawnEndpoint::abandoned).
    pub(super) fn receive(&mut self) -> (u64, u64, u64) {
        loop {
            if self.abandoned {
                return (0, 0, 0);
            }
            let (w0, w1, w2, badge) = receive_badged(self.ep);
            if badge == spawnproto::UNDERTAKER_BADGE {
                self.reaped(w0);
                continue;
            }
            if badge != self.serving {
                self.stash((w0, w1, w2, badge));
                continue;
            }
            return (w0, w1, w2);
        }
    }

    /// The served sender's next delegation, `(w0, slot, w1)` as `RECEIVE_CAP` returns them, with
    /// every reaped message before it taken out of the way. A reaped message read here carries its
    /// label in `x0` and no capability (`spawnproto::UNDERTAKER_BADGE` has the layout). Another
    /// sender's first message is a plain `SEND` and is kept, as [`receive`](SpawnEndpoint::receive)
    /// keeps it. No capability once the request is abandoned.
    pub(super) fn receive_cap(&mut self) -> (u64, u64, u64) {
        loop {
            if self.abandoned {
                return (0, abi::rendezvous::NO_CAP, 0);
            }
            let (w0, slot, w1, badge) = receive_cap_badged(self.ep);
            if badge == spawnproto::UNDERTAKER_BADGE {
                if slot != abi::rendezvous::NO_CAP {
                    cap_delete(slot);
                }
                self.reaped(w0);
                continue;
            }
            if badge != self.serving {
                if slot != abi::rendezvous::NO_CAP {
                    cap_delete(slot);
                }
                self.stash((w0, w1, 0, badge));
                continue;
            }
            return (w0, slot, w1);
        }
    }

    /// **Window `w` of the first tree was taken and no job came to hold it**: its grant, if
    /// [`bound_channel`] or [`bind_sync_only`] made one, is taken back now, as at a reap.
    pub(super) fn release(&mut self, w: u64) {
        self.windows.release(w);
        unbind(self.files, &mut self.windows, w);
    }

    /// **Wait for one reap.** Only between the end of one request and the next, when the shell is
    /// parked on the result endpoint. `false` if what arrived was a request instead, which is kept
    /// for [`request`](SpawnEndpoint::request). Gives up at once while a first message is kept,
    /// and the request in hand is then refused.
    fn await_reap(&mut self) -> bool {
        if self.any_stashed() {
            return false;
        }
        let (w0, w1, w2, badge) = receive_badged(self.ep);
        if badge == spawnproto::UNDERTAKER_BADGE {
            self.reaped(w0);
            true
        } else {
            self.stash((w0, w1, w2, badge));
            false
        }
    }

    /// **`attempt`, again after each reap, while a finished job's reap is due.** `None` once
    /// nothing is due and the attempt still fails: the pool is genuinely short.
    pub(super) fn until_reaped<T>(
        &mut self,
        mut attempt: impl FnMut(&mut Self) -> Option<T>,
    ) -> Option<T> {
        loop {
            if let Some(t) = attempt(self) {
                return Some(t);
            }
            if !self.due.any() || !self.await_reap() {
                return None;
            }
        }
    }

    /// **Carve `pages` from `pool`, waiting for the reaps that are due if it is short.** Called
    /// only after the request in hand has been read to its end ([`await_reap`]'s condition).
    ///
    /// This replaces a retry that yielded up to 1,024 times (#1402's stopgap), a timing guess:
    /// whether that many yields covered one reap depended on what else was runnable, and CI found
    /// it short on 2026-09-27 (`/installed/std-grep needle` typed right after
    /// `/installed/std-grep needle docs`, aarch64, "could not spawn").
    ///
    /// [`await_reap`]: SpawnEndpoint::await_reap
    pub(super) fn split(&mut self, pool: u64, pages: u64) -> Option<u64> {
        self.until_reaped(|_| memory_region_split(pool, pages).ok())
    }
}

/// **The caller's image frames, each mapped read-only in the loader's scratch window and not yet
/// copied** (milestone 685): phase one of an image's staging. `views` of them are mapped, at the
/// addresses in [`FRAME_VIEWS`]; `ok` is false if any did not arrive or did not map.
pub(super) struct Frames {
    pub(super) pages: u64,
    pub(super) views: u64,
    pub(super) ok: bool,
}

/// Where [`take_frames`] mapped each frame, for [`stage_frames`] to copy from: the page's index in
/// `supervision_protocol::SCRATCH_WINDOW`, one entry per page of the largest image. The progenitor
/// has one thread, so `Relaxed` is all the ordering there is. An index, not an address, to keep
/// this `.bss` at two bytes an entry: the kernel suite's frame ledger counts every progenitor.
static FRAME_VIEWS: [AtomicU16; spawnproto::IMAGE_MAX_PAGES as usize] =
    [const { AtomicU16::new(0) }; spawnproto::IMAGE_MAX_PAGES as usize];

// Every page of the scratch window has an index that fits.
const _: () = assert!(supervision_protocol::SCRATCH_WINDOW_PAGES <= u16::MAX as u64 + 1);

/// The scratch page with index `i`.
fn scratch_page(i: u16) -> u64 {
    supervision_protocol::SCRATCH_WINDOW.start + u64::from(i) * supervision_protocol::PAGE
}

/// **Take an image request's frames off the spawn endpoint**, mapping each read-only in the
/// loader's scratch window and deleting its capability before the next is taken.
///
/// Every frame is received whatever happens, so the grants sent next land in the `RECEIVE_CAP`s
/// that expect them. Nothing is carved here: [`stage_frames`] copies later, because a carve may
/// wait for a reap, which is sound only once the request has been read to its end. The views stay
/// valid until the caller reclaims its frames, after its answer.
pub(super) fn take_frames(spawn: &mut SpawnEndpoint, len: u64, own_ut: u64) -> Frames {
    let pages = spawnproto::image_pages(len);
    let mut f = Frames {
        pages,
        views: 0,
        ok: pages > 0 && pages <= spawnproto::IMAGE_MAX_PAGES,
    };
    for _ in 0..pages {
        let Some(frame) = opt_cap(spawn.receive_cap().1) else {
            f.ok = false;
            continue;
        };
        if f.ok {
            match supervision_protocol::map_scratch(frame, false, own_ut) {
                Ok(va) => {
                    let i = (va - supervision_protocol::SCRATCH_WINDOW.start)
                        / supervision_protocol::PAGE;
                    FRAME_VIEWS[f.views as usize].store(i as u16, Relaxed);
                    f.views += 1;
                }
                Err(()) => f.ok = false,
            }
        }
        // The mapping outlives the capability, and the slot is what is scarce.
        cap_delete(frame);
    }
    f
}

/// **Copy the frames [`take_frames`] mapped into pages of our own**, and return the staging region
/// holding the copy, or `None` if it could not be staged. Every view is given up either way.
///
/// A copy, because the caller keeps its mapping and could change the bytes between the hash and the
/// build (§219 (how the shell names an installed program to the spawner): "the progenitor hashes
/// its own copy"). Each staging page's capability is deleted once mapped, so an image of any size
/// costs one slot. `stage` false (the request cannot be built anyway) only gives the views up.
pub(super) fn stage_frames(
    spawn: &mut SpawnEndpoint,
    f: &Frames,
    len: u64,
    own_ut: u64,
    images_ut: u64,
    stage: bool,
) -> Option<u64> {
    let staging = if stage && f.ok {
        spawn.split(images_ut, f.pages)
    } else {
        None
    };
    let mut ok = staging.is_some();
    for i in 0..f.views {
        let theirs = scratch_page(FRAME_VIEWS[i as usize].load(Relaxed));
        if let (Some(st), true) = (staging, ok) {
            let ours = IMAGE_STAGING_VA + i * spawnproto::IMAGE_PAGE;
            ok = match retype_page_frame(st) {
                Ok(page) => {
                    let mapped = map_page_frame(page, ours, true, own_ut);
                    cap_delete(page);
                    mapped
                }
                Err(()) => false,
            };
            if ok {
                let n = (len - i * spawnproto::IMAGE_PAGE).min(spawnproto::IMAGE_PAGE) as usize;
                // SAFETY: `theirs` was mapped read-only by `take_frames` and is still mapped (the
                // caller reclaims its frames only after its answer), `ours` read/write just above,
                // one page each, and `n` is at most a page. Different windows, so no overlap.
                unsafe {
                    core::ptr::copy_nonoverlapping(theirs as *const u8, ours as *mut u8, n);
                }
            }
        }
        supervision_protocol::give_up_own_page(theirs);
    }
    match (staging, ok) {
        (Some(st), true) => Some(st),
        (Some(st), false) => {
            supervision_protocol::memory_region_destroy(st);
            cap_delete(st);
            None
        }
        _ => None,
    }
}

/// The staged copy [`stage_frames`] made, as bytes.
pub(super) fn staged_image(len: u64) -> &'static [u8] {
    // SAFETY: `stage_frames` mapped and filled `image_pages(len)` pages from `IMAGE_STAGING_VA`,
    // and the caller destroys the staging region (revoking the mapping) only after its last use of
    // this slice, within the same iteration of the spawn loop.
    unsafe { core::slice::from_raw_parts(IMAGE_STAGING_VA as *const u8, len as usize) }
}

/// **Is this image vouched for, and was it granted the network?** Its SHA-256, looked up in the
/// live generation of the activation set (`installed_programs`), with that generation's `grant`
/// lines. [`grant_plan::Vouch::Unvouched`] on a miss and on every failure to read the table, which
/// then vouches for nothing. The digest comes back too, for [`granted_zones`]. The page used is
/// window 0, the shell's, sound because the shell is parked on the result endpoint for the whole
/// of a spawn.
pub(super) fn vouched(
    bytes: &[u8],
    fs: Option<Fs>,
    own_ut: u64,
    fs_mapped: &mut bool,
) -> (grant_plan::Vouch, measured_boot::Digest) {
    use grant_plan::Vouch;
    let digest = measured_boot::sha256(bytes);
    let vouch = with_live_table(fs, own_ut, fs_mapped, |table| {
        match installed_programs::lookup_digest(table, &digest) {
            // A grant that cannot be read grants nothing; the row still vouches.
            Ok(Some(_)) => Vouch::Vouched {
                network: installed_programs::network_granted(table, &digest).unwrap_or(false),
            },
            _ => Vouch::Unvouched,
        }
    });
    (vouch.unwrap_or(Vouch::Unvouched), digest)
}

/// **Each resolver zone the live generation grants the bytes with `digest`, and their note asks
/// for**, handed to
/// `each` as dotted text, empty for the root zone, in the table's order. A grant gives nothing the
/// note did not ask for, so a `grant` line for a zone the bytes never asked is skipped. A table that
/// cannot be read grants none.
pub(super) fn granted_zones(
    (fs, own_ut, fs_mapped): (Option<Fs>, u64, &mut bool),
    digest: &measured_boot::Digest,
    asked: capability_manifest_note::Zones<'_>,
    mut each: impl FnMut(&str),
) {
    with_live_table(fs, own_ut, fs_mapped, |table| {
        for grant in installed_programs::grants(table) {
            let Ok(grant) = grant else {
                return;
            };
            if let installed_programs::Granted::Zone(zone) = grant.what
                && grant.digest == *digest
                && asked.iter().any(|z| z == zone.as_bytes())
            {
                each(zone);
            }
        }
    });
}

/// **Read the live generation of the activation set and hand it to `f`**, or `None` on every way of
/// failing to read it: no disk, no `activation` directory, a `current` that does not parse, a
/// generation that is not text or is larger than a page.
#[inline(never)]
fn with_live_table<R>(
    fs: Option<Fs>,
    own_ut: u64,
    fs_mapped: &mut bool,
    f: impl FnOnce(&str) -> R,
) -> Option<R> {
    let files = FsCalls::map(fs, own_ut, fs_mapped)?;
    let d = files.named(
        fs_operation::OPENDIR,
        fs_operation::ROOT,
        installed_programs::DIRECTORY,
        dir::READ,
    );
    if d < 0 {
        return None;
    }
    let mut table = [0u8; PAGE_BYTES];
    let answer = match files.live_generation(d as u64, &mut table) {
        Ok((_, n)) => core::str::from_utf8(&table[..n]).ok().map(f),
        Err(()) => None,
    };
    files.close(d);
    answer
}

/// **The manifest a file's bytes are endowed with, or `None` to refuse them.** Their note is read
/// with `elf::Elf::note` and decoded by `capability_manifest_note`; a note
/// that is there and unreadable refuses the bytes, since running them as if they carried none would
/// endow a program with a manifest it did not declare. `grant_plan::image_manifest` then applies
/// the vouch (the shell's `caps` calls the same function to preview it), and the request's own
/// words must fit what results.
pub(super) fn endowed_image(
    elf: &elf::Elf<'_>,
    vouch: grant_plan::Vouch,
    arg: u64,
    mem_pages: u64,
    words: bool,
) -> Option<grant_plan::Manifest> {
    let declared = match elf.note(
        capability_manifest_note::OWNER,
        capability_manifest_note::MANIFEST,
    ) {
        Ok(None) => None,
        Ok(Some(d)) => Some(capability_manifest_note::decode(d).ok()?),
        Err(_) => return None,
    };
    // Installed bytes carrying a verb table are refused: endowing from the manifest alone would
    // hand every line every verb's grants.
    if !matches!(
        elf.note(
            capability_manifest_note::OWNER,
            capability_manifest_note::VERBS
        ),
        Ok(None)
    ) {
        return None;
    }
    let m = grant_plan::image_manifest(declared, vouch).ok()?;
    grant_plan::image_request_fits(&m, arg, mem_pages, words).then_some(m)
}
