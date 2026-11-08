# generational_table and the existing crates

*Name: provisional, minted 2026-11-05 (UTC) by lane/generational-table-vs-slotmap.*

The release record for `nifeos/generational_table` owes one answer before publication:
whether `slotmap`, or a crate like it, already covers an allocation-free, fixed-capacity generational
table with what nife needs from one. This note answers from each candidate's source, a host experiment and Kani
harnesses run against the candidates, and ends with the ruling. The
mechanism itself is in [Generational names](generational-names.md).

The short answer: no published crate covers it. `slotmap` and `thunderdome` cover the growable,
heap-backed case well and are the right choice there. The two fixed-capacity crates with
generations, `coca` and `triple_arena`'s stack backing, have no `const` constructor and keep their
storage in `unsafe` code, and `coca` has had no release since 2022. What separates
nife's table is the combination of no allocation, a `const` constructor, a name format the kernel
already relies on, and proofs, one of which `slotmap` fails.

## What nife needs from the table

`generational_table::Table<T, N>` (about 820 lines with its tests and harnesses, no dependencies,
`#![forbid(unsafe_code)]`) stores `[Option<T>; N]` beside `[u32; N]` generations. It names an entry with an
`EntryName<T>`, a `u64` tagged with the entry type: the slot's generation in the high 32 bits and
the slot index in the low 32. Remove bumps the slot's
generation with a saturating add, so every copy of the old name fails the compare from then on, slot
reused or not. A slot whose generation reaches `u32::MAX` retires and is never filled again. The kernel holds seven tables of it (threads, rendezvous, notifications, timers,
processes, address spaces, virtio devices) and `crates/memory_regions` holds one more.

Each of these is a requirement some kernel code depends on today:

1. No allocation, capacity fixed at compile time. The kernel has no global allocator. `N` is a
   const generic and a full table refuses an insert rather than growing.
2. A `const fn new`. Every kernel table lives in a `static` behind a lock
   (`kernel/src/sched/ipc_tables.rs`), so the constructor must run at compile time.
3. The first name in a fresh table is `0`. That is the boot thread's id. No table mints
   `u64::MAX` (`EntryName::NEVER_MINTED`), which is `cpu::NO_TID`.
4. The slot index is reachable. `slot_of` returns it, because the thread table keeps control
   blocks in a parallel pool where pool slot *i* is table slot *i*.
5. A resumable walk. `iter_from(slot)` yields `(slot, name, &T)` from a slot onward, and
   `rendezvous::SURVEY` hands that slot to userspace as a cursor. Every walk stops at `top`, one past
   the highest live slot, so it costs what the table holds and not `N`.
6. A name the table never minted resolves to nothing. Names arrive in syscall registers, so any
   `u64` at all must either fail or land on the entry whose minted name is exactly that `u64`.

Its stated limits: insert scans first-fit in O(N), `[Option<T>; N]` spends a tag per slot on a `T`
with no niche, and a slot retires after 2^32 - 1 occupants, which shrinks the capacity by one.

## What the proofs cover

The crate ships five Kani harnesses, each with a falsification patch under
`falsifications/` that makes it fail, and the repository's `proofs` workflow runs all five and all five patches.

| harness | what it proves |
|---|---|
| `a_removed_name_never_resolves_again` | from any starting generations, the last before retirement included, a removed name resolves through none of `get`, `get_mut`, `remove`, and the slot's next occupant gets a different name |
| `a_removed_name_stays_dead_whatever_comes_after` | the induction step: from any reachable table, any operations keep a dead name dead and keep every live slot below the retired generation, so no number of reuses brings an old name back |
| `live_names_are_distinct_and_resolve_to_their_own_entry` | the packing never aliases two live entries |
| `a_name_the_table_never_minted_resolves_to_nothing` | any `u64` either resolves to nothing or is exactly the name the table minted |
| `a_bounded_walk_yields_every_live_entry` | the walks bounded by `top` see exactly `len` entries, holes included |

The harnesses prove the properties for tables of two and three slots. Why two slots exhibit every
case is argued in the harness module's doc comment, not proved.

## The candidates

Versions read are the newest on crates.io. Sources came from the published `.crate` files,
downloads and reverse dependencies from the crates.io API, and repository state from GitHub.

| crate | version, release | license | 90 days / all time | reverse deps | repository |
|---|---|---|---|---|---|
| `slab` | 0.4.12, 2026-01-31 | MIT | 288M / 1.11G | (not counted) | tokio-rs/slab, active |
| `heapless` | 0.9.3, 2026-04-30 | MIT OR Apache-2.0 | 57.5M / 176M | (not counted) | rust-embedded/heapless, active |
| `slotmap` | 1.1.1, 2025-12-06 | Zlib | 36.9M / 129M | 571 | orlp/slotmap, active |
| `generational-arena` | 0.2.9, 2023-05-22 | MPL-2.0 | 1.22M / 10.7M | 57 | archived on GitHub |
| `thunderdome` | 0.6.1, 2023-06-25 | MIT OR Apache-2.0 | 227k / 1.17M | 32 | LPGhatguy/thunderdome, pushed 2026-09 |
| `triple_arena` | 0.15.1, 2026-08-26 | MIT OR Apache-2.0 | 226k / 352k | 3 | AaronKutch/triple_arena |
| `atomic-pool` | 2.0.0, 2025-01-02 | MIT OR Apache-2.0 | 63.7k / 520k | (not counted) | embassy-rs/atomic-pool |
| `typed-generational-arena` | 0.2.9, 2025-05-26 | MPL-2.0 | 11.9k / 98.6k | (not counted) | GitLab |
| `sparseset` | 1.0.1, 2022-04-16 | MIT | 175 / 11.5k | (not counted) | bombela/sparseset |
| `coca` | 0.3.0, 2022-03-04 | Zlib OR Apache-2.0 | 19 / 4.4k | 0 | teryror/coca, last push 2022-03 |
| `slots` | 0.4.0, 2022-03-25 | MIT | 18 / 8.8k | (not counted) | archived on GitHub |
| `dense-slotmap-mem` | 0.1.1, 2025-12-29 | MIT | 719 / 1.3k | (not counted) | piot/dense-slotmap-mem |

A crates.io search for fixed-capacity, `no_std` and const-generic slot maps found `coca`, `slots`,
`dense-slotmap-mem` and `triple_arena`'s stack backing, and nothing else with generations.
`heapless` 0.9.3, the usual home for fixed-capacity collections, has no generational map. Its
`pool` module (`BoxPool`, `ArcPool`, `ObjectPool`) hands out owning boxes from a static Treiber
stack, in 86 lines that use `unsafe`, and gives the caller no handle it can store as a word, so
nothing can go stale. `atomic-pool` has the same shape. Neither names an entry, so neither is a
generational table. `triple_arena`'s 226k recent downloads arrive through its dependents,
`recasting` and `awint_macro_internals`.

### How each one stores its entries

| crate | storage | fixed `N` | `const new` | handle | generation and overflow | free slots |
|---|---|---|---|---|---|---|
| nife | `[Option<T>; N]` + `[u32; N]` | yes | yes | `EntryName<T>`, a typed `u64`, generation high, slot low; first is `0` | per slot, `u32`, saturates: the slot retires after 2^32 - 1 occupants and never wraps | first-fit scan, lowest slot |
| `slotmap` | `Vec`, sentinel at index 0 | no | no | `KeyData { idx: u32, version: NonZeroU32 }`; `as_ffi` packs version high, index low; first is `0x1_0000_0001` | per slot, odd while occupied, so wraps after 2^31 reuses (its docs say so) | intrusive LIFO list |
| `thunderdome` | `Vec` | no | yes (`Vec::new`) | `Index { slot: u32, generation: NonZeroU32 }`; `to_bits` packs generation high, slot low; first is `0x1_0000_0000` | per slot, `checked_add(1).unwrap_or(1)`, so it wraps past zero after 2^32 - 1 | LIFO list |
| `generational-arena` | `Vec` | runtime only (`try_insert` refuses when full) | no | `(usize, u64)`, 16 bytes | one `u64` counter for the whole arena, bumped on remove | LIFO list |
| `typed-generational-arena` | `Vec` | runtime only | no | typed, index and generation types chosen by the user | one counter for the whole arena; the default `NonzeroGeneration` panics on overflow, `Wrap` variants wrap | LIFO list |
| `coca` `DirectPool` | inline storage, `MaybeUninit` | yes | no | `u64`, generation high, index low; generation odd, so 31 bits; first is `0x1_0000_0000` | per slot, masked wrap after 2^31 | free list |
| `triple_arena`, `StackBacking<LIMIT>` | `[MaybeUninit<_>; LIMIT]` | yes; `insert_within_capacity` refuses when full | no (`Arena::new` is a trait method built on `array::from_fn`) | a caller type from its `ptr_struct!` macro: `NonZero` index and an optional `NonZeroU64` generation; first has index 1, generation 2 | one counter for the whole arena, skipping 1 on overflow | free list |
| `slab` | `Vec` | no | yes | `usize`, no generation | none | LIFO list |
| `sparseset` | two `Vec`s | no | no | `usize`, no generation | none | dense swap-remove |
| `slots` | `[Entry; N]` | yes | no | a non-`Copy` key object owned by the caller; the unrestricted variant takes a bare `usize` | none | free list |
| `dense-slotmap-mem` | a caller-supplied raw byte region | yes, runtime `u16` | n/a | `(u16 id, u16 generation)` | per slot, `u16` | free stack |

Only nife's table, `coca` and `triple_arena` on its stack backing are allocation-free, fixed at
compile time and generational. Every
candidate with generations uses nife's packing (generation high, index low) in some form, and none
of them mints `0` first: each starts generations at one or keeps them odd, to give `Option<Key>` a
niche or to mark occupancy.

### What each one offers beyond insert, get and remove

| crate | slot index exposed | walk from a slot | walk bounded by highest live slot | `unsafe` lines (grep) |
|---|---|---|---|---|
| nife | `slot_of` | `iter_from` | yes | 0 |
| `slotmap` | only through `as_ffi` (issue #105 asks; its author answered that 2.0 will expose it) | no | no, walks the `Vec` | 122 |
| `thunderdome` | `Index::slot`, `get_by_slot`, `insert_at_slot` | buildable on `get_by_slot` | no | 4 |
| `generational-arena` | `into_raw_parts` | no | no | 0 (`forbid(unsafe_code)`) |
| `typed-generational-arena` | yes | no | no | 0 (`forbid(unsafe_code)`) |
| `coca` | `into_raw_parts` | no | no | 382 across the crate |
| `triple_arena` | `Ptr::inx` | no | no | 72 across the crate |

None of the candidates carries machine-checked proofs. Against their documented behavior, nife's
harnesses prove three things none states:

- No forgery. `slotmap`'s `from_ffi` maps two `u64`s to one key (below). `thunderdome`'s
  `from_bits` passes the same harness, so there nife differs by shipping the proof.
- Reuse without end. `slotmap` and `coca` wrap after 2^31 reuses of a slot and `thunderdome` after
  2^32 - 1, and past the wrap the first occupant's name resolves again. nife's slot retires instead,
  and the induction harness proves a dead name stays dead for any number of operations.
  `triple_arena` and `generational-arena` share one 64-bit counter, so a wrap is impractical rather
  than impossible.
- The walk bound. No candidate keeps one, so none has the property to prove.

The limit: the harnesses check two- and three-slot tables, and the other properties of `slotmap`
and `thunderdome` hold at those bounds too. The proofs are evidence for nife's code, not against
theirs. `slotmap` runs Miri over one test and has had two soundness bugs in its `unsafe` code, both fixed: a double free in `HopSlotMap` (#69, 2021)
and a leak in `Slot::clone_from` (#110, open from 2023-10 to 2025-12).

## The experiment

A host crate depending on `slotmap` 1.1.1 and `thunderdome` 0.6.1 (both with default features
off), `coca` 0.3.0 and `generational_table` printed the first names and sizes. Five Kani 0.68.0
harnesses checked two of nife's proved properties against the two maintained growable
crates.

What the host run printed:

| | first name | after remove and reinsert | size for 256 `u64` values |
|---|---|---|---|
| nife | `0x0` | `0x1_0000_0000` | 5,136 bytes, inline |
| `slotmap` | `0x1_0000_0001` | `0x3_0000_0001` | 32-byte header plus heap |
| `thunderdome` | `0x1_0000_0000` | `0x2_0000_0000` | 32-byte header plus heap |
| `coca` | `(0, 1)`, packing to `0x1_0000_0000` | | 3,080 bytes, inline |

`thunderdome::Arena::new` compiled inside a `static`, as nife's does, and `slotmap::SlotMap::new`
cannot, because it pushes the sentinel slot.

The size row is a cost of nife's design: one `Option` tag per slot, and the kernel's thread table
stores a raw pointer, which has no niche. `coca` fits in 60% of the space by keeping occupancy in the
generation's low bit and values in `MaybeUninit`, at the price of its 382 `unsafe` lines.

The Kani results, each harness at unwind 4 and each under five seconds:

| property | nife | `slotmap` | `thunderdome` |
|---|---|---|---|
| a removed name never resolves again, slot reused | proved | proved | proved |
| a `u64` the table never minted resolves to nothing | proved (in the crate) | fails | proved, through `from_bits` |

The failure is the concrete example of what separates the two. `KeyData::from_ffi` forces the
version odd (`(value >> 32) | 1`), so two different `u64`s decode to the same key. In the host run, a
map holding one live entry named `0x3_0000_0001` resolves `from_ffi(0x2_0000_0001)` to that entry,
though the map never minted `0x2_0000_0001`. Its documentation allows this ("the behavior is safe but
unspecified" for a value that did not come from `as_ffi`), and for a game it does not matter. For a
kernel that takes names from syscall registers, it means two names reach one object, and nife's
claim that nobody can forge a name would weaken to "nobody can forge one, except in the low bit of
the generation". The harness that finds it:

```rust
#[kani::proof]
#[kani::unwind(4)]
fn slotmap_a_name_never_minted_resolves_to_nothing() {
    let mut s: SlotMap<DefaultKey, u8> = SlotMap::with_capacity(2);
    let k = s.insert(kani::any());
    let probe: u64 = kani::any();
    if s.get(KeyData::from_ffi(probe).into()).is_some() {
        assert_eq!(probe, k.data().as_ffi());
    }
}
```

Kani handled the `Vec`-backed crates as quickly as nife's. The difference is that nife's harnesses
ship with the crate, each with a patch that makes it fail.

## Could the kernel adopt one instead

- `slotmap`: no. It needs an allocator, cannot construct in a `static`, starts names at
  `0x1_0000_0001`, hides the slot index and fails the forgery property.
- `thunderdome`: the closest in API. It exposes slots, has `get_by_slot`, keeps the same packing
  and passes both harnesses. It still needs an allocator and grows rather than refusing, its first
  name is not `0`, and its last release was 2023-06-25.
- `generational-arena` and `typed-generational-arena`: both need an allocator, both carry an
  arena-wide counter rather than one per slot, and its author archived the first.
- `triple_arena` on `StackBacking`: the one maintained crate with an allocation-free,
  fixed-capacity generational arena. It does not fit the kernel. `Arena::new` is not `const`, so it
  cannot sit in a `static`. Its single arena-wide generation means a removal anywhere changes the
  generation every later insert carries. Its first handle has index 1 and generation 2, not `0`,
  and the handle is a struct, not a word a syscall register carries. It has no slot cursor, and its
  stack backing keeps `MaybeUninit` behind 72 lines of `unsafe` with no proofs.
- `coca`: the other true match on storage. It has no release or commit since March 2022, no
  reverse dependencies, 19 downloads in 90 days, a non-`const` constructor, a first name of
  `0x1_0000_0000`, no slot cursor and 382 `unsafe` lines. Taking it would put a dormant crate
  with heavy `unsafe` into the kernel's shipping graph, which §46 (thin primitives or whole
  subsystems) exists to prevent.
- `slab`, `sparseset`, `slots`: no generations, so a stale index reaches the next
  occupant. That is the bug the table exists to remove.

The answer holds if both options cost the same work: adopting any of them costs the kernel an
allocator, a `const` constructor or a proved property, and buys nothing nife lacks.

## What the demand says

The measured demand sits almost entirely with growable tables: `slotmap` takes 36.9M downloads in
90 days, and the archived `generational-arena` still takes 1.22M with 57 dependents. The demand for
the fixed-capacity, allocation-free kind is close to nothing: `coca` takes 19 and `slots` 18, and
`dense-slotmap-mem` 719. `triple_arena`'s 226k measure its two dependents, and nothing in the count
says how many use the stack backing. A published `generational_table` would not capture `generational-arena`'s
orphaned users, because those users grow their arenas.

The users a fixed-capacity table can serve are kernels, firmware and embedded code with no heap.
That audience is small and does not show up in download counts, because it writes its own. The crate name `generational_table` is free on
crates.io. The cost of publishing is the release machinery that milestone 887 (crate repositories share one CI template) already built, and the
risk is a crate few people find.

## Recommendation, as ruled

Publish, with the difference stated. No existing crate covers what the kernel needs, and the
kernel cannot adopt one without an allocator, or a non-`const` constructor and an `unsafe`-heavy
dependency with no proofs. The README's
first paragraph should say what the crate is for: fixed capacity, no allocation, a `const`
constructor usable in a `static`, unforgeable typed names with a proof, and a resumable walk. It
should also say what it is not for, and send growable tables to `slotmap` or `thunderdome`. It should name
the costs in its `BUGS` section: O(N) first-fit insert, one `Option` tag per slot, and a slot
retiring after 2^32 - 1 occupants.

The public name type is a typed key, `EntryName<T>`, as every growable candidate's is
(notes/generational-names.md, "Typed names"). One thing stays open after this ruling and is not
asked here: whether the README states expected demand as low, in the words of the measurement
above.

## The ruling

nife publishes `generational_table` as its own crate, as the fixed-capacity, allocation-free,
proved member of the slotmap family (ruled 2026-11-05 UTC). Its README states how it differs from
the growable crates and lists its costs in `BUGS`. Users who need growth are pointed to `slotmap`
and `thunderdome`. The kernel keeps the table; it does not adopt `coca`, or `thunderdome` behind an
allocator. Milestone 1033 (`generational_table`: the fixed-capacity, proved slotmap, released on its
own) owns the move, the soak, the publication and nife consuming the release.
