# Milestone 607 and section 235 state the end state: the references

[The proposal](../milestone-607-and-section-235-state-the-end-state.md) holds the replacement text
and the ask. This appendix lists what the text touches outside §235 and milestone 607 (nife updated from packages), and the other
statements in the tree that equate a package with a repository.

## Inbound references a title change touches

`script/citations` accepts a gloss when every content word appears in the cited title, or when the
gloss appears as a phrase in the body. With both new titles applied to the scratch copy, the
glosses below stop matching: they quote "the tree divides by what releases together" or "the
repository divides along what releases together". A lane retargets each to the new title, "and its
source divides by purpose", once the titles land. Those in `design/decisions/` change through
`script/record-ruling` with the rest.

38 citations in 35 files. Six of them cite milestone 607, and the rest cite §235. One sits in §297
(software reaches a nife system as a package `jig` installs from basalt's index), at line 64.
Another sits in milestone 909 (basalt composes the bootable image from its packages), and two in
milestone 976 (an OS update through a slot). The last two are the proposal
`what-an-os-update-writes-into-a-slot.md` and `packages/base-image.approval.toml`.

- `crates/hot_swap_server/Cargo.toml`: line 13
- `crates/hot_swap_server/src/lib.rs`: line 8
- `crates/process_builder/Cargo.toml`: line 24
- `crates/process_builder/src/lib.rs`: line 12
- `crates/supervision_contract/src/lib.rs`: line 11
- `crates/system_initializer/Cargo.toml`: line 64
- `design/decisions/0239-package-kinds-and-toml-declarations.md`: line 54
- `design/decisions/0297-software-reaches-nife-as-a-package-jig-installs-from-basalt.md`: line 64, 79
- `design/decisions/0316-one-repository-per-package-leaving-the-tree.md`: line 14
- `design/roadmap/0610-the-interfaces-stability-is-measured-weekly.md`: line 70
- `design/roadmap/0814-the-nvme-queue-crate-released-on-its-own.md`: line 95
- `design/roadmap/0815-the-e1000e-crate-released-on-its-own.md`: line 86
- `design/roadmap/0816-the-designware-ethernet-crate-released-on-its-own.md`: line 90
- `design/roadmap/0817-the-designware-mobile-storage-crate-released-on-its-own.md`: line 99
- `design/roadmap/0818-the-jh7110-entropy-crate-released-on-its-own.md`: line 86
- `design/roadmap/0819-the-jh7110-clock-and-reset-crate-proven-and-released-on-its-own.md`: line 101
- `design/roadmap/0820-the-xhci-crate-released-on-its-own.md`: line 89
- `design/roadmap/0821-the-portable-executable-crate-proven-and-released-on-its-own.md`: line 92
- `design/roadmap/0857-crates-released-on-their-own/what-was-already-on-the-record.md`: line 22
- `design/roadmap/0909-basalt-composes-the-bootable-image-from-its-packages.md`: line 65
- `design/roadmap/0956-the-linked-interfaces-carry-checkable-versions.md`: line 55
- `design/roadmap/0976-an-os-update-through-a-slot.md`: line 14, 231, 234
- `design/roadmap/0979-version-carrying-interfaces.md`: line 50
- `design/roadmap/proposals/message-contracts-carry-bind-time-version-checks.md`: line 23
- `design/roadmap/proposals/the-contract-crates-leave-in-this-order.md`: line 142
- `design/roadmap/proposals/what-an-os-update-writes-into-a-slot.md`: line 39
- `helpers/interface_stability.py`: line 56
- `helpers/name_checks.py`: line 86
- `notes/doc-coverage.md`: line 72
- `notes/interface-stability.md`: line 103
- `notes/packages-and-divisions.md`: line 3
- `notes/the-first-split.md`: line 8
- `packages/base-image.approval.toml`: line 16
- `packages/homes.toml`: line 14
- `script/metrics`: line 1869

The 9 links to §235's file name, in 8 files, need no change unless someone renames the file:

- `design/decisions/0235-packages-build-the-os-and-the-tree-divides-by-release.md` line 210, its own
  appendix link.
- `design/decisions/0235-packages-build-the-os-and-the-tree-divides-by-release/README.md` line 3,
  and `seven-questions.md` line 3 in the same appendix directory, which a rename moves too.
- `design/roadmap/0607-nife-is-built-and-updated-from-packages.md` line 23.
- `design/roadmap/0976-an-os-update-through-a-slot.md` line 29.
- `notes/bugs-without-milestones/docs-briefs-and-naming.md` line 10.
- `notes/packages-and-divisions.md` line 4.
- `design/passive-baseline.tsv` lines 276 and 277, which key the passive ratchet by path.

## Other statements that a package equals a repository

This lane edits none of these. Each wants its own change once the text above lands.

- §316 (one repository per package leaves the tree), lines 13 to 16, the strongest: "A program
  package that leaves this repository leaves as one repository under `nifeos`, named after the
  package. The package is the unit that releases together (§235 (the tree divides by what releases
  together)), so the package is the unit that moves. One repository holds that package's crates and
  its programs". Under §151 (the goal of the repository split) the crates a program links leave before it, each to a repository of its
  own (milestone 857's 2026-11-01 ruling), so the package repository cannot hold them. Line 30 says
  the same: "The package, not the division, is what releases together." The replacement in item 3
  leaves §316's three first-cut repositories possible, where each package's source has one purpose,
  but §316's premise and its crates clause want their own rewrite.
- `notes/package-boundaries.md` lines 207 to 209: "Answered, ruled 2026-10-28 (UTC) with the rule
  that a program package leaves as one repository per package, named after the package (§316 (one
  repository per package leaves the tree))."
- Milestone 756 (process-tools, core-tools and disk-tools are the first code to leave this
  repository), lines 35 to 36: "Ruled 2026-10-28: the home's grain, one repository per package
  (§316 (one repository per package leaves the tree))". Lines 90 to 92: "And §235 (the OS is built
  and updated from packages)'s own amendment says no package moves before 610's per-package
  co-change column exists."
- `packages/process-tools.package.toml` lines 13 to 14, `core-tools` lines 13 to 14, `disk-tools`
  lines 15 to 16: "one repository per package (§316 (one repository per package leaves the
  tree))". `packages/nife-libc.package.toml` line 13 cites §316 for its provisional `home` only.
- `packages/homes.toml` lines 15 to 19: "A home is sized by what changes together, measured by a
  per-package co-change column milestone 610 (the interface's stability is measured weekly) owes and
  does not yet carry. No package moves to a new home, and no `.package.toml` file's `home = { status
  = "provisional" }` settles, before that measurement exists."
- `notes/interface-stability.md` lines 103 to 104: "sizes a home by what changes together, and no
  package moves before this measurement exists."
- `notes/the-first-split.md` line 40, a table row: "No package moves before a per-package
  co-change column exists | §235 Fork 3 amendment | no | Milestone 610 is BUILT and its own
  Follow-on records the column as owed and not built." Milestone 610's Follow-on now records the
  column as done.
- `design/roadmap/0857-crates-released-on-their-own/what-was-already-on-the-record.md` lines 23 to
  25: "Under it, no package moves to a new repository before milestone 610 (the interface's
  stability is measured weekly) carries its per-package co-change column. Fork 1 has to respect
  that."
- Milestone 857 (crates released on their own), lines 40 to 42: "milestone 610 (the interface's
  stability is measured weekly) still decides which crates are ready (§235 (the OS is built and
  updated from packages))". Under item 3 the column decides no move.
- Milestone 854 (the init package holds only what init does), line 229: "611's question 3 (one
  repository per package, or one per division) is unanswered". §316 answered it on 2026-10-28.
- `design/roadmap/proposals/the-contract-crates-leave-in-this-order/questions.md` lines 48 to 49:
  "The split's rule for a program package is one repository per package (§316 (one repository per
  package leaves the tree))."

Two near misses read true and need nothing: the contract-crate proposal's "A repository per
package or per layer is refused" (line 93), and `notes/interfaces-note.md` line 15, "Once a
package leaves this repository", which reads as the package's source leaving.
