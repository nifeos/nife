//! The sustained runs under QEMU: the multicore soak (milestone 219 (the boot tour ends and the
//! kernel halts, so there is nothing to soak)) and the multi-tasking job mix (milestone 168 (a
//! multi-tasking workload benchmark)).
//!
//! Both return their own exit codes rather than a bool, because a rehearsal that cannot say
//! *how* it failed is not a rehearsal.

use std::path::PathBuf;
use std::process::{Command, ExitCode};
use std::sync::atomic::Ordering;

use crate::archive::{
    initial_ramdisk_path, initial_ramdisk_riscv, initial_ramdisk_x86, riscv_initial_ramdisk_path,
    x86_initial_ramdisk_path,
};
use crate::board::{open_capture, parse_duration};
use crate::disk::mkdisk;
use crate::host::workspace_root;
use crate::{
    RELEASE, RISCV_TARGET, TARGET, X86_TARGET, cargo_profiled, maybe_hvf, profile_dir, user,
};

/// **The QEMU rehearsal of milestone 168's multi-tasking workload sweep.** Boot a
/// `--features job_mix` kernel, echo its lines, stop when it says it is done, and kill it.
///
/// Returns `script/board-console`'s exit statuses, judged by `serial_console` (milestone 324 (the
/// bench console cannot speak to any of the three boards), part 2), so a sweep that wedges
/// mid-subrun ends with status 2 rather than hanging. This is a rehearsal, not the measurement: TCG
/// models no caches and HVF puts the host scheduler under every guest thread, so the number
/// DECISIONS §96 (process kernel or event kernel, and how to decide it) waits for is taken on radon
/// by `notes/job-mix.md`'s procedure. It proves the workload runs, the sweep completes and the
/// output has the shape the bench evening reads. See `script/job-mix`.
pub(crate) fn job_mix_sweep() -> ExitCode {
    use std::time::Duration;

    use serial_console::progress::Stage;
    use serial_console::watch::{Policy, watch};

    let policy = Policy {
        // Ten minutes. The whole sweep is eighteen subruns and took well under a minute on the
        // machine this was written on; the cap is for the run that never finishes, and a cap that
        // is too generous costs a slow failure where one that is too tight costs a wrong answer.
        total: Duration::from_secs(600),
        until: Some(Stage::SweepDone),
        // Sized against a subrun, not a heartbeat: a sweep speaks only when a subrun ends. The
        // longest, the top of `job_mix::TASK_SWEEP`, measured 249,234,771 ticks on a 62.5 MHz
        // counter (4.0 s, QEMU aarch64, 2026-09-19) in the capture
        // `crates/serial_console/tests/fixtures/captured/qemu-2026-09-19-aarch64-job-mix-medians.log`.
        // Sixty seconds is fifteen times that and still names a wedge inside a minute. A default,
        // so overridable.
        quiet_after: Some(Duration::from_secs(60)),
        // Nothing this kernel prints after `job-mix: done` can change the verdict: it halts. The
        // settle window exists for the measured-boot refusal that arrives *after* the awaited rung,
        // and a sweep that has printed its last point is past every such gate.
        settle: Duration::from_secs(0),
        // An emulator has no firmware prologue to climb; see `boot_check`'s note.
        board: &serial_console::board::EMULATOR,
    };
    // `--hvf` and `--release` (for the HVF cross-check in notes/job-mix.md): the
    // two flags the tree already spells this way, `run`'s and `bench`'s, rather than new ones. HVF
    // is aarch64 on an Apple host only, and release is what `script/board-image` builds for radon,
    // so the cross-check runs the optimisation level the board does.
    //
    // `--lock-wait` (2026-10-04, fatal risk 4; provisional): build `--features lock_wait`, the job
    // mix plus kernel/src/lock_wait.rs's count of contended lock acquisitions, which prints
    // `job-mix-lock:` lines after each point. The kernel feature's name, hyphenated the way this
    // script's flags are.
    let run = match RunArgs::parse(
        "job-mix",
        &["--hvf", "--lock-wait", "--release"],
        "usage: cargo xtask job-mix [--arch aarch64|riscv64|x86_64] [--smp <n>] \
         [--hvf] [--release] [--for <duration>] [--quiet-after <duration>] \
         [--log <file>]",
        policy,
    ) {
        Ok(run) => run,
        Err(code) => return code,
    };
    let (hvf, lock_wait) = (run.has("--hvf"), run.has("--lock-wait"));
    if run.has("--release") {
        RELEASE.store(true, Ordering::Relaxed);
    }
    let arch = run.arch.as_str();

    if hvf && arch != "aarch64" {
        eprintln!("job-mix: --hvf runs the Apple core, so it is aarch64 only");
        return ExitCode::from(4);
    }
    if hvf {
        maybe_hvf();
    }

    let Some((target, runner, initial_ramdisk)) = kernel_leg("job-mix", arch) else {
        return ExitCode::from(4);
    };

    if !cargo_profiled(&[
        "build",
        "-p",
        "kernel",
        "--features",
        if lock_wait { "lock_wait" } else { "job_mix" },
        "--target",
        target,
    ]) {
        return ExitCode::from(4);
    }

    // The log is not optional, for `soak-test`'s reason: a console session whose evidence exists
    // only in a terminal that has since scrolled is the failure this tree keeps writing down, and a
    // capture can be re-read with `script/board-console --replay`.
    let Some((log_path, mut sink)) =
        open_capture("job-mix", &format!("job-mix-{arch}"), run.log.clone())
    else {
        return ExitCode::from(4);
    };

    let mut cmd = qemu_command(runner, target, &initial_ramdisk, arch, run.smp.as_deref());

    eprintln!(
        "--- job-mix: {arch}, a rehearsal; the number is taken on radon (notes/job-mix.md), \
         logging to {} ---",
        log_path.display()
    );
    let Some((mut child, stdout)) = start_qemu("job-mix", runner, &mut cmd) else {
        return ExitCode::from(4);
    };

    // `false`, not `true`: a pipe from a process really does end when that process dies, which a
    // serial port never does.
    let session = watch(stdout, &mut sink, &run.policy, false);

    stop_qemu(&mut child, &mut sink);

    let session = match session {
        Ok(s) => s,
        Err(e) => {
            eprintln!("job-mix: {e}");
            eprintln!("job-mix: log at {}", log_path.display());
            return ExitCode::from(4);
        }
    };
    report_sweep(&session, &log_path);
    // The same five statuses `script/board-console` returns, computed by the same code: `2` is a
    // sweep that spoke and then stopped, a hang, distinct from one that finished.
    ExitCode::from(u8::try_from(session.exit_code()).unwrap_or(4))
}

/// What `job-mix` says about a sweep that finished, however it finished.
fn report_sweep(session: &serial_console::watch::Session, log_path: &std::path::Path) {
    eprintln!();
    eprintln!("job-mix: {}", session.summary());
    match session.progress.sweep_point() {
        // The spread and not the median alone: the kernel prints the two ends so a figure is
        // never quoted without them, and a summary that dropped them here would undo that at the
        // one line a person reads instead of the log.
        Some(point) => eprintln!(
            "job-mix: last point tasks={} jobs={} repeats={} ticks_min={} ticks_median={} \
             ticks_max={} jpm_median={}",
            point.tasks,
            point.jobs,
            point.repeats,
            point.ticks_min,
            point.ticks_median,
            point.ticks_max,
            point.jpm_median
        ),
        // Said out loud rather than left as an absence, because an empty tail is exactly what a
        // kernel that refused and a kernel that wedged before its first point both look like.
        None => eprintln!("job-mix: no point of the sweep was measured"),
    }
    eprintln!("job-mix: log at {}", log_path.display());
    // Said on every clean run, on purpose, for `soak_test`'s reason: this is the sentence that gets
    // dropped when a number is quoted.
    eprintln!(
        "job-mix: these magnitudes are NOT the measurement. TCG models no cache and HVF puts a \
         host scheduler under every guest thread; DECISIONS \u{a7}96's number is taken on radon."
    );
}

/// **The command line `job-mix` and `soak-test` share**: `--arch`, `--smp`, `--log`, `--for` (or
/// `--timeout`) and `--quiet-after`, plus the switches each command names for itself.
struct RunArgs {
    arch: String,
    smp: Option<String>,
    log: Option<PathBuf>,
    policy: serial_console::watch::Policy,
    /// Whether `--for` was given, so a command can pick its own default for a run of its kind.
    duration_given: bool,
    switches: Vec<&'static str>,
}

impl RunArgs {
    /// Read the command line after the subcommand. `policy` holds the command's defaults, which
    /// `--for` and `--quiet-after` override. `Err` carries exit status 4, after saying why.
    fn parse(
        command: &str,
        switches: &[&'static str],
        usage: &str,
        policy: serial_console::watch::Policy,
    ) -> Result<Self, ExitCode> {
        let args: Vec<String> = std::env::args().skip(2).collect();
        let mut run = RunArgs {
            arch: "aarch64".to_string(),
            smp: None,
            log: None,
            policy,
            duration_given: false,
            switches: Vec::new(),
        };
        let mut i = 0;
        while i < args.len() {
            if let Some(switch) = switches.iter().find(|s| **s == args[i]) {
                run.switches.push(switch);
                i += 1;
                continue;
            }
            let value = |i: usize| -> Result<&str, ExitCode> {
                args.get(i + 1).map(String::as_str).ok_or_else(|| {
                    eprintln!("{command}: {} wants a value", args[i]);
                    ExitCode::from(4)
                })
            };
            match args[i].as_str() {
                "--arch" => run.arch = value(i)?.to_string(),
                "--smp" => run.smp = Some(value(i)?.to_string()),
                "--log" => run.log = Some(PathBuf::from(value(i)?)),
                "--for" | "--timeout" => match parse_duration(value(i)?) {
                    Some(d) => {
                        run.policy.total = d;
                        run.duration_given = true;
                    }
                    None => {
                        eprintln!("{command}: --for wants a duration like 90, 90s, 30m or 2h");
                        return Err(ExitCode::from(4));
                    }
                },
                "--quiet-after" => match parse_duration(value(i)?) {
                    // Zero disables it, for the operator who knows their board is slower than any
                    // number written here and would rather wait out the cap than be told it wedged.
                    Some(d) => run.policy.quiet_after = if d.is_zero() { None } else { Some(d) },
                    None => {
                        eprintln!("{command}: --quiet-after wants a duration, or 0 to disable");
                        return Err(ExitCode::from(4));
                    }
                },
                other => {
                    eprintln!("{command}: unknown argument {other}");
                    eprintln!("{usage}");
                    return Err(ExitCode::from(4));
                }
            }
            i += 2;
        }
        Ok(run)
    }

    fn has(&self, switch: &str) -> bool {
        self.switches.contains(&switch)
    }
}

/// **The kernel target, runner script and initial ramdisk for `arch`**, after building that
/// architecture's archive (and the disk image its runner checks for). `None` after saying why not.
fn kernel_leg(command: &str, arch: &str) -> Option<(&'static str, &'static str, String)> {
    match arch {
        "aarch64" => (mkdisk() && user()).then(|| {
            (
                TARGET,
                "helpers/qemu-runner-aarch64.sh",
                initial_ramdisk_path(),
            )
        }),
        // `mkdisk` here too: the riscv64 runner refuses a `NIFE_DISK` naming a missing file, so
        // a fresh worktree dies before the kernel prints a line. The run itself reads no disk.
        "riscv64" => (mkdisk() && initial_ramdisk_riscv()).then(|| {
            (
                RISCV_TARGET,
                "helpers/qemu-runner-riscv64.sh",
                riscv_initial_ramdisk_path(),
            )
        }),
        "x86_64" => initial_ramdisk_x86().then(|| {
            (
                X86_TARGET,
                "helpers/qemu-runner-x86_64.sh",
                x86_initial_ramdisk_path(),
            )
        }),
        other => {
            eprintln!("{command}: unknown architecture {other} (aarch64, riscv64 or x86_64)");
            None
        }
    }
}

/// **The QEMU command for a run**: the runner script on the kernel this command built, with its
/// initial ramdisk, `smp` cores if asked, the guest's console piped back, and the runner's own
/// diagnostics left on this terminal.
fn qemu_command(
    runner: &str,
    target: &str,
    initial_ramdisk: &str,
    arch: &str,
    smp: Option<&str>,
) -> Command {
    let mut cmd = Command::new(runner);
    cmd.arg(format!(
        "{}/target/{target}/{}/kernel",
        workspace_root().display(),
        profile_dir()
    ));
    cmd.env("NIFE_INITIAL_RAMDISK", initial_ramdisk);
    // No disk on `x86_64`: `host::cargo` exports a `NIFE_DISK` naming a file only `mkdisk`
    // writes, and the `x86_64` runner treats a named but missing disk as fatal. The run reads no
    // disk.
    if arch == "x86_64" {
        cmd.env_remove("NIFE_DISK");
    }
    if let Some(n) = smp {
        cmd.env("NIFE_SMP", n);
    }
    cmd.stdout(std::process::Stdio::piped());
    // The runner's own diagnostics stay on this terminal rather than joining the captured stream:
    // the log is meant to be the guest's console and nothing else, so that a replay through
    // `script/board-console --replay` sees what a serial cable would have seen.
    cmd.stderr(std::process::Stdio::inherit());
    cmd
}

/// Start `cmd` and take the guest's console from it. `None` after saying why not.
fn start_qemu(
    command: &str,
    runner: &str,
    cmd: &mut Command,
) -> Option<(std::process::Child, std::process::ChildStdout)> {
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{command}: cannot start {runner}: {e}");
            return None;
        }
    };
    let Some(stdout) = child.stdout.take() else {
        eprintln!("{command}: the runner gave us no stdout to read");
        let _ = child.kill();
        return None;
    };
    Some((child, stdout))
}

/// **Kill the run whatever happened**, and flush its capture. A nife kernel that has finished its
/// work sits in `wfi` forever and QEMU with it, and a soak never even finishes: leaving it running
/// is the leak `AGENTS.md` spends a whole section on.
///
/// **The children first, then the wrapper**, the same order and for the same reason as `run_bench`:
/// `helpers/qemu-runner-x86_64.sh` runs QEMU as a plain foreground child rather than `exec`-ing into
/// it, so killing the wrapper alone orphans the emulator, which keeps holding this pipe. The other
/// two runners `exec`, so `pkill -P` finds nothing there and costs one process.
fn stop_qemu(child: &mut std::process::Child, sink: &mut impl std::io::Write) {
    let _ = Command::new("pkill")
        .args(["-9", "-P", &child.id().to_string()])
        .status();
    let _ = child.kill();
    let _ = child.wait();
    let _ = sink.flush();
}

/// **The QEMU half of milestone 219's sustained run.** Boot a `--features soak_test` kernel (or the
/// variant a switch names), watch it with the recogniser and policy `script/board-console` points at
/// a real board, and return the same exit statuses, 4 for arguments or a build.
///
/// One recogniser for both sources, so the workload and the console agree about what a hang is, as
/// milestone 219's block requires; this adds only building, starting QEMU and killing it. Named for
/// the command rather than the workload (milestone 297 (`soak` becomes `soak-test`)): this *is*
/// `script/soak-test`, where `BootProgress::soak` one crate over keeps the workload's spelling. See
/// `script/soak-test` and notes/soak.md.
pub(crate) fn soak_test() -> ExitCode {
    use std::time::Duration;

    use serial_console::watch::{Policy, watch};

    // A minute by default: long enough that the beat, the rate and the cross-core counters are all
    // real numbers rather than a first sample, and short enough that nobody is tempted to skip it.
    // The runs that matter are hours long and happen on a board.
    let policy = Policy {
        total: Duration::from_secs(60),
        until: None,
        quiet_after: Some(Duration::from_secs(15)),
        settle: Duration::from_secs(0),
        // The emulator's profile; see `boot_check_policy`'s note.
        board: &serial_console::board::EMULATOR,
    };
    // Milestone 249 (the boot lottery is sampled by a person walking to the board)'s QEMU proof:
    // `--reboot` builds the rebooting soak and passes only when a second boot starts soaking after
    // the first asked for a reset. See [`second_boot`].
    //
    // Milestone 593 (a wedged kernel resets itself)'s two QEMU proofs (number provisional), x86_64
    // only: with `--watchdog` a healthy watchdog soak is never reset over a bounded window, and
    // with `--wedge` a wedged one is. See [`watchdog_verdict`]. A plain x86_64 soak arms the
    // watchdog too, and `--no-watchdog` is the plain soak without it; see [`SoakKind::Unwatched`].
    let mut run = match RunArgs::parse(
        "soak-test",
        &["--reboot", "--watchdog", "--wedge", "--no-watchdog"],
        "usage: cargo xtask soak-test [--arch aarch64|riscv64|x86_64] [--for <duration>] \
         [--smp <n>] [--quiet-after <duration>] [--log <file>] \
         [--reboot | --watchdog | --wedge | --no-watchdog]",
        policy,
    ) {
        Ok(run) => run,
        Err(code) => return code,
    };
    let Some(kind) = SoakKind::from_run(&run) else {
        return ExitCode::from(4);
    };
    let arch = run.arch.clone();

    let Some((target, runner, initial_ramdisk)) = kernel_leg("soak-test", &arch) else {
        return ExitCode::from(4);
    };

    if !run.duration_given
        && let Some(total) = kind.default_total()
    {
        run.policy.total = total;
    }
    if !cargo_profiled(&[
        "build",
        "-p",
        "kernel",
        "--features",
        kind.feature(),
        "--target",
        target,
    ]) {
        return ExitCode::from(4);
    }

    let Some((log_path, mut sink)) =
        open_capture("soak-test", &format!("soak-test-{arch}"), run.log.clone())
    else {
        return ExitCode::from(4);
    };

    let mut cmd = qemu_command(runner, target, &initial_ramdisk, &arch, run.smp.as_deref());
    if kind.expects_reset() {
        // The x86 runner passes `-no-reboot` so a triple fault exits instead of looping; this run
        // is the one where a reset must reset. The other two runners never pass it.
        cmd.env("NIFE_ALLOW_REBOOT", "1");
        // Nothing on stdin, so nothing can reach the guest's console and disarm the loop: the
        // escape is the byte-on-the-console the kernel polls for, and a terminal's stray keypress
        // would turn this proof into a run that never resets.
        cmd.stdin(std::process::Stdio::null());
    }

    eprintln!(
        "--- soak-test: {arch}, up to {:?}, logging to {} ---",
        run.policy.total,
        log_path.display()
    );
    let Some((mut child, stdout)) = start_qemu("soak-test", runner, &mut cmd) else {
        return ExitCode::from(4);
    };

    if kind.expects_reset() {
        let verdict = if kind == SoakKind::Reboot {
            second_boot(stdout, &mut sink, run.policy.total)
        } else {
            watchdog_verdict(stdout, &mut sink, run.policy.total, kind == SoakKind::Wedge)
        };
        stop_qemu(&mut child, &mut sink);
        eprintln!("soak-test: log at {}", log_path.display());
        return verdict;
    }

    // `false`, not `true`: a pipe from a process really does end when that process dies, which a
    // serial port never does. Getting this bit wrong turns a QEMU that died into a board that has
    // not spoken yet.
    let session = watch(stdout, &mut sink, &run.policy, false);

    // Kill it whatever happened: [`stop_qemu`] says why, and why in that order.
    stop_qemu(&mut child, &mut sink);

    let session = match session {
        Ok(s) => s,
        Err(e) => {
            eprintln!("soak-test: {e}");
            eprintln!("soak-test: log at {}", log_path.display());
            return ExitCode::from(4);
        }
    };
    soak_verdict(&session, run.policy.total, &log_path)
}

/// **Which run `soak-test` was asked for**: the plain soak, with or without the watchdog, or one of
/// the three that must see the machine reset (`--reboot`, `--watchdog`, `--wedge`).
#[derive(Clone, Copy, PartialEq, Eq)]
enum SoakKind {
    /// `--features soak_test`, which on x86_64 arms the watchdog (`kernel/build.rs`).
    Plain,
    /// **`--no-watchdog`, the opt-out** (milestone 593 (a wedged kernel resets itself)): the plain
    /// soak on x86_64 with nothing to reset it, so a wedge is seen as a wedge, a console gone
    /// quiet, rather than as a reset. Name provisional.
    Unwatched,
    Reboot,
    Watchdog,
    Wedge,
}

impl SoakKind {
    /// From the switches, refusing two at once and the watchdog runs off x86_64. `None` after
    /// saying why.
    fn from_run(run: &RunArgs) -> Option<Self> {
        let (reboot, watchdog, wedge, unwatched) = (
            run.has("--reboot"),
            run.has("--watchdog"),
            run.has("--wedge"),
            run.has("--no-watchdog"),
        );
        if u8::from(reboot) + u8::from(watchdog) + u8::from(wedge) + u8::from(unwatched) > 1 {
            eprintln!(
                "soak-test: --reboot, --watchdog, --wedge and --no-watchdog are four different runs; \
                 pick one"
            );
            return None;
        }
        if (watchdog || wedge || unwatched) && run.arch != "x86_64" {
            eprintln!(
                "soak-test: --watchdog, --wedge and --no-watchdog are x86_64-only: they need the \
                 Intel TCO that QEMU's q35 emulates. Neither virt board models a watchdog, and \
                 riscv64's (the JH7110's) is proven by a boot on radon instead (milestone 593's \
                 block)"
            );
            return None;
        }
        Some(if unwatched {
            Self::Unwatched
        } else if reboot {
            Self::Reboot
        } else if wedge {
            Self::Wedge
        } else if watchdog {
            Self::Watchdog
        } else {
            Self::Plain
        })
    }

    /// The kernel feature that builds this run's workload.
    fn feature(self) -> &'static str {
        match self {
            Self::Reboot => "reboot_soak_test",
            Self::Wedge => "wedge_soak_test",
            Self::Watchdog => "watchdog_soak_test",
            Self::Plain => "soak_test",
            Self::Unwatched => "unwatched_soak_test",
        }
    }

    /// Whether the run passes by seeing the machine reset, rather than by the plain soak's verdict.
    fn expects_reset(self) -> bool {
        !matches!(self, Self::Plain | Self::Unwatched)
    }

    /// How long the run lasts when `--for` did not say, where it differs from the plain soak's
    /// minute.
    fn default_total(self) -> Option<std::time::Duration> {
        let secs = match self {
            // One window of `REBOOT_AFTER_SECONDS` (120s), the five-second grace, the reset
            // attempts and two boots fit in six minutes under TCG with room to spare.
            Self::Reboot => 360,
            // The wedge comes 30s into the soak and the reset 60s after the last pet; two boots and
            // that fit in six minutes for the same reason as above.
            Self::Wedge => 360,
            // The healthy run's window is counted from the moment the watchdog is armed, and three
            // minutes is three full resets' worth of time.
            Self::Watchdog => 180,
            Self::Plain | Self::Unwatched => return None,
        };
        Some(std::time::Duration::from_secs(secs))
    }
}

/// **What a plain soak says, and its exit status**: the watcher's summary and the last heartbeat,
/// and a QEMU that ended early or never beat is a failure whatever the watcher scored.
fn soak_verdict(
    session: &serial_console::watch::Session,
    total: std::time::Duration,
    log_path: &std::path::Path,
) -> ExitCode {
    use serial_console::watch::Outcome;

    eprintln!();
    eprintln!("soak-test: {}", session.summary());
    match session.progress.soak() {
        Some(beat) => report_beat(beat),
        None => eprintln!("soak-test: no heartbeat was seen; the workload never started"),
    }
    eprintln!("soak-test: log at {}", log_path.display());

    // The one judgement that is this driver's rather than the watcher's, because it is about a
    // process and not about a board. A serial port cannot end; a pipe can, and QEMU exiting before
    // the deadline means the guest is gone. `serial_console` scores `Ended` as success when nothing
    // was being waited for, which is right for a replayed capture and wrong here.
    if session.outcome == Outcome::Ended
        && session.elapsed + std::time::Duration::from_secs(1) < total
    {
        eprintln!(
            "soak-test: QEMU exited after {:?}, before the deadline",
            session.elapsed
        );
        return ExitCode::from(3);
    }
    if session.progress.soak().is_none() {
        return ExitCode::from(3);
    }
    ExitCode::from(u8::try_from(session.exit_code()).unwrap_or(4))
}

/// **The last heartbeat of a soak, and what it does and does not mean**, said on every run.
fn report_beat(beat: &serial_console::progress::SoakBeat) {
    eprintln!(
        "soak-test: {} round trips in {}s ({} /s at the last beat), {} cross-core handoffs",
        beat.rounds, beat.seconds, beat.rate, beat.crossings
    );
    eprintln!(
        "soak-test: refused={} mismatch={} stalled={} (each must be 0)",
        beat.refused, beat.mismatches, beat.stalled
    );
    // Said on every clean run, on purpose, because this is the sentence the milestone's own
    // BUGS section says will otherwise be dropped when the number is quoted.
    eprintln!(
        "soak-test: a clean run is a number to compare against, NOT evidence that the \
         concurrency is correct."
    );
    // **What the crossings are, said on every run that has any** (milestone 221). The
    // kernel prints this at the start of a soak, but a reader quoting this summary never
    // saw that, and the wrong reading is available and flattering: that the IPC workload
    // itself is migrating. It is not, and only a periodic rebalancer would make it, which
    // DECISIONS 138 (how a saturated workload is made to hand threads across cores)
    // declines.
    if beat.wakes > 0 {
        eprintln!(
            "soak-test: the {} crossings are tick waiters being placed by the wake protocol \
             ({} tick wakes drove them), NOT the IPC pairs migrating. See notes/soak.md.",
            beat.crossings, beat.wakes
        );
    }
    // The gap milestone 219 measured rather than assumed, said on every run that shows it
    // because a reader who does not know it will read the round-trip total as covering
    // more than it does. See notes/soak.md.
    if beat.crossings < beat.rounds / 1000 {
        if beat.wakes == 0 {
            eprintln!(
                "soak-test: and it barely crossed cores ({} handoffs against {} round trips): \
                 this scheduler does not rebalance, so a saturated workload stays where it \
                 was placed. See notes/soak.md.",
                beat.crossings, beat.rounds
            );
        } else {
            // The tick route was live and the machine still did not cross, which on one
            // core is the only possible answer and on several is a finding. Both are named
            // rather than one being assumed, because this summary cannot see the core
            // count and the kernel's own banner can.
            eprintln!(
                "soak-test: and it barely crossed cores ({} handoffs against {} tick wakes), \
                 which on a single-core run is arithmetic and on a multicore one is a \
                 finding: check the core count in the soak's own start line.",
                beat.crossings, beat.wakes
            );
        }
    }
}

/// **Did the machine actually come back?** Milestone 249's proof under QEMU, for `soak-test --reboot`.
///
/// Passes only on the soak's start line appearing a second time, after a `rebooting now` line,
/// which QEMU produces only by resetting the machine and loading `-kernel` again. A reset hung in
/// firmware, a guest that printed its reset line and stopped, and a QEMU that exited on the reset
/// all fail. It reads the stream itself because `serial_console::watch` judges one boot and treats a
/// second banner as a fresh start; multi-boot captures are `serial_console::lottery`'s job. Returns
/// 0 came back, 1 the kernel said the reset failed or panicked, 2 the deadline passed first, 3
/// QEMU's output ended.
fn second_boot(
    stdout: std::process::ChildStdout,
    sink: &mut impl std::io::Write,
    deadline: std::time::Duration,
) -> ExitCode {
    use std::sync::mpsc;
    use std::time::Instant;

    // The soak's own start line and the reboot loop's prefix, as `kernel/src/soak.rs` spells them.
    const STARTED: &str = "soak-test: started";
    const REBOOT: &str = "soak-test-reboot:";

    let rx = console_lines(stdout);

    let begun = Instant::now();
    let mut starts = 0u32;
    let mut rebooting = false;
    let mut last_attempt: Option<String> = None;
    loop {
        let Some(left) = deadline.checked_sub(begun.elapsed()) else {
            eprintln!(
                "soak-test: FAIL, {}s passed without a second boot (starts={starts}, reset asked \
                 for: {rebooting})",
                deadline.as_secs()
            );
            return ExitCode::from(2);
        };
        let line = match rx.recv_timeout(left) {
            Ok(line) => line,
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                eprintln!(
                    "soak-test: FAIL, QEMU's output ended (starts={starts}, reset asked for: \
                     {rebooting}); a QEMU that exits on a reset has not rebooted anything"
                );
                return ExitCode::from(3);
            }
        };
        let _ = writeln!(sink, "{line}");
        if line.contains("[PANIC]") {
            eprintln!("soak-test: FAIL, the kernel panicked: {line}");
            return ExitCode::from(1);
        }
        if let Some(rest) = line.split_once(REBOOT).map(|(_, rest)| rest.trim()) {
            if rest.starts_with("rebooting now") {
                rebooting = true;
            } else if rest.starts_with("attempt") {
                last_attempt = Some(rest.to_string());
            } else if rest.starts_with("FAILED") || rest.starts_with("DISARMED") {
                eprintln!("soak-test: FAIL, the reboot loop stopped: {rest}");
                return ExitCode::from(1);
            }
        }
        if line.contains(STARTED) {
            starts += 1;
            if rebooting && starts >= 2 {
                eprintln!();
                eprintln!(
                    "soak-test: PASS, the machine reset and this kernel booted and began soaking \
                     again, {}s in",
                    begun.elapsed().as_secs()
                );
                if let Some(attempt) = last_attempt {
                    eprintln!("soak-test: the route that reset it: {attempt}");
                }
                return ExitCode::SUCCESS;
            }
        }
    }
}

/// The guest's console as a channel of lines, read on a thread of its own so the caller can wait on
/// it with a deadline. Shared by [`second_boot`] and [`watchdog_verdict`].
fn console_lines(stdout: std::process::ChildStdout) -> std::sync::mpsc::Receiver<String> {
    use std::io::BufRead;

    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for line in std::io::BufReader::new(stdout).split(b'\n') {
            let Ok(line) = line else { break };
            if tx
                .send(String::from_utf8_lossy(&line).into_owned())
                .is_err()
            {
                break;
            }
        }
    });
    rx
}

/// **Milestone 593's two proofs under QEMU q35**: the watchdog resets a wedged kernel, and it never
/// resets a healthy one. Provisional number.
///
/// `--wedge` passes on three facts in order: the watchdog armed, the kernel announced its wedge, and
/// the soak's start line appeared again (milestone 249's "it came back") with that boot reporting
/// `SECOND_TO_STS` set, which the chipset sets only on a second timeout and nothing else in QEMU's
/// reset path touches. `--watchdog` passes when `window` has elapsed since arming with one boot, no
/// failure, and a pet that read its count below the reload value; a halted TCO reads its reload
/// value forever. Returns 0 proven, 1 disproved by the kernel, 2 the deadline passed first, 3
/// QEMU's output ended.
fn watchdog_verdict(
    stdout: std::process::ChildStdout,
    sink: &mut impl std::io::Write,
    window: std::time::Duration,
    wedge: bool,
) -> ExitCode {
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    // A healthy run's deadline is the window after arming plus the boot before it; two minutes
    // covers a TCG boot to the soak with a wide margin. A wedge run's window is already the whole
    // run.
    let deadline = if wedge {
        window
    } else {
        window + Duration::from_secs(120)
    };
    let rx = console_lines(stdout);
    let begun = Instant::now();
    let mut run = WatchdogRun {
        wedge,
        starts: 0,
        armed_at: None,
        counting: false,
        wedged_at: None,
    };
    loop {
        // The healthy run's pass condition is a quiet one: the window ran out and nothing bad
        // happened. So it is checked before waiting, not only when a line arrives.
        if let Some(verdict) = run.window_passed(window) {
            return verdict;
        }
        let wait_until = match (wedge, run.armed_at) {
            (false, Some(at)) => (at + window).min(begun + deadline),
            _ => begun + deadline,
        };
        let Some(left) = wait_until.checked_duration_since(Instant::now()) else {
            eprintln!(
                "soak-test: FAIL, {}s passed first (starts={}, armed: {}, wedged: {})",
                deadline.as_secs(),
                run.starts,
                run.armed_at.is_some(),
                run.wedged_at.is_some()
            );
            return ExitCode::from(2);
        };
        let line = match rx.recv_timeout(left) {
            Ok(line) => line,
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                eprintln!(
                    "soak-test: FAIL, QEMU's output ended (starts={}, wedged: {}); a QEMU \
                     that exits on a reset has not rebooted anything",
                    run.starts,
                    run.wedged_at.is_some()
                );
                return ExitCode::from(3);
            }
        };
        let _ = writeln!(sink, "{line}");
        if let Some(verdict) = run.read(&line) {
            return verdict;
        }
    }
}

/// What [`watchdog_verdict`] has seen of a run so far.
struct WatchdogRun {
    wedge: bool,
    starts: u32,
    armed_at: Option<std::time::Instant>,
    /// Whether a pet has reported a count below its reload value: the watchdog was counting.
    counting: bool,
    wedged_at: Option<std::time::Instant>,
}

impl WatchdogRun {
    // As `kernel/src/soak.rs` spells them.
    const STARTED: &str = "soak-test: started";
    const FAILED: &str = "soak-test: FAILED";
    const WATCHDOG: &str = "soak-test-watchdog:";

    /// A healthy run's verdict once `window` has passed since the watchdog armed, or `None` while
    /// it has not (and always for a wedge run).
    fn window_passed(&self, window: std::time::Duration) -> Option<ExitCode> {
        let at = self.armed_at.filter(|_| !self.wedge)?;
        if at.elapsed() < window {
            return None;
        }
        if !self.counting {
            eprintln!(
                "soak-test: FAIL, the watchdog armed but no pet ever found its count below the \
                 reload value: it was never counting, so surviving the window proves nothing"
            );
            return Some(ExitCode::from(1));
        }
        eprintln!();
        eprintln!(
            "soak-test: PASS, the watchdog was armed and counting for {}s and never reset this \
             healthy soak ({} boot)",
            window.as_secs(),
            self.starts
        );
        Some(ExitCode::SUCCESS)
    }

    /// Take in one console line. `Some` when the line decides the run.
    fn read(&mut self, line: &str) -> Option<ExitCode> {
        if line.contains("[PANIC]") || line.contains(Self::FAILED) {
            eprintln!("soak-test: FAIL, the soak itself failed: {line}");
            return Some(ExitCode::from(1));
        }
        if line.contains(Self::STARTED) {
            self.starts += 1;
            if self.starts >= 2 && self.wedged_at.is_none() {
                eprintln!(
                    "soak-test: FAIL, the machine reset and booted again without being wedged: a \
                     healthy soak was reset"
                );
                return Some(ExitCode::from(1));
            }
        }
        let rest = line
            .split_once(Self::WATCHDOG)
            .map(|(_, rest)| rest.trim())?;
        if rest.starts_with("NOT ARMED") {
            eprintln!("soak-test: FAIL, the watchdog did not arm: {rest}");
            return Some(ExitCode::from(1));
        }
        if rest.starts_with("ARMED.") && self.armed_at.is_none() {
            self.armed_at = Some(std::time::Instant::now());
        }
        if pet_was_counting(rest) {
            self.counting = true;
        }
        if rest.starts_with("WEDGING NOW") {
            self.wedged_at = Some(std::time::Instant::now());
        }
        if !self.wedge || self.starts < 2 {
            return None;
        }
        self.second_boot_verdict(rest)
    }

    /// A wedge run's verdict from the second boot's report on why the first one ended.
    fn second_boot_verdict(&self, rest: &str) -> Option<ExitCode> {
        if rest.starts_with("the previous boot was ended by this watchdog") {
            eprintln!();
            eprintln!(
                "soak-test: PASS, the wedged kernel was reset by the TCO watchdog and booted \
                 again, {}s after the wedge; the second boot read SECOND_TO_STS set",
                self.wedged_at.map_or(0, |at| at.elapsed().as_secs())
            );
            return Some(ExitCode::SUCCESS);
        }
        if rest.starts_with("the previous boot was not ended by this watchdog") {
            eprintln!(
                "soak-test: FAIL, the machine came back but SECOND_TO_STS was clear: something \
                 other than the watchdog reset it"
            );
            return Some(ExitCode::from(1));
        }
        None
    }
}

/// Whether a pet's report (`... the count had <left> of <reload> ticks ...`) found the count below
/// its reload value, which a halted TCO never does.
fn pet_was_counting(rest: &str) -> bool {
    if let Some(counts) = rest
        .split_once("the count had ")
        .and_then(|(_, tail)| tail.split_once(" ticks"))
        .map(|(counts, _)| counts)
        && let Some((left, reload)) = counts.split_once(" of ")
        && let (Ok(left), Ok(reload)) = (left.parse::<u32>(), reload.parse::<u32>())
    {
        left < reload
    } else {
        false
    }
}
