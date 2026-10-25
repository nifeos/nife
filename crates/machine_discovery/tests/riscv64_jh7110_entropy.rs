//! The JH7110 TRNG, found in both of the chip's device trees and absent from QEMU's.
//!
//! `jh7110_entropy` names both spellings and decodes the properties; `machine_discovery::jh7110`
//! walks the tree. These pin the walk against the board-shaped fixtures and the tree every CI boot
//! uses.

use jh7110_entropy::{COMPATIBLE, COMPATIBLE_VENDOR};
use machine_discovery::jh7110::entropy::discover;

const JH7110_TRNG_PRESENT: &[u8] = include_bytes!("fixtures/jh7110-trng-present.dtb");
const JH7110_TRNG_VENDOR_UBOOT: &[u8] = include_bytes!("fixtures/jh7110-trng-vendor-uboot.dtb");
const QEMU_RISCV64_VIRT: &[u8] =
    include_bytes!("../../device_tree_blob/tests/fixtures/qemu-riscv64-virt.dtb");

fn tree(bytes: &[u8]) -> device_tree_blob::DeviceTreeBlob<'_> {
    device_tree_blob::DeviceTreeBlob::from_bytes(bytes).expect("fixture parses")
}

#[test]
fn discover_finds_the_device_on_a_tree_shaped_like_the_real_board() {
    let found = discover(&tree(JH7110_TRNG_PRESENT))
        .expect("no parse error")
        .expect("the node is there");
    assert_eq!(found.reg_base, 0x1600_C000);
    assert_eq!(found.reg_size, 0x4000);
    assert_eq!(found.interrupt, Some(30));
    assert_eq!(found.compatible, COMPATIBLE);
    // Mainline's binding example states no `status`, and the device-tree specification's default
    // for an absent `status` is "okay".
    assert!(found.status_okay);
}

/// **Milestone 239's finding, in one assertion** (radon's device tree does not describe the TRNG,
/// so a working driver never runs): the vendor U-Boot's control DTB describes the TRNG as
/// `trng@1600C000`, `compatible = "starfive,trng"`, `status = "disabled"`, and `discover` must
/// find it. The fixture is transcribed from the firmware's source, not dumped from a board, so
/// whether the running firmware's tree carries the node is still a bench fact;
/// `design/roadmap/0239-radons-tree-describes-less-than-the-chip-has.md` has the two commands
/// that settle it.
#[test]
fn discover_finds_the_device_under_the_vendor_uboots_own_spelling() {
    let found = discover(&tree(JH7110_TRNG_VENDOR_UBOOT))
        .expect("no parse error")
        .expect("the vendor tree describes the device under its own compatible");
    assert_eq!(found.reg_base, 0x1600_C000);
    assert_eq!(found.reg_size, 0x4000);
    assert_eq!(found.interrupt, Some(30));
    assert_eq!(found.compatible, COMPATIBLE_VENDOR);
    // The one property a caller must not lose: the firmware calls its own TRNG disabled.
    assert!(!found.status_okay);
}

/// **The negative case provable without hardware.** QEMU's riscv64 `virt` machine has no TRNG
/// node, so `discover` must say so rather than match some unrelated node's `reg`. The fixture is
/// the same `qemu-riscv64-virt.dtb` that `crates/device_tree_blob/tests/qemu_riscv64_virt.rs`
/// verifies, so this is a claim about the tree this project runs.
#[test]
fn discover_finds_nothing_on_qemus_virt_board() {
    assert_eq!(discover(&tree(QEMU_RISCV64_VIRT)).expect("no parse error"), None);
}
