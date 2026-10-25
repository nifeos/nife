---
status: NOT-STARTED
raised: 2026-10-07
milestone_dependencies: 887
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 817. `designware_mobile_storage`: a proven DesignWare SD and eMMC host, released on its own

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

- `crates/designware_mobile_storage`: 2,645 code lines (3,736 with comments), written by milestone
  53 (the board's own peripherals: network and storage on real silicon), which is PARTIAL. It is
  the whole Synopsys Mobile Storage Host Controller driver (`snps,dw-mshc`) behind one small
  `host::Registers` trait: card bring-up, SD and eMMC commands, the FIFO, partitions. It drives
  radon's microSD slot and eMMC socket.
- Four Kani harnesses, each with a falsification patch. The scratch range never touches a partition
  or sector zero, and a served block never leaves its window. A FIFO step never overruns the FIFO
  or the buffer, and an admitted range is inside the card and the address.
- Class B in #1806's note `notes/releasable-crates-2026-10-07.md`. **The cut** is larger than that
  note's inventory says. `jh7110::discover` reads a `DeviceTreeBlob`; under codebase rule 2 the
  caller passes the register region instead, and the query moves to the kernel side. The `serve`
  module is nife's own: it speaks nife's block protocol (`filesystem_contract::blk`) and places
  nife's virtual addresses (`address_space_map::pair_page`). So `serve` and its harness move to
  `components/src/designware_mobile_storage.rs`, or stay with the two constants inlined; the lane
  picks and records why. The tests also `include_bytes!` a fixture from
  `../../device_tree_blob/tests/fixtures/`, outside the package.
- Ruled 2026-10-25 (UTC): the crates take plain values; the device-tree walk moves to nife, and
  the knowledge of the vendor and mainline JH7110 trees stays in each crate as data and decoders.
- Cut on 2026-10-25 (UTC) by #2719. `Slot::from_node` decodes one slot from a node's properties,
  and `machine_discovery::jh7110::designware_mobile_storage` walks the tree, with the fixtures
  beside its tests. `serve` and its harness moved to `crates/driver_process_contract`, a contract
  crate, because the kernel and the EL0 driver in `components/` both link it.
- Its consumers: `kernel/src/designware_mobile_storage.rs`, `kernel/src/storage_bench.rs`,
  `kernel/src/memory.rs`, `kernel/src/arch/riscv64/mmu.rs`,
  `kernel/src/user/designware_mobile_storage_service.rs`, and
  `components/src/designware_mobile_storage.rs` (the EL0 driver).
- crates.io answers 404 for the name (checked 2026-10-07).

**Reuse:** the alternatives were surveyed in #1806's `other-implementations.md` appendix (looked up 2026-10-07),
and none is taken, because calef ruled this crate "publish ours". rcore-os's `dwmmc-host` 0.4.2
(Apache-2.0, 1.7k downloads) and `starfive-jh7110-dwmmc` 0.1.8 (911) live in the `tgoskits`
monorepo with no proofs found. drivercraft's `sdmmc` 0.1.0 (11k) is a general SD/MMC crate with no
proofs found. `embedded-sdmmc` is SD over SPI plus FAT, another scope, and its project bans
AI-generated contributions. Linux's `dw_mmc` (GPL-2.0) stays the reference, which the crate reads
and never copies. No upstream offer goes anywhere: calef ruled on #1806 that nife contributes only to
projects nife depends on.

## What is proven

Each row is a property this crate must prove before it publishes. The harness cell names the Kani
harness that proves it, or says `none yet`; every named harness carries a falsification patch that
makes it fail at its own assertion. Each patch is at
`crates/designware_mobile_storage/falsifications/`. Measured 2026-10-11 (UTC): 4 of 6 proven.
Measured 2026-10-25 (UTC), after the cut: 3 of 5 proven.

| property | harness |
|---|---|
| The scratch range never touches a partition or sector zero | `proofs::the_scratch_range_never_touches_a_partition_or_sector_zero` |
| A FIFO step never overruns the FIFO or the buffer | `proofs::a_fifo_step_never_overruns_the_fifo_or_the_buffer` |
| An admitted range is inside the card and the address | `proofs::an_admitted_range_is_inside_the_card_and_the_address` |
| The MBR parse (`partition::Mbr::parse`) is total, and every entry it accepts lies inside the card | none yet |
| The capacity decoders (`card::Csd`, `card::ext_csd_sectors`) are total for any register or EXT_CSD bytes | none yet |

The last two rows are this lane's reading of the hostile surface: a stranger's SD card supplies
its partition table and its registers. The served-block row left with `serve` on 2026-10-25: its
harness is `designware_mobile_storage::proofs::a_served_block_never_leaves_its_window` in
`crates/driver_process_contract`. Not proven: the card bring-up state machine
(`sd`, `command`), timing, and the CID decode, which names the card and sizes nothing.

## Release, by #1806's rulings

1. The name is settled before anything leaves (857's Fork 5 as calef amended it on 2026-10-11 UTC).
   `script/names designware_mobile_storage` reports it ratified, crates.io answers 404 for it (or it
   is already ours), and `nifeos/designware_mobile_storage` does not exist yet.
   `script/crate-standalone` checks the first.
2. Make the cut above. `cargo tree -p designware_mobile_storage` then lists no nife crate, and the
   tests read only fixtures inside the package.
3. In the tree, `script/verify` proves the harnesses and `Cargo.toml` says `publish = false`.
4. The package is 0.1.0 with its own semver, states `rust-version` (#1806's note built it on stable
   1.94.1), and ships its harnesses and falsification patches. `cargo package --list` shows them.
5. Ready means proofs passing and the API stable. §235 (the OS is built and updated from packages,
   and the tree divides by what releases together) has milestone 610 (the interface's stability is
   measured weekly)'s per-package co-change column, and the soak (step 6) is how the second is
   measured. When the soak exits, calef adds it to the lint-checked publish allowlist. The
   `nifeos/designware_mobile_storage` repository is created when the crate moves, on the shared CI
   template (milestone 887), through reusable workflows: Kani through `kani-github-action`, lint,
   release. Its release workflow refuses to publish unless the proof job is green. No harness here
   reads `target_arch`, so stock Kani proves them. The patched Kani of §218 (carry a Kani patch so
   riscv64 is proved, and send it upstream) is carried only if one comes to need riscv64. Milestone
   589 (Kani can prove riscv64 from the hosts we already have) built it.
6. Soak, as its own repository (calef, 2026-10-17 UTC, replying "Agrred" (sic) to the maintainer's
   proposal; §305 (git dependencies are allowed on `nifeos/*` only, pinned to a landed commit)).
   Milestone 818 (`jh7110_entropy`: proven JH7110 TRNG logic, released on its own) is the pilot and
   soaks first. Before publication the crate is moved, not copied: it leaves this tree, and nife
   depends on `nifeos/designware_mobile_storage` by `rev`. A copy left behind does not count. The
   soak ends on evidence, not a duration, when all five hold:
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
`x86_64-unknown-none` and the host, all exit 0. The release workflow builds the same four.

## Standalone, measured 2026-10-11 (UTC)

`script/crate-standalone designware_mobile_storage` fails today, at base `ae6b91e79`. The manifest
inherits `license` and `lints` from nife's workspace and states no `rust-version`, and there is no
README, `LICENSE-MIT` or `LICENSE-APACHE`. Those hold for all nine crates. It does not build alone:
`address_space_map`, `filesystem_contract` and `device_tree_blob` are path dependencies outside the
copy, so neither the build nor Kani ran. A test reads
`../../device_tree_blob/tests/fixtures/qemu-riscv64-virt.dtb`. Beyond manifest and files, 14 lines
cite this tree: 5 `milestone N`, 4 tree paths, 2 `§N`, 2 provenance blocks and 1 naming calef.

`script/names` reports the name `designware_mobile_storage` ratified (checked 2026-10-25).

Measured again on 2026-10-25 (UTC) with `--no-kani`: the manifest and files pass, and the only
citations left sit in `Name:` blocks, which stay until the ratification pass. `address_space_map`,
`filesystem_contract` and `device_tree_blob` are still path dependencies, so the copy does not
build; the second half of the cut removes them.

Measured again after the cut on 2026-10-25 (UTC), with Kani: the manifest has no dependencies, the
crate builds for the host and the three targets on 1.88. Stock Kani proves all 4 harnesses, and
each falsification patch makes its harness fail. The only failures left are citations in `Name:`
blocks, which the ratification pass settles.

## Review before release

- The public API speaks the device's vocabulary, not nife's: no nife capability type, spawn handoff
  or nife error enum in a public signature. A reviewer judges this; no script can. Known today:
  nothing. `serve`, which spoke nife's block protocol, left with the cut.

## Exit

A check proves it. In nife, `cargo tree --locked -i designware_mobile_storage` resolves to `v0.1.x`
from crates.io with a checksum in `Cargo.lock`, `crates/designware_mobile_storage` no longer exists,
and the tree builds for all three targets. In `nifeos/designware_mobile_storage`, the run that
published 0.1.0 shows the Kani job green ahead of the publish job, and every harness's falsification
patch makes its proof fail. The five soak criteria of release step 6 hold before it publishes.

The name check of release step 1 passes first, so the repository, the package and every doc link
carry a ratified name. Before the move, `script/crate-standalone designware_mobile_storage` exits 0
in nife, and every row of *What is proven* names a harness that stock Kani proves in the copy and
whose falsification patch makes it fail. No crate publishes without Kani harnesses covering its
hostile-input and state-machine surface, each carrying a falsification patch that makes it fail at
its own assertion, replayable in the crate's own repository with stock Kani.

## What the crate no longer cites

The crate's source cites nothing of this tree beyond its `Name:` blocks, which stay at their
names until the ratification pass, so this section keeps the links it carried.

- `Cargo.toml`, the package comment: the `no_std` and host-test split is the pure-logic split of
  §7 (Testing: QEMU harness + host-testable crates, from commit one) that every driver crate here
  uses. Milestone 53 (the board's own peripherals: network and storage on real silicon) wrote the
  crate.
- `src/lib.rs`, the crate doc: the design is in `notes/designware-mobile-storage.md`, and
  `kernel/src/designware_mobile_storage.rs` implements `host::Registers` over the mapped window.
  That note also records why the lane did not take the crates.io candidate. The IDMAC bench step that
  the crate's BUGS waits on is the one that note describes.
- `src/sim.rs`, the module doc: `notes/designware-mobile-storage/driver.md` lists each behavior
  the simulated controller models.
- `src/tests.rs`, the module doc: the tests left `lib.rs` on 2026-10-12 (UTC), in lane
  maintainer/mutation-triage-2026w42, so the crate root stays an index under §282 (when a file is
  split, the parent becomes an index).
- `src/partition.rs`, the module doc: `notes/visionfive2.md`, "The microSD payload", describes the
  card radon boots from.
- `src/jh7110.rs`, the module doc: `notes/designware-mobile-storage/driver.md` holds every node,
  clock, reset and log line of the two device trees.
- `src/sd.rs`, the module doc: `notes/designware-mobile-storage/driver.md` holds the steps, `CMD0`
  to `CMD16` and `EXT_CSD`.
- `src/bench.rs`, `Watch`: it serves milestone 53 (the board's own peripherals: network and storage
  on real silicon)'s multi-block read diagnosis.

## BUGS

- Steps 5 and 6 wait on milestone 887 (crate repositories share one CI template),
  which owns the shared CI template, the publish allowlist lint and the `nifeos` crates.io team,
  and on milestone 610's per-package co-change column.
- Milestone 53 (the board's own peripherals: network and storage on real silicon) is PARTIAL, so the
  API is still moving.
- The device-tree fixtures (`tests/fixtures/*.dts`) are excerpts of the mainline and vendor JH7110
  trees. Their licenses were not checked here; record each before `cargo package` ships them.
- `needs_person` is `yes` because the exit includes a publication only calef can approve. Everything
  before step 7 can start now.

## Index row

nife's Synopsys DesignWare Mobile Storage host driver for SD cards and eMMC, with four Kani proofs
and their falsification patches, cut free of nife's device tree, block protocol and address map. It
is published on crates.io as `designware_mobile_storage` 0.1 from its own `nifeos` repository.
