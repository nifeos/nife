---
status: BUILT
raised: 2026-10-10
built: 2026-10-10
promoted_from: the-comment-block-sweep-closes
milestone_dependencies: 865
decision_dependencies: 267
machine_requirements: none
specific_machine: none
needs_person: no
---
# 866. The comment-block sweep closes, worth six

Raised 2026-10-10 (UTC) by the lane that built milestone 865 (the comment-block sweep continues a
third time, worth five), which found the worst rows to be constraint floors; minted from the
proposal of the same name. §267 (a comment
states the constraint as it is now) caps a comment block at 40 lines, with history in the commit
message and git and no narrative transplanted into documents.

This worth takes the next three rows: `crates/login_contract/src/lib.rs` (module header) (169),
`crates/jh7110_entropy/src/lib.rs` (module header) (169), `kernel/src/arch/x86_64/iommu.rs` (module header) (164). It expects
floors. Three worths before it took the history. What remains should be constraint.

If the worth confirms that, it closes the sweep. The ratchet and the baseline stay as the
tripwire against new over-cap blocks. The rows that stay over cap are the recorded cost of the
constraints they carry, led by the gated `no_run` doctest in `crates/system_initializer/src/lib.rs` (module header).

Reuse: the ratchet, the lexer and the baseline already exist (milestone 860 (comments state the constraint as it is now) built them). This
worth only runs the sweep, banks and closes, and takes no new code.

## The worth, 2026-10-10 (UTC): built, and the sweep is closed

- `crates/login_contract/src/lib.rs` (module header) 169 to 164. The Name block keeps its three rulings with
  their dates (ratified 2026-08-23, suffix at milestone 265 (`_proto` is a truncation), stem
  stays 2026-09-15). The
  parked-family anecdote between them went to git.
- `crates/jh7110_entropy/src/lib.rs` (module header) held at 169, floor. Worths two and four took the fetch
  mechanics and the refused names. What remains is grounding (three drivers, one sequence) and
  measurements.
- `kernel/src/arch/x86_64/iommu.rs` (module header) held at 164, floor. The FIXED entries are dated rulings the
  kernel's tests still lean on; they stay.

The sweep is closed. The ratchet and the baseline remain as the standing tripwire against new
over-cap blocks. The rows that stay over cap are the recorded cost of the constraints they carry,
led by the gated `no_run` doctest in `crates/system_initializer/src/lib.rs` (module header). Six worths ran
(860, 862, 863, 864, 865, 866); the worst block in the tree fell from 689 lines to 177, and every
shrink is banked in `design/comment-block-baseline.tsv`.

## Follow-on

- **Decision.** `design/decisions/0267-a-comment-states-the-constraint-as-it-is-now.md` is what
  stands after the close: the cap, the ratchet, and the amendment that puts history in commit
  messages and git. The tripwire it arms is `helpers/comment_block_ratchet.py` over
  `design/comment-block-baseline.tsv`, run by every lint.

## Index row

Worth six, the closing worth of the comment-block sweep: the last three over-cap rows checked, expected floors, and the sweep closed with the ratchet left standing as the tripwire.
