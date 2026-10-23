//! **The GUID Partition Table**: reading one, writing one, and refusing a bad one.
//!
//! The disk format of UEFI 2.10 section 5.3, not the language models, for milestone 57
//! (partitioning and formatting a real drive). No I/O, no allocation, no `unsafe`: it takes bytes
//! the caller read and returns bytes to place, so its tests run on the host against real disks
//! (DECISIONS §7 (testing: QEMU harness + host-testable crates)). [`header::Header::decode`]
//! judges one block, [`entry::Entry::decode`] judges nothing,
//! [`GloballyUniqueIdentifierPartitionTable::parse`] holds every geometry rule, `check_backup`
//! compares the two copies, and every failure is an error. [`span`] says where to read. See
//! notes/globally-unique-identifier-partition-table.md for the layout on disk and the traps.

//! Name: ratified 2026-09-18 (calef, `design/decisions/` §154 (the acronym test is whether the
//! phrase is spoken)): "GUID partition table" and "globally unique identifier" are both spoken, so
//! the name expands all the way. Refused `gpt` and `guid_partition_table`. Known cost: datasheets,
//! error messages and this kernel's own output say `gpt`, so grepping the printed word will not
//! find this identifier. UEFI's own vocabulary (`GPT`, the GPT header) keeps its spelling.

#![no_std]
// milestone 68's ratchet is workspace-wide (§107); this crate opts out until its 23-item
// worklist (notes/doc-coverage.md) is burned down.
#![allow(missing_docs)]

pub mod crc;
pub mod entry;
pub mod guid;
pub mod header;
pub mod span;

use crc::crc32;
pub use entry::Entry;
pub use header::Header;
use uuid::Uuid;

/// The smallest logical block a GPT disk can have, and what almost every disk reports.
pub const MIN_BLOCK_SIZE: usize = 512;

/// The largest logical block this crate handles. 4096 is what a native-4K drive reports, and the
/// header still occupies exactly one block, so the only thing that changes is the LBA arithmetic.
pub const MAX_BLOCK_SIZE: usize = 4096;

/// The primary header always lives at LBA 1, immediately after the protective MBR. Not a
/// convention: the spec fixes it, because it is the one address a reader can know before it has
/// read anything.
pub const PRIMARY_HEADER_LBA: u64 = 1;

/// Entries in a table written by [`GloballyUniqueIdentifierPartitionTable::create`] with a
/// [`ENTRY_ARRAY_BYTES`]-sized buffer, and what every tool in the world writes.
pub const DEFAULT_ENTRY_COUNT: u32 = 128;

/// 16 KiB: `DEFAULT_ENTRY_COUNT * entry::SIZE`, and the minimum array size UEFI 2.10 §5.3.3
/// requires of a *conformant writer*.
///
/// [`GloballyUniqueIdentifierPartitionTable::create`] will build a smaller array if handed a
/// smaller buffer, and [`GloballyUniqueIdentifierPartitionTable::parse`] will read one, because
/// refusing a table we can read perfectly well would be the wrong kind of strict. Anything writing
/// a real disk should pass a buffer of exactly this size.
pub const ENTRY_ARRAY_BYTES: usize = DEFAULT_ENTRY_COUNT as usize * entry::SIZE;

/// True if `len` is a logical block size this crate accepts: a power of two from
/// [`MIN_BLOCK_SIZE`] to [`MAX_BLOCK_SIZE`].
///
/// Block size is never a parameter in this crate's API. A caller passes the block it read, and the
/// slice's own length is the block size, which removes a whole class of "we told it 512 and handed
/// it 4096" bug.
pub const fn is_block_size_ok(len: usize) -> bool {
    len >= MIN_BLOCK_SIZE && len <= MAX_BLOCK_SIZE && len.is_power_of_two()
}

/// The protective MBR at LBA 0: one classic MBR record of type `0xEE` covering the whole disk, so
/// a tool that predates GPT sees a disk entirely spoken for and does not offer to initialise it.
///
/// Only the two fields that do the protecting are checked (it starts at LBA 1 and covers the disk).
/// The CHS fields are vestigial: `sgdisk` writes the real cylinder/head/sector of the last block,
/// macOS writes `FE FF FF`, and both are correct because nothing reads them.
pub mod mbr;

/// Sample tables, for this crate's own doc examples and for anything downstream that wants a GPT
/// without a disk.
///
/// Not behind `#[cfg(test)]`: rustdoc examples compile against the public API, so a helper only the
/// tests can see is a helper the documentation cannot use.
pub mod testing;

// Machine-checked proofs (Kani), behind `#[cfg(kani)]`, which only `cargo kani` (script/verify)
// sets. Where a domain is small enough to count, the tests count it exhaustively: every single-byte
// corruption of a real header block (tests/real_disks.rs) and of a small table (tests/table.rs).
// The harnesses quantify over the table itself, and over all 2^1024 entries and 2^128 GUIDs. See
// notes/globally-unique-identifier-partition-table/the-proofs.md and notes/verification.md.
#[cfg(kani)]
mod verification;

mod error;
mod table;

pub use error::{BackupMismatch, Error, MbrProblem};
pub use table::GloballyUniqueIdentifierPartitionTable;
