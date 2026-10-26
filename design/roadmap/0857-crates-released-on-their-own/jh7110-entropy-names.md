# The names in `jh7110_entropy`, for ratification

An appendix of [milestone 857 (crates released on their own)](../0857-crates-released-on-their-own.md), for the crate of milestone 818 (proven JH7110 TRNG logic, released on its own). A release-track crate's names take their ratification before it leaves this tree (ruled 2026-10-25 UTC). This file lists every name a stranger meets in it, in the record format `helpers/name_release.py` reads.

This file ratifies nothing. Until the line below carries a date, the crate's own `Name:` blocks stay the record, and the status column copies them as of 2026-10-26 (UTC). `python3 helpers/name_release.py --draft jh7110_entropy` prints a fresh copy. At ratification each row's status takes the ruling and any refusal its block holds, and the blocks leave the crate.

Ratified: not yet.

59 names: 54 no block, 4 provisional, 1 ratified. A status of `unrecorded` means the crate holds no block for the name. The column `outside` says whether a caller outside the crate can reach the name; 0 sit in private modules without a re-export. Enum variants, fields and trait methods appear only where a block names one, as `script/names` counts them.

The last column reads each name against design/naming.md's tests and 857's Fork 5, which asks for names that stand on their own because the crate may outlive nife. 6 rows say something other than `clear` or `term`. This file renames nothing.

- `clear`: a stranger reads it right.
- `term`: the specification or databook owns it, and it keeps that spelling, as design/naming.md asks of a name across somebody else's interface.
- `nife`: it names nife's own process plan (address-space windows, the handoff a server receives). A stranger has no such plan, and the crate may shed the name before it leaves.
- `scope`: it belongs to something other than the crate's subject.
- `generic`: a word that could name almost anything, design/naming.md's second failure mode.
- `unclear`: what a stranger would trip on, said in the cell.

| name | kind | path | status | outside | a stranger reads it |
|---|---|---|---|---|---|
| `jh7110_entropy` | crate | `src/lib.rs` | ratified 2026-09-13. | yes | clear |
| `AUTO_AGE` | item | `src/lib.rs` | unrecorded. | yes | term |
| `AUTO_RQSTS` | item | `src/lib.rs` | unrecorded. | yes | term |
| `BUILD_CONFIG` | item | `src/lib.rs` | unrecorded. | yes | term |
| `COMPATIBLE` | item | `src/lib.rs` | provisional. | yes | unclear: which tree's string; the sibling crates spell mainline's `COMPATIBLE_MAINLINE` |
| `COMPATIBLES` | item | `src/lib.rs` | provisional. | yes | clear |
| `COMPATIBLE_VENDOR` | item | `src/lib.rs` | unrecorded. | yes | clear |
| `CTRL` | item | `src/lib.rs` | unrecorded. | yes | term |
| `CTRL_EXEC_NONCE_RESEED` | item | `src/lib.rs` | unrecorded. | yes | term |
| `CTRL_EXEC_NOP` | item | `src/lib.rs` | unrecorded. | yes | term |
| `CTRL_EXEC_RANDRESEED` | item | `src/lib.rs` | unrecorded. | yes | term |
| `CTRL_GENE_RANDNUM` | item | `src/lib.rs` | unrecorded. | yes | term |
| `Discovered` | item | `src/lib.rs` | unrecorded. | yes | clear |
| `FEATURES` | item | `src/lib.rs` | unrecorded. | yes | term |
| `IE` | item | `src/lib.rs` | unrecorded. | yes | term |
| `IE_AGE_ALARM_EN` | item | `src/lib.rs` | unrecorded. | yes | term |
| `IE_GLBL_EN` | item | `src/lib.rs` | unrecorded. | yes | term |
| `IE_LFSR_LOCKUP_EN` | item | `src/lib.rs` | unrecorded. | yes | term |
| `IE_RAND_RDY_EN` | item | `src/lib.rs` | unrecorded. | yes | term |
| `IE_RQST_ALARM_EN` | item | `src/lib.rs` | unrecorded. | yes | term |
| `IE_SEED_DONE_EN` | item | `src/lib.rs` | unrecorded. | yes | term |
| `ISTAT` | item | `src/lib.rs` | unrecorded. | yes | term |
| `ISTAT_ALL` | item | `src/lib.rs` | unrecorded. | yes | term |
| `ISTAT_LFSR_LOCKUP` | item | `src/lib.rs` | unrecorded. | yes | term |
| `ISTAT_RAND_RDY` | item | `src/lib.rs` | unrecorded. | yes | term |
| `ISTAT_SEED_DONE` | item | `src/lib.rs` | unrecorded. | yes | term |
| `MODE` | item | `src/lib.rs` | unrecorded. | yes | term |
| `MODE_R256` | item | `src/lib.rs` | unrecorded. | yes | term |
| `Outcome` | item | `src/lib.rs` | unrecorded. | yes | generic |
| `Pool` | item | `src/lib.rs` | unrecorded. | yes | clear |
| `RAND0` | item | `src/lib.rs` | unrecorded. | yes | term |
| `RAND1` | item | `src/lib.rs` | unrecorded. | yes | term |
| `RAND2` | item | `src/lib.rs` | unrecorded. | yes | term |
| `RAND3` | item | `src/lib.rs` | unrecorded. | yes | term |
| `RAND4` | item | `src/lib.rs` | unrecorded. | yes | term |
| `RAND5` | item | `src/lib.rs` | unrecorded. | yes | term |
| `RAND6` | item | `src/lib.rs` | unrecorded. | yes | term |
| `RAND7` | item | `src/lib.rs` | unrecorded. | yes | term |
| `SEED0` | item | `src/lib.rs` | unrecorded. | yes | term |
| `SEED7` | item | `src/lib.rs` | unrecorded. | yes | term |
| `SMODE` | item | `src/lib.rs` | unrecorded. | yes | term |
| `SMODE_MISSION_MODE` | item | `src/lib.rs` | unrecorded. | yes | term |
| `STAT` | item | `src/lib.rs` | unrecorded. | yes | term |
| `STAT_MISSION_MODE` | item | `src/lib.rs` | unrecorded. | yes | term |
| `STAT_NONCE_MODE` | item | `src/lib.rs` | unrecorded. | yes | term |
| `STAT_R256` | item | `src/lib.rs` | unrecorded. | yes | term |
| `STAT_RAND_GENERATING` | item | `src/lib.rs` | unrecorded. | yes | term |
| `STAT_RAND_SEEDING` | item | `src/lib.rs` | unrecorded. | yes | term |
| `STAT_SEEDED` | item | `src/lib.rs` | unrecorded. | yes | term |
| `STAT_SRVC_RQST` | item | `src/lib.rs` | unrecorded. | yes | term |
| `WORD_BYTES` | item | `src/lib.rs` | unrecorded. | yes | clear |
| `assemble` | item | `src/lib.rs` | unrecorded. | yes | generic |
| `interpret` | item | `src/lib.rs` | unrecorded. | yes | generic |
| `interrupt` | item | `src/lib.rs` | provisional. | yes | unclear: reads as raising an interrupt; it decodes the `interrupts` property |
| `new` | item | `src/lib.rs` | unrecorded. | yes | clear |
| `refill` | item | `src/lib.rs` | unrecorded. | yes | clear |
| `remaining` | item | `src/lib.rs` | unrecorded. | yes | clear |
| `status_okay` | item | `src/lib.rs` | provisional. | yes | unclear: answers yes or no without design/naming.md's `is_` shape |
| `take` | item | `src/lib.rs` | unrecorded. | yes | clear |

Every `pub` module and item in the crate's source has a row. `script/names` does not count these as names of their own, so they have none:

- the inline module `regs`, whose constants have rows;
- the fields of `Discovered`: `reg_base`, `reg_size`, `interrupt`, `compatible`, `status_okay`;
- the variants of `Outcome`: `Ready`, `Lockup`, `NotReady`, `Unseeded`.

`interrupt` and `status_okay` each name both a `Discovered` field and the function that decodes it. Their rows are the functions'.
