//! **A boot file as a whole boot disk**, the shape a cloud's custom-image import takes (milestone
//! 578 (an ACPI discovery path for aarch64), item 7). `cargo xtask acpi-boot` writes the aarch64
//! one beside the stick and boots it, so the file a person uploads is the file the gate booted.
//! `cargo xtask stick-boot` writes and boots the riscv64 one, whose `BOOTRISCV64.EFI` is the name
//! that needs `file_allocation_table`'s long-name entries, so EDK2 reads that run on every run of
//! the gate.
//!
//! ```text
//! cargo xtask acpi-boot                                   # writes target/cloud-image/nife-aarch64.img
//! cargo xtask stick-boot                                  # and target/cloud-image/nife-riscv64.img
//! qemu-img convert -f raw -O vmdk -o subformat=streamOptimized \
//!     target/cloud-image/nife-aarch64.img target/cloud-image/nife-aarch64.vmdk
//! ```
//!
//! The disk is a GPT with one partition, an EFI system partition holding the boot file at its
//! removable-media path (`\EFI\BOOT\BOOTAA64.EFI` on aarch64), which every UEFI firmware looks
//! for when it has no boot entry of its own, and a freshly imported disk never has one. The partition table and the FAT32 volume are laid
//! out by the same crates `components/src/system_installer.rs` writes a real disk with, so this is
//! a host-side caller of the installer's formats, not a second spelling of them. Nothing here reads
//! its own output back: EDK2 reading it in `acpi-boot` is the check, as OVMF is for the installer.
//!
//! The import procedure and what it costs are in `notes/rented-metal/oracle-a1.md`.
//!
//! # BUGS
//!
//! - Raw only. Conversion is `qemu-img`'s, run by the person importing, because which container a
//!   cloud wants (VMDK for Oracle, VHD for Azure, a tarball for Google) is that cloud's choice.
//! - The disk and partition GUIDs come from the host's `/dev/urandom`, so two runs on the same
//!   boot file write different bytes and the image is not reproducible.
//!
//! Name: provisional (milestone 578's lane, 2026-10-21 UTC), for the module and the path.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use file_allocation_table as fat;
use globally_unique_identifier_partition_table::entry::Entry;
use globally_unique_identifier_partition_table::guid::types;
use globally_unique_identifier_partition_table::{
    DEFAULT_ENTRY_COUNT, GloballyUniqueIdentifierPartitionTable, PRIMARY_HEADER_LBA, entry,
};
use uuid::Builder;

use crate::host::workspace_root;

/// The logical block this disk is written in. Every cloud boot volume this lane has read about
/// reports 512, and `file_allocation_table` writes nothing else.
const LBA: u64 = fat::SECTOR as u64;

/// Where the partition starts and how much the end keeps free: 1 MiB, the installer's alignment,
/// which also leaves the backup table's 33 blocks room at the far end.
const ALIGN: u64 = 2048;

/// **The EFI system partition, 300 MiB.** FAT32 needs 65525 data clusters, a little over 256 MiB
/// at 4096-byte clusters (`file_allocation_table::MIN_CLUSTERS`), and below that the firmware reads
/// the volume as FAT16 whatever its boot sector says. The image is sparse, so the size costs no
/// disk; a cloud grows the boot volume to its own minimum on import regardless.
const ESP_SECTORS: u64 = 300 * 1024 * 1024 / LBA;

/// Where `arch`'s disk is written.
pub(super) fn image_path(arch: &str) -> PathBuf {
    workspace_root().join(format!("target/cloud-image/nife-{arch}.img"))
}

/// **Write `arch`'s disk around `boot_file`**, replacing whatever was at [`image_path`]. The file
/// keeps its own name on the volume, so `boot_file` is named for the removable-media path.
pub(super) fn write(arch: &str, boot_file: &Path) -> Result<PathBuf, String> {
    let file = std::fs::read(boot_file)
        .map_err(|e| format!("cannot read {}: {e}", boot_file.display()))?;
    let name = boot_file
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| format!("{} has no file name", boot_file.display()))?;
    let random = random_bytes::<36>()?;
    let disk_guid =
        Builder::from_random_bytes(random[..16].try_into().expect("16 bytes")).into_uuid();
    let part_guid =
        Builder::from_random_bytes(random[16..32].try_into().expect("16 bytes")).into_uuid();
    let volume_id = u32::from_le_bytes(random[32..].try_into().expect("4 bytes"));

    let volume = fat::Volume::new(
        ESP_SECTORS,
        ALIGN,
        file.len() as u64,
        volume_id,
        name,
    )
    .map_err(|e| format!("the FAT32 volume: {e:?}"))?;
    let blocks = ALIGN + ESP_SECTORS + ALIGN;
    let esp = Entry::new(types::EFI_SYSTEM, part_guid, ALIGN, ALIGN + ESP_SECTORS - 1)
        .with_name("EFI system partition")
        .map_err(|e| format!("the partition entry: {e:?}"))?;
    let mut array = vec![0u8; DEFAULT_ENTRY_COUNT as usize * entry::SIZE];
    let table = GloballyUniqueIdentifierPartitionTable::create(
        disk_guid,
        LBA as usize,
        blocks,
        &[esp],
        &mut array,
    )
    .map_err(|e| format!("the partition table: {e:?}"))?;

    let path = image_path(arch);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    }
    let mut out =
        File::create(&path).map_err(|e| format!("cannot create {}: {e}", path.display()))?;
    // Sparse: only the tables, the volume's metadata and the boot file are ever written.
    out.set_len(blocks * LBA)
        .map_err(|e| format!("cannot size {}: {e}", path.display()))?;

    let mut block = [0u8; LBA as usize];
    table
        .write_protective_mbr(&mut block)
        .map_err(|e| format!("the protective MBR: {e:?}"))?;
    put(&mut out, 0, &block)?;
    block.fill(0);
    table
        .write_primary_header(&mut block)
        .map_err(|e| format!("the primary header: {e:?}"))?;
    put(&mut out, PRIMARY_HEADER_LBA, &block)?;
    put(&mut out, table.primary_entry_lba(), table.entry_array())?;
    put(&mut out, table.backup_entry_lba(), table.entry_array())?;
    block.fill(0);
    table
        .write_backup_header(&mut block)
        .map_err(|e| format!("the backup header: {e:?}"))?;
    put(&mut out, table.backup_header_lba(), &block)?;

    for sector in 0..volume.metadata_sectors() {
        if !volume.sector(sector, &mut block) {
            return Err(format!("the FAT32 volume has no sector {sector}"));
        }
        put(&mut out, ALIGN + sector, &block)?;
    }
    put(&mut out, ALIGN + volume.file_first_sector(), &file)?;
    out.sync_all()
        .map_err(|e| format!("cannot flush {}: {e}", path.display()))?;
    Ok(path)
}

/// Write `bytes` at logical block `lba`.
fn put(out: &mut File, lba: u64, bytes: &[u8]) -> Result<(), String> {
    out.seek(SeekFrom::Start(lba * LBA))
        .and_then(|_| out.write_all(bytes))
        .map_err(|e| format!("cannot write at LBA {lba}: {e}"))
}

fn random_bytes<const N: usize>() -> Result<[u8; N], String> {
    let mut bytes = [0u8; N];
    File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut bytes))
        .map_err(|e| format!("cannot read /dev/urandom: {e}"))?;
    Ok(bytes)
}
