//! **The port status and control register** (`PORTSC`, xHCI 1.2 section 5.4.8), and the trap in it.
//!
//! Half its bits are write-one-to-clear, and one of those is Port Enabled: writing back what was
//! read, with a single bit added, **disables the port** and acknowledges every pending change
//! nobody has looked at yet. So a write is always [`write_value`]: the bits that are safe to write
//! back, plus the one the caller means.

/// Current Connect Status: a device is attached.
pub const CONNECTED: u32 = 1 << 0;
/// Port Enabled. **Write one to clear, which disables the port.**
pub const ENABLED: u32 = 1 << 1;
/// Port Reset: write one to reset the port, reads one until the reset finishes.
pub const RESET: u32 = 1 << 4;
/// Port Power.
pub const POWER: u32 = 1 << 9;
/// Connect Status Change, write one to clear.
pub const CONNECT_CHANGE: u32 = 1 << 17;
/// Port Enabled/Disabled Change, write one to clear.
pub(crate) const ENABLE_CHANGE: u32 = 1 << 18;
/// Warm Port Reset Change, write one to clear.
pub(crate) const WARM_RESET_CHANGE: u32 = 1 << 19;
/// Over-current Change, write one to clear.
pub(crate) const OVER_CURRENT_CHANGE: u32 = 1 << 20;
/// Port Reset Change, write one to clear.
pub const RESET_CHANGE: u32 = 1 << 21;
/// Port Link State Change, write one to clear.
pub(crate) const LINK_CHANGE: u32 = 1 << 22;
/// Port Config Error Change, write one to clear.
pub(crate) const CONFIG_ERROR_CHANGE: u32 = 1 << 23;

/// Every change bit: what a driver writes back to acknowledge all of them.
pub const CHANGES: u32 = CONNECT_CHANGE
    | ENABLE_CHANGE
    | WARM_RESET_CHANGE
    | OVER_CURRENT_CHANGE
    | RESET_CHANGE
    | LINK_CHANGE
    | CONFIG_ERROR_CHANGE;

/// The bits a write may carry back unchanged: the read-only ones (ignored on write) and the
/// read-write ones that must keep their value (power, indicator, link state without its strobe,
/// wake enables). The same set Linux's `xhci_port_state_to_neutral` keeps.
pub(crate) const PRESERVE: u32 =
    1 | 1 << 3 | 0xf << 10 | 1 << 30 | 0xf << 5 | 1 << 9 | 0x3 << 14 | 0x7 << 25;

/// Port speed ids, the default ones every xHCI uses (xHCI 1.2 Table 7-13).
pub mod speed {
    /// Full speed, 12 Mb/s: most keyboards.
    pub const FULL: u8 = 1;
    /// Low speed, 1.5 Mb/s: cheap keyboards.
    pub(crate) const LOW: u8 = 2;
    /// High speed, 480 Mb/s.
    pub(crate) const HIGH: u8 = 3;
    /// `SuperSpeed`, 5 Gb/s.
    pub(crate) const SUPER: u8 = 4;
    /// `SuperSpeedPlus`, 10 Gb/s.
    pub(crate) const SUPER_PLUS: u8 = 5;
}

/// The attached device's speed id, from bits 13:10.
pub const fn speed_of(portsc: u32) -> u8 {
    ((portsc >> 10) & 0xf) as u8
}

/// **What to write to set `bits`** without disabling the port or acknowledging a change nobody
/// asked to: `portsc`'s preserved bits, plus `bits`.
pub const fn write_value(portsc: u32, bits: u32) -> u32 {
    (portsc & PRESERVE) | bits
}

/// **Endpoint 0's packet size before the device has said**, by speed (xHCI 1.2 section 4.3): 8 for low
/// and full speed (a full-speed device may want up to 64, which the driver learns from the first
/// eight bytes of its descriptor and fixes with Evaluate Context), 64 for high speed, 512 above.
/// `None` for a speed id this table does not know.
pub const fn default_control_packet(speed: u8) -> Option<u16> {
    match speed {
        speed::LOW | speed::FULL => Some(8),
        speed::HIGH => Some(64),
        speed::SUPER | speed::SUPER_PLUS => Some(512),
        _ => None,
    }
}

/// The name a bench transcript prints for a speed id.
pub const fn speed_name(speed: u8) -> &'static str {
    match speed {
        speed::LOW => "low speed",
        speed::FULL => "full speed",
        speed::HIGH => "high speed",
        speed::SUPER => "SuperSpeed",
        speed::SUPER_PLUS => "SuperSpeedPlus",
        _ => "an unknown speed",
    }
}

/// **When the root ports are believed after a controller reset.** The reset disconnects every
/// device, powered or not, and silicon reports each one again only after some milliseconds; QEMU
/// does it at once. A boot-time scan therefore waits until no port's `CCS` has changed for
/// [`DEBOUNCE_STABLE_MS`], read every [`DEBOUNCE_STEP_MS`], giving up at [`DEBOUNCE_TIMEOUT_MS`].
/// The clock starts when the ports are powered, so nothing is believed before
/// [`DEBOUNCE_STABLE_MS`] after power either.
///
/// The three numbers are Linux's (`drivers/usb/core/hub.c`), and none was measured on a machine.
/// Nothing here is the last word on what is plugged in: a device that shows up later reaches the
/// driver as a connect change.
///
/// Name: provisional, minted 2026-10-22 (UTC) by maintainer/xenon-usb-no-ports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Settle {
    start_ms: u64,
    changed_ms: u64,
    last: Connections,
}

/// Every root port's `CCS`, one bit per port number: 256 bits, since `HCSPARAMS1.MaxPorts` is a byte.
///
/// Name: provisional, minted 2026-10-22 (UTC) by maintainer/xenon-usb-no-ports.
pub type Connections = [u32; 8];

/// USB 2.0's `TATTDB` (section 7.1.7.3), a connection's debounce interval, and Linux's
/// `HUB_DEBOUNCE_STABLE`. It also covers USB 2.0's `TSIGATT` (a device attaches within 100 ms of
/// power) and is Linux's floor on a root hub's power-on-to-good time.
///
/// Name: provisional, minted 2026-10-22 (UTC) by maintainer/xenon-usb-no-ports.
pub const DEBOUNCE_STABLE_MS: u64 = 100;
/// Linux's `HUB_DEBOUNCE_STEP`.
///
/// Name: provisional, minted 2026-10-22 (UTC) by maintainer/xenon-usb-no-ports.
pub const DEBOUNCE_STEP_MS: u64 = 25;
/// Linux's `HUB_DEBOUNCE_TIMEOUT`.
///
/// Name: provisional, minted 2026-10-22 (UTC) by maintainer/xenon-usb-no-ports.
pub const DEBOUNCE_TIMEOUT_MS: u64 = 2000;

impl Settle {
    /// Start at `now_ms`, when the ports were powered, having read `first`.
    ///
    /// Name: provisional, minted 2026-10-22 (UTC) by maintainer/xenon-usb-no-ports.
    pub const fn new(now_ms: u64, first: Connections) -> Settle {
        Settle {
            start_ms: now_ms,
            changed_ms: now_ms,
            last: first,
        }
    }

    /// The ports read `seen` at `now_ms`: `None` to keep reading, `Some(true)` once nothing has
    /// changed for [`DEBOUNCE_STABLE_MS`], `Some(false)` at [`DEBOUNCE_TIMEOUT_MS`].
    ///
    /// Name: provisional, minted 2026-10-22 (UTC) by maintainer/xenon-usb-no-ports.
    pub fn observe(&mut self, now_ms: u64, seen: Connections) -> Option<bool> {
        if seen != self.last {
            self.last = seen;
            self.changed_ms = now_ms;
        }
        if now_ms.saturating_sub(self.changed_ms) >= DEBOUNCE_STABLE_MS {
            Some(true)
        } else if now_ms.saturating_sub(self.start_ms) >= DEBOUNCE_TIMEOUT_MS {
            Some(false)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The trap, closed**: a port that is enabled with a reset change pending, written to start
    /// another reset, keeps its enable bit OFF in the written value (writing it would disable the
    /// port) and does not acknowledge the change.
    #[test]
    fn a_write_never_carries_the_enable_bit_or_a_change() {
        let read = CONNECTED | ENABLED | POWER | RESET_CHANGE | 3 << 10;
        let w = write_value(read, RESET);
        assert_eq!(w & ENABLED, 0);
        assert_eq!(w & CHANGES, 0);
        assert_ne!(w & POWER, 0);
        assert_ne!(w & RESET, 0);
        assert_eq!(speed_of(read), speed::HIGH);
    }

    /// Each speed id names itself in the boot line, and an id no table knows says so rather than
    /// claiming one.
    #[test]
    fn every_speed_has_its_own_name() {
        let names = [
            speed::LOW,
            speed::FULL,
            speed::HIGH,
            speed::SUPER,
            speed::SUPER_PLUS,
            15,
        ]
        .map(speed_name);
        assert_eq!(names[1], "full speed");
        assert_eq!(names[5], "an unknown speed");
        for (i, a) in names.iter().enumerate() {
            assert!(names[i + 1..].iter().all(|b| a != b), "{a} is named twice");
        }
    }

    #[test]
    fn the_preserved_set_is_linuxs() {
        assert_eq!(PRESERVE, 0x4e00_ffe9);
    }

    /// The bit positions, pinned against xHCI 1.2 Table 5-27, so a shift written the wrong way
    /// round fails here rather than as a port that never enables on the bench.
    #[test]
    fn the_portsc_bits_are_where_the_specification_puts_them() {
        assert_eq!([CONNECTED, ENABLED, RESET, POWER], [1, 2, 0x10, 0x200]);
        assert_eq!(
            [
                CONNECT_CHANGE,
                ENABLE_CHANGE,
                WARM_RESET_CHANGE,
                OVER_CURRENT_CHANGE
            ],
            [0x2_0000, 0x4_0000, 0x8_0000, 0x10_0000]
        );
        assert_eq!(
            [RESET_CHANGE, LINK_CHANGE, CONFIG_ERROR_CHANGE],
            [0x20_0000, 0x40_0000, 0x80_0000]
        );
        assert_eq!(CHANGES, 0x00fe_0000);
    }

    /// Setting a bit the port already has keeps it set: power stays on when the caller writes
    /// power again.
    #[test]
    fn writing_a_preserved_bit_again_keeps_it() {
        assert_eq!(write_value(CONNECTED | POWER, POWER), CONNECTED | POWER);
    }

    #[test]
    fn endpoint_zero_starts_at_the_speeds_minimum() {
        assert_eq!(default_control_packet(speed::LOW), Some(8));
        assert_eq!(default_control_packet(speed::FULL), Some(8));
        assert_eq!(default_control_packet(speed::HIGH), Some(64));
        assert_eq!(default_control_packet(speed::SUPER), Some(512));
        assert_eq!(default_control_packet(0), None);
    }

    fn ports(numbers: &[u8]) -> Connections {
        let mut bits = [0; 8];
        for &n in numbers {
            bits[usize::from(n / 32)] |= 1 << (n % 32);
        }
        bits
    }

    /// Run a settle over `timeline` (milliseconds after power, the ports connected from then on),
    /// reading every step as the driver does: when it stops, and what it last read.
    fn settle(timeline: &[(u64, &[u8])]) -> (u64, Option<bool>, Connections) {
        let at = |t: u64| {
            ports(
                timeline
                    .iter()
                    .rev()
                    .find(|(from, _)| *from <= t)
                    .unwrap()
                    .1,
            )
        };
        let mut s = Settle::new(0, at(0));
        let mut t = 0;
        loop {
            t += DEBOUNCE_STEP_MS;
            if let Some(settled) = s.observe(t, at(t)) {
                return (t, Some(settled), at(t));
            }
        }
    }

    /// **xenon's ports after the reset**, from the PORTSC probe on 2026-10-21 (UTC): nothing at
    /// first, ports 4 and 9 back about 10 ms later, port 1 a millisecond after them. The scan the
    /// boot line counts read none of them; a settled scan reads all three.
    #[test]
    fn xenons_ports_are_all_counted_once_they_settle() {
        let (t, settled, seen) = settle(&[(0, &[]), (10, &[4, 9]), (11, &[1, 4, 9])]);
        assert_eq!(settled, Some(true));
        assert_eq!(seen, ports(&[1, 4, 9]));
        assert_eq!(t, 125);
    }

    /// QEMU reports a connection the moment the controller runs: nothing changes, and the scan
    /// still waits the power-on floor rather than reading at once.
    #[test]
    fn a_port_connected_from_the_start_is_believed_after_the_floor() {
        assert_eq!(settle(&[(0, &[5])]), (100, Some(true), ports(&[5])));
    }

    /// A port that keeps changing never settles, and the wait ends at the timeout with `false`, so
    /// the boot line can say its count is not a fact.
    #[test]
    fn a_port_that_never_holds_still_ends_at_the_timeout() {
        let mut s = Settle::new(0, ports(&[]));
        let mut t = 0;
        let end = loop {
            t += DEBOUNCE_STEP_MS;
            let bouncing = if (t / DEBOUNCE_STEP_MS) % 2 == 0 {
                ports(&[3])
            } else {
                ports(&[])
            };
            if let Some(settled) = s.observe(t, bouncing) {
                break settled;
            }
        };
        assert!(!end);
        assert_eq!(t, DEBOUNCE_TIMEOUT_MS);
    }
}
