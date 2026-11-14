# Ledger

An appendix to [the milestone](../1099-machine-statistics-contract-outside-review.md). Each item
is DONE with the test or harness that holds it, or LEFT with the reason.

| item | state | where |
|---|---|---|
| 1 names and the ratification table | LEFT | the architect ratifies the names; the table and its flags are in the block |
| 2 manifest, license files, README | LEFT | the move commit writes them |
| 3 citations of this tree | DONE | the module and item documentation and the manifest comment say what the page is in the contract's own terms; the `Name:` blocks stay for the move to strip, and the `publish` comment stays with item 2 |
| 4 saturating totals | DONE | `a_page_of_maximal_counters_saturates_every_accessor`; the harness `every_accessor_is_total_over_an_admitted_page` |
| 5 `counter` lifetime and read-only contract | DONE | `counter<'a>` returns `AtomicU64::from_ptr`, a `const` assertion refuses a pointer narrower than the address, and the docs say a read-only reader loads with `Relaxed` only; the `u64` address parameter stays, because the callers hold an address |
| 6 line count owned by this crate | LEFT | Decision 1 |
| 7 three Kani harnesses | DONE | `the_reader_admits_exactly_the_pages_it_names`, `every_accessor_is_total_over_an_admitted_page` and `a_built_header_reads_back_as_an_empty_page`, each with a patch in `falsifications/` |
| 8 host tests over an aligned frame | DONE | `read_returns_what_the_writer_stored_through_counter` and `read_of_an_unprepared_frame_is_none`; Miri is not installed on the lane's machine, so the Miri run waits for the move's template |
| 9 header and line namespaces | LEFT | public names, for the ratification table |
| 10 `online_cpus` type and the word core | DONE | `usize` stays and the accessor's documentation says why; the prose says cpu where it means the scheduler's unit |
| 11 documentation defects | DONE | the unit column spelling is gone with the rewrite of BUGS, the reserved words and the growth rule are in the crate documentation, `TICK_HERTZ` says what it is for, `read` says why its loads are relaxed, and BUGS lists the contract's limits |
| 12 dry run against the standalone check | LEFT | a finding against `helpers/crate_move.py`, for a lane of its own |
| 13 end-to-end test note | LEFT | the sentence belongs in the README that item 2 writes |
| 14 unconditional `no_std` | DONE | `#![no_std]` and the host tests and doctests pass |
| 15 `Snapshot` size | DONE | the type's documentation states the 416 bytes |
