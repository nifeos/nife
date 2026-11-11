# P2: can follow later

An appendix to [the milestone](../1071-entropy-contract-outside-review.md).

9. A reply builder for the service side.
   Finding: the crate's claim is that the service, its clients and their tests share one
   definition. The client half does, but the three backends compose `(count, word)` by hand. The
   virtio pool packs bytes into the word by shifts, the instruction backend returns `(requested,
   u64::from_le_bytes(draw))`, and the JH7110 pool packs its own. Item 6 (c) becomes enforceable only
   when one function makes the word.
   Evidence: `components/src/entropy.rs:269-289, 519-525`; `components/src/jh7110_entropy.rs:268-273`. <!-- line-citation: exception. Reason: the lines are in the tree at the commit the review read, which does not change -->
   Change: `reply(delivered: &[u8]) -> (u64, u64)` (provisional) clamps to `MAXIMUM_BYTES`, packs
   little-endian and zeros the rest, with a harness: `copy_delivered_bytes` of a `reply` yields the
   input, and the undelivered bytes read zero. The backends adopt it in their own pull requests.
   Door: two-way (an addition). Effort: S.

10. `#[must_use]` on the pure functions.
    Finding: only `bring_up_failure` carries it. `request`, `operation`, `requested_bytes` and
    `delivered_bytes` do not (clippy's `must_use_candidate`, run by hand, names lines 69, 77, 86 and
    218), and neither does `copy_delivered_bytes`, whose ignored count is the bug the function's
    documentation warns about.
    Evidence: `src/lib.rs:69, 77, 86, 218, 266`. <!-- line-citation: exception. Reason: the lines are in the tree at the commit the review read, which does not change -->
    Change: add the attribute to the five. Effort: XS. Door: two-way.

11. `request` and `bring_up_failure` accept values that do not fit their fields.
    Finding: `request(0x1ff, 1)` yields an opcode that reads back as `0xff`; `bring_up_failure(0x1_0000)`
    puts the step outside the low byte (both verified). The harness assumes the opcode fits, and
    the documentation says a step is `0x01..=0x11`.
    Evidence: `src/lib.rs:69-71, 135-137`. <!-- line-citation: exception. Reason: the lines are in the tree at the commit the review read, which does not change -->
    Change: `debug_assert!(operation <= 0xff)` and `debug_assert!(step <= 0xff)` (both legal in
    `const fn`), or masks; the documentation says which. Effort: XS. Door: two-way.

12. The readiness proof's bound is below every draw a backend makes.
    Finding: the harness proves draws of up to four bytes; the backends draw 8, 32 and 256, which the
    host tests cover for the all-zero case only.
    Evidence: `src/lib.rs:579-583` and `420-437`. <!-- line-citation: exception. Reason: the lines are in the tree at the commit the review read, which does not change -->
    Change: raise the bound to `MAXIMUM_BYTES` (unwind 10); item 8 needs that anyway. Effort: XS.

13. The `Falsification:` lines sit at the foot of each harness's comment.
    Finding: `falsify.py --check` reports all four as "replayable with no patch path" because the
    template reads the line at the head of the comment. The runbook says the dry run moves the lines.
    Evidence: `src/lib.rs:494, 512, 538, 577`; `notes/crate-move.md` (the dry run). <!-- line-citation: exception. Reason: the lines are in the tree at the commit the review read, which does not change -->
    Change: none, if the dry run does what the runbook says; the move lane confirms `proofs /
    falsify` is green on the first push. Effort: none. Door: two-way.

14. Two lines of the crate documentation name the architect.
    Finding: `standalone.py` reports two "architect's name" lines, the crate-level `Name:` block.
    `strip_name_blocks` removes a `//!` `Name:` line and its continuation lines, so the move deletes
    both (verified by reading `helpers/crate_release.py:180-206`). <!-- line-citation: exception. Reason: the lines are in the tree at the commit the review read, which does not change -->
    Evidence: `src/lib.rs:17-18`. <!-- line-citation: exception. Reason: the lines are in the tree at the commit the review read, which does not change -->
    Change: none before the move; the move lane confirms the staged `src/lib.rs` carries neither
    line. Effort: none.

15. The README does not name `INTERFACE_ID`.
    Finding: the contract list names `VERSION` (1) but not `INTERFACE_ID` (12), though every program
    declares the pair and the crate documentation says so.
    Evidence: `README.md` `## The contract`; `src/lib.rs:51-57`. <!-- line-citation: exception. Reason: the lines are in the tree at the commit the review read, which does not change -->
    Change: one bullet. Effort: XS.

16. The library's prose is over the tree's own caps.
    Finding: `prose_caps.py` over `src/lib.rs`: 3,278 words against a cap of 3,000, a median
    sentence of 21 against 20, a longest of 164 against 40, and 11.6 bold spans per 1,000 words
    against 4. The template gates the README and CHANGELOG only, so this is red nowhere; it is what
    docs.rs shows a stranger.
    Evidence: `src/lib.rs`, the whole file.
    Change: a prose pass that shortens the long sentences and drops most bold. Effort: S.

17. Lints the crate cannot trip.
    Finding: `clippy::undocumented_unsafe_blocks`, `cast_ptr_alignment` and `ptr_as_ptr` under
    `#![forbid(unsafe_code)]` do nothing. The template requires `unsafe_op_in_unsafe_fn` and
    `missing_docs`, which stay.
    Evidence: `Cargo.toml:18-30`; `crate-workflows/helpers/standalone.py:62`. <!-- line-citation: exception. Reason: the lines are in the tree at the commit the review read, which does not change -->
    Change: delete the three. Effort: XS.
