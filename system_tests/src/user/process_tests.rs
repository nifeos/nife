//! `process_tests`: the process object (milestone 812 (`std::thread::spawn` runs real threads in
//! one address space); §269 (how threads share a process) forks 1, 3 and 5, as calef ruled them on
//! pull request #1892).
//!
//! Two hand-written threads join one process through `CONFIGURE` with a `BIND` capability and
//! share its space, its capability table and a futex; a sibling's `exit` ends the main thread and
//! its supervisor hears; `DESTROY` and a destroyed region end every member; and each refusal the
//! contract states is made. One stub per ISA for each program, so the same claims hold on all three
//! (§19 (architectural parity is a tenet)).

//! # BUGS
//!
//! - **A killed member runs on until its next preemption**, the §16 (object revocation) amendment's
//!   mechanism, so these tests wait for members to go rather than asserting they are gone the
//!   instant a process ends. `notes/processes.md` records it.

use abi::Error;
use abi::fault::{EVENT_EXIT, FAULT_EP_SLOT};
use abi::futex::{PRIVATE, SIZE_U32};

use super::*;
use crate::cap::{Object, Rights};
use crate::sched;
use crate::syscall::invoke;

const CODE_VA: u64 = address_space_map::IMAGE_BASE;
const STACK_VA: u64 = address_space_map::STACK_TOP_PAGE;
/// A second stack, two pages below the first so a guard page sits between them.
const STACK_B_VA: u64 = STACK_VA - 2 * page_frames::FRAME_SIZE;
/// The shared page: the futex word at `+0`, the waiter's return count at `+8` and results at
/// `+16` (as `futex_tests` lays them out), the waker's `WAKE` result at `+24`.
const VA: u64 = address_space_map::pair_page(0x0060_0000);

/// **The waker.** Entry registers: a capability slot, `VA`, the futex flags. Stores `1` in the
/// word, `WAKE`s one waiter on it through the slot, stores the result at `+24`, then
/// `SYS_EXIT_THREAD(0)`: it ends itself and not its process.
#[cfg(target_arch = "aarch64")]
const WAKER: &[u32] = &[
    0xAA00_03F3, // mov  x19, x0
    0xAA01_03F4, // mov  x20, x1
    0xAA02_03F5, // mov  x21, x2
    0x5280_0023, // mov  w3, #1
    0xB900_0283, // str  w3, [x20]
    0xAA13_03E0, // mov  x0, x19
    0xD280_0081, // mov  x1, #4            (WAKE)
    0xAA14_03E2, // mov  x2, x20
    0xAA15_03E3, // mov  x3, x21
    0xD280_0024, // mov  x4, #1            (one waiter)
    0xD280_0048, // mov  x8, #2            (SYS_INVOKE)
    0xD400_0001, // svc  #0
    0xF900_0E80, // str  x0, [x20, #24]
    0xD280_0000, // mov  x0, #0
    0xD280_0088, // mov  x8, #4            (SYS_EXIT_THREAD)
    0xD400_0001, // svc  #0
    0x1400_0000, // b    .
];
#[cfg(target_arch = "riscv64")]
const WAKER: &[u32] = &[
    0x0005_0913, // mv   s2, a0
    0x0005_8993, // mv   s3, a1
    0x0006_0A13, // mv   s4, a2
    0x0010_0293, // li   t0, 1
    0x0059_A023, // sw   t0, 0(s3)
    0x0009_0513, // mv   a0, s2
    0x0040_0593, // li   a1, 4             (WAKE)
    0x0009_8613, // mv   a2, s3
    0x000A_0693, // mv   a3, s4
    0x0010_0713, // li   a4, 1             (one waiter)
    0x0020_0893, // li   a7, 2             (SYS_INVOKE)
    0x0000_0073, // ecall
    0x00A9_BC23, // sd   a0, 24(s3)
    0x0000_0513, // li   a0, 0
    0x0040_0893, // li   a7, 4             (SYS_EXIT_THREAD)
    0x0000_0073, // ecall
    0x0000_006F, // j    .
];
/// `mov r12, rdi; mov r13, rsi; mov r14, rdx; mov dword [r13], 1; mov rdi, r12; mov esi, 4;
/// mov rdx, r13; mov r10, r14; mov r8d, 1; mov eax, 2; syscall; mov [r13 + 24], rdi; xor edi, edi;
/// mov eax, 4; syscall; jmp .`, then `nop`s to a word. The result is read from `rdi`, where this ABI
/// returns it (§124 (the `x86_64` syscall ABI)).
#[cfg(target_arch = "x86_64")]
const WAKER: &[u32] = &[
    0x49FC_8949,
    0x8949_F589,
    0x45C7_41D6,
    0x0000_0100,
    0xE789_4C00,
    0x0000_04BE,
    0xEA89_4C00,
    0x41F2_894D,
    0x0000_01B8,
    0x0002_B800,
    0x050F_0000,
    0x187D_8949,
    0x04B8_FF31,
    0x0F00_0000,
    0x90FE_EB05,
];

/// **The exiter**: `SYS_EXIT(0)`, which ends its whole process.
#[cfg(target_arch = "aarch64")]
const EXITER: &[u32] = &[0xD280_0000, 0xD280_0008, 0xD400_0001, 0x1400_0000];
#[cfg(target_arch = "riscv64")]
const EXITER: &[u32] = &[0x0000_0513, 0x0000_0893, 0x0000_0073, 0x0000_006F];
/// `xor edi, edi; xor eax, eax; syscall; jmp .`.
#[cfg(target_arch = "x86_64")]
const EXITER: &[u32] = &[0xC031_FF31, 0xFEEB_050F];

/// **The spinner**: a branch to itself. It never syscalls, so only its process's end stops it.
#[cfg(target_arch = "aarch64")]
const SPINNER: &[u32] = &[0x1400_0000];
#[cfg(target_arch = "riscv64")]
const SPINNER: &[u32] = &[0x0000_006F];
/// `jmp .`, then `nop`s to a word.
#[cfg(target_arch = "x86_64")]
const SPINNER: &[u32] = &[0x9090_FEEB];

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

fn word(page: u64, offset: u64) -> u64 {
    // SAFETY: a data frame retyped for this test, through the direct map.
    unsafe { core::ptr::read_volatile((mmu::phys_to_virt(page) + offset) as *const u64) }
}

/// A space in `region` with each of `programs` at its own code page from [`CODE_VA`], two stacks
/// and the zeroed shared page at [`VA`]. Returns `(space name, shared page)`.
fn lay_out(region: u64, programs: &[&[u32]]) -> (u64, u64) {
    let none = crate::revoke::PageMapSource::NoCapability;
    let name = user_address_space_create(region).expect("no address space");
    for (i, program) in programs.iter().enumerate() {
        let code = code_page(region, program);
        let va = CODE_VA + i as u64 * page_frames::FRAME_SIZE;
        user_address_space_map(name, va, code, Flags::user_code(), none).expect("map code");
    }
    for va in [STACK_VA, STACK_B_VA] {
        let stack = crate::memory_region::retype_page(region).expect("no stack frame");
        user_address_space_map(name, va, stack, Flags::user_data(), none).expect("map stack");
    }
    let page = crate::memory_region::retype_page(region).expect("no data frame");
    for offset in (0..32).step_by(8) {
        // SAFETY: a frame retyped for this test alone, through the direct map.
        unsafe { core::ptr::write_volatile((mmu::phys_to_virt(page) + offset) as *mut u64, 0) };
    }
    user_address_space_map(name, VA, page, Flags::user_data(), none).expect("map the page");
    (name, page)
}

/// **A process around `space`, through the real `RETYPE_OBJ`.** Returns `(capability slot, id)`.
/// Retyped from the root budget's rights, as the progenitor's are, because the process carries its
/// budget's rights (milestone 824 (a retype mints no right its budget lacks)) and the joins below
/// need `BIND`.
fn create_process(region: u64, space: u64) -> (u64, sched::ProcessId) {
    let untyped =
        sched::grant(crate::cap::memory_region_lending_capability(region)).expect("grant");
    let given = sched::grant(crate::cap::address_space_cap(space, Rights::WRITE)).expect("grant");
    let slot = call(
        untyped,
        abi::memory_region::RETYPE_OBJ,
        abi::objtype::PROCESS,
        given,
        0,
    )
    .expect("RETYPE_OBJ refused the process") as u64;
    // The freed slot is the first free one, so the process capability may land in it; what must
    // not be there any more is the space.
    assert!(
        !matches!(
            sched::current_cap(given).map(|c| c.object),
            Ok(Object::AddressSpace(_))
        ),
        "the address-space capability a process was created with was not consumed",
    );
    let _ = sched::delete_current_cap(untyped);
    let Ok(crate::cap::Cap {
        object: Object::Process(pid),
        ..
    }) = sched::current_cap(slot)
    else {
        panic!("RETYPE_OBJ of a process did not hand back a process capability");
    };
    (slot, pid)
}

/// **An embryo from `region`, joined to the process in `process_slot` through the real
/// `CONFIGURE`**, at program `index`'s code page and on `stack`. Returns `(tid, its TCB slot)`.
fn join(region: u64, process_slot: u64, index: u64, stack: u64) -> (u64, u64) {
    let (tid, tcb, page) = try_join(region, process_slot, index, stack);
    let page = page.expect("CONFIGURE refused to join a thread to its process");
    assert!(
        current_cpu_contract::THREAD_PAGE_SLOTS
            .gt(&((current_cpu_contract::PAGE_VA - page as u64) as usize / 4096)),
        "a joined thread's current-CPU page {page:#x} is outside the thread page window",
    );
    (tid, tcb)
}

/// [`join`], handing back `CONFIGURE`'s answer: the thread's current-CPU page, or the refusal.
fn try_join(
    region: u64,
    process_slot: u64,
    index: u64,
    stack: u64,
) -> (u64, u64, Result<i64, Error>) {
    let tid = sched::create_thread_control_block(region).expect("no tcb");
    let tcb = sched::grant(crate::cap::thread_control_block_cap(tid, Rights::ALL)).expect("grant");
    let page = call(
        tcb,
        abi::thread_control_block::CONFIGURE,
        CODE_VA + index * page_frames::FRAME_SIZE,
        stack + page_frames::FRAME_SIZE,
        process_slot,
    );
    (tid, tcb, page)
}

/// **Two threads of one process share its space, its capability table and a futex.**
///
/// The waiter (`futex_tests`' program) and the waker run in one space, and the futex word is one
/// word of it. Both pass slot 0 to `WAIT` and `WAKE`, inserted once through the waiter's TCB: the
/// waker can name it only because the table is the process's. The waker wakes the waiter (a
/// user-to-user handoff, where `futex_tests`' waker is the kernel), then ends itself with
/// `SYS_EXIT_THREAD`, and the process goes on with one member. `DESTROY` then ends that member
/// too, and the space goes with the process.
///
/// Falsification: replayable `system_tests/falsifications/user.process_tests.two_threads_of_one_process_share_its_space_its_table_and_a_futex.patch`
#[test_case]
fn two_threads_of_one_process_share_its_space_its_table_and_a_futex() {
    let region = crate::memory_region::create(32).expect("no region");
    let (space, page) = lay_out(region, &[super::futex_tests::WAITER, WAKER]);
    let (process, pid) = create_process(region, space);
    assert_eq!(
        sched::process_state(pid),
        Some((0, false)),
        "a new process has members"
    );

    let (waiter, waiter_tcb) = join(region, process, 0, STACK_VA);
    assert!(
        sched::current_cap(process).is_ok(),
        "CONFIGURE consumed the process capability it joined through",
    );
    let reader = sched::grant(crate::cap::address_space_cap(
        space,
        Rights::READ.union(Rights::GRANT),
    ))
    .expect("grant");
    assert_eq!(
        call(
            waiter_tcb,
            abi::thread_control_block::CAP_INSERT,
            reader,
            Rights::READ.bits() as u64,
            1, // slot 0
        ),
        Ok(0),
        "CAP_INSERT into a member refused",
    );
    let (waker, waker_tcb) = join(region, process, 1, STACK_B_VA);
    assert_eq!(
        sched::process_state(pid),
        Some((2, false)),
        "two joins, not two members"
    );

    let flags = PRIVATE | SIZE_U32;
    sched::start_thread_control_block(waiter, [0, VA, flags]).expect("start the waiter");
    assert!(
        wait_for(5, || sched::futex_waiters(space, VA) == 1),
        "the waiter never parked on the shared word",
    );
    sched::start_thread_control_block(waker, [0, VA, flags]).expect("start the waker");
    assert!(
        wait_for(5, || !sched::is_thread_present(waker)),
        "the waker never ended itself with SYS_EXIT_THREAD",
    );
    assert_eq!(
        word(page, 24),
        1,
        "the waker's WAKE through the shared slot 0 woke nobody",
    );
    assert!(
        wait_for(5, || word(page, 8) >= 2),
        "the woken waiter did not return",
    );
    assert_eq!(
        word(page, 16),
        0b11,
        "the waiter's results were {:#x}: a wake, then a changed word, and nothing else",
        word(page, 16),
    );
    assert_eq!(
        sched::process_state(pid),
        Some((1, false)),
        "one thread's SYS_EXIT_THREAD ended its process, or left it with the wrong member count",
    );

    assert_eq!(
        call(process, abi::process::DESTROY, 0, 0, 0),
        Ok(0),
        "DESTROY refused"
    );
    assert!(
        wait_for(5, || !sched::is_thread_present(waiter)),
        "DESTROY did not end the process's last member",
    );
    assert_eq!(
        sched::process_state(pid),
        Some((0, true)),
        "the process outlived its members"
    );
    assert!(
        user_address_space_root(space).is_none(),
        "the process's space outlived the process and every member",
    );

    for slot in [process, waiter_tcb, waker_tcb, reader] {
        let _ = sched::delete_current_cap(slot);
    }
    sched::reclaim_region(region).expect("the process's region did not come back");
}

/// **A member's `exit` ends its process, and the supervisor of its first member hears.**
///
/// The first member is supervised through the reserved fault slot of the process's table and spins
/// forever; the second calls `SYS_EXIT`. The spinner is ended with its process, and its supervisor
/// receives the end, `EVENT_EXIT`, carried by the spinner it supervised, so `REAP` collects the
/// thread it built.
///
/// Falsification: replayable `system_tests/falsifications/user.process_tests.a_members_exit_ends_its_process_and_the_supervisor_hears.patch`
#[test_case]
fn a_members_exit_ends_its_process_and_the_supervisor_hears() {
    let region = crate::memory_region::create(32).expect("no region");
    let endpoints = crate::memory_region::create(1).expect("no endpoint region");
    let supervisor = sched::create_rendezvous_from(endpoints).expect("no rendezvous");
    let (space, _) = lay_out(region, &[SPINNER, EXITER]);
    let (process, pid) = create_process(region, space);

    let (spinner, spinner_tcb) = join(region, process, 0, STACK_VA);
    sched::thread_control_block_insert_cap(
        spinner,
        crate::cap::rendezvous_cap(supervisor, Rights::READ),
        Some(FAULT_EP_SLOT),
    )
    .expect("insert the supervision endpoint");
    sched::start_thread_control_block(spinner, [0; 3]).expect("start the spinner");
    let (exiter, exiter_tcb) = join(region, process, 1, STACK_B_VA);
    sched::start_thread_control_block(exiter, [0; 3]).expect("start the exiter");

    // Bounded first, so a process the exit failed to end fails here rather than parking this test
    // in a receive nothing will ever answer.
    assert!(
        wait_for(5, || sched::with_binders(|b| b(spinner))
            != sched::Binder::CanRun),
        "the spinner outlived its process: a member's exit did not end it",
    );
    let msg = sched::ipc_receive(supervisor);
    assert_eq!(
        msg[0], EVENT_EXIT,
        "the supervisor heard {msg:?}, not the process's exit"
    );
    assert_eq!(
        msg[1], spinner,
        "the death came from thread {}, not the supervised member {spinner}",
        msg[1],
    );
    assert!(
        wait_for(5, || !sched::is_thread_present(exiter)),
        "the exiter never left"
    );
    sched::reap_supervised(supervisor, spinner).expect("the supervised member was not reapable");
    assert!(
        wait_for(5, || sched::process_state(pid)
            .is_none_or(|(m, ended)| m == 0 && ended)),
        "the process did not end with its members",
    );

    // `REAP` reclaims the region the supervised thread was built from, which here is the whole
    // process: its page, its space and both members.
    assert!(
        crate::memory_region::region_bounds(region).is_none(),
        "REAP left the process's region standing",
    );
    assert!(
        user_address_space_root(space).is_none(),
        "the space outlived its region"
    );
    for slot in [process, spinner_tcb, exiter_tcb] {
        let _ = sched::delete_current_cap(slot);
    }
    sched::reclaim_region(endpoints).expect("the endpoint region did not come back");
}

/// **The thread that ends itself**: `SYS_EXIT_THREAD(0)`.
#[cfg(target_arch = "aarch64")]
const THREAD_EXITER: &[u32] = &[0xD280_0000, 0xD280_0088, 0xD400_0001, 0x1400_0000];
#[cfg(target_arch = "riscv64")]
const THREAD_EXITER: &[u32] = &[0x0000_0513, 0x0040_0893, 0x0000_0073, 0x0000_006F];
/// `xor edi, edi; mov eax, 4; syscall; jmp .`, then a `nop`.
#[cfg(target_arch = "x86_64")]
const THREAD_EXITER: &[u32] = &[0x04B8_FF31, 0x0F00_0000, 0x90FE_EB05];

/// The bytes of the current-CPU page `space` maps at `va`, through its own tables.
fn page_of(space: u64, va: u64) -> (u64, current_cpu_contract::CurrentCpuPage) {
    let root = user_address_space_root(space).expect("the space is gone");
    let (phys, _) = crate::arch::mmu::translate_at(root, va).expect("no current-CPU page mapped");
    // SAFETY: a frame the space maps read-only for its thread, read here through the direct map;
    // the kernel is its only writer.
    let page = unsafe { current_cpu_contract::CurrentCpuPage::new(mmu::phys_to_virt(phys)) };
    (phys, page)
}

/// **Each member of a process has its own current-CPU page, carrying its own core and the
/// process's allowance** (§269 forks 6 and 7, and question 2 on pull request #1892).
///
/// `CONFIGURE` answers each joining thread's page: the first gets `PAGE_VA`, where every thread
/// read before, and the next the slot below it. The two pages are two frames, each running member's
/// names an online core, and both carry the allowance, which is the online count. A member that
/// ends gives its slot back to the next thread to join, and a process with every slot taken
/// refuses one more thread with `OutOfMemory`.
///
/// Falsification: replayable `system_tests/falsifications/user.process_tests.each_member_of_a_process_has_its_own_current_cpu_page.patch`
#[test_case]
fn each_member_of_a_process_has_its_own_current_cpu_page() {
    let region = crate::memory_region::create(64).expect("no region");
    let (space, _) = lay_out(region, &[SPINNER, THREAD_EXITER]);
    let (process, _) = create_process(region, space);
    let below = current_cpu_contract::PAGE_VA - page_frames::FRAME_SIZE;

    let (a, a_tcb, a_page) = try_join(region, process, 0, STACK_VA);
    let (b, b_tcb, b_page) = try_join(region, process, 0, STACK_B_VA);
    assert_eq!(
        a_page,
        Ok(current_cpu_contract::PAGE_VA as i64),
        "the first member's page"
    );
    assert_eq!(
        b_page,
        Ok(below as i64),
        "the second member's page is not the slot below"
    );
    sched::start_thread_control_block(a, [0; 3]).expect("start");
    sched::start_thread_control_block(b, [0; 3]).expect("start");

    let online = crate::smp::online_harts_mask();
    let ((a_phys, a_view), (b_phys, b_view)) = (
        page_of(space, current_cpu_contract::PAGE_VA),
        page_of(space, below),
    );
    assert_ne!(a_phys, b_phys, "two members read one current-CPU page");
    assert!(
        wait_for(5, || a_view.cpu().is_some() && b_view.cpu().is_some()),
        "a running member's page was never published",
    );
    for (who, view) in [("first", a_view), ("second", b_view)] {
        let cpu = view.cpu().expect("published above");
        assert!(
            online & (1 << cpu) != 0,
            "the {who} member's page names cpu {cpu}, not online"
        );
        assert_eq!(
            view.allowance().map(|n| n.get()),
            Some(online.count_ones() as usize),
            "the {who} member's allowance is not the online count",
        );
    }

    // A member that ends gives its slot back: the next join takes the slot below again.
    let (exiter, exiter_tcb, exiter_page) = try_join(region, process, 1, STACK_B_VA);
    let third = below - page_frames::FRAME_SIZE;
    assert_eq!(exiter_page, Ok(third as i64), "the third member's page");
    sched::start_thread_control_block(exiter, [0; 3]).expect("start");
    assert!(
        wait_for(5, || !sched::is_thread_present(exiter)),
        "the exiter never left"
    );
    let (_, again_tcb, again_page) = try_join(region, process, 0, STACK_B_VA);
    assert_eq!(
        again_page,
        Ok(third as i64),
        "an ended member's slot was not reused"
    );

    // Fill every slot, then one more.
    let mut embryos = [0u64; current_cpu_contract::THREAD_PAGE_SLOTS];
    let mut filled = 3;
    let refused = loop {
        let (_, tcb, page) = try_join(region, process, 0, STACK_B_VA);
        match page {
            Ok(_) => {
                embryos[filled] = tcb;
                filled += 1;
            }
            Err(e) => break (filled, e, tcb),
        }
    };
    assert_eq!(
        (refused.0, refused.1),
        (current_cpu_contract::THREAD_PAGE_SLOTS, Error::OutOfMemory),
        "a process took more threads than it has current-CPU page slots, or fewer",
    );

    assert_eq!(call(process, abi::process::DESTROY, 0, 0, 0), Ok(0));
    assert!(
        wait_for(5, || !sched::is_thread_present(a)
            && !sched::is_thread_present(b)),
        "DESTROY did not end the running members",
    );
    for slot in [process, a_tcb, b_tcb, exiter_tcb, again_tcb, refused.2]
        .into_iter()
        .chain(embryos.into_iter().filter(|&s| s != 0))
    {
        let _ = sched::delete_current_cap(slot);
    }
    sched::reclaim_region(region).expect("the region did not come back");
}

/// **Every refusal the process contract states**, each with its error: `CONFIGURE` through a process
/// capability without `BIND` (`NotPermitted`), a second holder of a held space (`WrongObject`), an
/// embryo whose own table holds a slot the process's table already holds (`WrongObject`), and, once
/// the process has ended, a join or a member's `START` (`Gone`). And what a clean join does with an
/// embryo's own capabilities: they move into the process's table, where every member sees them.
///
/// Falsification: replayable `system_tests/falsifications/user.process_tests.a_process_refuses_what_its_contract_refuses.patch`
#[test_case]
fn a_process_refuses_what_its_contract_refuses() {
    let region = crate::memory_region::create(32).expect("no region");
    let (space, _) = lay_out(region, &[SPINNER]);
    let (process, pid) = create_process(region, space);
    let tcb_of =
        |tid| sched::grant(crate::cap::thread_control_block_cap(tid, Rights::ALL)).expect("grant");
    let configure = |tcb, through| {
        call(
            tcb,
            abi::thread_control_block::CONFIGURE,
            CODE_VA,
            STACK_VA + page_frames::FRAME_SIZE,
            through,
        )
    };

    let unbound = sched::grant(crate::cap::process_capability(
        pid,
        Rights::ALL.intersect(Rights::from_bits(!Rights::BIND.bits())),
    ))
    .expect("grant");
    let a = sched::create_thread_control_block(region).expect("no tcb");
    let a_tcb = tcb_of(a);
    assert_eq!(
        configure(a_tcb, unbound),
        Err(Error::NotPermitted),
        "a process capability without BIND joined a thread",
    );

    let untyped = sched::grant(crate::cap::memory_region_cap(region)).expect("grant");
    let again = sched::grant(crate::cap::address_space_cap(space, Rights::WRITE)).expect("grant");
    assert_eq!(
        call(
            untyped,
            abi::memory_region::RETYPE_OBJ,
            abi::objtype::PROCESS,
            again,
            0
        ),
        Err(Error::WrongObject),
        "a second process took a space another already holds",
    );

    // b joins carrying slot 5 in its own table, which moves into the process's.
    let b = sched::create_thread_control_block(region).expect("no tcb");
    let b_tcb = tcb_of(b);
    sched::thread_control_block_insert_cap(b, crate::cap::memory_region_cap(region), Some(5))
        .expect("insert into b's own table");
    assert!(configure(b_tcb, process).is_ok(), "premise: a clean join");
    assert!(
        sched::with_capability_table(b, |t| t.get(5).is_ok()).unwrap_or(false),
        "a joining embryo's capability did not move into its process's table",
    );
    // a, carrying slot 5 too, would put two capabilities in one slot of the shared table.
    sched::thread_control_block_insert_cap(a, crate::cap::memory_region_cap(region), Some(5))
        .expect("insert into a's own table");
    assert_eq!(
        configure(a_tcb, process),
        Err(Error::WrongObject),
        "an embryo joined a process whose table already held a slot it carries",
    );
    assert_eq!(
        call(process, abi::process::DESTROY, 0, 0, 0),
        Ok(0),
        "DESTROY refused"
    );
    assert_eq!(
        sched::start_thread_control_block(b, [0; 3]),
        Err(Error::Gone),
        "a member of an ended process started",
    );
    let c = sched::create_thread_control_block(region).expect("no tcb");
    let c_tcb = tcb_of(c);
    assert_eq!(
        configure(c_tcb, process),
        Err(Error::Gone),
        "a thread joined an ended process",
    );

    for slot in [process, unbound, untyped, again, a_tcb, b_tcb, c_tcb] {
        let _ = sched::delete_current_cap(slot);
    }
    sched::reclaim_region(region).expect("the region did not come back");
}

/// **Destroying the region a process lives in ends it, members elsewhere included.** The process
/// and its space come from one region and its member's TCB from another, so the member is not a
/// resident of the region being destroyed. The first `DESTROY` of the region is refused while the
/// member runs and ends it; the owner's retry reclaims the region, and the space is gone with the
/// process.
///
/// Falsification: replayable `system_tests/falsifications/user.process_tests.a_destroyed_region_ends_the_process_in_it_and_its_members.patch`
#[test_case]
fn a_destroyed_region_ends_the_process_in_it_and_its_members() {
    let region = crate::memory_region::create(32).expect("no region");
    let threads = crate::memory_region::create(2).expect("no thread region");
    let (space, _) = lay_out(region, &[SPINNER]);
    let (process, pid) = create_process(region, space);
    let (member, member_tcb) = join(threads, process, 0, STACK_VA);
    sched::start_thread_control_block(member, [0; 3]).expect("start");
    assert!(
        sched::reclaim_region(region).is_err(),
        "a region was reclaimed from under a process whose member still ran",
    );
    assert!(
        wait_for(5, || sched::reclaim_region(region).is_ok()),
        "the retry never reclaimed the process's region",
    );
    // The member's reaper leaves the process under the scheduler's lock, which is what lets the
    // retry succeed, and takes the thread out of the table after releasing it, on its own core.
    assert!(
        wait_for(5, || !sched::is_thread_present(member)),
        "the member outlived its process"
    );
    assert_eq!(
        sched::process_state(pid),
        None,
        "the process outlived its region"
    );
    assert!(
        user_address_space_root(space).is_none(),
        "the space outlived its process"
    );
    for slot in [process, member_tcb] {
        let _ = sched::delete_current_cap(slot);
    }
    sched::reclaim_region(threads).expect("the thread region did not come back");
}

/// **A member stranded killed on a futex is finished and reaped, not kept forever.** A member that
/// was mid-way into blocking when its process ended is marked killed rather than finished, because
/// it is still on its core; once its switch-out completes it is parked with nothing left to wake
/// it. CI's `thead-c906` leg met it as `rayon`'s workers, which spin and then sleep. The race cannot
/// be staged on demand, so this stages its outcome (`sched::strand_member`) and asks the sweep that
/// runs after a departing member's reap to clear it: the member is gone, nothing is left on the
/// futex, and the process has no member and no space.
///
/// Falsification: replayable `system_tests/falsifications/user.process_tests.a_member_stranded_killed_on_a_futex_is_finished_and_reaped.patch`
#[test_case]
fn a_member_stranded_killed_on_a_futex_is_finished_and_reaped() {
    let region = crate::memory_region::create(32).expect("no region");
    let (space, _) = lay_out(region, &[super::futex_tests::WAITER]);
    let (process, pid) = create_process(region, space);
    let (waiter, waiter_tcb) = join(region, process, 0, STACK_VA);
    let reader = sched::grant(crate::cap::address_space_cap(
        space,
        Rights::READ.union(Rights::GRANT),
    ))
    .expect("grant");
    assert_eq!(
        call(
            waiter_tcb,
            abi::thread_control_block::CAP_INSERT,
            reader,
            Rights::READ.bits() as u64,
            1, // slot 0
        ),
        Ok(0),
        "CAP_INSERT into a member refused",
    );
    sched::start_thread_control_block(waiter, [0, VA, PRIVATE | SIZE_U32]).expect("start");
    assert!(
        wait_for(5, || sched::futex_waiters(space, VA) == 1),
        "the waiter never parked",
    );

    sched::strand_member(pid, waiter);
    sched::reap_corpses_now(pid);
    assert!(
        wait_for(5, || !sched::is_thread_present(waiter)),
        "a member stranded killed on a futex was never finished",
    );
    assert_eq!(
        sched::futex_waiters(space, VA),
        0,
        "the finished member is still on the futex"
    );
    assert_eq!(
        sched::process_state(pid),
        Some((0, true)),
        "the process kept its member"
    );
    assert!(
        wait_for(5, || user_address_space_root(space).is_none()),
        "the process's space outlived its last member",
    );

    for slot in [process, waiter_tcb, reader] {
        let _ = sched::delete_current_cap(slot);
    }
    assert!(
        wait_for(5, || sched::reclaim_region(region).is_ok()),
        "the process's region did not come back"
    );
}
