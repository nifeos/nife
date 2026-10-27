//! **`cargo xtask coverage`**: which of the kernel's lines and regions the aarch64 test image reaches.
//!
//! Milestone 781 (measure kernel and service coverage under QEMU), ruled 2026-10-27 (UTC): build the
//! test kernel and the crates it links with `-C instrument-coverage`, link `minicov` (the LLVM
//! profiling runtime without a libc) behind the kernel's `coverage` feature, and let the kernel
//! write its counters to a file on the host through semihosting as the image exits. This command
//! builds that image in a target directory of its own (so the ordinary build cache is not
//! invalidated), boots it under `helpers/qemu-bounded.sh`, and turns the profile into the report
//! `script/coverage` writes for the host crates: an HTML report `helpers/error_paths.py` reads, and
//! an lcov file. The first increment covers the kernel's own unit-test image on aarch64. See
//! notes/kernel-coverage.md.
//!
//! Name: provisional, minted 2026-10-27 (UTC) by the lane for milestone 781.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::host::{llvm_tool, run, runner_env, workspace_root};
use crate::suite::{
    ArchLegs, attach_test_leg_devices, build_suite_programs, kernel_test_elf_with,
    prepare_aarch64_images,
};
use crate::{CONFINEMENT_ATTACKERS, TARGET};

/// Where the instrumented build, the profile and the reports live, relative to the repository root.
const OUT: &str = "target/kernel-coverage";

/// The bound on the instrumented boot, in seconds. The plain image finishes in a few minutes under
/// TCG; the counters cost some of that again.
const BOUND_SECONDS: &str = "2400";

/// Build, boot and report. True when the image booted, wrote a profile, and the report rendered;
/// a failing test still produces a report, and the verdict is then false.
pub(crate) fn coverage() -> bool {
    let root = workspace_root();
    let out = root.join(OUT);

    // The same userspace the suite packs, attackers included, so the image boots the tests it
    // would in `cargo xtask test`.
    CONFINEMENT_ATTACKERS.store(true, std::sync::atomic::Ordering::Relaxed);
    // SAFETY: `set_var` and `remove_var` are unsafe in edition 2024 because they race other
    // threads. xtask is single-threaded here, before any child is spawned.
    unsafe { std::env::remove_var("NIFE_ACCEL") };
    if !build_suite_programs(ArchLegs::AARCH64) {
        return false;
    }
    attach_test_leg_devices();
    if !prepare_aarch64_images() {
        return false;
    }
    runner_env();

    if let Err(e) = std::fs::create_dir_all(&out) {
        eprintln!("coverage: cannot create {}: {e}", out.display());
        return false;
    }
    let profraw = out.join("kernel.profraw");
    let _ = std::fs::remove_file(&profraw);
    // SAFETY: as above.
    unsafe {
        // The host path the kernel's semihosting write creates; `kernel/build.rs` reads it.
        std::env::set_var("NIFE_COVERAGE_PROFILE", &profraw);
        // Instrument the kernel and every crate it links. Set only now, after the userspace
        // archive is built: nothing built before this line is instrumented. The explicit
        // `--target` below keeps build scripts and proc macros uninstrumented.
        std::env::set_var(
            "RUSTFLAGS",
            "-C instrument-coverage -Zno-profiler-runtime",
        );
    }

    eprintln!("--- kernel unit tests, aarch64, instrumented (QEMU, bounded to {BOUND_SECONDS} s) ---");
    let out_arg = OUT;
    let Some(elf) = kernel_test_elf_with(
        "kernel",
        TARGET,
        "coverage",
        &["--features", "coverage", "--target-dir", out_arg],
    ) else {
        return false;
    };

    let booted = Command::new(root.join("helpers/qemu-bounded.sh"))
        .args([BOUND_SECONDS, "helpers/qemu-runner-aarch64.sh", &elf])
        .current_dir(&root)
        .status()
        .map(|s| s.success())
        .unwrap_or_else(|e| {
            eprintln!("coverage: cannot start the bounded runner: {e}");
            false
        });
    if !booted {
        eprintln!("coverage: the instrumented image did not pass; reporting what it covered");
    }
    if !profraw.is_file() || profraw.metadata().map(|m| m.len()).unwrap_or(0) == 0 {
        eprintln!(
            "coverage: no profile at {}; the image exited without writing one",
            profraw.display()
        );
        return false;
    }
    report(&root, &out, &profraw, Path::new(&elf)) && booted
}

/// Merge the profile, render the HTML and lcov reports, and run `helpers/error_paths.py` on the
/// HTML. The regions are the ones `cargo llvm-cov` reports for the host crates, because both ask
/// `llvm-cov` for the same thing of the same compiler's mapping.
fn report(root: &Path, out: &Path, profraw: &Path, elf: &Path) -> bool {
    let (Some(profdata), Some(llvm_cov)) = (llvm_tool("llvm-profdata"), llvm_tool("llvm-cov"))
    else {
        return false;
    };
    let merged = out.join("kernel.profdata");
    let html: PathBuf = out.join("html");
    let lcov = out.join("lcov.info");
    // Code the kernel links from outside the tree is not the kernel's coverage.
    let ignore = "--ignore-filename-regex=(/\\.cargo/registry/|/rustlib/)";
    let _ = std::fs::remove_dir_all(&html);
    let ok = run(
        &profdata,
        &[
            "merge",
            "-sparse",
            "-o",
            &merged.display().to_string(),
            &profraw.display().to_string(),
        ],
    ) && run(
        &llvm_cov,
        &[
            "show",
            "-format=html",
            ignore,
            &format!("-instr-profile={}", merged.display()),
            &format!("-output-dir={}", html.display()),
            &elf.display().to_string(),
        ],
    ) && export_lcov(&llvm_cov, ignore, &merged, elf, &lcov);
    if !ok {
        return false;
    }
    eprintln!("==> lcov: {}", lcov.display());
    eprintln!("==> html: {}/index.html", html.display());
    run_in(
        root,
        "python3",
        &["helpers/error_paths.py", &html.display().to_string()],
    )
}

/// `llvm-cov export -format=lcov`, written to `lcov`.
fn export_lcov(llvm_cov: &str, ignore: &str, merged: &Path, elf: &Path, lcov: &Path) -> bool {
    let exported = Command::new(llvm_cov)
        .args([
            "export",
            "-format=lcov",
            ignore,
            &format!("-instr-profile={}", merged.display()),
            &elf.display().to_string(),
        ])
        .output();
    match exported {
        Ok(o) if o.status.success() => std::fs::write(lcov, o.stdout).is_ok(),
        Ok(o) => {
            eprintln!("coverage: llvm-cov export failed: {}", String::from_utf8_lossy(&o.stderr));
            false
        }
        Err(e) => {
            eprintln!("coverage: cannot run llvm-cov: {e}");
            false
        }
    }
}

/// Run `program` with `root` as its working directory.
fn run_in(root: &Path, program: &str, args: &[&str]) -> bool {
    Command::new(program)
        .args(args)
        .current_dir(root)
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}
