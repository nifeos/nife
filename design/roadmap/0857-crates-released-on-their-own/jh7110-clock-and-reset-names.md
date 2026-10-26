# The names in `jh7110_clock_and_reset`, for ratification

An appendix of [milestone 857 (crates released on their own)](../0857-crates-released-on-their-own.md), for the crate of milestone 819 (JH7110 clock and reset logic, proven, then released on its own). A release-track crate's names take their ratification before it leaves this tree (ruled 2026-10-25 UTC). This file lists every name a stranger meets in it, in the record format `helpers/name_release.py` reads.

This file ratifies nothing. Until the line below carries a date, the crate's own `Name:` blocks stay the record, and the status column copies them as of 2026-10-25 (UTC). `python3 helpers/name_release.py --draft jh7110_clock_and_reset` prints a fresh copy. At ratification each row's status takes the ruling and any refusal its block holds, and the blocks leave the crate.

Ratified: not yet.

53 names: 41 no block, 11 provisional, 1 ratified. A status of `unrecorded` means the crate holds no block for the name. The column `outside` says whether a caller outside the crate can reach the name; 1 sit in private modules without a re-export. Enum variants, fields and trait methods appear only where a block names one, as `script/names` counts them.

The last column reads each name against design/naming.md's tests and 857's Fork 5, which asks for names that stand on their own because the crate may outlive nife. 4 rows say something other than `clear` or `term`. This file renames nothing.

- `clear`: a stranger reads it right.
- `term`: the specification or databook owns it, and it keeps that spelling, as design/naming.md asks of a name across somebody else's interface.
- `nife`: it names nife's own process plan (address-space windows, the handoff a server receives). A stranger has no such plan, and the crate may shed the name before it leaves.
- `scope`: it belongs to something other than the crate's subject.
- `generic`: a word that could name almost anything, design/naming.md's second failure mode.
- `unclear`: what a stranger would trip on, said in the cell.

| name | kind | path | status | outside | a stranger reads it |
|---|---|---|---|---|---|
| `jh7110_clock_and_reset` | crate | `src/lib.rs` | ratified 2026-09-13. | yes | clear |
| `tests` | module | `src/tests.rs` | unrecorded. | no | clear; private |
| `AON` | item | `src/lib.rs` | unrecorded. | yes | clear |
| `CLOCK_ENABLE` | item | `src/lib.rs` | unrecorded. | yes | clear |
| `DevicePlan` | item | `src/lib.rs` | provisional. | yes | clear |
| `Domain` | item | `src/lib.rs` | unrecorded. | yes | clear |
| `Found` | item | `src/lib.rs` | unrecorded. | yes | generic |
| `GMAC0_AON_BRING_UP` | item | `src/lib.rs` | unrecorded. | yes | clear: the steps that bring one device up |
| `GMAC0_SYS_BRING_UP` | item | `src/lib.rs` | unrecorded. | yes | clear: the steps that bring one device up |
| `IcClkWords` | item | `src/lib.rs` | unrecorded. | yes | unclear: three abbreviations; the I2C controller's clock words |
| `MAX_DEVICE_STEPS` | item | `src/lib.rs` | provisional. | yes | clear |
| `MAX_RECORDED_CLOCKS` | item | `src/lib.rs` | unrecorded. | yes | clear |
| `PMIC_RESET_BIT` | item | `src/lib.rs` | unrecorded. | yes | clear |
| `PMIC_RESET_REG` | item | `src/lib.rs` | unrecorded. | yes | clear |
| `PmicBus` | item | `src/lib.rs` | unrecorded. | yes | term |
| `Report` | item | `src/lib.rs` | unrecorded. | yes | generic |
| `ResetBit` | item | `src/lib.rs` | unrecorded. | yes | clear |
| `SDCARD_DIVIDER_MAX` | item | `src/lib.rs` | provisional. | yes | clear |
| `SDIO_BRING_UP` | item | `src/lib.rs` | unrecorded. | yes | clear: the steps that bring one device up |
| `STG` | item | `src/lib.rs` | unrecorded. | yes | clear |
| `STGRST_SEC_AHB` | item | `src/lib.rs` | unrecorded. | yes | clear |
| `SYS` | item | `src/lib.rs` | unrecorded. | yes | clear |
| `SYSCLK_APB_BUS_FUNC` | item | `src/lib.rs` | unrecorded. | yes | clear |
| `SYSCLK_AXI_CFG0` | item | `src/lib.rs` | unrecorded. | yes | clear |
| `SYSCLK_BUS_ROOT` | item | `src/lib.rs` | unrecorded. | yes | clear |
| `SYSCLK_SDIO_SDCARD` | item | `src/lib.rs` | provisional. | yes | clear |
| `SYSCLK_STG_AXIAHB` | item | `src/lib.rs` | unrecorded. | yes | clear |
| `SYS_SYSCON_BASE` | item | `src/lib.rs` | unrecorded. | yes | clear |
| `SYS_SYSCON_SIZE` | item | `src/lib.rs` | unrecorded. | yes | clear |
| `StandardMode` | item | `src/lib.rs` | unrecorded. | yes | clear: the I2C standard mode |
| `Step` | item | `src/lib.rs` | unrecorded. | yes | clear |
| `TRNG_BRING_UP` | item | `src/lib.rs` | unrecorded. | yes | clear: the steps that bring one device up |
| `axi_cfg0_hz` | item | `src/lib.rs` | provisional. | yes | clear |
| `clock_divider` | item | `src/lib.rs` | unrecorded. | yes | clear |
| `clock_offset` | item | `src/lib.rs` | unrecorded. | yes | clear |
| `device_plan` | item | `src/lib.rs` | provisional. | yes | clear |
| `discover` | item | `src/lib.rs` | unrecorded. | yes | clear |
| `discover_aon` | item | `src/lib.rs` | provisional. | yes | term |
| `discover_sys` | item | `src/lib.rs` | provisional. | yes | term |
| `every_reset_released` | item | `src/lib.rs` | unrecorded. | yes | clear |
| `has_clocks_running` | item | `src/lib.rs` | unrecorded. | yes | clear |
| `i2c5_ic_clk` | item | `src/lib.rs` | unrecorded. | yes | term |
| `is_clock_enabled` | item | `src/lib.rs` | unrecorded. | yes | clear |
| `is_deasserted` | item | `src/lib.rs` | unrecorded. | yes | clear |
| `one_based_div` | item | `src/lib.rs` | unrecorded. | yes | unclear: `div` abbreviates divider |
| `plan` | item | `src/lib.rs` | provisional. | yes | clear |
| `pmic_bus` | item | `src/lib.rs` | unrecorded. | yes | term |
| `reset_bit` | item | `src/lib.rs` | unrecorded. | yes | clear |
| `sdcard_divider_for` | item | `src/lib.rs` | provisional. | yes | clear |
| `standard_mode_100k` | item | `src/lib.rs` | unrecorded. | yes | clear |
| `was_already_up` | item | `src/lib.rs` | unrecorded. | yes | clear |
| `with_divider` | item | `src/lib.rs` | provisional. | yes | clear |
| `with_parent` | item | `src/lib.rs` | unrecorded. | yes | clear |
