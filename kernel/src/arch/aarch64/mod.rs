//! aarch64 support.
//!
//! Assembly, system registers, and CPU-specific behaviour live here and nowhere else (DECISIONS
//! §4 (kernel shape)). See notes/aarch64-port.md and notes/portability.md.

use core::arch::global_asm;

use crate::println;

pub mod context;
pub mod exceptions;
#[cfg(feature = "fastpath_pad")]
mod fastpath_pad;
pub mod fp;
mod instructions;
// The GICv3 CPU interface, `ICC_*` system registers (milestone 227). Private: `irq` is its only
// caller, and the one place that knows which GIC version this machine has.
mod gic_cpu_interface;
pub mod interrupts;
pub mod iommu;
pub mod irq;
pub mod isa;
pub mod mmu;
pub mod pmu;
pub mod semihosting;
pub mod thread_pointer;
pub mod timer;

// The saved thread context and how a new one is faked (the Rust half of context.s). Re-exported
// flat so `crate::arch::{Context, switch_to}` names them regardless of architecture.
pub use context::{Context, switch_to};
// E3's padding sled (milestone 134); see kernel/src/fastpath_pad.rs.
#[cfg(feature = "fastpath_pad")]
pub use fastpath_pad::fastpath_pad_body;

mod assembly;
mod cpu_init;
mod percpu;
mod primitives;
mod psci;

pub use cpu_init::*;
pub use percpu::*;
pub use primitives::*;
pub use psci::*;
