//! **What a subtree grant can reach, decided in one place a server links and a prover checks**
//! (milestone 606 (a directory walk costs what it does on Linux), calef's ruling D, 2026-09-27).
//!
//! An eligible server confines a client itself, keyed on the badge milestone 599 (a frame per
//! filesystem client channel) put on `RECEIVE_CAP`, instead of through `fs_subtree_caretaker`.
//! Every eligible server resolves every path through [`walk`] and every handle through [`admit`].
//! The Kani harnesses at the bottom prove that a step is one name, a walk never widens, follows a
//! symbolic link or crosses a mount, and a bound badge reaches only its own root and the handles it
//! minted, ever after. `..` and `.` are never names ([`is_name`]), and a file reached by a hard link
//! inside the grant is inside it. See notes/walk-cost.md, "The hard cases, as one crate decides
//! them".

//! # BUGS
//!
//! - A badge is a window and a grant at once (§230 (badged endpoint capabilities)): a revoked badge
//!   handed to a new client while its old holder lives reaches the new grant. The progenitor frees
//!   a window only at reap, after the holder is dead; nothing here can check that.
//! - The proof is bounded ([`PROOF_PATH`], [`PROOF_BADGES`], [`PROOF_OPERATIONS`]); that no loop
//!   behaves differently past the bounds is an argument, not a proof.
//!
//! Name: provisional 2026-09-27 (milestone 606's lane, ruling D). calef names crates.

#![cfg_attr(not(test), no_std)]

use filesystem_contract::dir::{self, Rights};
use filesystem_contract::fs::ROOT;

/// Why a path or a request was refused. Each maps to one errno at the server's boundary
/// ([`Refusal::errno`]); the crate itself speaks no wire.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// A component that is not a name: empty, `.`, `..`, or containing a `/` or a NUL. `EINVAL`.
    Malformed,
    /// No such name, or a directory the grant may not walk into. `ENOENT`: a holder that may not
    /// look must not learn that the name is there.
    NotFound,
    /// A step that should be a directory is a file. `ENOTDIR`.
    NotADirectory,
    /// A step would carry less than it was asked for. `EPERM`, the refusal `OPENDIR` gives under §47 (a directory
    /// capability carries six rights).
    Narrowed,
    /// A symbolic link, which a grant never follows. `ELOOP`, the errno POSIX's `O_NOFOLLOW`
    /// answers with.
    Symlink,
    /// A mount point, which a grant never crosses. `EXDEV`.
    MountCrossing,
    /// A handle this badge did not mint, or a badge that has been revoked. `EBADF`.
    NotYours,
    /// A binding change asked for by a caller that may not make one, or of a badge that cannot
    /// take it. `EPERM`.
    Refused,
}

impl Refusal {
    /// The errno the file-service contract answers with.
    pub const fn errno(self) -> i32 {
        match self {
            Refusal::Malformed => 22,                  // EINVAL
            Refusal::NotFound => 2,                    // ENOENT
            Refusal::NotADirectory => 20,              // ENOTDIR
            Refusal::Narrowed | Refusal::Refused => 1, // EPERM
            Refusal::Symlink => 40,                    // ELOOP
            Refusal::MountCrossing => 18,              // EXDEV
            Refusal::NotYours => 9,                    // EBADF
        }
    }
}

/// What a server found under a name. The only facts about a node this crate needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A directory: the only thing a walk steps through.
    Directory,
    /// A regular file, hard-linked or not.
    File,
    /// A symbolic link, never followed.
    Symlink,
    /// The root of another filesystem mounted here, never crossed.
    MountPoint,
}

/// **Whether `c` is one name**: non-empty, not `.` or `..`, and free of `/` and NUL. The only
/// way down a tree is a name, so this is what keeps a path from going up or sideways.
pub fn is_name(c: &[u8]) -> bool {
    !c.is_empty() && c != b"." && c != b".." && !c.contains(&b'/') && !c.contains(&0)
}

/// **Every component of `path` is a name**, checked before any is resolved, so a malformed path is
/// refused whatever the tree holds and never gives an answer that depends on how far a walk got.
pub fn check_path(path: &[u8]) -> Result<(), Refusal> {
    if path.split(|&b| b == b'/').all(is_name) {
        Ok(())
    } else {
        Err(Refusal::Malformed)
    }
}

/// **Resolve every component of `path` but the last**, from the directory `start` carrying
/// `rights`, and return where the walk stopped, the rights it carries there, and the last name.
///
/// Each step is exactly the hop-by-hop `OPENDIR` a client could have sent: it needs `DESCEND` on
/// its parent ([`Refusal::NotFound`] without it), it must be a directory (a file is
/// [`Refusal::NotADirectory`], a symbolic link [`Refusal::Symlink`], a mount point
/// [`Refusal::MountCrossing`]), and it carries `parent & (DESCEND | hop)`, refused as
/// [`Refusal::Narrowed`] if that is less than it asked for. `DESCEND` is added here rather than
/// left to the caller, so no server can build a walk whose steps cannot be walked on from. `lookup` is the server's: it finds a name in a directory and says what
/// it is, and it is the only thing here that touches the filesystem, and its own errors (a device error, say) pass through as they are.
pub fn walk<N: Copy, E: From<Refusal>>(
    start: N,
    rights: Rights,
    path: &[u8],
    hop: u64,
    mut lookup: impl FnMut(N, &[u8]) -> Result<(N, Kind), E>,
) -> Result<(N, Rights, &[u8]), E> {
    check_path(path).map_err(E::from)?;
    let (dirs, last) = match path.iter().rposition(|&b| b == b'/') {
        Some(i) => (&path[..i], &path[i + 1..]),
        None => return Ok((start, rights, path)),
    };
    let (mut node, mut rights) = (start, rights);
    for step in dirs.split(|&b| b == b'/') {
        if !rights.allows(dir::DESCEND) {
            return Err(Refusal::NotFound.into());
        }
        let (child, kind) = lookup(node, step)?;
        landing(kind, true).map_err(E::from)?;
        let asked = dir::DESCEND | hop;
        let granted = rights.attenuate(asked);
        if granted != Rights::root(asked) {
            return Err(Refusal::Narrowed.into());
        }
        (node, rights) = (child, granted);
    }
    Ok((node, rights, last))
}

/// **Whether a verb may land on a node of `kind`**: a directory when the verb wants one
/// (`want_dir`), a file when it does not, and never a symbolic link or a mount point. The last
/// step of every verb goes through here, as every step of [`walk`] does.
pub fn landing(kind: Kind, want_dir: bool) -> Result<(), Refusal> {
    match (kind, want_dir) {
        (Kind::Symlink, _) => Err(Refusal::Symlink),
        (Kind::MountPoint, _) => Err(Refusal::MountCrossing),
        (Kind::Directory, true) | (Kind::File, false) => Ok(()),
        (Kind::File, true) => Err(Refusal::NotADirectory),
        // The file-service contract answers `EISDIR` here, which is not a confinement answer;
        // the server keeps saying it, so this is the one arm a server maps itself.
        (Kind::Directory, false) => Err(Refusal::Malformed),
    }
}

/// What one badge is to a server.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Binding {
    /// Never bound. The badge carries the endpoint's whole authority, as every badge did before
    /// ruling D; caretakers hold such badges, and the unbadged value 0 is always this.
    Open,
    /// Scoped to the directory handle `root`: its `ROOT` means that directory, and it reaches the
    /// handles it minted and nothing else.
    Bound {
        /// The server's handle for the granted directory.
        root: u64,
    },
    /// **May sync and do nothing else** (milestone 805 (`reboot` at the prompt), the
    /// sync-only capability). It names no directory, so every handle is refused; the server
    /// answers `SYNC` for it and refuses every other verb before a handle is read. Bound by
    /// [`Bindings::bind_sync_only`], taken back by [`Bindings::unbind`] like any grant.
    ///
    /// Name: ratified 2026-10-06 (calef, #1783: "Approve sync for the file-server
    /// request and its capability, and flush for the device cache only.").
    SyncOnly,
    /// Was bound, and the grant was taken back. It reaches nothing. It never becomes
    /// [`Binding::Open`] again, which is the property that makes revocation mean anything: a badge
    /// that fell back to the whole endpoint's authority on revocation would be a widening.
    Revoked,
}

/// **The handle a request under `badge` may use**, or why not. `requested` is the handle the
/// client sent, and `owner` is the badge that minted it (`None` when no such handle is open).
///
/// An open badge passes every handle through untouched, as the server always has. A bound badge's
/// `ROOT` becomes its grant's root, and any other handle must be one it minted. A revoked badge
/// reaches nothing.
pub fn admit(
    binding: Binding,
    badge: u64,
    requested: u64,
    owner: Option<u64>,
) -> Result<u64, Refusal> {
    match binding {
        Binding::SyncOnly => Err(Refusal::NotYours),
        Binding::Open => Ok(requested),
        Binding::Revoked => Err(Refusal::NotYours),
        Binding::Bound { root } if requested == ROOT => Ok(root),
        Binding::Bound { .. } if owner == Some(badge) => Ok(requested),
        Binding::Bound { .. } => Err(Refusal::NotYours),
    }
}

/// **Every badge's binding**, for a server with `B` client windows. Badge `b` is index `b`. A
/// nonzero badge at or past `B` is one the server has no window for, and it is [`Binding::Revoked`]:
/// it reaches nothing (calef's ruling, 2026-10-03, recorded as an amendment to §230). Only badge 0
/// is the unbadged value, so a boundary that does not know a badge refuses it rather than handing
/// it the whole endpoint.
#[derive(Clone, Copy, Debug)]
pub struct Bindings<const B: usize> {
    state: [Binding; B],
}

impl<const B: usize> Default for Bindings<B> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const B: usize> Bindings<B> {
    /// Every badge open.
    pub const fn new() -> Self {
        Self {
            state: [Binding::Open; B],
        }
    }

    /// The table slot for `badge`. A badge past `B` folds to 0, which [`Bindings::bind`] and
    /// [`Bindings::unbind`] both refuse, so it can change nothing; [`Bindings::of`] answers for it.
    fn index(badge: u64) -> usize {
        if (badge as usize) < B {
            badge as usize
        } else {
            0
        }
    }

    /// What `badge` is. Badge 0 is always [`Binding::Open`]; a nonzero badge at or past `B` is
    /// always [`Binding::Revoked`].
    pub fn of(&self, badge: u64) -> Binding {
        if badge != 0 && badge >= B as u64 {
            return Binding::Revoked;
        }
        match Self::index(badge) {
            0 => Binding::Open,
            i => self.state[i],
        }
    }

    /// **Bind `badge` to the directory handle `root`**, asked for by a caller whose own badge is
    /// `caller`. Only an open caller may bind, because only an open caller already holds everything
    /// a binding could hand out; a bound caller binding a badge to a handle outside its grant would
    /// be a way out of it. Badge 0 cannot be bound (it is the unbadged value), and a badge already
    /// bound cannot be bound again without being revoked first.
    pub fn bind(&mut self, caller: u64, badge: u64, root: u64) -> Result<(), Refusal> {
        if self.of(caller) != Binding::Open {
            return Err(Refusal::Refused);
        }
        let i = Self::index(badge);
        if i == 0 || matches!(self.state[i], Binding::Bound { .. } | Binding::SyncOnly) {
            return Err(Refusal::Refused);
        }
        self.state[i] = Binding::Bound { root };
        Ok(())
    }

    /// **Make `badge` sync-only** (milestone 805), asked for by a caller whose own badge is
    /// `caller`: [`Bindings::bind`]'s rules, with no directory. A badge already bound either way
    /// cannot be bound again without being revoked first.
    ///
    /// Name: ratified 2026-10-06 (calef, #1783: "Approve sync for the file-server
    /// request and its capability, and flush for the device cache only.").
    pub fn bind_sync_only(&mut self, caller: u64, badge: u64) -> Result<(), Refusal> {
        if self.of(caller) != Binding::Open {
            return Err(Refusal::Refused);
        }
        let i = Self::index(badge);
        if i == 0 || matches!(self.state[i], Binding::Bound { .. } | Binding::SyncOnly) {
            return Err(Refusal::Refused);
        }
        self.state[i] = Binding::SyncOnly;
        Ok(())
    }

    /// **Take `badge`'s grant back**, asked for by `caller`. The badge becomes
    /// [`Binding::Revoked`], never open. Returns the root handle the grant held, which the server
    /// closes along with every handle the badge minted.
    pub fn unbind(&mut self, caller: u64, badge: u64) -> Result<u64, Refusal> {
        if self.of(caller) != Binding::Open {
            return Err(Refusal::Refused);
        }
        // A sync-only badge holds no directory, so there is no root to hand back for closing:
        // `ROOT`, which the server never closes.
        let f = Self::index(badge);
        if f != 0 && self.state[f] == Binding::SyncOnly {
            self.state[f] = Binding::Revoked;
            return Ok(ROOT);
        }
        let i = Self::index(badge);
        match self.state[i] {
            Binding::Bound { root } if i != 0 => {
                self.state[i] = Binding::Revoked;
                Ok(root)
            }
            _ => Err(Refusal::Refused),
        }
    }
}

/// Path length, in bytes, the harnesses prove over.
pub const PROOF_PATH: usize = 5;
/// Badges in the binding table the harnesses prove over.
pub const PROOF_BADGES: usize = 3;
/// Operations in the binding sequence the harnesses prove over.
pub const PROOF_OPERATIONS: usize = 4;

#[cfg(kani)]
mod proofs {
    use super::*;

    /// A symbolic path of up to [`PROOF_PATH`] bytes over the alphabet that matters: `/`, `.`, a
    /// letter and NUL.
    fn any_path(buf: &mut [u8; PROOF_PATH]) -> &[u8] {
        let len: usize = kani::any();
        kani::assume(len <= PROOF_PATH);
        for b in buf.iter_mut() {
            let c: u8 = kani::any();
            kani::assume(c == b'/' || c == b'.' || c == b'a' || c == 0);
            *b = c;
        }
        &buf[..len]
    }

    /// **Every step a walk takes is one name.** Whatever `lookup` is shown, it is only ever asked
    /// for a non-empty name that is not `.` or `..` and has no `/` or NUL in it; so no path goes
    /// up, and none names two things at once.
    /// Falsification: replayable `crates/subtree_scope/falsifications/proofs.a_step_is_one_name.patch`
    #[kani::proof]
    #[kani::unwind(7)]
    fn a_step_is_one_name() {
        let mut buf = [0u8; PROOF_PATH];
        let path = any_path(&mut buf);
        let rights = Rights::root(kani::any());
        let _ = walk::<u8, Refusal>(0u8, rights, path, kani::any(), |n, step| {
            // Stated here from scratch, not through `is_name`, so a defect there cannot hide.
            assert!(!step.is_empty() && step != b"." && step != b"..");
            assert!(!step.contains(&b'/') && !step.contains(&0));
            Ok((n, Kind::Directory))
        });
    }

    /// **A walk never widens, never follows a link and never crosses a mount.** The rights it
    /// returns are within what it started with; and if any step on the way is anything but a
    /// directory, the walk is refused.
    /// Falsification: replayable `crates/subtree_scope/falsifications/proofs.a_walk_never_widens_follows_or_crosses.patch`
    #[kani::proof]
    #[kani::unwind(7)]
    fn a_walk_never_widens_follows_or_crosses() {
        let mut buf = [0u8; PROOF_PATH];
        let path = any_path(&mut buf);
        let start = Rights::root(kani::any());
        let hop: u64 = kani::any();
        let mut saw_other = false;
        let result = walk::<u8, Refusal>(0u8, start, path, hop, |n, _| {
            let kind: u8 = kani::any();
            kani::assume(kind < 4);
            let kind =
                [Kind::Directory, Kind::File, Kind::Symlink, Kind::MountPoint][kind as usize];
            if kind != Kind::Directory {
                saw_other = true;
            }
            Ok((n, kind))
        });
        // Not vacuous: a walk that took a step and succeeded is reachable.
        kani::cover!(result.is_ok() && path.contains(&b'/'));
        if let Ok((_, rights, last)) = result {
            assert!(
                !saw_other,
                "a walk went through something that is not a directory"
            );
            assert_eq!(rights.bits() & !start.bits(), 0, "a walk widened");
            assert!(is_name(last));
        }
    }

    /// **A bound badge reaches its own root and what it minted, and nothing else.**
    /// Falsification: replayable `crates/subtree_scope/falsifications/proofs.a_bound_badge_reaches_only_its_own.patch`
    #[kani::proof]
    fn a_bound_badge_reaches_only_its_own() {
        let root: u64 = kani::any();
        let badge: u64 = kani::any();
        let requested: u64 = kani::any();
        let owner: Option<u64> = if kani::any() { Some(kani::any()) } else { None };
        if let Ok(h) = admit(Binding::Bound { root }, badge, requested, owner) {
            assert!(h == root || (h == requested && owner == Some(badge)));
        }
        assert!(admit(Binding::Revoked, badge, requested, owner).is_err());
        assert!(admit(Binding::SyncOnly, badge, requested, owner).is_err());
    }

    /// **Once bound, a badge is never open again, and a bound caller changes no binding.** Over any
    /// sequence of [`PROOF_OPERATIONS`] binds and unbinds by any callers.
    /// Falsification: replayable `crates/subtree_scope/falsifications/proofs.a_badge_once_bound_is_never_open_again.patch`
    #[kani::proof]
    #[kani::unwind(5)]
    fn a_badge_once_bound_is_never_open_again() {
        let mut t = Bindings::<PROOF_BADGES>::new();
        let mut ever_bound = [false; PROOF_BADGES];
        for _ in 0..PROOF_OPERATIONS {
            let caller: u64 = kani::any();
            let badge: u64 = kani::any();
            kani::assume(caller < PROOF_BADGES as u64 + 1 && badge < PROOF_BADGES as u64 + 1);
            let before = t;
            let caller_open = t.of(caller) == Binding::Open;
            let which: u8 = kani::any();
            let changed = match which % 3 {
                0 => t.bind(caller, badge, kani::any()).is_ok(),
                1 => t.bind_sync_only(caller, badge).is_ok(),
                _ => t.unbind(caller, badge).is_ok(),
            };
            if changed {
                assert!(caller_open, "a bound or revoked caller changed a binding");
            } else {
                for b in 0..PROOF_BADGES as u64 {
                    assert_eq!(t.of(b), before.of(b));
                }
            }
            for (b, ever) in ever_bound.iter_mut().enumerate() {
                if matches!(t.of(b as u64), Binding::Bound { .. } | Binding::SyncOnly) {
                    *ever = true;
                }
                if *ever {
                    assert!(t.of(b as u64) != Binding::Open, "a bound badge became open");
                }
            }
        }
        assert_eq!(t.of(0), Binding::Open, "the unbadged value was bound");
    }

    /// **A badge the table has no slot for is never open, whatever has been bound** (calef's
    /// ruling of 2026-10-03). Only badge 0 is the unbadged value; a nonzero badge at or past
    /// `B` reaches nothing, and no binding or unbinding changes that.
    /// Falsification: replayable `crates/subtree_scope/falsifications/proofs.a_badge_with_no_window_is_never_open.patch`
    #[kani::proof]
    fn a_badge_with_no_window_is_never_open() {
        let mut t = Bindings::<PROOF_BADGES>::new();
        let caller: u64 = kani::any();
        let badge: u64 = kani::any();
        let _ = if kani::any() {
            t.bind(caller, badge, kani::any()).is_ok() || t.bind_sync_only(caller, badge).is_ok()
        } else {
            t.unbind(caller, badge).is_ok()
        };
        let past: u64 = kani::any();
        kani::assume(past >= PROOF_BADGES as u64);
        assert!(
            t.of(past) != Binding::Open,
            "a badge past the table was open"
        );
        assert_eq!(t.of(0), Binding::Open, "the unbadged value changed");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(n: u8, step: &[u8]) -> Result<(u8, Kind), Refusal> {
        match (n, step) {
            (0, b"a") => Ok((1, Kind::Directory)),
            (1, b"b") => Ok((2, Kind::Directory)),
            (0, b"f") => Ok((3, Kind::File)),
            (0, b"l") => Ok((4, Kind::Symlink)),
            (0, b"m") => Ok((5, Kind::MountPoint)),
            _ => Err(Refusal::NotFound),
        }
    }

    #[test]
    fn a_walk_goes_down_by_names_and_stops_before_the_last() {
        let all = Rights::root(dir::ALL);
        let (n, r, last) = down(0, all, b"a/b/x", dir::READ).unwrap();
        assert_eq!((n, last), (2, &b"x"[..]));
        assert_eq!(r.bits(), dir::DESCEND | dir::READ);
        assert_eq!(down(0, all, b"x", dir::READ).unwrap(), (0, all, &b"x"[..]));
    }

    #[test]
    fn a_walk_refuses_everything_that_is_not_a_directory_or_a_name() {
        let all = Rights::root(dir::ALL);
        let err = |p: &[u8]| down(0, all, p, dir::READ).err();
        assert_eq!(err(b"f/x"), Some(Refusal::NotADirectory));
        assert_eq!(err(b"l/x"), Some(Refusal::Symlink));
        assert_eq!(err(b"m/x"), Some(Refusal::MountCrossing));
        assert_eq!(err(b"z/x"), Some(Refusal::NotFound));
        for bad in [&b"a/../x"[..], b"a//x", b"/a", b"a/", b"./a", b""] {
            assert_eq!(err(bad), Some(Refusal::Malformed), "{bad:?}");
        }
        let no_descend = Rights::root(dir::READ);
        assert_eq!(
            down(0, no_descend, b"a/x", dir::READ).err(),
            Some(Refusal::NotFound)
        );
        let read_only = Rights::root(dir::DESCEND | dir::READ);
        assert_eq!(
            down(0, read_only, b"a/x", dir::READ | dir::WRITE).err(),
            Some(Refusal::Narrowed)
        );
    }

    #[test]
    fn a_bound_badge_is_scoped_and_a_revoked_one_reaches_nothing() {
        let mut t = Bindings::<4>::new();
        assert_eq!(
            admit(t.of(2), 2, 7, None),
            Ok(7),
            "an open badge passes through"
        );
        t.bind(0, 2, 40).unwrap();
        assert_eq!(admit(t.of(2), 2, ROOT, None), Ok(40));
        assert_eq!(admit(t.of(2), 2, 7, Some(2)), Ok(7));
        assert_eq!(admit(t.of(2), 2, 7, Some(1)), Err(Refusal::NotYours));
        assert_eq!(
            t.bind(2, 3, 41),
            Err(Refusal::Refused),
            "a bound caller binds nothing"
        );
        assert_eq!(t.bind(0, 2, 41), Err(Refusal::Refused), "bound twice");
        assert_eq!(
            t.bind(0, 0, 41),
            Err(Refusal::Refused),
            "the unbadged value"
        );
        assert_eq!(t.unbind(0, 2), Ok(40));
        assert_eq!(t.of(2), Binding::Revoked);
        assert_eq!(admit(t.of(2), 2, ROOT, None), Err(Refusal::NotYours));
        t.bind(0, 2, 42).unwrap();
        assert_eq!(
            t.of(2),
            Binding::Bound { root: 42 },
            "a revoked badge may be bound anew"
        );
    }

    #[test]
    fn a_sync_only_badge_reaches_no_handle_and_is_revoked_like_a_grant() {
        let mut t = Bindings::<4>::new();
        t.bind_sync_only(0, 3).unwrap();
        assert_eq!(t.of(3), Binding::SyncOnly);
        assert_eq!(admit(t.of(3), 3, ROOT, None), Err(Refusal::NotYours));
        assert_eq!(admit(t.of(3), 3, 7, Some(3)), Err(Refusal::NotYours));
        assert_eq!(
            t.bind_sync_only(3, 2),
            Err(Refusal::Refused),
            "a sync-only caller binds"
        );
        assert_eq!(t.bind(3, 2, 40), Err(Refusal::Refused));
        assert_eq!(t.bind(0, 3, 40), Err(Refusal::Refused), "bound twice");
        assert_eq!(t.bind_sync_only(0, 3), Err(Refusal::Refused), "bound twice");
        assert_eq!(
            t.bind_sync_only(0, 0),
            Err(Refusal::Refused),
            "the unbadged value"
        );
        assert_eq!(t.unbind(0, 3), Ok(ROOT), "no directory to close");
        assert_eq!(t.of(3), Binding::Revoked);
        t.bind_sync_only(0, 3).unwrap();
    }

    #[test]
    fn a_nonzero_badge_past_the_table_reaches_nothing() {
        let mut t = Bindings::<4>::new();
        for badge in [4u64, 5, 1 << 40, u64::MAX] {
            assert_eq!(t.of(badge), Binding::Revoked, "{badge}");
            assert_eq!(admit(t.of(badge), badge, 7, None), Err(Refusal::NotYours));
            assert_eq!(Refusal::NotYours.errno(), 9, "EBADF");
            assert_eq!(t.bind(badge, 1, 40), Err(Refusal::Refused), "as a caller");
            assert_eq!(t.bind(0, badge, 40), Err(Refusal::Refused), "as a target");
            assert_eq!(t.unbind(0, badge), Err(Refusal::Refused));
        }
        assert_eq!(t.of(0), Binding::Open, "badge 0 stays the caretaker's");
        assert_eq!(admit(t.of(0), 0, 7, None), Ok(7));
        t.bind(0, 3, 40).unwrap();
        assert_eq!(
            t.of(3),
            Binding::Bound { root: 40 },
            "the last in-table badge"
        );
    }

    fn down(
        start: u8,
        rights: Rights,
        path: &[u8],
        hop: u64,
    ) -> Result<(u8, Rights, &[u8]), Refusal> {
        walk(start, rights, path, hop, tree)
    }

    /// A hop that already names `DESCEND` is still a hop that carries it: the walk adds `DESCEND`
    /// to what it asks for, it does not toggle it, so the next step can be walked from.
    #[test]
    fn a_hop_that_names_descend_itself_still_carries_it() {
        let all = Rights::root(dir::ALL);
        let (n, r, last) = down(0, all, b"a/b/x", dir::DESCEND | dir::READ).unwrap();
        assert_eq!((n, last), (2, &b"x"[..]));
        assert_eq!(r.bits(), dir::DESCEND | dir::READ);
        let (n, r, _) = down(0, all, b"a/b/x", dir::DESCEND).unwrap();
        assert_eq!(n, 2);
        assert!(r.allows(dir::DESCEND));
    }

    /// The numbers are the file-service contract's (POSIX errno), spelled once per refusal; a
    /// server answers a client with them and a client matches on them.
    #[test]
    fn each_refusal_answers_with_its_posix_errno() {
        for (refusal, errno) in [
            (Refusal::Malformed, 22),
            (Refusal::NotFound, 2),
            (Refusal::NotADirectory, 20),
            (Refusal::Narrowed, 1),
            (Refusal::Refused, 1),
            (Refusal::Symlink, 40),
            (Refusal::MountCrossing, 18),
            (Refusal::NotYours, 9),
        ] {
            assert_eq!(refusal.errno(), errno, "{refusal:?}");
        }
    }

    /// A badge exactly at the table's size is the first one it has no window for. It folds to the
    /// unbadged slot, which neither a bind nor an unbind may touch, and it must not index past the
    /// array on the way.
    #[test]
    fn the_first_badge_past_the_table_is_refused_not_indexed() {
        let mut t = Bindings::<4>::new();
        assert_eq!(t.bind(0, 4, 40), Err(Refusal::Refused));
        assert_eq!(t.unbind(0, 4), Err(Refusal::Refused));
        assert_eq!(t.of(4), Binding::Revoked);
        t.bind(0, 3, 40).unwrap();
        assert_eq!(
            t.unbind(0, 3),
            Ok(40),
            "the last badge in the table is bindable"
        );
    }
}
