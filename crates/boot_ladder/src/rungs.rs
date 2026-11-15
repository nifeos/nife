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
/// of 2026-10-20 (UTC) on #2447): `<system> self-test: 9 of 13 passed, 3 absent: block_device clock
/// true_random_number_generator`.
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
/// They are printed, and matched, so they are a contract.
///
/// Name: ratified 2026-11-14 (UTC), block 1078, each of the fifteen.
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

// The check list's invariants. The kernel finds a check's index with `position`, so a duplicate would
// leave its second entry unrecorded on every boot, and a reader splits the list on whitespace and
// commas, so a name outside `[a-z0-9_]` would split in two.
const _: () = {
    let mut i = 0;
    while i < SELF_TEST_CHECKS.len() {
        let name = SELF_TEST_CHECKS[i].as_bytes();
        assert!(!name.is_empty(), "a check name is empty");
        let mut k = 0;
        while k < name.len() {
            let b = name[k];
            assert!(
                b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_',
                "a check name has a byte outside [a-z0-9_]"
            );
            k += 1;
        }
        let mut j = i + 1;
        while j < SELF_TEST_CHECKS.len() {
            let other = SELF_TEST_CHECKS[j].as_bytes();
            let mut same = other.len() == name.len();
            let mut k = 0;
            while same && k < name.len() {
                same = name[k] == other[k];
                k += 1;
            }
            assert!(!same, "two checks share a name");
            j += 1;
        }
        i += 1;
    }
    // Every earlier length is below the current one, and they ascend.
    let mut e = 0;
    while e < SELF_TEST_CHECKS_EARLIER.len() {
        assert!(
            SELF_TEST_CHECKS_EARLIER[e] < SELF_TEST_CHECKS.len(),
            "an earlier length is not below the current one"
        );
        assert!(
            e == 0 || SELF_TEST_CHECKS_EARLIER[e - 1] < SELF_TEST_CHECKS_EARLIER[e],
            "the earlier lengths do not ascend"
        );
        e += 1;
    }
};

#[cfg(test)]
mod tests {
    use super::*;

    /// The check list's invariants hold at run time too, so a failing assertion names the check.
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
        assert!(SELF_TEST_CHECKS_EARLIER.windows(2).all(|w| w[0] < w[1]));
        assert!(
            SELF_TEST_CHECKS_EARLIER
                .iter()
                .all(|&n| n < SELF_TEST_CHECKS.len())
        );
    }
}
