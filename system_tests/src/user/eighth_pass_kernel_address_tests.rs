//! **A user program cannot read the kernel through its other addresses either** (an eighth
//! outsider pass at the confinement claim, claim 21).
//!
//! The cited test reads one kernel static through its link-time virtual address. This module aims
//! the same `kernel_test_subject` at two further kernel views of kernel memory that a real attacker
//! would reach for first: the kernel's own code (a function pointer, the text half) and the
//! **direct map** (the kernel's writable alias of physical memory, where the same sentinel page
//! appears a second time). The claim is per-ISA (§19 (architectural parity is a tenet)), so both
//! targets are driven on every architecture the suite boots.
//!
//! The precondition asserts each target is mapped for the kernel (`mmu::translate`), so a refusal
//! to fault cannot hide behind "the address was unmapped anyway".

use super::test_programs::{a_kernel_address, kernel_test_subject_image, spawn_bare};
use super::*;
use crate::arch::exceptions::last_user_fault;
use crate::arch::{UserFault, UserFaultAccess};
use crate::sched;

/// A kernel function whose address is a legitimate piece of kernel text.
fn kernel_text_marker() {}

/// The two fault shapes a kernel-address read can take, per the table layouts named above.
fn is_read_fault(kind: UserFault) -> bool {
    matches!(
        kind,
        UserFault::Permission(UserFaultAccess::Read)
            | UserFault::Translation(UserFaultAccess::Read)
    )
}

/// **The `kernel_test_subject`, aimed at kernel text and at the direct map, faults on both.**
///
/// Three targets in one boot: the sentinel static (the cited test's, kept so this module's faults
/// cannot all be about a mapping unique to this file), a function in the kernel's text, and the
/// direct-map alias of the sentinel's own physical page. The last is the sharpest: the page is the
/// very one the cited test proves unreadable at its link-time address, and the direct map is the
/// kernel's other name for it.
///
/// The observation is the `kernel_test_subject`'s death plus the recorded fault, read with bounded
/// retries because `last_user_fault` is one global slot on a loaded machine: an unrelated user
/// thread's non-page fault records itself with address 0 and can overwrite the record between the
/// fault and the read. A retry re-spawns and re-reads; the escape signal is different and
/// unambiguous, being a `kernel_test_subject` that never dies because its read never faulted.
///
/// The fault kind is `Permission(Read)` or `Translation(Read)`: the two table layouts (the page
/// present but supervisor-only, and the page absent from the low half entirely) split across
/// targets and ISAs, and what the claim requires is that each read was stopped as a read at the
/// exact address handed in.
///
/// Falsification: unfalsified. Written by the eighth outsider pass's reviewer, who booted it
/// green but did not break the code it guards to watch it go red.
#[test_case]
fn a_user_program_cannot_read_kernel_text_or_the_direct_map() {
    let sentinel = a_kernel_address();
    // Aligned down into the same page: `read_volatile`'s alignment precondition panics before
    // the load on an odd function address, and the fault this test exists for would never run.
    let text = kernel_text_marker as *const () as usize as u64 & !0x7;
    let direct_map = mmu::phys_to_virt(mmu::virt_to_phys(sentinel));
    assert!(
        mmu::translate(sentinel).is_some() && mmu::translate(text).is_some(),
        "the kernel-half targets are not mapped for the kernel, so a user read proving nothing \
         about the boundary was about to run",
    );

    // The direct-map alias is a target only where the kernel's own walk names it. On x86_64 the
    // direct map does not cover every physical page the kernel image occupies (a kernel-side
    // probe of this one faulted), and `translate` cannot name its huge pages either way, so there
    // is no alias there to attack through and the honest assert is that the walk finds nothing.
    let mut targets = [sentinel, text, sentinel];
    let mut count = 2;
    if mmu::translate(direct_map).is_some() {
        targets[2] = direct_map;
        count = 3;
    } else {
        assert_eq!(mmu::translate(direct_map), None);
    }

    for &target in &targets[..count] {
        let mut recorded = None;
        for _ in 0..3 {
            let kernel_test_subject = spawn_bare(
                kernel_test_subject_image(),
                KERNEL_TEST_SUBJECT_READ_KERNEL,
                target,
            )
            .expect("no kernel_test_subject thread");
            assert!(
                super::wait_for(|| !sched::is_thread_present(kernel_test_subject)),
                "the kernel_test_subject reading {target:#x} never died: its read was not stopped, and a user \
                 program read the kernel's memory",
            );
            if let Some((kind, addr)) = last_user_fault()
                && addr == target
                && is_read_fault(kind)
            {
                recorded = Some((kind, addr));
                break;
            }
        }
        let (kind, addr) = recorded.unwrap_or_else(|| {
            panic!(
                "no user fault was recorded as a read at {target:#x} in three attempts: the \
                 record was overwritten by unrelated faults, or the read was not stopped"
            )
        });
        assert_eq!(addr, target);
        assert!(
            is_read_fault(kind),
            "the fault on {target:#x} was {kind:?} rather than a fault on a read",
        );
    }
}
