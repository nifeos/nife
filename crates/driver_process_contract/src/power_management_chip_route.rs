//! What the kernel, the JH7110 PMIC (power management integrated circuit) route and `reboot` agree on (§251 (restarting the machine is a
//! kernel object the progenitor hands out), "A board's PMIC route runs in userspace").
//!
//! The kernel's starter maps the I2C controller's page at [`CONTROLLER_VIRTUAL_ADDRESS`] and lists
//! it on the startup page as [`CONTROLLER_KIND`], and passes the target and the timing in the three
//! spawn words ([`Target::pack`], [`pack_timing`]). The route
//! (`components/src/power_management_chip_route.rs`) receives on [`REQUEST_SLOT`] and answers one
//! request, [`RESET`]. A reset the power controller honors never answers; every other outcome is a
//! [`Status`]. See notes/board-reboot.md.
//!
//! Name: provisional.

use address_space_map::pair_page;
use designware_inter_integrated_circuit::Timing;
use startup_page_contract::WindowKind;

/// Where the starter maps the I2C controller's register page, device-typed.
///
/// Name: provisional.
pub const CONTROLLER_VIRTUAL_ADDRESS: u64 = pair_page(0x0061_0000);

/// [`CONTROLLER_VIRTUAL_ADDRESS`]'s entry on the startup page.
///
/// Name: provisional.
pub const CONTROLLER_KIND: WindowKind = WindowKind(24);

/// The slot the route receives its one request on.
///
/// Name: provisional.
pub const REQUEST_SLOT: u64 = 0;

/// Where the operation sits in request word 0, as `entropy_contract::OPERATION_SHIFT` puts it.
///
/// Name: provisional.
pub const OPERATION_SHIFT: u32 = 56;

/// **Reset the board** through its power controller. No argument.
///
/// Name: provisional.
pub const RESET: u64 = 1;

/// Request word 0 for `operation`.
///
/// Name: provisional.
#[must_use]
pub const fn request(operation: u64) -> u64 {
    operation << OPERATION_SHIFT
}

/// The operation request word 0 carries.
///
/// Name: provisional.
#[must_use]
pub const fn operation(word: u64) -> u64 {
    word >> OPERATION_SHIFT
}

/// **Why the board is still running**, reply word 0. Reply word 1 carries the I2C failure as
/// [`pack_failure`] packs it, or the register's value for [`Status::StillRunning`].
///
/// Name: provisional.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// The request named an operation the route does not serve.
    UnknownOperation = 1,
    /// Reading the reset register failed, so nothing was written.
    ReadFailed = 2,
    /// Writing the reset bit failed.
    WriteFailed = 3,
    /// The write completed and the board kept running past the route's wait.
    StillRunning = 4,
}

impl Status {
    /// The reply word.
    ///
    /// Name: provisional.
    #[must_use]
    pub const fn to_word(self) -> u64 {
        self as u64
    }

    /// The status `word` names, or `None` for any word the route never sends, a kernel failure of
    /// the `CALL` (a small negative number) among them.
    ///
    /// Name: provisional.
    #[must_use]
    pub const fn from_word(word: u64) -> Option<Status> {
        match word {
            1 => Some(Status::UnknownOperation),
            2 => Some(Status::ReadFailed),
            3 => Some(Status::WriteFailed),
            4 => Some(Status::StillRunning),
            _ => None,
        }
    }
}

/// An I2C failure in one reply word: its kind in bits 39:32, its register word in bits 31:0.
///
/// Name: provisional.
#[must_use]
pub const fn pack_failure(kind: u8, word: u32) -> u64 {
    ((kind as u64) << 32) | word as u64
}

/// [`pack_failure`]'s two halves.
///
/// Name: provisional.
#[must_use]
pub const fn unpack_failure(word: u64) -> (u8, u32) {
    ((word >> 32) as u8, word as u32)
}

/// **What the route writes, and to whom**: the power controller's 7-bit number on the bus, its
/// reset register and the reset bit in it. Spawn word 0.
///
/// Name: provisional.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Target {
    /// The power controller's 7-bit number on the bus.
    pub chip: u8,
    /// The register that holds the reset bit.
    pub register: u8,
    /// The reset bit, 0 to 7.
    pub bit: u8,
}

impl Target {
    /// Spawn word 0: the chip in bits 7:0, the register in 15:8, the bit in 23:16.
    ///
    /// Name: provisional.
    #[must_use]
    pub const fn pack(self) -> u64 {
        self.chip as u64 | (self.register as u64) << 8 | (self.bit as u64) << 16
    }

    /// The target spawn word 0 names, or `None` for a word with a stray bit, a chip past 7
    /// bits or a bit past 7.
    ///
    /// Name: provisional.
    #[must_use]
    pub const fn unpack(word: u64) -> Option<Target> {
        let chip = word as u8;
        let bit = (word >> 16) as u8;
        if word >> 24 != 0 || chip > 0x7f || bit > 7 {
            return None;
        }
        Some(Target {
            chip,
            register: (word >> 8) as u8,
            bit,
        })
    }
}

/// Spawn words 1 and 2: the control word and the high count, then the low count and the hold.
///
/// Name: provisional.
#[must_use]
pub const fn pack_timing(timing: &Timing) -> [u64; 2] {
    [
        timing.control as u64 | (timing.high_count as u64) << 32,
        timing.low_count as u64 | (timing.data_hold as u64) << 32,
    ]
}

/// The timing [`pack_timing`] packed.
///
/// Name: provisional.
#[must_use]
pub const fn unpack_timing(words: [u64; 2]) -> Timing {
    Timing {
        control: words[0] as u32,
        high_count: (words[0] >> 32) as u32,
        low_count: words[1] as u32,
        data_hold: (words[1] >> 32) as u32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_request_carries_reset_in_the_top_byte() {
        assert_eq!(request(RESET), 1 << 56);
        assert_eq!(operation(request(RESET)), RESET);
    }

    #[test]
    fn every_status_round_trips_and_a_kernel_error_is_none() {
        for s in [
            Status::UnknownOperation,
            Status::ReadFailed,
            Status::WriteFailed,
            Status::StillRunning,
        ] {
            assert_eq!(Status::from_word(s.to_word()), Some(s));
        }
        assert_eq!(Status::from_word(0), None);
        assert_eq!(Status::from_word(-1i64 as u64), None);
    }

    #[test]
    fn the_target_round_trips_and_a_stray_bit_is_refused() {
        let t = Target {
            chip: 0x36,
            register: 0x32,
            bit: 6,
        };
        assert_eq!(t.pack(), 0x06_32_36);
        assert_eq!(Target::unpack(t.pack()), Some(t));
        assert_eq!(Target::unpack(t.pack() | 1 << 24), None);
        assert_eq!(Target::unpack(0x80), None);
        assert_eq!(Target::unpack(8 << 16), None);
    }

    #[test]
    fn the_timing_round_trips() {
        let t = Timing {
            control: 0x63,
            high_count: 0xd8,
            low_count: 0xfd,
            data_hold: 0x0f,
        };
        assert_eq!(unpack_timing(pack_timing(&t)), t);
    }

    #[test]
    fn a_failure_round_trips() {
        assert_eq!(unpack_failure(pack_failure(7, 0x80)), (7, 0x80));
    }
}
