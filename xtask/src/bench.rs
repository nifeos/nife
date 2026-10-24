//! The `cargo xtask bench` command: the cross-OS measurements, and the check that compares a
//! run against the recorded baselines.

use std::process::Command;
use std::sync::atomic::Ordering;

use crate::archive::{initial_ramdisk_path, initial_ramdisk_riscv, riscv_initial_ramdisk_path};
use crate::disk::{disk_path, mkdisk, mkredoxfs, redoxfs_server_build};
use crate::host::{flag_value, kernel_elf, run, workspace_root};
use crate::{RELEASE, RISCV_TARGET, RUNNER, TARGET, X86_TARGET, cargo_profiled, user};

/// The microbenchmarks (milestone 21; design/roadmap/0021-benchmarks.md), with two instruments:
///
/// - default: TCG with `-icount`, exact and reproducible counts. `--check` diffs them against
///   `bench/baseline-aarch64.txt` and fails on drift; `--save --why "<reason>"` rewrites the
///   baseline and is refused without a reason (milestone 302).
/// - `--real`: HVF on the host core, statistical nanoseconds, never gating. `--release` implies it.
///
/// The bench kernel never exits on its own (semihosting does not work under HVF), so xtask watches
/// the QEMU child for `bench: done` and kills it under both accelerators. See notes/benchmarks.md.
pub(crate) fn bench() -> bool {
    let check = std::env::args().any(|a| a == "--check");
    let save = std::env::args().any(|a| a == "--save");
    // Release changes instruction counts, so it never runs under icount and never gates.
    let release = std::env::args().any(|a| a == "--release");
    RELEASE.store(release, Ordering::Relaxed);
    let real = release || std::env::args().any(|a| a == "--real");
    if real && (check || save) {
        let why = if release { "--release" } else { "--real" };
        eprintln!("bench: {why} numbers are statistical and never gate; no --check/--save");
        return false;
    }

    // A save that cannot say why is refused (milestone 302, calef 2026-09-16; milestone 415 item
    // 2): a row records a number and nothing about why it holds it, so a reader cannot tell
    // compiler drift from a regression (notes/benchmarks.md, `rfence_self`). Refusing is the
    // highest rung a string command line can reach, and the cost §190 (must an icount baseline
    // save record why it moved) weighed is a reason typed on every save. Checked before the build
    // so it costs a second; `run_bench` reads the flag again at the write.
    if save && save_reasons().is_empty() {
        eprintln!("bench: --save needs --why \"<reason>\". A floor with no recorded reason cannot");
        eprintln!("  tell the next reader drift from a regression. The reason goes in the file,");
        eprintln!("  beside the numbers, because a commit message is read once, on the day it is");
        eprintln!("  written, by one person.");
        eprintln!();
        eprintln!("  e.g. script/bench --save --why \"the cycle-counter grant at the switch\"");
        eprintln!("  Repeat --why once per reason when one save covers several moves.");
        return false;
    }

    // E3 (milestone 134, design/roadmap/0134-the-measurements-that-decide.md): an extra kernel
    // feature beside `bench`, for the padded-fastpath experiment. `--real` only: there is no cache
    // under TCG for a footprint change to perturb, so an icount run would measure noise.
    let extra_features = flag_value("--extra-features");
    if extra_features.is_some() && !real {
        eprintln!(
            "bench: --extra-features only makes sense with --real (no cache under TCG for it to move)"
        );
        return false;
    }
    let features = match &extra_features {
        Some(f) => format!("bench,{f}"),
        None => "bench".to_string(),
    };

    // **`--restamp`: carry the floor across a nightly bump without re-saving it** (milestone 598 (a
    // nightly bump restamps the floors it proves it did not move), calef 2026-09-26). It runs this
    // leg twice, under the stamped nightly and under the pin, and never writes a number; see
    // `restamp.rs`. It is its own mode rather than a modifier, so it refuses the others.
    if std::env::args().any(|a| a == "--restamp") {
        if real || check || save {
            eprintln!("bench: --restamp is a mode of its own; drop --check, --save and --real");
            return false;
        }
        return crate::restamp::restamp(|| leg(false, false, false, &features));
    }
    leg(real, check, save, &features)
}

/// One bench pass on the architecture the command line names: build, boot, read the rows, and
/// check or save them if asked. Split out of `bench` so `--restamp` can run it twice.
fn leg(real: bool, check: bool, save: bool, features: &str) -> bool {
    // RISC-V has its own kernel target, runner and initial ramdisk, and no disk or HVF; the
    // instrument, parsing, table and gate are shared through `run_bench`.
    if std::env::args().any(|a| a == "--riscv") {
        return bench_riscv(check, save, features);
    }

    // x86_64 gates under icount like the other two (milestone 161); see `bench_x86`.
    if std::env::args().any(|a| a == "--x86") {
        return bench_x86(real, check, save, features);
    }

    // `--smp`: boot 4 harts under HVF for `smp_throughput` (DECISIONS §28) and `fs_read`
    // (DECISIONS §32), which self-skip on one hart. Only meaningful with `--real`.
    let smp = std::env::args().any(|a| a == "--smp");

    // For --smp, the FS server goes before `user()` so the archive packs it, and the RedoxFS image
    // is the second mmio disk `fs_read` opens. Without it the bench skips, so the icount gate's
    // build never needs it.
    if (smp && !redoxfs_server_build(TARGET))
        // `fs_walk` (milestone 606) runs `std_exerciser`, which reaches the archive only if built
        // first, from this worktree's own std farm (notes/std.md).
        || (smp && !crate::farm::std_exerciser())
        || !mkdisk()
        || !user()
        || (smp && !mkredoxfs())
        || !cargo_profiled(&[
            "build",
            "-p",
            "kernel",
            "--features",
            features,
            "--target",
            TARGET,
        ])
    {
        return false;
    }

    // sleep=off: under icount, virtual time never waits for the wall clock.
    let mut cmd = Command::new(RUNNER);
    cmd.arg(kernel_elf());
    if real {
        cmd.env("NIFE_ACCEL", "hvf");
        if smp {
            // Primitives in this run are not per-core clean (the reap-heavy ones inflate under
            // cross-core reap lag); see notes/benchmarks/per-core-and-multi-hart.md. The bench
            // reads the online count, so this only needs to be more than one.
            cmd.env("NIFE_SMP", "4");
            eprintln!(
                "--- bench: HVF, 4 harts (for smp_throughput; primitives are not per-core here) ---"
            );
        } else {
            // One hart: a primitive magnitude is a per-core number, and at `-smp 4` the reap-heavy
            // primitives pick up cross-core reap lag (notes/benchmarks/per-core-and-multi-hart.md).
            cmd.env("NIFE_SMP", "1");
            eprintln!(
                "--- bench: HVF, single hart, per-core magnitudes (statistical; medians matter) ---"
            );
        }
    } else {
        cmd.env_remove("NIFE_ACCEL");
        cmd.args(["-icount", "shift=0,sleep=off"]);
        // One hart: CNTVCT advances with QEMU's global virtual time, and under `-icount` an idle
        // hart in `wfi` jumps it to the next tick, so `-smp 4` counts other harts' idle jumps
        // (notes/benchmarks/icount-drift-and-provenance.md, the 2026-07-28 attribution).
        cmd.env("NIFE_SMP", "1");
        eprintln!(
            "--- bench: aarch64, single hart, TCG + icount (deterministic instruction counts) ---"
        );
    }
    cmd.env("NIFE_INITIAL_RAMDISK", initial_ramdisk_path());
    cmd.env("NIFE_DISK", disk_path());

    run_bench(
        cmd,
        real,
        check,
        save,
        workspace_root().join("bench/baseline-aarch64.txt"),
        "qemu-system-aarch64",
    )
}

/// **The RISC-V benchmark path**: the same primitive suite under the same icount instrument, so
/// its ticks compare directly with aarch64's. No HVF (no RISC-V hypervisor on this host) and no
/// disk (the bench boot runs no virtio); the riscv initial ramdisk carries
/// `os_primitives_benchmarker` and `coremark`. Its baseline is `bench/baseline-riscv64.txt`.
/// `cargo xtask bench --riscv [--check|--save]`.
fn bench_riscv(check: bool, save: bool, features: &str) -> bool {
    if !initial_ramdisk_riscv()
        || !run(
            "cargo",
            &[
                "build",
                "-p",
                "kernel",
                // `features`, not "bench": it carries `--extra-features` too.
                "--features",
                features,
                "--target",
                RISCV_TARGET,
            ],
        )
    {
        return false;
    }

    let mut cmd = Command::new("helpers/qemu-runner-riscv64.sh");
    cmd.arg(format!("target/{RISCV_TARGET}/debug/kernel"));
    // icount pins `rdtime` to the instruction stream; sleep=off never waits on the wall clock.
    cmd.args(["-icount", "shift=0,sleep=off"]);
    cmd.env("NIFE_INITIAL_RAMDISK", riscv_initial_ramdisk_path());
    // `host::cargo` exports `NIFE_DISK` for every child, and the riscv64 runner treats one naming
    // a missing file as fatal. Removing it moves rows by at most 0.05%
    // (notes/benchmarks/drift-since-the-2026-09-26-floors.md). `boot_check.rs` does the same.
    cmd.env_remove("NIFE_DISK");
    // One hart: a parent waiting on a spawned child leaves its hart in `wfi`, and under `-icount`
    // that jumps virtual time to the next tick, quantizing the spawn primitives.
    cmd.env("NIFE_SMP", "1");
    eprintln!(
        "--- bench: riscv64, single hart, TCG + icount (deterministic instruction counts) ---"
    );

    run_bench(
        cmd,
        false,
        check,
        save,
        workspace_root().join("bench/baseline-riscv64.txt"),
        "qemu-system-riscv64",
    )
}

/// **The `x86_64` benchmark path** (DECISIONS §121's amendment, milestone 161 item 4): the same
/// suite, `_el0` benches self-skipping (no initial ramdisk), plus `tss_iomap_switch`, the cost of
/// §121 (what a device capability is when the device has no page)'s option 1: `yield_switch` with
/// an I/O-bitmap write per switch-in (`kernel/src/arch/x86_64/segments.rs`).
///
/// Default is TCG plus `-icount shift=0,sleep=off`, gated against `bench/baseline-x86_64.txt`;
/// `--real` is plain TCG, statistical and never gating. `helpers/qemu-runner-x86_64.sh` attaches no
/// disk, so this needs neither `mkdisk` nor `user()`. `cargo xtask bench --x86 [--real]
/// [--check|--save]`; why icount works here though `icount()` refuses x86_64 is in
/// notes/benchmarks/x86-instruments.md.
fn bench_x86(real: bool, check: bool, save: bool, features: &str) -> bool {
    if !run(
        "cargo",
        &[
            "build",
            "-p",
            "kernel",
            // As in `bench_riscv`. `fastpath_pad` does not build on x86_64
            // (`script/fastpath-footprint`'s BUGS), so cargo refuses rather than mismeasures.
            "--features",
            features,
            "--target",
            X86_TARGET,
        ],
    ) {
        return false;
    }

    let mut cmd = Command::new("helpers/qemu-runner-x86_64.sh");
    cmd.arg(format!("target/{X86_TARGET}/debug/kernel"));
    // One core, pinned rather than inherited from the runner, whose default is 2. Two cores are a
    // different instrument: still deterministic, but `spawn_reap`'s parent spins until the reaper
    // on the other core runs, and unrelated edits swing rows by 40 to 90%
    // (notes/benchmarks/x86-instruments.md; gating them is
    // design/roadmap/proposals/two-core-bench-is-a-different-instrument.md).
    cmd.env("NIFE_SMP", "1");
    if real {
        eprintln!(
            "--- bench: x86_64, single hart, plain TCG (no KVM/HVF on this host; statistical) ---"
        );
    } else {
        cmd.args(["-icount", "shift=0,sleep=off"]);
        eprintln!(
            "--- bench: x86_64, single hart, TCG + icount (deterministic instruction counts) ---"
        );
    }

    run_bench(
        cmd,
        real,
        check,
        save,
        workspace_root().join("bench/baseline-x86_64.txt"),
        "qemu-system-x86_64",
    )
}

/// The rows of the most recent `run_bench` in this process, for `--restamp`, which runs a leg
/// twice and compares. A static rather than a return value because the three legs return a pass or
/// fail and `--check` and `--save` consume the rows inside `run_bench`; threading them out through
/// every leg would change four signatures to serve one caller.
static LAST_RUN: std::sync::Mutex<Vec<(String, u64, u64)>> = std::sync::Mutex::new(Vec::new());

/// Take the rows the last bench pass reported, leaving none behind.
pub(crate) fn take_last_run() -> Vec<(String, u64, u64)> {
    std::mem::take(&mut *LAST_RUN.lock().unwrap_or_else(|e| e.into_inner()))
}

/// The `--why <reason>` values of this invocation, in the order they were given.
///
/// Repeatable, unlike `flag_value`, because one save often covers several moves with different
/// causes: the 2026-09-15 saves blessed a real regression and a toolchain term into the same file
/// and had one sentence between them. Each value becomes its own `# why:` line.
///
/// `--why` is a provisional name (milestone 302's lane); the `# why:` line it writes is the one
/// calef ratified on 2026-09-16, and the flag is named after the line rather than the other way
/// round.
fn save_reasons() -> Vec<String> {
    let mut out = Vec::new();
    let mut args = std::env::args();
    while let Some(a) = args.next() {
        // **An empty or blank reason is not a reason**, and dropping it here is what makes the
        // refusal above mean something: `--why ""` otherwise satisfies the check, runs a full
        // emulator pass and writes a floor whose ledger line is blank. Found by typing it.
        let given = if a == "--why" {
            args.next()
        } else {
            a.strip_prefix("--why=").map(str::to_owned)
        };
        if let Some(v) = given.filter(|v| !v.trim().is_empty()) {
            out.push(v);
        }
    }
    out
}

/// Today, UTC, as `YYYY-MM-DD`.
///
/// Shelled out rather than taken as a dependency: §46 says thin primitives or whole subsystems and
/// nothing in between, and a date crate for one line in one header is squarely in between. UTC
/// because AGENTS.md says every date in this tree is UTC, and the machines writing most of them do
/// not all agree with the architect's clock. `unrecorded` when `date(1)` cannot be asked, which is
/// the same honest answer the `# qemu:` line already gives.
pub(crate) fn today_utc() -> String {
    Command::new("date")
        .args(["-u", "+%Y-%m-%d"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unrecorded".into())
}

/// The `# why:` lines a baseline file carries, in the order they were written.
///
/// Empty for the three floors saved before milestone 302 and for any file that predates the
/// ledger, which is why the caller prints nothing rather than claiming the numbers are unexplained
/// on purpose.
fn recorded_reasons(text: &str) -> Vec<&str> {
    text.lines()
        .filter_map(|l| l.trim_start().strip_prefix("# why:"))
        .map(str::trim)
        .collect()
}

/// The comment header a `--save` writes above the rows, built as a pure function of its four
/// facts so it can be tested without running a benchmark or rewriting a committed floor.
///
/// **A floor is an architect's to move**, so the only honest way to test the writer is not to call
/// the writer: `--save` commits a performance floor, and there is no dry run that would not be one.
/// Pulling the header out of the write is what makes the format checkable at all.
fn baseline_header(
    stem: &str,
    toolchain: &str,
    qemu: &str,
    date: &str,
    reasons: &[String],
) -> String {
    let why: String = reasons
        .iter()
        .map(|w| format!("# why: {}\n", w.trim()))
        .collect();
    format!(
        "# toolchain: {toolchain}
# qemu: {qemu}
# date: {date}
{why}# bench/{stem}: deterministic icount tick counts (cargo xtask bench --save).
# Recorded against the QEMU pinned in .qemu-version. icount counts guest instructions, so the
# emulator version is part of what these numbers mean: script/qemu-check warns when the QEMU on
# PATH is not the pinned one, precisely because that is when a baseline comparison stops being
# apples to apples.
# A tick is not an instruction: one guest instruction is one virtual nanosecond under -icount
# shift=0 and this counter ticks at its own rate (instructions = ticks * 1e9 / cntfrq), so raw
# ticks do not compare across architectures; notes/benchmarks/icount-tick-scales.md holds the
# per-architecture scales and the corrected cross-arch table.
             # Updating this file is a statement that a performance change is intended and
             # understood; do it in the commit that causes the change. Checked by --check, a coarse
             # 10% tripwire (icount counts drift across builds; see notes/benchmarks.md).
"
    )
}

/// The nightly `rust-toolchain.toml` pins, as the baseline header records it.
///
/// A three-line parse rather than a TOML dependency: §46 (thin primitives or whole subsystems) is the
/// rule, and one `channel = "..."` line does not justify one. `None` when the file has no channel
/// line at all, which the caller turns into `unrecorded` rather than a guess.
pub(crate) fn pinned_nightly() -> Option<String> {
    let text = std::fs::read_to_string(workspace_root().join("rust-toolchain.toml")).ok()?;
    text.lines()
        .find(|l| l.trim_start().starts_with("channel"))
        .and_then(|l| l.split('"').nth(1).map(str::to_owned))
}

/// The QEMU version `.qemu-version` pins, or `None` if the file is unreadable.
fn pinned_qemu() -> Option<String> {
    std::fs::read_to_string(workspace_root().join(".qemu-version"))
        .ok()
        .map(|s| s.trim().to_owned())
}

/// The version of the emulator that is actually going to run, asked of the binary itself.
///
/// **Not `.qemu-version`, and the asymmetry with `pinned_nightly` above is the whole point.**
/// `rustup` RESOLVES the compiler from `rust-toolchain.toml`, so the pin and the thing that ran are
/// one fact by construction. Nothing resolves QEMU from `.qemu-version`: it is a wish about the
/// machine, and on 2026-08-28 this machine stopped granting it (11.1.1 installed against a pinned
/// 11.0.2) with nothing to say so. An icount count is a function of the compiler that produced the
/// instructions AND the emulator that counted them, so recording the pin here would file intent
/// under the heading of provenance, which is the defect this whole mechanism exists to close.
fn running_qemu(binary: &str) -> Option<String> {
    let out = Command::new(binary).arg("--version").output().ok()?;
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .next()?
        .split_whitespace()
        .nth(3)
        .map(str::to_owned)
}

/// Run a bench kernel through `cmd`, read its `bench:` lines until `bench: done`, and report the
/// table (and, off the deterministic icount instrument, save or check against `baseline`). Shared by
/// the aarch64 and RISC-V bench paths so the parsing, the table, and the regression gate are one
/// implementation. `real` only chooses the "ns are fiction" footer.
fn run_bench(
    mut cmd: Command,
    real: bool,
    check: bool,
    save: bool,
    baseline_path: std::path::PathBuf,
    // The emulator that will count these instructions, named so the baseline can record which one
    // did. Per architecture, because the three legs run three different binaries.
    qemu_binary: &str,
) -> bool {
    cmd.stdout(std::process::Stdio::piped());

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("bench: failed to start the runner: {e}");
            return false;
        }
    };
    let runner_pid = child.id();

    let stdout = child.stdout.take().expect("piped stdout");
    let (results, cntfrq, done) = read_bench_lines(stdout);
    // Kill the runner's children before the runner. The x86_64 runner wraps QEMU rather than
    // `exec`-ing it (it translates `isa-debug-exit`'s status), so killing it first orphans the
    // emulator, and under `-icount sleep=off` a halted guest spins a full core forever. The other
    // runners `exec`, so `pkill -P` finds nothing there. Best-effort; the rule is
    // notes/skills/qemu-hygiene/SKILL.md, "Never leave QEMU running".
    let _ = Command::new("pkill")
        .args(["-9", "-P", &runner_pid.to_string()])
        .status();
    let _ = child.kill();
    let _ = child.wait();

    if !done {
        eprintln!("bench: the kernel stopped before printing `bench: done`; no results");
        return false;
    }

    report_bench_table(&results, cntfrq, real);
    if !real && !record_readings(&results, &baseline_path) {
        return false;
    }

    *LAST_RUN.lock().unwrap_or_else(|e| e.into_inner()) = results.clone();

    if save {
        return save_bench_baseline(&results, &baseline_path, qemu_binary);
    }
    if check {
        return check_bench_baseline(&results, &baseline_path, qemu_binary);
    }
    true
}

/// **The bench kernel's lines, read until it says `bench: done` or panics**: each result as (name,
/// ticks, iterations), the counter frequency, and whether `done` arrived.
///
/// A bench kernel that panics halts in `wfi` and never exits, so a reader waiting for `done` would
/// wait forever. The `[PANIC]` line is where it stops instead, with `done` false, which fails the
/// run. `TickWindow` (kernel/src/bench/timing.rs) relies on this: a tick in a timed window is a
/// panic, and it has to fail the gate rather than hang it.
fn read_bench_lines(stdout: impl std::io::Read) -> (Vec<(String, u64, u64)>, u64, bool) {
    // Read lines until the guest says it is done, then kill it: it is parked in wfi and will
    // never exit by itself (deliberately; see kernel/src/bench.rs).
    use std::io::BufRead;
    let reader = std::io::BufReader::new(stdout);
    let mut results: Vec<(String, u64, u64)> = Vec::new();
    let mut cntfrq: u64 = 0;
    let mut done = false;
    let mut lines = reader.lines();
    while let Some(Ok(line)) = lines.next() {
        if line.starts_with("[PANIC]") {
            // The panic's message is the line after its location.
            let message = lines.next().and_then(Result::ok).unwrap_or_default();
            eprintln!("bench: the kernel panicked: {line} {message}");
            break;
        }
        // A probe reports a count or a bitmask, not a duration, so it is echoed and never becomes
        // a row `--check` would police with a 10% tolerance. See `bench::map_new`'s probe.
        if let Some(probe) = line.strip_prefix("bench-probe: ") {
            eprintln!("  probe: {probe}");
            continue;
        }
        let Some(rest) = line.strip_prefix("bench: ") else {
            continue;
        };
        if rest == "done" {
            done = true;
            break;
        }
        let parts: Vec<&str> = rest.split_whitespace().collect();
        match parts.as_slice() {
            ["cntfrq", hz] => cntfrq = hz.parse().unwrap_or(0),
            [name, ticks, iters] => {
                if let (Ok(t), Ok(i)) = (ticks.parse(), iters.parse()) {
                    results.push((name.to_string(), t, i));
                }
            }
            _ => {}
        }
    }
    (results, cntfrq, done)
}

/// **`NIFE_BENCH_READINGS=<file>`: append this leg's icount rows to `file`** as `arch name ticks
/// iters`, tab-separated, so CI's bench job can keep what `main` measured as an artifact. The
/// weekly drift chart reads that artifact (`helpers/baseline_drift.py --update`, calef's ruling on
/// #2554, 2026-10-22 UTC) instead of parsing the table above out of a job log. An environment
/// variable rather than a flag because `script/ci-build bench` chains three legs in one command and
/// each must append to the same file. The architecture is the baseline's, read from its file name.
fn record_readings(results: &[(String, u64, u64)], baseline_path: &std::path::Path) -> bool {
    use std::io::Write;
    let Some(path) = std::env::var_os("NIFE_BENCH_READINGS") else {
        return true;
    };
    let Some(arch) = baseline_path
        .file_stem()
        .and_then(|s| s.to_str())
        .and_then(|s| s.strip_prefix("baseline-"))
    else {
        eprintln!("bench: {} names no architecture", baseline_path.display());
        return false;
    };
    let mut text = String::new();
    for (name, ticks, iters) in results {
        text.push_str(&format!("{arch}\t{name}\t{ticks}\t{iters}\n"));
    }
    let written = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .and_then(|mut f| f.write_all(text.as_bytes()));
    if let Err(e) = written {
        eprintln!(
            "bench: could not append readings to {}: {e}",
            path.to_string_lossy()
        );
        return false;
    }
    true
}

/// The results as a table, with ns computed from `cntfrq` for a magnitude to look at.
fn report_bench_table(results: &[(String, u64, u64)], cntfrq: u64, real: bool) {
    // Report. icount counts are the regression currency; ns is computed for both instruments
    // (fictional under icount, real under HVF) because a human wants a magnitude to look at.
    eprintln!();
    eprintln!(
        "{:<14} {:>12} {:>8} {:>12} {:>10}",
        "benchmark", "ticks", "iters", "ticks/iter", "ns/iter"
    );
    for (name, ticks, iters) in results {
        // `checked_div`, not `/`: a benchmark that reports zero iterations (a skip, or a future
        // diagnostic line) must not panic the whole harness after the run already happened.
        let per = ticks.checked_div(*iters).unwrap_or(0);
        let ns = (ticks * 1_000_000_000)
            .checked_div(cntfrq)
            .and_then(|v| v.checked_div(*iters))
            .unwrap_or(0);
        eprintln!("{name:<14} {ticks:>12} {iters:>8} {per:>12} {ns:>10}");
    }
    if !real {
        eprintln!("(TCG+icount: ticks are deterministic; ns are fiction. --real for magnitudes.)");
    }
}

/// **`--save`: write `results` as the baseline**, under a header that records what produced them.
fn save_bench_baseline(
    results: &[(String, u64, u64)],
    baseline_path: &std::path::Path,
    qemu_binary: &str,
) -> bool {
    // The header names the file it is in (milestone 73).
    let stem = baseline_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("baseline.txt");
    // The nightly comes from the pin, not the running compiler (a `RUSTUP_TOOLCHAIN` override
    // still writes the pin), and `script/lint` fails when it and `rust-toolchain.toml` disagree.
    // `--restamp` may later move it, so it reads "last proven valid for". The emulator comes from
    // the binary, and the line names both when they disagree. See
    // notes/benchmarks/icount-drift-and-provenance.md.
    let pinned = pinned_nightly().unwrap_or_else(|| "unrecorded".into());
    let ran = running_qemu(qemu_binary).unwrap_or_else(|| "unrecorded".into());
    let qemu = match pinned_qemu() {
        Some(pin) if pin != ran => format!("{ran} (run; .qemu-version pins {pin})"),
        _ => ran,
    };
    // calef's ratified `# saved:` line (2026-09-16), decomposed one fact per line because
    // `script/lint` reads `# toolchain:`. One `# why:` per `--why`, in the header: a save
    // rebuilds the file, so a comment beside a row would not survive the next one.
    let mut out = baseline_header(stem, &pinned, &qemu, &today_utc(), &save_reasons());
    for (name, ticks, iters) in results {
        out.push_str(&format!(
            "{name} {ticks} {iters}
"
        ));
    }
    if let Err(e) = std::fs::write(baseline_path, out) {
        eprintln!("bench: cannot write {}: {e}", baseline_path.display());
        return false;
    }
    eprintln!("bench: baseline saved to {}", baseline_path.display());
    true
}

/// **`--check`: compare `results` against the baseline** with the coarse tripwire, after asking
/// whether the baseline was counted by this emulator.
fn check_bench_baseline(
    results: &[(String, u64, u64)],
    baseline_path: &std::path::Path,
    qemu_binary: &str,
) -> bool {
    let Ok(text) = std::fs::read_to_string(baseline_path) else {
        eprintln!(
            "bench: no baseline at {} (run `cargo xtask bench --save --why \"...\"` first)",
            baseline_path.display()
        );
        return false;
    };
    let mut ok = true;

    ok &= emulator_matches_baseline(&text, baseline_path, qemu_binary);

    // `trim_start`: the header has indented comment lines, which a column-0 check would parse as
    // benchmark rows.
    for line in text.lines().filter(|l| !l.trim_start().starts_with('#')) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        let [name, base, _iters] = parts.as_slice() else {
            continue;
        };
        let base: u64 = base.parse().unwrap_or(0);
        let Some((_, cur, _)) = results.iter().find(|(n, _, _)| n == name) else {
            eprintln!("bench: CHECK FAIL {name}: in the baseline but not in this run");
            ok = false;
            continue;
        };
        // A COARSE tripwire: 10% either way, with a small absolute floor so tiny counts do not
        // false-alarm. Not 2%: adding unrelated *live* code shifts even untouched benchmarks by
        // several percent, non-uniformly, because the compiler remakes whole-crate inlining and
        // monomorphization decisions (measured: a new bench function moved yield_switch -7% while
        // ipc_rtt went +1.8%). So icount --check catches a gross regression, "you 3x'd IPC," not
        // a 3% one; --real medians, read by a human, are the fine signal. See notes/benchmarks.md.
        let slack = (base / 10).max(64);
        let (lo, hi) = (base.saturating_sub(slack), base + slack);
        if *cur < lo || *cur > hi {
            let delta = *cur as i64 - base as i64;
            eprintln!(
                "bench: CHECK FAIL {name}: {cur} vs baseline {base} ({delta:+} ticks,                      allowed +-{slack})"
            );
            ok = false;
        }
    }
    if ok {
        eprintln!("bench: check passed (all within 10% of baseline; coarse tripwire)");
    } else {
        eprintln!();
        // The saved reasons, printed where a person reading a red gate is already looking: is this
        // a cost somebody chose, or drift nobody has explained?
        let recorded = recorded_reasons(&text);
        if !recorded.is_empty() {
            eprintln!("bench: this baseline was last saved because:");
            for r in &recorded {
                eprintln!("  {r}");
            }
            eprintln!();
        }
        eprintln!(
            "bench: a benchmark moved. If intended, rerun with --save --why \"<reason>\" and                  commit the new baseline WITH the change that moved it."
        );
    }
    ok
}

/// Whether the baseline in `text` was recorded on the emulator this run used; false only when it
/// names a different one, after saying what to do about it.
fn emulator_matches_baseline(
    text: &str,
    baseline_path: &std::path::Path,
    qemu_binary: &str,
) -> bool {
    // Asked here because nothing resolves QEMU from `.qemu-version`, so only a run can ask it.
    // Compared against the emulator this run used, not the pin, which would forbid a baseline
    // truthfully recorded off-pin. `unrecorded` does not fail. A mismatch means re-recording and
    // settling the pin together, since CI builds the pinned emulator (`script/ci-qemu`). See
    // notes/benchmarks/icount-drift-and-provenance.md.
    let stamped_qemu = text
        .lines()
        .filter_map(|l| l.trim_start().strip_prefix("# qemu:"))
        .next()
        .and_then(|v| v.split_whitespace().next())
        .map(str::to_owned);
    let mut ok = true;
    match (stamped_qemu.as_deref(), running_qemu(qemu_binary)) {
        (None, _) | (Some("unrecorded"), _) => eprintln!(
            "bench: {} does not say which emulator produced it, so this comparison is \
             assumed rather than known. The next --save records it.",
            baseline_path.display()
        ),
        (Some(want), Some(have)) if want != have => {
            let f = baseline_path.display();
            eprintln!("bench: CHECK FAIL: {f} was recorded on QEMU {want}; this run used {have}.");
            eprintln!(
                "  icount counts guest instructions and a different emulator counts them \
                 differently, so a pass here would be evidence of nothing."
            );
            eprintln!(
                "  Either run the emulator these floors were recorded on, or re-record them \
                 deliberately with --save and settle .qemu-version in the same commit."
            );
            ok = false;
        }
        (Some(want), Some(_)) => eprintln!("bench: emulator matches the baseline's ({want})"),
        (Some(_), None) => {
            eprintln!(
                "bench: cannot ask {qemu_binary} its version; the emulator check is skipped."
            );
        }
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::baseline_header;

    /// The two lines `script/lint` and milestone 302 read, in the shape their readers expect.
    ///
    /// `script/lint`'s baseline-toolchain check matches `^#\s*toolchain:\s*(\S+)\s*$`, so the
    /// stamp has to stay alone on its line with nothing appended; a `# why:` line carrying a
    /// compiler name must not be mistakable for it. That is the regression this asserts.
    #[test]
    fn header_records_the_stamp_and_the_reasons() {
        let h = baseline_header(
            "baseline-riscv64.txt",
            "nightly-2026-09-22",
            "11.1.1",
            "2026-09-23",
            &[
                "rfence_self back to 5991 at one hart".to_string(),
                "nightly-2026-09-22: no codegen term, measured ~0".to_string(),
            ],
        );
        let lines: Vec<&str> = h.lines().collect();
        assert_eq!(lines[0], "# toolchain: nightly-2026-09-22");
        assert_eq!(lines[1], "# qemu: 11.1.1");
        assert_eq!(lines[2], "# date: 2026-09-23");
        assert_eq!(lines[3], "# why: rfence_self back to 5991 at one hart");
        assert_eq!(
            lines[4],
            "# why: nightly-2026-09-22: no codegen term, measured ~0"
        );
        assert!(lines[5].starts_with("# bench/baseline-riscv64.txt:"));
        // Every line of the header is a comment: `--check` parses the file by skipping lines whose
        // first non-space character is `#`, and a reason is free prose that would otherwise parse
        // as a benchmark row named after its first word.
        assert!(h.lines().all(|l| l.trim_start().starts_with('#')));
    }

    /// What `--save` writes is what `--check` reads back, including a reason whose prose would
    /// parse as a benchmark row if the `#` were ever dropped.
    #[test]
    fn a_saved_file_reads_its_own_reasons_back() {
        let mut file = baseline_header(
            "baseline-aarch64.txt",
            "nightly-2026-09-22",
            "11.1.1",
            "2026-09-23",
            &["spawn_el0 1292192 100 is the switch cost the grant added".to_string()],
        );
        file.push_str("yield_switch 1134718 2000\n");
        assert_eq!(
            super::recorded_reasons(&file),
            vec!["spawn_el0 1292192 100 is the switch cost the grant added"]
        );
        // The check path skips every line whose first non-space character is `#`, so exactly one
        // line of this file is data. A reason that lost its prefix would be read as a floor.
        let rows: Vec<&str> = file
            .lines()
            .filter(|l| !l.trim_start().starts_with('#'))
            .collect();
        assert_eq!(rows, vec!["yield_switch 1134718 2000"]);
    }

    /// No reasons at all still produces a well-formed header, because `--check` has to read the
    /// three floors saved before this milestone and they carry none.
    #[test]
    fn header_without_reasons_has_no_why_line() {
        let h = baseline_header(
            "baseline-aarch64.txt",
            "unrecorded",
            "unrecorded",
            "unrecorded",
            &[],
        );
        assert!(!h.contains("# why:"));
        assert!(h.lines().nth(3).unwrap().starts_with("# bench/"));
    }

    /// A panicking bench kernel fails the run instead of hanging it. The kernel halts in `wfi`
    /// after a panic and QEMU never exits, so without the stop the reader would block forever, and
    /// `TickWindow`'s check (a tick in a timed window) would hang CI instead of failing it. The
    /// lines after the panic stand in for a pipe that never closes: the reader must not reach them.
    #[test]
    fn a_kernel_panic_ends_the_run_without_done() {
        let out = "bench: cntfrq 10000000\n\
                   bench: yield_switch 183330 2000\n\
                   [PANIC] panicked at kernel/src/bench/timing.rs:1:1:\n\
                   1 preemption tick(s) landed in ipc_rtt's timed window\n\
                   bench: ipc_rtt 175709 1000\n\
                   bench: done\n";
        let (results, cntfrq, done) = super::read_bench_lines(out.as_bytes());
        assert!(!done);
        assert_eq!(cntfrq, 10_000_000);
        assert_eq!(results, vec![("yield_switch".to_string(), 183_330, 2000)]);
    }
}
