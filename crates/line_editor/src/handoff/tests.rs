use super::*;
use crate::test_support::{Screen, feed_all};
use crate::{MAXIMUM_LINE_BYTES, MAXIMUM_PROMPT_BYTES, MAXIMUM_RAW_QUEUE_BYTES, RawQueue, handoff};
extern crate std;
use std::vec::Vec;

/// **A handoff round trip carries the half-typed line, the cursor, the kill buffer and the
/// history**, so the replacement's next keystrokes land exactly where the old one's would have:
/// finishing the line returns all of it, and Up recalls a line typed before the swap.
#[test]
fn a_handed_off_discipline_carries_on_where_the_old_one_stopped() {
    let (mut old, mut s) = (LineEditor::new(), Screen::new());
    // A full ring of long lines, so the blob is the size a real terminal's is.
    // Distinct lines, because the ring skips an exact repeat of its newest entry.
    for i in 0..HISTORY_LINES - 1 {
        let long = [b'a' + i as u8; MAXIMUM_LINE_BYTES - 8];
        old.start_line(b"$ ", &mut s);
        feed_all(&mut old, &mut s, &long);
        feed_all(&mut old, &mut s, b"\r");
    }
    old.start_line(b"$ ", &mut s);
    feed_all(&mut old, &mut s, b"first\r");
    old.start_line(b"$ ", &mut s);
    feed_all(&mut old, &mut s, b"ec");

    let mut page = [0u8; 8192];
    let mut w = handoff::Writer::new(&mut page);
    old.save(&mut w);
    let n = w.finish().expect("a whole history fits");
    // Measured, correcting an estimate this lane recorded on 2026-09-26: a full ring of
    // near-maximal lines is under 2 KiB, so the whole terminal (with its four queued lines) fits
    // in one page. The estimate had counted buffers the blob does not carry.
    assert!(n < 2048, "the discipline's blob grew to {n} bytes");

    let mut new = LineEditor::new();
    let mut r = handoff::Reader::new(&page[..n]).unwrap();
    new.restore(&mut r).unwrap();
    new.resume_line(b"$ ");
    let mut s2 = Screen::new();
    feed_all(&mut new, &mut s2, b"ho\r");
    assert_eq!(
        new.line(),
        b"echo",
        "the half-typed line did not survive the swap"
    );
    new.start_line(b"$ ", &mut s2);
    feed_all(&mut new, &mut s2, b"\x1b[A\x1b[A\r");
    assert_eq!(
        new.line(),
        b"first",
        "history typed before the swap is not recallable after it"
    );
}

/// **A blob this build cannot read is refused whole, with the reason**, which is what lets a
/// supervisor not commit a swap to it.
#[test]
fn a_blob_in_another_layout_or_none_at_all_is_refused() {
    assert_eq!(
        handoff::Reader::new(&[0u8; 16]).err(),
        Some(handoff::Refusal::NoBlob)
    );
    let mut page = [0u8; 16];
    page[..4].copy_from_slice(&handoff::MAGIC.to_le_bytes());
    page[4..8].copy_from_slice(&2u32.to_le_bytes());
    assert_eq!(
        handoff::Reader::new(&page).err(),
        Some(handoff::Refusal::Layout(2))
    );
    let mut w = handoff::Writer::new(&mut page);
    w.field(&[1; 64]);
    assert_eq!(
        w.finish(),
        None,
        "a blob that does not fit must say so, not truncate"
    );
}

// ---- the survivors of the 2026-10-03 mutation census ----

/// A blob that exactly fills its buffer is a blob, and one byte short is refused: the header
/// alone is eight bytes.
#[test]
fn a_blob_may_fill_its_buffer_exactly_and_no_more() {
    let mut exact = [0u8; 8];
    assert_eq!(handoff::Writer::new(&mut exact).finish(), Some(8));
    let mut short = [0u8; 7];
    assert_eq!(handoff::Writer::new(&mut short).finish(), None);
    let mut page = [0u8; 12];
    let mut w = handoff::Writer::new(&mut page);
    w.field(&[]);
    assert_eq!(w.finish(), Some(12), "the length word fills the last four");
}

/// A field is refused when longer than the buffer it lands in and accepted when exactly as long.
#[test]
fn a_field_exactly_as_long_as_its_buffer_fits_and_one_longer_is_malformed() {
    let mut page = [0u8; 64];
    let mut w = handoff::Writer::new(&mut page);
    w.field(&[7; 5]);
    w.field(&[8; 5]);
    let n = w.finish().unwrap();
    let mut r = handoff::Reader::new(&page[..n]).unwrap();
    let mut five = [0u8; 5];
    assert_eq!(r.field(&mut five), Some(5));
    assert_eq!(five, [7; 5]);
    let mut four = [0u8; 4];
    assert_eq!(r.field(&mut four), None);
}

/// **The cursor a blob carries cannot lie past the line**: a cursor on the last byte or past
/// it is accepted only up to the line's length, and the history's write position survives a
/// swap, so Up recalls the newest line and not whichever sits at index 0.
#[test]
fn a_restored_cursor_stays_inside_its_line_and_history_keeps_its_place() {
    let (mut old, mut s) = (LineEditor::new(), Screen::new());
    for line in [&b"one\r"[..], b"two\r", b"three\r"] {
        old.start_line(b"$ ", &mut s);
        feed_all(&mut old, &mut s, line);
    }
    old.start_line(b"$ ", &mut s);
    feed_all(&mut old, &mut s, b"abc\x1b[D");
    let mut page = [0u8; 4096];
    let mut w = handoff::Writer::new(&mut page);
    old.save(&mut w);
    let n = w.finish().unwrap();

    let mut new = LineEditor::new();
    let mut r = handoff::Reader::new(&page[..n]).unwrap();
    assert_eq!(new.restore(&mut r), Some(()));
    assert_eq!(new.pending(), (b"abc".as_slice(), 2));
    feed_all(&mut new, &mut s, b"\x15\x1b[A"); // ^U, then Up
    assert_eq!(new.pending().0, b"three");

    // The same blob with its cursor word moved past the line: refused whole. The header is 8
    // bytes, the line field 4 + 3, so the cursor word sits at 15.
    let mut bad = page;
    bad[15..19].copy_from_slice(&4u32.to_le_bytes());
    let mut r = handoff::Reader::new(&bad[..n]).unwrap();
    assert_eq!(LineEditor::new().restore(&mut r), None);
    // And a cursor on the line's own end is not past it.
    bad[15..19].copy_from_slice(&3u32.to_le_bytes());
    let mut r = handoff::Reader::new(&bad[..n]).unwrap();
    assert_eq!(LineEditor::new().restore(&mut r), Some(()));
}

/// **Unread raw bytes cross a swap in order**, including when the ring has wrapped, and an
/// empty queue stays empty.
#[test]
fn the_raw_queue_crosses_a_swap_in_order_across_the_wrap() {
    let mut q = RawQueue::new();
    let first: Vec<u8> = (0..60).collect();
    q.push(&first);
    // Bounded, so a `pop_up_to_eight` that never shortens the queue fails the assertion instead of hanging.
    for _ in 0..MAXIMUM_RAW_QUEUE_BYTES {
        if q.length_bytes <= 4 {
            break;
        }
        q.pop_up_to_eight();
    }
    assert_eq!(q.length_bytes, 4);
    let more: Vec<u8> = (100..110).collect();
    q.push(&more); // the ring wraps: head is near the end
    assert!(q.head + q.length_bytes > MAXIMUM_RAW_QUEUE_BYTES);

    let mut page = [0u8; 256];
    let mut w = handoff::Writer::new(&mut page);
    q.save(&mut w);
    let n = w.finish().unwrap();
    let mut copy = RawQueue::new();
    let mut r = handoff::Reader::new(&page[..n]).unwrap();
    assert_eq!(copy.restore(&mut r), Some(()));

    let drain = |q: &mut RawQueue| {
        let mut out = Vec::new();
        for _ in 0..=MAXIMUM_RAW_QUEUE_BYTES {
            let Some((n, packed)) = q.pop_up_to_eight() else {
                break;
            };
            out.extend_from_slice(&packed.to_le_bytes()[..n]);
        }
        out
    };
    let mut expect: Vec<u8> = (56..60).collect();
    expect.extend(100..110);
    assert_eq!(drain(&mut q), expect);
    assert_eq!(drain(&mut copy), expect);

    let mut w = handoff::Writer::new(&mut page);
    RawQueue::new().save(&mut w);
    let n = w.finish().unwrap();
    let mut empty = RawQueue::new();
    empty.push(b"x");
    let mut r = handoff::Reader::new(&page[..n]).unwrap();
    assert_eq!(empty.restore(&mut r), Some(()));
    assert!(empty.is_empty());
}

/// **The blob's bytes are pinned**: a small editor state written by hand, field by field, so a
/// reordered or resized field fails here and not in a swap. Every word is a little-endian `u32`;
/// a field is its length word then its bytes; the history ring is eight fields in slot order.
#[test]
fn the_blob_layout_is_pinned_byte_for_byte() {
    let (mut d, mut s) = (LineEditor::new(), Screen::new());
    d.start_line(b"$", &mut s);
    feed_all(&mut d, &mut s, b"ab\x02"); // "ab", cursor between the two
    let mut page = [0u8; 256];
    let mut w = handoff::Writer::new(&mut page);
    d.save(&mut w);
    let n = w.finish().unwrap();

    let mut want: Vec<u8> = Vec::new();
    want.extend_from_slice(b"LEDT"); // MAGIC 0x5444_454c, little-endian
    want.extend_from_slice(&1u32.to_le_bytes()); // LAYOUT
    want.extend_from_slice(&2u32.to_le_bytes()); // the line: length
    want.extend_from_slice(b"ab"); //   and bytes
    want.extend_from_slice(&1u32.to_le_bytes()); // the cursor
    want.extend_from_slice(&0u32.to_le_bytes()); // the kill buffer: empty
    want.extend_from_slice(&1u32.to_le_bytes()); // the prompt: length
    want.extend_from_slice(b"$"); //   and bytes
    want.extend_from_slice(&0u32.to_le_bytes()); // history entries held
    want.extend_from_slice(&0u32.to_le_bytes()); // the next history slot
    for _ in 0..HISTORY_LINES {
        want.extend_from_slice(&0u32.to_le_bytes()); // eight empty history slots
    }
    assert_eq!(&page[..n], &want[..]);
}

/// **The blob has an upper bound**: with every buffer full the editor's part is the header, the
/// line, the cursor, the kill buffer, the prompt, two history words and eight full history
/// slots, 2688 bytes, and the raw queue's part is a length word and up to 64 bytes. Both fit the
/// one page the terminal's handoff frame has.
#[test]
fn a_full_blob_is_the_documented_size() {
    let mut d = LineEditor::new();
    d.length_bytes = MAXIMUM_LINE_BYTES;
    d.kill_length_bytes = MAXIMUM_LINE_BYTES;
    d.prompt_length_bytes = MAXIMUM_PROMPT_BYTES;
    for entry in &mut d.history {
        entry.1 = MAXIMUM_LINE_BYTES;
    }
    let mut page = [0u8; 4096];
    let mut w = handoff::Writer::new(&mut page);
    d.save(&mut w);
    assert_eq!(w.finish(), Some(2688));

    let mut q = RawQueue::new();
    q.push(&[7; MAXIMUM_RAW_QUEUE_BYTES]);
    let mut w = handoff::Writer::new(&mut page);
    q.save(&mut w);
    assert_eq!(w.finish(), Some(8 + 4 + MAXIMUM_RAW_QUEUE_BYTES));
}

/// A raw queue whose bytes wrap past the end of its ring is saved oldest first and restored whole.
#[test]
fn a_wrapped_raw_queue_round_trips() {
    let mut q = RawQueue::new();
    q.push(&[1; 60]);
    q.pop_up_to_eight();
    q.pop_up_to_eight();
    q.pop_up_to_eight();
    q.pop_up_to_eight();
    q.pop_up_to_eight();
    q.pop_up_to_eight();
    q.pop_up_to_eight(); // 56 popped: head is at 56
    q.push(&[9, 9, 9, 9, 9, 9, 9, 9, 8, 8]); // wraps the ring
    let mut page = [0u8; 128];
    let mut w = handoff::Writer::new(&mut page);
    q.save(&mut w);
    let n = w.finish().unwrap();
    let mut got = RawQueue::new();
    got.restore(&mut handoff::Reader::new(&page[..n]).unwrap())
        .unwrap();
    let a = drain(&mut q);
    let b = drain(&mut got);
    assert_eq!(a, b);
    assert_eq!(a.len(), 4 + 10);
}

/// Every byte a raw queue holds, oldest first. Bounded at one pop per byte the ring can hold, so a
/// `pop_up_to_eight` that never empties the queue fails the assertion instead of hanging.
fn drain(q: &mut RawQueue) -> Vec<u8> {
    let mut bytes = Vec::new();
    for _ in 0..=MAXIMUM_RAW_QUEUE_BYTES {
        let Some((n, packed)) = q.pop_up_to_eight() else {
            return bytes;
        };
        assert!(
            (1..=8).contains(&n),
            "a pop returns one to eight bytes, got {n}"
        );
        bytes.extend_from_slice(&packed.to_le_bytes()[..n]);
    }
    panic!("the queue never emptied");
}
