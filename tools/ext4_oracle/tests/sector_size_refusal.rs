//! A 1 KiB-block filesystem on a device with 4,096-byte logical sectors cannot be addressed, so the
//! mount must refuse it, and the error that reaches the caller must say why: the `BadSuperblock`
//! with its `device:filesystem_geometry` operation that `validate_filesystem_geometry` returns, not
//! a bare `Io`, which is the error a failed read gives. nifeos/rsext4 carries that fix (item 7 of
//! the upstream drafts). See nifeos/record:design/roadmap/0190-ext4-read-write.md.

use rsext4::error::Ext4Result;
use rsext4::*;

struct Memory {
    bytes: Vec<u8>,
    sector: u32,
}

impl BlockIo for Memory {
    fn read(&mut self, buffer: &mut [u8], sector: SectorId, count: u32) -> Ext4Result<()> {
        let start = sector.raw() as usize * self.sector as usize;
        let length = count as usize * self.sector as usize;
        buffer[..length].copy_from_slice(&self.bytes[start..start + length]);
        Ok(())
    }
    fn write(&mut self, _: &[u8], _: SectorId, _: u32) -> Ext4Result<()> {
        Err(Ext4Error::read_only())
    }
    fn flush(&mut self) -> Ext4Result<()> {
        Ok(())
    }
    fn geometry(&self) -> DeviceGeometry {
        DeviceGeometry::new(
            self.sector,
            (self.bytes.len() / self.sector as usize) as u64,
        )
    }
    fn capabilities(&self) -> DeviceCapabilities {
        DeviceCapabilities {
            read_only: true,
            ..DeviceCapabilities::default()
        }
    }
}

struct Fixed;
impl Clock for Fixed {
    fn now(&self) -> Ext4Result<Ext4Timestamp> {
        Ok(Ext4Timestamp::new(0, 0))
    }
}

/// A zeroed 1 MiB volume whose superblock says only "ext4, 1 KiB blocks": the magic at byte 56 of
/// the superblock and `s_log_block_size` 0. Nothing past the block-size check is reached.
fn one_kibibyte_block_volume(sector: u32) -> Memory {
    let mut bytes = vec![0u8; 1 << 20];
    bytes[1024 + 56..1024 + 58].copy_from_slice(&0xEF53u16.to_le_bytes());
    Memory { bytes, sector }
}

fn mount_error(sector: u32) -> Ext4Error {
    let services = MountServices::new(Fixed, (), NoopObserver);
    match Ext4::mount(
        one_kibibyte_block_volume(sector),
        services,
        MountOptions::read_only_no_journal_replay(),
    ) {
        Ok(_) => panic!("a zeroed volume mounted"),
        Err(e) => e,
    }
}

#[test]
fn block_size_below_the_sector_size_is_reported_as_the_geometry_refusal() {
    let error = mount_error(4096);
    assert_eq!(error.kind(), Ext4ErrorKind::BadSuperblock, "{error:?}");
    assert_eq!(
        error.context(),
        Some(rsext4::error::ErrorContext::Operation {
            op: "device:filesystem_geometry"
        }),
        "{error:?}"
    );
}

#[test]
fn the_same_volume_on_512_byte_sectors_gets_past_the_block_size_check() {
    let error = mount_error(512);
    assert_ne!(error.kind(), Ext4ErrorKind::Io, "{error:?}");
}
