//! **The plan, in words**: what `caps` would have said, said before anything fires, and the audit
//! of the archive a timetable was handed against what its plan will build.
//!
//! Moved out of the crate root unchanged by milestone 877 (the timetable crate root is split along
//! its seams), as §294 (the timetable crate root splits into private modules) lays out; the root
//! re-exports every public item at its old path.
//!
//! Name: provisional (milestone 877, 2026-10-12), as are its sibling modules' names.

use grant_plan::Endowment;

use crate::{Admission, NANOS_PER_SEC, Registry, Schedule, recurrence};

/// **Write the whole registered plan**: one block per entry, saying when it fires and the complete
/// authority its child will hold, or why it will not fire at all.
///
/// This is the milestone's claim in the form a person meets it. Milestone 31 made the shell able to
/// print a command's endowment **before** spawning anything (`caps least_authority_demo 7`); this prints the same
/// decision for every entry in a schedule, at registration, before the first tick. On Unix there is
/// nothing to print: a crontab line's authority is its user's, which no tool can enumerate and no
/// line chose.
///
/// It is not `swish::write_preview`, and the reason is the prose rather than the logic. Every
/// sentence that renderer writes names *the shell* ("split from this shell's budget", "this shell's
/// result endpoint"), and a scheduler is a different principal with a different budget; reusing it
/// would print sentences that are false about the process doing the printing. What must not drift is
/// the **decision**, and that cannot, because both callers get theirs from `grant_plan::plan`.
///
/// # Examples
///
/// ```
/// # use timetable::{Held, Registry};
/// let doc = timetable::parse("every 30s least_authority_demo 7\nevery 30s date\n").unwrap();
/// let reg = Registry::register(&doc, Held::default());
///
/// // A fixed buffer rather than a `String`, because this crate is `no_std` and the program that
/// // calls it writes straight down an endpoint.
/// let mut buf = [0u8; 1024];
/// let mut n = 0;
/// timetable::write_plan(&reg, &mut |b: &[u8]| {
///     buf[n..n + b.len()].copy_from_slice(b);
///     n += b.len();
/// });
/// let plan = core::str::from_utf8(&buf[..n]).unwrap();
/// assert!(plan.contains("every 30s     least_authority_demo 7"));
/// assert!(plan.contains("this timetable holds no clock, so it cannot grant one"));
/// ```
pub fn write_plan(reg: &Registry<'_>, out: &mut dyn FnMut(&[u8])) {
    out(b"timetable: the plan, before anything fires\n");
    for r in reg.rows() {
        out(b"  ");
        write_schedule(r.entry.schedule, out);
        out(b"  ");
        out(r.entry.command);
        out(b"\n");
        match r.admission {
            Admission::Fires(e) => write_grant(&e, r.entry.command, reg.held.report, out),
            Admission::Refused(refusal) => {
                out(b"    will not fire: ");
                out(refusal.message().as_bytes());
                out(b"\n");
            }
            Admission::Unbacked(u) => {
                out(b"    will not fire: ");
                out(u.message().as_bytes());
                out(b"\n");
            }
        }
    }
}

/// How wide the schedule column is, so the commands line up under each other. A schedule longer
/// than this pushes its own command right rather than being truncated: a table that lied about a
/// number to keep its shape would be the wrong trade in a file about authority.
const SCHEDULE_COLUMN: usize = 12;

/// The schedule column, padded so the commands line up.
fn write_schedule(s: Schedule, out: &mut dyn FnMut(&[u8])) {
    let mut buf = [b' '; 32];
    let n = match s {
        Schedule::AtBoot => {
            buf[..7].copy_from_slice(b"at-boot");
            7
        }
        Schedule::Every(nanos) => {
            buf[..6].copy_from_slice(b"every ");
            6 + write_interval(nanos, &mut buf[6..])
        }
        // The rule as the RRULE it is, which is longer than the column and says exactly what the
        // line means, including the UTC the line leaves implicit.
        Schedule::Calendar(rule) => {
            recurrence::write_rrule(&rule, out);
            out(b" (UTC)");
            return;
        }
    };
    out(&buf[..n.max(SCHEDULE_COLUMN)]);
}

/// An interval back in the document's own units, so what is printed is what a person typed.
fn write_interval(nanos: u64, out: &mut [u8]) -> usize {
    let (value, unit): (u64, &[u8]) = if nanos.is_multiple_of(60 * NANOS_PER_SEC) {
        (nanos / (60 * NANOS_PER_SEC), b"m")
    } else if nanos.is_multiple_of(NANOS_PER_SEC) {
        (nanos / NANOS_PER_SEC, b"s")
    } else {
        (nanos / (NANOS_PER_SEC / 1000), b"ms")
    };
    let mut n = write_u64(value, out);
    out[n..n + unit.len()].copy_from_slice(unit);
    n += unit.len();
    n
}

/// Decimal digits into a buffer, returning how many.
fn write_u64(v: u64, out: &mut [u8]) -> usize {
    let mut digits = [0u8; 20];
    let mut n = 0;
    let mut v = v;
    loop {
        digits[n] = b'0' + (v % 10) as u8;
        v /= 10;
        n += 1;
        if v == 0 {
            break;
        }
    }
    for i in 0..n {
        out[i] = digits[n - 1 - i];
    }
    n
}

/// **What the child will hold, and nothing else.** The `caps` half.
fn write_grant(e: &Endowment, command: &[u8], report: bool, out: &mut dyn FnMut(&[u8])) {
    out(b"    grants ");
    // The word typed, not `e.prog`'s name: an installed program's endowment is filed under
    // `grant_plan::IMAGE_ROW`, whose name is not the program's.
    out(grant_plan::parse_run(command).prog);
    out(b" exactly:\n");
    if report {
        out(b"      cap 0  endpoint  report its answer to this timetable\n");
    }
    if e.mem_pages > 0 {
        out(b"      cap 1  untyped   ");
        let mut buf = [0u8; 20];
        let n = write_u64(e.mem_pages, &mut buf);
        out(&buf[..n]);
        out(b" pages  split from this timetable's budget\n");
    }
    if e.arg != 0 {
        out(b"      arg      ");
        let mut buf = [0u8; 20];
        let n = write_u64(e.arg, &mut buf);
        out(&buf[..n]);
        out(b"\n");
    }
    // The line that is the whole milestone. It is not decoration: a reader of a crontab has no way
    // to write the equivalent sentence, because on Unix the answer is "whatever that user can do".
    out(b"      and nothing else: no clock, no disk, no console, no network\n");
}

// ===============================================================================================
// The archive endowment, audited against the plan.
// ===============================================================================================

/// **What the archive a timetable was handed carries, measured against what its plan will build.**
///
/// [`Registry::programs`] says which programs a document will ever start, and it says so before the
/// first tick. That number is only worth printing next to the other one: how many programs the
/// *capability* this process holds can actually reach. A scheduler handed the whole initial ramdisk
/// has a plan naming one program and an archive reaching fifty-seven, and nothing in the plan says
/// so.
///
/// This is the sentence that says so, and it is here rather than in the program because it is a
/// comparison rather than a print: the program contributes the names it can see, this decides what
/// they mean, and a host test pins the wording.
///
/// **It measures, it does not enforce.** A wider archive is a fact about the spawn site, and a
/// scheduler cannot narrow its own endowment; what it can do is say out loud how wide it is. That
/// asymmetry is the same one [`Unbacked`](crate::Unbacked) has: the fix is a decision somebody makes at the spawn
/// site, not an edit to the document.
///
/// # Examples
///
/// ```
/// # use timetable::{Audit, Held, Registry};
/// let doc = timetable::parse("every 30s least_authority_demo 7\nevery 30s date\n").unwrap();
/// let reg = Registry::register(&doc, Held::default());
///
/// // Handed an archive holding exactly what the plan builds.
/// let mut narrow = Audit::of(&reg);
/// narrow.saw("least_authority_demo");
/// assert!(narrow.is_exact());
/// assert_eq!(narrow.planned(), 1);
///
/// // Handed the whole initial ramdisk. `date` is refused, so the plan never names it, and
/// // holding an image of it is authority nothing in the document asked for.
/// let mut wide = Audit::of(&reg);
/// for name in ["least_authority_demo", "date", "swish", "memory_grant_depleter"] {
///     wide.saw(name);
/// }
/// assert!(!wide.is_exact());
/// assert_eq!(wide.beyond(), 3);
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Audit {
    /// The plan's program set, as [`Registry::programs`] computes it.
    plan: u64,
    held: usize,
    beyond: usize,
}

impl Audit {
    /// Start an audit against `reg`'s plan, having seen nothing yet.
    pub fn of(reg: &Registry<'_>) -> Self {
        Audit {
            plan: reg.programs(),
            held: 0,
            beyond: 0,
        }
    }

    /// Record one name the endowed archive carries.
    ///
    /// A name this crate does not recognise as a program counts as **beyond the plan**, which is the
    /// safe direction: an archive holding something nobody can name is still an archive holding
    /// something, and a reader would rather be told the endowment is wide than be told nothing.
    pub fn saw(&mut self, name: &str) {
        self.held += 1;
        let planned = grant_plan::Program::from_name(name.as_bytes())
            .is_some_and(|p| self.plan & (1 << p.id()) != 0);
        if !planned {
            self.beyond += 1;
        }
    }

    /// How many programs the archive carries.
    pub fn held(&self) -> usize {
        self.held
    }

    /// How many of them the plan will never build.
    pub fn beyond(&self) -> usize {
        self.beyond
    }

    /// How many distinct programs the plan will build.
    pub fn planned(&self) -> usize {
        self.plan.count_ones() as usize
    }

    /// Whether the archive reaches nothing beyond the plan.
    pub fn is_exact(&self) -> bool {
        self.beyond == 0
    }

    /// **The sentence a reader gets**, host-tested so the wording cannot drift.
    ///
    /// Two forms, and the second is the one that was true before the endowment was narrowed. Both
    /// are printed in the same place for the same reason the refusals are: an endowment nobody
    /// states is an endowment nobody checks.
    pub fn write(&self, out: &mut dyn FnMut(&[u8])) {
        let mut buf = [0u8; 20];
        if self.is_exact() {
            out(b"timetable: the archive it holds carries exactly the ");
            let n = write_u64(self.held as u64, &mut buf);
            out(&buf[..n]);
            out(if self.held == 1 {
                b" program its plan names\n"
            } else {
                b" programs its plan names\n"
            });
        } else {
            out(b"timetable: the archive it holds carries ");
            let n = write_u64(self.held as u64, &mut buf);
            out(&buf[..n]);
            out(b" programs, ");
            let n = write_u64(self.beyond as u64, &mut buf);
            out(&buf[..n]);
            out(b" of them beyond its plan\n");
        }
    }
}

#[cfg(test)]
mod tests {
    use grant_plan::Program;

    use super::*;
    use crate::test_support::{REFERENCE, shown};
    use crate::{Held, SHIPPED_HELD, parse};

    /// **The plan's rendering, in the units and the widths a reader meets.** The test below prints
    /// one plan and asserts substrings of it, which left three families of mutant alive until the
    /// 2026-09-19 run found them, and the shape is the same in all three: the schedule column is a
    /// 32-byte space-filled buffer emitted as `buf[..n.max(12)]`, so a wrong length is invisible
    /// for any schedule that fits in twelve columns, and a branch nothing renders is a branch
    /// nothing checks.
    ///
    /// - **The millisecond branch was never rendered.** Every plan in the tests uses seconds or
    ///   minutes, so `nanos / (NANOS_PER_SEC / 1000)` could become `%` or either `/` a `*`.
    /// - **The length arithmetic was never observed**, because `30s` and `1m` are short. A
    ///   seven-digit interval pushes past the column, where `6 + len` under `-` underflows and
    ///   `n += unit.len()` under `-=` truncates the text and under `*=` pads it.
    /// - **`e.mem_pages > 0` and `e.arg != 0` were only ever seen true.** An entry with neither is
    ///   what proves the lines are omitted rather than always printed.
    #[test]
    fn the_plan_prints_milliseconds_long_intervals_and_the_absent_grants() {
        let doc = parse(
            "every 1500ms least_authority_demo 7\n\
             every 1234567ms least_authority_demo 0\n",
        )
        .unwrap();
        let reg = Registry::register(&doc, Held::default());
        let s = shown(|out| write_plan(&reg, out));

        assert!(s.contains("every 1500ms  least_authority_demo 7"), "{s}");
        assert!(s.contains("every 1234567ms  least_authority_demo 0"), "{s}");
        assert!(s.contains("      arg      7\n"), "{s}");
        assert!(
            !s.contains("arg      0"),
            "a zero argument is not a grant: {s}"
        );
        assert!(
            !s.contains("cap 1"),
            "neither entry asked for memory, so nothing is split: {s}"
        );
    }

    #[test]
    fn the_plan_is_printable_before_anything_fires() {
        let doc = parse(
            "every 30s least_authority_demo 7\n\
             at-boot memory_grant_depleter --mem 4\n\
             every 1m date\n",
        )
        .unwrap();
        let reg = Registry::register(
            &doc,
            Held {
                mem_pages: 64,
                ..Held::default()
            },
        );
        let s = shown(|out| write_plan(&reg, out));

        // Every entry is named with its schedule in the units it was written in.
        assert!(s.contains("every 30s"), "{s}");
        assert!(s.contains("at-boot"), "{s}");
        assert!(s.contains("every 1m"), "{s}");
        // The grant, in full, for the ones that will fire.
        assert!(s.contains("grants least_authority_demo exactly:"), "{s}");
        assert!(
            s.contains("4 pages  split from this timetable's budget"),
            "{s}"
        );
        assert!(
            s.contains("and nothing else: no clock, no disk, no console, no network"),
            "{s}",
        );
        // And the reason, in full, for the one that will not.
        assert!(
            s.contains("will not fire: this timetable holds no clock, so it cannot grant one"),
            "{s}",
        );
    }

    /// **The endowment a spawn site hands over, measured against the plan it can compute.**
    ///
    /// The whole point of [`Registry::programs`] being computable at registration is that the spawn
    /// site can narrow what it hands over to exactly that set. This is the check that says whether
    /// it did, and the two sentences are what a reader meets on the console: one for an archive that
    /// reaches nothing beyond the plan, one for the whole initial ramdisk.
    ///
    /// **This test is also the mechanism behind a written list in another file**, which is why the
    /// first assertion names `least_authority_demo` and `memory_grant_depleter` rather than counting to two.
    /// `system_tests/src/user/timetable_tests.rs` builds the narrowed archive from a `PLANNED_PROGRAMS`
    /// list it does not compute, because computing it there means a `Registry` on a kernel stack and
    /// a `Registry` is 21632 bytes against a 4096-byte guard page (`script/stack-frame-check`). So
    /// the plan is computed **here**, on the host, in milliseconds, and a document edited without
    /// that list being edited fails right here. Change the shipped document's admitted set and this
    /// assertion is the first thing that goes red; keep them in step and nothing else has to
    /// remember.
    #[test]
    fn the_archive_a_timetable_holds_is_measured_against_what_it_will_build() {
        let doc = parse(REFERENCE).expect("the shipped document parses");
        let reg = Registry::register(&doc, SHIPPED_HELD);

        // The shipped document admits `least_authority_demo` twice and `memory_grant_depleter` once (the `--mem 4` line,
        // backed by `SHIPPED_HELD`) and refuses everything else, so its plan is two programs
        // however many entries name either. Asserted **by name**, because a spawn site in another
        // crate carries this same set as a written list and this is what keeps it true.
        assert_eq!(
            reg.programs(),
            1 << Program::LeastAuthorityDemo.id() | 1 << Program::MemoryGrantDepleter.id(),
            "the shipped document's plan changed; \
             system_tests/src/user/timetable_tests.rs's PLANNED_PROGRAMS must change with it",
        );
        assert_eq!(Audit::of(&reg).planned(), 2);

        // Narrowed to the plan: what the spawn site hands over after milestone 129's second
        // stratum. `least_authority_demo`, `memory_grant_depleter`, and nothing else.
        let mut narrow = Audit::of(&reg);
        narrow.saw("least_authority_demo");
        narrow.saw("memory_grant_depleter");
        assert!(narrow.is_exact());
        assert_eq!(narrow.held(), 2);
        assert_eq!(narrow.beyond(), 0);
        assert_eq!(
            shown(|out| narrow.write(out)),
            "timetable: the archive it holds carries exactly the 2 programs its plan names\n",
        );

        // The whole initial ramdisk: what it used to be handed. Every program the plan refused is
        // still an image this process could load, which is authority no line in the document asked
        // for. `date` is the sharpest case, because the document names it and registration refused
        // it.
        let mut wide = Audit::of(&reg);
        for name in [
            "least_authority_demo",
            "date",
            "ps",
            "memory_grant_depleter",
            "wc",
            "swish",
        ] {
            wide.saw(name);
        }
        assert!(!wide.is_exact());
        assert_eq!(wide.held(), 6);
        assert_eq!(wide.beyond(), 4);
        assert_eq!(
            shown(|out| wide.write(out)),
            "timetable: the archive it holds carries 6 programs, 4 of them beyond its plan\n",
        );

        // A name this tree does not know is still authority, and counts against the plan rather
        // than being waved through as unrecognised.
        let mut stranger = Audit::of(&reg);
        stranger.saw("not_a_program_in_this_tree");
        assert_eq!(stranger.beyond(), 1);
    }

    /// **Widening the scheduler widens the plan, and the audit follows it.** The same pairing
    /// [`Held`] has everywhere else: what counts as authority beyond the plan is a fact about a
    /// particular scheduler, not a constant.
    #[test]
    fn a_wider_scheduler_plans_more_programs_and_the_audit_agrees() {
        let doc = parse("every 1s least_authority_demo 7\nevery 1s date\n").expect("well formed");

        let bare = Registry::register(&doc, Held::default());
        let mut a = Audit::of(&bare);
        a.saw("least_authority_demo");
        a.saw("date");
        assert_eq!(
            a.beyond(),
            1,
            "a refused `date` is an image nothing will run"
        );

        let with_clock = Registry::register(
            &doc,
            Held {
                clock: true,
                ..Held::default()
            },
        );
        let mut b = Audit::of(&with_clock);
        b.saw("least_authority_demo");
        b.saw("date");
        assert!(
            b.is_exact(),
            "granting the scheduler a clock admits `date`, so its image is in the plan",
        );
        assert_eq!(b.planned(), 2);
    }

    /// **A job holds no report endpoint when its timetable holds none** (Fork 6 C of milestone 152,
    /// ruled 2026-09-27 on #1377), and the printed plan, which is a registrar's whole view of what a
    /// job holds, says so by not listing one.
    #[test]
    fn a_timetable_holding_no_report_endpoint_grants_none() {
        let durable = Held {
            report: false,
            ..SHIPPED_HELD
        };
        let doc = parse("every 5s least_authority_demo 7\n").unwrap();
        let shipped = shown(|out| write_plan(&Registry::register(&doc, SHIPPED_HELD), out));
        let registered = shown(|out| write_plan(&Registry::register(&doc, durable), out));
        assert!(shipped.contains("cap 0  endpoint  report its answer"));
        assert!(
            !registered.contains("endpoint"),
            "a registered plan lists a report endpoint: {registered}",
        );
        assert!(registered.contains("grants least_authority_demo exactly:"));
    }
}
