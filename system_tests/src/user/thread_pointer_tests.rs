//! `thread_pointer_tests`: each thread has its own thread pointer, set by the kernel (milestone 812
//! (`std::thread::spawn` runs real threads in one address space), §269 (how threads share a
//! process) fork 4).
//!
//! What a user program's thread-local storage hangs off is one register per thread: `TPIDR_EL0`,
//! `tp`, the `FS` base. The kernel now holds a value per thread, takes the first from `CONFIGURE`'s
//! sixth argument register, changes it through `ThreadControlBlock::SET_THREAD_POINTER`, and
//! installs it at every switch. These tests prove that from user mode, on all three architectures,
//! with one hand-written program per ISA that reads the word its thread pointer names.
//!
//! **Why the program reads memory through the register rather than the register itself.** `x86_64`
//! keeps `CR4.FSGSBASE` off (§269), so ring 3 cannot read its `FS` base, only address through it.
//! `fs:[0]`, `[TPIDR_EL0]` and `0(tp)` are the one observation all three can make alike (§19
//! (architectural parity is a tenet)).
//!
//! # BUGS
//!
//! - **Each thread here has its own address space**, because two threads cannot share one until
//!   the join is built (it waits on calef's ruling on pull request #1892). The spaces are laid out
//!   identically, so a thread reading another thread's pointer reads the other thread's word, and
//!   the test would see it. What this cannot show is two threads of one space; the milestone's exit
//!   test does.
//! - **Nothing pins the spinners to one core.** The test starts more spinners than the machine has
//!   cores, so at least two must share one and be switched between, but which two is the
//!   scheduler's choice, and a one-core boot puts them all together.

use abi::Error;

use super::*;
use crate::cap::Rights;
use crate::sched;
use crate::syscall::invoke;

const CODE_VA: u64 = address_space_map::IMAGE_BASE;
const STACK_VA: u64 = address_space_map::STACK_TOP_PAGE;
/// The page of words the thread pointers name, identical in every spinner's space.
const VA: u64 = address_space_map::pair_page(0x0060_0000);

/// Word `i` of the page holds `1 << i`, and spinner `i`'s thread pointer is `VA + 8 * i`.
const SPINNERS: usize = 2 * crate::cpu::MAX_CPUS;
/// Where each spinner accumulates what it read, past the words it reads from.
const RESULT_OFFSET: u64 = 8 * 64;
/// A word holding only bit 63, which a self-setting spinner's first pointer names: if its own
/// `SET_THREAD_POINTER` did not take, it reads this.
const POISON_INDEX: u64 = 63;

/// **The spinner.** Entry registers: the result address, the spinner's own TCB slot plus one (zero
/// to skip), and a thread pointer. If the slot is given it first calls
/// `SET_THREAD_POINTER(slot, value)` on itself. Then forever: load the word its thread pointer
/// names, OR it into the result word. An OR is sticky, so a thread that ever read through another
/// thread's pointer keeps the other thread's bit.
#[cfg(target_arch = "aarch64")]
const SPINNER: &[u32] = &[
    0xAA00_03F3, // mov  x19, x0
    0xB400_00A1, // cbz  x1, loop
    0xD100_0420, // sub  x0, x1, #1
    0xD280_0061, // mov  x1, #3            (SET_THREAD_POINTER)
    0xD280_0048, // mov  x8, #2            (SYS_INVOKE)
    0xD400_0001, // svc  #0
    0xD53B_D043, // loop: mrs x3, tpidr_el0
    0xF940_0064, // ldr  x4, [x3]
    0xF940_0265, // ldr  x5, [x19]
    0xAA04_00A5, // orr  x5, x5, x4
    0xF900_0265, // str  x5, [x19]
    0x17FF_FFFB, // b    loop
];
#[cfg(target_arch = "riscv64")]
const SPINNER: &[u32] = &[
    0x0005_0913, // mv   s2, a0
    0x0005_8A63, // beqz a1, loop
    0xFFF5_8513, // addi a0, a1, -1
    0x0030_0593, // li   a1, 3             (SET_THREAD_POINTER)
    0x0020_0893, // li   a7, 2             (SYS_INVOKE)
    0x0000_0073, // ecall
    0x0002_3283, // loop: ld t0, 0(tp)
    0x0009_3303, // ld   t1, 0(s2)
    0x0053_6333, // or   t1, t1, t0
    0x0069_3023, // sd   t1, 0(s2)
    0xFF1F_F06F, // j    loop
];
/// `mov r12, rdi; test rsi, rsi; jz loop; lea rdi, [rsi - 1]; mov esi, 3; mov eax, 2; syscall;
/// loop: mov rax, fs:[0]; or [r12], rax; jmp loop`, then one `nop` to a word. Packed the way
/// `user::x86_programs` packs its listings.
#[cfg(target_arch = "x86_64")]
const SPINNER: &[u32] = &[
    0x48FC_8949,
    0x1074_F685,
    0xFF7E_8D48,
    0x0000_03BE,
    0x0002_B800,
    0x050F_0000,
    0x048B_4864,
    0x0000_0025,
    0x0409_4900,
    0x90F1_EB24,
];

/// `invoke` through the real dispatcher, with the sixth argument register as given: the register
/// `CONFIGURE` reads its thread pointer from.
fn call6(slot: u64, method: u64, a: [u64; 3], sixth: u64) -> Result<i64, Error> {
    let mut frame = TrapFrame::for_user_entry(0, 0, [0, 0, 0]);
    frame.set_arg(5, sixth);
    invoke(&mut frame, slot, method, a[0], a[1], a[2])
}

/// A spinner's space: code, stack, and the page of words, returned as that page's physical address.
fn lay_out(region: u64, name: u64) -> u64 {
    let none = crate::revoke::PageMapSource::NoCapability;
    let code = code_page(region, SPINNER);
    user_address_space_map(name, CODE_VA, code, Flags::user_code(), none).expect("map code");
    let stack = crate::memory_region::retype_page(region).expect("no stack frame");
    user_address_space_map(name, STACK_VA, stack, Flags::user_data(), none).expect("map stack");
    let page = crate::memory_region::retype_page(region).expect("no data frame");
    let words = mmu::phys_to_virt(page) as *mut u64;
    for i in 0..64 {
        // SAFETY: a frame retyped for this test alone, named through the direct map, 512 words long.
        unsafe { words.add(i).write_volatile(1 << i) };
    }
    // SAFETY: as above; the result word starts clear.
    unsafe { words.add(RESULT_OFFSET as usize / 8).write_volatile(0) };
    user_address_space_map(name, VA, page, Flags::user_data(), none).expect("map the words");
    page
}

fn result(page: u64) -> u64 {
    // SAFETY: the spinner's own data frame, through the direct map; only the spinner writes it.
    unsafe { core::ptr::read_volatile((mmu::phys_to_virt(page) + RESULT_OFFSET) as *const u64) }
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

/// **Every thread reads through its own thread pointer, however the threads are switched**, and
/// both ways of giving a thread its pointer work: `CONFIGURE`'s sixth register for the even
/// spinners, and the spinner's own `SET_THREAD_POINTER` on itself for the odd ones.
///
/// Twice as many spinners as the kernel has cpu ids, all running at once, so at least two share a
/// core and are switched between by the timer, and each ORs what it reads into a sticky result. At
/// the end spinner `i` must hold exactly `1 << i`. A thread pointer that leaked across a switch
/// shows as a foreign bit; one that never arrived shows as the poison bit (a self-setter whose set
/// did not take reads word 63) or as no bit at all.
///
/// Before this milestone the kernel never touched these registers: on aarch64 every program saw
/// the last value any program on its core wrote, on `x86_64` every `FS` base was zero.
///
/// Falsification: replayable `system_tests/falsifications/user.thread_pointer_tests.each_thread_reads_through_its_own_thread_pointer.patch`
#[test_case]
fn each_thread_reads_through_its_own_thread_pointer() {
    let region = crate::memory_region::create(16 * SPINNERS as u64).expect("no region");
    let mut spinners = [(0u64, 0u64, 0u64); SPINNERS];
    for (i, s) in spinners.iter_mut().enumerate() {
        let name = user_address_space_create(region).expect("no address space");
        let page = lay_out(region, name);
        let given =
            sched::grant(crate::cap::address_space_cap(name, Rights::WRITE)).expect("grant");
        let tid = sched::create_thread_control_block(region).expect("no tcb");
        let tcb =
            sched::grant(crate::cap::thread_control_block_cap(tid, Rights::ALL)).expect("grant");
        let mine = VA + 8 * i as u64;
        let self_setter = i % 2 == 1;
        let first = if self_setter {
            VA + 8 * POISON_INDEX
        } else {
            mine
        };
        assert_eq!(
            call6(
                tcb,
                abi::thread_control_block::CONFIGURE,
                [CODE_VA, STACK_VA + page_frames::FRAME_SIZE, given],
                first,
            ),
            Ok(0),
            "CONFIGURE refused spinner {i}'s thread pointer {first:#x}",
        );
        let (slot_arg, value_arg) = if self_setter {
            // The spinner's own TCB, in its own slot 0, so it can name itself.
            sched::thread_control_block_insert_cap(
                tid,
                crate::cap::thread_control_block_cap(tid, Rights::WRITE),
                Some(0),
            )
            .expect("give the spinner its own TCB");
            (1, mine)
        } else {
            (0, 0)
        };
        sched::start_thread_control_block(tid, [VA + RESULT_OFFSET, slot_arg, value_arg])
            .expect("start");
        *s = (tid, tcb, page);
    }

    // Every spinner has read at least once, then they all run on together for a while, switched
    // by the timer against each other and everything else on their cores.
    assert!(
        wait_for(5, || spinners.iter().all(|&(_, _, p)| result(p) != 0)),
        "a spinner never ran, or faulted on its first read",
    );
    let deadline = crate::arch::timer::now() + crate::arch::timer::frequency() / 4;
    while crate::arch::timer::now() < deadline {
        sched::yield_now();
    }

    // Another started thread is refused; nothing about a running thread is reachable that way.
    let (_, other, _) = spinners[0];
    assert_eq!(
        call6(
            other,
            abi::thread_control_block::SET_THREAD_POINTER,
            [VA, 0, 0],
            0
        ),
        Err(Error::WrongObject),
        "SET_THREAD_POINTER reached a running thread that is not the caller",
    );

    let read: [u64; SPINNERS] = core::array::from_fn(|i| result(spinners[i].2));
    for &(tid, tcb, _) in &spinners {
        sched::kill_thread(tid);
        let _ = sched::delete_current_cap(tcb);
    }
    assert!(
        wait_for(5, || spinners
            .iter()
            .all(|&(tid, _, _)| !sched::is_thread_present(tid))),
        "premise: a killed spinner was never reaped",
    );
    sched::reclaim_region(region).expect("the spinners' region did not come back");

    for (i, &got) in read.iter().enumerate() {
        assert_eq!(
            got,
            1 << i,
            "spinner {i} read {got:#x} through its thread pointer, not {:#x}: {}",
            1u64 << i,
            if got & (1 << POISON_INDEX) != 0 {
                "its own SET_THREAD_POINTER did not take"
            } else {
                "it read through another thread's pointer"
            },
        );
    }
}

/// **A thread pointer the kernel could not install is refused before anything changes**, by both
/// ways in: `CONFIGURE` leaves the embryo unbound (a good `CONFIGURE` then succeeds), and
/// `SET_THREAD_POINTER` leaves the value as it was.
///
/// A kernel-half address is the case that matters: on `x86_64` the `wrmsr` that installs a
/// non-canonical `FS` base raises `#GP` in the kernel, and the same rule holds on all three
/// architectures so a program refused on one is refused on each (§19).
///
/// Falsification: replayable `system_tests/falsifications/user.thread_pointer_tests.a_thread_pointer_outside_the_user_half_is_refused_and_changes_nothing.patch`
#[test_case]
fn a_thread_pointer_outside_the_user_half_is_refused_and_changes_nothing() {
    let region = crate::memory_region::create(16).expect("no region");
    let name = user_address_space_create(region).expect("no address space");
    let given = sched::grant(crate::cap::address_space_cap(name, Rights::WRITE)).expect("grant");
    let tid = sched::create_thread_control_block(region).expect("no tcb");
    let tcb = sched::grant(crate::cap::thread_control_block_cap(tid, Rights::ALL)).expect("grant");
    let kernel_half = crate::arch::mmu::phys_to_virt(0);
    let configure = |tp| {
        call6(
            tcb,
            abi::thread_control_block::CONFIGURE,
            [CODE_VA, STACK_VA + page_frames::FRAME_SIZE, given],
            tp,
        )
    };

    assert_eq!(
        configure(kernel_half),
        Err(Error::BadPointer),
        "CONFIGURE took a kernel-half thread pointer",
    );
    assert!(
        sched::current_cap(given).is_ok(),
        "a refused CONFIGURE still consumed the address-space capability",
    );
    assert_eq!(
        call6(
            tcb,
            abi::thread_control_block::SET_THREAD_POINTER,
            [kernel_half, 0, 0],
            0
        ),
        Err(Error::BadPointer),
        "SET_THREAD_POINTER took a kernel-half value",
    );
    assert_eq!(
        configure(VA),
        Ok(0),
        "a refused CONFIGURE left the embryo unable to be configured",
    );
    assert_eq!(
        call6(
            tcb,
            abi::thread_control_block::SET_THREAD_POINTER,
            [VA + 8, 0, 0],
            0
        ),
        Ok(0),
        "an embryo's thread pointer could not be changed before it ran",
    );

    let _ = sched::delete_current_cap(tcb);
    sched::reclaim_region(region).expect("the region did not come back");
}
