//! **A serial console for a board this repository cannot see**: open the port, log
//! every byte, recognise how far the boot got, and stop on a deadline. See notes/board-console.md.
//!
//! - [`port`] finds the adapter and puts it in raw mode at 115200 8N1.
//! - [`watch`] reads until the policy says stop, teeing every byte to a log file.
//! - [`progress`] decides, from the text alone, how far the boot got.
//! - [`screen`] turns a screendump of nife's framebuffer console back into text, so [`progress`]
//!   judges a monitor exactly as it judges a cable (six target machines have no serial port).
//! - [`board`] declares each board's firmware prologue, the one board-specific thing here.
//! - [`lottery`] reports what the thread-placement lottery drew in each boot of a many-boot log.
//! - [`exposure`] reads a soak capture into an exposure row, in the columns
//!   `notes/multicore-defect-curve.md` defines.
//! - [`stop`] is the only part that writes: one byte that ends a self-rebooting soak, sent only
//!   after the board announces an armed reboot loop, and logged in hex.
//!
//! **It writes only what a named mode sends, and logs every byte it sends** (calef, 2026-09-19).
//! The hazard is an open keyboard beside U-Boot's autoboot countdown, and an echoed line could
//! spell a marker, since the recogniser matches substrings; keep the write surface at named
//! commands. It does not touch power.
//!
//! It is excluded from Miri (`script/undefined-behavior-check`) for cost: its lib tests took 55
//! minutes under the interpreter, and it has no `unsafe` for Miri to check. Some [`port`] tests
//! need the host filesystem and some [`watch`] tests are wall-clock driven, so they fail there.
//!
//! # BUGS
//!
//! notes/board-console.md's BUGS lists them, in full in `notes/board-console/limitations.md`.
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
