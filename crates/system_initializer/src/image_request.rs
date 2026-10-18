//! An image request: the executable's bytes as frames the caller owns, the spawn endpoint that
//! carries them, and the vouching that decides whether they run. Moved out of `lib.rs` unchanged
//! but for visibility by milestone 843 (the system-initializer crate root is split along its
//! seams), under §279 (the system-initializer crate root splits into private modules).
//!
//! Name: provisional (milestone 843's lane, 2026-10-11 UTC).

use super::*;

// -------------------------------------------------------------------------------------------
// An image request: the executable's bytes as frames the caller owns (DECISIONS §219 option D,
// ruled 2026-09-26; milestone 198 (a package manager) rung 3a).
// -------------------------------------------------------------------------------------------

/// Where the progenitor maps the file service's shared page to read the installed-programs table. Mapped
/// once, on the first image request, with page tables from its own budget. Clear of every other
/// window this process maps (`INIT_OUT_VA` and the three peek pages below it, and the loader's
/// scratch window, `supervision_protocol::SCRATCH_WINDOW`, which starts at `0x1000_0000`).
pub(super) const ACTIVATION_FS_VA: u64 = address_space_map::pair_page(0x0f40_0000);

/// **Where the progenitor keeps its own copy of an image**, [`spawnproto::IMAGE_MAX_PAGES`] pages
/// at most. A fixed window rather than more of the loader's scratch window, because every page mapped
/// here comes from a staging region *this process* destroys once the child is built, and a destroy
/// revokes the mapping (DECISIONS §13 (frame revocation)), so the next request finds the window empty again. The page
/// tables behind it come from `own_ut` and are reused.
const IMAGE_STAGING_VA: u64 = address_space_map::pair_page(0x0f80_0000);

// The largest image's copy stays inside the pair band, clear of the loader's scratch window at its
// top (milestone 595 raised [`spawnproto::IMAGE_MAX_PAGES`] to 1024, which is 4 of the 8 MiB here).
const _: () = assert!(
    IMAGE_STAGING_VA + spawnproto::IMAGE_MAX_PAGES * spawnproto::IMAGE_PAGE
        <= address_space_map::PAIR_PAGES.end
);

/// **Take the argv's frame off the spawn endpoint and map it where this process can read it**
/// (milestone 205 (how a foreign program is told what to do), DECISIONS §170 (how a foreign program is told what to do); `spawnproto::PROGRAM_ARGUMENTS_BIT`). Returns the address, or `None` if the
/// caller sent no frame or it would not map. The capability goes at once, as an image frame's does
/// ([`take_frames`], [`stage_frames`]): the mapping outlives it, and the slot is what is scarce.
///
/// The shell keeps its own mapping of this frame, so it is read once, by [`copy_args`], before the
/// child exists; nothing reads it after.
pub(super) fn receive_args(spawn: &mut SpawnEndpoint, own_ut: u64) -> Option<u64> {
    let frame = opt_cap(spawn.receive_cap().1)?;
    // The loader's scratch window, revoked when the shell reclaims its frame and reused on a later
    // lap (milestone 604 (the builder's scratch cursor is bounded)).
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

/// **The spawn endpoint, as the spawn loop reads it** (milestone 685 (a job is finished when its
/// memory is back), calef's ruling of 2026-10-06 UTC, option A).
///
/// Two senders share the endpoint: the boot shell, whose requests are unbadged, and
/// `job_undertaker`, which sends `spawnproto::reaped` through a copy badged
/// [`spawnproto::UNDERTAKER_BADGE`] after every collect. A reaped message can arrive in the middle
/// of a request, between any two of the shell's messages, because the shell's next `SEND` is
/// issued only once its last one is taken. So every receive here takes reaped messages out of the
/// way before it returns the shell's next message: [`receive`](SpawnEndpoint::receive) and
/// [`receive_cap`](SpawnEndpoint::receive_cap) are the only way the loop reads this endpoint.
///
/// A reap frees the job's file-service window ([`Windows`]) and settles any wait for it
/// ([`ReapsDue`]). **A pool that is short waits for the reaps it is owed** rather than counting
/// yields ([`split`](SpawnEndpoint::split)): it waits only once the request in hand has been read
/// to its end, when the shell is parked on the result endpoint and only the undertaker can send,
/// and only while a finished job's reap is due.
///
/// # BUGS
///
/// - **A job that sends its answer and then does not exit holds the wait.** A due reap is one the
///   shell has had the job's whole answer for, which an exiting program follows at once; one that
///   blocks instead is never reaped, and a request that finds a pool short while it is due waits
///   with it, where until milestone 685 it was refused after 1,024 yields. `^C` does not reach it:
///   the shell is parked on the result endpoint.
/// - **A sender other than the one being served is held to one message.** Since milestone 809 (the
///   package client becomes a program) a third kind of sender exists: a program holding an
///   installer endpoint, a copy badged [`spawnproto::installer_badge`] of its job's label. A message
///   from a sender this loop is not serving is a first message (each sender's next `SEND` waits
///   for its last to be taken), so it is kept ([`stashed`](SpawnEndpoint::stashed)) and served
///   next. [`STASHED`] of them at once; one more is dropped, and its sender's follow-ups then read
///   as requests of their own and are refused. Only a pipeline of that many installer holders
///   reaches it. A wait for a reap gives up while anything is kept, and the request in hand is
///   refused.
pub(super) struct SpawnEndpoint {
    ep: u64,
    /// The file service's client windows, each held until its job is reaped.
    pub(super) windows: Windows,
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
    pub(super) fn new(ep: u64, resolver: Option<u64>) -> Self {
        Self {
            ep,
            windows: job_windows(),
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

    /// **Withdraw job `label`'s resolver grant**, if it has one: the resolver denies its badge from
    /// now on and gives up the page it attached. At the job's reap, or at once for a job that never
    /// started.
    ///
    /// Name: provisional, milestone 892's lane (#2011), 2026-10-12 (UTC).
    /// The pull request records no other name considered.
    pub(super) fn forget_resolver(&mut self, label: u64) {
        let (Some(ep), Some(entry)) = (
            self.resolver,
            self.resolving.iter().position(|l| *l == Some(label)),
        ) else {
            return;
        };
        if let Some((w0, w1)) = domain_name_resolution_contract::forget_message(entry as u32 + 1) {
            send(ep, w0, w1, 0);
        }
        self.resolving[entry] = None;
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

    /// Job `label` is reaped: its memory is back in its pool, and its window is free. An installer
    /// holder's reply endpoint goes with it, and a request it was in the middle of is abandoned.
    fn reaped(&mut self, label: u64) {
        self.windows.reaped(label);
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

    /// **Wait for one reap.** Only between the end of one request and the next, when the shell is
    /// parked on the result endpoint. `false` if what arrived was a request instead, which is kept
    /// for [`request`](SpawnEndpoint::request).
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
/// `supervision_protocol::SCRATCH_WINDOW`, which is where every view lands. One entry per page of
/// the largest image; the progenitor has one thread, so `Relaxed` is all the ordering there is.
///
/// An index and not an address because this is `.bss` in every progenitor ever built: two bytes an
/// entry is 2 KiB, where eight was two whole pages more per instance, and the kernel suite's frame
/// ledger counts every progenitor a test leaves standing.
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
/// Every one of the frames is received whatever happens, because the caller has already committed
/// to sending them and the grants it sends next must land in the `RECEIVE_CAP`s that expect them.
///
/// **Mapped now, copied later** ([`stage_frames`]), and that split is milestone 685's. The copy
/// needs a staging region and the child's own region carved from the image pool, and a carve that
/// finds the pool short waits for a reap ([`SpawnEndpoint::split`]). That wait is sound only once
/// the request has been read to its end: until then the shell's next message, not the undertaker's,
/// is first in line. So nothing is carved here. The views stay valid until the caller reclaims its
/// frames, which it does only once its answer is in, after [`stage_frames`] has given every view up.
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
            // A page of the loader's scratch window, the same rule as every page this process maps
            // (milestone 604 (the builder's scratch cursor is bounded)).
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
/// # Why a copy, and why one slot
///
/// The caller keeps its own mapping of these frames, so hashing them in place would let it change
/// the bytes between the hash and the build (§219: "the progenitor hashes its own copy"). So each
/// frame is copied into a page retyped from the staging region, and the hash and the build read
/// that. The staging page's own capability is deleted as soon as it is mapped, and every frame's
/// was deleted in [`take_frames`]: an image of any size costs the staging region's slot.
///
/// `stage` is false when the request cannot be built anyway (no region for the child, or an
/// interruptible image), and then this only gives the views up.
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
                    // SAFETY: the page is ours, fresh, and mapped read/write at `ours`.
                    let mapped =
                        unsafe { invoke(page, abi::page_frame::MAP, ours, 1, own_ut) } == 0;
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
        // Copied (or abandoned): our view of the caller's frame is no longer needed (milestone 95
        // (an unmap primitive), §249 (a running address space stays nameable)).
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

/// **Is this image vouched for?** Its SHA-256, looked up in the live generation of the activation
/// set on the file service (`installed_programs`). `false` on a miss, and on every way of failing to
/// read the table: no disk, no `activation` directory, a `current` that does not parse, a
/// generation that is malformed or larger than a page. A table that cannot be read vouches for
/// nothing, which is `measured_boot`'s rule one table over.
///
/// The file page is the one the shell and every caretaker share with the server. That is sound for
/// the reason `build_caretaker` gives: the shell is parked in its `RECEIVE` on the result endpoint for
/// the whole of a spawn, so nothing else is mid-request on it.
pub(super) fn vouched(bytes: &[u8], fs: Option<Fs>, own_ut: u64, fs_mapped: &mut bool) -> bool {
    let Some(files) = FsCalls::map(fs, own_ut, fs_mapped) else {
        return false;
    };
    let digest = measured_boot::sha256(bytes);
    let d = files.named(
        fs_operation::OPENDIR,
        fs_operation::ROOT,
        installed_programs::DIRECTORY,
        dir::READ,
    );
    if d < 0 {
        return false;
    }
    let mut table = [0u8; PAGE_BYTES];
    let hit = match files.live_generation(d as u64, &mut table) {
        Ok((_, n)) => core::str::from_utf8(&table[..n]).is_ok_and(|table| {
            matches!(
                installed_programs::lookup_digest(table, &digest),
                Ok(Some(_))
            )
        }),
        Err(()) => false,
    };
    files.close(d);
    hit
}

/// **The manifest a file's bytes are endowed with, or `None` to refuse them** (milestone 597,
/// provisional). Their note is read with `elf::Elf::note` and decoded by `capability_manifest_note`; a note
/// that is there and unreadable refuses the bytes, since running them as if they carried none would
/// endow a program with a manifest it did not declare. `grant_plan::image_manifest` then applies
/// the vouch (the shell's `caps` calls the same function to preview it), and the request's own
/// words must fit what results.
pub(super) fn endowed_image(
    elf: &elf::Elf<'_>,
    vouched: bool,
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
    // **A verb table is not honored for installed bytes yet** (milestone 809 (the package client
    // becomes a program), `capability_manifest_note::VERBS`): endowing from the manifest alone would hand
    // every line every verb's grants, so bytes carrying one are refused instead. The image's own
    // `jig` narrows by `grant_plan`'s table, and nothing installed carries the note.
    if !matches!(
        elf.note(
            capability_manifest_note::OWNER,
            capability_manifest_note::VERBS
        ),
        Ok(None)
    ) {
        return None;
    }
    let m = grant_plan::image_manifest(declared, vouched).ok()?;
    grant_plan::image_request_fits(&m, arg, mem_pages, words).then_some(m)
}
