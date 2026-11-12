# P2: later

An appendix to [the milestone](../1098-line-editor-outside-review.md).

## P2: later

9. Stack copies in a program documented to run on one page of stack.
   Finding: `yank` copies the whole 256-byte kill buffer to the stack to satisfy the borrow
   checker where disjoint field borrows would do; `LineDisc::restore` builds a 256-byte local for
   each of eight history entries in turn; `RawQueue::save` builds a 64-byte local. The doc on
   `new` explains the editor lives in a `static` because the stack is one 4 KiB page.
   Evidence: `src/line_disc.rs` lines 157 to 160 and 552; `src/handoff.rs` lines 135 and 147.
   Change: borrow `self.buf` and `self.kill` as two fields; let `Reader::field` write into the
   history entry in place.
   Door: two-way. Effort: S.

10. `csi_move` holds a bound nothing states.
    Finding: its 11-byte sequence buffer fits eight decimal digits; every caller passes a count at
    most `LINE_MAXIMUM`, which is three digits, and no assertion or comment ties the two.
    Evidence: `src/line_disc.rs` lines 652 to 673.
    Change: a `debug_assert!(n <= LINE_MAXIMUM)` or a buffer sized from the constant.
    Door: two-way. Effort: S.

11. Three repaints with one body.
    Finding: `start_line`, `repaint` and `clear_and_repaint` each emit prompt, line, and a cursor
    step back; only the preface differs.
    Evidence: `src/line_disc.rs` lines 226 to 241 and 435 to 440.
    Change: the two callers call `repaint`.
    Door: two-way. Effort: S.

12. History is eight lines, fixed, and the kill ring is one entry.
    Finding: linenoise defaults to 100 entries and `rustyline` to a configurable size; the ring
    here is `HIST` entries of `LINE_MAXIMUM` bytes each, 2 KiB, which is right for the one-page
    process and small for a desk session. Dedup looks only at the newest entry.
    Evidence: `src/line_disc.rs` lines 17 and 137.
    Change: state it in `BUGS` now; a const generic on the history depth later, if a second user
    asks.
    Door: two-way until a const generic lands, then one-way. Effort: S now.

13. The test terminal model panics on a cursor past the row.
    Finding: `Screen::interpret` indexes `row[col]` when `col` is at most `row.len()`; a `CSI n C`
    past the end followed by a printable byte would panic with an index error rather than a
    message naming the sequence. The engine never does it today.
    Evidence: `src/test_support.rs` lines 66 to 74.
    Change: pad the row or assert with a message.
    Door: two-way. Effort: S.

14. `Writer::field` truncates a length to `u32` by cast.
    Finding: `b.len() as u32` cannot overflow for any field this crate writes (at most 256
    bytes), and a `u32::try_from` that sets `full` would make the writer refuse rather than wrap
    if a larger caller ever appears.
    Evidence: `src/handoff.rs` line 45.
    Change: `try_from`, with `full = true` on failure.
    Door: two-way. Effort: S.
