//! `el0_boundary_tests`: the boundary between user mode and the kernel: a program reaches EL0 and
//! comes back, cannot read a kernel address (and the hardware agrees), is preempted when it never
//! yields, and traps into a frame where the trap path rebuilds it.
//!
//! Name: provisional, minted 2026-10-12 (UTC) by the lane for milestone 873 (the catch-all
//! userspace test file is split by topic).

use core::sync::atomic::Ordering;

#[cfg(target_arch = "aarch64")]
use super::test_programs::loader_subject_image;
use super::test_programs::{a_kernel_address, kernel_test_subject_image, reap_bare, spawn_bare};
use super::*;
use crate::arch::exceptions::{SVC_COUNT, USER_FAULTS, last_user_fault};
use crate::arch::{UserFault, UserFaultAccess, timer};
use crate::sched;

/// The `interrupt_ignorer` program's ELF bytes: a `_start` that is nothing but a loop. Built for
/// the shell's forcible-interrupt tier (DECISIONS §24), it is the hostile binary DECISIONS §5
/// describes.
fn interrupt_ignorer_image() -> &'static [u8] {
    program("interrupt_ignorer").expect("no interrupt_ignorer program in the initial ramdisk")
}

/// The kernel runs on `SP_EL1`, and the whole trap frame depends on it.
///
/// At EL1, `sp` names `SP_EL1` when `SPSel.SP == 1` and `SP_EL0` when it is 0. Every `SAVE_CONTEXT`
/// does `sub sp, sp, #272` and every user entry does `msr sp_el0, x3`; with `SPSel` at 0 those are
/// one register, and the kernel would restore a user stack pointer into its own, silently.
///
/// aarch64-only: RISC-V has one `sp`, swapped with `sscratch` on trap entry, so the hazard cannot
/// exist there and a twin would assert nothing. See notes/riscv-parity-scope.md.
#[cfg(target_arch = "aarch64")]
#[test_case]
fn el1_runs_on_sp_el1() {
    let spsel = crate::arch::spsel();

    assert_eq!(
        spsel & 1,
        1,
        "SPSel says EL1 is using SP_EL0: the trap frame's sp_el0 field aliases the \
         kernel's own stack pointer"
    );
}

/// A user program reaches EL0/U-mode and returns, twice.
///
/// Two syscalls, because the second can happen only if the return from the first put the program
/// back in user mode: one proves it left, two prove it came back.
#[test_case]
fn a_user_program_reaches_el0_and_returns_twice() {
    let before = SVC_COUNT.load(Ordering::Relaxed);

    spawn_bare(
        kernel_test_subject_image(),
        KERNEL_TEST_SUBJECT_ROUND_TRIP,
        0,
    )
    .expect("spawn failed");

    assert!(
        wait_for(|| SVC_COUNT.load(Ordering::Relaxed) >= before + 2),
        "saw {} syscalls from user mode, wanted 2",
        SVC_COUNT.load(Ordering::Relaxed) - before,
    );
}

/// A user read of a kernel address takes a permission fault, not a translation fault.
///
/// The precondition asserts the address is mapped and the kernel reads it, since "userspace cannot
/// read this" proves nothing about an unmapped page. A permission fault is the whole assertion: a
/// translation fault would mean only that something went unmapped. Both ISAs assert it; only
/// aarch64 is told it (`arch::UserFault`, and the BUGS note on the RISC-V classifier).
///
/// Falsification: replayable `system_tests/falsifications/user.el0_boundary_tests.a_user_program_cannot_read_a_kernel_address.patch`
#[test_case]
fn a_user_program_cannot_read_a_kernel_address() {
    let kernel_addr = a_kernel_address();

    // The precondition, without which the fault proves nothing.
    assert!(
        mmu::translate(kernel_addr).is_some(),
        "the kernel's own static is not mapped, so this test proves nothing",
    );

    let before = USER_FAULTS.load(Ordering::Relaxed);

    spawn_bare(
        kernel_test_subject_image(),
        KERNEL_TEST_SUBJECT_READ_KERNEL,
        kernel_addr,
    )
    .expect("spawn failed");

    assert!(
        wait_for(|| USER_FAULTS.load(Ordering::Relaxed) > before),
        "the user program read a kernel address and was NOT stopped",
    );

    let (kind, addr) = last_user_fault().expect("the kernel recorded no user fault");

    assert_eq!(
        kind,
        UserFault::Permission(UserFaultAccess::Read),
        "not a PERMISSION fault on a read: a translation fault would mean we had merely \
         failed to map something, which proves nothing about the privilege boundary",
    );
    assert_eq!(addr, kernel_addr, "faulted on the wrong address");
}

/// DECISIONS §5's arbitrary binary, at user mode: a program that never yields is preempted.
///
/// `interrupt_ignorer`'s whole `_start` is a loop, with no yield, syscall or call, so only a timer
/// interrupt between two of its instructions can take the CPU back.
#[test_case]
fn a_user_program_that_never_yields_is_preempted_anyway() {
    let preemptions = sched::preemptions();
    let faults = USER_FAULTS.load(Ordering::Relaxed);

    let interrupt_ignorer = spawn_bare(interrupt_ignorer_image(), 0, 0).expect("spawn failed");

    timer::spin_for(timer::frequency() / 10);

    assert!(
        sched::preemptions() > preemptions,
        "nothing was preempted while a user thread spun at EL0",
    );
    assert_eq!(
        USER_FAULTS.load(Ordering::Relaxed),
        faults,
        "the spinning user thread faulted; it was supposed to just spin",
    );

    // Only an armed kill ends it, and a survivor would spend a core for the rest of the suite.
    assert!(
        reap_bare(interrupt_ignorer),
        "the spinning user thread outlived its kill"
    );
}

/// The user-entry path writes the trap frame where the trap path rebuilds it (milestone 71).
///
/// Rust and assembly on each ISA must agree the frame is at `stack_top - size_of::<TrapFrame>()`.
/// `interrupt_ignorer` never syscalls, so a PC in that frame got there by the timer preempting it
/// in user mode; a frame built elsewhere (a live-`sp` placement 16 bytes under `trap.s`'s S-mode
/// frame) never shows up, and the wait times out. The wait names the U-mode text range, not just
/// "nonzero": on the exec path the thread is a kernel thread first, and the frames of
/// `thread_entry` and `run` hold `[top - size, top)` until user entry overwrites them. Every user
/// program links at that base (`dump_threads` records why).
#[test_case]
fn a_user_threads_trap_frame_sits_where_the_trap_path_rebuilds_it() {
    const USER_TEXT: core::ops::Range<u64> =
        address_space_map::IMAGE.start..address_space_map::IMAGE.end;

    let interrupt_ignorer = spawn_bare(interrupt_ignorer_image(), 0, 0).expect("spawn failed");

    assert!(
        wait_for(|| sched::user_pc_of(interrupt_ignorer).is_some_and(|pc| USER_TEXT.contains(&pc))),
        "this thread's U-mode PC never appeared at stack_top - size_of::<TrapFrame>(), so the \
         user-entry path and the trap path do not agree on where the frame lives (read {:#x})",
        sched::user_pc_of(interrupt_ignorer).unwrap_or(0),
    );

    assert!(
        reap_bare(interrupt_ignorer),
        "the spinning user thread outlived its kill"
    );
}

/// The hardware itself says EL0 cannot read or write the kernel's text.
///
/// `AT S1E0R` translates an address as EL0 would, for a read. The precondition (the text is mapped)
/// keeps the test from being vacuous. aarch64-only because its RISC-V twin,
/// `riscv_virtio_tests::the_page_tables_say_u_mode_cannot_read_the_kernels_memory`, asserts the
/// same three things by walking the tables for the `U` bit; RISC-V has no such instruction, and
/// one portable test could assert only what both mechanisms can say.
///
/// Falsification: replayable `system_tests/falsifications/user.el0_boundary_tests.the_hardware_says_el0_cannot_read_the_kernels_memory.patch`
#[cfg(target_arch = "aarch64")]
#[test_case]
fn the_hardware_says_el0_cannot_read_the_kernels_memory() {
    const KERNEL_TEXT: u64 = 0xffff_0000_4008_0000;

    let (space, _) = load(loader_subject_image(), 0).expect("the initial ramdisk did not load");

    space.while_installed(|| {
        // The precondition: the address is mapped and the kernel can read it.
        assert!(
            mmu::translate(KERNEL_TEXT).is_some(),
            "the kernel's text is not mapped, so this test proves nothing",
        );

        assert!(
            !mmu::user_can_read(KERNEL_TEXT),
            "the hardware says EL0 could read the kernel's own text",
        );
        assert!(!mmu::user_can_write(KERNEL_TEXT));

        // Its own code is readable, or the check refuses everything.
        assert!(
            mmu::user_can_read(address_space_map::IMAGE_BASE),
            "EL0 cannot read its own .text, so the check refuses everything and proves nothing",
        );

        // The stack band's guard page, which the address-space map promises no loader maps.
        assert!(!mmu::user_can_read(address_space_map::STACK.start));
    });
    drop(space);
}
