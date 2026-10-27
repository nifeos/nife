# A split refused for a full region table

Status: PROPOSED, for calef. Filed 2026-09-26 by milestone 601 (the region table prints its
peak), whose number is provisional, from a finding in the lane of milestone 152 (durable
delegation). Nothing here changes code; the recorded limitation is the `# BUGS` entry on
`RegionTable::split` in `crates/memory_regions/src/table.rs`, whose doctest is the reproduction.

## What is being decided

Whether `RegionTable::split` should keep its bump-only rule on a full table. Today, when the
parent's budget covers the carve but no table slot is free, `split` returns `None` having already
advanced the parent's watermark and raised its child count. No child name was minted, so nothing
can ever bring that count back to zero, and the parent is refused by `claim_for_destroy` for the
rest of the boot.

In the kernel that means `sched::reclaim_region` refuses the parent, so `MemoryRegion::DESTROY` and
`rendezvous::REAP` both answer `NotPermitted` forever. A restart policy reads that as "not yet".
The parent's slot, its pages, and every thread and address space retyped from it stay held.

The failure ratchets. It only happens when the table is full, and it leaves the table fuller
by at least the parent, so the next caller that reaches the ceiling reaches it sooner. Milestone
152's lane measured the aarch64 suite at 252 of 256 live regions, so the ceiling is not the
"practically unreachable" case the rule was written for.

## Is the premise true

The rule's own reasons, read from the history rather than recalled:

- It was written on 2026-07-26 (`df3f64860`, LIFO return-of-pages), when the kernel carved under
  one hold of the region lock and inserted the child under a second hold. Rolling back after
  the second hold would have raced a carve another core made in between, so leaving the bump in
  place was the only safe answer. The comment called the full-table case "practically
  unreachable, sized for headroom".
- Milestone 135 (the region claim, under loom) moved `split` into `crates/memory_regions` as one
  `&mut self` borrow (`405113c08`). The second hold, and the race that justified the rule, no
  longer exist. The doc comment kept the rule and dropped the reason.
- The headroom is gone too: 252 of 256 on aarch64 before #1347, and `memory_region::MAX_REGIONS`
  now carries the measured ledger.

So both reasons have lapsed. What remains is the sentence "the honest record that a page of it is
unaccounted". Under one borrow, a refused split need not leave any page unaccounted at all.

## The options

### A. Keep the rule (status quo)

Cost: nothing to build. The `BUGS` entry and its doctest stay
as the record. Loses because the consequence is a permanent, compounding leak on exactly the path
the table is closest to failing, and the reasons for the rule no longer hold.

### B. Decide before mutating

Reserve the child's slot first, then bump the parent, inside the
same borrow. A refused split changes nothing. Measured on this branch as a trial patch, not
committed. It adds 9 lines and removes 10 in `split`, and all 13 host tests pass. The one doctest that fails is the `BUGS` reproduction, which
was built to fail on exactly this change.
The Kani proof covers `split_new_watermark`, which is unchanged. `script/interleaving-check` was
not run against the trial.

```rust
let r = self.table.get(parent)?;
let new_watermark = split_new_watermark(r.pages, r.watermark, pages)?;
let base_page = r.base_page + r.watermark;
let child = self.table.insert_with(|_| Region { base_page, pages, watermark: 0,
                                                  pinned: false, parent, children: 0 })?;
let r = self.table.get_mut(parent)?;
r.watermark = new_watermark;
r.children += 1;
Some(child)
```

### C. Mutate, then roll back on a failed insert

Same outcome as B. Loses to B on moving parts:
it writes a state and then unwrites it, and a watermark that goes down outside the LIFO return is
the one thing a reader of a bump allocator is primed to distrust.

### D. Raise `MAX_REGIONS` instead

Orthogonal. It makes the full table rarer and leaves the
ratchet in place for whenever it is reached. Worth doing on its own evidence (the peak line now
printed by every suite run), not as an answer to this.

## What the tree already does in the analogous case

Every neighbor refuses without side effects:

- `RegionTable::retype_object_page`: "`None` on an exhausted or dead region, and **nothing is
  pinned** in that case: a caller that got no page owes no unpin."
- `memory_region::create`: a failed `insert_root` hands the pages back to the frame allocator
  rather than leaking them.
- `syscall::memory_region_split`, as of this milestone: a `SPLIT` whose child capability finds no
  free slot now destroys the child, returning its pages and its count to the parent. Before, it
  orphaned the child with the same permanent consequence as the case above. That one was a kernel
  leak outside the crate's rule, cheap to reverse, and fixed with a test
  (`split_refused_for_a_full_capability_table_orphans_no_child`) rather than proposed.

## Prior art

From memory, not re-read for this note: seL4's untyped retype checks the destination slots and
the free index during decode, and advances the untyped's free index only in the invoke step that
cannot fail. A refused retype leaves the untyped as it was. That is option B's shape.

## Reversibility

Cheap. `split` is private to `crates/memory_regions` and the kernel's `memory_region::split`
wrapper. No wire format, syscall number or name changes. `MemoryRegion::SPLIT` still answers
`OutOfMemory` on a full table under every option. Nobody outside this tree has acted on the rule.

## Recommendation

B. It is the fewer-moving-parts option and matches every neighbor. Would it still be chosen if
A and B cost the same? Yes; A is only cheaper by the nine lines B needs.

## What is blocked

Nothing waits on this. The `BUGS` entry stands until it is answered, and on a yes the change is
the patch above plus rewriting the doctest from a reproduction into a regression test.
