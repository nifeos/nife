//! Fuzz the display terminal's escape-sequence engine with arbitrary bytes at several grid sizes,
//! fed whole and in pieces.
//!
//! `video_terminal::Vt::feed` interprets everything any program prints to the display: colors,
//! cursor motion, erase, scrolling, string sequences, UTF-8. Packaged software and foreign `std`
//! programs (ripgrep among them) write to it, so the bytes are ones nobody on this side chose. The
//! `display_terminal` and `graphical_terminal` components are confined, but a panic in either takes
//! the screen console down for every program on it, and a hang freezes it. The crate has no Kani
//! harness, and its parser is an unbounded loop over a stream, which is where a prover stops.

//! # What is asserted, beyond "it returned"
//!
//! 1. **The cursor is always inside the grid**, after every feed, at every size.
//! 2. **The damage rectangle is always inside the grid** (half-open, so `col + cols <= cols of the
//!    grid`), and a scroll report is always less than the grid's height, as `Damage` documents.
//! 3. **Where the input is cut does not change the screen.** The same bytes are fed in one call and
//!    in three pieces at fuzzer-chosen cut points, which land inside multi-byte UTF-8 sequences and
//!    escape sequences as often as anywhere. The two terminals must then agree on everything the
//!    handoff blob carries (the grid, the scrollback, the cursor, the pen, and the parser's own
//!    state, partial sequence included) and on every visible cell.
//!
//! # Input layout
//!
//! Byte 0 picks the grid size from a table (1 by 1, a single row, a single column, 80 by 24, the
//! largest grid, and sizes past the clamps). Bytes 1 and 2 are the cut points as a fraction of the
//! rest. Everything after is the stream. The seeds in `fuzz/seeds/video_terminal_feed` start with
//! those three bytes.

#![no_main]

use std::sync::Mutex;

use libfuzzer_sys::fuzz_target;
use video_terminal::Vt;

/// Grid sizes asked for; `Vt::new` clamps the last ones, which is part of what is exercised.
const SIZES: [(u32, u32); 12] = [
    (1, 1),
    (1, 24),
    (80, 1),
    (2, 2),
    (7, 3),
    (80, 24),
    (132, 50),
    (40, 150),
    (240, 40),
    (240, 150),
    (0, 0),
    (u32::MAX, u32::MAX),
];

/// A `Vt` is several hundred KiB, so the two live in a `static` built at compile time, as the
/// struct's doc asks, and are retargeted with `reset_to` on every input. Boxing them at runtime
/// would put each on the initializer's stack before it reached the heap.
static TERMINALS: Mutex<(Vt, Vt)> = Mutex::new((Vt::new(1, 1), Vt::new(1, 1)));

/// The grid, the cursor, damage: the invariants every feed must leave.
fn check_inside(vt: &mut Vt) {
    let (col, row) = vt.cursor();
    assert!(
        col < vt.cols() && row < vt.rows(),
        "the cursor left the grid"
    );
    if let Some(damage) = vt.take_damage() {
        let r = damage.rect;
        assert!(
            r.col.checked_add(r.cols).is_some_and(|x| x <= vt.cols())
                && r.row.checked_add(r.rows).is_some_and(|y| y <= vt.rows()),
            "the damage rectangle left the grid"
        );
        assert!(
            damage.scrolled < vt.rows(),
            "a scroll report of a whole grid"
        );
    }
}

fuzz_target!(|data: &[u8]| {
    let [size, first, second, stream @ ..] = data else {
        return;
    };
    let (cols, rows) = SIZES[usize::from(*size) % SIZES.len()];
    let cut = |selector: u8| stream.len() * usize::from(selector) / 255;
    let (a, b) = (cut(*first), cut(*second));
    let (a, b) = (a.min(b), a.max(b));

    {
        let mut terminals = TERMINALS.lock().expect("a failed input aborts the process");
        let (whole, pieces) = &mut *terminals;
        whole.reset_to(cols, rows);
        pieces.reset_to(cols, rows);

        whole.feed(stream);
        check_inside(whole);

        for piece in [&stream[..a], &stream[a..b], &stream[b..]] {
            pieces.feed(piece);
            check_inside(pieces);
        }

        assert_eq!(
            whole.cursor(),
            pieces.cursor(),
            "the cursor depends on the cut"
        );
        let length = whole.handoff_len();
        assert_eq!(
            length,
            pieces.handoff_len(),
            "the history depends on the cut"
        );
        let mut left = vec![0u8; length];
        let mut right = vec![0u8; length];
        whole
            .write_handoff(&mut left)
            .expect("the blob fits its own length");
        pieces
            .write_handoff(&mut right)
            .expect("the blob fits its own length");
        assert!(
            left == right,
            "the state depends on where the input was cut"
        );
        for row in 0..whole.rows() {
            for col in 0..whole.cols() {
                assert_eq!(whole.cell(col, row), pieces.cell(col, row));
            }
        }
    }
});
