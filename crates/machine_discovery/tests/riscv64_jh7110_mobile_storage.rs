//! The JH7110's two SD/MMC controllers, found in both of the chip's device trees and absent from
//! QEMU's.
//!
//! `designware_mobile_storage::jh7110` names both bindings and decodes the properties;
//! `machine_discovery::jh7110::designware_mobile_storage` walks the tree. These pin the walk
//! against the board-shaped fixtures and the tree every CI boot uses.

use designware_mobile_storage::jh7110::{
    COMPATIBLE_MAINLINE, COMPATIBLE_VENDOR, FIFO_DEPTH, INTERRUPTS, SDIO0_BASE, SDIO1_BASE,
    WINDOW_SIZE,
};
use device_tree_blob::DeviceTreeBlob;
use machine_discovery::jh7110::designware_mobile_storage::discover;

const VENDOR: &[u8] = include_bytes!("fixtures/jh7110-mmc-vendor.dtb");
const MAINLINE: &[u8] = include_bytes!("fixtures/jh7110-mmc-mainline.dtb");

#[test]
fn radons_vendor_tree_names_both_slots_with_the_generic_binding_and_no_interrupt() {
    let tree = DeviceTreeBlob::from_bytes(VENDOR).unwrap();
    let sd = discover(&tree, 1).unwrap().unwrap();
    assert_eq!((sd.base, sd.size), (SDIO1_BASE, WINDOW_SIZE));
    assert_eq!(sd.compatible, COMPATIBLE_VENDOR);
    assert_eq!(sd.bus_width, 4);
    assert_eq!(sd.fifo_depth, Some(FIFO_DEPTH));
    assert_eq!(sd.ciu_hz, 50_000_000);
    assert_eq!(sd.interrupt, None);
    assert!(sd.status_okay);
    let emmc = discover(&tree, 0).unwrap().unwrap();
    assert_eq!((emmc.base, emmc.bus_width), (SDIO0_BASE, 8));
    assert_eq!(emmc.role(), "eMMC socket");
}

#[test]
fn mainline_names_the_jh7110_binding_and_the_interrupts() {
    let tree = DeviceTreeBlob::from_bytes(MAINLINE).unwrap();
    let sd = discover(&tree, 1).unwrap().unwrap();
    assert_eq!(sd.compatible, COMPATIBLE_MAINLINE);
    assert_eq!(sd.interrupt, Some(INTERRUPTS[1]));
    assert_eq!(sd.bus_width, 4);
    let emmc = discover(&tree, 0).unwrap().unwrap();
    assert_eq!(emmc.interrupt, Some(INTERRUPTS[0]));
}

#[test]
fn a_tree_without_the_controller_or_a_third_slot_finds_nothing() {
    let tree = DeviceTreeBlob::from_bytes(MAINLINE).unwrap();
    assert_eq!(discover(&tree, 2).unwrap(), None);
    let qemu = DeviceTreeBlob::from_bytes(include_bytes!(
        "../../device_tree_blob/tests/fixtures/qemu-riscv64-virt.dtb"
    ))
    .unwrap();
    assert_eq!(discover(&qemu, 0).unwrap(), None);
    assert_eq!(discover(&qemu, 1).unwrap(), None);
}

/// A node at a slot's name with neither binding is not this controller, and a stated clock
/// rate is used rather than the default.
#[test]
fn a_foreign_binding_is_skipped_and_a_stated_rate_is_kept() {
    let tree = DeviceTreeBlob::from_bytes(include_bytes!("fixtures/jh7110-mmc-odd.dtb")).unwrap();
    assert_eq!(discover(&tree, 0).unwrap(), None);
    assert_eq!(discover(&tree, 1).unwrap().unwrap().ciu_hz, 100_000_000);
}
