//! Machine-checked proofs (DECISIONS §14, the project's direction: a verified-Rust capability
//! microkernel) for the two pieces of the engine that do index arithmetic on input nobody here
//! chose: the escape parser and the range delete. The editor's
//! invariant is `cursor <= length <= LINE_MAXIMUM`, and every edit has to keep it.
//!
//! Name: provisional, for the module and the file.

use super::*;

/// A sink that keeps nothing: the proofs are about the editor's state, not its echo.
struct Discard;

impl Sink for Discard {
    fn put(&mut self, _bytes: &[u8]) {}
}

/// The longest line a harness starts from. The arithmetic under proof moves bytes by offsets and
/// lengths and does not depend on the capacity, and a solver over a symbolic 256-byte move does
/// not finish; the bound keeps the move small and the offsets fully symbolic below it.
const LONGEST: usize = LINE_MAXIMUM;

/// An editor holding any line up to [`LONGEST`] bytes with the cursor anywhere in it, and no
/// history.
fn any_editor() -> LineDisc {
    let mut editor = LineDisc::new();
    let length: usize = kani::any();
    let cursor: usize = kani::any();
    kani::assume(length <= LONGEST && cursor <= length);
    editor.len = length;
    editor.cur = cursor;
    editor
}

/// **A control sequence leaves the cursor inside the line.** From any line of up to [`LONGEST`] bytes, any cursor, any
/// accumulated parameters and separator count, and any next byte, `continue_escape` ends with
/// `cursor <= length`, and the length does not grow: a digit, a separator, a final byte that moves, deletes or
/// is unknown, and a malformed byte all keep it.
/// Falsification: replayable `crates/line_editor/falsifications/verification.a_control_sequence_leaves_the_cursor_inside_the_line.patch`
#[kani::proof]
#[kani::unwind(5)]
#[kani::solver(minisat)]
fn a_control_sequence_leaves_the_cursor_inside_the_line() {
    let mut editor = any_editor();
    let state = EscState::Csi {
        params: [kani::any(), kani::any()],
        n: kani::any(),
    };
    let byte: u8 = kani::any();
    let _ = editor.continue_escape(state, byte, &mut Discard);
    assert!(editor.cur <= editor.len);
    assert!(editor.len <= LONGEST);
}

/// **A range delete removes exactly its range.** From any line and any cursor at or after `a`,
/// `delete_range(a, b)` shortens the line by `b - a`, keeps the bytes outside the range in order,
/// and puts the cursor where the doc comment says: shifted back by the removed count when it was
/// at or past `b`, at `a` when it was inside the range or at `a`.
/// Falsification: replayable `crates/line_editor/falsifications/verification.a_range_delete_removes_exactly_its_range.patch`
#[kani::proof]
#[kani::unwind(5)]
#[kani::solver(minisat)]
fn a_range_delete_removes_exactly_its_range() {
    let mut editor = any_editor();
    let (a, b): (usize, usize) = (kani::any(), kani::any());
    kani::assume(a <= b && b <= editor.len && a <= editor.cur);
    let (length, cursor) = (editor.len, editor.cur);
    // Two marked bytes: one before the range and one after it.
    if a > 0 {
        editor.buf[a - 1] = 0xa1;
    }
    if b < length {
        editor.buf[b] = 0xb2;
    }
    editor.delete_range(a, b, &mut Discard);
    assert_eq!(editor.len, length - (b - a));
    if cursor >= b {
        assert_eq!(editor.cur, cursor - (b - a));
    } else {
        assert_eq!(editor.cur, a);
    }
    assert!(editor.cur <= editor.len);
    if a > 0 {
        assert_eq!(editor.buf[a - 1], 0xa1);
    }
    if b < length {
        assert_eq!(editor.buf[a], 0xb2);
    }
}
