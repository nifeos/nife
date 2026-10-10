//! **What `errno` a C program sees when `std` fails** (milestone 835 (a C library, stage 1: files,
//! clock and memory), §265 (a C library started from relibc)).
//!
//! nife's C library implements relibc's `Pal` on nife's own `std` (`c_library/src/platform/nife.rs`),
//! so every failure arrives as a `std::io::Error` and leaves as an `errno`. This crate is that
//! translation and nothing else, so it can be proven on the host in milliseconds: the platform
//! layer itself only builds for the `*-unknown-nife` targets.
//!
//! The mapping is by meaning, the way `std` maps the other way (`decode_error_kind` on Unix). Where
//! a C caller's behavior turns on the value, the value matters more than the wording:
//!
//! - SQLite treats `ENOENT` from `open` as "no such database, create it", and `EACCES` and `EPERM`
//!   as "read-only", so a missing grant is `EACCES`, never `EIO`.
//! - nife's `std` answers `Unsupported` for anything the platform does not provide (no grant, no
//!   such operation), and C's word for that is `ENOSYS`.
//! - Anything with no closer meaning is `EIO`, the errno a C program is least likely to retry.
//!
//! The numbers are Linux's generic ones, which `c_library`'s `errno.h` also uses (`c_library` checks
//! at compile time that its constants equal these, so the two cannot drift).
//!
//! Name: provisional 2026-10-10 (UTC), milestone 835's lane. The C library's crate name (also
//! provisional) plus the one thing this holds. calef has not ruled.

#![forbid(unsafe_code)]

use std::io::ErrorKind;

/// Linux's generic errno values, the subset this mapping produces.
// Each constant is its POSIX name, and the name is its documentation (`errno.h`).
#[allow(missing_docs)]
pub mod value {
    pub const EPERM: i32 = 1;
    pub const ENOENT: i32 = 2;
    pub const E2BIG: i32 = 7;
    pub const EINTR: i32 = 4;
    pub const EIO: i32 = 5;
    pub const EAGAIN: i32 = 11;
    pub const ENOMEM: i32 = 12;
    pub const EACCES: i32 = 13;
    pub const EBUSY: i32 = 16;
    pub const EEXIST: i32 = 17;
    pub const EXDEV: i32 = 18;
    pub const ENOTDIR: i32 = 20;
    pub const EISDIR: i32 = 21;
    pub const EINVAL: i32 = 22;
    pub const ETXTBSY: i32 = 26;
    pub const EFBIG: i32 = 27;
    pub const ENOSPC: i32 = 28;
    pub const ESPIPE: i32 = 29;
    pub const EROFS: i32 = 30;
    pub const EMLINK: i32 = 31;
    pub const EPIPE: i32 = 32;
    pub const EDEADLK: i32 = 35;
    pub const ENAMETOOLONG: i32 = 36;
    pub const ENOSYS: i32 = 38;
    pub const ENOTEMPTY: i32 = 39;
    pub const ELOOP: i32 = 40;
    pub const EILSEQ: i32 = 84;
    pub const EADDRINUSE: i32 = 98;
    pub const EADDRNOTAVAIL: i32 = 99;
    pub const ENETDOWN: i32 = 100;
    pub const ENETUNREACH: i32 = 101;
    pub const ECONNABORTED: i32 = 103;
    pub const ECONNRESET: i32 = 104;
    pub const ENOTCONN: i32 = 107;
    pub const ETIMEDOUT: i32 = 110;
    pub const ECONNREFUSED: i32 = 111;
    pub const EHOSTUNREACH: i32 = 113;
    pub const ESTALE: i32 = 116;
    pub const EDQUOT: i32 = 122;
}

use value::*;

/// The `errno` for a `std::io::Error` of this kind.
pub fn errno_for(kind: ErrorKind) -> i32 {
    match kind {
        ErrorKind::NotFound => ENOENT,
        // A grant that does not include the right asked for. nife's filesystem has no permission
        // bits, so this is the only way a C program meets "permission denied".
        ErrorKind::PermissionDenied => EACCES,
        ErrorKind::AlreadyExists => EEXIST,
        ErrorKind::WouldBlock => EAGAIN,
        ErrorKind::NotADirectory => ENOTDIR,
        ErrorKind::IsADirectory => EISDIR,
        ErrorKind::DirectoryNotEmpty => ENOTEMPTY,
        ErrorKind::ReadOnlyFilesystem => EROFS,
        ErrorKind::StaleNetworkFileHandle => ESTALE,
        ErrorKind::InvalidInput | ErrorKind::InvalidFilename => EINVAL,
        // Bytes that are not text where text was required: on nife, a path that is not UTF-8.
        ErrorKind::InvalidData => EILSEQ,
        ErrorKind::TimedOut => ETIMEDOUT,
        ErrorKind::WriteZero => EIO,
        ErrorKind::StorageFull => ENOSPC,
        ErrorKind::NotSeekable => ESPIPE,
        ErrorKind::QuotaExceeded => EDQUOT,
        ErrorKind::FileTooLarge => EFBIG,
        ErrorKind::ResourceBusy => EBUSY,
        ErrorKind::ExecutableFileBusy => ETXTBSY,
        ErrorKind::Deadlock => EDEADLK,
        ErrorKind::CrossesDevices => EXDEV,
        ErrorKind::TooManyLinks => EMLINK,
        ErrorKind::ArgumentListTooLong => E2BIG,
        ErrorKind::Interrupted => EINTR,
        // The platform has no such operation, or this process holds no capability for it.
        ErrorKind::Unsupported => ENOSYS,
        ErrorKind::OutOfMemory => ENOMEM,
        ErrorKind::BrokenPipe => EPIPE,
        ErrorKind::ConnectionRefused => ECONNREFUSED,
        ErrorKind::ConnectionReset => ECONNRESET,
        ErrorKind::HostUnreachable => EHOSTUNREACH,
        ErrorKind::NetworkUnreachable => ENETUNREACH,
        ErrorKind::ConnectionAborted => ECONNABORTED,
        ErrorKind::NotConnected => ENOTCONN,
        ErrorKind::AddrInUse => EADDRINUSE,
        ErrorKind::AddrNotAvailable => EADDRNOTAVAIL,
        ErrorKind::NetworkDown => ENETDOWN,
        _ => EIO,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The values a C program branches on.** SQLite decides "create the database" on `ENOENT`
    /// and "open it read-only" on `EACCES`, and a portable program treats `ENOSYS` as "this
    /// platform has no such thing". Each is pinned to the kind nife's `std` actually returns for
    /// that case (a missing name, a grant without the right, no capability at all).
    #[test]
    fn the_kinds_a_c_program_branches_on_keep_their_posix_meaning() {
        assert_eq!(errno_for(ErrorKind::NotFound), ENOENT);
        assert_eq!(errno_for(ErrorKind::PermissionDenied), EACCES);
        assert_eq!(errno_for(ErrorKind::Unsupported), ENOSYS);
        assert_eq!(errno_for(ErrorKind::AlreadyExists), EEXIST);
        assert_eq!(errno_for(ErrorKind::IsADirectory), EISDIR);
        assert_eq!(errno_for(ErrorKind::NotADirectory), ENOTDIR);
        assert_eq!(errno_for(ErrorKind::DirectoryNotEmpty), ENOTEMPTY);
        assert_eq!(errno_for(ErrorKind::StorageFull), ENOSPC);
        assert_eq!(errno_for(ErrorKind::InvalidInput), EINVAL);
        assert_eq!(errno_for(ErrorKind::OutOfMemory), ENOMEM);
    }

    /// **Nothing maps to 0**, which C reads as success, and nothing unmapped is silent: a kind
    /// with no closer meaning is `EIO`. `Other` is the kind `std` gives an error it cannot
    /// classify, so it is the one most likely to reach a C program unplanned.
    #[test]
    fn every_kind_is_a_failure_and_the_unclassified_one_is_eio() {
        let kinds = [
            ErrorKind::NotFound,
            ErrorKind::PermissionDenied,
            ErrorKind::ConnectionRefused,
            ErrorKind::BrokenPipe,
            ErrorKind::WouldBlock,
            ErrorKind::TimedOut,
            ErrorKind::WriteZero,
            ErrorKind::Interrupted,
            ErrorKind::Unsupported,
            ErrorKind::UnexpectedEof,
            ErrorKind::Other,
        ];
        for k in kinds {
            assert!(errno_for(k) > 0, "{k:?} maps to {}", errno_for(k));
        }
        assert_eq!(errno_for(ErrorKind::Other), EIO);
        assert_eq!(errno_for(ErrorKind::UnexpectedEof), EIO);
    }

    /// **The numbers are Linux's**, which is what nife's `errno.h` says (it is relibc's Linux
    /// arm, `vendor/relibc/header/errno/mod.rs`). `c_library` asserts at compile time that its
    /// constants equal these; this pins these to the published values.
    #[test]
    fn the_numbers_are_linuxs_generic_ones() {
        assert_eq!(
            (EPERM, ENOENT, EIO, EACCES, EEXIST, EINVAL, ENOSYS),
            (1, 2, 5, 13, 17, 22, 38)
        );
    }
}
