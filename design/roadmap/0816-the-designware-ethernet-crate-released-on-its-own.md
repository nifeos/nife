---
status: NOT-STARTED
raised: 2026-10-07
milestone_dependencies: 887
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 816. `designware_ethernet`: proven DesignWare Ethernet QoS logic, released on its own

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

- `crates/designware_ethernet`: 2,513 code lines (3,596 with comments), written by milestone 53
  (the board's own peripherals: network and storage on real silicon), which is PARTIAL. It drives
  the Synopsys Ethernet QoS controller (`snps,dwmac-5.20`), the JH7110's two ports behind Motorcomm
  YT8531 PHYs on radon. The 4.x and 5.x descriptor format only; the 3.x GMAC that milestone 655 (DMA
  on a non-coherent RISC-V machine) names for the TH1520 would not fit.
- Two Kani harnesses, each with a falsification patch: a delivered frame always fits the buffer and
  the room, and every ring offset is inside the region.
- Class B in #1806's note `notes/releasable-crates-2026-10-07.md`. **The cut**, which is codebase
  rule 2 (a driver gets what it needs passed in): inline `address_space_map::pair_page`
  (`src/lib.rs`), and have the caller pass the register region instead of `jh7110::discover` and
  `discover_aon_syscon` reading a `DeviceTreeBlob`. The device-tree queries move to the kernel side,
  which already depends on `device_tree_blob`. The tests also `include_bytes!` a fixture from
  `../../device_tree_blob/tests/fixtures/`, outside the package, which `cargo package` cannot ship.
- Ruled 2026-10-25 (UTC): the crates take plain values; the device-tree walk moves to nife, and
  the knowledge of the vendor and mainline JH7110 trees stays in each crate as data and decoders.
- Cut on 2026-10-25 (UTC) by #2719. The crate names the compatible strings and node names, and
  `Discovered::from_node`, `mainline_phy` and `vendor_phy` decode a node's properties. The walk
  is `machine_discovery::jh7110::designware_ethernet`, and the fixtures moved with its tests. The
  two virtual addresses that `pair_page` placed moved to `crates/driver_process_contract`.
- Its consumers: `kernel/src/designware_ethernet.rs` (clocks, reset, PHY, coherence, rings),
  `kernel/src/user/designware_ethernet_service.rs`, `kernel/src/memory.rs`,
  `kernel/src/network_bench.rs`, and in `components/`, `designware_ethernet_transport.rs` and
  `net_stack.rs`.
- crates.io answers 404 for the name (checked 2026-10-07).

**Reuse:** the alternatives were surveyed in #1806's `other-implementations.md` appendix (looked up 2026-10-07),
and none is taken, because calef ruled this crate "publish ours". elliott10's `dwmac-my` 0.2.0
(2026-02, MIT) is the one Rust DWMAC crate on crates.io, with 35 downloads, no reverse dependencies
and no proofs found. Linux's `stmmac` (GPL-2.0) stays the reference for behavior and is read, never
copied. No upstream offer goes anywhere: calef ruled on #1806 that nife contributes only to projects
nife depends on.

## What is proven

Each row is a property this crate must prove before it publishes. The harness cell names the Kani
harness that proves it, or says `none yet`; every named harness carries a falsification patch that
makes it fail at its own assertion. Each patch is at `crates/designware_ethernet/falsifications/`.
Measured 2026-10-11 (UTC): 2 of 3 proven.

| property | harness |
|---|---|
| A delivered frame always fits the buffer and the room | `proofs::a_delivered_frame_always_fits_the_buffer_and_the_room` |
| Every ring offset is inside the region | `proofs::every_ring_offset_is_inside_the_region` |
| The PHY link decode (`motorcomm::link`) is total: any register answer yields a link or none, never a panic | none yet |

The third row is this lane's answer to the question `BUGS` asks below: the PHY's answers are
device input and the link decode is the state a stranger's driver acts on. Not proven: MDIO
sequencing (`mdio`), the PHY configuration writes, the DMA-coherence probe's verdict (`coherence`)
and controller bring-up.

## Release, by #1806's rulings

1. The name is settled before anything leaves (857's Fork 5 as calef amended it on 2026-10-11 UTC).
   `script/names designware_ethernet` reports it ratified, crates.io answers 404 for it (or it is
   already ours), and `nifeos/designware_ethernet` does not exist yet. `script/crate-standalone`
   checks the first.
2. Make the cut above. `cargo tree -p designware_ethernet` then lists no nife crate, and the tests
   read only fixtures inside the package.
3. In the tree, `script/verify` proves the harnesses and `Cargo.toml` says `publish = false`.
4. The package is 0.1.0 with its own semver, states `rust-version` (#1806's note built it on stable
   1.94.1), and ships its harnesses and falsification patches. `cargo package --list` shows them.
5. Ready means proofs passing and the API stable. §235 (the OS is built and updated from packages,
   and the tree divides by what releases together) has milestone 610 (the interface's stability is
   measured weekly)'s per-package co-change column, and the soak (step 6) is how the second is
   measured. When the soak exits, calef adds it to the lint-checked publish allowlist. The
   `nifeos/designware_ethernet` repository is created when the crate moves, on the shared CI
   template (milestone 887), through reusable workflows: Kani through `kani-github-action`, lint,
   release. Its release workflow refuses to publish unless the proof job is green. No harness here
   reads `target_arch`, so stock Kani proves them. The patched Kani of §218 (carry a Kani patch so
   riscv64 is proved, and send it upstream) is carried only if one comes to need riscv64. Milestone
   589 (Kani can prove riscv64 from the hosts we already have) built it.
6. Soak, as its own repository (calef, 2026-10-17 UTC, replying "Agrred" (sic) to the maintainer's
   proposal; §305 (git dependencies are allowed on `nifeos/*` only, pinned to a landed commit)).
   Milestone 818 (`jh7110_entropy`: proven JH7110 TRNG logic, released on its own) is the pilot and
   soaks first. Before publication the crate is moved, not copied: it leaves this tree, and nife
   depends on `nifeos/designware_ethernet` by `rev`. A copy left behind does not count. The soak
   ends on evidence, not a duration, when all five hold:
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

Pure logic, tested on the host against a simulated controller. This lane built it on 2026-10-07
with the pinned nightly for `aarch64-unknown-none-softfloat`, `riscv64imac-unknown-none-elf`,
`x86_64-unknown-none` and the host, all exit 0. The release workflow builds the same four. The
silicon is riscv64 only; the crate is not.

## Standalone, measured 2026-10-11 (UTC)

`script/crate-standalone designware_ethernet` fails today, at base `ae6b91e79`. The manifest
inherits `license` and `lints` from nife's workspace and states no `rust-version`, and there is no
README, `LICENSE-MIT` or `LICENSE-APACHE`. Those hold for all nine crates. It does not build alone:
`address_space_map` and `device_tree_blob` are path dependencies outside the copy, so neither the
build nor Kani ran. A test reads `../../device_tree_blob/tests/fixtures/qemu-riscv64-virt.dtb`,
which `cargo package` cannot ship. Beyond manifest and files, 18 lines cite this tree: 8 `milestone
N`, 5 tree paths, 2 `§N`, 2 provenance blocks and 1 naming calef.

`script/names` reports the name `designware_ethernet` ratified (checked 2026-10-25).

Measured again on 2026-10-25 (UTC) with `--no-kani`: the manifest and files pass, and the only
citations left sit in `Name:` blocks, which stay until the ratification pass. `address_space_map`
and `device_tree_blob` are still path dependencies, so the copy does not build; the second half of
the cut removes them.

Measured again after the cut on 2026-10-25 (UTC), with Kani: the manifest has no dependencies, the
crate builds for the host and the three targets on 1.88. Stock Kani proves both harnesses, and each
falsification patch makes its harness fail. The only failures left are citations in `Name:` blocks,
which the ratification pass settles.

## Review before release

- The public API speaks the device's vocabulary, not nife's: no nife capability type, spawn handoff
  or nife error enum in a public signature. A reviewer judges this; no script can. Known today:
  `Handoff` is nife's spawn ABI. The cut removed `jh7110::discover`, which took a `DeviceTreeBlob`.

## Exit

A check proves it. In nife, `cargo tree --locked -i designware_ethernet` resolves to `v0.1.x` from
crates.io with a checksum in `Cargo.lock`, `crates/designware_ethernet` no longer exists, and the
tree builds for all three targets. In `nifeos/designware_ethernet`, the run that published 0.1.0
shows the Kani job green ahead of the publish job, and every harness's falsification patch makes its
proof fail. The five soak criteria of release step 6 hold before it publishes.

The name check of release step 1 passes first, so the repository, the package and every doc link
carry a ratified name. Before the move, `script/crate-standalone designware_ethernet` exits 0 in
nife, and every row of *What is proven* names a harness that stock Kani proves in the copy and whose
falsification patch makes it fail. No crate publishes without Kani harnesses covering its
hostile-input and state-machine surface, each carrying a falsification patch that makes it fail at
its own assertion, replayable in the crate's own repository with stock Kani.

## What the crate no longer cites

The crate's source cites nothing of this tree beyond its `Name:` blocks, which stay at their
names until the ratification pass, so this section keeps the links it carried.

- `src/lib.rs`, the crate root: the design lives in `notes/designware-ethernet.md`.
- `src/data_plane.rs`, `DataPlane`: `notes/designware-ethernet/the-hardware.md` explains the
  held-back receive slot.
- `src/regs.rs`, `src/jh7110.rs` and `src/motorcomm.rs`, each module's doc: the register pages,
  the two device trees and the PHY settings are in `notes/designware-ethernet/the-hardware.md`.
- `src/coherence.rs`, the module's doc: `notes/designware-ethernet.md`, "Is radon's DMA coherent?
  Yes, by measurement", records the probe reporting `Coherent` on radon over three boots on
  2026-10-20 (UTC).
- `src/descriptor.rs`, `rx_verdict`: the distrust of every descriptor field applies finding 6 of
  `notes/shared-page-audit.md` (two virtio drivers trusted the buffer index the device wrote) to a
  third NIC.
- `src/tests.rs`, the module's doc: lane maintainer/mutation-triage-2026w42 split the tests out of
  `lib.rs` on 2026-10-12 (UTC) under §282 (when a file is split, the parent becomes an index).
- `Cargo.toml`, the package comment: the pure-logic split is §7 (the testing harness)'s, and the
  crate's scope came from milestone 53 (the board's own peripherals: network and storage on real
  silicon).

## BUGS

- Steps 5 and 6 wait on milestone 887 (crate repositories share one CI template),
  which owns the shared CI template, the publish allowlist lint and the `nifeos` crates.io team,
  and on milestone 610's per-package co-change column.
- Milestone 53 (the board's own peripherals: network and storage on real silicon) is PARTIAL and its
  network lane is active, so the API is still moving.
- The device-tree fixtures (`tests/fixtures/*.dts`) are excerpts of the mainline and vendor JH7110
  trees. Their licenses were not checked here; record each before `cargo package` ships them.
- Two harnesses is thin for a published driver crate. *What is proven* now requires a third, on
  the PHY link decode; whether MDIO sequencing wants one too is still the lane's question.
- `needs_person` is `yes` because the exit includes a publication only calef can approve. Everything
  before step 7 can start now.

## Index row

nife's Synopsys DesignWare Ethernet QoS controller logic, two Kani proofs and their falsification
patches, cut free of nife's device tree and address map and published on crates.io as
`designware_ethernet` 0.1 from its own `nifeos` repository, then consumed back by nife.
