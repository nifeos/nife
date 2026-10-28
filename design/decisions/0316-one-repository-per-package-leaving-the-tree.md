---
status: DECIDED
raised: 2026-10-28
decided: 2026-10-28
ratified_by: calef
---

# 316. One repository per package leaves the tree

*Section number provisional, and the name a lane's coinage; the merge queue mints the number at
merge. Ruled 2026-10-28 (UTC) in the maintainer's session, and relayed by the maintainer.*

A program package that leaves this repository leaves as one repository under `nifeos`, named after
the package. The package is the unit that releases together (§235 (the tree divides by what
releases together)), so the package is the unit that moves. One repository holds that package's
crates and its programs, and the member lists in `packages/<name>.package.toml` are the list of
what moves. The first cut applies the rule to three packages: `process-tools` leaves as
`nifeos/process-tools`, `core-tools` as `nifeos/core-tools`, and `disk-tools` as
`nifeos/disk-tools` (milestone 756 (process-tools, core-tools and disk-tools are the first code to
leave this repository)). A package's `home` line keeps `status = "provisional"` until its
repository exists; this section rules the destination and does not create one.

## What it refuses

Grouping several packages into one repository, whether by #1389's seven divisions or by any other
line. That alternative is what milestone 611 (every program and crate belongs to a package, and
every package has a home)'s question 3 carried as open, and
[notes/package-boundaries.md](../../notes/package-boundaries.md) records the answer where the
question sits. One repository holding all three first-cut packages came up; the ruling refuses it,
and refuses the divisions with it. The package, not the division, is what releases together.

## Beside the released-crate grain

milestone 857 (crates released on their own, and proofs offered to the projects that already
exist)'s Fork 1 rules a different grain for a different leaver. Its rule is one repository per
crate for released library crates, created at that crate's first publication. The two rulings share
one test and do not compete. A library crate that strangers consume on its own leaves alone. A
program package releases as one unit, its programs and the crates behind them, so it leaves at
package grain, one repository holding that package's crates and programs. Which grain governs
follows from what the piece is, not from its size.

## What it does not settle

The names. This section fixes the shape, three repositories at one repository per package, and not
the strings. `process-tools`, `core-tools` and `disk-tools` stay provisional until the naming
review ratifies them, and the review brief is
[design/roadmap/proposals/first-cut-names.md](../roadmap/proposals/first-cut-names.md). §151 (the
goal of the repository split is independent release) decides the test a piece passes to earn
separation; this section decides the grain a leaving package moves at. Neither names anything.
