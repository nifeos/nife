---
status: NOT-STARTED
promoted_from: records-that-equate-a-package-with-a-repository
raised: 2026-11-02
milestone_dependencies: none
decision_dependencies: 151, 235, 316
machine_requirements: none
specific_machine: none
needs_person: no
---
# 1028. A package's `home` can name the repositories its member crates leave to

Raised 2026-11-02 (UTC) by the lane `lane/607-and-235-are-rewritten-to-the-end-state`. §316 (a
program package leaves as one repository, with the crates only it uses) states where a package's
crates go (ruled 2026-11-04 UTC). A program package leaves as one repository named after the
package, holding its programs and every crate only that package uses. A crate gets a repository of
its own only when it would publish on its own merits or more than one repository consumes it. §151
(the goal of the repository split is independent release), §235 (the OS is built and updated from
packages, and its source divides by purpose), milestones 756, 857, 854 and 1017, `packages/` and
the notes state the same rule.

Reuse: milestone 611 (every program and crate belongs to a package)'s format in
`helpers/packages.py`, and the comment `packages/drivers.package.toml` carries for
`jh7110_entropy`.

## What is open

A package's `home` field names one repository. That holds for a program package under §316, whose
members all leave in its one repository. It does not hold for a package whose member crates each
get a repository of their own. `packages/contracts.package.toml` sets `home = { repo =
"contracts" }`, and milestone 1017 (the contract crates leave in this order) gives each of its 17
crates a repository named after the crate. `packages/drivers.package.toml` records in a comment,
not a field, that `jh7110_entropy` lives in `nifeos/jh7110_entropy`.

The format wants one of two shapes, and a lane chooses it under milestone 611's provisional key
names: a `home` on each member crate that leaves alone, or a package whose `home` names a repository
per crate by rule. `helpers/packages.py --check` should then refuse a crate whose repository the
records disagree on.

## Index row

A package's `home` names one repository, which fits a program package under section 316 but not a package whose crates each leave to a repository of their own, such as `contracts`. The format wants a per-crate home.
