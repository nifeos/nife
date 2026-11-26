//! Which CRC32C code `rsext4` runs in this process: the CPU's CRC32C instructions or the crate's
//! table-driven code, which runs on any CPU.
//!
//! `rsext4` uses the table until a caller promises that every CPU the code may run on implements
//! the instructions (`rsext4::crc32c::enable_hardware_crc32c`). Each oracle binary calls [`select`]
//! before it mounts, so the same corpus judges both paths: the instructions where this process can
//! make that promise, the table elsewhere.
//!
//! - aarch64: the ARMv8 CRC32 extension, when `is_aarch64_feature_detected!("crc")` says so. std
//!   reads `HWCAP_CRC32` on Linux, which Linux sets only when every CPU has the extension and
//!   refuses to bring up a later CPU that lacks it. On macOS it reads the `hw.optional.armv8_crc32`
//!   sysctl, one value for the whole machine, whose cores all implement the same instruction set.
//! - x86_64: SSE4.2, only when the binary is built with `-C target-feature=+sse4.2`. Such a build
//!   already assumes SSE4.2 wherever it runs, since the compiler may emit it anywhere. A run-time
//!   CPUID read describes only the CPU that made it, so it is not used.
//! - Any other architecture, riscv64 among them: the table. `rsext4` has no instruction path there.
//!
//! When the environment names a path in `EXT4_ORACLE_CRC32C` (`instructions` or `table`), a binary
//! that would take the other one exits 70 before it reads anything, so a CI leg that means to
//! judge one path cannot silently judge the other.
//!
//! Name: provisional, minted 2026-11-26 (UTC) by milestone/190-ext4-oracle-three-architectures,
//! as are the environment variable's name and its two values. calef names files.

/// The environment variable that requires one path.
///
/// Name: provisional, minted 2026-11-26 (UTC) by milestone/190-ext4-oracle-three-architectures.
pub const REQUIRED_PATH_VARIABLE: &str = "EXT4_ORACLE_CRC32C";

/// Turn on the CRC32C instructions where this process can promise them, and return the path
/// `rsext4` now runs: `"instructions"` or `"table"`.
///
/// Exits 70 with a message on standard error when `EXT4_ORACLE_CRC32C` names a value other than
/// the path taken, including a value that names neither.
///
/// Name: provisional, minted 2026-11-26 (UTC) by milestone/190-ext4-oracle-three-architectures.
pub fn select() -> &'static str {
    let path = if enable_instructions() {
        assert!(rsext4::crc32c::hardware_crc32c_enabled());
        "instructions"
    } else {
        "table"
    };
    if let Some(required) = std::env::var_os(REQUIRED_PATH_VARIABLE)
        && required != path
    {
        eprintln!(
            "{REQUIRED_PATH_VARIABLE}={} but this binary computes CRC32C with the {path} path",
            required.to_string_lossy()
        );
        std::process::exit(70);
    }
    path
}

#[cfg(target_arch = "aarch64")]
fn enable_instructions() -> bool {
    if !std::arch::is_aarch64_feature_detected!("crc") {
        return false;
    }
    // SAFETY: on Linux the detection above reads HWCAP_CRC32, which Linux sets only when every CPU
    // implements the CRC32 extension, and it refuses to bring up a later CPU without it. On macOS
    // it reads hw.optional.armv8_crc32, which describes the machine, and every core of an Apple
    // chip implements the same instruction set. Either way, every CPU this process may run on has
    // the instructions.
    unsafe { rsext4::crc32c::enable_hardware_crc32c() };
    true
}

#[cfg(all(target_arch = "x86_64", target_feature = "sse4.2"))]
fn enable_instructions() -> bool {
    // SAFETY: this binary is built with SSE4.2 enabled, so the compiler may already have emitted
    // SSE4.2 instructions anywhere in it; it runs correctly only where every CPU has them, which is
    // the promise rsext4 asks for.
    unsafe { rsext4::crc32c::enable_hardware_crc32c() };
    true
}

#[cfg(not(any(
    target_arch = "aarch64",
    all(target_arch = "x86_64", target_feature = "sse4.2")
)))]
fn enable_instructions() -> bool {
    false
}
