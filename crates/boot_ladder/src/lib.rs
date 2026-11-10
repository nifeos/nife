//! **The rungs of the boot ladder, as the exact bytes that appear on a console** (milestone 268).
//!
//! Each rung's line is a contract between the program that prints it and the programs that read it,
//! on a bench, in CI or in a photograph of a monitor. The kernel prints [`BANNER`] and [`MACHINE`];
//! `boot_self_test` prints [`SELF_TEST`]; `swish` prints [`PROMPT`]; `serial_console` matches all
//! of them (rule 7 of `AGENTS.md`). Every constant is the stable head of a line whose tail carries
//! numbers: a matcher keys on the head, which never changes, and the tail is free to improve. Each
//! carries its own trailing space or punctuation, so a match is on a boundary. See
//! notes/boot-ladder.md.

//! # BUGS
//!
//! - Nothing gates that a new rung is added here rather than as a fresh literal; review does. The
//!   kernel uses these constants, so its own output cannot drift from them.
//! - [`PROMPT`] is `swish`'s banner, not its `$ ` prompt, which is too weak to key on in a log;
//!   whether the prompt came out is `cargo xtask swish-check`'s question.
//! - The wordings are provisional (milestone 268 (every architecture boots the same way)): they
//!   are contracts, so they are an architect's.
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
