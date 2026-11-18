# Ledger

An appendix to [the milestone](../1100-memory-region-heap-outside-review.md). A DONE row names the test or harness that holds the
item, and a LEFT row names the reason it waits.

| item | state | where |
|---|---|---|
| 1 names and the missing `Name:` blocks | DONE | Decision A applied (#3597): the seven names read ratified and every public item carries a block; the names the later items added read provisional until a name review |
| 2 `talc` and the path dependencies | LEFT | Decision B is the rule; the move lane writes the pins and the README sentence |
| 3 manifest, README, license files | LEFT | the move lane writes them |
| 4 `va` binding and `commit_all` | DONE | `virtual_address` and `Result<(), Short>` (#3597); the `coverage` half waits for the move's first run |
| 5 a second `initialize` | DONE | `a_second_initialize_changes_nothing` and `a_refused_initialize_leaves_the_heap_without_a_wiring` |
| 6 `grow` of zero bytes | DONE | `a_grow_of_nothing_at_the_maximum_is_a_refusal`; the harness `a_grow_at_the_maximum_is_a_refusal` |
| 7 kernel refusals read as budget spent | DONE | `a_refused_page_stops_growth_for_good` and `a_spent_budget_keeps_the_pages_it_paid_for`; `last_refusal` reads the code |
| 8 page-source seam, host tests, proofs | LEFT | the seam, the host tests and three harnesses with falsification patches are done (#3609); the std overlay and `second_mount` keep their copies until a host-buildable path exists |
| 9 documentation, minimum cap | LEFT | the crate documentation and `the_first_heap_fits_in_one_page` are done; the README is the move lane's |
| 10 `PAGE_BYTES` | DONE | `PAGE_BYTES` in `src/lib.rs` (#3609) |
| 11 panic inside the allocator | DONE | read: the default handler `user_mode_runtime::panic_handler!` calls `trap` and allocates nothing, so a debug-build allocator panic faults legibly and does not spin; the type's documentation says so |
| 12 growth is monotonic | LEFT | the crate documentation says the heap never shrinks; the README sentence is the move lane's |
| 13 accessors for the cap and base | DONE | `maximum_bytes` and `base_virtual_address`; `the_readers_report_the_maximum_and_the_base` |
| 14 the `Sync` argument | DONE | the concurrency paragraph on `MemoryRegionHeap` (#3609) |
