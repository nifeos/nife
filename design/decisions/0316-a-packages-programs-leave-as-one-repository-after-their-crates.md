---
status: DECIDED
raised: 2026-10-28
decided: 2026-11-02
ratified_by: calef
---

# 316. A package's programs leave as one repository, after their crates

A program package that leaves has its programs leave as one repository under `nifeos`, named after
the package, when those programs have one purpose. The crates behind them have already left, each
as its own repository, bottom-up (§151 (the goal of the repository split is independent release and
third-party programs), and the grain of milestone 857 (crates released on their own, and proofs
offered to the projects that already exist) for released library crates). The programs depend on
those crates by version. The member lists in `packages/<name>.package.toml` remain the install
list, and each member's `home` names the repository it comes from.

The package is the unit of install: what releases together, and it may hold several programs. The
repository is the unit of source: it has one purpose, it may be finer than a package, and one
package may draw on several repositories (§235 (the OS is built and updated from packages, and the
tree divides by what releases together), and milestone 607 (nife is built and updated from
packages, and the repository divides along what releases together)). A repository and a package
coincide only where the package's programs have one purpose. This section rules the grain at which
a package's programs leave; it does not rule that a package and its source are one thing.

The first cut applies the rule to three packages: the programs of `process-tools` leave as
`nifeos/process-tools`, those of `core-tools` as `nifeos/core-tools`, and those of `disk-tools` as
`nifeos/disk-tools` (milestone 756 (process-tools, core-tools and disk-tools are the first code to
leave this repository)). A package's `home` line keeps `status = "provisional"` until its
repository exists; this section rules the destination and does not create one.

## What it refuses

Grouping several packages into one repository, whether by #1389's seven divisions or by any other
line. That alternative is what milestone 611 (every program and crate belongs to a package, and
every package has a home)'s question 3 carried as open, and
[notes/package-boundaries.md](../../notes/package-boundaries.md) records the answer where the
question sits. One repository holding all three first-cut packages came up and is refused, and the
divisions with it. A division comes from the dependency graph, not from a purpose, so it does not
size a repository.

## Beside the released-crate grain

milestone 857 (crates released on their own, and proofs offered to the projects that already
exist)'s Fork 1 rules a different grain for a different leaver. Its rule is one repository per
crate for released library crates, created at that crate's first publication. The two rulings share
one test and do not compete. A library crate that strangers consume on its own leaves alone, and it
leaves first. A package's programs then leave as one repository when they have one purpose, and
depend on those crates by version. Which grain governs follows from what the piece is, not from its
size.

## The names

Ruled 2026-10-28 (UTC), on the naming review this section held open
([milestone 978 (the first cut's names)](../roadmap/0978-first-cut-names.md), relayed by
the maintainer on pull request #2870). `process-tools`, `core-tools` and `disk-tools` stand as the
package names and as the repository names of the first cut. The repositories do not exist yet, so
ratifying a name and ratifying a home stay two events, per the rule above. milestone 756
(process-tools, core-tools and disk-tools are the first code to leave this repository)'s
precondition 6 asks for the home and the repository name ratified. The ruling covers the names;
the gap that remains is the homes, which wait on the repositories. §151 (the goal of the repository
split is independent release and third-party programs) decides the test a piece passes to earn
separation; this section decides the grain at which a package's programs leave.
