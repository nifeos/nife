# Names in `jh7110_clock_and_reset`, kept beside its milestone

<!-- writing-standards: exception. Marked 2026-10-25 (UTC) by lane crates-stand-alone. Reason: each block below is a provenance record moved verbatim from the crate's source, refusals and wording included, and a record keeps its words. -->

Several blocks cite milestone 53 (the board's own peripherals: network and storage on real
silicon) and milestone 593 (a wedged kernel resets itself, through a watchdog that does not need
asking), whose lanes minted them.

An appendix of [milestone 819 (`jh7110_clock_and_reset`: JH7110 clock and reset logic, proven,
then released on its own)](../0819-the-jh7110-clock-and-reset-crate-proven-and-released-on-its-own.md).
The crate ships on its own and may cite nothing of this tree, so the `Name:` block of its crate
and its marked items lives here. `script/names` and the module-block ratchet read each paragraph
below as if it sat at the name (helpers/name_census.py, `_sidecars`).

`crates/jh7110_clock_and_reset/src/lib.rs` Name: ratified 2026-09-13 (calef), replacing `jh7110_crg`; kept as the crates.io name (#1806). The argument when the name `jh7110_crg` was minted (milestone 220's lane, 2026-09-04): `crg` is the JH7110's own word for the thing, not an abbreviation this project coined: both device trees that describe this chip spell the blocks `syscrg`, `stgcrg` and `aoncrg` (mainline's `starfive,jh7110-stgcrg` compatible, the vendor U-Boot's `reg-names = "syscrg", "stgcrg", ...`), so it is the AGENTS.md "standard term a reader already knows from outside" case rather than the `capsh`/`uheap` case. Chip-qualified for the reason `jh7110_entropy` is: a bare `crg` would name almost anything.

`crates/jh7110_clock_and_reset/src/lib.rs::MAX_DEVICE_STEPS` Name: provisional, minted 2026-10-20 (UTC) by milestone/593-radon-watchdog.

`crates/jh7110_clock_and_reset/src/lib.rs::DevicePlan` Name: provisional, minted 2026-10-20 (UTC) by milestone/593-radon-watchdog.

`crates/jh7110_clock_and_reset/src/lib.rs::plan` Name: provisional, minted 2026-10-20 (UTC) by milestone/593-radon-watchdog.

`crates/jh7110_clock_and_reset/src/lib.rs::axi_cfg0_hz` Name: provisional, minted 2026-10-21 (UTC) by milestone/53-radon-multi-block-read.

`crates/jh7110_clock_and_reset/src/lib.rs::SYSCLK_SDIO_SDCARD` Name: provisional, minted 2026-10-21 (UTC) by milestone/53-radon-multi-block-read.

`crates/jh7110_clock_and_reset/src/lib.rs::SDCARD_DIVIDER_MAX` Name: provisional, minted 2026-10-21 (UTC) by milestone/53-radon-multi-block-read.

`crates/jh7110_clock_and_reset/src/lib.rs::sdcard_divider_for` Name: provisional, minted 2026-10-21 (UTC) by milestone/53-radon-multi-block-read.

`crates/jh7110_clock_and_reset/src/lib.rs::with_divider` Name: provisional, minted 2026-10-21 (UTC) by milestone/53-radon-multi-block-read.

`crates/jh7110_clock_and_reset/src/lib.rs::from_specifiers` Name: provisional, minted 2026-10-25 (UTC) by lane crates-stand-alone, when the device-tree walk left the crate: a plan built from a node's clock and reset specifiers.

`crates/jh7110_clock_and_reset/src/lib.rs::from_bus` Name: provisional, minted 2026-10-25 (UTC) by lane crates-stand-alone, when the device-tree walk left the crate: the plan from what the PMIC's bus node says.

`crates/jh7110_clock_and_reset/src/lib.rs::without_pmic` Name: provisional, minted 2026-10-25 (UTC) by lane crates-stand-alone, for the case it answers.

`crates/jh7110_clock_and_reset/src/lib.rs::from_regions` Name: provisional, minted 2026-10-25 (UTC) by lane crates-stand-alone: a window picked out of a matched node's `reg` entries.

`crates/jh7110_clock_and_reset/src/lib.rs::fallback` Name: provisional, minted 2026-10-25 (UTC) by lane crates-stand-alone, after the word the crate's docs already used for the constant answer.

`crates/jh7110_clock_and_reset/src/lib.rs::Spelling` Name: provisional, minted 2026-10-25 (UTC) by lane crates-stand-alone, after the word the crate's docs already used for one tree's way of naming a controller.

`crates/jh7110_clock_and_reset/src/lib.rs::Window` Name: provisional, minted 2026-10-25 (UTC) by lane crates-stand-alone, after the register window it locates.

`crates/jh7110_clock_and_reset/src/lib.rs::STG_WINDOW` Name: provisional, minted 2026-10-25 (UTC) by lane crates-stand-alone, beside the domain constant `STG`.

`crates/jh7110_clock_and_reset/src/lib.rs::AON_WINDOW` Name: provisional, minted 2026-10-25 (UTC) by lane crates-stand-alone, beside the domain constant `AON`.

`crates/jh7110_clock_and_reset/src/lib.rs::SYS_WINDOW` Name: provisional, minted 2026-10-25 (UTC) by lane crates-stand-alone, beside the domain constant `SYS`.

`crates/jh7110_clock_and_reset/src/lib.rs::MAX_WINDOWS` Name: provisional, minted 2026-10-25 (UTC) by lane crates-stand-alone, after `MAX_DEVICE_STEPS`.

`crates/jh7110_clock_and_reset/src/lib.rs::PMIC_COMPATIBLES` Name: provisional, minted 2026-10-25 (UTC) by lane crates-stand-alone, after `jh7110_entropy::COMPATIBLES`: both spellings, in the order to try them.

`crates/jh7110_clock_and_reset/src/lib.rs::COMPATIBLE_STGCRG` Name: provisional, made public 2026-10-25 (UTC) by lane crates-stand-alone when the device-tree walk left the crate. The binding's own word for the property it holds, as for every `COMPATIBLE_` constant below.

`crates/jh7110_clock_and_reset/src/lib.rs::COMPATIBLE_AONCRG` Name: provisional, made public 2026-10-25 (UTC) by lane crates-stand-alone.

`crates/jh7110_clock_and_reset/src/lib.rs::COMPATIBLE_SYSCRG` Name: provisional, made public 2026-10-25 (UTC) by lane crates-stand-alone.

`crates/jh7110_clock_and_reset/src/lib.rs::COMPATIBLE_VENDOR_CLKGEN` Name: provisional, made public 2026-10-25 (UTC) by lane crates-stand-alone.

`crates/jh7110_clock_and_reset/src/lib.rs::COMPATIBLE_VENDOR_RSTGEN` Name: provisional, made public 2026-10-25 (UTC) by lane crates-stand-alone.

`crates/jh7110_clock_and_reset/src/lib.rs::COMPATIBLE_PMIC_VENDOR` Name: provisional, made public 2026-10-25 (UTC) by lane crates-stand-alone.

`crates/jh7110_clock_and_reset/src/lib.rs::COMPATIBLE_PMIC_MAINLINE` Name: provisional, made public 2026-10-25 (UTC) by lane crates-stand-alone.
