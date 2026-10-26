# The names in `jh7110_clock_and_reset`

An appendix of [milestone 857 (crates released on their own)](../0857-crates-released-on-their-own.md),
for the crate of milestone 819 (JH7110 clock and reset logic, proven, then released on its own). This
file is the record of every name in the crate, which `helpers/name_release.py` reads in place of
`Name:` blocks: the crate's shipped files carry none.

Ratified: 2026-10-26 (UTC).

Ruled 2026-10-26 (UTC): the 59 names not listed below are ratified as they stand. Eight are renamed
before the first publish: Window becomes RegisterWindow, Spelling becomes WindowSpelling, Found
becomes WindowLocation, MAX_WINDOWS becomes MAX_REG_ENTRIES, PmicBus::from_bus becomes
PmicBus::from_nodes, Report becomes BringUpReport, IcClkWords becomes I2cClockWords, and
one_based_div becomes one_based_divider. AON_WINDOW, STG_WINDOW, SYS_WINDOW and i2c5_ic_clk keep
their names.

Each renamed row names the spelling the ruling turned down and why. A new name in the crate takes a
row here, `provisional` until a ruling, and `script/lint` fails a module or `pub` item without one.

The column `outside` says whether a caller outside the crate reaches the name. Some names have no
row because `script/names` does not count them: the fields of `ResetBit`, `Domain`, `PmicBus`,
`DevicePlan`, `I2cClockWords`, `StandardMode`, `BringUpReport`, `WindowLocation`, `WindowSpelling`
and `RegisterWindow`, and the variants of `Step`. The same ruling covers them. The row `plan`
covers both `PmicBus::plan` and `DevicePlan::plan`.

| name | kind | path | status | outside |
|---|---|---|---|---|
| `jh7110_clock_and_reset` | crate | `src/lib.rs` | ratified 2026-09-13, replacing `jh7110_crg`; kept as the crates.io name (#1806). | yes |
| `tests` | module | `src/tests.rs` | ratified 2026-10-26 (UTC). | no |
| `AON` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `AON_WINDOW` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). Kept as it stands. | yes |
| `BringUpReport` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). Refused `Report` (a word that could name almost anything; it reports one bring-up). | yes |
| `CLOCK_ENABLE` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `COMPATIBLE_AONCRG` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `COMPATIBLE_PMIC_MAINLINE` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `COMPATIBLE_PMIC_VENDOR` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `COMPATIBLE_STGCRG` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `COMPATIBLE_SYSCRG` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `COMPATIBLE_VENDOR_CLKGEN` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `COMPATIBLE_VENDOR_RSTGEN` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `DevicePlan` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `Domain` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `GMAC0_AON_BRING_UP` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `GMAC0_SYS_BRING_UP` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `I2cClockWords` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). Refused `IcClkWords` (three abbreviations; these are the I2C controller's clock words). | yes |
| `MAX_DEVICE_STEPS` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `MAX_RECORDED_CLOCKS` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `MAX_REG_ENTRIES` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). Refused `MAX_WINDOWS` (it bounds a node's `reg` entries, not windows). | yes |
| `PMIC_COMPATIBLES` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `PMIC_RESET_BIT` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `PMIC_RESET_REG` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `PmicBus` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `RegisterWindow` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). Refused `Window` (a register window, which the name did not say). | yes |
| `ResetBit` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `SDCARD_DIVIDER_MAX` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `SDIO_BRING_UP` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `STG` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `STGRST_SEC_AHB` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `STG_WINDOW` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). Kept as it stands. | yes |
| `SYS` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `SYSCLK_APB_BUS_FUNC` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `SYSCLK_AXI_CFG0` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `SYSCLK_BUS_ROOT` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `SYSCLK_SDIO_SDCARD` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `SYSCLK_STG_AXIAHB` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `SYS_SYSCON_BASE` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `SYS_SYSCON_SIZE` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `SYS_WINDOW` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). Kept as it stands. | yes |
| `StandardMode` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `Step` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `TRNG_BRING_UP` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `WindowLocation` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). Refused `Found` (a word that could name almost anything; it is where a window was located). | yes |
| `WindowSpelling` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). Refused `Spelling` (a word that could name almost anything; it is one tree's spelling of a window). | yes |
| `axi_cfg0_hz` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `clock_divider` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `clock_offset` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `every_reset_released` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `fallback` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `from_nodes` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). Refused `from_bus` (`PmicBus::from_bus` repeated its type; it reads the PMIC's node and its bus's node). | yes |
| `from_regions` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `from_specifiers` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `has_clocks_running` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `i2c5_ic_clk` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). Kept as it stands. | yes |
| `is_clock_enabled` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `is_deasserted` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `one_based_divider` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). Refused `one_based_div` (`div` abbreviated divider). | yes |
| `plan` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `reset_bit` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `sdcard_divider_for` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `standard_mode_100k` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `was_already_up` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `with_divider` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `with_parent` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `without_pmic` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
