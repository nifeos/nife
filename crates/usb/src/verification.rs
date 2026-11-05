use super::boot_keyboard::{BootReport, changes};
use super::descriptor::{ConfigurationHeader, DeviceDescriptor, find_boot_keyboard};

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
        assert!(k.max_packet >= 8, "a whole boot report fits one packet");
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
