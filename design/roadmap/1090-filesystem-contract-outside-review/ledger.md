# Ledger

An appendix to [the milestone](../1090-filesystem-contract-outside-review.md). Each item reads DONE with the test or harness that holds it, or LEFT with the reason.

| item | state | how |
|---|---|---|
| 1 stands alone | LEFT | the reword of every hit, the `Name:` strip and the manifest rows are the move commit's work (`script/crate-move`), which waits on whether the crate leaves |
| 2 clippy proper-noun list | LEFT | a crate-root `clippy.toml` would replace the workspace one for this crate; it belongs to the move commit |
| 3 falsification patch paths | LEFT | the paths change with the move; the six new patches follow the in-tree form too |
| 4 `rm::status` coverage | DONE | `a_status_is_the_errno_and_success_is_zero` in `src/rm.rs` |
| 5 names the gates refuse | LEFT | Decision B |
| 6 `reply_errno` total | LEFT | Decision C chooses the bound and the answer for a word outside it |
| 7 packers mask | LEFT | the proxies forward client-supplied lengths and handles through `fs::req` and `rename_dst`, so an assertion would let a client crash a caretaker; each call site needs a refusal first, which is a design change across three components |
| 8 proofs | PARTIAL | six harnesses in `src/proofs.rs`, each with a falsification patch: `a_file_request_round_trips`, `an_attribute_reply_is_never_an_error`, `a_block_request_round_trips`, `a_packed_name_comes_back_whole`, `the_verb_table_is_total`, `a_resolved_request_never_widens`; the four walkers' termination is not proven |
| 9 equivalent mutants | LEFT | `.github/mutants-exempts` exists only in the new repository |
| 10 README | LEFT | the scaffold's shape belongs to the move commit |
| 11 bare acronyms | PARTIAL | expanded in the crate's own text; the universally known ones go in the new repository's `prose_acronyms.toml` |
| 12 `rm` and `CLIENT_WINDOWS` | LEFT | where `rm` lands, and whether `CLIENT_WINDOWS` is a contract fact, are placement calls |
| 13 dates and spelling | DONE | `src/verb.rs`, `src/verb/tests.rs`, `src/fs.rs` state the present rule in American spelling |
| 14 `req_blocks` decoder | LEFT | clamp or `Option` changes what the contract says a decoder returns; an architect's call with Decision C |
| 15 empty name in `dirent::encode` | LEFT | refusing or documenting is a wire statement |
| 16 errno module | LEFT | public names (Decision B) |
| 17 length-or-offset doc | DONE | the doc on `fs::MAXIMUM_LENGTH_OR_OFFSET`; the name waits for Decision B |
| 18 distinctness tests | DONE | `every_opcode_is_distinct_and_fits_its_field` derives from `verb::TABLE` plus the bind verbs; the stale list in `src/fs/tests.rs` is replaced by `the_root_handle_stays_zero` |
| 19 `usize` width | DONE | `const` assertion that `usize::BITS >= 40` in `src/fs.rs` |
| 20 `in_opcode_order` warning | DONE | `allow(dead_code)` outside tests, with the reason |
| 21 fuzz targets | LEFT | a `fuzz/` crate per walker is a lane of its own |
| 22 `xattr::reply` masks | LEFT | the same call-site argument as item 7 |
