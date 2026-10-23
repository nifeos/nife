//! **The job frame: the shell's shared-memory channel to a supervised foreground child** (m24).
//!
//! The cooperative interrupt tier (DECISIONS §24) signals a running child through one page the
//! shell and the child both map, and the signal is a word the child reads with a plain load: a child
//! busy computing cannot poll an endpoint, since there is no non-blocking receive. See
//! notes/grant-expression.md ("The interrupt grant").
//!
//! The layout is four `u64` words at the byte offsets below. The shell writes [`INTERRUPT`]; the
//! child writes [`DONE`], [`STATUS`] and [`HEARTBEAT`]. One writer per word, so no locking is
//! needed: each side reads the other's words and writes only its own.

/// Byte offset of the interrupt flag (shell writes 1, child polls).
pub const INTERRUPT: usize = 0;
/// Byte offset of the done flag (child writes 1 as it exits; the shell watches it).
pub const DONE: usize = 8;
/// Byte offset of the exit-status word (child writes one of the `STATUS_*` codes).
pub const STATUS: usize = 16;
/// Byte offset of the heartbeat counter (child increments each work unit): liveness the shell
/// can show.
pub const HEARTBEAT: usize = 24;

/// The child stopped because it saw the cooperative interrupt and cleaned up. The graceful case.
pub const STATUS_INTERRUPTED: u64 = 1;
/// The child ran to natural completion without being interrupted.
pub const STATUS_DONE: u64 = 2;
