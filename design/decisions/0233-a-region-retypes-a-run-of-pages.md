---
status: DECIDED
raised: 2026-09-26
decided: 2026-09-26
ratified_by: calef
---

# 233. `MemoryRegion::RETYPE` takes a page count

*Section number provisional until the merge queue lands it, for the reason §231 (a swap's warning to
a dependent is advisory) gives. The file name is provisional too.*

Raised 2026-09-26 by the lane for milestone 23 (a capability-routed component OS with live replacement)
([block](../roadmap/0023-component-os-live-replacement.md)), while sizing the handoff page that §232
(the `line_editor` swap contract) needs. The finding and the options are in
[`design/roadmap/0659-a-region-retypes-a-frame-run.md`](https://github.com/nifeos/nife/blob/milestone/23-line-editor-swap/design/roadmap/proposals/a-region-retypes-a-frame-run.md),
on branch `milestone/23-line-editor-swap`, stacked on #1342 and not on `main` yet.

## The ruling

calef, 2026-09-26 (UTC): *"A."* Recorded by the maintainer at 18:22Z the same day.

- `RETYPE`'s first argument becomes a page count. `0` means one page, so every caller today, which
  passes zero, is unchanged.
- The region's watermark advances by the count and the call returns one `PageFrame` capability
  naming the whole run.
- If the region cannot supply the count, the call refuses with `OutOfMemory` and moves nothing.

This is a change to the system call surface under §10 (process model: capability-based, microkernel),
and it fits that model: it gives an existing method's unused argument a meaning, and adds no method.
It builds on §102 (a Frame names a run of pages), which made a frame able to name a run and taught
`MAP` and `MAP_INTO` to map one in a call. Until now only the kernel could mint a run. A region
already hands out pages from a watermark, so the pages a count asks for are contiguous by
construction.

## Refused

- B, routing N one-page frames. No kernel change, but every instance spends N capability-table
  slots on one blob, and `component_plan` needs a role that resolves to several slots. It was
  chosen only because it is less work, which is the elegance-over-convenience test failing.
- C, keeping one page and dropping `line_editor`'s history across a swap. It loses state the person
  can see, and `redoxfs_server`'s eventual handoff will not fit in a page either.

## What it unblocks

A page count on `component_plan::Handoff`, then the `line_editor` swap of §232, then
`redoxfs_server`'s handoff. The implementing lane records the argument's semantics beside
`memory_region_retype` in `kernel/src/system_call.rs` and proves it on every supported architecture,
per §19 (architectural parity is a tenet).
