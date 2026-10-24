//! The RISC-V (rv64) architecture layer: an implementation of the `arch/` contract, which the rest
//! of the kernel calls through `crate::arch` by the same names as on aarch64 and x86_64.
//!
//! See notes/riscv-port.md and its appendices in notes/riscv-port/.

use core::arch::global_asm;

pub mod context;
pub mod exceptions;
#[cfg(feature = "fastpath_pad")]
mod fastpath_pad;
pub mod fp;
mod instructions;
pub mod interrupts;
pub mod iommu;
pub mod irq;
pub mod isa;
pub mod machine;
pub mod mmu;
pub mod pmu;
mod sbi;
pub mod semihosting;
pub mod thread_pointer;
pub mod timer;

// The saved thread context and how a new one is faked (the Rust half of context.s). Re-exported
// flat so `crate::arch::{Context, switch_to}` names them regardless of architecture.
use core::sync::atomic::{AtomicUsize, Ordering};

pub use context::{Context, switch_to};
// E3's padding sled (milestone 134); see kernel/src/fastpath_pad.rs.
#[cfg(feature = "fastpath_pad")]
pub use fastpath_pad::fastpath_pad_body;
/// The arch contract for a kernel-initiated cold reboot. See [`semihosting::reboot`].
pub use semihosting::reboot;

mod assembly;
mod cpu_init;
mod harts;
mod percpu;
mod primitives;
mod remote_fence;

pub use cpu_init::*;
pub use harts::*;
pub use percpu::*;
pub use primitives::*;
pub use remote_fence::*;
