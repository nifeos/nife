//! **Why a table was refused**, and how each refusal reads.
//!
//! Name: ratified 2026-11-20 (#3864).

/// Everything this crate refuses, and why.
///
/// One variant per rule, carrying the numbers that make the failure diagnosable, because "invalid
/// GPT" is useless to somebody holding the only copy of their data.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[non_exhaustive]
pub enum Error {
    /// A buffer offered as one logical block is not a legal block size. See [`is_block_size_ok`](crate::is_block_size_ok).
    BlockSize(usize),
    /// The eight bytes at offset 0 are not `EFI PART`. Usually means "this disk has no GPT",
    /// which is a normal answer rather than a corruption.
    Signature,
    /// A revision other than [`header::REVISION`](crate::header::REVISION). Refused rather than guessed at.
    Revision(u32),
    /// `HeaderSize` outside `92 ..= block_bytes`.
    HeaderSize(u32),
    /// The reserved `u32` at offset 20 is not zero.
    ReservedNotZero,
    /// A non-zero byte after the header in its block, which UEFI 2.10 section 5.3.2 forbids.
    BlockTailNotZero,
    /// The header's own CRC does not match its bytes. Something rewrote part of the header.
    HeaderCrc {
        /// The CRC the header stores.
        stored: u32,
        /// The CRC computed over the header bytes.
        computed: u32,
    },
    /// `SizeOfPartitionEntry` is under 128 or is not a multiple of 8.
    EntryBytes(u32),
    /// `NumberOfPartitionEntries` is zero, or so large that the array cannot be addressed.
    EntryCount(u32),
    /// The caller supplied fewer entry-array bytes than the header says the array has. Not a
    /// corrupt disk: a caller that read too few blocks.
    EntryArrayLen {
        /// The bytes the header says the array has.
        need: usize,
        /// The bytes the caller supplied.
        have: usize,
    },
    /// [`PartitionTable::create`](crate::PartitionTable::create) was handed an entry-array buffer that is
    /// not a whole number of 128-byte entries, or is empty. The buffer's size *is*
    /// `NumberOfPartitionEntries`, so it cannot be a fractional entry.
    EntryArrayShape {
        /// The length of the buffer the caller supplied, in bytes.
        have: usize,
    },
    /// The entry array's CRC does not match the header's record of it. Either the array is
    /// corrupt or it was read from the wrong block: the header's array location is not always
    /// block 2.
    EntryArrayCrc {
        /// The CRC the header stores for the array.
        stored: u32,
        /// The CRC computed over the array bytes.
        computed: u32,
    },
    /// [`PartitionTable::parse`](crate::PartitionTable::parse) was given a header that says it lives
    /// somewhere other than LBA 1. A backup header goes to
    /// [`PartitionTable::check_backup`](crate::PartitionTable::check_backup), which knows what to compare it
    /// against.
    NotPrimary {
        /// The block the header says it lives at.
        my_logical_block_address: u64,
    },
    /// [`PartitionTable::parse_backup`](crate::PartitionTable::parse_backup)
    /// was given a header that says it lives somewhere other than the last block of the disk.
    NotBackup {
        /// The block the header says it lives at.
        claimed_block: u64,
    },
    /// The disk the header describes is not the size of the device.
    /// See [`PartitionTable::check_disk_size`](crate::PartitionTable::check_disk_size).
    DiskSize {
        /// The blocks the header says the disk has.
        header: u64,
        /// The blocks the device has.
        device: u64,
    },
    /// `FirstUsableLBA` is above `LastUsableLBA`, so the disk has no usable blocks at all.
    UsableRange {
        /// The header's first usable block.
        first: u64,
        /// The header's last usable block.
        last: u64,
    },
    /// The table does not fit outside the usable range: either the primary array runs into
    /// `FirstUsableLBA`, or `LastUsableLBA` runs into the backup array or header. A partition
    /// placed in that space would be overwritten by the next table update.
    TableOverlapsUsable,
    /// A used entry with `last_logical_block_address < first_logical_block_address`, which is not a partition.
    PartitionRange {
        /// The entry's zero-based index in the array.
        index: usize,
    },
    /// A used entry that starts below `FirstUsableLBA` or ends above `LastUsableLBA`. This is the
    /// check that catches a table copied onto a smaller disk.
    PartitionOutsideUsable {
        /// The entry's zero-based index in the array.
        index: usize,
    },
    /// Two used entries claim a block in common. Zero-indexed, and always reported with the lower
    /// index first.
    PartitionsOverlap {
        /// The lower entry index.
        first: usize,
        /// The higher entry index.
        second: usize,
    },
    /// The backup header disagrees with the primary about something they must agree on.
    Backup(BackupMismatch),
    /// The protective MBR at LBA 0 is wrong. See [`MasterBootRecordProblem`].
    ///
    /// Name: ratified 2026-11-20 (#3864).
    #[doc(alias = "MBR")]
    MasterBootRecord(MasterBootRecordProblem),
    /// [`PartitionTable::create`](crate::PartitionTable::create) was asked to build a table on a disk with
    /// no room for the tables themselves.
    DiskTooSmall {
        /// The blocks the disk has.
        blocks: u64,
        /// The fewest blocks a table of this shape fits on.
        need: u64,
    },
    /// More partitions than the entry array has room for.
    TooManyPartitions {
        /// The partitions the caller supplied.
        given: usize,
        /// The entries the array holds.
        room: u32,
    },
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
#[non_exhaustive]
pub enum BackupMismatch {
    /// The backup does not think it lives where the primary says it does.
    ///
    /// Name: ratified 2026-11-20 (#3864).
    #[doc(alias = "MyLBA")]
    MyLogicalBlockAddress,
    /// The backup does not point back at the primary.
    ///
    /// Name: ratified 2026-11-20 (#3864).
    #[doc(alias = "AlternateLBA")]
    AlternateLogicalBlockAddress,
    /// Different disk GUIDs: these are two different disks' tables.
    DiskGuid,
    /// `FirstUsableLBA` differs.
    ///
    /// Name: ratified 2026-11-20 (#3864).
    #[doc(alias = "FirstUsableLBA")]
    FirstUsableLogicalBlockAddress,
    /// `LastUsableLBA` differs.
    ///
    /// Name: ratified 2026-11-20 (#3864).
    #[doc(alias = "LastUsableLBA")]
    LastUsableLogicalBlockAddress,
    /// `NumberOfPartitionEntries` differs.
    EntryCount,
    /// `SizeOfPartitionEntry` differs.
    EntryBytes,
    /// The two copies of the entry array are not the same array.
    EntryArrayCrc,
    /// The backup's array is not immediately below the backup header, where it must be.
    ///
    /// Name: ratified 2026-11-20 (#3864).
    #[doc(alias = "PartitionEntryLBA")]
    EntryArrayLogicalBlockAddress,
}

/// What is wrong with the protective MBR: [`::master_boot_record::protective::Problem`], under the
/// name this crate's [`Error::MasterBootRecord`] carries.
#[doc(alias = "MBR")]
pub use ::master_boot_record::protective::Problem as MasterBootRecordProblem;

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
            Error::EntryBytes(n) => write!(f, "entry size {n} is under 128 or not a multiple of 8"),
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
            Error::NotPrimary {
                my_logical_block_address,
            } => {
                write!(
                    f,
                    "header says it lives at LBA {my_logical_block_address}, not 1"
                )
            }
            Error::NotBackup { claimed_block } => {
                write!(
                    f,
                    "header says it lives at LBA {claimed_block}, not the last block"
                )
            }
            Error::DiskSize { header, device } => {
                write!(
                    f,
                    "the header describes {header} blocks, the device has {device}"
                )
            }
            Error::UsableRange { first, last } => {
                write!(f, "usable range {first}..={last} is empty")
            }
            Error::TableOverlapsUsable => f.write_str("the table and the usable range overlap"),
            Error::PartitionRange { index } => write!(f, "partition {index} ends before it starts"),
            Error::PartitionOutsideUsable { index } => {
                write!(f, "partition {index} is outside the usable range")
            }
            Error::PartitionsOverlap { first, second } => {
                write!(f, "partitions {first} and {second} overlap")
            }
            Error::Backup(m) => write!(f, "the backup header disagrees about {m}"),
            Error::MasterBootRecord(p) => write!(f, "the protective MBR is wrong: {p}"),
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

impl core::error::Error for Error {}

impl core::fmt::Display for BackupMismatch {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            BackupMismatch::MyLogicalBlockAddress => "its MyLBA field",
            BackupMismatch::AlternateLogicalBlockAddress => "its AlternateLBA field",
            BackupMismatch::DiskGuid => "the disk GUID",
            BackupMismatch::FirstUsableLogicalBlockAddress => "FirstUsableLBA",
            BackupMismatch::LastUsableLogicalBlockAddress => "LastUsableLBA",
            BackupMismatch::EntryCount => "NumberOfPartitionEntries",
            BackupMismatch::EntryBytes => "SizeOfPartitionEntry",
            BackupMismatch::EntryArrayCrc => "the entry array's CRC",
            BackupMismatch::EntryArrayLogicalBlockAddress => "where its entry array lives",
        })
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
            Error::EntryBytes(7),
            Error::EntryCount(0),
            Error::EntryArrayLen { need: 2, have: 1 },
            Error::EntryArrayShape { have: 3 },
            Error::EntryArrayCrc {
                stored: 1,
                computed: 2,
            },
            Error::NotPrimary {
                my_logical_block_address: 9,
            },
            Error::NotBackup { claimed_block: 9 },
            Error::DiskSize {
                header: 8,
                device: 9,
            },
            Error::UsableRange { first: 5, last: 4 },
            Error::TableOverlapsUsable,
            Error::PartitionRange { index: 1 },
            Error::PartitionOutsideUsable { index: 1 },
            Error::PartitionsOverlap {
                first: 0,
                second: 1,
            },
            Error::Backup(BackupMismatch::DiskGuid),
            Error::MasterBootRecord(MasterBootRecordProblem::Signature),
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
