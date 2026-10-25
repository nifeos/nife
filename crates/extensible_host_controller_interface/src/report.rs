//! **The driver's one bring-up report**, which the kernel waits for before the boot goes on.
//!
//! Three words on a report endpoint, sent once: what the driver found, then two words of detail.
//! Both ends read them through this module, because they are two programs agreeing on a format,
//! and a format two binaries agree on belongs in one crate rather than in two "must match"
//! comments. The kernel prints a sentence from them; nothing else reads them.

/// Word 0: a boot keyboard is configured and its reports are queued. Word 1 is [`keyboard`]'s
/// packing; word 2 is the interface and endpoint.
pub const KEYBOARD: u64 = 0x5542_0001;
/// Word 0: the controller runs, and no boot keyboard was found at bring-up. Word 1 is [`ports`]'s
/// packing; word 2 is the last [`Step`] refused and its port.
/// The driver keeps watching for one being plugged in.
pub const NO_KEYBOARD: u64 = 0x5542_0002;
/// Word 0: the controller could not be brought up. Word 1 is the [`Step`]; word 2 its detail.
pub const FAILED: u64 = 0x5542_0003;

/// **Where bring-up or enumeration stopped**, as one byte a report can carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Step {
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

impl Step {
    /// The step for a report byte, if it names one.
    pub const fn from_code(code: u8) -> Option<Step> {
        Some(match code {
            1 => Step::HaltTimeout,
            2 => Step::ResetTimeout,
            3 => Step::NoFourKibibytePages,
            4 => Step::RegionTooSmall,
            5 => Step::RunTimeout,
            6 => Step::ControllerError,
            7 => Step::CommandTimeout,
            8 => Step::CommandFailed,
            9 => Step::PortResetTimeout,
            10 => Step::PortNotEnabled,
            11 => Step::UnknownSpeed,
            12 => Step::TransferTimeout,
            13 => Step::TransferFailed,
            14 => Step::BadDeviceDescriptor,
            15 => Step::BadControlPacket,
            16 => Step::NotABootKeyboard,
            17 => Step::BadHandoff,
            _ => return None,
        })
    }

    /// What a bench transcript prints for it.
    pub const fn describe(self) -> &'static str {
        match self {
            Step::HaltTimeout => "the controller did not halt",
            Step::ResetTimeout => "the controller did not finish its reset",
            Step::NoFourKibibytePages => "the controller does not take 4 KiB pages",
            Step::RegionTooSmall => "the controller wants more scratchpad pages than it was given",
            Step::RunTimeout => "the controller did not start running",
            Step::ControllerError => "the controller reported an internal error",
            Step::CommandTimeout => "a command was never completed",
            Step::CommandFailed => "a command failed",
            Step::PortResetTimeout => "a port reset never finished",
            Step::PortNotEnabled => "a port reset did not enable the port",
            Step::UnknownSpeed => "a device reported a speed with no packet size",
            Step::TransferTimeout => "a control transfer was never completed",
            Step::TransferFailed => "a control transfer failed",
            Step::BadDeviceDescriptor => "a device sent something that is not a device descriptor",
            Step::BadControlPacket => "a device declared an illegal control packet size",
            Step::NotABootKeyboard => "the device is not a keyboard that speaks the boot protocol",
            Step::BadHandoff => "the driver was handed words no kernel sends",
        }
    }
}

/// [`KEYBOARD`]'s word 1: the port, its speed id, and the device's vendor and product ids.
pub const fn keyboard(port: u8, speed: u8, vendor: u16, product: u16) -> u64 {
    port as u64 | (speed as u64) << 8 | (vendor as u64) << 16 | (product as u64) << 32
}

/// [`keyboard`] back: `(port, speed, vendor, product)`.
pub const fn keyboard_of(w: u64) -> (u8, u8, u16, u16) {
    (w as u8, (w >> 8) as u8, (w >> 16) as u16, (w >> 32) as u16)
}

/// [`NO_KEYBOARD`]'s word 1: how many root ports there are, how many had a device at the scan,
/// and whether every port's connection had held still before it (`false`: the scan ran at the
/// debounce timeout, so its count is a moment, not a settled fact).
///
/// Name: provisional, minted 2026-10-22 (UTC) by maintainer/xenon-usb-no-ports.
pub const fn ports(ports: u8, connected: u8, settled: bool) -> u64 {
    ports as u64 | (connected as u64) << 8 | (settled as u64) << 16
}

/// [`ports`] back: `(ports, connected, settled)`.
///
/// Name: provisional, minted 2026-10-22 (UTC) by maintainer/xenon-usb-no-ports.
pub const fn ports_of(w: u64) -> (u8, u8, bool) {
    (w as u8, (w >> 8) as u8, (w >> 16) & 1 != 0)
}

/// A step and its detail, packed for word 2 of [`NO_KEYBOARD`] or word 1/2 of [`FAILED`]: the
/// step in the low byte, the port in the next, the detail above.
pub const fn refusal(step: Step, port: u8, detail: u32) -> u64 {
    step as u64 | (port as u64) << 8 | (detail as u64) << 16
}

/// [`refusal`] back: `(step, port, detail)`, the step `None` for a byte that names none
/// (including 0, "nothing was refused").
pub const fn refusal_of(w: u64) -> (Option<Step>, u8, u32) {
    (Step::from_code(w as u8), (w >> 8) as u8, (w >> 16) as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_step_round_trips_through_its_byte() {
        for code in 0..=255u8 {
            if let Some(step) = Step::from_code(code) {
                assert_eq!(step as u8, code);
                assert!(!step.describe().is_empty());
            }
        }
        assert_eq!(Step::from_code(0), None);
        // Every code a driver sends names its step, and no two steps read the same.
        for code in 1..=17u8 {
            let step = Step::from_code(code).unwrap();
            for other in code + 1..=17 {
                assert_ne!(step.describe(), Step::from_code(other).unwrap().describe());
            }
        }
        assert_eq!(Step::from_code(18), None);
        assert_eq!(keyboard_of(keyboard(5, 1, 0x0627, 1)), (5, 1, 0x0627, 1));
        assert_eq!(ports_of(ports(26, 3, true)), (26, 3, true));
        assert_eq!(ports_of(ports(255, 255, false)), (255, 255, false));
        assert_eq!(
            refusal_of(refusal(Step::NotABootKeyboard, 3, 7)),
            (Some(Step::NotABootKeyboard), 3, 7)
        );
    }
}
