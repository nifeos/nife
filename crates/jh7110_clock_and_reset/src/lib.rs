#![cfg_attr(not(test), no_std)]
//! The `StarFive` JH7110's clock and reset generator, as pure logic: register offsets, bit
//! positions, the bring-up plans, and what each of the chip's two device trees calls the
//! controllers.
//!
//! This never dereferences a pointer, so it is host-testable and Kani-reachable; the volatile
//! stores are the caller's. Every number is transcribed from Linux mainline ([mainline-dts],
//! [mainline-trng], [mainline-ids], [mainline-clock-ids], [mainline-clk], [mainline-clk-header],
//! [mainline-rst], [mainline-rst-common]) and the vendor U-Boot ([vendor-dts], [vendor-ids]), which
//! agree. One word per clock at `base + 4 * idx` with enable bit 31; a set reset-status bit means
//! out of reset ([`is_deasserted`]).
//!
//! # The two device trees
//!
//! The crate reads no device tree. The caller finds a node and hands this crate its property
//! values. Mainline names one controller node per domain (`starfive,jh7110-stgcrg`, `-aoncrg`,
//! `-syscrg`); the vendor U-Boot names one clock node (`starfive,jh7110-clkgen`) and one reset node
//! (`starfive,jh7110-reset`), each carrying several windows told apart by `reg-names`. Each
//! [`RegisterWindow`] lists its spellings in the order to try them, and
//! [`WindowLocation::from_regions`] picks the window out of a matched node's `reg`.
//! [`PowerManagementChipBus::from_nodes`] and [`DevicePlan::from_specifiers`] turn a node's
//! `clocks` and `resets` into steps, asking the caller to look each phandle up.
//!
//! Mainline links are pinned to `torvalds/linux` commit `cbbddb6b08cc`; vendor links to
//! `starfive-tech/u-boot` tag `VF2_v2.10.4`, commit `1ae835c25e3d`, the release a VisionFive 2
//! ships with. All were fetched 2026-11-01 (UTC), and every claim that names a revision was checked
//! against these commits on 2026-11-06. The vendor branch `JH7110_VisionFive2_devel` was also read
//! at commit `c4c67bb66ae6`, and where the tag and the branch differ the item says which holds it.
//!
//! [mainline-dts]: https://github.com/torvalds/linux/blob/cbbddb6b08cc9900a87cccaa9b22bbe06aa4f50a/arch/riscv/boot/dts/starfive/jh7110.dtsi
//! [mainline-trng]: https://github.com/torvalds/linux/blob/cbbddb6b08cc9900a87cccaa9b22bbe06aa4f50a/drivers/char/hw_random/jh7110-trng.c
//! [mainline-ids]: https://github.com/torvalds/linux/blob/cbbddb6b08cc9900a87cccaa9b22bbe06aa4f50a/include/dt-bindings/reset/starfive%2Cjh7110-crg.h
//! [mainline-clock-ids]: https://github.com/torvalds/linux/blob/cbbddb6b08cc9900a87cccaa9b22bbe06aa4f50a/include/dt-bindings/clock/starfive%2Cjh7110-crg.h
//! [mainline-clk]: https://github.com/torvalds/linux/blob/cbbddb6b08cc9900a87cccaa9b22bbe06aa4f50a/drivers/clk/starfive/clk-starfive-jh71x0.c
//! [mainline-clk-header]: https://github.com/torvalds/linux/blob/cbbddb6b08cc9900a87cccaa9b22bbe06aa4f50a/drivers/clk/starfive/clk-starfive-jh71x0.h
//! [mainline-rst]: https://github.com/torvalds/linux/blob/cbbddb6b08cc9900a87cccaa9b22bbe06aa4f50a/drivers/reset/starfive/reset-starfive-jh7110.c
//! [mainline-rst-common]: https://github.com/torvalds/linux/blob/cbbddb6b08cc9900a87cccaa9b22bbe06aa4f50a/drivers/reset/starfive/reset-starfive-jh71x0.c
//! [vendor-dts]: https://github.com/starfive-tech/u-boot/blob/1ae835c25e3d8b7bc3c8981fb720f030c1892d57/arch/riscv/dts/jh7110.dtsi
//! [vendor-ids]: https://github.com/starfive-tech/u-boot/blob/1ae835c25e3d8b7bc3c8981fb720f030c1892d57/include/dt-bindings/clock/starfive-jh7110-clkgen.h
//!
//! Name: ratified 2026-09-13, replacing `jh7110_crg`; kept as the crates.io name (#1806).

// Runs the README's usage example as a doctest, so a stranger's first copy of it compiles.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;

/// The bit that turns a clock on, in every one of this controller's per-clock words
/// (\[mainline-clk-header\], `#define JH71X0_CLK_ENABLE BIT(31)`).
///
/// Name: ratified 2026-10-26 (UTC).
pub const CLOCK_ENABLE: u32 = 1 << 31;

/// A single step of a device's bring-up sequence, in the order it must be taken.
///
/// Deliberately a plan rather than a pair of methods on a driver: the order is the load-bearing
/// part (\[mainline-trng\] enables both clocks *before* deasserting, and
/// \[mainline-rst-common\] warns that a deassert against a gated clock can hang), and a plan is
/// the only shape that lets a host test assert on the order without a device. A driver walks this slice; it does not know which device
/// it is bringing up.
///
/// Name: ratified 2026-10-26 (UTC).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Step {
    /// Set [`CLOCK_ENABLE`] in the domain's word for this clock index.
    EnableClock(u32),
    /// Clear this reset's bit in the domain's assert word, then wait for its status bit to read
    /// as [`is_deasserted`].
    DeassertReset(u32),
    /// **Choose a multiplexed clock's parent**: replace the [`CLOCK_PARENT_MASK`] field of this
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

/// **The clock multiplexer mask**: a multiplexed clock's parent field, bits 27:24 of its word,
/// which holds the parent's position in that clock's parent list. OpenBSD spells it `CLKMUX_MASK`
/// (\[openbsd-stfclock\]), FreeBSD `JH7110_MUX_MASK` (\[freebsd-jh7110-clk\]) and Linux
/// `JH71X0_CLK_MUX_MASK`, `GENMASK(27, 24)` (\[mainline-clk-header\]). The three disagree on its
/// width: OpenBSD masks bits 25:24 and FreeBSD bits 29:24. This crate takes Linux's four bits, as it
/// takes every other number from mainline.
///
/// [openbsd-stfclock]: https://github.com/openbsd/src/blob/bc5d5205881e579d8da936d6039dee52c2284971/sys/arch/riscv64/dev/stfclock.c
/// [freebsd-jh7110-clk]: https://github.com/freebsd/freebsd-src/blob/7806832f23fb31d28386d42692b221776c9f4ee4/sys/dev/clk/starfive/jh7110_clk.c
///
/// Name: ratified 2026-11-05 (UTC), the maintainer's ruling, provisional until an architect
/// reviews it. Named for what the field holds, the parent, as [`with_parent`] and
/// [`Step::SelectParent`] already say. Refused `CLOCK_MUX_MASK` (`mux` abbreviates multiplexer),
/// `CLOCK_MULTIPLEXER_MASK` (it names the mechanism, not what the field selects) and
/// `CLOCK_PARENT_SELECT_MASK` (`select` adds a word and no meaning).
#[doc(
    alias = "CLKMUX_MASK",
    alias = "JH7110_MUX_MASK",
    alias = "JH71X0_CLK_MUX_MASK"
)]
pub const CLOCK_PARENT_MASK: u32 = 0xf << CLOCK_MUX_SHIFT;

/// Where [`CLOCK_PARENT_MASK`]'s field starts (`JH71X0_CLK_MUX_SHIFT`).
pub(crate) const CLOCK_MUX_SHIFT: u32 = 24;

/// **A clock word with its parent replaced**, leaving the enable bit, the divider and every other
/// field as they were. A parent too wide for the field is reduced to it rather than spilling into
/// the enable bit.
///
/// Name: ratified 2026-10-26 (UTC).
#[must_use]
pub const fn with_parent(word: u32, parent: u32) -> u32 {
    (word & !CLOCK_PARENT_MASK) | ((parent << CLOCK_MUX_SHIFT) & CLOCK_PARENT_MASK)
}

/// **What the JH7110's TRNG needs before its registers answer**, transcribed from
/// \[mainline-trng\]'s probe and cross-checked against \[vendor-dts\]: clocks 15 and 16, then
/// reset 3, all of the STG domain, so the plan is meaningless without [`SYSTEM_TOP_GROUP`].
///
/// # Examples
///
/// ```
/// use jh7110_clock_and_reset::{
///     is_deasserted, Step, CLOCK_ENABLE, ENTROPY_BRING_UP, SYSTEM_TOP_GROUP,
/// };
///
/// // The TRNG's plan is two clocks then one reset, in that order, which is the order
/// // Linux's own probe takes: a reset deassert against a gated clock can hang forever.
/// assert_eq!(
///     ENTROPY_BRING_UP,
///     &[Step::EnableClock(15), Step::EnableClock(16), Step::DeassertReset(3)]
/// );
///
/// // Clock 15 lives one word per clock from the domain's base.
/// assert_eq!(SYSTEM_TOP_GROUP.clock_offset(15), Some(0x3c));
/// assert_eq!(CLOCK_ENABLE, 1 << 31);
///
/// // Reset 3 is bit 3 of the word at 0x74, watched at 0x78. A SET status bit means
/// // "out of reset", which is the opposite of what the register name suggests.
/// let r = SYSTEM_TOP_GROUP.reset_bit(3).unwrap();
/// assert_eq!((r.assert_offset, r.status_offset, r.mask), (0x74, 0x78, 1 << 3));
/// assert!(is_deasserted(0b1000, r.mask));
/// assert!(!is_deasserted(0b0111, r.mask));
/// ```
///
/// Name: ratified 2026-10-26 (UTC), on #2760. Refused `TRNG_BRING_UP` (it names the block for its
/// abbreviation; the sequence brings up the entropy source).
pub const ENTROPY_BRING_UP: &[Step] = &[
    Step::EnableClock(STGCLK_SECURITY_AHB),
    Step::EnableClock(STGCLK_SECURITY_MISC_AHB),
    Step::DeassertReset(SYSTEM_TOP_GROUP_RESET_SECURITY_ADVANCED_HIGH_PERFORMANCE_BUS),
];

/// `JH7110_STGCLK_SEC_AHB` \[mainline-ids\]; the vendor tree's `JH7110_SEC_HCLK` (205) rebased on
/// its stg group's start (190). The TRNG's `hclk`.
#[doc(alias = "JH7110_STGCLK_SEC_AHB")]
pub(crate) const STGCLK_SECURITY_AHB: u32 = 15;

/// `JH7110_STGCLK_SEC_MISC_AHB` \[mainline-ids\]; the vendor tree's `JH7110_SEC_MISCAHB_CLK` (206)
/// rebased the same way. The TRNG's second clock, `ahb` to mainline and `miscahb_clk` to the
/// vendor, which is the same wire under two names.
#[doc(alias = "JH7110_STGCLK_SEC_MISC_AHB")]
pub(crate) const STGCLK_SECURITY_MISC_AHB: u32 = 16;

/// `JH7110_STGRST_SEC_AHB` \[mainline-ids\]; the vendor tree's `RSTN_U0_SEC_TOP_HRESETN` (131)
/// rebased on its stg group's start (128).
///
/// **It is a *shared* reset** (\[mainline-trng\] takes it with `devm_reset_control_get_shared`),
/// which is a fact about the silicon rather than about Linux's API: the same line resets the whole
/// security top block, the PL080 DMA at `0x1600_8000` included. Deasserting it is safe; asserting
/// it would reset a neighbour, which is why nothing in this crate offers an assert.
///
/// Name: ratified 2026-11-05 (UTC), #3279.
#[doc(alias = "JH7110_STGRST_SEC_AHB")]
pub const SYSTEM_TOP_GROUP_RESET_SECURITY_ADVANCED_HIGH_PERFORMANCE_BUS: u32 = 3;

/// **The AON (always-on) domain**, where `gmac0`'s bus clocks, its transmit clock mux and both
/// of its resets live.
///
/// Offsets from \[mainline-rst\]'s `jh7110_aon_info` (`.assert_offset = 0x38, .status_offset =
/// 0x3C`, read 2026-10-06); the counts from \[mainline-ids\]'s `JH7110_AONRST_END` (8) and
/// `JH7110_AONCLK_END` (14). The chip's own abbreviation is AON, kept in the copied `AONCLK_*`
/// and `AONRST_*` names.
///
/// Name: ratified 2026-10-26 (UTC), on #2760. Refused `AON` (the chip's abbreviation, kept as an
/// alias).
#[doc(alias = "aon")]
pub const ALWAYS_ON: Domain = Domain {
    reset_assert: 0x38,
    reset_status: 0x3c,
    resets: 8,
    clocks: 14,
};

/// **The AON domain's register window**, `0x1700_0000`, size `0x1_0000`: mainline's `aoncrg`
/// node and `reg-names` entry `"aon"`/`"aoncrg"` of both vendor nodes. The fallback when a tree
/// names none of them, for [`SYSTEM_TOP_GROUP_BASE_PHYSICAL_ADDRESS`]'s reason.
pub(crate) const ALWAYS_ON_BASE_PHYSICAL_ADDRESS: u64 = 0x1700_0000;

/// The AON window's size, `0x10000` in every tree that names it.
pub(crate) const ALWAYS_ON_SIZE_BYTES: u64 = 0x1_0000;

/// **AON clock and reset generator** (CRG, the device tree's `aoncrg` node, \[mainline-dts\]):
/// mainline's dedicated AON clock-and-reset controller node, `aoncrg: clock-controller@17000000`.
///
/// The vendor string is `starfive,jh7110-aoncrg`.
///
/// Name: ratified 2026-11-02 (UTC), #3090. The generator is spelled out, as the crate name
/// spells it.
#[doc(alias = "clock and reset generator")]
pub const COMPATIBLE_ALWAYS_ON_CLOCK_AND_RESET_GENERATOR: &[u8] = b"starfive,jh7110-aoncrg";

/// The `reg-names` entry naming the AON window in each vendor node's spelling (`"aon"` in
/// `clkgen`, the clock generator node, and `"aoncrg"` in `rstgen`, the reset generator node;
/// \[vendor-dts\]).
const VENDOR_CLOCK_GENERATOR_ALWAYS_ON_NAME: &[u8] = b"aon";
const VENDOR_RESET_GENERATOR_ALWAYS_ON_NAME: &[u8] = b"aoncrg";

/// `JH7110_AONCLK_GMAC0_AHB` \[mainline-ids\]: a gate, `gmac0`'s `pclk`. Word `0x08`.
pub(crate) const AONCLK_GMAC0_AHB: u32 = 2;
/// `JH7110_AONCLK_GMAC0_AXI`: a gate, `gmac0`'s `stmmaceth` (its CSR and bus clock). Word `0x0c`.
pub(crate) const AONCLK_GMAC0_AXI: u32 = 3;
/// **`gmac0` transmit clock** (`AONCLK_GMAC0_TX`, `JH7110_AONCLK_GMAC0_TX` in \[mainline-ids\]): a gate with a two-way mux, `gmac0_gtxclk` (0) or
/// `gmac0_rmii_rtx` (1). Word `0x14`, which the vendor U-Boot calls `GMAC5_0_CLK_TX_SHIFT`
/// and sets bit 24 of (`jh7110_gmac_sel_tx_to_rgmii` in \[vendor-board\]).
///
/// [vendor-board]: https://github.com/starfive-tech/u-boot/blob/1ae835c25e3d8b7bc3c8981fb720f030c1892d57/board/starfive/visionfive2/starfive_visionfive2.c
#[doc(alias = "JH7110_AONCLK_GMAC0_TX")]
#[doc(alias = "gmac0 transmit clock")]
pub(crate) const AONCLK_GMAC0_TX: u32 = 5;
/// `gmac0_tx`'s parent on a VisionFive 2 v1.3B: `gmac0_rmii_rtx`, mainline's
/// `assigned-clock-parents` for `&gmac0` in `jh7110-starfive-visionfive-2-v1.3b.dts` beside
/// `starfive,tx-use-rgmii-clk`, and the same bit the vendor U-Boot sets. A board whose EEPROM
/// reports PCB revision `0xb2` is a v1.3B.
pub(crate) const AONCLK_GMAC0_TRANSMIT_PARENT_RMII_RTX: u32 = 1;
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
/// [`GMAC0_ALWAYS_ON_BRING_UP`], whose resets must be released with every clock already running.
///
/// Name: ratified 2026-10-26 (UTC), on #2760. Refused `GMAC0_SYS_BRING_UP` (the domain spelled
/// out).
pub const GMAC0_SYSTEM_BRING_UP: &[Step] = &[
    Step::EnableClock(SYSCLK_GMAC0_GTXCLK),
    Step::EnableClock(SYSCLK_GMAC0_GTXC),
    Step::EnableClock(SYSCLK_GMAC0_PTP),
];

/// **`gmac0`'s AON-domain clocks, its transmit clock's parent, and both its resets**, in Linux's
/// probe order: mainline's `dwmac-starfive.c` and the stmmac platform code enable every clock
/// before deasserting either reset. OpenBSD's `dwqe_fdt_attach` differs: it enables `stmmaceth` and
/// `pclk`, deasserts both resets, then enables `tx` and `gtx`. The transmit clock's inverted twin
/// (`gmac0_tx_inv`, index 6) and both receive clocks (7, 8) have no gate to enable (they are
/// \[mainline-clk-header\] `JH71X0__INV` and `JH71X0__MUX` clocks), so they are not steps.
///
/// Name: ratified 2026-10-26 (UTC), on #2760. Refused `GMAC0_AON_BRING_UP` (the domain spelled
/// out).
pub const GMAC0_ALWAYS_ON_BRING_UP: &[Step] = &[
    Step::EnableClock(AONCLK_GMAC0_AXI),
    Step::EnableClock(AONCLK_GMAC0_AHB),
    Step::SelectParent {
        clock: AONCLK_GMAC0_TX,
        parent: AONCLK_GMAC0_TRANSMIT_PARENT_RMII_RTX,
    },
    Step::EnableClock(AONCLK_GMAC0_TX),
    Step::DeassertReset(AONRST_GMAC0_AXI),
    Step::DeassertReset(AONRST_GMAC0_AHB),
];

/// Where one reset lives: which word to write, which word to watch, and which bit in both.
///
/// Name: ratified 2026-10-26 (UTC).
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
/// "voutcrg"`); [`SYSTEM_TOP_GROUP`], [`ALWAYS_ON`] and [`SYSTEM`] are described here, each only
/// as far as the devices this crate plans for need it. The arithmetic is general, so another domain is a
/// table entry rather than a rewrite.
///
/// [vendor-dts]: https://github.com/starfive-tech/u-boot/blob/1ae835c25e3d8b7bc3c8981fb720f030c1892d57/arch/riscv/dts/jh7110.dtsi
///
/// Name: ratified 2026-10-26 (UTC).
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

/// **The STG (System-Top-Group) domain**, which is where the TRNG's clocks and reset live. The
/// expansion is the one the Linux STGCRG patch series gives; the copied `STGCLK_*` and `STGRST_*`
/// names keep the chip's abbreviation.
///
/// Offsets from \[mainline-rst\]'s `jh7110_stg_info`; the counts from \[mainline-ids\]'s
/// `JH7110_STGRST_END` (23) and `JH7110_STGCLK_END` (29).
///
/// Name: ratified 2026-10-26 (UTC), on #2760. Refused `STG` (the chip's abbreviation for
/// System-Top-Group, kept as an alias).
#[doc(alias = "stg")]
pub const SYSTEM_TOP_GROUP: Domain = Domain {
    reset_assert: 0x74,
    reset_status: 0x78,
    resets: 23,
    clocks: 29,
};

/// **The STG domain's register window, as both published trees give it.**
///
/// The fallback [`SYSTEM_TOP_GROUP_WINDOW`] answers with when the tree names no controller, and
/// [`WindowLocation::from_tree`] says which happened. A firmware tree may omit or misdescribe the
/// controller, and two independently published trees agree on this number. A caller must not
/// store through a fallback on a machine that is not a JH7110: QEMU's riscv64 `virt` has nothing
/// mapped there.
pub(crate) const SYSTEM_TOP_GROUP_BASE_PHYSICAL_ADDRESS: u64 = 0x1023_0000;

/// The STG window's size, `0x10000` in both trees.
pub(crate) const SYSTEM_TOP_GROUP_SIZE_BYTES: u64 = 0x1_0000;

/// **The SYS domain**, which is where I2C5's clock and reset live, and so where the only road to
/// a VisionFive 2's PMIC starts.
///
/// Offsets from \[mainline-rst\]'s `jh7110_sys_info` (`.assert_offset = 0x2F8, .status_offset =
/// 0x308`, fetched 2026-09-25); the counts from \[mainline-ids\]'s `JH7110_SYSRST_END` (126) and
/// `JH7110_SYSCLK_END` (190). The vendor header of the U-Boot a VisionFive 2 ships with agrees on the clock
/// count, as `JH7110_CLK_SYS_REG_END 190`, which is the boundary between clocks that have a
/// register and the vendor's virtual ones numbered above it. The chip's own abbreviation is SYS,
/// kept in the copied `SYSCLK_*` and `SYSRST_*` names.
///
/// Name: ratified 2026-10-26 (UTC), on #2760. Refused `SYS` (the chip's abbreviation, kept as an
/// alias).
#[doc(alias = "sys")]
pub const SYSTEM: Domain = Domain {
    reset_assert: 0x2f8,
    reset_status: 0x308,
    resets: 126,
    clocks: 190,
};

/// **The SYS domain's register window**, `0x1302_0000`, size `0x1_0000`, in mainline's `syscrg`
/// node and as `reg-names` entry `"sys"`/`"syscrg"` of both vendor nodes. The fallback when a tree
/// names none of them, for `SYSTEM_TOP_GROUP_BASE_PHYSICAL_ADDRESS`'s reason.
///
/// Name: ratified 2026-11-02 (UTC), #3090; renamed 2026-11-08 (UTC) to say the address is physical.
pub const SYSTEM_BASE_PHYSICAL_ADDRESS: u64 = 0x1302_0000;

/// The SYS window's size, `0x10000` in every tree that names it.
pub(crate) const SYSTEM_SIZE_BYTES: u64 = 0x1_0000;

/// `JH7110_SYSCLK_I2C5_APB` \[mainline-ids\], and `JH7110_I2C5_CLK_APB` in the vendor header,
/// both **143**: the vendor numbers the SYS group first, so no rebase is needed. Word `0x23c`.
///
/// **This is I2C5's only gate.** The vendor U-Boot also names a `u5_dw_i2c_clk_core` (vendor id
/// 298) and prints it at `Starting kernel`. At `VF2_v2.10.4` the clock driver
/// (\[vendor-clk-driver\]) registers it as a gate on `osc` at `SYS_OFFSET(298)`, byte `0x4a8`,
/// past the last SYS word (id 190, `JH7110_CLK_SYS_REG_END`), so no register backs it. At the
/// vendor branch head (\[vendor-clk-driver-head\]) that driver registers no clock by this name.
/// Mainline's `DesignWare` node names this clock alone. So there is no second gate to turn on, and
/// 298 is not a word to write.
///
/// [vendor-clk-driver]: https://github.com/starfive-tech/u-boot/blob/1ae835c25e3d8b7bc3c8981fb720f030c1892d57/drivers/clk/starfive/clk-jh7110.c
/// [vendor-clk-driver-head]: https://github.com/starfive-tech/u-boot/blob/c4c67bb66ae6f41c98537d18cf5c3abc8b97b8e4/drivers/clk/starfive/clk-jh7110.c
pub(crate) const SYSCLK_I2C5_APB: u32 = 143;

/// `JH7110_SYSRST_I2C5_APB` \[mainline-ids\], and `RSTN_U5_DW_I2C_APB` in the vendor header, both
/// **81**. Bit 17 of the word at `0x300`, watched at `0x310`.
///
/// **The reset a clock-only bring-up misses.** The vendor U-Boot removes its I2C driver before
/// handing over (`DM_FLAG_OS_PREPARE`), and `designware_i2c_remove` ends in `reset_release_bulk`
/// (\[vendor-i2c\]), whose `reset_release_all` *asserts* every reset before freeing it. The
/// OpenSBI of a VisionFive 2 re-enables a clock before its I2C transfer and never touches a reset.
/// A controller held in reset reads `IC_STATUS` as zero, so its transmit-FIFO-empty poll can never
/// succeed. That stall is the likely reason a PMIC transfer fails after the hand-over; a board run
/// of the plan below is what would confirm it.
///
/// [vendor-i2c]: https://github.com/starfive-tech/u-boot/blob/1ae835c25e3d8b7bc3c8981fb720f030c1892d57/drivers/i2c/designware_i2c.c
pub(crate) const SYSRST_I2C5_APB: u32 = 81;

/// **What I2C5 needs before OpenSBI can reach the PMIC**, when the tree does not say. Clock first,
/// then reset, for [`ENTROPY_BRING_UP`]'s reason: Linux's reset driver warns that a deassert against a
/// gated clock can hang.
///
/// Name: provisional.
pub const POWER_MANAGEMENT_CHIP_BUS_BRING_UP: &[Step] = &[
    Step::EnableClock(SYSCLK_I2C5_APB),
    Step::DeassertReset(SYSRST_I2C5_APB),
];

/// **eMMC controller bus clock** (`SYSCLK_SDIO0_AHB`, `JH7110_SYSCLK_SDIO0_AHB` in \[mainline-ids\]), the vendor header's `JH7110_SDIO0_CLK_AHB`, both
/// **91**: the eMMC socket's controller's bus clock, `biu` in both trees. Word `0x16c`.
#[doc(alias = "JH7110_SYSCLK_SDIO0_AHB")]
#[doc(alias = "emmc controller bus clock")]
pub(crate) const SYSCLK_SDIO0_AHB: u32 = 91;
/// **microSD controller bus clock** (`SYSCLK_SDIO1_AHB`, `JH7110_SYSCLK_SDIO1_AHB`; vendor `JH7110_SDIO1_CLK_AHB`), both **92**: the microSD slot's
/// controller's bus clock. Word `0x170`.
#[doc(alias = "JH7110_SYSCLK_SDIO1_AHB")]
#[doc(alias = "microsd controller bus clock")]
pub(crate) const SYSCLK_SDIO1_AHB: u32 = 92;
/// **eMMC controller card clock** (`SYSCLK_SDIO0_SDCARD`, `JH7110_SYSCLK_SDIO0_SDCARD`; vendor `JH7110_SDIO0_CLK_SDCARD`), both **93**: the eMMC
/// controller's card clock source, `ciu`, a gated divider that mainline (\[mainline-common-dts\]) and
/// the vendor branch head (\[vendor-board-dts-head\]) assign 50 MHz; the board file at
/// `VF2_v2.10.4` (\[vendor-board-dts\]) assigns none. Word `0x174`.
///
/// [mainline-common-dts]: https://github.com/torvalds/linux/blob/cbbddb6b08cc9900a87cccaa9b22bbe06aa4f50a/arch/riscv/boot/dts/starfive/jh7110-common.dtsi
/// [vendor-board-dts]: https://github.com/starfive-tech/u-boot/blob/1ae835c25e3d8b7bc3c8981fb720f030c1892d57/arch/riscv/dts/starfive_visionfive2.dts
/// [vendor-board-dts-head]: https://github.com/starfive-tech/u-boot/blob/c4c67bb66ae6f41c98537d18cf5c3abc8b97b8e4/arch/riscv/dts/starfive_visionfive2.dts
#[doc(alias = "JH7110_SYSCLK_SDIO0_SDCARD")]
#[doc(alias = "emmc controller card clock")]
pub(crate) const SYSCLK_SDIO0_SDCARD: u32 = 93;
/// **microSD controller card clock** (`SYSCLK_SDIO1_SDCARD`, `JH7110_SYSCLK_SDIO1_SDCARD`; vendor `JH7110_SDIO1_CLK_SDCARD`), both **94**: the microSD
/// controller's `ciu`. Word `0x178`.
#[doc(alias = "JH7110_SYSCLK_SDIO1_SDCARD")]
#[doc(alias = "microsd controller card clock")]
pub(crate) const SYSCLK_SDIO1_SDCARD: u32 = 94;
/// **eMMC controller reset** (`SYSRST_SDIO0_AHB`, `JH7110_SYSRST_SDIO0_AHB` in \[mainline-ids\]): the eMMC controller's one reset. Bit 0 of the
/// word at `0x300`.
#[doc(alias = "JH7110_SYSRST_SDIO0_AHB")]
#[doc(alias = "emmc controller reset")]
pub(crate) const SYSRST_SDIO0_AHB: u32 = 64;
/// **microSD controller reset** (`SYSRST_SDIO1_AHB`, `JH7110_SYSRST_SDIO1_AHB`): the microSD controller's one reset. Bit 1 of the word at `0x300`.
#[doc(alias = "JH7110_SYSRST_SDIO1_AHB")]
#[doc(alias = "microsd controller reset")]
pub(crate) const SYSRST_SDIO1_AHB: u32 = 65;

/// **What each SD/MMC controller needs before its registers answer**, slot 0 (the eMMC socket) then
/// slot 1 (the microSD slot): both clocks mainline's node names, then its reset, in
/// [`ENTROPY_BRING_UP`]'s order and for its reason. OpenBSD's `dwmmc_attach` takes the same order
/// (`clock_enable_all`, then `reset_deassert_all`). Every step is idempotent, which matters here:
/// the vendor U-Boot has already brought the microSD controller up to load the boot script, and a
/// caller can read whether it left it that way before it walks the plan.
///
/// Name: ratified 2026-10-26 (UTC); renamed 2026-11-08 (UTC) to spell out the controller's name.
pub const SD_INPUT_OUTPUT_BRING_UP: [&[Step]; 2] = [
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

/// The divider field of a JH7110 clock word, bits 23:0 (\[mainline-clk-header\]'s
/// `JH71X0_CLK_DIV_MASK`).
/// A caller can print it beside the enable bit, so the card clock's source rate is read off the
/// silicon rather than assumed from `assigned-clock-rates`.
///
/// Name: ratified 2026-10-26 (UTC).
#[must_use]
pub const fn clock_divider(word: u32) -> u32 {
    word & 0x00ff_ffff
}

/// The PMIC (power management integrated circuit) as the vendor tree spells it: the
/// `axp15060_reg@36` node under `&i2c5` in \[vendor-board-dts\] (`starfive-tech/u-boot` at tag
/// `VF2_v2.10.4`, commit `1ae835c25e3d`; the branch head `c4c67bb66ae6` agrees). Its `compatible`
/// is this string and its `reg` is `0x36`. A VisionFive 2's OpenSBI matches the same string to find
/// its reset device.
///
/// [vendor-board-dts]: https://github.com/starfive-tech/u-boot/blob/1ae835c25e3d8b7bc3c8981fb720f030c1892d57/arch/riscv/dts/starfive_visionfive2.dts
///
/// Name: ratified 2026-11-02 (UTC), #3090.
#[doc(alias = "pmic")]
pub const COMPATIBLE_POWER_MANAGEMENT_CHIP_VENDOR: &[u8] = b"stf,axp15060-regulator";

/// The same PMIC in mainline: the `pmic@36` node under `&i2c5` in \[mainline-common-dts\]
/// (`torvalds/linux` at commit `cbbddb6b08cc`), whose `compatible` is this string.
///
/// [mainline-common-dts]: https://github.com/torvalds/linux/blob/cbbddb6b08cc9900a87cccaa9b22bbe06aa4f50a/arch/riscv/boot/dts/starfive/jh7110-common.dtsi
///
/// Name: ratified 2026-11-02 (UTC), #3090.
#[doc(alias = "pmic")]
pub const COMPATIBLE_POWER_MANAGEMENT_CHIP_MAINLINE: &[u8] = b"x-powers,axp15060";

/// Both PMIC (power management integrated circuit) spellings, in the order to try them: mainline's
/// first.
///
/// Name: ratified 2026-10-26 (UTC), on #2760. Refused `PMIC_COMPATIBLES` (PMIC spelled out; `chip`
/// for integrated circuit, and not `power_management_bus`, which is the `PMBus` standard's name).
#[doc(alias = "pmic")]
pub const POWER_MANAGEMENT_CHIP_COMPATIBLES: [&[u8]; 2] = [
    COMPATIBLE_POWER_MANAGEMENT_CHIP_MAINLINE,
    COMPATIBLE_POWER_MANAGEMENT_CHIP_VENDOR,
];

/// How many steps a tree-derived plan can hold. The vendor tree names two clocks and one reset,
/// one of those clocks virtual; four is slack, and a bus naming more is recorded as truncated.
///
/// Name: ratified 2026-11-02 (UTC), #3090.
pub const MAXIMUM_POWER_MANAGEMENT_CHIP_BUS_STEPS: usize = 4;

/// **The plan for the PMIC's bus, and where it came from.** The PMIC (power management integrated
/// circuit) is the AXP15060 of the VisionFive 2, on I2C5.
///
/// Name: ratified 2026-10-26 (UTC), on #2760. Refused `PmicBus` (PMIC spelled out) and
/// `PowerManagementBus` (it collides with the `PMBus` standard's name).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
#[doc(alias = "pmic")]
pub struct PowerManagementChipBus {
    steps: [Step; MAXIMUM_POWER_MANAGEMENT_CHIP_BUS_STEPS],
    len: usize,
    /// **True when the plan is the tree's own `clocks` and `resets` of the PMIC's parent bus**,
    /// false when it is the constant I2C5 plan (its clock 143, then its reset 81). The field a
    /// caller's log must carry, for [`WindowLocation::from_tree`]'s reason.
    pub from_tree: bool,
    /// Which PMIC `compatible` was found, or `None` when the tree names no known PMIC.
    pub power_management_chip: Option<&'static [u8]>,
    /// Clock or reset specifiers the bus named that are **not** a SYS-domain gate or line, and
    /// were skipped rather than written: the vendor tree's virtual clock 298 is the expected one. A provider
    /// this crate does not recognise, or an id past [`SYSTEM`]'s bounds, lands here, never in a
    /// store.
    pub skipped: usize,
    /// True when the bus named more usable steps than [`MAXIMUM_POWER_MANAGEMENT_CHIP_BUS_STEPS`].
    pub truncated: bool,
    /// **The I2C controller's register window**, the `reg` of the PMIC's parent bus node, when the
    /// tree states it. The direct reset write needs it; a plan without one (the constant fallback,
    /// or a tree that names no `reg`) leaves it `None` and the caller does not attempt the write.
    ///
    /// Name: ratified 2026-11-08 (UTC), renamed to say the pair is a physical address and a size.
    pub controller_physical_window: Option<(u64, u64)>,
    /// **The PMIC's 7-bit I2C address**, the PMIC node's own `reg`: `0x36` in both
    /// trees. `None` when the tree names no known PMIC at all.
    pub power_management_chip_address: Option<u32>,
}

impl PowerManagementChipBus {
    /// The steps to walk, clocks before resets.
    ///
    /// Name: ratified 2026-10-26 (UTC).
    #[must_use]
    pub fn plan(&self) -> &[Step] {
        &self.steps[..self.len]
    }

    fn fallback(power_management_chip: Option<&'static [u8]>, skipped: usize) -> Self {
        let mut bus = PowerManagementChipBus {
            steps: [Step::EnableClock(0); MAXIMUM_POWER_MANAGEMENT_CHIP_BUS_STEPS],
            len: POWER_MANAGEMENT_CHIP_BUS_BRING_UP.len(),
            from_tree: false,
            power_management_chip,
            skipped,
            truncated: false,
            controller_physical_window: None,
            power_management_chip_address: None,
        };
        bus.steps[..POWER_MANAGEMENT_CHIP_BUS_BRING_UP.len()]
            .copy_from_slice(POWER_MANAGEMENT_CHIP_BUS_BRING_UP);
        bus
    }
}

/// One `reg` property's first address and size, from the cells either tree spelling uses: four
/// cells (`<hi lo size-hi size-lo>`, the root's `#address-cells 2` and `#size-cells 2`) or two
/// (`<addr size>`). Big-endian, as every device tree cell is. `None` for a shorter or partial
/// property, which the caller treats as "not stated".
fn parse_register_property(cells: &[u8]) -> Option<(u64, u64)> {
    let big_endian_word = |at: usize| -> u64 {
        u64::from(u32::from_be_bytes([
            cells[at],
            cells[at + 1],
            cells[at + 2],
            cells[at + 3],
        ]))
    };
    if cells.len() >= 16 {
        Some((
            (big_endian_word(0) << 32) | big_endian_word(4),
            (big_endian_word(8) << 32) | big_endian_word(12),
        ))
    } else if cells.len() >= 8 {
        Some((big_endian_word(0), big_endian_word(4)))
    } else {
        None
    }
}

impl PowerManagementChipBus {
    /// **The plan when the tree names no known PMIC**: the constant I2C5 plan, with `from_tree`
    /// false and `power_management_chip` `None`.
    ///
    /// Name: ratified 2026-11-02 (UTC), #3090.
    #[must_use]
    pub fn without_power_management_chip() -> Self {
        PowerManagementChipBus::fallback(None, 0)
    }

    /// **The plan for the PMIC `power_management_chip`, from what its node and its parent bus node
    /// say.**
    ///
    /// `power_management_chip` is the [`POWER_MANAGEMENT_CHIP_COMPATIBLES`] entry the caller
    /// matched. `power_management_chip_register_property` is the PMIC node's own `reg` (its I2C
    /// address, one cell), and `bus_register_property`, `clocks` and `resets` are its parent bus
    /// node's properties; each is `None` when the tree does not state it. A `<phandle id>` pair becomes a step only for a
    /// known one-cell SYS provider and an id inside [`SYSTEM`]'s bounds (the vendor numbers SYS
    /// first, so one bound serves both trees); the rest are counted in
    /// [`PowerManagementChipBus::skipped`]. `provider(phandle, name)` returns the property `name`
    /// of the node carrying `phandle`, as the caller's device-tree reader finds it. With no usable
    /// step the answer is the constant plan with `from_tree` false. An answer is not evidence of a
    /// JH7110.
    ///
    /// # Errors
    ///
    /// Propagates the first error `provider` returns.
    ///
    /// Name: ratified 2026-10-26 (UTC). Refused `from_bus` (`PmicBus::from_bus` repeated its type;
    /// it reads the PMIC's node and its bus's node).
    pub fn from_nodes<'t, E>(
        power_management_chip: &'static [u8],
        power_management_chip_register_property: Option<&[u8]>,
        bus_register_property: Option<&[u8]>,
        clocks: Option<&[u8]>,
        resets: Option<&[u8]>,
        mut provider: impl FnMut(u32, &[u8]) -> Result<Option<&'t [u8]>, E>,
    ) -> Result<Self, E> {
        let mut bus = PowerManagementChipBus::fallback(Some(power_management_chip), 0);
        // The PMIC's own `reg` is an I2C child address: one cell, size zero by `#size-cells 0`.
        // Both trees spell it `<0x36>`.
        if let Some(cells) = power_management_chip_register_property
            && cells.len() >= 4
        {
            bus.power_management_chip_address =
                Some(u32::from_be_bytes([cells[0], cells[1], cells[2], cells[3]]));
        }
        if let Some(cells) = bus_register_property {
            bus.controller_physical_window = parse_register_property(cells);
        }
        if clocks.is_none() && resets.is_none() {
            // A bus that names nothing keeps the constant plan, and the addresses it does state.
            return Ok(bus);
        }
        let collected = collect_system_steps(clocks, resets, &mut bus.steps, &mut provider)?;
        bus.len = collected.len;
        bus.skipped = collected.skipped;
        bus.truncated = collected.truncated;
        if bus.len == 0 {
            return Ok(PowerManagementChipBus::fallback(
                Some(power_management_chip),
                bus.skipped,
            ));
        }
        bus.from_tree = true;
        Ok(bus)
    }
}

/// What [`collect_system_steps`] filled in and what it left out.
struct Collected {
    len: usize,
    skipped: usize,
    truncated: bool,
}

/// **Turn a node's `clocks` and `resets` into SYS-domain steps**, clocks first, into `steps`. The
/// one walk [`PowerManagementChipBus::from_nodes`] and [`DevicePlan::from_specifiers`] share, so a
/// specifier is classified the same way whichever device named it: see [`Specifier`] for what is kept,
/// counted or abandoned.
fn collect_system_steps<'t, E>(
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
        let specifiers = list.chunks(8);
        let total = specifiers.len();
        for (i, specifier) in specifiers.enumerate() {
            match system_step(specifier, is_clock, provider)? {
                Specifier::Step(step) if out.len < steps.len() => {
                    steps[out.len] = step;
                    out.len += 1;
                }
                Specifier::Step(_) => out.truncated = true,
                Specifier::Foreign => out.skipped += 1,
                // The stride is wrong from here on, so nothing after this is a specifier.
                Specifier::Unreadable => {
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
/// Name: ratified 2026-10-26 (UTC), on #2760. Refused `MAX_DEVICE_STEPS` (max spelled out, as the
/// tree-wide ruling asks).
pub const MAXIMUM_DEVICE_STEPS: usize = 4;

/// **A device's own SYS-domain clocks and resets, as its node names them.** Unlike
/// [`PowerManagementChipBus`] there is no constant fallback: an empty plan means the tree named
/// nothing usable, and the caller decides what that costs.
///
/// Name: ratified 2026-10-26 (UTC).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct DevicePlan {
    steps: [Step; MAXIMUM_DEVICE_STEPS],
    len: usize,
    /// Specifiers that are not a SYS-domain gate or line, counted and never written.
    pub skipped: usize,
    /// True when the node named more usable steps than [`MAXIMUM_DEVICE_STEPS`].
    pub truncated: bool,
}

impl DevicePlan {
    /// The steps to walk, clocks before resets.
    ///
    /// Name: ratified 2026-10-26 (UTC).
    #[must_use]
    pub fn plan(&self) -> &[Step] {
        &self.steps[..self.len]
    }

    /// **A device node's `clocks` and `resets`, as SYS-domain steps.** Either property may be
    /// `None`, and a node naming neither gives an empty plan. Specifiers are classified exactly as
    /// [`PowerManagementChipBus::from_nodes`] classifies its bus's, so a vendor `clkgen`/`rstgen`
    /// pair and mainline's `syscrg` both answer; `provider` is the same phandle lookup.
    ///
    /// # Errors
    ///
    /// Propagates the first error `provider` returns.
    ///
    /// Name: ratified 2026-10-26 (UTC).
    pub fn from_specifiers<'t, E>(
        clocks: Option<&[u8]>,
        resets: Option<&[u8]>,
        mut provider: impl FnMut(u32, &[u8]) -> Result<Option<&'t [u8]>, E>,
    ) -> Result<Self, E> {
        let mut plan = DevicePlan {
            steps: [Step::EnableClock(0); MAXIMUM_DEVICE_STEPS],
            len: 0,
            skipped: 0,
            truncated: false,
        };
        let collected = collect_system_steps(clocks, resets, &mut plan.steps, &mut provider)?;
        plan.len = collected.len;
        plan.skipped = collected.skipped;
        plan.truncated = collected.truncated;
        Ok(plan)
    }
}

// ===== The direct reset write, and the numbers it needs =====
//
// On a VisionFive 2 whose firmware cannot reset the board, the bus can be provably up (clocks
// running, reset released) while OpenSBI's read of the PMIC still fails. The remedy is for the caller
// to write the AXP15060 itself over a DesignWare I2C master, which needs three numbers the tree and
// the CRG hold: the controller's base, the PMIC's address, and the I2C input clock that scales
// every timing count. Everything here is host-testable arithmetic over words the caller reads; the
// sources are the vendor trees cited in the crate's header.

/// The SYS **syscon** window, `0x1303_0000`: where the PLL control words live, distinct from
/// [`SYSTEM_BASE_PHYSICAL_ADDRESS`]'s CRG window the clock words live in. Vendor `jh7110-regs.h` at `VF2_v2.10.4`
/// (\[vendor-regs\]: `SYS_SYSCON_BASE 0x13030000`, `SYS_CRG_BASE 0x13020000`), which resolves
/// what looked like an overlap between the PLL words at `0x2c..0x34` and the early clock words:
/// different windows, same offsets. SYS is the chip's abbreviation for [`SYSTEM`], and syscon is
/// its system-controller block.
///
/// [vendor-regs]: https://github.com/starfive-tech/u-boot/blob/1ae835c25e3d8b7bc3c8981fb720f030c1892d57/arch/riscv/include/asm/arch-jh7110/jh7110-regs.h
///
/// Name: ratified 2026-11-05 (UTC), #3279.
#[doc(alias = "system syscon")]
#[doc(alias = "syscon")]
#[doc(alias = "SYS_SYSCON_BASE")]
pub const SYSTEM_CONTROLLER_BASE_PHYSICAL_ADDRESS: u64 = 0x1303_0000;

/// The SYS syscon window's size, matching the CRG window's. The name follows
/// [`SYSTEM_CONTROLLER_BASE_PHYSICAL_ADDRESS`]'s.
///
/// Name: ratified 2026-11-05 (UTC), #3279.
pub const SYSTEM_CONTROLLER_SIZE_BYTES: u64 = 0x1_0000;

/// `JH7110_BUS_ROOT` in the vendor header, **5**. Word `0x14`: a mux
/// whose bit 24 picks `osc` (0) or `pll2_out` (1), per the vendor clock driver's registration
/// (`bus_root_sels`, `starfive_clk_mux(..., SYS_OFFSET(JH7110_BUS_ROOT), 1, ...)`).
///
/// Name: ratified 2026-11-05 (UTC), #3279.
#[doc(alias = "JH7110_SYSCLK_BUS_ROOT")]
pub const SYSTEM_CLOCK_BUS_ROOT: u32 = 5;

/// `JH7110_AXI_CFG0`, **7**. Word `0x1c`: a one-based 2-bit divider off `bus_root`.
///
/// Name: ratified 2026-11-05 (UTC), #3279.
#[doc(alias = "JH7110_SYSCLK_AXI_CFG0")]
pub const SYSTEM_CLOCK_ADVANCED_EXTENSIBLE_INTERFACE_CONFIGURATION_0: u32 = 7;

/// `JH7110_STG_AXIAHB`, **8**. Word `0x20`: a one-based 2-bit divider off `axi_cfg0`.
///
/// Name: ratified 2026-11-05 (UTC), #3279.
#[doc(alias = "JH7110_SYSCLK_STG_AXIAHB")]
pub const SYSTEM_CLOCK_SYSTEM_TOP_GROUP_ADVANCED_EXTENSIBLE_INTERFACE_ADVANCED_HIGH_PERFORMANCE_BUS: u32 = 8;

/// `JH7110_APB_BUS_FUNC`, **11**. Word `0x2c`: a one-based 4-bit divider off `stg_axiahb`, the
/// last divider before the APB tree that `u5_dw_i2c_clk_apb` (gate id 143, bit 31 only) sits on.
///
/// Name: ratified 2026-11-05 (UTC), #3279.
#[doc(alias = "JH7110_SYSCLK_APB_BUS_FUNC")]
pub const SYSTEM_CLOCK_ADVANCED_PERIPHERAL_BUS_FUNCTION: u32 = 11;

/// The oscillator every JH7110 rate chain is rooted in, 24 MHz, `refclk` in the vendor PLL code.
pub(crate) const OSCILLATOR_HERTZ: u64 = 24_000_000;

/// The vendor PLL code's default for a PLL it cannot interpret (`deffreq` for PLL2), returned
/// rather than guessed from fields that are not in integer mode.
pub(crate) const PHASE_LOCKED_LOOP_2_DEFAULT_HERTZ: u64 = 1_188_000_000;

/// **The words the I2C input-clock chain reads**, one struct so the caller can read them in one
/// place and the arithmetic can be tested without a device. The CRG
/// words come from [`SYSTEM`]'s window at the offsets the ids above give; the PLL2 words from
/// [`SYSTEM_CONTROLLER_BASE_PHYSICAL_ADDRESS`]'s window at `0x2c` (DACPD bit 15, DSMPD bit 16, FBDIV bits 28:17), `0x30`
/// (POSTDIV1 bits 29:28) and `0x34` (PREDIV bits 5:0), exactly the vendor `pll.c` masks.
///
/// Name: ratified 2026-11-05 (UTC), #3279. Refused `IcClkWords` (three abbreviations; these are the
/// inter-integrated circuit controller's clock words).
#[doc(alias = "i2c")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InterIntegratedCircuitClockWords {
    /// Word `0x14` of the CRG: `bus_root`'s mux, bit 24.
    pub bus_root: u32,
    /// Word `0x1c`: `axi_cfg0`'s divider, bits 1:0, one-based.
    ///
    /// Name: ratified 2026-11-05 (UTC), #3279.
    pub advanced_extensible_interface_configuration_0: u32,
    /// Word `0x20`: `stg_axiahb`'s divider, bits 1:0, one-based.
    ///
    /// Name: ratified 2026-11-05 (UTC), #3279.
    pub system_top_group_advanced_extensible_interface_advanced_high_performance_bus: u32,
    /// Word `0x2c` of the CRG: `apb_bus_func`'s divider, bits 3:0, one-based.
    ///
    /// Name: ratified 2026-11-05 (UTC), #3279.
    pub advanced_peripheral_bus_function: u32,
    /// SYS syscon word `0x2c`: PLL2's DACPD, DSMPD and FBDIV.
    ///
    /// Name: ratified 2026-11-05 (UTC), #3279.
    pub phase_locked_loop_2_dacpd_dsmpd_and_feedback_divider_word: u32,
    /// SYS syscon word `0x30`: PLL2's POSTDIV1, bits 29:28.
    ///
    /// Name: ratified 2026-11-05 (UTC), #3279.
    pub phase_locked_loop_2_post_divider_1_word: u32,
    /// SYS syscon word `0x34`: PLL2's PREDIV, bits 5:0.
    ///
    /// Name: ratified 2026-11-05 (UTC), #3279.
    pub phase_locked_loop_2_pre_divider_word: u32,
}

/// A one-based divider field of `width` bits: the value when nonzero, 1 when the field reads zero
/// (a zero a one-based divider never means, and a boot that left one is a fact the caller's print
/// should carry rather than a divide-by-zero).
///
/// Name: ratified 2026-10-26 (UTC). Refused `one_based_div` (`div` abbreviated divider).
#[must_use]
pub const fn one_based_divider(word: u32, width: u32) -> u64 {
    let mask = if width >= 32 {
        u32::MAX
    } else {
        (1 << width) - 1
    };
    let value = (word & mask) as u64;
    if value == 0 { 1 } else { value }
}

/// **The I2C input clock of the PMIC's bus**, the frequency the `DesignWare` I2C databook calls
/// `ic_clk`, from the words above: the rate chain
/// `bus_root -> axi_cfg0 -> stg_axiahb -> apb_bus_func -> (gate) u5_dw_i2c_clk_apb`. Every step is
/// the vendor clock driver's own registration; the gates do not divide. PLL2's rate follows the
/// vendor `pll.c` exactly: integer mode (`dacpd == 1 && dsmpd == 1`) is
/// `24 MHz * feedback_divider / (pre_divider * post_divider_1)` (the vendor code's `fbdiv`, `prediv` and
/// `postdiv1`), anything else is the vendor's own default rather than a
/// guess. The u64 divisions truncate, as the vendor's do.
///
/// Name: ratified 2026-11-05 (UTC), #3279. Refused `i2c5_ic_clk` (`ic_clk` is the `DesignWare`
/// databook's signal name, which the doc and `doc(alias)` carry; the function returns the clock's
/// frequency), as `IcClkWords` was refused for `InterIntegratedCircuitClockWords`.
#[must_use]
#[doc(alias = "ic_clk")]
#[doc(alias = "i2c")]
pub const fn inter_integrated_circuit_5_clock_frequency(
    words: &InterIntegratedCircuitClockWords,
) -> u64 {
    advanced_extensible_interface_configuration_0_hertz(words)
        / one_based_divider(
            words.system_top_group_advanced_extensible_interface_advanced_high_performance_bus,
            2,
        )
        / one_based_divider(words.advanced_peripheral_bus_function, 4)
}

/// **`axi_cfg0`'s rate**: `bus_root` (PLL2 or the oscillator, by its mux) over `axi_cfg0`'s own
/// divider. The parent of both the PMIC bus's clock chain ([`inter_integrated_circuit_5_clock_frequency`]) and the SD/MMC
/// controllers' card clocks ([`SYSTEM_CLOCK_SD_INPUT_OUTPUT_SD_CARD`]), each a registration in the vendor clock driver.
///
///
/// Name: ratified 2026-11-05 (UTC), #3279.
#[must_use]
#[doc(alias = "axi_cfg0")]
pub const fn advanced_extensible_interface_configuration_0_hertz(
    words: &InterIntegratedCircuitClockWords,
) -> u64 {
    let parent = if (words.bus_root >> 24) & 1 == 1 {
        let dacpd = (words.phase_locked_loop_2_dacpd_dsmpd_and_feedback_divider_word >> 15) & 1;
        let dsmpd = (words.phase_locked_loop_2_dacpd_dsmpd_and_feedback_divider_word >> 16) & 1;
        let feedback_divider = ((words.phase_locked_loop_2_dacpd_dsmpd_and_feedback_divider_word
            >> 17)
            & 0xfff) as u64;
        let pre_divider_raw = (words.phase_locked_loop_2_pre_divider_word & 0x3f) as u64;
        let pre_divider = if pre_divider_raw == 0 {
            1
        } else {
            pre_divider_raw
        };
        let post_divider_1 = 1u64 << ((words.phase_locked_loop_2_post_divider_1_word >> 28) & 3);
        if dacpd == 1 && dsmpd == 1 && feedback_divider > 0 {
            OSCILLATOR_HERTZ * feedback_divider / (pre_divider * post_divider_1)
        } else {
            PHASE_LOCKED_LOOP_2_DEFAULT_HERTZ
        }
    } else {
        OSCILLATOR_HERTZ
    };
    parent / one_based_divider(words.advanced_extensible_interface_configuration_0, 2)
}

/// **Each SD/MMC controller's card clock** (`ciu` in the tree), by controller index: the eMMC
/// socket's, then the microSD slot's. Each divides [`advanced_extensible_interface_configuration_0_hertz`] by its own one-based divider.
///
/// Name: ratified 2026-11-05 (UTC), #3279.
#[doc(alias = "JH7110_SYSCLK_SDIO0_SDCARD")]
pub const SYSTEM_CLOCK_SD_INPUT_OUTPUT_SD_CARD: [u32; 2] =
    [SYSCLK_SDIO0_SDCARD, SYSCLK_SDIO1_SDCARD];

/// The card clocks' largest divider (the vendor driver registers them with a maximum of 15).
///
/// Name: ratified 2026-11-05 (UTC), #3279. Refused `SDCARD_DIVIDER_MAX` (max spelled out, as the
/// tree-wide ruling asks).
pub const MAXIMUM_SD_CARD_DIVIDER: u32 = 15;

/// **The divider that brings a card clock to at most `want_hertz`** from `parent_hertz`: the smallest
/// that does, so the rate is as close below the ask as the divider allows, clamped to
/// `1..=`[`MAXIMUM_SD_CARD_DIVIDER`]. The tree's `assigned-clock-rates` is a rate the OS is asked to
/// program, and the firmware of a VisionFive 2 leaves this divider at 2, four times the tree's 50 MHz.
///
/// Name: ratified 2026-11-05 (UTC), #3279; renamed 2026-11-08 (UTC) to separate the words of the card.
#[must_use]
pub const fn sd_card_divider_for(parent_hertz: u64, want_hertz: u64) -> u32 {
    if want_hertz == 0 {
        return MAXIMUM_SD_CARD_DIVIDER;
    }
    let divider = parent_hertz.div_ceil(want_hertz);
    if divider < 1 {
        1
    } else if divider > MAXIMUM_SD_CARD_DIVIDER as u64 {
        MAXIMUM_SD_CARD_DIVIDER
    } else {
        divider as u32
    }
}

/// `word` with its divider field ([`clock_divider`]'s bits) replaced by `divider`, every other bit
/// (the enable, the mux) as found.
///
/// Name: ratified 2026-10-26 (UTC).
#[must_use]
pub const fn with_divider(word: u32, divider: u32) -> u32 {
    (word & !0x00ff_ffff) | (divider & 0x00ff_ffff)
}

/// **The 100 kHz standard-mode programming for a `DesignWare` I2C controller**: counts of the
/// controller's input clock for the I2C standard mode's minimum SCL high time (4000 ns) and
/// minimum SCL low time (4700 ns), stretched so one SCL period is at least 10 microseconds, and a
/// 300 ns SDA hold in input-clock counts. The control word is master mode, restart enable, slave
/// disable, standard speed.
///
/// The limits are the I2C-bus specification's standard-mode figures (the NXP specification and
/// user manual UM10204, table of timing characteristics: `tHIGH` 4.0 us, `tLOW` 4.7 us, and the
/// 300 ns internal SDA hold a device must provide to bridge the undefined falling edge of SCL).
/// The counts treat a phase as lasting at least its count of input-clock periods, which is how the
/// `DesignWare` databook's `IC_SS_SCL_HCNT` and `IC_SS_SCL_LCNT` registers are described. No
/// permissively licensed driver that this crate could cite states the equations (OpenBSD's
/// `dwiic.c` takes the counts from firmware and FreeBSD has no `DesignWare` I2C driver), so they
/// are derived here from those two sources and from nothing else.
///
/// 100 kHz rather than a faster mode, because the AXP15060's datasheet timing is met by every
/// standard-mode controller and the failure this answers was OpenSBI trusting a controller at
/// reset defaults; slower than the specification floor is safe, faster is not.
///
/// Name: ratified 2026-10-26 (UTC).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StandardMode {
    /// The `IC_CON` word: master mode, restart enable, slave disable and standard speed set.
    ///
    /// Name: ratified 2026-11-05 (UTC), #3279.
    #[doc(alias = "IC_CON")]
    pub control: u32,
    /// `IC_SS_SCL_HCNT`.
    ///
    /// Name: ratified 2026-11-05 (UTC), #3279.
    #[doc(alias = "IC_SS_SCL_HCNT")]
    pub high_count: u32,
    /// `IC_SS_SCL_LCNT`.
    ///
    /// Name: ratified 2026-11-05 (UTC), #3279.
    #[doc(alias = "IC_SS_SCL_LCNT")]
    pub low_count: u32,
    /// `IC_SDA_HOLD`, the 300 ns hold in input-clock counts.
    ///
    /// Name: ratified 2026-11-05 (UTC), #3279.
    #[doc(alias = "IC_SDA_HOLD")]
    pub data_hold: u32,
}

/// The `IC_CON` word for a standard-speed master, written as one literal so no operator is left
/// to mutate (the fields do not overlap, so `|` and `^` would give the same word). Bit 0 is
/// master mode (`IC_CON_MM`), the speed field in bits 2:1 holds `0b01` for standard speed
/// (`IC_CON_SPD_SS`, bit 1 set), bit 5 is restart enable (`IC_CON_RE`) and bit 6 is slave disable
/// (`IC_CON_SD`).
const STANDARD_MODE_CONTROL: u32 = 0b0110_0011;
/// Standard mode's minimum SCL high time, in nanoseconds.
const STANDARD_HIGH_NANOSECONDS: u128 = 4_000;
/// Standard mode's minimum SCL low time, in nanoseconds.
const STANDARD_LOW_NANOSECONDS: u128 = 4_700;
/// The SDA hold a device provides to bridge SCL's undefined falling edge, in nanoseconds.
const STANDARD_SDA_HOLD_NANOSECONDS: u128 = 300;
/// The bus rate this module programs, in hertz.
const STANDARD_BUS_HERTZ: u128 = 100_000;

/// The smallest count of input-clock periods lasting at least `period_nanoseconds`, never below 1 (zero is
/// not a count a controller can be programmed with) and never above `u32::MAX`.
const fn counts(clock_frequency: u64, period_nanoseconds: u128) -> u32 {
    let ticks = (clock_frequency as u128 * period_nanoseconds).div_ceil(1_000_000_000);
    // The saturation subtracts the excess instead of comparing, so no boundary is left to
    // mutate: `ticks` minus whatever lies above `u32::MAX` is `min(ticks, u32::MAX)`.
    let ticks = ticks - ticks.saturating_sub(u32::MAX as u128);
    if ticks == 0 { 1 } else { ticks as u32 }
}

/// The counts for a 100 kHz standard-mode bus. `clock_frequency` is the controller's input clock
/// in hertz, the databook's `ic_clk`.
///
/// Each phase is rounded up to a whole count, so it never falls short of its specification
/// minimum: the high count covers 4000 ns and the low count covers 4700 ns. Those two alone make a
/// bus faster than 100 kHz, so the fill is this crate's choice and as simple as it can be: when the two
/// minimums add to fewer counts than one 10 microsecond period, the whole shortfall goes to the
/// low count. A longer low time is always legal (standard mode has no maximum, and a slave may
/// hold SCL low for longer anyway), and the high count stays at the specification minimum. The
/// result guarantees that `high_count + low_count` counts never span less than 10 microseconds, so the bus
/// never runs faster than 100 kHz, and that each phase meets its minimum. Rise and fall times on
/// the wire only add to the period. An input clock below 100 kHz still yields counts of at least 1
/// and cannot reach 100 kHz; a caller that logs the computed input clock makes that case visible
/// as one.
///
/// Name: ratified 2026-11-05 (UTC), #3279.
#[must_use]
pub const fn standard_mode_100_kilohertz(clock_frequency: u64) -> StandardMode {
    let high_count = counts(clock_frequency, STANDARD_HIGH_NANOSECONDS);
    let minimum_low_count = counts(clock_frequency, STANDARD_LOW_NANOSECONDS);
    let period = counts(clock_frequency, 1_000_000_000 / STANDARD_BUS_HERTZ);
    let shortfall = period.saturating_sub(high_count);
    // The larger of the two, written as the minimum plus whatever the shortfall exceeds it by.
    let low_count = minimum_low_count + shortfall.saturating_sub(minimum_low_count);
    StandardMode {
        control: STANDARD_MODE_CONTROL,
        high_count,
        low_count,
        data_hold: counts(clock_frequency, STANDARD_SDA_HOLD_NANOSECONDS),
    }
}

/// The AXP15060 PMIC (power management integrated circuit) register OpenSBI's reset and shutdown
/// both go through, `0x32`, and the bit that resets: bit 6 (bit 7 powers off), as read from a
/// VisionFive 2 board; the same register and bit are in the OpenSBI source for the board
/// (`platform/generic/starfive/jh7110.c`, BSD-2-Clause). The caller writes the reset bit alone,
/// read-modify-write, so the register's other bits keep whatever the PMIC already holds.
///
/// Name: ratified 2026-10-26 (UTC), on #2760. Refused `PMIC_RESET_REG` (PMIC and `reg` spelled out)
/// and `PMIC_RESET_REGISTER` (PMIC spelled out).
#[doc(alias = "pmic")]
pub const POWER_MANAGEMENT_CHIP_RESET_REGISTER: u8 = 0x32;

/// The reset bit of [`POWER_MANAGEMENT_CHIP_RESET_REGISTER`].
///
/// Name: ratified 2026-10-26 (UTC), on #2760. Refused `PMIC_RESET_BIT` (PMIC spelled out).
#[doc(alias = "pmic")]
pub const POWER_MANAGEMENT_CHIP_RESET_BIT: u8 = 6;

/// What one `<phandle id>` specifier turned out to be.
enum Specifier {
    /// A SYS-domain gate or line, in bounds.
    Step(Step),
    /// A well-formed specifier for something else: another provider, or an id past [`SYSTEM`]'s
    /// bounds (the vendor tree's virtual clock 298).
    Foreign,
    /// A provider without exactly one cell per specifier, or a short tail. The walk takes
    /// eight-byte strides, so everything from here on in the list is unreadable and skipped: the
    /// safe direction, since a skipped specifier costs a fallback and a misread one would be a
    /// store.
    Unreadable,
}

/// Classify one specifier; see [`Specifier`].
fn system_step<'t, E>(
    specifier: &[u8],
    is_clock: bool,
    provider: &mut impl FnMut(u32, &[u8]) -> Result<Option<&'t [u8]>, E>,
) -> Result<Specifier, E> {
    let [p0, p1, p2, p3, i0, i1, i2, i3] = *specifier else {
        return Ok(Specifier::Unreadable);
    };
    let phandle = u32::from_be_bytes([p0, p1, p2, p3]);
    let id = u32::from_be_bytes([i0, i1, i2, i3]);
    let cells = if is_clock {
        &b"#clock-cells"[..]
    } else {
        &b"#reset-cells"[..]
    };
    if provider(phandle, cells)? != Some(&[0, 0, 0, 1][..]) {
        return Ok(Specifier::Unreadable);
    }
    let Some(compatible) = provider(phandle, b"compatible")? else {
        return Ok(Specifier::Foreign);
    };
    let known = [
        COMPATIBLE_SYSTEM_CLOCK_AND_RESET_GENERATOR,
        if is_clock {
            COMPATIBLE_VENDOR_CLOCK_GENERATOR
        } else {
            COMPATIBLE_VENDOR_RESET_GENERATOR
        },
    ];
    if !compatible.split(|&b| b == 0).any(|c| known.contains(&c)) {
        return Ok(Specifier::Foreign);
    }
    let step = if is_clock {
        SYSTEM.clock_offset(id).map(|_| Step::EnableClock(id))
    } else {
        SYSTEM.reset_bit(id).map(|_| Step::DeassertReset(id))
    };
    Ok(step.map_or(Specifier::Foreign, Specifier::Step))
}

impl Domain {
    /// The byte offset of `index`'s clock word, or `None` if this domain has no such clock.
    ///
    /// One 32-bit word per clock, in index order, from \[mainline-clk\]: `priv->base + 4 *
    /// clk->idx`. The `Option` is the bound: a caller holding an identifier from the wrong domain
    /// gets nothing rather than a plausible-looking offset into somebody else's register.
    ///
    /// Name: ratified 2026-10-26 (UTC).
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
    ///
    /// Name: ratified 2026-10-26 (UTC).
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
///
/// Name: ratified 2026-10-26 (UTC).
#[must_use]
pub const fn is_deasserted(status_word: u32, mask: u32) -> bool {
    status_word & mask != 0
}

/// **Is this clock running?** Given the clock's own word.
///
/// Name: ratified 2026-10-26 (UTC).
#[must_use]
pub const fn is_clock_enabled(clock_word: u32) -> bool {
    clock_word & CLOCK_ENABLE != 0
}

/// How many `EnableClock` steps a report keeps words for. Two is what the TRNG needs; four is
/// slack for the next device, and a plan with more simply stops recording rather than growing the
/// report, which is what [`BringUpReport::truncated`] says out loud.
///
/// Name: ratified 2026-10-26 (UTC), on #2760. Refused `MAX_RECORDED_CLOCKS` (max spelled out, as
/// the tree-wide ruling asks).
pub const MAXIMUM_RECORDED_CLOCKS: usize = 4;

/// What the hardware said, in enough detail that a log of it is diagnosable without a second trip
/// to the machine.
///
/// **The `before` words are the load-bearing ones.** If the clocks read back already enabled and
/// the reset already released, then a gated clock or a held reset was not the cause, and an
/// all-zero TRNG register window has some other one. That is the outcome a first
/// bring-up most needs to be able to tell apart, and only a before-and-after can tell it.
///
/// Name: ratified 2026-10-26 (UTC). Refused `Report` (a word that could name almost anything; it
/// reports one bring-up).
#[derive(Clone, Copy, Debug, Default)]
#[non_exhaustive]
pub struct BringUpReport {
    /// Each `EnableClock` step's word as it read *before* anything was written, in plan order.
    pub clock_before: [u32; MAXIMUM_RECORDED_CLOCKS],
    /// The same words read back *after* the enable bit was stored.
    pub clock_after: [u32; MAXIMUM_RECORDED_CLOCKS],
    /// How many entries of the two arrays above are meaningful.
    pub clocks: usize,
    /// True when the plan had more clock steps than [`MAXIMUM_RECORDED_CLOCKS`]. They were still
    /// performed; only the recording stopped.
    pub truncated: bool,
    /// The reset-assert word as it read *before* the plan's last `DeassertReset` step wrote it.
    /// Zero when the plan had none, which [`BringUpReport::had_reset`] distinguishes from a real zero.
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
    /// How many status reads the deassert took. A count equal to the caller's poll limit means the
    /// line never came out.
    pub polls: u32,
    /// Steps whose identifier this domain rejected, which would mean the plan and the domain
    /// disagree: a programming error, not a hardware condition. Nonzero here invalidates the rest.
    pub rejected: usize,
    /// How many `DeassertReset` steps the plan took. The fields above describe the last of them;
    /// this and [`BringUpReport::resets_released`] cover a plan with more than one (`gmac0`'s
    /// has two).
    pub resets: u32,
    /// How many of those read as released before the poll gave up.
    pub resets_released: u32,
    /// The last `SelectParent` step's clock word before it was written, and
    /// [`BringUpReport::select_parent_after`] the word read back. Both zero when the plan had none, which
    /// [`BringUpReport::had_select_parent`] distinguishes.
    ///
    /// Name: ratified 2026-11-08 (UTC), renamed to use the step's name.
    pub select_parent_before: u32,
    /// See [`BringUpReport::select_parent_before`].
    ///
    /// Name: ratified 2026-11-08 (UTC), renamed to use the step's name.
    pub select_parent_after: u32,
    /// True when the plan contained a `SelectParent`.
    ///
    /// Name: ratified 2026-11-08 (UTC), renamed to use the step's name.
    pub had_select_parent: bool,
}

impl BringUpReport {
    /// **Did every reset step read as released?** True for a plan with none.
    ///
    /// Name: ratified 2026-10-28 (UTC) (calef, relayed by the maintainer), renamed from
    /// `every_reset_released`: the question shape asks a `bool` `is_`, `has_`, `can_` or a finite
    /// verb.
    #[must_use]
    pub fn all_resets_are_released(&self) -> bool {
        self.resets_released == self.resets
    }

    /// **Did every clock this plan named read its enable bit back?** A clock that does not is a
    /// window with nothing behind it, or a base address that is not this controller.
    ///
    /// Name: ratified 2026-10-26 (UTC).
    #[must_use]
    pub fn has_clocks_running(&self) -> bool {
        self.clocks > 0
            && self.clock_after[..self.clocks]
                .iter()
                .all(|&w| is_clock_enabled(w))
    }

    /// **Was the device already up before this ran?** True when every recorded clock was enabled
    /// and, if the plan had one, the reset was already released. See the type's own doc for why
    /// this is the question a first bring-up asks first.
    ///
    /// Name: ratified 2026-10-26 (UTC).
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

/// Where a domain's registers are, as a caller concluded from a tree or from [`RegisterWindow::fallback`].
///
/// Name: ratified 2026-10-26 (UTC). Refused `Found` (a word that could name almost anything; it is
/// where a window was located).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct WindowLocation {
    /// Physical base of the register window.
    ///
    /// Name: ratified 2026-11-08 (UTC), renamed to say the address is physical.
    pub base_physical_address: u64,
    /// Its size in bytes.
    pub size: u64,
    /// **True when the device tree said so, false when this is the window's constant.** The one
    /// field a caller's log must carry: a base that came from a constant is a base nobody on
    /// that machine confirmed, and a reader three months later cannot re-derive which it was.
    pub from_tree: bool,
    /// Which `compatible` string matched, or `None` when nothing did and the constant was used.
    pub compatible: Option<&'static [u8]>,
}

/// **STG clock and reset generator** (CRG, the device tree's `stgcrg` node, \[mainline-dts\]):
/// mainline's dedicated STG clock-and-reset controller node.
///
/// The vendor string is `starfive,jh7110-stgcrg`.
///
/// Name: ratified 2026-11-02 (UTC), #3090. The generator is spelled out, as the crate name
/// spells it.
#[doc(alias = "clock and reset generator")]
pub const COMPATIBLE_SYSTEM_TOP_GROUP_CLOCK_AND_RESET_GENERATOR: &[u8] = b"starfive,jh7110-stgcrg";

/// **Vendor clock generator** (the `clkgen` node, \[vendor-dts\]): the vendor U-Boot's single
/// clock controller covering sys, stg and aon. This is the one the firmware of a VisionFive 2 actually
/// serves, if it serves either.
///
/// Name: ratified 2026-11-05 (UTC), #3279.
#[doc(alias = "clock generator")]
pub const COMPATIBLE_VENDOR_CLOCK_GENERATOR: &[u8] = b"starfive,jh7110-clkgen";

/// **Vendor reset generator** (the `rstgen` node, \[vendor-dts\]): the vendor U-Boot's separate
/// reset controller, whose `reg` list covers the same windows. Tried last: it names the same
/// address as the clkgen node, so it is only reached by a tree that carries one and not the other.
///
/// Name: ratified 2026-11-05 (UTC), #3279.
#[doc(alias = "reset generator")]
pub const COMPATIBLE_VENDOR_RESET_GENERATOR: &[u8] = b"starfive,jh7110-reset";

/// The `reg-names` entry naming the STG window, in each vendor node's own spelling
/// (`"stg"` in `clkgen`, `"stgcrg"` in `rstgen`; \[vendor-dts\]).
const VENDOR_CLOCK_GENERATOR_SYSTEM_TOP_GROUP_NAME: &[u8] = b"stg";
const VENDOR_RESET_GENERATOR_SYSTEM_TOP_GROUP_NAME: &[u8] = b"stgcrg";

/// **SYS clock and reset generator** (CRG, the device tree's `syscrg` node, \[mainline-dts\]):
/// mainline's dedicated SYS clock-and-reset controller node, `syscrg: clock-controller@13020000`.
///
/// The vendor string is `starfive,jh7110-syscrg`.
///
/// Name: ratified 2026-11-02 (UTC), #3090. The generator is spelled out, as the crate name
/// spells it.
#[doc(alias = "clock and reset generator")]
pub const COMPATIBLE_SYSTEM_CLOCK_AND_RESET_GENERATOR: &[u8] = b"starfive,jh7110-syscrg";

/// The `reg-names` entry naming the SYS window, in each vendor node's own spelling (`"sys"` in
/// `clkgen`, `"syscrg"` in `rstgen`; \[vendor-dts\]). Window 0 in both today, found by name anyway
/// for the reason [`RegisterWindow`] gives.
const VENDOR_CLOCK_GENERATOR_SYSTEM_NAME: &[u8] = b"sys";
const VENDOR_RESET_GENERATOR_SYSTEM_NAME: &[u8] = b"syscrg";

/// One way a device tree names a domain's window: a node's `compatible`, and the `reg-names`
/// entry that selects the window when the node carries several.
///
/// Name: ratified 2026-10-26 (UTC). Refused `Spelling` (a word that could name almost anything; it
/// is one tree's spelling of a window).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowSpelling {
    /// The node's `compatible` string.
    pub compatible: &'static [u8],
    /// The `reg-names` entry naming the window, or `None` for a node whose single `reg` is it.
    pub window: Option<&'static [u8]>,
}

/// How many `reg` entries a matched node may carry. Five, because the vendor `rstgen` node names
/// five windows and a short buffer would silently truncate the list `reg-names` is indexing into.
///
/// Name: ratified 2026-11-05 (UTC), #3279. Refused `MAX_REG_ENTRIES` (max spelled out, as the
/// tree-wide ruling asks). Refused `MAX_WINDOWS` (it bounds a node's `reg` entries, not windows).
pub const MAXIMUM_REGISTER_ENTRIES: usize = 5;

/// **Where one domain's registers are, in every spelling the two trees use.**
///
/// A caller tries [`RegisterWindow::spellings`] in order, mainline's first, and asks
/// [`WindowLocation::from_regions`] about the first node that matches; a tree that names none of them gets
/// [`RegisterWindow::fallback`], the constant both published trees agree on, with `from_tree` false. The
/// two vendor nodes carry several windows and are indexed by `reg-names`, not position. An answer
/// is not evidence that the machine is a JH7110.
///
/// Name: ratified 2026-10-26 (UTC). Refused `Window` (a register window, which the name did not
/// say).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegisterWindow {
    /// The spellings to try, in order.
    pub spellings: [WindowSpelling; 3],
    /// The window's base in both published trees.
    ///
    /// Name: ratified 2026-11-08 (UTC), renamed to say the address is physical.
    pub base_physical_address: u64,
    /// The window's size in both published trees.
    pub size: u64,
}

impl RegisterWindow {
    /// The answer for a tree that names none of [`RegisterWindow::spellings`].
    ///
    /// Name: ratified 2026-10-26 (UTC).
    #[must_use]
    pub const fn fallback(&self) -> WindowLocation {
        WindowLocation {
            base_physical_address: self.base_physical_address,
            size: self.size,
            from_tree: false,
            compatible: None,
        }
    }
}

/// **The STG window**, where the TRNG's clocks and reset live.
///
/// Name: ratified 2026-10-26 (UTC), on #2760. Refused `STG_WINDOW` (the domain spelled out).
#[doc(alias = "stg", alias = "STG_WINDOW")]
pub const SYSTEM_TOP_GROUP_WINDOW: RegisterWindow = RegisterWindow {
    spellings: [
        WindowSpelling {
            compatible: COMPATIBLE_SYSTEM_TOP_GROUP_CLOCK_AND_RESET_GENERATOR,
            window: None,
        },
        WindowSpelling {
            compatible: COMPATIBLE_VENDOR_CLOCK_GENERATOR,
            window: Some(VENDOR_CLOCK_GENERATOR_SYSTEM_TOP_GROUP_NAME),
        },
        WindowSpelling {
            compatible: COMPATIBLE_VENDOR_RESET_GENERATOR,
            window: Some(VENDOR_RESET_GENERATOR_SYSTEM_TOP_GROUP_NAME),
        },
    ],
    base_physical_address: SYSTEM_TOP_GROUP_BASE_PHYSICAL_ADDRESS,
    size: SYSTEM_TOP_GROUP_SIZE_BYTES,
};

/// **The AON window**, where `gmac0`'s bus clocks, transmit clock and resets live.
///
/// Name: ratified 2026-10-26 (UTC), on #2760. Refused `AON_WINDOW` (the domain spelled out).
#[doc(alias = "aon", alias = "AON_WINDOW")]
pub const ALWAYS_ON_WINDOW: RegisterWindow = RegisterWindow {
    spellings: [
        WindowSpelling {
            compatible: COMPATIBLE_ALWAYS_ON_CLOCK_AND_RESET_GENERATOR,
            window: None,
        },
        WindowSpelling {
            compatible: COMPATIBLE_VENDOR_CLOCK_GENERATOR,
            window: Some(VENDOR_CLOCK_GENERATOR_ALWAYS_ON_NAME),
        },
        WindowSpelling {
            compatible: COMPATIBLE_VENDOR_RESET_GENERATOR,
            window: Some(VENDOR_RESET_GENERATOR_ALWAYS_ON_NAME),
        },
    ],
    base_physical_address: ALWAYS_ON_BASE_PHYSICAL_ADDRESS,
    size: ALWAYS_ON_SIZE_BYTES,
};

/// **The SYS window**, where I2C5's clock and reset live.
///
/// Name: ratified 2026-10-26 (UTC), on #2760. Refused `SYS_WINDOW` (the domain spelled out).
#[doc(alias = "sys", alias = "SYS_WINDOW")]
pub const SYSTEM_WINDOW: RegisterWindow = RegisterWindow {
    spellings: [
        WindowSpelling {
            compatible: COMPATIBLE_SYSTEM_CLOCK_AND_RESET_GENERATOR,
            window: None,
        },
        WindowSpelling {
            compatible: COMPATIBLE_VENDOR_CLOCK_GENERATOR,
            window: Some(VENDOR_CLOCK_GENERATOR_SYSTEM_NAME),
        },
        WindowSpelling {
            compatible: COMPATIBLE_VENDOR_RESET_GENERATOR,
            window: Some(VENDOR_RESET_GENERATOR_SYSTEM_NAME),
        },
    ],
    base_physical_address: SYSTEM_BASE_PHYSICAL_ADDRESS,
    size: SYSTEM_SIZE_BYTES,
};

impl WindowLocation {
    /// **One spelling's answer**, from the matched node's `reg` entries as `(base, size)` pairs
    /// and, for a spelling with a [`WindowSpelling::window`], its `reg-names`. `None` when the node
    /// has no `reg`, or when a multi-window node's `reg-names` is absent, lacks the entry, or
    /// names one past the end of `reg`: picking a window by position because one tree happened
    /// to put it there is a fact nobody wrote down, and [`RegisterWindow::fallback`] is the honest answer
    /// instead.
    ///
    /// Name: ratified 2026-10-26 (UTC).
    #[must_use]
    pub fn from_regions(
        spelling: &WindowSpelling,
        regions: &[(u64, u64)],
        register_names: Option<&[u8]>,
    ) -> Option<WindowLocation> {
        if regions.is_empty() {
            return None;
        }
        let index = match spelling.window {
            None => 0,
            Some(want) => match register_names.and_then(|bytes| name_index(bytes, want)) {
                Some(i) if i < regions.len() => i,
                _ => return None,
            },
        };
        let (base_physical_address, size) = regions[index];
        Some(WindowLocation {
            base_physical_address,
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

/// Machine-checked proofs, run with `cargo kani`. The domains and plans are this crate's own
/// constants and the device-tree properties are symbolic bytes, so each harness covers every word
/// and every property value inside its stated bound.
#[cfg(kani)]
mod verification {
    use super::*;

    /// One of the three domains this crate describes, chosen by the solver.
    fn any_domain() -> Domain {
        match kani::any::<u8>() % 3 {
            0 => SYSTEM_TOP_GROUP,
            1 => ALWAYS_ON,
            _ => SYSTEM,
        }
    }

    /// True when every id in `plan` is inside `domain`, no clock is selected a parent after it was
    /// enabled, and no clock step comes after a reset release.
    fn ungates_before_releasing(plan: &[Step], domain: &Domain) -> bool {
        let mut released = false;
        let mut i = 0;
        while i < plan.len() {
            match plan[i] {
                Step::EnableClock(clock) => {
                    if released || domain.clock_offset(clock).is_none() {
                        return false;
                    }
                }
                Step::SelectParent { clock, parent } => {
                    if released || domain.clock_offset(clock).is_none() || parent > 0xf {
                        return false;
                    }
                    let mut j = 0;
                    while j < i {
                        if plan[j] == Step::EnableClock(clock) {
                            return false;
                        }
                        j += 1;
                    }
                }
                Step::DeassertReset(reset) => {
                    if domain.reset_bit(reset).is_none() {
                        return false;
                    }
                    released = true;
                }
            }
            i += 1;
        }
        true
    }

    /// The phandle lookup a caller supplies, modeled as a SYS controller with one cell. Every
    /// specifier in the bytes the solver chooses therefore reaches the id bounds check, which is
    /// the step that makes a store. The solver cannot afford a lookup that answers differently on
    /// every call (the plan harnesses then run for over an hour), nor more than three specifiers, nor property lengths other than a whole number of specifiers, so the other answers (another
    /// provider, a different cell count, an error) are covered by the host tests, not proved.
    fn system_provider(_phandle: u32, name: &[u8]) -> Result<Option<&'static [u8]>, ()> {
        if name == b"compatible" {
            Ok(Some(COMPATIBLE_SYSTEM_CLOCK_AND_RESET_GENERATOR))
        } else {
            Ok(Some(&[0, 0, 0, 1]))
        }
    }

    /// **`with_parent` changes only the mux field**: for every word and every parent, each bit
    /// outside [`CLOCK_PARENT_MASK`] is as it was (the enable bit and the divider among them), and
    /// the field holds the parent's low four bits, so an over-wide parent never spills.
    /// Falsification: replayable `crates/jh7110_clock_and_reset/falsifications/verification.with_parent_changes_only_the_mux_field.patch`
    #[kani::proof]
    fn with_parent_changes_only_the_mux_field() {
        let word: u32 = kani::any();
        let parent: u32 = kani::any();
        let result = with_parent(word, parent);
        assert_eq!(result & !CLOCK_PARENT_MASK, word & !CLOCK_PARENT_MASK);
        assert_eq!(result & CLOCK_ENABLE, word & CLOCK_ENABLE);
        assert_eq!((result >> 24) & 0xf, parent & 0xf);
    }

    /// **A clock offset is one of the domain's own words**: `clock_offset` answers `None` exactly
    /// for an index at or past the domain's clock count, and otherwise a word-aligned offset below
    /// `4 * clocks`, which ends before the domain's first reset word.
    /// Falsification: replayable `crates/jh7110_clock_and_reset/falsifications/verification.a_clock_offset_is_one_of_the_domains_own_words.patch`
    #[kani::proof]
    fn a_clock_offset_is_one_of_the_domains_own_words() {
        let domain = any_domain();
        let index: u32 = kani::any();
        match domain.clock_offset(index) {
            None => assert!(index >= domain.clocks),
            Some(offset) => {
                assert!(index < domain.clocks);
                assert_eq!(offset % 4, 0);
                assert!(offset < 4 * u64::from(domain.clocks));
                assert!(offset + 4 <= domain.reset_assert);
            }
        }
    }

    /// **A reset line is one bit of one of the domain's own words**: `reset_bit` answers `None`
    /// exactly for an id at or past the domain's reset count, and otherwise word-aligned assert and
    /// status offsets that sit the same distance from their first words, a word index of
    /// `id / 32` at most, and a mask of exactly one bit, the bit `id % 32`.
    /// Falsification: replayable `crates/jh7110_clock_and_reset/falsifications/verification.a_reset_line_is_one_bit_of_one_of_the_domains_own_words.patch`
    #[kani::proof]
    fn a_reset_line_is_one_bit_of_one_of_the_domains_own_words() {
        let domain = any_domain();
        let id: u32 = kani::any();
        match domain.reset_bit(id) {
            None => assert!(id >= domain.resets),
            Some(bit) => {
                assert!(id < domain.resets);
                assert_eq!(bit.assert_offset % 4, 0);
                assert_eq!(bit.status_offset % 4, 0);
                let word = bit.assert_offset - domain.reset_assert;
                assert_eq!(word, bit.status_offset - domain.reset_status);
                assert_eq!(word, u64::from(id / 32) * 4);
                assert_eq!(bit.mask, 1u32 << (id % 32));
                assert!(bit.mask.is_power_of_two());
            }
        }
    }

    /// **Every fixed plan ungates its clocks before it releases a reset**: the TRNG's, both halves
    /// of the Ethernet controller's, the PMIC bus's and both SD/MMC slots' plans hold only ids
    /// inside their own domain, put every clock step ahead of every reset release, and choose a
    /// clock's parent before enabling that clock.
    /// Falsification: replayable `crates/jh7110_clock_and_reset/falsifications/verification.every_fixed_plan_ungates_its_clocks_before_releasing_a_reset.patch`
    #[kani::proof]
    #[kani::unwind(8)]
    fn every_fixed_plan_ungates_its_clocks_before_releasing_a_reset() {
        assert!(ungates_before_releasing(
            ENTROPY_BRING_UP,
            &SYSTEM_TOP_GROUP
        ));
        assert!(ungates_before_releasing(GMAC0_SYSTEM_BRING_UP, &SYSTEM));
        assert!(ungates_before_releasing(
            GMAC0_ALWAYS_ON_BRING_UP,
            &ALWAYS_ON
        ));
        assert!(ungates_before_releasing(
            POWER_MANAGEMENT_CHIP_BUS_BRING_UP,
            &SYSTEM
        ));
        assert!(ungates_before_releasing(
            SD_INPUT_OUTPUT_BRING_UP[0],
            &SYSTEM
        ));
        assert!(ungates_before_releasing(
            SD_INPUT_OUTPUT_BRING_UP[1],
            &SYSTEM
        ));
    }

    /// **A plan built from a device's specifiers holds only usable steps, in order**: for two
    /// clock specifiers and one reset specifier of any bytes, answered by a SYS controller, the
    /// plan has at most `MAXIMUM_DEVICE_STEPS` steps, every id in the SYS domain's bounds, every
    /// clock step ahead of every reset release.
    /// Falsification: replayable `crates/jh7110_clock_and_reset/falsifications/verification.a_device_plan_from_specifiers_ungates_before_it_releases.patch`
    #[kani::proof]
    #[kani::unwind(24)]
    fn a_device_plan_from_specifiers_ungates_before_it_releases() {
        let clocks: [u8; 16] = kani::any();
        let resets: [u8; 8] = kani::any();
        let clocks = Some(&clocks[..]);
        let resets = Some(&resets[..]);

        if let Ok(device) = DevicePlan::from_specifiers(clocks, resets, system_provider) {
            assert!(device.plan().len() <= MAXIMUM_DEVICE_STEPS);
            assert!(ungates_before_releasing(device.plan(), &SYSTEM));
        }
    }

    /// **The PMIC bus plan is never empty, never longer than its bound, and ungates before it
    /// releases**: from any node properties (two clock and one reset specifier, any
    /// `reg` bytes) and a SYS controller, `from_nodes` answers either the tree's own plan or, when the
    /// tree gave no usable step, the constant plan, and both keep every id in the SYS domain's
    /// bounds with every clock step ahead of every reset release.
    /// Falsification: replayable `crates/jh7110_clock_and_reset/falsifications/verification.the_power_management_chip_bus_plan_is_never_empty_and_ungates_before_it_releases.patch`
    #[kani::proof]
    #[kani::unwind(24)]
    fn the_power_management_chip_bus_plan_is_never_empty_and_ungates_before_it_releases() {
        let clocks: [u8; 16] = kani::any();
        let resets: [u8; 8] = kani::any();
        let bus_register_property: [u8; 16] = kani::any();
        let chip_register_property: [u8; 4] = kani::any();
        let clocks = Some(&clocks[..]);
        let resets = Some(&resets[..]);
        let bus_register_property = Some(&bus_register_property[..]);
        let chip_register_property = Some(&chip_register_property[..]);

        if let Ok(bus) = PowerManagementChipBus::from_nodes(
            COMPATIBLE_POWER_MANAGEMENT_CHIP_MAINLINE,
            chip_register_property,
            bus_register_property,
            clocks,
            resets,
            system_provider,
        ) {
            assert!(!bus.plan().is_empty());
            assert!(bus.plan().len() <= MAXIMUM_POWER_MANAGEMENT_CHIP_BUS_STEPS);
            assert!(ungates_before_releasing(bus.plan(), &SYSTEM));
            if !bus.from_tree {
                assert!(bus.plan() == POWER_MANAGEMENT_CHIP_BUS_BRING_UP);
            }
        }
    }

    /// **`is_deasserted` reads the bit the mask names**: for a one-bit mask and any status word,
    /// the answer is that bit, and a status word with the bit clear is never released.
    /// Falsification: replayable `crates/jh7110_clock_and_reset/falsifications/verification.is_deasserted_reads_the_bit_the_mask_names.patch`
    #[kani::proof]
    fn is_deasserted_reads_the_bit_the_mask_names() {
        let status: u32 = kani::any();
        let bit: u32 = kani::any();
        kani::assume(bit < 32);
        let mask = 1u32 << bit;
        assert_eq!(is_deasserted(status, mask), (status >> bit) & 1 == 1);
        assert!(!is_deasserted(status & !mask, mask));
        assert!(is_deasserted(status | mask, mask));
    }

    /// **`is_clock_enabled` reads bit 31 and nothing else**: for any word the answer is the top
    /// bit, whatever the mux and divider fields hold.
    /// Falsification: replayable `crates/jh7110_clock_and_reset/falsifications/verification.is_clock_enabled_reads_bit_31_and_nothing_else.patch`
    #[kani::proof]
    fn is_clock_enabled_reads_bit_31_and_nothing_else() {
        let word: u32 = kani::any();
        assert_eq!(is_clock_enabled(word), word >> 31 == 1);
        assert!(is_clock_enabled(word | CLOCK_ENABLE));
        assert!(!is_clock_enabled(word & !CLOCK_ENABLE));
    }

    /// **`clock_divider` is the low 24 bits and nothing else**: for any word the answer is the
    /// word's bits 23:0, so it never exceeds `0x00ff_ffff`, and neither the enable bit nor the
    /// mux field moves it.
    /// Falsification: replayable `crates/jh7110_clock_and_reset/falsifications/verification.clock_divider_is_the_low_24_bits_and_nothing_else.patch`
    #[kani::proof]
    fn clock_divider_is_the_low_24_bits_and_nothing_else() {
        let word: u32 = kani::any();
        assert_eq!(clock_divider(word), (word << 8) >> 8);
        assert!(clock_divider(word) <= 0x00ff_ffff);
        assert_eq!(clock_divider(word | CLOCK_ENABLE), clock_divider(word));
        assert_eq!(
            clock_divider(with_parent(word, kani::any())),
            clock_divider(word)
        );
    }
}

#[cfg(test)]
mod tests;
