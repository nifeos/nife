//! **The GUID Partition Table**: reading one, writing one, and refusing a bad one.
//!
//! (The three letters are the disk format from UEFI 2.10 section 5.3, not the language models.)
//!
//! A partition table is the map that says "the filesystem starts at block 2048 and stops at block
//! 32767". Milestone 57 (partitioning and formatting a real drive) needs it both ways: **reading**
//! one is how a kernel finds a partition on a disk it has never seen; **writing** one is the
//! `parted mkpart` equivalent.
//!
//! # This crate does no I/O, on purpose
//!
//! Nothing here reads or writes a block device: every function takes a byte buffer the caller
//! filled and returns bytes the caller will place, so the crate's tests run on the host against
//! real disks made by real tools (DECISIONS §7 (testing: QEMU harness + host-testable crates)). It
//! allocates nothing and has no `unsafe`.
//! [`span`] computes *where* to read: a GPT counts in 512-byte logical blocks and the block service
//! moves 4096 bytes per request, and reconciling them in a driver would mean doing it three times.
//!

//! # What a GPT actually is, in the order the bytes appear
//!
//! ```text
//!   LBA 0     protective MBR   one fake "the whole disk is in use" record, type 0xEE
//!   LBA 1     primary header   92 bytes: signature, geometry, two CRC32s, disk GUID
//!   LBA 2..   entry array      128 entries x 128 bytes = 16 KiB = 32 blocks
//!   ...       the partitions themselves, between first_usable_lba and last_usable_lba
//!   LBA n-33  backup array     a second copy of the same 16 KiB
//!   LBA n-1   backup header    the same header, with my_lba and alternate_lba swapped
//! ```
//!
//! Four things carry the integrity of that: the header CRC and the array CRC, each stored twice.
//! **A GPT is a format that can tell you it is broken**, which is exactly why every check in here
//! is an error and none of them is a warning. A table that half-validates is a table you are about
//! to write a filesystem on top of.
//!
//! # The layering, and why the errors are where they are
//!
//! - [`header::Header::decode`] judges one block on its own terms: is this a GPT header at all?
//! - [`entry::Entry::decode`] judges nothing. It is **total**, every bit pattern decodes, and the
//!   round trip is the identity. There is no such thing as a malformed entry, only an entry that
//!   makes no sense on a particular disk.
//! - [`GloballyUniqueIdentifierPartitionTable::parse`] is where the disk shows up, and therefore
//!   where every geometry rule lives: the array CRC, the usable range, partitions that run off the
//!   end, partitions that overlap.
//! - [`GloballyUniqueIdentifierPartitionTable::check_backup`] compares the two copies. A disk whose
//!   halves disagree has had something happen to it and is not to be trusted.
//!
//! See notes/globally-unique-identifier-partition-table.md for the format walk-through and the
//! surprises the fixtures pinned.
//!

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

/// The protective MBR at LBA 0.
///
/// # Why a GPT disk starts with a lie
///
/// The first block of a GPT disk holds a classic MBR partition table describing **one** partition,
/// of type `0xEE`, covering the whole disk. Nothing ever mounts it. It exists so that a tool old
/// enough to not know about GPT sees a disk that is entirely spoken for, and therefore does not
/// helpfully offer to initialise it. The failure mode it defends against is a partition editor from
/// 2001 deciding a 4 TB disk is empty.
///
/// It is genuinely a *protective* device rather than a description, which is why the checks here
/// are about the two fields that do the protecting (start at LBA 1, cover the disk) and not about
/// the CHS fields, which are vestigial: `sgdisk` writes the real cylinder/head/sector of the last
/// block, macOS writes `FE FF FF`, and both are correct because nothing reads them.
pub mod mbr;

/// Sample tables, for this crate's own doc examples and for anything downstream that wants a GPT
/// without a disk.
///
/// Not behind `#[cfg(test)]`: rustdoc examples compile against the public API, so a helper only the
/// tests can see is a helper the documentation cannot use.
pub mod testing;

// =================================================================================================
// Machine-checked proofs (Kani). Behind `#[cfg(kani)]`, so an ordinary build or test never sees
// them; only `cargo kani` (script/verify) sets that cfg. See notes/verification.md and
// notes/globally-unique-identifier-partition-table.md.
//
// The division of labour with the tests is deliberate and is the one `network_time_protocol` argued for:
// **where a domain is small enough to count, the tests count it, exhaustively, and that is a
// stronger result than any bounded solver run.** Every single-byte corruption of a real 512-byte
// header block is enumerated in tests/real_disks.rs; every value of every byte of a small table is
// enumerated in tests/table.rs.
//
// What enumeration cannot do is quantify over the *table*. Those tests say "every corruption of
// this table is caught"; the harnesses below say "for every buffer there is, a changed byte changes
// the CRC", which is the general fact the tests are instances of. That, and the round trips over
// 2^1024 entries and 2^128 GUIDs, is what a model checker is for.
// =================================================================================================
#[cfg(kani)]
mod verification;

mod error;
mod table;

pub use error::{BackupMismatch, Error, MbrProblem};
pub use table::GloballyUniqueIdentifierPartitionTable;
