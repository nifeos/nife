//! **The words inside the self-test verdict, and its list of checks.** The verdict's head is
//! [`Heads::self_test`](crate::Heads::self_test); these follow it. The crate root re-exports every
//! one, so a reader names them `boot_ladder::SELF_TEST_FAILED`. See notes/boot-ladder.md.
//!
//! Name: provisional. A child named for what it holds, so the crate root is an index (§282 (when a
//! file is split, the parent becomes an index)).

/// **What a red verdict says before it names the checks that failed.**
///
/// Uppercase, the tree's word for a failed check, which other boot lines also use. It appears
/// *inside* a [`self_test`](crate::Heads::self_test) line, after any [`SELF_TEST_ABSENT`] list; a
/// matcher wanting a failed verdict must find both.
pub const SELF_TEST_FAILED: &str = "FAILED: ";

/// **What a verdict says before it names the checks this machine had nothing for** (calef's ruling
/// of 2026-10-20 (UTC) on #2447): `<system> self-test: 9 of 13 passed, 3 absent: blockdev clock
/// trng`.
///
/// An absent check neither passed nor failed: the machine has no device for it to test, and the
/// kernel's line for that check says which. The verdict stays green unless something says
/// [`SELF_TEST_FAILED`], which always comes after this list when both are present. Whether an
/// absence is acceptable is the board's fact, not the kernel's: `serial_console`'s board profiles
/// name the checks that may not be absent on each board, and read such an absence as a failure.
///
/// Name: provisional, minted 2026-10-20 (UTC) by lane `milestone/890-tours-to-self-test`.
pub const SELF_TEST_ABSENT: &str = "absent: ";

/// **The boot self-test's checks, by name, in the order the verdict names them** (milestone 268).
///
/// One list for every architecture and both halves: the kernel runs the checks of its own
/// internals, and `boot_self_test` the ones that need a user program (milestone 1063 (the boot
/// self-test's spawning checks run from userspace)). Each takes its check's index from here at
/// compile time (`verdict::check`), so an unlisted check does not build, and the verdict counts
/// against this list rather than against what ran: a listed check that did not run fails by name,
/// and `serial_console` fails a verdict whose total is not this list's length. See
/// notes/boot-ladder.md, "One list of checks, for every architecture".
///
/// Names provisional (milestone 268): they are printed, and matched, so they are a contract.
/// `device_interrupt` (spelled `deviceirq` until 2026-10-26 (UTC)) awaits a ruling, and so does
/// reading older logs that print the old spelling.
pub const SELF_TEST_CHECKS: &[&str] = &[
    "exceptions",
    "mapping",
    "frames",
    "timer",
    "scheduler",
    "preemption",
    "userspace",
    "budget",
    "supervised",
    "teardown",
    "blockdev",
    "device_interrupt",
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
/// | 13 | 2026-10-20 to 2026-10-24 (UTC): the list without `supervised` and `teardown` (milestone 890 (riscv64 and `x86_64` join the shared boot path)) |
///
/// Name: provisional (milestone 890 (riscv64 and `x86_64` join the shared boot path)'s lane,
/// 2026-10-20 UTC).
pub const SELF_TEST_CHECKS_EARLIER: &[usize] = &[5, 13];
