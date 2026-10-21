//! Device drivers.
//!
//! A driver never reaches into a kernel global. It is handed what it needs
//! (a base address, later a DMA allocator, later an interrupt registration) and
//! knows nothing about the rest of the kernel. That rule is cheap now and is what
//! keeps the microkernel door open later. See DECISIONS §4.

// The GIC interrupt-controller driver, aarch64's. Gated now (milestone 20): portable code names
// `arch::irq`, not `drivers::gic`, so the only things that reach the GIC are aarch64 arch code
// (exceptions, timer) and the aarch64 `arch::irq` adapter. RISC-V's analog is `plic`. This gating is
// the last of the HAL leaks closed: a new ISA is a new `arch/` directory, not a diff across the
// kernel (DECISIONS §4, §17). aarch64 IRQ tests in portable files stay `cfg(test)`, aarch64-only.
#[cfg(target_arch = "aarch64")]
pub mod gic;
// Its GICv3 sibling (milestone 227): the distributor and redistributors, memory-mapped. The GICv3
// CPU interface is system registers and so lives in `arch/aarch64/gic_cpu_interface.rs`; the
// aarch64 `arch::irq` adapter picks the version at boot and is the only caller of either.
#[cfg(target_arch = "aarch64")]
pub mod gicv3;

// The PL011 UART, aarch64's `virt` console. Used only by the console (via a compile-time alias), so
// it gates cleanly. RISC-V's `virt` has an NS16550 instead, and so does x86 (at an I/O port rather
// than a memory address, which is a `RegisterSpace` in that driver rather than a second driver).
#[cfg(any(target_arch = "riscv64", target_arch = "x86_64"))]
pub mod ns16550;
#[cfg(target_arch = "aarch64")]
pub mod pl011;

// The PLIC, RISC-V's interrupt controller (milestone 20). Gated to riscv: it is the PLIC analog of
// the aarch64-only device-interrupt path. Pure MMIO Rust; takes its base and context from the caller.
#[cfg(target_arch = "riscv64")]
pub mod plic;

// The JH7110's clock and reset generator (milestone 220). Gated to riscv64: the JH7110 is a
// RISC-V SoC, so on the other two architectures the module would be dead code naming a device
// that cannot be present. Pure MMIO Rust over `crates/jh7110_clock_and_reset`'s offsets; takes its base and
// its plan from the caller and holds no state of its own. Its header carries the argument for why
// this one driver is kernel-resident when §21 and §23 put drivers at EL0.
#[cfg(target_arch = "riscv64")]
pub mod jh7110_clock_and_reset;

// The JH7110's watchdog (milestone 593 (a wedged kernel resets itself), provisional), only in a
// build whose soak arms it: the soak's supervisor is its one caller. Pure MMIO over
// `crates/jh7110_watchdog`'s sequences; takes its base from the caller.
#[cfg(all(target_arch = "riscv64", soak_watchdog))]
pub mod jh7110_watchdog;

// **QEMU's `ramfb`**, milestone 243 (a machine with no serial port): the guest supplies the pixels
// and `fw_cfg` tells the emulator where they are. Not gated by architecture, because the question
// it answers is "does this machine have a display the kernel can write to", and the two `virt`
// boards answer yes while `x86_64` answers with a real UEFI aperture instead.
// `kernel/src/screen.rs` is its one caller and owns the memory; this file holds only the register
// poking, and takes its base address from the caller.
pub mod ramfb;
