---
status: PROPOSED
raised: 2026-11-02
milestone_dependencies: none
decision_dependencies: 151, 235
machine_requirements: none
specific_machine: none
needs_person: yes
---
# Milestone 607 and section 235 state the end state

Raised 2026-11-02 (UTC) by the lane `lane/607-and-235-state-the-end-state`. It carries the exact
replacement text for the statements in milestone 607 (nife is built and updated from packages) and
§235 (the OS is built and updated from packages) that conflict with the end state §151 (the goal of
the repository split is independent release) now states. Each is rewritten in place, as the
architect directed on 2026-11-02 (UTC). A lane may not write `design/decisions/`. The architect
reads this text before anyone edits §235.

Reuse: the replacement text reuses the wording of §151's "The order: bottom-up" and of milestone
857 (crates released on their own)'s 2026-11-01 ruling bullet. The checks on it reuse
`helpers/prose_ratchet.py`, `helpers/history_ratchet.py` and `script/citations`. No code changes.

## Background

§235 and milestone 607 were written on 2026-09-27, when the repository split had a goal and no
order. They tie two things together: how packages build and update the OS, and where this
repository divides. Their titles say the repository divides along what releases together, which
is a package. Their Fork 3 sizes a future repository by what changes together and holds every move
until milestone 610 (the interface's stability is measured weekly) measures that per package.

§151 now states the end state. Everything leaves this repository for repositories with one purpose
each: the kernel, the programs, the tooling and the crates. `nifeos/nife` keeps only the history.
The order is bottom-up, it names no preconditions, and churn does not hold a move. The leaf pilot
`jh7110_entropy` already left, on 2026-10-31, before any per-package measurement gated it.

The architect ruled on 2026-11-02 (UTC) how packages and repositories relate under that end state.
The package stays the unit of install: what releases together, and it may hold several programs.
The repository is the unit of source: it has one purpose, it may be finer than a package, and one
package may be assembled from several repositories. So a package and a repository need not
coincide, and nothing in §235 or 607 may say the repository divides along packages.

A document here states what is in force now (§298 (a document states what is in force now)), and
git keeps what it said before. So each replacement below reads as the current rule, with no dated
note and no marker of change.

## The replacements, ready to paste

Each pair quotes the current text at `nifeos/nife@1d56e1e57`, then the text that takes its place.
Everything not shown stays as it is. Item numbers follow the brief this lane was given.

### 1. The precondition that is not met

File: `design/roadmap/0607-nife-is-built-and-updated-from-packages.md`

Current:

```markdown
- Contracts still change with their consumers: 76% of contract commits in 60 days touched another
  division. §151's precondition for ruling the split's order is not met.
```

Replacement:

```markdown
- Contracts still change with their consumers: 76% of contract commits in 60 days touched another
  division. That churn holds no move: §151 (the goal of the repository split is independent
  release) orders the split bottom-up, and a contract crate leaves carrying a version that each
  consumer checks.
```

File: `design/decisions/0235-packages-build-the-os-and-the-tree-divides-by-release.md`

Current:

```markdown
3. Contracts still move with their consumers. Of 215 commits touching a contract crate in 60 days,
   76% touched another division too. §151 names protocols that stop churning as a precondition of
   the split. It is not met.
```

Replacement:

```markdown
3. Contracts still move with their consumers. Of 215 commits touching a contract crate in 60 days,
   76% touched another division too. That churn holds no move under §151 (the goal of the
   repository split is independent release)'s bottom-up order: a contract crate leaves carrying a
   version that each consumer checks.
```

The same idea sits once more in §235, in "What is already decided", which says the order is open.

File: `design/decisions/0235-packages-build-the-os-and-the-tree-divides-by-release.md`

Current:

```markdown
- §151 (the goal of the repository split is independent release): the split's goal is independent release and third-party programs. The order is open.
```

Replacement:

```markdown
- §151 (the goal of the repository split is independent release): the split's goal is independent
  release and third-party programs. Everything leaves this repository for a repository with one
  purpose, bottom-up, and no precondition holds a move.
```

### 2. The end state is wider than every package

File: `design/roadmap/0607-nife-is-built-and-updated-from-packages.md`

Current:

```markdown
- The repositories: R1 now, with package boundaries drawn in-tree and enforced by lint. The end
  state is every package leaving this repository. Each package's definition carries a required
  `home` with no default. `milestone/611-package-boundaries` is building the boundaries.
```

Replacement:

```markdown
- The repositories: R1 for what has not left, with package boundaries drawn in-tree and enforced
  by lint. The end state is everything leaving this repository, bottom-up, for repositories with
  one purpose each: the kernel, the programs, the tooling and the crates. `nifeos/nife` keeps only
  the history (§151 (the goal of the repository split is independent release)). A package is the
  unit of install and a repository the unit of source, so one package may draw its source from
  several repositories. Each package's definition carries a required `home` with no default, and
  milestone 611 (every program and crate belongs to a package, and every package has a home) draws
  the boundaries.
```

§235 states the same end state in its Fork 3 ruling, its R4 row and its "What this unblocks".

File: `design/decisions/0235-packages-build-the-os-and-the-tree-divides-by-release.md`

Current:

```markdown
- Fork 3, the repositories: R1 now, and every package leaves this repository in the end. calef:
  *"R1 for now is right with package boundaries drawn and enforced inside it by lint check."* And:
```

Replacement:

```markdown
- Fork 3, the repositories: R1 for what has not left, and everything leaves this repository in the
  end, each piece to a repository with one purpose (§151 (the goal of the repository split is
  independent release)). calef: *"R1 for now is right with package boundaries drawn and enforced
  inside it by lint check."* And:
```

File: `design/decisions/0235-packages-build-the-os-and-the-tree-divides-by-release.md`

Current:

```markdown
| R4 (ruled, the end state). Every package leaves | Each package has a required `home`, no default | Forces a decision per package rather than a default of staying. |
```

Replacement:

```markdown
| R4 (ruled, the end state). Everything leaves | The kernel, the programs, the tooling and the crates each leave for a repository with one purpose, bottom-up; each package has a required `home`, no default | Forces a decision per piece rather than a default of staying. `nifeos/nife` keeps only the history. |
```

File: `design/decisions/0235-packages-build-the-os-and-the-tree-divides-by-release.md`

Current:

```markdown
Milestone 39 and §151's order now have an answer: R1 now, every package out in the end.
```

Replacement:

```markdown
Milestone 39 has its answer: R1 for what has not left, and everything out in the end, in the
bottom-up order of §151 (the goal of the repository split is independent release).
```

### 3. How big a repository is: the amendment, folded into the body

The dated amendment block becomes an undated subsection of Fork 3 that states the rule now in
force. The italic line under the title that announces the amendment comes out with it.

File: `design/decisions/0235-packages-build-the-os-and-the-tree-divides-by-release.md`

Current:

```markdown
*Amended 2026-09-27T15:06Z: Fork 3 gains a home-sizing rule, below.*

```

Replacement: none. The line comes out with the blank line after it.

File: `design/decisions/0235-packages-build-the-os-and-the-tree-divides-by-release.md`

Current:

```markdown
### Amendment, 2026-09-27T15:06Z: how big a home is (question 7c)

calef ruled, once R4 has packages leaving: **a home is sized by what changes together**, option C.
A (one repository per package) is refused: it maximizes two-PR commits wherever packages
co-change. B (one repository per division) is refused too: a division comes from the dependency
graph, not co-change ("The premise, checked" above measures 76% of contract commits crossing one).

Milestone 610 (the interface's stability is measured weekly) gains a per-package co-change column
beside the division figure it carries; packages that move together share a home. Measure first: no
package moves before that column exists. R1 is unchanged.

Every `.package.toml` file's provisional `home` field stays provisional. `packages/homes.toml`'s
header cites this. The column is owed work against 610's own block, not built here.
```

Replacement:

```markdown
### How big a repository is

**A repository has one purpose**, and its purpose sets its size. A package is the unit of install:
it is what releases together, and it may hold several programs. A repository is the unit of
source: it may be finer than a package, and one package may be assembled from several
single-purpose repositories. One repository per package is refused as the rule: it ties the
grain of the source to the grain of the install. A repository and a package coincide only where the
package's source has one purpose. One repository per division is refused too: a
division comes from the dependency graph, not from a purpose ("The premise, checked" above
measures 76% of contract commits crossing one).

Milestone 610 (the interface's stability is measured weekly) carries a per-package co-change
column beside its division figure. The column informs how packages group: what changes together
is a candidate to release together. It gates no move, since each piece leaves in the bottom-up
order of §151 (the goal of the repository split is independent release).

Every `.package.toml` file's provisional `home` field stays provisional. `packages/homes.toml`'s
header cites this.
```

### 4. The titles and the composition bullet

The package definition stays word for word in both composition bullets: "a package is what
releases together and may hold several programs". Neither bullet says the repository divides along
packages, so neither changes. The titles do say it, and so does 607's index row.

File: `design/decisions/0235-packages-build-the-os-and-the-tree-divides-by-release.md`

Current:

```markdown
# 235. The OS is built and updated from packages, and the tree divides by what releases together
```

Replacement:

```markdown
# 235. The OS is built and updated from packages, and its source divides by purpose
```

File: `design/roadmap/0607-nife-is-built-and-updated-from-packages.md`

Current:

```markdown
# 607. nife is built and updated from packages, and the repository divides along what releases together
```

Replacement:

```markdown
# 607. nife is built and updated from packages, and its source divides by purpose
```

File: `design/roadmap/0607-nife-is-built-and-updated-from-packages.md`

Current:

```markdown
how a package becomes part of the OS, and where the repository divides
```

Replacement:

```markdown
how a package becomes part of the OS, and how its source divides into repositories
```

The file names stay. Neither slug says the repository divides along packages in a way a reader
meets as a rule: `0607-nife-is-built-and-updated-from-packages.md` is still true, and
`0235-packages-build-the-os-and-the-tree-divides-by-release.md` reads stale but is a path, not a
statement. A rename of §235's file changes 9 links in 8 files (listed under "Inbound references"
below) and gains a reader nothing the title does not already give. The architect may rename it;
this lane does not.

### 5. R1, the interim state

R1's name ("One repository, boundaries enforced inside it") and its cost ("Nothing moves between
repositories yet") no longer read true. `jh7110_entropy` left for `nifeos/jh7110_entropy` on
2026-10-31, `basalt` holds the distribution, and this repository depends on the crate by `rev`.
What R1 still rules is the interim state of everything that has not left.

File: `design/decisions/0235-packages-build-the-os-and-the-tree-divides-by-release.md`

Current:

```markdown
| R1 (ruled, now). One repository, boundaries enforced inside it | Each package's boundary is drawn in-tree and checked by lint, in the spirit of milestone 39 (repository structure for a loosely-coupled OS)'s option B. `script/test` stays one command. | Nothing moves between repositories yet. |
```

Replacement:

```markdown
| R1 (ruled, until each piece leaves). What has not left stays in one repository, boundaries enforced inside it | Each package's boundary is drawn in-tree and checked by lint, in the spirit of milestone 39 (repository structure for a loosely-coupled OS)'s option B. `script/test` stays one command. | A piece leaves only in the bottom-up order, and this repository pins each piece that has left. |
```

### The frontmatter

§235 is already `status: AMENDED`, and it stays so. `raised`, `decided` and `ratified_by` stay.
Milestone 607 keeps `status: NOT-STARTED`; nothing above changes what is built.

## What stays unchanged

- Forks 1, 2 and 4, their tables, the proposals table and the follow-ons.
- Both composition bullets, which hold the package definition.
- §235's opening paragraph, which says what calef asked on 2026-09-26, and "The premise, checked"
  apart from fact 3.
- The second italic line under §235's title (a slot holds only the kernel, §159). It is a dated
  note, but not one this ruling replaces; "Found while writing this" lists it.

## Why each replacement reads as it does

- Item 1 keeps the measurement, the 76%, because it still says where the work is. It drops the
  claim that a precondition gates the order, since §151 names none.
- Item 2 widens "every package" to everything, in §151's own words, and adds the install and source
  units the 2026-11-02 ruling names. It drops 607's present-tense claim that a lane is building the
  boundaries: milestone 611 is BUILT.
- Item 3 keeps both refusals, one repository per package and one per division, and drops the
  option letters and the question number, which no reader can follow outside the session that
  asked them. "A home is sized by what changes together" gives way to one purpose per repository.
  The sentence on where a repository and a package coincide keeps §316 (one repository per package
  leaves the tree)'s first cut possible without restating it; §316 itself conflicts elsewhere, as
  the list below shows. The co-change column informs grouping and gates nothing. Milestone 610
  records the column as done, so the sentence calling it owed work comes out.
- Item 4 replaces "divides along what releases together" with "divides by purpose", the rule the
  ruling names. "Its source" and not "the tree", since the tree is what empties.
- Item 5 keeps R1 as the interim rule, which it still is, and drops the two claims the move of
  `jh7110_entropy` made false.

I applied every pair to a scratch copy of the tree, and each current text matched exactly once.
`helpers/history_ratchet.py --list` finds nothing in either changed file, and
`helpers/prose_ratchet.py --report` passes both: §235 at 2,924 words with 16 passives against 18
today, and 607 at 1,064 words with 3 passives, as today. Neither goes over a limit.

## Inbound references a title change touches

`script/citations` accepts a gloss when every content word appears in the cited title, or when the
gloss appears as a phrase in the body. With both new titles applied to the scratch copy, the
glosses below stop matching: they quote "the tree divides by what releases together" or "the
repository divides along what releases together". A lane retargets each to the new title, "and its
source divides by purpose", once the titles land. Those in `design/decisions/` change through
`script/record-ruling` with the rest.

38 citations in 35 files. Six of them cite milestone 607 (§297 line 64, milestone 909, milestone 976 lines 14 and 231, the proposal `what-an-os-update-writes-into-a-slot.md` and `packages/base-image.approval.toml`); the rest cite §235.

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

The 9 links to §235's file name, in 8 files, need no change unless the file is renamed:

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
  its programs". Under §151 the crates a program links leave before it, each to a repository of its
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

## Found while writing this

- §235 keeps a second dated italic line under its title: "*Amended 2026-10-07 (UTC): a slot holds
  only the kernel and what nothing can restart; see §159 (only a new kernel needs a reboot).*" This
  ruling does not replace it, so this proposal leaves it. It wants folding into Fork 1 the same way.
- Milestone 611 is BUILT, yet §235 line 45 says "The lane `milestone/611-package-boundaries`
  (provisional number 611) is building the boundaries", its P7 row says "decided; in the lane
  `milestone/611-package-boundaries`", and milestone 607's item 7 says the same. Record fixes.
- A package's `home` field names one repository. Once a package draws its source from several
  repositories, one field per package cannot say where all of it lives, and the home moves to each
  path or crate. Milestone 611's format wants a look; this proposal does not change it.
- `script/citations` exits 0 in the scratch copy even with the 38 failing glosses listed; the full
  `script/lint` is what fails on them. A lane landing the titles runs the full lint.

## What I need from you

§235 and milestone 607 still say the repository divides along what releases together, that §151's
order waits on a precondition, and that no package moves before a co-change measurement exists. §151
now says none of that: everything leaves, bottom-up, for repositories with one purpose each, and
churn holds no move. Your ruling of 2026-11-02 (UTC) set the two units: the package is the unit of
install and the repository the unit of source, finer where the purpose is finer.

The text above rewrites those statements in place, in 13 pairs: the precondition sentences, the
end state, the folded amendment on how big a repository is, the two titles and 607's index row,
and R1's row. Nothing in `design/decisions/` changes until you approve it.

Recommended: A. Approve the 13 replacements as written, with both titles changed and both file
names kept. A lane then pastes them under `script/record-ruling`, and retargets the 38 glosses the
titles touch in the same pull request.

- A. Approve as written.
- B. Approve the body text, but keep both titles. The 38 glosses then need no change, and the
  titles go on saying the repository divides along what releases together.
- C. Approve with changes. Name them, and the lane makes them before pasting.
- D. Refuse. §235 and 607 keep their text and go on contradicting §151.

What is unverified: that each package in §316's first cut has source with one purpose, which item
3's coincidence sentence relies on to leave §316 standing for now. §316 itself is the next decision:
its premise cites §235's old title, and its crates clause conflicts with the bottom-up order. I
bring it after this one.

The exact approval needed: "A" (or B, C with the changes, or D). This blocks only the edits to §235
and 607. No move, crate or milestone waits on it.
