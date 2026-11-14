//! **Each rung's marker**, the stable head of the line that announces it, and the other lines two
//! binaries agree on. The crate root re-exports every one, so a reader names them
//! `boot_ladder::BANNER`.
//!
//! Name: provisional. A child named for what it holds, so the crate root is an index (§282 (when a
//! file is split, the parent becomes an index)).

/// **The earliest line: `nife on <architecture> (...)`.**
///
/// Printed by the kernel as the first thing after the console starts, on every architecture. It
/// says the console works and it says nothing else, which is exactly the claim wanted at first
/// light on a board where the console is the thing in doubt. Its tail is free text: a reader takes
/// the architecture from [`MACHINE`]'s tail.
///
/// Deliberately generic across architectures: a recogniser that knew only one board's wording would
/// report a healthy board of another kind as never having booted.
pub const BANNER: &str = "nife on ";

/// **The machine description finished: `nife machine: <arch>, <n> processor(s), ...`.**
///
/// Printed by the kernel as the **last** line of its machine description, and that position is the point: reaching it means the whole description printed, which is
/// the claim a ladder rung should make. A header would only have meant the block started.
pub const MACHINE: &str = "nife machine: ";

/// **The self-test verdict: `nife self-test: 13 of 13 passed`, `... , 2 absent: <names>`, or
/// `... , 1 FAILED: <names>`.**
///
/// Printed by the userspace self-test program the first program runs before the shell, once it has
/// run its checks on top of the kernel's; the kernel prints it itself only when it has no such
/// program to hand the rest to. [`crate::verdict::Verdict`] renders it. The
/// counts are there on success as well as on failure, so that "ran nothing and passed" is
/// distinguishable from "ran the set and passed".
pub const SELF_TEST: &str = "nife self-test: ";

/// **The head of the line naming a thread the self-test kills on purpose**, before its death is
/// taken: `nife self-test subject: thread 19, which the userspace check kills on purpose`.
///
/// Deliberately not [`SELF_TEST`] with more after it: a matcher that finds the verdict by its head
/// must not find this line. [`crate::verdict::SubjectLine`] renders it and
/// [`crate::verdict::subject_thread`] reads it back.
///
/// Name: provisional (milestone 1063's lane, 2026-11-11 UTC).
pub const SELF_TEST_SUBJECT: &str = "nife self-test subject: thread ";

/// **What a red verdict says before it names the checks that failed.**
///
/// Uppercase, the tree's word for a failed check, which other boot lines also use. It appears
/// *inside* a [`SELF_TEST`] line, after any [`SELF_TEST_ABSENT`] list; a matcher wanting a failed
/// verdict must find both.
pub const SELF_TEST_FAILED: &str = "FAILED: ";

/// **What a verdict says before it names the checks this machine had nothing for**:
/// `nife self-test: 9 of 13 passed, 3 absent: blockdev clock trng`.
///
/// An absent check neither passed nor failed: the machine has no device for it to test, and the
/// kernel's line for that check says which. The verdict stays green unless something says
/// [`SELF_TEST_FAILED`], which always comes after this list when both are present. Whether an
/// absence is acceptable is the board's fact, not the kernel's: `serial_console`'s board profiles
/// name the checks that may not be absent on each board, and read such an absence as a failure.
///
/// Name: provisional, minted 2026-10-20 (UTC) by lane `milestone/890-tours-to-self-test`.
pub const SELF_TEST_ABSENT: &str = "absent: ";

/// **The boot self-test's checks, by name, in the order the verdict names them.**
///
/// One list for every architecture and both halves: the kernel runs the checks of its own
/// internals, and the self-test program the ones that need a user program. Each takes its check's
/// index from here at compile time (`verdict::check`), so an unlisted check does not build, and the
/// verdict counts against this list rather than against what ran: a listed check that did not run
/// fails by name, and a console reader fails a verdict whose total is not this list's length.
/// Each name is unique and spelled in `[a-z0-9_]`, so a reader may split a list of them on
/// whitespace or commas; a `const` assertion below holds that.
///
/// Names provisional: they are printed, and matched, so they are a contract.
/// `device_interrupt` (spelled `deviceirq` in older logs) awaits a ruling, and so does reading
/// those logs.
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
/// | length | the list held |
/// |---|---|
/// | 5 | `exceptions` to `scheduler` |
/// | 13 | the current list without `supervised` and `teardown` |
///
/// Name: provisional (milestone 890 (riscv64 and `x86_64` join the shared boot path)'s lane,
/// 2026-10-20 UTC).
pub const SELF_TEST_CHECKS_EARLIER: &[usize] = &[5, 13];

/// **The boot's middle stages ran to their end**: `nife: the capability core runs on <arch>.`
///
/// Printed by the kernel on every architecture after its own self-test checks, just before the
/// hand-over, so it says the machine was described and the kernel's half of [`SELF_TEST_CHECKS`]
/// ran. The [`SELF_TEST`] verdict follows it, from the hand-over, and says
/// whether they passed. The `<arch>` is the kernel's own `arch::NAME`.
pub const TOUR: &str = "nife: the capability core runs on ";

/// **Userspace is up and the shell is offering a prompt.**
///
/// The shell prints this immediately before its first `$ `, and a period after it. It is the
/// terminal state of a default boot: nothing halts, and the prompt rather than a halt is the signal
/// that the boot finished. Match it whole, without the period.
///
/// See the crate's `BUGS` for why the banner and not the prompt itself.
pub const PROMPT: &str = "nife capability shell";

/// **The kernel is holding the screen open for a host that wants to photograph it**, and will clear
/// it as soon as one byte comes back on the serial line. *Wording provisional, like every other
/// constant here.*
///
/// Not a rung, but a line two binaries agree on: the kernel prints it when its boot command line
/// asks it to hold, and a host that wants a screendump waits for it, takes the dump and answers.
/// A boot that was not asked to hold never prints it. It asks rather than samples, so the host
/// never photographs a half-drawn screen.
pub const SCREEN_HELD: &str = "nife screen: held for the host; ";

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

    const HEADS: [&str; 8] = [
        BANNER,
        MACHINE,
        SELF_TEST,
        SELF_TEST_SUBJECT,
        SELF_TEST_FAILED,
        SELF_TEST_ABSENT,
        TOUR,
        SCREEN_HELD,
    ];

    /// Every head but the shell banner ends in a space or punctuation, the boundary a reader
    /// matches on; the banner is matched whole.
    #[test]
    fn every_head_but_the_shell_banner_ends_on_a_boundary() {
        for head in HEADS {
            let last = head.chars().last().unwrap();
            assert!(
                !last.is_alphanumeric(),
                "{head:?} ends in a letter or digit"
            );
        }
        assert!(PROMPT.chars().last().unwrap().is_alphanumeric());
    }

    /// No head is a substring of another, so a reader that finds one never finds it in a line of a
    /// different rung.
    #[test]
    fn no_head_is_inside_another() {
        for (i, a) in HEADS.iter().enumerate() {
            for (k, b) in HEADS.iter().enumerate() {
                assert!(i == k || !b.contains(a), "{a:?} is inside {b:?}");
            }
            assert!(
                !PROMPT.contains(a) && !a.contains(PROMPT),
                "{a:?} and the banner overlap"
            );
        }
    }

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
