# The names in `jh7110_entropy`

An appendix of [milestone 857 (crates released on their own)](../0857-crates-released-on-their-own.md),
for the crate of milestone 818 (proven JH7110 TRNG logic, released on its own). This file is the
record of every name in the crate, which `helpers/name_release.py` reads in place of `Name:` blocks:
the crate's shipped files carry none.

Ratified: 2026-10-26 (UTC).

Ruled 2026-10-26 (UTC): the 53 names not listed below are ratified as they stand. Six are renamed
before the first publish: COMPATIBLE becomes COMPATIBLE_MAINLINE, interrupt becomes interrupt_line,
the status_okay function becomes is_status_okay (the Discovered.status_okay field keeps its name),
Outcome becomes GenerationOutcome, interpret becomes generation_outcome, and assemble becomes
output_bytes.

Each renamed row names the spelling the ruling turned down and why. A new name in the crate takes a
row here, `provisional` until a ruling, and `script/lint` fails a module or `pub` item without one.

The column `outside` says whether a caller outside the crate reaches the name. Some names have no
row because `script/names` does not count them: the inline module `regs`, whose constants have rows;
the fields of `Discovered` (`reg_base`, `reg_size`, `interrupt`, `compatible`, `status_okay`); and
the variants of `GenerationOutcome` (`Ready`, `Lockup`, `NotReady`, `Unseeded`). The same ruling
covers them.

| name | kind | path | status | outside |
|---|---|---|---|---|
| `jh7110_entropy` | crate | `src/lib.rs` | ratified 2026-09-13. Chip-qualified rather than generic, since `trng` alone could name almost anything; kept as the crates.io name (#1806). | yes |
| `AUTO_AGE` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `AUTO_RQSTS` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `BUILD_CONFIG` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `COMPATIBLES` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `COMPATIBLE_MAINLINE` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). Refused `COMPATIBLE` (it did not say which tree's string; the sibling crates spell mainline's `COMPATIBLE_MAINLINE`). | yes |
| `COMPATIBLE_VENDOR` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `CTRL` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `CTRL_EXEC_NONCE_RESEED` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `CTRL_EXEC_NOP` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `CTRL_EXEC_RANDRESEED` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `CTRL_GENE_RANDNUM` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `Discovered` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `FEATURES` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `GenerationOutcome` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). Refused `Outcome` (a word that could name almost anything; it is the outcome of one generation). | yes |
| `IE` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `IE_AGE_ALARM_EN` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `IE_GLBL_EN` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `IE_LFSR_LOCKUP_EN` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `IE_RAND_RDY_EN` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `IE_RQST_ALARM_EN` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `IE_SEED_DONE_EN` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `ISTAT` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `ISTAT_ALL` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `ISTAT_LFSR_LOCKUP` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `ISTAT_RAND_RDY` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `ISTAT_SEED_DONE` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `MODE` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `MODE_R256` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `Pool` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `RAND0` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `RAND1` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `RAND2` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `RAND3` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `RAND4` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `RAND5` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `RAND6` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `RAND7` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `SEED0` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `SEED7` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `SMODE` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `SMODE_MISSION_MODE` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `STAT` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `STAT_MISSION_MODE` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `STAT_NONCE_MODE` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `STAT_R256` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `STAT_RAND_GENERATING` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `STAT_RAND_SEEDING` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `STAT_SEEDED` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `STAT_SRVC_RQST` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `WORD_BYTES` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `generation_outcome` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). Refused `interpret` (a word that could name almost anything; it returns the generation's outcome). | yes |
| `interrupt_line` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). Refused `interrupt` (it read as raising an interrupt; it decodes the line a node names). | yes |
| `is_status_okay` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). Refused `status_okay` for the function (it answers yes or no, so it asks as a question). The `Discovered.status_okay` field keeps its name. | yes |
| `new` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `output_bytes` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). Refused `assemble` (it did not say what it assembles; it returns the output bytes). | yes |
| `refill` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `remaining` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
| `take` | item | `src/lib.rs` | ratified 2026-10-26 (UTC). | yes |
