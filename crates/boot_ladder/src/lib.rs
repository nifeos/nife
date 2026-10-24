//! **The rungs of the boot ladder, as the exact bytes that appear on a console** (milestone 268).
//!
//! Each rung's line is a contract between the program that prints it and the programs that read
//! it, on a bench, in CI or in a photograph of a monitor. The kernel prints [`BANNER`], [`MACHINE`]
//! and [`SELF_TEST`]; `swish` prints [`PROMPT`]; `serial_console` matches all of them (rule 7 of
//! `AGENTS.md`). Every constant is the stable head of a line whose tail carries numbers: a matcher
//! keys on the head, which never changes, and the tail is free to improve. Each carries its own
//! trailing space or punctuation, so a match is on a boundary. See notes/boot-ladder.md.

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

/// **The earliest line: `nife on <architecture> (...)`.**
///
/// Printed by `kernel_main` as the first thing after `console::init`, on every architecture since
/// milestone 268. aarch64 had no such line before it, so this rung was unreachable there for as
/// long as the recogniser has existed. It says the console works and it says nothing else, which
/// is exactly the claim wanted at first light on a board where the console is the thing in doubt.
///
/// Deliberately generic across the three: a recogniser that knew only the VisionFive 2's wording
/// would report a healthy aarch64 or `x86_64` board as never having booted.
pub const BANNER: &str = "nife on ";

/// **The machine description finished: `nife machine: <arch>, <n> processor(s), ...`.**
///
/// Printed by `kernel/src/machine_description.rs`'s `print_machine_description` as its **last**
/// line, and that position is the point: reaching it means the whole description printed, which is
/// the claim a ladder rung should make. A header would only have meant the block started.
pub const MACHINE: &str = "nife machine: ";

/// **The self-test verdict: `nife self-test: 13 of 13 passed`, `... , 2 absent: <names>`, or
/// `... , 1 FAILED: <names>`.**
///
/// Printed by `kernel/src/self_test.rs` after the description and before anything is handed to
/// userspace. The counts are there on success as well as on failure, so that "ran nothing and
/// passed" is distinguishable from "ran the set and passed".
pub const SELF_TEST: &str = "nife self-test: ";

/// **What a red verdict says before it names the checks that failed.**
///
/// Uppercase, the tree's word for a failed check, which other boot lines also use. It appears
/// *inside* a [`SELF_TEST`] line, after any [`SELF_TEST_ABSENT`] list; a matcher wanting a failed
/// verdict must find both.
pub const SELF_TEST_FAILED: &str = "FAILED: ";

/// **What a verdict says before it names the checks this machine had nothing for** (calef's ruling
/// of 2026-10-20 (UTC) on #2447): `nife self-test: 9 of 13 passed, 3 absent: blockdev clock trng`.
///
/// An absent check neither passed nor failed: the machine has no device for it to test, and the
/// kernel's line for that check says which. The verdict stays green unless something says
/// [`SELF_TEST_FAILED`], which always comes after this list when both are present. Whether an
/// absence is acceptable is the board's fact, not the kernel's: `serial_console`'s board profiles
/// name the checks that may not be absent on each board, and read such an absence as a failure.
///
/// Name: provisional, minted 2026-10-20 (UTC) by lane `milestone/890-tours-to-self-test`.
pub const SELF_TEST_ABSENT: &str = "absent: ";

/// **The boot self-test's checks, by name, in the order they run** (milestone 268).
///
/// One list for every architecture. The kernel takes its check names from here and counts its
/// verdict against this list rather than against what ran: a listed check that did not run fails
/// by name, an unlisted one fails by name, and `serial_console` fails a verdict whose total is not
/// this list's length. See notes/boot-ladder.md, "One list of checks, for every architecture".
///
/// Names provisional (milestone 268): they are printed, and matched, so they are a contract.
pub const SELF_TEST_CHECKS: &[&str] = &[
    "exceptions",
    "mapping",
    "frames",
    "timer",
    "scheduler",
    "preemption",
    "userspace",
    "budget",
    "blockdev",
    "deviceirq",
    "clock",
    "trng",
    "entropy",
];

/// **The lengths [`SELF_TEST_CHECKS`] had before, for reading a log a kernel of that time
/// printed.** A captured bench log and a board still running an older image say `5 of 5 passed`,
/// and that verdict was green against the list its kernel was built with. A recognizer accepts
/// these totals as well as the current one; the kernel itself always counts against the current
/// list, so a kernel built from this tree cannot print one of them.
///
/// | length | in force |
/// |---|---|
/// | 5 | 2026-09-14 to 2026-10-20 (UTC): `exceptions` to `scheduler` (milestone 268 (every architecture boots the same way)) |
///
/// Name: provisional (milestone 890 (riscv64 and `x86_64` join the shared boot path)'s lane,
/// 2026-10-20 UTC).
pub const SELF_TEST_CHECKS_EARLIER: &[usize] = &[5];

/// **The boot's middle stages ran to their end**: `nife: the capability core runs on <arch>.`
///
/// Printed by `kernel_main` on all three architectures after the [`SELF_TEST`] verdict and any
/// tour, just before the hand-over, so it says the machine was described and every check in
/// [`SELF_TEST_CHECKS`] ran. It does not say they passed; the verdict says that. The `<arch>` is
/// the kernel's own `arch::NAME`.
pub const TOUR: &str = "nife: the capability core runs on ";

/// **Userspace is up and the shell is offering a prompt.**
///
/// `components/src/swish.rs` prints this immediately before its first `$ `. It is the terminal
/// state of a default boot: milestone 268 decided that *nothing halts*, and that the prompt rather
/// than a halt is the signal that the boot finished.
///
/// See this module's `BUGS` for why the banner and not the prompt itself.
pub const PROMPT: &str = "nife capability shell";

/// **The kernel is holding the screen open for a host that wants to photograph it**
/// (milestone 445 (the screen check stops sampling and starts asking)), and will clear it as soon as one byte comes
/// back on the serial line. *Wording provisional, like every other constant here.*
///
/// Not a rung, but a line two binaries agree on: `kernel::console::yield_screen` prints it when the
/// boot command line carried `machine_discovery::framebuffer::SCREEN_HOLD`, and `cargo xtask
/// uefi-boot` waits for it, takes its screendump and answers. A boot that was not asked to hold
/// never prints it. Why it asks rather than samples is in notes/boot-ladder.md, "The screen hold".
pub const SCREEN_HELD: &str = "nife screen: held for the host; ";
