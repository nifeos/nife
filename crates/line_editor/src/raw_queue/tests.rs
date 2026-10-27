use super::*;
extern crate std;

/// [`RawQueue`] delivers bytes in order, packs up to 8 per pop, and drains to empty: the
/// primitive `OPERATION_READRAW` is built on, proven with no IPC involved.
#[test]
fn raw_queue_pops_in_order_up_to_eight() {
    let mut q = RawQueue::new();
    assert!(q.is_empty());
    assert_eq!(q.push(b"hello, kilo!"), 12); // 12 bytes, under the cap
    assert!(!q.is_empty());

    let (n, packed) = q.pop8().unwrap();
    assert_eq!(n, 8);
    let first8: [u8; 8] = packed.to_le_bytes();
    assert_eq!(&first8, b"hello, k");

    let (n, packed) = q.pop8().unwrap();
    assert_eq!(n, 4);
    assert_eq!(&packed.to_le_bytes()[..4], b"ilo!");
    assert!(q.is_empty());
    assert!(q.pop8().is_none());
}

/// A burst past [`RAW_QUEUE_MAXIMUM`] is truncated: `push` reports how many actually landed, so
/// the caller (which owns the [`Sink`]) knows to ring the bell for the rest, exactly the
/// `LineDisc` overflow contract one level over.
#[test]
fn raw_queue_overflow_is_reported_not_grown() {
    let mut q = RawQueue::new();
    assert_eq!(q.push(&[b'a'; RAW_QUEUE_MAXIMUM]), RAW_QUEUE_MAXIMUM);
    assert_eq!(q.push(b"more"), 0, "a full queue accepts nothing further");
    // Draining still returns exactly what was pushed, byte for byte.
    let mut drained = 0;
    // Bounded, so a `pop8` that never returns `None` fails the assertion instead of hanging.
    for _ in 0..=RAW_QUEUE_MAXIMUM {
        let Some((n, _)) = q.pop8() else { break };
        drained += n;
    }
    assert_eq!(drained, RAW_QUEUE_MAXIMUM);
    assert!(q.is_empty());
}

/// The queue wraps its ring correctly: push past the wrap point, pop some, push more, and the
/// bytes must still come out in the order they went in.
#[test]
fn raw_queue_wraps_the_ring_in_order() {
    let mut q = RawQueue::new();
    q.push(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
    let (n, packed) = q.pop8().unwrap(); // drain the first 8, head now at index 8
    assert_eq!(n, 8);
    assert_eq!(packed.to_le_bytes(), [1, 2, 3, 4, 5, 6, 7, 8]);
    q.push(&[11, 12, 13]); // wraps past RAW_QUEUE_MAXIMUM
    let mut out = std::vec::Vec::new();
    // Bounded for the same reason as the drain above.
    for _ in 0..=RAW_QUEUE_MAXIMUM {
        let Some((n, packed)) = q.pop8() else { break };
        out.extend_from_slice(&packed.to_le_bytes()[..n]);
    }
    assert_eq!(out, [9, 10, 11, 12, 13]);
}

/// **`head` is a ring index after every pop**, not a count that merely reads the same through
/// `% RAW_QUEUE_MAXIMUM` later. `save` and `restore` walk the ring from `head`, and a `head` that
/// grew by a whole ring each pop would still deliver the same bytes while no longer naming a
/// slot, so the bytes alone cannot catch it. Mutation survivor triage, milestone 326 (turn a
/// mutation score upward), batch 3.
#[test]
fn raw_queue_head_stays_inside_the_ring() {
    let mut q = RawQueue::new();
    for round in 0..20u8 {
        q.push(&[round; 5]);
        q.pop8().unwrap();
        assert!(q.head < RAW_QUEUE_MAXIMUM, "round {round}: head {}", q.head);
    }
}
