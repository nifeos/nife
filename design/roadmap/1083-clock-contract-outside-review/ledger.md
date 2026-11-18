# The ledger

An appendix to [the milestone](../1083-clock-contract-outside-review.md).

| item | state |
|---|---|
| 1 | partly done: every Decision 3 name is applied, `NANOSECONDS_PER_SECOND` and `propose::request` included; the British spellings are fixed and the README's two passives are rewritten; the badge row, the acronym expansions and the `accepted-words` entries are the move's |
| 2 | done: the crate carries a `clippy.toml` for `VisionFive` |
| 3 | left: the move's first mutation run |
| 4 | done: the `publish` comment names no file of this repository |
| 5 | done with Decision 1 |
| 6 | done in #3559 |
| 7, 8, 10, 11, 13, 14 | done: doc sentences that stand alone, a `compile_error!` on targets without 64-bit atomics, a `debug_assert!` on the opcode, the `virtual_address` doc, and the docs on `initialize` and `offset_for` |
| 9 | done: two Kani harnesses in `src/verification.rs`, each with a falsification patch that makes it fail |
| 12 | done with Decision 1 |
| 15 | done: the README says what the crate does not do, and names the 64-bit atomics under Building and testing |
