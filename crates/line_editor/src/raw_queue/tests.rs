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

    let (n, packed) = q.pop_up_to_eight().unwrap();
    assert_eq!(n, 8);
    let first8: [u8; 8] = packed.to_le_bytes();
    assert_eq!(&first8, b"hello, k");

    let (n, packed) = q.pop_up_to_eight().unwrap();
    assert_eq!(n, 4);
    assert_eq!(&packed.to_le_bytes()[..4], b"ilo!");
    assert!(q.is_empty());
    assert!(q.pop_up_to_eight().is_none());
}

/// A burst past [`MAXIMUM_RAW_QUEUE_BYTES`] is truncated: `push` reports how many actually landed, so
/// the caller (which owns the [`Sink`]) knows to ring the bell for the rest, exactly the
/// `LineEditor` overflow contract one level over.
#[test]
fn raw_queue_overflow_is_reported_not_grown() {
    let mut q = RawQueue::new();
    assert_eq!(
        q.push(&[b'a'; MAXIMUM_RAW_QUEUE_BYTES]),
        MAXIMUM_RAW_QUEUE_BYTES
    );
    assert_eq!(q.push(b"more"), 0, "a full queue accepts nothing further");
    // Draining still returns exactly what was pushed, byte for byte.
    let mut drained = 0;
    // Bounded, so a `pop_up_to_eight` that never returns `None` fails the assertion instead of hanging.
    for _ in 0..=MAXIMUM_RAW_QUEUE_BYTES {
        let Some((n, _)) = q.pop_up_to_eight() else {
            break;
        };
        drained += n;
    }
    assert_eq!(drained, MAXIMUM_RAW_QUEUE_BYTES);
    assert!(q.is_empty());
}

/// The queue wraps its ring correctly: push past the wrap point, pop some, push more, and the
/// bytes must still come out in the order they went in.
#[test]
fn raw_queue_wraps_the_ring_in_order() {
    let mut q = RawQueue::new();
    q.push(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
    let (n, packed) = q.pop_up_to_eight().unwrap(); // drain the first 8, head now at index 8
    assert_eq!(n, 8);
    assert_eq!(packed.to_le_bytes(), [1, 2, 3, 4, 5, 6, 7, 8]);
    q.push(&[11, 12, 13]); // wraps past MAXIMUM_RAW_QUEUE_BYTES
    let mut out = std::vec::Vec::new();
    // Bounded for the same reason as the drain above.
    for _ in 0..=MAXIMUM_RAW_QUEUE_BYTES {
        let Some((n, packed)) = q.pop_up_to_eight() else {
            break;
        };
        out.extend_from_slice(&packed.to_le_bytes()[..n]);
    }
    assert_eq!(out, [9, 10, 11, 12, 13]);
}

/// **`head` is a ring index after every pop**, not a count that merely reads the same through
/// `% MAXIMUM_RAW_QUEUE_BYTES` later. `save` and `restore` walk the ring from `head`, and a `head` that
/// grew by a whole ring each pop would still deliver the same bytes while no longer naming a
/// slot, so the bytes alone cannot catch it. Mutation testing found the gap.
#[test]
fn raw_queue_head_stays_inside_the_ring() {
    let mut q = RawQueue::new();
    for round in 0..20u8 {
        q.push(&[round; 5]);
        q.pop_up_to_eight().unwrap();
        assert!(
            q.head < MAXIMUM_RAW_QUEUE_BYTES,
            "round {round}: head {}",
            q.head
        );
    }
}
