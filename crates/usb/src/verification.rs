use super::Refusal;
use super::boot_keyboard::{BootReport, changes};
use super::descriptor::{
    CLASS_HID, CONFIGURATION, ConfigurationHeader, DeviceDescriptor, ENDPOINT, INTERFACE,
    PROTOCOL_KEYBOARD, SUBCLASS_BOOT, find_boot_keyboard,
};

/// **The configuration walk never panics, and anything it accepts is a real interrupt IN
/// endpoint**, for every configuration of up to 25 bytes the solver can choose. This is the
/// one function in the driver that walks a device-supplied linked structure.
///
/// **25 and not 24 is the whole point of the bound.** The shortest configuration that can name
/// a keyboard endpoint is a 9-byte header, a 9-byte interface and a 7-byte endpoint, which is
/// 25 bytes. At 24 the walk could never reach an endpoint descriptor, so the three assertions
/// below were never evaluated and the harness proved only totality. Found by trying to
/// falsify it: a walk that accepted an OUT endpoint stayed green.
/// Falsification: replayable `crates/usb/falsifications/verification.the_configuration_walk_is_total_and_accepts_only_an_interrupt_in.patch`
#[kani::proof]
#[kani::unwind(27)]
fn the_configuration_walk_is_total_and_accepts_only_an_interrupt_in() {
    let bytes: [u8; 25] = kani::any();
    let len: usize = kani::any();
    kani::assume(len <= bytes.len());
    if let Ok(k) = find_boot_keyboard(&bytes[..len]) {
        assert!(k.endpoint & 0x80 != 0, "an IN endpoint");
        assert!(k.endpoint & 0x0f != 0, "never endpoint zero");
        assert!(
            k.maximum_packet_bytes >= 8,
            "a whole boot report fits one packet"
        );
    }
    // Not vacuous: some 25-byte configuration is accepted, so the assertions above are
    // evaluated on a real keyboard and not only on the refusals.
    kani::cover!(find_boot_keyboard(&bytes[..len]).is_ok());
}

/// **The device descriptor and configuration header parsers are total.**
/// Falsification: replayable `crates/usb/falsifications/verification.the_header_parsers_are_total.patch`
#[kani::proof]
fn the_header_parsers_are_total() {
    let bytes: [u8; 20] = kani::any();
    let len: usize = kani::any();
    kani::assume(len <= bytes.len());
    let _ = DeviceDescriptor::parse(&bytes[..len]);
    let _ = ConfigurationHeader::parse(&bytes[..len]);
}

/// **Two reports a keystroke apart produce at most twenty events**: eight modifier changes and
/// six releases and six presses at the very most, so a driver's fixed buffer cannot overflow.
/// Falsification: replayable `crates/usb/falsifications/verification.two_reports_produce_a_bounded_number_of_events.patch`
#[kani::proof]
#[kani::unwind(10)]
fn two_reports_produce_a_bounded_number_of_events() {
    let a: [u8; 8] = kani::any();
    let b: [u8; 8] = kani::any();
    if let (Some(a), Some(b)) = (BootReport::parse(&a), BootReport::parse(&b)) {
        let mut n = 0usize;
        changes(&a, &b, &mut |_, _| n += 1);
        assert!(n <= 20);
    }
}

/// **Across two interfaces, the walk picks only a boot keyboard's interrupt IN endpoint, and a
/// refusal names a boot keyboard interface.** The fully symbolic harness above cannot reach a
/// second interface: that needs 41 bytes, a header, two interfaces and an endpoint after each,
/// and a symbolic byte string that long does not finish. This one fixes the shape instead. Every
/// descriptor's length and type byte is fixed, and every other byte is symbolic: the interface
/// numbers, alternate settings and class triples, and each endpoint's address, attributes,
/// packet size and interval. The buffer is cut at a symbolic length, so every truncation point
/// is in the proof too. The paths this reaches and the first harness does not are the three the
/// walk keeps state for: a boot interface left with no endpoint and then closed by the next
/// interface, a candidate replaced by a later interface, and an endpoint refused as too small
/// followed by one that is accepted. The `kani::cover!`s below say each is reached.
/// Falsification: replayable `crates/usb/falsifications/verification.two_interfaces_are_walked_as_two.patch`
#[kani::proof]
#[kani::unwind(8)]
fn two_interfaces_are_walked_as_two() {
    const TOTAL: usize = 9 + 9 + 7 + 9 + 7;
    let mut bytes = [0u8; TOTAL];
    bytes[..9].copy_from_slice(&[9, CONFIGURATION, TOTAL as u8, 0, 2, 1, 0, 0xa0, 50]);
    let interfaces = [9, 25];
    for at in interfaces {
        let shape: [u8; 7] = kani::any();
        bytes[at] = 9;
        bytes[at + 1] = INTERFACE;
        bytes[at + 2..at + 9].copy_from_slice(&shape);
        let endpoint: [u8; 5] = kani::any();
        bytes[at + 9] = 7;
        bytes[at + 10] = ENDPOINT;
        bytes[at + 11..at + 16].copy_from_slice(&endpoint);
    }
    let len: usize = kani::any();
    kani::assume(len <= TOTAL);

    let is_boot = |at: usize| {
        bytes[at + 3] == 0
            && bytes[at + 5] == CLASS_HID
            && bytes[at + 6] == SUBCLASS_BOOT
            && bytes[at + 7] == PROTOCOL_KEYBOARD
    };
    let result = find_boot_keyboard(&bytes[..len]);
    match result {
        Ok(k) => {
            assert!(k.endpoint & 0x80 != 0, "an IN endpoint");
            assert!(k.endpoint & 0x0f != 0, "never endpoint zero");
            assert!(
                k.maximum_packet_bytes >= 8,
                "a whole boot report fits one packet"
            );
            // The endpoint is one that follows a boot interface, and the interface named is that
            // one: a candidate never outlives the interface it was read from.
            assert!(
                interfaces.iter().any(|&at| {
                    is_boot(at)
                        && k.interface == bytes[at + 2]
                        && k.endpoint == bytes[at + 11] & 0x8f
                        && len >= at + 16
                }),
                "an endpoint of the boot interface it names"
            );
        }
        Err(Refusal::NoInterruptIn { interface }) => {
            assert!(
                interfaces
                    .iter()
                    .any(|&at| is_boot(at) && interface == bytes[at + 2] && len >= at + 9),
                "the refusal names a boot interface the buffer holds"
            );
        }
        Err(Refusal::PacketTooSmall {
            maximum_packet_bytes,
        }) => {
            assert!(
                maximum_packet_bytes < 8,
                "only a small packet is refused for its size"
            );
        }
        Err(_) => {}
    }
    // The three paths the first harness cannot reach.
    kani::cover!(
        is_boot(9)
            && !is_boot(25)
            && len == TOTAL
            && result
                == Err(Refusal::NoInterruptIn {
                    interface: bytes[11]
                }),
        "a boot interface with no usable endpoint, closed by a second interface"
    );
    kani::cover!(
        is_boot(9)
            && is_boot(25)
            && matches!(result, Ok(k) if k.interface == bytes[27] && bytes[27] != bytes[11]),
        "a first candidate replaced by a second"
    );
    kani::cover!(
        is_boot(9)
            && is_boot(25)
            && bytes[20] & 0x80 != 0
            && bytes[20] & 0x0f != 0
            && bytes[21] & 0x3 == 0x3
            && u16::from_le_bytes([bytes[22], bytes[23]]) & 0x07ff < 8
            && result.is_ok(),
        "an endpoint refused as too small, then one accepted"
    );
}

/// **Two reports' events come in the promised order and say what changed**: first one event per
/// modifier bit that flipped, as usage 0xe0 plus the bit, each pressed when the bit is now set;
/// then releases, then presses, each a usage held in exactly one of the two reports, pressed when
/// it is the later one. Together with the bound of twenty this is the whole contract [`changes`]
/// documents. It does not prove the converse, that every usage which changed produces an event:
/// a usage reported twice in one report produces one event by design, and the unit tests pin it.
/// Falsification: replayable `crates/usb/falsifications/verification.two_reports_produce_their_events_in_order.patch`
#[kani::proof]
#[kani::unwind(10)]
fn two_reports_produce_their_events_in_order() {
    let a: [u8; 8] = kani::any();
    let b: [u8; 8] = kani::any();
    let (Some(before), Some(after)) = (BootReport::parse(&a), BootReport::parse(&b)) else {
        return;
    };
    // Each event is checked as it arrives, so no loop runs over the events afterwards: a loop of
    // twenty would need an unwind of twenty-one, where every loop here runs at most eight times.
    let flipped = before.modifiers ^ after.modifiers;
    let modifier_events = flipped.count_ones() as usize;
    let mut n = 0usize;
    let mut next_bit = 0usize;
    let mut pressing = false;
    changes(&before, &after, &mut |usage, pressed| {
        if n < modifier_events {
            while next_bit < 8 && flipped & (1 << next_bit) == 0 {
                next_bit += 1;
            }
            assert!(next_bit < 8, "one event per flipped modifier");
            assert!(
                (usage, pressed)
                    == (
                        0xe0 + next_bit as u8,
                        after.modifiers & (1 << next_bit) != 0
                    ),
                "modifiers first"
            );
            next_bit += 1;
        } else {
            assert!(pressed || !pressing, "every release before every press");
            pressing = pressed;
            let (from, other) = if pressed {
                (&after, &before)
            } else {
                (&before, &after)
            };
            assert!(
                from.keys
                    .iter()
                    .any(|&u| u != 0 && !other.keys.contains(&u) && u == usage),
                "a usage held in exactly one report"
            );
        }
        n += 1;
    });
    assert!(n >= modifier_events, "every flipped modifier has its event");
    assert!(n <= 20);
}
