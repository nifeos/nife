//! **The JH7110 around the controller**: which node it is, what the board wired to it, and the
//! one syscon field that tells the MAC it is talking RGMII.
//!
//! The shape is OpenBSD's `if_dwqe_fdt.c` (`dwqe_fdt_attach`, `dwqe_setup_jh7110`; ISC license,
//! notice in the crate root): the controller's address picks the port, the `phy-mode` picks the
//! interface select, the PHY's node supplies its delays. radon's U-Boot hands over **the vendor's
//! own tree**, not mainline Linux's, and the two spell the controller differently (vendor
//! compatible, PHY as register values, interface select in board code). Both are read here, and
//! their shared facts agree. See notes/designware-ethernet/the-hardware.md.

use device_tree_blob::DeviceTreeBlob;

use crate::controller::Bus;
use crate::motorcomm::{Drive, Settings, Source};
use crate::regs;

/// Mainline's compatible for both ports.
pub(crate) const COMPATIBLE_MAINLINE: &[u8] = b"starfive,jh7110-dwmac";
/// The vendor U-Boot's compatible for both ports, the one radon's firmware serves.
pub(crate) const COMPATIBLE_VENDOR: &[u8] = b"starfive,jh7110-eqos-5.20";

/// `gmac0`'s window, in both trees and in OpenBSD's port table.
pub const GMAC0_BASE: u64 = 0x1603_0000;
/// `gmac1`'s window.
pub const GMAC1_BASE: u64 = 0x1604_0000;

/// **radon's bus configuration**, identical in both trees: `snps,fixed-burst`,
/// `snps,no-pbl-x8`, `snps,force_thresh_dma_mode`, `snps,txpbl = <16>`, `snps,rxpbl = <16>`, and
/// the `stmmac-axi-config` node's `snps,blen = <256 128 64 32 0 0 0>` with both outstanding-request
/// limits `0xf`. Carried as a constant rather than read, because the tree's spelling of it is the
/// one thing both trees share verbatim and parsing it would add a failure path for no new fact.
pub const BUS: Bus = Bus {
    fixed_burst: true,
    pbl_x8: false,
    tx_pbl: 16,
    rx_pbl: 16,
    threshold_mode: true,
    burst_lengths: regs::DMA_SYSBUS_MODE_BLEN_256
        | regs::DMA_SYSBUS_MODE_BLEN_128
        | regs::DMA_SYSBUS_MODE_BLEN_64
        | regs::DMA_SYSBUS_MODE_BLEN_32,
    read_outstanding: 0xf,
    write_outstanding: 0xf,
};

/// The AON syscon's window: mainline's `aon_syscon: syscon@17010000` and the vendor's
/// `aon_syscon@17010000`, both `0x1000` long.
pub(crate) const AON_SYSCON_BASE: u64 = 0x1701_0000;
/// Its size.
pub(crate) const AON_SYSCON_SIZE: u64 = 0x1000;
/// Mainline's compatible for it. The vendor's node says only `syscon`, which names nothing.
pub(crate) const COMPATIBLE_AON_SYSCON: &[u8] = b"starfive,jh7110-aon-syscon";
/// The word holding `gmac0`'s interface select: mainline's `<&aon_syscon 0xc 0x12>` second cell,
/// the vendor's `AON_SYSCFG_12`.
pub const GMAC0_SYSCON_OFFSET: u64 = 0xc;
/// The field's shift: mainline's third cell `0x12`, the vendor's `GMAC5_0_SEL_I_SHIFT`.
pub(crate) const GMAC0_SYSCON_SHIFT: u32 = 18;
/// The field is three bits wide (OpenBSD's `((1U << 3) - 1) << shift`; the vendor's mask
/// `0x1c0000`).
pub(crate) const SYSCON_FIELD: u32 = 0x7;
/// The RGMII interface (OpenBSD's `JH7110_PHY_INTF_RGMII`; the vendor's `BIT(SHIFT)` for 1000M).
pub(crate) const PHY_INTERFACE_RGMII: u32 = 1;

/// **The syscon word with `gmac0`'s interface select set to RGMII**, every other field kept.
pub const fn select_rgmii(word: u32) -> u32 {
    (word & !(SYSCON_FIELD << GMAC0_SYSCON_SHIFT)) | PHY_INTERFACE_RGMII << GMAC0_SYSCON_SHIFT
}

/// The interface select a syscon word holds for `gmac0`.
pub const fn interface(word: u32) -> u32 {
    (word >> GMAC0_SYSCON_SHIFT) & SYSCON_FIELD
}

/// What the tree says about the first port.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Discovered {
    /// The window's physical base.
    pub base: u64,
    /// Its size.
    pub size: u64,
    /// The first `interrupts` cell, `macirq`.
    pub interrupt: Option<u32>,
    /// Which spelling matched.
    pub compatible: &'static [u8],
    /// Whether `status` is absent, `"okay"` or `"ok"`.
    pub status_okay: bool,
    /// `local-mac-address`, which radon's U-Boot writes into the tree it hands over from the
    /// EEPROM's address (its `fdt_fixup_ethernet`, through the `ethernet0` alias). `None` when the
    /// tree has none, and then the driver falls back to what the address registers hold.
    pub mac: Option<[u8; 6]>,
    /// Whether `phy-mode` names an RGMII variant. Both trees say `rgmii-id`; anything else is a
    /// board this driver has no interface select for.
    pub rgmii: bool,
    /// The PHY's MDIO address: its node's `reg`, else its unit address, else 0.
    pub phy_address: u8,
    /// The PHY's board settings, and which spelling they were read from.
    pub phy: Settings,
}

/// The default RGMII internal delay when a mainline-style node gives one delay and not the
/// other: OpenBSD's `1950`.
const DEFAULT_DELAY_PS: u32 = 1950;

fn be32(bytes: &[u8]) -> Option<u32> {
    Some(u32::from_be_bytes(bytes.get(..4)?.try_into().ok()?))
}

/// **Find the first port in `tree`**: mainline's spelling first, then the vendor's, the order
/// `jh7110_entropy::discover` uses and for its reason. `Ok(None)` when neither names a node.
///
/// # Errors
///
/// Propagates [`device_tree_blob::Error`] if the blob is malformed.
pub fn discover(tree: &DeviceTreeBlob<'_>) -> Result<Option<Discovered>, device_tree_blob::Error> {
    for compatible in [COMPATIBLE_MAINLINE, COMPATIBLE_VENDOR] {
        let mut regions = [device_tree_blob::Region { start: 0, size: 0 }; 1];
        if tree.node_reg_compatible(compatible, &mut regions)? == 0 {
            continue;
        }
        let prop = |name: &[u8]| tree.node_prop_compatible(compatible, name);
        let status_okay =
            prop(b"status")?.is_none_or(|v| matches!(v, b"okay\0" | b"okay" | b"ok\0" | b"ok"));
        let mac = prop(b"local-mac-address")?
            .and_then(|b| b.get(..6))
            .and_then(|b| <[u8; 6]>::try_from(b).ok());
        let rgmii = prop(b"phy-mode")?.is_some_and(|v| v.starts_with(b"rgmii"));
        let (phy_address, phy) = match prop(b"phy-handle")?.and_then(be32) {
            Some(handle) => mainline_phy(tree, handle)?,
            None => vendor_phy(tree)?,
        };
        return Ok(Some(Discovered {
            base: regions[0].start,
            size: regions[0].size,
            interrupt: prop(b"interrupts")?.and_then(be32),
            compatible,
            status_okay,
            mac,
            rgmii,
            phy_address,
            phy,
        }));
    }
    Ok(None)
}

/// The PHY as mainline describes it: through `phy-handle`, in physical units.
fn mainline_phy(
    tree: &DeviceTreeBlob<'_>,
    handle: u32,
) -> Result<(u8, Settings), device_tree_blob::Error> {
    let num = |name: &[u8]| tree.phandle_prop(handle, name).map(|v| v.and_then(be32));
    let has = |name: &[u8]| tree.phandle_prop(handle, name).map(|v| v.is_some());
    let address = num(b"reg")?.map_or(0, |r| (r & 0x1f) as u8);
    let (rx, tx) = (num(b"rx-internal-delay-ps")?, num(b"tx-internal-delay-ps")?);
    if rx.is_none() && tx.is_none() {
        return Ok((address, Settings::FALLBACK_GMAC0));
    }
    let inverted = if has(b"motorcomm,tx-clk-adj-enabled")? {
        Some([
            has(b"motorcomm,tx-clk-10-inverted")?,
            has(b"motorcomm,tx-clk-100-inverted")?,
            has(b"motorcomm,tx-clk-1000-inverted")?,
        ])
    } else {
        None
    };
    Ok((
        address,
        Settings::from_mainline(
            rx.unwrap_or(DEFAULT_DELAY_PS),
            tx.unwrap_or(DEFAULT_DELAY_PS),
            num(b"motorcomm,rx-clk-drv-microamp")?.unwrap_or(0),
            num(b"motorcomm,rx-data-drv-microamp")?.unwrap_or(0),
            inverted,
        ),
    ))
}

/// The PHY as the vendor tree describes it: `gmac0`'s child `ethernet-phy@0`, in register
/// values. Found by node name, because the vendor tree gives it no `phy-handle`; `gmac0` precedes
/// `gmac1` in that tree, and `gmac1`'s PHY is `ethernet-phy@1`, so the first `ethernet-phy@0` is
/// `gmac0`'s. Any property it lacks keeps [`Settings::FALLBACK_GMAC0`]'s value.
fn vendor_phy(tree: &DeviceTreeBlob<'_>) -> Result<(u8, Settings), device_tree_blob::Error> {
    const NODE: &[u8] = b"ethernet-phy@0";
    let num = |name: &[u8]| tree.node_prop(NODE, name).map(|v| v.and_then(be32));
    let Some(rx) = num(b"rx_delay_sel")? else {
        return Ok((0, Settings::FALLBACK_GMAC0));
    };
    let f = Settings::FALLBACK_GMAC0;
    let flag = |v: Option<u32>, d: bool| v.map_or(d, |x| x != 0);
    Ok((
        num(b"reg")?.map_or(0, |r| (r & 0x1f) as u8),
        Settings {
            rx_delay: (rx & 0xf) as u8,
            rx_fixed_delay: flag(num(b"rxc_dly_en")?, f.rx_fixed_delay),
            tx_delay: num(b"tx_delay_sel")?.map_or(f.tx_delay, |v| (v & 0xf) as u8),
            rx_clock_drive: num(b"rgmii_sw_dr_rxc")?
                .map_or(f.rx_clock_drive, |v| Drive::Code((v & 7) as u8)),
            rx_data_drive: num(b"rgmii_sw_dr")?
                .map_or(f.rx_data_drive, |v| Drive::Code((v & 7) as u8)),
            tx_clock_inverted: [
                flag(num(b"tx_inverted_10")?, f.tx_clock_inverted[0]),
                flag(num(b"tx_inverted_100")?, f.tx_clock_inverted[1]),
                flag(num(b"tx_inverted_1000")?, f.tx_clock_inverted[2]),
            ],
            source: Source::VendorTree,
        },
    ))
}

/// Where the AON syscon is, and whether the tree said so.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Syscon {
    /// Physical base.
    pub base: u64,
    /// Size.
    pub size: u64,
    /// True when a tree node named it; false when this is [`AON_SYSCON_BASE`].
    pub from_tree: bool,
}

/// **Find the AON syscon**: mainline's compatible, then the vendor's node name, then the
/// constant both trees agree on. Like `jh7110_clock_and_reset::discover`, never fails to produce
/// an address, so an answer here is not evidence the machine is a JH7110.
///
/// # Errors
///
/// Propagates [`device_tree_blob::Error`] if the blob is malformed.
pub fn discover_aon_syscon(tree: &DeviceTreeBlob<'_>) -> Result<Syscon, device_tree_blob::Error> {
    let mut regions = [device_tree_blob::Region { start: 0, size: 0 }; 1];
    if tree.node_reg_compatible(COMPATIBLE_AON_SYSCON, &mut regions)? == 1
        || tree.node_reg(b"aon_syscon@", &mut regions)? == 1
    {
        return Ok(Syscon {
            base: regions[0].start,
            size: regions[0].size,
            from_tree: true,
        });
    }
    Ok(Syscon {
        base: AON_SYSCON_BASE,
        size: AON_SYSCON_SIZE,
        from_tree: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const VENDOR: &[u8] = include_bytes!("../tests/fixtures/jh7110-gmac-vendor.dtb");
    const MAINLINE: &[u8] = include_bytes!("../tests/fixtures/jh7110-gmac-mainline.dtb");
    const ONE_DELAY: &[u8] = include_bytes!("../tests/fixtures/jh7110-gmac-one-delay.dtb");
    const VENDOR_ADDRESSED: &[u8] =
        include_bytes!("../tests/fixtures/jh7110-gmac-vendor-addressed.dtb");
    const QEMU_RISCV64_VIRT: &[u8] =
        include_bytes!("../../device_tree_blob/tests/fixtures/qemu-riscv64-virt.dtb");

    #[test]
    fn radons_vendor_tree_gives_gmac0_its_phy_settings_and_its_address() {
        let tree = DeviceTreeBlob::from_bytes(VENDOR).unwrap();
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
        let tree = DeviceTreeBlob::from_bytes(MAINLINE).unwrap();
        let d = discover(&tree).unwrap().unwrap();
        assert_eq!(d.base, GMAC0_BASE);
        assert_eq!(d.compatible, COMPATIBLE_MAINLINE);
        assert_eq!(d.mac, None);
        assert_eq!(d.phy.source, Source::MainlineTree);
        assert_eq!(d.phy.rx_delay, 0xa);
        assert_eq!(d.phy.tx_delay, 0xa);
        assert!(!d.phy.rx_fixed_delay);
        assert_eq!(d.phy.tx_clock_inverted, [false, true, true]);
        assert_eq!(crate::motorcomm::drive_code(1800, d.phy.rx_clock_drive), 6);
    }

    #[test]
    fn qemu_virt_has_no_port_and_the_syscon_falls_back_and_says_so() {
        let tree = DeviceTreeBlob::from_bytes(QEMU_RISCV64_VIRT).unwrap();
        assert_eq!(discover(&tree).unwrap(), None);
        let s = discover_aon_syscon(&tree).unwrap();
        assert_eq!(s.base, AON_SYSCON_BASE);
        assert!(!s.from_tree);
    }

    #[test]
    fn both_trees_name_the_aon_syscon() {
        for blob in [VENDOR, MAINLINE] {
            let tree = DeviceTreeBlob::from_bytes(blob).unwrap();
            let s = discover_aon_syscon(&tree).unwrap();
            assert_eq!((s.base, s.size), (AON_SYSCON_BASE, AON_SYSCON_SIZE));
            assert!(s.from_tree);
        }
    }

    #[test]
    fn the_interface_select_touches_only_gmac0s_three_bits() {
        assert_eq!(select_rgmii(0), 0x0004_0000);
        assert_eq!(select_rgmii(0xffff_ffff), !0x0018_0000);
        assert_eq!(interface(select_rgmii(0x0010_0000)), PHY_INTERFACE_RGMII);
        // The vendor U-Boot's mask, 0x1c0000, is exactly this field.
        assert_eq!(SYSCON_FIELD << GMAC0_SYSCON_SHIFT, 0x1c_0000);
        assert_eq!(interface(0), 0);
        assert_eq!(interface(0x0018_0000), 6);
    }

    /// `snps,blen = <256 128 64 32 0 0 0>` is bits 7 to 4 of the bus mode, all four.
    #[test]
    fn radons_burst_lengths_are_all_four_bits() {
        assert_eq!(BUS.burst_lengths, 0xf0);
    }

    /// **One delay is enough to read a mainline PHY node**: the receive delay given is used, the
    /// transmit delay takes OpenBSD's 1950 ps (select 13), and the MDIO address is the node's
    /// `reg`. The fixture is synthetic; radon's trees give both delays at address 0.
    #[test]
    fn a_mainline_phy_with_one_delay_is_read_not_replaced_by_the_fallback() {
        let tree = DeviceTreeBlob::from_bytes(ONE_DELAY).unwrap();
        let d = discover(&tree).unwrap().unwrap();
        assert_eq!(d.phy.source, Source::MainlineTree);
        assert_eq!((d.phy.rx_delay, d.phy.tx_delay), (10, 13));
        assert_eq!(d.phy_address, 3);
        let tree = DeviceTreeBlob::from_bytes(MAINLINE).unwrap();
        assert_eq!(discover(&tree).unwrap().unwrap().phy_address, 0);
    }

    /// A vendor PHY node that states its address in `reg` is believed. radon's does not, and
    /// reads as 0.
    #[test]
    fn a_vendor_phy_reg_is_its_mdio_address() {
        let tree = DeviceTreeBlob::from_bytes(VENDOR_ADDRESSED).unwrap();
        let d = discover(&tree).unwrap().unwrap();
        assert_eq!(d.phy.source, Source::VendorTree);
        assert_eq!(d.phy_address, 3);
    }
}
