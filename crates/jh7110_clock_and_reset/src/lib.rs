#![cfg_attr(not(test), no_std)]
//! **The `StarFive` JH7110's clock and reset generator, as pure logic** (milestone 220; roadmap
//! `design/roadmap/0220-jh7110-clock-and-reset.md`).
//!
//! Register offsets, bit positions, the bring-up plan the TRNG needs, and the device-tree query
//! that finds the controller, with nothing an actual driver touches. The volatile shell is
//! `kernel/src/drivers/jh7110_clock_and_reset.rs`; this crate never dereferences a pointer, which is what
//! makes it host-testable and Kani-reachable, the same rule-7 split `jh7110_entropy` and `pci`
//! already use.
//!
//! # Why this exists at all, and it is a measurement rather than an inference
//!
//! Every device nife has driven came up already running: QEMU's virtio devices, the PL011 and
//! NS16550 consoles, the PLIC. Real `SoC` peripherals do not. On **2026-09-04** radon (the
//! `StarFive` VisionFive 2) booted milestone 159's confined userspace TRNG driver twice,
//! byte-identically, and the register window read as nothing:
//!
//! ```text
//! hw entropy  : FAILED: JH7110 TRNG at 0x1600c000 (tree says starfive,trng, status disabled):
//!               report 0x524e475550, bring-up diagnostic 0x0000000000000000,
//!               draws 0/0 bytes, first-all-zero true, draws-differ false
//! ```
//!
//! Transcript: `target/board/radon-2026-09-04-trng-bringup.log`. The all-zero diagnostic is the
//! raw `(STAT << 32) | ISTAT`, so the whole register file read back as zeros; the device tree
//! independently marks the node `status disabled`. Two signals, and they agree.
//!
//! # Sources, fetched rather than recalled
//!
//! Every number below appears in **two independent published trees** that describe the same
//! silicon, and they agree. That agreement is the reason this crate is willing to carry a
//! constant at all; see [`STG`]'s own note. The full quoted listings (both device trees, both
//! drivers, the binding headers) are in this section's git history and in the linked sources.
//!
//! - **[mainline-dts]** Linux's `jh7110.dtsi`: the TRNG node wired to `stgcrg` clocks
//!   `JH7110_STGCLK_SEC_AHB` and `JH7110_STGCLK_SEC_MISC_AHB` ("hclk", "ahb") and reset
//!   `JH7110_STGRST_SEC_AHB`; `stgcrg` is `starfive,jh7110-stgcrg` at `0x1023_0000`, 0x10000
//!   wide.
//! - **[mainline-trng]** Linux's `jh7110-trng.c`: the probe order [`TRNG_BRING_UP`] reproduces,
//!   two `clk_prepare_enable` calls then `reset_control_deassert`.
//! - **[mainline-ids]** the binding headers: `JH7110_STGCLK_SEC_AHB 15`,
//!   `JH7110_STGCLK_SEC_MISC_AHB 16`, `JH7110_STGRST_SEC_AHB 3`, `JH7110_STGRST_END 23`.
//! - **[mainline-clk]** `clk-starfive-jh71x0.c`: one 32-bit register per clock,
//!   `base + 4 * idx`, enable bit `BIT(31)`.
//! - **[mainline-rst]** `reset-starfive-jh7110.c`: the STG block asserts at `0x74`, reads status
//!   at `0x78`, and the JH7110 passes `asserted = NULL`, which inverts the poll's sense:
//!   **a set status bit means the line is out of reset.** That inversion is
//!   [`is_deasserted`]'s whole reason to exist.
//! - **[vendor-dts]** and **[vendor-ids]** `starfive-tech/u-boot`, the `JH7110_VisionFive2_devel`
//!   branch, which is the firmware radon actually runs. It spells the same wiring differently
//!   (`starfive,trng`, `clkgen`, `rstgen`, the TRNG node `status disabled`), and its numbers are
//!   **flat across all the domains**, so they must be rebased before they mean anything:
//!   `JH7110_SEC_HCLK 205` against a stg group starting at 190 gives 15, and
//!   `JH7110_SEC_MISCAHB_CLK 206` gives 16; `RSTN_U0_SEC_TOP_HRESETN 131` against a group
//!   starting at 128 gives 3.
//!
//! **So the two trees converge**: 15, 16, 3, in the STG domain at `0x1023_0000`. The vendor
//! spellings look nothing like mainline's, and a reader who checked only one would reasonably
//! fear the driver was written against the wrong chip.
//!
//! # This has not run against real silicon
//!
//! **Nothing here has been verified against a JH7110.** QEMU's riscv64 `virt` machine has no
//! clock or reset controller of any kind, so an emulator cannot validate the sequence end to end:
//! what CI exercises is the absence path and the arithmetic, never a device answering. The bench
//! procedure that would settle it, with a table mapping each observable outcome to what it means,
//! is `notes/jh7110-clock-and-reset.md`.
//!
//! Name: ratified 2026-09-13 (calef, working the unratified worklist), replacing the provisional
//! `jh7110_crg`, and kept as the crates.io name on 2026-10-07 (#1806). The refused names
//! (`jh7110_clock`, `jh7110_clkgen`, `jh7110_clock_and_reset_generator`), the argument that lost
//! over `crg`, and the vendor-spelling evidence are in git history; milestone 819 (JH7110 clock
//! and reset logic, proven, then released on its own) publishes it.
//! # Examples
//!
//! ```
//! use jh7110_clock_and_reset::{STG, Step, TRNG_BRING_UP, CLOCK_ENABLE, is_deasserted};
//!
//! // The TRNG's plan is two clocks then one reset, in that order, which is the order
//! // Linux's own probe takes: a reset deassert against a gated clock can hang forever.
//! assert_eq!(
//!     TRNG_BRING_UP,
//!     &[Step::EnableClock(15), Step::EnableClock(16), Step::DeassertReset(3)]
//! );
//!
//! // Clock 15 lives one word per clock from the domain's base.
//! assert_eq!(STG.clock_offset(15), Some(0x3c));
//! assert_eq!(CLOCK_ENABLE, 1 << 31);
//!
//! // Reset 3 is bit 3 of the word at 0x74, watched at 0x78. A SET status bit means
//! // "out of reset", which is the opposite of what the register name suggests.
//! let r = STG.reset_bit(3).unwrap();
//! assert_eq!((r.assert_offset, r.status_offset, r.mask), (0x74, 0x78, 1 << 3));
//! assert!(is_deasserted(0b1000, r.mask));
//! assert!(!is_deasserted(0b0111, r.mask));
//! ```
//!
//! [mainline-dts]: https://github.com/torvalds/linux/blob/master/arch/riscv/boot/dts/starfive/jh7110.dtsi
//! [mainline-trng]: https://github.com/torvalds/linux/blob/master/drivers/char/hw_random/jh7110-trng.c
//! [mainline-ids]: https://github.com/torvalds/linux/blob/master/include/dt-bindings/reset/starfive%2Cjh7110-crg.h
//! [mainline-clk]: https://github.com/torvalds/linux/blob/master/drivers/clk/starfive/clk-starfive-jh71x0.c
//! [mainline-rst]: https://github.com/torvalds/linux/blob/master/drivers/reset/starfive/reset-starfive-jh7110.c
//! [vendor-dts]: https://github.com/starfive-tech/u-boot/blob/JH7110_VisionFive2_devel/arch/riscv/dts/jh7110.dtsi
//! [vendor-ids]: https://github.com/starfive-tech/u-boot/blob/JH7110_VisionFive2_devel/include/dt-bindings/clock/starfive-jh7110-clkgen.h

/// The bit that turns a clock on, in every one of this controller's per-clock words
/// (\[mainline-clk\], `#define JH71X0_CLK_ENABLE BIT(31)`).
pub const CLOCK_ENABLE: u32 = 1 << 31;

/// A single step of a device's bring-up sequence, in the order it must be taken.
///
/// Deliberately a plan rather than a pair of methods on a driver: the order is the load-bearing
/// part (\[mainline-trng\] enables both clocks *before* deasserting, and \[mainline-rst\]'s own
/// comment says why: *"if the associated clock is gated, deasserting might otherwise hang
/// forever"*), and a plan is the only shape that lets a host test assert on the order without a
/// device. The kernel driver walks this slice; it does not know which device it is bringing up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Set [`CLOCK_ENABLE`] in the domain's word for this clock index.
    EnableClock(u32),
    /// Clear this reset's bit in the domain's assert word, then wait for its status bit to read
    /// as [`is_deasserted`].
    DeassertReset(u32),
    /// **Choose a multiplexed clock's parent**: replace the [`CLOCK_MUX_MASK`] field of this
    /// clock's word with `parent`, as \[mainline-clk\]'s `jh71x0_clk_set_parent` writes
    /// `index << JH71X0_CLK_MUX_SHIFT` under `JH71X0_CLK_MUX_MASK` (milestone 53 (the board's own
    /// peripherals: network and storage on real silicon)). Taken before the same clock's
    /// `EnableClock`, so the switch happens while the clock is gated rather than glitching a
    /// running one.
    SelectParent {
        /// The clock index in the domain.
        clock: u32,
        /// The parent's position in that clock's parent list.
        parent: u32,
    },
}

/// A multiplexed clock's parent field, bits 27:24 of its word (\[mainline-clk\]'s
/// `JH71X0_CLK_MUX_MASK`, `GENMASK(27, 24)`).
pub const CLOCK_MUX_MASK: u32 = 0xf << CLOCK_MUX_SHIFT;

/// Where [`CLOCK_MUX_MASK`]'s field starts (`JH71X0_CLK_MUX_SHIFT`).
pub const CLOCK_MUX_SHIFT: u32 = 24;

/// **A clock word with its parent replaced**, leaving the enable bit, the divider and every other
/// field as they were. A parent too wide for the field is reduced to it rather than spilling into
/// the enable bit.
#[must_use]
pub const fn with_parent(word: u32, parent: u32) -> u32 {
    (word & !CLOCK_MUX_MASK) | ((parent << CLOCK_MUX_SHIFT) & CLOCK_MUX_MASK)
}

/// **What the JH7110's TRNG needs before its registers answer**, transcribed from
/// \[mainline-trng\]'s probe and cross-checked against \[vendor-dts\] (see the module header:
/// both trees name clocks 15 and 16 and reset 3 of the STG domain, in different spellings).
///
/// All three identifiers are STG-domain, so this slice is meaningless without [`STG`]; that
/// coupling is why there is no `Domain` field on `Step`. A second device in a second domain would
/// carry its own plan and its own domain beside it, and the day that happens is the day to decide
/// whether the pair wants a type.
pub const TRNG_BRING_UP: &[Step] = &[
    Step::EnableClock(STGCLK_SEC_AHB),
    Step::EnableClock(STGCLK_SEC_MISC_AHB),
    Step::DeassertReset(STGRST_SEC_AHB),
];

/// `JH7110_STGCLK_SEC_AHB` \[mainline-ids\]; the vendor tree's `JH7110_SEC_HCLK` (205) rebased on
/// its stg group's start (190). The TRNG's `hclk`.
pub const STGCLK_SEC_AHB: u32 = 15;

/// `JH7110_STGCLK_SEC_MISC_AHB` \[mainline-ids\]; the vendor tree's `JH7110_SEC_MISCAHB_CLK` (206)
/// rebased the same way. The TRNG's second clock, `ahb` to mainline and `miscahb_clk` to the
/// vendor, which is the same wire under two names.
pub const STGCLK_SEC_MISC_AHB: u32 = 16;

/// `JH7110_STGRST_SEC_AHB` \[mainline-ids\]; the vendor tree's `RSTN_U0_SEC_TOP_HRESETN` (131)
/// rebased on its stg group's start (128).
///
/// **It is a *shared* reset** (\[mainline-trng\] takes it with `devm_reset_control_get_shared`),
/// which is a fact about the silicon rather than about Linux's API: the same line resets the whole
/// security top block, the PL080 DMA at `0x1600_8000` included. Deasserting it is safe; asserting
/// it would reset a neighbour, which is why nothing in this crate offers an assert.
pub const STGRST_SEC_AHB: u32 = 3;

/// **The AON (always-on) domain**, where `gmac0`'s bus clocks, its transmit clock mux and both
/// of its resets live (milestone 53, the JH7110's Ethernet).
///
/// Offsets from \[mainline-rst\]'s `jh7110_aon_info` (`.assert_offset = 0x38, .status_offset =
/// 0x3C`, read 2026-10-06); the counts from \[mainline-ids\]'s `JH7110_AONRST_END` (8) and
/// `JH7110_AONCLK_END` (14).
pub const AON: Domain = Domain {
    reset_assert: 0x38,
    reset_status: 0x3c,
    resets: 8,
    clocks: 14,
};

/// **The AON domain's register window**, `0x1700_0000`, size `0x1_0000`: mainline's `aoncrg`
/// node and `reg-names` entry `"aon"`/`"aoncrg"` of both vendor nodes. The fallback when a tree
/// names none of them, for [`STG_BASE`]'s reason.
pub const AON_BASE: u64 = 0x1700_0000;

/// The AON window's size, `0x10000` in every tree that names it.
pub const AON_SIZE: u64 = 0x1_0000;

/// Mainline's dedicated AON clock-and-reset controller node, `aoncrg: clock-controller@17000000`.
pub const COMPATIBLE_AONCRG: &[u8] = b"starfive,jh7110-aoncrg";

/// The `reg-names` entry naming the AON window in each vendor node's spelling (`"aon"` in
/// `clkgen`, `"aoncrg"` in `rstgen`; \[vendor-dts\]).
const VENDOR_CLKGEN_AON_NAME: &[u8] = b"aon";
const VENDOR_RSTGEN_AON_NAME: &[u8] = b"aoncrg";

/// `JH7110_AONCLK_GMAC0_AHB` \[mainline-ids\]: a gate, `gmac0`'s `pclk`. Word `0x08`.
pub const AONCLK_GMAC0_AHB: u32 = 2;
/// `JH7110_AONCLK_GMAC0_AXI`: a gate, `gmac0`'s `stmmaceth` (its CSR and bus clock). Word `0x0c`.
pub const AONCLK_GMAC0_AXI: u32 = 3;
/// `JH7110_AONCLK_GMAC0_TX`: a gate with a two-way mux, `gmac0_gtxclk` (0) or
/// `gmac0_rmii_rtx` (1). Word `0x14`, which radon's vendor U-Boot calls `GMAC5_0_CLK_TX_SHIFT`
/// and sets bit 24 of (`jh7110_gmac_sel_tx_to_rgmii`, `board/starfive/visionfive2/`).
pub const AONCLK_GMAC0_TX: u32 = 5;
/// `gmac0_tx`'s parent on a VisionFive 2 v1.3B: `gmac0_rmii_rtx`, mainline's
/// `assigned-clock-parents` for `&gmac0` in `jh7110-starfive-visionfive-2-v1.3b.dts` beside
/// `starfive,tx-use-rgmii-clk`, and the same bit the vendor U-Boot sets. radon's EEPROM says PCB
/// revision `0xb2`, a v1.3B (bench transcripts, `PCB revision: 0xb2`).
pub const AONCLK_GMAC0_TX_PARENT_RMII_RTX: u32 = 1;
/// `JH7110_AONRST_GMAC0_AXI` \[mainline-ids\], `gmac0`'s `stmmaceth` reset. Bit 0 at `0x38`.
pub const AONRST_GMAC0_AXI: u32 = 0;
/// `JH7110_AONRST_GMAC0_AHB`, `gmac0`'s `ahb` reset. Bit 1 at `0x38`.
pub const AONRST_GMAC0_AHB: u32 = 1;

/// `JH7110_SYSCLK_GMAC0_GTXCLK` \[mainline-ids\]: a gated divider of PLL0, the parent of
/// `gmac0_gtxc`. Word `0x1b0` of the SYS window.
pub const SYSCLK_GMAC0_GTXCLK: u32 = 108;
/// `JH7110_SYSCLK_GMAC0_PTP`: a gated divider, `gmac0`'s `ptp_ref`. Word `0x1b4`.
pub const SYSCLK_GMAC0_PTP: u32 = 109;
/// `JH7110_SYSCLK_GMAC0_GTXC`: a gate, `gmac0`'s `gtx`. Word `0x1bc`.
pub const SYSCLK_GMAC0_GTXC: u32 = 111;

/// **`gmac0`'s SYS-domain clocks**, the half of its clocks that are not in the AON domain
/// (milestone 53). Every clock mainline's `&gmac0` node names that lives in SYS, with the parent
/// of `gtxc` first because enabling a gate under a gated parent turns nothing on. Run before
/// [`GMAC0_AON_BRING_UP`], whose resets must be released with every clock already running.
pub const GMAC0_SYS_BRING_UP: &[Step] = &[
    Step::EnableClock(SYSCLK_GMAC0_GTXCLK),
    Step::EnableClock(SYSCLK_GMAC0_GTXC),
    Step::EnableClock(SYSCLK_GMAC0_PTP),
];

/// **`gmac0`'s AON-domain clocks, its transmit clock's parent, and both its resets** (milestone
/// 53), in Linux's probe order: OpenBSD's `dwqe_fdt_attach` and mainline's `dwmac-starfive.c`
/// both enable every clock before deasserting either reset. The transmit clock's inverted twin
/// (`gmac0_tx_inv`, index 6) and both receive clocks (7, 8) have no gate to enable (they are
/// \[mainline-clk\] `JH71X0__INV` and `JH71X0__MUX` clocks), so they are not steps.
pub const GMAC0_AON_BRING_UP: &[Step] = &[
    Step::EnableClock(AONCLK_GMAC0_AXI),
    Step::EnableClock(AONCLK_GMAC0_AHB),
    Step::SelectParent {
        clock: AONCLK_GMAC0_TX,
        parent: AONCLK_GMAC0_TX_PARENT_RMII_RTX,
    },
    Step::EnableClock(AONCLK_GMAC0_TX),
    Step::DeassertReset(AONRST_GMAC0_AXI),
    Step::DeassertReset(AONRST_GMAC0_AHB),
];

/// Where one reset lives: which word to write, which word to watch, and which bit in both.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResetBit {
    /// Byte offset from the domain's base of the word whose bit is written to assert or deassert.
    pub assert_offset: u64,
    /// Byte offset from the domain's base of the word whose bit reports what the hardware did.
    pub status_offset: u64,
    /// The bit, in both words.
    pub mask: u32,
}

/// One clock-and-reset domain of the JH7110: a register window, and where the resets sit in it.
///
/// Three exist ([vendor-dts]'s `reg-names = "syscrg", "stgcrg", "aoncrg", "ispcrg", "voutcrg"`
/// names five), and only [`STG`] is described here, deliberately. The milestone's own `BUGS`
/// section named unbounded scope as its main risk: "a clock and reset driver for the JH7110" could
/// mean the one clock the TRNG needs or the whole controller, and those differ by an order of
/// magnitude. This is the first, with the arithmetic general enough that the second is a table
/// rather than a rewrite.
///
/// [vendor-dts]: https://github.com/starfive-tech/u-boot/blob/JH7110_VisionFive2_devel/arch/riscv/dts/jh7110.dtsi
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Domain {
    /// Byte offset from the base of the first reset-assert word.
    pub reset_assert: u64,
    /// Byte offset from the base of the first reset-status word.
    pub reset_status: u64,
    /// How many resets this domain has. Bounds [`Domain::reset_bit`], so an out-of-range id
    /// cannot become a write to whatever word the arithmetic lands on.
    pub resets: u32,
    /// How many clocks this domain has, bounding [`Domain::clock_offset`] for the same reason.
    pub clocks: u32,
}

/// **The STG (system-transport-group) domain**, which is where the TRNG's clocks and reset live.
///
/// Offsets from \[mainline-rst\]'s `jh7110_stg_info`; the counts from \[mainline-ids\]'s
/// `JH7110_STGRST_END` (23) and `JH7110_STGCLK_END` (29).
pub const STG: Domain = Domain {
    reset_assert: 0x74,
    reset_status: 0x78,
    resets: 23,
    clocks: 29,
};

/// **The STG domain's register window, as both published trees give it.**
///
/// A constant address in a tree whose whole habit is to read addresses out of the device tree, so
/// it owes a reason. [`discover`] reads the tree first and falls back to this only when the tree
/// names no controller, and [`Found::from_tree`] says which happened, so nothing can quietly
/// mistake one for the other. It is here because the alternative is worse: radon's firmware tree
/// is already known to omit and misdescribe things (milestone 239; the same tree calls the S7 core
/// `okay` and gives it an MMU it does not have), and a bench session that comes back with "no
/// controller node, nothing attempted" has spent a trip to the machine and learned nothing. Two
/// independently published trees agree on this number, which is a stronger warrant than most
/// device-tree reads get.
pub const STG_BASE: u64 = 0x1023_0000;

/// The STG window's size, `0x10000` in both trees.
pub const STG_SIZE: u64 = 0x1_0000;

/// **The SYS domain** (milestone 592 (radon's cold reboot dies in OpenSBI's PMIC write), provisional), which is where I2C5's clock and reset live,
/// and so where the only road to radon's PMIC starts.
///
/// Offsets from \[mainline-rst\]'s `jh7110_sys_info` (`.assert_offset = 0x2F8, .status_offset =
/// 0x308`, fetched 2026-09-25); the counts from \[mainline-ids\]'s `JH7110_SYSRST_END` (126) and
/// `JH7110_SYSCLK_END` (190). The vendor header radon's U-Boot was built from agrees on the clock
/// count, as `JH7110_CLK_SYS_REG_END 190`, which is the boundary between clocks that have a
/// register and the vendor's virtual ones numbered above it.
pub const SYS: Domain = Domain {
    reset_assert: 0x2f8,
    reset_status: 0x308,
    resets: 126,
    clocks: 190,
};

/// **The SYS domain's register window**, `0x1302_0000`, size `0x1_0000`, in mainline's `syscrg`
/// node and as `reg-names` entry `"sys"`/`"syscrg"` of both vendor nodes. The fallback when a tree
/// names none of them, for [`STG_BASE`]'s reason.
pub const SYS_BASE: u64 = 0x1302_0000;

/// The SYS window's size, `0x10000` in every tree that names it.
pub const SYS_SIZE: u64 = 0x1_0000;

/// `JH7110_SYSCLK_I2C5_APB` \[mainline-ids\], and `JH7110_I2C5_CLK_APB` in the vendor header,
/// both **143**: the vendor numbers the SYS group first, so no rebase is needed. Word `0x23c`.
///
/// **This is I2C5's only gate.** Radon's U-Boot also names a `u5_dw_i2c_clk_core` (vendor id 298)
/// and prints it at `Starting kernel`, but its own clock driver registers that as
/// `starfive_clk_fix_factor(..., "u5_dw_i2c_clk_core", "u5_dw_i2c_clk_apb", 1, 1)`, a
/// divide-by-one child of this gate with no register of its own, and mainline's `DesignWare` node
/// names this clock alone. So there is no second gate to turn on, and 298 is not a word to write.
pub const SYSCLK_I2C5_APB: u32 = 143;

/// `JH7110_SYSRST_I2C5_APB` \[mainline-ids\], and `RSTN_U5_DW_I2C_APB` in the vendor header, both
/// **81**. Bit 17 of the word at `0x300`, watched at `0x310`.
///
/// **This is the line the proposal did not name, and probably the cause.** Radon's U-Boot removes
/// its I2C driver before handing over (`DM_FLAG_OS_PREPARE`), and `designware_i2c_remove` ends in
/// `reset_release_bulk`, whose `reset_release_all` *asserts* every reset before freeing it. Radon's
/// OpenSBI re-enables a clock before its I2C transfer and never touches a reset. A controller held
/// in reset reads `IC_STATUS` as zero, so its transmit-FIFO-empty poll can never succeed, which is
/// the ten `i2c read: write daddr 36 to` lines in radon's 2026-09-04 log. See
/// `design/roadmap/0592-radons-reboot-dies-in-opensbis-pmic-write.md` for every source.
pub const SYSRST_I2C5_APB: u32 = 81;

/// **What I2C5 needs before OpenSBI can reach the PMIC**, when the tree does not say (milestone
/// 592). Clock first, then reset, for [`TRNG_BRING_UP`]'s reason: Linux's reset driver warns that
/// a deassert against a gated clock "might otherwise hang forever".
pub const PMIC_BUS_BRING_UP: &[Step] = &[
    Step::EnableClock(SYSCLK_I2C5_APB),
    Step::DeassertReset(SYSRST_I2C5_APB),
];

/// `JH7110_SYSCLK_SDIO0_AHB` \[mainline-ids\], the vendor header's `JH7110_SDIO0_CLK_AHB`, both
/// **91**: the eMMC socket's controller's bus clock, `biu` in both trees (milestone 53 (the
/// board's own peripherals: network and storage on real silicon)). Word `0x16c`.
pub const SYSCLK_SDIO0_AHB: u32 = 91;
/// `JH7110_SYSCLK_SDIO1_AHB`, vendor `JH7110_SDIO1_CLK_AHB`, both **92**: the microSD slot's
/// controller's bus clock. Word `0x170`.
pub const SYSCLK_SDIO1_AHB: u32 = 92;
/// `JH7110_SYSCLK_SDIO0_SDCARD`, vendor `JH7110_SDIO0_CLK_SDCARD`, both **93**: the eMMC
/// controller's card clock source, `ciu`, a gated divider both trees assign 50 MHz. Word `0x174`.
pub const SYSCLK_SDIO0_SDCARD: u32 = 93;
/// `JH7110_SYSCLK_SDIO1_SDCARD`, vendor `JH7110_SDIO1_CLK_SDCARD`, both **94**: the microSD
/// controller's `ciu`. Word `0x178`.
pub const SYSCLK_SDIO1_SDCARD: u32 = 94;
/// `JH7110_SYSRST_SDIO0_AHB` \[mainline-ids\]: the eMMC controller's one reset. Bit 0 of the
/// word at `0x300`.
pub const SYSRST_SDIO0_AHB: u32 = 64;
/// `JH7110_SYSRST_SDIO1_AHB`: the microSD controller's one reset. Bit 1 of the word at `0x300`.
pub const SYSRST_SDIO1_AHB: u32 = 65;

/// **What each SD/MMC controller needs before its registers answer** (milestone 53), slot 0 (the
/// eMMC socket) then slot 1 (the microSD slot): both clocks mainline's node names, then its reset,
/// in [`TRNG_BRING_UP`]'s order and for its reason. OpenBSD's `dwmmc_attach` takes the same order
/// (`clock_enable_all`, then `reset_deassert_all`). Every step is idempotent, which matters here:
/// radon's U-Boot has already brought the microSD controller up to load the boot script, and the
/// bench step reads whether it left it that way before it walks the plan.
pub const SDIO_BRING_UP: [&[Step]; 2] = [
    &[
        Step::EnableClock(SYSCLK_SDIO0_AHB),
        Step::EnableClock(SYSCLK_SDIO0_SDCARD),
        Step::DeassertReset(SYSRST_SDIO0_AHB),
    ],
    &[
        Step::EnableClock(SYSCLK_SDIO1_AHB),
        Step::EnableClock(SYSCLK_SDIO1_SDCARD),
        Step::DeassertReset(SYSRST_SDIO1_AHB),
    ],
];

/// The divider field of a JH7110 clock word, bits 23:0 (\[mainline-clk\]'s `JH71X0_CLK_DIV_MASK`).
/// A transcript prints it beside the enable bit so the card clock's source rate is read off the
/// silicon rather than assumed from `assigned-clock-rates`.
#[must_use]
pub const fn clock_divider(word: u32) -> u32 {
    word & 0x00ff_ffff
}

#[cfg(test)]
mod sdio_tests {
    use super::*;

    #[test]
    fn each_sdio_plan_ungates_both_clocks_before_its_reset_and_stays_in_the_sys_domain() {
        for plan in SDIO_BRING_UP {
            assert!(matches!(plan.last(), Some(Step::DeassertReset(_))));
            assert_eq!(plan.len(), 3);
            for s in plan {
                if let Step::EnableClock(i) = *s {
                    assert!(SYS.clock_offset(i).is_some());
                } else if let Step::DeassertReset(i) = *s {
                    assert!(SYS.reset_bit(i).is_some());
                }
            }
        }
        assert_eq!(SYS.clock_offset(SYSCLK_SDIO1_SDCARD), Some(0x178));
        let r = SYS.reset_bit(SYSRST_SDIO1_AHB).unwrap();
        assert_eq!(
            (r.assert_offset, r.status_offset, r.mask),
            (0x300, 0x310, 1 << 1)
        );
        assert_eq!(clock_divider(CLOCK_ENABLE | 8), 8);
    }
}

/// The PMIC as radon's vendor tree spells it (U-Boot SDK `VF2_v2.10.4`, `starfive_visionfive2.dts`:
/// `pmic: axp15060_reg@36 { compatible = "stf,axp15060-regulator"; reg = <0x36>; }` under
/// `&i2c5`). This is also the string radon's OpenSBI matches to find its reset device.
pub const COMPATIBLE_PMIC_VENDOR: &[u8] = b"stf,axp15060-regulator";

/// The same PMIC in mainline's `jh7110-common.dtsi`: `axp15060: pmic@36 { compatible =
/// "x-powers,axp15060"; }` under `&i2c5`.
pub const COMPATIBLE_PMIC_MAINLINE: &[u8] = b"x-powers,axp15060";

/// How many steps a tree-derived plan can hold. Radon's tree names two clocks and one reset, one
/// of those clocks virtual; four is slack, and a bus naming more is recorded as truncated.
pub const MAX_PMIC_BUS_STEPS: usize = 4;

/// **The plan for the PMIC's bus, and where it came from** (milestone 592, provisional).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PmicBus {
    steps: [Step; MAX_PMIC_BUS_STEPS],
    len: usize,
    /// **True when the plan is the tree's own `clocks` and `resets` of the PMIC's parent bus**,
    /// false when it is [`PMIC_BUS_BRING_UP`]. The field a bench transcript must carry, for
    /// [`Found::from_tree`]'s reason.
    pub from_tree: bool,
    /// Which PMIC `compatible` was found, or `None` when the tree names no known PMIC.
    pub pmic: Option<&'static [u8]>,
    /// Clock or reset specifiers the bus named that are **not** a SYS-domain gate or line, and
    /// were skipped rather than written: radon's virtual clock 298 is the expected one. A provider
    /// this crate does not recognise, or an id past [`SYS`]'s bounds, lands here, never in a store.
    pub skipped: usize,
    /// True when the bus named more usable steps than [`MAX_PMIC_BUS_STEPS`].
    pub truncated: bool,
    /// **The I2C controller's register window**, the `reg` of the PMIC's parent bus node, when the
    /// tree states it. Option B's write needs it; a plan without one (the constant fallback, or a
    /// tree that names no `reg`) leaves it `None` and the kernel does not attempt the write.
    pub controller: Option<(u64, u64)>,
    /// **The PMIC's 7-bit I2C address**, the PMIC node's own `reg`: `0x36` in both of radon's
    /// trees. `None` when the tree names no known PMIC at all.
    pub pmic_address: Option<u32>,
}

impl PmicBus {
    /// The steps to walk, clocks before resets.
    #[must_use]
    pub fn plan(&self) -> &[Step] {
        &self.steps[..self.len]
    }

    fn fallback(pmic: Option<&'static [u8]>, skipped: usize) -> Self {
        let mut bus = PmicBus {
            steps: [Step::EnableClock(0); MAX_PMIC_BUS_STEPS],
            len: PMIC_BUS_BRING_UP.len(),
            from_tree: false,
            pmic,
            skipped,
            truncated: false,
            controller: None,
            pmic_address: None,
        };
        bus.steps[..PMIC_BUS_BRING_UP.len()].copy_from_slice(PMIC_BUS_BRING_UP);
        bus
    }
}

/// One `reg` property's first address and size, from the cells either tree spelling uses: four
/// cells (`<hi lo size-hi size-lo>`, the root's `#address-cells 2` and `#size-cells 2`) or two
/// (`<addr size>`). Big-endian, as every device tree cell is. `None` for a shorter or partial
/// property, which the caller treats as "not stated".
fn parse_reg(cells: &[u8]) -> Option<(u64, u64)> {
    let u32be = |at: usize| -> u64 {
        u64::from(u32::from_be_bytes([
            cells[at],
            cells[at + 1],
            cells[at + 2],
            cells[at + 3],
        ]))
    };
    if cells.len() >= 16 {
        Some(((u32be(0) << 32) | u32be(4), (u32be(8) << 32) | u32be(12)))
    } else if cells.len() >= 8 {
        Some((u32be(0), u32be(4)))
    } else {
        None
    }
}

/// Read the two option-B facts out of `tree` for the PMIC `pmic`: the PMIC node's own `reg` (its
/// I2C address) and its parent bus node's `reg` (the controller's window). Both are optional in the
/// tree, so both are `Option`, and neither affects the clocks-and-resets plan.
fn fill_pmic_address_and_bus(
    tree: &device_tree_blob::DeviceTreeBlob<'_>,
    pmic: &[u8],
    bus: &mut PmicBus,
) {
    // The PMIC's own `reg` is an I2C child address: one cell, size zero by `#size-cells 0`, which
    // `node_reg_compatible`'s Region shape cannot carry (it skips zero-size regions on purpose).
    // So read the raw cells and take the first, which both of radon's trees spell `<0x36>`.
    if let Ok(Some(cells)) = tree.node_prop_compatible(pmic, b"reg")
        && cells.len() >= 4
    {
        bus.pmic_address = Some(u32::from_be_bytes([cells[0], cells[1], cells[2], cells[3]]));
    }
    if let Ok(Some(cells)) = tree.parent_prop_compatible(pmic, b"reg") {
        bus.controller = parse_reg(cells);
    }
}

/// **Read the PMIC's I2C bus's clocks and resets out of `tree`** (milestone 592, provisional).
///
/// Finds the AXP15060 by `compatible` (mainline's spelling first, then the vendor's), reads the
/// `clocks` and `resets` of its **parent** node, which is the I2C controller it sits on, and keeps
/// each `<phandle id>` pair only when the phandle names a SYS-domain provider this crate knows
/// (mainline `syscrg`, or the vendor `clkgen`/`rstgen`), that provider has one cell per
/// specifier, and the id is inside [`SYS`]'s bounds. Everything else is counted in
/// [`PmicBus::skipped`] and never becomes a step.
///
/// The vendor providers number all their domains in one flat space with SYS first, so "inside
/// [`SYS`]'s bounds" is the same test as "a SYS-domain id" there; mainline's `syscrg` numbers SYS
/// alone. That is why one bound serves both.
///
/// Never fails to produce a plan: a tree with no PMIC, or a PMIC whose bus yields no usable step,
/// gets [`PMIC_BUS_BRING_UP`] with `from_tree: false`. Like [`discover`], an answer here says
/// nothing about whether the machine is a JH7110, and the caller must have established that first.
///
/// # Errors
///
/// Propagates [`device_tree_blob::Error`] if the blob is malformed.
pub fn pmic_bus(
    tree: &device_tree_blob::DeviceTreeBlob<'_>,
) -> Result<PmicBus, device_tree_blob::Error> {
    for pmic in [COMPATIBLE_PMIC_MAINLINE, COMPATIBLE_PMIC_VENDOR] {
        let clocks = tree.parent_prop_compatible(pmic, b"clocks")?;
        let resets = tree.parent_prop_compatible(pmic, b"resets")?;
        if clocks.is_none() && resets.is_none() {
            // Either no such PMIC (try the next spelling), or a bus that names nothing.
            if tree.node_prop_compatible(pmic, b"compatible")?.is_none() {
                continue;
            }
            let mut bus = PmicBus::fallback(Some(pmic), 0);
            fill_pmic_address_and_bus(tree, pmic, &mut bus);
            return Ok(bus);
        }
        let mut bus = PmicBus::fallback(Some(pmic), 0);
        fill_pmic_address_and_bus(tree, pmic, &mut bus);
        bus.len = 0;
        for (list, is_clock) in [(clocks, true), (resets, false)] {
            let Some(list) = list else { continue };
            let specs = list.chunks(8);
            let total = specs.len();
            for (i, spec) in specs.enumerate() {
                match sys_step(tree, spec, is_clock)? {
                    Spec::Step(step) if bus.len < MAX_PMIC_BUS_STEPS => {
                        bus.steps[bus.len] = step;
                        bus.len += 1;
                    }
                    Spec::Step(_) => bus.truncated = true,
                    Spec::Foreign => bus.skipped += 1,
                    // The stride is wrong from here on, so nothing after this is a specifier.
                    Spec::Unreadable => {
                        bus.skipped += total - i;
                        break;
                    }
                }
            }
        }
        if bus.len == 0 {
            return Ok(PmicBus::fallback(Some(pmic), bus.skipped));
        }
        bus.from_tree = true;
        return Ok(bus);
    }
    Ok(PmicBus::fallback(None, 0))
}

// ===== Option B: the direct reset write, and the numbers it needs (milestone 592, 2026-10-10) =====
//
// The 2026-10-10 bench (keep that transcript beside this) put radon's reset in the outcome table's
// fourth row: the bus provably up (`running`, `released`), and OpenSBI's read of the PMIC still
// failing ten times. Option B is this kernel writing the AXP15060 itself over a DesignWare I2C
// master, which needs three numbers the tree and the CRG hold: the controller's base, the PMIC's
// address, and the I2C input clock that scales every timing count. Everything here is host-testable
// arithmetic over words the kernel reads; the sources are the same vendor trees 592 already pinned.

/// The SYS **syscon** window, `0x1303_0000`: where the PLL control words live, distinct from
/// [`SYS_BASE`]'s CRG window the clock words live in. Vendor `jh7110-regs.h` of the pinned tree
/// (`SYS_SYSCON_BASE 0x13030000`, `SYS_CRG_BASE 0x13020000`, fetched 2026-10-10), which resolves
/// what looked like an overlap between the PLL words at `0x2c..0x34` and the early clock words:
/// different windows, same offsets.
pub const SYS_SYSCON_BASE: u64 = 0x1303_0000;

/// The SYS syscon window's size, matching the CRG window's.
pub const SYS_SYSCON_SIZE: u64 = 0x1_0000;

/// `JH7110_BUS_ROOT` in the vendor header radon's U-Boot builds from, **5**. Word `0x14`: a mux
/// whose bit 24 picks `osc` (0) or `pll2_out` (1), per the vendor clock driver's registration
/// (`bus_root_sels`, `starfive_clk_mux(..., SYS_OFFSET(JH7110_BUS_ROOT), 1, ...)`).
pub const SYSCLK_BUS_ROOT: u32 = 5;

/// `JH7110_AXI_CFG0`, **7**. Word `0x1c`: a one-based 2-bit divider off `bus_root`.
pub const SYSCLK_AXI_CFG0: u32 = 7;

/// `JH7110_STG_AXIAHB`, **8**. Word `0x20`: a one-based 2-bit divider off `axi_cfg0`.
pub const SYSCLK_STG_AXIAHB: u32 = 8;

/// `JH7110_APB_BUS_FUNC`, **11**. Word `0x2c`: a one-based 4-bit divider off `stg_axiahb`, the
/// last divider before the APB tree that `u5_dw_i2c_clk_apb` (gate id 143, bit 31 only) sits on.
pub const SYSCLK_APB_BUS_FUNC: u32 = 11;

/// The oscillator every JH7110 rate chain is rooted in, 24 MHz, `refclk` in the vendor PLL code.
pub const OSC_HZ: u64 = 24_000_000;

/// The vendor PLL code's default for a PLL it cannot interpret (`deffreq` for PLL2), returned
/// rather than guessed from fields that are not in integer mode.
pub const PLL2_DEFAULT_HZ: u64 = 1_188_000_000;

/// **The words the I2C input-clock chain reads**, one struct so the kernel can read them in one
/// place and the arithmetic can be tested without a device (milestone 592, provisional). The CRG
/// words come from [`SYS`]'s window at the offsets the ids above give; the PLL2 words from
/// [`SYS_SYSCON_BASE`]'s window at `0x2c` (DACPD bit 15, DSMPD bit 16, FBDIV bits 28:17), `0x30`
/// (POSTDIV1 bits 29:28) and `0x34` (PREDIV bits 5:0), exactly the vendor `pll.c` masks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IcClkWords {
    /// Word `0x14` of the CRG: `bus_root`'s mux, bit 24.
    pub bus_root: u32,
    /// Word `0x1c`: `axi_cfg0`'s divider, bits 1:0, one-based.
    pub axi_cfg0: u32,
    /// Word `0x20`: `stg_axiahb`'s divider, bits 1:0, one-based.
    pub stg_axiahb: u32,
    /// Word `0x2c` of the CRG: `apb_bus_func`'s divider, bits 3:0, one-based.
    pub apb_bus_func: u32,
    /// SYS syscon word `0x2c`: PLL2's DACPD, DSMPD and FBDIV.
    pub pll2_dacpd_dsmpd_fbdiv: u32,
    /// SYS syscon word `0x30`: PLL2's POSTDIV1, bits 29:28.
    pub pll2_postdiv1: u32,
    /// SYS syscon word `0x34`: PLL2's PREDIV, bits 5:0.
    pub pll2_prediv: u32,
}

/// A one-based divider field of `width` bits: the value when nonzero, 1 when the field reads zero
/// (a zero a one-based divider never means, and a boot that left one is a fact the caller's print
/// should carry rather than a divide-by-zero).
#[must_use]
pub const fn one_based_div(word: u32, width: u32) -> u64 {
    let mask = if width >= 32 {
        u32::MAX
    } else {
        (1 << width) - 1
    };
    let value = (word & mask) as u64;
    if value == 0 { 1 } else { value }
}

/// **The I2C input clock of the PMIC's bus**, from the words above: the rate chain
/// `bus_root -> axi_cfg0 -> stg_axiahb -> apb_bus_func -> (gate) u5_dw_i2c_clk_apb`. Every step is
/// the vendor clock driver's own registration; the gates do not divide. PLL2's rate follows the
/// vendor `pll.c` exactly: integer mode (`dacpd == 1 && dsmpd == 1`) is
/// `24 MHz * fbdiv / (prediv * postdiv1)`, anything else is the vendor's own default rather than a
/// guess. The u64 divisions truncate, as the vendor's do.
#[must_use]
pub const fn i2c5_ic_clk(w: &IcClkWords) -> u64 {
    let parent = if (w.bus_root >> 24) & 1 == 1 {
        let dacpd = (w.pll2_dacpd_dsmpd_fbdiv >> 15) & 1;
        let dsmpd = (w.pll2_dacpd_dsmpd_fbdiv >> 16) & 1;
        let fbdiv = ((w.pll2_dacpd_dsmpd_fbdiv >> 17) & 0xfff) as u64;
        let prediv_raw = (w.pll2_prediv & 0x3f) as u64;
        let prediv = if prediv_raw == 0 { 1 } else { prediv_raw };
        let postdiv1 = 1u64 << ((w.pll2_postdiv1 >> 28) & 3);
        if dacpd == 1 && dsmpd == 1 && fbdiv > 0 {
            OSC_HZ * fbdiv / (prediv * postdiv1)
        } else {
            PLL2_DEFAULT_HZ
        }
    } else {
        OSC_HZ
    };
    parent
        / one_based_div(w.axi_cfg0, 2)
        / one_based_div(w.stg_axiahb, 2)
        / one_based_div(w.apb_bus_func, 4)
}

/// **The 100 kHz standard-mode programming for a DesignWare I2C controller**, from the timing
/// formula in the vendor U-Boot `drivers/i2c/designware_i2c.c` this board's firmware was built from
/// (fetched 2026-10-10, same tree 592 pinned): counts of the input clock for the standard mode's
/// minimum high (4000 ns) and low (4700 ns) times, the default rise (1000 ns) and fall (300 ns)
/// times, no spike count, then the formula's period fill toward `ic_clk / 100_000`. U-Boot's own
/// init constants give the control word: master mode, restart enable, slave disable, standard speed.
///
/// 100 kHz rather than the fast mode U-Boot defaults to, because the AXP15060's datasheet timing is
/// met by every standard-mode controller and the 2026-10-10 failure this answers was OpenSBI
/// trusting a controller at reset defaults; slower than the spec floor is safe, faster is not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StandardMode {
    /// The `IC_CON` word: `IC_CON_MM | IC_CON_RE | IC_CON_SD | IC_CON_SPD_SS`.
    pub con: u32,
    /// `IC_SS_SCL_HCNT`.
    pub hcnt: u32,
    /// `IC_SS_SCL_LCNT`.
    pub lcnt: u32,
    /// `IC_SDA_HOLD`, the default 300 ns hold in input-clock counts.
    pub sda_hold: u32,
}

/// `IC_CON`'s standard-speed field, `0b10` (the vendor header's `IC_CON_SPD_SS`).
const IC_CON_SPD_SS: u32 = 0b10;
/// `IC_CON`'s master-mode bit (`IC_CON_MM`).
const IC_CON_MM: u32 = 1;
/// `IC_CON`'s restart-enable bit (`IC_CON_RE`).
const IC_CON_RE: u32 = 1 << 5;
/// `IC_CON`'s slave-disable bit (`IC_CON_SD`).
const IC_CON_SD: u32 = 1 << 6;

/// Count of input-clock ticks that covers `period_ns`, the vendor `calc_counts` (a rounding-up
/// division: `DIV_ROUND_UP(ic_clk / 1000 * period_ns, NANO_TO_KILO)`).
const fn counts(ic_clk: u64, period_ns: u64) -> u64 {
    (ic_clk / 1_000 * period_ns).div_ceil(1_000_000)
}

/// The formula the vendor driver names `dw_i2c_calc_timing`, standard mode, written as the
/// four-line derivation its comment carries, with the period fill. Inputs below the spec minima
/// clamp to the smallest legal counts rather than wrapping.
#[must_use]
pub const fn standard_mode_100k(ic_clk: u64) -> StandardMode {
    let rise = counts(ic_clk, 1_000);
    let fall = counts(ic_clk, 300);
    let thigh = counts(ic_clk, 4_000);
    let tlow = counts(ic_clk, 4_700);
    let period = if ic_clk >= 100_000 {
        ic_clk / 100_000
    } else {
        1
    };

    let mut hcnt = thigh.saturating_sub(fall + 7);
    let mut lcnt = tlow
        .saturating_sub(rise)
        .saturating_add(fall)
        .saturating_sub(1);

    let tot = hcnt + lcnt + 7 + rise + 1;
    if tot < period {
        let diff = (period - tot) / 2;
        hcnt += diff;
        lcnt += diff;
        let tot = hcnt + lcnt + 7 + rise + 1;
        lcnt += period.saturating_sub(tot);
    }
    // Below a few MHz of input clock the standard-mode minima outrun the period and the
    // derivation floors at zero; zero is not a count a controller can be programmed with, so it
    // becomes 1, the slowest legal bus this formula can produce. A bench transcript carries the
    // computed input clock, so a floored count is visible as one.
    let hcnt = if hcnt == 0 { 1 } else { hcnt };
    let lcnt = if lcnt == 0 { 1 } else { lcnt };
    let sda_hold = counts(ic_clk, 300);
    let sda_hold = if sda_hold == 0 { 1 } else { sda_hold };
    StandardMode {
        con: IC_CON_MM | IC_CON_RE | IC_CON_SD | IC_CON_SPD_SS,
        hcnt: hcnt as u32,
        lcnt: lcnt as u32,
        sda_hold: sda_hold as u32,
    }
}

/// The AXP15060 register OpenSBI's reset and shutdown both go through, `0x32`, and the bit that
/// resets: bit 6 (bit 7 powers off). The 2026-10-09 and 2026-10-10 benches on radon, and 592's
/// block, carry the reading; this kernel writes the reset bit alone, read-modify-write, so the
/// register's other bits keep whatever the PMIC already holds.
pub const PMIC_RESET_REG: u8 = 0x32;

/// The reset bit of [`PMIC_RESET_REG`].
pub const PMIC_RESET_BIT: u8 = 6;

/// What one `<phandle id>` specifier turned out to be.
enum Spec {
    /// A SYS-domain gate or line, in bounds.
    Step(Step),
    /// A well-formed specifier for something else: another provider, or an id past [`SYS`]'s
    /// bounds (radon's virtual clock 298).
    Foreign,
    /// A provider without exactly one cell per specifier, or a short tail. `pmic_bus` walks in
    /// eight-byte strides, so everything from here on in the list is unreadable and skipped: the
    /// safe direction, since a skipped specifier costs a fallback and a misread one would be a
    /// store.
    Unreadable,
}

/// Classify one specifier; see [`Spec`].
fn sys_step(
    tree: &device_tree_blob::DeviceTreeBlob<'_>,
    spec: &[u8],
    is_clock: bool,
) -> Result<Spec, device_tree_blob::Error> {
    let [p0, p1, p2, p3, i0, i1, i2, i3] = *spec else {
        return Ok(Spec::Unreadable);
    };
    let phandle = u32::from_be_bytes([p0, p1, p2, p3]);
    let id = u32::from_be_bytes([i0, i1, i2, i3]);
    let cells = if is_clock {
        &b"#clock-cells"[..]
    } else {
        &b"#reset-cells"[..]
    };
    if tree.phandle_prop(phandle, cells)? != Some(&[0, 0, 0, 1][..]) {
        return Ok(Spec::Unreadable);
    }
    let Some(compatible) = tree.phandle_prop(phandle, b"compatible")? else {
        return Ok(Spec::Foreign);
    };
    let known = [
        COMPATIBLE_SYSCRG,
        if is_clock {
            COMPATIBLE_VENDOR_CLKGEN
        } else {
            COMPATIBLE_VENDOR_RSTGEN
        },
    ];
    if !compatible.split(|&b| b == 0).any(|c| known.contains(&c)) {
        return Ok(Spec::Foreign);
    }
    let step = if is_clock {
        SYS.clock_offset(id).map(|_| Step::EnableClock(id))
    } else {
        SYS.reset_bit(id).map(|_| Step::DeassertReset(id))
    };
    Ok(step.map_or(Spec::Foreign, Spec::Step))
}

impl Domain {
    /// The byte offset of `index`'s clock word, or `None` if this domain has no such clock.
    ///
    /// One 32-bit word per clock, in index order, from \[mainline-clk\]: `priv->base + 4 *
    /// clk->idx`. The `Option` is the bound: a caller holding an identifier from the wrong domain
    /// gets nothing rather than a plausible-looking offset into somebody else's register.
    #[must_use]
    pub const fn clock_offset(&self, index: u32) -> Option<u64> {
        if index >= self.clocks {
            return None;
        }
        Some(4 * index as u64)
    }

    /// Where reset `id` is written and watched, or `None` if this domain has no such reset.
    ///
    /// 32 resets to a word, from \[mainline-rst\]'s `offset = id / 32; mask = BIT(id % 32)`. The
    /// STG domain has 23, so every one of them is in word zero; the arithmetic is written out
    /// anyway because the SYS domain has 126 and would otherwise be a second implementation of
    /// the same rule.
    #[must_use]
    pub const fn reset_bit(&self, id: u32) -> Option<ResetBit> {
        if id >= self.resets {
            return None;
        }
        let word = (id / 32) as u64 * 4;
        Some(ResetBit {
            assert_offset: self.reset_assert + word,
            status_offset: self.reset_status + word,
            mask: 1 << (id % 32),
        })
    }
}

/// **Is this reset released?** Given a status word and a reset's mask.
///
/// The sense is inverted from what the name `status` suggests and the inversion is not this
/// crate's invention. \[mainline-rst\]'s `jh71x0_reset_update` computes `done = 0` for an assert
/// and `done = mask` for a deassert (the JH7110 passes `asserted = NULL`), then polls until
/// `(value & mask) == done`. So a **set** bit is a line that is out of reset. Getting this
/// backwards would produce a driver that waits forever on a device that came up correctly, which
/// is the failure this function exists to have exactly one copy of.
#[must_use]
pub const fn is_deasserted(status_word: u32, mask: u32) -> bool {
    status_word & mask != 0
}

/// **Is this clock running?** Given the clock's own word.
#[must_use]
pub const fn is_clock_enabled(clock_word: u32) -> bool {
    clock_word & CLOCK_ENABLE != 0
}

/// How many `EnableClock` steps a report keeps words for. Two is what the TRNG needs; four is
/// slack for the next device, and a plan with more simply stops recording rather than growing the
/// report, which is what [`Report::truncated`] says out loud.
pub const MAX_RECORDED_CLOCKS: usize = 4;

/// What the hardware said, in enough detail that a bench transcript is diagnosable without a
/// second trip to the machine.
///
/// **The `before` words are the load-bearing ones.** If the clocks read back already enabled and
/// the reset already released, then this milestone's premise was wrong for radon and the TRNG's
/// all-zero register window on 2026-09-04 has some other cause. That is the outcome a bench
/// session most needs to be able to tell apart, and only a before-and-after can tell it.
#[derive(Clone, Copy, Debug, Default)]
pub struct Report {
    /// Each `EnableClock` step's word as it read *before* anything was written, in plan order.
    pub clock_before: [u32; MAX_RECORDED_CLOCKS],
    /// The same words read back *after* the enable bit was stored.
    pub clock_after: [u32; MAX_RECORDED_CLOCKS],
    /// How many entries of the two arrays above are meaningful.
    pub clocks: usize,
    /// True when the plan had more clock steps than [`MAX_RECORDED_CLOCKS`]. They were still
    /// performed; only the recording stopped.
    pub truncated: bool,
    /// The reset-assert word as it read *before* the plan's last `DeassertReset` step wrote it.
    /// Zero when the plan had none, which [`Report::had_reset`] distinguishes from a real zero.
    pub reset_assert_before: u32,
    /// The same word read back after the step's bit was cleared.
    pub reset_assert_after: u32,
    /// The status word as the poll last saw it. A set bit means out of reset; see
    /// [`is_deasserted`], whose doc records why that sense is the opposite of what the name
    /// suggests.
    pub reset_status_after: u32,
    /// True when the plan contained a `DeassertReset` at all, so a reader can tell "no reset in
    /// this plan" from "a reset whose registers all read zero".
    pub had_reset: bool,
    /// True when the status word said the line was out of reset before the poll gave up.
    pub released: bool,
    /// How many status reads the deassert took. The driver's own `POLL_LIMIT` here means it never
    /// came out.
    pub polls: u32,
    /// Steps whose identifier this domain rejected, which would mean the plan and the domain
    /// disagree: a programming error, not a hardware condition. Nonzero here invalidates the rest.
    pub rejected: usize,
    /// How many `DeassertReset` steps the plan took. The fields above describe the last of them;
    /// this and [`Report::resets_released`] cover a plan with more than one (milestone 53's
    /// `gmac0` has two).
    pub resets: u32,
    /// How many of those read as released before the poll gave up.
    pub resets_released: u32,
    /// The last `SelectParent` step's clock word before it was written, and
    /// [`Report::mux_after`] the word read back. Both zero when the plan had none, which
    /// [`Report::had_mux`] distinguishes.
    pub mux_before: u32,
    /// See [`Report::mux_before`].
    pub mux_after: u32,
    /// True when the plan contained a `SelectParent`.
    pub had_mux: bool,
}

impl Report {
    /// **Did every reset step read as released?** True for a plan with none.
    #[must_use]
    pub fn every_reset_released(&self) -> bool {
        self.resets_released == self.resets
    }

    /// **Did every clock this plan named read its enable bit back?** A clock that does not is a
    /// window with nothing behind it, or a base address that is not this controller.
    #[must_use]
    pub fn has_clocks_running(&self) -> bool {
        self.clocks > 0
            && self.clock_after[..self.clocks]
                .iter()
                .all(|&w| is_clock_enabled(w))
    }

    /// **Was the device already up before this ran?** True when every recorded clock was enabled
    /// and, if the plan had one, the reset was already released. See the type's own doc for why
    /// this is the question a bench session asks first.
    #[must_use]
    pub fn was_already_up(&self) -> bool {
        let clocks = self.clocks > 0
            && self.clock_before[..self.clocks]
                .iter()
                .all(|&w| is_clock_enabled(w));
        clocks && (!self.had_reset || self.reset_assert_before & self.reset_mask() == 0)
    }

    /// The bit the reset step used, recoverable from the two assert words. Zero when there was no
    /// reset step or when the bit was already clear, which is why `was_already_up` reads the
    /// before-word rather than trusting this alone.
    const fn reset_mask(&self) -> u32 {
        self.reset_assert_before ^ self.reset_assert_after
    }
}

/// What [`discover`] or [`discover_sys`] concluded about where a domain's registers are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Found {
    /// Physical base of the STG register window.
    pub base: u64,
    /// Its size in bytes.
    pub size: u64,
    /// **True when the device tree said so, false when this is [`STG_BASE`].** The one field a
    /// bench transcript must carry: a base that came from a constant is a base nobody on that
    /// machine confirmed, and a reader three months later cannot re-derive which it was.
    pub from_tree: bool,
    /// Which `compatible` string matched, or `None` when nothing did and the constant was used.
    pub compatible: Option<&'static [u8]>,
}

/// Mainline's dedicated STG clock-and-reset controller node \[mainline-dts\].
pub const COMPATIBLE_STGCRG: &[u8] = b"starfive,jh7110-stgcrg";

/// The vendor U-Boot's single clock controller covering sys, stg and aon \[vendor-dts\]. This is
/// the one radon's firmware actually serves, if it serves either.
pub const COMPATIBLE_VENDOR_CLKGEN: &[u8] = b"starfive,jh7110-clkgen";

/// The vendor U-Boot's separate reset controller, whose `reg` list covers the same windows
/// \[vendor-dts\]. Tried last: it names the same address as the clkgen node, so it is only
/// reached by a tree that carries one and not the other.
pub const COMPATIBLE_VENDOR_RSTGEN: &[u8] = b"starfive,jh7110-reset";

/// The `reg-names` entry naming the STG window, in each vendor node's own spelling
/// (`"stg"` in `clkgen`, `"stgcrg"` in `rstgen`; \[vendor-dts\]).
const VENDOR_CLKGEN_STG_NAME: &[u8] = b"stg";
const VENDOR_RSTGEN_STG_NAME: &[u8] = b"stgcrg";

/// Mainline's dedicated SYS clock-and-reset controller node, `syscrg: clock-controller@13020000`
/// (\[mainline-dts\]; milestone 592).
pub const COMPATIBLE_SYSCRG: &[u8] = b"starfive,jh7110-syscrg";

/// The `reg-names` entry naming the SYS window, in each vendor node's own spelling (`"sys"` in
/// `clkgen`, `"syscrg"` in `rstgen`; \[vendor-dts\]). Window 0 in both today, found by name anyway
/// for the reason [`discover`] gives.
const VENDOR_CLKGEN_SYS_NAME: &[u8] = b"sys";
const VENDOR_RSTGEN_SYS_NAME: &[u8] = b"syscrg";

/// **Find the STG clock and reset window in `tree`.**
///
/// Never returns `None` and never fails to produce an address: a tree that names no controller
/// gets [`STG_BASE`] with `from_tree: false`, for the reason [`STG_BASE`] records. What it can
/// return is an error, if the blob itself does not parse.
///
/// **Three spellings are tried, mainline's first**, the same order and the same reasoning
/// `jh7110_entropy::discover` uses: a tree that carries the standardised binding is describing itself
/// in the language the binding standardised, and that is the one to believe. The two vendor nodes
/// carry several windows and are indexed by `reg-names` rather than by position, because a
/// position that happens to be right today is a fact nobody wrote down.
///
/// # Errors
///
/// Propagates [`device_tree_blob::Error`] if the blob is malformed.
pub fn discover(
    tree: &device_tree_blob::DeviceTreeBlob<'_>,
) -> Result<Found, device_tree_blob::Error> {
    discover_window(
        tree,
        &[
            (COMPATIBLE_STGCRG, None),
            (COMPATIBLE_VENDOR_CLKGEN, Some(VENDOR_CLKGEN_STG_NAME)),
            (COMPATIBLE_VENDOR_RSTGEN, Some(VENDOR_RSTGEN_STG_NAME)),
        ],
        STG_BASE,
        STG_SIZE,
    )
}

/// **Find the AON clock and reset window in `tree`** (milestone 53, the JH7110's Ethernet), the
/// domain holding `gmac0`'s bus clocks, transmit clock and resets.
///
/// [`discover`]'s twin, with the same three spellings in the same order and the same refusal to
/// fail: a tree that names no controller gets [`AON_BASE`] with `from_tree: false`, and an answer
/// here is not evidence that the machine is a JH7110.
///
/// Name: provisional (milestone 53's lane, 2026-10-06 UTC), after [`discover_sys`].
///
/// # Errors
///
/// Propagates [`device_tree_blob::Error`] if the blob is malformed.
pub fn discover_aon(
    tree: &device_tree_blob::DeviceTreeBlob<'_>,
) -> Result<Found, device_tree_blob::Error> {
    discover_window(
        tree,
        &[
            (COMPATIBLE_AONCRG, None),
            (COMPATIBLE_VENDOR_CLKGEN, Some(VENDOR_CLKGEN_AON_NAME)),
            (COMPATIBLE_VENDOR_RSTGEN, Some(VENDOR_RSTGEN_AON_NAME)),
        ],
        AON_BASE,
        AON_SIZE,
    )
}

/// **Find the SYS clock and reset window in `tree`** (milestone 592, provisional), the domain
/// that holds I2C5's clock and reset and so the only road OpenSBI has to radon's PMIC.
///
/// [`discover`]'s twin, with the same three spellings in the same order and the same refusal to
/// fail: a tree that names no controller gets [`SYS_BASE`] with `from_tree: false`. The caller is
/// held to [`discover`]'s rule: an answer here is not evidence that the machine is a JH7110.
///
/// Name: provisional (milestone 592), 2026-09-25. calef names public functions.
///
/// # Errors
///
/// Propagates [`device_tree_blob::Error`] if the blob is malformed.
pub fn discover_sys(
    tree: &device_tree_blob::DeviceTreeBlob<'_>,
) -> Result<Found, device_tree_blob::Error> {
    discover_window(
        tree,
        &[
            (COMPATIBLE_SYSCRG, None),
            (COMPATIBLE_VENDOR_CLKGEN, Some(VENDOR_CLKGEN_SYS_NAME)),
            (COMPATIBLE_VENDOR_RSTGEN, Some(VENDOR_RSTGEN_SYS_NAME)),
        ],
        SYS_BASE,
        SYS_SIZE,
    )
}

/// The walk both `discover` functions share: each spelling in order, then the constant.
fn discover_window(
    tree: &device_tree_blob::DeviceTreeBlob<'_>,
    spellings: &[(&'static [u8], Option<&[u8]>)],
    base: u64,
    size: u64,
) -> Result<Found, device_tree_blob::Error> {
    for &(compatible, name) in spellings {
        if let Some(found) = discover_as(tree, compatible, name)? {
            return Ok(found);
        }
    }
    Ok(Found {
        base,
        size,
        from_tree: false,
        compatible: None,
    })
}

/// One spelling's worth of [`discover`]. `window` is the `reg-names` entry to select, or `None`
/// for a node whose single `reg` is the answer.
fn discover_as(
    tree: &device_tree_blob::DeviceTreeBlob<'_>,
    compatible: &'static [u8],
    window: Option<&[u8]>,
) -> Result<Option<Found>, device_tree_blob::Error> {
    // Five, because the vendor `rstgen` node names five windows and a short buffer would silently
    // truncate the list `reg-names` is indexing into.
    let mut regions = [device_tree_blob::Region { start: 0, size: 0 }; 5];
    let n = tree.node_reg_compatible(compatible, &mut regions)?;
    if n == 0 {
        return Ok(None);
    }
    let index = match window {
        None => 0,
        Some(want) => {
            let names = tree.node_prop_compatible(compatible, b"reg-names")?;
            // A multi-window node with no `reg-names` is not something this crate will guess at:
            // picking window 1 because the tree we read said so is exactly the "a fact nobody
            // wrote down" case, and the constant fallback is the honest answer instead.
            match names.and_then(|bytes| name_index(bytes, want)) {
                Some(i) if i < n => i,
                _ => return Ok(None),
            }
        }
    };
    Ok(Some(Found {
        base: regions[index].start,
        size: regions[index].size,
        from_tree: true,
        compatible: Some(compatible),
    }))
}

/// The position of `want` in a device-tree string list (NUL-separated, NUL-terminated).
///
/// Split rather than trimmed, and an empty trailing element is not counted: `"sys\0stg\0aon\0"`
/// has three entries, not four, and `stg` is index 1. Getting that wrong would shift every window
/// after the first.
fn name_index(list: &[u8], want: &[u8]) -> Option<usize> {
    list.split(|&b| b == 0)
        .filter(|s| !s.is_empty())
        .position(|s| s == want)
}

#[cfg(test)]
mod tests {
    use super::*;

    const STGCRG_MAINLINE: &[u8] = include_bytes!("../tests/fixtures/jh7110-stgcrg-mainline.dtb");
    const CLKGEN_VENDOR: &[u8] = include_bytes!("../tests/fixtures/jh7110-clkgen-vendor.dtb");
    const CLKGEN_VENDOR_UNNAMED: &[u8] =
        include_bytes!("../tests/fixtures/jh7110-clkgen-vendor-unnamed.dtb");
    const CLKGEN_VENDOR_MISMATCHED: &[u8] =
        include_bytes!("../tests/fixtures/jh7110-clkgen-vendor-mismatched-names.dtb");
    /// The blob `crates/device_tree_blob`'s own tests boot-verify against, so a change to QEMU's
    /// `virt` board is caught here rather than surfacing as a mystery at the bench.
    const QEMU_RISCV64_VIRT: &[u8] =
        include_bytes!("../../device_tree_blob/tests/fixtures/qemu-riscv64-virt.dtb");

    const PMIC_BUS_RADON: &[u8] = include_bytes!("../tests/fixtures/jh7110-pmic-bus-radon.dtb");
    const PMIC_BUS_MAINLINE: &[u8] =
        include_bytes!("../tests/fixtures/jh7110-pmic-bus-mainline.dtb");
    const PMIC_BUS_FOREIGN: &[u8] = include_bytes!("../tests/fixtures/jh7110-pmic-bus-foreign.dtb");
    const PMIC_BUS_OVERFULL: &[u8] =
        include_bytes!("../tests/fixtures/jh7110-pmic-bus-overfull.dtb");

    #[test]
    fn radons_pmic_bus_is_one_real_gate_and_one_reset_and_the_virtual_clock_is_skipped() {
        // The fixture is radon's own U-Boot tree. Its bus names clock 298 first, which is a
        // divide-by-one child of 143 with no register: writing "word 298" would be a store to
        // 0x4a8 of the SYS window, past every clock and reset word. It must be skipped, counted,
        // and the plan must still be the tree's own.
        let tree = device_tree_blob::DeviceTreeBlob::from_bytes(PMIC_BUS_RADON).unwrap();
        let bus = pmic_bus(&tree).unwrap();
        assert_eq!(
            bus.plan(),
            &[
                Step::EnableClock(SYSCLK_I2C5_APB),
                Step::DeassertReset(SYSRST_I2C5_APB)
            ]
        );
        assert!(bus.from_tree);
        assert_eq!(bus.pmic, Some(COMPATIBLE_PMIC_VENDOR));
        assert_eq!(bus.skipped, 1, "clock 298, and nothing else");
        assert!(!bus.truncated);
    }

    #[test]
    fn radons_sys_window_is_the_vendor_nodes_first_entry_found_by_name() {
        let tree = device_tree_blob::DeviceTreeBlob::from_bytes(PMIC_BUS_RADON).unwrap();
        let found = discover_sys(&tree).unwrap();
        assert_eq!((found.base, found.size), (SYS_BASE, SYS_SIZE));
        assert!(found.from_tree);
        assert_eq!(found.compatible, Some(COMPATIBLE_VENDOR_CLKGEN));
        // And STG, from the same node, is still its second window.
        assert_eq!(discover(&tree).unwrap().base, STG_BASE);
    }

    #[test]
    fn mainlines_pmic_bus_gives_the_same_plan_through_syscrg() {
        // Two trees that spell everything differently converge on 143 and 81, which is the
        // agreement that lets the constant fallback exist at all.
        let tree = device_tree_blob::DeviceTreeBlob::from_bytes(PMIC_BUS_MAINLINE).unwrap();
        let bus = pmic_bus(&tree).unwrap();
        assert_eq!(bus.plan(), PMIC_BUS_BRING_UP);
        assert!(bus.from_tree);
        assert_eq!(bus.pmic, Some(COMPATIBLE_PMIC_MAINLINE));
        assert_eq!(bus.skipped, 0);
        let found = discover_sys(&tree).unwrap();
        assert_eq!(found.base, SYS_BASE);
        assert_eq!(found.compatible, Some(COMPATIBLE_SYSCRG));
    }

    #[test]
    fn a_bus_naming_only_foreign_specifiers_falls_back_and_says_so() {
        // Wrong domain, out of bounds twice, and a two-cell provider that makes the rest of its
        // list unreadable (the trailing `<&syscrg 143>` included: refusing is the safe direction).
        // Nothing usable is left, so the constant plan is used, and `from_tree` says it was.
        let tree = device_tree_blob::DeviceTreeBlob::from_bytes(PMIC_BUS_FOREIGN).unwrap();
        let bus = pmic_bus(&tree).unwrap();
        assert_eq!(bus.plan(), PMIC_BUS_BRING_UP);
        assert!(!bus.from_tree);
        assert_eq!(bus.pmic, Some(COMPATIBLE_PMIC_MAINLINE));
        // aoncrg 3, syscrg 190, then 36 - 16 = 20 bytes read as three eight-byte strides, then
        // syscrg 126 in `resets`.
        assert_eq!(bus.skipped, 2 + 3 + 1);
    }

    /// More usable gates than the plan holds: the first four are kept in order, the rest are
    /// counted as cut off and not as foreign, and a bus that names clocks but no resets is still
    /// read (the absence of one property is not the absence of the bus).
    #[test]
    fn a_bus_with_more_gates_than_the_plan_holds_keeps_the_first_and_says_so() {
        let tree = device_tree_blob::DeviceTreeBlob::from_bytes(PMIC_BUS_OVERFULL).unwrap();
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
        assert_eq!(bus.plan().len(), MAX_PMIC_BUS_STEPS);
        assert!(bus.truncated);
        assert!(bus.from_tree);
        assert_eq!(bus.skipped, 0);
    }

    #[test]
    fn qemus_virt_board_has_no_pmic_and_gets_the_constant_plan_unclaimed() {
        let tree = device_tree_blob::DeviceTreeBlob::from_bytes(QEMU_RISCV64_VIRT).unwrap();
        let bus = pmic_bus(&tree).unwrap();
        assert_eq!(bus.plan(), PMIC_BUS_BRING_UP);
        assert!(!bus.from_tree);
        assert_eq!(bus.pmic, None);
        assert!(!discover_sys(&tree).unwrap().from_tree);
    }

    #[test]
    fn i2c5s_clock_and_reset_land_where_linux_puts_them() {
        // [mainline-clk] `base + 4 * idx`, and radon's OpenSBI's own `0x13020228 + 5 * 4` for the
        // same gate: two derivations of 0x23c.
        assert_eq!(SYS.clock_offset(SYSCLK_I2C5_APB), Some(0x23c));
        assert_eq!(0x228 + 5 * 4, 0x23c);
        let r = SYS.reset_bit(SYSRST_I2C5_APB).unwrap();
        assert_eq!(
            (r.assert_offset, r.status_offset, r.mask),
            (0x300, 0x310, 1 << 17)
        );
        // Radon's OpenSBI reads the index from the node name; `i2c@...` gives '@' - '0' = 16,
        // which is clock 138 + 16 = 154, JH7110_SYSCLK_UART4_CORE. Written down because it is
        // why the firmware's own re-enable cannot be relied on.
        assert_eq!(0x228 + (u64::from(b'@' - b'0')) * 4, 4 * 154);
    }

    #[test]
    fn the_sys_domains_last_clock_word_does_not_reach_its_reset_words() {
        let last = SYS.clock_offset(SYS.clocks - 1).unwrap();
        assert_eq!(last, 0x2f4);
        assert!(last + 4 <= SYS.reset_assert);
        assert_eq!(SYS.clock_offset(SYS.clocks), None);
        assert_eq!(SYS.clock_offset(298), None, "radon's virtual core clock");
        assert_eq!(SYS.reset_bit(125).unwrap().assert_offset, 0x2f8 + 12);
        assert_eq!(SYS.reset_bit(126), None);
    }

    #[test]
    fn the_pmic_plan_ungates_before_it_releases() {
        assert!(matches!(PMIC_BUS_BRING_UP[0], Step::EnableClock(_)));
        assert!(matches!(
            PMIC_BUS_BRING_UP[PMIC_BUS_BRING_UP.len() - 1],
            Step::DeassertReset(_)
        ));
    }

    #[test]
    fn mainline_stgcrg_is_read_from_its_single_reg() {
        let tree = device_tree_blob::DeviceTreeBlob::from_bytes(STGCRG_MAINLINE).unwrap();
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
        let tree = device_tree_blob::DeviceTreeBlob::from_bytes(CLKGEN_VENDOR).unwrap();
        let found = discover(&tree).unwrap();
        assert_eq!(found.base, STG_BASE, "the second window, not the first");
        assert_eq!(found.size, 0x1_0000);
        assert!(found.from_tree);
        assert_eq!(found.compatible, Some(COMPATIBLE_VENDOR_CLKGEN));
    }

    #[test]
    fn a_multi_window_node_without_reg_names_falls_back_rather_than_guessing() {
        let tree = device_tree_blob::DeviceTreeBlob::from_bytes(CLKGEN_VENDOR_UNNAMED).unwrap();
        let found = discover(&tree).unwrap();
        assert_eq!(found.base, STG_BASE);
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
        let tree = device_tree_blob::DeviceTreeBlob::from_bytes(QEMU_RISCV64_VIRT).unwrap();
        let found = discover(&tree).unwrap();
        assert!(!found.from_tree);
        assert_eq!(found.compatible, None);
        assert_eq!(found.base, STG_BASE);
    }

    #[test]
    fn a_reg_names_entry_past_the_end_of_reg_is_refused_rather_than_read() {
        // `reg-names` lists three names but `reg` has only two windows, with "stg" at index 2: an
        // index `regions` was never filled at. `discover_as`'s `i < n` guard exists for exactly
        // this: a tree this malformed must fall back to the corroborated constant rather than
        // read the zeroed slot an off-by-one would land on. Kills the `i < n -> true` and
        // `< -> <=` mutants, neither of which any other fixture reaches.
        let tree = device_tree_blob::DeviceTreeBlob::from_bytes(CLKGEN_VENDOR_MISMATCHED).unwrap();
        let found = discover(&tree).unwrap();
        assert!(
            !found.from_tree,
            "the named window does not exist; this must not be trusted"
        );
        assert_eq!(found.base, STG_BASE);
        assert_eq!(found.compatible, None);
    }

    #[test]
    fn a_report_of_zeros_is_not_a_report_of_success() {
        // This is radon's 2026-09-04 shape, one level up: a window with nothing behind it accepts
        // every store and reads back zero, so `has_clocks_running` must be false on the *after*
        // words rather than on the fact that the writes returned.
        let dead = Report {
            clocks: 2,
            had_reset: true,
            ..Report::default()
        };
        assert!(!dead.has_clocks_running());
        assert!(!dead.was_already_up());
        assert!(!dead.released);
    }

    #[test]
    fn a_report_with_no_clock_steps_claims_nothing() {
        // `clocks: 0` must not read as "all zero of my clocks are running", which is what a bare
        // `.iter().all()` would say. A plan that enabled nothing has proven nothing.
        assert!(!Report::default().has_clocks_running());
        assert!(!Report::default().was_already_up());
    }

    #[test]
    fn clocks_running_does_not_imply_already_up_while_the_reset_is_still_asserted() {
        // Every existing "clocks true" fixture also has the reset already released, so the
        // `!self.had_reset || ...` clause in `was_already_up` never gets to matter: had_reset is
        // true and the release check is true, and deleting the `!` leaves that OR true either
        // way. Force the reset bit still SET in `reset_assert_before` (not yet deasserted) so the
        // real answer is false and only the reset half of the OR can produce it.
        let clocked_but_held = Report {
            clock_before: [CLOCK_ENABLE, CLOCK_ENABLE, 0, 0],
            clock_after: [CLOCK_ENABLE, CLOCK_ENABLE, 0, 0],
            clocks: 2,
            had_reset: true,
            reset_assert_before: 1 << STGRST_SEC_AHB,
            reset_assert_after: 0,
            ..Report::default()
        };
        assert!(clocked_but_held.has_clocks_running());
        assert!(
            !clocked_but_held.was_already_up(),
            "the reset bit was still asserted before this ran"
        );
    }

    #[test]
    fn an_unrelated_bit_in_the_before_word_does_not_defeat_the_release_check() {
        // Complements the test above: here the target reset bit IS already clear (so the real
        // answer is true), but `reset_assert_before` carries an unrelated bit the mask does not
        // cover. `&` ignores it; `|` does not, which is what this catches.
        let released_with_noise = Report {
            clock_before: [CLOCK_ENABLE, CLOCK_ENABLE, 0, 0],
            clock_after: [CLOCK_ENABLE, CLOCK_ENABLE, 0, 0],
            clocks: 2,
            had_reset: true,
            reset_assert_before: 0x8000,
            reset_assert_after: 0x8000 ^ (1 << STGRST_SEC_AHB),
            ..Report::default()
        };
        assert_eq!(released_with_noise.reset_mask(), 1 << STGRST_SEC_AHB);
        assert!(
            released_with_noise.was_already_up(),
            "the target bit was already clear; the noise bit must not matter"
        );
    }

    #[test]
    fn reset_mask_recovers_the_bit_that_changed_between_the_two_assert_words() {
        let r = Report {
            reset_assert_before: 0b1010,
            reset_assert_after: 0b0010,
            ..Report::default()
        };
        assert_eq!(
            r.reset_mask(),
            0b1000,
            "the bit that differs, not 0 or 1 or their union"
        );
    }

    #[test]
    fn a_reset_word_past_the_first_multiplies_the_word_index_by_four() {
        // STG has only 23 resets, so every id this crate ever asks for lands in word 0 and
        // `* 4`/`/ 4` are indistinguishable there. A larger domain (SYS has 126, per this
        // module's own doc comment) is the shape `reset_bit`'s arithmetic is written for, so
        // construct one and ask for id 35, which is word 1.
        let big = Domain {
            reset_assert: 0x10,
            reset_status: 0x20,
            resets: 128,
            clocks: 0,
        };
        let r = big.reset_bit(35).unwrap();
        assert_eq!(r.assert_offset, 0x10 + 4, "word 1: (35 / 32) * 4 == 4");
        assert_eq!(r.status_offset, 0x20 + 4);
        assert_eq!(r.mask, 1 << 3, "35 % 32 == 3");
    }

    #[test]
    fn the_firmware_having_already_done_it_is_a_distinguishable_outcome() {
        // The one result that would refute this milestone's premise: the clocks were already on
        // and the reset already released before anything here ran, so the TRNG's zeros have some
        // other cause. A bench transcript has to be able to say this, which is why the report
        // keeps `before` words at all.
        let up = Report {
            clock_before: [CLOCK_ENABLE | 4, CLOCK_ENABLE | 4, 0, 0],
            clock_after: [CLOCK_ENABLE | 4, CLOCK_ENABLE | 4, 0, 0],
            clocks: 2,
            had_reset: true,
            reset_assert_before: 0,
            reset_assert_after: 0,
            reset_status_after: 1 << STGRST_SEC_AHB,
            released: true,
            polls: 1,
            ..Report::default()
        };
        assert!(up.has_clocks_running());
        assert!(up.was_already_up());
    }

    #[test]
    fn a_device_this_run_actually_started_is_not_reported_as_already_up() {
        let started = Report {
            clock_before: [0, 0, 0, 0],
            clock_after: [CLOCK_ENABLE, CLOCK_ENABLE, 0, 0],
            clocks: 2,
            had_reset: true,
            reset_assert_before: 1 << STGRST_SEC_AHB,
            reset_assert_after: 0,
            reset_status_after: 1 << STGRST_SEC_AHB,
            released: true,
            polls: 3,
            ..Report::default()
        };
        assert!(started.has_clocks_running());
        assert!(!started.was_already_up(), "this run is what turned it on");
    }

    #[test]
    fn the_trng_plan_enables_both_clocks_before_it_touches_the_reset() {
        // [mainline-rst]'s own comment is the reason this order is a test rather than a comment:
        // "if the associated clock is gated, deasserting might otherwise hang forever". A plan
        // that deasserted first would wedge the boot on hardware and pass every host test.
        let reset_at = TRNG_BRING_UP
            .iter()
            .position(|s| matches!(s, Step::DeassertReset(_)))
            .expect("the plan must deassert something");
        let clocks = TRNG_BRING_UP
            .iter()
            .filter(|s| matches!(s, Step::EnableClock(_)))
            .count();
        assert_eq!(clocks, 2, "hclk and ahb, per [mainline-trng]'s probe");
        assert_eq!(reset_at, 2, "the reset comes last");
    }

    #[test]
    fn the_two_trees_agree_on_the_identifiers() {
        // The rebase arithmetic from [vendor-ids], written out so a reader can check it rather
        // than take the module header's word for it. If StarFive ever renumbers a group, this is
        // the test that fails.
        const VENDOR_STG_CLK_BASE: u32 = 190; // JH7110_HIFI4_CLK_CORE
        const VENDOR_SEC_HCLK: u32 = 205;
        const VENDOR_SEC_MISCAHB_CLK: u32 = 206;
        const VENDOR_STG_RST_BASE: u32 = 128; // RSTN_U0_STG_SYSCON_PRESETN
        const VENDOR_SEC_TOP_HRESETN: u32 = 131;

        assert_eq!(VENDOR_SEC_HCLK - VENDOR_STG_CLK_BASE, STGCLK_SEC_AHB);
        assert_eq!(
            VENDOR_SEC_MISCAHB_CLK - VENDOR_STG_CLK_BASE,
            STGCLK_SEC_MISC_AHB
        );
        assert_eq!(VENDOR_SEC_TOP_HRESETN - VENDOR_STG_RST_BASE, STGRST_SEC_AHB);
    }

    #[test]
    fn clock_words_are_one_per_index_and_bounded() {
        assert_eq!(STG.clock_offset(0), Some(0x00));
        assert_eq!(STG.clock_offset(STGCLK_SEC_AHB), Some(0x3c));
        assert_eq!(STG.clock_offset(STGCLK_SEC_MISC_AHB), Some(0x40));
        // 29 clocks, so 28 is the last. An identifier from another domain gets nothing rather
        // than an offset that lands somewhere plausible.
        assert_eq!(STG.clock_offset(28), Some(0x70));
        assert_eq!(STG.clock_offset(29), None);
        assert_eq!(STG.clock_offset(205), None); // the vendor number, un-rebased
    }

    #[test]
    fn the_last_clock_word_does_not_reach_the_reset_words() {
        // 0x70 is the last clock word and 0x74 is the assert register. They abut, which means an
        // off-by-one in `clocks` would write an enable bit into a reset word: a stuck-on clock
        // would be the *good* outcome, and resetting the STG matrix mid-boot the bad one.
        let last = STG.clock_offset(STG.clocks - 1).unwrap();
        assert!(last + 4 <= STG.reset_assert);
    }

    #[test]
    fn resets_are_thirty_two_to_a_word_and_bounded() {
        let r = STG.reset_bit(STGRST_SEC_AHB).unwrap();
        assert_eq!(r.assert_offset, 0x74);
        assert_eq!(r.status_offset, 0x78);
        assert_eq!(r.mask, 1 << 3);
        assert_eq!(STG.reset_bit(22).unwrap().mask, 1 << 22);
        assert_eq!(STG.reset_bit(23), None);
        assert_eq!(STG.reset_bit(131), None); // the vendor number, un-rebased
    }

    #[test]
    fn a_set_status_bit_means_out_of_reset() {
        let mask = STG.reset_bit(STGRST_SEC_AHB).unwrap().mask;
        assert!(is_deasserted(u32::MAX, mask));
        assert!(!is_deasserted(0, mask));
        assert!(
            !is_deasserted(!mask, mask),
            "every other bit must not count"
        );
    }

    #[test]
    fn an_enabled_clock_reads_its_top_bit_back() {
        assert!(is_clock_enabled(CLOCK_ENABLE));
        assert!(is_clock_enabled(CLOCK_ENABLE | 0x1234));
        assert!(!is_clock_enabled(0x7fff_ffff));
        // Zero is what radon's TRNG window read on 2026-09-04, and it is what a gated clock
        // reads: the same value a window with nothing behind it reads, which is why the tour
        // reports `from_tree` and the before/after words rather than a verdict.
        assert!(!is_clock_enabled(0));
    }

    #[test]
    fn reg_names_index_by_name_not_by_position() {
        assert_eq!(name_index(b"sys\0stg\0aon\0", b"stg"), Some(1));
        assert_eq!(
            name_index(b"syscrg\0stgcrg\0aoncrg\0ispcrg\0voutcrg\0", b"stgcrg"),
            Some(1)
        );
        assert_eq!(name_index(b"sys\0aon\0", b"stg"), None);
        // A trailing NUL must not invent an empty fourth entry.
        assert_eq!(name_index(b"sys\0stg\0aon\0", b"aon"), Some(2));
        assert_eq!(name_index(b"", b"stg"), None);
    }

    #[test]
    fn radons_aon_window_is_the_vendor_nodes_third_entry_found_by_name() {
        let tree = device_tree_blob::DeviceTreeBlob::from_bytes(CLKGEN_VENDOR).unwrap();
        let found = discover_aon(&tree).unwrap();
        assert_eq!((found.base, found.size), (AON_BASE, AON_SIZE));
        assert!(found.from_tree);
        assert_eq!(found.compatible, Some(COMPATIBLE_VENDOR_CLKGEN));
    }

    #[test]
    fn a_tree_naming_no_aon_controller_gets_the_constant_and_says_so() {
        let tree = device_tree_blob::DeviceTreeBlob::from_bytes(STGCRG_MAINLINE).unwrap();
        let found = discover_aon(&tree).unwrap();
        assert_eq!(found.base, AON_BASE);
        assert!(!found.from_tree);
    }

    #[test]
    fn gmac0s_plans_ungate_every_clock_before_either_reset_and_pick_the_parent_while_gated() {
        let first_reset = GMAC0_AON_BRING_UP
            .iter()
            .position(|s| matches!(s, Step::DeassertReset(_)))
            .unwrap();
        assert!(
            GMAC0_AON_BRING_UP[first_reset..]
                .iter()
                .all(|s| matches!(s, Step::DeassertReset(_)))
        );
        assert!(
            GMAC0_SYS_BRING_UP
                .iter()
                .all(|s| matches!(s, Step::EnableClock(_)))
        );
        let mux = GMAC0_AON_BRING_UP
            .iter()
            .position(|s| matches!(s, Step::SelectParent { .. }))
            .unwrap();
        let enable = GMAC0_AON_BRING_UP
            .iter()
            .position(|s| *s == Step::EnableClock(AONCLK_GMAC0_TX))
            .unwrap();
        assert!(mux < enable);
        // Every identifier is inside its domain, so neither plan can be rejected.
        for s in GMAC0_AON_BRING_UP {
            match *s {
                Step::EnableClock(i) | Step::SelectParent { clock: i, .. } => {
                    assert!(AON.clock_offset(i).is_some());
                }
                Step::DeassertReset(i) => assert!(AON.reset_bit(i).is_some()),
            }
        }
        for s in GMAC0_SYS_BRING_UP {
            if let Step::EnableClock(i) = *s {
                assert!(SYS.clock_offset(i).is_some());
            }
        }
    }

    #[test]
    fn the_word_offsets_match_the_vendor_u_boots_own_constants() {
        // radon's U-Boot writes `AON_CRG_BASE + 0x14` for gmac0's transmit clock and `0x1c` for
        // its receive clock (`jh7110-regs.h`): indices 5 and 7, one word each.
        assert_eq!(AON.clock_offset(AONCLK_GMAC0_TX), Some(0x14));
        assert_eq!(AON.clock_offset(7), Some(0x1c));
        assert_eq!(AON.reset_bit(AONRST_GMAC0_AHB).unwrap().mask, 1 << 1);
        assert_eq!(SYS.clock_offset(SYSCLK_GMAC0_GTXC), Some(0x1bc));
    }

    #[test]
    fn selecting_a_parent_keeps_the_enable_bit_and_the_divider() {
        let running = CLOCK_ENABLE | 0x0000_0008;
        assert_eq!(with_parent(running, 1), CLOCK_ENABLE | 1 << 24 | 8);
        assert_eq!(with_parent(running | 1 << 24, 0), running);
        // A parent too wide for the four-bit field cannot reach the enable bit.
        assert_eq!(with_parent(0, 0xff) & CLOCK_ENABLE, 0);
    }

    // ===== Option B: the rate chain, the timing formula, and the two tree facts (milestone 592) =====

    #[test]
    fn radons_pmic_bus_also_names_the_controller_window_and_the_pmic_address() {
        let tree = device_tree_blob::DeviceTreeBlob::from_bytes(PMIC_BUS_RADON).unwrap();
        let bus = pmic_bus(&tree).unwrap();
        assert_eq!(bus.controller, Some((0x1205_0000, 0x1_0000)));
        assert_eq!(bus.pmic_address, Some(0x36));
    }

    #[test]
    fn the_mainline_tree_names_the_same_controller_and_address() {
        let tree = device_tree_blob::DeviceTreeBlob::from_bytes(PMIC_BUS_MAINLINE).unwrap();
        let bus = pmic_bus(&tree).unwrap();
        assert_eq!(bus.controller, Some((0x1205_0000, 0x1_0000)));
        assert_eq!(bus.pmic_address, Some(0x36));
    }

    #[test]
    fn the_foreign_tree_names_neither() {
        let tree = device_tree_blob::DeviceTreeBlob::from_bytes(PMIC_BUS_FOREIGN).unwrap();
        let bus = pmic_bus(&tree).unwrap();
        assert_eq!(bus.controller, None);
        assert_eq!(bus.pmic_address, None);
    }

    #[test]
    fn a_one_based_divider_field_reading_zero_divides_by_one() {
        assert_eq!(one_based_div(0, 2), 1);
        assert_eq!(one_based_div(0, 4), 1);
        assert_eq!(one_based_div(3, 2), 3);
        assert_eq!(one_based_div(0x1f, 4), 15);
        assert_eq!(one_based_div(u32::MAX, 31), (1u64 << 31) - 1);
    }

    #[test]
    fn the_osc_root_chain_divides_and_the_pll2_chain_multiplies() {
        // bus_root on osc, every divider at 1: the chain is transparent.
        let osc = IcClkWords {
            bus_root: 0,
            axi_cfg0: 1,
            stg_axiahb: 1,
            apb_bus_func: 1,
            pll2_dacpd_dsmpd_fbdiv: 0,
            pll2_postdiv1: 0,
            pll2_prediv: 0,
        };
        assert_eq!(i2c5_ic_clk(&osc), OSC_HZ);

        // bus_root on pll2 in integer mode. The fields are the vendor PLL2 table's 1228.8 MHz row
        // (fbdiv 768, prediv 15, postdiv1 1, dacpd 1, dsmpd 1, from the pll.c this crate cites),
        // laid into the words at the syscon's offsets: 0x2c carries dsmpd<<16 | dacpd<<15 |
        // fbdiv<<17, 0x30 carries postdiv1<<28, 0x34 carries prediv. The dividers are field
        // values, not words: the 2-bit fields hold 2 and 2, the 4-bit field 12, so the chain is
        // 1_228_800_000 / 2 / 2 / 12.
        let pll2 = IcClkWords {
            bus_root: 1 << 24,
            axi_cfg0: 2,
            stg_axiahb: 2,
            apb_bus_func: 12,
            pll2_dacpd_dsmpd_fbdiv: (1 << 15) | (1 << 16) | (768 << 17),
            pll2_postdiv1: 0,
            pll2_prediv: 15,
        };
        assert_eq!(i2c5_ic_clk(&pll2), 1_228_800_000 / 2 / 2 / 12);

        // A zero divider field is treated as divide-by-one rather than a panic, because the words
        // belong to firmware and a print of the computed rate is the transcript's fact.
        let zeros = IcClkWords {
            bus_root: 0,
            axi_cfg0: 0,
            stg_axiahb: 0,
            apb_bus_func: 0,
            pll2_dacpd_dsmpd_fbdiv: 0,
            pll2_postdiv1: 0,
            pll2_prediv: 0,
        };
        assert_eq!(i2c5_ic_clk(&zeros), OSC_HZ);

        // Fractional mode (dacpd 0, dsmpd 0) is the vendor's default rate, not a guess from the
        // fields: 1_188_000_000 divided by the chain.
        let frac = IcClkWords {
            bus_root: 1 << 24,
            axi_cfg0: 1,
            stg_axiahb: 1,
            apb_bus_func: 1,
            pll2_dacpd_dsmpd_fbdiv: 768 << 17,
            pll2_postdiv1: 0,
            pll2_prediv: 15,
        };
        assert_eq!(i2c5_ic_clk(&frac), PLL2_DEFAULT_HZ);
    }

    #[test]
    fn the_standard_mode_timing_is_the_vendor_formula_worked_by_hand() {
        // 50 MHz in, the formula's own numbers: rise = ceil(50e6 * 1000ns / 1e9) = 50 counts,
        // fall = 15, thigh = 200, tlow = 235, period = 500. Then
        // hcnt = 200 - 15 - 7 = 178, lcnt = 235 - 50 + 15 - 1 = 199, tot = 178 + 199 + 7 + 50 + 1
        // = 435 < 500, so diff = 32 and lcnt takes the remainder 1: (210, 232). sda_hold is
        // ceil(300ns / 20ns) = 15.
        let t = standard_mode_100k(50_000_000);
        assert_eq!((t.hcnt, t.lcnt, t.sda_hold), (210, 232, 15));
        assert_eq!(t.con, 0b0110_0011);
        // Every count legal and the mode word exactly the vendor's four init bits.
        assert!(t.hcnt > 0 && t.lcnt > 0);
    }

    #[test]
    fn a_slow_input_clock_still_produces_legal_counts() {
        // 1 MHz: every minimum-time count is 1, the period is 10, and the formula must not wrap.
        let t = standard_mode_100k(1_000_000);
        assert!(t.hcnt > 0 && t.lcnt > 0 && t.sda_hold > 0);
        // Below 100 kHz of input the period clamps to 1 rather than dividing by zero.
        let t = standard_mode_100k(1_000);
        assert!(t.hcnt > 0 && t.lcnt > 0);
    }

    #[test]
    fn reg_cells_parse_in_both_spellings() {
        // The vendor and mainline bus nodes: <0x0 0x12050000 0x0 0x10000>.
        let four = 0x0000_0000u32
            .to_be_bytes()
            .iter()
            .chain(0x1205_0000u32.to_be_bytes().iter())
            .chain(0x0000_0000u32.to_be_bytes().iter())
            .chain(0x0001_0000u32.to_be_bytes().iter())
            .copied()
            .collect::<Vec<u8>>();
        assert_eq!(parse_reg(&four), Some((0x1205_0000, 0x1_0000)));
        // A one-address-cell spelling: <0x12050000 0x10000>.
        let two = 0x1205_0000u32
            .to_be_bytes()
            .iter()
            .chain(0x0001_0000u32.to_be_bytes().iter())
            .copied()
            .collect::<Vec<u8>>();
        assert_eq!(parse_reg(&two), Some((0x1205_0000, 0x1_0000)));
        // A truncated property is not a window.
        assert_eq!(parse_reg(&[0, 0, 0]), None);
    }
}
