---
status: NOT-STARTED
raised: 2026-11-23
milestone_dependencies: 1050
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 1124. `designware_mobile_storage`: its eMMC and SD sequences checked against another driver's tests

The number, title and slug are provisional. The branch that wrote this block is
`roadmap/tgoskits-prior-art`.

No eMMC has run on `designware_mobile_storage`. The crate's `BUGS` says so: its eMMC bring-up,
the 4- and 8-bit bus switch and the 52 megahertz timing have run against the crate's own simulated
card only. A simulator written from the same reading of the standard as the driver agrees with the
driver by construction. A second driver's tests, written from another reading, can disagree, and
each disagreement is either our bug or theirs.

This milestone ports or adapts the card-sequence tests of tgoskits `drivers/blk/sdmmc-protocol`
(Apache-2.0) into the crate's tests, and the 4 KiB descriptor boundary tests of tgoskits
`drivers/blk/dwmmc-host` (Apache-2.0). It records each disagreement as a finding.

What drives it: milestone 1050 (the outside review's findings land before its first release) and
milestone 817 (`designware_mobile_storage`: a proven DesignWare SD and eMMC host, released on its
own). A release that claims eMMC support before any card has run it rests on the simulator alone.
What it saves: a second, independent check of the eMMC path before a board with an eMMC module
exists on the bench.

Reuse: the crate's simulator (`src/simulator.rs`), its host tests (`src/tests.rs`,
`tests/simulator.rs`) and its Kani harnesses (`src/proofs.rs`). The tgoskits tests are a source to
adapt, not a dependency, and the crate takes no new crate.

## Where the work lands

The crate lives in `nifeos/designware_mobile_storage`. Every change lands there, as an ordinary
pull request through that repository's queue. This block cites each one as
`nifeos/designware_mobile_storage#N` when it merges. nife changes nothing but this block, and its
pin moves only when the crate next releases.

## The cases to port or adapt

Read at `rcore-os/tgoskits@b6aa8e5d4`, on its `dev` branch.

1. `drivers/blk/sdmmc-protocol/src/sdio/tests/mmc_init.rs`. Its two cases check that the driver
   sets an eMMC's high-capacity erase geometry before it publishes trim, and that it turns on an
   advertised write cache before bring-up completes. The crate does neither. Each case becomes a test
   of what the crate does today against the same simulated card. Each difference becomes a finding
   that says whether the crate should do it, or a `BUGS` line where it should not yet.
2. `drivers/blk/sdmmc-protocol/src/sdio/tests/sd_speed.rs`. Its five cases cover speed selection.
   They are a cap at legacy high speed, the fall back to high speed when the UHS voltage switch fails, a
   reset of the voltage only, speed selection turned off, and a card that does not support the
   switch function keeping default speed. The crate runs default speed only. The last two cases
   port as written, since they assert default speed. The others become findings against the
   crate's `BUGS` line on speed.
3. The rest of that directory (`init_flow.rs` and its neighbors), read for any eMMC or SD step the
   crate takes in a different order. Each such step becomes a test or a finding.
4. `drivers/blk/dwmmc-host/src/dma/tests.rs`. Its cases cap a descriptor at 4 KiB and keep a
   descriptor from crossing a 4 KiB boundary (`idmac_descriptor_payload_is_limited_to_four_kib`,
   `idmac_ring_does_not_cross_a_four_kib_dma_boundary`). The crate's IDMAC meets the rule by
   construction: its region is page-aligned and each descriptor carries one 4096-byte data page.
   The Kani harness for the descriptor chain bounds each descriptor's size and keeps its buffer
   inside the data pages, but it does not assert the boundary. This item adds that assertion,
   `buffer % 4096 + size <= 4096`, to the harness with its falsification patch, and a host test
   for an unaligned region's refusal.

## The license notice

The rule in force comes from milestone 1019 (the driver crates carry facts from their sources, not
their text), items 3 to 6. A file followed closely carries its source's copyright and permission
notice, and the README, the notes and the `Reuse:` line name it. A test adapted from tgoskits
follows its source closely. The crate already ships `LICENSE-APACHE`, so the work adds the
attribution. That is the source path and commit at the top of each adapted test, a line in the
README's License section, and an entry in `NOTICE` or its equivalent if the crate adds one. The
tgoskits root `LICENSE` is the Apache License 2.0 text with no copyright line, and the repository
has no `NOTICE` file. The attribution therefore names the project, and the finding records that
tgoskits states no copyright holder.

## Findings

Each disagreement is a finding, numbered in this block in the order found, with a status:

- Ours: the crate is wrong, and a pull request fixes it with a test.
- Theirs: tgoskits is wrong, and the finding says why against the SD specification or the JEDEC
  eMMC standard.
- Scope: the crate does not do it by choice, and a crate `BUGS` line says so.

## Done when

- Every case in items 1, 2 and 4 is a test in the crate, or a finding with its status.
- The descriptor harness asserts the 4 KiB boundary, and its falsification patch fails it.
- The crate's README names the adapted source, and each adapted file carries its attribution.
- The crate's existing tests, harnesses and fuzz target still pass.

## Follow-on

- **Proposed.** An eMMC run on a board, which `docs/bench-emmc-bus-modes.md` in the crate
  describes. No item here replaces it.

## BUGS

- These tests check the crate against a second reading of the standards, not against a card. An
  agreement here is not a hardware result.
- The tgoskits facts come from one read of its `dev` branch at the commit above.

## Index row

No eMMC has run on `designware_mobile_storage`, and its simulator agrees with it by construction.
This block ports tgoskits' SD and eMMC sequence tests and its 4 KiB descriptor boundary tests into
the crate, with the Apache-2.0 attribution. It records each disagreement as a finding: ours,
theirs or out of scope.
