//! Writes the kernel's coverage counters to a file on the host when a test image exits.
//!
//! Built only with the `coverage` feature, by `cargo xtask coverage` (milestone 781 (measure kernel
//! and service coverage under QEMU)). The kernel and every crate it links are compiled with
//! `-C instrument-coverage`; this module hands the counters to `minicov`, the LLVM profiling
//! runtime without a libc, which serializes them in the `.profraw` format. The sink is a file the
//! host opens through ARM semihosting, so no allocator and no filesystem is involved. The host side
//! (`llvm-profdata`, `llvm-cov`) is in `xtask/src/coverage.rs`. See notes/kernel-coverage.md.
//!
//! Name: provisional, minted 2026-10-27 (UTC) by the lane for milestone 781.

#[cfg(not(target_arch = "aarch64"))]
compile_error!(
    "the coverage feature writes through aarch64 semihosting; riscv64 and x86_64 follow"
);

use core::sync::atomic::{AtomicBool, Ordering};

use crate::arch::semihosting::HostFile;

/// The host path of the profile, NUL-terminated for semihosting. `build.rs` sets it from
/// `NIFE_COVERAGE_PROFILE`, which `cargo xtask coverage` makes absolute.
const PROFILE_PATH: &str = concat!(env!("NIFE_COVERAGE_PROFILE"), "\0");

/// Set by the first dump. A panic while dumping exits again, and must not dump again.
static DUMPED: AtomicBool = AtomicBool::new(false);

/// A `.profraw` file on the host, written a chunk at a time.
struct Profile(HostFile);

impl minicov::CoverageWriter for Profile {
    fn write(&mut self, data: &[u8]) -> Result<(), minicov::CoverageWriteError> {
        if self.0.write(data) {
            Ok(())
        } else {
            Err(minicov::CoverageWriteError)
        }
    }
}

/// Write every counter to the host and print one line saying how it went. Runs once; a second call
/// returns at once.
pub(crate) fn dump() {
    if DUMPED.swap(true, Ordering::AcqRel) {
        return;
    }
    let Some(file) = HostFile::create(PROFILE_PATH) else {
        crate::println!("coverage: the host refused to create the profile");
        return;
    };
    let mut profile = Profile(file);
    // SAFETY: `capture_coverage` is not thread-safe. It runs once, from the exit path, and the
    // counters it reads are plain integers other cores may still bump, which skews a count by a
    // few and cannot corrupt the file.
    match unsafe { minicov::capture_coverage(&mut profile) } {
        Ok(()) => crate::println!("coverage: profile written"),
        Err(_) => crate::println!("coverage: the profile write failed"),
    }
}
