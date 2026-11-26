//! **Release or debug**: which cargo profile this run builds, and running cargo with it.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

/// `"release"` or `"debug"`: the cargo profile directory the built artifacts land in.
pub(crate) fn profile_dir() -> &'static str {
    if RELEASE.load(Ordering::Relaxed) {
        "release"
    } else {
        "debug"
    }
}

/// Run `cargo <args>`, adding `--release` when this is a release run. For the build commands whose
/// output profile must match `profile_dir()` (the kernel and user builds behind `bench`).
pub(crate) fn cargo_profiled(args: &[&str]) -> bool {
    let mut v = args.to_vec();
    if RELEASE.load(Ordering::Relaxed) {
        v.push("--release");
    }
    cargo(&v)
}
