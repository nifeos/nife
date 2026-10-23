//! **What the `rm` program reports** (`components/src/rm.rs`), on its report endpoint under the
//! sink contract: lines of text, then the verdict, told apart by the first word.
//!
//! - A line is framed as std's stdout is: `w0` = the byte count (at most 16), the bytes
//!   little-endian in `w1`|`w2`. Diagnostics always go out this way; `-v` adds one per name removed.
//! - The verdict, once and last: `w0` = `byte_sink_contract::eof()`, `w1` = the exit status, `w2` =
//!   how many names were removed. A receiver whose first message is the verdict knows the run
//!   printed nothing, since a `SEND` blocks until it is received. See notes/rm.md.

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
