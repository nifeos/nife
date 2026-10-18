//! **What the `rm` program reports, and how a diagnostic is told from the verdict**
//! (milestone 47 (navigation and naming)'s `rm -r`, `components/src/rm.rs`).
//!
//! **Where these two words travel** (corrected 2026-08-17). `rm` declares the sink contract
//! (`grant_plan::OutputDeclaration::Bytes`), so its report is framed text and then `byte_sink_contract`'s
//! `OPERATION_EOF`; the status and the removal count ride in the two words that end-of-stream message
//! leaves free. They used to ride on `filesystem_fixture::VERDICT`, a word the sink contract has no
//! meaning for, which no reader but a guest test could have decoded.
//!
//! `rm` is a **program**, not a shell builtin, so it holds a report endpoint and nothing that
//! names a terminal. Two kinds of message go out on it, and they are told apart by the first
//! word, which is what makes "silent on success" checkable from the far end:
//!
//! - **A line of text**, framed exactly as every std program's stdout is (`w0` = the byte count,
//!   at most 16, and the bytes little-endian in `w1`|`w2`). Diagnostics always go out this way;
//!   `-v` adds one line per name removed.
//! - **The verdict**, once, last: `w0` = `filesystem_fixture::VERDICT` (the test crate's verdict
//!   word), `w1` = the exit status, `w2` = how many names were removed.
//!
//! A byte count is at most 16 and the verdict word is far larger, so the two never collide. A
//! receiver that takes the verdict as its **first** message therefore knows the run printed
//! nothing, which is `rm(1)`'s default behaviour asserted rather than assumed: a `SEND` blocks
//! until somebody receives it, so a line that was emitted cannot be missed by looking later.
//!
//! This module stayed in the protocol crate when the rest of the fixture moved out (milestone 886
//! (the filesystem test fixture moves to its own crate), calef's ruling): the exit-status
//! convention is words shipping programs send each other, not test data.

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
