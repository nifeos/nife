# generational_table and the existing crates

*Name: provisional, minted 2026-11-05 (UTC) by lane/generational-table-vs-slotmap.*

The release record for `crates/generational_table` owes one answer before publication:
whether `slotmap`, or a crate like it, already covers an allocation-free, fixed-capacity generational
table with what nife needs from one. This note gives that answer from the source of each candidate,
a host experiment and five Kani harnesses, and ends with the question an architect rules on. The
mechanism itself is in [Generational names](generational-names.md).

The short answer: no published crate covers it. `slotmap` and `thunderdome` cover the growable,
heap-backed case well and are the right choice there. The one fixed-capacity crate with
generations, `coca`, has had no release since 2022 and almost no users. What separates
nife's table is the combination of no allocation, a `const` constructor, a name format the kernel
already relies on, and proofs, one of which `slotmap` fails.

## What nife needs from the table

`generational_table::Table<T, N>` (550 lines with its tests and harnesses, no dependencies, no
`unsafe`) stores `[Option<T>; N]` beside `[u32; N]` generations. It names an entry with a `u64`:
the slot's generation in the high 32 bits and the slot index in the low 32. Remove bumps the slot's
generation with a wrapping add, so every copy of the old name fails the compare from then on, slot
reused or not. The kernel holds seven tables of it (threads, rendezvous, notifications, timers,
processes, address spaces, virtio devices) and `crates/memory_regions` holds one more.

Each of these is a requirement some kernel code depends on today:

1. No allocation, capacity fixed at compile time. The kernel has no global allocator. `N` is a
   const generic and a full table refuses an insert rather than growing.
2. A `const fn new`. Every kernel table lives in a `static` behind a lock
   (`kernel/src/sched/ipc_tables.rs`), so the constructor must run at compile time.
3. The first name in a fresh table is `0`. That is the boot thread's id. No name equals
   `u64::MAX`, which is `cpu::NO_TID`.
4. The slot index is reachable. `slot_of` returns it, because the thread table keeps control
   blocks in a parallel pool where pool slot *i* is table slot *i*.
5. A resumable walk. `iter_from(slot)` yields `(slot, name, &T)` from a slot onward, and
   `rendezvous::SURVEY` hands that slot to userspace as a cursor. Every walk stops at `top`, one past
   the highest live slot, so it costs what the table holds and not `N`.
6. A name the table never minted resolves to nothing. Names arrive in syscall registers, so any
   `u64` at all must either fail or land on the entry whose minted name is exactly that `u64`. Kani
   proves this, together with three other properties (a removed name never resolves again, live
   names are distinct, a bounded walk sees every live entry), each with a falsification patch.

Its stated limits: insert scans first-fit in O(N), and a slot reused 2^32 times resolves an ancient
name again.

## The candidates

Versions read are the newest on crates.io on 2026-11-05 (UTC). Sources came from the published
`.crate` files; downloads and reverse dependencies come from the crates.io API on the same day, and
repository state from the GitHub API.

| crate | version, release | license | 90 days / all time | reverse deps | repository |
|---|---|---|---|---|---|
| `slab` | 0.4.12, 2026-01-31 | MIT | 282M / 1.10G | (not counted) | tokio-rs/slab, active |
| `slotmap` | 1.1.1, 2025-12-06 | Zlib | 35.8M / 128M | 571 | orlp/slotmap, active |
| `generational-arena` | 0.2.9, 2023-05-22 | MPL-2.0 | 1.21M / 10.7M | 57 | archived on GitHub |
| `thunderdome` | 0.6.1, 2023-06-25 | MIT OR Apache-2.0 | 226k / 1.17M | 32 | LPGhatguy/thunderdome, pushed 2026-09 |
| `typed-generational-arena` | 0.2.9, 2025-05-26 | MPL-2.0 | 11.9k / 98k | (not counted) | GitLab |
| `sparseset` | 1.0.1, 2022-04-16 | MIT | 173 / 11k | (not counted) | bombela/sparseset |
| `coca` | 0.3.0, 2022-03-04 | Zlib OR Apache-2.0 | 18 / 4.4k | 0 | teryror/coca, last push 2022-03 |
| `slots` | 0.4.0, 2022-03-25 | MIT | 18 / 8.8k | (not counted) | archived on GitHub |
| `dense-slotmap-mem` | 0.1.1, 2025-12-29 | MIT | 746 / 1.3k | (not counted) | piot/dense-slotmap-mem |

A crates.io search for fixed-capacity, `no_std` and const-generic slot maps found `coca`, `slots` and
`dense-slotmap-mem`, and nothing else with generations. `heapless` 0.9.3, the usual home for
fixed-capacity collections, has no generational map.

### How each one stores its entries

| crate | storage | fixed `N` | `const new` | handle | generation and overflow | free slots |
|---|---|---|---|---|---|---|
| nife | `[Option<T>; N]` + `[u32; N]` | yes | yes | `u64`, gen high, slot low; first is `0` | per slot, `u32`, wraps after 2^32 reuses | first-fit scan, lowest slot |
| `slotmap` | `Vec`, sentinel at index 0 | no | no | `KeyData { idx: u32, version: NonZeroU32 }`; `as_ffi` packs version high, index low; first is `0x1_0000_0001` | per slot, odd while occupied, so wraps after 2^31 reuses (its docs say so) | intrusive LIFO list |
| `thunderdome` | `Vec` | no | yes (`Vec::new`) | `Index { slot: u32, generation: NonZeroU32 }`; `to_bits` packs generation high, slot low; first is `0x1_0000_0000` | per slot, `checked_add(1).unwrap_or(1)`, so it wraps past zero after 2^32 - 1 | LIFO list |
| `generational-arena` | `Vec` | runtime only (`try_insert` refuses when full) | no | `(usize, u64)`, 16 bytes | one `u64` counter for the whole arena, bumped on remove | LIFO list |
| `typed-generational-arena` | `Vec` | runtime only | no | typed, index and generation types chosen by the user | one counter for the whole arena; the default `NonzeroGeneration` panics on overflow, `Wrap` variants wrap | LIFO list |
| `coca` `DirectPool` | inline storage, `MaybeUninit` | yes | no | `u64`, generation high, index low; generation odd, so 31 bits; first is `0x1_0000_0000` | per slot, masked wrap after 2^31 | free list |
| `slab` | `Vec` | no | yes | `usize`, no generation | none | LIFO list |
| `sparseset` | two `Vec`s | no | no | `usize`, no generation | none | dense swap-remove |
| `slots` | `[Entry; N]` | yes | no | a non-`Copy` key object owned by the caller; the unrestricted variant takes a bare `usize` | none | free list |
| `dense-slotmap-mem` | a caller-supplied raw byte region | yes, runtime `u16` | n/a | `(u16 id, u16 generation)` | per slot, `u16` | free stack |

Only nife's table and `coca` are allocation-free, fixed at compile time and generational. Every
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

None of the candidates carries machine-checked proofs. `slotmap` runs Miri over one test and has
had two soundness bugs in its `unsafe` code, both fixed: a double free in `HopSlotMap` (#69, 2021)
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

The size row is a cost of nife's design, stated plainly. `[Option<T>; N]` spends a tag per slot on
any `T` without a niche, and the kernel's thread table stores a raw pointer, which has none. `coca`
keeps occupancy in the generation's low bit and stores values in `MaybeUninit`, which is how it fits
in 60% of the space, and its 382 `unsafe` lines are the price of that.

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

The same run shows that Kani handles the `Vec`-backed crates at these bounds as quickly as it
handles nife's, so the proofs are not a reason ours could exist and theirs could not. They are a
reason ours does exist: the four harnesses ship with the crate, each with a patch that shows it can
fail, and no candidate ships any.

## Could the kernel adopt one instead

- `slotmap`: no. It needs an allocator the kernel does not have, it cannot construct in a
  `static`, its names start at `0x1_0000_0001`, it hides the slot index, and it fails the forgery
  property above. Working around all of that means a wrapper larger than the table.
- `thunderdome`: the closest in API. It exposes slots, has `get_by_slot`, keeps the same packing
  and passes both harnesses. It still needs an allocator and grows rather than refusing, its first
  name is not `0`, and its last release was 2023-06-25.
- `generational-arena` and `typed-generational-arena`: both need an allocator, both carry an
  arena-wide counter rather than one per slot, and its author archived the first.
- `coca`: the only true match on storage. It has no release or commit since March 2022, no
  reverse dependencies, 18 downloads in 90 days, a non-`const` constructor, a first name of
  `0x1_0000_0000`, no slot cursor and 382 `unsafe` lines. Taking it would put a dormant crate
  with heavy `unsafe` into the kernel's shipping graph, which §46 (thin primitives or whole
  subsystems) exists to prevent.
- `slab`, `sparseset`, `slots`: no generations, so a stale index reaches the next
  occupant. That is the bug the table exists to remove.

The answer holds if both options were the same amount of work. Adopting `slotmap` would cost the
kernel an allocator and a proved property, and adopting `coca` would trade a table with no `unsafe`
for a dormant one with 382 lines of it; neither trade buys anything nife lacks.

## What the demand says

The measured demand sits almost entirely with growable tables: `slotmap` takes 35.8M downloads in
90 days, and the archived `generational-arena` still takes 1.21M with 57 dependents. The demand for
the fixed-capacity, allocation-free kind is close to nothing: `coca` and `slots` take 18 each, and
`dense-slotmap-mem` 746. A published `generational_table` would not capture `generational-arena`'s
orphaned users, because those users grow their arenas.

The users a fixed-capacity table can serve are kernels, firmware and embedded code with no heap,
which today either write their own or accept `coca`. That audience is small and does not show up in
download counts, because it writes its own. The crate name `generational_table` is free on
crates.io. The cost of publishing is the release machinery that milestone 887 (crate repositories share one CI template) already built, and the
risk is a crate few people find.

## Recommendation, as ruled

Publish, with the difference stated. No existing crate covers what the kernel needs, and the
kernel cannot adopt one without an allocator or a dormant `unsafe`-heavy dependency. The README's
first paragraph should say what the crate is for: fixed capacity, no allocation, a `const`
constructor usable in a `static`, unforgeable `u64` names with a proof, and a resumable walk. It
should also say what it is not for, and send growable tables to `slotmap` or `thunderdome`. It should name
the costs in its `BUGS` section: O(N) first-fit insert, one `Option` tag per slot, and the 2^32
wrap.

Two things stay open after this ruling and are not asked here, one decision at a time. Whether the
public name type stays a bare `u64` or becomes a typed key (every growable candidate uses a newtype,
and a newtype is the stricter choice, since widening later is compatible and narrowing is not). And
whether the README states expected demand as low, in the words of the measurement above.

## The ruling

nife publishes `generational_table` as its own crate, as the fixed-capacity, allocation-free,
proved member of the slotmap family (ruled 2026-11-05 UTC). Its README states how it differs from
the growable crates and lists its costs in `BUGS`. Users who need growth are pointed to `slotmap`
and `thunderdome`. The kernel keeps the table; it does not adopt `coca`, or `thunderdome` behind an
allocator. Milestone 1033 (`generational_table`: the fixed-capacity, proved slotmap, released on its
own) owns the move, the soak, the publication and nife consuming the release.
