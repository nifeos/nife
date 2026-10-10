//! `futex_tests`: `AddressSpace::WAIT` and `WAKE` (milestone 812 (`std::thread::spawn` runs real
//! threads in one address space), §269 (how threads share a process) fork 2).
//!
//! A user thread parks on a word of its own space and sleeps until it is woken; a word that no
//! longer holds what the waiter expected does not sleep; every form §269 reserves is refused; and
//! a waiter whose region is destroyed leaves the futex table. One hand-written waiter per ISA, so
//! the same claims are proved on all three (§19 (architectural parity is a tenet)).
//!
//! # BUGS
//!
//! - **The waker here is the kernel test, not a second user thread.** Two threads cannot share a
//!   space until the join is built (it waits on calef's ruling on pull request #1892), so the wake
//!   is `sched::futex_wake` called directly, past the syscall layer's own-space check. The
//!   syscall layer's checks are proved separately below, by a caller that is refused at each one.
//!   The milestone's exit test, four threads contending a `std::sync::Mutex`, is the end-to-end
//!   proof.

use abi::Error;
use abi::futex::{PRIVATE, SIZE_U32, SIZE_U64};

use super::*;
use crate::cap::Rights;
use crate::sched;
use crate::syscall::invoke;

const CODE_VA: u64 = address_space_map::IMAGE_BASE;
const STACK_VA: u64 = address_space_map::STACK_TOP_PAGE;
/// The page the waiter uses: the futex word at `+0`, a count of returns at `+8`, and at `+16` the
/// OR of every `WAIT` result plus one, so a woken return (`0`) sets bit 0, a changed-word return
/// (`1`) sets bit 1, and any error (negative) sets high bits that cannot be mistaken for either.
const VA: u64 = address_space_map::pair_page(0x0060_0000);

/// **The waiter.** Entry registers: its own space's capability slot, `VA`, the flags. Forever:
/// `WAIT(slot, VA, flags, expected 0)`, OR the result plus one into `+16`, count the return in
/// `+8`.
#[cfg(target_arch = "aarch64")]
const WAITER: &[u32] = &[
    0xAA00_03F3, // mov  x19, x0
    0xAA01_03F4, // mov  x20, x1
    0xAA02_03F5, // mov  x21, x2
    0xAA13_03E0, // loop: mov x0, x19
    0xD280_0061, // mov  x1, #3            (WAIT)
    0xAA14_03E2, // mov  x2, x20
    0xAA15_03E3, // mov  x3, x21
    0xD280_0004, // mov  x4, #0            (expected)
    0xD280_0048, // mov  x8, #2            (SYS_INVOKE)
    0xD400_0001, // svc  #0
    0x9100_0400, // add  x0, x0, #1
    0xF940_0A86, // ldr  x6, [x20, #16]
    0xAA00_00C6, // orr  x6, x6, x0
    0xF900_0A86, // str  x6, [x20, #16]
    0xF940_0685, // ldr  x5, [x20, #8]
    0x9100_04A5, // add  x5, x5, #1
    0xF900_0685, // str  x5, [x20, #8]
    0x17FF_FFF2, // b    loop
];
#[cfg(target_arch = "riscv64")]
const WAITER: &[u32] = &[
    0x0005_0913, // mv   s2, a0
    0x0005_8993, // mv   s3, a1
    0x0006_0A13, // mv   s4, a2
    0x0009_0513, // loop: mv a0, s2
    0x0030_0593, // li   a1, 3             (WAIT)
    0x0009_8613, // mv   a2, s3
    0x000A_0693, // mv   a3, s4
    0x0000_0713, // li   a4, 0             (expected)
    0x0020_0893, // li   a7, 2             (SYS_INVOKE)
    0x0000_0073, // ecall
    0x0015_0513, // addi a0, a0, 1
    0x0109_B303, // ld   t1, 16(s3)
    0x00A3_6333, // or   t1, t1, a0
    0x0069_B823, // sd   t1, 16(s3)
    0x0089_B283, // ld   t0, 8(s3)
    0x0012_8293, // addi t0, t0, 1
    0x0059_B423, // sd   t0, 8(s3)
    0xFC9F_F06F, // j    loop
];
/// `mov r12, rdi; mov r13, rsi; mov r14, rdx; loop: mov rdi, r12; mov esi, 3; mov rdx, r13;
/// mov r10, r14; xor r8d, r8d; mov eax, 2; syscall; lea rax, [rdi + 1]; or [r13 + 16], rax;
/// inc qword [r13 + 8]; jmp loop`, then a `nop` to a word. Packed the way `user::x86_programs`
/// packs its listings. The result is read from `rdi`, where this ABI returns it (§124 (the
/// `x86_64` syscall ABI)); `rax` still holds the syscall number.
#[cfg(target_arch = "x86_64")]
const WAITER: &[u32] = &[
    0x49FC_8949,
    0x8949_F589,
    0xE789_4CD6,
    0x0000_03BE,
    0xEA89_4C00,
    0x45F2_894D,
    0x02B8_C031,
    0x0F00_0000,
    0x478D_4805,
    0x4509_4901,
    0x45FF_4910,
    0x90DA_EB08,
];

fn call(slot: u64, method: u64, a0: u64, a1: u64, a2: u64) -> Result<i64, Error> {
    let mut frame = TrapFrame::for_user_entry(0, 0, [0, 0, 0]);
    invoke(&mut frame, slot, method, a0, a1, a2)
}

fn wait_for(secs: u64, mut cond: impl FnMut() -> bool) -> bool {
    let deadline = crate::arch::timer::now() + secs * crate::arch::timer::frequency();
    while crate::arch::timer::now() < deadline {
        if cond() {
            return true;
        }
        sched::yield_now();
    }
    cond()
}

/// The word at `offset` in the waiter's page, through the direct map.
fn word(page: u64, offset: u64) -> u64 {
    // SAFETY: the waiter's own data frame, retyped for this test, through the direct map.
    unsafe { core::ptr::read_volatile((mmu::phys_to_virt(page) + offset) as *const u64) }
}

/// **A waiter, started and parked.** Its space, TCB and pages come from `region`; it holds a
/// `READ` capability to its own space in slot 0. Returns `(tid, space, data page)`.
fn start_waiter(region: u64) -> (u64, u64, u64) {
    let none = crate::revoke::PageMapSource::NoCapability;
    let name = user_address_space_create(region).expect("no address space");
    let code = code_page(region, WAITER);
    user_address_space_map(name, CODE_VA, code, Flags::user_code(), none).expect("map code");
    let stack = crate::memory_region::retype_page(region).expect("no stack frame");
    user_address_space_map(name, STACK_VA, stack, Flags::user_data(), none).expect("map stack");
    let page = crate::memory_region::retype_page(region).expect("no data frame");
    for offset in [0, 8, 16] {
        // SAFETY: a frame retyped for this test alone, through the direct map.
        unsafe {
            core::ptr::write_volatile((mmu::phys_to_virt(page) + offset) as *mut u64, 0);
        }
    }
    user_address_space_map(name, VA, page, Flags::user_data(), none).expect("map the word");

    let tid = sched::create_thread_control_block(region).expect("no tcb");
    sched::thread_control_block_insert_cap(
        tid,
        crate::cap::address_space_cap(name, Rights::READ),
        Some(0),
    )
    .expect("give the waiter its own space");
    sched::configure_thread_control_block(tid, CODE_VA, STACK_VA + page_frames::FRAME_SIZE, name)
        .expect("configure");
    sched::start_thread_control_block(tid, [0, VA, PRIVATE | SIZE_U32]).expect("start");
    assert!(
        wait_for(5, || sched::futex_waiters(name, VA) == 1),
        "the waiter never parked on its word (it returned {} times, results {:#x})",
        word(page, 8),
        word(page, 16),
    );
    (tid, name, page)
}

fn stop(tid: u64, region: u64) {
    sched::kill_thread(tid);
    assert!(
        wait_for(5, || !sched::is_thread_present(tid)),
        "premise: the killed waiter was never reaped",
    );
    sched::reclaim_region(region).expect("the waiter's region did not come back");
}

/// **A thread parked on a word sleeps until a wake names that word, and a word that has changed
/// does not put it to sleep.**
///
/// The waiter calls `WAIT` expecting `0` and the word is `0`, so it parks, and it must stay parked
/// with no return at all: a futex that returned on its own would make every `std` lock a spin. A
/// wake for the neighboring word wakes nobody. Then the word becomes `1` and one wake releases
/// it; from then on its `WAIT` expects `0`, finds `1`, and returns `1` at once, every time. So at
/// the end it has returned many times, and its results are exactly a wake (`0`) and changed words
/// (`1`), and nothing else.
///
/// Falsification: replayable `system_tests/falsifications/user.futex_tests.a_waiter_sleeps_until_its_word_is_woken.patch`
#[test_case]
fn a_waiter_sleeps_until_its_word_is_woken() {
    let region = crate::memory_region::create(16).expect("no region");
    let (tid, name, page) = start_waiter(region);

    let deadline = crate::arch::timer::now() + crate::arch::timer::frequency() / 10;
    while crate::arch::timer::now() < deadline {
        sched::yield_now();
    }
    assert_eq!(word(page, 8), 0, "a parked waiter returned without a wake");
    assert_eq!(
        sched::futex_wake(name, VA + 4, u64::MAX),
        0,
        "a wake of the neighboring word woke somebody",
    );
    assert_eq!(
        sched::futex_waiters(name, VA),
        1,
        "the waiter left its word"
    );

    // The waker's half of the protocol: change the word, then wake.
    // SAFETY: the waiter's data frame through the direct map; the waiter only reads this word.
    unsafe {
        (*(mmu::phys_to_virt(page) as *const core::sync::atomic::AtomicU32))
            .store(1, core::sync::atomic::Ordering::Release);
    }
    assert_eq!(
        sched::futex_wake(name, VA, 1),
        1,
        "the wake found nobody on the word"
    );
    assert!(
        wait_for(5, || word(page, 8) >= 2),
        "the woken waiter did not return, or blocked again on a word that had changed",
    );
    let results = word(page, 16);
    stop(tid, region);
    assert_eq!(
        results, 0b11,
        "the waiter's WAIT results were {results:#x}: bit 0 is a wake, bit 1 a changed word, and \
         anything above them an error",
    );
}

/// **A waiter whose region is destroyed leaves the futex table.** Left there, a later wake of the
/// same word would walk to a TCB page the region has already handed back.
///
/// Falsification: replayable `system_tests/falsifications/user.futex_tests.a_waiter_whose_region_is_destroyed_leaves_the_futex_table.patch`
#[test_case]
fn a_waiter_whose_region_is_destroyed_leaves_the_futex_table() {
    let region = crate::memory_region::create(16).expect("no region");
    let (tid, name, _) = start_waiter(region);
    let queued_before = sched::futex_queued();
    assert!(
        wait_for(5, || sched::reclaim_region(region).is_ok()),
        "the region of a thread parked on a futex could not be destroyed",
    );
    assert!(
        !sched::is_thread_present(tid),
        "the parked waiter outlived its region"
    );
    assert_eq!(
        sched::futex_waiters(name, VA),
        0,
        "a destroyed waiter is still parked on its word",
    );
    // The count above asks each queued waiter its key, and a torn-down thread no longer has one,
    // so a dead TCB left linked would not be counted there. This counts links, whatever they are.
    assert_eq!(
        sched::futex_queued(),
        queued_before - 1,
        "a destroyed waiter's TCB is still linked on the futex table",
    );
    assert_eq!(
        sched::futex_wake(name, VA, u64::MAX),
        0,
        "a wake found a waiter that no longer exists",
    );
}

/// **Every form §269 reserves is refused, and so is every word a caller has no business naming**,
/// each with the error `abi::address_space::WAIT` documents and in its order: a shared futex or
/// another size (`BadMethod`), a capability without `READ` (`NotPermitted`), a misaligned or
/// kernel-half address (`BadPointer`), and a space that is not the caller's own (`WrongObject`).
///
/// The caller is this test, a kernel thread with no space of its own, so the last refusal is the
/// one every well-formed call from it meets.
///
/// Falsification: replayable `system_tests/falsifications/user.futex_tests.the_reserved_futex_forms_and_foreign_words_are_refused.patch`
#[test_case]
fn the_reserved_futex_forms_and_foreign_words_are_refused() {
    let region = crate::memory_region::create(16).expect("no region");
    let name = user_address_space_create(region).expect("no address space");
    let reader = sched::grant(crate::cap::address_space_cap(name, Rights::READ)).expect("grant");
    let writer = sched::grant(crate::cap::address_space_cap(name, Rights::WRITE)).expect("grant");
    let good = PRIVATE | SIZE_U32;
    let kernel_half = mmu::phys_to_virt(0);

    for method in [abi::address_space::WAIT, abi::address_space::WAKE] {
        for (flags, what) in [
            (SIZE_U32, "a shared futex"),
            (PRIVATE | SIZE_U64, "a 64-bit futex"),
            (good | 1 << 8, "an unknown flag"),
        ] {
            assert_eq!(
                call(reader, method, VA, flags, 0),
                Err(Error::BadMethod),
                "method {method}: {what} was not refused as a form this kernel does not answer",
            );
        }
        assert_eq!(
            call(writer, method, VA, good, 0),
            Err(Error::NotPermitted),
            "method {method}: a capability without READ was not refused",
        );
        for (va, what) in [
            (VA + 2, "a misaligned word"),
            (kernel_half, "a kernel address"),
        ] {
            assert_eq!(
                call(reader, method, va, good, 0),
                Err(Error::BadPointer),
                "method {method}: {what} was not refused",
            );
        }
        assert_eq!(
            call(reader, method, VA, good, 0),
            Err(Error::WrongObject),
            "method {method}: a word in a space that is not the caller's own was not refused",
        );
    }

    for slot in [reader, writer] {
        let _ = sched::delete_current_cap(slot);
    }
    sched::reclaim_region(region).expect("the region did not come back");
}
