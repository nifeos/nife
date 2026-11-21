//! **The fixed heads of the lines a kernel and its shell print while booting, and the list of
//! self-test check names.**
//!
//! Each line is a contract between the program that prints it and the programs that read it, on a
//! bench, in CI or in a photograph of a monitor. Each head is the system's name followed by a
//! fixed shape: a system builds its [`Heads`] from its name, the kernel prints
//! [`banner`](Heads::banner) and [`machine`](Heads::machine), the self-test program prints
//! [`self_test`](Heads::self_test), the shell prints [`shell_banner`](Heads::shell_banner), and a
//! console reader matches all of them through the same [`Heads`]. A tail that carries numbers
//! follows its head: a matcher keys on the head, which never changes, and the tail is free to
//! improve.
//!
//! # The matching rule
//!
//! A head begins a line, but a serial link may put bytes in front of it, so a reader finds the head
//! anywhere in a line and takes the tail from there ([`Head::find_in`]). Every head but the shell
//! banner ends in a space or punctuation, so a match is on a boundary; the shell banner is matched
//! whole, and the shell prints a period after it that is not part of the head. The architecture
//! after the banner is free text: a reader takes it from the tail of
//! [`machine`](Heads::machine).
//!
//! The self-test verdict has a grammar of its own, which [`verdict::Verdict`] prints and
//! [`verdict::ParsedVerdict`] reads, so the writer and every reader share one definition.
//!
//! # BUGS
//!
//! - Nothing gates that a new rung is added here rather than as a fresh literal in a program; review
//!   does. The kernel prints through [`Heads`], so its own output cannot drift from them.
//! - [`shell_banner`](Heads::shell_banner) is the shell's banner, not its `$ ` prompt, which is too
//!   weak to key on in a log; a reader that needs the prompt waits for the banner and then looks for
//!   `$ `.
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
mod rungs;
pub mod verdict;

pub use heads::{Head, Heads};
pub use rungs::*;
