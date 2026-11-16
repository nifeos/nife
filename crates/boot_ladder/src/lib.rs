//! **The rungs of the boot ladder, as the exact bytes that appear on a console** (milestone 268).
//!
//! Each rung's line is a contract between the program that prints it and the programs that read it,
//! on a bench, in CI or in a photograph of a monitor. Each head is the system's name followed by a
//! fixed shape: the system builds its [`Heads`] from its name, the kernel prints
//! [`banner`](Heads::banner) and [`machine`](Heads::machine), the self-test program prints
//! [`self_test`](Heads::self_test), the shell prints [`prompt`](Heads::prompt), and a console
//! reader matches all of them through the same [`Heads`] (rule 7 of `AGENTS.md`). Each head starts a
//! line whose tail carries numbers: a matcher keys on the head, which never changes, and the tail is
//! free to improve. Every head but the shell banner ends in a space or punctuation, so a match is on
//! a boundary; the banner is matched whole. The shapes, and the words inside the verdict, were
//! ratified as printed on 2026-11-14 (UTC). See notes/boot-ladder.md.

//! # BUGS
//!
//! - Nothing gates that a new rung is added here rather than as a fresh literal; review does. The
//!   kernel prints through [`Heads`], so its own output cannot drift from them.
//! - [`prompt`](Heads::prompt) is the shell's banner, not its `$ ` prompt, which is too weak to key
//!   on in a log; whether the prompt came out is `cargo xtask swish-check`'s question.
//! - Nothing here checks that every program of one system builds its [`Heads`] from the same name;
//!   each passes its own, and a reader of that system's logs finds a mismatch as a missing rung.
//!
//! Name: ratified 2026-09-14 (calef, working the unratified worklist). A noun for the ladder a boot
//! climbs, coined rather than standard. `AGENTS.md` spends *ladder* on its mechanism-strength
//! metaphor; the two are unambiguous in context, and *rungs of a boot* is legible on sight.
//! Refused `boot_markers` (names the mechanism, strings that get matched, rather than the thing).
//! Refused `boot_proto` (the `_protocol` suffix names a request/reply vocabulary; this is one-way
//! text on a console). Refused `boot_stages` (`serial_console`'s `progress::Stage` already spends
//! the word). Refused `console_contract` (describes the mechanism).

#![no_std]

mod heads;
pub mod kernel_half;
mod rungs;
pub mod self_test_program;
pub mod verdict;

pub use heads::{Head, Heads};
pub use rungs::*;
