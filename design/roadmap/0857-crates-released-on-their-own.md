---
status: NOT-STARTED
raised: 2026-10-07
promoted_from: crates-released-on-their-own
milestone_dependencies: none
decision_dependencies: none
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
session, relayed in comments on pull request #1806. Each fork's options, as weighed, are in
[the forks and their options](0857-crates-released-on-their-own/the-forks-and-their-options.md).

- Fork 1, replacing the recommendation: one repository per crate under `nifeos`, for a crate that
  would publish on its own or that several repositories consume. Any other crate leaves in its
  package's repository (§316 (a program package leaves as one repository), 2026-11-04 UTC). Ruled 2026-11-01 (UTC),
  replacing creation at first publication: a crate gets its repository when it moves, and each
  creation still waits for calef's approval. Until it moves, a crate stays in this tree. Every crate repository uses one shared
  CI template (Kani, lint, release) through reusable workflows. calef: *"I lean one repo per
  crate."* then *"Yes a"*.
- Fork 2: a published crate is consumed as a crates.io release pinned with its checksum in
  `Cargo.lock`. A `[patch]` to a local checkout or branch is allowed only during development, and a
  lint fails if one reaches `main`. calef: *"Ratify fork two"*.
- Fork 3: its own semantic version from 0.1, a stated minimum Rust version, and its Kani harnesses
  and falsification patches shipped inside it. Every crate carries `publish = false` unless it is on
  a lint-checked allowlist calef approves. calef: *"Yes"*.
- Fork 4: `script/verify` proves a crate while it is here. After it moves, the shared template runs
  `kani-github-action`, and the release workflow refuses to publish unless the proof job is green.
  calef: *"Yes"*.
- Fork 5, names that stand on their own: no `nife` prefix. A crate keeps this tree's name where it
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
  offer. Internal is at most a short-lived state, not a standing class (ruled 2026-11-01 UTC), and
  [the survey note](../../notes/releasable-crates-2026-10-07.md) reclassifies every row. calef: *"I think we need
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

- Ruled 2026-10-25 (UTC): the crates take plain values (a register base and size, and a small
  settings type), and the device-tree walk moves to nife. The knowledge of the vendor and mainline
  JH7110 trees (compatible strings, node names, property differences) stays in each crate as data
  and decoders, documented in its README. nife's driver-process layout and block server leave the
  device crates for the programs that drive them. The walks live in `machine_discovery::jh7110`,
  so `jh7110_entropy` and `jh7110_clock_and_reset` became package interfaces.
- Ruled 2026-10-26 (UTC): `driver_process_contract` stays one crate, one module per driver. A
  crate per driver would add names and make no program link less, since every driver is a binary
  of `components`; revisit if that changes. The name holds for now. The crate breaks up as
  components become programs and crates of their own, and each piece takes its name then.
- Ruled 2026-11-01 (UTC): this repository breaks up into single-purpose repositories and remains,
  as `nifeos/nife`, only for its history. The design record (roadmap, decisions,
  journeys, naming, notes) moves to its own repository, `nifeos/record`, which the site published
  at nifeos.org pulls. The kernel, programs, tooling and crates all leave, and a crate consumable
  outside nife shows good crate boundaries. What leaves references nothing back here, so moves go
  bottom-up: the crates a program links leave before it or with it. The kernel leaves last among
  the code, to `nife-kernel` (provisional): like Linux, a named part of the system, while the
  project name stays with the whole. Publish-ours
  crates publish; contract crates move and carry versions whether or not they publish. Milestone
  818 (proven JH7110 TRNG logic, released on its own) is the leaf pilot; pull request #3026
  rules the contract crates' order: bottom-up, one repository each. A crate nife does not publish, or
  has not yet, is a git dependency pinned by `rev` with its version requirement stated. The
  revision is the commit of a tagged release whose manifest carries that version; the tag is a
  label and the semver baseline, never what Cargo follows. At first publication the line becomes a
  plain version requirement, and each publication waits for the architect's review of the exact
  crate, version and name.

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
  falsifications. It runs while the crate is still here.
- Provenance. A crate's `Name:` blocks stay in its source until the breaking-out change strips
  them. Nothing replaces them; git history and the ratifying pull requests are the only record
  (ruled 2026-11-04 UTC). Each milestone's "What the crate no longer cites" keeps every other link.
- Gates. Milestone 887 (crate repositories share one CI template) adapts nife's gates row by row.
  It also dual licenses each crate as nife is: *"nife is MIT OR Apache-2.0, so we should dual
  license our crates as well."*

What nife does at each extraction is a checklist, built with the first (813):
[what-nife-does-at-each-extraction.md](0857-crates-released-on-their-own/what-nife-does-at-each-extraction.md).

Adoption is measured once the first crate publishes or the first offer goes out, and not before.
The measures: crates.io downloads over 90 days, reverse dependencies, and issues and pull requests
from outside `nifeos`. And for offers: made, accepted and declined, with bug reports filed and
fixed.

## The release order, ruled 2026-10-27 (UTC)

Publish-first, as risk 8's first attempts ([the entry](../fatal-risks/8-nobody-else-builds-on-nife.md),
which holds the crates.io figures). Contributions to other projects wait until a published crate
has users who ask for them upstream.

1. `file_allocation_table` (milestone 813 (`file_allocation_table`: a proven FAT crate, released on
   its own)).
2. `extensible_host_controller_interface`, released with `usb` (published as `universal_serial_bus`), after Kani proofs
   (milestone 820 (`extensible_host_controller_interface`: proven xHCI logic, released on its own)).
3. `generational_table`, on its own, once the release record says whether `slotmap` already covers
   allocation-free fixed capacity.

`capability` and `non_volatile_memory_express` are not first attempts. The rest follow in a
reversible order, and each contribution waits on the ruling above.

4. `paging`, `globally_unique_identifier_partition_table`, `elf`
   ([deferred](0857-crates-released-on-their-own/contribute-proofs-instead-of-releasing.md)),
   `network_time_protocol`, `device_tree_blob`, `glob`, `domain_name_system`.
5. `calendar`, revisited under fork 8, since `time` wants a person's words.
6. `non_volatile_memory_express`, `e1000e`, the DesignWare pair and the JH7110 pair, after their cuts.
7. The internal set, closing as one sitting. Withdrawn 2026-11-01 (UTC).

Ruled 2026-11-01 (UTC): items 4 and 5 move in pull request #3026's bottom-up order, each in its own repository, and the wait
above decides only whether each also publishes or goes upstream.

## What is already on the record

Milestone 39 (repository structure for a loosely-coupled OS) and three decisions framed this
question first; [what-was-already-on-the-record.md](0857-crates-released-on-their-own/what-was-already-on-the-record.md)
says what each settled and what has moved since.

The survey shrank the question. Of the 31 candidates, eight are "publish ours": the FAT crate, six
device-logic crates (NVMe, e1000e, two DesignWare controllers, two JH7110 blocks), and xHCI. Eleven
outside projects are candidates for our proofs. The survey classed the rest internal, no standing class since
2026-11-01.

## BUGS

- Milestone 39 warned that a published proof is a promise to keep it passing across Kani versions.
  That obligation is real and is not costed here. The falsification workflow is the mechanism that
  notices; no one is assigned to answer it for a stranger.
- Whether an outside project's code can be restructured for Kani was not tried. An offer can come
  back as "only if you rewrite our parser", which is §46's first rule seen from the other side.

## Index row

Crates are released on their own, and proofs are offered to the projects that already exist: one repository per crate, trusted publishing, harnesses and falsifications shipped inside each release, and each crate's disposition reviewed with calef one at a time. All eight forks ruled 2026-10-07 on #1806.
