//! The JH7110's two controllers, as its device trees describe them (read 2026-10-06 UTC). Mainline
//! Linux's `arch/riscv/boot/dts/starfive/jh7110.dtsi` has `mmc0: mmc@16010000` and
//! `mmc1: mmc@16020000`, `compatible = "starfive,jh7110-mmc"` (its binding includes
//! `synopsys-dw-mshc-common.yaml`), `interrupts` 74 and 75 and `fifo-depth = <32>`. The vendor
//! U-Boot tree radon's firmware hands over (`starfive-tech/u-boot`, `JH7110_VisionFive2_devel`) has
//! `sdio0@16010000` and `sdio1@16020000`, `compatible = "snps,dw-mshc"`, and no `interrupts`. On a
//! VisionFive 2 `mmc0` is the 8-bit eMMC socket and `mmc1` the 4-bit microSD slot, both at 50 MHz.
//! The part is `DW_mshc` (Linux's `dw_mmc`), not the SDHCI-compatible `DWC_mshc`
//! (`sdhci-of-dwcmshc`).

use device_tree_blob::{DeviceTreeBlob, Region};

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
const NAMES: [[&[u8]; 2]; 2] = [
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

/// **Find slot `index` (0 or 1) in `tree`**, or `None` when the tree names no such controller
/// under either spelling, or names one whose `compatible` is neither of the two above.
///
/// No constant fallback, unlike the clock and reset windows: a tree that does not describe the
/// controller is not a tree for a board this driver knows the wiring of, and guessing a bus width
/// for a slot nobody described is how a write lands on the wrong device.
///
/// # Errors
///
/// Propagates [`device_tree_blob::Error`] if the blob is malformed.
pub fn discover(
    tree: &DeviceTreeBlob<'_>,
    index: u8,
) -> Result<Option<Slot>, device_tree_blob::Error> {
    let Some(names) = NAMES.get(usize::from(index)) else {
        return Ok(None);
    };
    for name in names {
        let mut reg = [Region { start: 0, size: 0 }; 1];
        if tree.node_reg(name, &mut reg)? == 0 {
            continue;
        }
        let prop = |p: &[u8]| tree.node_prop(name, p);
        let Some(compat) = prop(b"compatible")? else {
            continue;
        };
        let compatible = if compat.split(|&b| b == 0).any(|s| s == COMPATIBLE_MAINLINE) {
            COMPATIBLE_MAINLINE
        } else if compat.split(|&b| b == 0).any(|s| s == COMPATIBLE_VENDOR) {
            COMPATIBLE_VENDOR
        } else {
            continue;
        };
        let status_okay =
            prop(b"status")?.is_none_or(|v| matches!(v, b"okay\0" | b"okay" | b"ok\0" | b"ok"));
        let bus_width = prop(b"bus-width")?
            .and_then(be32)
            .map_or(1, |w| u8::try_from(w).unwrap_or(1));
        return Ok(Some(Slot {
            index,
            base: reg[0].start,
            size: reg[0].size,
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
        }));
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    const VENDOR: &[u8] = include_bytes!("../tests/fixtures/jh7110-mmc-vendor.dtb");
    const MAINLINE: &[u8] = include_bytes!("../tests/fixtures/jh7110-mmc-mainline.dtb");

    #[test]
    fn radons_vendor_tree_names_both_slots_with_the_generic_binding_and_no_interrupt() {
        let tree = DeviceTreeBlob::from_bytes(VENDOR).unwrap();
        let sd = discover(&tree, 1).unwrap().unwrap();
        assert_eq!((sd.base, sd.size), (SDIO1_BASE, WINDOW_SIZE));
        assert_eq!(sd.compatible, COMPATIBLE_VENDOR);
        assert_eq!(sd.bus_width, 4);
        assert_eq!(sd.fifo_depth, Some(FIFO_DEPTH));
        assert_eq!(sd.ciu_hz, CIU_HZ);
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
        let tree =
            DeviceTreeBlob::from_bytes(include_bytes!("../tests/fixtures/jh7110-mmc-odd.dtb"))
                .unwrap();
        assert_eq!(discover(&tree, 0).unwrap(), None);
        assert_eq!(discover(&tree, 1).unwrap().unwrap().ciu_hz, 100_000_000);
    }
}
