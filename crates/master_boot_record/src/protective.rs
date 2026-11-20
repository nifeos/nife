//! **The protective MBR**: the one record a `GPT` disk carries at sector 0, of type `0xEE` and
//! covering the whole disk, so a tool that predates `GPT` sees a disk entirely spoken for and does
//! not offer to initialize it.
//!
//! Only the two fields that do the protecting are checked: the record starts at sector 1 and covers
//! the disk. The cylinder-head-sector fields are vestigial: `sgdisk` writes the real cylinder, head
//! and sector of the last block, macOS writes `FE FF FF`, and both are correct because nothing
//! reads them.
//!
//! The functions take the first 512 bytes of the disk's first block. A disk whose blocks are larger
//! keeps the record at the same offsets, and the rest of that block is the caller's.
//!
//! Name: provisional, minted 2026-11-20 (UTC) by lane/1093-partition-table-before-the-move for the
//! code's move out of `globally_unique_identifier_partition_table`.

/// The operating system type byte that makes a record protective. Not a filesystem, a keep-out
/// sign. The Unified Extensible Firmware Interface (UEFI) calls the field `OSType` in its legacy MBR
/// partition record, and `0xEE` is the value it reserves for a protective MBR.
///
/// Name: ratified 2026-10-27 (#2797), in `globally_unique_identifier_partition_table`.
#[doc(alias = "OSType")]
pub const PROTECTIVE_OPERATING_SYSTEM_TYPE: u8 = 0xEE;

/// Where the four 16-byte partition records start.
const RECORDS: usize = 446;

/// Where the `0x55AA` signature sits, in every MBR since 1983.
const SIGNATURE: usize = 510;

/// Field offsets inside one 16-byte record. `OPERATING_SYSTEM_TYPE` is UEFI's `OSType` field.
#[doc(alias = "OSType")]
const OPERATING_SYSTEM_TYPE: usize = 4;
/// Name: provisional.
#[doc(alias = "StartingLBA")]
const START_LOGICAL_BLOCK_ADDRESS: usize = 8;
/// Name: provisional.
#[doc(alias = "SizeInLBA")]
const SIZE_BLOCKS: usize = 12;

/// What is wrong with a protective MBR.
///
/// Name: provisional, minted 2026-11-20 (UTC) by lane/1093-partition-table-before-the-move;
/// `globally_unique_identifier_partition_table` names it `MasterBootRecordProblem`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[non_exhaustive]
pub enum Problem {
    /// The `0x55AA` at offset 510 is missing. Not an MBR at all.
    Signature,
    /// No partition record of type `0xEE`. A disk with a real MBR partition table and no `GPT`.
    NoProtectiveRecord,
    /// A second non-empty record beside the protective one: a **hybrid MBR**, where the same disk
    /// is described twice and the two descriptions can disagree. Refused deliberately; see
    /// [`validate`].
    ExtraRecord {
        /// The zero-based index of the extra record among the four.
        index: usize,
    },
    /// The protective record does not start at sector 1, so it does not cover the `GPT` it
    /// protects.
    ///
    /// Name: provisional.
    #[doc(alias = "StartingLBA")]
    StartLogicalBlockAddress(u32),
    /// The protective record's size does not cover the disk. Its whole job is to make a tool that
    /// does not know `GPT` see a full disk.
    ///
    /// Name: provisional.
    #[doc(alias = "SizeInLBA")]
    SizeBlocks(u32),
}

impl core::fmt::Display for Problem {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Problem::Signature => f.write_str("the 0x55AA signature is missing"),
            Problem::NoProtectiveRecord => f.write_str("no record has the protective type 0xEE"),
            Problem::ExtraRecord { index } => {
                write!(f, "record {index} is a second partition: a hybrid MBR")
            }
            Problem::StartLogicalBlockAddress(n) => {
                write!(f, "the protective record starts at LBA {n}, not 1")
            }
            Problem::SizeBlocks(n) => {
                write!(f, "the protective record covers {n} blocks, not the disk")
            }
        }
    }
}

impl core::error::Error for Problem {}

/// Write a protective MBR for a disk of `block_count` blocks into the first 512 bytes of its first
/// block.
///
/// The size field is 32 bits, so on a disk larger than 2 TiB (at 512-byte blocks) it cannot hold
/// the truth and the convention is `0xFFFFFFFF`. That is the one place this format admits it is a
/// fiction.
///
/// ```
/// use master_boot_record::protective;
///
/// let mut sector = [0u8; 512];
/// protective::write(&mut sector, 1 << 20);
/// assert_eq!(protective::validate(&sector, 1 << 20), Ok(()));
/// ```
///
/// Name: provisional.
pub fn write(sector: &mut [u8; 512], block_count: u64) {
    sector.fill(0);
    let r = &mut sector[RECORDS..RECORDS + 16];
    r[OPERATING_SYSTEM_TYPE] = PROTECTIVE_OPERATING_SYSTEM_TYPE;
    // The cylinder-head-sector fields stay at the maximum this crate can express: 0x000200 is
    // "cylinder 0, head 0, sector 2", which is sector 1, and 0xFFFFFF is the standard "beyond
    // cylinder-head-sector" marker.
    r[1..4].copy_from_slice(&[0x00, 0x02, 0x00]);
    r[5..8].copy_from_slice(&[0xFF, 0xFF, 0xFF]);
    r[START_LOGICAL_BLOCK_ADDRESS..START_LOGICAL_BLOCK_ADDRESS + 4]
        .copy_from_slice(&1u32.to_le_bytes());
    r[SIZE_BLOCKS..SIZE_BLOCKS + 4].copy_from_slice(&protective_size(block_count).to_le_bytes());
    sector[SIGNATURE] = 0x55;
    sector[SIGNATURE + 1] = 0xAA;
}

/// Check that the first 512 bytes of a disk's first block are a protective MBR for a disk of
/// `block_count` blocks.
///
/// A **hybrid MBR**, where records 2 to 4 also describe partitions so that a system that does not
/// know `GPT` can boot from them, is rejected ([`Problem::ExtraRecord`]), except that a record with
/// a size of zero may be ignored by a reader (UEFI 2.10 section 5.2.1) and is not counted. A hybrid
/// MBR is a real thing people do (Boot Camp made a generation of them) and it is also the format's
/// worst failure mode: two descriptions of one disk, maintained by two different tools, silently
/// diverging. Refusing it is a decision rather than an omission, and the error names it so the
/// message can say so.
///
/// # Errors
///
/// The first [`Problem`] found, in the order the variants are declared.
///
/// Name: provisional.
pub fn validate(sector: &[u8; 512], block_count: u64) -> Result<(), Problem> {
    if sector[SIGNATURE] != 0x55 || sector[SIGNATURE + 1] != 0xAA {
        return Err(Problem::Signature);
    }

    // Find the protective record first, then complain about anything beside it. Done in that
    // order because the two failures mean different things and the message has to be the right
    // one: a disk with a real MBR partition table and no 0xEE record is "not a GPT disk", while
    // a 0xEE record with company is a hybrid MBR.
    let operating_system_type = |i: usize| sector[RECORDS + i * 16 + OPERATING_SYSTEM_TYPE];
    let word = |at: usize| {
        u32::from_le_bytes([sector[at], sector[at + 1], sector[at + 2], sector[at + 3]])
    };
    let Some(i) = (0..4).find(|&i| operating_system_type(i) == PROTECTIVE_OPERATING_SYSTEM_TYPE)
    else {
        return Err(Problem::NoProtectiveRecord);
    };
    // A record whose size is zero may be ignored (UEFI 2.10 section 5.2.1), so a typed record with
    // no blocks is noise and not a second description of the disk.
    let size_of = |j: usize| word(RECORDS + j * 16 + SIZE_BLOCKS);
    if let Some(extra) =
        (0..4).find(|&j| j != i && operating_system_type(j) != 0x00 && size_of(j) != 0)
    {
        return Err(Problem::ExtraRecord { index: extra });
    }

    let start = word(RECORDS + i * 16 + START_LOGICAL_BLOCK_ADDRESS);
    if start != 1 {
        return Err(Problem::StartLogicalBlockAddress(start));
    }
    let size = size_of(i);
    // Either the exact remainder of the disk or the saturated marker. Both are in the wild:
    // sgdisk and macOS both write the exact value on a small disk, and everybody writes
    // 0xFFFFFFFF once the disk is too big to describe.
    if size != protective_size(block_count) && size != u32::MAX {
        return Err(Problem::SizeBlocks(size));
    }
    Ok(())
}

/// Every block but the first, saturated to what 32 bits can hold.
fn protective_size(block_count: u64) -> u32 {
    u32::try_from(block_count.saturating_sub(1)).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_written_record_validates_and_parses_as_protective() {
        for blocks in [2, 1 << 20, u64::from(u32::MAX) + 1, u64::MAX] {
            let mut sector = [0xA5u8; 512];
            write(&mut sector, blocks);
            assert_eq!(validate(&sector, blocks), Ok(()));
            let parsed = crate::MasterBootRecord::parse(&sector).unwrap();
            assert_eq!(parsed.entries[0].kind, PROTECTIVE_OPERATING_SYSTEM_TYPE);
            assert_eq!(parsed.entries[0].start, 1);
            assert_eq!(parsed.entries[0].sectors, protective_size(blocks));
            assert_eq!(parsed.scratch_sector_range(8), None);
        }
    }

    #[test]
    fn each_problem_is_named() {
        let mut good = [0u8; 512];
        write(&mut good, 100);

        let mut s = good;
        s[511] = 0;
        assert_eq!(validate(&s, 100), Err(Problem::Signature));

        let mut s = good;
        s[RECORDS + OPERATING_SYSTEM_TYPE] = 0x83;
        assert_eq!(validate(&s, 100), Err(Problem::NoProtectiveRecord));

        let mut s = good;
        s[RECORDS + 16 + OPERATING_SYSTEM_TYPE] = 0x0b;
        assert_eq!(
            validate(&s, 100),
            Ok(()),
            "a typed record with no size is ignored"
        );
        s[RECORDS + 16 + SIZE_BLOCKS] = 1;
        assert_eq!(validate(&s, 100), Err(Problem::ExtraRecord { index: 1 }));

        let mut s = good;
        s[RECORDS + START_LOGICAL_BLOCK_ADDRESS] = 2;
        assert_eq!(validate(&s, 100), Err(Problem::StartLogicalBlockAddress(2)));

        assert_eq!(validate(&good, 101), Err(Problem::SizeBlocks(99)));
        let mut s = good;
        s[RECORDS + SIZE_BLOCKS..RECORDS + SIZE_BLOCKS + 4].copy_from_slice(&[0xFF; 4]);
        assert_eq!(
            validate(&s, 101),
            Ok(()),
            "the saturated size covers any disk"
        );
    }
}
