//! **Errors, mapped by meaning**.
//!
//! A seam of `sys/fs/nife.rs` (§282 (when a file is split, the parent becomes an index)):
//! `pub(super)` here reaches `nife` and its children.
//!
//! Name: provisional (lane maintainer/near-ceiling-splits, 2026-10-12 UTC), for the module and the
//! file.

use super::*;

/// One request on the FS service. Returns the reply's first word as a count/handle, or an
/// `io::Error`. The wire convention is `filesystem_contract`'s: non-negative is a result, negative is a
/// negated errno ([`fsproto::reply_errno`]).
pub(super) fn request(w0: u64, w1: u64) -> io::Result<u64> {
    let (r0, _) = rt::call(FS, w0, w1);
    reply(r0)
}

/// A reply word as a result: [`request`]'s mapping, split out so a reply can be kept and turned into
/// an `io::Error` later. [`FileAttr`] needs that, because `Metadata` is a value std hands back
/// whole and `modified()` is where a refused `GETMTIME` has to surface, not `metadata()`.
pub(super) fn reply(r0: u64) -> io::Result<u64> {
    match fsproto::reply_errno(r0 as i64) {
        None => Ok(r0),
        // The kernel refusing the invoke (a revoked or missing endpoint) is the same answer a
        // program with no directory capability gets: the platform cannot do this.
        Some(_) if no_capability(r0) => Err(unsupported_err()),
        Some(errno) => Err(from_errno(errno)),
    }
}

/// The server's errno into an `io::ErrorKind`, by meaning. There is no errno anywhere else in this
/// PAL: the FS service is the one place nife speaks one, because the component behind it
/// (RedoxFS) does, and §27 maps it at the server boundary and nowhere deeper.
pub(super) fn from_errno(errno: i32) -> io::Error {
    match errno {
        // `EPERM` (milestone 47): the capability lacks the right the verb needs, as when a directory
        // capability lacks `ENUMERATE` or a descent asks for more than the parent holds. It is about
        // *you*, not the name. Unlike `count_names`' refusals, where no capability designates the
        // name and no permission was consulted, here the server judged authority, so
        // `PermissionDenied` is honest. `ReadOnlyFilesystem` is its sibling for the mutating rights.
        1 => io::const_error!(
            io::ErrorKind::PermissionDenied,
            "this directory capability does not carry the right that verb needs"
        ),
        2 => io::const_error!(io::ErrorKind::NotFound, "no such name in the granted directory"),
        9 => io::const_error!(
            io::ErrorKind::InvalidInput,
            "the FS server does not honor that handle"
        ),
        // The four below arrived with milestone 64's bindings, and each one exists because the
        // verbs it belongs to distinguish cases the read/write half never had to. Mapping them all
        // to `Other` would have made `remove_dir` on a full directory indistinguishable from a disk
        // error, which is exactly what a caller loops on.
        17 => io::const_error!(io::ErrorKind::AlreadyExists, "that name is already taken"),
        20 => io::const_error!(io::ErrorKind::NotADirectory, "that name is not a directory"),
        21 => io::const_error!(io::ErrorKind::IsADirectory, "that name is a directory"),
        22 => io::const_error!(io::ErrorKind::InvalidInput, "the FS server refused the request"),
        28 => io::const_error!(io::ErrorKind::StorageFull, "the filesystem is full"),
        // `EROFS` here is a **capability** answer, not a mode bit: the directory capability this
        // process holds does not carry the right this verb needs (§47's ladder). With `EPERM`, it
        // is one of the two errnos in this map that mean "you", rather than "that name".
        30 => io::const_error!(
            io::ErrorKind::ReadOnlyFilesystem,
            "this directory capability does not carry the right that verb needs"
        ),
        39 => io::const_error!(io::ErrorKind::DirectoryNotEmpty, "that directory is not empty"),
        _ => io::const_error!(io::ErrorKind::Other, "the FS server reported a failure"),
    }
}
