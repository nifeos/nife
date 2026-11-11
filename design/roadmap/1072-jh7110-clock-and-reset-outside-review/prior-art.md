# Prior art for the jh7110_clock_and_reset crate

An appendix to [the milestone](../1072-jh7110-clock-and-reset-outside-review.md).

How the crate sits against what a reader already knows. Downloads are crates.io's "recent" figure
at the time of the review.

| where | shape | what a reader coming from it will miss here |
|---|---|---|
| Linux `clk-starfive-jh7110-{sys,aon,stg}.c`, `reset-starfive-jh71x0.c` | one table entry per clock (`JH71X0__GATE`, `_GDIV`, `_GMUX`), one word per clock, the reset controller an auxiliary device of the clock driver; a divider is `DIV_ROUND_UP(parent, rate)` clamped to `max_div`; a parent choice is board data (`assigned-clock-parents`) | set-rate for every divider, the PLL as a clock with presets, the ISP and VOUT domains, fractional PLL rates |
| mainline and vendor device trees | the bus's `clocks`/`resets` name what a device needs | nothing: `DevicePlan::from_specifiers` reads the same two properties |
| OpenBSD `stfclock.c`, FreeBSD `jh7110_clk.c` (as the crate cites them; not re-read) | the same per-clock word, a narrower or wider mux mask | the mux width disagreement the crate documents at `CLOCK_PARENT_MASK` |
| vendor U-Boot `clk-jh7110.c`, `starfive_visionfive2.c` | the rate chain the crate transcribes; the board file hard-codes the GMAC0 transmit parent | the crate makes the same board choice a chip constant (item 4) |
| OpenSBI `platform/generic/starfive/jh7110.c` | the PMIC reset register and bit | the same numbers, cited |
| `jh71xx-pac` 0.11.1 (819), `jh71xx-hal` 0.7.2 (743) | svd2rust register access to every block, a HAL above it | no bring-up order, no proofs, no device-tree spellings; register access this crate leaves to the caller |
| `jh7110-vf2-12a-pac`, `jh7110-vf2-13b-pac` 0.2.0 (6 each) | one package per board revision | the ecosystem already splits v1.2A from v1.3B, which item 4 and decision 5 are about |
| `starfive-jh7110-dwmmc` 0.1.8 (670), `axplat-riscv64-visionfive2` 0.1.4 (386) | a card driver and a platform layer for the same board | the card clock arithmetic this crate holds |

Where this crate differs: it is pure logic with no register access. Nine Kani harnesses prove the
order of its plans. It reads both trees' spellings, its bounds turn a wrong-domain id into `None`
rather than a store, and its host tests pin every offset against both numberings. What it lacks: a fractional PLL2 rate,
any domain beyond STG, AON and SYS, and a line between the chip's facts and the board's.
