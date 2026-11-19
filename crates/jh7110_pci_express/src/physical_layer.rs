//! The PHY's words and port 0's PHY mode bits in the two system controllers.
//!
//! Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.

/// The PHY's phase-locked loop (PLL) `KVCO` level word.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
#[doc(alias = "PCIE_KVCO_LEVEL_OFF")]
pub const OSCILLATOR_GAIN_LEVEL: u32 = 0x28;
/// The value Linux writes to [`OSCILLATOR_GAIN_LEVEL`].
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
#[doc(alias = "PHY_KVCO_FINE_TUNE_LEVEL")]
pub const OSCILLATOR_GAIN_FINE_TUNE_LEVEL: u32 = 0x91;
/// The PHY's PLL `KVCO` tuning signal word.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
#[doc(alias = "PCIE_KVCO_TUNE_SIGNAL_OFF")]
pub const OSCILLATOR_GAIN_TUNE_SIGNAL: u32 = 0x80;
/// The value Linux writes to [`OSCILLATOR_GAIN_TUNE_SIGNAL`].
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
#[doc(alias = "PHY_KVCO_FINE_TUNE_SIGNALS")]
pub const OSCILLATOR_GAIN_FINE_TUNE_SIGNALS: u32 = 0xc;
/// The PHY's PLL control word.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
#[doc(alias = "PCIE_USB3_PHY_PLL_CTL_OFF")]
pub const PHASE_LOCKED_LOOP_CONTROL: u32 = 0x7c;
/// [`PHASE_LOCKED_LOOP_CONTROL`] and [`super::physical_layer::USB_SELECT`]: the USB 3 PHY is on. Cleared for PCI Express.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
#[doc(alias = "PCIE_USB3_PHY_ENABLE")]
pub const USB3_PHYSICAL_LAYER_ENABLE: u32 = 1 << 4;
/// Port 0's PHY mode word in the STG system controller (the tree's first `stg-syscon` cell).
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
pub const MODE: u32 = 0x148;
/// [`MODE`]: the mode field, bits 21:20, zero for PCI Express.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
#[doc(alias = "PCIE_PHY_MODE_MASK")]
pub const MODE_MASK: u32 = 0x3 << 20;
/// Port 0's USB selection word in the STG system controller (the second cell).
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
pub const USB_SELECT: u32 = 0x1f4;
/// [`USB_SELECT`]: the USB 3 bus width field, bits 3:2.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
#[doc(alias = "PCIE_USB3_BUS_WIDTH_MASK")]
pub const USB3_BUS_WIDTH_MASK: u32 = 0x3 << 2;
/// [`USB_SELECT`]: the bus width the PCI Express mode selects.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
#[doc(alias = "PCIE_USB3_BUS_WIDTH")]
pub const USB3_BUS_WIDTH: u32 = 1 << 3;
/// Port 0's PHY connection word in the SYS system controller (the tree's `sys-syscon` cell).
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
pub const CONNECT: u32 = 0x18;
/// [`CONNECT`]: split the USB power-down reset; cleared for both modes.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.
#[doc(alias = "USB_PDRSTN_SPLIT")]
pub const USB_POWER_DOWN_RESET_SPLIT: u32 = 1 << 17;
