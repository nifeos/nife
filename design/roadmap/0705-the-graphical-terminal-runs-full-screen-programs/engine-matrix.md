# The reference engines, feature by feature

This appendix belongs to [milestone 705 (the graphical terminal runs full-screen programs)](../0705-the-graphical-terminal-runs-full-screen-programs.md).
It holds the feature matrix behind [the engine plan](engine-plan.md): five engines read against the
target list, as references for behavior and as prior art for each step. No code was copied, and
nothing downloaded was built or run.

## Sources

Read on 2026-10-21 (UTC).

| engine | version | where it was read |
|---|---|---|
| `alacritty_terminal` | 0.26.0 | the crates.io copy |
| `vt100` | 0.17.0 | the crates.io copy (`.cargo_vcs_info` 4c07e2109c26) |
| `vte` | 0.15.0, `ansi` feature | the crates.io copy |
| libvterm | 0.3.3 | the Launchpad tarball, sha256 `09156f43dd21...7226e0` |
| libghostty-vt | `ghostty-org/ghostty` at 47bd2033cd01, 2026-10-21 | `src/terminal/` |

How to read a cell. Y has it, P has part of it, N does not. The location follows the colon:
alacritty paths are under `src/` (`term` is `src/term/mod.rs`), libghostty-vt's under
`src/terminal/`. Each location names a function or a dispatch case; §277 (cite a heading or a symbol) keeps line numbers out. `vte` is a parser, not an engine: its Y means it
recognizes the sequence and calls a `Handler` method, and the state is the implementer's.

"ours" is `video_terminal` at `main` 810666a39. "step" is the [engine plan](engine-plan.md)'s
increment that adds it. "out" is out of scope, with the reason in the plan. "consume" means the
sequence is parsed and dropped on purpose, like a window title on a screen with no title bar.

## Size and tests

| engine | lines, excluding tests | how counted | conformance and fuzzing in its tree |
|---|---|---|---|
| `alacritty_terminal` | 8,496; 6,499 without the pty and event loop | `src/**/*.rs`, blanks and comments included | 45 recorded test captures, 6 of them from vttest; no fuzz target |
| `vt100` | 4,177 | `src/*.rs`, blanks and comments included | one unit test in the published crate; no fuzz target |
| `vte` | 701 parser, 1,362 `ansi.rs` | code lines only | 57 unit tests; no fuzz target |
| libvterm | 6,935 | `src/*.c src/*.h include/*.h` | 43 test scripts, 8 transcribed from vttest; no fuzz target |
| libghostty-vt | 91,182 | all of `src/terminal/`, far more than an engine | about 3,500 inline tests, esctest2 in CI (report only), a fuzz target |

The line counts are not comparable with each other: they count different things, and the last one
includes kitty graphics, search and a C API. `video_terminal` is 1,361 lines in `vt.rs` (883 of code)
with 1,437 lines of tests in `vt/tests.rs`.

Counts over the 51 features: `alacritty_terminal` 32 Y, 12 P, 7 N; `vt100` 15, 11, 25; `vte` 30,
10, 11; libvterm 31, 10, 10; libghostty-vt 47, 4, 0.

## What the matrix found that the census missed

- `ESC ( B` is three bytes, and ours ends the sequence after two, so the `B` is printed. Every
  ncurses program under `xterm` or `xterm-256color` sends it in `sgr0`. All four engines that parse
  designations consume it (F20); the census tables count it, and step 1 fixes it.
- A control code inside a CSI is executed by every engine and dropped by ours (F60), which fails
  vttest's "control characters inside ESC sequences" screen.
- Default replies differ by engine. libghostty-vt's engine answers DA1 `CSI ? 62 ; 22 c` and DA2
  `CSI > 1 ; 0 ; 0 c`; libvterm `CSI ? 1 ; 2 c` and `CSI > 0 ; 100 ; 0 c`. None claims to be xterm.
  That is the prior art for decision 2 in the plan.
- DECSLRM (F24) is in libvterm and libghostty-vt and in neither Rust engine. The census found it
  used only by programs told they were talking to xterm.
- `vt100` has no tab stops, no insert mode, no character sets and no replies; it is the weaker
  oracle, which the vttest results already showed.

## The matrix

| ID | feature | ours | step | alacritty_terminal | vt100 | vte | libvterm | libghostty-vt |
|---|---|---|---|---|---|---|---|---|
| F01 | DECSTBM scrolling region `CSI t;b r`, with LF/IND/RI/NEL scrolling inside it | N | 2 | Y: term set_scrolling_region | Y: perform.rs `'r'` | Y: ansi.rs csi_dispatch ('r') | Y: state.c (CSI r DECSTBM) | Y: Terminal.zig setTopAndBottomMargin |
| F02 | RI `ESC M` (reverse index, scroll down at top of region) | N | 2 | Y: term reverse_index | Y: perform.rs `b'M'` | Y: ansi.rs esc_dispatch (b'M') | Y: state.c (on_control) | Y: Terminal.zig reverseIndex |
| F03 | IND `ESC D`, NEL `ESC E` | N | 1 | Y: vte ansi.rs esc_dispatch | N: perform.rs esc_dispatch | Y: ansi.rs esc_dispatch (b'D' b'E') | Y: state.c (0x84 IND), (0x85 NEL) | Y: stream.zig ESC D ESC E |
| F04 | SU/SD `CSI n S`, `CSI n T` | N | 2 | Y: term scroll_up, scroll_down | Y: perform.rs | Y: ansi.rs ('S' 'T') | Y: state.c (SU 0x53), (SD 0x54) | Y: Terminal.zig scrollUp |
| F05 | IL/DL `CSI n L`, `CSI n M` (region-aware) | N | 2 | Y: term insert_blank_lines, delete_lines | Y: perform.rs | Y: ansi.rs ('L' 'M') | Y: state.c (IL), (DL) | Y: Terminal.zig insertLines |
| F06 | ICH/DCH/ECH `CSI n @`, `CSI n P`, `CSI n X` | N | 1, 3 | Y: term insert_blank, delete_chars, erase_chars | Y: perform.rs `'@'`, `'P'`, `'X'` | Y: ansi.rs ('@' 'P' 'X') | Y: state.c (ICH), (DCH), (ECH) | Y: Terminal.zig insertBlanks |
| F07 | CHA/HPA/VPA/HPR/VPR/CNL/CPL/HVP (`CSI G`, `CSI \``, `CSI d`, `CSI a`, `CSI e`, `CSI E`, `CSI F`, `CSI f`) | P | 1 | Y: vte ansi.rs csi_dispatch | P: perform.rs | Y: ansi.rs | Y: state.c CHA, HPA, VPA, HPR, VPR, CNL, CPL, HVP | Y: stream.zig E |
| F08 | DECSC/DECRC `ESC 7`/`ESC 8` and SCOSC/SCORC `CSI s`/`CSI u`: what is saved (position, attributes, origin mode, wrap-pending flag, charsets) | N | 4 | P: term save_cursor_position, restore_cursor_position | P: perform.rs | Y: ansi.rs (ESC 7 8) | P: state.c savecursor | Y: Terminal.zig saveCursor |
| F09 | DECOM origin mode `?6` | N | 2 | Y: term goto, set_private_mode | Y: screen.rs `[6]` | Y: ansi.rs PrivateMode::new (6 -> NamedPrivateMode::Origin) | Y: state.c set_dec_mode case 6 | Y: stream_terminal |
| F10 | DECAWM `?7` autowrap off/on; deferred wrap (xenl) semantics | P | 1 | Y: term input, wrapline | P: grid.rs col_inc, col_wrap | Y: ansi.rs PrivateMode::new (7 -> LineWrap) | Y: state.c case 7 in set_dec_mode | Y: Terminal.zig print |
| F11 | IRM insert mode `CSI 4 h/l` | N | 3 | Y: term set_mode, input | N: perform.rs | Y: ansi.rs Mode::new (4 -> NamedMode::Insert) | Y: state.c set_mode case 4 | Y: Terminal.zig print (modes insert) |
| F12 | Tab stops: HTS `ESC H`, TBC `CSI g` / `CSI 3 g`, CBT `CSI Z`, CHT `CSI I` | P | 5 | Y: term set_horizontal_tabstop, clear_tabs, move_backward_tabs, move_forward_tabs, put_tab | N: grid.rs col_tab | Y: ansi.rs (ESC H) | Y: state.c (0x88 HTS), TBC, CBT, CHT | Y: Terminal.zig |
| F13 | Alternate screen: `?47`, `?1047`, `?1048`, `?1049` (clear on entry, saved cursor), and whether scrollback is suppressed on the alt screen | N | 4 | P: term swap_alt, set_private_mode | P: screen.rs | P: ansi.rs PrivateMode::new (1049 -> SwapScreenAndSetRestoreCursor) | P: state.c set_dec_mode cases / | Y: Terminal.zig switchScreenMode |
| F14 | DECTCEM `?25`; cursor blink `?12`; DECSCUSR `CSI Ps SP q` cursor shape | N | 1 | Y: term set_private_mode, set_cursor_style, set_cursor_shape | P: screen.rs `[25]` | Y: ansi.rs PrivateMode::new (25 ShowCursor | Y: state.c set_dec_mode | Y: modes.zig |
| F15 | DECCKM `?1` and DECKPAM/DECKPNM `ESC =` / `ESC >` (mode state; key encoding if the engine has an encoder) | N | 1 | Y: term set_private_mode, set_keypad_application_mode | Y: screen.rs `[1]` | Y: ansi.rs PrivateMode::new (1 -> CursorKeys) | Y: state.c set_dec_mode case 1 | Y: modes.zig cursor_keys |
| F16 | BCE: erase operations fill with the current background color | P | 3 | Y: term/cell.rs Cell::reset | P: row.rs erase, cell.rs clear | N (implementer's) | Y: screen.c erase_internal | Y: Screen.zig blankCell |
| F17 | ED 3 / `CSI 3 J` (clear scrollback) | P | 8 | Y: term clear_screen | Y: screen.rs `3 =>` | Y: ansi.rs ('J') | Y: state.c ED case 3 (~) | Y: Terminal.zig eraseDisplay .scrollback |
| F18 | REP `CSI n b` (repeat preceding graphic character) | N | 3 | Y: vte ansi.rs csi_dispatch, Performer::print | Y: perform.rs `'b'` | Y: ansi.rs ('b') | Y: state.c REP | Y: Terminal.zig printRepeat |
| F19 | DECALN `ESC # 8` | N | 1 | Y: term decaln | N: perform.rs | Y: ansi.rs (b'8' with [b'#']) | Y: state.c on_escape '#' '8' | Y: Terminal.zig decaln |
| F20 | Character sets: G0/G1 designation `ESC ( 0`, `ESC ( B`, `ESC ) 0`, SO/SI, DEC Special Graphics (line drawing) translation; G2/G3 and single shifts | N | 5 | P: vte ansi.rs esc_dispatch, StandardCharset::map | N: perform.rs (SO/SI) | P: ansi.rs configure_charset! | Y: state.c SCS | Y: charsets.zig |
| F21 | DECSTR soft reset `CSI ! p`; RIS `ESC c` | P | 4 | P: term reset_state | P: perform.rs `b'c'` | P: ansi.rs (ESC c) | Y: state.c DECSTR | Y: Terminal.zig softReset |
| F22 | DECCOLM `?3` (what it does: ignore, clear, resize) | N | 8 | Y: term deccolm | N: screen.rs decset | Y: ansi.rs PrivateMode::new (3 -> ColumnMode) | N: state.c set_dec_mode | Y: Terminal.zig deccolm |
| F23 | DECSCNM `?5` reverse video screen | N | 8 | N: vte ansi.rs PrivateMode::new | N: screen.rs decset | P: ansi.rs PrivateMode::new | Y: state.c set_dec_mode case 5 | Y: modes.zig reverse_colors |
| F24 | DECSLRM left/right margins `?69` + `CSI l;r s` | N | out | N: vte ansi.rs PrivateMode::new, csi_dispatch | N: screen.rs | N: ansi.rs ('s') | Y: state.c set_dec_mode case | Y: Terminal.zig setLeftAndRightMargin |
| F25 | Wide characters (East Asian Wide, emoji: two cells, wrap of a wide char at the margin, erasing half of one), which width table | N | 7 | Y: term input, write_at_cursor | Y: screen.rs text | N (implementer's): lib.rs ground_dispatch | P: unicode.c vterm_unicode_width | Y: Terminal.zig |
| F26 | Combining characters / grapheme clusters (zero-width joins to the previous cell) | N | 7 | P: term input | P: screen.rs | N (implementer's): lib.rs ground_dispatch | P: state.c on_text | Y: Terminal.zig |
| F27 | Resize: what happens to content (reflow or truncate), cursor clamping, alt-screen | P | n/a | Y: term resize | Y: screen.rs set_size | N (implementer's) | Y: state.c on_resize | Y: Terminal.zig resize |
| F28 | Synchronized output `?2026` | N | out | Y: vte ansi.rs Processor::advance, advance_sync, csi_dispatch `?2026h` | N: screen.rs decset | Y: ansi.rs consts | N: state.c set_dec_mode | Y: modes.zig |
| F29 | Scrollback storage, size limit | Y | n/a | Y: grid/storage.rs Storage | Y: grid.rs, scroll_up, set_scrollback | N (implementer's) | P: screen.c sb_pushline_from_row, moverect_internal | Y: PageList.zig |
| F30 | SGR coverage: bold/dim/italic/underline styles (4:x), underline color 58/59, blink, reverse, invisible, strike, overline 53, 256 and 24-bit colors in `;` and `:` forms | Y | n/a | P: vte ansi.rs attrs_from_sgr_parameters | P: screen.rs sgr | P: ansi.rs attrs_from_sgr_parameters | P: pen.c vterm_state_setpen | Y: sgr.zig |
| F31 | OSC 4 / 104 set/reset palette entries; OSC 10/11/12 set default fg/bg/cursor color AND their `?` query forms (reply) | N | 6, 8 | Y: vte ansi.rs osc_dispatch | N: perform.rs osc_dispatch | Y: ansi.rs (OSC 4) | N: state.c on_osc | Y: stream_terminal colorOperation |
| F32 | OSC 0/1/2 window title; XTWINOPS title stack `CSI 22 t` / `CSI 23 t` | N | consume | P: vte ansi.rs osc_dispatch | P: perform.rs | P: ansi.rs (OSC 0/2) | P: state.c on_osc cases 0/1/2 | P: stream_terminal windowTitle |
| F33 | OSC 8 hyperlinks | N | out | Y: vte ansi.rs osc_dispatch (b"8") | N: perform.rs | Y: ansi.rs (OSC 8) | N: state.c on_osc | Y: Screen.zig startHyperlink/endHyperlink |
| F34 | OSC 52 clipboard | N | out | Y: term clipboard_store, clipboard_load, Config.osc52 | Y: perform.rs | Y: ansi.rs (OSC ) | Y: state.c osc_selection | Y: stream_terminal clipboardContents ~ |
| F35 | OSC 7 / 133 (cwd, shell integration): just presence | N | out | N: vte ansi.rs osc_dispatch | N: perform.rs | N: ansi.rs osc_dispatch | N: state.c on_osc | Y: stream_terminal reportPwd |
| F40 | DA1 `CSI c`, DA2 `CSI > c`, DA3 `CSI = c`: exact reply string | N | 6 | P: term identify_terminal | N: perform.rs | Y: ansi.rs ('c' any intermediate, first param 0) | P: state.c DA, DA2 | Y: device_attributes.zig |
| F41 | DSR 5 and DSR 6 (CPR), and DECXCPR `CSI ? 6 n` | N | 6 | P: term device_status | N: perform.rs | P: ansi.rs ('n') | Y: state.c DSR/DECDSR | Y: stream_terminal deviceStatus |
| F42 | DECRQM `CSI ? Ps $ p` and ANSI form | N | 6 | Y: term report_private_mode, report_mode | N: perform.rs | Y: ansi.rs ('p' $) ('p' ? $) | P: state.c dispatch | Y: stream_terminal requestMode |
| F43 | XTVERSION `CSI > 0 q` | N | 6 | N: vte ansi.rs csi_dispatch | N: perform.rs | N: ansi.rs csi_dispatch | Y: state.c request_version_string | Y: stream_terminal reportXtversion |
| F44 | XTWINOPS reports `CSI 14 t`, `CSI 16 t`, `CSI 18 t` | N | 6 | P: term text_area_size_pixels, text_area_size_chars | N: perform.rs | P: ansi.rs ('t') | N: state.c on_csi | Y: stream_terminal reportSize |
| F45 | DCS DECRQSS `DCS $ q ... ST` | N | 6 | N: vte ansi.rs Performer::hook | N: perform.rs (no hook/put/unhook) | N: ansi.rs hook/put/unhook | Y: state.c request_status_string, on_dcs | P: dcs.zig DECRQSS.encode |
| F46 | DCS XTGETTCAP `DCS + q ... ST` | N | 6 | N: vte ansi.rs Performer::hook | N: same as F45 | N: ansi.rs | N: state.c on_dcs | Y: stream_terminal dcsCommand xtgettcap |
| F47 | Kitty keyboard protocol queries `CSI ? u`, push/pop `CSI > u` / `CSI < u` | N | out | Y: term report_keyboard_mode, push_keyboard_mode, pop_keyboard_modes, set_keyboard_mode | N: perform.rs | Y: ansi.rs ('u' with ? = > <) | N: state.c on_csi | Y: stream.zig u |
| F48 | How replies leave the engine (callback, event, buffer) | N | 6 | Y: event.rs Event, EventListener::send_event | N: callbacks.rs Callbacks | Y: ansi.rs Handler trait | Y: vterm.c vterm_output_set_callback, vterm_push_output_bytes, vterm_output_read | Y: stream_terminal Effects.write_pty |
| F50 | Bracketed paste `?2004` | N | out | Y: term set_private_mode | Y: screen.rs `[2004]` | Y: ansi.rs PrivateMode::new (2004 -> BracketedPaste) | Y: state.c set_dec_mode | Y: modes.zig |
| F51 | Focus events `?1004` | N | out | Y: term set_private_mode | N: screen.rs decset | Y: ansi.rs PrivateMode::new (1004 -> ReportFocusInOut) | Y: state.c set_dec_mode | Y: modes.zig |
| F52 | Mouse tracking modes 9/1000/1002/1003 and encodings 1005/1006/1015; mouse report encoder present? | N | out | P: term set_private_mode | P: screen.rs | P: ansi.rs PrivateMode::new | P: state.c set_dec_mode //// | Y: stream_terminal |
| F53 | modifyOtherKeys `CSI > 4 ; Ps m` | N | out | N: vte ansi.rs csi_dispatch ('m', '>') | N: perform.rs | Y: ansi.rs ('m' >) ('m' ?) | N: state.c on_csi | P: stream.zig CSI > m |
| F54 | Alternate scroll `?1007`, meta `?1034`/`?1036` | N | out | P: term set_private_mode, TermMode::default | N: screen.rs decset | P: ansi.rs PrivateMode::new (1007 -> AlternateScroll) | N: state.c set_dec_mode | P: modes.zig |
| F60 | Parser model (Paul Williams DEC ANSI state machine or other), max params, max intermediates, OSC/DCS length limits, UTF-8 handling of invalid bytes, C1 8-bit controls | P | 1 | Y: vte:lib.rs Parser, advance_ground, advance_osc_string | Y: vte-0.15.0:lib.rs (doc) | Y: lib.rs (State enum | Y: parser.c vterm_input_write | Y: Parser.zig |
| F61 | Damage / dirty tracking exposed to the caller | Y | n/a | Y: term damage, reset_damage, LineDamageBounds, TermDamage | N: screen.rs state_diff, contents_diff, rows_diff, input_mode_diff | N (implementer's) | Y: screen.c damagerect, vterm_screen_flush_damage, vterm_screen_set_damage_merge | Y: render.zig RenderState.dirty |
