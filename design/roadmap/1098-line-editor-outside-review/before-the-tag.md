# P1: before the tag

An appendix to [the milestone](../1098-line-editor-outside-review.md).

## P1: before the first tag

3. Public names the naming rules refuse, in the engine that leaves.
   Finding: `LineDisc` abbreviates "line discipline", the term the crate name ruling refused as
   an overclaim, and `Disc` reads as a disk to a stranger. `HIST` is not a word and is the one
   public constant with no `Name:` block, so the worklist cannot see it. `pop8` carries a digit
   where the sibling `OPERATION_READRAW` carries its eight in prose. `Event::Eof` keeps the
   standard library's spelling (`UnexpectedEof`) and stays here on that ground. The unit rule (above) reopens the three limits.
   Evidence: `src/line_disc.rs` lines 16, 21, 23, 27 and 142; `src/raw_queue.rs` lines 15 and 65.
   Change: `LineDisc` becomes `LineEditor` (a crate that holds a component's logic takes the component's name; `rustyline` and `noline` both call theirs `Editor`). `HIST` becomes `HISTORY_LINES` (a count of things names the things). `LINE_MAXIMUM`, `PROMPT_MAXIMUM` and `RAW_QUEUE_MAXIMUM` become `MAXIMUM_LINE_BYTES`, `MAXIMUM_PROMPT_BYTES` and `MAXIMUM_RAW_QUEUE_BYTES`. `pop8` becomes `pop_up_to_eight` or takes a `[u8; 8]` return that
   says it. No `doc(alias)` for the old spellings before the first release, per the rule in force.
   Door: two-way before the first tag; the milestone's decision 2 decides `LineEditor` and
   `HISTORY_LINES`, provisionally, and the limits are calef's. Effort: S.

4. Names the template's day-one gates refuse, in `proto` and `component`, and in parameters.
   Finding: `spelled_out.py` fails at a ceiling of 0 on `HANDOFF_VA`, `ELF_VA` (`va` abbreviates
   `virtual_address`) and the parameter `dir` of `csi_move`. The address rule
   (`_VIRTUAL_ADDRESS` in every location) also reaches `OWN_IMAGE_ADDRESS` and
   `DISPLAY_IMAGE_ADDRESS`. `proto` itself is on the spelled-out list (`protocol`), and the
   template's word list does not carry it, so the two disagree (unverified why the lint that
   holds them equal does not fire). `OPERATION_INTRCOUNT` abbreviates where its twin
   `OPERATION_SUSPEND_COUNT` does not; `CTL_RESUME`, `CTL_QUIT` and `CTL_HANDOFF` abbreviate
   `control`; `req` abbreviates `request`; `OPERATION_READLINE`, `OPERATION_READRAW` and
   `OPERATION_RAWMODE` join words that `OPERATION_READ_LINE_BYTES` separates. The private fields
   `buf`, `cur`, `hist`, `esc` and `params`, and the locals `elen`, `slen`, `scur`, fail
   `abbreviations.py`.
   Evidence: `src/component.rs` lines 7, 143, 148 and 160; `src/line_disc.rs` lines 88, 129 to
   145, 480, 605, 615 and 652; `src/proto.rs` lines 23, 171 to 184 and 218.
   Change: the `proto` and `component` renames belong to the new contract crate's own worklist after item 2. Listing them here lets the cut lane carry them. The engine's parameter and field spellings (`direction`, `buffer`, `cursor`, `history`, `escape`, `parameters`) land in the same pull request as item 3. Add `proto` to the template's word list or record why not.
   Door: two-way (values do not change). Effort: S.

5. A stranger cannot read the bindings or the limits without nife's notes, and two limits
   lack a record.
   Finding: the crate documentation sends the reader to `notes/line-discipline.md` and
   `notes/terminal-contract.md` for the key bindings, and there is no README. The `BUGS` section
   records the one-row redraw and nothing else. Verified by probe: the editor drops a byte at or above 0x80 without echo, so UTF-8 input vanishes silently. `ESC ESC [ A` (a Meta-prefixed arrow, or a double Escape) drops the first escape and inserts `[A` into the line. An X10 mouse report
   `ESC [ M b x y` drops the introducer and inserts its three payload bytes, where the comment
   says mouse noise is harmless. A prompt past `PROMPT_MAXIMUM` truncates silently, as the doc
   says.
   Evidence: `src/lib.rs` lines 1 to 20; `src/line_disc.rs` lines 351, 369 and 384 to 385.
   Change: a README and a crate-documentation section. They carry the bindings table from `notes/line-discipline.md` ("What the engine does and leaves out"), the limits with their numbers and a `Sink` example with a `Vec`. They also carry a `BUGS` list that states ASCII-only input, the
   one-row redraw, the eight-line history, the Meta prefix and the mouse-report payload. Either
   swallow `ESC ESC` as one escape and the X10 payload with its introducer, or state the gap.
   Door: two-way. Effort: S.

6. Nothing shows the parser is total: no fuzz target, no proof, no `forbid(unsafe_code)`.
   Finding: `feed` is a byte-at-a-time state machine over input the editor does not trust, with
   five fixed buffers and an index arithmetic the tests exercise at a few dozen points. The tree's
   fuzz workspace has 18 targets and none for this crate; the crate has no Kani harness; the
   crate's roots carry no `#![forbid(unsafe_code)]` although no `unsafe` exists, which is the one
   line the template's `unsafe_scan.py` reads as a promise.
   Evidence: `fuzz/fuzz_targets/` (no `line_editor_*`); `src/lib.rs` line 20;
   `src/line_disc.rs` lines 266 to 393 and 521 to 539.
   Change: `#![forbid(unsafe_code)]` in `lib.rs`. Add a cargo-fuzz target that drives `feed`,
   `insert_text`, `start_line`, `resume_line`, `repaint` and `abandon` from arbitrary bytes through
   a counting `Sink` and asserts `cur <= len <= MAXIMUM_LINE_BYTES` after every byte. Add a second target over `handoff::Reader::new` and `restore` from arbitrary bytes. Add one Kani harness for `continue_escape` over any parameter digits and separators, and one for `delete_range` from any `(a, b, cur, len)` inside the bounds. Whether the repository opts into the template's public
   crash inputs is the maintainer's call after the first campaign, not the lane's.
   Door: two-way. Effort: M.

7. The handoff blob is an identity with no declared version policy.
   Finding: `MAGIC` ("LEDT") and `LAYOUT` (1) are a single-version identity whose reader refuses any other layout, which is the rule milestone 979 (interfaces a peer can check) gives a shared page. by its documentation `restore` leaves a half-written editor on refusal, so the caller discards it. Nothing states that a field-order change bumps `LAYOUT`, that the blob is bytes
   little-endian throughout, or what the blob's upper size bound is (the test asserts under 2 KiB
   for a full ring; `HANDOFF_PAGES` is 1). A reader outside the tree has no way to know that
   `LAYOUT` is the version to compare.
   Evidence: `src/handoff.rs` lines 5 to 8 and 121 to 141; `src/handoff/tests.rs` lines 26 to 33.
   Change: a documentation section on `handoff` that states the layout (field order, each field `u32` length then bytes, the two words, the ring order), the bound, and the rule that any change to it bumps `LAYOUT`. Add a test that pins the byte layout of a small blob by hand, as the `package_archive` triage did for its header. Then a reordering fails a test rather than a swap.
   Door: `LAYOUT`'s meaning is one-way once a replacement built elsewhere reads it; the
   documentation and the test are two-way. The review proposes no new value, so nothing goes to the
   architect. Effort: S.

8. `proto::QUIESCED` and `hot_swap_contract::QUIESCED` are two definitions of one value.
   Finding: both are `0x5155_4954`, each with its own comment, which is the shape rule 7 of
   `AGENTS.md` (anything two binaries must agree on is one definition) exists to refuse. A drift
   would make a quiesce reply unreadable to one side.
   Evidence: `src/proto.rs` lines 166 to 167; `crates/hot_swap_contract/src/lib.rs` line 49.
   Change: re-export, or a `const` assertion that the two are equal until item 2 moves `proto`.
   Door: two-way. Effort: S.
