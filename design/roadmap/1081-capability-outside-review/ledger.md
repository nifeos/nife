# The ledger

An appendix to [the milestone](../1081-capability-outside-review.md).

| item | state |
|---|---|
| 1 | left: the move's (the scaffold, licenses and manifest) |
| 2 | done in `src/lib.rs` and the fourteen patch headers: no section, milestone, note path, kernel path or person outside a `Name:` block remains in the crate source; the broken harness link and the wrong kernel path are gone; the headers lost their re-cut history. The `Name:` blocks stay until the move strips them |
| 3 | done: `Capability::delegate` (provisional) holds the rule once, with `Error::GrantRequired` (provisional); `CapabilityTable::derive` and the kernel's `Delegation::derive` both call it, so `delegation_needs_grant_and_never_widens` and its falsification patch prove the code the kernel runs. `get_with` is deleted (it had no caller); `put` stays as the builder's door and its doc says so. The two `REVOKE` arms in `kernel/src/syscall.rs` still test `GRANT` inline, left: they are a one-line test of a right, and a harness over the syscall layer is the architect's call on shape |
| 4 | done: `get`, `put` and `delete` convert the slot with `usize::try_from` (`a_slot_past_the_width_of_usize_names_nothing`) |
| 5 | done: `len` is `capacity` (provisional), with the kernel's call sites; no `len` is added, since `used` counts the occupied slots |
| 6 | done before this lane: `regenerate_patch` keeps the trailing blank context line, and `add_repository` handles an inherited license |
| 7 | done: `insert_at_or_above` and a second `put` join `the_count_is_the_slots` and `the_free_mask_is_the_empty_slots`, and `the_peak_never_falls_and_a_floor_insert_stays_above_its_floor` is new with its falsification patch |
| 8 | done: `ALL` is the union of the five rights |
| 9 | done: the parameters are `supervising_rendezvous`, `invoked_rendezvous` and `source` |
| 10 | done: `Display`, `core::error::Error` and `Hash`, and `SlotOccupied` (provisional) for `insert_at` on an occupied slot, apart from `NoFreeSlot` for a full table. `GrantRequired` replaces the `InsufficientRights` the review proposed: `get_with`, the only place that answered a missing right, is gone |
| 11 | done: the peak static and its readers sit behind `target_has_atomic = "32"`; a cargo feature is left |
| 12 | done: `delete_matching` takes `FnMut` (`a_sweep_may_count_what_it_deletes`), and `iter` (provisional) walks the occupied mask; the kernel's three walks over `0..len()` and the reply-capability scan use `delete_matching` or `iter` |
| 13 | left: the move's `.github/mutants-exempts` lines |
| 14 | declined with a reason: no table has more than 64 slots, so no test can exceed the pinned `(64, 64)`; the exact assertion cannot be broken by a wider table |
| 15 | done: the two notes name `Capability`, the scheduler directory and `CannotWiden` |
| 16 | left: optional, and it changes the input domain of every rights harness; to measure before anyone takes it |
