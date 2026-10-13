---
status: BUILT
raised: 2026-09-26
built: 2026-09-27
promoted_from: a-region-retypes-a-frame-run
---
# 659. A region retypes a frame run

Status BUILT 2026-09-27: built on PR #1373, recorded as DECISIONS §233. *(Title and slug are drafts.)*

Raised by the lane for milestone 23 (a capability-routed component
OS with live replacement) while starting the handoff page count the `line_editor` swap needs. Its
state with history is a little over one page (notes/interactive-stack-swap.md).

It changes the meaning of an argument of an existing method,
`MemoryRegion::RETYPE`, which is the syscall surface and so an architect's call under §10 (process model:
capability-based, microkernel).

## The finding

§102 (a Frame names a run of pages) made a `PageFrame` capability able to name a run, and `MAP` and
`MAP_INTO` map a whole run in one call. Only the kernel can mint one. `MemoryRegion::RETYPE` takes
no arguments and always makes a one-page frame (`kernel/src/syscall.rs`, `memory_region_retype`).
So a supervisor that wants a two-page handoff page for a component has no one-capability way to
make it, although the pages it would get are already contiguous: a region hands pages out from a
watermark (`crates/memory_regions/src/table.rs`, `retype_page`).

## Options

- **A. `RETYPE`'s first argument becomes a page count, with `0` meaning one** (recommended). One
  capability per handoff, however large, which is what §102 argued for at 475 pages. Additive:
  every caller today passes zero. The region's watermark advances by the count, or the call
  refuses with `OutOfMemory` and moves nothing.
- B. Route N one-page frames. No kernel change. `component_plan` would need a role that resolves
  to several slots, and every instance spends N capability-table slots on one blob. Chosen only
  because it is less work, which is AGENTS.md's elegance-over-convenience test failing out loud.
- C. Keep one page and drop history across a swap. The coordinator's default was a page count
  instead, so this is the fallback if A and B are both refused.

## Ruled

calef, 2026-09-26: option A. Built on PR #1373 (branch `milestone/23-retype-run`), with the
arithmetic proved in `crates/memory_regions` and a guest test through the real handler; the
decision is recorded as DECISIONS §233 (`MemoryRegion::RETYPE` takes a page count).

## What it unblocks

The handoff page count (`component_plan::Handoff` grows `pages`), then the `line_editor` swap, then
`redoxfs_server`'s eventual handoff, which will not fit in one page either.

## Follow-on

- **Milestone 669.** Milestone 669 (swap `line_editor` live under `system_initializer`). The handoff page count and the `line_editor` swap this unblocked: the swap itself is `design/roadmap/0669-swap-line-editor-live-under-system-initializer.md`.

## Index row

`MemoryRegion::RETYPE` always makes a one-page frame, so a supervisor cannot make a two-page handoff page in one capability. Proposed: let it retype a run, which changes an argument of an existing syscall and is an architect's call.
