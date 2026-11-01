---
status: DECIDED
raised: 2026-10-04
decided: 2026-10-04
ratified_by: calef
---

# 247. The split begins with basalt holding nife, and `procps` moves first

*Section number and slug provisional; the maintainer mints the number at merge. Written 2026-10-04
(UTC) by the lane `lane/split-order-proposal`. The options and their costs are in milestone 756
(`procps`, `coreutils` and `util-linux` are the first code to leave this repository); the
measurements are in [notes/the-first-split.md](../../notes/the-first-split.md).*

§151 (the goal of the repository split is independent release) ruled where the split ends and said
its order "should be ruled on its own". This is that ruling. calef answered both questions on
2026-10-04 (UTC), relayed by the maintainer on pull request #1609.

## The rulings

1. **basalt v0 is the first step.** basalt pins `nife` whole at a commit and runs its image build
   and system test on all three architectures; nothing leaves this repository. The ruling includes
   the go-ahead for a lane's first write to `nifeos/basalt`. The manifest's file name and format
   ship provisional. Milestone 755 (basalt v0 pins nife and runs its gate) builds it.
2. **The first code cut is `procps` + `coreutils` + `util-linux`, and only once all six
   conditions hold**:
   - milestone 610 (the interface's stability is measured weekly) carries the per-package co-change
     column §235 (the OS is built and updated from packages) requires before any move;
   - `components` is split so those programs are in crates of their own (milestone 691 (packages
     move out of this repository, one per pull request));
   - the cut builds without a clone of nife, through §235's S2 archive or a dated, recorded
     exception;
   - a crate may leave only when all of these hold (amended 2026-11-01, below):
     1. it is classified under milestone 956 (the linked interfaces carry checkable versions): it
        carries an `INTERFACE_ID` and a version, or it is recorded as not a format;
     2. it has a tagged release whose commit carries the Cargo version, and
        `cargo-semver-checks --baseline-rev` against that tag passes;
     3. nife consumes it as a git dependency pinned by revision with the version requirement
        stated, the form ruled for crates not yet published;
     4. a producer release can move each consumer's pin, by the mechanism of the proposal
        `a-producer-release-moves-each-consumers-pin.md` or an agreed manual step until it exists;
     5. the architect reads the crate's code by hand and reviews it before it leaves;
   - basalt takes a component's output as a package by digest (§235's P1, milestone 607 (nife is
     built and updated from packages) item 1);
   - the home and its repository name are ratified, and §201 (one roadmap until a citation has to
     cross)'s citation identity is ruled, since this cut is its revisit event.

## Why, in one paragraph

Of the nine preconditions the records name, only the package format held on 2026-10-04: 157
breaking changes to the contract crates in four weeks, and 56 programs from 18 packages in one
crate. basalt v0 needs none of them and builds the whole-system gate that §151's BUGS calls the real
cost of a split, before any change has to land in two repositories. Among code cuts, `procps` and
its neighbors had the fewest focused cross-package commits (5 of 11) and link the fewest moving
interfaces. `redoxfs` was cheaper to move but more coupled, and preferring it would have been an
effort argument.

## What is not decided here

- The repository name and home for the first cut. Names are calef's under the naming rule.
- The citation identity across repositories, which §201 deferred to the first code cut.
- Whether milestone 691's order puts the SDK last among the leaves, as milestone 756 recommends.


## Amended 2026-10-06: the cut's packages are renamed

calef renamed the three packages of ruling 2 on 2026-10-06 (UTC): `procps` is `process-tools`,
`coreutils` is `core-tools` and `util-linux` is `disk-tools`, and `uuid` moved from the last to
the second. The cut holds the same fifteen programs, so the six conditions stand unchanged. The
names above are the ones ruled on 2026-10-04 and stay as written. Milestone 756 (`process-tools`,
`core-tools` and `disk-tools` are the first code to leave this repository) carries the new ones.
The reasons are in each package's manifest and in
[notes/package-boundaries.md](../../notes/package-boundaries.md).

## Amended 2026-11-01: condition 4 is a mechanism gate and a review

calef replaced condition 4 on 2026-11-01 (UTC), relayed on pull request #3033. The four-week
stability measure is gone and no time-based measure remains: a crate whose interface is still
breaking may leave, and each break is a new version and a new tagged release. The review in item 5
applies to every crate at first, the first crate in particular. Feedback from those reviews is
generalized into automated gates over time as patterns appear; the ruling does not say when, or
whether, the manual review ends. The other five conditions and the destination stand. The crates
now move bottom-up (milestone 756), so the four named in the original wording are among those that
move and are no longer a precondition of the cut.

## BUGS

- The coupling figures behind the ruling are a one-off reading at one commit over thirty days.
  Milestone 610's owed column replaces them, and condition 1 makes that column a gate.
