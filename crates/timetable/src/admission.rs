//! **Registration, the security boundary**: what this scheduler holds, and what each entry
//! becomes against it. An entry fires, is refused by its program's own manifest, or is unbacked by
//! this scheduler, and these are the types and the checks that tell the three apart.
//!
//! Moved out of the crate root unchanged by milestone 877 (the timetable crate root is split along
//! its seams), as §294 (the timetable crate root splits into private modules) lays out; the root
//! re-exports every public item at its old path.
//!
//! Name: provisional (milestone 877, 2026-10-12), as are its sibling modules' names.

use grant_plan::expand::Expansion;
use grant_plan::{Endowment, Holdings, Manifest, Refusal};

use crate::{Entry, Schedule};

///
/// This is `grant_plan::Holdings` one level out, and it is a parameter for exactly the same reason:
/// "this scheduler holds nothing to back that" must be a statement about a particular scheduler's
/// capability table rather than a hardcoded era. The same document is four refusals in a scheduler granted
/// nothing but a budget and four running jobs in one granted a clock, a directory and a terminal,
/// and neither the document nor the program manifests can tell them apart.
///
/// Every field but `mem_pages` is `false` today, because the scheduler `components/src/timetable.rs`
/// really does hold nothing but memory and its own children's channels. Adding a capability to it
/// is then a visible edit here and a visible change in what the printed plan says, which is the
/// property worth having: **widening a scheduler is a decision somebody makes on purpose**, not
/// something an entry can talk it into.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Held {
    /// Pages of untyped this scheduler may split off for an entry's `--mem` grant. Zero means it
    /// can back no memory grant at all.
    pub mem_pages: u64,
    /// A read-only mapping of the clock page it could endow a child with (DECISIONS §43).
    pub clock: bool,
    /// A supervision endpoint it could hand a child as a process view (milestone 126).
    pub domain: bool,
    /// A directory capability it could narrow into a per-file or per-directory grant.
    pub dir: bool,
    /// A terminal whose `^C` could reach a child that declares itself interruptible (DECISIONS
    /// §24). A scheduled job has nobody at a keyboard, so this is the field least likely ever to be
    /// true, and it is here rather than assumed so that the refusal has a reason a reader can find.
    pub interrupt: bool,
    /// **An endpoint a job may report its answer on**, handed to every job as its slot 0.
    ///
    /// `components/src/timetable.rs` sets it from whether its spawn site placed one, probed at
    /// `_start`. A durable session places none: calef ruled milestone 152 (durable delegation)'s
    /// Fork 6 as option C on 2026-09-27 (UTC, on #1377), so a durable job writes its output through
    /// whatever its entry grants, and nobody is left holding a stream nobody drains. The kernel
    /// harness still places one, because reports are how it watches jobs fire. Name: provisional.
    pub report: bool,
}

/// **What the shipped `components/src/timetable.rs` actually holds**, and the one `Held` value it and
/// this crate's own host test both use, so the two cannot drift the way [`Registry::register`](crate::Registry::register)'s
/// module doc already warns a document and a spawn site can. `PLANNED_PROGRAMS` in
/// `system_tests/src/user/timetable_tests.rs` is the same idea applied to the archive; this is it applied
/// to what the scheduler holds.
///
/// Four pages: enough to admit one `at-boot memory_grant_depleter --mem 4` line as this milestone's proof that a
/// grant can be backed at all, and small on purpose. Nothing about the mechanism needs a bigger
/// number; picking one only because it looks more impressive would be gold-plating a demonstration
/// that already makes its point at four.
pub const SHIPPED_HELD: Held = Held {
    mem_pages: 4,
    clock: false,
    domain: false,
    dir: false,
    interrupt: false,
    report: true,
};

/// **An authority the entry's plan needs and this scheduler does not hold.**
///
/// The distinction from [`Refusal`] is the whole point and it is not a shade of the same thing.
/// A `Refusal` is a fact about **the line**: `memory_grant_depleter` with no `--mem` is wrong wherever it is
/// typed, and the fix is to edit the entry. An `Unbacked` is a fact about **the scheduler**: the
/// line is exactly right and would run at a prompt, and the fix is to grant the scheduler something
/// it was not granted. Collapsing the two would tell a person to edit a line that has nothing wrong
/// with it.
///
/// # BUGS
///
/// **[`Unbacked::File`] is reached only through a streamed operand** (`wc report.txt`), never
/// through `Endowment::file`, because no shipped program declares a `FileSpec::Required`.
/// `grant_plan` keeps that branch live with a fixture of its own rather than a program.
///
/// Fixed 2026-09-26 by milestone 129 (scheduled execution): `admit` used to plan against the scheduler's own `dir`, so a
/// designation this scheduler could not back was refused during planning and arrived as
/// [`Admission::Refused`] carrying `Refusal::NoSuchCapability`, telling a reader to edit a line
/// that had nothing wrong with it. Milestone 326 (turn a mutation score upward) found it on 2026-09-19 from two mutants that
/// deleted the `!` in `unbacked`'s `!held.dir` tests and survived the whole suite. The planner is
/// now lent a directory unconditionally and `unbacked` alone answers whether one is held.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Unbacked {
    /// The entry designates a file, and this scheduler holds no directory to narrow.
    File,
    /// The entry designates a directory, and this scheduler holds none.
    Directory,
    /// The program's manifest declares a clock, and this scheduler holds none.
    ///
    /// **The entry Unix cannot refuse.** `date` in a crontab always runs, because there the clock is
    /// ambient; here it is a capability, so a scheduler that was not granted one cannot endow one,
    /// and saying so at registration beats firing a program that can only print that it does not
    /// know what time it is.
    Clock,
    /// The program's manifest declares a process view, and this scheduler holds none to hand over.
    Domain,
    /// The program declares itself interruptible, and there is no terminal behind a scheduled job
    /// for a `^C` to come from.
    Interrupt,
    /// The entry's `--mem` grant is larger than this scheduler's whole budget. An entry cannot ask
    /// for more than the scheduler was given, and that ceiling is the scheduler's rather than the
    /// program manifest's, which is why the manifest's own `MemSpec` range did not catch it.
    Memory,
    /// The line is a calendar entry, and this timetable holds no clock to keep a time of day by.
    /// Unlike [`Unbacked::Clock`] this is the timetable's own need, whatever the program is.
    WallClock,
}

impl Unbacked {
    /// The fixed half of the sentence a reader gets, host-tested so the wording cannot drift.
    ///
    /// Every one of these reads as a statement about **this scheduler**, deliberately: "holds no
    /// clock" rather than "clock not permitted", because the second sounds like a policy somebody
    /// could relax and the first names the thing that would have to change.
    pub fn message(self) -> &'static str {
        match self {
            Unbacked::File => "this timetable holds no directory, so it cannot grant a file",
            Unbacked::Directory => "this timetable holds no directory to grant",
            Unbacked::Clock => "this timetable holds no clock, so it cannot grant one",
            Unbacked::Domain => "this timetable holds no process view to grant",
            Unbacked::Interrupt => "nobody is at a keyboard behind a scheduled job",
            Unbacked::Memory => "more memory than this timetable's whole budget",
            Unbacked::WallClock => "this timetable holds no clock to keep a time of day by",
        }
    }
}

/// What registration decided about one entry. See the table in the crate docs.
///
/// **The large variant is the point rather than a defect, so clippy is overruled here on purpose.**
/// `Endowment` is about a kilobyte, mostly the name set a directory grant can carry, and carrying it
/// whole is what makes "the plan *is* the authority" true instead of a summary of it. The usual fix
/// is a `Box`, and this crate has no allocator to box into: it is `no_std` with no `alloc`, by the
/// same rule that keeps it host-testable and Kani-reachable. So the alternative is not a smaller
/// enum, it is a `Row` that stores an outcome beside a separate endowment and lets the two disagree.
/// The size is paid for where it lands, which is a fixed inline table sized for exactly this
/// (`system_tests/src/user/timetable_tests.rs` states the stack the program needs, and why).
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Admission {
    /// Planned, backed, armed. The [`Endowment`] is the complete authority the child will hold, and
    /// printing it is what "`caps` before anything fires" means.
    Fires(Endowment),
    /// The program's own manifest refuses the line, exactly as it would at the prompt.
    Refused(Refusal),
    /// The line is legal and this scheduler holds nothing to back it.
    Unbacked(Unbacked),
}

/// Plan one entry's command against its program's manifest and against what the scheduler holds.
pub(super) fn admit(command: &[u8], held: Held) -> Admission {
    let spec = grant_plan::parse_run(command);
    // **The plan is made as if this scheduler held a directory, whether or not it does**, and
    // `unbacked` below is what answers whether it really does. Planning against `held.dir` instead
    // made `grant_plan` refuse a designation (`rm -r logs`, `wc report.txt`) during planning, so a
    // line with nothing wrong with it came back as a `Refused` ("edit the line") when the truth was
    // an `Unbacked` ("grant the scheduler a directory"), and `Unbacked::File`/`Directory` could not
    // arrive at all. Milestone 326's mutation run found that on 2026-09-19; milestone 129 closed it
    // on 2026-09-26. A line that is wrong for any other reason is still refused here, because the
    // directory is the only holding this lends the planner.
    //
    // The position is the root, because a scheduler has no `cd`: there is no prompt to have typed
    // one at, and a working directory that could move between registration and a fire would make
    // an already planned grant mean something different later.
    let holdings = Holdings {
        dir: true,
        ..Holdings::default()
    };
    match grant_plan::plan(&spec, holdings, Expansion::none()) {
        Err(r) => Admission::Refused(r),
        Ok(e) => match unbacked(&e, &e.program.manifest(), held) {
            Some(u) => Admission::Unbacked(u),
            None => Admission::Fires(e),
        },
    }
}

/// [`admit_installed`] for a whole entry: a calendar line needs the wall clock before its
/// program's manifest is asked, as [`Registry::register`](crate::Registry::register) rules for an archive program, so store
/// mode keeps that answer rather than overwriting it (milestone 129's rule met milestone 152's
/// store mode at the merge of 2026-10-03 (UTC)).
pub(super) fn admit_installed_row(entry: &Entry<'_>, m: Manifest, held: Held) -> Admission {
    match entry.schedule {
        Schedule::Calendar(_) if !held.clock => Admission::Unbacked(Unbacked::WallClock),
        _ => admit_installed(entry.command, m, held),
    }
}

/// **[`admit`], for an installed program**, planned against the manifest its bytes carry rather
/// than against a row of the image's table. The endowment is filed under `grant_plan::IMAGE_ROW`,
/// the prompt's own convention for bytes, so nothing may be read off `e.prog`: [`unbacked`] is
/// handed `m` for that reason.
fn admit_installed(command: &[u8], m: Manifest, held: Held) -> Admission {
    let spec = grant_plan::parse_run(command);
    let holdings = Holdings {
        dir: true,
        ..Holdings::default()
    };
    match grant_plan::plan_against(&spec, grant_plan::IMAGE_ROW, m, holdings, Expansion::none()) {
        Err(r) => Admission::Refused(r),
        Ok(e) => match unbacked(&e, &m, held) {
            Some(u) => Admission::Unbacked(u),
            None => Admission::Fires(e),
        },
    }
}

/// **Whether a timetable can build a program that carries `m` at all**, before any line is planned
/// against it: what an image request can carry (`grant_plan::image_can_carry`), less what no
/// timetable endows. An argv (`grant_plan::ArgSpec::Words`), entropy, the network and a
/// configuration page have no slot in `components/src/timetable.rs`'s `fire`, and the verdicts a
/// page carries have no word for them, so an installed program declaring one is refused when its
/// document is resolved rather than planned. Name: provisional.
pub fn schedulable(m: &Manifest) -> bool {
    grant_plan::image_can_carry(m)
        && !grant_plan::image_hears_words(m)
        && !m.entropy
        && !m.network
        && !m.config
}

/// The first authority `e` needs that `held` does not have.
///
/// **Order is meaning here.** The designations a person typed come first (a file, a directory),
/// because those are the ones an editor can fix by changing the line. The manifest-declared
/// endowments come after, because those are facts about the program that no line can change and the
/// fix is at the scheduler's spawn site. A reader who gets `Clock` back knows not to go looking for
/// a typo.
fn unbacked(e: &Endowment, m: &Manifest, held: Held) -> Option<Unbacked> {
    // A file reaches a program by four routes: the manifest's own per-file grant, an operand
    // streamed in by an adapter (`wc report.txt`), a redirected output and a redirected second
    // stream. Each is narrowed from a directory, so each is unbacked in a scheduler that holds
    // none. (`plan` passes no operators today, so the last two cannot arrive; they are checked
    // anyway so that teaching the timetable `>` cannot quietly admit a grant it cannot back.)
    let streams_a_file = matches!(e.source, grant_plan::line::Source::File(_))
        || matches!(e.sink, grant_plan::line::Sink::File(..))
        || matches!(e.diagnostics, grant_plan::line::Diagnostics::File(..));
    if (e.file.is_some() || streams_a_file) && !held.dir {
        return Some(Unbacked::File);
    }
    if e.dir.is_some() && !held.dir {
        return Some(Unbacked::Directory);
    }
    if e.mem_pages > held.mem_pages {
        return Some(Unbacked::Memory);
    }
    if m.clock && !held.clock {
        return Some(Unbacked::Clock);
    }
    if m.domain && !held.domain {
        return Some(Unbacked::Domain);
    }
    if m.interruptible && !held.interrupt {
        return Some(Unbacked::Interrupt);
    }
    None
}

#[cfg(test)]
mod tests {
    use grant_plan::Program;

    use super::*;
    use crate::test_support::{REFERENCE, shown, with_clock};
    use crate::{Error, MAX_ENTRIES, Registry, parse, write_plan};

    #[test]
    fn the_shipped_document_parses_and_says_what_it_claims() {
        let doc = parse(REFERENCE).expect("the shipped timetable must parse");
        let reg = Registry::register(&doc, SHIPPED_HELD);
        // The document is written to show every answer registration can give, so every variant
        // must actually appear in it. A document that drifted into being all-happy-path would
        // still pass a "it parses" test and would have stopped demonstrating anything.
        let mut fires = 0;
        let mut refused = 0;
        let mut unbacked = 0;
        for r in reg.rows() {
            match r.admission {
                Admission::Fires(_) => fires += 1,
                Admission::Refused(_) => refused += 1,
                Admission::Unbacked(_) => unbacked += 1,
            }
        }
        assert!(fires > 0, "the shipped document schedules nothing");
        assert!(
            refused > 0,
            "the shipped document shows no manifest refusal"
        );
        assert!(
            unbacked > 0,
            "the shipped document shows nothing this scheduler cannot back",
        );
    }

    /// **The four answers, one document.** This is the negative-control test: the happy path alone
    /// would prove only that something can be scheduled, and what makes the milestone's claim is
    /// what gets turned away.
    #[test]
    fn registration_gives_four_distinguishable_answers() {
        let doc = parse(
            "every 5s least_authority_demo 7\n\
             every 5s memory_grant_depleter\n\
             every 5s date\n\
             every 5s wc\n\
             every 5s ps\n\
             every 5s interrupt_heeder\n\
             every 5s nosuchprogram\n",
        )
        .unwrap();
        let reg = Registry::register(&doc, Held::default());
        let rows = reg.rows();

        // Fires, with the argument the line named and no memory it did not.
        let e = rows[0].endowment().unwrap();
        assert_eq!(e.program, Program::LeastAuthorityDemo);
        assert_eq!(e.arg, 7);
        assert_eq!(e.mem_pages, 0);

        // The manifest refuses the line, and it is the same refusal the prompt gives.
        assert_eq!(rows[1].admission, Admission::Refused(Refusal::MemRequired));

        // Legal lines this scheduler holds nothing to back. All four are entries a Unix cron would
        // simply have run.
        assert_eq!(rows[2].admission, Admission::Unbacked(Unbacked::Clock));
        // `wc` reads a stream, and `grant_plan` refuses that at the prompt already
        // (`InputSpec::Required` with nothing feeding it). It arrives here as the shell's own
        // refusal rather than as an `Unbacked`, which is the right shape: one authority decides
        // whether a line is runnable, and this crate only adds the question the shell cannot ask,
        // which is what the *scheduler* holds.
        assert_eq!(
            rows[3].admission,
            Admission::Refused(Refusal::InputRequired)
        );
        assert_eq!(rows[4].admission, Admission::Unbacked(Unbacked::Domain));
        assert_eq!(rows[5].admission, Admission::Unbacked(Unbacked::Interrupt));

        // And a name that resolves to nothing is a fact about the system, not about a program.
        assert_eq!(
            rows[6].admission,
            Admission::Refused(Refusal::NoSuchProgram)
        );

        assert_eq!(reg.admitted(), 1);
        assert_eq!(reg.programs(), 1 << Program::LeastAuthorityDemo.id());
    }

    /// **Widening the scheduler changes the answer, and that is the point of [`Held`].**
    ///
    /// The same document, twice. Nothing about the entries changed; what changed is what the
    /// process doing the scheduling was granted. This is the property that makes `Unbacked` a
    /// different thing from `Refusal` rather than a shade of it.
    #[test]
    fn what_the_scheduler_holds_decides_what_it_can_schedule() {
        let doc = parse("every 5s date\nevery 5s memory_grant_depleter --mem 4\n").unwrap();

        let bare = Registry::register(&doc, Held::default());
        assert_eq!(
            bare.rows()[0].admission,
            Admission::Unbacked(Unbacked::Clock)
        );
        assert_eq!(
            bare.rows()[1].admission,
            Admission::Unbacked(Unbacked::Memory),
        );

        let endowed = Registry::register(
            &doc,
            Held {
                clock: true,
                mem_pages: 64,
                ..Held::default()
            },
        );
        assert!(matches!(endowed.rows()[0].admission, Admission::Fires(_)));
        let e = endowed.rows()[1].endowment().unwrap();
        assert_eq!(e.program, Program::MemoryGrantDepleter);
        assert_eq!(e.mem_pages, 4);

        // And the ceiling is the scheduler's, not the manifest's: `memory_grant_depleter` declares 1..=64 pages,
        // so 8 is a legal line and still more than a scheduler holding 4 can back.
        let doc = parse("every 5s memory_grant_depleter --mem 8\n").unwrap();
        let tight = Registry::register(
            &doc,
            Held {
                mem_pages: 4,
                ..Held::default()
            },
        );
        assert_eq!(
            tight.rows()[0].admission,
            Admission::Unbacked(Unbacked::Memory),
        );
    }

    /// **What a designation costs when the scheduler holds a directory, and what it costs when it
    /// does not.** `admit` lends `grant_plan::plan` a directory, and until the 2026-09-19 mutation
    /// run nothing tested that it does: that run deleted the field from the `Holdings` expression,
    /// falling back to the default `false`, and no test noticed. A timetable that holds a directory
    /// and reports every `rm` line as unbackable is a scheduler that has forgotten what it was
    /// given.
    ///
    /// The two `!held.dir` tests in `unbacked` are also called here directly. Until 2026-09-26
    /// `admit` could not reach either, because the `dir` it passed to `plan` was the same bit
    /// `unbacked` then re-tested, and the mutants that deleted both `!`s survived on exactly that;
    /// `a_designation_the_scheduler_cannot_back_is_unbacked_rather_than_refused` now reaches them
    /// through `register`. `e.file` has no route in at all, since no shipped program declares a
    /// `FileSpec::Required`, so the grant below is lifted off a real `wc` plan rather than forged.
    #[test]
    fn a_designation_is_backed_by_the_directory_the_scheduler_holds() {
        let doc = parse("every 5s rm -r logs\nevery 5s wc report.txt\n").unwrap();
        let with_dir = Held {
            dir: true,
            ..Held::default()
        };
        let reg = Registry::register(&doc, with_dir);

        let mut e = reg.rows()[0]
            .endowment()
            .expect("a scheduler holding a directory can back `rm -r logs`");
        assert!(e.dir.is_some(), "the plan narrowed the directory it holds");
        assert_eq!(unbacked(&e, &e.program.manifest(), with_dir), None);
        assert_eq!(
            unbacked(&e, &e.program.manifest(), Held::default()),
            Some(Unbacked::Directory)
        );

        let wc = reg.rows()[1].endowment().expect("and `wc report.txt`");
        let grant_plan::line::Source::File(f) = wc.source else {
            panic!(
                "`wc report.txt` resolves its operand to a file: {:?}",
                wc.source
            );
        };
        e.dir = None;
        e.file = Some(f);
        assert_eq!(unbacked(&e, &e.program.manifest(), with_dir), None);
        assert_eq!(
            unbacked(&e, &e.program.manifest(), Held::default()),
            Some(Unbacked::File)
        );
    }

    /// **A calendar line needs the timetable to hold a clock**, whatever its program: keeping a
    /// time of day is the timetable's own need. `least_authority_demo` declares no clock and is
    /// still unbacked on a calendar line in a clockless timetable, and fires in one that holds one.
    #[test]
    fn a_calendar_line_is_unbacked_without_a_clock_and_admitted_with_one() {
        let doc = parse("every day at 02:00 least_authority_demo 7\n").unwrap();
        let bare = Registry::register(&doc, Held::default());
        assert_eq!(
            bare.rows()[0].admission,
            Admission::Unbacked(Unbacked::WallClock)
        );
        let held = Registry::register(&doc, with_clock());
        assert!(held.rows()[0].endowment().is_some());
    }

    /// **A designation in a scheduler holding no directory is the scheduler's fault, not the
    /// line's.** Until 2026-09-26 `admit` planned against the scheduler's own `dir`, so these two
    /// lines came back `Refused(NoSuchCapability)`, which tells a reader to edit a line that has
    /// nothing wrong with it; milestone 326's mutation run found the arms unreachable. The negative
    /// control is the third line: a line that is wrong for its own reasons is still refused even
    /// though the planner is lent a directory.
    #[test]
    fn a_designation_the_scheduler_cannot_back_is_unbacked_rather_than_refused() {
        let doc = parse(
            "every 5s rm -r logs\n\
             every 5s wc report.txt\n\
             every 5s memory_grant_depleter\n",
        )
        .unwrap();
        let bare = Registry::register(&doc, Held::default());
        assert_eq!(
            bare.rows()[0].admission,
            Admission::Unbacked(Unbacked::Directory)
        );
        assert_eq!(
            bare.rows()[1].admission,
            Admission::Unbacked(Unbacked::File)
        );
        assert!(
            matches!(bare.rows()[2].admission, Admission::Refused(_)),
            "a line missing its own `--mem` is the line's fault in any scheduler: {:?}",
            bare.rows()[2].admission
        );
        assert_eq!(bare.admitted(), 0, "and nothing unbacked is armed");
    }

    /// Every `Unbacked` has a distinct sentence. Two variants sharing wording would make the
    /// difference between "fix the line" and "fix the scheduler" invisible to a reader.
    #[test]
    fn every_refusal_reads_differently() {
        let all = [
            Unbacked::File,
            Unbacked::Directory,
            Unbacked::Clock,
            Unbacked::Domain,
            Unbacked::Interrupt,
            Unbacked::Memory,
        ];
        for (i, a) in all.iter().enumerate() {
            for b in &all[i + 1..] {
                assert_ne!(a.message(), b.message(), "{a:?} and {b:?} read the same");
            }
        }
        let errs = [
            Error::Malformed(1),
            Error::UnknownSchedule(1),
            Error::MissingInterval(1),
            Error::BadInterval(1),
            Error::ZeroInterval(1),
            Error::MissingCommand(1),
            Error::TooManyEntries(1),
        ];
        for (i, a) in errs.iter().enumerate() {
            for b in &errs[i + 1..] {
                assert_ne!(a.message(), b.message(), "{a:?} and {b:?} read the same");
            }
        }
    }

    /// **An installed program's line is planned against the manifest its bytes carry** (Fork 8 D
    /// of milestone 152, ruled 2026-09-27 on #1377), never against `IMAGE_ROW`'s row. Each assertion
    /// would pass for the wrong reason if the manifest were read off the endowment: `IMAGE_ROW` is
    /// `uptime`, which takes no argument and holds no clock.
    #[test]
    fn an_installed_program_is_planned_against_its_own_manifest() {
        let doc = parse(
            "every 5s tick 7\n\
             every 5s quiet 7\n\
             every 5s clocked\n",
        )
        .unwrap();
        let mut manifests = [grant_plan::NO_NOTE_MANIFEST; MAX_ENTRIES];
        manifests[0] = Program::LeastAuthorityDemo.manifest();
        manifests[2] = Program::Date.manifest();
        let reg = Registry::register_installed(&doc, SHIPPED_HELD, &manifests);

        let e = reg.rows()[0]
            .endowment()
            .expect("a note that takes an argument binds `tick 7`");
        assert_eq!(e.arg, 7);
        assert!(
            matches!(reg.rows()[1].admission, Admission::Refused(_)),
            "no note is `uptime`'s manifest, which takes no argument, so `quiet 7` is refused",
        );
        assert_eq!(
            reg.rows()[2].admission,
            Admission::Unbacked(Unbacked::Clock),
            "a note that wants the clock is unbacked here, whatever row the endowment is filed under",
        );
        let plan = shown(|out| write_plan(&reg, out));
        assert!(
            plan.contains("grants tick exactly:"),
            "the plan names the word typed, not the row: {plan}",
        );
    }

    /// **An upgrade between two fires still fires, with the new version** (calef's amendment of
    /// Fork 8 D, 2026-09-27 on #1377). The line was registered against one manifest; at the next
    /// fire the program carries another that the line still binds, and the fire is planned from
    /// the new one. Falsified by planning from the registered row, which keeps the old endowment.
    #[test]
    fn an_upgraded_program_fires_its_new_version() {
        let doc = parse("every 5s tick --mem 2\n").unwrap();
        let old = Manifest {
            mem: Program::MemoryGrantDepleter.manifest().mem,
            ..grant_plan::NO_NOTE_MANIFEST
        };
        let mut manifests = [grant_plan::NO_NOTE_MANIFEST; MAX_ENTRIES];
        manifests[0] = old;
        let reg = Registry::register_installed(&doc, SHIPPED_HELD, &manifests);
        assert_eq!(reg.rows()[0].endowment().unwrap().mem_pages, 2);

        // Version two flips whether it reports, and still takes `--mem`.
        let new = Manifest {
            reports: !old.reports,
            ..old
        };
        match reg.plan_at_fire(0, new) {
            Admission::Fires(e) => {
                assert_eq!(e.mem_pages, 2);
                assert_eq!(
                    e.reports, new.reports,
                    "planned from the new version's manifest"
                );
            }
            other => panic!("the new version must fire: {other:?}"),
        }
    }

    /// **A new version that asks for more than its line grants does not fire.** Version two wants
    /// the clock, which this scheduler cannot back, and a later one drops the `--mem` its line
    /// passes. Both are refusals at the fire, not a silently different job.
    #[test]
    fn a_new_version_asking_for_more_than_its_line_grants_does_not_fire() {
        let doc = parse("every 5s tick --mem 2\n").unwrap();
        let old = Manifest {
            mem: Program::MemoryGrantDepleter.manifest().mem,
            ..grant_plan::NO_NOTE_MANIFEST
        };
        let mut manifests = [grant_plan::NO_NOTE_MANIFEST; MAX_ENTRIES];
        manifests[0] = old;
        let reg = Registry::register_installed(&doc, SHIPPED_HELD, &manifests);
        assert!(reg.rows()[0].endowment().is_some());

        let wants_clock = Manifest { clock: true, ..old };
        assert_eq!(
            reg.plan_at_fire(0, wants_clock),
            Admission::Unbacked(Unbacked::Clock),
        );
        assert!(
            matches!(
                reg.plan_at_fire(0, grant_plan::NO_NOTE_MANIFEST),
                Admission::Refused(_)
            ),
            "a version that takes no `--mem` refuses the line that passes one",
        );
    }

    /// **What a timetable can build at all** is what an image can carry, less what `fire` has no
    /// slot for.
    #[test]
    fn a_program_needing_what_no_timetable_endows_is_not_schedulable() {
        assert!(schedulable(&grant_plan::NO_NOTE_MANIFEST));
        assert!(schedulable(&Program::LeastAuthorityDemo.manifest()));
        for (what, m) in [
            (
                "entropy",
                Manifest {
                    entropy: true,
                    ..grant_plan::NO_NOTE_MANIFEST
                },
            ),
            (
                "the network",
                Manifest {
                    network: true,
                    ..grant_plan::NO_NOTE_MANIFEST
                },
            ),
            (
                "a configuration page",
                Manifest {
                    config: true,
                    ..grant_plan::NO_NOTE_MANIFEST
                },
            ),
        ] {
            assert!(!schedulable(&m), "a program that needs {what}");
        }
    }

    // ---- survivor triage of the 2026-10-03 census ---------------------------------------------

    /// An installed program's calendar line needs the wall clock before its manifest is asked, and
    /// asks it once there is one.
    #[test]
    fn an_installed_calendar_line_waits_for_the_clock_and_then_for_its_manifest() {
        let doc = parse("every day at 09:00 tick 7\n").unwrap();
        let mut manifests = [grant_plan::NO_NOTE_MANIFEST; MAX_ENTRIES];
        manifests[0] = Program::LeastAuthorityDemo.manifest();
        let without = Registry::register_installed(&doc, SHIPPED_HELD, &manifests);
        assert_eq!(
            without.rows()[0].admission,
            Admission::Unbacked(Unbacked::WallClock)
        );
        let with = Registry::register_installed(
            &doc,
            Held {
                clock: true,
                ..SHIPPED_HELD
            },
            &manifests,
        );
        assert!(matches!(with.rows()[0].admission, Admission::Fires(_)));
    }

    /// An installed program's file operand is planned against a directory, and backed only when the
    /// scheduler holds one. Planned with none held it would be refused as unnameable, which is a
    /// different answer from the honest "unbacked".
    #[test]
    fn an_installed_file_operand_is_planned_against_a_directory() {
        let doc = parse("every 5s wc report.txt\n").unwrap();
        let mut manifests = [grant_plan::NO_NOTE_MANIFEST; MAX_ENTRIES];
        manifests[0] = Program::Wc.manifest();
        let held = Held {
            dir: true,
            ..SHIPPED_HELD
        };
        let reg = Registry::register_installed(&doc, held, &manifests);
        assert!(
            matches!(reg.rows()[0].admission, Admission::Fires(_)),
            "{:?}",
            reg.rows()[0].admission
        );
        let without = Registry::register_installed(&doc, SHIPPED_HELD, &manifests);
        assert_eq!(
            without.rows()[0].admission,
            Admission::Unbacked(Unbacked::File)
        );
    }
}
