//! **macOS: running `diskutil`.** The pure half, which reads what it prints, is `crate::macos`.

use std::process::Command;

use crate::disk::Disk;
use crate::macos::assemble;
use crate::plist::{self, Value};

const DISKUTIL: &str = "/usr/sbin/diskutil";

fn diskutil_plist(args: &[&str]) -> Result<Value, String> {
    let output = Command::new(DISKUTIL)
        .args(args)
        .output()
        .map_err(|e| format!("cannot run {DISKUTIL}: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "diskutil {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    plist::parse(&text).map_err(|e| format!("diskutil {}: {e}", args.join(" ")))
}

/// **Every external disk this Mac can see**, offered or not.
///
/// `external` is `diskutil`'s own filter and leaves out the internal SSD before this program has
/// to reason about it; the offer rule then checks `Internal` again anyway.
pub fn discover() -> Result<Vec<Disk>, String> {
    let list = diskutil_plist(&["list", "-plist", "external"])?;
    Ok(assemble(&list, &|id| {
        diskutil_plist(&["info", "-plist", id]).ok()
    }))
}

/// **Erase a whole disk as one FAT32 volume named `NIFE`, with an MBR partition table.**
///
/// MBR rather than GPT, and it is a measured choice: `diskutil eraseDisk FAT32 ... GPT` writes a
/// 200 MB `EFI` partition *ahead of* the data partition (the two USB hard disks in
/// `tests/fixtures/macos/list-external.plist` show that layout), so the stick would
/// carry two FAT volumes and a firmware that only looks at the first finds an empty one. MBR gives
/// exactly one, which every UEFI implementation and U-Boot's distro boot read.
///
/// No administrator password: `diskutil` erases external media as the logged-in user.
pub fn erase(disk: &Disk) -> Result<(), String> {
    let device = format!("/dev/{}", disk.id);
    let status = Command::new(DISKUTIL)
        .args(["eraseDisk", "FAT32", "NIFE", "MBR", &device])
        .status()
        .map_err(|e| format!("cannot run {DISKUTIL}: {e}"))?;
    if !status.success() {
        return Err(format!("diskutil eraseDisk {device} failed ({status})"));
    }
    Ok(())
}

/// Mount a volume that is not mounted, which `diskutil` does for external media without a password.
pub fn mount(volume: &str) -> Result<(), String> {
    let status = Command::new(DISKUTIL)
        .args(["mount", volume])
        .status()
        .map_err(|e| format!("cannot run {DISKUTIL}: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("diskutil mount {volume} failed ({status})"))
    }
}

/// Eject the whole disk so it can be pulled out, which flushes every volume on it first.
pub fn eject(disk: &Disk) -> Result<(), String> {
    let status = Command::new(DISKUTIL)
        .args(["eject", &disk.id])
        .status()
        .map_err(|e| format!("cannot run {DISKUTIL}: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("diskutil eject {} failed ({status})", disk.id))
    }
}
