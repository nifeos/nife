---
status: BUILT
raised: 2026-10-04
built: 2026-10-04
promoted_from: a-mapping-cannot-outlive-its-frames-revoke
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 762. A mapping cannot outlive its frame's revoke

*(Title and slug are drafts.)* It follows #1640 (the delegation side of the same window) and
milestone 761 (capability lookup off the global lock).

## Why

A revocation sweep deletes capabilities first, then unmaps what the mapping log holds when it scans.
`PageFrame::MAP` and `AddressSpace::MAP_INTO` read the frame capability in one critical section and
mapped and recorded in later ones, so a sweep could run wholly between. It deleted the capability
and scanned a log the mapping was not yet in, and the mapping was recorded after it, alive. Under
`PageFrame::REVOKE` that is authority the revoker took back. Under `MemoryRegion::DESTROY` it is a
live mapping of a page the allocator hands to somebody else, the use-after-free DECISIONS §13
(capability revocation and untyped reclamation) exists to prevent and named in 2026-07 as "the one
honest race". #1640 recorded it reasoned and undriven. It is fatal risk 7's confinement claim.

## What was built

- The defect was real. `system_tests::user::map_revocation_window_tests` holds a mapper inside
  its `MAP` with the revocation-race lane's seam (`delegation_pause`, reused), runs a whole sweep,
  and reads the page tables afterwards. Before the fix, on aarch64, both paths kept the mapping
  under both sweeps (four red rounds, every one answering 0 with the premise held).
- `MemoryRegion::MAP` had it too, unrecorded: a retype that succeeded before `DESTROY` claimed
  the region and recorded after the scan left a mapping of a reclaimed page. A third test drives it.
- The fix is one critical section, not record-then-recheck. `revoke::MappingHold` (provisional)
  is a hold of the mapping registry every unmap pass already takes after its capability pass. Each
  map path reads its source under it (`MappingHold::current_cap`, the thread's table lock at rank
  57 beneath the registry's 59), then maps and records, then releases. A sweep is wholly before the
  read (the slot is empty) or its scan is after the record. Record-then-recheck, the shape #1640
  suggested, leaves the mapping live in a page table for the instant between the record and the
  undo, which under `DESTROY` is a reclaimed page mapped; this has no such instant.
- ABI and semantics unchanged. A `MAP` that loses its source answers what the same call made
  afterwards answers (`NoSuchSlot` for the frame paths, `OutOfMemory` for a region), and each test
  asserts exactly that by making the call again. `page_frame_map` takes the slot rather than the
  capability `invoke` dispatched on, so a stale read has no parameter to arrive through.
- `AddressSpace::map_physical_held`, `user::with_user_address_space` and the two hold methods
  (`record_mapping`, `forget_mapping`, which replace the free functions of the same names) are the
  plumbing. All names are provisional.

## Falsification

One replayable patch per path under `system_tests/falsifications/`, each putting back the
read-outside-the-hold shape. All three replayed red on riscv64 at the predicted assertion.

## Cost

riscv64 icount, the same tree with and without the fix: `map_el0` 120 to 113 ticks a map (-6.3%),
`spawn_el0` 2,221 to 2,208 (-0.6%), every other row within a tick. Cheaper because a run takes the
registry once instead of once per page plus a table lookup. The hold is now longer (it spans
page-table construction), and what that costs under contention is not measured: no benchmark maps
from two cores at once.

## Gates

- The three tests, `script/test --test map_revocation_window`, pass on aarch64, riscv64 and
  x86_64 locally. CI runs the whole suite on all three.
- `script/fastpath-footprint`: within band on all three ISAs; against the unfixed tree on aarch64,
  `system_call_entry` moved from -1.2% to -1.5% of its baseline and the IPC paths did not move.
- `script/stack-frame-check` passes on aarch64 and riscv64, its default set. On x86_64 it fails on
  `kernel_main` and `iommu::owner_index`, both present before this lane and outside its diff.

## BUGS

- The read is rung three, not rung one. A caller can still read a frame with
  `sched::current_cap` and map it outside a hold; the map handlers' taking a slot is the defense,
  the same one `sched::Delegation` relies on. Written at `MappingHold`.
- `DeviceFrame` through `MAP_INTO` is covered by construction and not driven.
- A destroyed region's page tables stayed linked: `MemoryRegion::DESTROY` returned tables a live
  space walked. Fixed by milestone 763 (a destroyed region cannot take a live space's page tables
  with it), provisional, #1665; the root is left as that lane's proposal.

## Follow-on

- **Milestone 763.** The destroyed-region page-table hole, driven and fixed: tables are recorded
  like leaves and cut on `DESTROY`. The root stays open as
  milestone 765 (a destroyed region cannot free the root a running thread walks).

## Index row

A frame revoked while somebody was mapping it no longer stays mapped. `PageFrame::MAP`,
`AddressSpace::MAP_INTO` and `MemoryRegion::MAP` read their source under the same hold of the
mapping registry that maps and records, so a revoke or a region destroy is wholly before or wholly
after. All three were driven red first; fatal risk 7's claim that a revoked frame is unreachable now
holds for mappings as well as capabilities.
