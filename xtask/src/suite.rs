//! The `cargo xtask test` command: the host tests in milliseconds, then each architecture's
//! kernel under QEMU, and the Miri run beside them. See notes/xtask-test.md.

//! # EXAMPLES
//!
//! ```text
//! $ script/test --arch aarch64 --test frames_are_zeroed
//! --- test filter: frames_are_zeroed (kernel legs only; the host crates have `cargo test`) ---
//! --- kernel tests, aarch64 (QEMU 11.1.2) ---
//! running 1 of 312 tests (filter: frames_are_zeroed)
//! test kernel::memory::tests::frames_are_zeroed ... ok
//! test result: ok. 1 passed
//! ```

use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::archive::{
    initial_ramdisk_path, initial_ramdisk_riscv, initial_ramdisk_x86, riscv_initial_ramdisk_path,
    x86_initial_ramdisk_path,
};
use crate::disk::{
    disk_path, mkblankdisk, mkdisk, mkgptdisk, mknvmedisk, mkredoxfs, mkredoxfs_crash,
    nvme_disk_path, redoxfs_server_build,
};
use crate::disk_check::{
    blank_check_after_run, redoxfs_check_after_run, redoxfs_crash_check_after_run,
};
use crate::farm::std_exerciser;
use crate::host::{
    cargo, cargo_test_counting_selected, flag_value, run, runner_env, selected_by, workspace_root,
};
use crate::inbound::InboundProber;
use crate::scanout::{
    HostLoad, ScanoutReferee, cargo_test_with_scanout_check, cargo_test_with_scanout_only,
};
use crate::uefi::{uefi_boot, uefi_test};
use crate::{RELEASE, RISCV_TARGET, RUNNER, TARGET, X86_TARGET, time_record, user};

/// The architecture legs `test` should run: all three by default, the ones `--arch` names otherwise.
///
/// A set rather than an enum, because CI's suite runs as two jobs and one of them runs two legs
/// (`--arch aarch64,riscv64`). An enum needs a variant per combination; three flags have no default
/// arm for a leg to fall into, so each predicate is explicit. No `--arch` means every leg, so the
/// parity gate (§19) cannot be weakened by forgetting a flag. The constants keep the four values
/// every other caller names. See notes/xtask-test.md.
#[derive(Clone, Copy, PartialEq)]
pub(crate) struct ArchLegs {
    aarch64: bool,
    riscv64: bool,
    x86_64: bool,
}

impl ArchLegs {
    pub(crate) const ALL: ArchLegs = ArchLegs {
        aarch64: true,
        riscv64: true,
        x86_64: true,
    };
    pub(crate) const AARCH64: ArchLegs = ArchLegs {
        aarch64: true,
        riscv64: false,
        x86_64: false,
    };
    pub(crate) const RISCV64: ArchLegs = ArchLegs {
        aarch64: false,
        riscv64: true,
        x86_64: false,
    };
    pub(crate) const X86_64: ArchLegs = ArchLegs {
        aarch64: false,
        riscv64: false,
        x86_64: true,
    };

    /// `--arch`'s value as `test` reads it: one architecture or a comma-separated list of them.
    /// A name that is not an architecture, or an empty entry, is an error naming it, so a typo
    /// cannot quietly drop a leg. Name: provisional (lane maintainer/split-test-job, 2026-10-12 UTC).
    pub(crate) fn parse_list(value: &str) -> Result<ArchLegs, String> {
        let mut legs = ArchLegs {
            aarch64: false,
            riscv64: false,
            x86_64: false,
        };
        for name in value.split(',') {
            match name {
                "aarch64" => legs.aarch64 = true,
                "riscv64" => legs.riscv64 = true,
                "x86_64" => legs.x86_64 = true,
                other => {
                    return Err(format!(
                        "--arch {other:?} is not an architecture (aarch64, riscv64 or x86_64, \
                         or a comma-separated list of them)"
                    ));
                }
            }
        }
        Ok(legs)
    }

    /// Name: provisional, flagged 2026-09-24 by the boolean-predicate pass
    /// (design/naming/functions-that-answer-yes-or-no-worklist.md). It does not yet follow the Rust predicate
    /// rule calef ratified 2026-09-24; recommended `includes_aarch64`, because `is_aarch64` would
    /// be false for `ALL`, which also answers yes.
    pub(crate) fn aarch64(self) -> bool {
        self.aarch64
    }
    /// Name: provisional, flagged 2026-09-24 by the boolean-predicate pass
    /// (design/naming/functions-that-answer-yes-or-no-worklist.md). It does not yet follow the Rust predicate
    /// rule calef ratified 2026-09-24; recommended `includes_riscv64`, because `is_riscv64` would
    /// be false for `ALL`, which also answers yes.
    pub(crate) fn riscv64(self) -> bool {
        self.riscv64
    }
    /// Name: provisional, flagged 2026-09-24 by the boolean-predicate pass
    /// (design/naming/functions-that-answer-yes-or-no-worklist.md). It does not yet follow the Rust predicate
    /// rule calef ratified 2026-09-24; recommended `includes_x86_64`, because `is_x86_64` would be
    /// false for `ALL`, which also answers yes.
    pub(crate) fn x86_64(self) -> bool {
        self.x86_64
    }
}

/// Host tests first, then each architecture's kernel under QEMU, the boot tour on a screen and the
/// post-run image checks; true when all passed. The host crates fail in milliseconds with no
/// emulator, so they go first (DECISIONS §7 (testing: QEMU harness + host-testable crates)). Flags:
/// `--arch aarch64|riscv64|x86_64[,...]` runs the named legs in the suite's order; `--no-host` skips
/// the host pass; `--cpu <model>` sets `NIFE_CPU`; `--hvf` runs the aarch64 leg on the physical core
/// ([`hvf_kernel_leg`], notes/hvf-leg.md) and refuses `--cpu`; `--test <substring>` runs matching
/// kernel tests on every leg (§19), skips the host pass, the image checks and the referees'
/// verdicts, and fails when nothing matches; `--repeat <N>` repeats that selection in one boot.
/// Callers: `script/cpu_matrix` (`--arch`, `--cpu`; notes/cpu-models.md) and `script/ci-build`
/// (`--hvf`, an `--arch` list, `--no-host`). See notes/xtask-test.md for each flag's reasons.
pub(crate) fn test() -> bool {
    let Some((hvf, legs)) = legs_from_flags() else {
        return false;
    };
    // The suite's userspace carries the confinement tests' attacker roles; no other boot's does.
    // Set before any leg builds its archive, and for the whole run, so a `--test` falsification
    // replay gets them too. See `CONFINEMENT_ATTACKERS`.
    crate::CONFINEMENT_ATTACKERS.store(true, std::sync::atomic::Ordering::Relaxed);
    if !export_cpu_model(hvf) {
        return false;
    }
    let filter = export_test_filter();
    if !export_test_repeat(filter.as_deref()) {
        return false;
    }

    // A plain run starts its own time record file (milestone 807 (the kernel suite reports what
    // each test cost)); a caller that set
    // `NIFE_TIME_RECORD` owns its file and gets rows appended. See `time_record::start`.
    time_record::start();

    // Nothing cargo starts inherits an accelerator choice. The default leg is TCG, which is the
    // right place for reproducible tests (deterministic, identical on any host), and the HVF leg
    // does not go through cargo at all: it sets `NIFE_ACCEL` on the one child that needs it
    // (see `hvf_kernel_leg`), so a stale value from the caller's shell cannot reach anything else.
    // SAFETY: `set_var`/`remove_var` became unsafe in edition 2024 because they race other
    // threads. xtask is single-threaded here: this runs on the main thread before the child
    // that reads it is spawned, and the only thread xtask ever starts (the transcript reader
    // in swish_check_leg) copies pipe bytes into a String and never touches the environment.
    unsafe { std::env::remove_var("NIFE_ACCEL") };
    if !host_pass(hvf, filter.is_some()) {
        return false;
    }
    if !build_suite_programs(legs) {
        return false;
    }
    attach_test_leg_devices();

    if legs.aarch64() && !aarch64_suite(hvf, filter.is_some()) {
        return false;
    }
    // The same booted suite on riscv64's Sv39 kernel (parity workstream B), exiting through the
    // SiFive test finisher. What stays aarch64-only genuinely needs aarch64: the userspace-exec
    // suite's hand-written machine code, and SMP. See notes/riscv-parity-scope.md.
    if legs.riscv64() && !riscv64_suite(filter.is_some()) {
        return false;
    }
    // The third architecture (milestone 161 (the x86_64 kernel port), roadmap item 4), exiting
    // through `isa-debug-exit`.
    // It runs last and sets its own archive, disk and NIC, since the earlier legs leave theirs in
    // the environment. notes/xtask-test.md has what it builds and attaches.
    if legs.x86_64() && !x86_64_suite(filter.is_some()) {
        return false;
    }
    // The boot tour on a `ramfb` for the two boards, milestone 243 (a machine with no serial port):
    // the only check here that fails when the screen is black. A boot of its own, because `ramfb`
    // adds a QEMU console and the suite's machine has a virtio-gpu on console 0. Not under `--cpu`,
    // a cost decision: nothing on the screen path varies with `-cpu`, and the leg still runs on
    // every ordinary `script/test`. See notes/serial-less-output/ramfb.md.
    if flag_value("--cpu").is_none() {
        if legs.aarch64() && !crate::screen::screen_boot("aarch64") {
            return false;
        }
        if legs.riscv64() && !crate::screen::screen_boot("riscv64") {
            return false;
        }
    }
    images_after_the_run(legs, filter.is_some())
}

/// `--hvf` and `--arch`, read together because the first constrains the second: which legs this
/// run boots, and whether the aarch64 one runs on the physical core. `None` after saying why the
/// two cannot go together.
fn legs_from_flags() -> Option<(bool, ArchLegs)> {
    // Milestone 81. Read before `--arch`, because it constrains it: Hypervisor.framework runs the
    // host's own ISA and this host is aarch64, so there is no riscv64 leg to accelerate and asking
    // for one is a mistake worth naming rather than ignoring.
    let hvf = std::env::args().any(|a| a == "--hvf");
    let legs = match flag_value("--arch").as_deref() {
        None if hvf => ArchLegs::AARCH64,
        None => ArchLegs::ALL,
        Some(list) => match ArchLegs::parse_list(list) {
            Ok(legs) => legs,
            Err(why) => {
                eprintln!("test: {why}");
                return None;
            }
        },
    };
    if hvf && legs.riscv64() {
        eprintln!(
            "test: --hvf is aarch64 only (Hypervisor.framework runs this host's own ISA; riscv64 \
             has no equivalent until the board lands)"
        );
        return None;
    }
    if hvf && legs.x86_64() {
        eprintln!(
            "test: --hvf is aarch64 only (Hypervisor.framework runs this host's own ISA, and \
             this host is aarch64)"
        );
        return None;
    }
    Some((hvf, legs))
}

/// `--cpu`, onto `NIFE_CPU` for the runners, or the variable cleared when no flag was given.
/// False after refusing `--cpu` under `--hvf`.
fn export_cpu_model(hvf: bool) -> bool {
    if hvf && flag_value("--cpu").is_some() {
        eprintln!(
            "test: --cpu cannot apply under --hvf (the guest runs the physical core; -cpu host is \
             mandatory)"
        );
        return false;
    }
    // The CPU model rides to the runners in the environment rather than on the QEMU command line,
    // because cargo owns that command line: the runner is invoked by cargo, and the only channel we
    // have to it is env. Unset it when no flag was given so a stale value from the caller's shell
    // cannot silently change what a plain `script/test` means.
    match flag_value("--cpu") {
        Some(model) => {
            eprintln!("--- CPU model: {model} (NIFE_CPU) ---");
            // SAFETY: `set_var`/`remove_var` became unsafe in edition 2024 because they race other
            // threads. xtask is single-threaded here: this runs on the main thread before the child
            // that reads it is spawned, and the only thread xtask ever starts (the transcript reader
            // in swish_check_leg) copies pipe bytes into a String and never touches the environment.
            unsafe { std::env::set_var("NIFE_CPU", model) };
        }
        // SAFETY: `set_var`/`remove_var` became unsafe in edition 2024 because they race other
        // threads. xtask is single-threaded here: this runs on the main thread before the child
        // that reads it is spawned, and the only thread xtask ever starts (the transcript reader
        // in swish_check_leg) copies pipe bytes into a String and never touches the environment.
        None => unsafe { std::env::remove_var("NIFE_CPU") },
    }
    true
}

/// `--repeat <N>`, onto `NIFE_TEST_REPEAT` for the kernel's build, or the variable cleared when
/// no flag was given. False after refusing a count that is not a whole number of at least 1, or a
/// repeat with no `--test` (the kernel refuses that too, but only after a whole build and boot).
///
/// It runs the filtered tests that many times in one boot (`testing::TEST_REPEAT` has the
/// semantics), so a failure seen once in many boots gets a frequency from one bench visit
/// (milestone 321 (a corpse that did not park, on four real cores)). Flag and variable names are
/// provisional.
pub(crate) fn export_test_repeat(filter: Option<&str>) -> bool {
    let Some(count) = flag_value("--repeat") else {
        // SAFETY: `remove_var` became unsafe in edition 2024 because it races other threads.
        // xtask is single-threaded here: this runs on the main thread before any child is spawned.
        unsafe { std::env::remove_var("NIFE_TEST_REPEAT") };
        return true;
    };
    if !count.parse::<u32>().is_ok_and(|n| n >= 1) {
        eprintln!("test: --repeat takes a whole number of at least 1, not {count:?}");
        return false;
    }
    if filter.is_none() {
        eprintln!(
            "test: --repeat runs named tests, not the suite (the suite keeps services up for the \
             boot, so a second pass measures them rather than the tests); add --test <name>"
        );
        return false;
    }
    eprintln!(
        "--- repeat: each selected test runs {count} times in one boot (NIFE_TEST_REPEAT) ---"
    );
    // SAFETY: as above.
    unsafe { std::env::set_var("NIFE_TEST_REPEAT", count) };
    true
}

/// `--test`, onto `NIFE_TEST_FILTER` for the kernel's build, or the variables cleared when no flag
/// was given. Returns the filter.
pub(crate) fn export_test_filter() -> Option<String> {
    // `--test <substring>` (milestone 210) rides the environment into `kernel/build.rs`, which
    // bakes it into the test binary as a `rustc-env`: a booted kernel has no command line. Unset
    // without the flag, so a stale shell value cannot change what a plain `script/test` means. It
    // does not narrow the architectures (DECISIONS §19). Timings in notes/xtask-test.md.
    let filter = flag_value("--test");
    match filter.as_deref() {
        Some(f) => {
            eprintln!(
                "--- test filter: {f} (kernel legs only; the host crates have `cargo test`) ---"
            );
            // SAFETY: `set_var`/`remove_var` became unsafe in edition 2024 because they race other
            // threads. xtask is single-threaded here: this runs on the main thread before the child
            // that reads it is spawned, and the only thread xtask ever starts (the transcript reader
            // in swish_check_leg) copies pipe bytes into a String and never touches the environment.
            unsafe { std::env::set_var("NIFE_TEST_FILTER", f) };
            // Each leg boots two test images now (milestone 609 (the system tests leave the
            // kernel crate)), and the filter may match in only one. This tells the kernel's runner
            // that an image with no match is not the verdict; `filtered_leg` adds the legs up.
            // SAFETY: as above.
            unsafe { std::env::set_var("NIFE_TEST_FILTER_ACROSS_IMAGES", "1") };
        }
        // SAFETY: as above.
        None => unsafe {
            std::env::remove_var("NIFE_TEST_FILTER");
            std::env::remove_var("NIFE_TEST_FILTER_ACROSS_IMAGES");
        },
    }
    filter
}

/// **The host pass**: every host crate's tests, the vendored RedoxFS round trip and `no_std` build,
/// the FS server's core and the TLS graph's host tests. Skipped, and said so, under `--hvf` and
/// `--no-host`, and skipped under `--test`.
fn host_pass(hvf: bool, filtered: bool) -> bool {
    if hvf {
        eprintln!(
            "--- host tests, the vendored redoxfs round trip and the redoxfs_server core: SKIPPED under \
             --hvf ---"
        );
        eprintln!(
            "    They are host code on the host; no accelerator exists on that path, so running \
             them again would cost ~30 s and prove nothing the TCG leg has not. What --hvf re-runs \
             is the part an accelerator can change: the kernel, under QEMU."
        );
    }
    // `!filtered` because `--test` names a KERNEL test: the host crates already have
    // `cargo test <name>`, which is what milestone 210 exists to give the kernel, and running the
    // whole host pass (about 72 s here) to reach one kernel test would keep most of the cost the
    // flag is meant to remove.
    //
    // `--no-host` says so in the log, like `--hvf` does, so a transcript cannot be read as one
    // that ran the host pass.
    let no_host = std::env::args().any(|a| a == "--no-host");
    if no_host && !hvf && !filtered {
        eprintln!(
            "--- host tests, the vendored redoxfs round trip and the TLS graph's host tests: SKIPPED \
             under --no-host ---"
        );
    }
    if !hvf && !no_host && !filtered {
        let began = std::time::Instant::now();
        let ok = host_tests();
        time_record::host_pass(began.elapsed());
        ok
    } else {
        true
    }
}

/// The host-excluded crates as `--exclude <crate>` pairs for a `--workspace` selection, derived by
/// `helpers/host-excluded-crates.sh` rather than listed. Panics when the helper cannot run or fails.
///
/// Every member that reaches `crates/user_mode_runtime` cannot compile for the host (`svc` and
/// `ecall` there are EL0 syscalls), and the whole set must go: `--exclude` removes a package from
/// the test selection, not from the dependency graph. One derivation serves script/lint's two
/// clippy passes, script/coverage, script/mutation and this file's host and Miri passes.
/// script/lint's "the host pass excludes exactly the bare-metal crates" gate fails if this file
/// stops calling it or the derivation disagrees with `cargo metadata`. See notes/xtask-test.md.
fn host_excluded_args() -> Vec<String> {
    let output = Command::new("sh")
        .arg("helpers/host-excluded-crates.sh")
        .current_dir(workspace_root())
        .output()
        .unwrap_or_else(|e| {
            panic!(
                "host tests: cannot run helpers/host-excluded-crates.sh to derive the \
                    host-excluded set: {e}"
            )
        });
    if !output.status.success() {
        panic!(
            "host tests: helpers/host-excluded-crates.sh failed, and a wrong or empty set would \
             send the bare-metal crates into the host pass: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let mut args = Vec::new();
    for name in String::from_utf8_lossy(&output.stdout).split_whitespace() {
        args.push("--exclude".to_string());
        args.push(name.to_string());
    }
    args
}

/// The host pass itself ([`host_pass`] says when it runs).
fn host_tests() -> bool {
    eprintln!("--- host tests (pure logic, no emulator) ---");
    // Every host crate, by asking cargo rather than keeping a list ([`host_excluded_args`]).
    // `kernel` and `system_tests` stay out by judgment: they are bare metal through `kernel`,
    // which the dependency walk does not see.
    let mut args: Vec<String> = ["test", "--workspace"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    args.extend(host_excluded_args());
    args.extend(
        ["--exclude", "kernel", "--exclude", "system_tests"]
            .iter()
            .map(|s| s.to_string()),
    );
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    if !cargo(&args) {
        return false;
    }

    // The vendored RedoxFS pin (vendor/redoxfs, milestone 32), both halves of vendor/README.md's
    // promise, by --manifest-path because the engine and the host tool are their own workspaces,
    // outside our clippy/fmt gates. The host tool's round trip (mkfs, put, ls, cat) runs against
    // the pinned engine; the engine's no_std core is built for both bare-metal targets below,
    // because upstream does not CI the no_std path.
    eprintln!();
    eprintln!("--- vendored redoxfs: host round trip + no_std core (both targets) ---");
    if !run(
        "cargo",
        &["test", "--manifest-path", "tools/redoxfs_host/Cargo.toml"],
    ) {
        return false;
    }
    // The atlas generator's regenerate-and-compare gate (milestone 142 (a text display good enough
    // that people use it instead of a GUI)): the coverage tables the terminal draws with are
    // exactly what the pinned rasteriser makes from vendor/dejavu, on this host. Its own workspace,
    // so the `--workspace` pass above never reached it.
    eprintln!();
    eprintln!("--- glyph_atlas_generator: the checked-in atlas regenerates byte for byte ---");
    if !run(
        "cargo",
        &[
            "test",
            "--locked",
            "--manifest-path",
            "tools/glyph_atlas_generator/Cargo.toml",
        ],
    ) {
        return false;
    }
    // The FS server's sans-IO core (redoxfs_server, its own workspace): open, read, write, close against
    // a real RedoxFS image in memory, in milliseconds. This proves the filesystem logic for BOTH the
    // read and write paths on the host, which the on-device test can only do for reads today.
    eprintln!();
    eprintln!("--- redoxfs_server sans-IO core (host, its own workspace) ---");
    if !run(
        "cargo",
        &["test", "--manifest-path", "redoxfs_server/Cargo.toml"],
    ) {
        return false;
    }
    // The TLS graph's host tests (milestone 855 (the TLS graph enters the gated build)): the
    // provider's own checks, and the pinned client against `helpers/tls-peer`, which is Python's
    // `ssl` and so OpenSSL, the same peer the guest meets. Each is its own workspace, so the
    // `--workspace` pass above never reached them. `--locked` because the graph a gate builds
    // is the one the lockfile names. The client's one test that needs the internet stays
    // `#[ignore]`d.
    eprintln!();
    eprintln!("--- the TLS graph: cryptography_provider and pinned_tls_client (host) ---");
    for manifest in [
        "cryptography_provider/Cargo.toml",
        "pinned_tls_client/Cargo.toml",
    ] {
        if !run("cargo", &["test", "--locked", "--manifest-path", manifest]) {
            return false;
        }
    }
    for target in [TARGET, RISCV_TARGET] {
        if !run(
            "cargo",
            &[
                "build",
                "--manifest-path",
                "vendor/redoxfs/Cargo.toml",
                "--no-default-features",
                "--target",
                target,
            ],
        ) {
            return false;
        }
    }
    true
}

/// **The programs every leg's archive packs that the suite builds first**: `std_exerciser` and the
/// nifefs data disk, then the TLS graph's programs for the legs this run boots.
fn build_suite_programs(legs: ArchLegs) -> bool {
    // Build the std demo (milestone 27 (Rust `std` on the native ABI)) for every custom target
    // first, so every initial ramdisk carries it: initial_ramdisk_aarch64 (inside `user`) packs the
    // aarch64 std_exerciser, initial_ramdisk_riscv packs the riscv one. Outside the leg guards
    // below because BOTH legs need it, and the nifefs data disk with it: it is arch-neutral, and
    // the riscv leg reads it whether or not the aarch64 leg ran.
    if !std_exerciser() || !mkdisk() {
        return false;
    }
    // The TLS graph's programs, for the legs this run boots (milestone 855 (the TLS graph enters
    // the gated build)): `cryptography_exerciser`, `pinned_tls_exerciser` and milestone 801
    // (packages over the internet)'s `package_fetch_exerciser`. Built here rather than in a `script/ci-build` row, as `rg` is, so
    // that `script/test` and CI's kernel legs mean the same suite and a build that breaks fails
    // this gate instead of turning three tests into skips. After `std_exerciser`, whose `std-src`
    // the helpers repeat for nothing.
    let mut tls_triples = Vec::new();
    if legs.aarch64() {
        tls_triples.push("aarch64-unknown-nife");
    }
    if legs.riscv64() {
        tls_triples.push("riscv64-unknown-nife");
    }
    if legs.x86_64() {
        tls_triples.push("x86_64-unknown-nife");
    }
    crate::farm::tls_graph(&tls_triples)
}

/// The test-leg devices: a virtio-gpu, a virtio keyboard, two virtio-rngs and an NVMe controller,
/// set in this process's environment for every leg's runner to read.
fn attach_test_leg_devices() {
    // Attach a virtio-gpu for the display test (milestone 29). Set here, in `test`, rather than in
    // `cargo()`: the benchmark boot uses the same runner and adding a device to it would change what
    // the icount instrument measures, so the GPU is a test-leg device only. Both ISA legs get it,
    // because parity is the gate (§19), and the display test ASSERTS the device is present rather
    // than skipping, so a leg that lost this line fails loudly.
    // SAFETY: `set_var`/`remove_var` became unsafe in edition 2024 because they race other
    // threads. xtask is single-threaded here: this runs on the main thread before the child
    // that reads it is spawned, and the only thread xtask ever starts (the transcript reader
    // in swish_check_leg) copies pipe bytes into a String and never touches the environment.
    unsafe { std::env::set_var("NIFE_GPU", "1") };
    // And a virtio keyboard (milestone 29's input), on the same terms and for the same reason: a
    // test-leg device only, on both ISA legs, and the keyboard test ASSERTS one is present rather
    // than skipping, so a leg that lost this line fails loudly instead of quietly proving nothing.
    // SAFETY: `set_var`/`remove_var` became unsafe in edition 2024 because they race other
    // threads. xtask is single-threaded here: this runs on the main thread before the child
    // that reads it is spawned, and the only thread xtask ever starts (the transcript reader
    // in swish_check_leg) copies pipe bytes into a String and never touches the environment.
    unsafe { std::env::set_var("NIFE_KEYBOARD", "1") };
    // And two virtio-rng devices, one per transport (milestone 56, the entropy half), on the same
    // terms again: a test-leg device only, both ISA legs, and the entropy tests ASSERT a device on
    // each bus rather than skipping. Out of the benchmark boot for the same reason as the GPU: it
    // shares the runner, and a device the instrument did not measure last time is drift.
    // SAFETY: `set_var`/`remove_var` became unsafe in edition 2024 because they race other
    // threads. xtask is single-threaded here: this runs on the main thread before the child
    // that reads it is spawned, and the only thread xtask ever starts (the transcript reader
    // in swish_check_leg) copies pipe bytes into a String and never touches the environment.
    unsafe { std::env::set_var("NIFE_RNG", "1") };
    // And an NVMe controller (milestone 53's storage half), on the same terms: a test-leg device
    // only (the benchmark boot shares the runner and must not grow devices its instrument never
    // measured), on both ISA legs because parity is the gate (§19), and the NVMe test ASSERTS the
    // controller is present rather than skipping. The variable carries the image path; each leg
    // regenerates the image below, beside the other write-target disks.
    // SAFETY: `set_var`/`remove_var` became unsafe in edition 2024 because they race other
    // threads. xtask is single-threaded here: this runs on the main thread before the child
    // that reads it is spawned, and the only thread xtask ever starts (the transcript reader
    // in swish_check_leg) copies pipe bytes into a String and never touches the environment.
    unsafe { std::env::set_var("NIFE_NVME", nvme_disk_path()) };
}

/// **The aarch64 leg**: its images, then both test images, on the physical core under `--hvf`.
fn aarch64_suite(hvf: bool, filtered: bool) -> bool {
    eprintln!();
    // The version rides in the banner (milestone 414 (a red post-run check does not say which
    // emulator produced it)): a red line under this heading is interpreted by which emulator
    // produced it, so the heading carries that fact.
    eprintln!(
        "--- kernel tests, aarch64 (QEMU {}) ---",
        crate::host::qemu_version("aarch64")
    );
    // The FS server (milestone 32 phase 2), for the aarch64 bare target, before `user()` so
    // initial_ramdisk_aarch64 packs it; then the RedoxFS test images the runner attaches as
    // extra mmio disks.
    if !redoxfs_server_build(TARGET)
        || !user()
        || !mkredoxfs()
        || !mkredoxfs_crash()
        || !mkgptdisk()
        || !mkblankdisk()
        || !mknvmedisk()
    {
        return false;
    }
    // `cargo()` only exports the env the runner needs; the test itself runs under the scanout
    // check, which drives QEMU's monitor beside the suite and proves the pixels reached the
    // device's scanout rather than only the driver's frames.
    if !cargo(&["build", "-p", "kernel", "--target", TARGET]) {
        return false;
    }
    // **Two images per leg** (milestone 609 (the system tests leave the kernel crate)): the
    // kernel's own unit tests, then the whole-system suite, which is the kernel linked as a
    // library under `system_tests/`. The scanout and inbound checks watch the second, because
    // the display, compositor and network tests that paint and listen moved with it.
    if hvf {
        hvf_both_images(filtered)
    } else if filtered {
        filtered_leg("aarch64", TARGET)
    } else {
        // The environment `cargo()` sets, since this child is spawned by the recorder.
        runner_env();
        time_record::cargo_test(
            &time_record::leg("aarch64"),
            "kernel",
            &["test", "-p", "kernel", "--target", TARGET],
        ) && cargo_test_with_scanout_check(
            "aarch64",
            &["test", "-p", "system_tests", "--target", TARGET],
        )
    }
}

/// **The riscv64 leg**: its own archive and fresh images, then both test images.
fn riscv64_suite(filtered: bool) -> bool {
    eprintln!();
    eprintln!(
        "--- kernel tests, riscv64 (QEMU {}) ---",
        crate::host::qemu_version("riscv64")
    );
    // The riscv userspace tests (parity C) load programs from the initial ramdisk and read the
    // disk, so build the riscv archive and point the runner at IT, not at the aarch64 archive
    // `cargo()` exports: the riscv ELF loader must never be handed aarch64 ELFs. The disk is
    // arch-neutral (a nifefs data image) and was built by mkdisk() above.
    // The riscv FS server, before the riscv archive that packs it.
    if !redoxfs_server_build(RISCV_TARGET) || !initial_ramdisk_riscv() {
        return false;
    }
    // Fresh images for this leg: the aarch64 leg writes the shared image, and reusing it would
    // order-couple the legs. The cross-boot write failure this separates out is a tracked item in
    // notes/fs-server.md, with its recipe.
    if !mkredoxfs() || !mkredoxfs_crash() || !mkgptdisk() || !mkblankdisk() || !mknvmedisk() {
        return false;
    }
    // SAFETY: `set_var`/`remove_var` became unsafe in edition 2024 because they race other
    // threads. xtask is single-threaded here: this runs on the main thread before the child
    // that reads it is spawned, and the only thread xtask ever starts (the transcript reader
    // in swish_check_leg) copies pipe bytes into a String and never touches the environment.
    unsafe { std::env::set_var("NIFE_INITIAL_RAMDISK", riscv_initial_ramdisk_path()) };
    // SAFETY: `set_var`/`remove_var` became unsafe in edition 2024 because they race other
    // threads. xtask is single-threaded here: this runs on the main thread before the child
    // that reads it is spawned, and the only thread xtask ever starts (the transcript reader
    // in swish_check_leg) copies pipe bytes into a String and never touches the environment.
    unsafe { std::env::set_var("NIFE_DISK", disk_path()) };
    // SAFETY: `set_var`/`remove_var` became unsafe in edition 2024 because they race other
    // threads. xtask is single-threaded here: this runs on the main thread before the child
    // that reads it is spawned, and the only thread xtask ever starts (the transcript reader
    // in swish_check_leg) copies pipe bytes into a String and never touches the environment.
    unsafe { std::env::set_var("NIFE_NET", "1") }; // a virtio-net NIC for the net test (m30)
    // The kernel's unit tests, then the system suite under the scanout check (milestone 609 (the
    // system tests leave the kernel crate)); the aarch64 leg says why the check is on the second.
    if filtered {
        if !filtered_leg("riscv64", RISCV_TARGET) {
            return false;
        }
    } else if !time_record::cargo_test(
        &time_record::leg("riscv64"),
        "kernel",
        &["test", "-p", "kernel", "--target", RISCV_TARGET],
    ) || !cargo_test_with_scanout_check(
        "riscv64",
        &["test", "-p", "system_tests", "--target", RISCV_TARGET],
    ) {
        return false;
    }
    true
}

/// **The x86_64 leg**: its archive and images, both test images, and then the same kernel on three
/// other machines: a PCIe root port, AMD-Vi, and real firmware.
fn x86_64_suite(filtered: bool) -> bool {
    eprintln!();
    eprintln!(
        "--- kernel tests, x86_64 (QEMU q35, {}) ---",
        crate::host::qemu_version("x86_64")
    );
    // The FS server before the archive that packs it (milestone 164 (x86_64 userspace can't build
    // `aes`, and therefore `fs_server`)), as on the other legs. The
    // runner attaches five fixtures as virtio-blk-pci functions (milestones 215, 303 and 420), so
    // this leg builds all five, regenerating every image the earlier legs wrote so it is not
    // order-coupled to them.
    if !redoxfs_server_build(X86_TARGET)
        || !initial_ramdisk_x86()
        || !mkdisk()
        || !mkredoxfs()
        || !mkredoxfs_crash()
        || !mkgptdisk()
        || !mkblankdisk()
        || !mknvmedisk()
    {
        return false;
    }
    // SAFETY: `set_var` became unsafe in edition 2024 because it races other threads. xtask is
    // single-threaded here: this runs on the main thread before the child that reads it is
    // spawned, and the only thread xtask ever starts (the transcript reader in swish_check_leg)
    // copies pipe bytes into a String and never touches the environment.
    unsafe { std::env::set_var("NIFE_INITIAL_RAMDISK", x86_initial_ramdisk_path()) };
    // **`NIFE_DISK` names the fixture set, not one disk**, exactly as it does on both other
    // runners. This one derives the `-pci.img`, `-redoxfs.img`, `-redoxfs-crash.img`, `-gpt.img`
    // and `-blank.img` siblings from it and attaches those as the first through fifth
    // virtio-blk-pci functions (milestones 215, 303 and 420); `q35` has no virtio-mmio bus, so
    // the image the variable itself names is not attached anywhere here.
    //
    // SAFETY: `set_var` became unsafe in edition 2024 because it races other threads. xtask is
    // single-threaded here: this runs on the main thread before the child that reads it is
    // spawned, and the only thread xtask ever starts (the transcript reader in
    // swish_check_leg) copies pipe bytes into a String and never touches the environment.
    unsafe { std::env::set_var("NIFE_DISK", disk_path()) };
    // The e1000e NIC (milestone 494 (a driver for the network card a PC actually has)), set here
    // because `attach_test_leg_devices` does not export NIFE_NET: an `--arch x86_64` run would boot
    // NIC-less where a full run inherits the riscv64 leg's value. One machine per run.
    //
    // SAFETY: `set_var` became unsafe in edition 2024 because it races other threads. xtask is
    // single-threaded here: this runs on the main thread before the child that reads it is
    // spawned, and the only thread xtask ever starts (the transcript reader in
    // swish_check_leg) copies pipe bytes into a String and never touches the environment.
    unsafe { std::env::set_var("NIFE_NET", "1") };
    // **This leg's runner owes all five block fixtures, and the system suite is told so**
    // (milestone 420). The runner attaches the crash, GPT and blank disks softly because a bench or
    // a boot-check has none, so a runner that lost those lines boots a smaller machine and every
    // test that wants them skips green. `system_tests/build.rs` bakes this in and the roster test
    // fails by name when it is set and a fixture is missing. Name provisional. `uefi_test` unsets
    // it: the OVMF runner attaches the nifefs disk alone.
    //
    // SAFETY: `set_var` became unsafe in edition 2024 because it races other threads. xtask is
    // single-threaded here: this runs on the main thread before the child that reads it is
    // spawned, and the only thread xtask ever starts (the transcript reader in
    // swish_check_leg) copies pipe bytes into a String and never touches the environment.
    unsafe { std::env::set_var("NIFE_X86_FIXTURES_REQUIRED", "1") };
    // The kernel's unit tests, then the system suite (milestone 609 (the system tests leave the
    // kernel crate)).
    if filtered {
        if !filtered_leg("x86_64", X86_TARGET) {
            return false;
        }
    } else if !time_record::cargo_test(
        &time_record::leg("x86_64"),
        "kernel",
        &["test", "-p", "kernel", "--target", X86_TARGET],
    ) || !cargo_test_with_scanout_only(
        "x86_64",
        &["test", "-p", "system_tests", "--target", X86_TARGET],
    ) {
        return false;
    }
    if !filtered && !x86_64_behind_a_root_port() {
        return false;
    }
    if !filtered && std::env::var_os("NIFE_IOMMU").is_none() && !x86_64_on_amd_vi() {
        return false;
    }
    // **And the same kernel started by real firmware** (milestone 87). The suite above rides
    // QEMU's PVH loader, which is a hypervisor protocol no machine speaks; this boots the same
    // code through OVMF from `\EFI\BOOT\BOOTX64.EFI`, which is what the Dell OptiPlex does.
    // It is the tour rather than the suite, and that is a cost decision stated where it is
    // paid: the tour is ten seconds and covers the whole boot path, where re-running 200 tests
    // under a second firmware buys coverage of the tests rather than of the firmware.
    if !uefi_boot() {
        return false;
    }
    // **And the suite itself under that firmware** (milestone 195). The line above boots the
    // tour, which is the shipping image; this boots the test binary, which is the same kernel
    // with `test_main()` on the end of the same tour. It costs a second firmware boot and buys
    // the claim the tour cannot make: that the 200 tests pass on a memory map, an ACPI root and
    // a PCIe window that came from firmware.
    if !uefi_test() {
        return false;
    }
    true
}

/// **One more boot, with the NVMe controller behind a `pcie-root-port`** (milestone 320 (every PCI
/// bus the machine has, not just bus zero)), the topology xenon's M.2 slot has. `q35` is flat, so a
/// kernel that enumerates only bus 0 passes there. Returns the test's verdict.
///
/// One test, `found_on_the_bus_behind_it`, not the suite: the claim needing a second topology is
/// that the walk follows a bridge, and re-running the suite would cover the tests rather than the
/// topology. About three seconds. Skipped under `--test`, whose filter this would clobber, running
/// something the caller did not ask for. See notes/xtask-test.md.
fn x86_64_behind_a_root_port() -> bool {
    eprintln!();
    eprintln!("--- kernel test, x86_64 with the NVMe behind a PCIe root port ---");
    // SAFETY: `set_var` became unsafe in edition 2024 because it races other threads. xtask
    // is single-threaded here: this runs on the main thread before the child that reads it
    // is spawned, and the only thread xtask ever starts (the transcript reader in
    // swish_check_leg) copies pipe bytes into a String and never touches the environment.
    unsafe {
        std::env::set_var("NIFE_PCIE_ROOT_PORT", "1");
        std::env::set_var("NIFE_TEST_FILTER", "found_on_the_bus_behind_it");
    }
    let bridged = time_record::cargo_test(
        "x86_64 root-port",
        "kernel",
        &["test", "-p", "kernel", "--target", X86_TARGET],
    );
    // Removed whether or not it passed: the UEFI boots below and every later leg must get
    // the flat machine they were written against.
    //
    // SAFETY: as above. Single-threaded, on the main thread, and the child that read these
    // has already exited.
    unsafe {
        std::env::remove_var("NIFE_PCIE_ROOT_PORT");
        std::env::remove_var("NIFE_TEST_FILTER");
    }
    bridged
}

/// **The whole suite again, on a machine whose IOMMU is AMD-Vi** (lane `amd-vi`, provisional).
/// `q35` with `-device amd-iommu` writes an IVRS and no DMAR, so the kernel brings up
/// `arch/x86_64/amd_vi.rs` and every confinement test (the virtio and NVMe DMA escapes among them)
/// runs against the other unit. The whole suite, because DECISIONS §19 (architectural parity is a
/// tenet) has the same suite prove a capability on every machine it ships on; one PVH boot per
/// image. Returns the verdict. Skipped under `--test`; a caller wanting one test here sets
/// `NIFE_IOMMU=amd` for the run, as a falsification record's `Environment:` line does.
fn x86_64_on_amd_vi() -> bool {
    eprintln!();
    eprintln!("--- kernel and system tests, x86_64 with AMD-Vi in place of VT-d ---");
    // Every fixture the runner attaches, for the freshness rule the leg above states: this
    // machine's suite writes the blank disk (the write-half test runs here too), so starting
    // from the VT-d leg's damage would be order-coupled.
    if !mkdisk()
        || !mkredoxfs()
        || !mkredoxfs_crash()
        || !mkgptdisk()
        || !mkblankdisk()
        || !mknvmedisk()
    {
        return false;
    }
    // SAFETY: `set_var` became unsafe in edition 2024 because it races other threads. xtask
    // is single-threaded here: this runs on the main thread before the child that reads it
    // is spawned, and the only thread xtask ever starts (the transcript reader in
    // swish_check_leg) copies pipe bytes into a String and never touches the environment.
    unsafe { std::env::set_var("NIFE_IOMMU", "amd") };
    let amd = time_record::cargo_test(
        &time_record::leg("x86_64"),
        "kernel",
        &["test", "-p", "kernel", "--target", X86_TARGET],
    ) && cargo_test_with_scanout_only(
        "x86_64",
        &["test", "-p", "system_tests", "--target", X86_TARGET],
    );
    // Removed whether or not it passed, so every later boot gets the VT-d machine.
    //
    // SAFETY: as above. Single-threaded, on the main thread, and the child that read it
    // has already exited.
    unsafe { std::env::remove_var("NIFE_IOMMU") };
    amd
}

/// FS-level consistency after the runs (milestone 32 (a real filesystem: RedoxFS behind a
/// capability FS server) phase 2): reopen the images with the host tool and confirm the FS server's
/// writes persisted and the filesystems still parse. The images are the last leg's: x86_64 in a
/// full run, riscv64 when `--arch x86_64` was not asked for, aarch64 when it was the only leg. No
/// crash test runs on x86_64 (the twins in `filesystem_tests` and `riscv_virtio_tests` are cfg-gated
/// to their ISAs), so after an x86_64 leg the crash check is skipped: a check whose subject did not
/// happen is worse than no check. See notes/xtask-test.md.
fn images_after_the_run(legs: ArchLegs, filtered: bool) -> bool {
    // **Not under a filter** (milestone 210 (no kernel test can be run by name, so one
    // falsification costs a whole suite)), for the same reason the `--arch x86_64` guard
    // exists: these checks assert what the FS tests WROTE, so a run that did not select them
    // would open a stale image and report "motd did not read back", which is a true statement
    // about a leftover file and a false one about this run.
    if filtered {
        return true;
    }
    eprintln!();
    eprintln!("--- redoxfs image consistency after the run (host tool) ---");
    // The shared fixture check runs either way: every leg's FS tests write it.
    if legs.x86_64() {
        return redoxfs_check_after_run() && blank_check_after_run();
    }
    // Both images: the shared fixture (the write persisted, the filesystem still parses), the
    // crash test's own disk (milestone 37 (prove RedoxFS's crash consistency): after a kill
    // mid-transaction, `cut` is one payload whole), and milestone 57
    // (partitioning and formatting a real drive, and extended attributes)'s blank disk, where the
    // guest wrote both the partition table and the filesystem inside it.
    redoxfs_check_after_run() && redoxfs_crash_check_after_run() && blank_check_after_run()
}

/// Both test images on the physical core, and under `--test` the selection counted across them
/// (milestone 609 (the system tests leave the kernel crate)); [`filtered_leg`] says why.
fn hvf_both_images(filtered: bool) -> bool {
    let (kernel_ok, kernel_selected) = hvf_kernel_leg("kernel", false);
    if !kernel_ok {
        return false;
    }
    let (system_ok, system_selected) = hvf_kernel_leg("system_tests", true);
    if !system_ok {
        return false;
    }
    if filtered && kernel_selected + system_selected == 0 {
        eprintln!("test --hvf: no test in either image matches the filter");
        return false;
    }
    true
}

/// One leg under `--test`: both test images, each run with its transcript counted, and the leg
/// failed if either failed or if together they selected nothing (milestone 609 (the system tests
/// leave the kernel crate)). The kernel's runner keeps "a filter that selects nothing fails" for a
/// bare `cargo test`; here the rule moves up to the leg, because the test the filter names lives in
/// exactly one of the two images. The scanout and inbound checks are skipped, as they were already
/// advisory under a filter: they assert on what particular tests draw and receive.
fn filtered_leg(arch: &str, target: &str) -> bool {
    let mut selected = 0;
    for package in ["kernel", "system_tests"] {
        // **The system image runs with the referee beside it, verdict dropped.** The referee is also
        // what presses the key the keyboard test waits for (`sendkey` over the monitor), so a filter
        // that selects that test failed it in ten seconds on every ISA for want of a finger. Its
        // scanout verdict is advisory under a filter (the tests that paint may not be selected), so
        // it is never reported; only the key matters here.
        let referee = (package == "system_tests").then(|| ScanoutReferee::new(arch));
        let (running, watcher) = watch_the_scanout(referee);
        let (ok, n) = cargo_test_counting_selected(
            &["test", "-p", package, "--target", target],
            &format!("{} {package}", time_record::leg(arch)),
        );
        running.store(false, Ordering::Relaxed);
        let _ = watcher.join();
        if !ok {
            return false;
        }
        selected += n;
    }
    if selected == 0 {
        eprintln!(
            "test: no test on the {arch} leg matches the filter, in either image (a test only this \
             architecture lacks? `--test` runs every leg; add `--arch`)"
        );
        return false;
    }
    true
}

/// One test image on the physical Apple Silicon core (milestone 81 (an HVF leg: the test suite on
/// the physical core), `--hvf`). Returns the verdict and how many tests its harness selected;
/// `watch_devices` adds the scanout referee and inbound prober, which only the system suite needs.
/// QEMU does not answer ARM semihosting under HVF, and the harness reports through it, so the guest
/// prints its verdict and wedges. Like `run_bench`, this owns the QEMU child and reads the
/// transcript: `test result: ok.` passes, `[PANIC] ` or `WATCHDOG:` fails, and so does output
/// ending with neither. The guest's watchdogs still fire (the virtual timer is passed through and
/// QEMU injects the interrupt), so they bound the leg and there is no host deadline. The referee
/// runs on a thread, never touching the environment, because it presses the keys the keyboard test
/// waits for. See notes/hvf-leg.md and notes/hvf-leg/semihosting.md.
fn hvf_kernel_leg(package: &str, watch_devices: bool) -> (bool, usize) {
    let Some(elf) = kernel_test_elf(package, TARGET, "test --hvf") else {
        return (false, 0);
    };

    eprintln!();
    eprintln!(
        "--- {package} tests, aarch64, ON THE PHYSICAL CORE (QEMU + Hypervisor.framework) ---"
    );

    if !hvf_machine_starts() {
        return (false, 0);
    }

    // Constructed before the child, because it sets `NIFE_GPU_MON`, which the runner reads when it
    // builds the QEMU command line. Only the system suite paints and answers the network
    // (milestone 609 (the system tests leave the kernel crate)).
    let referee = watch_devices.then(|| ScanoutReferee::new("aarch64"));
    // Before the child too, since it sets `NIFE_HOSTFWD_PORT`. The accept test is in the suite, so
    // a leg without a prober would fail it.
    let mut prober = watch_devices.then(|| InboundProber::new("aarch64"));

    let mut cmd = Command::new(RUNNER);
    cmd.arg(&elf);
    // The one child that gets the accelerator. `test()` cleared it from our own environment, so
    // nothing else in this process can inherit it.
    cmd.env("NIFE_ACCEL", "hvf");
    // The same devices the TCG leg attaches, set by `test()` and `cargo()` in our environment and
    // inherited from there: the initial ramdisk, the disks, the NIC, the GPU, the keyboard, the
    // RNGs.
    cmd.env("NIFE_INITIAL_RAMDISK", initial_ramdisk_path());
    cmd.env("NIFE_DISK", disk_path());
    cmd.env("NIFE_NET", "1");
    cmd.stdout(std::process::Stdio::piped());

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("test --hvf: failed to start {RUNNER}: {e}");
            return (false, 0);
        }
    };

    let (running, watcher) = watch_the_scanout(referee);

    let stdout = child.stdout.take().expect("piped stdout");
    let (verdict, selected, load) = relay_until_verdict(stdout, prober.as_mut());

    // A panic ends the suite (no unwinding in the custom test framework), so a scanout or inbound
    // failure reported after it may only mean the guest never reached that test.
    if verdict == Some(false) {
        eprintln!(
            "test --hvf: the suite stopped at the failure above; a scanout or inbound failure \
             reported below may only mean the guest never reached that test, so fix the first \
             failure before reading them"
        );
    }

    // Stop and collect the referee BEFORE killing QEMU: its last look at the scanout has to happen
    // while there is still a device to look at.
    running.store(false, Ordering::Relaxed);
    let scanout_ok = match watcher.join() {
        Ok(Some(referee)) => referee.report(),
        Ok(None) => true,
        Err(_) => {
            eprintln!("test --hvf: the scanout referee panicked");
            false
        }
    };

    // Same ordering argument as the referee's: the prober has to stop while the guest is still
    // there, and its verdict is collected before QEMU is killed.
    let inbound_ok = prober.is_none_or(|prober| prober.report());

    // It is parked at a semihosting trap HVF will not answer, so it will never exit by itself.
    let _ = child.kill();
    let _ = child.wait();

    let ok = hvf_verdict(verdict, scanout_ok && inbound_ok);
    load.report_if_failed(ok, "aarch64 --hvf");
    (ok, selected)
}

/// **Whether QEMU will start the HVF machine at all**, asked of the runner before anything else
/// stands up. False after relaying the runner's own words for why not.
fn hvf_machine_starts() -> bool {
    // Ask whether QEMU will start this machine at all, BEFORE standing up the referee and the two
    // probers (milestone 222). If it will not, each of those reports its own failure about a QEMU
    // that never existed, and the transcript then carries four confident-sounding messages about
    // monitors and forwarded ports, none of which is the reason. The runner owns the question and
    // the words; this only decides when to ask. See helpers/qemu-runner-aarch64.sh.
    let probe = Command::new(RUNNER)
        .env("NIFE_PROBE", "1")
        .env("NIFE_ACCEL", "hvf")
        .output();
    match probe {
        Ok(out) if !out.status.success() => {
            eprint!("{}", String::from_utf8_lossy(&out.stdout));
            eprint!("{}", String::from_utf8_lossy(&out.stderr));
            eprintln!(
                "test --hvf: nothing ran. `script/ci-build` skips this leg and says so; only an \
                 explicit --hvf fails."
            );
            return false;
        }
        Ok(_) => {}
        Err(e) => {
            eprintln!("test --hvf: failed to start {RUNNER} for the machine probe: {e}");
            return false;
        }
    }
    true
}

/// **Drive the scanout referee from its own thread** until the flag this returns is cleared; the
/// join hands the referee back for its report.
fn watch_the_scanout(
    referee: Option<ScanoutReferee>,
) -> (
    std::sync::Arc<AtomicBool>,
    std::thread::JoinHandle<Option<ScanoutReferee>>,
) {
    // The referee, on its own thread, polling on the same 100 ms cadence the TCG leg uses. It stops
    // when `running` clears and hands itself back through the join, so the reporting happens on this
    // thread exactly as it does for TCG.
    let running = std::sync::Arc::new(AtomicBool::new(true));
    let watcher = {
        let running = running.clone();
        let mut referee = referee;
        std::thread::spawn(move || {
            while running.load(Ordering::Relaxed) {
                if let Some(referee) = referee.as_mut() {
                    referee.poll();
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            referee
        })
    };
    (running, watcher)
}

/// **Relay the leg's transcript a line at a time until it gives a verdict**: `Some(true)` at the
/// harness's `test result: ok.`, `Some(false)` at a panic or a watchdog (and then up to
/// `AFTER_FAILURE` more lines, for the dump), `None` when the output ended with neither. Also
/// returns how many tests the harness selected and the host-load samples taken meanwhile.
fn relay_until_verdict(
    stdout: std::process::ChildStdout,
    mut prober: Option<&mut InboundProber>,
) -> (Option<bool>, usize, HostLoad) {
    use std::io::BufRead;
    let reader = std::io::BufReader::new(stdout);
    let mut verdict: Option<bool> = None;
    let mut selected = 0;
    // Sampled from the transcript reader rather than from the referee thread, because this is the
    // loop that runs for the length of the leg and the sampler is rate-limited anyway. **This leg
    // needs it more than the TCG one does**: HVF runs the guest on the physical cores, so the
    // host's other work competes with it directly rather than through an interpreter.
    let mut load = HostLoad::new();
    // How much more transcript to relay once something has failed. The watchdogs print a thread
    // dump after the line that names the failure and that dump is the diagnosis, so we cannot stop
    // at the marker; but we cannot read to the end either, because **there is no end**. The
    // semihosting trap the failure path takes is not answered under HVF: it raises a real
    // synchronous exception (EC 0x00, "Unknown reason") into the guest's own vector table, whose
    // handler panics, whose panic handler takes the same trap again. Four cores doing that write
    // interleaved garbage at native speed forever. 200 lines is comfortably more than the longest
    // dump and stops well short of the storm.
    const AFTER_FAILURE: usize = 200;
    let mut budget = AFTER_FAILURE;
    for line in reader.lines() {
        let Ok(line) = line else { break };
        // Stream it, because a test suite you cannot watch is a test suite you cannot debug. The
        // TCG leg inherits stdio and prints as it goes; this leg has to relay.
        println!("{line}");
        // The listeners' own lines, onto the prober's clock (milestone 514 (the HVF std listener
        // hangs in some full runs)). Only this leg can, because only this leg reads the transcript
        // a line at a time as it happens.
        if let Some(prober) = prober.as_mut() {
            prober.mark(&line);
        }
        selected += selected_by(&line).unwrap_or(0);
        load.sample();
        if line.starts_with("test result: ok.") {
            verdict = Some(true);
            break;
        }
        if line.contains("[PANIC] ") || line.contains("WATCHDOG:") {
            verdict = Some(false);
        }
        if verdict == Some(false) {
            budget -= 1;
            if budget == 0 {
                break;
            }
        }
    }
    (verdict, selected, load)
}

/// The leg's verdict from the transcript's and the referees' (`watched`).
fn hvf_verdict(verdict: Option<bool>, watched: bool) -> bool {
    // A filtered run drops both verdicts, as `test`'s doc promises: the scanout and inbound
    // referees watch a guest that never ran their tests, and fail on the absence.
    let filtered = std::env::var_os("NIFE_TEST_FILTER").is_some_and(|f| !f.is_empty());
    match verdict {
        Some(true) => filtered || watched,
        Some(false) => {
            eprintln!();
            eprintln!(
                "test --hvf: the suite failed on the physical core (see the transcript above)"
            );
            false
        }
        None => {
            eprintln!();
            eprintln!(
                "test --hvf: QEMU's output ended without a verdict. The harness prints one before \
                 every exit, so this is QEMU dying rather than the suite finishing."
            );
            false
        }
    }
}

/// Ask cargo to build a kernel test image and say where it put it, without running it. `package` is
/// `kernel` for the kernel's own unit tests or `system_tests` for the whole-system suite (milestone
/// 609 (the system tests leave the kernel crate)); both are the kernel, booted to run tests.
///
/// `cargo test --no-run` is the build; `--message-format=json` is how we learn the path, which
/// carries a content hash and lives under the build script's `OUT_DIR`, so it cannot be spelled
/// out by hand. The scan is a substring match rather than a parse because xtask has no JSON
/// dependency and taking one for a single field would be the wrong trade (DECISIONS §46): the
/// field is a filesystem path emitted by cargo, so it contains no escapes, and the only artifact
/// line `cargo test --no-run -p kernel` emits with a non-null `executable` is the one we want.
pub(crate) fn kernel_test_elf(package: &str, target: &str, who: &str) -> Option<String> {
    let mut args = std::vec![
        "test",
        "-p",
        package,
        "--target",
        target,
        "--no-run",
        // Diagnostics still render as text on stderr; only the machine-readable artifact
        // records go to stdout. A compile error is as readable as it always was.
        "--message-format=json-render-diagnostics",
    ];
    if RELEASE.load(Ordering::Relaxed) {
        args.push("--release");
    }
    let out = Command::new("cargo")
        .args(&args)
        .stdout(std::process::Stdio::piped())
        .spawn()
        .and_then(|c| c.wait_with_output());
    let out = match out {
        Ok(o) if o.status.success() => o,
        Ok(_) => {
            eprintln!("{who}: building the kernel test binary failed");
            return None;
        }
        Err(e) => {
            eprintln!("{who}: cannot run cargo: {e}");
            return None;
        }
    };

    const KEY: &str = "\"executable\":\"";
    let found = String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|l| {
            l.contains("\"reason\":\"compiler-artifact\"")
                && l.contains(&format!("\"name\":\"{package}\""))
        })
        .filter_map(|l| l.split_once(KEY).map(|(_, rest)| rest.to_string()))
        .filter_map(|rest| rest.split_once('"').map(|(path, _)| path.to_string()))
        .next_back();

    if found.is_none() {
        eprintln!(
            "{who}: cargo built the kernel but named no test executable. That is a change in \
             cargo's JSON output, not a test failure."
        );
    }
    found
}

/// **The host tests again, under Miri's interpreter** (milestone 79 (Miri over the host crates)):
/// aliasing (tree borrows), provenance, uninitialized reads, leaks. Returns cargo's verdict. See
/// notes/undefined-behavior.md. The selection is `test()`'s, `--workspace` less
/// [`host_excluded_args`], plus four judgment exclusions with no `unsafe` Miri could execute:
/// `xtask` (safe pixel arithmetic, about seven minutes under Miri); `serial_console` (about 55
/// minutes, and ten tests reach the host filesystem or wall-clock timeouts); `stick_maker`
/// (isolation refuses `statx`, its `unsafe` is FFI; `measured_boot` stays in); `walk_pricing`
/// (`lstat` refused, a timing instrument; `filesystem_fixture` stays in). Extra args are forwarded
/// but do not narrow the run; check one crate with `cargo miri test -p <crate>`.
pub(crate) fn undefined_behavior_check() -> bool {
    eprintln!("--- host tests under Miri (aliasing, provenance, uninitialized reads) ---");
    let mut args: Vec<String> = ["miri", "test", "--workspace"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    args.extend(host_excluded_args());
    args.extend(
        ["--exclude", "kernel", "--exclude", "system_tests"]
            .iter()
            .map(|s| s.to_string()),
    );
    args.extend(
        [
            "--exclude",
            "xtask",
            "--exclude",
            "serial_console",
            "--exclude",
            "stick_maker",
            "--exclude",
            "walk_pricing",
        ]
        .iter()
        .map(|s| s.to_string()),
    );
    let mut args: Vec<&str> = args.iter().map(String::as_str).collect();
    let extra: Vec<String> = std::env::args().skip(2).collect();
    args.extend(extra.iter().map(String::as_str));
    run("cargo", &args)
}

#[cfg(test)]
mod tests {
    use super::ArchLegs;

    #[test]
    fn a_list_selects_exactly_the_legs_it_names() {
        let legs = ArchLegs::parse_list("aarch64,riscv64").unwrap();
        assert!(legs.aarch64() && legs.riscv64() && !legs.x86_64());
        assert!(ArchLegs::parse_list("x86_64").unwrap() == ArchLegs::X86_64);
    }

    #[test]
    fn a_misspelled_or_empty_entry_is_refused_rather_than_dropped() {
        assert!(ArchLegs::parse_list("aarch64,risc64").is_err());
        assert!(ArchLegs::parse_list("aarch64,").is_err());
    }
}
