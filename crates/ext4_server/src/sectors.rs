//! **Sectors over the block capability**: an ext4 implementation reads sectors, and the block
//! capability (`file_system_contract::block`) moves whole 4,096-byte blocks. [`Sectors`] serves the
//! first from the second, and never writes.
//!
//! Name: provisional, minted 2026-11-26 (UTC) by milestone/190-ext4-server-read-only.

use file_system_contract::block;

use crate::{ErrorNumber, INPUT_OUTPUT_ERROR};

/// The sector size the server reports to the ext4 implementation:
/// `file_system_contract::block::LOGICAL_BLOCK_BYTES`, because the block capability does not
/// carry the device's own. Every read reaches the device as whole blocks, so a read-only mount
/// is right whatever the device's sector is, and an ext4 block of 1,024 or 2,048 bytes mounts on
/// any device.
///
/// Name: provisional, minted 2026-11-26 (UTC) by milestone/190-ext4-server-read-only.
pub const SECTOR_BYTES: u32 = block::LOGICAL_BLOCK_BYTES as u32;

/// One block, in bytes.
const BLOCK: usize = block::BLOCK_BYTES;

/// **The block capability, as [`Sectors`] reaches it**: read one block, and the device's size.
/// The confined server's implementation is a `block::READ` request; a host test's is a file.
///
/// Name: provisional, minted 2026-11-26 (UTC) by milestone/190-ext4-server-read-only.
pub trait BlockDevice {
    /// Read block `index` (of [`file_system_contract::block::BLOCK_BYTES`] bytes) into `out`.
    fn read_block(&mut self, index: u64, out: &mut [u8; BLOCK]) -> Result<(), ErrorNumber>;

    /// The device's size in bytes, as `block::GET_SIZE_BYTES` answers.
    fn size_bytes(&self) -> u64;
}

/// **A block device read as sectors of one size**, with the last block read kept, since an ext4
/// implementation reads its metadata a sector or two at a time.
///
/// Name: provisional, minted 2026-11-26 (UTC) by milestone/190-ext4-server-read-only.
pub struct Sectors<D> {
    device: D,
    sector_bytes: u32,
    cached: Option<(u64, [u8; BLOCK])>,
}

impl<D: BlockDevice> Sectors<D> {
    /// Read `device` as sectors of `sector_bytes`, or `None` unless that is a power of two from
    /// 512 to one block.
    ///
    /// Name: provisional, minted 2026-11-26 (UTC) by milestone/190-ext4-server-read-only.
    pub fn new(device: D, sector_bytes: u32) -> Option<Self> {
        let valid = sector_bytes.is_power_of_two() && (512..=BLOCK as u32).contains(&sector_bytes);
        valid.then_some(Sectors {
            device,
            sector_bytes,
            cached: None,
        })
    }

    /// The sector size this reads in.
    ///
    /// Name: provisional, minted 2026-11-26 (UTC) by milestone/190-ext4-server-read-only.
    pub fn sector_bytes(&self) -> u32 {
        self.sector_bytes
    }

    /// How many whole sectors the device holds.
    ///
    /// Name: provisional, minted 2026-11-26 (UTC) by milestone/190-ext4-server-read-only.
    pub fn sectors(&self) -> u64 {
        self.device.size_bytes() / u64::from(self.sector_bytes)
    }

    /// Read `count` sectors from `first` into the start of `out`. `EIO` when `out` is too short
    /// or the range runs past the device, and the device's own error when a block read fails.
    ///
    /// Name: provisional, minted 2026-11-26 (UTC) by milestone/190-ext4-server-read-only.
    pub fn read(&mut self, first: u64, count: u32, out: &mut [u8]) -> Result<(), ErrorNumber> {
        let sector = u64::from(self.sector_bytes);
        let length = u64::from(count) * sector;
        let start = first.checked_mul(sector).ok_or(INPUT_OUTPUT_ERROR)?;
        let end = start.checked_add(length).ok_or(INPUT_OUTPUT_ERROR)?;
        if (out.len() as u64) < length || end > self.sectors() * sector {
            return Err(INPUT_OUTPUT_ERROR);
        }
        let mut at = start;
        let mut filled = 0;
        while at < end {
            let index = at / BLOCK as u64;
            let within = (at % BLOCK as u64) as usize;
            let take = (BLOCK - within).min((end - at) as usize);
            let data = self.block(index)?;
            out[filled..filled + take].copy_from_slice(&data[within..within + take]);
            filled += take;
            at += take as u64;
        }
        Ok(())
    }

    /// **Refused, always**: `EROFS`. Nothing reaches the device.
    ///
    /// Name: provisional, minted 2026-11-26 (UTC) by milestone/190-ext4-server-read-only.
    pub fn write(&mut self, _first: u64, _count: u32, _data: &[u8]) -> Result<(), ErrorNumber> {
        Err(file_system_contract::directory::EROFS)
    }

    /// The device, for a test that counts its reads.
    #[cfg(test)]
    pub(crate) fn device(&self) -> &D {
        &self.device
    }

    fn block(&mut self, index: u64) -> Result<&[u8; BLOCK], ErrorNumber> {
        if !matches!(self.cached, Some((at, _)) if at == index) {
            let mut data = [0u8; BLOCK];
            self.device.read_block(index, &mut data)?;
            self.cached = Some((index, data));
        }
        match &self.cached {
            Some((_, data)) => Ok(data),
            None => Err(INPUT_OUTPUT_ERROR),
        }
    }
}
