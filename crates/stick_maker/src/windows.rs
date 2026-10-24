//! **Windows: drives from the Win32 volume functions; erasing is described, not performed.**
//!
//! `GetDriveTypeW` answers `DRIVE_REMOVABLE` from the same removable-media bit the other hosts
//! read, and a USB hard disk answers `DRIVE_FIXED`, so `disk.rs`'s one rule refuses it. The four
//! `kernel32` functions are declared by hand rather than through a bindings crate (§46 (thin
//! primitives or whole subsystems)). This file is the pure half, [`disk_from`], tested on every
//! host; `host/windows.rs` calls it. See notes/boot-stick/the-program.md, "Windows".

//! # BUGS
//!
//! - **None of this has run on Windows**; the four calls into `kernel32` have not executed.
//! - **Erasing is not automated**, by choice: the `diskpart` steps are printed, because destructive
//!   code that has never run on its host must not ship. The note says what would promote it.
//! - **FAT32 through `format` stops at 32 GB**, so the printed steps create a 1 GB partition.
//! - **A drive letter is a volume, not a disk**, so a stick with two partitions appears twice.

use std::path::PathBuf;

use crate::disk::{Disk, Filesystem, Volume};

/// `GetDriveTypeW`'s answer for removable media.
pub const DRIVE_REMOVABLE: u32 = 2;
/// `GetDriveTypeW`'s answer for a fixed disk, which includes USB hard disks.
pub const DRIVE_FIXED: u32 = 3;

/// **One drive letter as a [`Disk`]**, from what the Win32 volume functions reported.
pub fn disk_from(
    letter: char,
    drive_type: u32,
    label: &str,
    filesystem: &str,
    total: u64,
    free: Option<u64>,
) -> Disk {
    let id = format!("{letter}:");
    let fs = match filesystem.to_ascii_uppercase().as_str() {
        "FAT" | "FAT32" | "FAT12" | "FAT16" => Filesystem::Fat,
        "" => Filesystem::Unknown,
        other => Filesystem::Other(other.to_owned()),
    };
    Disk {
        id: id.clone(),
        description: if label.is_empty() {
            "removable drive".to_owned()
        } else {
            label.to_owned()
        },
        size: total,
        bus: match drive_type {
            DRIVE_REMOVABLE => "Removable",
            DRIVE_FIXED => "Fixed",
            _ => "Other",
        }
        .to_owned(),
        // Windows cannot say "inside the machine" without a device query; a fixed drive is refused
        // as fixed media either way, which is the true reason.
        internal: false,
        removable_media: drive_type == DRIVE_REMOVABLE,
        disk_image: false,
        volumes: vec![Volume {
            id: id.clone(),
            label: label.to_owned(),
            filesystem: fs,
            mount: Some(PathBuf::from(format!("{letter}:\\"))),
            free,
            size: total,
        }],
    }
}

/// The `diskpart` steps for a person to run, since this program does not run them (see BUGS).
pub fn erase_instructions(disk: &Disk) -> String {
    format!(
        "Windows cannot erase {id} from this program yet. To prepare it by hand, as administrator:\n\
         \n\
         \x20 diskpart\n\
         \x20 list disk                  (find the disk whose size matches {size}; be certain)\n\
         \x20 select disk N\n\
         \x20 clean                      (this erases the whole disk)\n\
         \x20 create partition primary size=1024\n\
         \x20 format fs=fat32 quick label=NIFE\n\
         \x20 assign\n\
         \x20 exit\n\
         \n\
         Then run this program again: it will find the FAT32 drive and copy the boot files.",
        id = disk.id,
        size = crate::disk::human_size(disk.size),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::disk::{Plan, Policy, Refusal, plan, refusal};

    #[test]
    fn a_removable_fat32_drive_is_offered_for_copying() {
        let disk = disk_from(
            'E',
            DRIVE_REMOVABLE,
            "NIFE",
            "FAT32",
            31_000_000_000,
            Some(30_000_000_000),
        );
        assert_eq!(refusal(&disk, Policy::default()), None);
        assert_eq!(plan(&disk, 1, |_| 0), Plan::Copy { volume: 0 });
    }

    #[test]
    fn a_usb_hard_disk_and_the_system_drive_are_refused() {
        let backups = disk_from('F', DRIVE_FIXED, "Backups", "NTFS", 3_000_000_000_000, None);
        assert_eq!(
            refusal(&backups, Policy::default()),
            Some(Refusal::FixedMedia)
        );
        let system = disk_from('C', DRIVE_FIXED, "", "NTFS", 500_000_000_000, None);
        assert_eq!(
            refusal(&system, Policy::default()),
            Some(Refusal::HoldsTheSystem)
        );
    }

    #[test]
    fn an_exfat_stick_gets_instructions_rather_than_an_erase() {
        let disk = disk_from('E', DRIVE_REMOVABLE, "", "exFAT", 64_000_000_000, Some(1));
        assert!(matches!(plan(&disk, 1, |_| 0), Plan::Erase { .. }));
        let steps = erase_instructions(&disk);
        assert!(steps.contains("64.0 GB"));
        assert!(steps.contains("format fs=fat32 quick label=NIFE"));
    }

    /// **The bus a person is shown says which kind of drive Windows reported.** Milestone 326 (turn a mutation score upward),
    /// 2026-09-24: both arms could be deleted with every test green.
    #[test]
    fn the_drive_type_is_named() {
        let bus = |kind| disk_from('E', kind, "", "FAT32", 1, None).bus;
        assert_eq!(bus(DRIVE_REMOVABLE), "Removable");
        assert_eq!(bus(DRIVE_FIXED), "Fixed");
        assert_eq!(bus(0), "Other");
    }
}
