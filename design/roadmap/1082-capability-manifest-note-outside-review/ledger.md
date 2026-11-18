# The ledger

An appendix to [the milestone](../1082-capability-manifest-note-outside-review.md).

| item | state |
|---|---|
| 1 | done: the manifest stands alone, with literal `license` and lint tables, `rust-version` 1.88, `description`, `repository`, `readme`, `keywords`, `categories`, a `version` on every dependency, a `publish` comment a stranger can act on, and the two license files |
| 2 | done but for the stranger run: the README has the scaffold's sections; the run is recorded in `docs/stranger-test.md`, which the template's scaffold supplies, so it belongs to the move pull request |
| 3 | done: `write_zones` is one body for `encode_zoned` and `encode_serviced`, so both check each zone with `is_canonical_zone`; `encode` asserts `minimum <= maximum`; the harness `an_encoded_manifest_decodes_to_itself` proves the encoding direction, with its falsification patch |
| 4 | done: the source states each fact with no milestone, section, ruling, person or tree path; the doctests build their own manifests and verb table; test zones are example domains |
| 5 | done: `BARE_VERSION`, `Error::WrongLength`, the exact offsets, the cut version 3 descriptor and the `verification.rs` header say what the code does |
| 6 | done: the private offsets, decoders and bindings spell their fields out (`PROGRAM_ARGUMENT`, `MEMORY_MINIMUM_PAGES`, `OPTION_COUNT`, `STARTUP` and the rest) |
| 7 | done: the doctests no longer name a program of the catalog, and `every_compiled_in_manifest_round_trips` runs in `crates/program_table`, where the catalog lives; the other tests encode one catalog row each as a fixture and stay |
| 8 | left: the fuzz workspace moves with the crate, in the move pull request |
| 9 | decided: the weekly fuzz opt-in stays off, and the crate is fuzzed on the project's own host; the template's scan is not changed |
| 10 | done: `Display`, `core::error::Error` and `Hash` for `Error` |
| 11 | done: `Zones::len`, `ZonesIter` and `VerbsIter`, with `IntoIterator` for both views |
| 12 | done: `zero` is a loop that leaves `memchr` out of the proofs, and three harnesses were added, each with a falsification patch. `decode_subtree_grants_never_panics` passes in 4 seconds. `decode_never_panics_through_a_serviced_descriptor` walks six zone layouts fixed in the harness, with every other byte chosen by the solver and every length up to 62 bytes, in about 6 minutes and 3.8 GB; with the count or the lengths chosen by the solver it ran out of memory under a 12 GiB cap. `a_decoded_verb_table_finds_each_verb` covers tables of one or two verbs of one or two bytes and builds its view of the records in the harness, in about a minute and 1.3 GB. `grants_for` now halves a number of times fixed by the table's length, as `slice::binary_search` does; with the old search the proof ran out of memory |
| 13 | done: `write_note` writes every note's header and copies the owner whatever its length |
| 14 | done: every dependency has a `version`, and the README says the two crates share the note header |
| 15 | done: `std` is named once, at the crate root, for tests |
| decisions | the owner literal ruled 2026-11-14 (UTC); `asks_service` and `BARE_VERSION` decided by the maintainer, provisional; the `_BYTES` family applied as the units rule decides it, provisional names |
