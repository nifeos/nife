//! **Linux: disks from sysfs, filesystems from udev, erasing with `sfdisk` and `mkfs.vfat`.**
//!
//! Every fact the offer rule needs is readable without root: `/sys/block` for the removable bit, an
//! SD card's `device/type` (its `removable` is 0 though the card comes out) and the USB path;
//! `/proc/self/mounts` for mounts; udev's `/run/udev/data` for an unmounted filesystem. Only
//! erasing needs privileges. This is the pure half: [`assemble`] takes the three roots as
//! arguments, so tests use a fake sysfs. `host/linux.rs` runs `sfdisk`, `mkfs.vfat` and `mount`.
//! See notes/boot-stick/the-program.md, "Linux".

//! # BUGS
//!
//! - **None of this has run on Linux**; the pure half is tested against a synthetic sysfs.
//! - **Free space is not known before copying**; a full volume fails the copy, keeping old files.
//! - **Erasing needs root, `sfdisk` and `mkfs.vfat`** (dosfstools); the program names the package.
//! - **Without udev** an unmounted FAT stick is offered for erasing; mounting it first avoids that.

use std::fs;
use std::path::{Path, PathBuf};

use crate::disk::{Disk, Filesystem, Volume};

fn read_trimmed(path: &Path) -> Option<String> {
    fs::read_to_string(path).ok().map(|s| s.trim().to_owned())
}

/// Block device names this program considers at all. Everything else (`ram`, `zram`, `dm-`, `md`,
/// `sr`, `nbd`) is not a disk anyone writes a boot stick to.
fn is_considered(name: &str) -> bool {
    ["sd", "mmcblk", "nvme", "vd", "xvd", "hd", "loop"]
        .iter()
        .any(|p| name.starts_with(p))
}

/// Unescape the octal escapes the kernel uses in `/proc/self/mounts` (`\040` for a space).
fn unescape_mount(field: &str) -> String {
    let bytes = field.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\'
            && i + 3 < bytes.len()
            && bytes[i + 1..i + 4]
                .iter()
                .all(|b| (b'0'..=b'7').contains(b))
        {
            let digit = |b: u8| u32::from(b - b'0');
            let value = digit(bytes[i + 1]) * 64 + digit(bytes[i + 2]) * 8 + digit(bytes[i + 3]);
            // The kernel only ever escapes single bytes, so a value past 255 is not one of its
            // escapes and is kept as written.
            let Ok(byte) = u8::try_from(value) else {
                out.push(bytes[i]);
                i += 1;
                continue;
            };
            out.push(byte);
            i += 4;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// `(mount point, filesystem type)` for a device, from the text of `/proc/self/mounts`.
pub fn mounted(mounts: &str, device: &str) -> Option<(PathBuf, String)> {
    mounts.lines().find_map(|line| {
        let mut fields = line.split_whitespace();
        let dev = fields.next()?;
        let mount = fields.next()?;
        let fs = fields.next()?;
        (dev == device).then(|| (PathBuf::from(unescape_mount(mount)), fs.to_owned()))
    })
}

/// udev's properties for a block device `major:minor`, as `(ID_FS_TYPE, ID_FS_LABEL)`.
fn udev_filesystem(udev: &Path, dev: &str) -> (Option<String>, Option<String>) {
    let Ok(text) = fs::read_to_string(udev.join(format!("b{dev}"))) else {
        return (None, None);
    };
    let property = |key: &str| {
        text.lines()
            .find_map(|l| l.strip_prefix(&format!("E:{key}=")))
            .map(str::to_owned)
    };
    (property("ID_FS_TYPE"), property("ID_FS_LABEL"))
}

fn filesystem_from(name: &str) -> Filesystem {
    match name {
        "vfat" | "msdos" | "fat" => Filesystem::Fat,
        "" => Filesystem::Unknown,
        other => Filesystem::Other(other.to_owned()),
    }
}

/// **Every considered block device, as a [`Disk`].** `sys_block` is `/sys/block`, `mounts` the text
/// of `/proc/self/mounts`, `udev` is `/run/udev/data`.
pub fn assemble(sys_block: &Path, mounts: &str, udev: &Path) -> Vec<Disk> {
    let Ok(entries) = fs::read_dir(sys_block) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| is_considered(n))
        .collect();
    names.sort();

    let mut disks = Vec::new();
    for name in names {
        let dir = sys_block.join(&name);
        let loop_device = name.starts_with("loop");
        if loop_device && !dir.join("loop/backing_file").exists() {
            continue; // an unattached loop device: nothing behind it
        }
        let canonical = fs::canonicalize(&dir).unwrap_or_else(|_| dir.clone());
        let usb = canonical.to_string_lossy().contains("/usb");
        let sd_card = read_trimmed(&dir.join("device/type")).as_deref() == Some("SD");
        let removable = read_trimmed(&dir.join("removable")).as_deref() == Some("1") || sd_card;
        let sectors: u64 = read_trimmed(&dir.join("size"))
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        let description = if loop_device {
            read_trimmed(&dir.join("loop/backing_file")).unwrap_or_default()
        } else {
            let vendor = read_trimmed(&dir.join("device/vendor")).unwrap_or_default();
            let model = read_trimmed(&dir.join("device/model"))
                .or_else(|| read_trimmed(&dir.join("device/name")))
                .unwrap_or_default();
            format!("{vendor} {model}").trim().to_owned()
        };
        let bus = if loop_device {
            "Loop"
        } else if sd_card {
            "Secure Digital"
        } else if usb {
            "USB"
        } else if name.starts_with("nvme") {
            "NVMe"
        } else {
            "Other"
        };

        let volumes = volumes_of(&dir, &name, mounts, udev);

        disks.push(Disk {
            id: name,
            description,
            size: sectors * 512,
            bus: bus.to_owned(),
            internal: !(usb || removable || loop_device),
            removable_media: removable,
            disk_image: loop_device,
            volumes,
        });
    }
    disks
}

/// The volumes on one block device: each partition (a subdirectory that carries a `partition`
/// file), or with none, the whole disk if it has a filesystem.
fn volumes_of(dir: &Path, name: &str, mounts: &str, udev: &Path) -> Vec<Volume> {
    // Partitions are the subdirectories that carry a `partition` file.
    let mut parts: Vec<String> = fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter(|e| e.path().join("partition").exists())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    parts.sort();
    let volume_names: Vec<String> = if parts.is_empty() {
        vec![name.to_owned()] // a whole-disk filesystem, if it has one
    } else {
        parts
    };
    let mut volumes = Vec::new();
    for vname in volume_names {
        let vdir = if vname == name {
            dir.to_path_buf()
        } else {
            dir.join(&vname)
        };
        let device = format!("/dev/{vname}");
        let dev = read_trimmed(&vdir.join("dev")).unwrap_or_default();
        let (udev_type, udev_label) = udev_filesystem(udev, &dev);
        let mount = mounted(mounts, &device);
        let fs_name = mount
            .as_ref()
            .map(|(_, fs)| fs.clone())
            .or(udev_type)
            .unwrap_or_default();
        if vname == name && fs_name.is_empty() && mount.is_none() {
            continue; // a whole disk with no filesystem on it is not a volume
        }
        let size = read_trimmed(&vdir.join("size"))
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0)
            * 512;
        volumes.push(Volume {
            id: vname,
            label: udev_label.unwrap_or_default(),
            filesystem: filesystem_from(&fs_name),
            mount: mount.map(|(m, _)| m),
            free: None,
            size,
        });
    }
    volumes
}

/// The device node of partition 1 on `disk`: `sdb1`, but `mmcblk0p1`, `nvme0n1p1`, `loop0p1`,
/// because a name ending in a digit takes a `p` first.
pub fn first_partition(disk: &str) -> String {
    if disk.ends_with(|c: char| c.is_ascii_digit()) {
        format!("{disk}p1")
    } else {
        format!("{disk}1")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::disk::{Plan, Policy, Refusal, plan, refusal};

    /// A fake `/sys/block` with the four shapes that matter: an internal NVMe, a USB hard disk
    /// (removable 0), a USB flash stick (removable 1), an SD card on the MMC bus, and a loop device.
    fn fake_machine(root: &Path) -> (PathBuf, String, PathBuf) {
        let _ = fs::remove_dir_all(root);
        let devices = root.join("devices");
        let block = root.join("block");
        let udev = root.join("udev");
        fs::create_dir_all(&block).unwrap();
        fs::create_dir_all(&udev).unwrap();

        let disk = |name: &str, under: &str, removable: &str, sectors: &str| {
            let dir = devices.join(under).join(name);
            fs::create_dir_all(dir.join("device")).unwrap();
            fs::write(dir.join("removable"), removable).unwrap();
            fs::write(dir.join("size"), sectors).unwrap();
            std::os::unix::fs::symlink(&dir, block.join(name)).unwrap();
            dir
        };
        let part = |dir: &Path, name: &str, dev: &str| {
            let p = dir.join(name);
            fs::create_dir_all(&p).unwrap();
            fs::write(p.join("partition"), "1").unwrap();
            fs::write(p.join("dev"), dev).unwrap();
            fs::write(p.join("size"), "1000").unwrap();
        };

        let nvme = disk(
            "nvme0n1",
            "pci0000:00/0000:00:1d.0/nvme/nvme0",
            "0\n",
            "1000215216\n",
        );
        part(&nvme, "nvme0n1p2", "259:2");

        let hdd = disk(
            "sda",
            "pci0000:00/usb2/2-1/host0/block",
            "0\n",
            "3906250000\n",
        );
        fs::write(hdd.join("device/vendor"), "Generic \n").unwrap();
        fs::write(hdd.join("device/model"), "USB Hard Disk   \n").unwrap();
        part(&hdd, "sda1", "8:1");

        let stick = disk(
            "sdb",
            "pci0000:00/usb3/3-2/host1/block",
            "1\n",
            "62500000\n",
        );
        fs::write(stick.join("device/vendor"), "Generic \n").unwrap();
        fs::write(stick.join("device/model"), "USB Flash       \n").unwrap();
        part(&stick, "sdb1", "8:17");
        fs::write(
            udev.join("b8:17"),
            "S:disk/by-label/NIFE\nE:ID_FS_TYPE=vfat\nE:ID_FS_LABEL=NIFE\n",
        )
        .unwrap();

        let card = disk(
            "mmcblk0",
            "platform/mmc0/mmc0:aaaa/block",
            "0\n",
            "15523840\n",
        );
        fs::write(card.join("device/type"), "SD\n").unwrap();
        fs::write(card.join("device/name"), "SD16G\n").unwrap();
        part(&card, "mmcblk0p1", "179:1");
        fs::write(udev.join("b179:1"), "E:ID_FS_TYPE=exfat\n").unwrap();

        let image = disk("loop0", "virtual/block", "0\n", "131072\n");
        fs::create_dir_all(image.join("loop")).unwrap();
        fs::write(image.join("loop/backing_file"), "/home/someone/stick.img\n").unwrap();
        let unattached = disk("loop1", "virtual/block", "0\n", "0\n");
        let _ = unattached;
        fs::create_dir_all(devices.join("virtual/block/ram0")).unwrap();
        std::os::unix::fs::symlink(devices.join("virtual/block/ram0"), block.join("ram0")).unwrap();

        let mounts = "/dev/nvme0n1p2 / ext4 rw 0 0\n\
                      /dev/sda1 /media/someone/My\\040Backups ext4 rw 0 0\n\
                      /dev/sdb1 /media/someone/NIFE vfat rw 0 0\n"
            .to_owned();
        (block, mounts, udev)
    }

    #[test]
    fn a_synthetic_linux_machine_offers_the_stick_and_the_card_and_nothing_else() {
        let root = std::env::temp_dir().join(format!("stick_maker-linux-{}", std::process::id()));
        let (block, mounts, udev) = fake_machine(&root);
        let disks = assemble(&block, &mounts, &udev);
        let ids: Vec<&str> = disks.iter().map(|d| d.id.as_str()).collect();
        assert_eq!(ids, ["loop0", "mmcblk0", "nvme0n1", "sda", "sdb"]);
        let by = |id: &str| disks.iter().find(|d| d.id == id).unwrap();

        assert_eq!(
            refusal(by("nvme0n1"), Policy::default()),
            Some(Refusal::Internal)
        );
        let backups = by("sda");
        assert_eq!(backups.bus, "USB");
        assert_eq!(
            refusal(backups, Policy::default()),
            Some(Refusal::FixedMedia)
        );
        assert_eq!(
            backups.volumes[0].mount.as_deref(),
            Some(Path::new("/media/someone/My Backups"))
        );

        let stick = by("sdb");
        assert_eq!(stick.description, "Generic USB Flash");
        assert_eq!(refusal(stick, Policy::default()), None);
        assert_eq!(stick.volumes[0].filesystem, Filesystem::Fat);
        assert_eq!(plan(stick, 1, |_| 0), Plan::Copy { volume: 0 });

        let card = by("mmcblk0");
        assert_eq!(card.bus, "Secure Digital");
        assert_eq!(refusal(card, Policy::default()), None);
        assert!(matches!(plan(card, 1, |_| 0), Plan::Erase { .. }), "exFAT");

        let image = by("loop0");
        assert_eq!(refusal(image, Policy::default()), Some(Refusal::DiskImage));
        assert_eq!(
            refusal(
                image,
                Policy {
                    include_disk_images: true
                }
            ),
            None
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn partition_names_follow_the_kernel_convention() {
        assert_eq!(first_partition("sdb"), "sdb1");
        assert_eq!(first_partition("mmcblk0"), "mmcblk0p1");
        assert_eq!(first_partition("nvme0n1"), "nvme0n1p1");
        assert_eq!(first_partition("loop3"), "loop3p1");
    }

    #[test]
    fn mount_points_with_spaces_are_unescaped() {
        assert_eq!(unescape_mount("/media/a\\040b"), "/media/a b");
        assert_eq!(unescape_mount("/plain"), "/plain");
        assert_eq!(unescape_mount("/tail\\04"), "/tail\\04");
    }

    // Milestone 326 (turn a mutation score upward), 2026-09-24: the properties below were each
    // unasserted, found by the 2026-09-21 census. notes/mutation-testing/stick-maker.md.

    /// **Every octal escape decodes by place value, and a value past a byte is kept as written.**
    /// `\040` alone cannot tell `4 * 8` from much else; `\134` (a backslash) and `\011` (a tab) use
    /// all three digits.
    #[test]
    fn octal_escapes_decode_by_place_value() {
        assert_eq!(unescape_mount("/a\\134b"), "/a\\b");
        assert_eq!(unescape_mount("/a\\011b"), "/a\tb");
        assert_eq!(unescape_mount("/a\\777b"), "/a\\777b");
    }

    /// **What a volume is, read from sysfs, udev and the mount table together.** A partition's
    /// filesystem and label come from udev when it is not mounted, and its size from its own
    /// directory; a whole disk is a volume only if something says it carries a filesystem (a
    /// "superfloppy" stick), and a bare whole disk contributes none, so the plan says "it has no
    /// volumes" rather than inventing an unknown one.
    #[test]
    fn volumes_come_from_partitions_or_from_a_whole_disk_filesystem() {
        let root =
            std::env::temp_dir().join(format!("stick_maker-linux-vol-{}", std::process::id()));
        let (block, mounts, udev) = fake_machine(&root);
        // A superfloppy: FAT on the whole of `sdc`, no partition table, not mounted.
        let sdc = root.join("devices/pci0000:00/usb3/3-3/host2/block/sdc");
        fs::create_dir_all(sdc.join("device")).unwrap();
        fs::write(sdc.join("removable"), "1\n").unwrap();
        fs::write(sdc.join("size"), "2048\n").unwrap();
        fs::write(sdc.join("dev"), "8:32\n").unwrap();
        std::os::unix::fs::symlink(&sdc, block.join("sdc")).unwrap();
        fs::write(
            udev.join("b8:32"),
            "E:ID_FS_TYPE=vfat\nE:ID_FS_LABEL=FLOPPY\n",
        )
        .unwrap();

        let disks = assemble(&block, &mounts, &udev);
        let by = |id: &str| disks.iter().find(|d| d.id == id).unwrap();

        let stick = by("sdb");
        assert_eq!(stick.size, 62_500_000 * 512, "sectors are 512 bytes");
        assert_eq!(stick.volumes.len(), 1);
        assert_eq!(stick.volumes[0].id, "sdb1");
        assert_eq!(stick.volumes[0].label, "NIFE", "from udev");
        assert_eq!(
            stick.volumes[0].size,
            1000 * 512,
            "the partition's own size"
        );

        let card = by("mmcblk0");
        assert_eq!(card.volumes.len(), 1, "unmounted, and still a volume");
        assert_eq!(
            card.volumes[0].filesystem,
            Filesystem::Other("exfat".into()),
            "udev says what an unmounted partition holds"
        );
        assert_eq!(card.volumes[0].mount, None);

        let floppy = by("sdc");
        assert_eq!(
            floppy.volumes.len(),
            1,
            "a whole-disk filesystem is a volume"
        );
        assert_eq!(floppy.volumes[0].id, "sdc");
        assert_eq!(floppy.volumes[0].filesystem, Filesystem::Fat);
        assert_eq!(floppy.volumes[0].label, "FLOPPY");
        assert_eq!(floppy.volumes[0].size, 2048 * 512);
        let Plan::Erase { because } = plan(floppy, 1, |_| 0) else {
            panic!("an unmounted FAT volume is not a copy target");
        };
        assert!(because.contains("not mounted"), "{because}");

        let image = by("loop0");
        assert!(image.volumes.is_empty(), "a bare whole disk is no volume");
        let Plan::Erase { because } = plan(image, 1, |_| 0) else {
            panic!("nothing to copy onto");
        };
        assert!(because.contains("no volumes"), "{because}");
        fs::remove_dir_all(root).unwrap();
    }
}
