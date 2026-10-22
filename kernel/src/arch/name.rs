//! **What this architecture is called**, in the spelling the rest of the tree uses for it.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

/// **What this architecture is called**, in the spelling the rest of the tree already uses for it:
/// `cargo xtask test --arch`, `script/swish-check --arch`, and the target directory names.
///
/// It is in the shared arch layer rather than in each architecture's own module because it is the
/// one fact about an architecture that is not about the hardware: it is what a person types and
/// what a log is grepped for. Milestone 268's machine-description summary line prints it, so `serial_console` can
/// say which of the three answered without keeping three copies of three spellings.
///
/// Name: provisional (milestone 268 (every architecture boots the same way)).
#[cfg(target_arch = "aarch64")]
// The machine description and the boot self-test are the only callers, and both are
// `#[cfg(not(any(test, feature = "bench")))]`: a test boot exits through semihosting and a bench
// boot diverges into `bench::run`, so neither reads a bring-up transcript. Same treatment
// `memory::print_summary` already carries, and for the same reason.
#[cfg_attr(
    any(test, feature = "system_tests", feature = "bench"),
    allow(dead_code)
)]
pub const NAME: &str = "aarch64";

#[cfg(target_arch = "riscv64")]
#[cfg_attr(
    any(test, feature = "system_tests", feature = "bench"),
    allow(dead_code)
)]
pub const NAME: &str = "riscv64";

#[cfg(target_arch = "x86_64")]
#[cfg_attr(
    any(test, feature = "system_tests", feature = "bench"),
    allow(dead_code)
)]
pub const NAME: &str = "x86_64";
