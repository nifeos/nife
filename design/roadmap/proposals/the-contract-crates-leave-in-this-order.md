---
status: PROPOSED
raised: 2026-11-01
milestone_dependencies: 755, 956
decision_dependencies: 151, 305
machine_requirements: none
specific_machine: none
needs_person: yes
---
# The contract crates leave in this order

Raised 2026-11-01 (UTC) by the lane `lane/contract-crates-move-order`. It answers two questions: in
what order the 22 crates that the first cut's programs link could leave this repository, and what
policy makes frequent breaking change safe across repositories. It moves no crate and changes no
code. Every new name below is provisional.

Reuse: the measurements reuse `cargo metadata`, git's rename detection and milestone 610's
per-crate series; the policy reuses milestone 956's constants, the crate template and basalt's gate.
No new code and no dependency.

Two directions from the architect, both dated 2026-11-01 (UTC), frame it:

- Crossing repositories is the goal of the split (§151 (the goal of the repository split is
  independent release)), not a cost to avoid, and the project should become good at changes that
  span two repositories. Contract crates should carry versions and may leave `nifeos/nife`, to ease
  that change and to let others write alternative implementations against them. Churn will continue
  for all of them, so waiting for a crate to settle is not a criterion.
- Anything that leaves must not reference back into nife. So a crate leaves bottom-up in the
  dependency graph, before or with every program that links it, and nothing that has left depends
  on a path or a pin into nife. nife may depend on what left.

The second direction inverts the order of 2026-10-04 in milestone 756 (`process-tools`,
`core-tools` and `disk-tools` are the first code to leave this repository) and milestone 691
(packages move out of this repository, one per pull request). Programs first and the SDK last
maximizes references back: each program repository would pin nife for the crates it links.

Churn here sizes the coordination cost per crate and picks the version policy. It never sets a
wait. Appendices: [measurements](the-contract-crates-leave-in-this-order/measurements.md), [prior
art](the-contract-crates-leave-in-this-order/prior-art.md) and [questions for the
architect](the-contract-crates-leave-in-this-order/questions.md).

## What the measurements say

Measured at `nifeos/nife@a8b7524d7` with one script, reproduced in the measurements appendix.

- Through normal dependencies the 22 link no other nife crate, so no 23rd crate has to leave with
  them. Fifteen are leaves, four sit one layer up and three sit two up.
- 39 crates that stay link at least one of the 22. `abi` alone has 12, the kernel among them.
- In 30 days 348 focused commits touched the 22. 244 of them (70%) also changed code outside the
  22, mostly `components` (80), `kernel` (72) and `system_tests` (51). Only 5 spanned two of the 22
  and stayed inside them. Grouping the 22 into one repository would save almost no crossings.
- `grant_plan` dominates: 151 commits in 30 days, 120 crossing, 227 breaking API items in four
  weeks. `filesystem_contract` follows with 263, item-level churn that milestone 610's note
  attributes to the fixture extraction and the file splits.
- Identity values (a `MAGIC`, a `VERSION`) changed five times in 90 days. Two reverts followed
  within a day, `current_cpu_contract` on 2026-10-21 and `program_argument_contract` on 2026-10-30, because
  basalt's published programs, built from an older nife, still compared the old value. The tree
  already pays cross-repository breaks, without a policy.
- The packages differ. `disk-tools` reaches 12 of the 22 and not `grant_plan`. `process-tools`
  reaches 13 and `core-tools` 15, both through `grant_plan`. Pull request #3022's `INTERFACES`
  macro adds `capability_manifest_note`, whose note type is `grant_plan::Manifest`, so with it each
  package reaches 16 to 18.

## What a crate references in nife that blocks its move

All 22 inherit `license` and `lints` from nife's workspace manifest; each move writes them as
literals, as `jh7110_entropy` did. `system_tests` is a dev-dependency of 17 of them; that edge
points from nife to the crate, which the direction permits. Beyond those, six references would block:

| crate | reference back | resolution |
|---|---|---|
| `address_space_map` | a test reads `../../user_mode_runtime/link.ld` | the test moves to `user_mode_runtime` |
| `byte_sink_contract` | dev-dependency on `abi` | leaves after `abi` |
| `counter_frequency_contract` | dev-dependency on `address_space_map` | leaves after it |
| `filesystem_contract` | dev-dependency on `filesystem_fixture`, which links it | the two leave together |
| `grant_plan` | dev-dependency on `primitive_benchmark_contract`, which stays | that test moves, or the crate leaves too |
| `capability_manifest_note` | a test reads `../../fuzz/seeds/`, and nife's fuzz workspace builds it | its seeds and fuzz target leave with it |

Six more of the 22 have fuzz targets in nife's `fuzz/` workspace. Those point from nife to the
crate, so they do not block, but a crate's fuzzing belongs with it, the way `jh7110_entropy` took
its proofs. Between 5 and 36 files in nife name each crate's path; that is each move's edit inside
nife.

## The recommended order

The architect reads each crate's code by hand before it leaves, at first every crate (condition 4
of §247 (the split begins with basalt holding nife, and `procps` moves first), ruled 2026-11-01
(UTC)). A crate also needs a tagged release whose commit carries its Cargo version, the baseline
for `cargo-semver-checks --baseline-rev`. The first crate reviewed is the pilot, `jh7110_entropy`.
It moved on 2026-10-31 (UTC) and has no tag, so milestone 818 tags it first. No time-based measure
remains: a crate whose interface still breaks may leave, each break a new tagged release.

The rule: a crate leaves when every crate it links (normal, build and dev) has left or leaves with
it, and no file in it names a nife path. Within a layer, the cheapest coordination goes first, so
the project learns the landing on crates where a mistake costs little. A package leaves when its
whole linked set has left.

| step | crates | why here |
|---|---|---|
| 0, the review pilot | `jh7110_entropy` | tag it, then the architect reads it; the others follow only as each review clears |
| 1, the contract pilot | `block_roster` | a leaf with no blocking reference; one writer in the kernel, one reader in `disk_surveyor`; its magic carries its version, so a reader refuses a mismatch today |
| 2, leaves for `disk-tools` | `entropy_contract`, `clock_contract`, `socket_contract`, `globally_unique_identifier_partition_table`, `counter_frequency_contract`, then `filesystem_contract` with `filesystem_fixture`, then `abi` | cheapest first by crossing commits and outside dependents; `abi` last, at 66 crossing commits and 12 dependents |
| 3 | `address_space_map`, then `current_cpu_contract`, then `user_mode_runtime` | after the `link.ld` test moves; `user_mode_runtime` links six crates of step 2 |
| 4, first package | `disk-tools` | its 12 crates are gone; milestone 756's fourth precondition has it carry the `INTERFACES` macro, so it also waits for step 6 unless the note's type first moves to a leaf crate (below) |
| 5, leaves for `process-tools` | `byte_sink_contract`, `glob`, `program_argument_contract`, `domain_name_system` | leaves; `program_argument_contract` after the pair test moves to basalt (the proposal the cross-repository pair is tested where the pin is) |
| 6 | `machine_statistics_contract`, `domain_name_resolution_contract`, `grant_plan`, then `capability_manifest_note` | `grant_plan` is the largest coordination cost of all 22 |
| 7, second package | `process-tools` | |
| 8 | `calendar`, `environment_contract`, then `core-tools` | the last two crates only `core-tools` reaches |

`jh7110_entropy` stays the leaf pilot (milestone 818 (`jh7110_entropy`: proven JH7110 TRNG logic,
released on its own)). It proves that a leaf crate builds and proves from its own repository and
that nife pins it by `rev` with nothing pointing back. Its 31-line manifest holds no path or pin
into nife; only its README and documents name `nifeos/nife`, as its first caller and as the home of
the device-tree fixtures. It does not prove the contract case: no consumer outside nife, so no two
pins must agree. And no version check has refused a real mismatch across the boundary. The
`block_roster` pilot proves all three once `disk-tools` leaves.

Two findings want lanes of their own, proposed as follow-on work:

- The `INTERFACES` note type is `grant_plan::Manifest`, and the slot constants programs use
  (`DOMAIN_SLOT`) live in `grant_plan` too. A leaf crate holding the manifest type and the slots
  would let a package declare what it links without linking the shell's planner.
- A contract crate's tests that drive nife's own implementation live in `system_tests`. A third
  party needs tests in the contract crate that drive any implementation.

## The version policy

Two numbers, two checkers, already separate in this tree:

- The Cargo version covers the Rust API. A move starts each crate at `0.1.0`. A break bumps the
  minor number and an addition bumps the patch, per Cargo's rule that the left-most nonzero number
  marks an incompatible change. Cargo's resolver then refuses to unify two incompatible versions.
- The wire identity covers what a peer checks: `INTERFACE_ID` and the crate's `VERSION` constant
  (milestone 956 (the linked interfaces carry checkable versions)). A wire break bumps both the
  constant and the Cargo minor. A Rust-only break bumps only the Cargo minor.

The rule a peer checks is the one milestone 979 (interfaces a peer can check) ruled on 2026-10-29
(UTC). The syscall ABI must match at spawn and never dual-serves. A shared page stays
single-version and its reader refuses another magic. A message contract serves N and N-1. This
proposal decides no wire format the rulings leave open; the bind-time check for message contracts
stays with its own proposal.

One rule is new: one revision per contract per image. Every component in a composed image resolves
a contract to the same commit, so the image is the coherent set that §235 (the tree divides by what
releases together) names.

Two checks move with each crate. nife's diff check, which fails a moved magic without its version
constant (milestone 956's step 4), joins the crate template. The template already runs
`cargo-semver-checks` on each pull request against the last published release. These crates stay
unpublished while nife pins them by `rev` (§305 (git dependencies on `nifeos/*` only, pinned to a
landed commit)), so that rule has no baseline and never fires. `jh7110_entropy`'s pull
request #5 renamed five public constants and drew no hold. The fix: compare against the commit
whose manifest last changed version (`--baseline-rev`), so a break without a minor bump fails. Inside
nife, `cargo-semver-checks` stays refused.

## What the integration gate must test

basalt's gate (milestone 755 (basalt v0 pins nife and runs its gate)) does this today. It pins one
component, nife, by commit. Its `gate` workflow runs that commit's `script/ci-build test` on three
architectures. Its `packages` workflow builds basalt's recipes at nife's pin twice and requires
byte-identical output. It installs them with `jig` and composes and boots an image from basalt's own
base packages. Its `main` requires seven checks.

It does not hold a second first-party pin. It cannot see two components pinning different revisions
of one contract. It reads no program's `INTERFACES` note against what the image serves. And it can
test a pair only after both halves merge, because a pin never names an unmerged commit. For a
contract and a consumer to land together it needs four things:

1. A `pins.toml` table for each contract and each program repository.
2. A coherence check that refuses an image whose components' lock files name two revisions of one
   contract.
3. An interface agreement check: each program's `INTERFACES` entries against the versions the
   image's kernel and servers serve. That is the spawn check, run before boot over the whole image.
   A server's served versions can come from its own note; the kernel carries no note today, so it
   needs a list of its own.
4. A trial run: a manual dispatch that takes candidate commits, merged or not, runs checks 2 and 3
   and the system test, and writes no pin. It proves a pair before either half merges.

## How a breaking change lands across two repositories

A real one, 2026-10-31 (UTC). `jh7110_entropy` pull request #5 renamed five constants and merged at
22:22 (`nifeos/jh7110_entropy@535f79c`). nife pull request #2979 opened at 22:27, moved the three
manifests that pin the crate and the program that names the constants, and merged at 23:15. Fifty-three
minutes end to end. nife's `main` never broke, because it pinned the old commit until the bump.

The contract case, worked for `block_roster` after step 4: a hypothetical page change to version 2,
on durations measured above.

1. The contract repository merges version 2: `VERSION = 2`, magic `blkrosv2`, Cargo `0.2.0`, a dated
   ledger entry. A trial run against nife's and `disk-tools`' current pins shows what breaks.
   About 7 minutes of the crate's CI.
2. nife bumps its `rev`, and the kernel writes version 2. Its pull request names the landed commit.
   About 50 minutes through the merge queue.
3. `disk-tools` bumps its `rev`, and `disk_surveyor` reads version 2.
4. basalt moves the three pins in one pull request. The coherence check passes only with all three.
   About 10 to 15 minutes of gate.

The page is single-version, so step 4 is atomic. If basalt moved nife alone, the agreement check
would refuse: the kernel serves (5, 2) and `disk_surveyor` declares (5, 1). A message contract
serves N and N-1, so steps 2 and 3 may ship in different basalt bumps. As §201 (one roadmap until
a citation has to cross) requires, the pull requests in steps 3 and 4 name what they wait for, and
step 1 names nothing.

## How nife consumes a crate that left

By `rev`, as §305 allows. A root `[workspace.dependencies]` entry would pin each contract once for
every nife crate. nife pins `jh7110_entropy` in three manifests, which pull request #2979 had to
move together. Working on both sides at once uses a local `[patch]`, which never reaches `main`.

The CI cost is small per build and real per change. The fetch is one git checkout, cached. The real
cost is the second pull request: 53 minutes for the example above. Had all 22 lived elsewhere for
the last 30 days, up to 244 changes would have needed one. nife's tools that walk `crates/` also
lose each crate: the Kani reach list, the verify table, milestone 610 (the interface's stability is
measured weekly)'s series and the declaration check. Each must read the pinned checkout or the
crate's own repository instead.

## What a third-party implementation needs

- The contract in a repository of its own, licensed in its own manifest, with no path into nife:
  this order delivers that.
- The wire format as code with its constants (§311 (a contract between programs is code, not
  prose)); this exists.
- Tests in the contract crate that drive any implementation through a trait. Today none do.
- The version ledger, so N-1 means something to a stranger.
- basalt able to pin the alternative in place of nife's server and run the same gate: checks 1 to 3.

## Options considered

- A. Bottom-up, one repository per crate, the order above: recommended.
- B. Programs first and the SDK last, the 2026-10-04 order: refused by the second direction. Each
  of the three program repositories would pin nife for 12 to 15 crates.
- C. All 22 into one repository in one move: it saves 5 crossing commits in 348. It makes the
  largest one-way step at once and gives no contract its own version.
- D. Highest churn first, to practice where it hurts: `grant_plan` sits a layer up and needs three
  crates first, so bottom-up already fixes its place.
- E. Wait for crates to settle: refused by the first direction.

## Reversibility

A move is cheap to undo while nife and basalt are its only consumers: copy the crate back and drop
the pin. Fuchsia's 2019 fold-back of its layers is the precedent. Other parties act on four things,
and those make the door narrow: a repository name, a published crate version (crates.io never deletes
one), an `INTERFACE_ID` value and a release binary that compares a magic. The two reverts above came
from the fourth. So the moved crates stay unpublished under §305 until their own release milestone.
Ratifying the `INTERFACE_ID` values, still open per milestone 979, comes before the first
contract leaves. §201 already asks for a name ratified before anything moves.

## The seven questions

1. Considered: the options above, each with its reason.
2. The tree: `jh7110_entropy` left on 2026-10-31 and nife pins it by `rev`; the crate template exists;
   basalt pins nife; milestone 956 carries the constants.
3. Prior art, in the appendix. This lane read Fuchsia's layers and petals, Android's stable AIDL,
   Cargo's semver guide, `cargo-semver-checks` and the Linux kernel's stable API document. It
   recalls Rust's crater.
4. The premise holds with one correction: the first cut's 15 programs do not need all 22 at once.
   `disk-tools` needs 12.
5. Cost: the measurements appendix, by command.
6. Reversibility: above.
7. At equal effort the choice stands. The order follows the graph and the second direction, not
   effort. Within a layer, cheapest first is a choice about learning cost, and this sentence says so.

## Questions for the architect

Asked one at a time, in this order, with the background in the questions appendix:

1. The first cut waits for its linked crates and leaves package by package, `disk-tools` first.
2. One repository per contract crate.
3. The architect hold on a semver break applies to wire identity changes only.
4. basalt refuses an image with two revisions of one contract.
5. basalt may run a trial on unmerged commits, writing no pin.

## The review packet

`script/crate-review-packet <crate>...` writes one markdown packet per crate to
`target/crate-review-packets/<crate>.md` (`--out DIR` changes the directory; `--no-rustdoc` forces the
grep listing). Packets live under `target/` and are never committed. Each holds the crate's path,
version, tag and `src` line counts; its public API from rustdoc JSON on the pinned toolchain, and
the types from other crates in public signatures (`cargo public-api` is not installed, so it is not
used; the fallback is a grep listing); every `unsafe` site with file and line; every dependency and
where it points; the references back into nife that `script/crate-standalone` reports without
building; its Kani harnesses and falsification patches; the nife packages that depend on it; its
`INTERFACE_ID`, `VERSION` and `MAGIC` values; and the reviewer's checklist. The name is a provisional
maintainer decision.

## BUGS

- The packet leaves out the test list, the license notices and the `cargo-semver-checks
  --baseline-rev` output. The last needs the tagged baseline that milestone 818 cuts first.

## Index row

The order in which the 22 crates the first cut links leave this repository: bottom-up, one
repository each, `block_roster` as the contract pilot, `disk-tools` as the first package. It sets
the version policy and the gate that let a breaking change land across two repositories.
