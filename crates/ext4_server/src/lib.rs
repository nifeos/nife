//! **A read-only ext4 server's contract side**: one confined server holding one block-device
//! capability, answering the file-service contract (`file_system_contract::file_system`) over an
//! ext4 volume it never writes. Milestone 190 (ext4, read and write), phase 1.
//!
//! The crate does no input or output of its own and holds no ext4 code. [`Server`] decodes each request, checks the
//! rights a directory capability carries, and asks a [`Volume`] for what is on the disk. A
//! [`Volume`] is an ext4 implementation mounted read-only; [`Sectors`] adapts the block
//! capability's 4,096-byte blocks to the sectors such an implementation reads. Until the adoption
//! ruling on `rsext4`, the only [`Volume`] is the one in `tools/ext4_oracle`, which drives this
//! server through contract requests and compares what it answers with what `debugfs` reads.
//!
//! **Every write is refused.** A verb the contract marks as mutating
//! (`file_system_contract::verb::Verb::mutates`) answers `EROFS` before its handle or its rights
//! are read, so a mutating verb added to the contract is refused from the day its row exists.
//! The binding verbs answer `EINVAL`, as the contract asks of a server that does not resolve
//! through `subtree_scope`'s bindings. The block side refuses too: [`Sectors::write`] answers
//! `EROFS` and reaches no device.
//!
//! # BUGS
//!
//! - Symbolic links and special files are listed but cannot be opened: the contract has no verb
//!   that reads a link's target, and opening one answers `ELOOP` (a link) or `ENXIO` (a device,
//!   a named pipe or a socket).
//! - A modification time before 1970 answers `EOVERFLOW`, because the contract's reply is a
//!   non-negative word of Unix seconds.
//! - A node with more than `extended_attribute::MAXIMUM_ATTRIBUTES` attributes, or more names
//!   than one page holds, answers `LISTXATTR` with `ERANGE`, and a value longer than
//!   `extended_attribute::MAXIMUM_VALUE_BYTES` answers `GETXATTR` with `E2BIG`. ext4 allows both.
//! - The block capability does not carry the device's sector size, so the server reports
//!   [`SECTOR_BYTES`] whatever the device is. Reads are whole blocks either way, so a read-only
//!   mount is right on any device; a write would not be, which is why phase 3 needs the size
//!   carried first.
//!
//! Name: provisional (milestone 190 phase 1's lane). The crate, [`Server`], [`Volume`],
//! [`Sectors`], [`BlockDevice`] and every public item below are the lane's proposals; calef names
//! crates, modules and public functions.

#![no_std]

extern crate alloc;

mod error_number;
mod sectors;
mod server;
mod volume;

#[cfg(test)]
mod tests;

pub use error_number::{
    BAD_HANDLE, ErrorNumber, INPUT_OUTPUT_ERROR, NO_SUCH_DEVICE, NO_SUCH_ENTRY,
    TOO_MANY_OPEN_HANDLES, TOO_MANY_SYMBOLIC_LINKS, VALUE_TOO_LARGE,
};
pub use sectors::{BlockDevice, SECTOR_BYTES, Sectors};
pub use server::Server;
pub use volume::{DirectoryEntry, NodeId, NodeKind, NodeStatus, Volume, VolumeStatistics};
