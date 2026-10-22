---
status: BUILT
raised: 2026-10-10
built: 2026-10-10
promoted_from: the-comment-block-sweep-continues-a-third-time
milestone_dependencies: 864
decision_dependencies: 267
machine_requirements: none
specific_machine: none
needs_person: no
---
# 865. The comment-block sweep continues a third time, worth five

Raised 2026-10-10 (UTC) by the lane that built milestone 864 (the comment-block sweep continues
again, worth four); minted from the proposal of the same name. §267 (a comment states the constraint as it is now) caps a comment block at
40 lines. Its 2026-10-09 amendment rules the method: the constraint stays, history goes to the
commit message and git, and no narrative is transplanted into a document.

The baseline holds 336 rows. The worst after worth four:
`components/src/timetable.rs:1` (191), `components/src/login.rs:1` (177),
`crates/current_cpu_contract/src/lib.rs:1` (176), `components/src/system_installer.rs:1` (175)
and `crates/serial_console/src/lib.rs:1` (173). A worth
takes the worst handful that is not a gated example, and every shrink is banked. The method, the
refused alternatives and the evidence are milestone 860 (comments state the constraint as it
is now)'s block and `notes/comment-cost-2026-10-07.md`.

Reuse: the ratchet, the lexer and the baseline already exist (milestone 860 built them). This
worth only runs the sweep and banks, and takes no new code.

## The worth, 2026-10-10 (UTC): built, and the sweep has converged

- `components/src/timetable.rs:1` 191 to 190. The Name paragraph's milestone-129 anecdote went
  to git. The BUGS consumer list kept all four consumers in fewer words.
- `components/src/login.rs:1` held at 177, floor. The milestone 246 (measured boot's refusal path
  is tested by nothing) sentence is now a plain citation; no line was saved. All that remains is
  constraint with dated rulings.
- `crates/current_cpu_contract/src/lib.rs:1` held at 176, floor. Worth four already took its
  history. What remains is §204 (how userspace asks where a thread runs)'s argument.
- `components/src/system_installer.rs:1` held at 175, floor, untouched by earlier worths. The
  two-slot ruling, the GPT attribute-bit ownership and the RedoxFS origin scan are constraint.
- `crates/serial_console/src/lib.rs:1` held at 173, floor. Worth four took the narration.

The finding this worth adds: the worst rows are now constraint floors. The one remaining gated
example, `crates/system_initializer/src/lib.rs:6` (300), is a `no_run` doctest a gate holds open.
Shrinkage from here is wording. Wording is not what §267 (a comment states the constraint as it
is now) asked the sweep to remove.

## Follow-on

- **Milestone 866.** Milestone 866 (the comment-block sweep closes, worth six). It took the next
  three (`crates/login_contract/src/lib.rs:2` 169, `crates/jh7110_entropy/src/lib.rs:2` 169,
  `kernel/src/arch/x86_64/iommu.rs:1` 164), found floors, and milestone 866 closed the sweep. The ratchet and
  the baseline remain as the standing tripwire against new over-cap blocks; the rows that stay
  over cap are the recorded cost of the constraints they carry.

## Index row

Worth five of the comment-block sweep: the worst five over-cap blocks shrunk to their constraints and banked, history to commit messages and git, no transplant documents.
