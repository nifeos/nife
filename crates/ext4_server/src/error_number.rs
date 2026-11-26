//! **The error numbers the server answers with**, as the contract numbers them (Linux's), and
//! the type that carries one until the server turns it into a reply word. Each constant spells out
//! what its C library name abbreviates, and carries that name as its alias and the first word of its
//! documentation.
//!
//! Name: provisional, minted 2026-11-26 (UTC) by milestone/190-ext4-server-read-only.

/// An error number, as the contract numbers them (Linux's), from 1 to
/// `file_system_contract::MAXIMUM_ERRNO`. The server turns it into a reply word at one place.
///
/// Name: provisional, minted 2026-11-26 (UTC) by milestone/190-ext4-server-read-only.
#[doc(alias = "errno")]
pub type ErrorNumber = i32;

/// `ENOENT`: no such name. The contract carries no constant for it.
///
/// Name: provisional, minted 2026-11-26 (UTC) by milestone/190-ext4-server-read-only.
#[doc(alias = "ENOENT")]
pub const NO_SUCH_ENTRY: ErrorNumber = 2;
/// `EIO`: the volume could not be read.
///
/// Name: provisional, minted 2026-11-26 (UTC) by milestone/190-ext4-server-read-only.
#[doc(alias = "EIO")]
pub const INPUT_OUTPUT_ERROR: ErrorNumber = 5;
/// `ENXIO` (no such device or address): a device, a named pipe or a socket, which this server cannot open.
///
/// Name: provisional, minted 2026-11-26 (UTC) by milestone/190-ext4-server-read-only.
#[doc(alias = "ENXIO")]
pub const NO_SUCH_DEVICE: ErrorNumber = 6;
/// `EBADF`: a handle this server did not mint, or one that lacks the right a verb on it needs.
///
/// Name: provisional, minted 2026-11-26 (UTC) by milestone/190-ext4-server-read-only.
#[doc(alias = "EBADF")]
pub const BAD_HANDLE: ErrorNumber = file_system_contract::verb::file_grant::EBADF;
/// `EMFILE`: every handle this server can hold is open.
///
/// Name: provisional, minted 2026-11-26 (UTC) by milestone/190-ext4-server-read-only.
#[doc(alias = "EMFILE")]
pub const TOO_MANY_OPEN_HANDLES: ErrorNumber = 24;
/// `ELOOP`: a symbolic link, which this server never follows.
///
/// Name: provisional, minted 2026-11-26 (UTC) by milestone/190-ext4-server-read-only.
#[doc(alias = "ELOOP")]
pub const TOO_MANY_SYMBOLIC_LINKS: ErrorNumber = 40;
/// `EOVERFLOW`: a modification time the contract's reply word cannot carry.
///
/// Name: provisional, minted 2026-11-26 (UTC) by milestone/190-ext4-server-read-only.
#[doc(alias = "EOVERFLOW")]
pub const VALUE_TOO_LARGE: ErrorNumber = 75;
