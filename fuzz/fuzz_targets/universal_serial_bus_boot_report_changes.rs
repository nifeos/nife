//! Fuzz the boot report parser and the difference of two reports.
//!
//! The input is two reports back to back: `data[0..8]` is the report before and `data[8..16]` the
//! report after. The Kani harnesses prove the bound and the order for every pair; this holds the
//! same properties to the parser's own acceptance, so only pairs the parser admits are compared
//! (notes/fuzzing/targets.md).

#![no_main]

use libfuzzer_sys::fuzz_target;
use universal_serial_bus::boot_keyboard::{BootReport, REPORT_BYTES, changes, evdev_code};

fuzz_target!(|data: &[u8]| {
    if data.len() < REPORT_BYTES * 2 {
        return;
    }
    let (Some(before), Some(after)) = (
        BootReport::parse(&data[..REPORT_BYTES]),
        BootReport::parse(&data[REPORT_BYTES..]),
    ) else {
        return;
    };

    // Eight modifiers, six releases and six presses at the most, and every event names a usage the
    // keyboard page defines or reports no mapping.
    let mut events = 0usize;
    changes(&before, &after, &mut |usage, _pressed| {
        events += 1;
        let _ = evdev_code(usage);
    });
    assert!(events <= 20);

    // A report against itself changes nothing.
    let mut same = 0usize;
    changes(&after, &after, &mut |_, _| same += 1);
    assert_eq!(same, 0);
});
