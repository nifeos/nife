//! The boot ladder's gate (milestone 268, item 6).

use std::process::Command;

use crate::archive::{
    initial_ramdisk_path, initial_ramdisk_riscv, initial_ramdisk_x86, riscv_initial_ramdisk_path,
    x86_initial_ramdisk_path,
};
use crate::host::{flag_value, run};
use crate::{RISCV_TARGET, RUNNER, TARGET, X86_TARGET, profile_dir, user};

/// **Boot every architecture's default kernel and fail if it does not climb the whole ladder**:
/// the configuration `cargo xtask run` gives a person (nothing else in CI boots the default riscv64
/// or `x86_64` kernel), through the machine description (`Stage::Machine`), every boot self-test's
/// printed verdict, and up to the `swish` prompt. `--inject` rebuilds with `--features
/// self_test_injection`, which fails one check, and requires red: a gate only seen green is untested.
///
/// It attaches the archive but no disk: nothing here types, so `<` and `>` are `swish-check`'s. The
/// archive costs about six seconds; `x86_64` boots PVH `-kernel`, not the UEFI image `swish-check`
/// pays minutes for under OVMF, since the prompt reaches COM1 either way. See notes/boot-ladder.md.
///
/// # BUGS
///
/// - `quiet_after` is off from `Stage::Tour` up, below the prompt on riscv64, so a wedge there
///   waits for the 180-second cap.
/// - The prompt rung is the shell's banner, not `$ ` (`boot_ladder::PROMPT`'s `BUGS`): it proves
///   `swish` started, not that it takes input; `swish-check` types.
/// - An emulator was once found orphaned after many `x86_64` boots, cause unknown; after a session,
///   `pgrep -l qemu` and walk `ps -o pid,ppid` before killing anything.
/// - A red verdict ends the watch early, so the report names the first failure, not the worst.
/// - Name provisional; calef has not ruled on `boot-check`.
pub(crate) fn boot_check() -> bool {
    let legs = match flag_value("--arch").as_deref() {
        None => [true, true, true],
        Some("aarch64") => [true, false, false],
        Some("riscv64") => [false, true, false],
        Some("x86_64") => [false, false, true],
        Some(other) => {
            eprintln!(
                "boot-check: --arch {other} is not an architecture (aarch64, riscv64 or x86_64)"
            );
            return false;
        }
    };
    let inject = std::env::args().any(|a| a == "--inject");

    // TCG only, for `swish-check`'s reason: this boot never exits, so it is killed rather than
    // waited on, and there is nothing acceleration would buy a gate that spends its time waiting
    // for a line on a serial port.
    // SAFETY: xtask is single-threaded here. This runs on the main thread before any child that
    // reads the environment is spawned, and the only thread started below is `watch`'s reader,
    // which copies pipe bytes and never touches the environment.
    unsafe { std::env::remove_var("NIFE_ACCEL") };

    let mut ok = true;
    // The archive packer per architecture, and where it writes. The packer runs before the kernel
    // build: it writes the measurement manifest `kernel/build.rs` reads (milestone 22 (trusted
    // init) phase B.1), and a kernel built against a stale one refuses to load the progenitor.
    for (i, (arch, target, runner, pack, archive)) in [
        (
            "aarch64",
            TARGET,
            RUNNER,
            user as fn() -> bool,
            initial_ramdisk_path as fn() -> String,
        ),
        (
            "riscv64",
            RISCV_TARGET,
            "helpers/qemu-runner-riscv64.sh",
            initial_ramdisk_riscv as fn() -> bool,
            riscv_initial_ramdisk_path as fn() -> String,
        ),
        (
            "x86_64",
            X86_TARGET,
            "helpers/qemu-runner-x86_64.sh",
            initial_ramdisk_x86 as fn() -> bool,
            x86_initial_ramdisk_path as fn() -> String,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        if legs[i] && !boot_check_leg(arch, target, runner, inject, pack, archive) {
            ok = false;
        }
    }
    if ok {
        eprintln!();
        // Two sentences rather than one with a hole in it: an injected run **stops at the red
        // verdict** and never reaches the prompt, so a single wording would claim a rung that leg
        // deliberately did not climb.
        eprintln!(
            "{}",
            if inject {
                "boot-check: every architecture's self-test verdict came back RED, as the \
                 injection asked"
            } else {
                "boot-check: every architecture climbed the ladder to a shell prompt, and the \
                 self-test verdict on the way was green"
            },
        );
    }
    ok
}

/// One architecture's leg of [`boot_check`]. Packs the archive, builds, boots, watches for the
/// prompt, kills QEMU, and says what it found either way.
fn boot_check_leg(
    arch: &str,
    target: &str,
    runner: &str,
    inject: bool,
    pack: fn() -> bool,
    archive: fn() -> String,
) -> bool {
    eprintln!();
    eprintln!(
        "--- boot-check ({arch}): boot the default kernel and climb the ladder to the prompt{} ---",
        if inject {
            ", with a failure injected"
        } else {
            ""
        },
    );

    // Before the kernel, always: see the table in [`boot_check`].
    if !pack() {
        eprintln!("boot-check ({arch}): could not pack the userspace archive");
        return false;
    }

    let mut build = vec!["build", "-p", "kernel", "--target", target];
    if inject {
        build.extend_from_slice(&["--features", "self_test_injection"]);
    }
    if !run("cargo", &build) {
        return false;
    }

    let Some((mut child, stdout)) = start_boot_check_leg(arch, target, runner, archive) else {
        return false;
    };

    // The same recogniser `cargo xtask board-console` points at a real board, which is the point:
    // a gate that used a second reader would be gating something the bench does not measure.
    let policy = boot_check_policy();

    let log_path = format!(
        "target/boot-check-{arch}{}.log",
        if inject { "-injected" } else { "" }
    );
    let mut sink: Box<dyn std::io::Write> = match std::fs::File::create(&log_path) {
        Ok(f) => Box::new(std::io::BufWriter::new(f)),
        Err(e) => {
            eprintln!("boot-check ({arch}): cannot write {log_path}: {e}");
            let _ = child.kill();
            let _ = child.wait();
            return false;
        }
    };
    // `stream_never_ends: false`: this is a pipe, so a zero-byte read means the emulator is gone.
    let session = serial_console::watch::watch(stdout, &mut *sink, &policy, false);
    let _ = sink.flush();

    stop_boot_check_leg(&mut child);

    let session = match session {
        Ok(s) => s,
        Err(e) => {
            eprintln!("boot-check ({arch}): reading the console failed: {e}");
            eprintln!("boot-check ({arch}): log at {log_path}");
            return false;
        }
    };

    eprintln!("boot-check ({arch}): {}", session.summary());
    if let Some(line) = session.progress.machine_line() {
        eprintln!("boot-check ({arch}): {line}");
    }
    if let Some(line) = session.progress.self_test_line() {
        eprintln!("boot-check ({arch}): {line}");
    }

    boot_check_verdict(&session, arch, inject, &log_path)
}

/// **Start the leg's emulator** on the kernel just built and the archive `archive` names, its
/// console piped. `None` after saying why it did not start.
fn start_boot_check_leg(
    arch: &str,
    target: &str,
    runner: &str,
    archive: fn() -> String,
) -> Option<(std::process::Child, std::process::ChildStdout)> {
    // The runner directly rather than through `cargo run`, so the process this owns is the
    // emulator (or, on x86, its wrapper; see the kill below). A `cargo run` in between would leave
    // the emulator alive when the kill lands on cargo, which is the leak AGENTS.md's QEMU rule
    // exists about.
    let mut cmd = Command::new(runner);
    cmd.arg(format!("target/{target}/{}/kernel", profile_dir()));
    // Only the archive: this gate types nothing, so no disk. `host::cargo` exports `NIFE_DISK` and
    // `NIFE_NET` into this process for every cargo call, and the aarch64 and `x86_64` runners treat
    // a `NIFE_DISK` naming a missing file as fatal, so the child's environment is set here rather
    // than inherited; an exported `NIFE_DISK` in a shell cannot change what this boots either.
    cmd.env("NIFE_INITIAL_RAMDISK", archive());
    cmd.env_remove("NIFE_DISK");
    cmd.env_remove("NIFE_NET");
    cmd.stdout(std::process::Stdio::piped());
    // The guest's own stderr is not interesting and QEMU's is, so it is left attached: a runner
    // that cannot find its emulator should say so on this terminal rather than into a pipe nobody
    // reads.
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("boot-check ({arch}): failed to start {runner}: {e}");
            return None;
        }
    };
    let stdout = child.stdout.take().expect("piped stdout");

    Some((child, stdout))
}

/// **What the leg waits for and how long**, in `serial_console`'s terms.
fn boot_check_policy() -> serial_console::watch::Policy {
    use serial_console::progress::Stage;

    serial_console::watch::Policy {
        // Generous: a debug kernel under TCG on a loaded CI runner is slow, and the cost of a
        // too-short cap is a red build that means nothing.
        total: std::time::Duration::from_secs(180),
        // **The top rung, on every architecture** (2026-09-19). `Stage::SelfTest` until then,
        // because `x86_64` could reach no prompt; see this module's "The top rung".
        until: Some(Stage::Prompt),
        // Each rung follows the one below it within milliseconds up to the verdict, and the
        // hand-over then loads and measures every program the progenitor builds, which is the one
        // gap in the ladder with real work in it. Half a minute of silence is a wedge rather than
        // slowness at any point in that.
        quiet_after: Some(std::time::Duration::from_secs(30)),
        // Keep reading after the rung arrives, so a verdict followed immediately by a panic is
        // reported as the panic rather than as a success. `watch`'s own doc records the capture
        // this defends against.
        settle: std::time::Duration::from_secs(2),
        // **The emulator's profile**: no firmware prologue (there is none on the `virt` or `q35`
        // machines to print `U-Boot SPL`), and no self-test check it must have, because these
        // machines attach no disk and, on riscv64, no entropy source. xenon's and radon's lists
        // name devices those boards have, and would read a QEMU leg's honest `absent` as a
        // failure.
        board: &serial_console::board::EMULATOR,
    }
}

/// Kill any QEMU the runner spawned before killing the runner itself.
fn stop_boot_check_leg(child: &mut std::process::Child) {
    // On `q35` the runner is a
    // *wrapper* that runs the emulator as a plain child rather than `exec`-ing into it, so
    // `child.kill()` alone orphans the emulator rather than ending it. The other two runners
    // `exec`, so this finds nothing there. Best effort, silent either way. Same reasoning as
    // `run_bench`; see notes/skills/qemu-hygiene/SKILL.md, "Never leave QEMU running".
    let _ = Command::new("pkill")
        .args(["-9", "-P", &child.id().to_string()])
        .status();
    let _ = child.kill();
    let _ = child.wait();
}

/// **The leg's verdict**: green climbed to the prompt with nothing failed, and an injected run
/// must come back red instead. A failure says which, and where the log is.
fn boot_check_verdict(
    session: &serial_console::watch::Session,
    arch: &str,
    inject: bool,
    log_path: &str,
) -> bool {
    use serial_console::progress::{Failure, Stage};
    use serial_console::watch::Outcome;

    let red = matches!(
        session.outcome,
        Outcome::Announced(Failure::SelfTestFailed(_))
    );
    // `>= Stage::Prompt` rather than `== `: the rungs are ordered, so a boot that went further
    // still climbed past this one, and an architecture that grows a rung above the prompt should
    // not turn this gate red on the day it lands.
    let green = matches!(session.outcome, Outcome::Reached(_))
        && session.progress.reached() >= Stage::Prompt
        && session.progress.failure().is_none();

    let pass = if inject { red } else { green };
    if !pass {
        eprintln!();
        if inject {
            eprintln!(
                "boot-check ({arch}): FAILED. A kernel built with `--features self_test_injection` \
                 must print a RED verdict and this run did not, so the gate cannot see a failing \
                 self-test and is not gating anything. This is the defect milestone 268's block \
                 predicted: a verdict nobody reads."
            );
        } else {
            eprintln!(
                "boot-check ({arch}): FAILED. The default boot did not climb the ladder to a \
                 `swish` prompt with a green self-test verdict on the way. The log is the \
                 diagnosis, and the rung named above says where it stopped: `nife machine:` says \
                 how far discovery got, `nife self-test:` names whatever did not pass, and a boot \
                 that reached the verdict and no further did not get userspace up."
            );
        }
        eprintln!("boot-check ({arch}): log at {log_path}");
    }
    pass
}
