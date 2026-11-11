# A split refused for a full region table

Status: the maintainer's provisional ruling, option B, 2026-11-11 (UTC). A reversible change (a
two-way door); calef may overrule it. The code is `RegionTable::split` in
`crates/memory_regions/src/table.rs`.

## The rule

A split the table refuses moves nothing, whatever refused it: a dead parent, a budget that cannot
cover the carve, a parent or ancestor at its region ceiling, or a full table. `split` makes every
check before it changes the parent, then takes the child's table slot, and only then advances the
parent's watermark, raises its child count and counts the new region against every ancestor. All of
it happens in one `&mut self` borrow, so no other caller sees a step in between.

A full table is the case that needs the order. If the parent moved first, a refused insert would
leave a child count that no child name could ever bring down. `claim_for_destroy` would then refuse
the parent for the rest of the boot. `sched::reclaim_region` refuses such a parent, so
`MemoryRegion::DESTROY` and `rendezvous::REAP` would answer `NotPermitted` forever. The parent's
slot, its pages and everything retyped from it would stay held. Each refusal would leave the table
fuller, so the next caller would reach the ceiling sooner, and the aarch64 suite runs within a few
slots of `memory_region::MAXIMUM_REGIONS`.

The host test `a_split_refused_for_a_full_table_leaves_the_parent_unchanged` and the Kani harness
`a_refused_split_moves_nothing` (which fills the table on one branch) hold the rule.
`MemoryRegion::SPLIT` still answers `OutOfMemory` on a full table.

## The options

- **A. Keep the parent spent on a full table.** The bump-only rule existed because the kernel once
  carved under one hold of the region lock and inserted under a second, where a rollback could
  race another core's carve. `split` is now one borrow, so that reason does not apply, and the cost
  is a permanent, compounding leak on the path closest to failing. Refused.
- **B. Decide before mutating** (in force). Fewest moving parts, and it matches every neighbor.
- **C. Mutate, then roll back on a failed insert.** The same outcome as B, but it writes a state
  and then unwrites it. A watermark that falls outside the LIFO return is the one thing a reader of
  a bump allocator is primed to distrust. Refused.
- **D. Raise `MAXIMUM_REGIONS` instead.** It makes a full table rarer and leaves the leak in place
  for whenever it is reached. A separate question, decided on the peak line every suite run
  prints.

## What the tree does in the analogous case

Every neighbor refuses without side effects:

- `RegionTable::retype_object_page` pins nothing and counts nothing when it refuses, so a caller
  that got no page owes no unpin.
- `memory_region::create` hands the pages back to the frame allocator when `insert_root` refuses.
- `syscall::memory_region_split` destroys the child when its capability finds no free slot, and
  `split_refused_for_a_full_capability_table_orphans_no_child` tests it.

## Prior art

seL4's untyped retype checks the destination slots and the free index during decode, and advances
the untyped's free index only in the invoke step, which cannot fail. A refused retype leaves the
untyped as it was. That is option B's shape. This is from memory, not re-read for this note.

## Reversibility

Cheap. `split` belongs to `crates/memory_regions` and the kernel's `memory_region::split` wrapper.
No wire format, syscall number or name changed.
