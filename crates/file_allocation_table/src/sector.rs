//! **The sector interface**: the one thing a caller implements to let this crate read a volume.
//!
//! The crate never names a device. A caller wraps whatever holds the bytes (a block driver, a
//! file holding a disk image, a slice in memory) in [`SectorDevice`], and every read the crate
//! makes goes through it, one whole sector at a time.
//!
//! Name: provisional.

/// **The size of one sector**, in the four sizes the FAT specification permits.
///
/// A device states its own; the boot sector states the volume's in `BPB_BytsPerSec`. A volume
/// whose size differs from its device's is refused rather than read at the wrong offsets.
///
/// Name: provisional.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SectorSize {
    /// 512 bytes, which nearly every disk and image reports.
    Bytes512,
    /// 1024 bytes.
    Bytes1024,
    /// 2048 bytes, the sector of optical media.
    Bytes2048,
    /// 4096 bytes, the sector of a 4K-native disk.
    Bytes4096,
}

impl SectorSize {
    /// The largest size, and so the largest buffer this crate ever passes to a device.
    ///
    /// Name: provisional.
    pub const MAX: usize = 4096;

    /// The size for `bytes`, or `None` when the specification does not permit it.
    ///
    /// Name: provisional.
    pub const fn from_bytes(bytes: u16) -> Option<SectorSize> {
        match bytes {
            512 => Some(SectorSize::Bytes512),
            1024 => Some(SectorSize::Bytes1024),
            2048 => Some(SectorSize::Bytes2048),
            4096 => Some(SectorSize::Bytes4096),
            _ => None,
        }
    }

    /// The size in bytes.
    ///
    /// Name: provisional.
    pub const fn bytes(self) -> usize {
        match self {
            SectorSize::Bytes512 => 512,
            SectorSize::Bytes1024 => 1024,
            SectorSize::Bytes2048 => 2048,
            SectorSize::Bytes4096 => 4096,
        }
    }

    /// The base-two logarithm of the size, so that dividing by it is a shift.
    ///
    /// Name: provisional.
    pub const fn shift(self) -> u32 {
        match self {
            SectorSize::Bytes512 => 9,
            SectorSize::Bytes1024 => 10,
            SectorSize::Bytes2048 => 11,
            SectorSize::Bytes4096 => 12,
        }
    }
}

/// **Anything that reads whole sectors by index**: what a caller implements so this crate can read
/// a FAT volume.
///
/// Sector 0 is the volume's boot sector. A volume inside a partition is read through an
/// implementation that adds the partition's first sector to every index; this trait knows nothing
/// of partitions.
///
/// # Examples
///
/// A disk image held in memory:
///
/// ```
/// use file_allocation_table::{SectorDevice, SectorSize};
///
/// struct Image<'a>(&'a [u8]);
///
/// impl SectorDevice for Image<'_> {
///     type Error = ();
///
///     fn sector_size(&self) -> SectorSize {
///         SectorSize::Bytes512
///     }
///
///     fn read_sector(&mut self, index: u64, buffer: &mut [u8]) -> Result<(), ()> {
///         let start = usize::try_from(index).map_err(|_| ())?.checked_mul(512).ok_or(())?;
///         let bytes = self.0.get(start..start + buffer.len()).ok_or(())?;
///         buffer.copy_from_slice(bytes);
///         Ok(())
///     }
/// }
/// ```
///
/// Name: provisional.
pub trait SectorDevice {
    /// Why a read failed. The crate hands it back to the caller unchanged.
    type Error;

    /// The device's sector size. The crate reads it once, before it reads the boot sector, and
    /// expects it not to change.
    fn sector_size(&self) -> SectorSize;

    /// Fill `buffer` with sector `index`.
    ///
    /// `buffer` is always exactly [`SectorDevice::sector_size`] bytes. An index past the end of
    /// the device is an error the implementation reports; the crate does not know the device's
    /// length, and a hostile boot sector can name any index.
    fn read_sector(&mut self, index: u64, buffer: &mut [u8]) -> Result<(), Self::Error>;
}

impl<D: SectorDevice + ?Sized> SectorDevice for &mut D {
    type Error = D::Error;

    fn sector_size(&self) -> SectorSize {
        (**self).sector_size()
    }

    fn read_sector(&mut self, index: u64, buffer: &mut [u8]) -> Result<(), Self::Error> {
        (**self).read_sector(index, buffer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_permitted_size_round_trips_and_nothing_else_parses() {
        for size in [
            SectorSize::Bytes512,
            SectorSize::Bytes1024,
            SectorSize::Bytes2048,
            SectorSize::Bytes4096,
        ] {
            assert_eq!(SectorSize::from_bytes(size.bytes() as u16), Some(size));
            assert_eq!(1usize << size.shift(), size.bytes());
            assert!(size.bytes() <= SectorSize::MAX);
        }
        for bytes in [0u16, 256, 511, 513, 768, 8192, u16::MAX] {
            assert_eq!(SectorSize::from_bytes(bytes), None);
        }
    }
}
