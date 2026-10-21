# The first split: what is ready, what goes first, and what basalt must be

Measurements for the proposal
[milestone 756 (`process-tools`, `core-tools` and `disk-tools` are the first code to leave this repository)](../design/roadmap/0756-process-tools-core-tools-and-disk-tools-leave-first.md), promoted from the proposal `which-repository-split-goes-first`,
on calef's prompt of 2026-10-04 (UTC): *"I do think we should soon be breaking things out of the mono
repo and into their own repos. I also think the basalt repo is sitting there waiting to package up
those repos for its distribution."* The direction is ruled (§151 (the goal of the repository split is
independent release), §235 (the OS is built and updated from packages, and the tree divides by what
releases together)); the order is not. This note holds the numbers; the proposal holds the options
and the recommendation. The file name is a lane's coinage and provisional.

Measured 2026-10-04 at `origin/main` `04a8f9e6c`. "Thirty days" means non-merge commits reachable
from that commit with a commit date on or after 2026-09-04: 2,447 commits, from **876 merged
pull requests**. The scripts are reproducible from what is described here; none is committed,
because the column they compute belongs to milestone 610 (the interface's stability is measured
weekly) and should be built there, not twice.

## The rulings, 2026-10-04

calef ruled both questions on 2026-10-04 (UTC), on pull request #1609. §247 (the split begins with
basalt holding nife, and `procps` moves first) records them. basalt v0 is the first step, with the
go-ahead for a lane's first write to `nifeos/basalt`; that is milestone 755 (basalt v0 pins nife and
runs its gate). The first code cut is `procps` + `coreutils` + `util-linux`, only once all six
conditions in milestone 756 hold. calef renamed the three `process-tools`, `core-tools` and
`disk-tools` on 2026-10-06 (UTC), with `uuid` moving from the third to the second; the cut's
contents are unchanged. The repository name and home, the citation identity and the SDK's
place in milestone 691's order stay open.

## 1. Are §151's preconditions met?

§151 lists two, from the records: the wire protocols carry real version numbers and have stopped
churning weekly, and a package format exists. §235 and the blocks under it added more. Each is
checked below.

| precondition | source | met? | measured |
|---|---|---|---|
| Protocols carry version numbers | §151, milestone 39 (repository structure for a loosely-coupled OS) | no | All 93 crates in the tree are `version = "0.1.0"`. Of 19 `*_protocol` crates, 5 carry a numbered magic on a shared-page layout (`TIMEBAS1`, `CURRCPU2`, `ENVCONF1`, `MACHSTA1`, `CLOCKv01`) and 2 an unnumbered one (`nifeargv`, `nifeklog`). None of the IPC operation sets (`filesystem_contract`, `socket_contract`, `login_contract`, `hot_swap_contract` and the rest) carries a version a peer could check. Formats do better: `capability_manifest_note` (`VERSION = 1`), `boot_slot` (`SLOT_VERSION = 1`), `package_archive` (`NIFEPKG1`), `nifefs` (`CRKR0002`). |
| Protocols stopped churning weekly | §151 | no | Milestone 610's own series, last four weeks (2026W37 to W40): 6, 44, 46 and 61 breaking changes to the contract crates, 157 in all against the proposed threshold of four. Co-change excluding mass commits: 66.7%, 51.9%, 74.4%, 74.4%, against the proposed 50% line. Syscall surface: 3 changed and 1 renamed in W40. |
| A package format exists | §151, milestone 198 (a package manager) | yes | §197 (a package is one archive file)'s archive (`crates/package_archive`), `cargo xtask package` from a reviewed recipe, and `package install`, `remove` and `rollback` at the prompt on all three architectures (milestone 198, rung 3a, built 2026-09-26). |
| No package moves before a per-package co-change column exists | §235 Fork 3 amendment | no | Milestone 610 is BUILT and its own Follow-on records the column as owed and not built. Section 2 below is a one-off reading of it, which is not the column. |
| The image is assembled from base packages by digest | §235 P1, milestone 607 (nife is built and updated from packages) item 1 | no | Not started. `xtask/src/archive.rs` packs ELFs out of `target/`, not packages by digest. This is the thing `basalt` would assemble from. |
| An SDK archive (S2) | §235 Fork 4, milestone 607 item 4 | no | No `xtask` command builds one. A program outside the tree has no way to get `abi` and its protocols except a clone. |
| A contract never links the runtime | §235 P3, milestone 607 item 3 | no | Four contract crates still depend on `user_mode_runtime`: `counter_frequency_contract`, `current_cpu_contract`, `supervision_protocol`, `hot_swap_contract`. |
| `components` split into one crate per package | milestone 691 (packages move out, one per pull request) | no | One crate, `components`, holds 56 binaries from 18 packages. A crate cannot live in two repositories, so no package with a program in it can move. |
| A ratified home | milestone 611 (every program and crate belongs to a package), §235 Fork 3 | no | 0 of 27 packages ratified; 26 provisional, 1 undecided (`redoxfs`). |

**Three of the nine hold no, structurally, for any code-carrying cut**: the components crate, the
SDK archive, and the co-change column. Protocol churn is the precondition §151 named, and it is the
furthest from met.

### The churn per protocol crate

Commits in thirty days touching each crate. "Sweep" is a commit that touched 15 or more top-level
directories (a tree-wide rename or audit); `#1599` (spell `op` as `operation` across the tree,
merged 2026-10-04T05:32Z) is broken out because the brief named it. It touched 19 crates under
`crates/`, 10 of them `*_protocol`. "Alone" is a commit that touched that crate and nothing else.

| crate | commits | #1599 | sweeps | alone | breaking, 4 weeks |
|---|---|---|---|---|---|
| `abi` | 29 | 0 | 6 | 1 | 4 |
| `filesystem_contract` | 23 | 1 | 6 | 1 | 10 |
| `login_contract` | 15 | 1 | 5 | 0 | 1 |
| `hot_swap_contract` | 14 | 1 | 7 | 0 | 10 |
| `supervision_protocol` | 10 | 0 | 4 | 0 | 2 |
| `socket_contract` | 8 | 1 | 5 | 0 | 13 |
| `byte_sink_contract`, `credential_contract`, `std_runtime_contract` | 6 each | 1, 1, 0 | 4, 3, 2 | 0 | 5, 4, 0 |
| the other ten | 2 to 5 each | | | 0 to 2 | 0 to 5 |

Even net of sweeps and `#1599`, `abi` moved 23 times and `filesystem_contract` 17 times in thirty
days, and **six commits in thirty days changed a protocol crate and nothing else**. A protocol here
changes together with its server and its client, which is what a monorepo is good at and a split
makes into two or three pull requests.

Settled by 610's proposed rule (four weeks with no breaking change): `program_argument_contract`,
`boot_slot`, `capability_witness_contract`, `compositor`, `current_cpu_contract`, `elf`,
`capability_manifest_note`, `measured_boot`, `std_runtime_contract`. Nine of the 30 crates 610 measures.

## 2. Which division could go first: coupling by candidate

Membership comes from `packages/*.package.toml` through `helpers/packages.py`'s own claim rule
(longest prefix wins), so every changed path is attributed to the package or home milestone 611
drew. For each candidate first cut:

- touched: commits in thirty days touching any of its paths.
- cross: of those, commits that also touched another package's code. Each one is a change the
  split would have turned into pull requests in two repositories, landed in order.
- sweeps: commits touching six or more packages (42 of 2,447 in the window).
- dependents: crates outside the cut whose `Cargo.toml` links a crate inside it.
- path refs: lines outside the cut, `design/` and `notes/` naming its directories (gates,
  runners, CI, build).

| candidate | crates | touched | cross | of which sweeps | focused cross | dependents | path refs | cites out |
|---|---|---|---|---|---|---|---|---|
| `contracts` | 37 | 273 | 172 (63%) | 38 | 134 | 35 | 187 in 61 files | 57 milestone, 52 § |
| `contracts` + `runtime` + `cryptography` (the SDK) | 42 | 298 | 187 | 41 | 146 | 35 | | |
| `host-tools` (`xtask`, `stick_maker`) | 4 | 227 | 139 (61%) | 32 | 107 | 1 | 276 in 152 files | |
| `redoxfs` (vendored engine, server, host tool) | 3 | 31 | 26 (84%) | 14 | 12 of 17 | 0 | 254 in 64 files | 155 milestone (25 distinct), 53 § |
| `procps` + `coreutils` + `util-linux` | 8 | 32 | 26 (81%) | 21 | 5 of 11 | 1 (`components`) | 0 path, 24 by program name | 24 milestone (5 distinct), 32 § |
| `rmle` + `mdr` + `demos` | 0 | 17 | 16 | 15 | 1 of 2 | 0 | | |
| `basalt`'s manifest, with nothing moved | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |

What the table says, read rather than summarized:

- **The SDK cut is the worst first cut by every column**, and it is the order milestone 691 proposes
  ("`contracts` and `runtime` move before the programs that link them"). 134 focused changes in a
  month would have been multi-repository, against 35 dependents. 691's rule is right as a
  dependency order for a settled tree; on this tree it moves the most volatile piece first.
- **The leaves are cheap except for sweeps.** `procps` and friends had 11 focused commits in a
  month, 5 of which crossed. But 21 of their 32 commits were sweeps, and a sweep after a split is a
  pull request per repository. Sweeps are 1.7% of commits overall and land disproportionately on
  small packages, because a small package has little else happening to it.
- **`redoxfs` is the most separable build and among the most coupled code.** It is already three
  workspaces of its own, outside the root workspace, with its own `Cargo.lock` files, gated by
  `--manifest-path`, and nothing links it. But 12 of its 17 focused commits crossed, almost all
  milestone 606 (a directory walk costs what it does on Linux)'s subtree-grant work against `filesystem_contract`, `subtree_scope` and
  `capability_manifest_note`. Its interfaces are the least settled ones: `filesystem_contract` 10 breaking
  changes in four weeks, the partition table 27, `entropy_contract` 3. Every QEMU runner starts it,
  which is the 254 path references.
- **Programs in `components` cannot move at all yet.** `rmle`, `mdr`, `demos`, `core-tools`,
  `disk-tools`, `entropy`, `installer`, and the programs of `process-tools`, `login`, `init`, `terminal`
  and seven more are `[[bin]]` targets of one crate. The `rmle` + `mdr` + `demos` row has no crate of its own at all.

### What these numbers do not say

- The `procps` + `coreutils` + `util-linux` row, and "`procps` and friends" below it, keep the
  package names measured on 2026-10-04. They are `process-tools`, `core-tools` and `disk-tools`
  since 2026-10-06, holding the same 15 programs.
- The dependents column counts Cargo links. A program named by a gate (`swish-check`'s `ps`
  line, a QEMU runner starting `redoxfs_server`) is a dependency the split must also carry, and
  the path-reference column only half catches it.
- 610's per-crate view measures 30 crates. `machine_statistics_contract`, `glob`,
  `address_space_map` and `subtree_scope`, which the candidate cuts link, have no row, so their
  stability is unmeasured. The proposal carries this as a precondition rather than a BUGS line on
  610, because a lane does not edit another milestone's block.
- Thirty days is one regime: milestones 606, 609 and 611 all landed inside it. The column 610 owes
  is weekly so this reading can be replaced by a trend.

## 3. What basalt needs before the first cut, and the smallest basalt

`github.com/nifeos/basalt` is empty. Milestone 120 (the rename: the OS becomes `nife`) reserved it; §151 names its job: it assembles
released pieces, as a Linux distribution assembles upstreams, owning none of them. §235's BUGS
notes "manifest" already means two things; the list of what an image holds needs its own name.
The names below are placeholders for calef.

What it needs, in the order the work depends on it:

1. A pin per component. Source by repository and commit, the way an Android `repo` manifest or a
   kas file for Yocto pins a layer (from memory, not re-read this session). With one component,
   that is one line: `nife` at `04a8f9e6c`.
2. A pin per artifact, once anything is prebuilt. The tree already has this format: a reviewed
   recipe carries the digest (§195 (a reviewed recipe vouches for a package)), the archive is §197's, and the image's catalog is `name
   digest` lines. A second component's output reaches the image as a package pinned by digest, and
   `script/build-is-reproducible` is what makes a digest pin re-checkable from source.
3. **A CI job that builds the image from the pins and runs the system gate on all three
   architectures.** Today that is `script/test` and `ci.yml`'s 19 jobs (merge-group median 16
   minutes over the last 30 green runs). §151's BUGS calls this the real cost; §235 says the
   integration suite "needs every division at a known version to run". basalt is where that
   known version is written down.
4. A toolchain rule. `rust-toolchain.toml` changed 27 times in thirty days. Each component
   keeps its own pin and basalt builds each at the pin the component's commit carries; basalt has no
   toolchain of its own. Both candidate code cuts are `no_std`, so neither needs the `nife-dev`
   `std` toolchain; a `std` program in another repository would.

### basalt v0, the smallest thing that is still a distribution

One manifest file names one component (`nife` at a commit). A workflow checks that commit out and
runs its own image build and system test, and keeps the image as a build artifact. No new format beyond the pin, no package
assembly, nothing moved out of nife. A scheduled job bumps the pin the way `toolchain-bump.yml`
bumps the nightly. v0 cannot break nife, because nife does not know it exists.

basalt v1 is the first cut: two components, the second's output entering the image as a
package by digest. That needs §235's P1 (the image assembled from packages by digest), which is
milestone 607's item 1 and unbuilt.

## 4. What a two-repository world breaks or needs

| what | today | after the first cut | measured size |
|---|---|---|---|
| Citations | one tree; `script/citations` resolves every `milestone N` and `§N` offline | §201 (one roadmap until a citation has to cross)'s revisit fires on the first citation that crosses: its event condition | `redoxfs` would carry 155 milestone and 53 § citations out; `procps` 24 and 32; basalt v0 none if it is written to cite nothing |
| `design/roadmap/MOVED` | absent | §201 requires it written at the split, not after | one line per record that moves; a code cut moves code, not records, so it may stay empty |
| Merge queue and `nife-smelter[bot]` | 19 workflows, one ruleset, one queue | a second ruleset and queue; the bot App installed on the repository; the watchers (`merge-drain`, `ready-status`, `architect-label`, `trunk-health`) either copied or turned into reusable workflows | 19 workflow files, 48 jobs |
| A cross-repository change | one pull request, one queue pass | component pull request, its queue, then basalt's pin bump and its queue; §201 puts the dependency on the blocked side only | at least two serial queue passes, about 32 minutes at today's median |
| Lane workflow | one worktree, one branch, one draft pull request claims a milestone (§90 (the claim is a draft pull request)) | two clones; a lane spanning both opens two drafts; the board becomes `gh search prs --owner nifeos --draft` | the `nife-worktrees` directory grows a sibling |
| Gates that assume one tree | `script/lint` runs 73 sections; 33 scripts and helpers read `design/roadmap/` directly | the moving repository needs a subset (clippy, fmt, its own tests, naming, em-dashes, spelling); the roadmap gates stay in nife | the subset is unmeasured until a cut is chosen |
| `packages.py` boundaries | every tracked path has one home in this tree | a moved package's `home` flips to ratified; its paths stop being tracked here | 1 of 27 packages per cut |
| Supply chain | `script/vendor-verify` and `vendor-watch.yml` check `vendor/redoxfs` against its pin | if `redoxfs` leaves, both move with it | 2 vendor commits in thirty days |
| CI cost | one run per merge group | the component's run, plus basalt's full-system run per pin bump | basalt's run is nife's system gate, so it roughly doubles the system-test minutes for any change that crosses |

**The top cost is the cross-repository change**, not CI minutes. At this month's rate, the
smallest code cut (`procps` and friends) would have turned 26 of 32 commits into ordered two-repo
landings, and the SDK cut 187 of 298. Merge throughput is this project's binding constraint
(`CLAUDE.md`, "The bottleneck moves"), and a two-repo landing is two passes through it.

## BUGS

- The co-change reading is a one-off, at one commit, over one month. It is not milestone 610's
  owed column and should not be cited as if it were.
- "Sweep" uses two thresholds: six or more packages for the coupling table, fifteen or more
  top-level directories for the protocol table. They are judgment calls and change the focused
  numbers by a few commits either way.
- Prior art in section 3 is recalled, not re-read for this note. Fuchsia's splitting of its tree into
  layered repositories and later folding them back is from memory and is the most relevant case;
  it should be read before any ruling leans on it.
- CI wall time is the merge-group median of the last 30 green `ci.yml` runs, which includes
  short-circuited runs (minimum 3 minutes); the full-system run is longer than the median.
