//! **The USB keyboard driver's one bring-up report**, which the kernel waits for before the boot
//! goes on.
//!
//! Three words on a report endpoint, sent once: what the driver found, then two words of detail.
//! Both ends read them through this crate, because they are two programs agreeing on a format,
//! and a format two binaries agree on belongs in one crate rather than in two "must match"
//! comments. The kernel prints a sentence from them; nothing else reads them.
//!
//! Name: provisional, minted 2026-10-26 (UTC) by lane xhci-names-ratified, as the 2026-10-25
//! names ruling suggested when it moved this module out of `extensible_host_controller_interface`.

#![no_std]

/// Word 0: a boot keyboard is configured and its reports are queued. Word 1 is
/// [`keyboard_report`]'s
/// packing; word 2 is the interface and endpoint.
///
/// Name: ratified 2026-10-25 (#2720). Refused `KEYBOARD` (the word reports that a keyboard was
/// found, not a keyboard).
pub const KEYBOARD_FOUND: u64 = 0x5542_0001;
/// Word 0: the controller runs, and no boot keyboard was found at bring-up. Word 1 is
/// [`ports_report`]'s
/// packing; word 2 is the last [`BringUpStep`] refused and its port.
/// The driver keeps watching for one being plugged in.
///
/// Name: ratified 2026-10-25 (#2720). Refused `NO_KEYBOARD` (the same reason as `KEYBOARD`).
pub const NO_KEYBOARD_FOUND: u64 = 0x5542_0002;
/// Word 0: the controller could not be brought up. Word 1 is the [`BringUpStep`]; word 2 its
/// detail.
///
/// Name: ratified 2026-10-25 (#2720). Refused `FAILED` (it did not say what failed).
pub const BRING_UP_FAILED: u64 = 0x5542_0003;

/// **Where bring-up or enumeration stopped**, as one byte a report can carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
///
/// Name: ratified 2026-10-25 (#2720). Refused `Step` (it did not say a step of what).
pub enum BringUpStep {
    /// The controller did not halt when asked.
    HaltTimeout = 1,
    /// The controller did not finish its reset.
    ResetTimeout = 2,
    /// `PAGESIZE` does not offer 4 KiB pages.
    NoFourKibibytePages = 3,
    /// The controller wants more scratchpad pages than the region the kernel gave holds.
    RegionTooSmall = 4,
    /// The controller did not start running.
    RunTimeout = 5,
    /// `USBSTS` reported a host system or controller error.
    ControllerError = 6,
    /// A command got no completion.
    CommandTimeout = 7,
    /// A command completed with an error code (the detail).
    CommandFailed = 8,
    /// A port did not finish its reset.
    PortResetTimeout = 9,
    /// A port finished its reset and was not enabled.
    PortNotEnabled = 10,
    /// The port reported a speed this driver has no packet size for.
    UnknownSpeed = 11,
    /// A control transfer got no completion.
    TransferTimeout = 12,
    /// A control transfer failed with a completion code (the detail).
    TransferFailed = 13,
    /// The device descriptor was not one.
    BadDeviceDescriptor = 14,
    /// `bMaxPacketSize0` is not a size the specification allows at this speed.
    BadControlPacket = 15,
    /// The configuration had no boot keyboard; the detail is the `usb::Refusal` code.
    NotABootKeyboard = 16,
    /// The handoff words were not ones a kernel sends.
    BadHandoff = 17,
}

impl BringUpStep {
    /// The step for a report byte, if it names one.
    ///
    /// Name: ratified 2026-10-25 (#2720). It kept its name.
    pub const fn from_code(code: u8) -> Option<BringUpStep> {
        Some(match code {
            1 => BringUpStep::HaltTimeout,
            2 => BringUpStep::ResetTimeout,
            3 => BringUpStep::NoFourKibibytePages,
            4 => BringUpStep::RegionTooSmall,
            5 => BringUpStep::RunTimeout,
            6 => BringUpStep::ControllerError,
            7 => BringUpStep::CommandTimeout,
            8 => BringUpStep::CommandFailed,
            9 => BringUpStep::PortResetTimeout,
            10 => BringUpStep::PortNotEnabled,
            11 => BringUpStep::UnknownSpeed,
            12 => BringUpStep::TransferTimeout,
            13 => BringUpStep::TransferFailed,
            14 => BringUpStep::BadDeviceDescriptor,
            15 => BringUpStep::BadControlPacket,
            16 => BringUpStep::NotABootKeyboard,
            17 => BringUpStep::BadHandoff,
            _ => return None,
        })
    }

    /// What a bench transcript prints for it.
    ///
    /// Name: ratified 2026-10-25 (#2720). Refused `describe` (it returns the sentence a transcript
    /// prints).
    pub const fn sentence(self) -> &'static str {
        match self {
            BringUpStep::HaltTimeout => "the controller did not halt",
            BringUpStep::ResetTimeout => "the controller did not finish its reset",
            BringUpStep::NoFourKibibytePages => "the controller does not take 4 KiB pages",
            BringUpStep::RegionTooSmall => {
                "the controller wants more scratchpad pages than it was given"
            }
            BringUpStep::RunTimeout => "the controller did not start running",
            BringUpStep::ControllerError => "the controller reported an internal error",
            BringUpStep::CommandTimeout => "a command was never completed",
            BringUpStep::CommandFailed => "a command failed",
            BringUpStep::PortResetTimeout => "a port reset never finished",
            BringUpStep::PortNotEnabled => "a port reset did not enable the port",
            BringUpStep::UnknownSpeed => "a device reported a speed with no packet size",
            BringUpStep::TransferTimeout => "a control transfer was never completed",
            BringUpStep::TransferFailed => "a control transfer failed",
            BringUpStep::BadDeviceDescriptor => {
                "a device sent something that is not a device descriptor"
            }
            BringUpStep::BadControlPacket => "a device declared an illegal control packet size",
            BringUpStep::NotABootKeyboard => {
                "the device is not a keyboard that speaks the boot protocol"
            }
            BringUpStep::BadHandoff => "the driver was handed words no kernel sends",
        }
    }
}

/// [`KEYBOARD_FOUND`]'s word 1: the port, its speed id, and the device's vendor and product ids.
///
/// Name: ratified 2026-10-25 (#2720). Refused `keyboard` (it packs a report word, not a keyboard).
pub const fn keyboard_report(port: u8, speed: u8, vendor: u16, product: u16) -> u64 {
    port as u64 | (speed as u64) << 8 | (vendor as u64) << 16 | (product as u64) << 32
}

/// [`keyboard_report`] back: `(port, speed, vendor, product)`.
///
/// Name: ratified 2026-10-25 (#2720). Refused `keyboard_of` (the same reason as `keyboard`).
pub const fn keyboard_report_of(w: u64) -> (u8, u8, u16, u16) {
    (w as u8, (w >> 8) as u8, (w >> 16) as u16, (w >> 32) as u16)
}

/// [`NO_KEYBOARD_FOUND`]'s word 1: how many root ports there are, how many had a device at the
/// scan,
/// and whether every port's connection had held still before it (`false`: the scan ran at the
/// debounce timeout, so its count is a moment, not a settled fact).
///
/// Name: ratified 2026-10-25 (#2720). Refused `ports` (it packs a report word, not ports).
pub const fn ports_report(ports: u8, connected: u8, settled: bool) -> u64 {
    ports as u64 | (connected as u64) << 8 | (settled as u64) << 16
}

/// [`ports_report`] back: `(ports, connected, settled)`.
///
/// Name: ratified 2026-10-25 (#2720). Refused `ports_of` (the same reason as `ports`).
pub const fn ports_report_of(w: u64) -> (u8, u8, bool) {
    (w as u8, (w >> 8) as u8, (w >> 16) & 1 != 0)
}

/// A step and its detail, packed for word 2 of [`NO_KEYBOARD_FOUND`] or word 1/2 of
/// [`BRING_UP_FAILED`]: the
/// step in the low byte, the port in the next, the detail above.
///
/// Name: ratified 2026-10-25 (#2720). Refused `refusal` (it packs where bring-up failed, and
/// `refusal` is `usb`'s word for something else).
pub const fn failure_report(step: BringUpStep, port: u8, detail: u32) -> u64 {
    step as u64 | (port as u64) << 8 | (detail as u64) << 16
}

/// [`failure_report`] back: `(step, port, detail)`, the step `None` for a byte that names none
/// (including 0, "nothing was refused").
///
/// Name: ratified 2026-10-25 (#2720). Refused `refusal_of` (the same reason as `refusal`).
pub const fn failure_report_of(w: u64) -> (Option<BringUpStep>, u8, u32) {
    (
        BringUpStep::from_code(w as u8),
        (w >> 8) as u8,
        (w >> 16) as u32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_step_round_trips_through_its_byte() {
        for code in 0..=255u8 {
            if let Some(step) = BringUpStep::from_code(code) {
                assert_eq!(step as u8, code);
                assert!(!step.sentence().is_empty());
            }
        }
        assert_eq!(BringUpStep::from_code(0), None);
        // Every code a driver sends names its step, and no two steps read the same.
        for code in 1..=17u8 {
            let step = BringUpStep::from_code(code).unwrap();
            for other in code + 1..=17 {
                assert_ne!(
                    step.sentence(),
                    BringUpStep::from_code(other).unwrap().sentence()
                );
            }
        }
        assert_eq!(BringUpStep::from_code(18), None);
        assert_eq!(
            keyboard_report_of(keyboard_report(5, 1, 0x0627, 1)),
            (5, 1, 0x0627, 1)
        );
        assert_eq!(ports_report_of(ports_report(26, 3, true)), (26, 3, true));
        assert_eq!(
            ports_report_of(ports_report(255, 255, false)),
            (255, 255, false)
        );
        assert_eq!(
            failure_report_of(failure_report(BringUpStep::NotABootKeyboard, 3, 7)),
            (Some(BringUpStep::NotABootKeyboard), 3, 7)
        );
    }
}
