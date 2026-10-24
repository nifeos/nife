//! **A user program cannot read the kernel's *code*, not only a kernel data static** (a ninth
//! outsider pass at the confinement claim, claim 21; DECISIONS §19 (architectural parity is a
//! tenet)).
//!
//! `el0_boundary_tests::a_user_program_cannot_read_a_kernel_address` reads a kernel data static;
//! this reads kernel text, where the gadgets are, at the address of a kernel function this build
//! runs (mapped, and no per-ISA constant to keep). The fault must be a permission fault: a
//! translation fault would prove only that the page was absent. aarch64 reads it from `ESR_EL1`;
//! riscv64 walks the tables (its caveat is in `riscv64::exceptions::user_fault`).
//!
//! Name: provisional (lane/pass-9-tests, 2026-10-14 UTC), for the module, the file and each
//! function. Cross-ISA: it runs wherever the `kernel_test_subject` is packed, which is every target
//! with an initial ramdisk (DECISIONS §19); on x86_64 the fault arrives as a page fault whose error
//! code says a user-mode read of a present page.

use core::sync::atomic::Ordering;

use super::test_programs::{kernel_test_subject_image, spawn_bare};
use super::*;
use crate::arch::exceptions::{USER_FAULTS, last_user_fault};
use crate::arch::{UserFault, UserFaultAccess};

/// The machine-code address of a kernel function this build runs, rounded down to eight bytes: a
/// mapped text page, denied to EL0, with no magic constant to keep current.
///
/// The eight-byte alignment is load-bearing on riscv64. The `kernel_test_subject` reads with an
/// eight-byte load, a function entry is only two- or four-byte aligned, and a misaligned load traps
/// (`scause` 4) before any permission check, as `UserFault::Other`. Rounding down stays in the same
/// text page (a function never begins within seven bytes of a page end), so the fault is the
/// boundary's, not the bus's. The aligned `u64` static `el0_boundary_tests` reads never hit this.
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
/// `kernel_test_subject` reads the address handed to it in its second argument register, faults,
/// and is killed; the kernel records a permission fault on a read at exactly that address, and the
/// kernel is still executing the assertions below, which is the other half of the claim.
///
/// Falsification: unfalsified. Written by the ninth outsider pass's reviewer, who booted it
/// green but did not break the code it guards to watch it go red.
#[test_case]
fn a_user_program_cannot_read_a_kernel_code_address() {
    let kernel_text = a_kernel_text_address();

    assert!(
        mmu::translate(kernel_text).is_some(),
        "the kernel's own text is not mapped at this address, so this test proves nothing",
    );

    let before = USER_FAULTS.load(Ordering::Relaxed);

    spawn_bare(
        kernel_test_subject_image(),
        KERNEL_TEST_SUBJECT_READ_KERNEL,
        kernel_text,
    )
    .expect("spawn failed");

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
