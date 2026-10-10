//! The `cargo xtask test` command: the host tests in milliseconds, then each architecture's
//! kernel under QEMU, and the Miri run beside them.

use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::archive::{initrd_path, initrd_riscv, initrd_x86, riscv_initrd_path, x86_initrd_path};
use crate::disk::{
    disk_path, mkblankdisk, mkdisk, mkgptdisk, mknvmedisk, mkredoxfs, mkredoxfs_crash,
    nvme_disk_path, redoxfs_server_build,
};
use crate::disk_check::{
    blank_check_after_run, redoxfs_check_after_run, redoxfs_crash_check_after_run,
};
use crate::farm::std_exerciser;
use crate::host::{cargo, cargo_test_counting_selected, flag_value, run, runner_env, selected_by};
use crate::inbound::InboundProber;
use crate::scanout::{HostLoad, ScanoutReferee, cargo_test_with_scanout_check};
use crate::uefi::{uefi_boot, uefi_test};
use crate::{RELEASE, RISCV_TARGET, RUNNER, TARGET, X86_TARGET, time_record, user};

/// The architecture legs `test` should run: both by default, one when `--arch` names it.
///
/// **`--arch` did not exist before milestone 59**, and this is the correction worth stating: the
/// milestone brief said to follow how it "already threads through", and nothing in the tree parsed
/// it. `test` ran both ISA legs unconditionally, which is right for the parity gate (§19) and wrong
/// for a CPU-model matrix that wants the riscv64 leg four times over with a different `-cpu` each
/// time. So the flag is new here, and the default is unchanged: no `--arch` means both legs, and
/// the parity gate cannot be weakened by forgetting to pass something.
///
/// **It was `Both` and is now `All`** (milestone 161, roadmap item 4), because there are three
/// architectures. The rename is not cosmetic: the two predicates below were written as
/// `self != the_other_one`, which is correct for exactly two variants and answers `true` for every
/// leg the moment there is a third. That shape is the same default-arm trap `crates/elf`'s
/// `EXPECTED_MACHINE` fell into on the same day, so both are now explicit `matches!`.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum ArchLegs {
    All,
    Aarch64,
    Riscv64,
    X86_64,
}

impl ArchLegs {
    /// Name: provisional, flagged 2026-09-24 by the boolean-predicate pass
    /// (design/naming/boolean-predicates-worklist.md). It does not yet follow the Rust predicate
    /// rule calef ratified 2026-09-24; recommended `includes_aarch64`, because `is_aarch64` would
    /// be false for `All`, which also answers yes.
    pub(crate) fn aarch64(self) -> bool {
        matches!(self, ArchLegs::All | ArchLegs::Aarch64)
    }
    /// Name: provisional, flagged 2026-09-24 by the boolean-predicate pass
    /// (design/naming/boolean-predicates-worklist.md). It does not yet follow the Rust predicate
    /// rule calef ratified 2026-09-24; recommended `includes_riscv64`, because `is_riscv64` would
    /// be false for `All`, which also answers yes.
    pub(crate) fn riscv64(self) -> bool {
        matches!(self, ArchLegs::All | ArchLegs::Riscv64)
    }
    /// Name: provisional, flagged 2026-09-24 by the boolean-predicate pass
    /// (design/naming/boolean-predicates-worklist.md). It does not yet follow the Rust predicate
    /// rule calef ratified 2026-09-24; recommended `includes_x86_64`, because `is_x86_64` would be
    /// false for `All`, which also answers yes.
    pub(crate) fn x86_64(self) -> bool {
        matches!(self, ArchLegs::All | ArchLegs::X86_64)
    }
}

/// Host tests first, then the kernel under QEMU.
///
/// The host crates (`device_tree_blob`, `frames`) hold the pure logic and run in *milliseconds*
/// with no emulator, so they fail fast and cheap. Only once they pass is it worth spending twenty
/// seconds booting QEMU. See DECISIONS §7.
///
/// Four flags narrow what runs, and all four default to today's behaviour:
///
/// - `--arch aarch64|riscv64|x86_64` runs one ISA leg instead of all three (milestone 59; the
///   third arrived with milestone 161).
/// - `--cpu <model>` picks the emulated CPU model (`NIFE_CPU`, read by both QEMU runners).
///   Unset means `cortex-a72` on aarch64 and `rv64` on riscv64, exactly as before (milestone 59).
/// - `--hvf` runs the aarch64 kernel leg on the physical Apple Silicon core (milestone 81). It is
///   aarch64-only by construction, so it narrows the run to that leg and refuses `--cpu`; see
///   [`hvf_kernel_leg`] for the mechanism and notes/hvf-leg.md for what differs.
/// - `--test <substring>` runs only the kernel tests whose full path contains `<substring>`, the
///   same shape `cargo test <name>` has (milestone 210). It selects TESTS, not architectures: every
///   leg still runs, because a filter that quietly narrowed to one ISA is what DECISIONS §19
///   distrusts. It skips the host pass (those crates already have `cargo test`), skips the post-run
///   image checks, and drops the host-side referees' verdicts, all because those assert what the
///   unselected tests would have written. A filter matching nothing fails the run rather than
///   reporting a green zero.
///
/// # EXAMPLES
///
/// ```text
/// $ script/test --arch aarch64 --test frames_are_zeroed
/// --- test filter: frames_are_zeroed (kernel legs only; the host crates have `cargo test`) ---
/// --- kernel tests, aarch64 (QEMU) ---
/// running 1 of 312 tests (filter: frames_are_zeroed)
/// test kernel::memory::tests::frames_are_zeroed ... ok
/// test result: ok. 1 passed
/// ```
///
/// `script/cpu_matrix` is the caller that needs the first two (notes/cpu-models.md); `script/ci-build`
/// is the caller that needs the third.
pub(crate) fn test() -> bool {
    // Milestone 81. Read before `--arch`, because it constrains it: Hypervisor.framework runs the
    // host's own ISA and this host is aarch64, so there is no riscv64 leg to accelerate and asking
    // for one is a mistake worth naming rather than ignoring.
    let hvf = std::env::args().any(|a| a == "--hvf");
    let legs = match flag_value("--arch").as_deref() {
        None if hvf => ArchLegs::Aarch64,
        None => ArchLegs::All,
        Some("aarch64") => ArchLegs::Aarch64,
        Some("riscv64") if hvf => {
            eprintln!(
                "test: --hvf is aarch64 only (Hypervisor.framework runs this host's own ISA; riscv64 \
                 has no equivalent until the board lands)"
            );
            return false;
        }
        Some("riscv64") => ArchLegs::Riscv64,
        Some("x86_64") if hvf => {
            eprintln!(
                "test: --hvf is aarch64 only (Hypervisor.framework runs this host's own ISA, and \
                 this host is aarch64)"
            );
            return false;
        }
        Some("x86_64") => ArchLegs::X86_64,
        Some(other) => {
            eprintln!("test: --arch {other} is not an architecture (aarch64, riscv64 or x86_64)");
            return false;
        }
    };
    // The suite's userspace carries the confinement tests' attacker roles; no other boot's does.
    // Set before any leg builds its archive, and for the whole run, so a `--test` falsification
    // replay gets them too. See `CONFINEMENT_ATTACKERS`.
    crate::CONFINEMENT_ATTACKERS.store(true, std::sync::atomic::Ordering::Relaxed);
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

    // **`--test <substring>`: run one kernel test rather than the suite** (milestone 210).
    //
    // It rides to the kernel through the environment like `--cpu` does, but it lands in a different
    // place: `kernel/build.rs` bakes it into the test binary as a `rustc-env`, because a kernel test
    // runs inside a booted kernel and there is no command line to hand it. Changing it costs a
    // kernel relink (~2.3 s) and buys the ~53 s the aarch64 suite spends running 312 tests. Unset it
    // when no flag was given, on `--cpu`'s reasoning exactly: a stale value in the caller's shell
    // must not silently change what a plain `script/test` means.
    //
    // **It does not narrow the architectures**, and that is deliberate (DECISIONS §19). A filter
    // that quietly ran one ISA would be the parity hole the tenet distrusts; say `--arch aarch64`
    // as well when one leg is what you want.
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
    // `filter.is_none()` because `--test` names a KERNEL test: the host crates already have
    // `cargo test <name>`, which is what milestone 210 exists to give the kernel, and running the
    // whole host pass (about 72 s here) to reach one kernel test would keep most of the cost the
    // flag is meant to remove.
    if !hvf && filter.is_none() {
        eprintln!("--- host tests (pure logic, no emulator) ---");
        // Every host crate, by asking cargo which ones those are instead of listing them.
        //
        // This was a hand-maintained list of twenty `-p` flags, and it drifted exactly the way a
        // hand-maintained list does. It was written because `paging`, `heap` and `slab` were silently not
        // run for four milestones; by milestone 51 it had five crates missing again, and `filesystem_protocol`,
        // `compositor`, `video_terminal`, `bitmap_font` and `grant_plan` carried **82 host tests that this gate never ran**. All
        // 82 passed when finally run, which is the point: nobody noticed because nothing failed, and a
        // gate that quietly covers less than it claims is the failure mode script/fmt's `--check` bug
        // already cost this project a day over.
        //
        // The exclusions are every crate that cannot compile for the host, which means `user_mode_runtime` (EL0
        // syscall `asm!`) and everything that depends on it.
        //
        // **`--exclude` removes a package from the test SELECTION, not from the dependency graph.**
        // Excluding `user_mode_runtime` alone stopped working on 2026-08-03, when `swap_protocol`, `virtio` and
        // `supervision_protocol` took unconditional `user_mode_runtime` dependencies (`system_initializer`
        // followed a day later): cargo still had to build it for them, so the host pass stopped
        // compiling on an x86_64 host and nobody noticed, because CI moved to `ubuntu-24.04-arm` the
        // same day and on an aarch64 host it builds by accident. A stranger with a clean x86_64
        // checkout found it on 2026-08-14 (milestone 117's first run).
        //
        // `script/lint`'s "host pass excludes exactly the bare-metal crates" gate now DERIVES this
        // set from `cargo metadata` and fails if this list disagrees with it, so the next crate to
        // take a `user_mode_runtime` dependency breaks the gate rather than the host build.
        if !cargo(&[
            "test",
            "--workspace",
            "--exclude",
            "kernel",
            "--exclude",
            "system_tests",
            "--exclude",
            "components",
            "--exclude",
            "fixtures",
            "--exclude",
            "user_mode_runtime",
            "--exclude",
            "swap_protocol",
            "--exclude",
            "virtio",
            "--exclude",
            "supervision_protocol",
            "--exclude",
            "system_initializer",
        ]) {
            return false;
        }

        // The vendored RedoxFS pin (vendor/redoxfs, milestone 32) is kept honest here, both halves of
        // vendor/README.md's promise. Both are driven by --manifest-path because the engine and the
        // host tool are their OWN workspaces, deliberately outside ours so upstream code never reaches
        // our clippy/fmt gates (see the workspace `exclude` in Cargo.toml).
        //
        // First: the host tool's round trip (mkfs, put, ls, cat) against the pinned engine, the same
        // code phase 2's FS server will open images with, so a regression is caught on the host in
        // milliseconds. Second: the engine's no_std core built for BOTH bare-metal targets, because
        // upstream does not CI the no_std path and it bit-rotted once already (the two Vec imports the
        // pin carries); this build catches the next such regression instead of phase 2 doing it.
        eprintln!();
        eprintln!("--- vendored redoxfs: host round trip + no_std core (both targets) ---");
        if !run(
            "cargo",
            &["test", "--manifest-path", "tools/redoxfs_host/Cargo.toml"],
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
    }

    // Build the std demo (milestone 27) for every custom target first, so every initrd carries it:
    // initrd_aarch64 (inside `user`) packs the aarch64 std_exerciser, initrd_riscv packs the riscv one. Outside
    // the leg guards below because BOTH legs need it, and the nifefs data disk with it: it is
    // arch-neutral, and the riscv leg reads it whether or not the aarch64 leg ran.
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
    if !crate::farm::tls_graph(&tls_triples) {
        return false;
    }
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

    if legs.aarch64() {
        eprintln!();
        eprintln!("--- kernel tests, aarch64 (QEMU) ---");
        // The FS server (milestone 32 phase 2), for the aarch64 bare target, before `user()` so
        // initrd_aarch64 packs it; then the RedoxFS test images the runner attaches as extra mmio disks.
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
        let leg = if hvf {
            hvf_both_images(filter.is_some())
        } else if filter.is_some() {
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
        };
        if !leg {
            return false;
        }
    }

    // The same booted kernel test suite on the second architecture (parity workstream B). The
    // portable tests (scheduler, capabilities, revocation, memory, sync) run on RISC-V's real Sv39
    // kernel; what stays gated to aarch64 is what genuinely needs aarch64 (the userspace-exec suite's
    // hand-written machine code, and SMP). The two interrupt-delivery tests used to be on that list
    // because they trigger with a GIC SGI; milestone 19 made the trigger per-arch instead, so they
    // run here too. RISC-V exits via the sifive_test finisher, same harness. See
    // notes/riscv-parity-scope.md and notes/interrupts.md.
    if legs.riscv64() {
        eprintln!();
        eprintln!("--- kernel tests, riscv64 (QEMU) ---");
        // The riscv userspace tests (parity C) load programs from the initrd and read the disk, so
        // build the riscv archive and point the runner at IT, not at the aarch64 archive `cargo()`
        // exports: the riscv ELF loader must never be handed aarch64 ELFs. The disk is arch-neutral
        // (a nifefs data image) and was built by mkdisk() above.
        // The riscv FS server, before the riscv archive that packs it.
        if !redoxfs_server_build(RISCV_TARGET) || !initrd_riscv() {
            return false;
        }
        // **A fresh RedoxFS image for this leg.** The two ISA legs share one image path, and the
        // aarch64 leg above WRITES it (the std::fs test and the FS client both do). Reusing it here
        // would make the riscv leg's writes land on an image a previous boot mutated, so the legs
        // would be order-coupled and neither would be reproducible on its own. Each leg gets the
        // same known-good fixture instead. This is test determinism, not a workaround: the
        // cross-boot write failure it separates out is real, and notes/fs-server.md carries it as a
        // tracked open item with the exact recipe to reproduce it (run one leg, then the other,
        // without regenerating in between).
        if !mkredoxfs() || !mkredoxfs_crash() || !mkgptdisk() || !mkblankdisk() || !mknvmedisk() {
            return false;
        }
        // SAFETY: `set_var`/`remove_var` became unsafe in edition 2024 because they race other
        // threads. xtask is single-threaded here: this runs on the main thread before the child
        // that reads it is spawned, and the only thread xtask ever starts (the transcript reader
        // in swish_check_leg) copies pipe bytes into a String and never touches the environment.
        unsafe { std::env::set_var("NIFE_INITRD", riscv_initrd_path()) };
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
        if filter.is_some() {
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
    }

    // **The third architecture** (milestone 161, roadmap item 4). The same booted kernel suite on
    // x86_64's real 4-level map, scheduler and ring 3, exiting through `isa-debug-exit` where the
    // other two use semihosting and the SiFive test finisher.
    //
    // **It builds a userspace archive and one disk image**, which is where it now sits between the
    // other two rather than below both. `initrd_x86` compiles every program in `user/` for this
    // target and packs the same table RISC-V's archive uses, so the thirty `cfg(initrd)` test
    // modules are in this binary; since milestone 164 it packs the FS server and `mkfs` too. What
    // it still does not build is a `std` farm, and the runner still attaches no virtio-blk, so the
    // tests wanting a filesystem `skip!()` for want of a DISK rather than of a server. The one
    // exception
    // is the NVMe image (decisions §86's x86_64/VT-d data point, milestone 161's VT-d having
    // landed): `mknvmedisk` writes it here the same way the aarch64 and riscv64 legs do, since
    // NIFE_NVME names this leg's image too (set unconditionally above) and the runner now attaches
    // a controller behind it.
    //
    // **`NIFE_INITRD` is set here rather than left to `cargo()`**, and it has to be: this leg runs
    // last, so whatever the aarch64 or riscv64 leg left in that variable is still there, and an x86
    // kernel handed an aarch64 archive refuses every program in it with a `machine` error that
    // names neither the archive nor the leg. `run` rather than `cargo` because that wrapper also
    // exports `NIFE_DISK` and `NIFE_NET`, and this runner attaches neither.
    if legs.x86_64() {
        eprintln!();
        eprintln!("--- kernel tests, x86_64 (QEMU q35) ---");
        // The FS server for this target BEFORE the archive that packs it (milestone 164), the same
        // order the aarch64 and riscv64 legs use. `mkdisk` since milestone 215 (a PCI function's
        // interrupt on x86_64), because this runner attaches the sibling `-pci.img` as a
        // virtio-blk-pci function, and `mkredoxfs` since milestone 303, because it now attaches the
        // `-redoxfs.img` sibling as a second one. The FS tests reach a real filesystem here.
        //
        // **It runs after the other two legs, and regenerates every image they wrote**, which is
        // the same freshness discipline the riscv64 leg's own `mkredoxfs` call documents: a leg
        // that wrote to an image a previous leg mutated would be order-coupled and reproducible
        // only in sequence. Each leg gets the known-good fixture.
        if !redoxfs_server_build(X86_TARGET)
            || !initrd_x86()
            || !mkdisk()
            || !mkredoxfs()
            || !mknvmedisk()
        {
            return false;
        }
        // SAFETY: `set_var` became unsafe in edition 2024 because it races other threads. xtask is
        // single-threaded here: this runs on the main thread before the child that reads it is
        // spawned, and the only thread xtask ever starts (the transcript reader in swish_check_leg)
        // copies pipe bytes into a String and never touches the environment.
        unsafe { std::env::set_var("NIFE_INITRD", x86_initrd_path()) };
        // **`NIFE_DISK` names the fixture set, not one disk**, exactly as it does on both other
        // runners. This one derives the `-pci.img` and `-redoxfs.img` siblings from it and attaches
        // those as the first and second virtio-blk-pci functions (milestones 215 and 303); `q35`
        // has no virtio-mmio bus, so the image the variable itself names is not attached anywhere
        // here.
        //
        // SAFETY: `set_var` became unsafe in edition 2024 because it races other threads. xtask is
        // single-threaded here: this runs on the main thread before the child that reads it is
        // spawned, and the only thread xtask ever starts (the transcript reader in
        // swish_check_leg) copies pipe bytes into a String and never touches the environment.
        unsafe { std::env::set_var("NIFE_DISK", disk_path()) };
        // The kernel's unit tests, then the system suite (milestone 609 (the system tests leave the
        // kernel crate)).
        if filter.is_some() {
            if !filtered_leg("x86_64", X86_TARGET) {
                return false;
            }
        } else if !time_record::cargo_test(
            &time_record::leg("x86_64"),
            "kernel",
            &["test", "-p", "kernel", "--target", X86_TARGET],
        ) || !time_record::cargo_test(
            &time_record::leg("x86_64"),
            "system_tests",
            &["test", "-p", "system_tests", "--target", X86_TARGET],
        ) {
            return false;
        }
        // **And one more boot, on a machine with a bridge on it** (milestone 320). `q35` is a flat
        // root complex: every device hangs off bus 0, which is why a kernel that mapped one
        // megabyte of configuration space and enumerated bus 0 passed every test in this tree for a
        // year and then found no disk at all on the first real machine it met. `NIFE_PCIE_ROOT_PORT`
        // puts the NVMe controller behind a `pcie-root-port`, which is the topology xenon's M.2 slot
        // has.
        //
        // **One test, not the suite**, and that is the cost decision stated where it is paid. The
        // claim needing a second topology is one claim (the walk follows a bridge and finds what is
        // behind it); re-running two hundred tests under it would buy coverage of the tests rather
        // than of the topology, the same argument `uefi_boot` above makes about firmware. It costs
        // about three seconds.
        //
        // Skipped under `--test`, like the image checks below and for the same reason: the filter
        // names a kernel test, and clobbering it here would run something the caller did not ask
        // for and report it as what they did.
        if filter.is_none() {
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
            if !bridged {
                return false;
            }
        }
        // **And the whole suite again, on a machine whose IOMMU is AMD-Vi** (lane `amd-vi`,
        // provisional). `q35` with `-device amd-iommu` writes an IVRS and no DMAR, so the kernel
        // brings up `arch/x86_64/amd_vi.rs` instead of VT-d, and every confinement test above
        // (the virtio and NVMe DMA escapes among them) runs against the other unit. The whole
        // suite rather than the escape tests alone, because the claim is that every device the
        // kernel confines is confined on this hardware too, and DECISIONS §19 (architectural
        // parity is a tenet) says the same suite proves a capability on every machine it ships
        // on; the cost is one more PVH boot of each image. Fresh images first, on the freshness
        // rule the leg above states.
        //
        // Skipped under `--test`, like the root-port boot and for its reason. A caller who wants
        // one test on this machine sets `NIFE_IOMMU=amd` for the whole run, which is what a
        // falsification record's `Environment:` line does.
        if filter.is_none() && std::env::var_os("NIFE_IOMMU").is_none() {
            eprintln!();
            eprintln!("--- kernel and system tests, x86_64 with AMD-Vi in place of VT-d ---");
            if !mkdisk() || !mkredoxfs() || !mknvmedisk() {
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
            ) && time_record::cargo_test(
                &time_record::leg("x86_64"),
                "system_tests",
                &["test", "-p", "system_tests", "--target", X86_TARGET],
            );
            // Removed whether or not it passed, so every later boot gets the VT-d machine.
            //
            // SAFETY: as above. Single-threaded, on the main thread, and the child that read it
            // has already exited.
            unsafe { std::env::remove_var("NIFE_IOMMU") };
            if !amd {
                return false;
            }
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
    }

    // **And the boot tour on a screen, on the two boards**, milestone 243 (a machine with no serial
    // port). The x86_64 leg above proves the same claim through `uefi_boot`, on a framebuffer the
    // firmware lit; these two prove it on a `ramfb`, which is what QEMU's `virt` can present. One
    // extra TCG boot per leg, and it is the only thing in this suite that can fail when the screen
    // is black: every other assertion here reads a serial line that says the same words whether or
    // not a pixel was written.
    //
    // It is a boot of its own rather than a stage of the suite above, because `ramfb` adds a QEMU
    // console and the suite's machine already has a virtio-gpu on console 0.
    //
    // **Not under `--cpu`**, and this is a cost decision rather than a way around a failure. The
    // matrix (`script/cpu-matrix`) runs this suite five times to narrow the *ISA*, and nothing on
    // the screen path varies with `-cpu`: the `fw_cfg` conversation is byte moves and MMIO stores,
    // `screen_console` is integer arithmetic and byte stores, and every instruction either uses is
    // one the three hundred tests above have already executed on that same model. Five more TCG
    // boots buy a claim that cannot differ between models, and the script's own warning ("do not
    // route around it by dropping the model") is not what this is: no model is dropped, and the leg
    // still runs on every ordinary `script/test`, including `--arch riscv64`.
    if flag_value("--cpu").is_none() {
        if legs.aarch64() && !crate::screen::screen_boot("aarch64") {
            return false;
        }
        if legs.riscv64() && !crate::screen::screen_boot("riscv64") {
            return false;
        }
    }

    // FS-level consistency after the runs (milestone 32 phase 2): reopen the RedoxFS image with the
    // host tool and confirm the FS server's write persisted and the filesystem still parses. This
    // checks the image of whichever leg ran LAST, each of which regenerates the fixture and then
    // writes it: x86_64 in a full run (milestone 303 gave it a RedoxFS disk), riscv64 when
    // `--arch x86_64` was not asked for, aarch64 when it was the only leg.
    //
    // **The crash and blank images are aarch64's and riscv64's alone**, and that is why the x86_64
    // arm below is separate rather than the guard simply going away. `helpers/qemu-runner-x86_64.sh`
    // attaches two PCI functions, the nifefs image and the RedoxFS one; milestone 37's crash disk
    // and milestone 57's GPT and blank disks are not among them, so those two checks would open
    // whatever a previous full run left and report a true statement about a stale file and a false
    // one about this run. A check whose subject did not happen is worse than no check.
    if !legs.aarch64() && !legs.riscv64() {
        if filter.is_some() {
            return true;
        }
        eprintln!();
        eprintln!("--- redoxfs image consistency after the run (host tool) ---");
        return redoxfs_check_after_run();
    }
    // **And not under a filter** (milestone 210), for the same reason the `--arch x86_64` guard
    // above exists: these checks assert what the FS tests WROTE, so a run that did not select them
    // would open a stale image and report "motd did not read back", which is a true statement about
    // a leftover file and a false one about this run. A check whose subject did not happen is worse
    // than no check.
    if filter.is_some() {
        return true;
    }
    eprintln!();
    eprintln!("--- redoxfs image consistency after the run (host tool) ---");
    // Both images: the shared fixture (the write persisted, the filesystem still parses) and the
    // crash test's own disk (milestone 37: after a kill mid-transaction, `cut` is one payload whole).
    // Three images: the shared fixture (the write persisted and the filesystem still parses), the
    // crash test's own disk (milestone 37), and milestone 57's blank disk, where the guest wrote
    // both the partition table and the filesystem inside it.
    redoxfs_check_after_run() && redoxfs_crash_check_after_run() && blank_check_after_run()
}

/// **The aarch64 kernel suite on the physical Apple Silicon core** (milestone 81, `--hvf`).
///
/// # Why this is not just `cargo test` with an env var set
///
/// **QEMU does not intercept ARM semihosting under HVF**, and the whole test harness reports its
/// verdict through it: the kernel's `testing::runner` ends in `semihosting::exit`, and so do the panic
/// handler and both watchdogs. Measured against QEMU 11.0.2 with a nine-instruction guest that
/// writes a byte to the PL011 and then executes the semihosting trap: under TCG the process exits
/// at the trap, under HVF the byte appears and `hlt #0xf000` never returns. So under HVF the guest
/// prints its result and then wedges, and cargo (which waits for the child) would wait forever.
///
/// The answer is the one `run_bench` already uses for the same reason: **own the QEMU child and
/// read its transcript.** We ask cargo for the test ELF without running it, hand that ELF to the
/// same runner script everything else boots through, and read stdout until the harness says how it
/// went. Three markers decide the verdict, all of them printed *before* the exit that will not
/// happen:
///
/// - `test result: ok. N passed` from the runner: the suite passed;
/// - `[PANIC] ` from the panic handler: a failing assertion, which is a failing test;
/// - `WATCHDOG:` from either watchdog: a hang or a livelock, also a failure.
///
/// Reaching end of output with none of them means QEMU died on its own, which is a failure too.
/// The guest's own watchdogs are what bound this leg, exactly as they bound the TCG one: they still
/// fire (the virtual timer is passed through and QEMU injects the interrupt), and their message is
/// what we read. So there is no host-side deadline.
///
/// # The referee runs beside it, on a thread, and it has to
///
/// [`ScanoutReferee`] is not optional here even though it is a *display* check: it is also what
/// presses keys, over QEMU's monitor, and the keyboard test asserts that a keystroke arrived
/// ("the keyboard driver came up but never typed anything in ten seconds"). Nothing in the guest
/// can press a key. Reading the transcript blocks, so the referee is driven from a second thread
/// and joined when the verdict is in. It touches a unix socket and two files and never the
/// environment.
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
        let (ok, n) = cargo_test_counting_selected(&["test", "-p", package, "--target", target]);
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

/// One test image on the physical core. Returns the verdict and how many tests its harness selected.
fn hvf_kernel_leg(package: &str, watch_devices: bool) -> (bool, usize) {
    let Some(elf) = kernel_test_elf(package, TARGET, "test --hvf") else {
        return (false, 0);
    };

    eprintln!();
    eprintln!(
        "--- {package} tests, aarch64, ON THE PHYSICAL CORE (QEMU + Hypervisor.framework) ---"
    );

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
            return (false, 0);
        }
        Ok(_) => {}
        Err(e) => {
            eprintln!("test --hvf: failed to start {RUNNER} for the machine probe: {e}");
            return (false, 0);
        }
    }

    // Constructed before the child, because it is what sets `NIFE_GPU_MON`: the runner reads
    // that when it builds the QEMU command line, so a referee born later would find no monitor.
    //
    // Only the system suite paints the scanout and answers the network (milestone 609 (the system
    // tests leave the kernel crate)), so only its run gets either.
    let referee = watch_devices.then(|| ScanoutReferee::new("aarch64"));
    // And the inbound prober, for the same reason and on the same terms: it sets
    // `NIFE_HOSTFWD_PORT` before the child exists, and it runs on its own thread throughout. The
    // accept test is not accelerator-sensitive, but it is in the suite, so a leg without a prober
    // would fail it. Its "before the child" placement is load-bearing exactly as the referee's is.
    let prober = watch_devices.then(|| InboundProber::new("aarch64"));

    let mut cmd = Command::new(RUNNER);
    cmd.arg(&elf);
    // The one child that gets the accelerator. `test()` cleared it from our own environment, so
    // nothing else in this process can inherit it.
    cmd.env("NIFE_ACCEL", "hvf");
    // The same devices the TCG leg attaches, set by `test()` and `cargo()` in our environment and
    // inherited from there: the initrd, the disks, the NIC, the GPU, the keyboard, the RNGs.
    cmd.env("NIFE_INITRD", initrd_path());
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

    use std::io::BufRead;
    let stdout = child.stdout.take().expect("piped stdout");
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

    // A panic ends the suite on either leg (no unwinding in the custom test framework), so the
    // scanout and inbound checks below then report on a guest that may never have reached the tests
    // they referee. On 2026-09-24 one calibration failure printed four more FAILED lines that were
    // all consequences of it, and they read as four more findings. Say which failure is first.
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

    // **A filtered run drops both verdicts**, as `test`'s own doc promises for every leg: the scanout
    // and inbound tests were not selected, so their referees watch a guest that never ran them and
    // fail on the absence. This leg printed three scanout FAILs and an inbound FAIL under `--test`
    // until 2026-09-26, when milestone 121 (`ripgrep` on nife: enumeration as a capability)'s
    // lane met it with its two selected tests passing.
    let filtered = std::env::var_os("NIFE_TEST_FILTER").is_some_and(|f| !f.is_empty());
    let ok = match verdict {
        Some(true) => filtered || (scanout_ok && inbound_ok),
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
    };
    load.report_if_failed(ok, "aarch64 --hvf");
    (ok, selected)
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

/// **The host tests again, under Miri's interpreter** (milestone 79, notes/undefined-behavior.md).
///
/// Miri checks the rules nothing else in the tree checks: aliasing (tree borrows), pointer
/// provenance, uninitialized reads, leaks. Kani proves the properties it is asked about and the
/// fuzzers see crashes; neither sees a `&mut` that aliases. The crate selection is `test()`'s,
/// verbatim and for the same reason it is `--workspace --exclude` there rather than a list: a
/// hand-maintained list drifted twice, and this way a new crate is covered the moment it joins
/// the workspace. The exclusions are the three bare-metal crates that do not compile for the
/// host, plus one that does:
///
/// **`xtask` is excluded like it is from `script/coverage`, and for cost, not principle.** It is
/// the build tool, not a host-logic crate; its three tests are the scanout referees, safe pixel
/// arithmetic with no `unsafe` on any path, and under the interpreter they cost around seven
/// minutes for nothing the type system has not already said. They were run once under Miri during
/// milestone 79's first full sweep and were clean; the recurring run leaves them out.
///
/// **`board_console` is excluded for the same reason and a much larger number** (milestone 238). It
/// is the serial-console reader for a board on a bench, and it measured **3,307 seconds, 55
/// minutes, for its lib tests alone** under Miri on 2026-09-03, against roughly four minutes for
/// the entire rest of the workspace. It has **no dependencies and no `unsafe`**, so there is
/// nothing in its call graph that Miri's rules can be broken by. Ten of its forty-one tests could
/// not run under the interpreter anyway, and each one says why it is out rather than being a
/// puzzle: five reach the host filesystem (`open`, `/dev`, the temp dir), which isolation refuses,
/// and five in `watch` are wall-clock driven, so a 15-second quiet timeout and a 120-second budget
/// expire against interpreted time and the watcher reports `Reached(Banner)` where a real run
/// reaches `Reached(Tour)`. That last family is the same category as `credentialer`'s timing test, which
/// notes/undefined-behavior.md already records: a wall-clock ratio under an interpreter measures
/// Miri, not the thing being timed.
///
/// This crate was never *deliberately* covered: it joined the workspace after milestone 79, and the
/// weekly job had been red on an unrelated failure ever since, so its cost was only discovered when
/// milestone 238 cleared the failure in front of it.
///
/// **`stick_maker` is excluded for `board_console`'s reason** (milestone 636 (the scheduled
/// workflows are failing), 2026-10-03 UTC). Its `cli` and `linux` tests build and remove trees under
/// the temp dir, and isolation refuses the first `statx`: "unsupported operation: `statx` not
/// available when isolation is enabled", which failed the weekly run on 2026-09-21 and again on a
/// 2026-10-03 dispatch (run 37109746541) once the crate in front of it was repaired. That is the
/// harness refusing I/O, not a finding. Its only `unsafe` is FFI in `src/host/` (`geteuid`, four
/// `kernel32` calls), which Miri cannot execute either, so nothing Miri checks is lost; its one
/// dependency, `measured_boot`, stays in the run on its own tests.
///
/// **`walk_pricing` is out for the same reason, found the same day.** It is "ordinary `std::fs`
/// code" by its own header, with no `unsafe`, and three of its four tests stage a tree under the
/// temp dir, where isolation refuses `lstat`; the fourth fits a line. It is also a timing
/// instrument, and a wall-clock price under the interpreter measures Miri. Its tree comes from
/// `filesystem_protocol::fixture`, which stays in the run.
///
/// **"Miri-clean" means the sampled paths.** An interpreter runs roughly a thousand times slower
/// than the silicon, so the exhaustive suites gate themselves down under `cfg(miri)`: `network_time_protocol`
/// strides its 10^9-value sweep, `globally_unique_identifier_partition_table` skips its 460k-parse
/// corruption sweeps, `calendar` and
/// `glob` shrink their strides and scales, `credentialer` derives at Argon2's floor (each site says so,
/// next to the test). What Miri certifies is every path the sampled suite executes, not the
/// exhaustive claims; those remain native-only.
///
/// The two out-of-workspace test surfaces stay out deliberately: `tools/redoxfs_host` and
/// `redoxfs_server` spend their runtime inside the vendored RedoxFS engine, and a finding in vendored
/// code lands in the vendor pin, not in a crate this tree can fix (vendor/README.md). Extra args
/// are forwarded to `cargo miri test`, but they do **not** narrow it: `-p` beside `--workspace`
/// still ran the whole workspace when milestone 636 tried it on 2026-10-03. To check one crate,
/// run `cargo miri test -p <crate>` directly.
pub(crate) fn undefined_behavior_check() -> bool {
    eprintln!("--- host tests under Miri (aliasing, provenance, uninitialized reads) ---");
    let mut args = vec![
        "miri",
        "test",
        "--workspace",
        "--exclude",
        "kernel",
        "--exclude",
        "system_tests",
        "--exclude",
        "components",
        "--exclude",
        "fixtures",
        "--exclude",
        "user_mode_runtime",
        "--exclude",
        "xtask",
        "--exclude",
        "board_console",
        "--exclude",
        "stick_maker",
        "--exclude",
        "walk_pricing",
    ];
    let extra: Vec<String> = std::env::args().skip(2).collect();
    args.extend(extra.iter().map(String::as_str));
    run("cargo", &args)
}
