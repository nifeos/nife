//! Where the JH7110's two SD/MMC controllers are, and how each slot is wired, for
//! `designware_mobile_storage`.
//!
//! The controllers came with milestone 53 (the board's own peripherals: network and storage on
//! real silicon). The driver crate knows no system-on-chip, so every JH7110 fact it needs is here.
//!
//! The JH7110's two controllers, as its device trees describe them (read 2026-10-06 UTC). Mainline
//! Linux's `arch/riscv/boot/dts/starfive/jh7110.dtsi` has `mmc0: mmc@16010000` and
//! `mmc1: mmc@16020000`, `compatible = "starfive,jh7110-mmc"` (its binding includes
//! `synopsys-dw-mshc-common.yaml`), `interrupts` 74 and 75 and `fifo-depth = <32>`. The vendor
//! U-Boot tree the `VisionFive` 2's firmware hands over (`starfive-tech/u-boot`, tag `VF2_v2.10.4` at commit
//! `1ae835c25e3d`, and branch `JH7110_VisionFive2_devel` at `c4c67bb66ae6`, both fetched 2026-11-01
//! UTC) has `sdio0@16010000` and `sdio1@16020000`, `compatible = "snps,dw-mshc"`, and no
//! `interrupts`. Mainline is `torvalds/linux` at `cbbddb6b08cc`. On a
//! `VisionFive` 2 `mmc0` is the 8-bit eMMC socket and `mmc1` the 4-bit microSD slot. Mainline's
//! `jh7110-common.dtsi` and the vendor branch head's `starfive_visionfive2.dts` assign both a 50 megahertz
//! card clock source; the vendor tag `VF2_v2.10.4` assigns none.
//! The part is `DW_mshc` (Linux's `dw_mmc`), not the `SDHCI`-compatible `DWC_mshc`
//! (`sdhci-of-dwcmshc`).
//!
//! [`discover`] finds the node named for a slot in [`NODE_NAMES`] and hands [`Slot::from_node`] its
//! `reg` and its properties.
//!
//! Name: provisional, minted 2026-10-25 (UTC) by lane crates-stand-alone-cuts, after the crate it
//! serves. calef has not named it. The items from [`COMPATIBLE_MAINLINE`] to [`Slot`] keep the
//! names they had in the driver crate's `jh7110` module, which they left; those are provisional
//! too.

use device_tree_blob::{DeviceTreeBlob, Error, Region};

/// Mainline's `compatible`.
pub const COMPATIBLE_MAINLINE: &[u8] = b"starfive,jh7110-mmc";
/// The vendor tree's `compatible`, the generic binding.
pub const COMPATIBLE_VENDOR: &[u8] = b"snps,dw-mshc";

/// Slot 0's controller registers: the eMMC socket on a `VisionFive` 2. The vendor tree labels the
/// instance `sdio0`.
#[doc(alias = "sdio0")]
pub const SLOT_0_REGISTERS_PHYSICAL_ADDRESS: u64 = 0x1601_0000;
/// Slot 1's controller registers: the microSD slot on a `VisionFive` 2, the card the `VisionFive` 2 boots from.
/// The vendor tree labels the instance `sdio1`.
#[doc(alias = "sdio1")]
pub const SLOT_1_REGISTERS_PHYSICAL_ADDRESS: u64 = 0x1602_0000;
/// Each controller's register window, `0x10000` bytes in both trees.
pub const REGISTERS_SIZE_BYTES: u64 = 0x1_0000;
/// The rate mainline and the vendor branch head assign the `ciu` clock (the module documentation
/// gives the revisions; the vendor tag `VF2_v2.10.4` assigns none), which the controller divides
/// for the card.
pub const CARD_INTERFACE_UNIT_HERTZ: u32 = 50_000_000;
/// Both trees' `fifo-depth`: the FIFO's depth in entries of its width, which on the JH7110 (and in
/// every controller `designware_mobile_storage`'s `HostController::new` accepts)
/// is 32 bits, so a count of 32-bit words.
#[doc(alias = "fifo")]
#[doc(alias = "fifo-depth")]
pub const QUEUE_DEPTH_WORDS: u32 = 32;
/// Mainline's interrupt numbers at the platform-level interrupt controller (PLIC), indexed by slot:
/// slot 0 is 74 and slot 1 is 75.
pub const SLOT_INTERRUPT_NUMBERS: [u32; 2] = [74, 75];

/// The node names each slot goes by: mainline's first, then the vendor's.
pub const NODE_NAMES: [[&[u8]; 2]; 2] = [
    [b"mmc@16010000", b"sdio0@16010000"],
    [b"mmc@16020000", b"sdio1@16020000"],
];

/// One controller, as the tree describes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Slot {
    /// 0 or 1.
    pub index: u8,
    /// The register window's physical address, the address of the tree's `reg`.
    pub registers_physical_address: u64,
    /// The register window's size in bytes, the size of the tree's `reg`.
    pub registers_size_bytes: u64,
    /// The tree's `interrupts`, if it has one (the vendor tree does not).
    pub interrupt: Option<u32>,
    /// The tree's `bus-width`, 1 when absent.
    pub bus_width: u8,
    /// The tree's `fifo-depth`, if it has one, in 32-bit words (see [`QUEUE_DEPTH_WORDS`]).
    #[doc(alias = "fifo")]
    #[doc(alias = "fifo-depth")]
    pub queue_depth_words: Option<u32>,
    /// The tree's first `assigned-clock-rates` cell, or [`CARD_INTERFACE_UNIT_HERTZ`] when absent.
    pub card_interface_unit_hertz: u32,
    /// Which `compatible` matched.
    pub compatible: &'static [u8],
    /// `status` is `okay` or absent.
    pub status_okay: bool,
    /// `non-removable` is present (an eMMC socket, in the binding's terms).
    pub non_removable: bool,
}

impl Slot {
    /// What the slot is for on a `VisionFive` 2, for a transcript.
    #[must_use]
    pub const fn board_role(&self) -> &'static str {
        match self.index {
            0 => "eMMC socket",
            _ => "microSD slot",
        }
    }
}

fn big_endian_u32(bytes: &[u8]) -> Option<u32> {
    Some(u32::from_be_bytes(bytes.get(..4)?.try_into().ok()?))
}

impl Slot {
    /// **Slot `index`, from the node named for it**: `registers_physical_address` and
    /// `registers_size_bytes` are its `reg`, and
    /// `property(name)` is the node's property `name`. `None` when the node's `compatible` names
    /// neither binding, since a node at the slot's name under another binding is not this
    /// controller. An absent `bus-width` is 1, and an absent or zero `assigned-clock-rates` is
    /// the 50 megahertz that mainline and the vendor branch head assign.
    ///
    /// # Errors
    ///
    /// Propagates the first error `property` returns.
    pub fn from_node<'t, E>(
        index: u8,
        registers_physical_address: u64,
        registers_size_bytes: u64,
        mut property: impl FnMut(&[u8]) -> Result<Option<&'t [u8]>, E>,
    ) -> Result<Option<Slot>, E> {
        let Some(compatible_strings) = property(b"compatible")? else {
            return Ok(None);
        };
        let compatible = if compatible_strings
            .split(|&b| b == 0)
            .any(|s| s == COMPATIBLE_MAINLINE)
        {
            COMPATIBLE_MAINLINE
        } else if compatible_strings
            .split(|&b| b == 0)
            .any(|s| s == COMPATIBLE_VENDOR)
        {
            COMPATIBLE_VENDOR
        } else {
            return Ok(None);
        };
        let status_okay =
            property(b"status")?.is_none_or(|v| matches!(v, b"okay\0" | b"okay" | b"ok\0" | b"ok"));
        let bus_width = property(b"bus-width")?
            .and_then(big_endian_u32)
            .map_or(1, |w| u8::try_from(w).unwrap_or(1));
        Ok(Some(Slot {
            index,
            registers_physical_address,
            registers_size_bytes,
            interrupt: property(b"interrupts")?.and_then(big_endian_u32),
            bus_width,
            queue_depth_words: property(b"fifo-depth")?.and_then(big_endian_u32),
            card_interface_unit_hertz: property(b"assigned-clock-rates")?
                .and_then(big_endian_u32)
                .filter(|&hertz| hertz != 0)
                .unwrap_or(CARD_INTERFACE_UNIT_HERTZ),
            compatible,
            status_okay,
            non_removable: property(b"non-removable")?.is_some(),
        }))
    }
}

/// **Find slot `index` (0 or 1) in `tree`**, or `None` when the tree names no such controller
/// under either node name, or names one whose `compatible` is neither binding.
///
/// No constant fallback, unlike the clock and reset windows: a tree that does not describe the
/// controller is not a tree for a board this driver knows the wiring of, and guessing a bus width
/// for a slot nobody described is how a write lands on the wrong device.
///
/// # Errors
///
/// Propagates [`Error`] if the blob is malformed.
///
/// Name: recorded (milestone 53), which wrote it as `designware_mobile_storage::jh7110::discover`.
/// calef has not ruled on it.
pub fn discover(tree: &DeviceTreeBlob<'_>, index: u8) -> Result<Option<Slot>, Error> {
    let Some(names) = NODE_NAMES.get(usize::from(index)) else {
        return Ok(None);
    };
    for name in names {
        let mut reg = [Region { start: 0, size: 0 }; 1];
        if tree.node_reg(name, &mut reg)? == 0 {
            continue;
        }
        let prop = |p: &[u8]| tree.node_property(name, p);
        if let Some(slot) = Slot::from_node(index, reg[0].start, reg[0].size, prop)? {
            return Ok(Some(slot));
        }
    }
    Ok(None)
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
        let slot = Slot::from_node(
            1,
            SLOT_1_REGISTERS_PHYSICAL_ADDRESS,
            REGISTERS_SIZE_BYTES,
            node(table),
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            slot,
            Slot {
                index: 1,
                registers_physical_address: SLOT_1_REGISTERS_PHYSICAL_ADDRESS,
                registers_size_bytes: REGISTERS_SIZE_BYTES,
                interrupt: Some(75),
                bus_width: 4,
                queue_depth_words: Some(QUEUE_DEPTH_WORDS),
                card_interface_unit_hertz: 100_000_000,
                compatible: COMPATIBLE_MAINLINE,
                status_okay: true,
                non_removable: true,
            }
        );
        assert_eq!(slot.board_role(), "microSD slot");
    }

    #[test]
    fn a_vendor_node_with_nothing_but_its_binding_takes_the_defaults() {
        let table: &[(&[u8], &[u8])] = &[(b"compatible", b"snps,dw-mshc\0")];
        let slot = Slot::from_node(
            0,
            SLOT_0_REGISTERS_PHYSICAL_ADDRESS,
            REGISTERS_SIZE_BYTES,
            node(table),
        )
        .unwrap()
        .unwrap();
        assert_eq!(slot.compatible, COMPATIBLE_VENDOR);
        assert_eq!(
            (
                slot.bus_width,
                slot.card_interface_unit_hertz,
                slot.interrupt
            ),
            (1, CARD_INTERFACE_UNIT_HERTZ, None)
        );
        assert!(slot.status_okay && !slot.non_removable);
        assert_eq!(slot.board_role(), "eMMC socket");
        let table: &[(&[u8], &[u8])] = &[
            (b"compatible", b"snps,dw-mshc\0"),
            (b"assigned-clock-rates", &[0, 0, 0, 0]),
            (b"bus-width", &[0, 0, 1, 0]),
            (b"status", b"disabled\0"),
        ];
        let slot = Slot::from_node(0, 0, 0, node(table)).unwrap().unwrap();
        assert_eq!(
            (slot.card_interface_unit_hertz, slot.bus_width),
            (CARD_INTERFACE_UNIT_HERTZ, 1)
        );
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
