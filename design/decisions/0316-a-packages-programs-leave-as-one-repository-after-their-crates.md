---
status: DECIDED
raised: 2026-10-28
decided: 2026-11-04
ratified_by: calef
---

# 316. A program package leaves as one repository, with the crates only it uses

A program package leaves this repository as one repository under `nifeos`, named after the
package. That repository holds the package's programs and every crate that only this package uses,
in a Cargo workspace beside the programs (ruled 2026-11-04 UTC). The member lists in
`packages/<name>.package.toml` remain the install list, and the package's `home` names that
repository.

A crate gets a repository of its own on one of two grounds, and on no other:

- It would be published to crates.io on its own merits (milestone 857 (crates released on their
  own, and proofs offered to the projects that already exist)).
- More than one repository consumes it. The contract crates of milestone 1017 (the contract crates
  leave in this order) are the case: the kernel and every package link them.

Any other crate stays in its package's repository. `number_writer`, which only `process-tools`
links, leaves inside `nifeos/process-tools`. A crate that later meets one of the two grounds is
promoted to a repository of its own, and that promotion is a split like any other.

The order follows from the rule. A crate with a repository of its own leaves before every package
that links it, bottom-up, and the package repository depends on it by version. A crate only one
package uses leaves with that package, in the same move. §151 (the goal of the repository split is
independent release and third-party programs) states that order for the whole split.

## A package and a repository

The package is the unit of install: what releases together, and it may hold several programs. A
repository is the unit of source, with one purpose. A program package's own source, its programs
and the crates only it uses, is one purpose and one repository. The crates it shares with other
repositories, and the crates published on their own, are each another purpose, so the package
draws on several repositories at install. §235 (the OS is built and updated from packages, and its
source divides by purpose) says how big a repository is; this section says where a program
package's crates go.

The first cut applies the rule to three packages. `process-tools` leaves as
`nifeos/process-tools`, `core-tools` as `nifeos/core-tools`, and `disk-tools` as
`nifeos/disk-tools` (milestone 756 (process-tools, core-tools and disk-tools are the first code to
leave this repository)). A package's `home` line keeps `status = "provisional"` until its
repository exists; this section rules the destination and does not create one.

## What it refuses

- Grouping several packages into one repository, whether by #1389's seven divisions or by any other
  line. That alternative is what milestone 611 (every program and crate belongs to a package, and
  every package has a home)'s question 3 carried as open, and
  [notes/package-boundaries.md](../../notes/package-boundaries.md) records the answer where the
  question sits. The ruling refuses one repository holding all three first-cut packages, and it
  refuses the divisions with it. A division comes from the dependency graph, not from a purpose, so
  it does not size a repository.
- A repository for every crate a program links. A crate that no stranger would take on its own and
  that one package alone consumes gains nothing from a repository, a template and a release line of
  its own, and it costs a second pull request for every change that spans it and its programs.

## The names

Ruled 2026-10-28 (UTC), on the naming review this section held open
([milestone 978 (the first cut's names)](../roadmap/0978-first-cut-names.md), relayed by
the maintainer on pull request #2870). `process-tools`, `core-tools` and `disk-tools` stand as the
package names and as the repository names of the first cut. The repositories do not exist yet.
Ratifying a name and ratifying a home stay two events, per the rule above. Milestone 756
(process-tools, core-tools and disk-tools are the first code to leave this repository)'s
precondition 6 asks for the home and the repository name ratified. The ruling covers the names;
the gap that remains is the homes, which wait on the repositories.

§151 (the goal of the repository split is independent release and third-party programs) decides
why the split happens and its order; this section decides which repository a program package's
crates leave in.
