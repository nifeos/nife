//! **How a timetable is spawned: its slots, its arguments, and how its document arrives**
//! (milestone 129 (scheduled execution), §222 (who holds a user's schedule)).
//!
//! The boot-time test spawns one with the compiled-in document, and a durable session
//! (milestone 152 (durable delegation)) to register its user's schedule; both read this module
//! rather than a copy (AGENTS.md rule 7). Never hand one the run-unvouched capability
//! (`grant_plan::spawnproto::RUN_UNVOUCHED_SLOT`): a timetable holding it runs nothing and exits
//! with [`E_UNVOUCHED`], so a scheduled job stays within reach of §220 (signed builds, and trusting
//! a key is scoped).

//! # Slots
//!
//! | slot | constant | rights | what |
//! |---|---|---|---|
//! | 0 | [`OUT_SLOT`] | `WRITE` | the plan and the summary, `byte_sink_contract` bytes; compiled-in mode only |
//! | 1 | [`BUDGET_SLOT`] | `abi::rights::SPEND_ONLY` (since milestone 824 (a retype mints no right its budget lacks)) | the untyped every instance is split from |
//! | 2 | [`CHILD_REPORT_SLOT`] | `WRITE`, `GRANT` | handed to each job as its slot 0, when placed; a durable session places none |
//! | 3 | [`DEATHS_SLOT`] | `READ`, `GRANT` | each job's supervision endpoint, and what corpses are reaped through |
//! | 4 | [`CLOCK_SLOT`] | `READ`, `GRANT`, optional | the clock page (§43 (reading the clock is a page)), also mapped read-only at [`CLOCK_VA`] |
//! | 5 | [`ACTIVATION_SLOT`] | `WRITE` | the store's `activation/` directory, read-only; store mode only |
//! | 6 | [`PACKAGES_SLOT`] | `WRITE` | the store's `packages/` directory, read-only; store mode only |

//! Slot 4 is the one optional grant. With it `Held::clock` is true: calendar entries keep a time of
//! day, and a job whose manifest declares a clock gets the page read-only at its own slot 1 and
//! [`CLOCK_VA`], as the progenitor gives `date` one; without it every calendar line is
//! `Unbacked::WallClock`. The spawn site must map the page before the timetable starts; the
//! timetable probes the slot at `_start`, for `swish`'s reason: a region it splits later could land
//! there. **Store mode** (milestone 152) is a timetable holding [`ACTIVATION_SLOT`]: no archive
//! (`a1` is 0), each job's program the live activation generation's entry for its name, resolved at
//! registration and checked at every fire, slots 5 and 6 read-only caretakers over a file-service
//! channel whose page is at [`STORE_PAGE_VA`], and a registrar required. A durable session grants
//! no clock yet (`components/src/user_timetable_keeper.rs`'s BUGS).

//! # Arguments
//!
//! - `a0` ([`ARG_FIRES`]): how many fires before the timetable reports and exits; `0` is forever.
//! - `a1` ([`ARG_ARCHIVE_LEN`]): the length of the archive mapped read-only at
//!   `user_mode_runtime::initial_ramdisk::INITIAL_RAMDISK_VA`, holding its jobs' programs.
//! - `a2` ([`ARG_REGISTRATION_PAGE`]): where a writable registration page is mapped, or `0`.

//! # How the document arrives
//!
//! With `a2 == 0` it is the compiled-in `components/timetable.conf`, and everything the timetable
//! says goes down [`OUT_SLOT`]. With a page, it starts empty and the registrar sends the first
//! document with `REPLACE` ([`crate::registration`]). It is then **silent on [`OUT_SLOT`]**: a
//! session blocked on supervision cannot drain a stream, and a `SEND` nobody takes would stop the
//! timetable. The page carries each reply's status, verdicts and plan, and once it has stopped its
//! exit code at [`crate::registration::EXIT`]. Slots 0 and 2 may then be empty: a timetable holding
//! no report endpoint hands its jobs none (`crate::Held::report`), as a durable session's does.

//! Name: provisional, minted 2026-09-26 (UTC) by milestone 129's lane, for this module and every
//! constant here. Naming is calef's.

/// The output endpoint's slot.
pub const OUT_SLOT: u64 = 0;
/// The budget's slot.
pub const BUDGET_SLOT: u64 = 1;
/// The child report endpoint's slot.
pub const CHILD_REPORT_SLOT: u64 = 2;
/// The supervision endpoint's slot.
pub const DEATHS_SLOT: u64 = 3;

/// The clock page's slot, when the timetable is granted one.
pub const CLOCK_SLOT: u64 = 4;
/// Where the clock page is mapped, read-only: in the timetable, and in each job that declares a
/// clock. The address `date` reads it at, `system_initializer`'s `CHILD_CLOCK_VA`.
pub const CLOCK_VA: u64 = 0x00c0_0000;
/// Store mode's `activation/` directory: an endpoint to a caretaker serving it read-only.
pub const ACTIVATION_SLOT: u64 = 5;
/// Store mode's `packages/` directory, likewise.
pub const PACKAGES_SLOT: u64 = 6;
/// **Where a store-mode timetable finds its file-service channel's page**, mapped read/write by the
/// spawn site. The page is shared with the two caretakers and the file service, and with nothing
/// else: a channel of its own, because a job fires while its user may be using the store at the
/// prompt, and two clients staging bytes in one page overwrite each other (milestone 599 (a frame
/// per filesystem client channel)).
pub const STORE_PAGE_VA: u64 = 0x0610_0000;
/// **The largest program a store-mode timetable loads**, in bytes: its staging buffer. A job's
/// segments, stack and tables must fit an instance's 48 pages (192 KiB), and this bounds the file,
/// which carries a symbol table the instance never maps. It must hold the suite's installed
/// fixture, `least_authority_demo` as the archive packs it (`--strip-debug`, so the symbol table
/// stays): 71,664 bytes on aarch64 and 12,184 on riscv64. 128 KiB costs 16 pages of `.bss` more
/// than 64 KiB did (`user_timetable_keeper.rs`'s `TIMETABLE_REGION_PAGES`).
pub const STAGING_BYTES: usize = 128 << 10;

/// Which start argument carries the fire count.
pub const ARG_FIRES: usize = 0;
/// Which start argument carries the archive's length.
pub const ARG_ARCHIVE_LEN: usize = 1;
/// Which start argument carries the registration page's address.
pub const ARG_REGISTRATION_PAGE: usize = 2;

/// Stack pages a timetable needs. Its working set is the plan, a kilobyte per entry, and a
/// replacement holds two plans at once. The store-mode registration chain's debug-build frames
/// (`-Z emit-stack-sizes`) sum to about 125 KiB: `replace_if_asked` 48.0 KiB, `Registry::register`
/// 21.2, `_start` 20.0, `plan_against_with` 13.4, the resolve closures 11.2 and 10.0,
/// `register_installed` 9.8, `admit_installed` 5.6, and `resolve_installed` -> `load_current` ->
/// `store::load` 11.0. 48 pages is that with roughly half again; the bound is a size rather than an
/// assertion, so growth in `grant_plan`'s planning path is met by raising it.
pub const STACK_PAGES: u64 = 48;

/// Exit codes, the verdict word on [`OUT_SLOT`] or the page's exit word. A clean finish is `0`.
///
/// The document did not parse; the low byte is the line number.
pub const E_CONFIG: u64 = 0xE300;
/// The archive did not parse.
pub const E_ARCHIVE: u64 = 0xE301;
/// A program an admitted entry names is not in the archive.
pub const E_IMAGE: u64 = 0xE302;
/// The budget cannot back even one instance.
pub const E_BUDGET: u64 = 0xE303;
/// It was handed the run-unvouched capability, so it ran nothing.
pub const E_UNVOUCHED: u64 = 0xE304;
/// It holds the store and no registration page, which store mode requires.
pub const E_STORE: u64 = 0xE305;
