//! What machine is this? One record per architecture, populated once at boot and printed at boot,
//! and the readers every architecture shares: the core list, interrupt ids, the screen token and
//! ACPI.
//!
//! Each record is `Copy` with public fields, and `missing_requirements` is the one verb a call
//! site branches on. There is no trait. See `notes/isa-discovery.md` for the tiers of discovery
//! and `notes/machine-discovery.md` for the shared readers.

//! # BUGS
//!
//! - Discovery makes the kernel honest, not portable: a missing requirement is reported and the
//!   boot stops.
//! - The device tree can lie. The tier-2 probe exists for that, and on RISC-V it covers one fact,
//!   the ASID width.
//!
//! Name: ratified 2026-08-23 (calef, a kernel-dependency crate naming review). Renamed from `isa`,
//! which collided with the ISA bus and described only part of the scope. Linux ARM's `machine_desc`
//! and FreeBSD's `hw.machine` use "machine" for the same device-tree-plus-probe scope.

#![no_std]
// milestone 68's ratchet is workspace-wide (§107); this crate opts out until its 54-item
// worklist (notes/doc-coverage.md) is burned down.
#![allow(missing_docs)]

// ACPI (milestone 161), and it sits here rather than in the x86_64 module for the reason
// `cpu_list` does: ACPI is not an x86 standard. Every aarch64 server that is not a device-tree
// board describes itself with the same tables, and milestone 20's own text expects the machine
// after the VisionFive 2 to be a UEFI/ACPI one. x86 is its first consumer, not its owner.
pub mod aarch64;
pub mod acpi;
pub mod cpu_list;
// Where the screen is (milestone 243), and it belongs to neither architecture for the same reason
// `cpu_list` does not: the loader asks the firmware where the linear framebuffer is and tells the
// kernel in one command-line token, and milestone 157's U-Boot handoff has the identical sentence
// to say on aarch64. x86_64 is its first consumer, not its owner.
pub mod framebuffer;
// Which Arm interrupt controller (milestone 227): aarch64's question, but a device-tree binding
// read plus a pure cross-check of raw ID-register words, so it is host logic like `plic` beside it.
pub mod gic;
pub mod interrupt_id;
// The JH7110's peripherals, found for the device crates that take plain values (milestone 857
// (crates released on their own)).
pub mod jh7110;
pub mod plic;
pub mod riscv64;
// The third architecture's boot handoff (milestone 161). Not an ISA record like the two beside it:
// x86 answers "what CPU is this" from CPUID and needs no parser for that, but it has no device
// tree, so where RAM is and where the ACPI tables are arrive in the boot protocol's own structure.
// Same reason the RISC-V half is a parser: what the firmware says is a format somebody has to read,
// and a format read only inside a booting kernel is read by nothing that runs in milliseconds.
pub mod x86_64;
