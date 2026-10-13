---
status: BUILT
raised: 2026-10-09
built: 2026-10-09
promoted_from: the-comment-block-sweeps-next-worth
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 862. The comment-block sweep's next worth

*(The title and slug are provisional.)*

§267 (a comment states the constraint as it is now) caps a comment block at 40 lines.
`helpers/comment_block_ratchet.py` holds the cap against `design/comment-block-baseline.tsv` (both
names provisional). The sweep works through the baseline's rows per file, worst first, one
milestone's worth at a time, after milestone 860 (comments state the constraint as it is now).

This worth's rows: `kernel/src/testing.rs:240` (468 lines),
`crates/system_initializer/src/lib.rs:6` (406), `components/src/timetable.rs:1` (224),
`crates/jh7110_entropy/src/lib.rs:2` (209), then `kernel/src/arch/x86_64/iommu.rs:1` and
`crates/job_mix/src/lib.rs:1` (197 each). A worth takes the worst handful. Each block's history
moves to a commit message, its findings to a note, its semantics to a `§N` citation. The shrinks
are banked (`python3 helpers/comment_block_ratchet.py --bank`). The method, the refused
alternatives and the evidence are milestone 860's block and `notes/comment-cost-2026-10-07.md`.
This worth claims no new decision, only the next slice of a ruled sweep.

Reuse: the ratchet, the lexer and the baseline already exist (milestone 860 built them). This
worth only runs the sweep and banks, and takes no new code.

## The worth, 2026-10-09 (UTC): built

Worst first, each shrunk block banked as it fell:

- `kernel/src/testing.rs:240` (468 lines): the frame-budget ledger left for
  `notes/frame-budget.md`, its dated narrative compressed to a number-line table there and its
  stories left to the commits that wrote them. The doc comment is 35 lines and its row is gone
  (337 rows to 336).
- `crates/system_initializer/src/lib.rs:6` (406): the mutation essay deleted to a pointer at
  milestone 244 (the largest crate in the tree is proved by nothing a mutation can reach)'s
  block, the naming saga moved to `design/naming/crates.md`, the history
  compressed to pointers. Banked at 301. The bulk that remains is the `no_run` doctest that
  type-checks the whole `BootEndowment` literal, which is a gate and stays. This row stays on the
  baseline at its banked ceiling.
- `components/src/timetable.rs:1` (224): the `--mem` design essay moved to
  `notes/scheduled-execution/mem-entries.md`. Banked at 201.
- `crates/jh7110_entropy/src/lib.rs:2` (209): the naming saga moved to
  `design/naming/jh7110-entropy-name.md`, the health-test lane meta folded into the standing
  fact. Banked at 171.
- `kernel/src/arch/x86_64/iommu.rs:1` (197): the FIXED detective stories compressed to their
  still-true facts, the §86 (whether an NVMe driver can leave the kernel) restatement replaced by a citation of
  that section's own amendment. Banked at
  164.
- `crates/job_mix/src/lib.rs:1` (197): the naming saga and the console-marker rule moved to
  `design/naming/job-mix-name.md`. Banked at 141.

## Follow-on

- **Milestone 863.** Milestone 863 (the comment-block sweep continues, worth three):
  with the new worst-first list. The baseline still holds 336 rows.

## Index row

The comment-block sweep's next worth: the 337 over-cap blocks the baseline holds, swept a milestone's worth at a time, worst first. Every shrink is banked, so the baseline only falls. History moves to commit messages, findings to notes, rulings to sections.
