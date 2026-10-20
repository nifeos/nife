//! **A soak capture, read into the exposure row the defect-discovery curve wants.**
//!
//! Built for milestone 225 (run the soak on radon, argon and xenon), whose rows feed milestone 201
//! (is multicore reliability converging).
//!
//! `notes/multicore-defect-curve.md` keeps two tables, and the one most likely to go unwritten is
//! an *exposure* row for every boot that ran the soak, clean or not. This makes that row the same
//! way every time instead of by hand from the last heartbeat.
//!
//! It reads a finished capture (a `script/board-console` log, or a `script/soak-test` one from
//! QEMU, which the curve does not accept but which proves the reader) and returns one [`Boot`] per
//! kernel banner. A boot that never printed `soak-test: started` is kept, because "the board came
//! up and the workload did not" is a finding, but it produces no row.
//!

//! # What it reads, and what it cannot
//!
//! From the log alone: the architecture and the tick rate (the `nife machine:` line), the cores
//! online (`smp: N core(s) online`, falling back to the machine line's processor count), and every
//! heartbeat. From those, every column of the row except three, which no console line carries:
//!
//! - **machine**: the board's name. `nife machine: aarch64` is argon, an Apple Silicon Mac under
//!   HVF, or QEMU, and the log cannot tell them apart.
//! - **build**: the kernel does not print its commit.
//! - **start**: the capture has no wall clock. The heartbeat's `t=` is the kernel's own.
//!
//! The caller passes those three, and anything not passed prints as `?` rather than as a guess.
//!
//! # The two checks to make before walking away
//!
//! Both come from milestone 221 (the soak never crosses cores, so build the hook that makes it).
//! [`Boot::checks`] repeats them after the fact, so a run that was never checked at the bench is
//! still classified correctly. `crossings` must rise between the first and last beat (frozen is a
//! soak that never crossed cores, which the curve excludes), and `wakerate` must be near
//! `TICK_HZ * cores` (well under it is the timer or the wake path falling behind).
//!
//! # BUGS
//!
//! - **The id is `E?`.** Ids are never reused and are assigned in the note, where the next free
//!   one is visible. A reader that guessed would collide with a row another bench session added.
//! - **"Rising" is judged on the first and last beats only.** A soak that crossed for a minute and
//!   then froze for seven hours would pass. The per-beat series is in the log; reading the slope
//!   would be a stronger claim than this check makes, and nothing has yet produced that shape.
//! - **The wakerate threshold is 80% of `TICK_HZ * cores`**, a judgment, not a measurement; a
//!   board below it is printed as a question rather than failing the row.
//! - **A boot is split at the banner (`nife on `)**, so a capture that lost the banner to line
//!   noise merges two boots into one. The rebooting soak would show it as one long boot with two
//!   `soak-test: started` lines; the count is printed so that is visible.

use crate::board;
use crate::progress::{BootProgress, field};

/// One heartbeat's numbers, as many of them as the row and the checks read.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Beat {
    /// `t=`, seconds since the soak started, by the kernel's timer.
    pub seconds: u64,
    /// `beat=`.
    pub beat: u64,
    /// `rounds=`, cumulative round trips.
    pub rounds: u64,
    /// `rate=`, round trips a second over the last interval.
    pub rate: u64,
    /// `wakes=`, cumulative tick-route wakes.
    pub wakes: u64,
    /// `wakerate=`, tick-route wakes a second over the last interval.
    pub wakerate: u64,
    /// `refused=`, which must be zero.
    pub refused: u64,
    /// `mismatch=`, which must be zero.
    pub mismatch: u64,
    /// `stalled=`, which must be zero.
    pub stalled: u64,
    /// `crossings=`, the curve's x-axis (calef, 2026-09-25).
    pub crossings: u64,
}

impl Beat {
    fn parse(line: &str) -> Self {
        let get = |name| field(line, name).unwrap_or_default();
        Self {
            seconds: get("t="),
            beat: get("beat="),
            rounds: get("rounds="),
            rate: get("rate="),
            // `wakes=` is a substring of nothing else on the line; `wakerate=` is read on its own.
            wakes: get("wakes="),
            wakerate: get("wakerate="),
            refused: get("refused="),
            mismatch: get("mismatch="),
            stalled: get("stalled="),
            crossings: get("crossings="),
        }
    }
}

/// One boot in a capture: from one kernel banner to the next, or to the end of the log.
#[derive(Debug, Clone, Default)]
pub struct Boot {
    /// From `nife machine: <arch>, ...`.
    pub arch: Option<String>,
    /// From `smp: N core(s) online`, or the machine line's processor count without one.
    pub cores: Option<u64>,
    /// From the machine line's `N Hz`, which is the tick route's rate per core.
    pub tick_hz: Option<u64>,
    /// How many `soak-test: started` lines this boot printed. One is a boot that soaked; more is
    /// two boots merged by a lost banner (see this module's `BUGS`).
    pub starts: usize,
    /// Whether this boot was the rebooting soak, which is its own workload name: milestone 249
    /// (the boot lottery is sampled by a person walking to the board).
    pub rebooting: bool,
    /// The first heartbeat, which is what the bench check reads.
    pub first: Option<Beat>,
    /// The last heartbeat, which is the row.
    pub last: Option<Beat>,
    /// What the boot announced as a failure, if it did: `soak-test: FAILED`, a panic, anything
    /// [`BootProgress`] recognises.
    pub failure: Option<String>,
}

/// The three columns a console line cannot supply, and the log's own path for the last column.
#[derive(Debug, Clone, Default)]
pub struct Meta {
    /// The board's name: `radon`, `argon`, `xenon`, or `hvf`.
    pub machine: Option<String>,
    /// The commit the image was built from.
    pub build: Option<String>,
    /// Power-on time, UTC, as the note writes it (`2026-09-25 ~01:02`).
    pub start: Option<String>,
    /// The log's path in the tree.
    pub source: Option<String>,
}

/// **Split a capture into boots and read each one.**
///
/// # Examples
///
/// ```
/// use board_console::exposure::{self, Meta};
///
/// let log = "nife on aarch64 (EL1)\n  smp: 4 core(s) online\n\
///            nife machine: aarch64, 4 processor(s), 256 MiB, 100 Hz\n\
///            soak-test: started 4 groups\n\
///            soak-test: t=5s beat=1 rounds=10 rate=2/s wakes=2000 wakerate=400/s refused=0 mismatch=0 stalled=0 crossings=900\n\
///            soak-test: t=7200s beat=1440 rounds=99 rate=2/s wakes=2880000 wakerate=400/s refused=0 mismatch=0 stalled=0 crossings=1234567\n";
/// let boots = exposure::read(log);
/// let meta = Meta { machine: Some("argon".into()), ..Meta::default() };
/// assert!(boots[0].is_clean());
/// assert!(boots[0].row(&meta).contains("| argon | aarch64 | 4 | ? | soak-test | 2.00 | 1,234,567 | 1,440 | clean |"));
/// ```
pub fn read(log: &str) -> Vec<Boot> {
    read_for(log, &board::EMULATOR)
}

/// [`read`], judging each boot's self-test against `board`'s profile, so a device that board has
/// (`board::Profile::must_not_be_absent`) reported `absent` makes the boot not clean.
///
/// [`read`] forbids no absence, which is right for a capture whose machine has no profile (argon,
/// `hvf`, QEMU) and wrong for radon or xenon: pass the profile whenever the machine has one.
///
/// Name: provisional, minted 2026-10-20 (UTC) by lane `milestone/225-radon-soak-draw`.
#[must_use]
pub fn read_for(log: &str, board: &'static board::Profile) -> Vec<Boot> {
    let mut boots: Vec<(Boot, BootProgress)> = Vec::new();
    for raw in log.lines() {
        let line = raw.trim_end_matches(['\r', '\n']);
        if line.starts_with(boot_ladder::BANNER) {
            // The recogniser starts at the banner, so a board's prologue rungs are never reached
            // here; what the profile decides is which self-test absences fail the boot.
            boots.push((Boot::default(), BootProgress::new(board)));
        }
        let Some((boot, progress)) = boots.last_mut() else {
            continue;
        };
        progress.observe_line(line);
        if let Some(rest) = line.trim_start().strip_prefix(boot_ladder::MACHINE) {
            observe_machine(boot, rest);
        } else if let Some(at) = line.find("smp: ")
            && let Some(n) = online_cores(&line[at + "smp: ".len()..])
        {
            boot.cores = Some(n);
        }
        if line.contains("soak-test: started") {
            boot.starts += 1;
        }
        if line.contains("soak-test-reboot:") {
            boot.rebooting = true;
        }
        if line.contains("soak-test: t=") {
            let beat = Beat::parse(line);
            boot.first.get_or_insert(beat);
            boot.last = Some(beat);
        }
    }
    boots
        .into_iter()
        .map(|(mut boot, progress)| {
            boot.failure = progress.failure().map(crate::progress::Failure::describe);
            boot
        })
        .collect()
}

/// `aarch64, 4 processor(s), 256 MiB, 100 Hz`.
fn observe_machine(boot: &mut Boot, rest: &str) {
    let mut parts = rest.split(", ");
    if let Some(arch) = parts.next() {
        boot.arch = Some(arch.trim().to_string());
    }
    for part in parts {
        if let Some(n) = part.strip_suffix(" processor(s)") {
            // The smp line, when there is one, is the better count: the machine line counts what
            // the device tree describes, and radon's describes a fifth hart it cannot start.
            if boot.cores.is_none() {
                boot.cores = n.trim().parse().ok();
            }
        } else if let Some(hz) = part.trim().strip_suffix(" Hz") {
            boot.tick_hz = hz.trim().parse().ok();
        }
    }
}

/// `4 core(s) online` and nothing else; `4 core(s) in the device tree` is a different claim.
fn online_cores(tail: &str) -> Option<u64> {
    let n = tail.strip_suffix(" core(s) online")?;
    n.trim().parse().ok()
}

impl Boot {
    /// Whether this boot ran the soak at all.
    #[must_use]
    pub fn soaked(&self) -> bool {
        self.starts > 0 && self.last.is_some()
    }

    /// Whether the three failure counters are zero and nothing was announced.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.failure.is_none()
            && self
                .last
                .is_some_and(|b| b.refused == 0 && b.mismatch == 0 && b.stalled == 0)
    }

    /// `Some(true)` when crossings rose between the first and last beat, `None` with one beat.
    #[must_use]
    pub(crate) fn crossed(&self) -> Option<bool> {
        let (first, last) = (self.first?, self.last?);
        if first.beat == last.beat {
            return None;
        }
        Some(last.crossings > first.crossings && last.wakes > 0)
    }

    /// The curve's workload column. A soak that did not cross cores is named so, because the
    /// note excludes it from the curve and the name is how a reader knows.
    #[must_use]
    pub(crate) fn workload(&self) -> String {
        let name = if self.rebooting {
            "soak-test-reboot"
        } else {
            "soak-test"
        };
        match self.crossed() {
            Some(false) => format!("{name} (not crossing)"),
            _ => name.to_string(),
        }
    }

    /// **The exposure row**, in `notes/multicore-defect-curve.md`'s column order, ready to paste.
    #[must_use]
    pub fn row(&self, meta: &Meta) -> String {
        let unknown = || "?".to_string();
        let last = self.last.unwrap_or_default();
        let result = if self.is_clean() {
            "clean".to_string()
        } else {
            "NOT CLEAN: open a defect row, class unclassified".to_string()
        };
        format!(
            "| E? | {} | {} | {} | {} | {} | {} | {:.2} | {} | {} | {} | {} |",
            meta.start.clone().unwrap_or_else(unknown),
            meta.machine.clone().unwrap_or_else(unknown),
            self.arch.clone().unwrap_or_else(unknown),
            self.cores.map_or_else(unknown, |n| n.to_string()),
            meta.build
                .as_deref()
                .map_or_else(unknown, |b| format!("`{b}`")),
            self.workload(),
            // Hundredths, as E4 is written. f64 is exact far past any soak's length in seconds.
            last.seconds as f64 / 3600.0,
            thousands(last.crossings),
            thousands(last.beat),
            result,
            meta.source
                .as_deref()
                .map_or_else(unknown, |s| format!("`{s}`")),
        )
    }

    /// The four figures `notes/soak.md` asks every run to record, all four, from the last beat.
    #[must_use]
    pub fn figures(&self) -> String {
        let b = self.last.unwrap_or_default();
        format!(
            "rounds={} rate={}/s wakes={} ({}/s) crossings={}",
            thousands(b.rounds),
            thousands(b.rate),
            thousands(b.wakes),
            b.wakerate,
            thousands(b.crossings),
        )
    }

    /// The bench checks and the verdict, one line each, worded for the person reading them.
    #[must_use]
    pub fn checks(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.starts > 1 {
            out.push(format!(
                "{} `soak-test: started` lines in one boot: a banner was lost and two boots merged",
                self.starts
            ));
        }
        match self.crossed() {
            Some(true) => out.push("crossings rose: the cross-core experiment ran".into()),
            Some(false) => out.push(
                "crossings FROZEN: this soak never crossed cores, and the curve excludes it".into(),
            ),
            None => out.push("one heartbeat only: whether it crossed cannot be judged".into()),
        }
        if let (Some(hz), Some(cores), Some(last)) = (self.tick_hz, self.cores, self.last) {
            let expected = hz * cores;
            if last.wakerate * 10 >= expected * 8 {
                out.push(format!(
                    "wakerate {}/s against {expected}/s expected: the tick route kept up",
                    last.wakerate
                ));
            } else {
                out.push(format!(
                    "wakerate {}/s against {expected}/s expected: the timer or the wake path fell \
                     behind, so this run measured something else",
                    last.wakerate
                ));
            }
        }
        if let Some(failure) = &self.failure {
            out.push(format!("FAILED: {failure}"));
        } else if let Some(b) = self.last
            && (b.refused | b.mismatch | b.stalled) != 0
        {
            out.push(format!(
                "FAILED: refused={} mismatch={} stalled={}",
                b.refused, b.mismatch, b.stalled
            ));
        }
        out
    }
}

/// `4108581` as `4,108,581`, which is how the note's rows are written.
fn thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The real thing**: radon's eight-hour log, read into the row a person wrote by hand as E4.
    /// Every column the log can supply must agree with that row, or one of the two is wrong.
    #[test]
    fn radon_eight_hours_reads_back_as_e4() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../bench/radon-2026-09-25/soak-8h.log"
        );
        let bytes = std::fs::read(path).expect("E4's log is in the tree");
        let boots = read(&String::from_utf8_lossy(&bytes));
        let soaked: Vec<&Boot> = boots.iter().filter(|b| b.soaked()).collect();
        assert_eq!(soaked.len(), 1, "one boot soaked in E4's capture");
        let boot = soaked[0];
        assert_eq!(boot.arch.as_deref(), Some("riscv64"));
        // The device tree describes five harts and the smp line says four are online. The row
        // records four, and so must this.
        assert_eq!(boot.cores, Some(4));
        assert_eq!(boot.crossed(), Some(true));
        assert!(boot.is_clean());
        let meta = Meta {
            machine: Some("radon".into()),
            build: Some("9e879f1e7".into()),
            start: Some("2026-09-25 ~01:02".into()),
            source: Some("bench/radon-2026-09-25/soak-8h.log".into()),
        };
        assert_eq!(
            boot.row(&meta),
            "| E? | 2026-09-25 ~01:02 | radon | riscv64 | 4 | `9e879f1e7` | soak-test | 8.16 | \
             4,108,581 | 5,818 | clean | `bench/radon-2026-09-25/soak-8h.log` |"
        );
        // And the 225 block's table, the other place these were copied by hand.
        assert_eq!(
            boot.figures(),
            "rounds=10,193,815,048 rate=350,753/s wakes=11,747,350 (404/s) crossings=4,108,581"
        );
    }

    /// argon's architecture, from a QEMU capture: the smp line comes *before* the machine line on
    /// aarch64, so a reader that split boots at the machine line would lose the core count.
    #[test]
    fn an_aarch64_capture_keeps_the_smp_line_that_precedes_the_machine_line() {
        let boots = read(include_str!(
            "../tests/fixtures/captured/qemu-2026-10-05-aarch64-soak-test.log"
        ));
        assert_eq!(boots.len(), 1);
        let boot = &boots[0];
        assert_eq!(boot.arch.as_deref(), Some("aarch64"));
        assert_eq!(boot.cores, Some(4));
        assert_eq!(boot.tick_hz, Some(100));
        assert_eq!(boot.crossed(), Some(true));
        assert!(boot.is_clean());
        assert!(boot.checks().iter().any(|c| c.contains("kept up")));
    }

    #[test]
    fn a_frozen_soak_is_named_and_a_failed_one_is_not_clean() {
        let frozen = "nife on x\nnife machine: riscv64, 4 processor(s), 1 MiB, 100 Hz\n\
                      soak-test: started\n\
                      soak-test: t=5s beat=1 wakes=0 wakerate=0/s crossings=15 refused=0 mismatch=0 stalled=0\n\
                      soak-test: t=10s beat=2 wakes=0 wakerate=0/s crossings=15 refused=0 mismatch=0 stalled=0\n";
        let boot = &read(frozen)[0];
        assert_eq!(boot.workload(), "soak-test (not crossing)");
        assert!(boot.checks().iter().any(|c| c.contains("fell behind")));

        let failed = "nife on x\nsoak-test: started\n\
                      soak-test: t=5s beat=1 wakes=9 crossings=1 refused=1 mismatch=0 stalled=0\n\
                      soak-test: FAILED refused a wake\n";
        let boot = &read(failed)[0];
        assert!(!boot.is_clean());
        assert!(boot.row(&Meta::default()).contains("NOT CLEAN"));
    }

    /// radon has no device that interrupts on its own, so `deviceirq` absent is its normal
    /// verdict, and only xenon's profile makes that absence a failure. A reader that judged every
    /// capture by one board's list would call each radon soak NOT CLEAN.
    #[test]
    fn an_absence_fails_the_boot_only_on_the_board_that_has_the_device() {
        let log = format!(
            "nife on x\nnife self-test: 11 of 13 passed, 2 absent: blockdev deviceirq\n\
             soak-test: started\n{}",
            beat(5, 400, 9, 2000, (0, 0, 0))
        );
        assert!(read_for(&log, &board::RADON)[0].is_clean());
        assert!(!read_for(&log, &board::XENON)[0].is_clean());
    }

    #[test]
    fn a_boot_that_never_soaked_produces_no_row_but_is_counted() {
        let boots = read(
            "U-Boot\nnife on x\nnife self-test: 5 of 5 passed\nnife on x\nsoak-test: started\n",
        );
        assert_eq!(boots.len(), 2);
        assert!(!boots[0].soaked());
        assert!(
            !boots[1].soaked(),
            "started with no heartbeat is not yet a soak"
        );
    }

    /// A beat line with the given wake rate and failure counters, for the bench-check tests.
    fn beat(
        t: u64,
        wakerate: u64,
        crossings: u64,
        wakes: u64,
        failures: (u64, u64, u64),
    ) -> String {
        let (refused, mismatch, stalled) = failures;
        format!(
            "soak-test: t={t}s beat={t} wakes={wakes} wakerate={wakerate}/s crossings={crossings} \
             refused={refused} mismatch={mismatch} stalled={stalled}\n"
        )
    }

    /// A one-boot capture on a four-core, 100 Hz machine, whose tick route expects 400 wakes/s.
    fn boot_of(beats: &[String]) -> Boot {
        let mut log = String::from(
            "nife on x\nnife machine: riscv64, 4 processor(s), 1 MiB, 100 Hz\nsoak-test: started\n",
        );
        for b in beats {
            log.push_str(b);
        }
        read(&log).remove(0)
    }

    /// **The online count wins over the device tree's**, whichever line comes first: a machine
    /// line claiming five processors and an smp line bringing four online is a four-core run.
    #[test]
    fn the_smp_line_overrides_the_machine_lines_processor_count() {
        let boot = &read(
            "nife on x\nnife machine: riscv64, 5 processor(s), 1 MiB, 100 Hz\n  smp: 4 core(s) online\n",
        )[0];
        assert_eq!(boot.cores, Some(4));
    }

    /// The wake-rate check's line is 80% of `hz * cores`: 320/s keeps up with 400/s expected and
    /// 319/s does not.
    #[test]
    fn the_tick_route_keeps_up_at_exactly_eighty_percent() {
        let ok = boot_of(&[beat(5, 320, 1, 9, (0, 0, 0))]);
        assert!(
            ok.checks()
                .iter()
                .any(|c| c == "wakerate 320/s against 400/s expected: the tick route kept up")
        );
        let slow = boot_of(&[beat(5, 319, 1, 9, (0, 0, 0))]);
        assert!(slow.checks().iter().any(|c| c.contains("fell behind")));
    }

    /// Any one nonzero failure counter fails the run, two together still do, and none is clean.
    #[test]
    fn any_failure_counter_fails_the_checks() {
        let failed = |f| {
            boot_of(&[beat(5, 400, 1, 9, f)])
                .checks()
                .iter()
                .any(|c| c.starts_with("FAILED"))
        };
        assert!(failed((1, 0, 0)));
        assert!(failed((0, 0, 1)));
        assert!(failed((1, 1, 0)));
        assert!(failed((0, 1, 1)));
        assert!(!failed((0, 0, 0)));
    }

    /// Two `started` lines in one boot are two boots merged by a lost banner, and one is not.
    #[test]
    fn two_starts_in_one_boot_are_reported_as_merged() {
        let merged = |b: &Boot| b.checks().iter().any(|c| c.contains("two boots merged"));
        let one = boot_of(&[beat(5, 400, 1, 9, (0, 0, 0))]);
        assert!(!merged(&one));
        let two = boot_of(&[
            String::from("soak-test: started\n"),
            beat(5, 400, 1, 9, (0, 0, 0)),
        ]);
        assert!(merged(&two));
    }

    /// Crossing needs both: crossings that rose, and wakes at all. Either alone is not a soak that
    /// crossed cores.
    #[test]
    fn crossing_needs_rising_crossings_and_some_wakes() {
        let rose_no_wakes = boot_of(&[beat(5, 0, 1, 0, (0, 0, 0)), beat(10, 0, 2, 0, (0, 0, 0))]);
        assert_eq!(rose_no_wakes.crossed(), Some(false));
        let flat = boot_of(&[
            beat(5, 400, 3, 9, (0, 0, 0)),
            beat(10, 400, 3, 9, (0, 0, 0)),
        ]);
        assert_eq!(flat.crossed(), Some(false));
        let rose = boot_of(&[
            beat(5, 400, 3, 9, (0, 0, 0)),
            beat(10, 400, 4, 9, (0, 0, 0)),
        ]);
        assert_eq!(rose.crossed(), Some(true));
    }

    /// Heartbeats with no `started` line are not a soak: the start is what says this boot ran it.
    #[test]
    fn heartbeats_without_a_start_are_not_a_soak() {
        let boot = &read(&format!("nife on x\n{}", beat(5, 400, 1, 9, (0, 0, 0))))[0];
        assert!(boot.last.is_some());
        assert!(!boot.soaked());
    }

    #[test]
    fn thousands_matches_the_note() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(5818), "5,818");
        assert_eq!(thousands(10_193_815_048), "10,193,815,048");
    }
}
