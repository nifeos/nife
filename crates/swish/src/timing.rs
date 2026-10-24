//! **`time`: what the shell may say about how long something took** (milestone 86 (`time`: the
//! shell times a command)): [`write_duration`], [`write_timing`], and [`Untimed`] for a line with
//! no duration to print.
//!
//! Name: provisional (milestone 884, 2026-10-12 UTC), for calef's naming pass.

use super::*;

/// **Why a line was not timed**, printed at the prompt instead of a duration, before anything is
/// spawned.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Untimed {
    /// `time` with nothing after it: a usage error, and the only reason, since a duration is two
    /// reads of an ambient counter (§72 (`time` needs no clock)).
    NothingToTime,
}

/// Write why a line was not timed.
pub fn write_untimed(r: Untimed, out: &mut dyn FnMut(&[u8])) {
    out(match r {
        Untimed::NothingToTime => b"  time: name a command to time: time <command>\n".as_slice(),
    });
}

/// **An elapsed duration, in the largest unit that keeps it readable**, with three digits after the
/// point and nothing after that.
///
/// Seconds, milliseconds or microseconds, and never nanoseconds: three digits is a rendering
/// choice, and the counter's resolution differs by board (notes/shell/the-swish-crate.md, "What
/// `time` may say").
///
/// ```
/// use swish::write_duration;
///
/// let shown = |nanos| {
///     let mut s = Vec::new();
///     write_duration(nanos, &mut |b| s.extend_from_slice(b));
///     String::from_utf8(s).unwrap()
/// };
///
/// assert_eq!(shown(0), "0.000 us");
/// assert_eq!(shown(1_234), "1.234 us");
/// assert_eq!(shown(4_213_000), "4.213 ms");
/// assert_eq!(shown(90_500_000_000), "90.500 s");
/// ```
pub fn write_duration(nanos: u64, out: &mut dyn FnMut(&[u8])) {
    const MICRO: u64 = 1_000;
    const MILLI: u64 = 1_000_000;
    const SEC: u64 = 1_000_000_000;
    let (whole, thousandths, unit) = if nanos >= SEC {
        (nanos / SEC, (nanos % SEC) / MILLI, b" s".as_slice())
    } else if nanos >= MILLI {
        (nanos / MILLI, (nanos % MILLI) / MICRO, b" ms".as_slice())
    } else {
        (nanos / MICRO, nanos % MICRO, b" us".as_slice())
    };
    write_num(whole, out);
    out(b".");
    // Zero padded, which [`write_num`] is not: `4.13 ms` and `4.013 ms` differ. `thousandths` is
    // below 1000, since every branch above divides its remainder by the next unit down.
    out(&[
        b'0' + (thousandths / 100 % 10) as u8,
        b'0' + (thousandths / 10 % 10) as u8,
        b'0' + (thousandths % 10) as u8,
    ]);
    out(unit);
}

/// **The line `time` prints when it measured something.**
///
/// `real` is wall-clock time from the shell deciding to run the line to the line being over, a
/// difference of two reads of the monotonic counter (§72), so nothing needs disclaiming. There is
/// no `user` or `sys` row: nothing in this kernel is asked how much processor a thread consumed.
pub fn write_timing(nanos: u64, out: &mut dyn FnMut(&[u8])) {
    out(b"  time: real ");
    write_duration(nanos, out);
    out(b"\n");
}

#[cfg(test)]
mod tests;
