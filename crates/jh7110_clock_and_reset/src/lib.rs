#![cfg_attr(not(test), no_std)]
//! The `StarFive` JH7110's clock and reset generator, as pure logic: register offsets, bit
//! positions, the bring-up plans, and what each of the chip's two device trees calls the
//! controllers.
//!
//! This never dereferences a pointer, so it is host-testable and Kani-reachable; the volatile
//! stores are the caller's. Every number is transcribed from Linux mainline ([mainline-dts],
//! [mainline-trng], [mainline-ids], [mainline-clk], [mainline-rst]) and the vendor U-Boot
//! ([vendor-dts], [vendor-ids]), which agree. One word per clock at `base + 4 * idx` with enable
//! bit 31; a set reset-status bit means out of reset ([`is_deasserted`]).
//!
//! # The two device trees
//!
//! The crate reads no device tree. The caller finds a node and hands this crate its property
//! values. Mainline names one controller node per domain (`starfive,jh7110-stgcrg`, `-aoncrg`,
//! `-syscrg`); the vendor U-Boot names one clock node (`starfive,jh7110-clkgen`) and one reset
//! node (`starfive,jh7110-reset`), each carrying several windows told apart by `reg-names`. Each
//! [`Window`] lists its spellings in the order to try them, and [`Found::from_regions`] picks the
//! window out of a matched node's `reg`. [`PmicBus::from_bus`] and [`DevicePlan::from_specifiers`]
//! turn a node's `clocks` and `resets` into steps, asking the caller to look each phandle up.
//!
//! [mainline-dts]: https://github.com/torvalds/linux/blob/master/arch/riscv/boot/dts/starfive/jh7110.dtsi
//! [mainline-trng]: https://github.com/torvalds/linux/blob/master/drivers/char/hw_random/jh7110-trng.c
//! [mainline-ids]: https://github.com/torvalds/linux/blob/master/include/dt-bindings/reset/starfive%2Cjh7110-crg.h
//! [mainline-clk]: https://github.com/torvalds/linux/blob/master/drivers/clk/starfive/clk-starfive-jh71x0.c
//! [mainline-rst]: https://github.com/torvalds/linux/blob/master/drivers/reset/starfive/reset-starfive-jh7110.c
//! [vendor-dts]: https://github.com/starfive-tech/u-boot/blob/JH7110_VisionFive2_devel/arch/riscv/dts/jh7110.dtsi
//! [vendor-ids]: https://github.com/starfive-tech/u-boot/blob/JH7110_VisionFive2_devel/include/dt-bindings/clock/starfive-jh7110-clkgen.h
//!
//! Name: ratified 2026-09-13 (calef), replacing `jh7110_crg`; kept as the crates.io name (#1806).
//! The argument when the name `jh7110_crg` was minted (the lane of milestone 220 (this kernel
//! drives no clock or reset controller, and the first real device will need one), 2026-09-04):
//! `crg` is the JH7110's own word for the thing, not an abbreviation this project coined: both
//! device trees that describe this chip spell the blocks `syscrg`, `stgcrg` and `aoncrg`
//! (mainline's `starfive,jh7110-stgcrg` compatible, the vendor U-Boot's `reg-names = "syscrg",
//! "stgcrg", ...`), so it is the AGENTS.md "standard term a reader already knows from outside" case
//! rather than the `capsh`/`uheap` case. Chip-qualified for the reason `jh7110_entropy` is: a bare
//! `crg` would name almost anything.

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
    /// `index << JH71X0_CLK_MUX_SHIFT` under `JH71X0_CLK_MUX_MASK`. Taken before the same clock's
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
pub(crate) const CLOCK_MUX_MASK: u32 = 0xf << CLOCK_MUX_SHIFT;

/// Where [`CLOCK_MUX_MASK`]'s field starts (`JH71X0_CLK_MUX_SHIFT`).
pub(crate) const CLOCK_MUX_SHIFT: u32 = 24;

/// **A clock word with its parent replaced**, leaving the enable bit, the divider and every other
/// field as they were. A parent too wide for the field is reduced to it rather than spilling into
/// the enable bit.
#[must_use]
pub const fn with_parent(word: u32, parent: u32) -> u32 {
    (word & !CLOCK_MUX_MASK) | ((parent << CLOCK_MUX_SHIFT) & CLOCK_MUX_MASK)
}

/// **What the JH7110's TRNG needs before its registers answer**, transcribed from
/// \[mainline-trng\]'s probe and cross-checked against \[vendor-dts\]: clocks 15 and 16, then
/// reset 3, all of the STG domain, so the plan is meaningless without [`STG`].
///
/// # Examples
///
/// ```
/// use jh7110_clock_and_reset::{STG, Step, TRNG_BRING_UP, CLOCK_ENABLE, is_deasserted};
///
/// // The TRNG's plan is two clocks then one reset, in that order, which is the order
/// // Linux's own probe takes: a reset deassert against a gated clock can hang forever.
/// assert_eq!(
///     TRNG_BRING_UP,
///     &[Step::EnableClock(15), Step::EnableClock(16), Step::DeassertReset(3)]
/// );
///
/// // Clock 15 lives one word per clock from the domain's base.
/// assert_eq!(STG.clock_offset(15), Some(0x3c));
/// assert_eq!(CLOCK_ENABLE, 1 << 31);
///
/// // Reset 3 is bit 3 of the word at 0x74, watched at 0x78. A SET status bit means
/// // "out of reset", which is the opposite of what the register name suggests.
/// let r = STG.reset_bit(3).unwrap();
/// assert_eq!((r.assert_offset, r.status_offset, r.mask), (0x74, 0x78, 1 << 3));
/// assert!(is_deasserted(0b1000, r.mask));
/// assert!(!is_deasserted(0b0111, r.mask));
/// ```
pub const TRNG_BRING_UP: &[Step] = &[
    Step::EnableClock(STGCLK_SEC_AHB),
    Step::EnableClock(STGCLK_SEC_MISC_AHB),
    Step::DeassertReset(STGRST_SEC_AHB),
];

/// `JH7110_STGCLK_SEC_AHB` \[mainline-ids\]; the vendor tree's `JH7110_SEC_HCLK` (205) rebased on
/// its stg group's start (190). The TRNG's `hclk`.
pub(crate) const STGCLK_SEC_AHB: u32 = 15;

/// `JH7110_STGCLK_SEC_MISC_AHB` \[mainline-ids\]; the vendor tree's `JH7110_SEC_MISCAHB_CLK` (206)
/// rebased the same way. The TRNG's second clock, `ahb` to mainline and `miscahb_clk` to the
/// vendor, which is the same wire under two names.
pub(crate) const STGCLK_SEC_MISC_AHB: u32 = 16;

/// `JH7110_STGRST_SEC_AHB` \[mainline-ids\]; the vendor tree's `RSTN_U0_SEC_TOP_HRESETN` (131)
/// rebased on its stg group's start (128).
///
/// **It is a *shared* reset** (\[mainline-trng\] takes it with `devm_reset_control_get_shared`),
/// which is a fact about the silicon rather than about Linux's API: the same line resets the whole
/// security top block, the PL080 DMA at `0x1600_8000` included. Deasserting it is safe; asserting
/// it would reset a neighbour, which is why nothing in this crate offers an assert.
pub const STGRST_SEC_AHB: u32 = 3;

/// **The AON (always-on) domain**, where `gmac0`'s bus clocks, its transmit clock mux and both
/// of its resets live.
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
pub(crate) const AON_BASE: u64 = 0x1700_0000;

/// The AON window's size, `0x10000` in every tree that names it.
pub(crate) const AON_SIZE: u64 = 0x1_0000;

/// Mainline's dedicated AON clock-and-reset controller node, `aoncrg: clock-controller@17000000`.
///
/// Name: provisional, made public 2026-10-25 (UTC) by lane crates-stand-alone.
pub const COMPATIBLE_AONCRG: &[u8] = b"starfive,jh7110-aoncrg";

/// The `reg-names` entry naming the AON window in each vendor node's spelling (`"aon"` in
/// `clkgen`, `"aoncrg"` in `rstgen`; \[vendor-dts\]).
const VENDOR_CLKGEN_AON_NAME: &[u8] = b"aon";
const VENDOR_RSTGEN_AON_NAME: &[u8] = b"aoncrg";

/// `JH7110_AONCLK_GMAC0_AHB` \[mainline-ids\]: a gate, `gmac0`'s `pclk`. Word `0x08`.
pub(crate) const AONCLK_GMAC0_AHB: u32 = 2;
/// `JH7110_AONCLK_GMAC0_AXI`: a gate, `gmac0`'s `stmmaceth` (its CSR and bus clock). Word `0x0c`.
pub(crate) const AONCLK_GMAC0_AXI: u32 = 3;
/// `JH7110_AONCLK_GMAC0_TX`: a gate with a two-way mux, `gmac0_gtxclk` (0) or
/// `gmac0_rmii_rtx` (1). Word `0x14`, which radon's vendor U-Boot calls `GMAC5_0_CLK_TX_SHIFT`
/// and sets bit 24 of (`jh7110_gmac_sel_tx_to_rgmii`, `board/starfive/visionfive2/`).
pub(crate) const AONCLK_GMAC0_TX: u32 = 5;
/// `gmac0_tx`'s parent on a VisionFive 2 v1.3B: `gmac0_rmii_rtx`, mainline's
/// `assigned-clock-parents` for `&gmac0` in `jh7110-starfive-visionfive-2-v1.3b.dts` beside
/// `starfive,tx-use-rgmii-clk`, and the same bit the vendor U-Boot sets. radon's EEPROM says PCB
/// revision `0xb2`, a v1.3B (bench transcripts, `PCB revision: 0xb2`).
pub(crate) const AONCLK_GMAC0_TX_PARENT_RMII_RTX: u32 = 1;
/// `JH7110_AONRST_GMAC0_AXI` \[mainline-ids\], `gmac0`'s `stmmaceth` reset. Bit 0 at `0x38`.
pub(crate) const AONRST_GMAC0_AXI: u32 = 0;
/// `JH7110_AONRST_GMAC0_AHB`, `gmac0`'s `ahb` reset. Bit 1 at `0x38`.
pub(crate) const AONRST_GMAC0_AHB: u32 = 1;

/// `JH7110_SYSCLK_GMAC0_GTXCLK` \[mainline-ids\]: a gated divider of PLL0, the parent of
/// `gmac0_gtxc`. Word `0x1b0` of the SYS window.
pub(crate) const SYSCLK_GMAC0_GTXCLK: u32 = 108;
/// `JH7110_SYSCLK_GMAC0_PTP`: a gated divider, `gmac0`'s `ptp_ref`. Word `0x1b4`.
pub(crate) const SYSCLK_GMAC0_PTP: u32 = 109;
/// `JH7110_SYSCLK_GMAC0_GTXC`: a gate, `gmac0`'s `gtx`. Word `0x1bc`.
pub(crate) const SYSCLK_GMAC0_GTXC: u32 = 111;

/// **`gmac0`'s SYS-domain clocks**, the half of its clocks that are not in the AON domain.
/// Every clock mainline's `&gmac0` node names that lives in SYS, with the parent
/// of `gtxc` first because enabling a gate under a gated parent turns nothing on. Run before
/// [`GMAC0_AON_BRING_UP`], whose resets must be released with every clock already running.
pub const GMAC0_SYS_BRING_UP: &[Step] = &[
    Step::EnableClock(SYSCLK_GMAC0_GTXCLK),
    Step::EnableClock(SYSCLK_GMAC0_GTXC),
    Step::EnableClock(SYSCLK_GMAC0_PTP),
];

/// **`gmac0`'s AON-domain clocks, its transmit clock's parent, and both its resets**, in Linux's
/// probe order: OpenBSD's `dwqe_fdt_attach` and mainline's `dwmac-starfive.c`
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
/// The JH7110 has five ([vendor-dts]'s `reg-names = "syscrg", "stgcrg", "aoncrg", "ispcrg",
/// "voutcrg"`); [`STG`], [`AON`] and [`SYS`] are described here, each only as far as a device in
/// this tree needs it. The arithmetic is general, so another domain is a table entry rather than a
/// rewrite.
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
/// The fallback [`STG_WINDOW`] answers with when the tree names no controller, and
/// [`Found::from_tree`] says which happened. A firmware tree may omit or misdescribe the
/// controller, and two independently published trees agree on this number. A caller must not
/// store through a fallback on a machine that is not a JH7110: QEMU's riscv64 `virt` has nothing
/// mapped there.
pub(crate) const STG_BASE: u64 = 0x1023_0000;

/// The STG window's size, `0x10000` in both trees.
pub(crate) const STG_SIZE: u64 = 0x1_0000;

/// **The SYS domain**, which is where I2C5's clock and reset live, and so where the only road to
/// a VisionFive 2's PMIC starts.
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
pub(crate) const SYS_BASE: u64 = 0x1302_0000;

/// The SYS window's size, `0x10000` in every tree that names it.
pub(crate) const SYS_SIZE: u64 = 0x1_0000;

/// `JH7110_SYSCLK_I2C5_APB` \[mainline-ids\], and `JH7110_I2C5_CLK_APB` in the vendor header,
/// both **143**: the vendor numbers the SYS group first, so no rebase is needed. Word `0x23c`.
///
/// **This is I2C5's only gate.** Radon's U-Boot also names a `u5_dw_i2c_clk_core` (vendor id 298)
/// and prints it at `Starting kernel`, but its own clock driver registers that as
/// `starfive_clk_fix_factor(..., "u5_dw_i2c_clk_core", "u5_dw_i2c_clk_apb", 1, 1)`, a
/// divide-by-one child of this gate with no register of its own, and mainline's `DesignWare` node
/// names this clock alone. So there is no second gate to turn on, and 298 is not a word to write.
pub(crate) const SYSCLK_I2C5_APB: u32 = 143;

/// `JH7110_SYSRST_I2C5_APB` \[mainline-ids\], and `RSTN_U5_DW_I2C_APB` in the vendor header, both
/// **81**. Bit 17 of the word at `0x300`, watched at `0x310`.
///
/// **This is the line the proposal did not name, and probably the cause.** Radon's U-Boot removes
/// its I2C driver before handing over (`DM_FLAG_OS_PREPARE`), and `designware_i2c_remove` ends in
/// `reset_release_bulk`, whose `reset_release_all` *asserts* every reset before freeing it. Radon's
/// OpenSBI re-enables a clock before its I2C transfer and never touches a reset. A controller held
/// in reset reads `IC_STATUS` as zero, so its transmit-FIFO-empty poll can never succeed, which is
/// the ten `i2c read: write daddr 36 to` lines in radon's 2026-09-04 log.
pub(crate) const SYSRST_I2C5_APB: u32 = 81;

/// **What I2C5 needs before OpenSBI can reach the PMIC**, when the tree does not say. Clock first,
/// then reset, for [`TRNG_BRING_UP`]'s reason: Linux's reset driver warns that a deassert against a
/// gated clock "might otherwise hang forever".
pub(crate) const PMIC_BUS_BRING_UP: &[Step] = &[
    Step::EnableClock(SYSCLK_I2C5_APB),
    Step::DeassertReset(SYSRST_I2C5_APB),
];

/// `JH7110_SYSCLK_SDIO0_AHB` \[mainline-ids\], the vendor header's `JH7110_SDIO0_CLK_AHB`, both
/// **91**: the eMMC socket's controller's bus clock, `biu` in both trees. Word `0x16c`.
pub(crate) const SYSCLK_SDIO0_AHB: u32 = 91;
/// `JH7110_SYSCLK_SDIO1_AHB`, vendor `JH7110_SDIO1_CLK_AHB`, both **92**: the microSD slot's
/// controller's bus clock. Word `0x170`.
pub(crate) const SYSCLK_SDIO1_AHB: u32 = 92;
/// `JH7110_SYSCLK_SDIO0_SDCARD`, vendor `JH7110_SDIO0_CLK_SDCARD`, both **93**: the eMMC
/// controller's card clock source, `ciu`, a gated divider both trees assign 50 MHz. Word `0x174`.
pub(crate) const SYSCLK_SDIO0_SDCARD: u32 = 93;
/// `JH7110_SYSCLK_SDIO1_SDCARD`, vendor `JH7110_SDIO1_CLK_SDCARD`, both **94**: the microSD
/// controller's `ciu`. Word `0x178`.
pub(crate) const SYSCLK_SDIO1_SDCARD: u32 = 94;
/// `JH7110_SYSRST_SDIO0_AHB` \[mainline-ids\]: the eMMC controller's one reset. Bit 0 of the
/// word at `0x300`.
pub(crate) const SYSRST_SDIO0_AHB: u32 = 64;
/// `JH7110_SYSRST_SDIO1_AHB`: the microSD controller's one reset. Bit 1 of the word at `0x300`.
pub(crate) const SYSRST_SDIO1_AHB: u32 = 65;

/// **What each SD/MMC controller needs before its registers answer**, slot 0 (the eMMC socket) then
/// slot 1 (the microSD slot): both clocks mainline's node names, then its reset, in
/// [`TRNG_BRING_UP`]'s order and for its reason. OpenBSD's `dwmmc_attach` takes the same order
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

/// The PMIC as radon's vendor tree spells it (U-Boot SDK `VF2_v2.10.4`, `starfive_visionfive2.dts`:
/// `pmic: axp15060_reg@36 { compatible = "stf,axp15060-regulator"; reg = <0x36>; }` under
/// `&i2c5`). This is also the string radon's OpenSBI matches to find its reset device.
///
/// Name: provisional, made public 2026-10-25 (UTC) by lane crates-stand-alone.
pub const COMPATIBLE_PMIC_VENDOR: &[u8] = b"stf,axp15060-regulator";

/// The same PMIC in mainline's `jh7110-common.dtsi`: `axp15060: pmic@36 { compatible =
/// "x-powers,axp15060"; }` under `&i2c5`.
///
/// Name: provisional, made public 2026-10-25 (UTC) by lane crates-stand-alone.
pub const COMPATIBLE_PMIC_MAINLINE: &[u8] = b"x-powers,axp15060";

/// Both PMIC spellings, in the order to try them: mainline's first.
///
/// Name: provisional, minted 2026-10-25 (UTC) by lane crates-stand-alone, after
/// `jh7110_entropy::COMPATIBLES`: both spellings, in the order to try them.
pub const PMIC_COMPATIBLES: [&[u8]; 2] = [COMPATIBLE_PMIC_MAINLINE, COMPATIBLE_PMIC_VENDOR];

/// How many steps a tree-derived plan can hold. Radon's tree names two clocks and one reset, one
/// of those clocks virtual; four is slack, and a bus naming more is recorded as truncated.
pub(crate) const MAX_PMIC_BUS_STEPS: usize = 4;

/// **The plan for the PMIC's bus, and where it came from.**
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PmicBus {
    steps: [Step; MAX_PMIC_BUS_STEPS],
    len: usize,
    /// **True when the plan is the tree's own `clocks` and `resets` of the PMIC's parent bus**,
    /// false when it is the constant I2C5 plan (its clock 143, then its reset 81). The field a
    /// bench transcript must carry, for [`Found::from_tree`]'s reason.
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
    /// tree states it. The direct reset write needs it; a plan without one (the constant fallback,
    /// or a tree that names no `reg`) leaves it `None` and the kernel does not attempt the write.
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

impl PmicBus {
    /// **The plan when the tree names no known PMIC**: the constant I2C5 plan, with `from_tree`
    /// false and `pmic` `None`.
    ///
    /// Name: provisional, minted 2026-10-25 (UTC) by lane crates-stand-alone, for the case it
    /// answers.
    #[must_use]
    pub fn without_pmic() -> Self {
        PmicBus::fallback(None, 0)
    }

    /// **The plan for the PMIC `pmic`, from what its node and its parent bus node say.**
    ///
    /// `pmic` is the [`PMIC_COMPATIBLES`] entry the caller matched. `pmic_reg` is the PMIC node's
    /// own `reg` (its I2C address, one cell), and `bus_reg`, `clocks` and `resets` are its parent
    /// bus node's properties; each is `None` when the tree does not state it. A `<phandle id>`
    /// pair becomes a step only for a known one-cell SYS provider and an id inside [`SYS`]'s
    /// bounds (the vendor numbers SYS first, so one bound serves both trees); the rest are counted
    /// in [`PmicBus::skipped`]. `provider(phandle, name)` returns the property `name` of the node
    /// carrying `phandle`, as the caller's device-tree reader finds it. With no usable step the
    /// answer is the constant plan with `from_tree` false. An answer is not evidence of a JH7110.
    ///
    /// # Errors
    ///
    /// Propagates the first error `provider` returns.
    ///
    /// Name: provisional, minted 2026-10-25 (UTC) by lane crates-stand-alone, when the device-tree
    /// walk left the crate: the plan from what the PMIC's bus node says.
    pub fn from_bus<'t, E>(
        pmic: &'static [u8],
        pmic_reg: Option<&[u8]>,
        bus_reg: Option<&[u8]>,
        clocks: Option<&[u8]>,
        resets: Option<&[u8]>,
        mut provider: impl FnMut(u32, &[u8]) -> Result<Option<&'t [u8]>, E>,
    ) -> Result<Self, E> {
        let mut bus = PmicBus::fallback(Some(pmic), 0);
        // The PMIC's own `reg` is an I2C child address: one cell, size zero by `#size-cells 0`.
        // Both of radon's trees spell it `<0x36>`.
        if let Some(cells) = pmic_reg
            && cells.len() >= 4
        {
            bus.pmic_address = Some(u32::from_be_bytes([cells[0], cells[1], cells[2], cells[3]]));
        }
        if let Some(cells) = bus_reg {
            bus.controller = parse_reg(cells);
        }
        if clocks.is_none() && resets.is_none() {
            // A bus that names nothing keeps the constant plan, and the addresses it does state.
            return Ok(bus);
        }
        let collected = collect_sys_steps(clocks, resets, &mut bus.steps, &mut provider)?;
        bus.len = collected.len;
        bus.skipped = collected.skipped;
        bus.truncated = collected.truncated;
        if bus.len == 0 {
            return Ok(PmicBus::fallback(Some(pmic), bus.skipped));
        }
        bus.from_tree = true;
        Ok(bus)
    }
}

/// What [`collect_sys_steps`] filled in and what it left out.
struct Collected {
    len: usize,
    skipped: usize,
    truncated: bool,
}

/// **Turn a node's `clocks` and `resets` into SYS-domain steps**, clocks first, into `steps`.
/// The one walk [`PmicBus::from_bus`] and [`DevicePlan::from_specifiers`] share, so a specifier
/// is classified the same way whichever device named it: see [`Spec`] for what is kept, counted
/// or abandoned.
fn collect_sys_steps<'t, E>(
    clocks: Option<&[u8]>,
    resets: Option<&[u8]>,
    steps: &mut [Step],
    provider: &mut impl FnMut(u32, &[u8]) -> Result<Option<&'t [u8]>, E>,
) -> Result<Collected, E> {
    let mut out = Collected {
        len: 0,
        skipped: 0,
        truncated: false,
    };
    for (list, is_clock) in [(clocks, true), (resets, false)] {
        let Some(list) = list else { continue };
        let specs = list.chunks(8);
        let total = specs.len();
        for (i, spec) in specs.enumerate() {
            match sys_step(spec, is_clock, provider)? {
                Spec::Step(step) if out.len < steps.len() => {
                    steps[out.len] = step;
                    out.len += 1;
                }
                Spec::Step(_) => out.truncated = true,
                Spec::Foreign => out.skipped += 1,
                // The stride is wrong from here on, so nothing after this is a specifier.
                Spec::Unreadable => {
                    out.skipped += total - i;
                    break;
                }
            }
        }
    }
    Ok(out)
}

/// How many steps a [`DevicePlan`] holds. The watchdog names two clocks and two resets.
///
/// Name: provisional, minted 2026-10-20 (UTC) by milestone/593-radon-watchdog.
pub const MAX_DEVICE_STEPS: usize = 4;

/// **A device's own SYS-domain clocks and resets, as its node names them.** Unlike [`PmicBus`]
/// there is no constant fallback: an empty plan means the tree named nothing usable, and the
/// caller decides what that costs.
///
/// Name: provisional, minted 2026-10-20 (UTC) by milestone/593-radon-watchdog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DevicePlan {
    steps: [Step; MAX_DEVICE_STEPS],
    len: usize,
    /// Specifiers that are not a SYS-domain gate or line, counted and never written.
    pub skipped: usize,
    /// True when the node named more usable steps than [`MAX_DEVICE_STEPS`].
    pub truncated: bool,
}

impl DevicePlan {
    /// The steps to walk, clocks before resets.
    ///
    /// Name: provisional, minted 2026-10-20 (UTC) by milestone/593-radon-watchdog.
    #[must_use]
    pub fn plan(&self) -> &[Step] {
        &self.steps[..self.len]
    }

    /// **A device node's `clocks` and `resets`, as SYS-domain steps.** Either property may be
    /// `None`, and a node naming neither gives an empty plan. Specifiers are classified exactly
    /// as [`PmicBus::from_bus`] classifies its bus's, so a vendor `clkgen`/`rstgen` pair and
    /// mainline's `syscrg` both answer; `provider` is the same phandle lookup.
    ///
    /// # Errors
    ///
    /// Propagates the first error `provider` returns.
    ///
    /// Name: provisional, minted 2026-10-25 (UTC) by lane crates-stand-alone, when the device-tree
    /// walk left the crate: a plan built from a node's clock and reset specifiers.
    pub fn from_specifiers<'t, E>(
        clocks: Option<&[u8]>,
        resets: Option<&[u8]>,
        mut provider: impl FnMut(u32, &[u8]) -> Result<Option<&'t [u8]>, E>,
    ) -> Result<Self, E> {
        let mut plan = DevicePlan {
            steps: [Step::EnableClock(0); MAX_DEVICE_STEPS],
            len: 0,
            skipped: 0,
            truncated: false,
        };
        let collected = collect_sys_steps(clocks, resets, &mut plan.steps, &mut provider)?;
        plan.len = collected.len;
        plan.skipped = collected.skipped;
        plan.truncated = collected.truncated;
        Ok(plan)
    }
}

// ===== The direct reset write, and the numbers it needs =====
//
// On a VisionFive 2 whose firmware cannot reset the board, the bus can be provably up (clocks
// running, reset released) while OpenSBI's read of the PMIC still fails. The remedy is the kernel
// writing the AXP15060 itself over a DesignWare I2C master, which needs three numbers the tree and
// the CRG hold: the controller's base, the PMIC's address, and the I2C input clock that scales
// every timing count. Everything here is host-testable arithmetic over words the caller reads; the
// sources are the vendor trees cited in the crate's header.

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
pub(crate) const OSC_HZ: u64 = 24_000_000;

/// The vendor PLL code's default for a PLL it cannot interpret (`deffreq` for PLL2), returned
/// rather than guessed from fields that are not in integer mode.
pub(crate) const PLL2_DEFAULT_HZ: u64 = 1_188_000_000;

/// **The words the I2C input-clock chain reads**, one struct so the kernel can read them in one
/// place and the arithmetic can be tested without a device. The CRG
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
    axi_cfg0_hz(w) / one_based_div(w.stg_axiahb, 2) / one_based_div(w.apb_bus_func, 4)
}

/// **`axi_cfg0`'s rate**: `bus_root` (PLL2 or the oscillator, by its mux) over `axi_cfg0`'s own
/// divider. The parent of both the PMIC bus's clock chain ([`i2c5_ic_clk`]) and the SD/MMC
/// controllers' card clocks ([`SYSCLK_SDIO_SDCARD`]), each a registration in the vendor clock driver.
///
///
/// Name: provisional, minted 2026-10-21 (UTC) by milestone/53-radon-multi-block-read.
#[must_use]
pub const fn axi_cfg0_hz(w: &IcClkWords) -> u64 {
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
    parent / one_based_div(w.axi_cfg0, 2)
}

/// **Each SD/MMC controller's card clock** (`ciu` in the tree), by controller index: the eMMC
/// socket's, then the microSD slot's. Each divides [`axi_cfg0_hz`] by its own one-based divider.
///
/// Name: provisional, minted 2026-10-21 (UTC) by milestone/53-radon-multi-block-read.
pub const SYSCLK_SDIO_SDCARD: [u32; 2] = [SYSCLK_SDIO0_SDCARD, SYSCLK_SDIO1_SDCARD];

/// The card clocks' largest divider (the vendor driver registers them with a maximum of 15).
///
/// Name: provisional, minted 2026-10-21 (UTC) by milestone/53-radon-multi-block-read.
pub const SDCARD_DIVIDER_MAX: u32 = 15;

/// **The divider that brings a card clock to at most `want_hz`** from `parent_hz`: the smallest
/// that does, so the rate is as close below the ask as the divider allows, clamped to
/// `1..=`[`SDCARD_DIVIDER_MAX`]. The tree's `assigned-clock-rates` is a rate the OS is asked to
/// program, and radon's firmware leaves this divider at 2, four times the tree's 50 MHz.
///
/// Name: provisional, minted 2026-10-21 (UTC) by milestone/53-radon-multi-block-read.
#[must_use]
pub const fn sdcard_divider_for(parent_hz: u64, want_hz: u64) -> u32 {
    if want_hz == 0 {
        return SDCARD_DIVIDER_MAX;
    }
    let div = parent_hz.div_ceil(want_hz);
    if div < 1 {
        1
    } else if div > SDCARD_DIVIDER_MAX as u64 {
        SDCARD_DIVIDER_MAX
    } else {
        div as u32
    }
}

/// `word` with its divider field ([`clock_divider`]'s bits) replaced by `divider`, every other bit
/// (the enable, the mux) as found.
///
/// Name: provisional, minted 2026-10-21 (UTC) by milestone/53-radon-multi-block-read.
#[must_use]
pub const fn with_divider(word: u32, divider: u32) -> u32 {
    (word & !0x00ff_ffff) | (divider & 0x00ff_ffff)
}

/// **The 100 kHz standard-mode programming for a `DesignWare` I2C controller**, from the timing
/// formula in the vendor U-Boot's `DesignWare` I2C driver (`designware_i2c.c`, fetched 2026-10-10)
/// this board's firmware was built from: counts of the input clock for the standard mode's
/// minimum high (4000 ns) and low (4700 ns) times, the default rise (1000 ns) and fall (300 ns)
/// times, no spike count, then the formula's period fill toward `ic_clk / 100_000`. U-Boot's own
/// init constants give the control word: master mode, restart enable, slave disable, standard speed.
///
/// 100 kHz rather than the fast mode U-Boot defaults to, because the AXP15060's datasheet timing is
/// met by every standard-mode controller and the failure this answers was OpenSBI trusting a
/// controller at reset defaults; slower than the spec floor is safe, faster is not.
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
/// resets: bit 6 (bit 7 powers off), as read on a VisionFive 2 bench on 2026-10-09 and 2026-10-10.
/// The caller writes the reset bit alone, read-modify-write, so the
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
    /// A provider without exactly one cell per specifier, or a short tail. The walk takes
    /// eight-byte strides, so everything from here on in the list is unreadable and skipped: the
    /// safe direction, since a skipped specifier costs a fallback and a misread one would be a
    /// store.
    Unreadable,
}

/// Classify one specifier; see [`Spec`].
fn sys_step<'t, E>(
    spec: &[u8],
    is_clock: bool,
    provider: &mut impl FnMut(u32, &[u8]) -> Result<Option<&'t [u8]>, E>,
) -> Result<Spec, E> {
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
    if provider(phandle, cells)? != Some(&[0, 0, 0, 1][..]) {
        return Ok(Spec::Unreadable);
    }
    let Some(compatible) = provider(phandle, b"compatible")? else {
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
    /// this and [`Report::resets_released`] cover a plan with more than one (`gmac0`'s
    /// has two).
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

/// Where a domain's registers are, as a caller concluded from a tree or from [`Window::fallback`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Found {
    /// Physical base of the register window.
    pub base: u64,
    /// Its size in bytes.
    pub size: u64,
    /// **True when the device tree said so, false when this is the window's constant.** The one
    /// field a bench transcript must carry: a base that came from a constant is a base nobody on
    /// that machine confirmed, and a reader three months later cannot re-derive which it was.
    pub from_tree: bool,
    /// Which `compatible` string matched, or `None` when nothing did and the constant was used.
    pub compatible: Option<&'static [u8]>,
}

/// Mainline's dedicated STG clock-and-reset controller node \[mainline-dts\].
///
/// Name: provisional, made public 2026-10-25 (UTC) by lane crates-stand-alone when the device-tree
/// walk left the crate. The binding's own word for the property it holds, as for every
/// `COMPATIBLE_` constant below.
pub const COMPATIBLE_STGCRG: &[u8] = b"starfive,jh7110-stgcrg";

/// The vendor U-Boot's single clock controller covering sys, stg and aon \[vendor-dts\]. This is
/// the one radon's firmware actually serves, if it serves either.
///
/// Name: provisional, made public 2026-10-25 (UTC) by lane crates-stand-alone.
pub const COMPATIBLE_VENDOR_CLKGEN: &[u8] = b"starfive,jh7110-clkgen";

/// The vendor U-Boot's separate reset controller, whose `reg` list covers the same windows
/// \[vendor-dts\]. Tried last: it names the same address as the clkgen node, so it is only
/// reached by a tree that carries one and not the other.
///
/// Name: provisional, made public 2026-10-25 (UTC) by lane crates-stand-alone.
pub const COMPATIBLE_VENDOR_RSTGEN: &[u8] = b"starfive,jh7110-reset";

/// The `reg-names` entry naming the STG window, in each vendor node's own spelling
/// (`"stg"` in `clkgen`, `"stgcrg"` in `rstgen`; \[vendor-dts\]).
const VENDOR_CLKGEN_STG_NAME: &[u8] = b"stg";
const VENDOR_RSTGEN_STG_NAME: &[u8] = b"stgcrg";

/// Mainline's dedicated SYS clock-and-reset controller node, `syscrg: clock-controller@13020000`
/// (\[mainline-dts\]).
///
/// Name: provisional, made public 2026-10-25 (UTC) by lane crates-stand-alone.
pub const COMPATIBLE_SYSCRG: &[u8] = b"starfive,jh7110-syscrg";

/// The `reg-names` entry naming the SYS window, in each vendor node's own spelling (`"sys"` in
/// `clkgen`, `"syscrg"` in `rstgen`; \[vendor-dts\]). Window 0 in both today, found by name anyway
/// for the reason [`Window`] gives.
const VENDOR_CLKGEN_SYS_NAME: &[u8] = b"sys";
const VENDOR_RSTGEN_SYS_NAME: &[u8] = b"syscrg";

/// One way a device tree names a domain's window: a node's `compatible`, and the `reg-names`
/// entry that selects the window when the node carries several.
///
/// Name: provisional, minted 2026-10-25 (UTC) by lane crates-stand-alone, after the word the
/// crate's docs already used for one tree's way of naming a controller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Spelling {
    /// The node's `compatible` string.
    pub compatible: &'static [u8],
    /// The `reg-names` entry naming the window, or `None` for a node whose single `reg` is it.
    pub window: Option<&'static [u8]>,
}

/// How many `reg` entries a matched node may carry. Five, because the vendor `rstgen` node names
/// five windows and a short buffer would silently truncate the list `reg-names` is indexing into.
///
/// Name: provisional, minted 2026-10-25 (UTC) by lane crates-stand-alone, after `MAX_DEVICE_STEPS`.
pub const MAX_WINDOWS: usize = 5;

/// **Where one domain's registers are, in every spelling the two trees use.**
///
/// A caller tries [`Window::spellings`] in order, mainline's first, and asks
/// [`Found::from_regions`] about the first node that matches; a tree that names none of them gets
/// [`Window::fallback`], the constant both published trees agree on, with `from_tree` false. The
/// two vendor nodes carry several windows and are indexed by `reg-names`, not position. An answer
/// is not evidence that the machine is a JH7110.
///
/// Name: provisional, minted 2026-10-25 (UTC) by lane crates-stand-alone, after the register window
/// it locates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    /// The spellings to try, in order.
    pub spellings: [Spelling; 3],
    /// The window's base in both published trees.
    pub base: u64,
    /// The window's size in both published trees.
    pub size: u64,
}

impl Window {
    /// The answer for a tree that names none of [`Window::spellings`].
    ///
    /// Name: provisional, minted 2026-10-25 (UTC) by lane crates-stand-alone, after the word the
    /// crate's docs already used for the constant answer.
    #[must_use]
    pub const fn fallback(&self) -> Found {
        Found {
            base: self.base,
            size: self.size,
            from_tree: false,
            compatible: None,
        }
    }
}

/// **The STG window**, where the TRNG's clocks and reset live.
///
/// Name: provisional, minted 2026-10-25 (UTC) by lane crates-stand-alone, beside the domain
/// constant `STG`.
pub const STG_WINDOW: Window = Window {
    spellings: [
        Spelling {
            compatible: COMPATIBLE_STGCRG,
            window: None,
        },
        Spelling {
            compatible: COMPATIBLE_VENDOR_CLKGEN,
            window: Some(VENDOR_CLKGEN_STG_NAME),
        },
        Spelling {
            compatible: COMPATIBLE_VENDOR_RSTGEN,
            window: Some(VENDOR_RSTGEN_STG_NAME),
        },
    ],
    base: STG_BASE,
    size: STG_SIZE,
};

/// **The AON window**, where `gmac0`'s bus clocks, transmit clock and resets live.
///
/// Name: provisional, minted 2026-10-25 (UTC) by lane crates-stand-alone, beside the domain
/// constant `AON`.
pub const AON_WINDOW: Window = Window {
    spellings: [
        Spelling {
            compatible: COMPATIBLE_AONCRG,
            window: None,
        },
        Spelling {
            compatible: COMPATIBLE_VENDOR_CLKGEN,
            window: Some(VENDOR_CLKGEN_AON_NAME),
        },
        Spelling {
            compatible: COMPATIBLE_VENDOR_RSTGEN,
            window: Some(VENDOR_RSTGEN_AON_NAME),
        },
    ],
    base: AON_BASE,
    size: AON_SIZE,
};

/// **The SYS window**, where I2C5's clock and reset live.
///
/// Name: provisional, minted 2026-10-25 (UTC) by lane crates-stand-alone, beside the domain
/// constant `SYS`.
pub const SYS_WINDOW: Window = Window {
    spellings: [
        Spelling {
            compatible: COMPATIBLE_SYSCRG,
            window: None,
        },
        Spelling {
            compatible: COMPATIBLE_VENDOR_CLKGEN,
            window: Some(VENDOR_CLKGEN_SYS_NAME),
        },
        Spelling {
            compatible: COMPATIBLE_VENDOR_RSTGEN,
            window: Some(VENDOR_RSTGEN_SYS_NAME),
        },
    ],
    base: SYS_BASE,
    size: SYS_SIZE,
};

impl Found {
    /// **One spelling's answer**, from the matched node's `reg` entries as `(base, size)` pairs
    /// and, for a spelling with a [`Spelling::window`], its `reg-names`. `None` when the node
    /// has no `reg`, or when a multi-window node's `reg-names` is absent, lacks the entry, or
    /// names one past the end of `reg`: picking a window by position because one tree happened
    /// to put it there is a fact nobody wrote down, and [`Window::fallback`] is the honest answer
    /// instead.
    ///
    /// Name: provisional, minted 2026-10-25 (UTC) by lane crates-stand-alone: a window picked out
    /// of a matched node's `reg` entries.
    #[must_use]
    pub fn from_regions(
        spelling: &Spelling,
        regions: &[(u64, u64)],
        reg_names: Option<&[u8]>,
    ) -> Option<Found> {
        if regions.is_empty() {
            return None;
        }
        let index = match spelling.window {
            None => 0,
            Some(want) => match reg_names.and_then(|bytes| name_index(bytes, want)) {
                Some(i) if i < regions.len() => i,
                _ => return None,
            },
        };
        let (base, size) = regions[index];
        Some(Found {
            base,
            size,
            from_tree: true,
            compatible: Some(spelling.compatible),
        })
    }
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
mod tests;
