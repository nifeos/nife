# The ledger

An appendix to [the milestone](../1079-byte-sink-contract-outside-review.md).

| item | state |
|---|---|
| 1 | the crate-root documentation is done: the wire table, the two shape decisions with their reasons, the `Gone` versus `NoSink` split with the standard library's mapping and what the contract does not carry, with no path into this tree; the manifest comment is rewritten, and the `line_editor` hit is gone. The scaffold, licenses and `Name:` stripping are the move's |
| 2 | done: `VERSION` states that the contract has no wire version operation and that a sink serving two versions accepts both framings; decision A stays for the architect |
| 3, 8, 9 | left: decisions B, C and D (the doc already states decision C's recommended reading, which the architect may reverse) |
| 4 | done: the `abi` dev-dependency and its test are gone; `system_tests/src/user/sink_tests.rs` pins `GONE` to `abi::Error::Gone` in a `const` assertion |
| 5 | done: five harnesses (`any_prefix_survives_pack_then_unpack`, `unpack_never_answers_more_than_a_message_holds`, `severity_reads_back_clamped_and_leaves_the_rest`, `classify_separates_the_three_outcomes`, `end_of_stream_ignores_its_other_words`), each with a falsification patch, and a row in `script/verify` |
| 6 | done: `unpack` refuses a bytes message of another body kind (`a_bytes_message_of_another_kind_is_refused`); `system_log`'s own check stays |
| 7 | done: `neighbors`, `authorize` and `neighbor` |
| 10 | done: the documentation says the count is 0 to `INLINE_MAXIMUM` and a zero count is legal |
| 11 | done: `unpack` compares the count as `u64` before it casts |
| 12 | left: the opcode constants are `u64`, so typed parameters mean retyping public constants |
| 13 | left: a public type is the architect's |
| 14 | left: the move's `.github/mutants-exempts` lines |
| 15 | done: both docs say a consumer reads the bits only on a bytes message |
