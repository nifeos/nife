//! Fuzz the event ring as a controller fills it: every TRB it writes, decoded the way a driver
//! drains them.
//!
//! The first byte is the segment's entry count less one, so 1 to 256 entries: the sizes a driver
//! gives one segment, kept small so that a run wraps the ring often (the crate's model checker
//! harness covers every count). Each 16 bytes after
//! that is one TRB the controller writes, little-endian, into the next entry of the segment in
//! turn. After each write the consumer drains every entry whose cycle bit marks it as written this
//! lap, decoding each with `Event::decode`. The target holds the dequeue index inside the segment,
//! the `ERDP` address to the index, and every decoded field to the bits the specification gives it.

#![no_main]

use extensible_host_controller_interface::{Event, EventRing, TransferRequestBlock};
use libfuzzer_sys::fuzz_target;

/// Where the segment sits: a page-aligned address with room for any entry count above it.
const SEGMENT_BASE: u64 = 0x1_0000_0000;

/// The three event type codes `Event::decode` gives a variant of its own (xHCI 1.2 Table 6-91).
const TRANSFER_EVENT: u8 = 32;
const COMMAND_COMPLETION: u8 = 33;
const PORT_STATUS_CHANGE: u8 = 34;

fn dword(bytes: &[u8]) -> u32 {
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

/// Hold one decoded event to the TRB it came from.
fn check_event(transfer_request_block: TransferRequestBlock) {
    let words = transfer_request_block.0;
    let event = Event::decode(transfer_request_block);
    assert_eq!(event, Event::decode(transfer_request_block));
    let pointer = u64::from(words[0]) | u64::from(words[1]) << 32;
    let kind = transfer_request_block.kind();
    assert!(kind < 64, "the type code is six bits");
    match event {
        Event::Transfer {
            transfer_request_block: address,
            residual,
            code,
            slot,
            endpoint,
        } => {
            assert_eq!(kind, TRANSFER_EVENT);
            assert_eq!(address, pointer);
            assert!(residual < 1 << 24, "the residual length is 24 bits");
            assert_eq!(residual, words[2] & 0x00ff_ffff);
            assert_eq!(code, (words[2] >> 24) as u8);
            assert_eq!(slot, (words[3] >> 24) as u8);
            assert!(endpoint < 32, "the endpoint identifier is five bits");
        }
        Event::Command {
            transfer_request_block: address,
            code,
            slot,
        } => {
            assert_eq!(kind, COMMAND_COMPLETION);
            assert_eq!(address, pointer);
            assert_eq!(code, (words[2] >> 24) as u8);
            assert_eq!(slot, (words[3] >> 24) as u8);
        }
        Event::PortStatus { port } => {
            assert_eq!(kind, PORT_STATUS_CHANGE);
            assert_eq!(port, (words[0] >> 24) as u8);
        }
        Event::Other { kind: other } => {
            assert_eq!(other, kind);
            assert!(![TRANSFER_EVENT, COMMAND_COMPLETION, PORT_STATUS_CHANGE].contains(&other));
        }
    }
}

fuzz_target!(|data: &[u8]| {
    let Some((&head, writes)) = data.split_first() else {
        return;
    };
    let entries = u16::from(head) + 1;
    let mut ring = EventRing::new(SEGMENT_BASE, entries);
    let table_entry = ring.segment_table_entry();
    assert_eq!(
        u64::from(table_entry[0]) | u64::from(table_entry[1]) << 32,
        SEGMENT_BASE
    );
    assert_eq!(table_entry[2], u32::from(entries));

    // The segment as memory: zeroed, so nothing reads as written until the controller writes it.
    let mut segment = vec![TransferRequestBlock([0; 4]); usize::from(entries)];
    let mut write_index = 0usize;
    for chunk in writes.as_chunks::<16>().0 {
        segment[write_index] = TransferRequestBlock([
            dword(&chunk[0..4]),
            dword(&chunk[4..8]),
            dword(&chunk[8..12]),
            dword(&chunk[12..16]),
        ]);
        write_index = (write_index + 1) % segment.len();

        // A segment holds `entries` events, so a drain that consumes more than a lap has followed
        // a stale cycle bit back round the ring.
        let mut consumed = 0usize;
        loop {
            let index = ring.index();
            assert!(
                index < entries,
                "the dequeue index stays inside the segment"
            );
            assert_eq!(ring.dequeue_pointer(), SEGMENT_BASE + u64::from(index) * 16);
            let current = segment[usize::from(index)];
            if !ring.is_ready(&current) {
                break;
            }
            check_event(current);
            ring.advance();
            consumed += 1;
            assert!(
                consumed <= segment.len(),
                "one drain consumes at most one lap"
            );
        }
    }
});
