//! **Somebody else's FAT driver reads what this crate wrote**: the host's, on an image holding
//! `\EFI\BOOT\BOOTRISCV64.EFI`, the name that needs long-name entries. `#[ignore]`d, because it
//! needs a host filesystem driver and attaches a disk image.
//!
//! On macOS it runs `fsck_msdos -n` on the image, which must exit 0, then attaches it read-only,
//! requires the listing to hold `BOOTRISCV64.EFI` and not the alias `BOOTRI~1.EFI`, and compares
//! every byte. Elsewhere it writes the image and stops. notes/file-allocation-table.md, "Who reads
//! it back", has why this is the check that counts.

//! # EXAMPLES
//!
//! ```console
//! $ cargo test -p file_allocation_table -- --ignored --nocapture
//! wrote target/file-allocation-table-check.img
//! fsck_msdos: ** Phase 1 - Preparing FAT ... ** Phase 4 - Checking Directories ...
//! the host's driver lists ["BOOTRISCV64.EFI"] in /private/tmp/dmg.XXXXXX/EFI/BOOT
//! ```
//!
//! On Linux, `fsck.fat -n` and `sudo mount -o loop,ro` are the same two checks, run by hand.

use std::io::Write;
use std::path::Path;
use std::process::Command;

use file_allocation_table::{SECTOR, Volume};

/// The file the volume carries. Not a round number, so a length the writer got wrong by a cluster
/// cannot pass by accident.
const FILE_LEN: u64 = 3_000_000;

/// 700 MiB, in 512-byte sectors: comfortably over the FAT32 cluster floor and small enough that a
/// sparse file costs nothing.
const TOTAL_SECTORS: u64 = 700 * 1024 * 2;

/// The riscv64 removable-media name, fifteen units and so two long-name entries.
const NAME: &str = "BOOTRISCV64.EFI";

#[test]
#[ignore = "attaches an image for the host's FAT driver to read; see this file's header"]
fn the_hosts_own_fat_driver_reads_the_long_name_back() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../target/file-allocation-table-check.img"
    );
    let body = write_image(Path::new(path));
    eprintln!("wrote {path}");
    if cfg!(target_os = "macos") {
        check_with_fsck(path);
        read_back(path, &body);
    }
}

/// Write the volume to `path`, returning the file's bytes.
fn write_image(path: &Path) -> Vec<u8> {
    let volume = Volume::new(TOTAL_SECTORS, 0, FILE_LEN, 0x1234_5678, NAME)
        .expect("700 MiB is a FAT32 volume");
    let mut image = std::fs::File::create(path).expect("create the image");
    let mut sector = [0u8; SECTOR];
    for index in 0..volume.metadata_sectors() {
        assert!(
            volume.sector(index, &mut sector),
            "sector {index} is metadata"
        );
        image.write_all(&sector).expect("write a metadata sector");
    }

    // A pattern with a prime stride, so a copy that is off by a cluster or that duplicates one
    // reads back wrong everywhere rather than in one place.
    let body: Vec<u8> = (0..FILE_LEN).map(|i| (i % 251) as u8).collect();
    image.write_all(&body).expect("write the file");
    let tail = volume.file_sectors() * SECTOR as u64 - FILE_LEN;
    image
        .write_all(&vec![0u8; tail as usize])
        .expect("pad the last cluster");
    image
        .set_len(TOTAL_SECTORS * SECTOR as u64)
        .expect("size the image");
    body
}

/// `fsck_msdos -n` checks every long-name run against its short entry and exits non-zero on a
/// mismatch, without writing.
fn check_with_fsck(path: &str) {
    let out = Command::new("fsck_msdos")
        .args(["-n", path])
        .output()
        .expect("run fsck_msdos");
    let report =
        String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr);
    eprintln!("fsck_msdos: {}", report.replace('\n', " "));
    assert!(out.status.success(), "fsck_msdos refused the volume");
}

/// Attach the image read-only at a fresh mount point, list `EFI/BOOT`, compare the file and
/// detach, detaching even when an assertion fails.
fn read_back(path: &str, body: &[u8]) {
    let out = Command::new("hdiutil")
        .args([
            "attach",
            "-imagekey",
            "diskimage-class=CRawDiskImage",
            "-nobrowse",
            "-readonly",
            "-mountrandom",
            "/tmp",
            path,
        ])
        .output()
        .expect("run hdiutil attach");
    assert!(out.status.success(), "hdiutil attach failed");
    let attached = String::from_utf8_lossy(&out.stdout).into_owned();
    let line = attached
        .lines()
        .find(|l| l.contains("/tmp"))
        .expect("a mount point");
    let device = line.split_whitespace().next().expect("a device").to_owned();
    let mount = &line[line
        .find("/private/tmp")
        .or_else(|| line.find("/tmp"))
        .unwrap()..];
    let checked = std::panic::catch_unwind(|| {
        let dir = Path::new(mount.trim()).join("EFI/BOOT");
        let names: Vec<String> = std::fs::read_dir(&dir)
            .expect("list EFI/BOOT")
            .map(|e| {
                e.expect("an entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        eprintln!("the host's driver lists {names:?} in {}", dir.display());
        assert_eq!(names, [NAME], "the long name, not the alias");
        assert_eq!(std::fs::read(dir.join(NAME)).expect("read the file"), body);
    });
    let detached = Command::new("hdiutil").args(["detach", &device]).status();
    assert!(
        detached.is_ok_and(|s| s.success()),
        "hdiutil detach {device}"
    );
    if let Err(panic) = checked {
        std::panic::resume_unwind(panic);
    }
}
