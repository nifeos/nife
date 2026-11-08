# Review package: `generational_table`

*Name: provisional, minted by lane lane/crate-review-packages-five. The `generational_table`
package of [crate-reviews.md](../crate-reviews.md), in its own file because the five packages
together outgrow the page.*

## In one paragraph

A fixed-capacity table that names each entry with an `EntryName<T>`, typed by the entry it names and
holding one `u64`: the slot index in the low 32 bits and the slot's generation in the high 32. Removing an entry raises its slot's generation, so every copy
of the old name resolves to `None` for good, while the slot itself serves a new entry. The kernel keeps its
thread table and its object tables in it. It is `no_std` with no allocator, no `unsafe` and no
dependency, and five Kani harnesses prove the safety claims. The architect ruled that it publishes,
third in the order of milestone 857 (crates released on their own), as milestone 1033
(`generational_table`: the fixed-capacity, proved slotmap, released on its own) records. Its
repository, `nifeos/generational_table`, carries its own manifest values with the `repository` key,
a README whose example a doctest runs, and both license files.

## Identity

| item | value |
|---|---|
| crate | `generational_table`, ratified (the crate name, the 14 item names, `EntryName<T>` and its `to_bits`, `from_bits` and `NEVER_MINTED`) |
| version | `0.1.0`; no tag |
| edition, `rust-version` | 2024; `rust-version` 1.88 |
| license | `MIT OR Apache-2.0`, written in its own manifest, with `LICENSE-MIT` and `LICENSE-APACHE` in the crate |
| `publish` | `false`; publication ruled, waiting on the move, the soak and the architect's approval of 0.1.0 |
| size | one `src/lib.rs` of 816 lines: about 400 of library, 155 of Kani harnesses, 260 of tests |
| consumers in nife | `kernel` and `memory_regions`, both by `git` and `rev` |

## Public API

One type, `Table<T, const N: usize>`, and its methods.

| group | items |
|---|---|
| build | `new` (a `const fn`; a `const` assertion refuses `N == 0` and `N > u32::MAX`), `Default` |
| change | `insert_with(f)`, which mints the name, hands it to `f` and stores what `f` returns; `remove(name)` |
| resolve | `slot_of(name)`, `get(name)`, `get_mut(name)`, each taking an `EntryName<T>` |
| count | `len`, `is_empty` |
| walk | `values`, `values_mut`, `live_slots`, `iter` (`EntryName<T>` and value), `iter_from(slot)` (slot, name and value) |

What a stranger may find surprising:

- A name is an `EntryName<T>`, a newtype over the `u64`, so the compiler tells it from any other
  number and a name of one entry type does not resolve in a table of another. Every growable
  candidate in [the release record](../generational-table-and-existing-crates.md) uses a newtype
  (`slotmap`'s `KeyData`, `thunderdome`'s `Index`). `EntryName<T>` has `to_bits`, `from_bits`
  and `NEVER_MINTED` (the name no table mints), and the kernel converts at its system-call
  boundary with them. Each kernel table has an entry type of its own, so a name of one table does
  not resolve in another. The rendezvous, notification, timer and process tables in
  `sched/ipc_tables.rs` hold `RendezvousPageAddress`, `NotificationPageAddress`, `TimerPageAddress`
  and `ProcessPageAddress`. Each is a one-word `repr(transparent)` newtype over the page's address.
  Four conflicting impls in that file stop the build if two of them ever share a type.
- A slot whose generation reaches `u32::MAX` retires and is never filled again, so capacity can
  shrink. The release record's table shows `slotmap` and `thunderdome` wrapping instead.
- `Table` implements neither `Debug` nor `Clone`. Adding either later breaks nobody.
- `insert_with` scans from slot 0 for the first free slot, O(N). `get` is O(1).

## Proofs and falsification patches

Five harnesses, each with one patch under `falsifications/`. `script/crate-standalone` proved all
five and saw each patch fail its own harness.

| harness | what it proves |
|---|---|
| `a_removed_name_never_resolves_again` | from any starting generations, the last before retirement included, a removed name resolves through none of `get`, `get_mut` and `remove`, even after its slot takes a new entry |
| `a_removed_name_stays_dead_whatever_comes_after` | the induction step: from any reachable two-slot table, two arbitrary operations keep a dead name's slot generation above the name's, and keep every live slot below the retired generation, so the claim holds after any number of operations |
| `live_names_are_distinct_and_resolve_to_their_own_entry` | two live entries never share a name, and each name reaches its own value |
| `a_name_the_table_never_minted_resolves_to_nothing` | for any `u64`, resolution fails or lands on the entry whose minted name is that number |
| `a_bounded_walk_yields_every_live_entry` | after any subset of three entries is removed, every walk reports exactly `len` entries and the survivors still resolve |

Left out of the proofs: tables of more than three slots, and operation sequences longer than two
in the walk harness. Host tests cover the retired slot, a fully retired table, the hole below the
walk bound and a refused insert that never runs its closure.

## Names

The crate has 19 names with a block, all ratified. They are the crate itself, the 14 items on #3119
(the standard library's spellings win: `len`, `iter`, `get_mut`), `EntryName<T>`, and its
associated items `EntryName::to_bits`, `EntryName::from_bits` and `EntryName::NEVER_MINTED`.
`to_bits` and `from_bits` take the standard library's spelling (`f64::to_bits`). `NEVER_MINTED`
beat `NONE`, which reads as `Option::None`, and `INVALID`, which it is not, since resolving it
answers `None`.

## Naming rules

Against the rulings in force, checked with the template's own checks
(`vendor/crate-workflows/helpers/`, run with `--manifest Cargo.toml` over `src/lib.rs`; each judged
122 names):

| rule | result |
|---|---|
| units spelled out (`unit_symbols.py`) | 0 findings. The crate names no quantity. |
| spelled-out words (`spelled_out.py`) | 0 findings. The checker judges only the words on its fixed list, so a clipped word that is not on it passes; the private field is `generations` and a read of every field, local, parameter and test name found no other clipped word. Single-letter names remain: the closure parameter `f` of `insert_with`, closure bindings `n`, `i` and `s`, and the test locals `a`, `b`, `c`, `d` and `t`. |
| question shape (`question_shape.py`) | 0 findings. `is_empty` is the only predicate, and it is the standard library's spelling. |
| British spelling (`british_spelling.py`) | 0 findings. |
| standard library spellings | `len`, `iter`, `get_mut`, `values_mut`, `is_empty`, `to_bits` and `from_bits` keep the standard library's names. |
| aliases of an old nife spelling | none. The two `doc(alias)` strings on `EntryName` (`generational index`, `slotmap key`) are terms a reader searches for. |

## License

`license = "MIT OR Apache-2.0"`. The crate adapts no source, so no `AND` term follows the pair. Its
documentation names `slotmap` as the same shape and copies nothing from it.

## What the release still needs

| item | owner |
|---|---|
| The tag `v0.1.0`, made by the release workflow. | calef |

## Proposed items

None. No defect found in the code.

## Open questions

None. The README says in one sentence that expected demand is low (the crates.io figures are in the
release record).

## Result of `script/crate-standalone`

Passes every check but Kani, which a run with `--no-kani` skips. The build on the host and the
three bare-metal targets passes on stable. A full run proved all five harnesses and saw all five
patches fail.

## Recommendation

Ready for review.
