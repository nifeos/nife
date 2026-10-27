use super::*;
use crate::handoff;
use crate::test_support::{Screen, feed_all};
extern crate std;
use std::vec::Vec;

/// **A handoff round trip carries the half-typed line, the cursor, the kill buffer and the
/// history**, so the replacement's next keystrokes land exactly where the old one's would have:
/// finishing the line returns all of it, and Up recalls a line typed before the swap.
#[test]
fn a_handed_off_discipline_carries_on_where_the_old_one_stopped() {
    let (mut old, mut s) = (LineDisc::new(), Screen::new());
    // A full ring of long lines, so the blob is the size a real terminal's is.
    // Distinct lines, because the ring skips an exact repeat of its newest entry.
    for i in 0..HIST - 1 {
        let long = [b'a' + i as u8; LINE_MAX - 8];
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

    let mut new = LineDisc::new();
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
    let (mut old, mut s) = (LineDisc::new(), Screen::new());
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

    let mut new = LineDisc::new();
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
    assert_eq!(LineDisc::new().restore(&mut r), None);
    // And a cursor on the line's own end is not past it.
    bad[15..19].copy_from_slice(&3u32.to_le_bytes());
    let mut r = handoff::Reader::new(&bad[..n]).unwrap();
    assert_eq!(LineDisc::new().restore(&mut r), Some(()));
}

/// **Unread raw bytes cross a swap in order**, including when the ring has wrapped, and an
/// empty queue stays empty.
#[test]
fn the_raw_queue_crosses_a_swap_in_order_across_the_wrap() {
    let mut q = RawQueue::new();
    let first: Vec<u8> = (0..60).collect();
    q.push(&first);
    // Bounded, so a `pop8` that never shortens the queue fails the assertion instead of hanging.
    for _ in 0..RAW_QUEUE_MAXIMUM {
        if q.len <= 4 {
            break;
        }
        q.pop8();
    }
    assert_eq!(q.len, 4);
    let more: Vec<u8> = (100..110).collect();
    q.push(&more); // the ring wraps: head is near the end
    assert!(q.head + q.len > RAW_QUEUE_MAXIMUM);

    let mut page = [0u8; 256];
    let mut w = handoff::Writer::new(&mut page);
    q.save(&mut w);
    let n = w.finish().unwrap();
    let mut copy = RawQueue::new();
    let mut r = handoff::Reader::new(&page[..n]).unwrap();
    assert_eq!(copy.restore(&mut r), Some(()));

    let drain = |q: &mut RawQueue| {
        let mut out = Vec::new();
        for _ in 0..=RAW_QUEUE_MAXIMUM {
            let Some((n, packed)) = q.pop8() else { break };
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
