//! **What the `rm` program reports**: lines of text on its report endpoint under the sink
//! contract, then its verdict on an endpoint of its own.
//!
//! - A line is framed as std's stdout is: `w0` = the byte count (at most 16), the bytes
//!   little-endian in `w1`|`w2`. Diagnostics always go out this way; `-v` adds one per name removed.
//! - The report stream ends with the sink contract's end of stream, `(eof(), 0, 0)`, whose two
//!   words after the first are reserved and carry nothing.
//! - The verdict, once and after that end of stream, on [`VERDICT_SLOT`]: `w0` = the exit status,
//!   `w1` = how many names were removed, `w2` = 0. A `SEND` returns only once its message is
//!   received, so by the time the verdict goes out every line and the end of stream are already
//!   with the reader. A reader drains the report to its end and only then receives the verdict,
//!   and a run whose report holds only the end of stream printed nothing.
//! - A spawner that wants no verdict leaves the slot empty. The `SEND` is then refused, and the
//!   program exits as it would have anyway.

/// The slot `rm` sends its verdict on: after the directory at 0 and the report at 1. It is a
/// separate endpoint, rather than words on the report stream, because the report is a byte sink
/// that `wc` or a terminal may hold, and a verdict there would be a meaning those sinks cannot read.
///
/// Name: provisional.
pub const VERDICT_SLOT: u64 = 2;

/// The exit status of a run in which everything named was removed. `rm(1)`: "exits 0 if all
/// of the named files or file hierarchies were removed".
pub const OK: u64 = 0;

/// A non-zero exit status **is the errno of the first failure**, rather than a generic 1.
///
/// `rm(1)` promises only "a value >0", so this is a choice, and it is the one that costs
/// nothing and says more: a test that expected `EROFS` and got `ENOENT` has learned that the
/// walk stopped somewhere else than it thought. The [`crate::dir::explain`] sentence
/// for that number is what the program prints as its diagnostic, so the status word and the
/// text a human reads come from the same decision.
pub const fn status(errno: i32) -> u64 {
    errno as u64
}
