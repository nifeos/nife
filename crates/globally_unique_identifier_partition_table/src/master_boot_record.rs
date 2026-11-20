//! The protective master boot record (MBR) that a GUID partition table disk carries at block 0.
//!
//! Name: provisional. Spelled out, with the alias `MBR` for the specification's spelling.

/// The operating system type byte that makes a record protective, `0xEE`.
///
/// Name: ratified 2026-10-27 (#2797).
#[doc(alias = "OSType")]
pub use ::master_boot_record::protective::PROTECTIVE_OPERATING_SYSTEM_TYPE;

#[cfg(doc)]
use crate::MasterBootRecordProblem;
use crate::{Error, is_block_size_ok};

/// Write a protective MBR for a disk of `block_count` blocks into `block`, the disk's first block,
/// and zero the rest of it. The record is [`::master_boot_record::protective::write`]'s.
///
/// The size field is 32 bits, so on a disk larger than 2 TiB (at 512-byte blocks) it cannot
/// hold the truth and the convention is `0xFFFFFFFF`. That is the one place this format admits
/// it is a fiction.
///
/// # Errors
///
/// [`Error::BlockSize`] when `block` is not a block this crate accepts.
///
/// Name: provisional.
pub fn write(block: &mut [u8], block_count: u64) -> Result<(), Error> {
    let sector = first_sector(block)?;
    ::master_boot_record::protective::write(sector, block_count);
    block[SECTOR_BYTES..].fill(0);
    Ok(())
}

/// Check the block at LBA 0 is a protective MBR for a disk of `block_count` blocks. The check is
/// [`::master_boot_record::protective::validate`]'s, and it refuses a **hybrid MBR**
/// ([`MasterBootRecordProblem::ExtraRecord`]): two descriptions of one disk, maintained by two
/// different tools, silently diverging.
///
/// # Errors
///
/// [`Error::BlockSize`] when `block` is not a block this crate accepts, and
/// [`Error::MasterBootRecord`] with the first problem the record has.
///
/// Name: provisional.
pub fn validate(block: &[u8], block_count: u64) -> Result<(), Error> {
    if !is_block_size_ok(block.len()) {
        return Err(Error::BlockSize(block.len()));
    }
    let sector: &[u8; SECTOR_BYTES] = block[..SECTOR_BYTES]
        .try_into()
        .map_err(|_| Error::BlockSize(block.len()))?;
    ::master_boot_record::protective::validate(sector, block_count).map_err(Error::MasterBootRecord)
}

/// The MBR's length: the first 512 bytes of the first block, whatever the block size.
const SECTOR_BYTES: usize = 512;

/// The first 512 bytes of an accepted block, or the block-size refusal.
fn first_sector(block: &mut [u8]) -> Result<&mut [u8; SECTOR_BYTES], Error> {
    let len = block.len();
    if !is_block_size_ok(len) {
        return Err(Error::BlockSize(len));
    }
    (&mut block[..SECTOR_BYTES])
        .try_into()
        .map_err(|_| Error::BlockSize(len))
}
