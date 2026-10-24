//! **The `x86_64` architecture layer.** The third implementation of the `arch/` contract (milestone
//! 161, notes/x86-port.md), and the one milestone 20 said would be the real test of it: the first
//! two architectures are both RISC machines with a device tree, weak memory and a similar MMU, and
//! this one is none of those things.
//!
//! notes/x86-port.md says what runs here, how to build, boot and test it, and what had to change
//! above `arch/`; its appendix what-does-not-fit.md records where the seam did not stretch and the
//! two names calef ratified for it, `arch::cpu_start` and `boot_info_pointer`.

use core::arch::{asm, global_asm};

// AMD's IOMMU (lane `amd-vi`), beside VT-d's `iommu`, which forwards to it on an AMD machine.
pub mod amd_vi;
// The AP real-mode trampoline's copy-and-prepare step (milestone 161's SMP item). See its own
// header, and `boot.s`'s `secondary_boot` for what it prepares.
pub mod ap_boot;
pub mod context;
// What the loader and ACPI describe, read once inside `memory::init` (milestone 890 (riscv64
// and `x86_64` join the shared boot path)).
pub mod discovery;
pub mod exceptions;
pub mod fp;
mod instructions;
pub mod interrupts;
pub mod iommu;
pub mod irq;
pub mod isa;
// The NMI, double-fault and machine-check stacks the TSS's IST names: milestone 915 (NMI, double
// fault and machine check run on their own stacks), provisional.
pub mod ist;
// What the loader said (milestone 161): the kernel side of `machine_discovery::x86_64`.
pub mod machine;
pub mod mmu;
// Unhalted core cycles (milestone 309), the x86_64 half of milestone 74's measurement side. It is
// here rather than folded into `timer` because it is a different counter answering a different
// question: `timer::now()` is `rdtsc`, a constant-rate clock, and this is what the core actually
// ran. Its own header has the table of all three architectures' two counters.
pub mod pmu;
pub mod port;
pub mod reset;
pub mod rtc;
pub mod segments;
pub mod semihosting;
// The Intel TCO watchdog (milestone 593 (a wedged kernel resets itself), provisional number). Only
// the watchdog soak drives it, so only that build compiles it; see its own header.
#[cfg(soak_watchdog)]
pub mod tco;
pub mod thread_pointer;
pub mod timer;
/// The TSC calibration's estimator and its two boot assertions. Test-only; see the file header for
/// why they run under QEMU rather than on the host.
#[cfg(test)]
mod timer_calibration_tests;
/// The TSC-vs-RTC measurement instrument. Off by default; see notes/tsc-under-tcg.md.
#[cfg(feature = "tsc_probe")]
pub mod tsc_probe;

// The saved thread context and how a new one is faked (the Rust half of context.s). Re-exported
// flat so `crate::arch::{Context, switch_to}` names them regardless of architecture.
pub use context::{Context, switch_to};
// How the console reaches its UART's registers on this architecture. Named flat through `arch`
// because `console.rs` picks it by `target_arch` and must not reach into `arch::x86_64::` directly.
pub use port::PortIo;
/// The arch contract for a kernel-initiated cold reboot (milestone 249 (the boot lottery is sampled by a person walking to the board)). See [`reset::reboot`].
pub use reset::reboot;

mod assembly;
mod bring_up;
mod cpu_init;
mod msr;
mod percpu;
mod primitives;

pub use bring_up::*;
pub use cpu_init::*;
pub use msr::*;
pub use percpu::*;
pub use primitives::*;
