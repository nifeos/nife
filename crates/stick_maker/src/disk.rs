//! **Which disks this program may touch, and what it would do to each.**
//!
//! Each host module turns its own description of its disks into a [`Disk`]; from there on all is
//! host-independent, so whether a disk may be written is one function, [`refusal`], tested once.
//!
//! A disk is offered only when its media is removable: the SCSI removable-media bit, read as
//! macOS's `RemovableMedia`, Linux's `/sys/block/<dev>/removable` (or an `SD` card type on the MMC
//! bus) and Windows' `DRIVE_REMOVABLE`. "External" or "USB" is not enough: a USB hard disk is both
//! and is not removable (`tests/fixtures/macos/usb-*-info.plist`). See notes/boot-stick.md,
//! "Which disks it will touch, and the measurement that decided it".

//! # BUGS
//!
//! - A flash stick sold as a "fixed disk" (some built for Windows To Go) clears the bit and is not
//!   offered.

use std::path::PathBuf;

/// What a volume is formatted as, as far as this program cares.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Filesystem {
    /// FAT12, FAT16 or FAT32: what UEFI firmware reads on removable media (UEFI 2.10, 13.3), and
    /// what U-Boot's `fatload` reads.
    Fat,
    /// Anything else, named as the host named it (exFAT, NTFS, APFS, HFS+, ext4, ...).
    Other(String),
    /// The host could not say.
    Unknown,
}

/// One volume (a partition with a filesystem, or a whole unpartitioned disk).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Volume {
    /// The host's name for it: `disk7s1`, `sdb1`, `E:`.
    pub id: String,
    /// Its label, if it has one.
    pub label: String,
    /// What it is formatted as.
    pub filesystem: Filesystem,
    /// Where it is mounted, if it is.
    pub mount: Option<PathBuf>,
    /// Free bytes, if the host said.
    pub free: Option<u64>,
    /// Its size in bytes.
    pub size: u64,
}

/// One whole disk, as the host described it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Disk {
    /// The host's short name: `disk7`, `sdb`, `E:`. This is what a person types to confirm an
    /// erase, so it is the name they would see in the host's own tools.
    pub id: String,
    /// A human description: the vendor and model where the host has them.
    pub description: String,
    /// Size in bytes.
    pub size: u64,
    /// The bus, as the host spelled it: `USB`, `Secure Digital`, `Disk Image`, `Apple Fabric`.
    pub bus: String,
    /// The host says this disk is inside the machine.
    pub internal: bool,
    /// The removable-media bit (see the module documentation).
    pub removable_media: bool,
    /// A disk backed by a file (`hdiutil attach`, a Linux loop device). Never a person's data disk;
    /// offered only when asked, because it is how this program is tested.
    pub disk_image: bool,
    /// Its volumes.
    pub volumes: Vec<Volume>,
}

/// What the caller has asked to be offered beyond the default.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Policy {
    /// Offer file-backed disks too (`--include-disk-images`), which is how every test here runs.
    pub include_disk_images: bool,
}

/// Why a disk is not offered. Each is a sentence the program prints under `--list --all`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// The host says it is inside the machine.
    Internal,
    /// Its media is not removable: a USB hard disk or SSD, most often somebody's backups.
    FixedMedia,
    /// A file-backed disk, and `--include-disk-images` was not given.
    DiskImage,
    /// One of its volumes is mounted where the running system lives.
    HoldsTheSystem,
}

impl Refusal {
    /// The sentence for a person.
    pub const fn reason(self) -> &'static str {
        match self {
            Refusal::Internal => "inside this computer",
            Refusal::FixedMedia => {
                "not removable media (a USB hard disk or SSD, which is where backups live)"
            }
            Refusal::DiskImage => "a disk image (offered only with --include-disk-images)",
            Refusal::HoldsTheSystem => "holds the running system",
        }
    }
}

/// **The one rule**: `None` means the disk may be offered.
///
/// The checks run in order of how certain they are, so the reason printed is the strongest one.
pub fn refusal(disk: &Disk, policy: Policy) -> Option<Refusal> {
    if disk.internal {
        return Some(Refusal::Internal);
    }
    if disk.volumes.iter().any(holds_the_system) {
        return Some(Refusal::HoldsTheSystem);
    }
    if disk.disk_image {
        return (!policy.include_disk_images).then_some(Refusal::DiskImage);
    }
    if !disk.removable_media {
        return Some(Refusal::FixedMedia);
    }
    None
}

/// A volume mounted where a running system keeps itself. Defence in depth: no removable disk
/// should hold one, and a live USB system booted from a stick is exactly the case where one does.
///
/// **Exact mount points for the directories a system is mounted AT, prefixes only for the two
/// trees whose every mount is the system's.** A volume mounted at `/home` holds everyone's home
/// directories; one mounted at `/home/calef/stick` is somebody's stick, and so is one under the
/// macOS temporary directory (`/var/folders/...`), which is where this crate's tests mount theirs.
fn holds_the_system(volume: &Volume) -> bool {
    let Some(mount) = &volume.mount else {
        return false;
    };
    let mount = mount.to_string_lossy();
    let mount = mount.trim_end_matches(['/', '\\']);
    let exact = [
        "",
        "/usr",
        "/var",
        "/home",
        "/private",
        "/private/var",
        "/nix",
    ];
    let trees = ["/boot", "/System/Volumes"];
    exact.contains(&mount)
        || trees
            .iter()
            .any(|t| mount == *t || mount.starts_with(&format!("{t}/")))
        || mount.eq_ignore_ascii_case("C:")
}

/// What the program would do to an offered disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Plan {
    /// **The common case**: a mounted FAT volume with room. Files are added under `EFI/BOOT/`,
    /// nothing is erased, and no administrator rights are needed.
    Copy {
        /// The volume, by index into [`Disk::volumes`].
        volume: usize,
    },
    /// No usable FAT volume: the disk has to be erased and formatted first.
    Erase {
        /// Why copying was not possible, for the confirmation prompt.
        because: String,
    },
}

/// Decide between copying and erasing.
///
/// `needed` is the bytes the payload set occupies, and `replaced` says how many of those bytes a
/// volume already holds under the same names (a stick being refreshed with a newer build), since
/// those are freed by the copy.
pub fn plan(disk: &Disk, needed: u64, replaced: impl Fn(&Volume) -> u64) -> Plan {
    let mut best: Option<(usize, u64)> = None;
    let mut unmounted_fat = false;
    let mut too_small = false;
    for (i, volume) in disk.volumes.iter().enumerate() {
        if volume.filesystem != Filesystem::Fat {
            continue;
        }
        if volume.mount.is_none() {
            unmounted_fat = true;
            continue;
        }
        let room = volume
            .free
            .unwrap_or(u64::MAX)
            .saturating_add(replaced(volume));
        if room < needed {
            too_small = true;
            continue;
        }
        if best.is_none_or(|(_, r)| room > r) {
            best = Some((i, room));
        }
    }
    if let Some((volume, _)) = best {
        return Plan::Copy { volume };
    }
    let because = if too_small {
        "its FAT volume does not have room for the boot files".to_owned()
    } else if unmounted_fat {
        "its FAT volume is not mounted".to_owned()
    } else if disk.volumes.is_empty() {
        "it has no volumes".to_owned()
    } else {
        let names: Vec<String> = disk
            .volumes
            .iter()
            .map(|v| match &v.filesystem {
                Filesystem::Other(name) => name.clone(),
                Filesystem::Unknown => "an unrecognised format".to_owned(),
                Filesystem::Fat => "FAT".to_owned(),
            })
            .collect();
        format!("it is formatted {}, not FAT", names.join(", "))
    };
    Plan::Erase { because }
}

/// A byte count the way a person reads one on a stick's label: decimal gigabytes, since that is
/// what the packaging says.
pub fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "kB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1000.0 && unit + 1 < UNITS.len() {
        value /= 1000.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stick() -> Disk {
        Disk {
            id: "disk8".into(),
            description: "SanDisk Ultra".into(),
            size: 30_752_636_928,
            bus: "USB".into(),
            internal: false,
            removable_media: true,
            disk_image: false,
            volumes: vec![Volume {
                id: "disk8s1".into(),
                label: "NIFE".into(),
                filesystem: Filesystem::Fat,
                mount: Some("/Volumes/NIFE".into()),
                free: Some(30_000_000_000),
                size: 30_752_636_928,
            }],
        }
    }

    #[test]
    fn a_flash_stick_is_offered_for_copying() {
        let disk = stick();
        assert_eq!(refusal(&disk, Policy::default()), None);
        assert_eq!(plan(&disk, 30_000_000, |_| 0), Plan::Copy { volume: 0 });
    }

    #[test]
    fn a_usb_hard_disk_is_never_offered() {
        let disk = Disk {
            removable_media: false,
            description: "WD My Book 1140".into(),
            ..stick()
        };
        assert_eq!(refusal(&disk, Policy::default()), Some(Refusal::FixedMedia));
        // Not even when disk images are asked for: that flag widens a different door.
        let permissive = Policy {
            include_disk_images: true,
        };
        assert_eq!(refusal(&disk, permissive), Some(Refusal::FixedMedia));
    }

    #[test]
    fn an_internal_disk_is_never_offered_even_if_it_claims_removable_media() {
        let disk = Disk {
            internal: true,
            ..stick()
        };
        assert_eq!(refusal(&disk, Policy::default()), Some(Refusal::Internal));
    }

    #[test]
    fn a_disk_holding_the_running_system_is_never_offered() {
        for mount in ["/", "/boot/efi", "/System/Volumes/Data", "C:\\", "/home"] {
            let mut disk = stick();
            disk.volumes[0].mount = Some(mount.into());
            assert_eq!(
                refusal(&disk, Policy::default()),
                Some(Refusal::HoldsTheSystem),
                "{mount}"
            );
        }
        for mount in [
            "/media/calef/NIFE",
            "/home/calef/stick",
            "/var/folders/xy/T/stick",
        ] {
            let mut disk = stick();
            disk.volumes[0].mount = Some(mount.into());
            assert_eq!(refusal(&disk, Policy::default()), None, "{mount}");
        }
    }

    #[test]
    fn a_disk_image_needs_asking_for() {
        let disk = Disk {
            disk_image: true,
            removable_media: false,
            bus: "Disk Image".into(),
            ..stick()
        };
        assert_eq!(refusal(&disk, Policy::default()), Some(Refusal::DiskImage));
        let asked = Policy {
            include_disk_images: true,
        };
        assert_eq!(refusal(&disk, asked), None);
    }

    #[test]
    fn exfat_means_erase_and_says_why() {
        let mut disk = stick();
        disk.volumes[0].filesystem = Filesystem::Other("ExFAT".into());
        let Plan::Erase { because } = plan(&disk, 1, |_| 0) else {
            panic!("an exFAT stick cannot be copied to");
        };
        assert!(because.contains("ExFAT"), "{because}");
    }

    #[test]
    fn a_full_fat_volume_means_erase_unless_the_files_being_replaced_make_room() {
        let mut disk = stick();
        disk.volumes[0].free = Some(1_000);
        assert!(matches!(plan(&disk, 5_000, |_| 0), Plan::Erase { .. }));
        assert_eq!(plan(&disk, 5_000, |_| 4_000), Plan::Copy { volume: 0 });
    }

    #[test]
    fn an_unmounted_fat_volume_is_not_a_copy_target() {
        let mut disk = stick();
        disk.volumes[0].mount = None;
        let Plan::Erase { because } = plan(&disk, 1, |_| 0) else {
            panic!("nothing to copy onto");
        };
        assert!(because.contains("not mounted"));
    }

    #[test]
    fn the_roomiest_fat_volume_wins() {
        let mut disk = stick();
        let mut second = disk.volumes[0].clone();
        second.id = "disk8s2".into();
        second.free = Some(40_000_000_000);
        disk.volumes.push(second);
        assert_eq!(plan(&disk, 1, |_| 0), Plan::Copy { volume: 1 });
    }

    #[test]
    fn sizes_read_like_the_label() {
        assert_eq!(human_size(512), "512 B");
        assert_eq!(human_size(30_752_636_928), "30.8 GB");
        assert_eq!(human_size(10_158_080), "10.2 MB");
    }

    // Milestone 326 (turn a mutation score upward), 2026-09-24.

    /// **Between two FAT volumes with the same room, the first is chosen**, so the same stick gets
    /// the same answer on every run rather than whichever the comparison happens to favour.
    #[test]
    fn equal_room_keeps_the_first_volume() {
        let mut disk = stick();
        let mut second = disk.volumes[0].clone();
        second.id = "disk8s2".into();
        disk.volumes.push(second);
        assert_eq!(plan(&disk, 1, |_| 0), Plan::Copy { volume: 0 });
    }

    /// **The largest unit is the last one**: a size past a thousand terabytes stays in terabytes
    /// rather than indexing past the table.
    #[test]
    fn sizes_past_the_last_unit_stay_in_it() {
        assert_eq!(human_size(999_000_000_000_000), "999.0 TB");
        assert_eq!(human_size(5_000_000_000_000_000), "5000.0 TB");
    }
}
