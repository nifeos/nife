---
status: NOT-STARTED
raised: 2026-09-03
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 359. The proof-scope predicate runs the whole suite for changes that cannot reach a harness

Read against `script/verify` on 2026-09-19, the two open cases are both still open: the attribution loop still ends
`needed.append((f, "not attributable to a crate; runs by default"))` with no classification for a
binary file. The header still lists `Cargo.lock` among the paths it cannot attribute, with the
comment beside `cargo metadata` explaining that registry packages have no file a diff can name. The
third blind spot stays fixed: `script/` and `scripts/` are both recognized, commented in place.

The predicate is one Python block inside `script/verify`, it has no dependency on
any other milestone, and a change to it is exercised by the wiring it lives in.

**In brief.** `script/verify --affected-since <base>` decides whether a pull request needs the Kani
suite by attributing every changed file to a crate and asking whether that crate is in a harness
crate's dependency closure. Anything it cannot attribute runs the proofs, which is the correct
default and is also where the false positives come from. Milestone 119 named three of them. One,
`scripts/` being spelled differently from `script/`, was fixed on 2026-08-26 and the fix is
commented in place. Two remain: any `Cargo.lock` touch proves everything, and a binary file falls
through to `not attributable to a crate; runs by default`.

## Why this matters

The prover is the merge queue's long pole. A group build goes green while `verify` is still
running, every time. Every needless full run is real waiting on the critical path. The
comment in `script/verify` already prices one instance of it: a one-line comment added to
`script/citations` cost a 40-minute suite on a red trunk. Milestone 119 measured that these blind
spots are nearly all of the remaining tail, and priced fixing them **above** adding more shards,
because `glob`'s proofs are atomic at 15.0 minutes and are half the suite's time on their own. More
runners cannot divide one harness; a narrower scope predicate can skip it.

The two open cases are not equal. `Cargo.lock` is the harder one and the current behavior is
defensible: a lockfile bump can move a registry dependency inside a harness crate's closure. Those
packages live outside the repository, so no diff names a file that attributes to them. Making
this precise means reading which package versions the diff actually moved and asking whether any of
them is in the closure, rather than treating the file as opaque. A binary file is the easy one: a
`.png` under `user/` or an image fixture is not an input to `cargo kani` under any reading, and it
falls through only because the attribution step has nothing to say about it.

## What it would take

Two edits in the `--affected-since` block, each with a test of its own. The binary case can be a
classification beside the documentation one. The `Cargo.lock` case needs `cargo metadata` on both
sides of the diff, or a parse of the lockfile's own package list. It should keep failing toward
running when anything about the comparison is unavailable, which is the posture the surrounding
code already takes.

## Index row

`script/verify --affected-since <base>` decides whether a pull request needs the Kani suite by
attributing every changed file to a crate in a harness crate's dependency closure. What it cannot
attribute runs the proofs. That is the right default, and it is the source of the false positives. Two of
milestone 119's three blind spots remain: any `Cargo.lock` touch proves everything, and a binary
file falls through to "not attributable to a crate". The prover is the merge queue's long pole. Milestone 119 priced fixing these above adding shards,
because `glob`'s proofs are atomic at 15.0 minutes. The
two cases differ: the binary one is a classification beside the documentation one, while
`Cargo.lock` means reading which package versions the diff moved, and it should keep failing toward
running when the comparison is unavailable.
