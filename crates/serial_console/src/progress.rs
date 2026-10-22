//! **How far did the boot get?**, decided from the console text alone.
//!
//! Every marker below is quoted from something in this tree rather than remembered: the bench
//! runbook in `notes/visionfive2.md` ("What appears, in order, on a good day" and the
//! failure-triage ladder), and the kernel's boot files and `kernel/src/panic.rs` for the lines that
//! are ours. Nothing here was invented, which matters more than usual, because a recogniser that
//! matches text no board ever prints fails in the direction that looks like success.
//!
//! **And every one of them has now been checked against the board**, which is a different and
//! better claim than "quoted from documentation" and was not available when this was written.
//! calef captured a full boot and a full failure on 2026-09-01; both are in
//! `tests/fixtures/captured/` and both are asserted on. The documentation was right about the
//! seven markers it named. It was silent about two things the board does, and both are here now:
//! U-Boot refusing outright before the kernel runs, and the line that means our whole boot tour
//! finished rather than merely started.
//!
//! # Two halves, and only one of them is this board's
//!
//! **The firmware prologue is a board profile and lives in [`crate::board`]** (milestone 324 part
//! 3). `U-Boot SPL`, OpenSBI, U-Boot proper and `Starting kernel ...` are radon's firmware chain
//! and nobody else's; they used to be four `if line.contains(...)` arms here and four variants of
//! [`Stage`], which is the hard-coding calef's ruling named. They are now declared as data, and a
//! boot that climbs them reaches [`Stage::Firmware`].
//!
//! **Everything from [`Stage::Banner`] up is the kernel's own ladder**, reachable on all three
//! architectures since milestone 268, and it is shared by every board this tool will ever watch.
//! `crates/boot_ladder` holds its markers; a board profile may not name one.
//!

//! # BUGS
//!
//! **A marker is shared; the fields inside the line are not, and that gap is silent.** The sweep's
//! seven heads are `crates/job_mix`'s own constants, so a kernel that renames one cannot disagree
//! with this file. Everything after the head is matched here by a string literal that exists
//! nowhere else: `tasks=`, `ticks_median=`, `rounds=`, `beat=`. A kernel that renames a field, or
//! inserts one, still prints a line this recogniser matches, and the parse reads **nothing** while
//! every test stays green.
//!
//! That is not hypothetical. On **2026-09-19** two sessions did it to each other inside a day. One
//! lane moved the markers into `crates/job_mix`; another changed
//! `kernel/src/job_mix.rs`'s point line from `ticks=<t> jpm=<r>` to a median of 21 repeats with its
//! two ends, so `ticks=` and `jpm=` stopped existing. The head `job-mix: tasks=` never moved.
//! Both branches were green: the parser and the committed fixture had been made from the same old
//! kernel and agreed with each other, and neither agreed with the kernel. It was found by reading,
//! which is rung zero of `AGENTS.md`'s ladder.
//!
//! Two things blunt it and neither closes it. The fixtures are **captures from a real boot** rather
//! than hand-written lines, so re-capturing catches a rename the moment somebody re-captures; and
//! this module's fields are spelled exactly as the wire spells them, so the two can be diffed by
//! eye. Sharing the field names the way the heads are shared is the fix, and it is not built here.
//!
//! **A sweep's longest legitimate silence is a measurement, and it belongs to the machine that
//! measured it.** The quiet timer that `script/job-mix` and `script/board-console` set against
//! [`Stage::Sweep`] is sized against the slowest subrun at the top of `job_mix::TASK_SWEEP`, since
//! a sweep has no wall-clock heartbeat to miss. Under TCG on 2026-09-19 that subrun was
//! 249,234,771 ticks on a 62.5 MHz counter, which is **4.0 seconds**, from the capture in
//! `tests/fixtures/captured/`. The default is sixty seconds, fifteen times it. That margin used to
//! be twenty to one against a 2.6-second subrun, and milestone 168 spent a quarter of it by taking
//! twenty-one repeats of a seven-kind mix instead of three of a five-kind one; a figure quoted from
//! a capture is only as current as the capture. A board outside the margin reads as wedged when it
//! is merely slow, and `--quiet-after 0` is the escape that gives up the detection entirely.

use core::fmt;

use crate::board;

/// How far the boot got, as an ordered ladder.
///
/// Ordered because the only question the tool ever asks is "did we reach at least here", and
/// because the sequence is what the runbook records: SPL, OpenSBI, U-Boot, handoff, ours. The
/// derived `Ord` is the comparison; do not reorder these variants without reading the runbook
/// again.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Stage {
    /// Nothing recognisable yet. Either the board is unpowered, the adapter is on the wrong pins,
    /// or the baud is wrong; the failure-triage ladder's first row covers all three.
    #[default]
    Cold,
    /// **A rung of the active board's firmware prologue** (milestone 324 part 3), which is
    /// whichever [`board::Profile`] this session was given.
    ///
    /// Four variants used to sit here instead (`Spl`, `OpenSbi`, `UBoot`, `Handoff`), and every one
    /// of them was radon's. They are now [`board::RADON`]'s four rungs, ordered by
    /// [`board::Rung::depth`], and a board with a different firmware chain declares its own rather
    /// than sharing radon's words. A board with no prologue this tool recognises, which is what
    /// xenon's capture shows, never reaches this stage at all and goes straight from
    /// [`Stage::Cold`] to [`Stage::Banner`].
    ///
    /// Below [`Stage::Banner`] because the firmware runs before the kernel does, whatever the
    /// board: that is what makes one ordering serve every profile.
    Firmware(&'static board::Rung),
    /// Our own banner (`boot_ladder::BANNER`). The kernel's console works, which on this board is
    /// not a given: the runbook is explicit that this line is the *second* target, after the
    /// DW-8250 driver work.
    ///
    /// **Reachable on all three architectures**, as every rung of the kernel's ladder is.
    Banner,
    /// The machine described itself and finished (`boot_ladder::MACHINE`), so the boot has read the
    /// device tree or the ACPI tables, counted the processors, found the console's interrupt line,
    /// the PCIe window and the IOMMU, and printed all of it.
    ///
    /// **A stronger claim than the banner and a weaker one than the self-test**, which is exactly
    /// what a rung is for: the banner says the console works, this says discovery worked, and the
    /// verdict below says the kernel works. A board that reaches the banner and stops here has a
    /// console and a machine it cannot read.
    Machine,
    /// The boot self-test printed its verdict (`boot_ladder::SELF_TEST`).
    ///
    /// **This is the stage to wait for**, and it is the one milestone 268 built: it is reachable on
    /// every architecture, it means paging, traps, the frame allocator, the timer with its
    /// interrupt, and the scheduler all came up, and the line it matches carries counts rather than
    /// a claim. A *failed* verdict is not this stage: it is [`Failure::SelfTestFailed`], so a
    /// degraded board on the bench does not read as a good one.
    SelfTest,
    /// The boot's middle stages ran to their end (`nife: the capability core runs on ...`).
    ///
    /// Printed after the self-test verdict and the tour on all three architectures, just before
    /// the hand-over, so it says the machine was described and every self-test check ran. It is
    /// not the default to wait for: [`Stage::Prompt`] is above it, and a boot that stops here has
    /// not handed over. Whether the checks passed is [`Failure::SelfTestFailed`]'s to say, not
    /// this rung's.
    Tour,
    /// Userspace is up and the shell is offering a prompt (`boot_ladder::PROMPT`).
    ///
    /// **The terminal state of a default boot** since milestone 268 (every architecture boots the
    /// same way): nothing halts, and the prompt
    /// rather than a halt is the signal that the boot finished. Above [`Stage::Tour`] because a
    /// boot that reaches a prompt has gone past any demonstration on the way.
    ///
    /// **Reachable on all three architectures since 2026-09-19**, on each one's *default* boot with
    /// an archive attached: aarch64 and riscv64 hand over at the end of the boot ladder's tour,
    /// and `x86_64` does too, once DECISIONS §149 (may the kernel answer on an endpoint) said how
    /// a shell reaches a console there and milestone 299 (the x86 port-range capability) made
    /// `console` a userspace driver holding one. It was aarch64-and-riscv64-only before that, which is stated
    /// here rather than left for a reader to discover from a watch that times out, and
    /// `cargo xtask boot-check` asserts it on every architecture now.
    Prompt,
    /// A sustained workload announced itself and is expected to keep speaking (milestone 219).
    ///
    /// **This is the only stage after which silence is a failure again.** Every stage below it is
    /// a step in a boot that ends with the kernel halting in `wfi`, so quiet after [`Stage::Tour`]
    /// is how a good boot ends and reporting it as a hang would fail every one. A soak is the
    /// opposite contract: `kernel/src/soak.rs` prints a heartbeat on the wall clock every five
    /// seconds whatever the workload is doing, so a gap says the thing that prints is itself
    /// wedged. See `watch::Policy::quiet_after`, which is where that asymmetry is implemented.
    Soak,
    /// **The job-mix sweep announced itself and is expected to keep making progress** (milestone
    /// 324 part 2, matching [`job_mix::STARTED`]).
    ///
    /// Like [`Stage::Soak`] in the way that matters: it is past the end of the boot tour, and
    /// silence from here is a failure rather than a kernel that halted on purpose. **Unlike it in
    /// the way that bites.** A soak beats on the wall clock, so a missed beat is a missed deadline.
    /// A sweep's finest progress line is [`job_mix::SUBRUN`], one per measured subrun, and a subrun
    /// takes as long as it takes; the longest legitimate silence is the slowest subrun at the top
    /// of [`job_mix::TASK_SWEEP`]. `watch::Policy::quiet_after` has to be set against that, and
    /// this module's `BUGS` carries the measured figure.
    ///
    /// Above [`Stage::Soak`] because the enum has to order them somehow and neither is reachable in
    /// the other's build: `script/job-mix`'s own `BUGS` records that the two features are
    /// alternatives and that a kernel carrying both would sweep and never soak. Nothing compares
    /// them, and nothing should.
    Sweep,
    /// **The sweep ran to its end** ([`job_mix::DONE`]), and the kernel is parking in `wfi`.
    ///
    /// **This is the rung milestone 324 part 2 exists for.** Before it, a finished sweep and a
    /// wedged one both ended as the clock running out, so they shared an exit status and a bench
    /// script could not tell them apart. Now `--until sweep-done` is `0` when the sweep finished,
    /// `2` when it spoke and then stopped, and `3` when the time ran out with points still to
    /// print.
    ///
    /// Silence after this is the correct end state, the same as [`Stage::Tour`]'s, and
    /// `watch::Policy::quiet_after` exempts it for the same reason.
    SweepDone,
}

impl Stage {
    /// The stage's name as a person would say it, for a report line.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Stage::Cold => "nothing recognisable",
            // The profile's own word for the rung, which is why a report can name `U-Boot SPL`
            // without this file knowing what a VisionFive 2 is.
            Stage::Firmware(rung) => rung.label(),
            Stage::Banner => "kernel banner",
            Stage::Machine => "machine described",
            Stage::SelfTest => "self-test verdict",
            Stage::Tour => "boot tour complete",
            Stage::Prompt => "shell prompt",
            Stage::Soak => "soak running",
            Stage::Sweep => "job-mix sweep running",
            Stage::SweepDone => "job-mix sweep complete",
        }
    }
}

impl fmt::Display for Stage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// A failure the console text names outright, so the tool can stop now instead of waiting out a
/// deadline that will tell it nothing new.
///
/// Every one of these is a *positive* statement printed by something that is still alive. A board
/// that has gone silent is not in here; silence is the watcher's business, not the recogniser's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// **The board's firmware gave up before the kernel ran**, carrying the profile's own
    /// diagnosis and, where that firmware puts the reason on the line before, that line.
    ///
    /// **One variant for a whole board profile** (milestone 324 part 3), where two used to be hard
    /// coded here: `BadImageMagic` for U-Boot rejecting the payload's header, and `UBootRefused`
    /// for `### ERROR ### Please RESET the board ###`. Both are U-Boot's words rather than ours, so
    /// they are [`board::Refusal`]s declared beside the rungs they belong to, and a board with
    /// different firmware refuses in its own words without a variant being added here.
    ///
    /// `reason` is empty when the firmware said only that it gave up.
    FirmwareRefused {
        /// One line naming what went wrong, from [`board::Refusal::diagnosis`].
        diagnosis: &'static str,
        /// The line before the refusal, when this firmware puts the reason there.
        reason: String,
    },
    /// `MEASURED BOOT REFUSED`: the kernel would not vouch for the archive it was handed. Boot 12
    /// (2026-08-15) is the worked example, and it was `script/board-image` building the pair in the
    /// wrong order rather than anything on the board.
    MeasuredBootRefused,
    /// `[PANIC] ...`: our own panic handler (`kernel/src/panic.rs`), carrying its message.
    KernelPanic(String),
    /// **`nife self-test: 4 of 5 passed, 1 FAILED: <names>`**: the kernel tested itself on this
    /// machine and one of the checks did not pass, carrying the names of the ones that did not.
    ///
    /// **This is a failure and not a stage**, which is milestone 268's item 5: a degraded board on
    /// the bench must not read as a good one, and a boot that reaches the verdict red has reached
    /// something different from a boot that reaches it green.
    ///
    /// **The boot did not stop**, and that is the one thing about this variant a reader has to
    /// know. The self-test reports and does not gate (calef, 2026-09-09: a machine you cannot log
    /// into is a machine you cannot fix), so the kernel carried on to userspace and a prompt may
    /// well be waiting. What this says is that something the kernel needs is broken, and the board
    /// is worth looking at rather than worth using.
    SelfTestFailed(String),
    /// **The kernel would not start the job-mix sweep** ([`job_mix::FAILED`]), carrying the reason
    /// it gave (milestone 324 part 2).
    ///
    /// Printed before [`Stage::Sweep`] and followed by a halt, in all three cases
    /// `kernel/src/job_mix.rs` has: no `job_mix_task` in the archive, or either kind of spawn
    /// failing. It is a failure rather than a stage for milestone 268's item 5's reason: a board
    /// that refused to run the workload must not read like one that ran it.
    SweepFailed(String),
}

impl Failure {
    /// One line naming what went wrong, for the report and the exit message.
    #[must_use]
    pub fn describe(&self) -> String {
        match self {
            // The profile wrote the sentence; this only decides whether the reason is appended.
            // `notes/board-console.md` has where each board's wording came from.
            Failure::FirmwareRefused { diagnosis, reason } if reason.is_empty() => {
                (*diagnosis).to_string()
            }
            Failure::FirmwareRefused { diagnosis, reason } => format!("{diagnosis}: {reason}"),
            // Worded carefully, because this is the one failure that is not a defect. The gate
            // did its job: it noticed that the archive on the card is not the one this kernel was
            // built to vouch for, and halted instead of running it. A report that read like a
            // crash would send somebody debugging the boot mechanism, when what is wrong is that
            // two files came from different builds.
            Failure::MeasuredBootRefused => "the measured-boot gate refused the archive and \
                 halted, which is the gate working rather than a crash: the kernel and the archive \
                 on the card are from different builds. Rebuild both with script/board-image, \
                 which orders those steps"
                .to_string(),
            Failure::KernelPanic(message) => format!("the kernel panicked: {message}"),
            // Worded to say what happened next, because the obvious reading of "a self-test
            // failed" is that the machine stopped, and it did not. Someone acting on this report
            // needs to know the board is still up and is worth logging into.
            Failure::SelfTestFailed(names) => format!(
                "the kernel's boot self-test failed on this machine: {names}. The boot continued to \
                 userspace anyway (it reports, it does not gate), so the board is up and degraded \
                 rather than dead"
            ),
            // Worded to say that nothing ran, because the sweep's own output is absent either way
            // and an empty log reads the same as a wedge until this line is found.
            Failure::SweepFailed(why) => format!(
                "the kernel refused to start the job-mix sweep and halted: {why}. No point of the \
                 sweep was measured"
            ),
        }
    }
}

/// One soak heartbeat's numbers (milestone 219), as the kernel printed them.
///
/// Every field is cumulative except [`rate`](Self::rate), which is the last interval's. The one a
/// later run is compared against is [`rounds`](Self::rounds); the rest are what make it
/// interpretable, and [`refused`](Self::refused) is the one that is a finding rather than a
/// statistic.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SoakBeat {
    /// Seconds since the soak started, by the kernel's own timer.
    pub seconds: u64,
    /// Which heartbeat this is. A gap in this sequence means console output was lost, which is a
    /// different fault from the workload stopping and is worth being able to tell apart.
    pub beat: u64,
    /// **Cumulative IPC round trips completed by every worker.** The comparable number.
    pub rounds: u64,
    /// Round trips per second over the last interval.
    pub rate: u64,
    /// Cumulative refused wakes. Expected to be zero, and a nonzero value is the defect
    /// `design/fatal-risks/README.md`'s multicore entry exists for.
    pub refused: u64,
    /// Cumulative wrong replies seen by callers. Expected to be zero.
    pub mismatches: u64,
    /// How many workers made no progress in the last interval. Expected to be zero.
    pub stalled: u64,
    /// **Cumulative times a thread ran on a different core than the one it last ran on.** The
    /// honest cross-core handoff count; see `kernel/src/soak.rs` and `notes/soak.md` for the
    /// measured reason a steady-state workload barely moves this at all.
    pub crossings: u64,
    /// Cumulative wakes and placements that named a remote core. Narrower than
    /// [`crossings`](Self::crossings) and kept beside it because the two disagreeing is the
    /// finding: a rendezvous wake queues its peer locally, so a migration it performs is invisible
    /// here.
    pub remote: u64,
    /// **Cumulative tick-route wakes** (milestone 221), and the reason a soak crosses cores at all.
    ///
    /// A soak build signals a rendezvous from `sched::on_tick`, and one worker per group blocks on
    /// it through `Irq::WAIT`; every wake takes the real device-interrupt path down through
    /// `sched::wake_load_aware`, which is where this project's one observed multicore defect was.
    ///
    /// Zero in a log from before milestone 221, which is exactly how a reader (and the summary
    /// below) tells a run whose scheduler had nothing to make it cross from a run that did and
    /// still did not.
    ///
    /// It is **not** part of [`rounds`](Self::rounds), because a wake is not a round trip.
    pub wakes: u64,
}

/// **One completed point of the job-mix sweep**, as the kernel printed it (milestone 324 part 2,
/// from a [`job_mix::POINT`] line).
///
/// One per entry in [`job_mix::TASK_SWEEP`], carrying the [`job_mix::Spread`] of that entry's
/// [`job_mix::REPEATS`] subruns and the jobs-per-minute figure the kernel computed from the median.
/// See `crates/job_mix` for what a jobs-per-minute figure is and is not, and `notes/job-mix.md` for
/// why one boot's is a draw rather than a result.
///
/// # Why all seven fields, when liveness needs none of them
///
/// Counting points is the ratchet's job, not this struct's: a watcher deciding whether a sweep is
/// moving reads [`Stage`] and [`SweepSubrun`]. This exists to be **reported**, which is the one
/// place a person reads a sweep's numbers without opening the log, and that is what settles the
/// field list.
///
/// The kernel prints the fastest and slowest repeats beside the median *so the spread is never
/// hidden* (`job_mix::REPEATS` carries the argument, and milestone 168 changed the statistic for
/// exactly that reason). A reader that kept only the median would re-hide it at the last step, and
/// would have reintroduced the defect one line away from where it was fixed.
/// [`repeats`](Self::repeats) is here for the same reason and is the one easiest to think
/// unnecessary: a median of 21 and a median of 3 are different claims, and a struct that drops the
/// count lets a transcript from either be quoted as the other.
///
/// The field names are the wire's, so the struct can be diffed against a [`job_mix::POINT`] line by
/// eye. That is not tidiness; it is the cheapest defence available against this module's first
/// `BUGS` entry.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SweepPoint {
    /// Tasks released for this point, an entry of [`job_mix::TASK_SWEEP`].
    pub tasks: u64,
    /// Jobs the pool completed, which is `tasks` times [`job_mix::JOBS_PER_TASK`].
    pub jobs: u64,
    /// Subruns measured at this point, which is [`job_mix::REPEATS`] on any kernel that printed
    /// the field at all. Read from the line rather than assumed, because the whole point of
    /// reading it is to catch a log whose kernel disagrees with this build.
    pub repeats: u64,
    /// The fastest repeat's wall-clock ticks, on the kernel's own counter.
    pub ticks_min: u64,
    /// The median repeat's ticks, which is the statistic [`jpm_median`](Self::jpm_median) is
    /// computed from.
    pub ticks_median: u64,
    /// The slowest repeat's ticks. With [`ticks_min`](Self::ticks_min) it is the spread, and the
    /// spread is what says whether the median means anything on this machine.
    pub ticks_max: u64,
    /// Jobs per minute at the median, as the kernel computed it.
    pub jpm_median: u64,
}

/// **One completed subrun of the sweep** (a [`job_mix::SUBRUN`] line).
///
/// Finer than [`SweepPoint`] by a factor of [`job_mix::REPEATS`], and it is the finest progress a
/// sweep emits. That is what makes it worth keeping separately: a sweep still inside a point has
/// printed one of these and no point at all, and a watcher deciding whether a sweep is moving has
/// nothing else to look at.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SweepSubrun {
    /// Tasks released for the point this subrun belongs to.
    pub tasks: u64,
    /// Which repeat of that point this was, counted from zero.
    pub repeat: u64,
    /// Its wall-clock ticks, on the kernel's own counter.
    pub ticks: u64,
}

/// The ratchet: how far the boot got, and the first failure it announced.
///
/// It only ever moves forward. That is what lets the caller re-offer a partial line as more bytes
/// arrive without the recogniser double-counting, which it must do because U-Boot's `StarFive #`
/// prompt has no newline after it and would otherwise never be seen.
#[derive(Debug, Clone)]
pub struct BootProgress {
    /// **Which board's firmware prologue to expect** (milestone 324 part 3). Fixed for the life of
    /// the ratchet: a session watches one board, and a profile swapped halfway through would make
    /// two rungs at the same depth compare equal while meaning different things.
    board: &'static board::Profile,
    reached: Stage,
    failure: Option<Failure>,
    relocated: bool,
    userspace_ran: bool,
    banner_line: Option<String>,
    /// The machine description's summary line (milestone 268), so a report can say what machine
    /// answered without a reader going back to the log. `None` until [`Stage::Machine`].
    machine_line: Option<String>,
    /// The self-test verdict exactly as it arrived (milestone 268), counts and all. Kept even when
    /// it is green, because "five of five" and "one of one" are different facts about a build and
    /// the difference is what a vacuous pass looks like.
    self_test_line: Option<String>,
    /// The most recent soak heartbeat's numbers (milestone 219), so the tool can put the run's own
    /// figure in its summary instead of making a reader go back to a log for it. `None` until a
    /// heartbeat has been seen and parsed.
    soak: Option<SoakBeat>,
    /// The last job-mix point that printed (milestone 324 part 2). `None` until one has.
    sweep_point: Option<SweepPoint>,
    /// The last job-mix subrun that printed. `None` until one has.
    sweep_subrun: Option<SweepSubrun>,
    /// The last complete non-empty line, kept for exactly one reason: U-Boot's `### ERROR ###`
    /// says that it gave up and the line before it says why, and a reader handed only the first
    /// half has to go back to the log to learn anything.
    last_line: String,
}

/// A ratchet for [`board::RADON`], which is what every caller watched before board profiles
/// existed and what `watch::Policy::default` still chooses.
///
/// **A default that names a board is a foot gun and says so.** A report from a session that took
/// this default over an emulator will say `U-Boot SPL` was never reached, which is true and
/// useless. Prefer [`BootProgress::new`] with the board in hand; see [`board`]'s `BUGS` for why
/// QEMU's callers do not bother.
impl Default for BootProgress {
    fn default() -> Self {
        Self::new(&board::RADON)
    }
}

impl BootProgress {
    /// A fresh ratchet for one board, before any byte has arrived.
    #[must_use]
    pub fn new(board: &'static board::Profile) -> Self {
        Self {
            board,
            reached: Stage::default(),
            failure: None,
            relocated: false,
            userspace_ran: false,
            banner_line: None,
            machine_line: None,
            self_test_line: None,
            soak: None,
            sweep_point: None,
            sweep_subrun: None,
            last_line: String::new(),
        }
    }

    /// The board profile this ratchet was given.
    #[must_use]
    pub fn board(&self) -> &'static board::Profile {
        self.board
    }

    /// The furthest stage recognised so far.
    #[must_use]
    pub fn reached(&self) -> Stage {
        self.reached
    }

    /// The first announced failure, if the board announced one.
    #[must_use]
    pub fn failure(&self) -> Option<&Failure> {
        self.failure.as_ref()
    }

    /// Whether a **pre-milestone-295** kernel's userspace init built its child
    /// (`init/build  : ...`, which `kernel/src/lib.rs` printed until 2026-09-14).
    ///
    /// **This answers a question about captured logs, not about a live board**, and that is the
    /// whole of what milestone 295 changed here. calef retired `components/src/builder.rs` on
    /// 2026-09-14, so no kernel this tree builds prints `init/build` any more and this is `false`
    /// on every live boot. The matcher stays because
    /// `tests/fixtures/captured/vf2-2026-09-01-userspace.log` carries the line: that is evidence
    /// off real VisionFive 2 silicon and cannot be re-taken with a different kernel, and a
    /// recogniser that could no longer read it would throw the evidence away to tidy the code.
    ///
    /// **The live rung that replaced it is [`Stage::Prompt`]** (`boot_ladder::PROMPT`), and it says
    /// more: `init/build` meant userspace built one child from two capabilities, where a prompt
    /// cannot appear unless userspace built the console server, the line discipline, the input
    /// driver and the shell. Ask `reached() >= Stage::Prompt` of a board booted today.
    ///
    /// Not a stage, and deliberately, because the ladder has to stay a ladder: a kernel with no
    /// archive on the card ran its whole tour and never reached this, so putting it below
    /// [`Stage::Tour`] would make reaching the tour imply something that did not happen. It is a
    /// detail of a successful boot, like [`Self::is_relocated`], and it is the difference between
    /// the two successful captures.
    #[must_use]
    pub fn userspace_ran(&self) -> bool {
        self.userspace_ran
    }

    /// Whether U-Boot said it moved the image (`Moving Image from ...`).
    ///
    /// Not a stage, because the runbook lists it as a *discriminator* rather than a step: it is
    /// what you check when `Starting kernel ...` is followed by silence.
    #[must_use]
    pub fn is_relocated(&self) -> bool {
        self.relocated
    }

    /// The latest soak heartbeat, if this session saw one.
    ///
    /// **This is the number milestone 219 is for.** A soak that ends with nothing printed proves
    /// very little; one that ends with a round-trip total is something a later run can be compared
    /// against. It is a progress figure and nothing else: see
    /// `design/roadmap/0219-a-workload-that-does-not-stop.md` for why a clean run is weak evidence.
    #[must_use]
    pub fn soak(&self) -> Option<&SoakBeat> {
        self.soak.as_ref()
    }

    /// The last job-mix sweep point that printed, if this session saw one (milestone 324 part 2).
    ///
    /// A bench script reading this gets the curve's latest entry without re-parsing the log. What
    /// it does **not** get is the whole curve: this is a progress ratchet, and the points are in
    /// the capture. `crates/serial_console::lottery` is the precedent for reading a finished log
    /// for its results.
    #[must_use]
    pub fn sweep_point(&self) -> Option<&SweepPoint> {
        self.sweep_point.as_ref()
    }

    /// The last job-mix subrun that printed, if this session saw one.
    ///
    /// The finest sign a sweep is still moving, and the only one during the long subruns at the top
    /// of [`job_mix::TASK_SWEEP`], where no point line is due for minutes.
    #[must_use]
    pub fn sweep_subrun(&self) -> Option<&SweepSubrun> {
        self.sweep_subrun.as_ref()
    }

    /// Our banner line exactly as it arrived, which is how the reader learns which architecture
    /// the board that answered actually is.
    #[must_use]
    pub fn banner_line(&self) -> Option<&str> {
        self.banner_line.as_deref()
    }

    /// The machine description's summary line, if this session saw one (milestone 268). It names
    /// the architecture, the processor count, the memory size and the tick rate, which is the
    /// shortest honest answer to "what did I just boot".
    #[must_use]
    pub fn machine_line(&self) -> Option<&str> {
        self.machine_line.as_deref()
    }

    /// The self-test verdict exactly as the kernel printed it, if this session saw one (milestone
    /// 268), whether it was green or red.
    ///
    /// **Kept on a green boot too**, deliberately: the counts are the only defence against a
    /// vacuous pass, and a reader comparing two runs needs to see that both ran the same number of
    /// checks. A red verdict also appears as [`Failure::SelfTestFailed`]; this is the raw line.
    #[must_use]
    pub fn self_test_line(&self) -> Option<&str> {
        self.self_test_line.as_deref()
    }

    /// Offer a line that ended with a line terminator.
    ///
    /// Idempotent: stages ratchet and the failure is recorded once, so the same text may be
    /// offered any number of times.
    pub(crate) fn observe_line(&mut self, line: &str) {
        self.observe(line, true);
    }

    /// Offer the incomplete tail, the bytes after the last line terminator.
    ///
    /// This exists because U-Boot's `StarFive #` prompt is printed with no newline after it, so a
    /// tool that waited for complete lines would sit there while the board sat waiting for it.
    ///
    /// **A partial line is not the same evidence as a complete one**, which is the distinction the
    /// two methods exist to keep, and it was found by a test feeding a good boot one byte at a
    /// time rather than by reasoning. Two things go wrong if a tail is treated as a line. `U-Boot `
    /// on its own looks like U-Boot proper right up until the next three bytes turn out to be
    /// `SPL`, so a board dying in SPL is reported two stages further along than it got. And any
    /// marker carrying a *payload* captures a truncated one: the tail `nife on ` recorded the
    /// banner as the empty string, and `[PANIC] ` recorded a panic with no message, both of them
    /// latched before the rest of the line arrived. So a tail may ratchet a stage, because a
    /// substring match is monotone and more bytes cannot unmake it; it may not settle a word
    /// boundary, and it may not capture text.
    pub(crate) fn observe_partial(&mut self, tail: &str) {
        self.observe(tail, false);
    }

    fn observe(&mut self, line: &str, complete: bool) {
        let line = line.trim_end_matches(['\r', '\n']);

        self.observe_stages(line, complete);
        // Failures. Recorded once: the first thing that went wrong is the one worth reporting,
        // and everything after it is downstream.
        if self.failure.is_none() {
            self.failure = self.failure_in(line, complete);
        }

        // Last, and only for a complete line: this is what the NEXT line may need, so recording a
        // partial here would hand the refusal a truncated reason.
        if complete && !line.trim().is_empty() {
            self.last_line = line.trim().to_string();
        }
    }

    /// Ratchet every stage `line` shows, and capture what a complete line carries: the banner,
    /// machine and self-test lines, the soak heartbeat and the sweep's numbers.
    fn observe_stages(&mut self, line: &str, complete: bool) {
        // **The firmware prologue, from the board profile** (milestone 324 part 3). Four `if`s
        // with radon's markers written into them used to stand here; the rungs are now declared in
        // `crate::board` and this loop is the whole of what reads them, which is what makes adding
        // a board a profile rather than an edit to a recogniser.
        //
        // Every rung is offered the line rather than stopping at the first match, because the
        // ratchet is what decides and a log can carry two rungs on one line.
        for rung in self.board.prologue {
            if rung.is_seen_in(line, complete) {
                self.reach(Stage::Firmware(rung));
            }
        }
        // **Every marker below is `boot_ladder`'s rather than a literal** (milestone 268), so this
        // recogniser and the kernel cannot hold two copies of one contract. Before that crate the
        // only marker that existed was a string literal in one architecture's arm of
        // `kernel/src/lib.rs` and a second copy of it here, which is exactly how finding 3 hid.
        //
        // `nife on ` rather than the RISC-V line specifically: the kernel's boot prints one of
        // these per architecture, and a recogniser that only knew the VisionFive 2's would report
        // a healthy aarch64 or x86_64 board as never having booted. The full line is kept so the
        // reader sees which one answered, and only from a complete line, or it is kept truncated.
        if let Some(at) = line.find(boot_ladder::BANNER) {
            self.reach(Stage::Banner);
            if complete && self.banner_line.is_none() {
                self.banner_line = Some(line[at..].to_string());
            }
        }
        // The machine description's closing summary (milestone 268). Its *last* line rather than
        // its first, so reaching this rung means the whole block printed.
        if let Some(at) = line.find(boot_ladder::MACHINE) {
            self.reach(Stage::Machine);
            // `&&`, not `||`: kept the same way `banner_line` is, from first arrival only. A
            // second machine line in one session is not expected, but the field's own doc says
            // "the machine description's summary line", singular, and this is what keeps that
            // true rather than accidental.
            if complete && self.machine_line.is_none() {
                self.machine_line = Some(line[at..].to_string());
            }
        }
        // The self-test verdict (milestone 268). The stage ratchets on a partial line, because a
        // substring match is monotone and more bytes cannot unmake it; the *text* is only captured
        // from a complete line, and so is the failure below, because a truncated verdict would
        // record a count that has not finished arriving.
        if let Some(at) = line.find(boot_ladder::SELF_TEST) {
            self.reach(Stage::SelfTest);
            if complete && self.self_test_line.is_none() {
                self.self_test_line = Some(line[at..].to_string());
            }
        }
        // The shell's banner, which is the top rung: userspace is up and a prompt is coming. See
        // `boot_ladder::PROMPT`'s own BUGS for why the banner rather than the `$ ` itself.
        if line.contains(boot_ladder::PROMPT) {
            self.reach(Stage::Prompt);
        }

        if self
            .board
            .relocation
            .iter()
            .any(|marker| line.contains(marker))
        {
            self.relocated = true;
        }
        // A captured-log marker rather than a live one since milestone 295: no kernel prints
        // `init/build` after `components/src/builder.rs` was retired on 2026-09-14. Kept because
        // the VisionFive 2 capture carries it; see [`BootProgress::userspace_ran`].
        if line.contains("init/build") {
            self.userspace_ran = true;
        }
        if let Some(at) = line.find(boot_ladder::TOUR) {
            self.reach(Stage::Tour);
            if complete && self.banner_line.is_none() {
                self.banner_line = Some(line[at..].to_string());
            }
        }
        // The soak (milestone 219). `soak-test: started` is `kernel/src/soak.rs`'s `START_MARKER`, and
        // the two agree by one of them being tested against the other's text rather than by both
        // being remembered. Ratcheting on the START line rather than on any `soak:` line is
        // deliberate: a `soak-test: FAILED` line reaches the failure arm below and should not also be
        // read as the workload having got going.
        if line.contains("soak-test: started") {
            self.reach(Stage::Soak);
        }
        if complete && line.contains("soak-test: t=") {
            self.observe_soak_beat(line);
        }
        // **The job-mix sweep** (milestone 324 part 2). Every marker is `crates/job_mix`'s rather
        // than a literal, the same move milestone 268 made when it put the boot ladder's markers in
        // `crates/boot_ladder`: the kernel prints these very constants, so this recogniser and the
        // thing it recognises cannot hold two copies of one contract. The census prefix
        // (`job-mix-census:`) is outside all of them, which is what keeps a census block from
        // reading as a sweep starting.
        if line.contains(job_mix::STARTED) {
            self.reach(Stage::Sweep);
        }
        if line.contains(job_mix::DONE) {
            self.reach(Stage::SweepDone);
        }
        // Complete lines only, for both. A stage ratchet is monotone so a partial may set it; a
        // captured number is not, and a truncated `ticks=` would record a figure that has not
        // finished arriving. Same rule as the self-test verdict above.
        if complete && let Some(at) = line.find(job_mix::SUBRUN) {
            self.observe_sweep_subrun(&line[at..]);
        }
        if complete && let Some(at) = line.find(job_mix::POINT) {
            self.observe_sweep_point(&line[at..]);
        }
    }

    /// The failure `line` announces, if any, in the order the arms are tried: the firmware's
    /// refusals, measured boot, the sweep, a panic, then the self-test verdict.
    fn failure_in(&self, line: &str, complete: bool) -> Option<Failure> {
        // **The firmware's own refusals, from the board profile** (milestone 324 part 3).
        // First, because a firmware that gave up did so before the kernel ran and everything
        // after it is downstream. `reason_is_the_line_before` deliberately reads `last_line`
        // before `observe` updates it for this line, because U-Boot puts the refusal on one line
        // and its reason on the one before.
        if let Some(refusal) = self
            .board
            .refusals
            .iter()
            .find(|refusal| line.contains(refusal.marker))
        {
            Some(Failure::FirmwareRefused {
                diagnosis: refusal.diagnosis,
                reason: if refusal.reason_is_the_line_before {
                    self.last_line.clone()
                } else {
                    String::new()
                },
            })
        } else if line.contains("MEASURED BOOT REFUSED") {
            Some(Failure::MeasuredBootRefused)
        } else if complete && let Some(at) = line.find(job_mix::FAILED) {
            // Before the panic arm because the sweep's refusal is its own announcement and
            // carries a better reason than the halt that follows it.
            Some(Failure::SweepFailed(
                line[at + job_mix::FAILED.len()..].trim().to_string(),
            ))
        } else if complete && let Some(at) = line.find("[PANIC] ") {
            Some(Failure::KernelPanic(
                line[at + "[PANIC] ".len()..].to_string(),
            ))
        } else if complete
            && let Some(at) = line.find(boot_ladder::SELF_TEST)
            && let Some(failed) = line[at..].find(boot_ladder::SELF_TEST_FAILED)
        {
            // **Both markers, on one complete line.** `FAILED:` on its own is a word the boot
            // tour has used since milestone 267 for its preemption check, so matching it alone
            // would report that unrelated line as a self-test failure; and the verdict prefix
            // alone is every green boot. The names are what follows the token.
            let names = line[at + failed + boot_ladder::SELF_TEST_FAILED.len()..].trim();
            Some(Failure::SelfTestFailed(names.to_string()))
        } else if complete
            && let Some(at) = line.find(boot_ladder::SELF_TEST)
            && let Some(name) = Self::verdict_absences(&line[at..])
                .find(|name| self.board.must_not_be_absent.contains(name))
        {
            // **An absence this board does not allow** (calef's ruling of 2026-10-20 (UTC) on
            // #2447). The kernel reports a check `absent` when the machine has nothing for it to
            // test, and only the board's profile knows whether this machine should have had it.
            Some(Failure::SelfTestFailed(format!(
                "{name} (absent on {}, which has the device for it)",
                self.board.name
            )))
        } else if complete
            && let Some(at) = line.find(boot_ladder::SELF_TEST)
            && let Some(total) = Self::verdict_total(&line[at + boot_ladder::SELF_TEST.len()..])
            && total != boot_ladder::SELF_TEST_CHECKS.len()
            && !boot_ladder::SELF_TEST_CHECKS_EARLIER.contains(&total)
        {
            // **A green verdict over the wrong set is not green** (milestone 268). The kernel
            // counts against `boot_ladder::SELF_TEST_CHECKS`, so a total that is neither that
            // list's length nor one it had before (`SELF_TEST_CHECKS_EARLIER`, for logs older
            // kernels printed) is a kernel built against a list this tree never had, such as one
            // architecture's set cut and the count adjusted to hide it, which printed
            // `4 of 4 passed`.
            Some(Failure::SelfTestFailed(format!(
                "the verdict counted {total} checks and boot_ladder lists {}",
                boot_ladder::SELF_TEST_CHECKS.len()
            )))
        } else {
            None
        }
    }

    /// The names a verdict lists after `boot_ladder::SELF_TEST_ABSENT`, given the verdict line
    /// from its `boot_ladder::SELF_TEST`. The list ends at the comma before a `FAILED:` count, or
    /// at the end of the line.
    fn verdict_absences(verdict: &str) -> impl Iterator<Item = &str> {
        let names = verdict
            .find(boot_ladder::SELF_TEST_ABSENT)
            .map_or("", |at| {
                let rest = &verdict[at + boot_ladder::SELF_TEST_ABSENT.len()..];
                rest.split(',').next().unwrap_or("")
            });
        names.split_whitespace()
    }

    /// The `Y` in a verdict's `X of Y passed`, given the text after `boot_ladder::SELF_TEST`.
    ///
    /// `None` for anything that is not that shape, so an unfamiliar verdict is left alone rather
    /// than reported as a count it never printed.
    fn verdict_total(tail: &str) -> Option<usize> {
        let mut words = tail.split_whitespace();
        words.next()?.parse::<usize>().ok()?;
        if words.next()? != "of" {
            return None;
        }
        words.next()?.trim_end_matches(',').parse().ok()
    }

    /// Pull the numbers out of one `soak-test: t=... beat=... rounds=...` line.
    ///
    /// Field-name-directed rather than positional, so adding a field to the kernel's heartbeat does
    /// not silently shift what this reads. A field that is missing or unparseable leaves the
    /// previous value in place rather than zeroing it, because a garbled line on a serial link is
    /// a lost measurement, not a measurement of zero.
    fn observe_soak_beat(&mut self, line: &str) {
        let field = |name: &str| field(line, name);
        let beat = self.soak.get_or_insert_with(SoakBeat::default);
        for (name, slot) in [
            ("t=", &mut beat.seconds),
            ("beat=", &mut beat.beat),
            ("rounds=", &mut beat.rounds),
            ("rate=", &mut beat.rate),
            ("refused=", &mut beat.refused),
            ("mismatch=", &mut beat.mismatches),
            ("stalled=", &mut beat.stalled),
            ("crossings=", &mut beat.crossings),
            ("remote=", &mut beat.remote),
            ("wakes=", &mut beat.wakes),
        ] {
            if let Some(v) = field(name) {
                *slot = v;
            }
        }
    }

    /// Pull the numbers out of one `job-mix-repeat: tasks=... repeat=... ticks=...` line.
    ///
    /// Field-name-directed, for [`Self::observe_soak_beat`]'s reason: a field added to the kernel's
    /// line must not silently shift what this reads. A line missing a field leaves the previous
    /// value in place rather than zeroing it, because a garbled line on a serial link is a lost
    /// measurement and not a measurement of zero.
    fn observe_sweep_subrun(&mut self, tail: &str) {
        let subrun = self.sweep_subrun.get_or_insert_with(SweepSubrun::default);
        for (name, slot) in [
            ("tasks=", &mut subrun.tasks),
            ("repeat=", &mut subrun.repeat),
            ("ticks=", &mut subrun.ticks),
        ] {
            if let Some(v) = field(tail, name) {
                *slot = v;
            }
        }
    }

    /// Pull the numbers out of one `job-mix: tasks=... jobs=... repeats=... ticks_min=...
    /// ticks_median=... ticks_max=... jpm_median=...` line.
    ///
    /// Field-name-directed, for [`Self::observe_soak_beat`]'s reason, and **that is not the same as
    /// safe**: this list held `ticks=` and `jpm=` for a day after the kernel stopped printing
    /// either, and nothing said so. The module's `BUGS` has it. A line missing a field leaves the
    /// previous value in place rather than zeroing it, because a garbled line on a serial link is a
    /// lost measurement and not a measurement of zero.
    ///
    /// The seven names are all distinct as substrings, which is what makes [`field`]'s `find` the
    /// right tool here: `ticks_min=`, `ticks_median=` and `ticks_max=` share a prefix and none is a
    /// prefix of another, so no search can land on a neighbour's digits.
    fn observe_sweep_point(&mut self, tail: &str) {
        let point = self.sweep_point.get_or_insert_with(SweepPoint::default);
        for (name, slot) in [
            ("tasks=", &mut point.tasks),
            ("jobs=", &mut point.jobs),
            ("repeats=", &mut point.repeats),
            ("ticks_min=", &mut point.ticks_min),
            ("ticks_median=", &mut point.ticks_median),
            ("ticks_max=", &mut point.ticks_max),
            ("jpm_median=", &mut point.jpm_median),
        ] {
            if let Some(v) = field(tail, name) {
                *slot = v;
            }
        }
    }

    fn reach(&mut self, stage: Stage) {
        if stage > self.reached {
            self.reached = stage;
        }
    }

    // `>` rather than `>=` above is deliberately not load-bearing, and that is worth recording
    // rather than leaving for the next reader to wonder about. `stage == self.reached` can only
    // hold two ways: same variant with no data, where reassigning changes nothing observable;
    // or two `Stage::Firmware` values whose rungs compare equal, which (per `board::Rung`'s own
    // `PartialEq`, depth alone) means same depth. Within one session every `Stage::Firmware` is
    // drawn from the one profile `BootProgress::new` was given, and
    // `every_profiles_depths_count_from_one_without_gaps` (in `board`) is what makes a depth
    // point at exactly one rung in that profile. So an "equal" `Stage::Firmware` is the *same*
    // `&'static Rung`, and reassigning it to itself is not observable either. `>=`'s extra
    // branch (firing when `stage == self.reached`) is therefore equivalent to `>`'s here, and a
    // mutant that makes this `>=` is expected to survive.
}

/// The decimal run after `name=` in `text`, if there is one.
///
/// Shared by the soak's heartbeat and the sweep's two lines, which is not a tidiness: all three are
/// `name=value` lines from the same console, and a second copy of this would be a second place for
/// a trailing `/s` or a comma to be got wrong.
pub(crate) fn field(text: &str, name: &str) -> Option<u64> {
    let at = text.find(name)?;
    let rest = &text[at + name.len()..];
    let end = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    rest[..end].parse().ok()
}

/// Splits a byte stream into lines, and hands back the incomplete tail as well.
///
/// The tail is the reason this exists rather than a call to [`str::lines`]. U-Boot's `StarFive #`
/// prompt is printed with no newline after it, so a console tool that only recognises complete
/// lines waits forever at exactly the moment the board is waiting for *it*.
#[derive(Debug, Default)]
pub(crate) struct LineFeeder {
    pending: String,
}

impl LineFeeder {
    /// A fresh splitter.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The bytes seen since the last line terminator, which have not been offered as a line.
    ///
    /// A caller that has stopped reading should hand this to [`BootProgress::observe_line`]: at
    /// the end of a session there is nothing more coming, so the tail is as complete as it will
    /// ever be. That is what catches a last line printed without a newline, which a panic followed
    /// by a halted machine can be.
    #[must_use]
    pub fn tail(&self) -> &str {
        &self.pending
    }

    /// Feed a chunk of bytes; returns every line it completed, plus the incomplete tail.
    ///
    /// Bytes are decoded lossily on purpose. A wrong baud rate produces bytes that are not UTF-8,
    /// and that is a case the tool must survive and report (runbook row two) rather than a case it
    /// may fail on.
    ///
    /// Both `\n` and `\r` end a line, because a serial console emits `\r\n` and because a firmware
    /// progress display emits bare `\r` to overwrite itself.
    pub fn feed(&mut self, bytes: &[u8]) -> Feeding {
        self.pending.push_str(&String::from_utf8_lossy(bytes));
        let mut lines = Vec::new();
        while let Some(at) = self.pending.find(['\n', '\r']) {
            let line = self.pending[..at].to_string();
            self.pending.drain(..=at);
            lines.push(line);
        }
        Feeding {
            lines,
            tail: self.pending.clone(),
        }
    }
}

/// What one chunk of bytes turned into: the lines it completed and the tail still in flight.
#[derive(Debug)]
pub(crate) struct Feeding {
    /// Lines that ended with `\n` or `\r`, in order.
    pub lines: Vec<String>,
    /// The bytes after the last line ending, which may still grow.
    pub tail: String,
}

#[cfg(test)]
mod tests;
