//! **An image made with `NoClock` is the image the crate made before it had a clock.**
//!
//! The digests below were taken from the crate at the commit before the clock became required, over
//! every metadata sector of a set of volumes. A volume formatted with `NoClock` must still match
//! them, so a reproducible image (the installer's, the cloud image's) stays byte for byte the same.
//! The digest is FNV-1a over the sectors in order; it detects a change and is not a security
//! claim.

use file_allocation_table::{FormatOptions, FormatPlan, NoClock, SectorSize, VolumeLayout};

fn fnv(mut hash: u64, bytes: &[u8]) -> u64 {
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100_0000_01b3);
    }
    hash
}

const START: u64 = 0xcbf2_9ce4_8422_2325;

fn efi(sectors: u64, file: &str) -> u64 {
    let volume =
        VolumeLayout::new(sectors, 2048, 9_000_000, 0x1234_5678, file, &mut NoClock).unwrap();
    let mut sector = [0u8; 512];
    let mut hash = START;
    for index in 0..volume.metadata_sectors() {
        assert!(volume.fill_sector(index, &mut sector));
        hash = fnv(hash, &sector);
    }
    hash
}

fn formatted(sectors: u32, label: Option<&str>) -> u64 {
    let mut options = FormatOptions::new(SectorSize::Bytes512, sectors).with_volume_id(0xC0FF_EE11);
    if let Some(label) = label {
        options = options.with_label(label).unwrap();
    }
    let format = FormatPlan::new(options, &mut NoClock).unwrap();
    let mut sector = [0u8; 512];
    let mut hash = START;
    for index in 0..format.metadata_sectors() {
        assert!(format.fill_sector(index, &mut sector));
        hash = fnv(hash, &sector);
    }
    hash
}

#[test]
fn the_efi_volumes_are_the_bytes_they_were() {
    assert_eq!(efi(1024 * 1024, "BOOTX64.EFI"), 0xe66f_a0b2_dc40_4618);
    assert_eq!(efi(1024 * 1024, "BOOTRISCV64.EFI"), 0xec9e_2585_479f_201a);
    assert_eq!(
        efi(140_000 * 8 + 4096, "BOOTAA64.EFI"),
        0xf12a_2995_e7b9_b5f6
    );
}

#[test]
fn the_formatted_volumes_are_the_bytes_they_were() {
    assert_eq!(formatted(16 * 1024, Some("EXAMPLE")), 0x41e0_d0b5_8660_3ad1);
    assert_eq!(formatted(16 * 1024, None), 0xd68a_ae2d_ad81_49b7);
    assert_eq!(formatted(64 * 1024, Some("SIXTEEN")), 0x9e2a_64e6_32b6_3606);
    assert_eq!(
        formatted(200 * 1024, Some("THIRTYTWO")),
        0xb0f0_ce40_72c3_15bb
    );
    assert_eq!(formatted(200 * 1024, None), 0xe9e9_73ac_21d9_6985);
}
