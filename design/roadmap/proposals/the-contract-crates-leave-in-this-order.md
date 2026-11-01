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
code. Every new name is provisional. The text states each ruling (2026-11-01 UTC).

Reuse: `cargo metadata`, git's rename detection, milestone 610's series and the crate template.

Two directions from the architect, both dated 2026-11-01 (UTC), frame it:

- Crossing repositories is the goal of the split (§151 (the goal of the repository split is
  independent release)), not a cost to avoid, and the project should become good at changes that
  span two repositories. Contract crates should carry versions and may leave `nifeos/nife`, to ease
  that change and to let others write alternative implementations against them. Waiting for a crate to settle is not a criterion.
- Anything that leaves must not reference back into nife. A crate leaves bottom-up in the
  dependency graph, before or with every program that links it, and nothing that has left depends
  on a path or a pin into nife. nife may depend on what left.

The second direction inverts the order of 2026-10-04 in milestone 756 (`process-tools`,
`core-tools` and `disk-tools` are the first code to leave this repository) and milestone 691
(packages move out of this repository, one per pull request). Programs first and the SDK last
maximizes references back: each program repository would pin nife for the crates it links.

Churn sizes the coordination cost and picks the version policy; it never sets a wait. Appendices: [measurements](the-contract-crates-leave-in-this-order/measurements.md), [prior
art](the-contract-crates-leave-in-this-order/prior-art.md) and [questions for the
architect](the-contract-crates-leave-in-this-order/questions.md).

## What the measurements say

Measured at `nifeos/nife@a8b7524d7`; the measurements appendix reproduces it.

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
crate and do not block, but a crate's fuzzing belongs with it. Between 5 and 36 files in nife name each crate's path.

## The recommended order

The architect reads each crate's code by hand before it leaves, at first every crate (condition 4
of §247 (the split begins with basalt holding nife, and `procps` moves first), ruled 2026-11-01
(UTC)). A crate also needs a tagged release whose commit carries its Cargo version, the baseline
for `cargo-semver-checks --baseline-rev`. The first crate reviewed is the pilot, `jh7110_entropy`.
It moved on 2026-10-31 (UTC) and has no tag, so milestone 818 tags it first. No time-based measure
remains: a crate whose interface still breaks may leave, each break a new tagged release.

The rule: a crate leaves when every crate it links (normal, build and dev) has left or leaves with
it, and no file in it names a nife path. Within a layer, the cheapest coordination goes first, so
the project learns the landing on crates where a mistake costs little. Ruled 2026-11-01 (UTC): each
package leaves as soon as its own crates have left, in the order `disk-tools`, `process-tools`,
`core-tools`, replacing milestone 756's single cut, and `block_roster` stays the first crate a
consumer outside nife reads. Ruled the same day: one repository per contract crate, created when
the crate moves and only with the architect's approval of that exact repository. No lane creates
one. A repository per package or per layer is refused. The four crates whose disposition is contribute
(`globally_unique_identifier_partition_table`, `glob`, `domain_name_system`, `calendar`) move in this
order too (ruled 2026-11-01 UTC); only the offer waits.

| step | crates | why here |
|---|---|---|
| 0, the review pilot | `jh7110_entropy` | tag it, then the architect reads it; the others follow only as each review clears |
| 1, the contract pilot | `block_roster` | a leaf with no blocking reference; one writer in the kernel, one reader in `disk_surveyor`; its magic carries its version, so a reader refuses a mismatch today |
| 2, leaves for `disk-tools` | `entropy_contract`, `clock_contract`, `socket_contract`, `globally_unique_identifier_partition_table`, `counter_frequency_contract`, then `filesystem_contract` with `filesystem_fixture`, then `abi` | cheapest first by crossing commits and outside dependents; `abi` last, at 66 crossing commits and 12 dependents |
| 3 | `address_space_map`, then `current_cpu_contract`, then `user_mode_runtime`, then the `INTERFACES` leaf crate | after the `link.ld` test moves; `user_mode_runtime` links six crates of step 2. The leaf crate (`interfaces_note`, provisional) first takes the note type out of `capability_manifest_note` inside nife, then leaves like any leaf (below) |
| 4, first package | `disk-tools` | leaves as soon as its 12 crates and the leaf crate have left; milestone 756's condition 4 has it carry the `INTERFACES` macro, and with the leaf crate it never waits for `grant_plan` |
| 5, leaves for `process-tools` | `byte_sink_contract`, `glob`, `program_argument_contract`, `domain_name_system` | leaves; `program_argument_contract` after the pair test moves to basalt (the proposal the cross-repository pair is tested where the pin is) |
| 6 | `machine_statistics_contract`, `domain_name_resolution_contract`, `grant_plan`, then `capability_manifest_note` | `grant_plan` is the largest coordination cost of all 22 |
| 7, second package | `process-tools` | leaves as soon as its own crates have left |
| 8 | `calendar`, `environment_contract`, then `core-tools` | the last two crates only `core-tools` reaches; the third package leaves as soon as they have |
| 9, `redoxfs` | `subtree_scope`, `memory_region_heap`, then `redoxfs` as one repository | after the 10 contract crates it links; ruled 2026-11-01 (UTC) in the questions appendix |

`jh7110_entropy` stays the leaf pilot (milestone 818 (`jh7110_entropy`: proven JH7110 TRNG logic,
released on its own)). It proves that a leaf crate builds and proves from its own repository and
that nife pins it by `rev` with nothing pointing back. Its 31-line manifest holds no path or pin
into nife. It does not prove the contract case: no consumer outside nife, so no two
pins must agree, and no version check has refused a real mismatch. The
`block_roster` pilot proves all three once `disk-tools` leaves.

Two findings want lanes of their own, proposed as follow-on work:

- Ruled 2026-11-01 (UTC): the `INTERFACES` note type moves into a leaf crate of its own, so
  `disk-tools` need not wait for `grant_plan`. It is step 3, and its lane is owed. The questions
  appendix records the check against pull request #3022's lint.
- A contract crate's tests that drive nife's own implementation live in `system_tests`. A third
  party needs tests in the contract crate that drive any implementation.

## The version policy

Two numbers, two checkers, already separate in this tree:

- The Cargo version covers the Rust API. A move starts each crate at `0.1.0`. A break bumps the
  minor number and an addition bumps the patch, per Cargo's rule that the left-most nonzero number
  marks an incompatible change. Cargo's resolver then refuses to unify two incompatible versions.
  Ruled 2026-11-01 (UTC): a Rust-API break that bumps the minor needs no architect.
- The wire identity covers what a peer checks: `INTERFACE_ID` and the crate's `VERSION` constant
  (milestone 956 (the linked interfaces carry checkable versions)). A wire break bumps both the
  constant and the Cargo minor. A Rust-only break bumps only the Cargo minor.

The rule a peer checks is the one milestone 979 (interfaces a peer can check) ruled on 2026-10-29
(UTC). The syscall ABI must match at spawn. A shared page stays
single-version and its reader refuses another magic. A message contract serves N and N-1. This
proposal decides no open wire format; the bind-time check stays with its own proposal.

One rule is new, ruled 2026-11-01 (UTC): one revision per contract per image. basalt refuses an
image whose components pin different revisions of one contract, and the bump pull request that
moves a contract moves every pin of it together. Overriding pins and building each as pinned are refused. The image is then the coherent set that §235 (the tree divides by what releases
together) names. A pin is a git dependency on `nifeos/<crate>` by `rev` with its version stated.

Two checks move with each crate. nife's diff check, which fails a moved magic without its version
constant (milestone 956's step 4), joins the crate template. The template already runs
`cargo-semver-checks` on each pull request against the last published release. These crates stay
unpublished while nife pins them by `rev` (§305 (git dependencies on `nifeos/*` only, pinned to a
landed commit)), so that rule has no baseline and never fires. Compare against the commit
whose manifest last changed version (`--baseline-rev`), so a break without a minor bump fails. Inside
nife, `cargo-semver-checks` stays refused.

Ruled 2026-11-01 (UTC): for a contract crate the `needs-architect` hold applies to a wire-identity
change only (`INTERFACE_ID`, `VERSION` or a magic). A Rust-API break must bump the Cargo minor,
checked mechanically, with no hold. Other crates keep the hold on every break. Telling the two apart needs an identity to compare, and 14 of the 22 have
no `INTERFACE_ID` (the `OWED` table of milestone 956). Those 14 get one first; until then their breaks keep the hold.

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
   contract (ruled 2026-11-01 (UTC)).
3. An interface agreement check: each program's `INTERFACES` entries against the versions the
   image's kernel and servers serve. That is the spawn check, run before boot over the whole image.
   A server's served versions can come from its own note; the kernel carries no note today, so it
   needs a list of its own.
4. A trial run, ruled 2026-11-01 (UTC): a manual dispatch that takes candidate commits, merged or
   not, of repositories in the `nifeos` organization, runs checks 2 and 3 and the system test, and
   writes no pin and no file. It never runs a fork's pull request, and never on a self-hosted runner
   for a fork. It proves a pair before either half merges. The pin rule stands: a pin never names an
   unmerged commit (§305 (git dependencies on `nifeos/*` only, pinned to a landed commit)).

## How a breaking change lands across two repositories

A real one took 53 minutes (`jh7110_entropy` pull request #5, then nife's #2979). The
[landing appendix](the-contract-crates-leave-in-this-order/landing.md) works that case, then the
contract case for `block_roster` in four steps. The page is single-version, so the three pins move
in one basalt pull request; a message contract serves N and N-1 and may ship in separate bumps.

## How nife consumes a crate that left

By `rev`, as §305 allows. A root `[workspace.dependencies]` entry would pin each contract once for
every nife crate. nife pins `jh7110_entropy` in three manifests, which pull request #2979 had to
move together. A local `[patch]` serves work on both sides and never reaches `main`.

The real cost is the second pull request: 53 minutes for the example above. Had all 22 lived
elsewhere for the last 30 days, up to 244 changes would have needed one. nife's tools that walk `crates/` also
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

- A. Bottom-up, one repository per crate, the order above: ruled 2026-11-01 (UTC).
- B. Programs first and the SDK last, the 2026-10-04 order: refused by the second direction. Each
  of the three program repositories would pin nife for 12 to 15 crates.
- C. All 22 into one repository in one move: it saves 5 crossing commits in 348 and gives no
  contract its own version.
- D. Highest churn first: `grant_plan` needs three crates first, so bottom-up fixes its place.
- E. Wait for crates to settle: refused.

## Reversibility

A move is cheap to undo while only nife and basalt consume it: copy the crate back. Other parties
act on four things, which narrow the door: a repository name, a published crate version (crates.io never deletes
one), an `INTERFACE_ID` value and a release binary that compares a magic. The two reverts above came
from the fourth. The moved crates stay unpublished under §305 until their own release milestone.
Ratifying the `INTERFACE_ID` values, still open per milestone 979, comes before the first
contract leaves. §201 already asks for a name ratified before anything moves.

## The seven questions

1. Considered: the options above.
2. The tree: `jh7110_entropy` left on 2026-10-31 and nife pins it by `rev`; the crate template exists;
   basalt pins nife; milestone 956 carries the constants.
3. Prior art, in the appendix.
4. The premise holds with one correction: the first cut's 15 programs do not need all 22 at once.
   `disk-tools` needs 12.
5. Cost: the measurements appendix.
6. Reversibility: above.
7. At equal effort the choice stands. The order follows the graph and the second direction, not
   effort. Within a layer, cheapest first is a choice about learning cost, and this sentence says so.

## The review packet

`script/crate-review-packet <crate>...` writes one packet per crate to
`target/crate-review-packets/<crate>.md`, never committed: the facts a reading needs, and a checklist
for the judgments. The API comes from rustdoc JSON, since `cargo public-api` is not installed. The
name is a provisional maintainer decision.

## BUGS

- Proposed milestone, provisional and unnumbered: the crate template, which holds every
  `cargo-semver-checks` break, labels only a moved `INTERFACE_ID`, `VERSION` or magic, and fails a Rust-API break
  that leaves the Cargo minor alone. It waits on the 14 crates getting an `INTERFACE_ID`.
- Proposed milestone, provisional and unnumbered, following milestone 756: move the note type and
  update the declaration check and the 15 programs.
- The leaf crate is a 23rd contract crate. Its repository waits for the architect's approval.
- Proposed milestone, provisional and unnumbered: the QEMU runners start `redoxfs_server` from a
  pinned image, not a tree path. It blocks step 9, as does the open placement of `vendor/talc`.
- The packet omits the test list, the license notices and the `cargo-semver-checks` output.

## Index row

The order in which the 22 crates the first cut links leave this repository: bottom-up, one
repository each, `block_roster` as the contract pilot, `disk-tools` as the first package. It sets
the version policy and the gate that let a breaking change land across two repositories.
