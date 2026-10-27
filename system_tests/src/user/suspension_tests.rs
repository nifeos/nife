//! `suspension_tests`: `ThreadControlBlock::SUSPEND` and `RESUME` from the syscall boundary, on a
//! user thread, on all three architectures (DECISIONS §300 (the paused state, a TCB verb, silent)).
//!
//! The kernel's own tests (`kernel/src/sched/suspension.rs`) prove the scheduler half on kernel
//! threads. These prove what only a user thread can show: the methods are reached through
//! `SYS_INVOKE` on a TCB capability and gated on its `WRITE`, a thread stopped at EL0 or U-mode
//! comes back with its registers, and a stopped thread's region can still be destroyed, which is
//! the shell's `kill -9` on a stopped job.
//!
//! Name: provisional (lane/48-tcb-suspend-resume, 2026-10-18 UTC).

use abi::Error;

use super::*;
// The `cap` module still carries its pre-sweep names (design/naming/capability-spelling-worklist.md);
// aliased so this file spells `capability` out (calef, 2026-10-06).
use crate::cap::{Cap as Capability, Object, Rights};
use crate::sched::{self, delete_current_cap as delete_current_capability};
use crate::syscall::invoke;

const CODE_VA: u64 = address_space_map::IMAGE_BASE;
const STACK_VA: u64 = address_space_map::STACK_TOP_PAGE;
/// The spinner's data page: word 0 is its count, word 1 how many times it has been entered.
const VA: u64 = address_space_map::pair_page(0x0060_0000);

/// **The spinner.** Entry `x0` is its data page. It counts its own entries in word 1, then counts in
/// a callee-saved register from zero and stores the register to word 0 after every step, forever.
/// The count lives in the register, so a resume that lost the registers shows as the count falling.
#[cfg(target_arch = "aarch64")]
const SPINNER: &[u32] = &[
    0xF940_0405, // ldr  x5, [x0, #8]
    0x9100_04A5, // add  x5, x5, #1
    0xF900_0405, // str  x5, [x0, #8]
    0xD280_0013, // mov  x19, #0
    0x9100_0673, // loop: add x19, x19, #1
    0xF900_0013, // str  x19, [x0]
    0x17FF_FFFE, // b    loop
];
#[cfg(target_arch = "riscv64")]
const SPINNER: &[u32] = &[
    0x0085_3283, // ld   t0, 8(a0)
    0x0012_8293, // addi t0, t0, 1
    0x0055_3423, // sd   t0, 8(a0)
    0x0000_0913, // li   s2, 0
    0x0019_0913, // loop: addi s2, s2, 1
    0x0125_3023, // sd   s2, 0(a0)
    0xFF9F_F06F, // j    loop
];
/// `inc qword [rdi + 8]; xor r12d, r12d; loop: inc r12; mov [rdi], r12; jmp loop`, then one `nop`
/// to a word. Packed the way `user::x86_programs` packs its listings.
#[cfg(target_arch = "x86_64")]
const SPINNER: &[u32] = &[0x0847_FF48, 0x49E4_3145, 0x894C_C4FF, 0x90F8_EB27];

fn capability(object: Object, rights: Rights) -> Capability {
    Capability { object, rights }
}

fn call(slot: u64, method: u64, a: [u64; 3]) -> Result<i64, Error> {
    let mut frame = TrapFrame::for_user_entry(0, 0, [0, 0, 0]);
    invoke(&mut frame, slot, method, a[0], a[1], a[2])
}

fn word(page: u64, i: usize) -> u64 {
    // SAFETY: the spinner's own data frame, through the direct map; only the spinner writes it.
    unsafe { core::ptr::read_volatile((mmu::phys_to_virt(page) as *const u64).add(i)) }
}

fn wait_for(seconds: u64, mut cond: impl FnMut() -> bool) -> bool {
    let deadline = crate::arch::timer::now() + seconds * crate::arch::timer::frequency();
    while crate::arch::timer::now() < deadline {
        if cond() {
            return true;
        }
        sched::yield_now();
    }
    cond()
}

/// Yield for `1 / divisor` of a second.
fn let_time_pass(divisor: u64, mut each: impl FnMut()) {
    let until = crate::arch::timer::now() + crate::arch::timer::frequency() / divisor;
    while crate::arch::timer::now() < until {
        each();
        sched::yield_now();
    }
}

/// **A user thread stopped by `SUSPEND` stands still, and `RESUME` continues it with its
/// registers**; a TCB capability without `WRITE` can do neither; and a stopped thread's region is
/// destroyed the way a running one's is.
///
/// Every observation is of the spinner's data page. Its count frozen across fifty milliseconds is
/// the stop. After `RESUME` the count never reads below the frozen value and climbs past it, and the
/// entry count stays one: a re-run from the entry, or a resume that lost the register the count
/// lives in, would each restart it from zero.
///
/// Falsification: replayable `system_tests/falsifications/user.suspension_tests.a_suspended_user_thread_stands_still_and_resumes_with_its_registers.patch`
#[test_case]
fn a_suspended_user_thread_stands_still_and_resumes_with_its_registers() {
    let region = crate::memory_region::create(16).expect("no region");
    let name = user_address_space_create(region).expect("no address space");
    let none = crate::revoke::PageMapSource::NoCapability;
    let code = code_page(region, SPINNER);
    user_address_space_map(name, CODE_VA, code, Flags::user_code(), none).expect("map code");
    let stack = crate::memory_region::retype_page(region).expect("no stack frame");
    user_address_space_map(name, STACK_VA, stack, Flags::user_data(), none).expect("map stack");
    let page = crate::memory_region::retype_page(region).expect("no data frame");
    // SAFETY: a frame retyped for this test alone, named through the direct map.
    unsafe { core::ptr::write_bytes(mmu::phys_to_virt(page) as *mut u64, 0, 2) };
    user_address_space_map(name, VA, page, Flags::user_data(), none).expect("map the data");

    let given = sched::grant(capability(Object::AddressSpace(name), Rights::WRITE)).expect("grant");
    let tid = sched::create_thread_control_block(region).expect("no tcb");
    let tcb =
        sched::grant(capability(Object::ThreadControlBlock(tid), Rights::ALL)).expect("grant");
    let read_only =
        sched::grant(capability(Object::ThreadControlBlock(tid), Rights::READ)).expect("grant");
    call(
        tcb,
        abi::thread_control_block::CONFIGURE,
        [CODE_VA, STACK_VA + page_frames::FRAME_SIZE, given],
    )
    .expect("CONFIGURE");
    sched::start_thread_control_block(tid, [VA, 0, 0]).expect("start");
    assert!(
        wait_for(5, || word(page, 0) > 1000),
        "the spinner never ran"
    );
    // Long enough that a count restarted from zero would still be below the frozen value when the
    // first sample after `RESUME` is taken.
    let_time_pass(10, || {});

    for method in [
        abi::thread_control_block::SUSPEND,
        abi::thread_control_block::RESUME,
    ] {
        assert_eq!(
            call(read_only, method, [0; 3]),
            Err(Error::NotPermitted),
            "method {method} obeyed a TCB capability without WRITE",
        );
    }

    assert_eq!(call(tcb, abi::thread_control_block::SUSPEND, [0; 3]), Ok(0));
    let frozen = word(page, 0);
    let_time_pass(20, || {});
    assert_eq!(word(page, 0), frozen, "a suspended thread went on counting");

    assert_eq!(call(tcb, abi::thread_control_block::RESUME, [0; 3]), Ok(0));
    let mut lowest = u64::MAX;
    let_time_pass(50, || lowest = lowest.min(word(page, 0)));
    assert!(
        lowest >= frozen,
        "the count fell to {lowest} from {frozen}: RESUME lost the thread's registers",
    );
    assert!(
        wait_for(5, || word(page, 0) > frozen + 1000),
        "a resumed thread did not run on"
    );
    assert_eq!(word(page, 1), 1, "RESUME re-ran the thread from its entry");

    // `kill -9` on a stopped job: the region comes back with its thread suspended, not running.
    assert_eq!(call(tcb, abi::thread_control_block::SUSPEND, [0; 3]), Ok(0));
    let _ = delete_current_capability(tcb);
    let _ = delete_current_capability(read_only);
    assert!(
        wait_for(5, || sched::reclaim_region(region).is_ok()),
        "the region of a suspended thread did not come back",
    );
    assert!(
        !sched::is_thread_present(tid),
        "the suspended thread outlived its region"
    );
}
