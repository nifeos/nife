//! **The heads of the boot's console lines**, each the system's name followed by a fixed shape.
//!
//! A system builds one [`Heads`] from its name, and the program that prints a line and every
//! program that reads it take the line's [`Head`] from it, so both sides hold the same bytes. A
//! [`Head`] is two borrowed pieces, the name and the shape, and nothing here allocates: it prints
//! through `Display` and matches a line in place.
//!
//! Name: provisional. A child named for what it holds, so the crate root is an index (§282 (when a
//! file is split, the parent becomes an index)).

use core::fmt;
use core::ops::Range;

/// **Every head a system prints, built from the system's name.**
///
/// The shapes are fixed here; only the name before them is the caller's. With the name `example`,
/// [`banner`](Heads::banner) is `example on ` and [`self_test`](Heads::self_test) is
/// `example self-test: `.
///
/// Name: provisional.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Heads<'a> {
    system: &'a str,
}

impl<'a> Heads<'a> {
    /// The heads of the system called `system`, the word every one of its heads begins with.
    ///
    /// Name: provisional.
    pub const fn new(system: &'a str) -> Self {
        Self { system }
    }

    const fn head(&self, shape: &'static str) -> Head<'a> {
        Head {
            system: self.system,
            shape,
        }
    }

    /// **The earliest line: `<system> on <architecture> (...)`.**
    ///
    /// Printed by the kernel as the first thing after the console starts, on every architecture.
    /// It says the console works and it says nothing else, which is exactly the claim wanted at
    /// first light on a board where the console is the thing in doubt. Its tail is free text: a
    /// reader takes the architecture from [`machine`](Heads::machine)'s tail.
    ///
    /// Deliberately generic across architectures: a recogniser that knew only one board's wording
    /// would report a healthy board of another kind as never having booted.
    ///
    /// Name: provisional, minted 2026-11-14 (UTC) by lane `milestone/1078-console-heads-name-the-system`.
    pub const fn banner(&self) -> Head<'a> {
        self.head(" on ")
    }

    /// **The machine description finished: `<system> machine: <arch>, <n> processor(s), ...`.**
    ///
    /// Printed by the kernel as the **last** line of its machine description, and that position is
    /// the point: reaching it means the whole description printed, which is the claim a ladder rung
    /// should make. A header would only have meant the block started.
    ///
    /// Name: provisional, minted 2026-11-14 (UTC) by lane `milestone/1078-console-heads-name-the-system`.
    pub const fn machine(&self) -> Head<'a> {
        self.head(" machine: ")
    }

    /// **The self-test verdict: `<system> self-test: 13 of 13 passed`, `... , 2 absent: <names>`,
    /// or `... , 1 FAILED: <names>`.**
    ///
    /// Printed by the userspace self-test program once it has run its checks on top of the
    /// kernel's; the kernel prints it itself only when it has no such program to hand the rest to.
    /// [`crate::verdict::Verdict`] renders it. The counts are there on success as well as on
    /// failure, so that "ran nothing and passed" is distinguishable from "ran the set and passed".
    ///
    /// Name: provisional, minted 2026-11-14 (UTC) by lane `milestone/1078-console-heads-name-the-system`.
    pub const fn self_test(&self) -> Head<'a> {
        self.head(" self-test: ")
    }

    /// **The head of the line naming a thread the self-test kills on purpose**, before its death
    /// is taken: `<system> self-test subject: thread 19, which the userspace check kills on
    /// purpose`.
    ///
    /// Deliberately not [`self_test`](Heads::self_test) with more after it: a matcher that finds
    /// the verdict by its head must not find this line. [`crate::verdict::SubjectLine`] renders it
    /// and [`crate::verdict::subject_thread`] reads it back.
    ///
    /// Name: provisional, minted 2026-11-14 (UTC) by lane `milestone/1078-console-heads-name-the-system`.
    pub const fn self_test_subject(&self) -> Head<'a> {
        self.head(" self-test subject: thread ")
    }

    /// **The boot's middle stages ran to their end**: `<system>: the capability core runs on
    /// <arch>.`
    ///
    /// Printed by the kernel on every architecture after its own self-test checks, just before it
    /// hands over to the first program, so it says the machine was described and the kernel's half
    /// of [`SELF_TEST_CHECKS`](crate::SELF_TEST_CHECKS) ran. The
    /// [`self_test`](Heads::self_test) verdict follows it and says whether they passed.
    ///
    /// Name: provisional, minted 2026-11-14 (UTC) by lane `milestone/1078-console-heads-name-the-system`.
    pub const fn tour(&self) -> Head<'a> {
        self.head(": the capability core runs on ")
    }

    /// **Userspace is up and the shell is offering a prompt.**
    ///
    /// The shell prints this immediately before its first `$ `, and a period after it. It is the
    /// terminal state of a default boot: nothing halts, and the prompt rather than a halt is the
    /// signal that the boot finished. Match it whole, without the period.
    ///
    /// See the crate's `BUGS` for why the banner and not the prompt itself.
    ///
    /// Name: provisional, minted 2026-11-14 (UTC) by lane `milestone/1078-console-heads-name-the-system`.
    pub const fn prompt(&self) -> Head<'a> {
        self.head(" capability shell")
    }

    /// **The kernel is holding the screen open for a host that wants to photograph it**, and will
    /// clear it as soon as one byte comes back on the serial line.
    ///
    /// Not a rung, but a line two binaries agree on: the kernel prints it when its boot command
    /// line asks it to hold, and a host that wants a screendump waits for it, takes the dump and
    /// answers. A boot that was not asked to hold never prints it.
    ///
    /// Name: provisional, minted 2026-11-14 (UTC) by lane `milestone/1078-console-heads-name-the-system`.
    pub const fn screen_held(&self) -> Head<'a> {
        self.head(" screen: held for the host; ")
    }
}

/// **One head: the system's name, then the line's fixed shape.**
///
/// Prints as the two pieces back to back. A reader matches it with [`find_in`](Head::find_in),
/// [`is_in`](Head::is_in) or [`strip_prefix_of`](Head::strip_prefix_of), each of which answers
/// exactly as the same `str` method would for the two pieces joined.
///
/// Name: provisional.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Head<'a> {
    system: &'a str,
    shape: &'static str,
}

impl<'a> Head<'a> {
    /// **The byte range of the first place `text` holds this head**, or `None` when it holds none.
    /// The tail a reader parses starts at the range's end.
    ///
    /// Name: provisional.
    pub fn find_in(&self, text: &str) -> Option<Range<usize>> {
        let mut from = 0;
        while let Some(found) = text.get(from..)?.find(self.system) {
            let start = from + found;
            let after = start + self.system.len();
            if text[after..].starts_with(self.shape) {
                return Some(start..after + self.shape.len());
            }
            // The next candidate starts one character on, so overlapping names are all tried.
            from = start + text[start..].chars().next()?.len_utf8();
        }
        None
    }

    /// Whether `text` holds this head anywhere.
    ///
    /// Name: provisional.
    pub fn is_in(&self, text: &str) -> bool {
        self.find_in(text).is_some()
    }

    /// **`text` after this head**, when `text` begins with it, or `None`.
    ///
    /// Name: provisional.
    pub fn strip_prefix_of<'t>(&self, text: &'t str) -> Option<&'t str> {
        text.strip_prefix(self.system)?.strip_prefix(self.shape)
    }

    /// **The two pieces, in the order they print**: the system's name, then the shape. For a
    /// printer with no formatter, which writes each in turn.
    ///
    /// Name: provisional.
    pub const fn parts(&self) -> [&'a str; 2] {
        [self.system, self.shape]
    }
}

impl fmt::Display for Head<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.system)?;
        f.write_str(self.shape)
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::format;
    use std::string::String;

    use super::*;

    const HEADS: Heads<'static> = Heads::new("example");

    fn all(heads: Heads<'_>) -> [Head<'_>; 7] {
        [
            heads.banner(),
            heads.machine(),
            heads.self_test(),
            heads.self_test_subject(),
            heads.tour(),
            heads.prompt(),
            heads.screen_held(),
        ]
    }

    /// Each head prints the caller's name and then its shape.
    #[test]
    fn a_head_prints_the_system_name_then_its_shape() {
        let printed = all(HEADS).map(|h| format!("{h}"));
        assert_eq!(
            printed,
            [
                "example on ",
                "example machine: ",
                "example self-test: ",
                "example self-test subject: thread ",
                "example: the capability core runs on ",
                "example capability shell",
                "example screen: held for the host; ",
            ]
        );
        for head in all(HEADS) {
            assert_eq!(head.parts().concat(), format!("{head}"));
        }
    }

    /// The three matchers answer as `str`'s methods do for the joined head, including where a
    /// name overlaps itself and where the name appears earlier without its shape.
    #[test]
    fn the_three_readers_agree_with_the_joined_string() {
        let texts = [
            "",
            "example on riscv64",
            "  example on riscv64",
            "example example on x",
            "exampleexample self-test: 3 of 3 passed",
            "\u{fffd}example machine: aarch64, 4 processor(s)",
            "example: the capability core runs on x86_64.",
            "example capability shell. naming a resource",
            "example self-test subject: thread 19, which the userspace check kills on purpose",
            "example screen: held for the host; send any byte",
            "nothing here",
            "example",
        ];
        for system in ["example", "aa", "a", ""] {
            let heads = Heads::new(system);
            for head in all(heads) {
                let joined: String = format!("{head}");
                for base in texts {
                    for text in [String::from(base), base.replace("example", system)] {
                        let want = text.find(&joined).map(|at| at..at + joined.len());
                        assert_eq!(head.find_in(&text), want, "{joined:?} in {text:?}");
                        assert_eq!(head.is_in(&text), text.contains(&joined));
                        assert_eq!(head.strip_prefix_of(&text), text.strip_prefix(&joined));
                    }
                }
            }
        }
        let doubled = Heads::new("aa").banner();
        assert_eq!(doubled.find_in("aaa on "), Some(1..7));
    }

    /// Every head but the shell banner ends in a space or punctuation, the boundary a reader
    /// matches on; the banner is matched whole.
    #[test]
    fn every_head_but_the_shell_banner_ends_on_a_boundary() {
        for head in all(HEADS) {
            let last = format!("{head}").chars().last().unwrap();
            assert_eq!(
                last.is_alphanumeric(),
                head == HEADS.prompt(),
                "{head:?} ends wrongly"
            );
        }
    }

    /// No head is inside another, so a reader that finds one never finds it in another rung's line.
    #[test]
    fn no_head_is_inside_another() {
        for (i, a) in all(HEADS).iter().enumerate() {
            for (k, b) in all(HEADS).iter().enumerate() {
                assert!(i == k || !a.is_in(&format!("{b}")), "{a} is inside {b}");
            }
        }
    }
}
