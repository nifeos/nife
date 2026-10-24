//! **Reading a transcript two writers share**: the kernel's fault tokens, the job pool's
//! `free` row, markers found through a shuffle, and the boot claim. See
//! notes/swish-check-harness.md, "Two writers share one UART".
//!
//! Name: provisional (milestone 847's lane, 2026-10-11 UTC; §280 (the swish-check harness
//! splits into submodules, with its script in one)).

/// How many foreign characters [`find_marker`] will step over inside one marker before it gives up.
///
/// The intruder is one kernel fault report, three lines and about 150 characters. 400 is that with
/// room to spare. It is a ceiling and not the thing doing the work: what makes this safe is the
/// **order** the checks run in ([`boot_claim`]), not how generous this number is.
pub(super) const SWISH_CHECK_MARKER_SLACK: usize = 400;

/// Text the **kernel** prints only in a user-fault report, which is the only thing it writes after
/// the userspace console has started: the test for a second writer while the progenitor printed.
///
/// Six short tokens, because any single string can itself be shuffled apart. The first two are a
/// pair, used as one by the killed-thread check and by [`kernel_wrote_during_boot`]: in order they
/// are the aarch64 and riscv64 fault printer's first line (`kernel/src/arch/*/exceptions.rs`), and
/// `user thread ` alone appears in ordinary prose. One array, so the two uses cannot drift apart.
pub(super) const KERNEL_FAULT_TOKENS: [&str; 6] = [
    "user thread ",
    " killed: ",
    "the kernel is fine",
    "stval 0x",
    "esr 0x",
    " sp 0x",
];

/// What `kernel::cap::report_peak` prints, verbatim (milestone 231). The line carries the boot's
/// capability-slot high-water mark against the table's capacity, and `swish-check` both echoes the
/// last one it sees and fails if the kernel flagged it as past the recorded peak.
pub(super) const SLOT_GAUGE: &str = "capability slots:";

/// **The most of the job pool `free`'s `Yours:` line may report in use**, in KiB: two job regions
/// (`system_initializer`'s `JOB_REGION_PAGES`, 48 pages of 4 KiB). One is `free`'s own, which its
/// BUGS says it counts. The other is one predecessor the reaper may not have reached when `free`
/// was carved, a race the gate does not control and so must allow. A reading of 576 is `free` plus
/// two stranded regions. Name: provisional.
pub(super) const YOURS_CEILING_KIB: u64 = 2 * 48 * 4;

/// The used figure of the first `Yours:` row in `transcript`, in KiB, or `None` when there is none
/// (the line's own expectation already fails a run that lost it).
pub(super) fn yours_used_kib(transcript: &str) -> Option<u64> {
    let row = transcript
        .lines()
        .find_map(|l| l.trim_start().strip_prefix("Yours:"))?;
    row.split_whitespace().nth(1)?.parse().ok()
}

/// Where a marker was found in a transcript, and what it cost to find it.
pub(super) enum Marker<'a> {
    /// Present, contiguous. What a boot with one writer gives.
    Exact,
    /// The marker's characters are all present, in order, with someone else's bytes wedged between
    /// them. Carries the intruding text so a caller can name it rather than hide it.
    Interleaved(String),
    /// Not there. Carries the longest prefix of the marker that appears contiguously, which is the
    /// most useful single fact about a transcript that does not have it.
    Absent { matched: &'a str },
}

/// Find `needle` in `haystack`, tolerating another writer's bytes wedged into the middle of it.
///
/// **This is not a safety mechanism and must not be used as one.** It answers "could these
/// characters be this marker, shuffled?", which has false positives by construction: `budget NOT
/// dropped` answers yes for `budget dropped`. [`boot_claim`] makes it safe by asking the
/// un-shuffle-able question first.
pub(super) fn find_marker<'a>(haystack: &str, needle: &'a str) -> Marker<'a> {
    if haystack.contains(needle) {
        return Marker::Exact;
    }
    let text: Vec<char> = haystack.chars().collect();
    let want: Vec<char> = needle.chars().collect();
    let mut best: Option<String> = None;
    for start in 0..text.len() {
        if text[start] != want[0] {
            continue;
        }
        let mut i = 1usize;
        let mut j = start + 1;
        let mut skipped = String::new();
        while i < want.len()
            && j < text.len()
            && skipped.chars().count() <= SWISH_CHECK_MARKER_SLACK
        {
            if text[j] == want[i] {
                i += 1;
            } else {
                skipped.push(text[j]);
            }
            j += 1;
        }
        if i == want.len() && best.as_ref().is_none_or(|b| skipped.len() < b.len()) {
            best = Some(skipped);
        }
    }
    match best {
        Some(skipped) => Marker::Interleaved(skipped),
        None => {
            let mut matched = 0usize;
            for end in (1..=needle.len()).rev() {
                if needle.is_char_boundary(end) && haystack.contains(&needle[..end]) {
                    matched = end;
                    break;
                }
            }
            Marker::Absent {
                matched: &needle[..matched],
            }
        }
    }
}

/// Was the kernel writing the UART **while the progenitor was printing**?
///
/// Only the boot phase counts, which is everything before the shell's banner: a fault after the
/// prompt is out cannot explain a boot line that was already read. The kernel's boot tour is
/// deliberately not among the tokens, because it prints on every boot and a test that is always
/// true would make [`BootClaim::Unreadable`] a way to pass without evidence.
fn kernel_wrote_during_boot(transcript: &str) -> bool {
    let boot = transcript
        .find("nife capability shell")
        .map_or(transcript, |at| &transcript[..at]);
    KERNEL_FAULT_TOKENS.iter().any(|t| boot.contains(t))
}

/// What a transcript says about one thing the progenitor reports on itself.
enum BootClaim {
    /// The progenitor said the thing that is true. Carries the intruding text when it had to be un-shuffled.
    Affirmed(Option<String>),
    /// **The progenitor said the opposite**, contiguously. The failing answer, and the one with teeth.
    Denied,
    /// Neither sentence is readable, and the kernel was writing over the progenitor while it printed. Not a
    /// failure: this check cannot see through a shuffle and should not pretend it can.
    Unreadable { longest_run: String },
    /// Neither sentence is there and nothing else was writing, so nothing shuffled it. The progenitor did not
    /// say this at all, which is a real failure and the one that keeps this check from passing
    /// against a boot that stopped reporting.
    Silent,
}

/// Read one of the progenitor's claims about itself out of a transcript that **two processes wrote at once**.
///
/// Interleaving can destroy a string but not create one, so `negative` is searched exactly and
/// first: present means [`BootClaim::Denied`]. Then `positive`, exact or shuffled, is
/// [`BootClaim::Affirmed`]. Neither, while the kernel wrote during the boot, is
/// [`BootClaim::Unreadable`], a pass the caller reports; neither with one writer is
/// [`BootClaim::Silent`], a failure. This trades false red for false green on purpose; the
/// residual hole is in `script/swish-check`'s BUGS. See notes/swish-check-harness.md, "Two writers
/// share one UART".
fn boot_claim(transcript: &str, positive: &str, negative: &str) -> BootClaim {
    if transcript.contains(negative) {
        return BootClaim::Denied;
    }
    match find_marker(transcript, positive) {
        Marker::Exact => BootClaim::Affirmed(None),
        Marker::Interleaved(skipped) => BootClaim::Affirmed(Some(skipped)),
        Marker::Absent { matched } => {
            if kernel_wrote_during_boot(transcript) {
                BootClaim::Unreadable {
                    longest_run: matched.to_string(),
                }
            } else {
                BootClaim::Silent
            }
        }
    }
}

/// Read the transcript as it stands right now.
///
/// A named function rather than `seen.lock().expect(..).clone()` written inline at each check,
/// because the lock is held for the length of the expression and a check that also wants to format
/// the transcript into a message would otherwise hold it while doing so.
pub(super) fn transcript_now(seen: &std::sync::Arc<std::sync::Mutex<String>>) -> String {
    seen.lock().expect("transcript lock").clone()
}

/// Check one of the progenitor's claims and turn it into a complaint, or `None` if it passed.
///
/// The two passing outcomes both print to stderr when they were not clean, because a transcript
/// that needed un-shuffling is evidence of the UART defect and swallowing it would hide the thing
/// this whole mechanism exists because of.
///
/// `subject` is what the progenitor reports on, in words a reader can act on. It describes the
/// **program's** behavior, which this function can honestly assert, and never the machine's state,
/// which it cannot see.
pub(super) fn boot_claim_complaint(
    transcript: &str,
    subject: &str,
    positive: &str,
    negative: &str,
) -> Option<String> {
    match boot_claim(transcript, positive, negative) {
        BootClaim::Affirmed(None) => None,
        BootClaim::Affirmed(Some(skipped)) => {
            eprintln!(
                "swish-check: read {positive:?} only after stepping over {} characters another \
                 writer had spliced through it. Every byte is present and in order, so this is the \
                 kernel's fault printer and the userspace console sharing the UART, not a lost \
                 read. The intruding text was {skipped:?}",
                skipped.chars().count(),
            );
            None
        }
        BootClaim::Unreadable { longest_run } => {
            eprintln!(
                "swish-check: could not read the progenitor's report on {subject}. Neither sentence survives \
                 in the transcript (the longest run of the affirmative one that does is \
                 {longest_run:?}), AND the kernel printed a fault report during the boot, so two \
                 processes were writing the UART at once and the line cannot be recovered. NOT \
                 treated as a failure, because this check cannot see through a byte-level shuffle. \
                 A real regression prints the negative sentence on every boot and is caught by the \
                 exact search for it above.",
            );
            None
        }
        BootClaim::Denied => Some(format!(
            "The progenitor reported the failing answer on {subject}: the transcript contains {negative:?}, \
             contiguously. Interleaving can destroy a string and cannot create one, so the progenitor printed \
             this."
        )),
        BootClaim::Silent => Some(format!(
            "the transcript carries neither of the progenitor's two sentences about {subject} ({positive:?} \
             nor {negative:?}), and nothing else was writing the UART during the boot, so nothing \
             shuffled them. The progenitor did not report this at all. That is what this check knows; it \
             reads strings out of a transcript and asserts nothing about the kernel. The full \
             transcript is below."
        )),
    }
}

#[cfg(test)]
mod tests;
