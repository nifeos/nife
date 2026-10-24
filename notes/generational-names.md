# Generational names

*(Milestone 14 phase A. The mechanism behind `crates/generational_table`, the scheduler's thread table since
the `BTreeMap` was retired. See design/kernel-objects-from-untyped.md, decision D2.)*

## The problem it solves

A kernel constantly hands out names for things that die: thread ids, file descriptors,
capability slots. Every such name can outlive what it named. A Unix pid you stashed last week
may belong to a different process today; that is a real bug class (pid reuse races), and Unix
mitigates it with ceremony (pidfds) rather than by construction.

We had the same exposure in miniature: a `ThreadId` inside a `Reply` capability, or parked in a wait
queue, can in principle outlive its thread. The old thread table was safe against that only in
the weak sense: a dead ThreadId's map lookup failed, because ids came from a global counter and were
never reused. Never-reused names are safe but unbounded, and the map holding them was unbounded
too, which milestone 14 forbids.

## The mechanism

A name is `(generation, slot)` packed in one u64: slot in the low 32 bits, generation in the
high 32.

- Lookup indexes the slot and compares the generation. O(1), two loads.
- Remove bumps the slot's generation. Every outstanding name for the old occupant now fails
  the compare, *forever*, including after the slot is reused: the new occupant's names carry the
  new generation.

So slots are reused (the table is a fixed array, milestone 14's requirement) while names behave
as if they never were (the safety the counter used to buy). A stale name is not a dangling
reference and not somebody else's thread; it is `None`.

Game engines rediscovered this shape independently. They call it a slotmap, and entity ids in ECS
engines are exactly generational indices. The reason is the one kernels have: many
short-lived objects, and stored references that must fail safely rather than dangle.

## What it buys the capability model

The path to capability-only thread naming (design doc, D2) ends with capabilities carrying
direct thread references. seL4 makes that safe with the CDT: destroying a TCB revokes every
capability naming it. We deferred the CDT. Generational names are the other way to be safe:
a `Reply(ThreadId)` whose caller is gone resolves to nothing, checked at use, no revocation sweep
required. The table is therefore the first step of the capability-only path, not a detour;
the intrusive-list work (D1) removes the lookups one structure at a time, and what remains of
the table at the end is exactly this validity check.

Recorded-accepted by milestone 94's sweep (2026-08-04): "we deferred the CDT" reads like an
unpaid debt and is not one. A derivation tree and generational names are two answers to the same
safety question, and this tree took the second, on purpose, for the reason in the paragraph above.
notes/live-replacement.md still says the real tree is wanted; that is the same object, blessed here
rather than twice. notes/security.md carries the other half (no revocation, and what it costs). An
audit may pass over it; §71 says what would promote it. See notes/untracked-work-sweep.md.

## The details that matter

- **The boot thread is tid 0 by construction**: a fresh table's first insert mints slot 0,
  generation 0, which packs to 0. That is a property, not a hardcoded key.
- **`u64::MAX` can never be minted** (slot would have to be 2^32-1 with 128 slots), so
  `cpu::NO_TID` keeps working as the "no thread" sentinel, and it is also the `UNNAMED`
  placeholder a `Thread` carries between construction and insertion.
- Generations are 32-bit and wrap. A single slot reused 2^32 times could resurrect an
  ancient name. Recorded honestly in the crate doc; not a bound anything real approaches.
- Insert is O(N) (scan for a free slot); lookup, the hot-path operation, is O(1).

## Limits, in numbers

A full table refuses an insert, and the caller decides what that means. For the kernel, spawn fails
with the same contract as running out of memory, and `N` is a documented limit of the image.

A slot must be reused 2^32 times before an ancient name could resolve again. At one spawn per
microsecond against a single slot, that is an hour and a quarter of doing nothing else. A real
workload spreads reuse over every free slot.

Insert scans for a free slot, which is spawn-rate work. The switch path only calls `get` and
`get_mut`.

## The occupancy bound

The table keeps `top`, one past the highest live slot. Every slot at or above it is empty, so a walk
bounded by `top` sees exactly what a walk over all `N` slots would. Insert is first-fit, so
occupancy packs toward slot 0 and `top` stays close to `len`. A walk then costs what the table holds
rather than what it could hold.

The bound exists because sweep cost was tracking `N`. The iterators yield only live entries but
visited every slot to filter them. `sched::delete_page_frame_caps_where` walks every thread's
capability table on every `MemoryRegion::DESTROY`, so it got dearer whenever `sched::MAX_THREADS`
rose, with no more live threads. On the `spawn_el0` icount benchmark, raising that ceiling from 128
to 256 cost 141,359 ticks through this path alone (notes/benchmarks.md). Every later raise would
have paid again.

`top` shrinks as well as grows, so it tracks live occupancy rather than peak occupancy. Removing the
top entry walks down over empty slots and stops at the first live one. That is amortised O(1),
because a step down happens at most once per slot that was filled and freed. The walk is a `for`
over `N` with a `break`, since CBMC cannot see `top <= N` from the field and would unwind a `while`
without limit.

A bound that came down too far is the one way a walk could miss a live entry, so
`a_bounded_walk_yields_every_live_entry` proves it cannot. It removes an arbitrary subset of three
filled slots rather than running an arbitrary operation sequence. The sequence version did not
finish, and `script/verify` is already the merge queue's long pole.

## Resumable sweeps

`iter_from` yields `(slot, name, &T)` from a given slot onward. The slot index is what lets a sweep
resume after a caller gives its lock back between entries. A position in the filtered sequence
would move when entries in the gap are removed. A slot index does not: if the entry at slot `k`
died, `k` is empty rather than somebody else's.

`rendezvous::SURVEY` (milestone 126) reports one supervised thread per syscall and hands the next
slot back to userspace as a cursor. The set can change between calls, so a resumed sweep is a
sequence of snapshots rather than one. It never reports an entry twice and never resolves a cursor
to the wrong thread, which is all a `readdir` shape ever promised.

## What is proved (milestone 18 style, before anything leans on it)

Three Kani harnesses in `crates/generational_table` (`script/verify`):

| Harness | Property |
|---|---|
| `a_removed_name_never_resolves_again` | after remove, the name fails `get`/`get_mut`/`remove` even once the slot is reused, and the reuser's name differs |
| `live_names_are_distinct_and_resolve_to_their_own_entry` | the packing cannot alias two live entries |
| `a_name_the_table_never_minted_resolves_to_nothing` | for any u64, resolution succeeds only on exactly the name the table issued: names cannot be forged |
