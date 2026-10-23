//! The CMOS real-time clock, read by the kernel (§121, §130; milestone 176's piece 2): two fixed
//! I/O ports and no page, so no device capability maps it. It is read once per clock-service spawn,
//! and `kernel::user::clock_service::start` passes the reading as a `Spawn` argument
//! (`clock_contract::rtc::CMOS`). Two snapshots, each after its own UIP wait, must agree. See
//! notes/clock.md for the register map, tearing and the century.
//!
//! # BUGS
//! - No century byte: the year is `2000 + value`, so dates outside 2000..=2099 cannot be read.
//! - Daylight saving and the alarm and periodic-interrupt registers are untouched.
//! - Not proven against real hardware, only QEMU's MC146818A model.

use super::port::{in8, out8};

/// The index port: write a register number here before reading or writing [`DATA`].
const INDEX: u16 = 0x70;
/// The data port: [`INDEX`]'s last write names which CMOS byte this reaches.
const DATA: u16 = 0x71;

const REG_SECONDS: u8 = 0x00;
const REG_MINUTES: u8 = 0x02;
const REG_HOURS: u8 = 0x04;
const REG_DAY: u8 = 0x07;
const REG_MONTH: u8 = 0x08;
const REG_YEAR: u8 = 0x09;
const REG_STATUS_A: u8 = 0x0A;
const REG_STATUS_B: u8 = 0x0B;

/// Status register A, bit 7: the RTC is mid-update and every other register is unreliable.
const STATUS_A_UPDATE_IN_PROGRESS: u8 = 1 << 7;
/// Status register B, bit 2: fields are binary (set) rather than BCD (clear).
const STATUS_B_BINARY: u8 = 1 << 2;
/// Status register B, bit 1: the hours field is 24-hour (set) rather than 12-hour with a PM flag.
const STATUS_B_24_HOUR: u8 = 1 << 1;
/// The hours register's bit 7 in 12-hour mode: set for PM. In 24-hour mode this bit is unused and
/// reads clear.
const HOUR_PM: u8 = 1 << 7;

/// Read one CMOS register.
fn read(reg: u8) -> u8 {
    // SAFETY: 0x70/0x71 are the CMOS index/data ports on every PC-compatible machine, driven only
    // from ring 0. DECISIONS §121: no port-range capability names them under its current
    // recommendation, so nothing in userspace could reach these ports even in principle, and
    // nothing here could race a driver that does not exist.
    unsafe {
        out8(INDEX, reg);
        in8(DATA)
    }
}

fn is_update_in_progress() -> bool {
    read(REG_STATUS_A) & STATUS_A_UPDATE_IN_PROGRESS != 0
}

/// One look at the six date/time registers, taken together so [`read_unix_nanos`]'s tearing check
/// can compare two of them for equality.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Snapshot {
    second: u8,
    minute: u8,
    hour: u8,
    day: u8,
    month: u8,
    year: u8,
}

fn snapshot() -> Snapshot {
    Snapshot {
        second: read(REG_SECONDS),
        minute: read(REG_MINUTES),
        hour: read(REG_HOURS),
        day: read(REG_DAY),
        month: read(REG_MONTH),
        year: read(REG_YEAR),
    }
}

/// One BCD byte (two packed decimal digits) to binary: `(v & 0x0F) + 10 * (v >> 4)`.
fn bcd_to_binary(v: u8) -> u8 {
    (v & 0x0F) + 10 * (v >> 4)
}

/// Read the CMOS RTC once and return the wall clock it reports, in nanoseconds since the Unix
/// epoch.
///
/// `None` when the reading does not describe a real calendar date: every field zero, which is what
/// a CMOS with a dead battery and no host to seed it reads as, is the case this actually exists to
/// catch (`calendar::Civil::new` refuses month 0 and day 0). A machine this kernel can otherwise
/// run on has no way to make CMOS report a well-formed but implausible date;
/// `components/src/clock.rs` still runs whatever it is handed through
/// `clock_contract::policy::is_plausible` before publishing it, the same as every other RTC
/// binding.
pub(crate) fn read_unix_nanos() -> Option<u64> {
    let raw = loop {
        while is_update_in_progress() {
            core::hint::spin_loop();
        }
        let first = snapshot();
        while is_update_in_progress() {
            core::hint::spin_loop();
        }
        let second = snapshot();
        if first == second {
            break first;
        }
        // An update began between the snapshots; one UIP wait alone is not enough
        // (notes/clock.md, "Tearing").
    };

    let status_b = read(REG_STATUS_B);
    let binary = status_b & STATUS_B_BINARY != 0;
    let twenty_four_hour = status_b & STATUS_B_24_HOUR != 0;

    // The PM flag shares the hours byte with the BCD/binary hour in every mode, so it is peeled
    // off before either decode rather than after.
    let pm = !twenty_four_hour && raw.hour & HOUR_PM != 0;
    let hour_field = raw.hour & !HOUR_PM;

    let (second, minute, hour, day, month, year) = if binary {
        (
            raw.second, raw.minute, hour_field, raw.day, raw.month, raw.year,
        )
    } else {
        (
            bcd_to_binary(raw.second),
            bcd_to_binary(raw.minute),
            bcd_to_binary(hour_field),
            bcd_to_binary(raw.day),
            bcd_to_binary(raw.month),
            bcd_to_binary(raw.year),
        )
    };

    let hour = match (twenty_four_hour, pm, hour) {
        (true, _, h) => h,
        (false, true, 12) => 12, // 12 PM is noon, unchanged
        (false, true, h) => h + 12,
        (false, false, 12) => 0, // 12 AM is midnight
        (false, false, h) => h,
    };

    // No century byte; see notes/clock.md.
    let year = 2000 + i32::from(year);

    let civil = calendar::Civil::new(year, month, day, hour, minute, second).ok()?;
    let secs = u64::try_from(civil.to_unix()).ok()?;
    Some(secs * clock_contract::NANOS_PER_SEC)
}
