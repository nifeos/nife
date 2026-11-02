//! **Why a table was refused**, and how each refusal reads.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

/// Everything this crate refuses, and why.
///
/// One variant per rule, carrying the numbers that make the failure diagnosable, because "invalid
/// GPT" is useless to somebody holding the only copy of their data.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Error {
    /// A buffer offered as one logical block is not a legal block size. See [`is_block_size_ok`](crate::is_block_size_ok).
    BlockSize(usize),
    /// The eight bytes at offset 0 are not `EFI PART`. Usually means "this disk has no GPT",
    /// which is a normal answer rather than a corruption.
    Signature,
    /// A revision other than [`header::REVISION`](crate::header::REVISION). Refused rather than guessed at.
    Revision(u32),
    /// `HeaderSize` outside `92 ..= block_size`.
    HeaderSize(u32),
    /// The reserved `u32` at offset 20 is not zero.
    ReservedNotZero,
    /// A non-zero byte after the header in its block, which UEFI 2.10 section 5.3.2 forbids.
    BlockTailNotZero,
    /// The header's own CRC does not match its bytes. Something rewrote part of the header.
    HeaderCrc { stored: u32, computed: u32 },
    /// `SizeOfPartitionEntry` is under 128 or is not a multiple of 8.
    EntrySize(u32),
    /// `NumberOfPartitionEntries` is zero, or so large that the array cannot be addressed.
    EntryCount(u32),
    /// The caller supplied fewer entry-array bytes than the header says the array has. Not a
    /// corrupt disk: a caller that read too few blocks.
    EntryArrayLen { need: usize, have: usize },
    /// [`GloballyUniqueIdentifierPartitionTable::create`](crate::GloballyUniqueIdentifierPartitionTable::create) was handed an entry-array buffer that is
    /// not a whole number of 128-byte entries, or is empty. The buffer's size *is*
    /// `NumberOfPartitionEntries`, so it cannot be a fractional entry.
    EntryArrayShape { have: usize },
    /// The entry array's CRC does not match the header's record of it.
    EntryArrayCrc { stored: u32, computed: u32 },
    /// [`GloballyUniqueIdentifierPartitionTable::parse`](crate::GloballyUniqueIdentifierPartitionTable::parse) was given a header that says it lives
    /// somewhere other than LBA 1. A backup header goes to
    /// [`GloballyUniqueIdentifierPartitionTable::check_backup`](crate::GloballyUniqueIdentifierPartitionTable::check_backup), which knows what to compare it
    /// against.
    NotPrimary { my_lba: u64 },
    /// `FirstUsableLBA` is above `LastUsableLBA`, so the disk has no usable blocks at all.
    UsableRange { first: u64, last: u64 },
    /// The table does not fit outside the usable range: either the primary array runs into
    /// `FirstUsableLBA`, or `LastUsableLBA` runs into the backup array or header. A partition
    /// placed in that space would be overwritten by the next table update.
    TableOverlapsUsable,
    /// A used entry with `last_lba < first_lba`, which is not a partition.
    PartitionRange { index: usize },
    /// A used entry that starts below `FirstUsableLBA` or ends above `LastUsableLBA`. This is the
    /// check that catches a table copied onto a smaller disk.
    PartitionOutsideUsable { index: usize },
    /// Two used entries claim a block in common. Zero-indexed, and always reported with the lower
    /// index first.
    PartitionsOverlap { a: usize, b: usize },
    /// The backup header disagrees with the primary about something they must agree on.
    Backup(BackupMismatch),
    /// The protective MBR at LBA 0 is wrong. See [`MbrProblem`].
    Mbr(MbrProblem),
    /// [`GloballyUniqueIdentifierPartitionTable::create`](crate::GloballyUniqueIdentifierPartitionTable::create) was asked to build a table on a disk with
    /// no room for the tables themselves.
    DiskTooSmall { blocks: u64, need: u64 },
    /// More partitions than the entry array has room for.
    TooManyPartitions { given: usize, room: u32 },
    /// A partition name that does not fit in 36 UTF-16 code units, or an output buffer too small
    /// to receive one.
    NameTooLong,
}

/// Which field the backup header disagreed with the primary on.
///
/// Every one of these is a field the two copies must hold identically, so a mismatch means one of
/// them was written by something that did not update the other. That is usually a resize or a
/// clone-to-a-different-disk, and it is exactly the state where guessing which copy is right does
/// real damage.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BackupMismatch {
    /// The backup does not think it lives where the primary says it does.
    MyLba,
    /// The backup does not point back at the primary.
    AlternateLba,
    /// Different disk GUIDs: these are two different disks' tables.
    DiskGuid,
    FirstUsableLba,
    LastUsableLba,
    EntryCount,
    EntrySize,
    /// The two copies of the entry array are not the same array.
    EntryArrayCrc,
    /// The backup's array is not immediately below the backup header, where it must be.
    EntryArrayLba,
}

/// What is wrong with the protective MBR.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MbrProblem {
    /// The `0x55AA` at offset 510 is missing. Not an MBR at all.
    Signature,
    /// No partition record of type `0xEE`. A disk with a real MBR partition table and no GPT.
    NoProtectiveRecord,
    /// A second non-empty record beside the protective one: a **hybrid MBR**, where the same disk
    /// is described twice and the two descriptions can disagree. Refused deliberately; see
    /// [`mbr::validate`](crate::mbr::validate).
    ExtraRecord { index: usize },
    /// The protective record does not start at LBA 1, so it does not cover the GPT it protects.
    StartLba(u32),
    /// The protective record's size does not cover the disk. Its whole job is to make a
    /// GPT-unaware tool see a full disk.
    SizeLba(u32),
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Error::BlockSize(n) => write!(
                f,
                "{n} is not a logical block size (512..=4096, a power of two)"
            ),
            Error::Signature => f.write_str("no EFI PART signature: this is not a GPT header"),
            Error::Revision(r) => write!(f, "GPT revision {r:#010x} is not 1.0"),
            Error::HeaderSize(n) => write!(f, "header size {n} is outside 92..=block size"),
            Error::ReservedNotZero => {
                f.write_str("the reserved word at header offset 20 is not zero")
            }
            Error::BlockTailNotZero => {
                f.write_str("the reserved tail of the header block is not zero")
            }
            Error::HeaderCrc { stored, computed } => {
                write!(f, "header CRC {stored:#010x}, computed {computed:#010x}")
            }
            Error::EntrySize(n) => write!(f, "entry size {n} is under 128 or not a multiple of 8"),
            Error::EntryCount(n) => write!(f, "entry count {n} is unusable"),
            Error::EntryArrayLen { need, have } => {
                write!(f, "entry array needs {need} bytes, {have} supplied")
            }
            Error::EntryArrayShape { have } => {
                write!(f, "{have} bytes is not a whole number of 128-byte entries")
            }
            Error::EntryArrayCrc { stored, computed } => {
                write!(
                    f,
                    "entry array CRC {stored:#010x}, computed {computed:#010x}"
                )
            }
            Error::NotPrimary { my_lba } => {
                write!(f, "header says it lives at LBA {my_lba}, not 1")
            }
            Error::UsableRange { first, last } => {
                write!(f, "usable range {first}..={last} is empty")
            }
            Error::TableOverlapsUsable => f.write_str("the table and the usable range overlap"),
            Error::PartitionRange { index } => write!(f, "partition {index} ends before it starts"),
            Error::PartitionOutsideUsable { index } => {
                write!(f, "partition {index} is outside the usable range")
            }
            Error::PartitionsOverlap { a, b } => write!(f, "partitions {a} and {b} overlap"),
            Error::Backup(m) => write!(f, "the backup header disagrees about {m:?}"),
            Error::Mbr(p) => write!(f, "the protective MBR is wrong: {p:?}"),
            Error::DiskTooSmall { blocks, need } => {
                write!(f, "{blocks} blocks is too small for a GPT, {need} needed")
            }
            Error::TooManyPartitions { given, room } => {
                write!(f, "{given} partitions, room for {room}")
            }
            Error::NameTooLong => f.write_str("a partition name does not fit"),
        }
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::string::{String, ToString};
    use std::vec::Vec;

    use super::*;

    /// **Every refusal reads as its own sentence.** The message is what a person sees when a disk
    /// will not open, so two checks that rendered the same text would leave them unable to tell
    /// which one failed. One value of every variant, each rendered, and no two alike.
    #[test]
    fn every_refusal_renders_a_distinct_message() {
        let all = [
            Error::BlockSize(1000),
            Error::Signature,
            Error::Revision(2),
            Error::HeaderSize(10),
            Error::ReservedNotZero,
            Error::BlockTailNotZero,
            Error::HeaderCrc {
                stored: 1,
                computed: 2,
            },
            Error::EntrySize(7),
            Error::EntryCount(0),
            Error::EntryArrayLen { need: 2, have: 1 },
            Error::EntryArrayShape { have: 3 },
            Error::EntryArrayCrc {
                stored: 1,
                computed: 2,
            },
            Error::NotPrimary { my_lba: 9 },
            Error::UsableRange { first: 5, last: 4 },
            Error::TableOverlapsUsable,
            Error::PartitionRange { index: 1 },
            Error::PartitionOutsideUsable { index: 1 },
            Error::PartitionsOverlap { a: 0, b: 1 },
            Error::Backup(BackupMismatch::DiskGuid),
            Error::Mbr(MbrProblem::Signature),
            Error::DiskTooSmall {
                blocks: 1,
                need: 68,
            },
            Error::TooManyPartitions {
                given: 200,
                room: 128,
            },
            Error::NameTooLong,
        ];
        let rendered: Vec<String> = all.iter().map(ToString::to_string).collect();
        for (i, a) in rendered.iter().enumerate() {
            assert!(!a.is_empty(), "{:?} renders nothing", all[i]);
            for (j, b) in rendered.iter().enumerate().skip(i + 1) {
                assert_ne!(a, b, "{:?} and {:?} read the same", all[i], all[j]);
            }
        }
    }
}
