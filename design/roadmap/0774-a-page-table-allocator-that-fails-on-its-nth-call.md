---
status: NOT-STARTED
promoted_from: a-page-table-allocator-that-fails-on-its-nth-call
raised: 2026-10-04
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 774. A page-table allocator that fails on its Nth call

Raised 2026-10-04 (UTC) by the lane for milestone 745 (count the error paths no test reaches),
provisional, as its recommended first fault-injection pilot. *(Title and slug are drafts.)*

## Why here first

Of the twenty unreached error paths milestone 745 ranked as releasing memory or authority, twelve
are in `crates/page_tables` ([`notes/untested-error-paths.md`](../../notes/untested-error-paths.md)).
Five fire midway through a mapping, after something was taken: `map_span` and `map_range` after
earlier blocks are mapped, `map_block` after upper tables were allocated, and
`build_identity_domain` after earlier pages or regions are in a device's domain. No test has
reached any of them.

`Mapper` already takes its frame allocator as a closure (`alloc_page_frame`), which is codebase
rule 2 paying off: the seam exists, so the pilot adds no production code.

## What to build

A test allocator that hands out frames from a pool, records each one, and returns `None` on its Nth
call. Sweep N from 1 until a mapping succeeds, over `map_span` (with block sizes in play),
`map_range` and `build_identity_domain` (two regions, several pages each). After each failure:

1. Every frame the allocator handed out is reachable from the root, so a failed map orphans no
   table.
2. What is left mapped is stated: either nothing, or a prefix the caller is documented to unwind.
   The kernel's `unmap_run_prefix` is that unwind for `PAGE_FRAME_MAP`; this pins down what it is
   given.
3. The error is `OutOfPageFrames`, the one the kernel turns into `OutOfMemory`.

Host-only, milliseconds per test. If (1) or (2) fails, the finding is a frame leak or a half-mapped
grant per failed call. That is the evidence for or against the larger pilot in
milestone 757 (a test kernel fails a process on its Nth retype), now built.

## What it does not do

It does not reach the kernel's own rollbacks, which run under QEMU. It measures the library they
call, so a leak found here is a leak there too, and a clean result says nothing about the caller.
## Index row

Twelve of the unreached error paths that release memory or authority sit in `crates/page_tables`, five of them midway through a mapping. A test allocator that fails on its Nth call, swept over `map_span`, `map_range` and `build_identity_domain`, reaches them with no production code added.
