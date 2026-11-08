---
status: PARTIAL
raised: 2026-10-07
milestone_dependencies: 887
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 815. `e1000e`: proven Intel e1000e ring logic, released on its own

*(From calef's rulings on pull request #1806; the title and slug are drafts. The FAT crate's block,
milestone 813 (a proven FAT crate released on its own), is the shape this follows.)*

calef's rulings, quoted from the maintainer comments on #1806 of 2026-10-07 (UTC):

- Policy: *"Each publish-ours crate (NVMe, e1000e, the DesignWare pair, the JH7110 pair, xHCI, and
  `portable_executable` once it carries proofs) is published when ready (proofs passing, API
  stable), each through its own milestone at its review."* His words: *"Since the consumer would be
  other hobby OSes, I think it makes sense to publish them. [...] a nice legacy is a bunch of hobby
  oses that use its parts."*
- Crates: *"one milestone per crate, minted now, each keeping its tree name (all free on crates.io
  as of today) [...] Names are ratified now and permanent on first publication."* His words: *"Yes,
  one per crate"*.
- The release rules are forks 1 to 4 and 6 of the same pull request, applied in *Release* below.

## What exists, checked 2026-10-07 (UTC)

- `nifeos/e1000e`: 2,151 code lines (3,210 with comments), written by milestone 494 (a driver for
  the network card a PC actually has), which is PARTIAL. It claims the 82574L that QEMU's
  `-device e1000e` presents and the Sunrise Point I219 rows, which is xenon's I219-LM5. Later PCH
  generations are refused until a bench proves one; the crate's `BUGS` says why.
- Three Kani harnesses, each with a falsification patch. A delivered frame always fits the buffer
  and the room. Every ring offset is inside the region, and a paged access selects the offset's own
  page and register. The harness that the handoff round-trips and accepts only what `pack` can make
  lives with `Handoff` in `driver_process_contract`.
- Class B in #1806's note `notes/releasable-crates-2026-10-07.md`. **The cut**: inline
  `address_space_map::pair_page`, its one use (`src/lib.rs`, in the handoff). That function places
  nife's virtual addresses, so the handoff it serves is nife's spawn ABI; whether the handoff stays
  in the published crate or moves to the driver is the lane's call, recorded in its pull request.
- Ruled 2026-10-25 (UTC): the crates take plain values; the device-tree walk moves to nife, and
  the knowledge of the vendor and mainline JH7110 trees stays in each crate as data and decoders.
- Cut on 2026-10-25 (UTC) by #2719. The three virtual addresses that `pair_page` placed moved to
  `crates/driver_process_contract`, which the kernel and the components link, and `Handoff`, its
  `pack` and its harness have since moved there too.
- Its consumers: `kernel/src/e1000e.rs` (reset, MAC address, ring bases), `kernel/src/pci.rs`,
  `kernel/src/user/e1000e_service.rs`, `kernel/src/network_bench.rs`, and in `components/`,
  `e1000e_transport.rs` and `net_stack.rs` (descriptors and tails).
- crates.io answers 404 for the name (checked 2026-10-07).

**Reuse:** the alternatives were surveyed in #1806's `other-implementations.md` appendix (looked up 2026-10-07),
and none is taken, because calef ruled this crate "publish ours". rcore-os's `eth-intel` 0.2.4
(MIT, 10.8k downloads, no proofs found) is a young driver inside the `tgoskits` monorepo.
elliott10's `e1000-driver` 0.1.0 (2023-02) is GPL-2.0 and drives the older e1000. Redox's `e1000d`
is a daemon whose GitHub mirror is archived. Linux's `e1000e` and FreeBSD's `em` stay the references
for behavior and are read, never copied. No upstream offer goes anywhere: calef ruled on #1806 that
nife contributes only to projects nife depends on.

## What is proven

Each row is a property this crate must prove before it publishes. The harness cell names the Kani
harness that proves it, or says `none yet`; every named harness carries a falsification patch that
makes it fail at its own assertion. Each patch is under `falsifications/` in that repository. All
three prove.

| property | harness |
|---|---|
| A delivered frame always fits the buffer and the room | `proofs::a_delivered_frame_always_fits_the_buffer_and_the_room` |
| Every ring offset is inside the region | `proofs::every_ring_offset_is_inside_the_region` |
| A paged PHY access selects the offset's own page and register | `platform_controller_hub::physical_layer_device::proofs::a_paged_access_selects_the_offsets_own_page_and_register` |

Not proven: the NVM (flash) reads that yield the MAC address and pick a bank, the reset, ULP and
PHY workaround sequences (ordered register writes, tested against the simulated device but not
model checked), and transmit descriptor status. The NVM read is device-supplied input and is the
first candidate for a fifth row.

## Release, by #1806's rulings

1. The name is settled before anything leaves (857's Fork 5 as calef amended it on 2026-10-11 UTC).
   `script/names e1000e` reports it ratified, crates.io answers 404 for it (or it is already
   ours), and `nifeos/e1000e` does not exist yet. `script/crate-standalone` checks the first.
2. Make the cut above. `cargo tree -p e1000e` then lists no nife crate.
3. In the tree, `script/verify` proves the harnesses and `Cargo.toml` says `publish = false`.
4. The package is 0.1.0 with its own semver, states `rust-version` (#1806's note built it on stable
   1.94.1), and ships its harnesses and falsification patches. `cargo package --list` shows them.
5. Ready means proofs passing and the API stable. §235 (the OS is built and updated from packages,
   and its source divides by purpose) has milestone 610 (the interface's stability is
   measured weekly)'s per-package co-change column, and the soak (step 6) is how the second is
   measured. When the soak exits, calef adds it to the lint-checked publish allowlist. The
   `nifeos/e1000e` repository exists since the crate moved, on the shared CI template (milestone
   887), through reusable workflows: Kani through `kani-github-action`, lint, release. Its release
   workflow refuses to publish unless the proof job is green. No harness here reads `target_arch`,
   so stock Kani proves them. The patched Kani of §218 (carry a Kani patch so riscv64 is proved, and
   send it upstream) is carried only if one comes to need riscv64. Milestone 589 (Kani can prove
   riscv64 from the hosts we already have) built it.
6. Soak, as its own repository (calef, 2026-10-17 UTC, replying "Agrred" (sic) to the maintainer's
   proposal; §305 (git dependencies are allowed on `nifeos/*` only, pinned to a landed commit)).
   Milestone 818 (`jh7110_entropy`: proven JH7110 TRNG logic, released on its own) is the pilot and
   soaks first. Before publication the crate is moved, not copied: it leaves this tree, and nife
   depends on `nifeos/e1000e` by `rev`. A copy left behind does not count. The soak ends on
   evidence, not a duration, when all five hold:
   1. nife builds and passes CI against the repository at a pinned commit, with no path dependency
      left.
   2. The shared template's weekly run (milestone 887) is green on the crate.
   3. A release dry run is green.
   4. The stranger test builds and proves the crate from its README alone.
   5. The API has gone four weeks with no change `cargo-semver-checks` calls breaking.
7. crates.io trusted publishing from that workflow, owned by a `nifeos` team, with no stored token.
8. calef approves this publication, as he does every one.
9. nife consumes the crates.io release by semantic version, and `Cargo.lock` records its checksum. A `[patch]` to
   a checkout is allowed only during development, and a lint fails one that reaches `main`. The
   soak's git dependency goes.

## Parity

Pure logic, tested on the host against a simulated device. This lane built it on 2026-10-07 with
the pinned nightly for `aarch64-unknown-none-softfloat`, `riscv64imac-unknown-none-elf`,
`x86_64-unknown-none` and the host, all exit 0. The release workflow builds the same four.

## Standalone, measured 2026-10-11 (UTC)

`script/crate-standalone e1000e` fails today, at base `ae6b91e79`. The manifest inherits `license`
and `lints` from nife's workspace and states no `rust-version`, and there is no README,
`LICENSE-MIT` or `LICENSE-APACHE`. Those hold for all nine crates. It does not build alone:
`address_space_map` is a path dependency outside the copy, so neither the build nor Kani ran.
Release step 2's cut is what fixes that. Beyond manifest and files, 23 lines cite this tree: 12
`milestone N`, 5 tree paths, 2 `§N`, 2 naming calef and 2 provenance blocks.

`script/names` reports the name `e1000e` ratified (checked 2026-10-25).

Measured again on 2026-10-25 (UTC) with `--no-kani`: the manifest and files pass, and the only
citations left sit in `Name:` blocks, which stay until the ratification pass. `address_space_map` is
still a path dependency, so the copy does not build; the second half of the cut removes it.

Measured again after the cut on 2026-10-25 (UTC), with Kani: the manifest has no dependencies, the
crate builds for the host and the three targets on 1.88. Stock Kani proves all three harnesses, and
each falsification patch makes its harness fail. The only failures left are citations in `Name:`
blocks, which the ratification pass settles.

## Review before release

- The public API speaks the device's vocabulary, not nife's: no nife capability type, spawn handoff
  or nife error enum in a public signature. A reviewer judges this; no script can. Known today:
  `Handoff`, nife's spawn ABI, no longer sits in the crate. The addresses taken from `address_space_map` left with the cut.

## Names a stranger may misread, open before release

design/naming.md's tests and Fork 5 of milestone 857 (names that stand on their own) judge
each name. The review package, [notes/crate-reviews/e1000e.md](../../notes/crate-reviews/e1000e.md),
lists the names still open: five ratified before the rule of 2026-11-02 that the architect reopens
at the release review, and the ones nobody has ratified.

## Exit

A check proves it. In nife, `cargo tree --locked -i e1000e` resolves to `v0.1.x` from crates.io with
a checksum in `Cargo.lock`, `crates/e1000e` no longer exists, and the tree builds for all three
targets. In `nifeos/e1000e`, the run that published 0.1.0 shows the Kani job green ahead of the
publish job, and every harness's falsification patch makes its proof fail. The five soak criteria of
release step 6 hold before it publishes.

The name check of release step 1 passes first, so the repository, the package and every doc link
carry a ratified name. Before the move, `script/crate-standalone e1000e` exits 0 in nife, and every
row of *What is proven* names a harness that stock Kani proves in the copy and whose falsification
patch makes it fail. No crate publishes without Kani harnesses covering its hostile-input and
state-machine surface, each carrying a falsification patch that makes it fail at its own assertion,
replayable in the crate's own repository with stock Kani.

## What the crate no longer cites

The crate's source cites nothing of this tree beyond its `Name:` blocks, which stay at their
names until the ratification pass, so this section keeps the links it carried.

- `src/lib.rs`, the crate doc: milestone 494 (a driver for the network card a PC actually has)
  wrote the crate. Its design is `notes/e1000e.md`, and `notes/e1000e/the-i219-port.md` says where
  the register facts come from.
- `Cargo.toml`, the old manifest comment: the crate is a pure-logic crate under §7 (the testing
  harness), no_std for the kernel and `net_stack` and std for host tests.
- `src/descriptor.rs`, `receive_verdict`: trusting no field of the status word answers finding 6 of
  `notes/shared-page-audit.md`.
- `src/platform_controller_hub/mod.rs`, `non_volatile_memory.rs`, `physical_layer_device.rs` and `sequence.rs`, the
  module docs: each cited milestone 494 and `notes/e1000e/the-i219-port.md`, the whole note for
  `platform_controller_hub` and its sections "The NVM", "The PHY layer" and "The three passes" for the others. The PHY
  layer's two divergences now stand in `physical_layer_device.rs` itself.
- `src/platform_controller_hub/mod.rs`, the first BUGS entry: the run on silicon without a cable is in that note's
  "BUGS", which links the BUGS of `notes/e1000e.md`.

## The soak

- The crate lives in `nifeos/e1000e`, on the shared template of milestone 887 (crate repositories
  share one CI template) with its callers pinned at `nifeos/crate-workflows` `e0c1fca`.
  `script/apply-settings` applied the settings there: both rulesets, the `crates-io` environment
  with calef as reviewer, the labels, the team and private vulnerability reporting. The manifest
  keeps `publish = false`, and the caller sets `allow-unpublished` for the soak. No tag exists.
- nife keeps no copy. `kernel`, `components` and `system_tests` depend on `nifeos/e1000e` by `rev`
  (§305 (git dependencies are allowed on `nifeos/*` only, pinned to a landed commit)), at the root
  commit of its `main`, because no later commit has landed. The verify table, the Kani peaks table
  and the mutation exclusions no longer list it; the repository's own `proofs / kani` and
  `proofs / falsify` jobs prove the three harnesses. The `drivers` package no longer lists it as
  an interface crate.
- End condition 1 holds once the repository's `main` is green at the pinned commit. Condition 2
  waits on the first weekly run, 3 on a release dry run, 4 on a recorded stranger run, and 5 on
  four weeks from the tag nife pins.
- A bump of the pin is a separate pull request citing the landed commit as `nifeos/e1000e@<sha>`.

## What remains

1. The register naming rules, so that `check / standalone` passes (see *BUGS*).
2. The tag `v0.1.0`, which the release workflow makes when calef dispatches it, and the pin moved
   to that tag's commit.
3. The soak, which ends on evidence and not a duration, as in milestone 818 (`jh7110_entropy`:
   proven JH7110 TRNG logic, released on its own).
4. The publication of 0.1.0 by trusted publishing, which waits for the architect's approval.
5. nife consuming the release by semantic version, with `Cargo.lock` recording its checksum.

## BUGS

- `check / standalone` in `nifeos/e1000e` fails the template's register naming gate
  (`register_names.py`, ceiling 0) on 103 names. They are register offsets without the
  `_REGISTER` ending, bit constants written as values and not as shifts, addresses without
  `_VIRTUAL_ADDRESS` or `_PHYSICAL_ADDRESS`, and sizes without a unit. Many of them carry ratified names, so
  the repository's `main` cannot go green, and the repository's ruleset cannot merge a pull
  request, until the architect rules on them. The rest of the gates pass.
- Nobody has installed the `nife-smelter` app on `nifeos/e1000e`, and the repository holds none of the
  organization secrets `NIFE_SMELTER_APP_ID`, `NIFE_SMELTER_PRIVATE_KEY` and `SETTINGS_TOKEN`.
  `script/apply-settings` refused the app (the API wants an organization owner). The release
  workflow's tag job and the weekly settings-drift job need them, so both wait on calef.
- The mutation census lists 72 missed mutants and no locations, and the exclusions for mutants that
  cannot differ (an OR of operands that share no bit) did not travel. The first weekly mutation run
  in `nifeos/e1000e` names each survivor, and each goes into `.github/mutants-exempts` there with
  its reason.

- Steps 5 and 6 wait on milestone 887 (crate repositories share one CI template),
  which owns the shared CI template, the publish allowlist lint and the `nifeos` crates.io team,
  and on milestone 610's per-package co-change column.
- Milestone 494 is still PARTIAL, so the API may still move. Semver 0.x allows that; publishing
  before 494 lands costs only version churn.
- `needs_person` is `yes` because the exit includes a publication only calef can approve. Everything
  before step 7 can start now.

## Follow-on

- **Outstanding.** The register naming ruling, the pin moved to a green commit, the tag `v0.1.0`,
  the soak's end conditions 2 to 5 and the publication. Checked by `gh run list -R nifeos/e1000e
  --workflow ci.yml`, which shows `check` red, and `gh api repos/nifeos/e1000e/tags`, which lists no
  tag.
- **Recorded.** An organization owner installs the `nife-smelter` app and grants the three secrets;
  see *BUGS* above.

## Index row

nife's Intel e1000e ring and register logic, with three Kani proofs and their falsification patches,
cut free of nife's address map. It is published on crates.io as `e1000e` 0.1 from its own `nifeos`
repository, and nife consumes that release.
