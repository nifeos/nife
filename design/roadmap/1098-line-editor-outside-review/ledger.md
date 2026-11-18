# Ledger

An appendix to [the milestone](../1098-line-editor-outside-review.md). A DONE row names the test or harness that holds the item, and a LEFT row
names the reason it waits.

| item | state | where |
|---|---|---|
| 1 stand-alone text and manifest | LEFT | the move commit's work (manifest rows, licenses, citations); `crates/line_editor/README.md` already speaks in the stranger's terms |
| 2 cut `proto` and `component` | LEFT | decision 1 (where the cut falls) |
| 3 public names | LEFT | decisions 2 and 3 (`LineEditor`, `HISTORY_LINES`, the three limits) |
| 4 gate spellings, `proto` and `component` | LEFT | they belong to the contract crate after the cut; the engine's private fields and parameters land with item 3's renames |
| 5 bindings, limits and `BUGS` | DONE | `crates/line_editor/README.md` and the crate documentation carry the bindings table, the limits and the `BUGS` list |
| 5 Meta prefix | DONE | `a_doubled_escape_is_one_prefix` |
| 5 mouse report | DONE | `a_mouse_report_is_dropped_with_its_payload` |
| 5 non-ASCII input | DONE | stated in `BUGS`; `a_byte_above_ascii_is_dropped_without_echo` pins the behavior |
| 6 `forbid(unsafe_code)` | DONE | `#![forbid(unsafe_code)]` in `lib.rs` |
| 6 fuzz targets | DONE | `fuzz/fuzz_targets/line_editor_feed.rs` and `line_editor_handoff.rs` |
| 6 Kani harnesses | DONE | `a_control_sequence_leaves_the_cursor_inside_the_line` and `a_range_delete_removes_exactly_its_range`, each with a falsification patch |
| 7 handoff layout | DONE | the `handoff` module documents the layout, the 2688-byte bound and the bump rule; `the_blob_layout_is_pinned_byte_for_byte` and `a_full_blob_is_the_documented_size` hold them |
| 8 one `QUIESCED` | DONE | a `const` assertion in the `line_editor` program holds the two values equal |
| 9 stack copies | DONE | `yank` reads the kill buffer in place (`a_yank_into_a_nearly_full_line_inserts_what_fits`); `restore` reads each history entry in place and `RawQueue::save` writes the ring in two runs (`a_handed_off_discipline_carries_on_where_the_old_one_stopped`, `a_wrapped_raw_queue_round_trips`) |
| 10 `csi_move` bound | DONE | a `const` assertion ties the buffer to `LINE_MAXIMUM`, a `debug_assert` holds the distance; `the_largest_cursor_move_is_a_whole_sequence` |
| 11 three repaints | DONE | `start_line` and `clear_and_repaint` call `repaint`; the existing repaint tests hold the bytes |
| 12 history depth | DONE | stated in `BUGS`; the const generic waits for a second user |
| 13 test terminal row | DONE | `Screen::interpret` pads the row when the cursor is past its end |
| 14 `Writer::field` width | DONE | `try_from`, setting `full` on failure; the writer refuses rather than wraps |
