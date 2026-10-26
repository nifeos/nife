//! The JH7110's first Ethernet port and its AON syscon, found in both of the chip's device trees
//! and absent from QEMU's.
//!
//! `designware_ethernet::jh7110` names both spellings and decodes the properties;
//! `machine_discovery::jh7110::designware_ethernet` walks the tree. These pin the walk against the
//! board-shaped fixtures and the tree every CI boot uses.

use designware_ethernet::jh7110::{COMPATIBLE_MAINLINE, COMPATIBLE_VENDOR, GMAC0_BASE};
use designware_ethernet::motorcomm::{Settings, Source};
use machine_discovery::jh7110::designware_ethernet::{discover, discover_aon_syscon};

const VENDOR: &[u8] = include_bytes!("fixtures/jh7110-gmac-vendor.dtb");
const MAINLINE: &[u8] = include_bytes!("fixtures/jh7110-gmac-mainline.dtb");
const ONE_DELAY: &[u8] = include_bytes!("fixtures/jh7110-gmac-one-delay.dtb");
const VENDOR_ADDRESSED: &[u8] = include_bytes!("fixtures/jh7110-gmac-vendor-addressed.dtb");
const QEMU_RISCV64_VIRT: &[u8] =
    include_bytes!("../../device_tree_blob/tests/fixtures/qemu-riscv64-virt.dtb");

fn tree(bytes: &[u8]) -> device_tree_blob::DeviceTreeBlob<'_> {
    device_tree_blob::DeviceTreeBlob::from_bytes(bytes).expect("fixture parses")
}

#[test]
fn radons_vendor_tree_gives_gmac0_its_phy_settings_and_its_address() {
    let tree = tree(VENDOR);
    let d = discover(&tree).unwrap().unwrap();
    assert_eq!((d.base, d.size), (GMAC0_BASE, 0x1_0000));
    assert_eq!(d.compatible, COMPATIBLE_VENDOR);
    assert_eq!(d.interrupt, Some(7));
    assert!(d.status_okay && d.rgmii);
    assert_eq!(d.mac, Some([0x6c, 0xcf, 0x39, 0x00, 0x47, 0x2c]));
    assert_eq!(d.phy_address, 0);
    assert_eq!(d.phy.source, Source::VendorTree);
    assert_eq!(
        Settings {
            source: Source::Fallback,
            ..d.phy
        },
        Settings::FALLBACK_GMAC0,
        "the vendor's gmac0 values are the fallback's, field for field"
    );
}

#[test]
fn mainlines_tree_reaches_the_same_registers_through_phy_handle() {
    let tree = tree(MAINLINE);
    let d = discover(&tree).unwrap().unwrap();
    assert_eq!(d.base, GMAC0_BASE);
    assert_eq!(d.compatible, COMPATIBLE_MAINLINE);
    assert_eq!(d.mac, None);
    assert_eq!(d.phy.source, Source::MainlineTree);
    assert_eq!(d.phy.rx_delay, 0xa);
    assert_eq!(d.phy.tx_delay, 0xa);
    assert!(!d.phy.rx_fixed_delay);
    assert_eq!(d.phy.tx_clock_inverted, [false, true, true]);
}

#[test]
fn qemu_virt_has_no_port_and_the_syscon_falls_back_and_says_so() {
    let tree = tree(QEMU_RISCV64_VIRT);
    assert_eq!(discover(&tree).unwrap(), None);
    let s = discover_aon_syscon(&tree).unwrap();
    assert_eq!(s.base, 0x1701_0000);
    assert!(!s.from_tree);
}

#[test]
fn both_trees_name_the_aon_syscon() {
    for blob in [VENDOR, MAINLINE] {
        let tree = tree(blob);
        let s = discover_aon_syscon(&tree).unwrap();
        assert_eq!((s.base, s.size), (0x1701_0000, 0x1000));
        assert!(s.from_tree);
    }
}

/// **One delay is enough to read a mainline PHY node**: the receive delay given is used, the
/// transmit delay takes OpenBSD's 1950 ps (select 13), and the MDIO address is the node's
/// `reg`. The fixture is synthetic; radon's trees give both delays at address 0.
#[test]
fn a_mainline_phy_with_one_delay_is_read_not_replaced_by_the_fallback() {
    let one_delay = tree(ONE_DELAY);
    let d = discover(&one_delay).unwrap().unwrap();
    assert_eq!(d.phy.source, Source::MainlineTree);
    assert_eq!((d.phy.rx_delay, d.phy.tx_delay), (10, 13));
    assert_eq!(d.phy_address, 3);
    let mainline = tree(MAINLINE);
    assert_eq!(discover(&mainline).unwrap().unwrap().phy_address, 0);
}

/// A vendor PHY node that states its address in `reg` is believed. radon's does not, and
/// reads as 0.
#[test]
fn a_vendor_phy_reg_is_its_mdio_address() {
    let tree = tree(VENDOR_ADDRESSED);
    let d = discover(&tree).unwrap().unwrap();
    assert_eq!(d.phy.source, Source::VendorTree);
    assert_eq!(d.phy_address, 3);
}
