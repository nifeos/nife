//! **Is a filesystem reachable at all?** ([`is_reachable`]), and which reply words are the
//! kernel's rather than the server's ([`no_capability`]).
//!
//! A seam of `sys/fs/nife.rs` (§282 (when a file is split, the parent becomes an index)):
//! `pub(super)` here reaches `nife` and its children.
//!
//! Name: provisional (lane maintainer/near-ceiling-splits, 2026-10-12 UTC), for the module and the
//! file.

use super::*;

/// [`is_reachable`]'s answer: 0 = not yet asked, 1 = granted, 2 = not granted. Cached, because a
/// capability table slot's contents are fixed at spawn on this ABI.
pub(super) static REACHABLE: AtomicU8 = AtomicU8::new(0);

/// True if this process holds a directory capability.
///
/// The probe must not touch the shared page: a program granted no directory has no page mapped,
/// and writing a name into it would fault. So it is `FSTAT` on a handle number the server's table
/// can never contain. An empty slot draws one of the kernel's small negatives (`NoSuchSlot` = -1,
/// `WrongObject` = -2, `NotPermitted` = -3); a real server answers `-EBADF` (-9), a *reply*.
///
/// `pub(crate)` for `sys/paths/nife.rs`, which needs the same fact for the same reason: naming a
/// place a process has no capability for is the answer-instead-of-refusing failure.
pub(crate) fn is_reachable() -> bool {
    match REACHABLE.load(Ordering::Relaxed) {
        1 => true,
        2 => false,
        _ => {
            let (r0, _) = rt::call(FS, proto::req(proto::FSTAT, proto::MAX_HANDLE, 0), 0);
            // A payload-free probe cannot draw `ENOENT` (whose -2 collides with the kernel's
            // `WrongObject`), so all three of the kernel's refusals are unambiguous here.
            let ok = !matches!(r0 as i64, -1 | -2 | -3);
            REACHABLE.store(if ok { 1 } else { 2 }, Ordering::Relaxed);
            ok
        }
    }
}

/// Whether a reply word is the *kernel* refusing the invoke rather than the server answering.
///
/// The kernel's own errors are -1..-8 (`abi::Error`); the server's are negated errnos, and the two
/// spaces overlap (a wart of the contract, notes/std/fs.md). Only -3 reads as the kernel's: `ESRCH`
/// is not in the FS server's vocabulary, and `NotPermitted` is what a WRITE-less endpoint answers.
/// -1 is the server's `dir::EPERM` (milestone 122): every entry point checks [`is_reachable`]
/// first, so a kernel `NoSuchSlot` cannot follow, and reading -1 as "no capability" would report a
/// withheld right as "this platform cannot do that", the silent degradation DECISIONS §42 forbids.
/// The fix is a tag or an offset in the reply word, a contract change (notes/std/fs.md).
///
/// Name: provisional, flagged 2026-09-24 by the boolean-predicate pass
/// (design/naming/functions-that-answer-yes-or-no-worklist.md). It does not yet follow the Rust predicate rule
/// calef ratified 2026-09-24; recommended `is_missing_capability`, because `is_no_capability` does
/// not parse.
pub(super) fn no_capability(r0: u64) -> bool {
    matches!(r0 as i64, -3)
}
