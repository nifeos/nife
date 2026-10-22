//! **A board's firmware prologue, declared rather than hard-coded** (milestone 324 part 3).
//!
//! calef ruled on 2026-09-19: *one tool with a board profile, and the profile is the firmware
//! prologue only*. This module is the part that is **not** shared, written as data instead of as a
//! run of `if line.contains(...)` inside [`crate::progress`]. The lower rungs a board's firmware
//! prints (on the VisionFive 2: `U-Boot SPL`, OpenSBI, U-Boot proper, `Starting kernel ...`) are a
//! `&'static Profile`; the kernel's own ladder (banner, machine, self-test, tour, prompt, and the
//! workloads above them), the same on all three architectures, is [`crate::progress::Stage`].
//! Adding a board is adding a [`Profile`].
//!
//! # What a profile may and may not contain
//!
//! Only what the **firmware** prints, before the kernel has run an instruction:
//!
//! - [`Profile::prologue`], the ordered rungs. Order is the whole of their meaning.
//! - [`Profile::refusals`], the ways that firmware gives up before handing over
//!   (`Bad Linux RISCV Image magic!` is U-Boot's, so it belongs to the board, not the shared half).
//! - [`Profile::relocation`], a discriminator that is deliberately not a rung: `Moving Image from`
//!   answers what to check when `Starting kernel ...` is followed by silence
//!   (`notes/visionfive2.md`), a different question from how far the boot got.
//!
//! Everything the kernel prints stays out; that is `crates/boot_ladder`'s.
//!

//! # BUGS
//!
//! - **Only [`RADON`] has been checked against a machine** (bytes off the wire in
//!   `tests/fixtures/captured/`). [`XENON`]'s empty prologue is a measurement of one capture: it
//!   boots through PVH straight into `nife on `, and a xenon that fell over inside its firmware
//!   would print something nothing here names.
//! - **argon has no profile** until it boots nife and prints something on a wire (milestone 127
//!   (the seL4 machine)); one written from vendor documentation would be an assertion shaped as a
//!   measurement.
//! - **The default is [`RADON`]**, so an unconfigured session says it expected a VisionFive 2. The
//!   QEMU callers in `xtask` (`boot-check`, `soak-test`, `job-mix`) name [`XENON`], a machine with
//!   no firmware prologue.
//! - **A [`Rung`] compares by depth alone**, so rung 2 of one profile equals rung 2 of another. A
//!   session has exactly one profile, chosen before any byte is read, so they never meet.
//! - **Nothing gates a profile against the board it claims to describe** beyond review and a
//!   capture in `tests/fixtures/captured/` for every rung anybody asserts on.
//!
//! Names provisional, this lane's coinage (2026-09-19, milestone 324 part 3). `Profile` is the word
//! calef's own ruling used (*"a board profile"*); `Rung` is `crates/boot_ladder`'s word for the
//! same shape one level up; `Sign` was chosen over `Match` (a verb) and `Pattern` (which promises a
//! regular expression).

use core::cmp::Ordering;

/// **What text means a rung was reached.**
///
/// Two kinds, and the second exists for exactly one real ambiguity rather than for generality:
/// `U-Boot SPL 2021.10` and `U-Boot 2021.10` differ by one word, and reading the first as the
/// second reports a board that died in SPL two rungs further along than it got.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sign {
    /// The text appears anywhere in the line.
    ///
    /// `contains` rather than `starts_with` throughout, because a console log interleaves output
    /// from more than one stage and a line can arrive with a hart prefix or a partial line glued
    /// to its front.
    Text(&'static str),
    /// `prefix` appears, and the word after it is none of `not`.
    ///
    /// Only settles on a **complete** line. In a partial one the word after the prefix may not have
    /// finished arriving, and an unterminated word cannot be compared to `SPL`; the honest answer
    /// there is "not yet".
    WordAfter {
        /// The text to find, including its trailing space.
        prefix: &'static str,
        /// The words that mean this is a different announcement.
        not: &'static [&'static str],
    },
}

impl Sign {
    /// Is this sign present in `line`? `complete` says whether the line has finished arriving.
    #[must_use]
    pub(crate) fn is_seen_in(&self, line: &str, complete: bool) -> bool {
        match self {
            Sign::Text(text) => line.contains(text),
            Sign::WordAfter { prefix, not } => {
                let Some(at) = line.find(prefix) else {
                    return false;
                };
                let rest = &line[at + prefix.len()..];
                let word = match rest.find(char::is_whitespace) {
                    Some(end) => &rest[..end],
                    // No whitespace after it: the word is finished only if the line is.
                    None if complete => rest,
                    None => return false,
                };
                !word.is_empty() && !not.contains(&word)
            }
        }
    }
}

/// **One rung of a board's firmware prologue.**
///
/// [`depth`](Self::depth) is the whole of the ordering and the whole of the identity; see this
/// module's `BUGS` for why that is an accepted exception rather than an oversight.
#[derive(Debug, Clone, Copy, Eq)]
pub struct Rung {
    depth: u8,
    key: &'static str,
    label: &'static str,
    signs: &'static [Sign],
}

impl Rung {
    /// Declare a rung. `depth` is its position in the profile, counted from one.
    ///
    /// `key` is the word a person types at `--until`; `label` is what a report calls it. They are
    /// separate because one is an interface and the other is prose: `uboot` is not a thing anyone
    /// wants to read in a sentence, and `U-Boot` is not a thing anyone wants to type.
    #[must_use]
    pub const fn new(
        depth: u8,
        key: &'static str,
        label: &'static str,
        signs: &'static [Sign],
    ) -> Self {
        Self {
            depth,
            key,
            label,
            signs,
        }
    }

    /// Position in the profile's prologue, counted from one.
    #[must_use]
    pub const fn depth(&self) -> u8 {
        self.depth
    }

    /// The word a person types at `--until` to wait for this rung.
    #[must_use]
    pub const fn key(&self) -> &'static str {
        self.key
    }

    /// What a report calls this rung.
    #[must_use]
    pub const fn label(&self) -> &'static str {
        self.label
    }

    /// Does this line say the rung was reached? `complete` says whether the line has finished.
    #[must_use]
    pub(crate) fn is_seen_in(&self, line: &str, complete: bool) -> bool {
        self.signs
            .iter()
            .any(|sign| sign.is_seen_in(line, complete))
    }
}

// Depth alone, for both. Ord and Eq have to agree or a `Rung` in an ordered collection misbehaves,
// and depth is the only field whose comparison means anything: a rung's label and signs are how it
// is recognised, not which rung it is.
impl PartialEq for Rung {
    fn eq(&self, other: &Self) -> bool {
        self.depth == other.depth
    }
}

impl PartialOrd for Rung {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Rung {
    fn cmp(&self, other: &Self) -> Ordering {
        self.depth.cmp(&other.depth)
    }
}

/// **A way the firmware gives up before the kernel runs.**
///
/// A positive statement printed by something that is still alive. A board that has gone silent is
/// not in here; silence is the watcher's business, not the recogniser's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Refusal {
    /// The text that means it refused.
    pub marker: &'static str,
    /// One line naming what went wrong, for the report and the exit message.
    ///
    /// Written as the whole sentence rather than as a code, because the report is read by somebody
    /// at a bench deciding what to do next and a token would send them back to the source.
    pub diagnosis: &'static str,
    /// Whether the line **before** this one carries the reason.
    ///
    /// U-Boot's `### ERROR ###` says that it gave up and the line before it says why, so a reader
    /// handed only the first half has to go back to the log to learn anything.
    pub reason_is_the_line_before: bool,
}

/// **One board's firmware prologue**, and nothing else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Profile {
    /// The board this describes, as this project names it.
    pub name: &'static str,
    /// The rungs, in the order the firmware climbs them. Declared shallowest first; nothing
    /// enforces that beyond a test in this module, because the depths are written down beside the
    /// rungs where a reader meets them.
    pub prologue: &'static [Rung],
    /// How this firmware announces that it has given up.
    pub refusals: &'static [Refusal],
    /// Text meaning the firmware moved the payload, which is a discriminator and not a rung.
    pub relocation: &'static [&'static str],
    /// **The self-test checks this board has the device for**, so an `absent` on one of them is a
    /// failure (calef's ruling of 2026-10-20 (UTC) on #2447). The kernel cannot tell a machine with
    /// no TRNG from a board whose TRNG went missing; the profile can, because it describes one
    /// board. Names from `boot_ladder::SELF_TEST_CHECKS`, checked by a test in this module.
    pub must_not_be_absent: &'static [&'static str],
}

impl Profile {
    /// The rung a person means by `key`, or `None` if this board has no such rung.
    ///
    /// **`None` is the useful half.** `--until spl` against a board with no SPL is a request that
    /// can never be satisfied, and answering it with a refusal beats watching a board for two
    /// minutes and then reporting that the time ran out.
    #[must_use]
    pub fn rung(&self, key: &str) -> Option<&'static Rung> {
        self.prologue.iter().find(|rung| rung.key == key)
    }

    /// Every `--until` word this board understands, for an error message that names them.
    pub fn keys(&self) -> impl Iterator<Item = &'static str> {
        self.prologue.iter().map(|rung| rung.key)
    }
}

/// **radon, a `StarFive` VisionFive 2** (JH7110), and the only profile checked against a machine.
///
/// Every marker was first quoted from `notes/visionfive2.md`'s bench runbook and then checked
/// against bytes off the wire on 2026-09-01; the captures are in `tests/fixtures/captured/` and the
/// tests assert on them. See `notes/board-console.md` for the table of where each one came from.
pub(crate) static RADON: Profile = Profile {
    name: "radon",
    prologue: &[
        // DRAM and the PLLs are up and we are running out of SRAM.
        Rung::new(1, "spl", "U-Boot SPL", &[Sign::Text("U-Boot SPL")]),
        // OpenSBI's banner block ends with a "Platform Name" table, but the version line is the one
        // the runbook says to record, and it is the only line guaranteed to carry the word followed
        // by a version.
        Rung::new(2, "opensbi", "OpenSBI", &[Sign::Text("OpenSBI v")]),
        // Two signs, because U-Boot announces itself twice in two shapes: its banner, and the
        // prompt it offers if the countdown is interrupted. The banner needs the word test; the
        // prompt is unambiguous text.
        Rung::new(
            3,
            "uboot",
            "U-Boot",
            &[
                Sign::WordAfter {
                    prefix: "U-Boot ",
                    not: &["SPL", "TPL"],
                },
                Sign::Text("StarFive #"),
            ],
        ),
        // U-Boot has handed over and everything after this is ours.
        Rung::new(
            4,
            "handoff",
            "kernel handoff",
            &[Sign::Text("Starting kernel ...")],
        ),
    ],
    refusals: &[
        Refusal {
            marker: "Bad Linux RISCV Image magic!",
            diagnosis: "U-Boot rejected the image header (Bad Linux RISCV Image magic!)",
            reason_is_the_line_before: false,
        },
        Refusal {
            marker: "### ERROR ### Please RESET the board ###",
            diagnosis: "U-Boot gave up before the kernel ran and wants the board reset",
            reason_is_the_line_before: true,
        },
    ],
    relocation: &["Moving Image from"],
    // The JH7110 has a TRNG and the clock controller that gates it, and the TRNG is this board's
    // entropy source: an absence of any of the three is a board that lost a device.
    must_not_be_absent: &["entropy", "trng", "clock"],
};

/// **xenon, whose firmware prologue is empty, and that is a measurement.**
///
/// It boots through PVH straight into `nife on `, so the portable half of the ladder is its whole
/// boot: `bench/xenon-2026-09-17/first-light-095500.log` is the capture, and a `--replay` of it
/// reports the banner, the machine line, the five-of-five self-test and the measured-boot refusal
/// with the right diagnosis without a single rung of prologue. Nothing was added for xenon when
/// that capture was taken, and this profile is that fact written down rather than a placeholder.
///
/// See this module's `BUGS`: one capture on one day is what stands behind the emptiness.
pub static XENON: Profile = Profile {
    name: "xenon",
    prologue: &[],
    refusals: &[],
    relocation: &[],
    // RDSEED and an IO APIC with a PIT behind it, on any PC this port runs on.
    must_not_be_absent: &["entropy", "deviceirq"],
};

/// **An emulator: no firmware prologue, and no device it must have.** `boot-check`'s and the soak's
/// QEMU legs, whose machines attach a disk, a virtio-rng or neither depending on the run, so every
/// device check may be absent there and the board profiles' lists would be wrong for them.
///
/// Name: provisional, minted 2026-10-20 (UTC) by lane `milestone/890-tours-to-self-test`.
pub static EMULATOR: Profile = Profile {
    name: "qemu",
    prologue: &[],
    refusals: &[],
    relocation: &[],
    must_not_be_absent: &[],
};

/// The boards a person may name at `--board`, in the order an error message lists them.
pub static PROFILES: &[&Profile] = &[&RADON, &XENON, &EMULATOR];

/// The profile a person means by `name`, or `None`.
#[must_use]
pub fn profile(name: &str) -> Option<&'static Profile> {
    PROFILES.iter().copied().find(|p| p.name == name)
}

#[cfg(test)]
mod tests {

    /// Every check a profile forbids to be absent is one the kernel runs: a misspelled name here
    /// would forbid nothing, silently.
    #[test]
    fn every_forbidden_absence_names_a_real_check() {
        for profile in PROFILES {
            for name in profile.must_not_be_absent {
                assert!(
                    boot_ladder::SELF_TEST_CHECKS.contains(name),
                    "{}'s must_not_be_absent names {name}, which boot_ladder does not list",
                    profile.name
                );
            }
        }
    }
    use super::*;

    /// **A rung built at run time reads back what it was given, and compares by depth alone.** The
    /// profiles build every rung in a `const`, so nothing else runs [`Rung::new`], [`Rung::key`] or
    /// [`Rung::label`] outside a failing assertion's message; and ordering by depth while ignoring
    /// label and signs is the module's stated identity rule, which only a comparison can check.
    #[test]
    fn a_rung_reads_back_its_fields_and_orders_by_depth_alone() {
        let first = Rung::new(1, "spl", "U-Boot SPL", &[Sign::Text("U-Boot SPL")]);
        let second = Rung::new(2, "uboot", "U-Boot", &[Sign::Text("StarFive #")]);
        assert_eq!(first.key(), "spl");
        assert_eq!(first.label(), "U-Boot SPL");
        assert_eq!(first.depth(), 1);
        assert!(first < second);
        assert_eq!(first.partial_cmp(&second), Some(Ordering::Less));
        let relabelled = Rung::new(1, "other", "Other", &[]);
        assert_eq!(first, relabelled);
        assert!(first.is_seen_in("U-Boot SPL 2021.10", true));
        assert!(!relabelled.is_seen_in("U-Boot SPL 2021.10", true));
        // `eq` is depth alone, and `<` above already goes through `partial_cmp` rather than
        // `eq`, so this is the one assertion in the module that actually calls it.
        assert_ne!(first, second, "different depths must not compare equal");
    }

    /// The depths are written by hand beside each rung, so something has to say they are the
    /// positions they claim to be: a duplicate or a gap would make the ratchet compare two rungs
    /// as equal and silently stop moving.
    #[test]
    fn every_profiles_depths_count_from_one_without_gaps() {
        for profile in PROFILES {
            for (i, rung) in profile.prologue.iter().enumerate() {
                assert_eq!(
                    usize::from(rung.depth()),
                    i + 1,
                    "{}'s rung {} declares depth {}",
                    profile.name,
                    rung.key(),
                    rung.depth()
                );
            }
        }
    }

    /// The `--until` words have to be distinct within a board, or `rung()` answers whichever was
    /// written first and a person gets a rung they did not ask for.
    #[test]
    fn a_boards_rung_keys_are_distinct() {
        for profile in PROFILES {
            let mut seen: Vec<&str> = Vec::new();
            for key in profile.keys() {
                assert!(
                    !seen.contains(&key),
                    "{} repeats --until {key}",
                    profile.name
                );
                seen.push(key);
            }
        }
    }

    /// The distinction the whole prologue turns on, and the one a `contains("U-Boot")` gets wrong.
    /// Kept here beside the declaration as well as in `progress`, because this is where somebody
    /// adding a board will copy the shape from.
    #[test]
    fn spl_does_not_satisfy_the_u_boot_rung() {
        let uboot = RADON.rung("uboot").expect("radon has a U-Boot rung");
        assert!(!uboot.is_seen_in("U-Boot SPL 2021.10 (Feb 12 2023 - 20:24:34 +0800)", true));
        assert!(uboot.is_seen_in("U-Boot 2021.10 (Feb 12 2023 - 20:24:34 +0800)", true));
        assert!(
            uboot.is_seen_in("StarFive # ", false),
            "the prompt has no newline"
        );
        // Mid-word, the line is not yet evidence of anything: the next three bytes may be `SPL`.
        assert!(!uboot.is_seen_in("U-Boot ", false));
        // A *complete* line with nothing after the prefix and no trailing whitespace: the word
        // is the rest of the line, which is only true because the line has finished arriving.
        assert!(
            uboot.is_seen_in("U-Boot 2021.10", true),
            "a complete line settles a word even with no whitespace after it"
        );
    }

    /// A board with no prologue answers every `--until` firmware word with `None`, which is what
    /// turns `--until spl` against xenon into a refusal rather than a two-minute wait.
    #[test]
    fn a_board_without_a_prologue_knows_no_firmware_rungs() {
        assert!(XENON.rung("spl").is_none());
        assert!(XENON.rung("uboot").is_none());
        assert_eq!(XENON.keys().count(), 0);
    }

    /// A board **with** a prologue names its rungs, which `XENON.keys().count() == 0` above
    /// cannot tell apart from `keys()` always returning nothing.
    #[test]
    fn a_boards_keys_are_its_rungs_in_order() {
        assert_eq!(
            RADON.keys().collect::<Vec<_>>(),
            vec!["spl", "opensbi", "uboot", "handoff"]
        );
    }

    #[test]
    fn a_board_is_found_by_the_name_this_project_calls_it() {
        assert_eq!(profile("radon"), Some(&RADON));
        assert_eq!(profile("xenon"), Some(&XENON));
        assert_eq!(profile("argon"), None, "argon is deferred, not profiled");
    }
}
