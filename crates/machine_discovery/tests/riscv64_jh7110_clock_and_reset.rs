//! The JH7110's clock-and-reset windows and its PMIC's bus, found in both of the chip's device
//! trees and absent from QEMU's.
//!
//! `jh7110_clock_and_reset` names every spelling and decodes the properties;
//! `machine_discovery::jh7110::clock_and_reset` walks the tree. These pin the walk against the
//! board-shaped fixtures and the tree every CI boot uses.

use jh7110_clock_and_reset::{
    AON_WINDOW, COMPATIBLE_PMIC_MAINLINE, COMPATIBLE_PMIC_VENDOR, COMPATIBLE_STGCRG,
    COMPATIBLE_SYSCRG, COMPATIBLE_VENDOR_CLKGEN, STG_WINDOW, SYS_WINDOW, Step,
};
use machine_discovery::jh7110::clock_and_reset::{discover, discover_aon, discover_sys, pmic_bus};

const STGCRG_MAINLINE: &[u8] = include_bytes!("fixtures/jh7110-stgcrg-mainline.dtb");
const CLKGEN_VENDOR: &[u8] = include_bytes!("fixtures/jh7110-clkgen-vendor.dtb");
const CLKGEN_VENDOR_UNNAMED: &[u8] = include_bytes!("fixtures/jh7110-clkgen-vendor-unnamed.dtb");
const CLKGEN_VENDOR_MISMATCHED: &[u8] =
    include_bytes!("fixtures/jh7110-clkgen-vendor-mismatched-names.dtb");
/// The blob `crates/device_tree_blob`'s own tests boot-verify against, so a change to QEMU's
/// `virt` board is caught here rather than surfacing as a mystery at the bench.
const QEMU_RISCV64_VIRT: &[u8] =
    include_bytes!("../../device_tree_blob/tests/fixtures/qemu-riscv64-virt.dtb");

const PMIC_BUS_RADON: &[u8] = include_bytes!("fixtures/jh7110-pmic-bus-radon.dtb");
const PMIC_BUS_MAINLINE: &[u8] = include_bytes!("fixtures/jh7110-pmic-bus-mainline.dtb");
const PMIC_BUS_FOREIGN: &[u8] = include_bytes!("fixtures/jh7110-pmic-bus-foreign.dtb");
const PMIC_BUS_OVERFULL: &[u8] = include_bytes!("fixtures/jh7110-pmic-bus-overfull.dtb");

/// The plan a tree that names nothing usable gets: I2C5's one gate, then its one reset.
const CONSTANT_PLAN: &[Step] = &[Step::EnableClock(143), Step::DeassertReset(81)];

fn tree(bytes: &[u8]) -> device_tree_blob::DeviceTreeBlob<'_> {
    device_tree_blob::DeviceTreeBlob::from_bytes(bytes).expect("fixture parses")
}

#[test]
fn radons_pmic_bus_is_one_real_gate_and_one_reset_and_the_virtual_clock_is_skipped() {
    // The fixture is radon's own U-Boot tree. Its bus names clock 298 first, which is a
    // divide-by-one child of 143 with no register: writing "word 298" would be a store to
    // 0x4a8 of the SYS window, past every clock and reset word. It must be skipped, counted,
    // and the plan must still be the tree's own.
    let tree = tree(PMIC_BUS_RADON);
    let bus = pmic_bus(&tree).unwrap();
    assert_eq!(
        bus.plan(),
        &[Step::EnableClock(143), Step::DeassertReset(81)]
    );
    assert!(bus.from_tree);
    assert_eq!(bus.pmic, Some(COMPATIBLE_PMIC_VENDOR));
    assert_eq!(bus.skipped, 1, "clock 298, and nothing else");
    assert!(!bus.truncated);
}

#[test]
fn radons_sys_window_is_the_vendor_nodes_first_entry_found_by_name() {
    let tree = tree(PMIC_BUS_RADON);
    let found = discover_sys(&tree).unwrap();
    assert_eq!((found.base, found.size), (SYS_WINDOW.base, SYS_WINDOW.size));
    assert!(found.from_tree);
    assert_eq!(found.compatible, Some(COMPATIBLE_VENDOR_CLKGEN));
    // And STG, from the same node, is still its second window.
    assert_eq!(discover(&tree).unwrap().base, STG_WINDOW.base);
}

#[test]
fn mainlines_pmic_bus_gives_the_same_plan_through_syscrg() {
    // Two trees that spell everything differently converge on 143 and 81, which is the
    // agreement that lets the constant fallback exist at all.
    let tree = tree(PMIC_BUS_MAINLINE);
    let bus = pmic_bus(&tree).unwrap();
    assert_eq!(bus.plan(), CONSTANT_PLAN);
    assert!(bus.from_tree);
    assert_eq!(bus.pmic, Some(COMPATIBLE_PMIC_MAINLINE));
    assert_eq!(bus.skipped, 0);
    let found = discover_sys(&tree).unwrap();
    assert_eq!(found.base, SYS_WINDOW.base);
    assert_eq!(found.compatible, Some(COMPATIBLE_SYSCRG));
}

#[test]
fn a_bus_naming_only_foreign_specifiers_falls_back_and_says_so() {
    // Wrong domain, out of bounds twice, and a two-cell provider that makes the rest of its
    // list unreadable (the trailing `<&syscrg 143>` included: refusing is the safe direction).
    // Nothing usable is left, so the constant plan is used, and `from_tree` says it was.
    let tree = tree(PMIC_BUS_FOREIGN);
    let bus = pmic_bus(&tree).unwrap();
    assert_eq!(bus.plan(), CONSTANT_PLAN);
    assert!(!bus.from_tree);
    assert_eq!(bus.pmic, Some(COMPATIBLE_PMIC_MAINLINE));
    // aoncrg 3, syscrg 190, then 36 - 16 = 20 bytes read as three eight-byte strides, then
    // syscrg 126 in `resets`.
    assert_eq!(bus.skipped, 2 + 3 + 1);
}

#[test]
fn a_bus_with_more_gates_than_the_plan_holds_keeps_the_first_and_says_so() {
    let tree = tree(PMIC_BUS_OVERFULL);
    let bus = pmic_bus(&tree).unwrap();
    assert_eq!(
        bus.plan(),
        &[
            Step::EnableClock(140),
            Step::EnableClock(141),
            Step::EnableClock(142),
            Step::EnableClock(143)
        ]
    );
    assert_eq!(bus.plan().len(), 4);
    assert!(bus.truncated);
    assert!(bus.from_tree);
    assert_eq!(bus.skipped, 0);
}

#[test]
fn qemus_virt_board_has_no_pmic_and_gets_the_constant_plan_unclaimed() {
    let tree = tree(QEMU_RISCV64_VIRT);
    let bus = pmic_bus(&tree).unwrap();
    assert_eq!(bus.plan(), CONSTANT_PLAN);
    assert!(!bus.from_tree);
    assert_eq!(bus.pmic, None);
    assert!(!discover_sys(&tree).unwrap().from_tree);
}

#[test]
fn mainline_stgcrg_is_read_from_its_single_reg() {
    let tree = tree(STGCRG_MAINLINE);
    let found = discover(&tree).unwrap();
    assert_eq!(found.base, 0x1023_0000);
    assert_eq!(found.size, 0x1_0000);
    assert!(found.from_tree);
    assert_eq!(found.compatible, Some(COMPATIBLE_STGCRG));
}

#[test]
fn the_vendor_clkgens_stg_window_is_found_by_name() {
    // The whole point of the vendor arm: `reg[0]` is the SYS domain at 0x13020000, and a
    // driver that took it would enable clock 15 of the wrong controller.
    let tree = tree(CLKGEN_VENDOR);
    let found = discover(&tree).unwrap();
    assert_eq!(
        found.base, STG_WINDOW.base,
        "the second window, not the first"
    );
    assert_eq!(found.size, 0x1_0000);
    assert!(found.from_tree);
    assert_eq!(found.compatible, Some(COMPATIBLE_VENDOR_CLKGEN));
}

#[test]
fn a_multi_window_node_without_reg_names_falls_back_rather_than_guessing() {
    let tree = tree(CLKGEN_VENDOR_UNNAMED);
    let found = discover(&tree).unwrap();
    assert_eq!(found.base, STG_WINDOW.base);
    assert!(
        !found.from_tree,
        "the address is right and nothing in the tree said so; the transcript must say which"
    );
    assert_eq!(found.compatible, None);
}

#[test]
fn qemus_virt_board_has_no_clock_controller_at_all() {
    // This is the path every machine this repository's CI boots takes, and it is why this
    // milestone cannot be gated in an emulator: `virt` has no clock or reset controller to
    // program, so what runs here is the fallback and the arithmetic, never a device.
    let tree = tree(QEMU_RISCV64_VIRT);
    let found = discover(&tree).unwrap();
    assert!(!found.from_tree);
    assert_eq!(found.compatible, None);
    assert_eq!(found.base, STG_WINDOW.base);
}

#[test]
fn a_reg_names_entry_past_the_end_of_reg_is_refused_rather_than_read() {
    // `reg-names` lists three names but `reg` has only two windows, with "stg" at index 2: an
    // index `regions` was never filled at. `Found::from_regions`'s `i < regions.len()` guard exists for exactly
    // this: a tree this malformed must fall back to the corroborated constant rather than
    // read the zeroed slot an off-by-one would land on. Kills the `i < n -> true` and
    // `< -> <=` mutants, neither of which any other fixture reaches.
    let tree = tree(CLKGEN_VENDOR_MISMATCHED);
    let found = discover(&tree).unwrap();
    assert!(
        !found.from_tree,
        "the named window does not exist; this must not be trusted"
    );
    assert_eq!(found.base, STG_WINDOW.base);
    assert_eq!(found.compatible, None);
}

#[test]
fn radons_aon_window_is_the_vendor_nodes_third_entry_found_by_name() {
    let tree = tree(CLKGEN_VENDOR);
    let found = discover_aon(&tree).unwrap();
    assert_eq!((found.base, found.size), (AON_WINDOW.base, AON_WINDOW.size));
    assert!(found.from_tree);
    assert_eq!(found.compatible, Some(COMPATIBLE_VENDOR_CLKGEN));
}

#[test]
fn a_tree_naming_no_aon_controller_gets_the_constant_and_says_so() {
    let tree = tree(STGCRG_MAINLINE);
    let found = discover_aon(&tree).unwrap();
    assert_eq!(found.base, AON_WINDOW.base);
    assert!(!found.from_tree);
}

#[test]
fn radons_pmic_bus_also_names_the_controller_window_and_the_pmic_address() {
    let tree = tree(PMIC_BUS_RADON);
    let bus = pmic_bus(&tree).unwrap();
    assert_eq!(bus.controller, Some((0x1205_0000, 0x1_0000)));
    assert_eq!(bus.pmic_address, Some(0x36));
}

#[test]
fn the_mainline_tree_names_the_same_controller_and_address() {
    let tree = tree(PMIC_BUS_MAINLINE);
    let bus = pmic_bus(&tree).unwrap();
    assert_eq!(bus.controller, Some((0x1205_0000, 0x1_0000)));
    assert_eq!(bus.pmic_address, Some(0x36));
}

#[test]
fn the_foreign_tree_names_neither() {
    let tree = tree(PMIC_BUS_FOREIGN);
    let bus = pmic_bus(&tree).unwrap();
    assert_eq!(bus.controller, None);
    assert_eq!(bus.pmic_address, None);
}
