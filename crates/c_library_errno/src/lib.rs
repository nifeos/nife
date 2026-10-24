//! **What `errno` a C program sees when `std` fails** (milestone 835 (a C library, stage 1: files,
//! clock and memory), §265 (a C library started from relibc)).
//!
//! nife's C library implements relibc's `Pal` on nife's own `std` (`c_library/src/platform/nife.rs`),
//! so every failure arrives as a `std::io::Error` and leaves as an `errno`. This crate is that
//! translation alone, so it is proven on the host. The mapping is by meaning: a missing grant is
//! `EACCES` (SQLite reads it as read-only), `Unsupported` is `ENOSYS`, and anything with no closer
//! meaning is `EIO`. The numbers are Linux's generic ones, and `c_library` checks at compile time
//! that its `errno.h` agrees. See notes/c-library.md, "The errno a C program sees".
//!
//! Name: ratified 2026-10-18 (calef). The C library's crate name plus the one thing this holds.
//! `errno` is C's own identifier, kept whole as a foreign name (design/decisions/0154's table).

#![forbid(unsafe_code)]
// The kinds `std`'s Unix port already decodes `ELOOP`, `EINPROGRESS` and `EMFILE` into are unstable
// on the pinned nightly; mapping them is what keeps this table the inverse of `std`'s.
#![feature(io_error_more, io_error_inprogress, io_error_too_many_open_files)]

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
    pub const EMFILE: i32 = 24;
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
    pub const EINPROGRESS: i32 = 115;
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
        ErrorKind::InvalidInput => EINVAL,
        // `std`'s own Unix decoding reads `ENAMETOOLONG` as this kind, so this is its inverse.
        ErrorKind::InvalidFilename => ENAMETOOLONG,
        ErrorKind::FilesystemLoop => ELOOP,
        ErrorKind::InProgress => EINPROGRESS,
        ErrorKind::TooManyOpenFiles => EMFILE,
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

    /// Every kind this table maps on purpose, which is every kind `std`'s Unix decoding produces
    /// except `EIO`'s (`InputOutputError`, unstable, and `EIO` here through the default arm).
    const DECODED: [ErrorKind; 38] = [
        ErrorKind::ArgumentListTooLong,
        ErrorKind::AddrInUse,
        ErrorKind::AddrNotAvailable,
        ErrorKind::ResourceBusy,
        ErrorKind::ConnectionAborted,
        ErrorKind::ConnectionRefused,
        ErrorKind::ConnectionReset,
        ErrorKind::Deadlock,
        ErrorKind::QuotaExceeded,
        ErrorKind::AlreadyExists,
        ErrorKind::FileTooLarge,
        ErrorKind::HostUnreachable,
        ErrorKind::Interrupted,
        ErrorKind::InvalidInput,
        ErrorKind::IsADirectory,
        ErrorKind::FilesystemLoop,
        ErrorKind::NotFound,
        ErrorKind::OutOfMemory,
        ErrorKind::StorageFull,
        ErrorKind::TooManyLinks,
        ErrorKind::InvalidFilename,
        ErrorKind::NetworkDown,
        ErrorKind::NetworkUnreachable,
        ErrorKind::NotConnected,
        ErrorKind::NotADirectory,
        ErrorKind::DirectoryNotEmpty,
        ErrorKind::BrokenPipe,
        ErrorKind::ReadOnlyFilesystem,
        ErrorKind::NotSeekable,
        ErrorKind::StaleNetworkFileHandle,
        ErrorKind::TimedOut,
        ErrorKind::ExecutableFileBusy,
        ErrorKind::CrossesDevices,
        ErrorKind::InProgress,
        ErrorKind::TooManyOpenFiles,
        ErrorKind::PermissionDenied,
        ErrorKind::Unsupported,
        ErrorKind::WouldBlock,
    ];

    /// **This table is the inverse of `std`'s.** A C program's `errno` and the `std` error nife
    /// started from must name the same failure, and `std`'s Unix port already says which `errno`
    /// means which kind (`decode_error_kind`). On a Linux host, where `errno` has the numbers
    /// `errno.h` declares, every kind mapped here decodes back to itself. This found four
    /// disagreements when it was written (`InvalidFilename` mapped to `EINVAL`, and three kinds
    /// falling through to `EIO`).
    #[test]
    fn every_kind_decodes_back_through_stds_own_unix_table() {
        for kind in DECODED {
            let n = errno_for(kind);
            assert!(n > 0, "{kind:?} maps to {n}");
            #[cfg(target_os = "linux")]
            assert_eq!(
                std::io::Error::from_raw_os_error(n).kind(),
                kind,
                "{kind:?} maps to {n}, which std reads back as another kind",
            );
        }
    }

    /// **Kinds that name no POSIX failure map by meaning, not to a catch-all.** `InvalidData` is
    /// bytes that are not text where text was required, which on nife is a path that is not UTF-8:
    /// `EILSEQ`. `WriteZero` is a write that wrote nothing, which C reads as an I/O error.
    #[test]
    fn the_kinds_std_never_decodes_still_mean_something() {
        assert_eq!(errno_for(ErrorKind::InvalidData), EILSEQ);
        assert_eq!(errno_for(ErrorKind::WriteZero), EIO);
        assert_eq!(errno_for(ErrorKind::OutOfMemory), ENOMEM);
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
