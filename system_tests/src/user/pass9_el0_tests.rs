//! **Pass 9 (an outside security reviewer attacks the confinement claim): a user program cannot
//! read the kernel's *code*, not only a kernel data static** (fatal risk 7's confinement claim,
//! claim 21; DECISIONS §19 (architectural parity is a tenet)).
//!
//! `el0_boundary_tests::a_user_program_cannot_read_a_kernel_address` spawns the `outlaw` at a
//! kernel **data** static and asserts a permission fault. This comes at the same claim from a
//! different page class: the kernel's **text**. A privilege boundary that held for the one rodata
//! sentinel but leaked an executable page would be a real escape, because kernel code is where the
//! gadgets are. The forbidden address is the machine-code address of a kernel function this build
//! is demonstrably executing, so it is mapped and the kernel reads it, and it needs no per-ISA
//! table of text-base constants.
//!
//! The assertion is a **permission** fault, not a translation fault: the page is there, the
//! hardware found it, and the permission bits refused the EL0 read. A translation fault would mean
//! the text merely was not mapped, which proves nothing about the boundary. aarch64 is told this by
//! `ESR_EL1`; riscv64 derives it by walking the tables (its classifier's caveat is in
//! `riscv64::exceptions::user_fault`).
//!
//! Name: provisional (pass 9's lane, 2026-10-13 UTC), for the module, the file and each function.
//! aarch64 and riscv64 only: x86_64 packs no initial ramdisk and builds no `outlaw` ELF, so its
//! ring-3 boundary is driven by the hand-assembled `x86_port_tests` instead.

use core::sync::atomic::Ordering;

use super::test_programs::{outlaw_image, spawn_bare};
use super::*;
use crate::arch::exceptions::{USER_FAULTS, last_user_fault};
use crate::arch::{UserFault, UserFaultAccess};

/// The machine-code address of a kernel function this build runs, rounded down to an eight-byte
/// boundary: it lands in the kernel's text, which is mapped read-execute for the kernel and denied
/// to EL0 entirely. A function item cast to a pointer is its entry address, so this is a real,
/// mapped, kernel-only code page with no magic constant to keep current.
///
/// **The eight-byte alignment is load-bearing on riscv64.** The `outlaw` reaches the address with an
/// eight-byte load. A function entry is only two- or four-byte aligned, and an unaligned eight-byte
/// load on riscv traps as a misaligned access (`scause` 4) *before* any permission check, which the
/// classifier reports as `UserFault::Other`, not the permission fault the privilege boundary is
/// about. Rounding down to eight bytes stays inside the same text page (a function never begins
/// within seven bytes of a page end) and makes the load a clean, aligned read of a kernel-only page,
/// so the fault the test reads is the boundary's and not the bus's. A naturally aligned kernel
/// `u64` static (what `el0_boundary_tests` reads) never hit this; a code address does.
fn a_kernel_text_address() -> u64 {
    // Bind to a fn pointer first (the fn item coerces), then to an integer: a zero-sized fn item
    // cannot be cast to `usize` directly.
    let f: fn() -> Option<(UserFault, u64)> = last_user_fault;
    (f as usize as u64) & !0x7
}

/// **EL0 cannot read a kernel code page.**
///
/// The precondition is what gives the assertion its teeth: that address IS mapped and the kernel
/// can translate it, so a fault on the EL0 read is the boundary refusing, not a missing page. The
/// `outlaw` reads the address handed to it in its second argument register, faults, and is killed;
/// the kernel records a permission fault on a read at exactly that address, and the kernel is still
/// executing the assertions below, which is the other half of the claim.
#[test_case]
fn a_user_program_cannot_read_a_kernel_code_address() {
    let kernel_text = a_kernel_text_address();

    assert!(
        mmu::translate(kernel_text).is_some(),
        "the kernel's own text is not mapped at this address, so this test proves nothing",
    );

    let before = USER_FAULTS.load(Ordering::Relaxed);

    spawn_bare(outlaw_image(), OUTLAW_READ_KERNEL, kernel_text).expect("spawn failed");

    assert!(
        wait_for(|| USER_FAULTS.load(Ordering::Relaxed) > before),
        "the user program read a kernel code address and was NOT stopped",
    );

    let (kind, addr) = last_user_fault().expect("the kernel recorded no user fault");
    assert_eq!(
        kind,
        UserFault::Permission(UserFaultAccess::Read),
        "not a PERMISSION fault on a read of kernel text: a translation fault would mean the text \
         merely was not mapped, which proves nothing about the privilege boundary",
    );
    assert_eq!(addr, kernel_text, "faulted on the wrong address");
}
