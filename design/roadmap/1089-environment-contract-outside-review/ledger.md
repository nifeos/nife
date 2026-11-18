# Ledger

An appendix to [the milestone](../1089-environment-contract-outside-review.md). Each item reads DONE with the test or harness that holds it, or LEFT with the reason.

| item | state | how |
|---|---|---|
| 1 reader panics on a length byte | DONE | `a_length_past_its_field_reads_that_field_as_absent` (landed in #3529) |
| 2 reader-side domain check | DONE | `a_recognized_page_carrying_a_non_member_reads_that_key_as_absent` |
| 3 family and rule names | DONE | `the_frozen_identity_and_widths_are_pinned` reads `PAGE_CONTENT_BYTES`; `ConfigurationPage` and `virtual_address` are in the crate, the callers and the baselines; each name is provisional until the name review |
| 4 abbreviated accessors | DONE | `a_fully_declared_page_round_trips` and `the_builder_refuses_a_value_outside_its_domain` use `time_zone`, `locale`, `terminal_type`, `is_known_*` and `Unknown*`, each accessor with its POSIX `doc(alias)`; provisional until the name review |
| 5 proof | PARTIAL | `field_bytes_never_panics_for_any_length_byte`, proved in under a second, with a falsification patch. The round-trip harness (one ASCII field through `write_field` and `field_bytes`) and the domain-member harness (a free length byte over a page holding `UTC`) each ran past 25 minutes and 8 GB under Kani and were removed; each needs its text scan bounded or its shape split before it returns |
| 6 stands alone | LEFT | the manifest rows, README, licenses and rewritten citations belong to the move lane (`script/crate-move`), which waits on whether the crate leaves |
| 7 identity | LEFT | the decision below; the pinning test and `const` assertion follow the ruling |
| 8 compile-time check | DONE | `const` assertions in `lib.rs`, `every_domain_member_fits_its_field`; `write_field` asserts, private names spelled out, the `vendored_name_gates.py` exemption removed |
| 9 `Display` and `Error` | DONE | `a_refusal_displays_and_is_an_error` |
| 10 five documentation sentences | DONE | the zone length, the test name, `notes/env-config.md`, the stale BUGS row and the `UTF-8` note |
| 11 a safe reader | LEFT | a new public name and constructor: an architect's call |
| 12 growth path | DONE | the crate's `BUGS` section; the README row waits on item 6 |
