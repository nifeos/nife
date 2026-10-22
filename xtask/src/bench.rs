//! The `cargo xtask bench` command: the cross-OS measurements, and the check that compares a
//! run against the recorded baselines.

use std::process::Command;
use std::sync::atomic::Ordering;

use crate::archive::{initial_ramdisk_path, initial_ramdisk_riscv, riscv_initial_ramdisk_path};
use crate::disk::{disk_path, mkdisk, mkredoxfs, redoxfs_server_build};
use crate::host::{flag_value, kernel_elf, run, workspace_root};
use crate::{RELEASE, RISCV_TARGET, RUNNER, TARGET, X86_TARGET, cargo_profiled, user};

/// The microbenchmarks (milestone 21; design/roadmap/0021-benchmarks.md).
///
/// Two instruments:
/// - default: TCG with `-icount`, where virtual time is a deterministic function of instructions
///   executed. Counts are exact and reproducible; `--check` diffs them against
///   `bench/baseline-aarch64.txt` and fails on drift, `--save --why "<reason>"` rewrites the
///   baseline (a deliberate act, committed alongside whatever changed the numbers, and refused
///   without a reason to record beside them: milestone 302).
/// - `--real`: HVF, natively on the host core. Real caches and TLBs, statistical numbers,
///   reported in nanoseconds, never gating.
///
/// The bench kernel never exits on its own (semihosting does not work under HVF; see `test`).
/// We own the QEMU child, watch its output for `bench: done`, and kill it: one exit mechanism
/// for both accelerators.
pub(crate) fn bench() -> bool {
    let check = std::env::args().any(|a| a == "--check");
    let save = std::env::args().any(|a| a == "--save");
    // `--release` builds an optimized kernel and userspace, for a fair cross-OS comparison (the debug
    // default is fine for the icount gate, whose counts are path length, but not for magnitudes next
    // to release Linux). Release changes instruction counts, so it never runs under icount and never
    // gates: it implies `--real` (HVF magnitudes only).
    let release = std::env::args().any(|a| a == "--release");
    RELEASE.store(release, Ordering::Relaxed);
    let real = release || std::env::args().any(|a| a == "--real");
    if real && (check || save) {
        let why = if release { "--release" } else { "--real" };
        eprintln!("bench: {why} numbers are statistical and never gate; no --check/--save");
        return false;
    }

    // **A save that cannot say why it happened is refused, before anything is built.**
    // milestone 302 (a baseline records what it was saved against), calef's ruling of 2026-09-16;
    // milestone 415 (sub-tripwire drift accumulates across baseline saves) item 2 words the same
    // requirement as "make a save record its own attribution, beside the number". A baseline row records a number and
    // nothing about why it holds that value, so a reader cannot tell compiler drift from a real
    // regression, and that is not hypothetical: on 2026-09-22 a lane read riscv64's `rfence_self`
    // as a stale floor carrying 8% of dead margin, on 2026-09-23 another measured it back at 5991
    // to the tick, and notes/benchmarks.md shows the row oscillating between 5991 and 6476 across
    // seven consecutive saves under two pins. Two lanes, opposite conclusions, four days apart,
    // because the file could not tell either of them anything.
    //
    // **Refusing is the top of AGENTS.md's ladder that this boundary can reach.** A reason cannot
    // be made unrepresentable by a type here (the command line is strings), but it can be made
    // impossible to omit, and that is one rung above a gate somebody has to remember to write. The
    // cost is honest and is the reason §190 (must an icount baseline save
    // record why it moved) hesitated: it changes what a person types on every
    // bench evening on every board. That is the trade taken.
    //
    // **Checked here rather than at the write** so the refusal costs a second instead of a kernel
    // build and a full emulator run. `run_bench` reads the same flag again at the write.
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

    // E3 (milestone 134, design/roadmap/0134-the-measurements-that-decide.md): build with an extra
    // kernel feature alongside `bench`, so the padded-fastpath experiment can be measured with the
    // same harness as everything else rather than a one-off. `--real`-only for the same reason
    // `--release` is: there is no cache under TCG for a footprint change to perturb, so a gated
    // run with this set would just measure instruction-count noise and call it a finding.
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
    // The second architecture. RISC-V has its own path (its own kernel target, runner, and initial
    // ramdisk, no disk, no HVF); everything else -- the icount instrument, the parsing, the table,
    // the baseline gate -- is shared through run_bench. See bench_riscv.
    if std::env::args().any(|a| a == "--riscv") {
        return bench_riscv(check, save, features);
    }

    // The third architecture (milestone 161; DECISIONS §121's amendment, the TSS I/O-bitmap
    // switch cost, extended by milestone 161's icount leg, 2026-08-25). **Not** the same question
    // as `icount()` below, which still refuses `--arch x86_64` for a real reason (milestone 78's
    // instrument needs a re-armed deadline timer to compare against, and this port's LAPIC timer
    // is periodic hardware reload with no such deadline to read). Pinning QEMU's virtual clock to
    // the instruction stream is a strictly weaker ask than that, and it works: measured, not
    // assumed, three consecutive boots under `-icount shift=0,sleep=off` on `q35` produced
    // byte-identical tick counts on every bench line, including the PIT-calibrated TSC frequency
    // itself. So `--x86` defaults to that instrument now, exactly like the other two ISAs, and
    // gates the same way; `--real` keeps the plain-TCG statistical path notes/benchmarks.md's
    // 2026-08-24 section already used, and the `real`+`check`/`save` refusal above already
    // covers `--x86 --real --check`.
    if std::env::args().any(|a| a == "--x86") {
        return bench_x86(real, check, save, features);
    }

    // `--smp`: boot the full 4-hart machine under HVF so the multi-hart throughput bench
    // (`smp_throughput`, DECISIONS §28) and the FS service-path bench (`fs_read`, DECISIONS §32) have
    // cores and, for the FS one, a filesystem to work with. Both self-skip on one hart, so without
    // this flag the `--real` run is single-hart and neither builds the FS image nor prints their
    // lines. Only meaningful with `--real`.
    let smp = std::env::args().any(|a| a == "--smp");

    // For --smp, build the FS server (before user(), so initial_ramdisk_aarch64 packs the
    // redoxfs_server ELF) and the RedoxFS test image the runner attaches as the second mmio disk.
    // The fs_read bench opens it; on any run without the image the bench finds no second disk and
    // skips, so this stays out of the icount gate's build entirely.
    if (smp && !redoxfs_server_build(TARGET))
        // `fs_walk`, from milestone 606 (a directory walk costs what it does on Linux), runs
        // `std_exerciser`, which only reaches the archive if it was built first. It builds this worktree's own std farm (notes/std.md), as a gate does.
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

    // Run the kernel through the same runner script as everything else, with the accelerator
    // chosen by env and, for the deterministic instrument, icount pinning virtual time to the
    // instruction stream (sleep=off: virtual time never waits for the wall clock).
    let mut cmd = Command::new(RUNNER);
    cmd.arg(kernel_elf());
    if real {
        cmd.env("NIFE_ACCEL", "hvf");
        if smp {
            // The full machine, for the aggregate-throughput bench. The per-core primitive magnitudes
            // in this same run are then NOT per-core clean (the reap-heavy ones, spawn_el0 and
            // spawn_reap, inflate and go noisy under cross-core reap lag); read those from the default
            // single-hart run instead. See notes/benchmarks/per-core-and-multi-hart.md.
            // "4" matches the runner's default; the throughput bench reads
            // the actual online count at runtime, so this only needs to be more than one.
            cmd.env("NIFE_SMP", "4");
            eprintln!(
                "--- bench: HVF, 4 harts (for smp_throughput; primitives are not per-core here) ---"
            );
        } else {
            // One hart by default, the same choice the icount instrument makes and for a kindred
            // reason: a primitive magnitude is a PER-CORE number, and the cross-OS comparison
            // (notes/benchmarks.md) reads it as one. At `-smp 4` the reap-heavy primitives pick up
            // cross-core reap lag that has nothing to do with per-core cost (spawn_el0 ~4.8 us here
            // goes ~13.6 us and swings wildly there; spawn_reap likewise). So the default `--real`
            // run is single-hart and clean; `--real --smp` boots the whole machine for the throughput
            // bench, which needs more than one core to mean anything.
            cmd.env("NIFE_SMP", "1");
            eprintln!(
                "--- bench: HVF, single hart, per-core magnitudes (statistical; medians matter) ---"
            );
        }
    } else {
        cmd.env_remove("NIFE_ACCEL");
        cmd.args(["-icount", "shift=0,sleep=off"]);
        // One hart, the same reason the riscv path forces it (bench_riscv): a primitive benchmark
        // measures per-core path length, and the counter it reads (CNTVCT) advances with QEMU's
        // GLOBAL virtual time. Under `-icount` all vCPUs share that one clock, and an idle secondary
        // hart sitting in `wfi` jumps virtual time to the next timer tick, so with `-smp 4` the
        // measured window counts three other harts' idle jumps and load-balanced spawns, not the
        // path under test. That contamination (not any code change) is what made the counts swing
        // wildly and non-physically across today's merges: coremark, pure compute, moved 63%. See
        // notes/benchmarks/icount-drift-and-provenance.md, the 2026-07-28 attribution. The aarch64
        // default is 4 (SMP tests);
        // the icount bench pins 1 to match riscv and measure the primitive, not the machine.
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

/// **The RISC-V benchmark path** (parity E's follow-up). Same primitive suite, same deterministic
/// icount instrument, on the second architecture, so the tick counts are directly comparable to the
/// aarch64 ones: both are the virtual timer advancing under `-icount`, which is instruction-clocked,
/// not wall-clock. No HVF (there is no RISC-V hypervisor on this host) and no disk (the bench boot
/// runs no virtio); it just needs the riscv initial ramdisk carrying `os_primitives_benchmarker` +
/// `coremark`. Its baseline is a separate file, since the counts differ by ISA. `cargo xtask bench --riscv [--check|--save]`.
fn bench_riscv(check: bool, save: bool, features: &str) -> bool {
    if !initial_ramdisk_riscv()
        || !run(
            "cargo",
            &[
                "build",
                "-p",
                "kernel",
                // `features` and not "bench": it carries `--extra-features` too (E3's
                // `fastpath_pad`, milestone 134). This arm hardcoded "bench" until 2026-09-19, so
                // `--riscv --real --extra-features fastpath_pad` built an UN-padded kernel and
                // printed numbers for it, which is the silently-wrong shape this tree fears most.
                // riscv64 is the ISA radon runs, so it is the arm the flag mattered on.
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
    // icount pins virtual time (rdtime) to the instruction stream; sleep=off so it never waits on the
    // wall clock. This is what makes the riscv counts deterministic and comparable to aarch64's.
    cmd.args(["-icount", "shift=0,sleep=off"]);
    cmd.env("NIFE_INITIAL_RAMDISK", riscv_initial_ramdisk_path());
    // No disk, as this function's doc says, and that has to be said to the runner rather than
    // assumed: `host::cargo` exports `NIFE_DISK` for every child, and the riscv64 runner treats a set
    // `NIFE_DISK` naming a missing file as fatal. Inherited, it made `--riscv` fail on any checkout
    // where no aarch64 leg had run `mkdisk` first, and everywhere else it attached five disks this
    // boot never reads. Removing it moves rows by at most 0.05% (measured 2026-10-02 at main
    // 40ac2d5f0: `yield_switch` +96 ticks, the one-quantum step that
    // notes/benchmarks/drift-since-the-2026-09-26-floors.md shows between unrelated commits).
    // `boot_check.rs` removes it for the same reason.
    cmd.env_remove("NIFE_DISK");
    // One hart: a primitive benchmark measures per-core cost. With more harts, a thread that waits
    // for a spawned child leaves its hart idling in `wfi`, and under `-icount` a `wfi` jumps virtual
    // time to the next timer tick, inflating the spawn primitives to timer-quantized nonsense. The
    // single-core costs are what compare to aarch64 anyway.
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

/// **The `x86_64` benchmark path** (DECISIONS §121's amendment, milestone 161 item 4; the icount
/// leg, milestone 161, 2026-08-25). Same suite, minus everything that needs a real userspace ELF:
/// `crates/user_mode_runtime` has no `x86_64` arms yet, so every `_el0` bench self-skips (`crate::
/// user::program` finds nothing in the initial ramdisk this leg never builds), and it adds one
/// x86-only bench, `tss_iomap_switch`: `bench::yield_switch` with a full
/// I/O-permission-bitmap-sized write added on every switch-in. Reading its `ns/iter` against
/// `yield_switch`'s from the same boot is §121 (what a device capability is when the device has no
/// page)'s missing number, the dominant cost of option 1 (a port-range capability enforced by the
/// TSS bitmap) that the decision names as unmeasured. See `kernel/src/arch/x86_64/segments.rs`'s
/// `bench_write_io_bitmap`.
///
/// **Two instruments, the same split `bench()` makes for aarch64**: default is TCG + `-icount
/// shift=0,sleep=off`, gated against `bench/baseline-x86_64.txt`; `--real` is plain TCG (no
/// KVM/HVF on this ARM host to accelerate `x86_64`), statistical, never gating, the shape
/// notes/benchmarks/x86-tss-iomap.md's 2026-08-24 section already used for the `tss_iomap_switch`
/// measurement
/// before this leg existed.
///
/// **This is not `icount()`'s instrument** (see the `--x86` branch in `bench()` above): that one
/// still refuses `--arch x86_64`, because milestone 78's claims compare an interrupt's arrival
/// against a deadline the kernel re-armed, and this port's LAPIC timer is a periodic hardware
/// reload with no such deadline to read. Pinning the virtual clock for a `timed()`-style duration
/// measurement needs none of that: `now()` already dispatches to `rdtsc`
/// (`kernel/src/arch/x86_64/timer.rs`), and rdtsc tracks icount's virtual clock the same way
/// `CNTVCT_EL0` and riscv64's `rdtime` do. **Measured, not assumed**: three consecutive `--x86`
/// boots under `-icount shift=0,sleep=off` produced byte-identical tick counts on every bench
/// line, including the PIT-calibrated TSC frequency itself (`bench: cntfrq 999935600` on all
/// three), so `run_bench`'s existing tick-count machinery needed no x86-specific change.
///
/// `helpers/qemu-runner-x86_64.sh` attaches no disk and builds no initial ramdisk, so this needs
/// neither `mkdisk` nor `user()`. `cargo xtask bench --x86 [--real] [--check|--save]`.
fn bench_x86(real: bool, check: bool, save: bool, features: &str) -> bool {
    if !run(
        "cargo",
        &[
            "build",
            "-p",
            "kernel",
            // As in `bench_riscv`: `features` carries `--extra-features`, which this arm also
            // dropped on the floor. `fastpath_pad` does not build on x86_64 today
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
    // **One core, pinned here rather than inherited from the runner**, which is what `bench()` and
    // `bench_riscv` have always done and what this arm alone forgot. The reason is theirs: a
    // primitive benchmark measures per-core path length, and both instruments below already print
    // "single hart" because that is what this was for.
    //
    // It was inherited and not pinned until 2026-09-23, when milestone 315 (a port revoke that
    // reaches every core) moved `helpers/qemu-runner-x86_64.sh`'s default to 2 and five counters
    // went outside the tripwire without a line of benchmarked code changing. Measured rather than
    // argued (notes/benchmarks/x86-instruments.md, the 2026-09-23 section): at `NIFE_SMP=1` this
    // branch is within
    // 2.3% of the baseline on every row, and at `NIFE_SMP=2` `spawn_reap` is 7.4x it, because
    // `bench::spawn_reap`'s parent busy-yields until the reaper runs and the child is now on the
    // other core: 4,205 spin-yields over 64 iterations against 22.
    //
    // **Two cores is not merely a different number here, it is a different instrument.** The counts
    // stay deterministic (three runs byte-identical) but stop being a stable function of the code:
    // moving one `#[cfg]`-gated call inside a lock swung `tss_iomap_lazy_switch` 89%, and adding two
    // `fetch_add`s for a probe moved `spawn_reap` 42% the other way. A 10% tripwire over that
    // measures the interleaving, not the kernel. Gating the multi-core numbers on their own terms
    // is a proposal rather than a number a lane may mint:
    // design/roadmap/proposals/two-core-bench-is-a-different-instrument.md.
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
    // Kill any QEMU the runner itself spawned before killing the runner: on `q35`
    // (`helpers/qemu-runner-x86_64.sh`) `cmd` is a *wrapper* that runs `qemu-system-x86_64` as a
    // plain foreground child rather than `exec`-ing into it (the runner's own header explains why:
    // it has to translate `isa-debug-exit`'s odd-only exit status). `child.kill()` therefore only
    // ever reaches the wrapper on that leg, and killing the wrapper first orphans the emulator
    // rather than ending it, which under `-icount sleep=off` is not an idle leak: a halted guest
    // whose virtual clock never waits on the host spins a full core forever instead of parking in
    // `hlt`. The other two runners `exec` (their own PID already *is* QEMU's), so `pkill -P` finds
    // nothing there and this is a no-op. Best-effort and silent either way: a runner that already
    // exited leaves no children to find. The rule the bound enforces is
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
        // **Diagnostics pass straight through, and deliberately do not become rows.** A probe
        // reports a count or a bitmask, not a duration, so putting it in the table would invite
        // `--check` to police an exact value with a 10% tolerance and would print a meaningless
        // ns/iter beside it. Echoed instead, so a reader of a bench run sees it and the baseline
        // never grows a line that is not a measurement. See `bench::map_new`'s shootdown probe.
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
    // The header names the file it is in. It used to be the literal `bench/baseline.txt` for
    // both baselines, so the riscv one claimed to be the aarch64 one; deriving it from the path
    // makes the two agree with themselves (milestone 73).
    let stem = baseline_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("baseline.txt");
    // **The nightly these counts were produced by, written into the file that holds them.**
    // icount counts guest instructions, and a different compiler emits a different instruction
    // sequence for the same source, so the toolchain is part of what these numbers mean in
    // exactly the way `.qemu-version` already is. It went unrecorded until 2026-09-21, and the
    // cost was live: `nightly-2026-09-15` was pinned without re-recording, the tripwire's
    // headroom eroded for reasons no change was responsible for, and the tree was bumped again
    // before anyone acted. `script/lint`'s baseline-toolchain check compares this line against
    // `rust-toolchain.toml`, so the two cannot disagree silently. Written here, it is the
    // nightly that produced the counts; `--restamp` may later move it to a nightly it proved
    // produces the same counts within bounds (calef, 2026-09-26), so a reader of the file takes
    // it as "last proven valid for" and the `# why:` lines say which.
    //
    // Read from the PIN rather than from the compiler that happens to be running: the pin is
    // what CI and every other machine build with, and a `RUSTUP_TOOLCHAIN` override in one
    // shell is not a fact about the repository. A save under such an override therefore writes
    // the pin, which is a lie only if the override was deliberate and then committed without
    // raising the pin, and that is the case `script/lint` catches from the other side.
    let pinned = pinned_nightly().unwrap_or_else(|| "unrecorded".into());
    // **The emulator that counted these instructions, read from the binary rather than from
    // the pin.** See `running_qemu` for why this one is asked and the toolchain one is not.
    // When the machine disagrees with `.qemu-version` the line says both, because that
    // disagreement is a fact about the numbers below and not a thing to tidy away: it is how
    // this project spent a month comparing counts from an emulator nobody had recorded.
    let ran = running_qemu(qemu_binary).unwrap_or_else(|| "unrecorded".into());
    let qemu = match pinned_qemu() {
        Some(pin) if pin != ran => format!("{ran} (run; .qemu-version pins {pin})"),
        _ => ran,
    };
    // **The third field of the ratified line, and the ledger itself.** calef ratified the
    // format on 2026-09-16 as comment lines rather than a structured block, written as
    // `# saved: <nightly>, qemu <version>, <date>` plus a `# why:` line. Commit `99f13dad`
    // implemented the first of those decomposed into `# toolchain:` and `# qemu:`, one fact per
    // line, and `script/lint` now reads `# toolchain:`. So this keeps that decomposition and
    // adds the two parts it left out, rather than reuniting the line and breaking the gate:
    // `# date:` is the ratified line's third field and `# why:` is the ledger.
    //
    // **One `# why:` line per `--why`, in the header rather than above the row that moved.**
    // Milestone 415 item 2 words it as "beside the rows that moved", which is finer, and the
    // ratified format is the header, which is what this follows. It is also the only one that
    // survives: the save path rebuilds this file from a fixed header plus one `name ticks
    // iters` line per result, so a per-row comment is destroyed by the next save, which is
    // exactly what happened to the hand-written correction above `rfence_self` on 2026-09-21.
    // A reason that concerns one counter names it in its own prose.
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

    // `trim_start` matters: this file's own header has INDENTED comment lines, and a
    // column-0-only check would treat them as data. A three-word one would then be parsed as a
    // benchmark named after its first word. Filtering after `trim_start` drops every comment
    // line whatever its indent, so the destructure below only ever sees `name ticks iters` rows.
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
        // **What the floor says about itself, printed where the reader is already looking.**
        // The reasons are in the file, but a person reading a red gate in CI scrollback is not
        // reading the file, and the question they are holding is exactly the one these lines
        // answer: is this number a cost somebody chose, or drift nobody has explained? Empty
        // on the three floors saved before milestone 302, which say `unrecorded` for the same
        // reason the `# qemu:` line does.
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
    // **Is this the emulator that produced the floors?** An icount count is a function of two
    // things, the compiler that emitted the instructions and the emulator that counted them,
    // and until 2026-09-21 this tree recorded neither. The compiler half is answered
    // statically by `script/lint`, because the pin is in the repository. This half cannot be:
    // nothing resolves QEMU from `.qemu-version`, so the only moment the question can be asked
    // is the moment an emulator is actually being run, which is here.
    //
    // **Compared against the emulator this run used, NOT against `.qemu-version`.** A check
    // against the pin would fail any baseline honestly recorded off-pin, which is to say it
    // would forbid the file from stating the truth; and it would still pass a comparison run
    // on a third version. The apples-to-apples question is whether the counter that produced
    // the floor is the counter reading it now.
    //
    // **`unrecorded` is a truthful answer and does not fail**, the posture
    // milestone 115 (the names that were refused) already takes for names. Every baseline in
    // the tree carries it today, because the emulator that produced those counts was never
    // written down and inventing one now would be worse than the gap. So this fires at full
    // strength from the first honest `--save` onward and never on a number nobody stamped.
    //
    // **What it means when it does fire**, and it is not "upgrade something": a re-record and
    // the pin have to be decided together, since CI builds the pinned emulator
    // (`script/ci-qemu`) and would then read these floors on it.
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
