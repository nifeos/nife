---
status: PROPOSED
raised: 2026-11-02
milestone_dependencies: none
decision_dependencies: 151, 235, 316
machine_requirements: none
specific_machine: none
needs_person: yes
---
# Records that equate a package with a repository state the end state

Raised 2026-11-02 (UTC) by the lane `lane/607-and-235-are-rewritten-to-the-end-state`, while it
pasted the approved end-state text into milestone 607 (nife updated from packages) and §235 (the OS
updated from packages). §235 now says a package is the unit of install and a repository the unit of
source. A repository has one purpose and may be finer than a package. Milestone 610 (the
interface's stability, measured weekly)'s co-change column informs how packages group and gates no
move. §151 (the goal of the repository split) orders every move bottom-up.

Reuse: the wording of §235's "How big a repository is" and §151's "The order: bottom-up". No code.

## The records that still say otherwise

Each wants its own change. §316 is an architect's call; the rest follow from it or from §235 as it
stands, and a lane can fix them once the architect rules on §316.

- §316 (one repository per package leaves the tree), lines 13 to 16: "A program package that
  leaves this repository leaves as one repository under `nifeos`, named after the package. The
  package is the unit that releases together (§235 (its source divides by purpose)), so the
  package is the unit that moves. One repository holds that package's crates and its programs".
  Under §151 the crates a program links leave before it, each to a repository of its own
  (milestone 857 (crates released on their own)'s 2026-11-01 ruling), so the package repository
  cannot hold them. Line 30 says the same: "The package, not the division, is what releases
  together." §235 leaves §316's three first-cut repositories possible, where each package's source
  has one purpose, but §316's premise and its crates clause want a rewrite.
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
  Follow-on records the column as owed and not built." Milestone 610's Follow-on records the column
  as done.
- `design/roadmap/0857-crates-released-on-their-own/what-was-already-on-the-record.md` lines 23 to
  25: "Under it, no package moves to a new repository before milestone 610 (the interface's
  stability is measured weekly) carries its per-package co-change column. Fork 1 has to respect
  that."
- Milestone 857, lines 40 to 42: "milestone 610 (the interface's stability is measured weekly)
  still decides which crates are ready (§235 (the OS is built and updated from packages))". Under
  §235 the column decides no move.
- Milestone 854 (the init package holds only what init does), line 229: "611's question 3 (one
  repository per package, or one per division) is unanswered". §316 answered it on 2026-10-28.
- `design/roadmap/proposals/the-contract-crates-leave-in-this-order/questions.md` lines 48 to 49:
  "The split's rule for a program package is one repository per package (§316 (one repository per
  package leaves the tree))."

Two near misses read true and need nothing: the contract-crate proposal's "A repository per
package or per layer is refused" (line 93), and `notes/interfaces-note.md` line 15, "Once a
package leaves this repository", which reads as the package's source leaving.

## Also found

- §235 keeps a dated italic line under its title: "*Amended 2026-10-07 (UTC): a slot holds only the
  kernel and what nothing can restart; see §159 (only a new kernel needs a reboot).*" It wants
  folding into Fork 1, the way §235's Fork 3 amendment became "How big a repository is".
- Milestone 611 (every program and crate belongs to a package) reached BUILT. §235's Fork 3 ruling
  bullet still says "The lane `milestone/611-package-boundaries` (provisional number 611) is
  building the boundaries", its P7 row says "decided; in the lane
  `milestone/611-package-boundaries`", and milestone 607's item 7 says the same. Record fixes.
- A package's `home` field names one repository. Once a package draws its source from several
  repositories, one field per package cannot say where all of it lives. Milestone 611's format
  wants a look.

## What this needs

An architect's ruling on §316 first: whether a program package's repository holds only its
programs, or whether §316 gives way to §235's one purpose per repository. The other records follow
that ruling, and none blocks a move today.
