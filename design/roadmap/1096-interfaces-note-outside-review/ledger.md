# Ledger

An appendix to [the milestone](../1096-interfaces-note-outside-review.md). Each item is DONE with
the test or harness that holds it, or LEFT with the reason.

| item | state | where |
|---|---|---|
| 1 manifest stands alone | LEFT | the move's manifest rows |
| 2 README and license files | LEFT | the scaffold belongs to the move |
| 3 documentation cites this tree | LEFT | the move commit rewrites the citations and strips the provenance blocks |
| 4 the owner string `nife` | LEFT | decision 1 |
| 5 passive voice | LEFT | the template's gate runs in the moved repository; the rewrite lands with item 3 |
| 6 acronyms expanded | DONE | ABI and NUL terminator are spelled at first use in `src/lib.rs` |
| 7 a stranger can place the note | DONE | the crate documentation states the link-script requirement, the endianness rule, the registry rule and the absent-entry rule; the README repeats it at the move |
| 8 doctests | DONE | doctests on `carry_interfaces!`, `encode_interfaces`, `decode_interfaces`, `Interfaces::version_of` and `Interfaces::check_served`, none `ignore` |
| 9 private lengths carry a unit | DONE | `NAME_FIELD_BYTES`, `HEADER_BYTES` |
| 9 public lengths carry a unit | LEFT | decision 2 |
| 10 `Declared` names a state | LEFT | decision 2 |
| 11 fuzz target | LEFT | the fuzz opt-in is a per-repository choice, off by default, made at the move |
| 12 second proof direction | DONE | harness `an_encoded_interface_list_decodes_back_to_itself` with its falsification patch |
| 13 test name | DONE | `a_list_out_of_order_is_not_encoded` |
| 14 `iter` yields by value | DONE | the doc comment on `Interfaces::iter` says why |
| 15 triaged timeouts | LEFT | carried into `.github/mutants-exempts` at the move |
