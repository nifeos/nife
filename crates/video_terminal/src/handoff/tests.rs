//! Name: provisional, minted 2026-10-19 (UTC) by the lane for milestone 23 (a
//! capability-routed component OS with live replacement).

use super::*;
use crate::Vt;
use crate::geometry::{MAX_ROWS, SCROLLBACK_ROWS};
use crate::vt::MAX_PARAMS;
extern crate std;
use std::vec;

/// A terminal with everything a swap must carry set to something other than its default: a
/// scrollback ring `lines` deep (past 200 at 132 columns it has wrapped), a rendition in force, a wrap pending, a scrolled-back view,
/// and a 24-bit color sequence cut off after its `2;`, so the parser is mid-CSI with three
/// parameters taken.
fn busy(cols: u32, rows: u32, lines: u32) -> Vt {
    let mut t = Vt::new(cols, rows);
    for i in 0..lines {
        t.feed(std::format!("\x1b[3{}mline {i}\x1b[0m\r\n", i % 8).as_bytes());
    }
    t.feed(b"\x1b[1;4m");
    t.feed(&vec![b'x'; cols as usize]);
    t.feed(b"\x1b[38;2;");
    t.scroll_up(17);
    t
}

fn blob_of(t: &Vt) -> std::vec::Vec<u8> {
    let mut out = vec![0u8; t.handoff_len()];
    assert_eq!(t.write_handoff(&mut out), Some(out.len()));
    out
}

/// **What a replacement absorbs behaves as the incumbent would have**, which is the claim a
/// swap makes to the person at the screen. Compared on every pixel of the scrolled-back view, on
/// every row of history, and after feeding both the rest of the cut sequence and more text, so a
/// field the blob dropped shows as a difference somewhere a person would see it.
#[test]
fn an_absorbed_terminal_carries_on_exactly_where_the_incumbent_stopped() {
    // A ring that has wrapped, and one that has not, whose tail is not at slot zero.
    carries_on(400);
    carries_on(90);
}

fn carries_on(lines: u32) {
    let mut old = busy(132, 43, lines);
    let blob = blob_of(&old);
    let mut new = Vt::new(132, 43);
    new.absorb_handoff(&blob).expect("refused its own blob");
    assert_eq!(blob_of(&new), blob, "the blob does not read back to itself");

    let same = |a: &Vt, b: &Vt, what: &str| {
        for y in 0..a.height() {
            for x in 0..a.width() {
                assert_eq!(a.pixel(x, y), b.pixel(x, y), "{what}: pixel ({x}, {y})");
            }
        }
    };
    same(&old, &new, "the scrolled-back view");
    for depth in [0, 1, 150, old.scrollback_len()] {
        old.scroll_down(u32::MAX);
        new.scroll_down(u32::MAX);
        old.scroll_up(depth);
        new.scroll_up(depth);
        same(&old, &new, "history");
    }
    // The rest of the sequence: a red-orange `R`, which needs the parameters taken before the
    // swap, then more lines, which need the rendition, the pending wrap and the ring's tail.
    for t in [&mut old, &mut new] {
        t.feed(b"255;64;0mR\x1b[0m");
        for i in 0..60 {
            t.feed(std::format!("after {i}\r\n").as_bytes());
        }
    }
    assert_eq!(new.cell(1, 0), old.cell(1, 0));
    assert_eq!(blob_of(&new), blob_of(&old));
    same(&old, &new, "after more output");
}

/// A refused blob leaves the terminal as it was, so a replacement that refuses can still be told
/// apart from one that absorbed half: the supervisor collects it either way, but nothing it
/// reports is a lie about the state it held.
#[test]
fn every_refusal_leaves_the_terminal_untouched() {
    let blob = blob_of(&busy(132, 43, 400));
    let mut t = Vt::new(132, 43);
    t.feed(b"mine\r\n");
    let before = blob_of(&t);

    let mut bad = blob.clone();
    bad[0] ^= 1;
    assert_eq!(t.absorb_handoff(&bad), Err(Refusal::NoBlob));
    let mut bad = blob.clone();
    bad[4] = 9;
    assert_eq!(t.absorb_handoff(&bad), Err(Refusal::Layout(9)));
    // The last cell's character, a surrogate: no `char` is that.
    let mut bad = blob.clone();
    let last = bad.len() - 12;
    bad[last..last + 4].copy_from_slice(&0xd800u32.to_le_bytes());
    assert_eq!(t.absorb_handoff(&bad), Err(Refusal::Malformed));
    assert_eq!(
        t.absorb_handoff(&blob[..blob.len() - 1]),
        Err(Refusal::Malformed)
    );
    let mut wide = Vt::new(133, 43);
    assert_eq!(wide.absorb_handoff(&blob), Err(Refusal::Geometry));

    assert_eq!(blob_of(&t), before, "a refusal changed the terminal");
}

/// **The largest blob fits what a supervisor retypes for it.** A full live grid and a full ring at
/// the shape that fills both budgets exactly, which is the worst case [`HANDOFF_BYTES`] claims.
#[test]
fn the_largest_blob_fits_the_handoff_pages() {
    // 96 columns divides both MAX_CELLS (192 rows, clamped to MAX_ROWS) and SCROLLBACK_CELLS
    // (275 rows); 88 is the widest ring that is a full SCROLLBACK_ROWS deep.
    for cols in [88, 96, 132] {
        let mut t = Vt::new(cols, u32::MAX);
        for _ in 0..SCROLLBACK_ROWS + MAX_ROWS {
            t.feed(b"\r\n");
        }
        assert_eq!(t.scrollback_len(), t.scrollback_depth());
        assert!(t.handoff_len() <= HANDOFF_BYTES);
        assert!(t.write_handoff(&mut vec![0u8; HANDOFF_BYTES]).is_some());
    }
    assert!(HANDOFF_BYTES as u64 <= HANDOFF_PAGES * address_space_map::PAGE);
}

/// **What every fixture blob was written from**: a 20 by 6 terminal fed these bytes. A scrollback
/// ring that has wrapped (sixty lines at twenty columns is past its depth), every rendition flag and
/// both color forms in force on some cell, a wrap pending, and a 24-bit color cut after its `2;`.
/// Small on purpose, so a fixture is a few kilobytes in the tree.
const FIXTURE_COLS: u32 = 20;
const FIXTURE_ROWS: u32 = 6;

fn fixture_terminal() -> Vt {
    let mut t = Vt::new(FIXTURE_COLS, FIXTURE_ROWS);
    for i in 0..60 {
        t.feed(std::format!("\x1b[3{}m{i:02} history\x1b[0m\r\n", i % 8).as_bytes());
    }
    t.feed(b"\x1b[1;2;4;7;9m\x1b[38;5;202m\x1b[48;2;1;2;3mall of it\x1b[0m\r\n");
    t.feed(b"\x1b[33m");
    t.feed(&[b'w'; FIXTURE_COLS as usize]);
    t.feed(b"\x1b[38;2;");
    t
}

/// **Every handoff layout a build has ever written, as that build wrote it**, oldest first. A
/// replacement must read its predecessor's blob (calef, 2026-10-19 UTC), so a build that bumps
/// [`LAYOUT`] keeps the reader for the old one and adds its own blob here:
/// `cargo test -p video_terminal write_the_fixture_for_this_layout -- --ignored` writes it.
const FIXTURES: &[(u32, &[u8])] = &[
    (1, include_bytes!("layout1.blob")),
    (2, include_bytes!("layout2.blob")),
];

/// **This build reads every layout that came before it.** Each fixture is absorbed and must
/// behave exactly as a terminal fed the fixture's bytes today: every pixel at every scroll depth,
/// and after the cut sequence is finished. Compared through behavior rather than bytes, so the
/// test holds across a bump whatever the new layout looks like.
#[test]
fn a_replacement_reads_every_earlier_handoff_layout() {
    for &(layout, blob) in FIXTURES {
        let mut want = fixture_terminal();
        let mut got = Vt::new(FIXTURE_COLS, FIXTURE_ROWS);
        got.absorb_handoff(blob)
            .unwrap_or_else(|r| panic!("layout {layout}'s fixture was refused: {r:?}"));
        for depth in 0..=want.scrollback_len() {
            for t in [&mut want, &mut got] {
                t.scroll_down(u32::MAX);
                t.scroll_up(depth);
            }
            for y in 0..want.height() {
                for x in 0..want.width() {
                    assert_eq!(
                        got.pixel(x, y),
                        want.pixel(x, y),
                        "layout {layout}, history depth {depth}: pixel ({x}, {y})"
                    );
                }
            }
        }
        for t in [&mut want, &mut got] {
            t.feed(b"9;8;7mX\r\nafter");
        }
        assert_eq!(
            got.cell(0, 4),
            want.cell(0, 4),
            "layout {layout}: the cut sequence"
        );
        assert_eq!(got.cursor(), want.cursor(), "layout {layout}: the cursor");
    }
}

/// **The newest fixture is what this build writes**, so the layout cannot change without the
/// number changing, and the number cannot change without a fixture for it.
#[test]
fn the_newest_fixture_is_this_builds_layout_byte_for_byte() {
    let &(layout, blob) = FIXTURES.last().expect("no fixture");
    assert_eq!(
        layout, LAYOUT,
        "LAYOUT is {LAYOUT} but the newest fixture is layout {layout}: a bump adds its fixture \
         (run the ignored test write_the_fixture_for_this_layout) and keeps every older one"
    );
    let t = fixture_terminal();
    let mut out = vec![0u8; t.handoff_len()];
    t.write_handoff(&mut out)
        .expect("the fixture terminal's blob does not fit");
    assert!(
        out == blob,
        "this build writes layout {LAYOUT} differently from its fixture: a layout that changed \
         is a new LAYOUT, with the old reader kept"
    );
}

/// Writes `layout<N>.blob` beside this file for this build's [`LAYOUT`]. Run once at a bump, then
/// add the file to [`FIXTURES`]; never rerun it for an existing layout.
#[test]
#[ignore = "writes a fixture into the tree; run by hand at a LAYOUT bump"]
fn write_the_fixture_for_this_layout() {
    let t = fixture_terminal();
    let mut out = vec![0u8; t.handoff_len()];
    t.write_handoff(&mut out).expect("fits");
    let path = std::format!(
        "{}/src/handoff/layout{LAYOUT}.blob",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::write(&path, &out).expect("could not write the fixture");
}

/// **Every parser state, and every bound at its edge, survives a swap.** Each cut leaves the
/// terminal somewhere the fixture does not: at an escape, inside a title string and at its
/// terminator, inside a parameter list long enough to be ignored and one exactly full, three bytes
/// short of a four-byte character, with the cursor hidden and no wrap pending, and scrolled all
/// the way back. The rest of the sequence is fed to both, and what follows must match.
#[test]
fn every_parser_state_and_every_edge_carries_across() {
    let full = std::format!("\x1b[31{}", ";0".repeat(15));
    // Seventeen 31s: one more than fits, so the whole list is ignored, and red if it were not.
    let over = std::format!("\x1b[31{}", ";31".repeat(16));
    let cases: [(&[u8], &[u8]); 8] = [
        (b"\x1b", b"[32mG"),
        (b"\x1b]title", b"\x07T"),
        (b"\x1b]title\x1b", b"\\T"),
        (full.as_bytes(), b"mR"),
        (over.as_bytes(), b"mN"),
        (b"\xf0", b"\x9f\x98\x80U"),
        (b"", b"H"),
        (b"", b"Z"),
    ];
    for (k, (cut, rest)) in cases.iter().enumerate() {
        let mut old = Vt::new(12, 4);
        for i in 0..30 {
            old.feed(std::format!("{i}\r\n").as_bytes());
        }
        old.feed(cut);
        if k == 6 {
            old.set_cursor_visible(false); // with no wrap pending: both flags clear
        }
        if k == 7 {
            old.scroll_up(u32::MAX); // view_offset == sb_len, the bound's edge
        }
        let blob = blob_of(&old);
        let mut new = Vt::new(12, 4);
        new.absorb_handoff(&blob)
            .unwrap_or_else(|r| panic!("case {k}: refused {r:?}"));
        if k == 7 {
            assert_eq!(new.view_offset(), new.scrollback_len(), "case {k}");
        }
        for t in [&mut old, &mut new] {
            t.feed(rest);
        }
        assert_eq!(blob_of(&new), blob_of(&old), "case {k}");
        for y in 0..old.height() {
            for x in 0..old.width() {
                assert_eq!(
                    new.pixel(x, y),
                    old.pixel(x, y),
                    "case {k}: pixel ({x}, {y})"
                );
            }
        }
    }
}

/// **Each field out of range is refused**, one at a time, from a blob that is otherwise good. The
/// header words are counted from the blob's start: column 4, row 5, history 6, view 7, flags 8,
/// parameter count 10, UTF-8 bytes owed 11, rendition flags 16 (an unknown bit, and layout 1's
/// underline bit, which layout 2 does not use), underline color 17, underline style 18, the
/// sub-parameter mask 19, then the first cell's foreground.
#[test]
fn each_header_field_out_of_range_is_refused() {
    let mut t = Vt::new(12, 4);
    for i in 0..10 {
        t.feed(std::format!("{i}\r\n").as_bytes());
    }
    let good = blob_of(&t);
    let depth = t.scrollback_depth();
    let sb_len = t.scrollback_len();
    let first_cell = 4 * HEADER_WORDS;
    for (word, value) in [
        (4, 12),
        (5, 4),
        (6, depth + 1),
        (7, sb_len + 1),
        (8, 1 << 3),
        (10, MAX_PARAMS as u32 + 1),
        (11, 4),
        (16, 1 << 7),
        (16, 1 << 2),
        (17, 3 << 24),
        (18, 6),
        (19, 1 << 16),
    ] {
        let mut bad = good.clone();
        bad[4 * word..4 * word + 4].copy_from_slice(&value.to_le_bytes());
        assert_eq!(
            t.clone_absorb(&bad),
            Err(Refusal::Malformed),
            "word {word} = {value}"
        );
    }
    // An indexed color past 255 is not one.
    let mut bad = good.clone();
    bad[first_cell + 4..first_cell + 8].copy_from_slice(&0x100u32.to_le_bytes());
    assert_eq!(t.clone_absorb(&bad), Err(Refusal::Malformed));
}

impl Vt {
    /// Absorb into a fresh terminal of this one's shape.
    fn clone_absorb(&self, blob: &[u8]) -> Result<(), Refusal> {
        Vt::new(self.cols(), self.rows()).absorb_handoff(blob)
    }
}

/// **Layout 2's additions carry across a swap**: italic, a curly underline in its own color, and a
/// colon sequence cut after its `4:`, so the sub-parameter mask is the only thing that says the
/// `3` still to come is a style and not italic.
#[test]
fn the_faces_the_underline_styles_and_a_cut_colon_carry_across() {
    let mut old = Vt::new(12, 3);
    old.feed(b"\x1b[1;3;58;2;200;10;10;4:3mab\x1b[4:5mz\x1b[0m\x1b[4:");
    let mut new = Vt::new(12, 3);
    new.absorb_handoff(&blob_of(&old))
        .expect("refused its own blob");
    for t in [&mut old, &mut new] {
        t.feed(b"3mc");
    }
    assert_eq!(new.cell(0, 0), old.cell(0, 0));
    assert_eq!(
        new.cell(2, 0),
        old.cell(2, 0),
        "the dashed underline, the highest style"
    );
    assert_eq!(new.cell(3, 0), old.cell(3, 0));
    assert!(
        !new.cell(3, 0).attr.is_italic(),
        "the cut `4:` lost its colon"
    );
    assert_eq!(blob_of(&new), blob_of(&old));
}
