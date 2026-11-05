# Generational names

*(Milestone 14 (kernel objects from untyped) phase A. The mechanism behind
`crates/generational_table`, the scheduler's thread table since the `BTreeMap` was retired. See
design/kernel-objects-from-untyped.md, decision D2.)*

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
- Generations are 32-bit and saturate. A slot whose generation reaches `u32::MAX` retires: insert
  passes over it, so no name carries that generation and a removed name never comes back. The
  table loses one slot of capacity each time, and only after 2^32 - 1 occupants of that slot.
- Insert is O(N) (scan for a free slot); lookup, the hot-path operation, is O(1).

## Limits, in numbers

A full table refuses an insert, and the caller decides what that means. For the kernel, spawn fails
with the same contract as running out of memory, and `N` is a documented limit of the image.

A slot retires after 2^32 - 1 occupants. Insert takes the lowest free slot, so a loop that
creates and destroys one object puts every occupant in the same slot, and that slot is the one to
time. The next section has the times.

Insert scans for a free slot, which is spawn-rate work. The switch path only calls `get` and
`get_mut`.

## How soon a slot retires, and what the other designs cost

A wrapping generation would hand the 2^32nd occupant of a slot the generation 0 again, and a proof
that a removed name never resolves would need "the generation has not wrapped" as a precondition.
These measurements say how soon a real loop gets there, and what each other design costs.

How fast a loop can refill one slot, from the fastest create-and-destroy loops measured:

| table, `N` | loop | one cycle | 2^32 cycles |
|---|---|---|---|
| threads, 256 | `spawn_reap`, HVF on Apple Silicon, release, one hart | ~1.3 us | ~93 minutes |
| threads, 256 | `spawn_reap`, radon (JH7110, release kernel) | 19.8 us (79.3 ticks of the 4 MHz timer) | ~23.6 hours |
| memory regions, 256 | `SPLIT` then `DESTROY` from EL0 | not measured; two traps cost at least 54 ns at the 27 ns null syscall | at least ~3.9 minutes |

The rendezvous (512), notification, timer and process tables (256 each) free a slot only through a
region teardown, so their loops carry the region's cycle and more. The address-space registry
(288) and the virtio table (12) sit behind `spawn_el0` (~4.4 us) and a driver's death. More harts
do not speed up one slot: first-fit puts concurrent inserts in different slots.

So a wrapping generation is reachable. A user program with a region could resurrect a dead region name in
minutes of tight looping, and a kernel spawn loop could do it to a thread id inside two hours.
Under saturation the same loops retire a slot instead. Retiring all 256 thread slots takes about
16.5 days of nonstop spawning on HVF, and all 256 region slots at least 16 hours at the region
floor. After that, those creates fail as a full table does until the next boot.

Three designs, built in the crate and measured on the host (`Table<u64, N>`, one core of a
2.1 GHz Xeon Platinum 8160, best of five runs of five million insert-and-remove pairs, after a
settling churn; the walk is `values().count()`):

| `N`, live | first-fit (wraps) | saturate | next-fit | next-fit and saturate |
|---|---|---|---|---|
| 256, 1 | 6.9 ns, walk 3.1 ns | 6.7 ns, 2.5 ns | 83.9 ns, 31.5 ns | 80.7 ns, 20.1 ns |
| 256, 16 | 13.1 ns, 7.3 ns | 13.8 ns, 7.1 ns | 82.2 ns, 20.1 ns | 82.9 ns, 19.1 ns |
| 256, 64 | 31.9 ns, 17.3 ns | 27.6 ns, 15.5 ns | 66.8 ns, 26.7 ns | 71.4 ns, 22.0 ns |
| 256, 200 | 99.9 ns, 49.8 ns | 100.1 ns, 47.2 ns | 39.2 ns, 54.6 ns | 35.4 ns, 57.9 ns |
| 512, 16 | 11.1 ns, 13.3 ns | 14.3 ns, 7.5 ns | 198 ns, 107 ns | 152 ns, 99 ns |
| 512, 64 | 31.2 ns, 19.0 ns | 30.9 ns, 18.2 ns | 175 ns, 100 ns | 140 ns, 116 ns |
| 512, 400 | 166 ns, 102 ns | 178 ns, 111 ns | 61.8 ns, 104 ns | 55.8 ns, 110 ns |

Saturation costs nothing measurable: one more compare per scanned slot. Next-fit loses wherever the
table is sparse, which is where the kernel's tables live. The scan itself gets cheaper, but each
insert lands above every live entry, raising the occupancy bound, and the removal then walks the
bound back down over the gap. Only first-fit keeps that walk's cost at O(1) on average. Next-fit also
leaves live entries spread across the table, so every walk costs up to `N` again, the cost the
bound exists to remove. Next-fit wins only near full (200 of 256), where first-fit scans past every
live slot.

The kernel's icount bench measures the -O0 build the gate uses, here against the same tree without
saturation on the same host. There, the insert scan that tests the generation inside the scan costs
`spawn_reap` +1.07% on aarch64, +1.16% on riscv64 and +1.20% on x86_64, and `spawn_el0` +0.96% and
+1.04%. Every other row is unchanged. That is the form in the crate, and all five harnesses prove in
18 seconds. A second form keeps the plain scan for a free slot and tests the generation only on the
slot it finds, restarting past a retired one. It reads every row of all three architectures exactly
as the tree without saturation does, and matches first-fit in a release build at `N` = 256. But CBMC
did not finish the two harnesses that start from arbitrary generations within 300 seconds each. The
crate keeps the form whose proofs finish, and the kernel accepts its `spawn_reap` cost.

Next-fit is also visible to the kernel. Fresh-table names are the same (0, 1, 2 and on), but after
a removal the next name lands at the cursor and not in the hole, and
`a_name_to_a_released_device_is_refused_and_never_names_its_successor` in `kernel/src/virtio/tests.rs`
asserts that the successor reuses the released slot. And it does not delay exhaustion: `N` times
2^32 inserts retire every slot under either order, so next-fit only trades a gradual shrink for all
slots retiring at once.

Saturation is visible to no kernel caller before a slot's 2^32 - 1th occupant. Names, slot order,
`top` and every walk stay the same. A retired slot reads to the caller as a full table, which every
kernel caller already handles (spawn fails as it does on running out of memory).

A fourth design removes retirement from reach rather than making it safe. A name needs only as
many slot bits as `N` takes, eight for 256, which leaves 56 for the generation. Then 2^56 cycles at
the region floor is about 120 years. It costs 4 more bytes per slot (the generations become `u64`),
and it changes every name minted after a slot's first reuse. It also needs the two places that
decode the packing by hand, `slot_of` in `kernel/src/sched/thread_table.rs` and the virtio test
above, to take the decoder from the crate. Saturation stays under it either way, since saturation
is what makes the proof unconditional.

### The rule

The table saturates and retires, alone. Saturation closes the soundness
hole, its proof needs no precondition, and nothing a kernel caller sees changes until a slot's
2^32 - 1th occupant. The kernel accepts its `spawn_reap` cost of about 1.1%. Next-fit is out: it is
slower on sparse tables, undoes the occupancy bound, changes which slot a name lands in after a
removal, and retires no later. The wider generation waits until a measured capacity cost asks for
it. That cost is a program that creates and destroys objects without pause and so retires slots,
soonest in the region table, at a floor of about four minutes a slot. `MAXIMUM_REGIONS` and
`MAXIMUM_THREADS` carry it in their `BUGS`.

### The one-slot `Option` tag

`[Option<T>; N]` spends a tag on each slot when `T` has no niche. In the kernel that means the four
`u64` tables (rendezvous 512, notifications, timers and processes 256 each) and the thread table's
raw pointer (256). Each slot takes 16 bytes instead of 8, 12 KiB of the kernel's static data in all.
`Region` holds a `bool`, and the virtio `Device` and the address-space `Registered` entry hold
enums, so each has a niche, and Rust folds the tag into it at no cost. Taking the tag out means
storing an uninitialized `T` in a free slot, `MaybeUninit` and `unsafe`, in a crate that forbids
`unsafe_code`; a `T: Default` bound would avoid that but cannot hold the address-space entry. Leave
it: 12 KiB does not pay for the first `unsafe` in a proved crate.

## The occupancy bound

The table keeps `top`, one past the highest live slot. Every slot at or above it is empty, so a walk
bounded by `top` sees exactly what a walk over all `N` slots would. Insert is first-fit, so
occupancy packs toward slot 0 and `top` stays close to `len`. A walk then costs what the table holds
rather than what it could hold.

The bound exists because sweep cost was tracking `N`. The iterators yield only live entries but
visited every slot to filter them. `sched::delete_page_frame_capabilities_where` walks every thread's
capability table on every `MemoryRegion::DESTROY`, so it got dearer whenever
`sched::MAXIMUM_THREADS` rose, with no more live threads. On the `spawn_el0` icount benchmark,
raising that ceiling from 128 to 256 cost 141,359 ticks through this path alone
(notes/benchmarks.md). Every later raise would have paid again.

`top` shrinks as well as grows, so it tracks live occupancy rather than peak occupancy. Removing the
top entry walks down over empty slots and stops at the first live one. That is amortized O(1),
because a step down happens at most once per slot that was filled and freed. The walk is a `for`
over `N` with a `break`, since CBMC cannot see `top <= N` from the field and would unwind a `while`
without limit.

A bound that came down too far is the one way a walk could miss a live entry, so
`a_bounded_walk_yields_every_live_entry` proves it cannot. It removes an arbitrary subset of three
filled slots rather than running an arbitrary operation sequence. The sequence version did not
finish, and `script/verify` is already the merge queue's long pole.

## Iteration names

`values` and `values_mut` yield the live entries alone, by shared and by mutable reference. `iter`
yields `(name, &T)` pairs and `iter_from` yields `(slot, name, &T)`, for a sweep that must remove
what it finds. The names follow the standard library: `iter` yields what the collection holds
together with its key, as `HashMap::iter` does, and `values_mut` pairs with `values` as
`HashMap::values_mut` does.

## Resumable sweeps

`iter_from` yields `(slot, name, &T)` from a given slot onward. The slot index is what lets a sweep
resume after a caller gives its lock back between entries. A position in the filtered sequence
would move when entries in the gap are removed. A slot index does not: if the entry at slot `k`
died, `k` is empty rather than somebody else's.

`rendezvous::SURVEY` (milestone 126 (the `procps` package, now `process-tools`)) reports one
supervised thread per syscall and hands the next slot back to userspace as a cursor. The set can
change between calls, so a resumed sweep is a sequence of snapshots rather than one. It never
reports an entry twice and never resolves a cursor to the wrong thread, which is all a `readdir`
shape ever promised.

## What is proved (milestone 18 style, before anything leans on it)

Five Kani harnesses in `crates/generational_table` (`script/verify`):

| Harness | Property |
|---|---|
| `a_removed_name_never_resolves_again` | from any starting generations, after remove the name fails `get`/`get_mut`/`remove` even once the slot is reused, and the reuser's name differs |
| `a_removed_name_stays_dead_whatever_comes_after` | from any reachable table, any operations keep a dead name's slot generation above the name's and every live slot below `u32::MAX`, so by induction a removed name never resolves, however many reuses follow |
| `live_names_are_distinct_and_resolve_to_their_own_entry` | the packing cannot alias two live entries |
| `a_name_the_table_never_minted_resolves_to_nothing` | for any u64, resolution succeeds only on exactly the name the table issued: names cannot be forged |
| `a_bounded_walk_yields_every_live_entry` | after removing any subset of three filled slots, every walk bounded by `top` reports exactly `len` entries |
