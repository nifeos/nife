//! The hardware abstraction boundary.
//!
//! The rest of the kernel reaches the hardware only through what is re-exported here; `asm!` or a
//! system register outside `arch/` is the bug. The surface is listed in notes/portability.md, "The
//! arch contract"; anything else an architecture exports is reached under a `cfg`.
//!
//! # BUGS
//!
//! Nothing checks that list against the tree: an item added without a line, or a line left after
//! its last caller goes, is caught by nobody.

#[cfg(target_arch = "aarch64")]
mod aarch64;

#[cfg(target_arch = "aarch64")]
pub use aarch64::*;

// The second architecture (notes/riscv-port.md): a new ISA is a new directory.
#[cfg(target_arch = "riscv64")]
mod riscv64;

#[cfg(target_arch = "riscv64")]
pub use riscv64::*;

// riscv64's IOMMU driver, compiled for the prover on an aarch64 host, under its native module path
// so `script/falsifications` finds its patches. See notes/kernel-proofs.md, and for why only an
// aarch64 host, design/roadmap/0650-riscv64-code-the-prover-can-already-compile.md.
//
// A foot gun, deliberately taken: inside this module `crate::arch` is the host's architecture, not
// riscv64's. Only code that never calls through `crate::arch` is proved here, and `script/lint`
// fails if a listed file gains an unrecorded `crate::arch` reference. `dead_code` is allowed
// because only the pure functions the harnesses reach are live.
#[cfg(all(kani, target_arch = "aarch64"))]
#[allow(dead_code)]
mod riscv64 {
    mod iommu;
}

// The third architecture (notes/x86-port.md).
#[cfg(target_arch = "x86_64")]
mod x86_64;

#[cfg(target_arch = "x86_64")]
pub use x86_64::*;

mod halt_reason;
mod name;
mod user_fault;

pub use halt_reason::*;
// `NAME` is read only by boots that print a transcript, as its own `allow(dead_code)` says.
#[cfg_attr(
    any(test, feature = "system_tests", feature = "bench"),
    allow(unused_imports)
)]
pub use name::*;
pub use user_fault::*;
