//! **The assembly this architecture links**: the boot entry, the context switch, the `FXSAVE`
//! area and the trap stubs, with the Rust constants the trap stubs read substituted in.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

// The 32-bit entry (_start), the long-mode transition, the .bss zeroing, and the stack handoff to
// `kernel_main`.
global_asm!(include_str!("boot.s"));

// The context switch and the two first-run trampolines (the asm half of context.rs).
global_asm!(include_str!("context.s"));

// Saving and restoring the `FXSAVE` area (milestone 447 (a thread's vector registers are its own)).
// Separate from context.s because it moves
// a register file rather than a calling convention's callee-saved set; see fp.rs.
global_asm!(include_str!("fp.s"));

// The 256 trap stubs, the shared restore path, the `system_call` entry, and the door into ring 3
// (the asm half of exceptions.rs). The constants are substituted rather than duplicated: a 64-bit
// assembler cannot read a Rust `const`, and the alternative is two files that can drift. The three
// `*_OFF` ones are `gs`-relative byte offsets into `cpu::PerCpu::x86_trap` (milestone 161's SMP
// item), computed by `core::mem::offset_of!` rather than hand-counted, so a field added or reordered
// in `cpu.rs` cannot silently desynchronize the assembly that reaches through them.
global_asm!(
    include_str!("trap.s"),
    USER_CODE = const segments::USER_CODE as u64,
    USER_DATA = const segments::USER_DATA as u64,
    SYSTEM_CALL_VECTOR = const exceptions::SYSTEM_CALL_VECTOR,
    TSS_RSP0_PTR_OFF = const core::mem::offset_of!(crate::cpu::PerCpu, x86_trap.tss_rsp0_ptr),
    SYSTEM_CALL_KERNEL_RSP_OFF =
        const core::mem::offset_of!(crate::cpu::PerCpu, x86_trap.system_call_kernel_rsp),
    SYSTEM_CALL_USER_RSP_OFF =
        const core::mem::offset_of!(crate::cpu::PerCpu, x86_trap.system_call_user_rsp),
);
