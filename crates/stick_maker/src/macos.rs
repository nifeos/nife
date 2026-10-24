//! **macOS: disks from `diskutil`, erasing with `diskutil eraseDisk`.**
//!
//! `diskutil` is Apple's supported scripting interface to Disk Arbitration, in every install, so
//! this module reads its `-plist` output (`plist.rs` says why not the table). This file is the pure
//! half, [`assemble`], tested against captured documents; `host/macos.rs` runs the tool. See
//! notes/boot-stick/the-program.md, "macOS".

//! # BUGS
//!
//! - **The flash-stick fixture is synthesized**; its first line says so.
//! - **`diskutil eraseDisk` has run only on file-backed disks**, never on a physical stick.
//! - **Every stick `diskutil eraseDisk ... MBR` makes has MBR disk signature `0x00000000`**, so
//!   Linux gives each the `PARTUUID` `00000000-01`; see the note's GUID section.

use std::path::PathBuf;

use crate::disk::{Disk, Filesystem, Volume};
#[cfg(test)]
use crate::plist;
use crate::plist::Value;

/// **Turn `diskutil list -plist external` plus one `diskutil info -plist` per disk and volume into
/// [`Disk`]s.** `info` is the lookup, so tests pass captured documents and the real path runs the
/// tool.
///
/// Every whole disk in the listing becomes a `Disk`, offered or not: the offer rule is
/// [`crate::disk::refusal`]'s, and a disk this function silently dropped would be a disk nobody
/// could ever ask about.
pub fn assemble(list: &Value, info: &dyn Fn(&str) -> Option<Value>) -> Vec<Disk> {
    let mut disks = Vec::new();
    for entry in list.array("AllDisksAndPartitions") {
        let Some(id) = entry.string("DeviceIdentifier") else {
            continue;
        };
        let Some(whole) = info(id) else {
            continue;
        };
        let bus = whole.string("BusProtocol").unwrap_or("").to_owned();
        let media = whole.string("MediaName").unwrap_or("").trim();
        let registry = whole.string("IORegistryEntryName").unwrap_or("").trim();
        // `MediaName` is the model ("USB Flash"); the registry name carries the vendor ("Generic
        // USB Flash Media"). The longer of the two, without the " Media" suffix IOKit appends, reads
        // the way the stick's label does.
        let registry = registry.strip_suffix(" Media").unwrap_or(registry);
        let description = if registry.len() > media.len() {
            registry
        } else {
            media
        };

        let mut volumes = Vec::new();
        let partitions = entry.array("Partitions");
        if partitions.is_empty() {
            // An unpartitioned disk with a filesystem on the whole of it (a "superfloppy"), which
            // some cameras and old sticks use. APFS containers land here too and carry no
            // filesystem of their own; the volume info says which.
            if entry.get("MountPoint").is_some() || entry.get("VolumeName").is_some() {
                volumes.push(volume(id, entry, &whole));
            }
        } else {
            for partition in partitions {
                let Some(part_id) = partition.string("DeviceIdentifier") else {
                    continue;
                };
                let detail = info(part_id).unwrap_or_else(|| partition.clone());
                volumes.push(volume(part_id, partition, &detail));
            }
        }

        disks.push(Disk {
            id: id.to_owned(),
            description: description.to_owned(),
            size: whole
                .unsigned("TotalSize")
                .or_else(|| whole.unsigned("Size"))
                .or_else(|| entry.unsigned("Size"))
                .unwrap_or(0),
            // `Internal` absent is treated as internal: a document this reader does not
            // understand is a disk it will not offer.
            internal: whole.boolean("Internal").unwrap_or(true),
            removable_media: whole.boolean("RemovableMedia").unwrap_or(false),
            disk_image: bus == "Disk Image",
            bus,
            volumes,
        });
    }
    disks
}

fn volume(id: &str, listed: &Value, detail: &Value) -> Volume {
    let fs_type = detail.string("FilesystemType").unwrap_or("");
    let fs_name = detail.string("FilesystemName").unwrap_or("");
    let content = listed.string("Content").unwrap_or("");
    let filesystem = if fs_type == "msdos" {
        Filesystem::Fat
    } else if !fs_name.is_empty() {
        Filesystem::Other(fs_name.to_owned())
    } else if !fs_type.is_empty() {
        Filesystem::Other(fs_type.to_owned())
    } else if content.starts_with("DOS_FAT") || content.starts_with("Windows_FAT") {
        // Listed as FAT by its partition type, with no mounted volume to ask.
        Filesystem::Fat
    } else {
        Filesystem::Unknown
    };
    let mount = detail
        .string("MountPoint")
        .or_else(|| listed.string("MountPoint"))
        .filter(|m| !m.is_empty())
        .map(PathBuf::from);
    Volume {
        id: id.to_owned(),
        label: detail
            .string("VolumeName")
            .or_else(|| listed.string("VolumeName"))
            .unwrap_or("")
            .to_owned(),
        filesystem,
        mount,
        free: detail.unsigned("FreeSpace"),
        size: detail
            .unsigned("Size")
            .or_else(|| listed.unsigned("Size"))
            .unwrap_or(0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::disk::{Plan, Policy, Refusal, plan, refusal};

    fn fixture(name: &str) -> Value {
        let text = match name {
            "list" => include_str!("../tests/fixtures/macos/list-external.plist"),
            "disk2" => include_str!("../tests/fixtures/macos/usb-portable-disk-info.plist"),
            "disk3" => include_str!("../tests/fixtures/macos/usb-apfs-container-info.plist"),
            "disk4" => include_str!("../tests/fixtures/macos/usb-hard-disk-info.plist"),
            "disk4s2" => include_str!("../tests/fixtures/macos/usb-hard-disk-volume-info.plist"),
            "disk5" => include_str!("../tests/fixtures/macos/disk-image-info.plist"),
            "disk5s1" => {
                include_str!("../tests/fixtures/macos/disk-image-fat32-volume-info.plist")
            }
            "disk0" => include_str!("../tests/fixtures/macos/internal-info.plist"),
            "disk6" => include_str!("../tests/fixtures/macos/usb-flash-stick-info.plist"),
            _ => return Value::Dict(Default::default()),
        };
        plist::parse(text).unwrap()
    }

    fn captured() -> Vec<Disk> {
        assemble(&fixture("list"), &|id| {
            let v = fixture(id);
            (v != Value::Dict(Default::default())).then_some(v)
        })
    }

    /// **The test this module exists to pass**: on a captured machine with two USB hard disks
    /// attached, the kind that hold backups, and a disk image, the default policy offers
    /// nothing, and asking for disk images offers the disk image and nothing else.
    #[test]
    fn on_the_captured_machine_only_the_disk_image_is_ever_offered() {
        let disks = captured();
        let ids: Vec<&str> = disks.iter().map(|d| d.id.as_str()).collect();
        assert_eq!(ids, ["disk2", "disk3", "disk4", "disk5"]);

        for disk in &disks {
            assert!(
                refusal(disk, Policy::default()).is_some(),
                "{} offered by default",
                disk.id
            );
        }
        let permissive = Policy {
            include_disk_images: true,
        };
        let offered: Vec<&str> = disks
            .iter()
            .filter(|d| refusal(d, permissive).is_none())
            .map(|d| d.id.as_str())
            .collect();
        assert_eq!(offered, ["disk5"]);

        let backups = disks.iter().find(|d| d.id == "disk4").unwrap();
        assert_eq!(refusal(backups, permissive), Some(Refusal::FixedMedia));
        assert_eq!(backups.bus, "USB");
        assert!(
            !backups.internal,
            "it IS external, which is why external is not the rule"
        );
    }

    #[test]
    fn the_disk_image_reads_as_a_mounted_fat32_volume_to_copy_onto() {
        let disks = captured();
        let image = disks.iter().find(|d| d.id == "disk5").unwrap();
        assert!(image.disk_image);
        assert_eq!(image.size, 67_108_864);
        assert_eq!(image.volumes.len(), 1);
        let v = &image.volumes[0];
        assert_eq!(v.filesystem, Filesystem::Fat);
        assert_eq!(
            v.mount.as_deref(),
            Some(std::path::Path::new("/Volumes/NIFE"))
        );
        assert_eq!(v.free, Some(66_010_112));
        assert_eq!(plan(image, 30_000_000, |_| 0), Plan::Copy { volume: 0 });
    }

    #[test]
    fn the_internal_disk_is_refused_by_its_own_document() {
        let internal = fixture("disk0");
        let list = plist::parse(
            "<plist><dict><key>AllDisksAndPartitions</key><array><dict>\
             <key>DeviceIdentifier</key><string>disk0</string></dict></array></dict></plist>",
        )
        .unwrap();
        let disks = assemble(&list, &|_| Some(internal.clone()));
        assert_eq!(
            refusal(
                &disks[0],
                Policy {
                    include_disk_images: true
                }
            ),
            Some(Refusal::Internal)
        );
    }

    #[test]
    fn a_flash_stick_is_offered_and_named_by_vendor_and_model() {
        let list = plist::parse(
            "<plist><dict><key>AllDisksAndPartitions</key><array><dict>\
             <key>DeviceIdentifier</key><string>disk6</string>\
             <key>Partitions</key><array><dict>\
             <key>Content</key><string>Windows_FAT_32</string>\
             <key>DeviceIdentifier</key><string>disk6s1</string>\
             <key>MountPoint</key><string>/Volumes/UNTITLED</string>\
             </dict></array></dict></array></dict></plist>",
        )
        .unwrap();
        let disks = assemble(&list, &|id| (id == "disk6").then(|| fixture("disk6")));
        let stick = &disks[0];
        assert_eq!(stick.description, "Generic USB Flash");
        assert_eq!(refusal(stick, Policy::default()), None);
        assert_eq!(stick.volumes[0].filesystem, Filesystem::Fat);
        assert_eq!(plan(stick, 1, |_| 0), Plan::Copy { volume: 0 });
    }

    #[test]
    fn a_document_missing_the_internal_key_is_not_offered() {
        let list = plist::parse(
            "<plist><dict><key>AllDisksAndPartitions</key><array><dict>\
             <key>DeviceIdentifier</key><string>disk7</string></dict></array></dict></plist>",
        )
        .unwrap();
        let bare =
            plist::parse("<plist><dict><key>RemovableMedia</key><true/></dict></plist>").unwrap();
        let disks = assemble(&list, &|_| Some(bare.clone()));
        assert_eq!(
            refusal(&disks[0], Policy::default()),
            Some(Refusal::Internal)
        );
    }

    // Milestone 326 (turn a mutation score upward), 2026-09-24.

    /// **An unmounted superfloppy still has its volume.** A FAT filesystem on the whole disk,
    /// listed with its `VolumeName` and no `MountPoint`, is a volume that is not mounted, so the
    /// plan says so; dropping it would say "it has no volumes" and send the person to erase.
    #[test]
    fn an_unmounted_whole_disk_filesystem_is_still_a_volume() {
        let list = plist::parse(
            "<plist><dict><key>AllDisksAndPartitions</key><array><dict>\
             <key>DeviceIdentifier</key><string>disk6</string>\
             <key>Content</key><string>DOS_FAT_32</string>\
             <key>VolumeName</key><string>NIFE</string>\
             </dict></array></dict></plist>",
        )
        .unwrap();
        let whole = fixture("disk6");
        let disks = assemble(&list, &|_| Some(whole.clone()));
        assert_eq!(disks[0].volumes.len(), 1);
        assert_eq!(disks[0].volumes[0].mount, None);
        let Plan::Erase { because } = plan(&disks[0], 1, |_| 0) else {
            panic!("an unmounted volume is not a copy target");
        };
        assert!(because.contains("not mounted"), "{because}");
    }

    /// **Of two names of the same length, the model name is kept**: `IORegistryEntryName` is used
    /// only when it says more, which is when it carries the vendor.
    #[test]
    fn the_registry_name_wins_only_when_it_is_longer() {
        let list = plist::parse(
            "<plist><dict><key>AllDisksAndPartitions</key><array><dict>\
             <key>DeviceIdentifier</key><string>disk7</string></dict></array></dict></plist>",
        )
        .unwrap();
        let whole = plist::parse(
            "<plist><dict><key>MediaName</key><string>Stick</string>\
             <key>IORegistryEntryName</key><string>Flash Media</string></dict></plist>",
        )
        .unwrap();
        let disks = assemble(&list, &|_| Some(whole.clone()));
        assert_eq!(disks[0].description, "Stick");
    }
}
