//! The wire's numbers, pinned: each is one-way once a peer outside the tree builds against it
//! (§320 (the JH7110 clock and reset wire)'s "What is one-way").
//!
//! Name: provisional.

use super::*;

#[test]
fn the_identity_is_interface_24_version_1() {
    assert_eq!(INTERFACE_ID, 24);
    assert_eq!(VERSION, 1);
}

#[test]
fn the_operation_numbers_are_the_ruled_ones() {
    assert_eq!(
        [VERSION_QUERY, BRING_UP, READ_CLOCK, READ_RESET, SET_RATE],
        [0, 1, 2, 3, 4]
    );
    assert_eq!(OPERATION_SHIFT, 56);
}

#[test]
fn the_operation_sits_in_bits_63_to_56_and_the_index_in_bits_7_to_0() {
    assert_eq!(request(READ_RESET, 0xab), 0x0300_0000_0000_00ab);
    for number in 0..=255u64 {
        for index_value in [0u8, 1, 0x7f, 0xff] {
            let w0 = request(number, index_value);
            assert_eq!(operation(w0), number);
            assert_eq!(index(w0), index_value);
        }
    }
    // Bits 55:8 carry nothing: the accessors still read the operation and index, and a server
    // refuses the word as malformed.
    assert_eq!(index(0x0200_ffff_ffff_ff05), 5);
    assert_eq!(operation(0x0200_ffff_ffff_ff05), READ_CLOCK);
    assert!(!is_well_formed(0x0200_ffff_ffff_ff05, 0));
}

#[test]
fn a_request_may_set_only_the_bits_its_operation_defines() {
    // Each of bits 55:8 alone makes a word malformed, whatever the operation.
    for bit in 8..56 {
        for op in [BRING_UP, READ_CLOCK, READ_RESET, SET_RATE] {
            assert!(!is_well_formed(request(op, 0) | 1 << bit, 0), "bit {bit}");
        }
    }
    // The index belongs to the two reads alone.
    for op in [VERSION_QUERY, BRING_UP, SET_RATE] {
        assert!(is_well_formed(request(op, 0), 0));
        assert!(!is_well_formed(request(op, 1), 0));
    }
    for op in [READ_CLOCK, READ_RESET] {
        assert!(is_well_formed(request(op, 0xff), 0));
        assert!(!is_well_formed(request(op, 3), 1));
    }
    // Word 1 belongs to set rate alone.
    assert!(is_well_formed(request(SET_RATE, 0), u64::MAX));
    assert!(is_well_formed(request(SET_RATE, 0), 0));
    assert!(!is_well_formed(request(BRING_UP, 0), 1));
    assert!(!is_well_formed(request(VERSION_QUERY, 0), 1 << 63));
    assert!(is_well_formed(request(BRING_UP, 0), 0));
}

#[test]
fn every_status_has_its_ruled_number_and_reads_back() {
    let numbered = [
        (Status::Done, 0),
        (Status::UnknownDevice, 1),
        (Status::UnknownOperation, 2),
        (Status::ClockNotRunning, 3),
        (Status::ResetHeld, 4),
        (Status::IndexOutOfRange, 5),
        (Status::NoRatedClock, 6),
        (Status::NoRateAtOrBelow, 7),
        (Status::MalformedRequest, 8),
    ];
    for (status, word) in numbered {
        assert_eq!(status.to_word(), word);
        assert_eq!(Status::from_word(word), Some(status));
    }
    assert_eq!(Status::ALL.len(), numbered.len());
}

#[test]
fn a_kernel_error_or_an_unknown_word_is_no_status() {
    assert_eq!(Status::from_word(9), None);
    for error in 1..=16i64 {
        assert_eq!(Status::from_word((-error) as u64), None);
    }
}

#[test]
fn the_summary_fields_sit_where_the_ruling_puts_them() {
    let one = |s: Summary| s.pack();
    assert_eq!(
        one(Summary {
            clocks_in_plan: 0xff,
            ..Summary::default()
        }),
        0xff
    );
    assert_eq!(
        one(Summary {
            clocks_running: 0xff,
            ..Summary::default()
        }),
        0xff << 8
    );
    assert_eq!(
        one(Summary {
            resets_in_plan: 0xff,
            ..Summary::default()
        }),
        0xff << 16
    );
    assert_eq!(
        one(Summary {
            resets_released: 0xff,
            ..Summary::default()
        }),
        0xff << 24
    );
    assert_eq!(
        one(Summary {
            already_up: true,
            ..Summary::default()
        }),
        1 << 32
    );
    assert_eq!(
        one(Summary {
            refused: 0xff,
            ..Summary::default()
        }),
        0xff << 40
    );
}

#[test]
fn a_summary_round_trips_and_a_stray_bit_is_refused() {
    let s = Summary {
        clocks_in_plan: 3,
        clocks_running: 2,
        resets_in_plan: 1,
        resets_released: 0,
        already_up: true,
        refused: 4,
    };
    assert_eq!(Summary::unpack(s.pack()), Some(s));
    for stray in (33..40).chain(48..64) {
        assert_eq!(Summary::unpack(s.pack() | 1 << stray), None, "bit {stray}");
    }
}

#[test]
fn a_gated_clock_outranks_a_held_reset() {
    let up = Summary {
        clocks_in_plan: 2,
        clocks_running: 2,
        resets_in_plan: 1,
        resets_released: 1,
        ..Summary::default()
    };
    assert_eq!(up.status(), Status::Done);
    assert_eq!(
        Summary {
            resets_released: 0,
            ..up
        }
        .status(),
        Status::ResetHeld
    );
    assert_eq!(
        Summary {
            clocks_running: 1,
            resets_released: 0,
            ..up
        }
        .status(),
        Status::ClockNotRunning
    );
    // A refused step is counted, and changes no status.
    assert_eq!(Summary { refused: 3, ..up }.status(), Status::Done);
}
