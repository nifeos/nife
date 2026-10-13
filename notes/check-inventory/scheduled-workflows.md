# The check inventory: the scheduled workflows on 2026-09-03

An appendix to [`notes/check-inventory.md`](../check-inventory.md). Nothing blocks a scheduled
workflow by construction. `script/cadence-check` (milestone 238 (two scheduled checks have never
once succeeded)) now derives the last-result column, so read that column as the 2026-09-03 snapshot.
The table is kept because it also says what each workflow is for, which no API can.

| workflow | cadence | last result | what it is for |
|---|---|---|---|
| `mutation testing` | 05:00 UTC Monday | failure through 08-31, never once succeeded; repaired in part by milestone 238, and the runaway-mutant memory kill bounded by milestone 277 (bound what one mutant may allocate) | milestone 85 (mutation testing over the host crates), the instrument behind fatal risk 3 |
| `undefined-behavior check` | 06:00 UTC Monday | failure 08-10, 08-17, 08-24; cancelled 08-31; repaired by milestone 238 | milestone 79 (Miri over the host crates) |
| `toolchain drift` | 07:00 UTC daily | failure 09-02, 09-03 | builds against the newest nightly; red is its signal, not a defect on `main` |
| `audit cadence` | weekly | failure 08-17, 08-24, 08-31 | red means an audit is due; two are |
| `falsification sweep` | 06:00 UTC Monday | never run | added 2026-08-31; first fire 2026-09-07 |
| `project metrics` | 06:00 UTC Monday | never run | added 2026-09-02 by milestone 234 (the project's own numbers, one row per ISO week); first fire 2026-09-07 |
| `stranger cadence` | weekly | success | milestone 117 (the stranger test) |
| `vendor watch` | weekly | success | has upstream moved |
| `toolchain bump` | weekly | success | proposes a pin bump |
