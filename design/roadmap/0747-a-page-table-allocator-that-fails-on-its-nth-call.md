---
status: BUILT
raised: 2026-10-04
built: 2026-10-04
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 747. A page-table allocator that fails on its Nth call

Built 2026-10-04 (UTC) by the lane `lane/paging-nth-alloc`, from the proposal of the same title raised
on #1591 by the lane that counted the error paths no test reaches. The number 747 is provisional
until the queue lands it, and the integrator promotes the proposal. *(Title and slug are drafts; the
test file, helper and test names are provisional.)*

## What it does

`crates/page_tables/tests/nth_allocation.rs` wraps the mapper's `alloc_page_frame` closure so it fails on
exactly one call, N, counted from 0, and sweeps N upward until the mapping succeeds. It covers
`map_block` (each leaf size), `map_range` across a 1 GiB boundary, `map_span` with a 4 KiB head, two
2 MiB blocks, a 1 GiB block and a 4 KiB tail, and `build_identity_domain` over two regions. Each runs
on every format that can express it: aarch64, Sv39 and x86_64 (`Ia32e`, or `Vtd` for the domain).
After every failure it walks the tables independently of `Mapper` and asserts three things. The
reachable tables are exactly the root plus every frame handed out, so nothing leaked. What is left
mapped is a strict prefix of what the successful run maps. The error is `OutOfPageFrames`.

## What the sweep found

61 failure points across the four functions and the three formats each runs on. **No leak and no
half-installed mapping at any of them.** Each allocation in the walk is linked into its parent the
moment it is zeroed, and the leaf is written only after every table above it exists, so a failure
returns before anything unreachable or out of order can exist.

What a failure does leave is empty intermediate tables: reachable, so not a leak, and at most one per
level above the leaf per failure (up to three on a four-level format). That is the documented shape.
`unmap` leaves tables standing for the same reason, and teardown frees the recorded frame set
wholesale (`notes/teardown.md`). Nothing was fixed, because nothing was wrong.

## Falsifications

Four, one per test, under `crates/page_tables/falsifications/nth_allocation.*.patch`, each replayed red
and reverted green on 2026-10-04 (UTC). Two are an "atomic" unwind that clears the entry a failed map
installed (in `map_block`, and in `map`), which orphans the tables below it; assertion (1) catches
both. Two are a "best effort" `map_span` and domain build that skip a failed leaf and map the rest;
assertion (2) catches both. `script/falsifications --check` reports them as known gaps, because it
sweeps Kani harnesses and kernel `#[test_case]`s and these are host `#[test]`s.

## What it does not do

It measures the library. The kernel's rollbacks (`unmap_run_prefix` for `PAGE_FRAME_MAP`) run under
QEMU and are not reached. The kernel's one domain caller `expect`s the build and its allocator never
returns `None`, so the domain result says what a future fallible caller may rely on rather than what
runs today. It does give the larger pilot, the proposal *a test kernel fails a process on its Nth
retype*, the library-level answer it asked for: a leak that pilot finds is the kernel's own, not the
mapper's.

## Follow-on

- **Recorded.** Host `#[test]` falsifications are replayed by hand; teaching `script/falsifications`
  a third replay verb (`cargo test --test <file> <name>`) would let `--sweep` keep them honest. That
  is a format question for the script's owner, and the patch heads say how to replay them meanwhile.

## Index row

A test allocator fails on its Nth call, swept over N, for `map_block`, `map_range`, `map_span` and
the IOMMU domain build on every format. 61 failure points, no leaked table and no half-installed
mapping. Four falsifications, each replayed red.
