//! The JH7110's two controllers, as its device trees describe them (read 2026-10-06 UTC). Mainline
//! Linux's `arch/riscv/boot/dts/starfive/jh7110.dtsi` has `mmc0: mmc@16010000` and
//! `mmc1: mmc@16020000`, `compatible = "starfive,jh7110-mmc"` (its binding includes
//! `synopsys-dw-mshc-common.yaml`), `interrupts` 74 and 75 and `fifo-depth = <32>`. The vendor
//! U-Boot tree radon's firmware hands over (`starfive-tech/u-boot`, `JH7110_VisionFive2_devel`) has
//! `sdio0@16010000` and `sdio1@16020000`, `compatible = "snps,dw-mshc"`, and no `interrupts`. On a
//! VisionFive 2 `mmc0` is the 8-bit eMMC socket and `mmc1` the 4-bit microSD slot, both at 50 MHz.
//! The part is `DW_mshc` (Linux's `dw_mmc`), not the SDHCI-compatible `DWC_mshc`
//! (`sdhci-of-dwcmshc`).
//!
//! The crate reads no device tree. The caller finds the node named for a slot in [`NODE_NAMES`]
//! and hands [`Slot::from_node`] its `reg` and its properties.

/// Mainline's `compatible`.
pub const COMPATIBLE_MAINLINE: &[u8] = b"starfive,jh7110-mmc";
/// The vendor tree's `compatible`, the generic binding.
pub const COMPATIBLE_VENDOR: &[u8] = b"snps,dw-mshc";

/// Slot 0's controller: the eMMC socket on a VisionFive 2.
pub const SDIO0_BASE: u64 = 0x1601_0000;
/// Slot 1's controller: the microSD slot, the card radon boots from.
pub const SDIO1_BASE: u64 = 0x1602_0000;
/// Each controller's window, `0x10000` in both trees.
pub const WINDOW_SIZE: u64 = 0x1_0000;
/// The rate both trees assign the `ciu` clock, which the controller divides for the card.
pub(crate) const CIU_HZ: u32 = 50_000_000;
/// Both trees' `fifo-depth`.
pub const FIFO_DEPTH: u32 = 32;
/// Mainline's interrupt numbers at the PLIC, slot 0 and slot 1.
pub const INTERRUPTS: [u32; 2] = [74, 75];

/// The node names each slot goes by: mainline's first, then the vendor's.
///
/// Name: provisional, minted 2026-10-25 (UTC) by lane crates-stand-alone-cuts, when the device-tree
/// walk and the process layout left the driver crates.
pub const NODE_NAMES: [[&[u8]; 2]; 2] = [
    [b"mmc@16010000", b"sdio0@16010000"],
    [b"mmc@16020000", b"sdio1@16020000"],
];

/// One controller, as the tree describes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Slot {
    /// 0 or 1.
    pub index: u8,
    /// The window's physical base.
    pub base: u64,
    /// The window's size.
    pub size: u64,
    /// The tree's `interrupts`, if it has one (the vendor tree does not).
    pub interrupt: Option<u32>,
    /// The tree's `bus-width`, 1 when absent.
    pub bus_width: u8,
    /// The tree's `fifo-depth`, if it has one.
    pub fifo_depth: Option<u32>,
    /// The tree's first `assigned-clock-rates` cell, or [`CIU_HZ`] when absent.
    pub ciu_hz: u32,
    /// Which `compatible` matched.
    pub compatible: &'static [u8],
    /// `status` is `okay` or absent.
    pub status_okay: bool,
    /// `non-removable` is present (an eMMC socket, in the binding's terms).
    pub non_removable: bool,
}

impl Slot {
    /// What the slot is for on a VisionFive 2, for a transcript.
    #[must_use]
    pub const fn role(&self) -> &'static str {
        match self.index {
            0 => "eMMC socket",
            _ => "microSD slot",
        }
    }
}

fn be32(bytes: &[u8]) -> Option<u32> {
    Some(u32::from_be_bytes(bytes.get(..4)?.try_into().ok()?))
}

impl Slot {
    /// **Slot `index`, from the node named for it**: `base` and `size` are its `reg`, and
    /// `prop(name)` is the node's property `name`. `None` when the node's `compatible` names
    /// neither binding, since a node at the slot's name under another binding is not this
    /// controller. An absent `bus-width` is 1, and an absent or zero `assigned-clock-rates` is
    /// the 50 MHz both trees assign.
    ///
    /// # Errors
    ///
    /// Propagates the first error `prop` returns.
    ///
    /// Name: provisional, minted 2026-10-25 (UTC) by lane crates-stand-alone-cuts, when the device-
    /// tree walk and the process layout left the driver crates.
    pub fn from_node<'t, E>(
        index: u8,
        base: u64,
        size: u64,
        mut prop: impl FnMut(&[u8]) -> Result<Option<&'t [u8]>, E>,
    ) -> Result<Option<Slot>, E> {
        let Some(compat) = prop(b"compatible")? else {
            return Ok(None);
        };
        let compatible = if compat.split(|&b| b == 0).any(|s| s == COMPATIBLE_MAINLINE) {
            COMPATIBLE_MAINLINE
        } else if compat.split(|&b| b == 0).any(|s| s == COMPATIBLE_VENDOR) {
            COMPATIBLE_VENDOR
        } else {
            return Ok(None);
        };
        let status_okay =
            prop(b"status")?.is_none_or(|v| matches!(v, b"okay\0" | b"okay" | b"ok\0" | b"ok"));
        let bus_width = prop(b"bus-width")?
            .and_then(be32)
            .map_or(1, |w| u8::try_from(w).unwrap_or(1));
        Ok(Some(Slot {
            index,
            base,
            size,
            interrupt: prop(b"interrupts")?.and_then(be32),
            bus_width,
            fifo_depth: prop(b"fifo-depth")?.and_then(be32),
            ciu_hz: prop(b"assigned-clock-rates")?
                .and_then(be32)
                .filter(|&hz| hz != 0)
                .unwrap_or(CIU_HZ),
            compatible,
            status_okay,
            non_removable: prop(b"non-removable")?.is_some(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A property table standing in for one node.
    fn node<'t>(
        table: &'t [(&'static [u8], &'t [u8])],
    ) -> impl FnMut(&[u8]) -> Result<Option<&'t [u8]>, ()> + 't {
        move |name| Ok(table.iter().find(|(n, _)| *n == name).map(|(_, v)| *v))
    }

    #[test]
    fn a_mainline_node_reads_every_property() {
        let table: &[(&[u8], &[u8])] = &[
            (b"compatible", b"starfive,jh7110-mmc\0"),
            (b"interrupts", &[0, 0, 0, 75]),
            (b"bus-width", &[0, 0, 0, 4]),
            (b"fifo-depth", &[0, 0, 0, 32]),
            (b"assigned-clock-rates", &100_000_000u32.to_be_bytes()),
            (b"status", b"okay\0"),
            (b"non-removable", b""),
        ];
        let slot = Slot::from_node(1, SDIO1_BASE, WINDOW_SIZE, node(table))
            .unwrap()
            .unwrap();
        assert_eq!(
            slot,
            Slot {
                index: 1,
                base: SDIO1_BASE,
                size: WINDOW_SIZE,
                interrupt: Some(75),
                bus_width: 4,
                fifo_depth: Some(FIFO_DEPTH),
                ciu_hz: 100_000_000,
                compatible: COMPATIBLE_MAINLINE,
                status_okay: true,
                non_removable: true,
            }
        );
        assert_eq!(slot.role(), "microSD slot");
    }

    #[test]
    fn a_vendor_node_with_nothing_but_its_binding_takes_the_defaults() {
        let table: &[(&[u8], &[u8])] = &[(b"compatible", b"snps,dw-mshc\0")];
        let slot = Slot::from_node(0, SDIO0_BASE, WINDOW_SIZE, node(table))
            .unwrap()
            .unwrap();
        assert_eq!(slot.compatible, COMPATIBLE_VENDOR);
        assert_eq!(
            (slot.bus_width, slot.ciu_hz, slot.interrupt),
            (1, CIU_HZ, None)
        );
        assert!(slot.status_okay && !slot.non_removable);
        assert_eq!(slot.role(), "eMMC socket");
        let table: &[(&[u8], &[u8])] = &[
            (b"compatible", b"snps,dw-mshc\0"),
            (b"assigned-clock-rates", &[0, 0, 0, 0]),
            (b"bus-width", &[0, 0, 1, 0]),
            (b"status", b"disabled\0"),
        ];
        let slot = Slot::from_node(0, 0, 0, node(table)).unwrap().unwrap();
        assert_eq!((slot.ciu_hz, slot.bus_width), (CIU_HZ, 1));
        assert!(!slot.status_okay);
    }

    #[test]
    fn a_node_under_another_binding_or_none_is_not_this_controller() {
        let table: &[(&[u8], &[u8])] = &[(b"compatible", b"vendor,other\0")];
        assert_eq!(Slot::from_node(0, 0, 0, node(table)).unwrap(), None);
        assert_eq!(Slot::from_node(0, 0, 0, node(&[])).unwrap(), None);
        let failing = |_: &[u8]| -> Result<Option<&'static [u8]>, u8> { Err(4) };
        assert_eq!(Slot::from_node(0, 0, 0, failing), Err(4));
    }

    #[test]
    fn each_slot_is_named_mainline_first() {
        assert_eq!(NODE_NAMES[0], [&b"mmc@16010000"[..], b"sdio0@16010000"]);
        assert_eq!(NODE_NAMES[1], [&b"mmc@16020000"[..], b"sdio1@16020000"]);
    }
}
