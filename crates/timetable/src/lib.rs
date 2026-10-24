//! **`timetable`: a cron whose every entry is a grant** (milestone 129).
//!
//! An entry is a schedule plus a grant expression, checked at registration by [`grant_plan::plan`]
//! as the shell checks a prompt line, into the [`Endowment`](grant_plan::Endowment) its child will
//! hold, printable before anything fires. Registration answers with an [`Admission`]: it fires, its
//! program's manifest refuses it, or this scheduler holds nothing to back it, which is fixed at the
//! spawn site and not in the entry (DECISIONS §43 (reading the clock is a page)). A document that
//! does not parse is an [`Error`] with its line. `every` and `at-boot` entries run on the monotonic
//! counter; a calendar line ([`recurrence`]) needs the wall clock. No IO and no syscalls, so it is
//! all host-tested; the program is `components/src/timetable.rs`. See notes/scheduled-execution.md.

//! # The document
//!
//! ```text
//! # a comment
//! every 200ms  least_authority_demo 7
//! at-boot      least_authority_demo 3
//! every month on last fri at 17:00  least_authority_demo 5
//! ```

//! Name: ratified 2026-09-13 (calef), for this crate, the program `components/src/timetable.rs`
//! and `components/timetable.conf`. A noun for a table of scheduled entries that avoids
//! `scheduler`, which here means `kernel/src/sched.rs`; for the same reason `schedule_store` became
//! `timetable_store`. Refused `cron`, which names the ambient-authority model this inverts.
//! Refused `almanac`, which promises astronomical calendar scheduling. Refused `metronome`, which
//! names the tick and not the grant. Refused `grant_schedule`, worse as a word typed at a prompt.

#![no_std]

pub mod contract;
pub mod recurrence;
pub mod registration;

mod admission;
mod document;
mod plan_text;
mod registry;
#[cfg(test)]
mod test_support;

pub use admission::{Admission, Held, SHIPPED_HELD, Unbacked, schedulable};
pub use document::{Document, Entry, Error, Schedule, parse};
pub use plan_text::{Audit, write_plan};
pub use registry::{Registry, Row, SPENT, UNARMED, UNSTAMPED, WallReading, next_after};

/// Nanoseconds in a second. Spelled here rather than taken from `clock_contract`, because this crate
/// decides *when* rather than *what time it is*: it never touches the wall clock, and depending on
/// the wall-clock contract to name a unit would claim otherwise.
pub const NANOS_PER_SEC: u64 = 1_000_000_000;

/// The most entries one document may carry.
///
/// A ceiling rather than a limit anybody meets, and past it the document is
/// [`Error::TooManyEntries`] rather than quietly short of an entry somebody wrote down. Eight is
/// what a fixed array costs nothing to hold; the reason there is an array at all is that this crate
/// allocates nothing, so a document borrows its own bytes and the table is inline.
pub const MAX_ENTRIES: usize = 8;

// Machine-checked proofs (Kani). Behind `#[cfg(kani)]`, so an ordinary build or test never sees
// them; `script/verify` runs them.
#[cfg(kani)]
mod proofs;
