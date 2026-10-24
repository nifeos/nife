//! Running host commands, and finding what the build put where.
//!
//! The few helpers every command needs: `cargo`, a bare `run`, a captured `run`, the LLVM
//! tools out of the toolchain's sysroot, and the paths to the workspace and its binaries.

use std::process::Command;

use crate::archive::initial_ramdisk_path;
use crate::disk::disk_path;
use crate::{TARGET, profile_dir};

/// The absolute ELF path of the binary `name` the `user` package builds (milestone 19f.2+):
/// `hello`, `least_authority_demo`, `console`, and so on, each of which `initial_ramdisk_aarch64`
/// packs into the archive under that same name.
///
/// **The path is absolute** because cargo runs the runner script from the package directory for
/// `cargo test` and from the workspace root for `cargo run`. A relative path resolved under one and
/// silently not under the other, and the tests booted with no initial ramdisk at all.
pub(crate) fn bin_elf(name: &str) -> String {
    workspace_root()
        .join(format!("target/{TARGET}/{}/{name}", profile_dir()))
        .display()
        .to_string()
}

/// The repo root: the parent of xtask's manifest directory, as cargo reports it at run time, so it
/// does not depend on whatever working directory cargo hands us.
pub(crate) fn workspace_root() -> std::path::PathBuf {
    // Runtime, not env!: the compile-time form bakes the absolute path into the binary, so a
    // cached xtask built before the checkout moved aims every path it computes (the farm, the
    // initial ramdisks, the images) at a directory that no longer exists. Cargo sets the variable
    // at run time for every binary it invokes, and that one is always the live path.
    let manifest =
        std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR for xtask");
    std::path::Path::new(&manifest)
        .parent()
        .expect("xtask has no parent directory")
        .to_path_buf()
}

/// The value of a `--name value` or `--name=value` flag on our own command line, if it is there.
///
/// Both spellings, because a caller who has typed `--cpu=sifive-u54` once should not have to learn
/// that this particular tool only accepts one of them. Returns `None` when the flag is absent and
/// when it is present with nothing after it, which the callers treat as "not given"; a flag whose
/// value went missing is a typo, and defaulting is friendlier than a panic in a build tool.
pub(crate) fn flag_value(name: &str) -> Option<String> {
    let mut args = std::env::args();
    let eq = format!("{name}=");
    while let Some(a) = args.next() {
        if a == name {
            return args.next();
        }
        if let Some(v) = a.strip_prefix(&eq) {
            return Some(v.to_string());
        }
    }
    None
}

pub(crate) fn llvm_tool(name: &str) -> Option<String> {
    let sysroot = capture("rustc", &["--print", "sysroot"])?;
    let verbose = capture("rustc", &["-vV"])?;
    let host = verbose
        .lines()
        .find_map(|l| l.strip_prefix("host: "))?
        .trim();

    let path = format!("{}/lib/rustlib/{host}/bin/{name}", sysroot.trim());
    if std::path::Path::new(&path).exists() {
        Some(path)
    } else {
        eprintln!("cannot find {name} at {path}");
        eprintln!("the llvm-tools rustup component should provide it (see rust-toolchain.toml)");
        None
    }
}

pub(crate) fn capture(program: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(program).args(args).output().ok()?;
    String::from_utf8(out.stdout).ok()
}

/// The version of the emulator a leg will run, as the per-architecture banner and the red
/// post-run checks print it (milestone 414 (a red post-run check does not say which emulator
/// produced it)): `"11.1.2"`, or `"version unreadable"` when the binary cannot be asked.
///
/// The name resolves through PATH exactly as the runner's bare `exec qemu-system-...` does
/// (`helpers/qemu-path.sh` puts this project's own QEMU first), so the number names the
/// emulator that actually ran rather than one a config elsewhere prefers. Nothing new learns
/// which binary was chosen: PATH is still the mechanism, and `script/qemu-check` reads the
/// same field of the same output. Name provisional, minted by milestone 414's lane.
pub(crate) fn qemu_version(arch: &str) -> String {
    capture(&format!("qemu-system-{arch}"), &["--version"])
        .and_then(|out| parse_qemu_version(&out).map(str::to_string))
        .unwrap_or_else(|| "version unreadable".to_string())
}

/// The version field of a `--version` first line: whatever follows the word `version`.
///
/// Anchored on the word rather than counted from the left (`script/qemu-check`'s
/// `awk '{print $4}'`): upstream prints `QEMU emulator version 11.1.2`, where counting works,
/// and some distributions print `qemu-system-aarch64 version 8.2.2`, where the fourth field is
/// already inside the package suffix. The word appears in both shapes.
fn parse_qemu_version(version_output: &str) -> Option<&str> {
    let mut fields = version_output.lines().next()?.split_whitespace();
    while let Some(field) = fields.next() {
        if field == "version" {
            return fields.next();
        }
    }
    None
}

pub(crate) fn kernel_elf() -> String {
    format!("target/{TARGET}/{}/kernel", profile_dir())
}

pub(crate) fn cargo(args: &[&str]) -> bool {
    runner_env();
    run("cargo", args)
}

/// The environment [`cargo`] gives every runner, set without running anything: for a caller that
/// spawns the child itself, such as `time_record::cargo_test` timing a test image.
pub(crate) fn runner_env() {
    // The runner needs to know where the initial ramdisk is. Set it for every cargo invocation; the
    // script ignores it when the file is not there (which is any build before `user` exists).
    // SAFETY: `set_var`/`remove_var` became unsafe in edition 2024 because they race other
    // threads. xtask is single-threaded here: this runs on the main thread before the child
    // that reads it is spawned, and the only thread xtask ever starts (the transcript reader
    // in swish_check_leg) copies pipe bytes into a String and never touches the environment.
    unsafe { std::env::set_var("NIFE_INITIAL_RAMDISK", initial_ramdisk_path()) };
    // SAFETY: `set_var`/`remove_var` became unsafe in edition 2024 because they race other
    // threads. xtask is single-threaded here: this runs on the main thread before the child
    // that reads it is spawned, and the only thread xtask ever starts (the transcript reader
    // in swish_check_leg) copies pipe bytes into a String and never touches the environment.
    unsafe { std::env::set_var("NIFE_DISK", disk_path()) };
    // Attach a virtio-net NIC too (milestone 30): slirp needs no host file, so it is always on for
    // tests, and the net driver's DHCP round-trip test exercises it.
    // SAFETY: `set_var`/`remove_var` became unsafe in edition 2024 because they race other
    // threads. xtask is single-threaded here: this runs on the main thread before the child
    // that reads it is spawned, and the only thread xtask ever starts (the transcript reader
    // in swish_check_leg) copies pipe bytes into a String and never touches the environment.
    unsafe { std::env::set_var("NIFE_NET", "1") };
}

pub(crate) fn run(program: &str, args: &[&str]) -> bool {
    Command::new(program)
        .args(args)
        .status()
        .map(|s| s.success())
        .unwrap_or_else(|e| {
            eprintln!("failed to run {program}: {e}");
            false
        })
}

/// How many tests a transcript line says the harness selected: `K` in `running K of N tests`,
/// which `kernel/src/testing.rs`'s runner prints once per image under a filter.
pub(crate) fn selected_by(line: &str) -> Option<usize> {
    let rest = line.trim_start().strip_prefix("running ")?;
    let (count, rest) = rest.split_once(' ')?;
    rest.starts_with("of ").then(|| count.parse().ok())?
}

/// [`run`] for `cargo test` under `--test`: the child's stdout passes through line by line, and the
/// return is whether it succeeded and how many tests its harness selected (milestone 609 (the
/// system tests leave the kernel crate)). A leg boots two test images now and a filter may match
/// in only one, so the "selected nothing" verdict is the caller's, over both.
///
/// `who` names the leg and image for the host watchdog's failure line (`time_record::Watch`), which
/// bounds this run the way it bounds an unfiltered one.
pub(crate) fn cargo_test_counting_selected(args: &[&str], who: &str) -> (bool, usize) {
    let (mut child, watch) =
        match crate::time_record::Watch::spawn(Command::new("cargo").args(args)) {
            Ok(pair) => pair,
            Err(e) => {
                eprintln!("failed to run cargo: {e}");
                return (false, 0);
            }
        };
    let ok = watch.wait(&mut child, who).is_some_and(|s| s.success());
    let (lines, _) = watch.finish();
    let selected = lines
        .iter()
        .map(|l| selected_by(&l.text).unwrap_or(0))
        .sum();
    (ok, selected)
}

#[cfg(test)]
mod selected_by_tests {
    use super::selected_by;

    /// The filtered runner's line is the only one that counts, and it counts as its first number.
    /// An unfiltered run prints `running N tests` with no `of`, and a leg that summed that as a
    /// selection would call a whole suite "one test selected" (milestone 609 (the system tests
    /// leave the kernel crate)).
    #[test]
    fn only_the_filtered_runner_line_is_a_selection() {
        assert_eq!(selected_by("running 1 of 363 tests (filter: bss)"), Some(1));
        assert_eq!(
            selected_by("running 0 of 205 tests (filter: surveys)"),
            Some(0)
        );
        assert_eq!(selected_by("running 363 tests"), None);
        assert_eq!(
            selected_by("  cycles      : PMCCNTR_EL0 running on 4 of 4 cores"),
            None
        );
    }
}

#[cfg(test)]
mod qemu_version_tests {
    use super::parse_qemu_version;

    #[test]
    fn upstream_line() {
        assert_eq!(
            parse_qemu_version(
                "QEMU emulator version 11.1.2\nCopyright (c) 2003-2025 Fabrice Bellard\n"
            ),
            Some("11.1.2")
        );
    }

    #[test]
    fn distro_line_names_the_binary_first() {
        // Counting fields from the left reads the package suffix on this shape; the word
        // anchor does not.
        assert_eq!(
            parse_qemu_version("qemu-system-aarch64 version 8.2.2 (Debian 1:8.2.2+ds-0ubuntu1)\n"),
            Some("8.2.2")
        );
    }

    #[test]
    fn no_version_word_reads_as_unreadable() {
        assert_eq!(parse_qemu_version("qemu: something else entirely\n"), None);
        assert_eq!(parse_qemu_version(""), None);
    }
}
