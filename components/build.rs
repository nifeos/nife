//! An EL0 component gets the userspace linker script, and it must NOT be the kernel's.
//!
//! `cargo:rustc-link-arg` is per-package, which is the whole reason the EL0 programs are their own
//! packages rather than more binaries in `kernel/`. The script lives in `crates/user_mode_runtime`,
//! the runtime that supplies `_start`'s ABI and the panic handler to every program, because
//! `components` and `fixtures` link the identical EL0 layout: a copy in each would be two places to
//! be wrong about the image base with nothing comparing them.

use std::path::Path;

fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    link_el0(Path::new(&manifest_dir));
    carry_jh7110_entropy_identity(Path::new(&manifest_dir));
}

/// Point the linker at the shared EL0 script and keep the output an ELF.
///
/// Duplicated, four lines of it, in `fixtures/build.rs`: a build script is per-package and there is
/// nowhere below this that two packages can share code without a third crate existing to hold it.
/// The thing that would actually hurt to duplicate, the layout, is the one file both call.
fn link_el0(manifest_dir: &Path) {
    let script = manifest_dir.join("../crates/user_mode_runtime/link.ld");
    let script = script.display();
    println!("cargo::rerun-if-changed={script}");
    println!("cargo::rustc-link-arg=-T{script}");

    // Keep the ELF an ELF. The kernel's loader wants program headers, not a flat blob.
    println!("cargo::rustc-link-arg=--build-id=none");
}

/// Carry the `jh7110_entropy` version and commit that `Cargo.lock` resolved into this package's
/// programs, as `NIFE_JH7110_ENTROPY_VERSION` and `NIFE_JH7110_ENTROPY_COMMIT`.
///
/// `boot_self_test`'s `trng` line prints both, so a board transcript names the exact crate that
/// ran and nobody has to vouch for which build was on the board. That program and the
/// `jh7110_entropy` driver program are binaries of this package, built against the one workspace
/// lockfile, and measured boot refuses an archive from a different build than its kernel, so the
/// line names the driver that ran. A package the lockfile does not hold, or one from a source with
/// no commit, reads `unknown`, which `helpers/crate_hardware_run.py` refuses. See
/// design/roadmap/1035-a-crate-names-the-version-that-ran-on-hardware/bench-steps.md.
fn carry_jh7110_entropy_identity(manifest_dir: &Path) {
    let lockfile = manifest_dir
        .parent()
        .expect("the components manifest dir has no parent")
        .join("Cargo.lock");
    println!("cargo::rerun-if-changed={}", lockfile.display());
    let text = std::fs::read_to_string(&lockfile).unwrap_or_default();
    let (version, commit) = locked_git_package(&text, "jh7110_entropy")
        .unwrap_or(("unknown".to_string(), "unknown".to_string()));
    println!("cargo::rustc-env=NIFE_JH7110_ENTROPY_VERSION={version}");
    println!("cargo::rustc-env=NIFE_JH7110_ENTROPY_COMMIT={commit}");
}

/// The version and the 40-digit commit of the git package `name` in a `Cargo.lock`'s text.
///
/// Returns `None` when no `[[package]]` carries the name, or when its source is not a git source
/// ending in `#<commit>`.
fn locked_git_package(lock: &str, name: &str) -> Option<(String, String)> {
    for block in lock.split("[[package]]") {
        let field = |key: &str| {
            block.lines().find_map(|line| {
                line.strip_prefix(key)?
                    .strip_prefix(" = \"")?
                    .strip_suffix('"')
            })
        };
        if field("name") != Some(name) {
            continue;
        }
        let version = field("version")?;
        let source = field("source")?;
        let commit = source.strip_prefix("git+")?.rsplit_once('#')?.1;
        if commit.len() != 40 || !commit.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        return Some((version.to_string(), commit.to_string()));
    }
    None
}
