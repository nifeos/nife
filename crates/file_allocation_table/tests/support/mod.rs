//! What the writer's oracle tests share: a volume held in memory that reads and writes, volumes
//! rust-fatfs formats, and the host's `fsck`.
//!
//! Name: provisional.

#![allow(dead_code)]

use std::io::Cursor;
use std::path::Path;
use std::process::Command;

use file_allocation_table::{SectorDevice, SectorSize, WritableSectorDevice};

/// A volume held in memory, read and written one sector at a time.
pub struct Memory {
    pub bytes: Vec<u8>,
    pub size: SectorSize,
    /// Writes served so far.
    pub writes: usize,
}

impl SectorDevice for Memory {
    type Error = u64;

    fn sector_size(&self) -> SectorSize {
        self.size
    }

    fn read_sector(&mut self, index: u64, buffer: &mut [u8]) -> Result<(), u64> {
        let start = usize::try_from(index).map_err(|_| index)? * buffer.len();
        let bytes = self.bytes.get(start..start + buffer.len()).ok_or(index)?;
        buffer.copy_from_slice(bytes);
        Ok(())
    }
}

impl WritableSectorDevice for Memory {
    fn write_sector(&mut self, index: u64, buffer: &[u8]) -> Result<(), u64> {
        let start = usize::try_from(index).map_err(|_| index)? * buffer.len();
        let bytes = self
            .bytes
            .get_mut(start..start + buffer.len())
            .ok_or(index)?;
        bytes.copy_from_slice(buffer);
        self.writes += 1;
        Ok(())
    }
}

/// An empty volume of `mebibytes` that rust-fatfs formatted as `fat_type`.
pub fn formatted(mebibytes: usize, fat_type: lamfat::FatType, sector: u16) -> Memory {
    let mut image = vec![0u8; mebibytes << 20];
    lamfat::format_volume(
        &mut lamfat::StdIoWrapper::from(Cursor::new(&mut image)),
        lamfat::FormatVolumeOptions::new()
            .fat_type(fat_type)
            .bytes_per_sector(sector),
    )
    .expect("rust-fatfs formats the volume");
    Memory {
        bytes: image,
        size: SectorSize::from_bytes(sector).expect("a permitted sector size"),
        writes: 0,
    }
}

/// The `fsck` this host has, if any, and its name.
fn fsck() -> Option<&'static str> {
    [
        "fsck.fat",
        "/usr/sbin/fsck.fat",
        "/sbin/fsck.fat",
        "fsck_msdos",
    ]
    .into_iter()
    .find(|tool| Command::new(tool).arg("--help").output().is_ok())
}

/// Run the host's `fsck -n` on `image`, which must exit 0; report and skip when there is none.
pub fn check_with_fsck(image: &[u8], label: &str) {
    let Some(tool) = fsck() else {
        eprintln!("{label}: no fsck.fat or fsck_msdos on this host; the fsck check is skipped");
        return;
    };
    let path = std::env::temp_dir().join(format!(
        "file-allocation-table-{label}-{}.img",
        std::process::id()
    ));
    std::fs::write(&path, image).expect("write the image for fsck");
    let output = Command::new(tool)
        .arg("-n")
        .arg(&path)
        .output()
        .expect("run fsck");
    let _ = std::fs::remove_file(Path::new(&path));
    assert!(
        output.status.success(),
        "{label}: {tool} -n refused the image:\n{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// What rust-fatfs counts as free clusters on `image`.
pub fn oracle_free_clusters(image: &[u8]) -> u32 {
    let fs = lamfat::FileSystem::new(
        lamfat::StdIoWrapper::from(Cursor::new(image.to_vec())),
        lamfat::FsOptions::new(),
    )
    .expect("rust-fatfs mounts the volume");
    fs.stats().unwrap().free_clusters()
}
