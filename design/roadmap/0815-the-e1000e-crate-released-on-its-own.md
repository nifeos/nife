---
status: NOT-STARTED
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

- `crates/e1000e`: 2,151 code lines (3,210 with comments), written by milestone 494 (a driver for
  the network card a PC actually has), which is PARTIAL. It claims the 82574L that QEMU's
  `-device e1000e` presents and the Sunrise Point I219 rows, which is xenon's I219-LM5. Later PCH
  generations are refused until a bench proves one; the crate's `BUGS` says why.
- Four Kani harnesses, each with a falsification patch. A delivered frame always fits the buffer
  and the room. The handoff round-trips and accepts only what `pack` can make. Every ring offset is
  inside the region, and a paged access selects the offset's own page and register.
- Class B in #1806's note `notes/releasable-crates-2026-10-07.md`. **The cut**: inline
  `address_space_map::pair_page`, its one use (`src/lib.rs`, in the handoff). That function places
  nife's virtual addresses, so the handoff it serves is nife's spawn ABI; whether the handoff stays
  in the published crate or moves to the driver is the lane's call, recorded in its pull request.
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
makes it fail at its own assertion. Each patch is at `crates/e1000e/falsifications/`. Measured
2026-10-11 (UTC): 4 of 4 proven.

| property | harness |
|---|---|
| A delivered frame always fits the buffer and the room | `proofs::a_delivered_frame_always_fits_the_buffer_and_the_room` |
| The handoff round-trips and accepts only what `pack` can make | `proofs::the_handoff_round_trips_and_accepts_only_what_pack_can_make` |
| Every ring offset is inside the region | `proofs::every_ring_offset_is_inside_the_region` |
| A paged PHY access selects the offset's own page and register | `pch::phy::proofs::a_paged_access_selects_the_offsets_own_page_and_register` |

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
   and the tree divides by what releases together) has milestone 610 (the interface's stability is
   measured weekly)'s per-package co-change column, and the soak (step 6) is how the second is
   measured. When the soak exits, calef adds it to the lint-checked publish allowlist. The
   `nifeos/e1000e` repository is created when the crate moves, on the shared CI template (milestone
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
   5. The API has gone a set period with no change `cargo-semver-checks` calls breaking. The period
      is not yet set.
7. crates.io trusted publishing from that workflow, owned by a `nifeos` team, with no stored token.
8. calef approves this publication, as he does every one.
9. nife consumes the crates.io release, pinned with its checksum in `Cargo.lock`. A `[patch]` to
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

Name: `e1000e` is ratified today (`script/names`, checked 2026-10-11).

## Review before release

- The public API speaks the device's vocabulary, not nife's: no nife capability type, spawn handoff
  or nife error enum in a public signature. A reviewer judges this; no script can. Known today:
  `Handoff` and the addresses taken from `address_space_map` are nife's spawn ABI and address map.

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

## BUGS

- Steps 5 and 6 wait on milestone 887 (crate repositories share one CI template),
  which owns the shared CI template, the publish allowlist lint and the `nifeos` crates.io team,
  and on milestone 610's per-package co-change column.
- Milestone 494 is still PARTIAL, so the API may still move. Semver 0.x allows that; publishing
  before 494 lands costs only version churn.
- `needs_person` is `yes` because the exit includes a publication only calef can approve. Everything
  before step 7 can start now.

## Index row

nife's Intel e1000e ring and register logic, with four Kani proofs and their falsification patches,
cut free of nife's address map. It is published on crates.io as `e1000e` 0.1 from its own `nifeos`
repository, and nife consumes that release.
