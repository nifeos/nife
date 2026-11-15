# The ledger

An appendix to [the milestone](../1081-capability-outside-review.md).

| item | state |
|---|---|
| 1 | left: the move's (the scaffold, licenses and manifest) |
| 2 | done in `src/lib.rs` and the fourteen patch headers: no section, milestone, note path, kernel path or person outside a `Name:` block remains in the crate source; the broken harness link and the wrong kernel path are gone; the headers lost their re-cut history. The `Name:` blocks stay until the move strips them |
| 3 | left: `delegate` and its error variant are new public names, and a harness over the kernel's own delegation is the architect's call on shape |
| 4 | done: `get`, `put` and `delete` convert the slot with `usize::try_from` (`a_slot_past_the_width_of_usize_names_nothing`) |
| 5 | left: renaming `len` is a public rename, with four kernel call sites |
| 6 | done before this lane: `regenerate_patch` keeps the trailing blank context line, and `add_repository` handles an inherited license |
| 7 | done: `insert_at_or_above` and a second `put` join `the_count_is_the_slots` and `the_free_mask_is_the_empty_slots`, and `the_peak_never_falls_and_a_floor_insert_stays_above_its_floor` is new with its falsification patch |
| 8 | done: `ALL` is the union of the five rights |
| 9 | done: the parameters are `supervising_rendezvous`, `invoked_rendezvous` and `source` |
| 10 | `Display`, `core::error::Error` and `Hash` done (`an_error_prints_and_is_an_error`); the two new variants are a published name, left |
| 11 | done: the peak static and its readers sit behind `target_has_atomic = "32"`; a cargo feature is left |
| 12 | `delete_matching` takes `FnMut` (`a_sweep_may_count_what_it_deletes`); `iter` is a new public name, left |
| 13 | left: the move's `.github/mutants-exempts` lines |
| 14 | declined with a reason: no table has more than 64 slots, so no test can exceed the pinned `(64, 64)`; the exact assertion cannot be broken by a wider table |
| 15 | done: the two notes name `Capability`, the scheduler directory and `CannotWiden` |
| 16 | left: a layout change to measure first |
