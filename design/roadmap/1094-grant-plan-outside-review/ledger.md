# Ledger

An appendix to [the milestone](../1094-grant-plan-outside-review.md). Each item reads DONE with the
test that holds it, or LEFT with the reason. Item 4 landed with pull request 3559, and that change
also edits `before-the-tag.md`, so the rows for items 5 to 14 live here and not beside the items.

| item | state | where |
|---|---|---|
| 1 split the catalog from the planner | DONE | `grant_contract` and `program_table` leave `grant_plan`, built by lane milestone/1094-grant-plan-split |
| 2 documentation builds | DONE | `pack_name` is plain code in `ASK_NETWORK`'s doc, and `cargo doc` runs clean; the repository's documentation gate flag belongs to the move |
| 3 manifest and repository root | LEFT | follows the boundary: the description and the dev-dependency change with the split |
| 4 tokenizer drops a token | DONE | pull request 3559 |
| 5 proofs | LEFT | one harness per claim is a lane of its own; the two `job_windows` patches still replay host tests |
| 6 limits carry their unit | LEFT | the third decision, public names |
| 7 names that need a decoder | LEFT | the second decision, public names |
| 8 British spellings | DONE | the five doc comments now use the American forms |
| 9 documentation for a stranger | PARTIAL | DONE: the `missing_docs` worklist is closed and the crate root carries no `allow`. LEFT: the citations, quotations and `Name:` blocks, which the move strips |
| 10 numbers in refusal sentences | DONE | `every_number_a_refusal_names_is_its_constants_value`; `ScopeTooDeep` stays, since its number belongs to the shell |
| 11 values by value | PARTIAL | DONE: `the_planning_types_have_the_sizes_the_stack_budget_assumes`. LEFT: taking `&Holdings`, `&Expansion` and `&Streams`, which changes every caller in `swish` and the components |
| 12 narrow length fields | DONE | three `const` assertions in `expand.rs` and `nav.rs` |
| 13 undeclared widening option | PARTIAL | DONE: `Manifest::is_well_formed` with `every_shipped_manifest_is_well_formed` and `a_widening_option_must_be_declared`. LEFT: the `programs!` assertion and the note decoder's call |
| 14 mutation run | LEFT | follows the split |
| 15 `NameSet` debug | DONE | the pinned debug test now reads the names as text |
| 16 `Result<_, ()>` operands | LEFT | `parsing.rs` is the file pull request 3559 changes |
| 17 unchecked window shift | DONE | `a_window_outside_the_pool_cannot_set_another_windows_bit` |
| 18 wire ids | DONE | `the_wire_ids_already_shipped_never_move` now lists all 28 |
| 19 runnable example | LEFT | an `examples/` file waits for the boundary, since it shows a manifest |
