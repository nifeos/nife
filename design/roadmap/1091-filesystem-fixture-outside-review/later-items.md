# P2: later

An appendix to [the milestone](../1091-filesystem-fixture-outside-review.md).

## P2: worth doing while the lane is there

13. `rg_match_path` has no bound and a magic buffer size.
    Finding: for `i` at or past `RG_MATCHES` the function returns `narrow/n000` for every `i`, so a
    caller iterating one too far gets a duplicate rather than a failure. The signature takes
    `&mut [u8; 32]` where the longest path is 23 bytes and nothing names the bound. Callers iterate
    `0..RG_MATCHES` today.
    Evidence: lines 681 to 706.
    Change: add `debug_assert!(i < RG_MATCHES)` or return `Option<usize>`. Add a named constant for
    the buffer size.
    Door: two-way (a signature change before any release). Effort: XS.

14. `wide_name` wraps silently past 999.
    Finding: `wide_name(1000)` is `n000`; the test asserts `wide_name(987)`, which is outside
    `WIDE_COUNT`, so the test documents a range the fixture never uses.
    Evidence: lines 584 to 591; `src/tests.rs` line 333.
    Change: bound the argument to `WIDE_COUNT` in a `debug_assert!` and test the edge at 127.
    Door: two-way. Effort: XS.

15. The throughput module mixes widths.
    Finding: `UNIT` is `usize` (it is `TRANSFER_MAXIMUM`'s type) while `TOTAL`, `BLOCKS` and
    `WARMUP` are `u64`, so every caller writes `UNIT as u64` (`kernel/src/bench/filesystem.rs` line
    92, `fixtures/src/fs_test_client/throughput.rs` line 64).
    Evidence: lines 140 to 167.
    Change: one width for the module, with the cast made once at `UNIT`.
    Door: two-way. Effort: XS.

16. The distinct-bits tests are hand-copied lists.
    Finding: `the_escape_bits_are_distinct`, `the_directory_escape_bits_are_distinct`,
    `the_navigation_bits_are_distinct` and `the_witness_bits_are_distinct_single_bits` each hold a
    copy of the same constants; a bit added to a module and to one list but not another passes, and
    a comment at `src/tests.rs` line 178 says so.
    Evidence: `src/tests.rs` lines 28 to 305 and 376 to 406.
    Change: one `ALL` array per witness module, public so a consumer can assert membership, and
    one test over the arrays.
    Door: two-way (adds public items). Effort: S.

17. The doc comments are a note, not an API reference.
    Finding: `lib.rs` carries 11,606 words of prose for 192 constants and 4 functions: median
    sentence 21 words, longest 146, 18 bold spans per 1,000 words, 86 passive constructions. The
    template's prose caps apply to the README and `docs/`, so no gate is red, but the reasoning the
    comments hold (the record-read refutation, the payload fill, the durability classification) is
    what `notes/filesystem-fixture.md` already holds.
    Evidence: `prose_caps.py` and `passive_voice.py` over `src/lib.rs`.
    Change: the reasons move to the repository's `docs/`, and each item keeps one or two sentences
    that say what it is and what reads it.
    Door: two-way. Effort: M.

18. A third party gets vocabulary with nothing to run.
    Finding: 1017 records that the tests that drive the witnesses live in `system_tests`, so a
    consumer outside nife gets names and shapes and no harness. The prior-art suites above all ship
    the harness.
    Change: out of this block's scope; recorded so the next review starts from it.
    Door: two-way. Effort: not estimated.
