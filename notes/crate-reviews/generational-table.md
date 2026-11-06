# Review package: `generational_table`

*Name: provisional, minted by lane lane/crate-review-packages-five. The `generational_table`
package of [crate-reviews.md](../crate-reviews.md), in its own file because the five packages
together outgrow the page.*

## In one paragraph

A fixed-capacity table that names each entry with an `EntryName<T>`, typed by the entry it names and
holding one `u64`: the slot index in the low 32 bits and the slot's generation in the high 32. Removing an entry raises its slot's generation, so every copy
of the old name resolves to `None` for good, while the slot itself is reused. The kernel keeps its
thread table and its object tables in it. It is `no_std` with no allocator, no `unsafe` and no
dependency, and five Kani harnesses prove the safety claims. The architect ruled that it publishes,
third in the order of milestone 857 (crates released on their own), as milestone 1033
(`generational_table`: the fixed-capacity, proved slotmap, released on its own) records. It is not
yet shaped for release: it has no README, no license files and no `rust-version`, and it inherits
two keys from nife's workspace.

## Identity

| item | value |
|---|---|
| crate | `generational_table`, ratified (the crate name, the 14 item names and `EntryName<T>`); three names on `EntryName<T>` are provisional |
| version | `0.1.0`; no tag in this clone |
| edition, `rust-version` | 2024; no `rust-version` |
| license | `MIT OR Apache-2.0`, inherited from the nife workspace; no license files in the crate |
| `publish` | `false`; publication ruled, waiting on the move, the soak and the architect's approval of 0.1.0 |
| size | one `src/lib.rs` of 816 lines: about 400 of library, 155 of Kani harnesses, 260 of tests |
| consumers in nife | `kernel` and `memory_regions`, both by path |

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
  boundary with them. Four kernel tables (`rendezvous_table`, `notification_table`, `timer_table`
  and `processes` in `sched/ipc_tables.rs`) hold bare `u64` entries, so each of their names is an
  `EntryName<u64>` and one table accepts another's names. The ruling is that each gets its own
  entry newtype; that work has not landed on `main`.
- A slot whose generation reaches `u32::MAX` retires and is never filled again, so capacity can
  shrink. The release record's table shows `slotmap` and `thunderdome` wrapping instead.
- `Table` implements neither `Debug` nor `Clone`. Adding either later breaks nobody.
- `insert_with` scans from slot 0 for the first free slot, O(N). `get` is O(1).

## Proofs and falsification patches

Five harnesses, each with one patch under `falsifications/`. `script/crate-standalone` proved all
five and saw each patch fail its own harness.

| harness | what it proves |
|---|---|
| `a_removed_name_never_resolves_again` | from any starting generations, the last before retirement included, a removed name resolves through none of `get`, `get_mut` and `remove`, even after its slot is filled again |
| `a_removed_name_stays_dead_whatever_comes_after` | the induction step: from any reachable two-slot table, two arbitrary operations keep a dead name's slot generation above the name's, and keep every live slot below the retired generation, so the claim holds after any number of operations |
| `live_names_are_distinct_and_resolve_to_their_own_entry` | two live entries never share a name, and each name reaches its own value |
| `a_name_the_table_never_minted_resolves_to_nothing` | for any `u64`, resolution fails or lands on the entry whose minted name is that number |
| `a_bounded_walk_yields_every_live_entry` | after any subset of three entries is removed, every walk reports exactly `len` entries and the survivors still resolve |

Left out of the proofs: tables of more than three slots, and operation sequences longer than two
in the walk harness. Host tests cover the retired slot, a fully retired table, the hole below the
walk bound and a refused insert that never runs its closure.

## Names

The crate has 16 names with a block: the crate on its naming review, the 14 items on #3119
under the ruling that the standard library's spellings win (`len`, `iter`, `get_mut`), and
`EntryName<T>`, ratified on 2026-11-06. Three more names are provisional: the associated items
`EntryName::to_bits`, `EntryName::from_bits` and `EntryName::NEVER_MINTED`. `to_bits` and
`from_bits` take the standard library's spelling (`f64::to_bits`); `NEVER_MINTED` was chosen over
`NONE` (which reads as `Option::None`) and `INVALID` (which it is not, since resolving it answers
`None`). Recommendation: ratify all three. **architect** (names).

## License

`license = "MIT OR Apache-2.0"`. The crate adapts no source, so no `AND` term follows the pair. Its
documentation names `slotmap` as the same shape and copies nothing from it.

## What the move still needs

| item | owner |
|---|---|
| A README with what the crate does, its proofs by harness name, how to install and run Kani, usage and limits. | move lane |
| `LICENSE-MIT` and `LICENSE-APACHE` at the crate root, the `license` key written out, `rust-version = "1.88"` (after a build at 1.88), and the lint table copied in from the workspace. | move lane |
| `src/lib.rs` line 12 cites `notes/generational-names.md`; the retirement arithmetic it points to moves into the crate's own documentation. | move lane |
| Shipped text speaks of the kernel's tables: "revocation walks every capability table", "a Thread learns its `ThreadId`", "the `NO_TID` sentinel". Each becomes a general example. | move lane |

## Proposed items

No defect found in the code. Three records disagree with it:

1. Milestone 1033's block says four harnesses and lists under `BUGS` that a slot reused 2^32 times
   resolves an ancient name again. The crate now saturates a slot's generation and retires it, and
   a fifth harness proves a dead name stays dead after any number of operations. The block and the
   release record's comparison table (`wraps after 2^32 reuses`) and recommended `BUGS` line
   ("the 2^32 wrap") need the same correction, and the block's open item on the name type has
   its answer, `EntryName<T>`. Owner: a lane, after the maintainer confirms.
2. The crate's limits do not say that a table whose every slot has retired refuses every insert
   for good. A test shows it (`a_table_of_retired_slots_refuses`), and the README's `BUGS` section
   is where a stranger looks.
3. Milestone 1033 says the README states how the crate differs; no README exists yet. The move
   lane writes it from the release record's recommendation.

## Open questions

1. Whether the README states expected demand as low. The crates.io API, read for this package,
   shows `slotmap` at 36.2 million downloads in 90 days and the two fixed-capacity crates its search
   finds, `slots` and `coca`, at 18 and 19. Recommendation: say it plainly in one sentence of the
   README, as the release record suggests. The maintainer can decide it.

## Result of `script/crate-standalone`

Eight failures: the three provisional names, and seven of release shape (the manifest inherits
`license` and `lints` and has no `rust-version`; the crate has no README and no license files; one
line cites this tree). The build on the host and the three bare-metal targets passes on stable.
That run skipped Kani (`--no-kani`); an earlier full run, before `EntryName<T>`, proved all five
harnesses and saw all five patches fail.

## Recommendation

Blocked by the release shape (the README, the license files and the manifest keys) and the three provisional names. After those, ready for review.
