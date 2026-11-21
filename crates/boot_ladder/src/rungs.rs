//! **The words inside the self-test verdict, and its list of checks.** The verdict's head is
//! [`Heads::self_test`](crate::Heads::self_test); these follow it. The crate root re-exports every
//! one, so a reader names them `boot_ladder::SELF_TEST_FAILED`.
//!
//! Name: provisional. A child named for what it holds, so the crate root is an index.

/// **What a red verdict says before it names the checks that failed.**
///
/// Uppercase, the tree's word for a failed check, which other boot lines also use. It appears
/// *inside* a [`self_test`](crate::Heads::self_test) line, after any [`SELF_TEST_ABSENT`] list; a
/// matcher wanting a failed verdict must find both.
///
/// Name: ratified 2026-11-19 (UTC), block 1078.
pub const SELF_TEST_FAILED: &str = "FAILED: ";

/// **What a verdict says before it names the checks this machine had nothing for**:
/// `<system> self-test: 9 of 13 passed, 3 absent: block_device clock true_random_number_generator`.
///
/// An absent check neither passed nor failed: the machine has no device for it to test, and the
/// kernel's line for that check says which. The verdict stays green unless something says
/// [`SELF_TEST_FAILED`], which always comes after this list when both are present. Whether an
/// absence is acceptable is the board's fact, not the kernel's: `serial_console`'s board profiles
/// name the checks that may not be absent on each board, and read such an absence as a failure.
///
/// Name: ratified 2026-11-19 (UTC), block 1078.
pub const SELF_TEST_ABSENT: &str = "absent: ";

/// **The boot self-test's checks, by name, in the order the verdict names them.**
///
/// One list for every architecture and both halves: the kernel runs the checks of its own
/// internals, and the self-test program the ones that need a user program. Each takes its check's
/// index from here at compile time ([`verdict::check`](crate::verdict::check)), so an unlisted
/// check does not build, and the verdict counts against this list rather than against what ran: a
/// listed check that did not run fails by name, and a reader fails a verdict whose total is not
/// this list's length or one of [`EARLIER_SELF_TEST_CHECK_COUNTS`].
///
/// They are printed, and matched, so they are a contract. A name is lower-case ASCII letters,
/// digits and underscores, and appears once.
///
/// Name: ratified 2026-11-19 (UTC), block 1078; the fifteen strings ratified 2026-11-14 (UTC).
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
    "block_device",
    "device_interrupt",
    "clock",
    "true_random_number_generator",
    "entropy",
];

/// **The lengths [`SELF_TEST_CHECKS`] had before, for reading a log a kernel of that time
/// printed.** A captured bench log and a board still running an older image say `5 of 5 passed`,
/// and that verdict was green against the list its kernel was built with. A reader accepts these
/// totals as well as the current one; a kernel built from this version always counts against the
/// current list, so it cannot print one of them.
///
/// | length | the list held |
/// |---|---|
/// | 5 | `exceptions`, `mapping`, `frames`, `timer`, `scheduler` |
/// | 13 | the current list without `supervised` and `teardown` |
///
/// Name: ratified 2026-11-19 (UTC), block 1078. A name for what the numbers count.
pub const EARLIER_SELF_TEST_CHECK_COUNTS: &[usize] = &[5, 13];

#[cfg(test)]
mod tests {
    use super::*;

    /// **The check list's invariants.** The kernel finds a check's index with `position`, so a
    /// duplicate would leave its second entry unrecorded on every boot, and a reader splits the list
    /// on whitespace and commas, so a name outside `[a-z0-9_]` would split in two. The earlier
    /// lengths ascend and lie below the current one.
    #[test]
    fn the_check_names_are_plain_and_distinct() {
        for (i, name) in SELF_TEST_CHECKS.iter().enumerate() {
            assert!(
                !name.is_empty()
                    && name
                        .bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
            );
            assert!(
                !SELF_TEST_CHECKS[..i].contains(name),
                "{name} is listed twice"
            );
        }
        assert!(
            EARLIER_SELF_TEST_CHECK_COUNTS
                .windows(2)
                .all(|w| w[0] < w[1])
        );
        assert!(
            EARLIER_SELF_TEST_CHECK_COUNTS
                .iter()
                .all(|&n| n < SELF_TEST_CHECKS.len())
        );
    }
}
