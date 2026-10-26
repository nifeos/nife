//! **The JH7110 around the controller**: which node it is, what the board wired to it, and the
//! one syscon field that tells the MAC it is talking RGMII.
//!
//! The shape is OpenBSD's `if_dwqe_fdt.c` (`dwqe_fdt_attach`, `dwqe_setup_jh7110`; ISC license,
//! notice in the crate root): the controller's address picks the port, the `phy-mode` picks the
//! interface select, the PHY's node supplies its delays. radon's U-Boot hands over **the vendor's
//! own tree**, not mainline Linux's, and the two spell the controller differently (vendor
//! compatible, PHY as register values, interface select in board code). Both are read here, and
//! their shared facts agree.
//!
//! The crate reads no device tree. The caller finds the port's node (one of [`COMPATIBLES`]) and
//! hands [`Discovered::from_node`] its properties; it reads the PHY's node through `phy-handle`
//! for [`mainline_phy`], or the node [`VENDOR_PHY_NODE`] for [`vendor_phy`]. The AON syscon is
//! mainline's [`COMPATIBLE_AON_SYSCON`] node or the vendor's node named [`AON_SYSCON_NODE`], and
//! [`Syscon::fallback`] answers for a tree that names neither.

use crate::controller::Bus;
use crate::motorcomm::{Drive, Settings, Source};
use crate::regs;

/// Mainline's compatible for both ports.
///
/// Name: provisional, minted 2026-10-25 (UTC) by lane crates-stand-alone-cuts, when the device-tree
/// walk and the process layout left the driver crates.
pub const COMPATIBLE_MAINLINE: &[u8] = b"starfive,jh7110-dwmac";
/// The vendor U-Boot's compatible for both ports, the one radon's firmware serves.
///
/// Name: provisional, minted 2026-10-25 (UTC) by lane crates-stand-alone-cuts, when the device-tree
/// walk and the process layout left the driver crates.
pub const COMPATIBLE_VENDOR: &[u8] = b"starfive,jh7110-eqos-5.20";
/// Both spellings, in the order to try them: mainline's first.
///
/// Name: provisional, minted 2026-10-25 (UTC) by lane crates-stand-alone-cuts, when the device-tree
/// walk and the process layout left the driver crates.
pub const COMPATIBLES: [&[u8]; 2] = [COMPATIBLE_MAINLINE, COMPATIBLE_VENDOR];

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
///
/// Name: provisional, minted 2026-10-25 (UTC) by lane crates-stand-alone-cuts, when the device-tree
/// walk and the process layout left the driver crates.
pub const COMPATIBLE_AON_SYSCON: &[u8] = b"starfive,jh7110-aon-syscon";
/// The vendor's node name for it, a prefix that matches `aon_syscon@17010000`.
///
/// Name: provisional, minted 2026-10-25 (UTC) by lane crates-stand-alone-cuts, when the device-tree
/// walk and the process layout left the driver crates.
pub const AON_SYSCON_NODE: &[u8] = b"aon_syscon@";
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

/// `gmac0`'s PHY in the vendor tree: `gmac0`'s child, found by node name because the vendor tree
/// gives it no `phy-handle`. `gmac0` precedes `gmac1` in that tree, and `gmac1`'s PHY is
/// `ethernet-phy@1`, so the first `ethernet-phy@0` is `gmac0`'s.
///
/// Name: provisional, minted 2026-10-25 (UTC) by lane crates-stand-alone-cuts, when the device-tree
/// walk and the process layout left the driver crates.
pub const VENDOR_PHY_NODE: &[u8] = b"ethernet-phy@0";

/// The first cell of a property, big-endian, as every device-tree cell is: `interrupts`'s first
/// interrupt, `phy-handle`'s phandle, a `reg`'s first word.
///
/// Name: provisional, minted 2026-10-25 (UTC) by lane crates-stand-alone-cuts, when the device-tree
/// walk and the process layout left the driver crates.
#[must_use]
pub fn first_cell(value: &[u8]) -> Option<u32> {
    Some(u32::from_be_bytes(value.get(..4)?.try_into().ok()?))
}

impl Discovered {
    /// **The port, from its node.** `base` and `size` are its `reg`, `compatible` the
    /// [`COMPATIBLES`] entry it matched, `prop(name)` the port node's property `name`, and `phy`
    /// what [`mainline_phy`] or [`vendor_phy`] read: mainline's when the node has a `phy-handle`,
    /// the vendor's otherwise.
    ///
    /// # Errors
    ///
    /// Propagates the first error `prop` returns.
    ///
    /// Name: provisional, minted 2026-10-25 (UTC) by lane crates-stand-alone-cuts, when the device-
    /// tree walk and the process layout left the driver crates.
    pub fn from_node<'t, E>(
        base: u64,
        size: u64,
        compatible: &'static [u8],
        mut prop: impl FnMut(&[u8]) -> Result<Option<&'t [u8]>, E>,
        phy: (u8, Settings),
    ) -> Result<Self, E> {
        let status_okay =
            prop(b"status")?.is_none_or(|v| matches!(v, b"okay\0" | b"okay" | b"ok\0" | b"ok"));
        let mac = prop(b"local-mac-address")?
            .and_then(|b| b.get(..6))
            .and_then(|b| <[u8; 6]>::try_from(b).ok());
        let rgmii = prop(b"phy-mode")?.is_some_and(|v| v.starts_with(b"rgmii"));
        let (phy_address, phy) = phy;
        Ok(Discovered {
            base,
            size,
            interrupt: prop(b"interrupts")?.and_then(first_cell),
            compatible,
            status_okay,
            mac,
            rgmii,
            phy_address,
            phy,
        })
    }
}

/// **The PHY as mainline describes it**, in physical units: `prop(name)` is the property `name`
/// of the node the port's `phy-handle` names. Gives the MDIO address (its `reg`, else 0) and the
/// settings; a node with neither internal delay keeps [`Settings::FALLBACK_GMAC0`].
///
/// # Errors
///
/// Propagates the first error `prop` returns.
///
/// Name: provisional, minted 2026-10-25 (UTC) by lane crates-stand-alone-cuts, when the device-tree
/// walk and the process layout left the driver crates.
pub fn mainline_phy<'t, E>(
    mut prop: impl FnMut(&[u8]) -> Result<Option<&'t [u8]>, E>,
) -> Result<(u8, Settings), E> {
    let mut num = |name: &[u8]| prop(name).map(|v| v.and_then(first_cell));
    let address = num(b"reg")?.map_or(0, |r| (r & 0x1f) as u8);
    let (rx, tx) = (num(b"rx-internal-delay-ps")?, num(b"tx-internal-delay-ps")?);
    if rx.is_none() && tx.is_none() {
        return Ok((address, Settings::FALLBACK_GMAC0));
    }
    let rx_clock = num(b"motorcomm,rx-clk-drv-microamp")?.unwrap_or(0);
    let rx_data = num(b"motorcomm,rx-data-drv-microamp")?.unwrap_or(0);
    let mut has = |name: &[u8]| prop(name).map(|v| v.is_some());
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
            rx_clock,
            rx_data,
            inverted,
        ),
    ))
}

/// **The PHY as the vendor tree describes it**, in register values: `prop(name)` is the property
/// `name` of the node [`VENDOR_PHY_NODE`]. Any property it lacks keeps
/// [`Settings::FALLBACK_GMAC0`]'s value, and a node without `rx_delay_sel` is the fallback whole.
///
/// # Errors
///
/// Propagates the first error `prop` returns.
///
/// Name: provisional, minted 2026-10-25 (UTC) by lane crates-stand-alone-cuts, when the device-tree
/// walk and the process layout left the driver crates.
pub fn vendor_phy<'t, E>(
    mut prop: impl FnMut(&[u8]) -> Result<Option<&'t [u8]>, E>,
) -> Result<(u8, Settings), E> {
    let mut num = |name: &[u8]| prop(name).map(|v| v.and_then(first_cell));
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
    /// True when a tree node named it; false when this is [`Syscon::fallback`].
    pub from_tree: bool,
}

impl Syscon {
    /// **The constant both trees agree on**, `0x1701_0000`, `0x1000` long, with `from_tree`
    /// false. An answer here is not evidence the machine is a JH7110.
    ///
    /// Name: provisional, minted 2026-10-25 (UTC) by lane crates-stand-alone-cuts, when the device-
    /// tree walk and the process layout left the driver crates.
    #[must_use]
    pub const fn fallback() -> Self {
        Syscon {
            base: AON_SYSCON_BASE,
            size: AON_SYSCON_SIZE,
            from_tree: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A property table standing in for one node: each name and its value.
    fn node<'t>(
        table: &'t [(&'static [u8], &'t [u8])],
    ) -> impl FnMut(&[u8]) -> Result<Option<&'t [u8]>, ()> + 't {
        move |name| Ok(table.iter().find(|(n, _)| *n == name).map(|(_, v)| *v))
    }

    const ZERO: &[u8] = &[0, 0, 0, 0];
    const ONE: &[u8] = &[0, 0, 0, 1];

    #[test]
    fn a_port_node_reads_its_status_address_mode_and_interrupt() {
        let table: &[(&[u8], &[u8])] = &[
            (b"status", b"okay\0"),
            (b"local-mac-address", &[0x6c, 0xcf, 0x39, 0x00, 0x47, 0x2c]),
            (b"phy-mode", b"rgmii-id\0"),
            (b"interrupts", &[0, 0, 0, 7, 0, 0, 0, 6]),
        ];
        let phy = (0, Settings::FALLBACK_GMAC0);
        let d = Discovered::from_node(GMAC0_BASE, 0x1_0000, COMPATIBLE_VENDOR, node(table), phy)
            .unwrap();
        assert_eq!(
            (d.base, d.size, d.compatible),
            (GMAC0_BASE, 0x1_0000, COMPATIBLE_VENDOR)
        );
        assert!(d.status_okay && d.rgmii);
        assert_eq!(d.mac, Some([0x6c, 0xcf, 0x39, 0x00, 0x47, 0x2c]));
        assert_eq!(d.interrupt, Some(7));
        assert_eq!((d.phy_address, d.phy), phy);
    }

    #[test]
    fn a_bare_port_node_is_okay_with_no_address_and_no_rgmii() {
        let d = Discovered::from_node(
            1,
            2,
            COMPATIBLE_MAINLINE,
            node(&[]),
            (3, Settings::FALLBACK_GMAC0),
        )
        .unwrap();
        assert!(d.status_okay);
        assert!(!d.rgmii);
        assert_eq!((d.mac, d.interrupt, d.phy_address), (None, None, 3));
        let table: &[(&[u8], &[u8])] = &[
            (b"status", b"disabled\0"),
            (b"local-mac-address", &[1, 2, 3]),
            (b"phy-mode", b"rmii\0"),
        ];
        let d = Discovered::from_node(
            1,
            2,
            COMPATIBLE_MAINLINE,
            node(table),
            (0, Settings::FALLBACK_GMAC0),
        )
        .unwrap();
        assert!(!d.status_okay && !d.rgmii);
        assert_eq!(d.mac, None);
        for okay in [&b"ok\0"[..], b"ok", b"okay"] {
            let table: &[(&[u8], &[u8])] = &[(b"status", okay)];
            let d = Discovered::from_node(
                1,
                2,
                COMPATIBLE_MAINLINE,
                node(table),
                (0, Settings::FALLBACK_GMAC0),
            )
            .unwrap();
            assert!(d.status_okay);
        }
    }

    #[test]
    fn a_mainline_phy_reads_its_delays_drive_and_inversions() {
        let table: &[(&[u8], &[u8])] = &[
            (b"reg", &[0, 0, 0, 0x23]),
            (b"rx-internal-delay-ps", &1500u32.to_be_bytes()),
            (b"tx-internal-delay-ps", &1500u32.to_be_bytes()),
            (b"motorcomm,rx-clk-drv-microamp", &3970u32.to_be_bytes()),
            (b"motorcomm,rx-data-drv-microamp", &2910u32.to_be_bytes()),
            (b"motorcomm,tx-clk-adj-enabled", b""),
            (b"motorcomm,tx-clk-100-inverted", b""),
            (b"motorcomm,tx-clk-1000-inverted", b""),
        ];
        let (address, s) = mainline_phy(node(table)).unwrap();
        assert_eq!(address, 3, "the address is five bits");
        assert_eq!(
            s,
            Settings::from_mainline(1500, 1500, 3970, 2910, Some([false, true, true]))
        );
        assert_eq!(s.source, Source::MainlineTree);
    }

    #[test]
    fn a_mainline_phy_with_one_delay_takes_the_default_for_the_other() {
        let table: &[(&[u8], &[u8])] = &[(b"rx-internal-delay-ps", &1500u32.to_be_bytes())];
        let (address, s) = mainline_phy(node(table)).unwrap();
        assert_eq!(address, 0);
        assert_eq!(
            s,
            Settings::from_mainline(1500, DEFAULT_DELAY_PS, 0, 0, None)
        );
        let table: &[(&[u8], &[u8])] = &[(b"tx-internal-delay-ps", &1500u32.to_be_bytes())];
        assert_eq!(
            mainline_phy(node(table)).unwrap().1,
            Settings::from_mainline(DEFAULT_DELAY_PS, 1500, 0, 0, None)
        );
        let table: &[(&[u8], &[u8])] = &[(b"reg", ONE)];
        assert_eq!(
            mainline_phy(node(table)).unwrap(),
            (1, Settings::FALLBACK_GMAC0)
        );
    }

    #[test]
    fn a_vendor_phy_reads_register_values_and_keeps_the_fallback_for_the_rest() {
        let table: &[(&[u8], &[u8])] = &[
            (b"reg", &[0, 0, 0, 0x23]),
            (b"rx_delay_sel", &[0, 0, 0, 0x1b]),
            (b"tx_delay_sel", &[0, 0, 0, 0x15]),
            (b"rxc_dly_en", ONE),
            (b"rgmii_sw_dr_rxc", &[0, 0, 0, 0xe]),
            (b"rgmii_sw_dr", &[0, 0, 0, 0xb]),
            (b"tx_inverted_10", ONE),
            (b"tx_inverted_100", ZERO),
        ];
        let f = Settings::FALLBACK_GMAC0;
        let (address, s) = vendor_phy(node(table)).unwrap();
        assert_eq!(address, 3);
        assert_eq!((s.rx_delay, s.tx_delay), (0xb, 0x5), "four-bit selects");
        assert!(s.rx_fixed_delay);
        assert_eq!(
            (s.rx_clock_drive, s.rx_data_drive),
            (Drive::Code(6), Drive::Code(3))
        );
        assert_eq!(s.tx_clock_inverted, [true, false, f.tx_clock_inverted[2]]);
        assert_eq!(s.source, Source::VendorTree);
        let table: &[(&[u8], &[u8])] = &[(b"rx_delay_sel", ZERO), (b"rxc_dly_en", ZERO)];
        let (address, s) = vendor_phy(node(table)).unwrap();
        assert_eq!(address, 0);
        assert!(!s.rx_fixed_delay);
        assert_eq!(
            (s.tx_delay, s.rx_clock_drive),
            (f.tx_delay, f.rx_clock_drive)
        );
        let table: &[(&[u8], &[u8])] = &[(b"reg", ONE)];
        assert_eq!(vendor_phy(node(table)).unwrap(), (0, f));
    }

    #[test]
    fn a_reader_error_reaches_the_caller() {
        let failing = |_: &[u8]| -> Result<Option<&'static [u8]>, u8> { Err(9) };
        assert_eq!(mainline_phy(failing), Err(9));
        assert_eq!(vendor_phy(failing), Err(9));
        let phy = (0, Settings::FALLBACK_GMAC0);
        assert_eq!(
            Discovered::from_node(0, 0, COMPATIBLE_MAINLINE, failing, phy),
            Err(9)
        );
    }

    #[test]
    fn the_first_cell_is_big_endian_and_needs_four_bytes() {
        assert_eq!(first_cell(&[0, 0, 1, 2, 9]), Some(0x102));
        assert_eq!(first_cell(&[0, 0, 1]), None);
    }

    #[test]
    fn the_syscon_falls_back_to_the_window_both_trees_name() {
        assert_eq!(
            Syscon::fallback(),
            Syscon {
                base: 0x1701_0000,
                size: 0x1000,
                from_tree: false
            }
        );
        assert_eq!(COMPATIBLES, [COMPATIBLE_MAINLINE, COMPATIBLE_VENDOR]);
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
}
