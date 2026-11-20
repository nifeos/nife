//! Fuzz the descriptor parsers with whatever a device chose to send.
//!
//! The input is one byte string, handed to the device descriptor parser, the configuration header
//! parser and the walk to a boot keyboard. The crate's Kani harnesses prove the walk total up to 25
//! bytes in any shape and 41 bytes in one fixed shape; this reaches the longer, composite
//! configurations a real keyboard sends, with lengths and types that disagree with each other
//! (notes/fuzzing/targets.md).

#![no_main]

use libfuzzer_sys::fuzz_target;
use universal_serial_bus::descriptor::{
    ConfigurationHeader, DeviceDescriptor, Speed, control_maximum_packet_bytes, find_boot_keyboard,
};

fuzz_target!(|data: &[u8]| {
    if let Ok(device) = DeviceDescriptor::parse(data) {
        for speed in [Speed::Low, Speed::Full, Speed::High, Speed::Super] {
            let _ = control_maximum_packet_bytes(device.control_packet_size_code, speed);
        }
    }
    let _ = ConfigurationHeader::parse(data);

    // The property the walk exists for: an answer names an endpoint that the bytes held, and its
    // packet size is one the walk accepted (at least a boot report), never a panic or an overrun.
    if let Ok(keyboard) = find_boot_keyboard(data) {
        assert!(keyboard.endpoint & 0x80 != 0);
        assert!(keyboard.maximum_packet_bytes >= 8);
    }
});
