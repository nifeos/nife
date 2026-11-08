//! Fuzz what a driver reads from the root ports and the device context: each `PORTSC` value, the
//! connections a debounce samples, and the slot context the controller writes back.
//!
//! The first 32 bytes are a slot context, eight little-endian dwords, read for the device address.
//! Each 40 bytes after that is one sample: a 32-bit value for one port's `PORTSC`, a 32-bit step in
//! milliseconds, and the 32-byte connection map the debounce reads at that time. The target holds
//! the speed to its four bits, the write value to the preserved bits plus exactly what was asked
//! (so no write acknowledges a change or disables the port that the caller did not ask for), and
//! the debounce to its clock: it answers only once the connections are stable or the timeout has
//! passed, and always answers by then.

#![no_main]

use extensible_host_controller_interface::context::slot_device_number;
use extensible_host_controller_interface::port::{
    CHANGES, CONNECTED, Connections, DEBOUNCE_STABLE_MILLISECONDS, DEBOUNCE_TIMEOUT_MILLISECONDS,
    ENABLED, POWER, PortDebounce, RESET, default_control_packet, speed_name, speed_of, write_value,
};
use libfuzzer_sys::fuzz_target;

fn dword(bytes: &[u8]) -> u32 {
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

/// Hold everything a driver derives from one `PORTSC` read.
fn check_port_status(port_status: u32) {
    let speed = speed_of(port_status);
    assert!(speed < 16, "the speed id is four bits");
    assert_eq!(u32::from(speed), (port_status >> 10) & 0xf);
    assert!(matches!(
        default_control_packet(speed),
        None | Some(8) | Some(64) | Some(512)
    ));
    assert!(!speed_name(speed).is_empty());

    // Writing nothing, or any one bit a driver sets, carries no change bit and no Port Enabled
    // bit unless that is the bit asked for, and keeps the preserved bits as read.
    let preserved = write_value(port_status, 0);
    assert_eq!(
        preserved & !port_status,
        0,
        "a write carries back only bits it read"
    );
    assert_eq!(preserved & (CHANGES | ENABLED | RESET), 0);
    assert_eq!(preserved & CONNECTED, port_status & CONNECTED);
    assert_eq!(preserved & POWER, port_status & POWER);
    for bits in [0, RESET, POWER, CHANGES, ENABLED, port_status] {
        let written = write_value(port_status, bits);
        assert_eq!(written, preserved | bits);
        assert_eq!(written & (CHANGES | ENABLED), bits & (CHANGES | ENABLED));
    }
}

fuzz_target!(|data: &[u8]| {
    let Some((context_bytes, samples)) = data.split_first_chunk::<32>() else {
        return;
    };
    let context: [u32; 8] = core::array::from_fn(|i| dword(&context_bytes[i * 4..i * 4 + 4]));
    assert_eq!(slot_device_number(&context), context[3] as u8);

    let start_milliseconds = 0u64;
    let mut now_milliseconds = start_milliseconds;
    let first: Connections = [0; 8];
    let mut debounce = PortDebounce::new(now_milliseconds, first);
    let mut last_change_milliseconds = now_milliseconds;
    let mut last = first;
    for sample in samples.as_chunks::<40>().0 {
        check_port_status(dword(&sample[0..4]));
        now_milliseconds = now_milliseconds.saturating_add(u64::from(dword(&sample[4..8])));
        let seen: Connections = core::array::from_fn(|i| dword(&sample[8 + i * 4..12 + i * 4]));
        if seen != last {
            last = seen;
            last_change_milliseconds = now_milliseconds;
        }
        let stable = now_milliseconds - last_change_milliseconds >= DEBOUNCE_STABLE_MILLISECONDS;
        let timed_out = now_milliseconds - start_milliseconds >= DEBOUNCE_TIMEOUT_MILLISECONDS;
        let answer = debounce.sample(now_milliseconds, seen);
        let expected = if stable {
            Some(true)
        } else if timed_out {
            Some(false)
        } else {
            None
        };
        assert_eq!(answer, expected);
    }
});
