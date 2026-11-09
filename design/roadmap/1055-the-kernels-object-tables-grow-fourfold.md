---
status: NOT-STARTED
promoted_from: the-kernels-object-tables-grow-fourfold
raised: 2026-11-05
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 1055. The kernel's object tables grow fourfold

Raised by lane/record-follow-on-items.

Reuse: the existing constants and `registry_footprint` in `kernel/src/user/address_space_registry.rs`,
`script/stack-frame-check` and `cargo xtask bench`. No new tool.

## Background

The kernel holds each kind of object in a fixed array sized at build time:
`sched::ipc_tables::MAXIMUM_THREADS` (256), `MAXIMUM_RENDEZVOUS` (512),
`sched::notification::MAXIMUM_NOTIFICATIONS` (256), `sched::timer::MAXIMUM_TIMERS` (256),
`memory_region::MAXIMUM_REGIONS` (256), and the process and address-space tables beside them. A
larger table used to cost boot stack, because installing the scheduler's tables built the whole
aggregate in one frame. The tables are now copied from a `static` into place, so their size no
longer reaches a stack frame, and `script/stack-frame-check` holds that function under the guard
page.

## The work

1. Raise each table to four times its size, one kind per commit.
2. Check every identifier and index field that carries a slot number for width, so no id wraps.
3. Measure, before and after, the kernel's `.bss`, boot time to the shell on each architecture, and
   the bench's instruction counts, and record them in this block.

## Done when

- Each table is four times its current size on all three architectures, with the measurements
  recorded.
- `script/stack-frame-check` passes, and no bench row leaves its band.

## Index row

The kernel's object tables are small enough to bind early. This grows each fourfold on all three architectures, with the memory and stack measurements recorded.
