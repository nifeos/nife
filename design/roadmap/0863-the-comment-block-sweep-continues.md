---
status: BUILT
raised: 2026-10-09
built: 2026-10-09
promoted_from: the-comment-block-sweep-continues
milestone_dependencies: 862
decision_dependencies: 267
machine_requirements: none
specific_machine: none
needs_person: no
---
# 863. The comment-block sweep continues, worth three

*(The title and slug are provisional.)*

§267 (a comment states the constraint as it is now) caps a comment block at 40 lines. It follows
milestone 862 (the comment-block sweep's next worth).

This worth's rows: `crates/system_initializer/src/lib.rs:6` (301, held by
its `no_run` doctest, which is a gate and stays), `components/src/timetable.rs:1` (201), then
`crates/glob/src/lib.rs:1` (190), `crates/board_console/src/lib.rs:1` (186), and
`crates/non_volatile_memory_express/src/lib.rs:2` and `crates/component_plan/src/lib.rs:1` (185
each). A worth takes the worst handful that is not a gated example. Each block's history goes to
a commit message, its findings to a note, its naming record to `design/naming/`, and every shrink
is banked. The method, the refused alternatives and the evidence are milestone 860's block and
`notes/comment-cost-2026-10-07.md`.

Reuse: the ratchet, the lexer and the baseline already exist (milestone 860 built them). This
worth only runs the sweep and banks, and takes no new code.

## The worth, 2026-10-09 (UTC): built

Swept and banked: `components/src/timetable.rs:1` 201 to 191 (the slots list stopped restating
`timetable::contract`'s table), `crates/glob/src/lib.rs:1` 190 to 153, `crates/board_console/`
186 to 176, `crates/non_volatile_memory_express/` 186 to 92, and `crates/component_plan/` 185 to
159. Baseline rows held at 336.

Mid-worth, calef amended the method in session (2026-10-09 UTC, recorded in §267): the tree does
not need the full history in document form, and the refusal records the earlier worths
transplanted into `design/naming/` are removed, since git holds them. This worth deleted
`rmle-name-search.md`, `jh7110-entropy-name.md`, `job-mix-name.md` and `component-plan-name.md`,
the NVMe section of `spelled-out-rulings.md`, the `system_initializer` section of `crates.md` and
the `board_console` section of `programs-scripts-and-directories.md`, and pointed each crate's
Name block at git history instead. The accounts are in the commits that wrote them. What stays in
the tree: the rulings with their dates, the rules (the console-marker rule is milestone 297 (`soak` becomes `soak-test`)'s block), the
standing measurements, and the constraints.

## Follow-on

- **Milestone 864.** Milestone 864 (the comment-block sweep continues again, worth four),
  under the amended method: constraint stays, history goes to the commit message and git, no
  transplant documents. Next worst: `jh7110_clock_and_reset` (184), `current_cpu_protocol`
  (183), `login` (178), `jh7110_entropy` (176) and `board_console` (176).

## Index row

The comment-block sweep's third worth: timetable's remainder, glob, board_console, the NVMe crate and component_plan, each shrunk to the constraint as it is now and banked. Mid-worth, calef ruled the tree keeps no history in document form, so the transplanted refusal records went to git.
