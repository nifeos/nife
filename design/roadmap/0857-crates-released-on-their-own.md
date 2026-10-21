---
status: NOT-STARTED
raised: 2026-10-07
promoted_from: crates-released-on-their-own
milestone_dependencies: none
decision_dependencies: unwritten
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 857. Crates released on their own, and proofs offered to the projects that already exist

*(The title and slug are provisional.)*

calef, 2026-10-07 (UTC), ruling fork 2 of #1803 (nife writes its
own FAT crate): *"It seems like something we should build and prove and release on its own
independently of nife. I suspect we have other such crates."* Then: *"Dispatch the crates that
could be released on their own. We should also assess for each of the releasable ones what the
other implementations are so that we could work with those owners to add the proofs we need rather
than jumping their missions."*

The inventory and the survey are in
[notes/releasable-crates-2026-10-07.md](../../notes/releasable-crates-2026-10-07.md). This file
holds the forks and does not restate the numbers beyond what a fork needs. Names in it are
provisional, this one included.

Reuse: the release mechanics borrow from Bytecode Alliance's `wasmtime` (many crates published
from one repository), RustCrypto (one repository per family), rust-embedded and rust-osdev (one
repository per crate), and crates.io trusted publishing (RFC 3691). Nothing is written here; this
decides how existing crates leave.

## The rulings

calef ruled all eight forks on 2026-10-07 (UTC), between 13:46 and 14:16, in the maintainer
session, relayed in comments on pull request #1806. Each fork below keeps its options as the
record of what was weighed, and its heading says how it was ruled.

- Fork 1, replacing the recommendation: one repository per crate under `nifeos`, created when that
  crate is first published. Until then it stays in this tree, and milestone 610's co-change
  measurement still decides which crates are ready (§235). Every crate repository uses one shared
  CI template (Kani, lint, release) through reusable workflows. calef: *"I lean one repo per
  crate."* then *"Yes a"*.
- Fork 2: a crate that has left is consumed as a crates.io release pinned with its checksum in
  `Cargo.lock`. A `[patch]` to a local checkout or branch is allowed only during development, and a
  lint fails if one reaches `main`. calef: *"Ratify fork two"*.
- Fork 3: its own semantic version from 0.1, a stated minimum Rust version, and its Kani harnesses
  and falsification patches shipped inside it. Every crate carries `publish = false` unless it is on
  a lint-checked allowlist calef approves. calef: *"Yes"*.
- Fork 4: `script/verify` proves a crate while it is here. After it moves, the shared template runs
  `kani-github-action`, and the release workflow refuses to publish unless the proof job is green.
  calef: *"Yes"*.
- Fork 5: names stand on their own, with no `nife` prefix. A crate keeps this tree's name where it
  is free and descriptive, and otherwise gets a fresh descriptive name, ratified when it is first
  published. calef: *"It seems like we should pick create names that can stand on their own. If
  Nife fails the crates may survive."* Amended by calef on 2026-10-11 (UTC): *"We should ensure
  that we ratify the crate names before we break them out."* So a name is ratified, as `script/names`
  records it, before the crate leaves the tree: before its `nifeos/<name>` repository and before
  the standalone copy, since both carry it. `script/crate-standalone` checks it.
- Fork 6: crates.io trusted publishing from a `nifeos` workflow, no long-lived token, a `nifeos`
  team as owner, and a release only when calef approves it. calef: *"Yes"*.
- Fork 7, replacing approval of the table as a whole: each crate is reviewed with calef one at a
  time. A crate approved to leave, published or with proofs contributed, becomes its own milestone
  at that review, quoting his ruling, its disposition, the target project and who writes the
  offer. A crate marked internal gets a line in the note and no milestone. calef: *"I think we need
  to review them one by one. Should we create milestones for each?"* then *"Yes"*.
- Fork 8, replacing the recommendation: lanes write offers, disclosed as agent-written, only to
  projects whose policy admits disclosed AI work. Where a project bans AI-generated contributions or
  requires a person's own words, nife sends no code or proofs, and calef writes nothing by hand.
  calef: *"I need to scale myself. I'm not signing up to be a manual contributor. I think where AI
  is banned I just shouldn't contribute."* Refined at 14:16: bug reports are fine everywhere,
  including there. They stay disclosed, and a security finding goes privately to the maintainer
  first. calef: *"Bug reports are fine."*
- Approval cadence, ruled at 14:17: every upstream submission, whether an offer, a reply carrying
  code or a bug report, comes to calef before it is sent, as his 2026-10-06 rule says. Approving
  once per project at its crate review was offered and not taken for now. calef: *"Let's start
  with b."*

No per-crate milestone is minted by this block. They come from the reviews below.

## What every crate clears before it publishes (calef, 2026-10-11)

calef, 2026-10-11 (UTC): *"As a gate for each of these milestones, I want to ensure that we're
exposing clean crates without nife assumptions. I also want to ensure that the gates that we have
for nife will be adapted for these crate repos as well."* Then: *"Don't we want proofs and
falsifications for our crates?"* This adds to Forks 3 and 4 and changes neither.

- Proofs. No crate publishes without Kani harnesses covering its hostile-input and state-machine
  surface, each failing at its own assertion under a falsification patch, replayable in the crate's
  own repository with stock Kani. Each block's *What is proven* table lists every required property
  and its harness, so coverage is judged, not counted.
- Standalone. `script/crate-standalone <crate>` (provisional name) copies the crate alone and
  builds and tests it on stable for the host and its targets. It refuses this tree's citations and
  provenance, requires a README, both license files and a `rust-version`, and replays the
  falsifications. It runs while the crate is still here; all eight fail it today.
- Gates. Milestone 887 (crate repositories share one CI template) adapts nife's gates row by row.
  It also dual licenses each crate as nife is: *"nife is MIT OR Apache-2.0, so we should dual
  license our crates as well."*

What nife does at each extraction is a checklist, built with the first (813):
[what-nife-does-at-each-extraction.md](0857-crates-released-on-their-own/what-nife-does-at-each-extraction.md).

Adoption is measured once the first crate publishes or the first offer goes out, and not before.
The measures: crates.io downloads over 90 days, reverse dependencies, and issues and pull requests
from outside `nifeos`. And for offers: made, accepted and declined, with bug reports filed and
fixed.

## The per-crate reviews, in order

Proposed order, reversible. FAT is first by calef's ruling, and it is the only crate already
ruled to be written. Then owners who already run Kani, then the rest by harnesses offered, then the
rows fork 8 changes, then the publications that need a cut or proofs first. The internal crates
close the list as one sitting, since each needs only a line.

1. FAT (the ruled crate; `file_allocation_table` today).
2. `paging` to `x86_64` and `aarch64-paging`.
3. `globally_unique_identifier_partition_table` and `universally_unique_identifier`.
4. `elf`, then `network_time_protocol`, `device_tree_blob`, `glob`, `domain_name_system`.
5. `calendar`, revisited under fork 8, since `time` wants a person's words.
6. `non_volatile_memory_express`, `extensible_host_controller_interface` (rust-osdev asked first).
7. `e1000e`, the DesignWare pair and the JH7110 pair, after their cuts.
8. The internal set, `redoxfs`'s bug report question among them.

## What is already on the record

Milestone 39 (repository structure for a loosely-coupled OS) answered a narrower form of this on
2026-07-31. It found that `cargo publish -p` works from a workspace, so publishing needs no split. It
set a trigger: publish a crate only after it has an in-tree consumer, because the first caller finds
the API's shape wrong. And it framed publication as evidence for the verified-Rust claim, not as a
product line. Three things have moved since:

- calef now wants release as a goal. §151 (the goal of the repository split is independent release)
  already says a piece earns separation when it can ship on its own schedule.
- The trigger is met. Every one of the 30 crates classed A or B has at least one consumer in this
  tree, counted from the `Cargo.toml` files.
- §46 (thin primitives or whole subsystems) says take first outside the kernel and the proven
  crates. That rule cuts both ways here. Where a maintained crate exists, it argues
  for offering our proofs to it rather than publishing a rival.

And one thing has not moved. §235 (the OS is built and updated from packages, and the tree divides
by what releases together) was amended to size a home by what changes together. Under it, no package
moves to a new repository before milestone 610 (the interface's stability is measured weekly)
carries its per-package co-change column. Fork 1 has to respect that.

The survey shrank the question. Of the 31 candidates, eight are "publish ours": the FAT crate, six
device-logic crates (NVMe, e1000e, two DesignWare controllers, two JH7110 blocks), and xHCI if
rust-osdev declines. `portable_executable` is not published: milestone 821 (`portable_executable`:
an ELF-to-PE converter, proven, then released on its own) is refused. Eleven outside projects are
candidates for our proofs. The rest stay internal.

## Fork 1. Where a released crate lives (ruled: one repository per crate)

| option | shape | prior art | cost |
|---|---|---|---|
| R-a. Publish from this repository | Each crate keeps its path; `cargo publish -p` from a workflow here | `wasmtime` publishes its `crates/` directory (38 entries) from one repository | A stranger's issue lands in an OS tracker. No move, so §235 holds |
| R-b. One repository for every released crate | `nifeos/<name>`, one workspace | google/gpt-disk-rs holds three crates | A move before 610's column exists |
| R-c. One repository per crate | `nifeos/<crate>` each | rust-osdev (`x86_64`, `uefi-rs`), rust-embedded-community | Nine repositories, each with its own CI, and two pull requests whenever a driver and its crate move together |
| R-d. One repository per family | Two today: device logic (seven crates) and formats (FAT, PE) | RustCrypto: `hashes` holds 23 crates | A move before 610's column exists; the family lines are a guess until it does |

Recommendation as raised, replaced by the ruling: R-a now, R-d as the destination once 610 measures co-change. The reason is
§235's rule, not effort. R-d is the shape I would pick at equal cost, since a family is what
changes together, and R-c multiplies the two-pull-request cost §235 already refused. Reversible
until a stranger depends on the repository URL.

Prior art beyond the table. seL4 keeps its proofs in a repository apart from the kernel (`l4v`,
recalled rather than re-read), and its Rust crates are consumed by git, not from crates.io. The
rust-embedded working group gives each crate its own repository and owns them through GitHub teams.
Fuchsia vendors third-party crates in its own tree and publishes few of its own (recalled). None of
them publishes proofs beside the crate, which is the part this tree would add.

## Fork 2. How nife consumes a crate that has left (ruled: C-a, `[patch]` only in development)

| option | prior art | cost |
|---|---|---|
| C-a. crates.io version, pinned in `Cargo.lock` | RustCrypto, rust-osdev | `cargo-deny` and advisories work against it, which is the reason §46's 2026-07-31 amendment prefers registry crates |
| C-b. git dependency at a commit | seL4's `rust-sel4`, whose README says its crates are not on crates.io | A crates.io crate cannot depend on a git crate, so nobody can publish on top of it |
| C-c. path, as today | this tree | Only while the crate is in this repository |

Recommendation as raised, and ruled with a `[patch]` rule added: C-c while a crate is here, C-a after. C-b would block exactly the third parties §151
names as the goal. §305 (git dependencies on `nifeos/*` only) takes C-b for the soak before a
first publish.

## Fork 3. Versions, and what a release carries (ruled as recommended)

Recommendation as raised, and ruled:

- Each crate carries its own semver version and starts at 0.1. Nothing moves in lockstep.
- Each crate states a minimum Rust version. Measured 2026-10-07: all 30 build on stable 1.94.1,
  for the host and for `riscv64imac-unknown-none-elf`. Nightly is this tree's need, not theirs.
- A release ships its Kani harnesses (behind `#[cfg(kani)]`, as now) and its `falsifications/`
  patches. A proof a stranger cannot rerun is a claim, and the patches show the proofs can fail.
- `publish = false` becomes the workspace default, and a lint check allows publishing only for
  crates on a list calef ratifies. Today 9 of 116 manifests say `publish = false`, so a stray
  `cargo publish` with a token would succeed for the rest. That is rung 2 of the ladder in place of
  rung zero.

## Fork 4. How proofs run where the crate lives (ruled as recommended)

- P-a. In this repository, `script/verify` and the falsification workflow as today.
- P-b. In a released repository, `model-checking/kani-github-action`, which Kani's maintainers
  publish, plus a job that applies each falsification patch and expects Kani to fail.
- P-c. Proofs stay here and the released crate carries none.

Recommendation as raised, and ruled: P-a while a crate is here, P-b after a move. P-c is refused: it publishes the code without
the evidence, and the evidence is milestone 39's reason to publish at all.

## Fork 5. The crates.io names (ruled: names that stand on their own)

Options only, because a crates.io name cannot be taken back: a crate can be yanked but its name
stays. Each name is ratified under naming-authority and `script/names` before the crate leaves the
tree.

- N-a. This tree's names, prefixed only where taken. Free today: `device_tree_blob`,
  `globally_unique_identifier_partition_table`, `network_time_protocol` and most of the long names.
  Taken: `calendar`, `elf`, `glob`, `paging`, `pci`, `usb`, `http_response`.
- N-b. One prefix for all, as `sel4-*`, `cranelift-*` and `embassy-*` do. It says who stands behind
  the proofs. It also says "nife" to a reader who wanted a GPT parser.
- N-c. A short name chosen per crate, as RustCrypto does (`sha2`, `aes-gcm`). Readable, and every
  one is a separate naming decision.

## Fork 6. Who publishes (ruled: W-a)

- W-a. crates.io trusted publishing from a `nifeos` workflow (RFC 3691; the
  `rust-lang/crates-io-auth-action` action exists since 2025-06-26). Short-lived tokens, no secret
  stored, and the owner is a `nifeos` GitHub team.
- W-b. calef's own token, by hand.

Recommendation as raised, and ruled: W-a. It is the same reasoning §220 (signed builds) applied to keys: no
long-lived secret where a workflow can mint one. Nothing is published until calef approves that
crate's first release.

## Fork 7. Each crate's disposition (ruled: one crate at a time)

The note's table classes every A and B crate as contribute upstream, publish ours, or keep internal,
with the evidence for each. The ask is to approve the classes, not to send anything. Every upstream
approach and every first publication then comes to calef one at a time, with its text, as the
upstream-hardening ruling of 2026-10-06 requires. A security finding in another project goes to
its maintainers privately first.

Recommendation as raised, replaced by one review per crate: approve the table as it stands. Two rows carry the judgment most worth checking.
`calendar` goes to `time` rather than out as a rival, although milestone 39 called it a real gap;
`time` has 951M downloads and no proofs, so ours added there reach more callers. And
`cryptography_provider` stays internal, although no good alternative exists, because a second
alpha provider from a project with no security team is worse than none.

## Fork 8. Who writes an offer to another project (ruled: A-a without calef's hand, bug reports everywhere)

The survey found contribution policies, not licenses, to be the binding constraint. Redox and
embedded-sdmmc refuse AI-generated contributions. `time` takes AI-written code, disclosed, but wants
every message written by a person and no LLM as co-author. rcore-os's `tgoskits` asks commits to
carry no agent branding, where this tree requires an agent's pull request to say so.

- A-a. Approach only projects whose policy admits agent-written code with disclosure. calef writes
  the message where a project asks for a person's words, and the commit carries no co-author
  trailer where that is asked.
- A-b. calef writes and submits every offer himself, and an agent only prepares the patch.
- A-c. Offer findings only: an issue describing a bug and the harness that found it, never code.

Recommendation as raised, replaced by the ruling: A-a, with A-c for projects that refuse AI work. A refusal is the owner's call,
and a bug report is still welcome under every policy read. Each approach is a fact that leaves the
machine, so the policy is reversible and each use of it is not.

## What is blocked, and what is not

Nothing in the tree waits on these forks. The FAT crate is written in this repository and gets its
own repository at first publication, under Fork 1's ruling; its name is ratified then.

## BUGS

- Milestone 39 warned that a published proof is a promise to keep it passing across Kani versions.
  That obligation is real and is not costed here. The falsification workflow is the mechanism that
  notices; no one is assigned to answer it for a stranger.
- The survey's activity and acceptance figures were looked up, not measured, and age fast.
- Whether an outside project's code can be restructured for Kani was not tried. An offer can come
  back as "only if you rewrite our parser", which is §46's first rule seen from the other side.

## Index row

Crates are released on their own, and proofs are offered to the projects that already exist: one repository per crate, trusted publishing, harnesses and falsifications shipped inside each release, and each crate's disposition reviewed with calef one at a time. All eight forks ruled 2026-10-07 on #1806.
