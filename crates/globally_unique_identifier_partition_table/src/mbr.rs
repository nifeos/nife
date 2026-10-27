use crate::{Error, MbrProblem, is_block_size_ok};

/// The OS type byte that makes a record protective. Not a filesystem, a keep-out sign.
pub const PROTECTIVE_OS_TYPE: u8 = 0xEE;

/// Where the four 16-byte partition records start.
const RECORDS: usize = 446;

/// Where the `0x55AA` signature sits, in every MBR since 1983.
const SIGNATURE: usize = 510;

/// Field offsets inside one 16-byte record.
const OS_TYPE: usize = 4;
const START_LBA: usize = 8;
const SIZE_LBA: usize = 12;

/// Write a protective MBR for a disk of `block_count` blocks.
///
/// The size field is 32 bits, so on a disk larger than 2 TiB (at 512-byte blocks) it cannot
/// hold the truth and the convention is `0xFFFFFFFF`. That is the one place this format admits
/// it is a fiction.
pub fn write(block: &mut [u8], block_count: u64) -> Result<(), Error> {
    if !is_block_size_ok(block.len()) {
        return Err(Error::BlockSize(block.len()));
    }
    block.fill(0);
    let r = &mut block[RECORDS..RECORDS + 16];
    r[OS_TYPE] = PROTECTIVE_OS_TYPE;
    // The CHS fields stay at the maximum this crate can express: 0x000200 is "cylinder 0,
    // head 0, sector 2", which is LBA 1, and 0xFFFFFF is the standard "beyond CHS" marker.
    r[1..4].copy_from_slice(&[0x00, 0x02, 0x00]);
    r[5..8].copy_from_slice(&[0xFF, 0xFF, 0xFF]);
    r[START_LBA..START_LBA + 4].copy_from_slice(&1u32.to_le_bytes());
    r[SIZE_LBA..SIZE_LBA + 4].copy_from_slice(&protective_size(block_count).to_le_bytes());
    block[SIGNATURE] = 0x55;
    block[SIGNATURE + 1] = 0xAA;
    Ok(())
}

/// Check the block at LBA 0 is a protective MBR for a disk of `block_count` blocks.
///
/// A **hybrid MBR**, where records 2 to 4 also describe partitions so that a GPT-unaware system
/// can boot from them, is rejected ([`MbrProblem::ExtraRecord`]). That is a real thing people
/// do (Boot Camp made a generation of them) and it is also the format's worst failure mode: two
/// descriptions of one disk, maintained by two different tools, silently diverging. Refusing it
/// is a decision rather than an omission, and the error names it so the message can say so.
pub fn validate(block: &[u8], block_count: u64) -> Result<(), Error> {
    if !is_block_size_ok(block.len()) {
        return Err(Error::BlockSize(block.len()));
    }
    if block[SIGNATURE] != 0x55 || block[SIGNATURE + 1] != 0xAA {
        return Err(Error::Mbr(MbrProblem::Signature));
    }

    // Find the protective record first, then complain about anything beside it. Done in that
    // order because the two failures mean different things and the message has to be the right
    // one: a disk with a real MBR partition table and no 0xEE record is "not a GPT disk", while
    // a 0xEE record with company is a hybrid MBR.
    let os_type = |i: usize| block[RECORDS + i * 16 + OS_TYPE];
    let Some(i) = (0..4).find(|&i| os_type(i) == PROTECTIVE_OS_TYPE) else {
        return Err(Error::Mbr(MbrProblem::NoProtectiveRecord));
    };
    if let Some(extra) = (0..4).find(|&j| j != i && os_type(j) != 0x00) {
        return Err(Error::Mbr(MbrProblem::ExtraRecord { index: extra }));
    }
    let r = &block[RECORDS + i * 16..RECORDS + (i + 1) * 16];

    let start = u32::from_le_bytes(r[START_LBA..START_LBA + 4].try_into().unwrap());
    if start != 1 {
        return Err(Error::Mbr(MbrProblem::StartLba(start)));
    }
    let size = u32::from_le_bytes(r[SIZE_LBA..SIZE_LBA + 4].try_into().unwrap());
    // Either the exact remainder of the disk or the saturated marker. Both are in the wild:
    // sgdisk and macOS both write the exact value on a small disk, and everybody writes
    // 0xFFFFFFFF once the disk is too big to describe.
    if size != protective_size(block_count) && size != u32::MAX {
        return Err(Error::Mbr(MbrProblem::SizeLba(size)));
    }
    Ok(())
}

/// Every block but LBA 0, saturated to what 32 bits can hold.
fn protective_size(block_count: u64) -> u32 {
    u32::try_from(block_count.saturating_sub(1)).unwrap_or(u32::MAX)
}
