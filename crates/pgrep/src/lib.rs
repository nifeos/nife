//! **`pgrep`: a domain names its members, and that is all naming buys you** (notes/process-view.md).
//! This is the filter, lifted out so it runs on the host; `components/src/pgrep.rs` is the syscall
//! and the output. It takes a finished [`ps::Survey`] from [`ps::collect`] rather than walking,
//! because two implementations of `SURVEY`'s resume protocol could disagree.
//!
//! **There is no `pkill`, and the ABI is why.** A survey returns a tid, a tid is a name, and
//! [`abi::thread_control_block`] has no `DESTROY`: killing a child is `DESTROY` on the region it was
//! built from, held by whoever spawned it (§24 (interrupting the foreground process)). Nothing this
//! program prints carries authority over what it printed.
//!
//! Four answers, kept apart because Unix collapses the last two into "prints nothing, exits 1":
//!
//! | what happened | output stream | diagnostics |
//! |---|---|---|
//! | members matched | their tids, one per line | silent |
//! | the domain has members, none matched | nothing | `pgrep: none of the 3 ...` |
//! | the domain is empty | nothing | `pgrep: this domain holds no processes` |
//! | the walk was refused | nothing | `pgrep: this endpoint may be sent to, but not looked at ...` |
//!
//! The refusal wording is [`ps::Survey::complaint`]'s, so the two programs cannot drift. With no
//! process names in this system, the pattern matches a member's run state (`pgrep dead`,
//! `pgrep 'r*'`), and [`Selector::from_pattern`] resolves it into a bitmask before it crosses a
//! process boundary, so the program is handed a mask rather than text.
//!
//! # BUGS
//!
//! - The selector cannot be typed at the prompt: a command line delivers capabilities and a few
//!   words, never bytes, so the shipped `pgrep` is `pgrep '*'` until `ProgramArgumentDeclaration` gains positional
//!   arguments.
//! - It is glob, not an extended regex (no regex engine here, §46 (thin primitives or whole
//!   subsystems)), so `pgrep 'r*'` works and `pgrep 'r.*'` does not; matched with
//!   `glob::Dot::Ordinary`, since a state name is not a filename.
//! - No exit status: "nothing matched" is a diagnostic sentence, since `exit` takes no code.
//! - "none of the N processes" counts the domain as this walk saw it; a member born mid-walk is not
//!   in it.
//! - The survey cursor must never be exposed here: it leaks thread-table density.
//!
//! Name: ratified 2026-08-23 (calef): upstream's name, shared with `components/src/pgrep.rs` as
//! one thing split at the IO boundary. `Selector` is provisional.

#![cfg_attr(not(test), no_std)]

/// **A state's bit in a [`Selector`] mask is one shifted left by its `abi::survey` code**, which is
/// why there are four of them and why bit 0 is unused: `abi::survey::DONE` is zero and is the cursor
/// sentinel rather than a state a thread can be in.
///
/// Deriving the bit from the code rather than numbering the bits separately means a state added to
/// `abi::survey` cannot be given a bit that already belongs to another one.
pub mod state {
    /// On a run queue, waiting for a CPU.
    pub const READY: u64 = 1 << abi::survey::READY;
    /// On a CPU right now.
    pub const RUNNING: u64 = 1 << abi::survey::RUNNING;
    /// Blocked in an IPC rendezvous that has not happened.
    pub const BLOCKED: u64 = 1 << abi::survey::BLOCKED;
    /// A corpse, waiting for its supervisor to collect it (DECISIONS §26).
    pub const DEAD: u64 = 1 << abi::survey::DEAD;

    /// Every state a supervised thread can be found in. Four, not six: `abi::survey`'s own
    /// documentation says why `Embryo` and `Finished` cannot appear in a domain.
    pub const EVERY: u64 = READY | RUNNING | BLOCKED | DEAD;
}

/// The `abi::survey` codes a selector can name, in the order a listing reports them and a sentence
/// says them.
const STATES: [u64; 4] = [
    abi::survey::READY,
    abi::survey::RUNNING,
    abi::survey::BLOCKED,
    abi::survey::DEAD,
];

/// **Which members of a domain are wanted.** A mask over [`state`]'s bits, and nothing else: a
/// selector cannot name a tid, a parent, a user or a terminal, because a domain does not know any of
/// those.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Selector(u64);

impl Selector {
    /// Every member of the domain. What the prompt gets today, since it cannot spell a selector.
    pub const EVERY: Selector = Selector(state::EVERY);

    /// No member at all. Reachable only from a [`from_pattern`](Selector::from_pattern) that no
    /// state name matched, and it is a **fourth answer** rather than a synonym for `EVERY`: see
    /// [`from_wire`](Selector::from_wire).
    pub const NONE: Selector = Selector(0);

    /// Exactly these [`state`] bits. Bits outside [`state::EVERY`] are dropped rather than carried,
    /// so a mask from a stranger cannot make [`selects`](Selector::selects) answer about a state
    /// this system does not have.
    pub const fn of(mask: u64) -> Selector {
        Selector(mask & state::EVERY)
    }

    /// **The mask as a spawner delivers it, where zero means every state.**
    ///
    /// This is the one place zero is generous, and the asymmetry with [`of`](Selector::of) is
    /// deliberate. A program is started with its registers zeroed by whoever did not think about
    /// them (`system_initializer` starts every child `start_child(child, 0, arg, 0)` and the shell
    /// leaves `arg` at zero for a program whose `ProgramArgumentDeclaration` is `Forbidden`), so "nobody said" has to
    /// mean "no filter" or `pgrep` would print nothing at the prompt and look broken.
    ///
    /// A **pattern** that matched no state must not collapse into the same value, because that
    /// would turn a selector naming nothing into a selector naming everything: the one direction to
    /// be wrong in that a monitor cannot recover from. So [`from_pattern`](Selector::from_pattern)
    /// goes through [`of`](Selector::of) and can return [`NONE`](Selector::NONE), which
    /// [`Selection::write_diagnostics`] reports as its own sentence.
    pub const fn from_wire(mask: u64) -> Selector {
        if mask == 0 {
            Selector::EVERY
        } else {
            Selector::of(mask)
        }
    }

    /// **The states whose names this glob pattern matches.**
    ///
    /// `dead` is one state, `r*` is ready and running, `*` is all four, and `zzz` is
    /// [`NONE`](Selector::NONE). The names are `ps::state_name`'s, so the word a person types is the
    /// word `ps` printed in the `STATE` column and there is no second vocabulary to learn.
    ///
    /// `glob::Dot::Ordinary`, because a state name never begins with a dot and the leading-dot rule
    /// is a filename-listing convention (see this module's `BUGS`).
    pub fn from_pattern(pattern: &[u8]) -> Selector {
        let mut mask = 0u64;
        for code in STATES {
            if glob::matches_with(
                pattern,
                ps::state_name(code).as_bytes(),
                glob::Dot::Ordinary,
            ) {
                mask |= 1 << code;
            }
        }
        Selector::of(mask)
    }

    /// The raw mask, for a spawner that has to put it in a register.
    pub const fn mask(self) -> u64 {
        self.0
    }

    /// **Does this selector name a thread in `state`?** False for any code outside `abi::survey`'s
    /// four, including a state code from a kernel newer than this program: an unknown state is not
    /// silently swept into a match, because `pgrep dead` returning a live thread is the failure this
    /// program has available.
    pub const fn selects(self, state: u64) -> bool {
        state < 64 && self.0 & (1 << state) != 0
    }

    /// **Can this selector name anything at all?** True only for [`NONE`](Selector::NONE).
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// The selector in English, as the tail of a sentence: `dead`, `ready or running`, `ready,
    /// running, blocked or dead`.
    ///
    /// It exists so that "none of the 3 processes in this domain are **dead**" names the criterion
    /// rather than leaving the reader to guess it from the command line, which they may not have
    /// typed.
    pub fn write(self, out: &mut dyn FnMut(&[u8])) {
        if self.is_empty() {
            out(b"nothing");
            return;
        }
        let n = STATES.iter().filter(|&&c| self.selects(c)).count();
        let mut written = 0usize;
        for code in STATES {
            if !self.selects(code) {
                continue;
            }
            if written > 0 {
                out(if written + 1 == n { b" or " } else { b", " });
            }
            out(ps::state_name(code).as_bytes());
            written += 1;
        }
    }
}

/// **A finished filter over one walk of one domain.** Built by [`select`], which is the only
/// constructor, so a `Selection` always describes a survey that really happened.
pub struct Selection<'a> {
    rows: &'a [ps::Row],
    selector: Selector,
    matched: usize,
    /// [`ps::Survey::complaint`], carried rather than re-derived so the two programs describe a
    /// refusal in the same words.
    complaint: Option<&'static str>,
    /// [`ps::Survey::is_complete`]: whether the listing underneath is the whole domain. False means
    /// nothing is printed, because a filter over a partial listing is a partial answer wearing a
    /// complete one's clothes.
    complete: bool,
}

/// **Apply a selector to a walked domain.** Counts here rather than at print time, because
/// DECISIONS §67 makes a program say everything it has to complain about *before* it writes a byte
/// of output, and "nothing matched" is a complaint that needs the count.
///
/// # Examples
///
/// From a host test, with no kernel and no emulator:
///
/// ```
/// use ps::{MAX_ROWS, Row};
///
/// // A domain of one blocked thread and one corpse.
/// let mut reader = |cursor: u64| match cursor {
///     0 => (1, 7, abi::survey::BLOCKED),
///     1 => (2, 9, abi::survey::DEAD),
///     _ => (abi::survey::DONE as i64, 0, 0),
/// };
/// let mut rows = [Row::default(); MAX_ROWS];
/// let survey = ps::collect(&mut rows, &mut reader);
///
/// let corpses = pgrep::Selector::from_pattern(b"dead");
/// let found = pgrep::select(&survey, corpses);
/// assert_eq!(found.matched(), 1);
///
/// let mut text = Vec::new();
/// found.write_report(&mut |b| text.extend_from_slice(b));
/// assert_eq!(String::from_utf8(text).unwrap(), "9\n");
/// ```
pub fn select<'s>(survey: &'s ps::Survey<'_>, selector: Selector) -> Selection<'s> {
    let rows = survey.rows();
    Selection {
        rows,
        selector,
        matched: rows.iter().filter(|r| selector.selects(r.state)).count(),
        complaint: survey.complaint(),
        complete: survey.is_complete(),
    }
}

impl Selection<'_> {
    /// How many members the selector named.
    pub fn matched(&self) -> usize {
        self.matched
    }

    /// How many members the domain had, matched or not.
    pub fn members(&self) -> usize {
        self.rows.len()
    }

    /// **Everything to complain about, said before a byte of output** (DECISIONS §67), and silent
    /// when members matched.
    ///
    /// The order is not cosmetic. A walk that failed is reported first, because a selector's verdict
    /// over a listing that is not the domain means nothing. Then a selector that can never match,
    /// because that is a fact about what was asked rather than about what is running, and it is true
    /// whatever the domain holds: the same reason `grant_plan::plan_against_with` answers a
    /// misquoted line before it resolves the program name.
    pub fn write_diagnostics(&self, out: &mut dyn FnMut(&[u8])) {
        if !self.complete {
            if let Some(c) = self.complaint {
                say(c, out);
            }
            return;
        }
        if self.selector.is_empty() {
            say(
                "no run state is called that, so this can never name a member of any domain",
                out,
            );
            return;
        }
        // The empty-domain sentence, in `ps`'s words: the caller held the domain and was allowed to
        // look, and there was nothing in it.
        if let Some(c) = self.complaint {
            say(c, out);
            return;
        }
        if self.matched == 0 {
            // **The fourth answer.** Upstream cannot say this: `pgrep` there prints nothing and
            // exits 1 whether the pattern missed or the caller could not look, so a monitor cannot
            // tell an idle machine from a closed door.
            out(b"pgrep: none of the ");
            write_num(self.rows.len() as u64, out);
            out(b" processes in this domain are ");
            self.selector.write(out);
            out(b"\n");
        }
    }

    /// **The tids, one per line, and nothing else.** No header and no columns, because this answer
    /// is for a program to read (upstream's shape, and the reason `pgrep` exists beside `ps` at
    /// all).
    ///
    /// Nothing at all when the walk was incomplete, when the selector can never match, or when
    /// nothing matched, so `pgrep > names.txt` in any of those cases leaves an empty file rather
    /// than a plausible-looking list of nothing.
    pub fn write_report(&self, out: &mut dyn FnMut(&[u8])) {
        if !self.complete || self.selector.is_empty() {
            return;
        }
        for row in self.rows {
            if self.selector.selects(row.state) {
                write_num(row.tid, out);
                out(b"\n");
            }
        }
    }
}

/// One diagnostics line, program name and all. A refusal reads as a fact about authority, which is
/// the standard `ps::refusal` holds itself to and the reason this crate reuses that catalogue rather
/// than writing a second one.
fn say(clause: &str, out: &mut dyn FnMut(&[u8])) {
    out(b"pgrep: ");
    out(clause.as_bytes());
    out(b"\n");
}

/// A decimal number, unpadded. `ps` right-aligns its tids in a column; this one is a name on a line
/// by itself, and leading spaces would be bytes the next program has to strip.
fn write_num(v: u64, out: &mut dyn FnMut(&[u8])) {
    let mut buf = [0u8; 20];
    let mut i = buf.len();
    let mut n = v;
    loop {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    out(&buf[i..]);
}

#[cfg(test)]
mod tests {
    use ps::{MAX_ROWS, Row};

    use super::*;

    /// A reader over a canned domain: `entries` in slot order, then done. The same fixture shape
    /// `crates/ps` uses, so both crates' tests describe a domain the same way.
    fn domain(entries: &'static [(u64, u64)]) -> impl FnMut(u64) -> (i64, u64, u64) {
        move |cursor: u64| match entries.get(cursor as usize) {
            Some(&(tid, state)) => (cursor as i64 + 1, tid, state),
            None => (abi::survey::DONE as i64, 0, 0),
        }
    }

    fn shown(f: impl FnOnce(&mut dyn FnMut(&[u8]))) -> String {
        let mut v = Vec::new();
        f(&mut |b| v.extend_from_slice(b));
        String::from_utf8(v).unwrap()
    }

    /// The raw mask is the bit of each named state, which is what a spawner puts in a register, and
    /// a state code outside the mask's width selects nothing instead of wrapping a shift.
    #[test]
    fn the_mask_is_the_named_bits_and_a_wild_code_selects_nothing() {
        assert_eq!(Selector::from_pattern(b"dead").mask(), state::DEAD);
        assert_eq!(
            Selector::from_pattern(b"ready").mask(),
            1 << abi::survey::READY
        );
        assert_eq!(Selector::from_pattern(b"*").mask(), state::EVERY);
        assert_eq!(Selector::from_pattern(b"nothing like it").mask(), 0);
        let all = Selector::of(u64::MAX);
        assert!(all.selects(abi::survey::DEAD));
        assert!(!all.selects(64), "bit 64 does not exist in a u64 mask");
        assert!(!all.selects(u64::MAX));
    }

    const MIXED: &[(u64, u64)] = &[
        (3, abi::survey::RUNNING),
        (5, abi::survey::BLOCKED),
        (9, abi::survey::DEAD),
    ];

    #[test]
    fn a_selector_names_the_states_its_pattern_names() {
        assert_eq!(Selector::from_pattern(b"dead"), Selector::of(state::DEAD));
        assert_eq!(
            Selector::from_pattern(b"r*"),
            Selector::of(state::READY | state::RUNNING),
        );
        assert_eq!(Selector::from_pattern(b"*"), Selector::EVERY);
        assert_eq!(Selector::from_pattern(b"?????"), Selector::of(state::READY));
    }

    /// **A pattern no state answers is `NONE`, not `EVERY`.** The whole reason
    /// [`Selector::from_wire`] and [`Selector::of`] treat zero differently: a selector naming
    /// nothing that widened into one naming everything is the one direction to be wrong in that a
    /// monitor cannot recover from.
    #[test]
    fn a_pattern_that_matches_no_state_names_nothing_rather_than_everything() {
        let none = Selector::from_pattern(b"zombie");
        assert!(none.is_empty());
        assert_eq!(none, Selector::NONE);
        assert_ne!(none, Selector::EVERY);

        // And the same bits arriving over the wire mean the opposite, on purpose.
        assert_eq!(Selector::from_wire(0), Selector::EVERY);
        assert_eq!(Selector::from_wire(state::DEAD), Selector::of(state::DEAD));
    }

    /// A mask with bits this system has no state for cannot make `selects` answer about one.
    #[test]
    fn a_mask_from_a_stranger_is_narrowed_to_the_states_that_exist() {
        let wide = Selector::of(u64::MAX);
        assert_eq!(wide, Selector::EVERY);
        assert!(!wide.selects(0), "DONE is a cursor sentinel, not a state");
        assert!(
            !wide.selects(99),
            "an unknown state matched a wide selector"
        );
        assert!(
            !wide.selects(u64::MAX),
            "a shift that would have overflowed"
        );
    }

    #[test]
    fn the_report_is_the_matching_thread_ids_one_per_line() {
        let mut rows = [Row::default(); MAX_ROWS];
        let survey = ps::collect(&mut rows, &mut domain(MIXED));
        let found = select(&survey, Selector::from_pattern(b"dead"));
        assert_eq!(found.matched(), 1);
        assert_eq!(found.members(), 3);
        assert_eq!(shown(|o| found.write_report(o)), "9\n");
        assert_eq!(
            shown(|o| found.write_diagnostics(o)),
            "",
            "a filter that matched has nothing to complain about",
        );
    }

    /// With no selector the answer is the whole domain, which is what the prompt gets today and is
    /// `ps` without the columns.
    #[test]
    fn no_selector_names_every_member() {
        let mut rows = [Row::default(); MAX_ROWS];
        let survey = ps::collect(&mut rows, &mut domain(MIXED));
        let found = select(&survey, Selector::from_wire(0));
        assert_eq!(found.matched(), 3);
        assert_eq!(shown(|o| found.write_report(o)), "3\n5\n9\n");
    }

    /// **The four answers, all distinct, in one test on purpose**: none of them means anything
    /// without the others. A `pgrep` whose "nothing matched" and "you may not look" produced the
    /// same bytes would be the monitor that reads like a quiet machine, which is the failure this
    /// whole stratum exists to avoid, and it is exactly what upstream's exit-1-and-print-nothing
    /// does.
    #[test]
    fn matched_nothing_empty_domain_and_refused_are_three_different_answers() {
        let mut rows_a = [Row::default(); MAX_ROWS];
        let live = ps::collect(&mut rows_a, &mut domain(MIXED));
        let missed = select(&live, Selector::from_pattern(b"ready"));

        let mut rows_b = [Row::default(); MAX_ROWS];
        let vacant = ps::collect(&mut rows_b, &mut domain(&[]));
        let empty = select(&vacant, Selector::EVERY);

        let mut rows_c = [Row::default(); MAX_ROWS];
        let denied = ps::collect(&mut rows_c, &mut |_| {
            (abi::Error::NotPermitted as i64, 0, 0)
        });
        let refused = select(&denied, Selector::EVERY);

        let matched = {
            let mut rows_d = [Row::default(); MAX_ROWS];
            let s = ps::collect(&mut rows_d, &mut domain(MIXED));
            shown(|o| select(&s, Selector::EVERY).write_diagnostics(o))
        };

        let missed_diag = shown(|o| missed.write_diagnostics(o));
        let empty_diag = shown(|o| empty.write_diagnostics(o));
        let refused_diag = shown(|o| refused.write_diagnostics(o));

        assert!(
            missed_diag.contains("none of the 3 processes"),
            "{missed_diag}"
        );
        assert!(missed_diag.contains("are ready"), "{missed_diag}");
        assert!(empty_diag.contains("holds no processes"), "{empty_diag}");
        assert!(refused_diag.contains("not looked at"), "{refused_diag}");
        assert_eq!(matched, "", "a successful filter said something");

        // All four distinct, and pairwise, so a change that merged any two fails here.
        let all = [&missed_diag, &empty_diag, &refused_diag, &matched];
        for (i, a) in all.iter().enumerate() {
            for b in &all[i + 1..] {
                assert_ne!(a, b, "two of the four answers are the same output");
            }
        }

        // And none of the three non-answers puts a byte on the output stream, so a redirect of any
        // of them leaves an empty file.
        for s in [&missed, &empty, &refused] {
            assert_eq!(shown(|o| s.write_report(o)), "");
        }
    }

    /// A selector that can never match is reported as a fact about what was **asked**, before
    /// anything about the domain, and it is true even of an empty domain.
    #[test]
    fn a_selector_that_names_no_state_says_so_ahead_of_the_domain() {
        for entries in [MIXED, &[][..]] {
            let mut rows = [Row::default(); MAX_ROWS];
            let survey = ps::collect(
                &mut rows,
                &mut |cursor| match entries.get(cursor as usize) {
                    Some(&(tid, state)) => (cursor as i64 + 1, tid, state),
                    None => (abi::survey::DONE as i64, 0, 0),
                },
            );
            let found = select(&survey, Selector::from_pattern(b"nonesuch"));
            let diag = shown(|o| found.write_diagnostics(o));
            assert!(diag.contains("no run state is called that"), "{diag}");
            assert_eq!(shown(|o| found.write_report(o)), "");
        }
    }

    /// **A refusal outranks everything**, including a selector that could not have matched: a
    /// verdict about a listing that is not the domain is not a verdict.
    #[test]
    fn a_refused_walk_is_reported_ahead_of_a_bad_selector() {
        let mut rows = [Row::default(); MAX_ROWS];
        let denied = ps::collect(&mut rows, &mut |_| (abi::Error::NoSuchSlot as i64, 0, 0));
        let found = select(&denied, Selector::from_pattern(b"nonesuch"));
        let diag = shown(|o| found.write_diagnostics(o));
        assert!(diag.contains("no process-domain capability"), "{diag}");
        assert!(!diag.contains("no run state"), "{diag}");
    }

    /// A truncated listing is not a filter's answer either: `ps` says the domain has more in it and
    /// `pgrep` prints nothing, because a selector's verdict over part of a domain is not a verdict
    /// over the domain.
    #[test]
    fn a_truncated_listing_prints_nothing_and_says_why() {
        let mut rows = [Row::default(); 2];
        let survey = ps::collect(&mut rows, &mut |cursor| {
            (cursor as i64 + 1, cursor + 100, abi::survey::READY)
        });
        let found = select(&survey, Selector::EVERY);
        assert!(shown(|o| found.write_diagnostics(o)).contains("more in it"));
        assert_eq!(shown(|o| found.write_report(o)), "");
    }

    /// The criterion is named in the sentence, because the reader may not have typed the command
    /// line and a complaint that hides what was asked is a complaint they cannot act on.
    #[test]
    fn the_sentence_names_the_states_that_were_wanted() {
        assert_eq!(shown(|o| Selector::of(state::DEAD).write(o)), "dead");
        assert_eq!(
            shown(|o| Selector::of(state::READY | state::RUNNING).write(o)),
            "ready or running",
        );
        assert_eq!(
            shown(|o| Selector::EVERY.write(o)),
            "ready, running, blocked or dead",
        );
        assert_eq!(shown(|o| Selector::NONE.write(o)), "nothing");
    }

    /// A tid is a generational name and can be ten digits; it goes out whole and with no padding,
    /// because it is the name `abi::rendezvous::REAP` accepts and the next program in a pipeline
    /// should not have to strip spaces off it.
    #[test]
    fn a_wide_thread_id_is_printed_whole_and_unpadded() {
        let big = (1u64 << 32) | 6;
        let mut rows = [Row::default(); MAX_ROWS];
        let survey = ps::collect(&mut rows, &mut |cursor| match cursor {
            0 => (1, big, abi::survey::DEAD),
            _ => (abi::survey::DONE as i64, 0, 0),
        });
        let found = select(&survey, Selector::EVERY);
        assert_eq!(shown(|o| found.write_report(o)), "4294967302\n");
    }

    /// A state code from a kernel newer than this program matches nothing, rather than being swept
    /// into a match by a wide selector. `pgrep dead` naming a live thread is the failure available
    /// here, so an unknown state is never counted as any known one.
    #[test]
    fn an_unknown_state_matches_no_selector() {
        let mut rows = [Row::default(); MAX_ROWS];
        let survey = ps::collect(&mut rows, &mut domain(&[(3, 99)]));
        let found = select(&survey, Selector::EVERY);
        assert_eq!(found.members(), 1);
        assert_eq!(found.matched(), 0, "an unknown state matched `*`");
        assert!(shown(|o| found.write_diagnostics(o)).contains("none of the 1 processes"));
    }
}
