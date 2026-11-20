//! **The globally unique identifier (GUID) partition table**, the GPT (GUID Partition Table):
//! reading one, writing one, and refusing a bad one.
//!
//! The disk format of the Unified Extensible Firmware Interface (UEFI) 2.10 section 5.3, not the
//! language models, for partitioning and formatting a real drive. No I/O, no allocation, no
//! `unsafe`: it takes bytes the caller read and returns bytes to place, so its tests run on the
//! host against real disks. [`header::Header::decode`] judges one block, [`entry::Entry::decode`]
//! judges nothing, [`PartitionTable::parse`] holds every geometry rule, `check_backup` compares the
//! two copies, and every failure is an error. [`span`] says where to read.
//!
//! The reader enforces the header's reserved tail (a non-zero byte after the header refuses the
//! table) and tolerates a non-zero tail on an entry and non-zero reserved attribute bits, though
//! UEFI says those "must be zero" too: the header's tail protects the meaning of its CRC, and
//! nothing this crate reads depends on the others.
//!
//! A GUID here is a [`uuid::Uuid`], re-exported as [`uuid`] so a caller needs no second dependency
//! to name one. It is an opaque sixteen-byte identifier: the crate neither checks nor generates a
//! version or a variant, and it adds only the on-disk byte order ([`guid::from_disk`] and
//! [`guid::to_disk`]).

//! Name: ratified 2026-09-18 (calef, `design/decisions/` §154 (the acronym test is whether the
//! phrase is spoken)): "GUID partition table" and "globally unique identifier" are both spoken, so
//! the name expands all the way. Refused `gpt` and `guid_partition_table`. Known cost: datasheets,
//! error messages and this kernel's own output say `gpt`, so grepping the printed word will not
//! find this identifier. UEFI's own vocabulary (`GPT`, the GPT header) keeps its spelling.

#![no_std]
#![forbid(unsafe_code)]

#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeExample;

pub mod crc;
pub mod entry;
pub mod guid;
pub mod header;
pub mod span;

use crc::crc32;
pub use entry::Entry;
pub use header::Header;
pub use uuid;
use uuid::Uuid;

/// The smallest logical block a GPT disk can have, and what almost every disk reports.
///
/// Name: provisional.
pub const MINIMUM_BLOCK_BYTES: usize = 512;

/// The format's version: the generation of this crate's reading and writing of the two on-disk
/// tables it implements, the GUID partition table and the master boot record (MBR). The tables' own
/// identities are the specifications' (`header::REVISION`, the `EFI PART` and `0x55AA` signatures);
/// this constant is a declaration, so assembly can check what a package was built against. Nothing
/// checks it at run time.
///
/// Name: provisional, minted 2026-10-29 (UTC) by lane milestone/956-checkable-interface-versions.
pub const VERSION: u32 = 1;

/// The largest logical block this crate handles. 4096 is what a native-4K drive reports, and the
/// header still occupies exactly one block, so the only thing that changes is the LBA arithmetic.
///
/// Name: provisional.
pub const MAXIMUM_BLOCK_BYTES: usize = 4096;

/// The primary header always lives at LBA 1, immediately after the protective MBR. Not a
/// convention: the spec fixes it, because it is the one address a reader can know before it has
/// read anything.
///
/// Name: provisional.
pub const PRIMARY_HEADER_LOGICAL_BLOCK_ADDRESS: u64 = 1;

/// Entries in a table written by [`PartitionTable::create`] with a
/// [`ENTRY_ARRAY_BYTES`]-sized buffer, and what every tool in the world writes.
pub const DEFAULT_ENTRY_COUNT: u32 = 128;

/// 16 KiB: `DEFAULT_ENTRY_COUNT * entry::ENTRY_BYTES`, and the minimum array size UEFI 2.10 section
/// 5.3.3 requires of a *conformant writer*.
///
/// [`PartitionTable::create`] will build a smaller array if handed a
/// smaller buffer, and [`PartitionTable::parse`] will read one, because
/// refusing a table we can read perfectly well would be the wrong kind of strict. Anything writing
/// a real disk should pass a buffer of exactly this size.
pub const ENTRY_ARRAY_BYTES: usize = DEFAULT_ENTRY_COUNT as usize * entry::ENTRY_BYTES;

/// True if `len` is a logical block size this crate accepts: a power of two from
/// [`MINIMUM_BLOCK_BYTES`] to [`MAXIMUM_BLOCK_BYTES`].
///
/// Block size is never a parameter in this crate's API. A caller passes the block it read, and the
/// slice's own length is the block size, which removes a whole class of "we told it 512 and handed
/// it 4096" bug.
pub const fn is_block_size_ok(len: usize) -> bool {
    len >= MINIMUM_BLOCK_BYTES && len <= MAXIMUM_BLOCK_BYTES && len.is_power_of_two()
}

/// The protective MBR at LBA 0: one classic MBR record of type `0xEE` covering the whole disk, so
/// a tool that predates GPT sees a disk entirely spoken for and does not offer to initialize it.
/// The record and its check are [`::master_boot_record::protective`]'s; this module fits them to a
/// block of any accepted size.
///
/// Name: provisional.
#[doc(alias = "MBR")]
pub mod master_boot_record;

/// Sample tables, for this crate's own doc examples and for anything downstream that wants a GPT
/// without a disk.
///
/// Not behind `#[cfg(test)]`: rustdoc examples compile against the public API, so a helper only the
/// tests can see is a helper the documentation cannot use.
pub mod testing;

// Machine-checked proofs (Kani), behind `#[cfg(kani)]`, which only `cargo kani` sets. Where a domain is small enough to count, the tests count it exhaustively: every single-byte
// corruption of a real header block (tests/real_disks.rs) and of a small table (tests/table.rs).
// The harnesses quantify over the table itself, and over all 2^1024 entries and 2^128 GUIDs.
#[cfg(kani)]
mod verification;

mod error;
mod table;

pub use error::{BackupMismatch, Error, MasterBootRecordProblem};
pub use table::PartitionTable;
