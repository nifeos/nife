use super::*;
use crate::test_support::{Screen, feed_all};
extern crate std;
use std::vec::Vec;

/// Typing echoes, Enter completes, and the completed line is exactly what was typed. The
/// baseline everything else builds on.
#[test]
fn plain_typing_makes_a_line() {
    let (mut d, mut s) = (LineDisc::new(), Screen::new());
    let events = feed_all(&mut d, &mut s, b"echo hi\r");
    assert_eq!(events, [Event::Line]);
    assert_eq!(d.line(), b"echo hi");
    assert_eq!(s.text(), "echo hi");
}

/// **Tab is reported and changes nothing** (§227 option D): no echo, no edit, and the line in
/// progress and its cursor are what [`LineDisc::pending`] says. A terminal that ignores the
/// event therefore behaves exactly as it did when Tab fell into the catch-all.
#[test]
fn tab_is_an_event_that_leaves_the_line_alone() {
    let (mut d, mut s) = (LineDisc::new(), Screen::new());
    feed_all(&mut d, &mut s, b"ls do\x1b[D");
    let before = s.text();
    assert_eq!(feed_all(&mut d, &mut s, b"\t"), [Event::Tab]);
    assert_eq!(s.text(), before);
    assert_eq!(d.pending(), (b"ls do".as_slice(), 4));
}

/// **A completion inserts at the cursor as typing would**, mid-line included, and cannot
/// smuggle a control byte into the line: an escape in the text is skipped, not inserted.
#[test]
fn inserted_text_lands_at_the_cursor_and_control_bytes_are_refused() {
    let (mut d, mut s) = (LineDisc::new(), Screen::new());
    feed_all(&mut d, &mut s, b"cat do x\x1b[D\x1b[D");
    d.insert_text(b"cs/\x1b[2J", &mut s);
    assert_eq!(s.text(), "cat docs/[2J x");
    assert_eq!(d.pending().1, 12);
    assert_eq!(feed_all(&mut d, &mut s, b"\r"), [Event::Line]);
    assert_eq!(d.line(), b"cat docs/[2J x");
}

/// A repaint after a listing redraws the prompt and the line with the cursor where it was.
#[test]
fn repaint_restores_prompt_line_and_cursor() {
    let (mut d, mut s) = (LineDisc::new(), Screen::new());
    d.start_line(b"$ ", &mut s);
    feed_all(&mut d, &mut s, b"ls a\x1b[D");
    let mut fresh = Screen::new();
    d.repaint(&mut fresh);
    assert_eq!(fresh.text(), "$ ls a");
    assert_eq!(fresh.col, 5);
}

/// Backspace erases from the screen as well as the buffer, including mid-line, where the
/// tail must shift left visibly.
#[test]
fn backspace_erases_on_screen() {
    let (mut d, mut s) = (LineDisc::new(), Screen::new());
    feed_all(&mut d, &mut s, b"ekko");
    feed_all(&mut d, &mut s, &[0x7f, 0x7f, 0x7f]);
    assert_eq!(s.text(), "e");
    feed_all(&mut d, &mut s, b"cho\r");
    assert_eq!(d.line(), b"echo");
}

/// Cursor keys move the insertion point: arrow-left three times into "hllo", insert 'e',
/// get "hello", and the screen agrees at every step.
#[test]
fn arrow_keys_edit_mid_line() {
    let (mut d, mut s) = (LineDisc::new(), Screen::new());
    feed_all(&mut d, &mut s, b"hllo");
    feed_all(&mut d, &mut s, b"\x1b[D\x1b[D\x1b[D");
    assert_eq!(feed_all(&mut d, &mut s, b"e"), []);
    assert_eq!(s.text(), "hello");
    assert_eq!(feed_all(&mut d, &mut s, b"\r"), [Event::Line]);
    assert_eq!(d.line(), b"hello");
}

/// SS3 arrows (application cursor mode, `ESC O D`) work exactly like CSI arrows.
#[test]
fn ss3_arrows_are_understood() {
    let (mut d, mut s) = (LineDisc::new(), Screen::new());
    feed_all(&mut d, &mut s, b"ab\x1bODc\r");
    assert_eq!(d.line(), b"acb");
}

/// Home/End in all three encodings land the cursor where they claim: type at Home and the
/// character appears at the front.
#[test]
fn home_and_end_place_the_cursor() {
    for home in [&b"\x1b[H"[..], b"\x1b[1~", b"\x01"] {
        let (mut d, mut s) = (LineDisc::new(), Screen::new());
        feed_all(&mut d, &mut s, b"bc");
        feed_all(&mut d, &mut s, home);
        feed_all(&mut d, &mut s, b"a");
        feed_all(&mut d, &mut s, b"\x1b[F"); // End
        feed_all(&mut d, &mut s, b"d\r");
        assert_eq!(d.line(), b"abcd", "home encoding {home:x?}");
        assert_eq!(s.text(), "abcd");
    }
}

/// Delete (CSI 3~) removes under the cursor; ^D does the same on a non-empty line.
#[test]
fn delete_forward_both_ways() {
    let (mut d, mut s) = (LineDisc::new(), Screen::new());
    feed_all(&mut d, &mut s, b"axbc");
    feed_all(&mut d, &mut s, b"\x1b[D\x1b[D\x1b[D"); // cursor on 'x'
    feed_all(&mut d, &mut s, b"\x1b[3~");
    assert_eq!(s.text(), "abc");
    feed_all(&mut d, &mut s, b"\x04"); // ^D on 'b'
    assert_eq!(s.text(), "ac");
    feed_all(&mut d, &mut s, b"\r");
    assert_eq!(d.line(), b"ac");
}

/// ^K kills to end, ^Y yanks it back: cut "ef", move home, yank, and the killed text lands
/// at the cursor, on screen and in the line.
#[test]
fn kill_to_end_and_yank() {
    let (mut d, mut s) = (LineDisc::new(), Screen::new());
    feed_all(&mut d, &mut s, b"abcdef");
    feed_all(&mut d, &mut s, b"\x02\x02"); // ^B x2, cursor before "ef"
    feed_all(&mut d, &mut s, b"\x0b"); // ^K
    assert_eq!(s.text(), "abcd");
    feed_all(&mut d, &mut s, b"\x01\x19"); // ^A then ^Y
    assert_eq!(s.text(), "efabcd");
    feed_all(&mut d, &mut s, b"\r");
    assert_eq!(d.line(), b"efabcd");
}

/// ^U kills to start; ^W kills the word before the cursor, blanks first.
#[test]
fn kill_to_start_and_kill_word() {
    let (mut d, mut s) = (LineDisc::new(), Screen::new());
    feed_all(&mut d, &mut s, b"rm -rf /\x15"); // ^U
    assert_eq!(s.text(), "");
    feed_all(&mut d, &mut s, b"echo one two");
    feed_all(&mut d, &mut s, b"\x17"); // ^W
    assert_eq!(s.text(), "echo one");
    feed_all(&mut d, &mut s, b"\x17\x17\r");
    assert_eq!(d.line(), b"");
}

/// Up arrow recalls the previous line; a second up goes older; down comes back; and
/// browsing past the newest entry restores the line that was in progress.
#[test]
fn history_browsing_is_non_destructive() {
    let (mut d, mut s) = (LineDisc::new(), Screen::new());
    feed_all(&mut d, &mut s, b"first\r");
    feed_all(&mut d, &mut s, b"second\r");
    s = Screen::new();
    feed_all(&mut d, &mut s, b"in prog"); // a line in progress
    feed_all(&mut d, &mut s, b"\x1b[A");
    assert_eq!(s.text(), "second");
    feed_all(&mut d, &mut s, b"\x1b[A");
    assert_eq!(s.text(), "first");
    feed_all(&mut d, &mut s, b"\x1b[B\x1b[B");
    assert_eq!(s.text(), "in prog");
    feed_all(&mut d, &mut s, b"\r");
    assert_eq!(d.line(), b"in prog");
}

/// A recalled history entry can be edited and submitted, and an exact repeat of the newest
/// entry is not recorded twice (up, up must reach the older command).
#[test]
fn history_recall_edits_and_dedups() {
    let (mut d, mut s) = (LineDisc::new(), Screen::new());
    feed_all(&mut d, &mut s, b"run 9\r");
    feed_all(&mut d, &mut s, b"run 9\r"); // repeat: must not duplicate
    feed_all(&mut d, &mut s, b"help\r");
    s = Screen::new();
    feed_all(&mut d, &mut s, b"\x1b[A\x1b[A"); // help, then run 9 (not help twice)
    assert_eq!(s.text(), "run 9");
    feed_all(&mut d, &mut s, &[0x7f]);
    feed_all(&mut d, &mut s, b"7\r");
    assert_eq!(d.line(), b"run 7");
}

/// The history ring holds [`HIST`] entries and evicts the oldest.
#[test]
fn history_ring_evicts_the_oldest() {
    let (mut d, mut s) = (LineDisc::new(), Screen::new());
    for i in 0..=HIST {
        feed_all(&mut d, &mut s, std::format!("cmd{i}\r").as_bytes());
    }
    s = Screen::new();
    for _ in 0..HIST + 5 {
        feed_all(&mut d, &mut s, b"\x1b[A"); // walk to the oldest, then bell
    }
    assert_eq!(s.text(), "cmd1", "cmd0 should have been evicted");
    assert!(s.bells > 0, "walking past the oldest should ring the bell");
}

/// ^C discards the line in progress and reports Interrupt; the next line starts clean.
#[test]
fn ctrl_c_discards_the_line() {
    let (mut d, mut s) = (LineDisc::new(), Screen::new());
    let events = feed_all(&mut d, &mut s, b"rm -rf\x03");
    assert_eq!(events, [Event::Interrupt]);
    let events = feed_all(&mut d, &mut s, b"ok\r");
    assert_eq!(events, [Event::Line]);
    assert_eq!(d.line(), b"ok");
}

/// ^Z reports Suspend and, like ^C, leaves no byte of itself in the line: a job-control key that
/// was stored as text would reach the next command (milestone 48 (job control)).
#[test]
fn ctrl_z_suspends_and_discards_the_line() {
    let (mut d, mut s) = (LineDisc::new(), Screen::new());
    let events = feed_all(&mut d, &mut s, b"partial\x1a");
    assert_eq!(events, [Event::Suspend]);
    let events = feed_all(&mut d, &mut s, b"ok\r");
    assert_eq!(events, [Event::Line]);
    assert_eq!(d.line(), b"ok");
}

/// ^D is EOF only on an empty line.
#[test]
fn ctrl_d_is_eof_only_when_empty() {
    let (mut d, mut s) = (LineDisc::new(), Screen::new());
    assert_eq!(feed_all(&mut d, &mut s, b"\x04"), [Event::Eof]);
    assert_eq!(feed_all(&mut d, &mut s, b"x\x04"), []);
}

/// CR, LF, and CRLF each end exactly one line: piped scripts (LF), terminals (CR), and
/// CRLF senders all behave.
#[test]
fn line_endings_cr_lf_crlf() {
    let (mut d, mut s) = (LineDisc::new(), Screen::new());
    let events = feed_all(&mut d, &mut s, b"a\rb\nc\r\nd\r");
    assert_eq!(events.len(), 4, "CRLF must not count as two Enters");
    assert_eq!(d.line(), b"d");
}

/// An overlong line refuses further input with a bell instead of truncating silently or
/// walking off the buffer.
#[test]
fn overflow_rings_the_bell() {
    let (mut d, mut s) = (LineDisc::new(), Screen::new());
    for _ in 0..LINE_MAX + 3 {
        d.feed(b'a', &mut s);
    }
    assert_eq!(s.bells, 3);
    feed_all(&mut d, &mut s, b"\r");
    assert_eq!(d.line().len(), LINE_MAX);
}

/// ^L repaints the prompt and the line after clearing, with the cursor where it was:
/// typing continues mid-line.
#[test]
fn ctrl_l_repaints_prompt_and_line() {
    let (mut d, mut s) = (LineDisc::new(), Screen::new());
    d.start_line(b"$ ", &mut s);
    feed_all(&mut d, &mut s, b"ab\x1b[D"); // cursor between a and b
    feed_all(&mut d, &mut s, b"\x0c");
    assert!(s.cleared);
    assert_eq!(s.text(), "$ ab");
    feed_all(&mut d, &mut s, b"x");
    assert_eq!(
        s.text(),
        "$ axb",
        "typing after ^L must land mid-line on screen"
    );
    feed_all(&mut d, &mut s, b"\r");
    assert_eq!(d.line(), b"axb");
}

/// `start_line` paints the prompt and any type-ahead, and the completed line includes the
/// type-ahead: bytes that arrived before the read are not lost.
#[test]
fn start_line_shows_type_ahead() {
    let (mut d, mut s) = (LineDisc::new(), Screen::new());
    feed_all(&mut d, &mut s, b"early");
    let mut s2 = Screen::new();
    d.start_line(b"$ ", &mut s2);
    assert_eq!(s2.text(), "$ early");
    feed_all(&mut d, &mut s2, b"\r");
    assert_eq!(d.line(), b"early");
}

/// **A read re-issued after `FLAG_RETRY` paints nothing and loses nothing.** The half-typed
/// line survives, the prompt is remembered for later repaints, and finishing the line returns
/// all of it: the promise `FLAG_RETRY` makes to a reader.
#[test]
fn a_resumed_read_keeps_the_half_typed_line_and_paints_nothing() {
    let (mut d, mut s) = (LineDisc::new(), Screen::new());
    d.start_line(b"$ ", &mut s);
    feed_all(&mut d, &mut s, b"ech");
    let mut quiet = Screen::new();
    d.resume_line(b"$ ");
    assert_eq!(
        quiet.text(),
        "",
        "a resumed read must not repaint the prompt"
    );
    feed_all(&mut d, &mut quiet, b"o\r");
    assert_eq!(d.line(), b"echo");
}

/// Unknown CSI sequences (F-keys and friends) are swallowed whole: no stray bytes appear
/// in the line or on the screen.
#[test]
fn unknown_sequences_are_swallowed() {
    let (mut d, mut s) = (LineDisc::new(), Screen::new());
    feed_all(&mut d, &mut s, b"a\x1b[15~b\x1b[Zc\r"); // F5, shift-tab
    assert_eq!(d.line(), b"abc");
    assert_eq!(s.text(), "abc");
}

/// `abandon` clears the buffer, cursor, and history browsing, but leaves history and the kill
/// buffer alone: a mode switch mid-line must not erase what earlier lines put there.
#[test]
fn abandon_clears_the_line_not_the_history() {
    let (mut d, mut s) = (LineDisc::new(), Screen::new());
    feed_all(&mut d, &mut s, b"first\r"); // one real history entry
    feed_all(&mut d, &mut s, b"x\x02\x0b"); // type 'x', ^B onto it, ^K kills it to end
    feed_all(&mut d, &mut s, b"\x1b[A"); // browse into history
    d.abandon();
    assert_eq!(
        d.line(),
        b"first",
        "abandon must not touch the last completed line"
    );
    // The kill buffer survives: yanking it after abandon still works.
    s = Screen::new();
    feed_all(&mut d, &mut s, b"\x19");
    assert_eq!(s.text(), "x", "abandon must not clear the kill buffer");
    // History survives: up-arrow still reaches the recorded line.
    s = Screen::new();
    feed_all(&mut d, &mut s, b"\x15"); // ^U: clear the yanked "x"
    feed_all(&mut d, &mut s, b"\x1b[A");
    assert_eq!(s.text(), "first", "abandon must not clear history");
}

/// The control-character encodings of movement (^B ^A ^E ^F), which the CSI tests do not
/// cover, including ^B at column 0 where there is nowhere to go.
#[test]
fn control_key_movement() {
    let (mut d, mut s) = (LineDisc::new(), Screen::new());
    feed_all(&mut d, &mut s, b"\x02"); // ^B on an empty line: nothing to do
    feed_all(&mut d, &mut s, b"ab");
    feed_all(&mut d, &mut s, b"\x01\x06x"); // ^A, ^F right one, insert mid-line
    assert_eq!(s.text(), "axb");
    feed_all(&mut d, &mut s, b"\x05c\r"); // ^E, append
    assert_eq!(d.line(), b"axbc");
    assert_eq!(s.text(), "axbc");
}

/// CSI C moves right mid-line and is a no-op at the end of the line; typing after each
/// proves where the cursor actually is (the echo alone would not).
#[test]
fn right_arrow_moves_and_stops_at_end() {
    let (mut d, mut s) = (LineDisc::new(), Screen::new());
    feed_all(&mut d, &mut s, b"ab\x1b[D\x1b[D"); // cursor to column 0
    feed_all(&mut d, &mut s, b"\x1b[Cx"); // right one, insert between a and b
    assert_eq!(s.text(), "axb");
    feed_all(&mut d, &mut s, b"\x1b[C\x1b[C"); // to the end, then once more (no-op)
    feed_all(&mut d, &mut s, b"c\r");
    assert_eq!(d.line(), b"axbc");
}

/// A CSI with a ';' (shift-Delete, `ESC [ 3 ; 2 ~`) still dispatches on the first
/// parameter: the ';' advances to the second slot instead of aborting the sequence.
#[test]
fn csi_second_parameter_is_parsed() {
    let (mut d, mut s) = (LineDisc::new(), Screen::new());
    feed_all(&mut d, &mut s, b"abc\x01"); // ^A, cursor on 'a'
    feed_all(&mut d, &mut s, b"\x1b[3;2~"); // delete under cursor, modifier ignored
    assert_eq!(s.text(), "bc");
    feed_all(&mut d, &mut s, b"\r");
    assert_eq!(d.line(), b"bc");
}

/// Type-ahead with the cursor moved off the end: `start_line` repaints and puts the
/// cursor back where it was, so the next insertion lands mid-line on screen too.
#[test]
fn start_line_restores_mid_line_cursor() {
    let (mut d, mut s) = (LineDisc::new(), Screen::new());
    feed_all(&mut d, &mut s, b"ab\x02"); // type ahead, cursor between a and b
    let mut s2 = Screen::new();
    d.start_line(b"$ ", &mut s2);
    feed_all(&mut d, &mut s2, b"X");
    assert_eq!(s2.text(), "$ aXb");
    feed_all(&mut d, &mut s2, b"\r");
    assert_eq!(d.line(), b"aXb");
}

/// Enter on an empty line completes an empty read but records nothing: up must recall
/// the last real command, not a blank.
#[test]
fn empty_lines_stay_out_of_history() {
    let (mut d, mut s) = (LineDisc::new(), Screen::new());
    feed_all(&mut d, &mut s, b"one\r\r");
    s = Screen::new();
    feed_all(&mut d, &mut s, b"\x1b[A");
    assert_eq!(s.text(), "one");
}

/// An immediate repeat is stored once: with "a" entered twice, the second up-arrow has
/// nowhere older to go and rings the bell. The screen alone cannot tell one stored copy
/// from two, which is why the bell count is the assertion here.
#[test]
fn duplicate_entry_is_stored_once() {
    let (mut d, mut s) = (LineDisc::new(), Screen::new());
    feed_all(&mut d, &mut s, b"a\ra\r");
    s = Screen::new();
    feed_all(&mut d, &mut s, b"\x1b[A\x1b[A");
    assert_eq!(s.text(), "a");
    assert_eq!(s.bells, 1, "one stored entry: the second up must bell");
}

/// Walk a wrapped ring end to end in both directions, asserting every entry: ten
/// commands in a ring of eight, plus a deduplicated repeat across the wrap. The
/// existing browsing tests never cross the wrap point, which is where the ring index
/// arithmetic can be wrong without them noticing.
#[test]
fn wrapped_ring_walks_correctly_both_ways() {
    let (mut d, mut s) = (LineDisc::new(), Screen::new());
    for i in 0..10 {
        feed_all(&mut d, &mut s, std::format!("cmd{i}\r").as_bytes());
    }
    feed_all(&mut d, &mut s, b"cmd9\r"); // repeat of the newest, post-wrap: not stored
    s = Screen::new();
    for i in (2..=9).rev() {
        feed_all(&mut d, &mut s, b"\x1b[A");
        assert_eq!(s.text(), std::format!("cmd{i}"), "up should reach cmd{i}");
    }
    feed_all(&mut d, &mut s, b"\x1b[A"); // past the oldest
    assert_eq!(s.bells, 1);
    assert_eq!(s.text(), "cmd2");
    for i in 3..=9 {
        feed_all(&mut d, &mut s, b"\x1b[B");
        assert_eq!(s.text(), std::format!("cmd{i}"), "down should reach cmd{i}");
    }
    feed_all(&mut d, &mut s, b"\x1b[B"); // past the newest: the (empty) line in progress
    assert_eq!(s.text(), "");
}

/// Browsing away and back restores not just the in-progress text but the cursor column:
/// typing after the round trip lands where the cursor was left. The buffer restore is
/// checked elsewhere; this pins the on-screen column, which only the echo carries.
#[test]
fn stash_restores_cursor_column() {
    let (mut d, mut s) = (LineDisc::new(), Screen::new());
    feed_all(&mut d, &mut s, b"old\r");
    s = Screen::new();
    feed_all(&mut d, &mut s, b"hello\x02\x02"); // cursor after "hel"
    feed_all(&mut d, &mut s, b"\x1b[A\x1b[B"); // up to "old", back down
    feed_all(&mut d, &mut s, b"X");
    assert_eq!(s.text(), "helXlo");
    feed_all(&mut d, &mut s, b"\r");
    assert_eq!(d.line(), b"helXlo");
}

/// A yank that does not fit is truncated to the room left: the line ends exactly full,
/// silently (the bell is reserved for a yank with no room at all).
#[test]
fn yank_truncates_to_the_room_left() {
    let (mut d, mut s) = (LineDisc::new(), Screen::new());
    for _ in 0..250 {
        d.feed(b'a', &mut s);
    }
    feed_all(&mut d, &mut s, b"\x15"); // ^U: kill all 250
    for _ in 0..10 {
        d.feed(b'b', &mut s);
    }
    feed_all(&mut d, &mut s, b"\x19\r"); // ^Y: only 246 of the 250 fit
    assert_eq!(s.bells, 0);
    assert_eq!(d.line().len(), LINE_MAX);
    assert_eq!(&d.line()[..10], b"bbbbbbbbbb");
    assert!(d.line()[10..].iter().all(|&b| b == b'a'));
}

/// ^Y with nothing ever killed is silent; ^Y with a kill buffer but a full line rings
/// the bell (there was something to yank and nowhere to put it). Both sides matter: the
/// distinction is the condition, not just the bell.
#[test]
fn yank_bell_needs_a_kill_and_a_full_line() {
    let (mut d, mut s) = (LineDisc::new(), Screen::new());
    feed_all(&mut d, &mut s, b"\x19"); // empty kill buffer: nothing, no bell
    assert_eq!(s.bells, 0);
    feed_all(&mut d, &mut s, b"x\x15"); // kill buffer now holds "x"
    for _ in 0..LINE_MAX {
        d.feed(b'a', &mut s);
    }
    feed_all(&mut d, &mut s, b"\x19"); // no room at all
    assert_eq!(s.bells, 1);
}

/// Yank into the middle of a line, then keep typing: the tail stays put on screen
/// through both the yank repaint and the next insertion.
#[test]
fn yank_mid_line_keeps_the_tail_in_place() {
    let (mut d, mut s) = (LineDisc::new(), Screen::new());
    feed_all(&mut d, &mut s, b"abcdef\x02\x02\x0b"); // ^K kills "ef"
    feed_all(&mut d, &mut s, b"\x02\x02\x19"); // yank it between "ab" and "cd"
    assert_eq!(s.text(), "abefcd");
    feed_all(&mut d, &mut s, b"X");
    assert_eq!(s.text(), "abefXcd");
    feed_all(&mut d, &mut s, b"\r");
    assert_eq!(d.line(), b"abefXcd");
}

/// ^W with only blanks before the cursor stops cleanly at column 0 (the blank-skipping
/// loop must not walk past the front of the buffer).
#[test]
fn word_kill_over_leading_blanks() {
    let (mut d, mut s) = (LineDisc::new(), Screen::new());
    feed_all(&mut d, &mut s, b"  \x17\r");
    assert_eq!(d.line(), b"");
    assert_eq!(s.text(), "");
}

/// ^W then ^Y round-trips exactly the killed word: the kill length is the word's, not
/// the cursor position's.
#[test]
fn word_kill_yank_round_trip() {
    let (mut d, mut s) = (LineDisc::new(), Screen::new());
    feed_all(&mut d, &mut s, b"ab cd\x17\x19\r");
    assert_eq!(d.line(), b"ab cd");
}

/// Home from column 12 needs a two-digit CSI count; typing at the front proves the
/// cursor really travelled all twelve columns. Every other test moves fewer than ten,
/// so the multi-digit encoding path was unexercised.
#[test]
fn multi_digit_cursor_move() {
    let (mut d, mut s) = (LineDisc::new(), Screen::new());
    feed_all(&mut d, &mut s, b"abcdefghijkl\x01X");
    assert_eq!(s.text(), "Xabcdefghijkl");
    feed_all(&mut d, &mut s, b"\r");
    assert_eq!(d.line(), b"Xabcdefghijkl");
}

// ---- the survivors of the 2026-10-03 mutation census ----

/// A sink that keeps the exact bytes, which the `Screen` model deliberately does not: it reads
/// `ESC [ D` and `ESC [ 1 D` alike.
struct Raw(Vec<u8>);
impl Sink for Raw {
    fn put(&mut self, b: &[u8]) {
        self.0.extend_from_slice(b);
    }
}

/// A move of one column elides its count, and a move of ten writes both digits.
#[test]
fn a_cursor_move_of_one_elides_its_count_and_ten_spells_it() {
    let mut o = Raw(Vec::new());
    csi_left(&mut o, 1);
    csi_right(&mut o, 1);
    csi_left(&mut o, 0);
    csi_right(&mut o, 10);
    assert_eq!(o.0, b"\x1b[D\x1b[C\x1b[10C");
}

/// **Resuming a read paints nothing** and still remembers the prompt, so a later repaint (after
/// a completion listing) prints it. A resume that forgot the prompt would repaint a bare line.
#[test]
fn resuming_a_read_remembers_the_prompt_and_paints_nothing() {
    let mut d = LineDisc::new();
    let mut o = Raw(Vec::new());
    d.resume_line(b"$ ");
    assert!(o.0.is_empty());
    d.repaint(&mut o);
    assert_eq!(o.0, b"$ ");
}

/// A repaint puts the cursor back where it was: the distance left of the end is `len - cur`,
/// nothing at the end of the line and the whole line at column 0.
#[test]
fn a_repaint_leaves_the_cursor_where_it_was() {
    let (mut d, mut s) = (LineDisc::new(), Screen::new());
    feed_all(&mut d, &mut s, b"abcde\x1b[D\x1b[D");
    let mut o = Raw(Vec::new());
    d.repaint(&mut o);
    assert_eq!(o.0, b"abcde\x1b[2D");
    feed_all(&mut d, &mut s, b"\x05"); // ^E: to the end
    let mut o = Raw(Vec::new());
    d.repaint(&mut o);
    assert_eq!(o.0, b"abcde");
    feed_all(&mut d, &mut s, b"\x01"); // ^A: to the start
    let mut o = Raw(Vec::new());
    d.repaint(&mut o);
    assert_eq!(o.0, b"abcde\x1b[5D");
}
