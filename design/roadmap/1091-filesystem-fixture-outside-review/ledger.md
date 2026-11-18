# Ledger

An appendix to [the milestone](../1091-filesystem-fixture-outside-review.md). Each item reads DONE with the test or harness that holds it, or LEFT with the reason.

| item | state | how |
|---|---|---|
| 1 cargo-deny refuses a git dependency | LEFT | the fix is in the template repository, not in this tree |
| 2 order and pairing | PARTIAL | DONE in the crate: the root `BUGS` and the README say the contract leaves first. LEFT: 1017's row belongs to the recording lane |
| 3 acronyms and spelling | PARTIAL | DONE: every doc comment expands its acronyms and uses American spelling (`defense`, `behavior`). LEFT: the `EL0` inside the `MOTD` data literal, which the system tests compare |
| 4 names the gates refuse | LEFT | Decision 2 |
| 5 ratification package | LEFT | the package, its index row and the ratification wait on the architect's rulings in Decision 2 |
| 6 the crate describes this tree | LEFT | the data literals (`nife walk entry`, the partition names, `MADE_BODY`) change the image builder and `xtask/src/disk_check.rs` with them; a lane of its own |
| 7 history residue | DONE | the lane, milestone, date and process words are gone from `src/lib.rs` |
| 8 `durability` | DONE | the module is deleted; `notes/filesystem-fixture.md` keeps the classification |
| 9 shared-frame documentation | DONE | `src/lib.rs` and `notes/filesystem-fixture.md` state the present verdicts |
| 10 equivalent mutants | DONE | the first bits are written as `1` and `attrs::EXPECTED` as `0xFF`; `the_witness_bits_are_distinct_single_bits` holds it equal to the union of the eight |
| 11 manifest and workflows | LEFT | the move commit |
| 12 README claims | DONE | the README names the programs that compile the crate in; the heading reads "What it holds" |
| 13 `rg_match_path` bound | DONE | `a_match_path_past_the_last_match_is_refused`; the buffer size is `walk::MATCH_PATH_BYTES` (provisional) |
| 14 `wide_name` wrap | DONE | `a_wide_name_past_the_last_file_is_refused`, and the edge at 127 in `the_wide_directory_names_are_n_and_three_digits` |
| 15 throughput widths | LEFT | one width changes the casts in the kernel bench and the client |
| 16 `ALL` arrays | LEFT | new public names |
| 17 doc comments | LEFT | a documentation rewrite of 11,000 words, with the reasons moving to the new repository's `docs/` |
| 18 a harness for outsiders | LEFT | out of this block's scope |
