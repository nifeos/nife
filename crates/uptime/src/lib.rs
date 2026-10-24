//! **`uptime`: how long the machine has been counting** (milestone 126,
//! design/roadmap/0126-who-else-is-running.md).
//!
//! The program's whole logic, lifted out so it runs on the host; `components/src/uptime.rs` is the
//! syscall and nothing else. It reads `user_mode_runtime::monotonic_nanos`, the ambient counter
//! `date` reads too, which `kernel/src/arch/*/timer.rs`'s `init` grants every EL0 program as a
//! deliberate exception to DECISIONS §10 (process model: capability-based, microkernel)'s
//! no-ambient-authority rule: a monotonic counter observes time and affects nothing. So `uptime`
//! needs no manifest field and no new capability.

//! # BUGS
//!
//! - No load average and no logged-in-user count: this kernel keeps no decaying load figure and no
//!   login registry, and a placeholder zero would read as a real answer.
//! - The counter's zero predates the kernel's init by an unmeasured amount: firmware or the VMM
//!   starts it, and there is no kernel-boot epoch (Linux's `CLOCK_BOOTTIME`). See notes/clock.md.
//! - One-second resolution, `date`'s own limitation: the printed format has no sub-second field.
//!
//! Name: ratified 2026-09-13 (calef, working the unratified worklist), for this crate and the
//! program: upstream `procps`'s own name. It prints only elapsed time, a narrowing rather than a
//! contradiction, since how long the machine has been up is exactly what `uptime` names.

#![cfg_attr(not(test), no_std)]

const NANOS_PER_SEC: u64 = 1_000_000_000;
const SECS_PER_MIN: u64 = 60;
const SECS_PER_HOUR: u64 = 60 * SECS_PER_MIN;
const SECS_PER_DAY: u64 = 24 * SECS_PER_HOUR;

/// The longest line [`format`] can produce: `u64::MAX` nanoseconds is about 584 years, and
/// `"up 213503982 days, 23:59:59\n"` is 29 bytes. Rounded up with room to spare.
pub const MAX_LEN: usize = 40;

/// A formatted line, owned inline: no allocator on this target.
pub struct Formatted {
    buf: [u8; MAX_LEN],
    len: usize,
}

impl Formatted {
    /// The formatted line, newline included, with no trailing garbage.
    pub fn as_bytes(&self) -> &[u8] {
        &self.buf[..self.len]
    }
}

/// Turn elapsed nanoseconds on the ambient monotonic counter into `up [D day[s], ]HH:MM:SS\n`.
///
/// Sub-day durations print no day count at all (`up 00:01:30\n`), rather than `up 0 days, ...`:
/// the zero case is the common one at a demo machine's uptime and Unix's own `uptime` elides it
/// too. Singular/plural is the one piece of English this program commits to, because "1 days" is
/// the detail that makes a status line read as generated rather than written.
///
/// # Examples
///
/// ```
/// use uptime::format;
///
/// assert_eq!(format(0).as_bytes(), b"up 00:00:00\n");
/// assert_eq!(format(90 * 1_000_000_000).as_bytes(), b"up 00:01:30\n");
/// assert_eq!(format(90_000 * 1_000_000_000).as_bytes(), b"up 1 day, 01:00:00\n");
/// ```
pub fn format(nanos: u64) -> Formatted {
    let total_secs = nanos / NANOS_PER_SEC;
    let days = total_secs / SECS_PER_DAY;
    let hours = (total_secs % SECS_PER_DAY) / SECS_PER_HOUR;
    let mins = (total_secs % SECS_PER_HOUR) / SECS_PER_MIN;
    let secs = total_secs % SECS_PER_MIN;

    let mut w = Writer {
        buf: [0u8; MAX_LEN],
        len: 0,
    };
    w.push_bytes(b"up ");
    if days > 0 {
        w.push_u64(days);
        w.push_bytes(if days == 1 { b" day, " } else { b" days, " });
    }
    w.push_2digit(hours);
    w.push_bytes(b":");
    w.push_2digit(mins);
    w.push_bytes(b":");
    w.push_2digit(secs);
    w.push_bytes(b"\n");

    Formatted {
        buf: w.buf,
        len: w.len,
    }
}

/// A fixed-size append cursor, the same shape `date`'s and `wc`'s hand-rolled buffers use: no
/// allocator on this target, so every formatter here is one.
struct Writer {
    buf: [u8; MAX_LEN],
    len: usize,
}

impl Writer {
    fn push_bytes(&mut self, bytes: &[u8]) {
        for &b in bytes {
            if self.len < self.buf.len() {
                self.buf[self.len] = b;
                self.len += 1;
            }
        }
    }

    /// Decimal, no leading zeros, no leading sign (every value this program formats is unsigned).
    fn push_u64(&mut self, mut v: u64) {
        let mut digits = [0u8; 20];
        let mut i = digits.len();
        loop {
            i -= 1;
            digits[i] = b'0' + (v % 10) as u8;
            v /= 10;
            if v == 0 {
                break;
            }
        }
        self.push_bytes(&digits[i..]);
    }

    /// Zero-padded to exactly two digits. Every caller here passes a value already reduced modulo
    /// 24 or 60, so `v < 100` always holds; a wider value would print its low two digits rather
    /// than panic, matching this crate's no-panic-on-bad-input posture elsewhere.
    fn push_2digit(&mut self, v: u64) {
        let v = v % 100;
        self.push_bytes(&[b'0' + (v / 10) as u8, b'0' + (v % 10) as u8]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The cursor drops what does not fit rather than writing past its buffer. `format` never gets
    /// there (its longest line is 29 of 40 bytes), so only the writer itself can be asked.
    #[test]
    fn the_writer_stops_at_the_end_of_its_buffer() {
        let mut w = Writer {
            buf: [0u8; MAX_LEN],
            len: 0,
        };
        w.push_bytes(&[b'x'; MAX_LEN + 5]);
        assert_eq!(w.len, MAX_LEN);
        w.push_bytes(b"y");
        assert_eq!(w.len, MAX_LEN);
        assert!(w.buf.iter().all(|&b| b == b'x'));
    }

    #[test]
    fn zero_is_up_zero() {
        assert_eq!(format(0).as_bytes(), b"up 00:00:00\n");
    }

    #[test]
    fn seconds_minutes_hours_all_render() {
        assert_eq!(format(5 * NANOS_PER_SEC).as_bytes(), b"up 00:00:05\n");
        assert_eq!(format(90 * NANOS_PER_SEC).as_bytes(), b"up 00:01:30\n");
        assert_eq!(format(3661 * NANOS_PER_SEC).as_bytes(), b"up 01:01:01\n");
    }

    #[test]
    fn a_day_rolls_over_and_pluralises() {
        // One full day plus one hour: singular "day".
        assert_eq!(
            format((SECS_PER_DAY + SECS_PER_HOUR) * NANOS_PER_SEC).as_bytes(),
            b"up 1 day, 01:00:00\n"
        );
        // Two days: plural.
        assert_eq!(
            format(2 * SECS_PER_DAY * NANOS_PER_SEC).as_bytes(),
            b"up 2 days, 00:00:00\n"
        );
    }

    #[test]
    fn sub_second_time_truncates_rather_than_rounds() {
        // 999,999,999 ns is still "0 seconds elapsed", not "1": this is a floor, matching how
        // date's own second-resolution read behaves.
        assert_eq!(format(NANOS_PER_SEC - 1).as_bytes(), b"up 00:00:00\n");
    }

    #[test]
    fn hours_and_minutes_never_reach_24_or_60_in_the_printed_field() {
        // 23 hours, 59 minutes, 59 seconds: the boundary just before a day rolls over.
        let secs = 23 * SECS_PER_HOUR + 59 * SECS_PER_MIN + 59;
        assert_eq!(format(secs * NANOS_PER_SEC).as_bytes(), b"up 23:59:59\n");
    }
}
