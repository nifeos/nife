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

/// The `interrupt_ignorer` program's ELF bytes: a `_start` that is nothing but a loop. It was built
/// for the shell's forcible-interrupt tier (DECISIONS §24), and it is exactly the hostile binary
/// DECISIONS §5 describes, so the preemption test uses it rather than a second copy of the same
/// idea.
fn interrupt_ignorer_image() -> &'static [u8] {
    program("interrupt_ignorer").expect("no interrupt_ignorer program in the initial ramdisk")
}

/// **We are running on `SP_EL1`, and the whole trap frame depends on it.**
///
/// At EL1 the name `sp` means `SP_EL1` if `SPSel.SP == 1`, and `SP_EL0` if it is 0. Every
/// `SAVE_CONTEXT` in the kernel does `sub sp, sp, #272` and every user entry does
/// `msr sp_el0, x3`. If `SPSel` were 0, those two would be **the same register**, and the
/// kernel would restore a user stack pointer straight into its own stack pointer.
///
/// This has been true since boot.s and we never checked it. A test that can only fail if
/// the world is upside down is still worth having when the failure is silent.
///
/// **aarch64-only, and there is no RISC-V analogue to write.** RISC-V does not bank the stack
/// pointer by privilege level at all: there is one `sp`, and the kernel swaps it with `sscratch`
/// on the way into a trap. The hazard this test guards (two names for one register, silently)
/// cannot exist there, so a RISC-V twin would have nothing to assert. See
/// notes/riscv-parity-scope.md.
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

/// User mode. The boundary.
///
/// Two syscalls, not one, and that is the point: the second can only happen if the return from
/// the first **put us back at EL0/U-mode**. One proves we left. Two prove we came back.
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

/// **The privilege boundary is real, and it is a PERMISSION fault, not a missing page.**
///
/// The address the user reaches for is mapped, and readable, and the kernel reads it all day.
/// The precondition assertions below say exactly that before the program is even started,
/// because without them the test is vacuous: "userspace cannot read this" proves nothing about
/// a page nobody mapped.
///
/// So a **permission** fault rather than a translation fault is the whole assertion. A
/// translation fault would mean we had merely failed to map something, which would pass a
/// sloppier test and prove nothing at all. Both ISAs assert it; only aarch64 is *told* it (see
/// `arch::UserFault`, and the BUGS note on the RISC-V classifier).
///
/// Falsification: replayable `system_tests/falsifications/user.el0_boundary_tests.a_user_program_cannot_read_a_kernel_address.patch`
#[test_case]
fn a_user_program_cannot_read_a_kernel_address() {
    let kernel_addr = a_kernel_address();

    // The precondition, and it is what gives the assertion below its teeth.
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

    // And the kernel is executing this line, which is the other half of the claim.
}

/// DECISIONS §5's arbitrary binary, at user mode, in the flesh.
///
/// A program with no yield, no syscall, and not even a function call: `interrupt_ignorer`'s whole
/// `_start` is a loop. The **only** thing in the universe that can take the CPU back from it is a
/// timer interrupt landing between two of its instructions. Milestone 6 proved this for a kernel
/// thread we compiled. This is the case that actually mattered.
#[test_case]
fn a_user_program_that_never_yields_is_preempted_anyway() {
    let preemptions = sched::preemptions();
    let faults = USER_FAULTS.load(Ordering::Relaxed);

    let interrupt_ignorer = spawn_bare(interrupt_ignorer_image(), 0, 0).expect("spawn failed");

    // Give it the CPU and then take it back, without asking.
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

    // And we are here, running, having taken the CPU back from a program that never
    // offered it.

    // Now take it back for good. `interrupt_ignorer` never yields, never syscalls and never
    // returns, so nothing but an armed kill ends it, and leaving it running would spend a core for
    // the rest of the suite. The assertions above are already done, so the kill cannot weaken them.
    assert!(
        reap_bare(interrupt_ignorer),
        "the spinning user thread outlived its kill"
    );
}

/// **The trap frame is where the trap path will look for it** (milestone 71).
///
/// The user-entry path writes a `TrapFrame` and the trap path rebuilds one, and the whole privilege
/// boundary rests on those two being the *same address*: `stack_top - size_of::<TrapFrame>()`. That
/// is an agreement between Rust and assembly, on two ISAs, and nothing checked it. It was wrong on
/// RISC-V for a year: the frame was computed from the live `sp` instead, which put it 16 bytes under
/// where `trap.s` builds an S-mode frame, so any interrupt in the window rewrote it. The symptom was
/// a thread dispatched to U-mode with a zero entry point, intermittently, only ever on CI.
///
/// So read that address and wait for this thread's U-mode PC to appear in it. `interrupt_ignorer`
/// is the subject because it never syscalls and never returns: anything that lands in its frame got
/// there by the timer preempting it at EL0, which is the agreement under test. A frame built
/// somewhere else never shows up here and the wait times out, which is precisely what the old
/// RISC-V placement would do.
///
/// **The window is checked, not just the value, and the test learned that the hard way.** Its first
/// form waited for a nonzero word and then asserted it was a user address, and it failed on RISC-V
/// against a correct kernel: on the exec path this thread was a *kernel* thread first, and the
/// frames of `thread_entry` and `run` occupy `[top - size, top)` until the user frame is written
/// over them. So "nonzero" was satisfied by a live kernel frame pointer before user entry had
/// happened at all. The condition has to name the U-mode text range, which every user program is
/// linked into (`dump_threads` records why that base is shared).
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

/// **The question the kernel must ask, asked of the hardware.**
///
/// `AT S1E0R` means *translate this address as EL0 would, for a read*. One instruction, and
/// it is the difference between a kernel and a confused deputy.
///
/// Note the precondition assertion. Without it the test is vacuous: "EL0 cannot read the
/// kernel's text" proves nothing if the kernel's text is not mapped in the first place.
///
/// **aarch64-only because it already has a RISC-V twin, not because RISC-V cannot ask.**
/// `riscv_virtio_tests::the_page_tables_say_u_mode_cannot_read_the_kernels_memory` asserts the
/// same three things there. They are deliberately separate tests rather than one portable one,
/// because the *mechanism* is what each is about: this one asks the silicon a question
/// (`AT S1E0R`), and RISC-V has no such instruction, so its twin walks the tables in software
/// and reads the `U` bit. Merging them would mean asserting only what both can say, which is
/// less than either says now.
///
/// Falsification: replayable `system_tests/falsifications/user.el0_boundary_tests.the_hardware_says_el0_cannot_read_the_kernels_memory.patch`
#[cfg(target_arch = "aarch64")]
#[test_case]
fn the_hardware_says_el0_cannot_read_the_kernels_memory() {
    const KERNEL_TEXT: u64 = 0xffff_0000_4008_0000;

    let (space, _) = load(loader_subject_image(), 0).expect("the initial ramdisk did not load");

    space.while_installed(|| {
        // The precondition, and it is what gives the assertion below its teeth: that address IS
        // mapped, and the KERNEL can read it. It reads it all day.
        assert!(
            mmu::translate(KERNEL_TEXT).is_some(),
            "the kernel's text is not mapped, so this test proves nothing",
        );

        // And EL0 cannot. Not "we decline to"; the silicon says no.
        assert!(
            !mmu::user_can_read(KERNEL_TEXT),
            "the hardware says EL0 could read the kernel's own text",
        );
        assert!(!mmu::user_can_write(KERNEL_TEXT));

        // It can read its own code, or the check is a rubber stamp that says no to everything.
        assert!(
            mmu::user_can_read(address_space_map::IMAGE_BASE),
            "EL0 cannot read its own .text, so the check refuses everything and proves nothing",
        );

        // And not an address in its own half that nobody mapped: the stack band's guard page, which the
        // address-space map promises no loader ever maps.
        assert!(!mmu::user_can_read(address_space_map::STACK.start));
    });
    drop(space);
}
