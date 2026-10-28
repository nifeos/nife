//! **A serial console for a board this repository cannot see**: open the port, log every byte,
//! recognise how far the boot got, and stop on a deadline. See notes/board-console.md.
//!
//! - [`port`] finds the adapter and puts it in raw mode at 115200 8N1; [`watch`] reads until the
//!   policy says stop, teeing every byte to a log file.
//! - [`progress`] decides from the text alone how far the boot got, and [`screen`] turns a
//!   screendump of nife's framebuffer console into text for it; [`board`] declares each board's
//!   firmware prologue.
//! - [`lottery`] and [`exposure`] read a finished soak capture into draws and exposure rows.
//!   [`stop`] alone writes, one logged byte ending a self-rebooting soak; none touches power.

//! # BUGS
//!
//! - Excluded from Miri (`script/undefined-behavior-check`) for cost: its lib tests took 55
//!   minutes under the interpreter, it has no `unsafe`, and some [`port`] and [`watch`] tests need
//!   the host filesystem or the wall clock, so they fail there.
//! - The mutation census's survivors in `port`'s `candidates`, `confirm_speed` and
//!   `dial_in_warning` are host-bound, and the census host cannot kill them:
//!   `candidates` reads this machine's `/dev`, `confirm_speed` needs a real tty to answer
//!   `stty`, and `dial_in_warning`'s body runs only on macOS (the `cfg!` guard returns `None`
//!   everywhere else, so its mutants are killed only by
//!   `the_dial_in_device_is_warned_about_and_the_call_out_one_is_not` on a macOS host). `scan`
//!   carries the tested logic; see notes/mutation-testing/serial-console.md and
//!   notes/board-console.md's BUGS.
//! - The rest are notes/board-console.md's BUGS, in full in `notes/board-console/limitations.md`.
//!
//! Name: ratified 2026-09-13 (calef), replacing `board_console`; half of a pair with
//! `screen_console`, two consoles named for where the text comes out. The command a bench session
//! types is still `script/board-console`, a name of its own (`script/names board-console`).

pub mod board;
pub mod exposure;
pub mod lottery;
pub mod port;
pub mod progress;
pub mod screen;
pub mod stop;
pub mod watch;

#[cfg(test)]
mod fixtures;

#[cfg(test)]
pub(crate) use fixtures::respell_pre_297_markers;
