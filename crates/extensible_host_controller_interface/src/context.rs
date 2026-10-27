//! **Slot and endpoint contexts** (xHCI 1.2 section 6.2): what the driver tells the controller about a
//! device and its endpoints.
//!
//! A *device context* is an array of contexts the controller owns once a device is addressed: the
//! slot context, then one endpoint context per *device context index* (DCI; endpoint 0 is DCI 1,
//! endpoint `n` OUT is `2n`, IN is `2n + 1`). An *input context* is what the driver hands a
//! command: an input control context saying which contexts to add or drop, then a slot context
//! and endpoint contexts in the same order, one index along. A context is 32 or 64 bytes depending
//! on the controller (`HCCPARAMS1.CSZ`); only the first 32 carry fields.
//!
//! Name: ratified 2026-10-25 (#2720).

use crate::port;
use crate::port::speed;

/// Endpoint type: control, bidirectional (xHCI 1.2 Table 6-9).
///
/// Name: ratified 2026-10-25 (#2720).
pub const CONTROL: u8 = 4;
/// Endpoint type: interrupt IN.
///
/// Name: ratified 2026-10-25 (#2720).
pub const INTERRUPT_IN: u8 = 7;

/// **The device context index** of endpoint `address` (direction bit included). Endpoint 0 is 1
/// in either direction.
///
/// Name: ratified 2026-10-25 (#2720).
pub const fn endpoint_index(address: u8) -> u8 {
    let number = address & 0x0f;
    if number == 0 {
        1
    } else {
        number * 2 + (address >> 7)
    }
}

/// Byte offset of the input control context's successor `index` in an input context: the slot
/// context is index 1, DCI `n` is index `n + 1`.
///
/// Name: ratified 2026-10-25 (#2720).
pub const fn input_offset(index: u8, context_bytes: u64) -> u64 {
    index as u64 * context_bytes
}

/// Byte offset of a context in a device (output) context: the slot is 0, DCI `n` is `n`.
///
/// Name: ratified 2026-10-25 (#2720).
pub const fn device_offset(dci: u8, context_bytes: u64) -> u64 {
    dci as u64 * context_bytes
}

/// **The input control context**: `add` and `drop` are bitmaps of context indices (bit 0 the slot
/// context, bit `n` DCI `n`).
///
/// Name: ratified 2026-10-25 (#2720).
pub const fn input_control(add: u32, drop: u32) -> [u32; 8] {
    [drop, add, 0, 0, 0, 0, 0, 0]
}

/// **A slot context for a device on root port `port`** (numbered from 1) at `speed`, whose highest
/// context is DCI `entries`. Route string 0: the device is on a root port, not behind a hub.
///
/// Name: ratified 2026-10-25 (#2720).
pub const fn slot(speed: u8, port: u8, entries: u8) -> [u32; 8] {
    [
        (speed as u32 & 0xf) << 20 | (entries as u32 & 0x1f) << 27,
        (port as u32) << 16,
        0,
        0,
        0,
        0,
        0,
        0,
    ]
}

/// The slot context's device address, assigned by Address Device: dword 3's low byte of a slot
/// context the controller wrote.
///
/// Name: ratified 2026-10-25 (#2720).
pub const fn slot_address(context: &[u32; 8]) -> u8 {
    context[3] as u8
}

/// **An endpoint context.** `dequeue` is the ring's physical base with the consumer cycle in bit
/// 0 (`ProducerRing::dequeue_pointer`). `interval` is already the xHCI exponent
/// (`crate::interrupt_interval`). Three retries on error, which is the specification's suggested
/// value for everything but isochronous.
///
/// Name: ratified 2026-10-25 (#2720).
pub const fn endpoint(
    kind: u8,
    maximum_packet: u16,
    interval: u8,
    dequeue: u64,
    average_trb: u16,
) -> [u32; 8] {
    // For an interrupt endpoint, the most it moves per service interval: one packet, since a
    // keyboard has no burst. Zero for control.
    let esit = if kind == CONTROL {
        0
    } else {
        maximum_packet as u32
    };
    [
        (interval as u32) << 16,
        3 << 1 | (kind as u32) << 3 | (maximum_packet as u32) << 16,
        dequeue as u32,
        (dequeue >> 32) as u32,
        average_trb as u32 | esit << 16,
        0,
        0,
        0,
    ]
}

/// Endpoint 0's context with a new packet size, everything else as [`endpoint`] built it for
/// `dequeue`: what Evaluate Context takes once the device's descriptor has said its size.
///
/// Name: ratified 2026-10-25 (#2720).
pub const fn control_endpoint(maximum_packet: u16, dequeue: u64) -> [u32; 8] {
    endpoint(CONTROL, maximum_packet, 0, dequeue, 8)
}

/// Whether `speed` is one at which `bMaxPacketSize0` is an exponent.
///
/// Name: ratified 2026-10-25 (#2720).
pub const fn is_superspeed(s: u8) -> bool {
    s == speed::SUPER || s == speed::SUPER_PLUS
}

/// **The `bInterval` of an interrupt endpoint as the xHCI Interval field**, which is an exponent:
/// the period is `2^Interval` microframes of 125 µs (xHCI 1.2 section 6.2.3.6).
///
/// Full- and low-speed devices state the period in 1 ms frames (1 to 255), so the field is the
/// largest exponent whose period does not exceed it, clamped to the specification's 3 (1 ms) to
/// 10 (128 ms). High-speed and `SuperSpeed` devices already state an exponent, plus one (1 to 16),
/// so the field is that minus one.
///
/// Name: ratified 2026-10-25 (#2720).
pub const fn interrupt_interval(speed: u8, b_interval: u8) -> u8 {
    if speed == port::speed::FULL || speed == port::speed::LOW {
        let microframes = (if b_interval == 0 { 1 } else { b_interval }) as u32 * 8;
        let exponent = 31 - microframes.leading_zeros();
        let exponent = if exponent < 3 { 3 } else { exponent };
        (if exponent > 10 { 10 } else { exponent }) as u8
    } else {
        let b = if b_interval == 0 {
            1
        } else if b_interval > 16 {
            16
        } else {
            b_interval
        };
        b - 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_context_indices_interleave_out_and_in() {
        assert_eq!(endpoint_index(0x00), 1);
        assert_eq!(endpoint_index(0x80), 1);
        assert_eq!(endpoint_index(0x01), 2);
        assert_eq!(endpoint_index(0x81), 3);
        assert_eq!(endpoint_index(0x8f), 31);
    }

    #[test]
    fn a_slot_context_names_its_port_speed_and_reach() {
        let s = slot(speed::FULL, 5, 3);
        assert_eq!((s[0] >> 20) & 0xf, 1);
        assert_eq!(s[0] >> 27, 3);
        assert_eq!(s[1] >> 16, 5);
    }

    #[test]
    fn an_interrupt_endpoint_context_carries_type_size_interval_and_ring() {
        let e = endpoint(INTERRUPT_IN, 8, 6, 0x1234_5000 | 1, 8);
        assert_eq!((e[0] >> 16) & 0xff, 6);
        assert_eq!((e[1] >> 3) & 7, 7);
        assert_eq!((e[1] >> 1) & 3, 3);
        assert_eq!(e[1] >> 16, 8);
        assert_eq!(e[2], 0x1234_5001);
        assert_eq!(e[4], 8 | 8 << 16);
        // A ring above 4 GiB: dword 3 is the dequeue pointer's high half.
        assert_eq!(endpoint(INTERRUPT_IN, 8, 6, 0x5_0000_1000, 8)[3], 5);
    }

    /// **Drop flags first, add flags second** (xHCI 1.2 section 6.2.5.1): the controller reads
    /// dword 0 as the contexts to drop, so swapping them would ask Configure Endpoint to remove
    /// the endpoint the driver meant to add.
    #[test]
    fn the_input_control_context_puts_drop_before_add() {
        assert_eq!(input_control(0b11, 0b100), [0b100, 0b11, 0, 0, 0, 0, 0, 0]);
    }

    /// Endpoint 0 is a control endpoint with three retries, no service-interval payload, and an
    /// eight-byte average TRB (the setup packet), whatever packet size it is given.
    #[test]
    fn endpoint_zero_is_control_with_no_periodic_payload() {
        for packet in [8u16, 64, 512] {
            let e = control_endpoint(packet, 0x6000 | 1);
            assert_eq!((e[1] >> 3) & 7, u32::from(CONTROL));
            assert_eq!((e[1] >> 1) & 3, 3);
            assert_eq!(e[1] >> 16, u32::from(packet));
            assert_eq!(e[4], 8, "average TRB 8, max ESIT payload 0");
            assert_eq!(e[0], 0, "no interval");
        }
    }

    /// The address the controller assigned, read back from the slot context it wrote; and which
    /// speeds state `bMaxPacketSize0` as an exponent.
    #[test]
    fn the_assigned_address_and_the_superspeed_test() {
        assert_eq!(slot_address(&[0, 0, 0, 0x0800_0005, 0, 0, 0, 0]), 5);
        assert!(is_superspeed(speed::SUPER) && is_superspeed(speed::SUPER_PLUS));
        assert!(
            !is_superspeed(speed::HIGH)
                && !is_superspeed(speed::FULL)
                && !is_superspeed(speed::LOW)
        );
    }

    #[test]
    fn offsets_follow_the_context_size() {
        assert_eq!(input_offset(1, 32), 32);
        assert_eq!(input_offset(2, 64), 128);
        assert_eq!(device_offset(3, 32), 96);
    }
}
