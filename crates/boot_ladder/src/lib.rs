//! **The rungs of the boot ladder, as the exact bytes that appear on a console.**
//!
//! Each rung's line is a contract between the program that prints it and the programs that read it,
//! on a bench, in CI or in a photograph of a monitor. The kernel prints [`BANNER`] and [`MACHINE`];
//! the boot self-test program prints [`SELF_TEST`]; the shell prints [`PROMPT`]; a console reader
//! matches all of them. Anything two binaries must agree on is code in one crate, and this is that
//! crate for the boot's console lines. Every constant is the stable head of a line whose tail
//! carries numbers: a matcher keys on the head, which never changes, and the tail is free to
//! improve.
//!
//! # Matching a head
//!
//! A head begins a line, possibly after bytes a serial link corrupted, so a reader finds it
//! anywhere in a line and takes the tail from there. Every head except [`PROMPT`] ends in a space or
//! punctuation, so a match falls on a boundary. [`PROMPT`] is the shell's banner and is matched
//! whole; the shell prints a period after it as part of its tail. The tail of [`BANNER`] is free
//! text: the architecture's name is on `MACHINE`'s tail, which a reader takes it from.

//! # BUGS
//!
//! - Nothing gates that a new rung is added here rather than as a fresh literal; review does. The
//!   kernel uses these constants, so its own output cannot drift from them.
//! - [`PROMPT`] is the shell's banner, not its `$ ` prompt, which is too weak to key on in a log;
//!   whether the prompt came out is a question for a test that drives the shell.
//! - The wordings are provisional: they are contracts, so they are an architect's.
//!
//! Name: ratified 2026-09-14 (calef, working the unratified worklist). A noun for the ladder a boot
//! climbs, coined rather than standard. `AGENTS.md` spends *ladder* on its mechanism-strength
//! metaphor; the two are unambiguous in context, and *rungs of a boot* is legible on sight.
//! Refused `boot_markers` (names the mechanism, strings that get matched, rather than the thing).
//! Refused `boot_proto` (the `_protocol` suffix names a request/reply vocabulary; this is one-way
//! text on a console). Refused `boot_stages` (`serial_console`'s `progress::Stage` already spends
//! the word). Refused `console_contract` (describes the mechanism).

#![no_std]

pub mod kernel_half;
mod rungs;
pub mod self_test_program;
pub mod verdict;

pub use rungs::*;
