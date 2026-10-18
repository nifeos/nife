# Parts 1 and 2, 2026-09-19: an appendix to milestone 326

An appendix to [milestone 326 (nobody has been assigned to turn a mutation score
upward)](../0326-turn-a-mutation-score-upward.md). It is the block's record of parts 1 and 2: the
per-crate table, what moved the numbers, what the part did not find, and the one behavior question
it recorded.

## Parts 1 and 2, 2026-09-19

169 survivors triaged, and none of the eight crates carries an untriaged one. Measured per crate
with `script/mutation -p <crate>` on the lane's own worktree, before and after. Every kill was
verified by re-running the sweep and watching the mutant die; every equivalence claim is a mutant
the second run still reports. The reasons are in
`notes/mutation-testing/regressions-2026-09-14-capability-to-device-tree-blob.md` (its `## 2026-09-19` section) and
`notes/mutation-testing/regressions-2026-09-14-clock-contract-swish-filesystem-contract.md`, crate by crate,
which is where a reader should go to disagree with one.

| crate | before | after | killed by a test | equivalent | excluded | recorded gap |
|---|---|---|---|---|---|---|
| `capability` | 8 | 3 | 5 | 3 | 0 | 0 |
| `memory_regions` | 8 | 0 | 6 | 0 | 2 | 0 |
| `elf` | 6 | 0 | 6 | 0 | 0 | 0 |
| `timetable` | 48 | 0 | 12 | 0 | 36 | 0 |
| `dtb` | 14 | 1 | 13 | 1 | 0 | 0 |
| `clock_contract` | 6 | 3 | 0 | 3 | 3 | 0 |
| `swish` | 20 | 4 | 16 | 4 | 0 | 0 |
| `filesystem_contract` | 59 | 40 | 19 | 38 | 0 | 2 |
| total | 169 | 51 | 77 | 49 | 41 | 2 |

`after` is what a re-run reports now: the equivalents plus the two recorded gaps. Excluded mutants are no longer generated, so they leave the column.

What moved the numbers, in order of how much a reader should care.

1. Forty-one of the 169 were never the crate's code. `timetable` keeps its Kani harnesses in
   `src/proofs.rs` rather than in an inline `mod proofs`. Cargo-mutants names a mutant by its
   path *within the parsed file*, so `.cargo/mutants.toml`'s `proofs::` regex matched nothing: 36 of
   that crate's 48 survivors were its own proofs, which is why it read 73.6%. The five `mod
   interleavings` loom models were unexcluded for the same reason one level over. Both are closed
   with globs, and both are the `system_initializer` (milestone 244) and `uefi_loader` (milestone
   280) arithmetic wearing new clothes.
2. Seventy-seven were real, and four of them are places a probe reports nothing.
   `capability::survey_includes`, the predicate deciding which threads a supervision rendezvous may
   see, had no `cargo test` caller at all: a constant `true` and a constant `false` both survived.
   `filesystem_contract::fixture::twodir` had no distinctness test, so five witness bits could each
   become zero while their boot passed. `memory_regions::has_children` is what the kernel asks before
   it offers to destroy, and every test proved the refusal through a different door.
3. Forty-nine are equivalent and that is a result, not a shortfall. They are argued by group in
   the note so a later pass can re-check a group rather than a mutant, and the two crates whose
   numbers barely moved (`clock_contract`, `filesystem_contract`) are the two where that is the
   honest answer.

And the one thing this part was told to look for, which it did not find. The block ordered
`timetable` second because it holds `next_after`, the property `design/fatal-risks/README.md`'s risk 2 calls
its strongest counterfactual. No survivor touched `next_after`, or the phase arithmetic, or the
firing decision; every mutant in them was caught before this lane changed anything. That is evidence
against risk 2 in the place this roadmap thought it most likely, and it is worth as much as a finding
would have been.

One behavior question is recorded rather than answered, in a `BUGS` section on
`timetable::Unbacked` where a reader meets the variants. `Admission::Unbacked` can never carry
`File` or `Directory`. That is because `admit` hands `grant_plan::plan` the same `dir` bit that `unbacked`
then re-tests, so a designation the scheduler cannot back is refused during planning instead. Two
mutants deleting the `!` in those tests survived on exactly that. Changing which of two true
sentences a reader meets is a behavior change, so it is written down and not made.
