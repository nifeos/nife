//! **How far did the boot get?**, decided from the console text alone.
//!
//! The firmware prologue, up to [`Stage::Firmware`], is a board profile in [`crate::board`].
//! Everything from [`Stage::Banner`] up is the kernel's own ladder and is shared by every board:
//! `crates/boot_ladder` and `crates/job_mix` hold its markers, and a profile may not name one.
//! Every marker is quoted from this tree and asserted against a capture in
//! `tests/fixtures/captured/`; notes/board-console/markers.md gives each one's source.
//!

//! # BUGS
//!
//! - The sweep's marker heads are `crates/job_mix`'s constants, but the fields after them
//!   (`tasks=`, `ticks_median=`, `rounds=`, `beat=`) are literals here and nowhere else. A kernel
//!   that renames or inserts a field still prints a line this matches, the parse reads nothing,
//!   and every test stays green. Captured fixtures and wire-spelled fields blunt it; sharing the
//!   field names as the heads are shared is the fix, and it is not built.
//! - The sweep's quiet timer is sized against the slowest subrun at the top of
//!   `job_mix::TASK_SWEEP` as measured under TCG, since a sweep has no wall-clock heartbeat.
//!   notes/board-console/limitations.md has the figure and the margin.

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
    /// A rung of the active board's firmware prologue, ordered by [`board::Rung::depth`].
    ///
    /// Below [`Stage::Banner`] on every board, because firmware runs before the kernel. A board
    /// with no prologue this tool recognises (xenon) goes from [`Stage::Cold`] straight to
    /// [`Stage::Banner`].
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
    /// **This is the stage to wait for**: it is reachable on every architecture, it means paging,
    /// traps, the frame allocator, the timer with its interrupt and the scheduler came up, and its
    /// line carries counts rather than a claim. A *failed* verdict is
    /// [`Failure::SelfTestFailed`], not this stage, so a degraded board does not read as a good one.
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
    /// The terminal state of a default boot on every architecture: nothing halts, so the prompt is
    /// the signal that the boot finished. Above [`Stage::Tour`] because a boot that reaches a
    /// prompt has gone past the tour. `cargo xtask boot-check` asserts it on every architecture.
    Prompt,
    /// A sustained workload announced itself and is expected to keep speaking.
    ///
    /// **The only stage after which silence is a failure again.** Below it, a boot ends with the
    /// kernel halting in `wfi`, so quiet is how a good boot ends. A soak is the opposite:
    /// `kernel/src/soak.rs` prints a heartbeat every five seconds of wall clock, so a gap means the
    /// printer itself is wedged. `watch::Policy::quiet_after` implements the asymmetry.
    Soak,
    /// **The job-mix sweep announced itself** ([`job_mix::STARTED`]) and is expected to keep
    /// making progress.
    ///
    /// Silence from here is a failure, as after [`Stage::Soak`], but a sweep has no wall-clock
    /// beat: its finest progress line is [`job_mix::SUBRUN`], so `watch::Policy::quiet_after` must
    /// exceed the slowest subrun at the top of [`job_mix::TASK_SWEEP`] (this module's `BUGS`).
    /// Above [`Stage::Soak`] only because the enum must order them: a kernel builds one or the
    /// other (`script/job-mix`'s `BUGS`), and nothing compares them.
    Sweep,
    /// **The sweep ran to its end** ([`job_mix::DONE`]), and the kernel is parking in `wfi`.
    ///
    /// It is what lets `--until sweep-done` exit `0` when the sweep finished, `2` when it spoke
    /// and then stopped, and `3` when the time ran out with points still to print. Silence after
    /// it is the correct end state, as after [`Stage::Tour`], and `watch::Policy::quiet_after`
    /// exempts it.
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
    /// **The board's firmware gave up before the kernel ran**: the profile's
    /// [`board::Refusal::diagnosis`] and, where that firmware puts the reason on the line before,
    /// that line.
    ///
    /// `reason` is empty when the firmware said only that it gave up.
    FirmwareRefused {
        /// One line naming what went wrong, from [`board::Refusal::diagnosis`].
        diagnosis: &'static str,
        /// The line before the refusal, when this firmware puts the reason there.
        reason: String,
    },
    /// `MEASURED BOOT REFUSED`: the kernel would not vouch for the archive it was handed.
    MeasuredBootRefused,
    /// `[PANIC] ...`: our own panic handler (`kernel/src/panic.rs`), carrying its message.
    KernelPanic(String),
    /// **`nife self-test: 4 of 5 passed, 1 FAILED: <names>`**: a check did not pass on this
    /// machine, carrying the names of the ones that did not.
    ///
    /// A failure and not a stage, so a degraded board does not read as a good one. **The boot did
    /// not stop**: the self-test reports and does not gate (calef, 2026-09-09: a machine you cannot
    /// log into is a machine you cannot fix), so a prompt may well be waiting.
    SelfTestFailed(String),
    /// **The kernel would not start the job-mix sweep** ([`job_mix::FAILED`]), carrying the reason
    /// it gave.
    ///
    /// Printed before [`Stage::Sweep`] and followed by a halt, for each refusal
    /// `kernel/src/job_mix.rs` has: no `job_mix_task` in the archive, or either kind of spawn
    /// failing. A failure, so a board that refused the workload does not read like one that ran it.
    SweepFailed(String),
}

impl Failure {
    /// One line naming what went wrong, for the report and the exit message.
    ///
    /// Each line says what happened next, because the obvious reading is often wrong: a
    /// measured-boot refusal is the gate working rather than a crash, a failed self-test left the
    /// board up, and a refused sweep measured nothing. A firmware refusal's sentence is the
    /// profile's; notes/board-console/markers.md has where each board's wording came from.
    #[must_use]
    pub fn describe(&self) -> String {
        match self {
            Failure::FirmwareRefused { diagnosis, reason } if reason.is_empty() => {
                (*diagnosis).to_string()
            }
            Failure::FirmwareRefused { diagnosis, reason } => format!("{diagnosis}: {reason}"),
            Failure::MeasuredBootRefused => "the measured-boot gate refused the archive and \
                 halted, which is the gate working rather than a crash: the kernel and the archive \
                 on the card are from different builds. Rebuild both with script/board-image, \
                 which orders those steps"
                .to_string(),
            Failure::KernelPanic(message) => format!("the kernel panicked: {message}"),
            Failure::SelfTestFailed(names) => format!(
                "the kernel's boot self-test failed on this machine: {names}. The boot continued to \
                 userspace anyway (it reports, it does not gate), so the board is up and degraded \
                 rather than dead"
            ),
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
    /// **Cumulative tick-route wakes**, and the reason a soak crosses cores at all.
    ///
    /// A soak build signals a rendezvous from `sched::on_tick` and one worker per group blocks on
    /// it through `Irq::WAIT`, so every wake takes the device-interrupt path through
    /// `sched::wake_load_aware`, where the one observed multicore defect was. Zero in a log from a
    /// kernel without tick-route wakes. Not part of [`rounds`](Self::rounds): a wake is not a round
    /// trip.
    pub wakes: u64,
}

/// **One completed point of the job-mix sweep**, as the kernel printed it in a
/// [`job_mix::POINT`] line.
///
/// One per entry in [`job_mix::TASK_SWEEP`]: the [`job_mix::Spread`] of that entry's
/// [`job_mix::REPEATS`] subruns and the jobs-per-minute figure the kernel computed from the
/// median. It exists to be reported, so it keeps every field the line carries, spread and repeat
/// count included, under the wire's own names. See notes/board-console/modules.md for why, and
/// `notes/job-mix.md` for why one boot's figure is a draw rather than a result.
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
    /// Which board's firmware prologue to expect, fixed for the ratchet's life: two profiles'
    /// rungs at one depth compare equal while meaning different things.
    board: &'static board::Profile,
    reached: Stage,
    failure: Option<Failure>,
    relocated: bool,
    userspace_ran: bool,
    banner_line: Option<String>,
    /// The machine description's summary line, so a report can say what machine answered.
    /// `None` until [`Stage::Machine`].
    machine_line: Option<String>,
    /// The self-test verdict exactly as it arrived, counts and all. Kept when green too, because
    /// "five of five" and "one of one" differ, and the difference is what a vacuous pass looks like.
    self_test_line: Option<String>,
    /// The most recent soak heartbeat's numbers, for the run's summary. `None` until a heartbeat
    /// has been seen and parsed.
    soak: Option<SoakBeat>,
    /// The last job-mix point that printed. `None` until one has.
    sweep_point: Option<SweepPoint>,
    /// The last job-mix subrun that printed. `None` until one has.
    sweep_subrun: Option<SweepSubrun>,
    /// The last complete non-empty line: U-Boot's `### ERROR ###` says that it gave up, and the
    /// line before it says why.
    last_line: String,
}

/// A ratchet for [`board::RADON`], which is what `watch::Policy::default` chooses.
///
/// A session over an emulator that takes this default reports `U-Boot SPL` never reached, which is
/// true and useless. Prefer [`BootProgress::new`] with the board in hand; [`board`]'s `BUGS` says
/// why QEMU's callers do not bother.
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

    /// Whether userspace init built its child (`init/build  : ...`), which only kernels from
    /// before `components/src/builder.rs` was retired print.
    ///
    /// `false` on every live boot. It reads captured logs:
    /// `tests/fixtures/captured/vf2-2026-09-01-userspace.log` carries the line, evidence off real
    /// silicon that cannot be re-taken. Ask `reached() >= Stage::Prompt` of a board booted today.
    /// Not a stage, so the ladder stays a ladder; notes/board-console/markers.md has why.
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
    /// The run's round-trip total, which a later run can be compared against. It is a progress
    /// figure and nothing else: `design/roadmap/0219-a-workload-that-does-not-stop.md` says why a
    /// clean run is weak evidence.
    #[must_use]
    pub fn soak(&self) -> Option<&SoakBeat> {
        self.soak.as_ref()
    }

    /// The last job-mix sweep point that printed, if this session saw one.
    ///
    /// The curve's latest entry, not the whole curve, which is in the capture; [`crate::lottery`]
    /// is the precedent for reading a finished log for its results.
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

    /// The machine description's summary line, if this session saw one: the architecture, the
    /// processor count, the memory size and the tick rate.
    #[must_use]
    pub fn machine_line(&self) -> Option<&str> {
        self.machine_line.as_deref()
    }

    /// The self-test verdict exactly as the kernel printed it, if this session saw one, green or
    /// red.
    ///
    /// Kept on a green boot too: the counts are the only defence against a vacuous pass. A red
    /// verdict also appears as [`Failure::SelfTestFailed`]; this is the raw line.
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
    /// U-Boot's `StarFive #` prompt has no newline after it. A tail may ratchet a stage, because a
    /// substring match is monotone, but it may not settle a word boundary (`U-Boot ` before `SPL`)
    /// or capture a payload; notes/board-console/design.md has the two failures that showed it.
    pub(crate) fn observe_partial(&mut self, tail: &str) {
        self.observe(tail, false);
    }

    fn observe(&mut self, line: &str, complete: bool) {
        let line = line.trim_end_matches(['\r', '\n']);

        self.observe_stages(line, complete);
        // Recorded once: the first thing that went wrong is the one worth reporting.
        if self.failure.is_none() {
            self.failure = self.failure_in(line, complete);
        }

        // Only a complete line: the next line's refusal may take this as its reason.
        if complete && !line.trim().is_empty() {
            self.last_line = line.trim().to_string();
        }
    }

    /// Ratchet every stage `line` shows, and capture what a complete line carries: the banner,
    /// machine and self-test lines, the soak heartbeat and the sweep's numbers.
    fn observe_stages(&mut self, line: &str, complete: bool) {
        // Every rung is offered the line: the ratchet decides, and one line can carry two rungs.
        for rung in self.board.prologue {
            if rung.is_seen_in(line, complete) {
                self.reach(Stage::Firmware(rung));
            }
        }
        // The banner is kept from a complete line only, or it is kept truncated.
        if let Some(at) = line.find(boot_ladder::BANNER) {
            self.reach(Stage::Banner);
            if complete && self.banner_line.is_none() {
                self.banner_line = Some(line[at..].to_string());
            }
        }
        // The machine description's last line, so reaching this rung means the whole block printed.
        if let Some(at) = line.find(boot_ladder::MACHINE) {
            self.reach(Stage::Machine);
            if complete && self.machine_line.is_none() {
                self.machine_line = Some(line[at..].to_string());
            }
        }
        // The stage may ratchet on a partial line; the text, and the failure in `failure_in`, wait
        // for a complete one, or a count is captured before it has finished arriving.
        if let Some(at) = line.find(boot_ladder::SELF_TEST) {
            self.reach(Stage::SelfTest);
            if complete && self.self_test_line.is_none() {
                self.self_test_line = Some(line[at..].to_string());
            }
        }
        // The shell's banner is the top rung; `boot_ladder::PROMPT`'s BUGS says why the banner
        // rather than the `$ ` itself.
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
        // Captured logs only; see [`BootProgress::userspace_ran`].
        if line.contains("init/build") {
            self.userspace_ran = true;
        }
        if let Some(at) = line.find(boot_ladder::TOUR) {
            self.reach(Stage::Tour);
            if complete && self.banner_line.is_none() {
                self.banner_line = Some(line[at..].to_string());
            }
        }
        // `soak-test: started` is `kernel/src/soak.rs`'s `START_MARKER`. Only the start line
        // ratchets: a `soak-test: FAILED` line is a failure, not the workload getting going.
        if line.contains("soak-test: started") {
            self.reach(Stage::Soak);
        }
        if complete && line.contains("soak-test: t=") {
            self.observe_soak_beat(line);
        }
        // The census prefix (`job-mix-census:`) matches none of these, so a census block never
        // reads as a sweep starting.
        if line.contains(job_mix::STARTED) {
            self.reach(Stage::Sweep);
        }
        if line.contains(job_mix::DONE) {
            self.reach(Stage::SweepDone);
        }
        // Complete lines only: a truncated `ticks=` would record a figure still arriving.
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
        // First, because a firmware refusal precedes everything. `last_line` still holds the
        // previous line here, which is where U-Boot puts its reason.
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
            // Both markers on one complete line: the tour's preemption check also prints
            // `FAILED:`, and the verdict prefix alone is every green boot.
            let names = line[at + failed + boot_ladder::SELF_TEST_FAILED.len()..].trim();
            Some(Failure::SelfTestFailed(names.to_string()))
        } else if complete
            && let Some(at) = line.find(boot_ladder::SELF_TEST)
            && let Some(name) = Self::verdict_absences(&line[at..])
                .find(|name| self.board.must_not_be_absent.contains(name))
        {
            // The kernel reports a check `absent` when the machine has nothing for it to test,
            // and only the profile knows whether it should have (calef, 2026-10-20 UTC, #2447).
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
            // A total that is neither `SELF_TEST_CHECKS`'s length nor an earlier one
            // (`SELF_TEST_CHECKS_EARLIER`) is a kernel built against a list this tree never had,
            // so its `4 of 4 passed` is not green.
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
    /// Field-name-directed and keeping the previous value, as [`Self::observe_soak_beat`]; that
    /// does not catch a renamed field (this module's `BUGS`). No name is a prefix of another, so
    /// [`field`]'s `find` cannot land on a neighbour's digits.
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

    // `>` is not load-bearing: an equal `Stage::Firmware` is the same `&'static Rung`, since a
    // depth names one rung per profile (`every_profiles_depths_count_from_one_without_gaps`), so a
    // `>=` mutant is expected to survive.
}

/// The decimal run after `name=` in `text`, if there is one.
///
/// Shared by the soak's heartbeat and the sweep's two lines, so a trailing `/s` or a comma is got
/// right in one place.
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
