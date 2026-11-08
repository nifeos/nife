# generational_table

A fixed-capacity table that names each entry with a typed `u64`, so a name outlives nothing it
named. Removing an entry raises its slot's generation, and every copy of the old name then
resolves to `None` for good, whether or not the slot has been filled again. The table is an array
of `N` slots chosen at compile time: it is `no_std`, needs no allocator, has no dependency, and
forbids `unsafe` code. Game engines call the same shape a slotmap.

Use it where you keep many short-lived records and hold references to them as plain words (in a
packet, a register, an atomic, a file). It suits a program that needs a reference to a dead record
to fail safely rather than dangle or reach somebody else's record. It is not for a collection that must grow
past a capacity known up front; see the comparison below.

## EXAMPLES

```rust
use generational_table::Table;

// Room for four sessions. `insert_with` hands the new entry its own name.
let mut sessions: Table<(u32, &str), 4> = Table::new();
let alice = sessions.insert_with(|name| (name.to_bits() as u32, "alice")).unwrap();
let bob = sessions.insert_with(|_| (0, "bob")).unwrap();
assert_eq!(sessions.get(alice).map(|s| s.1), Some("alice"));
assert_eq!(sessions.len(), 2);

// End Alice's session and let someone else take her slot.
assert!(sessions.remove(alice).is_some());
let carol = sessions.insert_with(|_| (0, "carol")).unwrap();

// Alice's old name does not reach Carol. It reaches nothing.
assert_ne!(alice, carol);
assert!(sessions.get(alice).is_none());
assert_eq!(sessions.get(carol).map(|s| s.1), Some("carol"));
assert_eq!(sessions.get(bob).map(|s| s.1), Some("bob"));

// A name crosses a boundary that carries a plain word, and comes back.
let word: u64 = carol.to_bits();
let again = generational_table::EntryName::<(u32, &str)>::from_bits(word);
assert_eq!(sessions.get(again).map(|s| s.1), Some("carol"));

// A word the table never minted resolves to nothing, whatever it holds.
assert!(sessions.get(generational_table::EntryName::NEVER_MINTED).is_none());
```

`Table::<T, N>::new` is a `const fn`, so a table can sit in a `static`. A name is an
`EntryName<T>`, which packs the generation in the high 32 bits of a `u64` and the slot in the low
32. It carries the entry type `T` as a tag only, so a name from a table of one entry type does not
compile where another table's name is expected. Two tables of the same entry type share a name
type; give each its own entry type (a one-field wrapper is enough) to keep them apart.

Besides `insert_with`, `get`, `get_mut` and `remove`, the table has `slot_of`, `len`,
`is_empty`, `values`, `values_mut`, `live_slots`, `iter` (each name with its entry) and
`iter_from` (a sweep that resumes from a slot index after you give a lock back). The walks cost
what the table holds, not what it could hold.

## What is proven

Five Kani harnesses prove the safety claims. Kani is a model checker that proves properties for
all inputs up to a stated size. The proofs need Kani, which `rustup` does not install. Run
`cargo install --locked
kani-verifier --version 0.68.0`, then `cargo kani setup`. The setup step
downloads a model checker, so it needs network access. CI uses version 0.68.0, and another
version may verify differently. After that, `cargo kani` from the crate's directory runs every
harness.

- `a_removed_name_never_resolves_again`: from any starting generations, the last one before
  retirement included, a removed name resolves through none of `get`, `get_mut` and `remove`,
  even after its slot is filled again.
- `a_removed_name_stays_dead_whatever_comes_after`: the induction step. From any reachable
  two-slot table, any two operations keep a dead name's slot generation above the name's and keep
  every live slot below the retired generation, so the claim holds after any number of
  operations.
- `live_names_are_distinct_and_resolve_to_their_own_entry`: two live entries never share a name,
  and each name reaches its own value.
- `a_name_the_table_never_minted_resolves_to_nothing`: for any `u64`, resolution fails or lands
  on the entry whose minted name is that number. A name cannot be forged.
- `a_bounded_walk_yields_every_live_entry`: after any subset of three entries is removed, every
  walk reports exactly `len` entries and the survivors still resolve.

Each harness has a falsification record at `falsifications/verification.<harness>.patch`.
Applying it makes that harness fail at its own assertion, which shows the proof checks
something. The proofs use tables of two or three slots and operation sequences of one or two
steps. The host tests cover the rest: a retired slot, a fully retired table, the hole below the
walk bound, and an insert that is refused without running its closure.

## BUGS

- Capacity is fixed. `insert_with` returns `None`, without running its closure, when every slot
  is live or retired.
- Generations are 32-bit and saturate; they never wrap. A slot whose generation reaches
  `u32::MAX` retires and is never filled again, so the table's capacity shrinks by one. A slot
  retires only after 2^32 - 1 occupants. A loop that does nothing but insert and remove one entry
  can get there in about half a minute (about 7 nanoseconds a cycle on a 2.1 gigahertz server core).
  A table whose every slot has retired refuses every insert for good. A program that creates
  and destroys entries without pause should treat a refused insert as a real outcome.
- `insert_with` scans from slot 0 for the first free slot, which is O(N). `get` is O(1).
- Storage is `[Option<T>; N]`, which spends a tag on each slot when `T` has no niche.
- `Table` implements neither `Debug` nor `Clone`.
- Expected demand is low. Fixed-capacity tables are a small corner of the ecosystem, and this
  crate does not try to be anything else.

## How it differs from slotmap and thunderdome

`slotmap` and `thunderdome` are the established, growable members of this family, and for most
programs one of them is the right choice. They keep their entries in a `Vec`, so they need an
allocator and grow without bound, and neither fixes its capacity at compile time. Their
generations wrap, `slotmap` after 2^31 reuses of a slot and `thunderdome` after 2^32 - 1, so
after that many reuses of one slot an ancient name can resolve to a new occupant. `slotmap` also
lets a plain `u64` convert into a key, and two such words can reach one entry. This crate trades
growth for the opposite properties: a fixed array, no allocation, a `const` constructor, a
generation that retires a slot rather than wrap, names that cannot be forged from a word, and
machine-checked proofs of those claims. It costs you the capacity you choose up front and a slot
now and then in a very long-lived table.

## Minimum Rust version

Rust 1.88.

## License

Licensed under either of the MIT license ([LICENSE-MIT](LICENSE-MIT)) or the Apache License,
Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE)), at your option.
