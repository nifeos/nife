//! The FS-server core: RedoxFS behind a capability-shaped file service (milestone 32 phase 2).
//!
//! The sans-IO filesystem logic, with no IPC and no device in it, so it is host-tested in
//! milliseconds against a real RedoxFS image and only the wiring needs QEMU. The EL0 binary wraps a
//! [`Server`] in the block-IPC [`redoxfs::Disk`] and the file-service serve loop. See
//! notes/fs-server.md, notes/fs-server/the-core.md and notes/fs-server/caches-and-io.md.

//! # The contract, capability-shaped from birth
//!
//! A [`Server`] is opened bound to one directory ([`Server::open`] binds the RedoxFS root), and
//! every name a client presents is resolved under a directory handle: no absolute path, no `..`
//! escape, no global namespace. The endpoint the server reads on is the directory capability; a
//! client without it can open nothing. A handle is a small integer this server minted and validates
//! against its own table, so forging one is meaningless.

//! # The directory capability (milestone 47)
//!
//! The bound directory is [`filesystem_contract::fs::ROOT`], handle 0, an ordinary table entry. A
//! directory handle carries a [`filesystem_contract::dir::Rights`] set, and [`Server::open_dir`]
//! gives a child the parent's rights intersected with the request. That intersection is the only
//! constructor for a non-root rights set, so no descendant carries a right its ancestor lacked. A
//! file handle inherits its directory's read and write bits. The table is per server, not per
//! client, so the endpoint is the boundary. See notes/dir-capability.md.

//! # The error boundary
//!
//! Every method here returns `syscall::error::Result`, RedoxFS's own error type, unmapped. The
//! translation to the wire (a negated errno, `filesystem_contract::reply_err`) happens once, at the
//! process boundary, and nowhere else: there is no ABI type in here to leak.

#![cfg_attr(not(test), no_std)]

extern crate alloc;

// Crash injection (milestone 37). Host-only: it exists to damage an in-memory image the way a dying
// machine damages a platter, and the EL0 binary has no business carrying it. Gated on `hosttest`
// rather than `test` so an integration test in `tests/` can reach it.
#[cfg(feature = "hosttest")]
pub mod crash;

// Milestone 606 (a directory walk costs what it does on Linux)'s instrument: the walk `std` sends,
// replayed against the core with the device reads counted. Host-only for the same reason.
#[cfg(feature = "hosttest")]
pub mod walk_model;

mod block;
mod memo;

// The serve loop's request dispatch, moved out of the EL0 binary so a host fuzz target and host
// tests drive the same code the running server does.
mod dispatch;
use alloc::vec::Vec;

pub use block::{BLOCK, BlockDisk, BlockIo, CACHE_SLOTS, CachedDisk};
pub use dispatch::ServeEdges;
use filesystem_contract::dir::{self, Rights};
use filesystem_contract::xattr;
use redoxfs::{Disk, FileSystem, Node, Transaction, TreePtr};
use subtree_scope::Kind;
/// **The engine's error type, for a host caller that implements [`redoxfs::Disk`]** (a fuzz target's
/// in-memory disk). Re-exported so such a caller spells it through this crate rather than naming
/// `redox_syscall` itself; it is the type every [`Server`] method returns. Host-only, because
/// nothing in the EL0 build implements a `Disk` from outside. Name: provisional
/// (`lane/fuzz-service-handlers`, 2026-10-04 UTC).
#[cfg(feature = "hosttest")]
pub use syscall::error as engine_error;
use syscall::error::{
    EBADF, EEXIST, EFBIG, EINVAL, EIO, EISDIR, ENOENT, ENOTDIR, ENOTEMPTY, EPERM, EROFS, Error,
    Result,
};

use crate::memo::Memo;

/// The largest end a file may have, in bytes: every record the engine's node tree can address, at
/// the smallest record (one block). A write ending past it, or a truncate growing past it, is
/// refused `EFBIG` before the engine sees it.
///
/// # BUGS
///
/// - Written for an upstream defect whose root fix is a pin divergence (#1606, level 4's `12` to
///   `8` in `vendor/redoxfs/src/node.rs`); kept so the boundary answers `EFBIG` rather than an
///   engine `ERANGE`. Conservative: 8 KiB records could address twice it. See
///   notes/fs-server/the-core.md.
pub const MAX_FILE_END: u64 =
    (128 + 64 * 256 + 32 * 256 * 256 + 16 * 256 * 256 * 256 + 8 * 256 * 256 * 256 * 256)
        * BLOCK as u64;

#[cfg(test)]
mod tests;

mod attribute_store;
mod listing;
mod names;
mod server;

use attribute_store::*;
use listing::*;
pub use names::NameCopy;
use names::*;
pub use server::Server;
