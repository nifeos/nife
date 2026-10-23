//! Tests for threads, the context switch, and preemption.
//!
//! `a_thread_that_never_yields_is_preempted_anyway` is the one this whole project has been
//! arguing about since DECISIONS §5. Everything else here is scaffolding for it.

use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// **A running thread cannot grant itself the cycle counter** (milestone 229, DECISIONS 139
/// part 2). The grant is a field in the thread's spawn manifest, and this is the mechanism that
/// makes that sentence true rather than descriptive: `grant_cycle_counter` refuses anything
/// that is not an `Embryo`, so the only window in which the bit can be set is before the thread
/// has ever run.
///
/// It asks on behalf of the caller, which is the strongest available form of the question: this
/// thread is `Running` by definition of executing this line, and it holds every authority a
/// kernel thread has. `WrongObject` is the same refusal `CONFIGURE` and `CAP_INSERT` make on a
/// started thread, reused rather than a new error minted for a new way of being too late.
#[test_case]
fn a_running_thread_cannot_be_granted_the_cycle_counter() {
    let me = super::current_thread_id();
    assert_eq!(
        super::grant_cycle_counter(me),
        Err(abi::Error::WrongObject),
        "a live thread was allowed to acquire a timing instrument it was not created with",
    );
}

/// Wait for `cond`, yielding, bounded by the clock (two seconds) rather than by a yield count.
/// Returns whether `cond` held, evaluating it once more after the deadline.
///
/// A yield count is not a duration once placement scatters work across cores
/// (notes/load-sensitive-assertions.md). Two seconds is far beyond an honest completion and well
/// inside the harness's 90 s per-test ceiling, which stays the backstop for a genuine hang.
pub(super) fn wait_for(mut cond: impl FnMut() -> bool) -> bool {
    let deadline = crate::arch::timer::now() + 2 * crate::arch::timer::frequency();
    while crate::arch::timer::now() < deadline {
        if cond() {
            return true;
        }
        crate::sched::yield_now();
    }
    cond()
}

/// Wait for `cond` **without yielding**, budgeted in `budget` timer ticks delivered to this core.
/// Returns whether `cond` held.
///
/// For `a_thread_that_never_yields_is_preempted_anyway`, which must not yield and waits for a
/// preemption, so preemption opportunities are its unit; [`crate::testing::TickBudget`] has why
/// that unit survives a contended host. If ticks stop arriving this does not return, and the
/// harness's 90 s per-test ceiling reports it.
pub(super) fn within_ticks(budget: u64, mut cond: impl FnMut() -> bool) -> bool {
    let mut budget = crate::testing::TickBudget::new(budget);
    loop {
        if cond() {
            return true;
        }
        if budget.is_expired() {
            return cond();
        }
        core::hint::spin_loop();
    }
}

/// Spin the scheduler until `cond`, bounded by wall-clock, returning whether it happened. Since
/// DECISIONS §28, work a test spawns runs on *other* cores, so this core is often idle and a
/// yield returns at once: a fixed count of yields elapses in almost no real time and times out
/// before the parallel result lands. A ~2 s deadline gives the other cores real time while
/// staying far under the 60 s hang watchdog, so a genuine lost wakeup still fails.
pub(super) fn spin_until(mut cond: impl FnMut() -> bool) -> bool {
    let deadline = crate::arch::timer::now() + 2 * crate::arch::timer::frequency();
    while crate::arch::timer::now() < deadline {
        if cond() {
            return true;
        }
        super::yield_now();
    }
    cond()
}

// --- Raising an interrupt on purpose, on each ISA ---------------------------------------
//
// The delivery tests below are portable; only the trigger is architectural. aarch64 sends itself
// an SGI, x86_64 a local-APIC self-IPI, and RISC-V, which has neither, makes the console UART
// assert its own transmit-empty line into the PLIC (`console::raise_uart_interrupt`). See
// notes/interrupts.md, "Testing it on RISC-V, which has no SGI".

/// The interrupt `an_interrupt_becomes_a_message` raises.
#[cfg(target_arch = "aarch64")]
fn delivery_irq() -> u32 {
    1 // an SGI: software-triggerable, no hardware behind it
}
/// The interrupt `an_interrupt_that_arrives_before_the_wait_is_not_lost` raises. A different SGI
/// from `delivery_irq` so the two tests cannot see each other's routes.
#[cfg(target_arch = "aarch64")]
fn pending_irq() -> u32 {
    2
}

/// The console UART's PLIC source, read from the device tree as the rest of the kernel reads it
/// (`user::uart_irq_and_source`): 10 on QEMU `virt` (the `UART_RX_INTID` fallback), 32 on the
/// VisionFive 2.
#[cfg(target_arch = "riscv64")]
fn delivery_irq() -> u32 {
    crate::user::uart_irq_and_source().0
}
/// **The same source, deliberately.** RISC-V has exactly one line these tests can assert by
/// hand, so unlike aarch64's two SGIs the two tests share it. They do not collide: each rebinds
/// the route to its own rendezvous before raising, and each quiets the line before it returns.
#[cfg(target_arch = "riscv64")]
fn pending_irq() -> u32 {
    crate::user::uart_irq_and_source().0
}

/// **x86 is aarch64's case, not RISC-V's.** The local APIC delivers any vector to its own CPU on
/// demand, through the ICR and a real delivery path (the IRR, the ISR, an EOI), so no device is
/// needed and there are two independent sources. The number is the vector; see
/// `arch::x86_64::exceptions::x86_trap_body`.
#[cfg(target_arch = "x86_64")]
fn delivery_irq() -> u32 {
    crate::arch::irq::SELF_TEST_VECTOR as u32
}
/// A second vector, so the two tests cannot see each other's routes. aarch64's two SGIs.
#[cfg(target_arch = "x86_64")]
fn pending_irq() -> u32 {
    crate::arch::irq::SELF_TEST_VECTOR_B as u32
}

/// Enable the test interrupt at the controller. Nothing is raised yet.
#[cfg(target_arch = "aarch64")]
fn arm_test_irq(intid: u32) {
    crate::arch::irq::enable(intid); // SGI: per-core, no target
}

#[cfg(target_arch = "riscv64")]
fn arm_test_irq(intid: u32) {
    // The affinity policy picks (and remembers) which hart's PLIC context this source lands on,
    // exactly as it does for a real driver's line. The handler masks the source when it fires,
    // so this also re-enables it for the second of the two tests.
    crate::arch::irq::enable(intid);
}

/// **Nothing to arm.** An IPI is not a line: it has no mask bit at any controller, because
/// nothing outside the CPU asserts it. `irq::enable` here takes a *legacy IRQ* and writes an IO
/// APIC redirection entry, which is the wrong device entirely for this source.
#[cfg(target_arch = "x86_64")]
fn arm_test_irq(intid: u32) {
    let _ = intid;
}

/// Raise it.
#[cfg(target_arch = "aarch64")]
fn raise_test_irq(intid: u32) {
    // Self, by asking rather than by assuming core 0: the test thread runs wherever the
    // scheduler put it, and a fixed target is the count-as-index disease in miniature.
    crate::arch::irq::send_sgi(intid, crate::cpu::id());
}

#[cfg(target_arch = "riscv64")]
fn raise_test_irq(intid: u32) {
    // The console UART's line is the only one this ISA can assert by hand, so a caller naming
    // any other source would silently raise the wrong interrupt and then wait for one that
    // never came, which reads as a kernel bug rather than a test bug.
    debug_assert_eq!(
        intid,
        delivery_irq(),
        "riscv can only raise the console UART's own line by hand"
    );
    crate::console::raise_uart_interrupt();
}

#[cfg(target_arch = "x86_64")]
fn raise_test_irq(intid: u32) {
    debug_assert!(
        intid == delivery_irq() || intid == pending_irq(),
        "only the two self-IPI test vectors are raisable this way; {intid} is not one"
    );
    crate::arch::irq::raise_self_interrupt(intid as u8);
}

/// Lower it again, so the next test starts from a quiet line.
#[cfg(target_arch = "aarch64")]
fn quiet_test_irq() {
    // An SGI is edge-triggered and one-shot: there is no line to lower.
}

#[cfg(target_arch = "riscv64")]
fn quiet_test_irq() {
    crate::console::quiet_uart_interrupt();
}

/// An IPI is edge-delivered and one-shot: there is no asserted line to lower, which is aarch64's
/// SGI case rather than RISC-V's held UART line.
#[cfg(target_arch = "x86_64")]
fn quiet_test_irq() {}

/// A spawned thread actually runs, and its closure's captured state comes with it.
#[test_case]
fn a_spawned_thread_runs() {
    static RAN: AtomicBool = AtomicBool::new(false);
    static SAW: AtomicU64 = AtomicU64::new(0);

    let captured = 0xdead_beefu64;
    crate::sched::spawn(move || {
        SAW.store(captured, Ordering::SeqCst);
        RAN.store(true, Ordering::SeqCst);
    })
    .expect("spawn failed");

    // Wait until it has had a turn (on this or another core, since §28).
    spin_until(|| RAN.load(Ordering::SeqCst));

    assert!(RAN.load(Ordering::SeqCst), "the thread never ran");
    assert_eq!(
        SAW.load(Ordering::SeqCst),
        0xdead_beef,
        "the closure's captured value did not survive the switch"
    );
}

/// The canary tripwire's contract, both halves: an unchanged watched range reports nothing, and a
/// byte flipped behind its back is counted (and printed) on the next check. The scratch is this
/// test's own static, so the live registries are never poked; arming over the real tables is
/// `canary_arm_registries`, plain plumbing over the same `arm`.
///
/// Both checks loop until a pass of this core's own runs: `check()` is single-flight, and timer
/// ticks on other cores call it too, so a tick's pass that read the byte before the flip can take
/// the slot and return `false` here (notes/cpu-models.md BUGS).
#[test_case]
fn the_canary_reports_a_byte_that_changed_behind_its_back() {
    use core::sync::atomic::AtomicU8;
    static SCRATCH: [AtomicU8; 32] = [const { AtomicU8::new(0xA5) }; 32];
    let base = SCRATCH.as_ptr() as usize;
    super::canary::arm(&[(base, 32)]);
    while !super::canary::check() {
        core::hint::spin_loop();
    }
    assert_eq!(
        super::canary::divergences(),
        0,
        "an unchanged range must not diverge"
    );
    SCRATCH[7].store(0x5A, Ordering::Relaxed);
    // The timer's own check may absorb the flip before this completed pass; either way the
    // count is visible here (this core ran a full pass after the store, and taking the gate
    // acquires whatever an earlier pass counted before releasing it).
    while !super::canary::check() {
        core::hint::spin_loop();
    }
    assert!(
        super::canary::divergences() >= 1,
        "a flipped watched byte must be reported"
    );
    super::canary::disarm();
}

/// Several threads take turns.
#[test_case]
fn threads_round_robin() {
    static COUNTS: [AtomicU64; 3] = [AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0)];
    static STOP: AtomicBool = AtomicBool::new(false);

    let mut tids = [0 as crate::thread::ThreadId; 3];
    for (t, c) in tids.iter_mut().zip(&COUNTS) {
        *t = crate::sched::spawn(move || {
            while !STOP.load(Ordering::SeqCst) {
                c.fetch_add(1, Ordering::SeqCst);
                crate::sched::yield_now();
            }
        })
        .expect("spawn failed");
    }

    // Wait on the property (every thread has run), clock-bounded, not on a yield count.
    let all_ran = || COUNTS.iter().all(|c| c.load(Ordering::SeqCst) > 0);
    let ran = wait_for(all_ran);
    STOP.store(true, Ordering::SeqCst);
    assert!(ran, "a spawned thread never ran");

    // Wait for the exits, so three threads mid-teardown are not what a later test's frame or
    // thread accounting finds in flight.
    assert!(
        wait_for(|| tids.iter().all(|&t| !crate::sched::is_thread_present(t))),
        "the round-robin threads were never reaped"
    );
}

/// **THE TEST**: a thread that never yields is preempted anyway.
///
/// DECISIONS §5 (preemptive threads with real stacks): "Under cooperative scheduling, one bad user
/// program hangs the machine permanently." The spinner is a tight loop with no yield, no syscall
/// and no call, nothing a cooperative scheduler could hook; only a timer interrupt can take its
/// core back. If this passes, the kernel can host untrusted code. See notes/scheduler/switch.md.
#[test_case]
fn a_thread_that_never_yields_is_preempted_anyway() {
    static SPINNING: AtomicU64 = AtomicU64::new(0);
    static STOP: AtomicBool = AtomicBool::new(false);
    static OTHER_RAN: AtomicBool = AtomicBool::new(false);

    // Both threads pinned to this core, so this stays a same-core preemption test: a spinner on
    // another idle core would never need preempting.
    let here = crate::cpu::id();

    // The hostile thread. This is the arbitrary ELF binary, in miniature.
    crate::sched::spawn_on(here, || {
        while !STOP.load(Ordering::Relaxed) {
            SPINNING.fetch_add(1, Ordering::Relaxed);
            // Deliberately nothing else. No yield. No call. Nothing to cooperate with.
        }
    })
    .expect("spawn failed");

    // The spinner reaching a CPU is a precondition, so it is waited on before the polite thread
    // exists: sampled afterwards, STOP could beat its first turn. The polite thread's turn can
    // then only come from preempting a thread that was genuinely running.
    assert!(
        within_ticks(200, || SPINNING.load(Ordering::Relaxed) > 0),
        "the spinner never reached a CPU in 200 tick periods: it was placed on a run queue \
         and never scheduled, which is a placement or wake failure rather than a preemption one"
    );

    // Counted from here, so the preemptions this test claims are the ones that gave the polite
    // thread its turn, not the ones that started the spinner.
    let preemptions_before = crate::sched::preemptions();

    // A well-behaved thread that just wants a turn.
    crate::sched::spawn_on(here, || {
        OTHER_RAN.store(true, Ordering::SeqCst);
    })
    .expect("spawn failed");

    // And now we wait, WITHOUT yielding either. If preemption does not work, nobody moves and
    // the budget runs out. The budget is 200 *delivered ticks* rather than one second of wall
    // clock: preemption opportunities are what this claim is counted in, and a host that
    // deschedules the emulator produces fewer of them, never more. See `within_ticks`.
    assert!(
        within_ticks(200, || OTHER_RAN.load(Ordering::SeqCst)),
        "TWO HUNDRED TICKS AND THE POLITE THREAD NEVER RAN. The spinner still owns the CPU, \
         which means preemption is not working and a single bad program can hang this \
         machine. This is precisely the failure DECISIONS §5 predicted for \
         cooperative scheduling."
    );

    // The spinner must have spun before it is stopped, or the test is vacuous. Its own second,
    // since the wait above may have spent the first.
    let spin_deadline = crate::arch::timer::now() + crate::arch::timer::frequency();
    while SPINNING.load(Ordering::Relaxed) == 0 {
        assert!(
            crate::arch::timer::now() < spin_deadline,
            "the spinner never ran at all, so nothing was monopolizing this core and the \
             polite thread's turn proves nothing about preemption"
        );
        core::hint::spin_loop();
    }

    STOP.store(true, Ordering::Relaxed);

    assert!(
        crate::sched::preemptions() > preemptions_before,
        "the CPU was never taken away from anyone: no preemption happened"
    );

    // Let the spinner notice STOP and exit, so it does not haunt the rest of the suite.
    for _ in 0..50 {
        crate::sched::yield_now();
    }
}

/// A finished thread's stack is unmapped and its frames returned.
///
/// The reaping cannot happen in `exit()`: a thread cannot unmap the stack it is standing
/// on. It happens in `schedule()`, from the *next* thread, once we are safely off it. Every
/// kernel has something called a reaper, and this is why.
#[test_case]
fn a_finished_thread_is_reaped_and_its_memory_returned() {
    // Reaping is proven per thread, by `is_thread_present` on this test's own ThreadIds, not by
    // the table headcount, which a neighbour's late teardown moves
    // (notes/load-sensitive-assertions.md). The probe reports its own stack address, because a
    // test cannot read a stack out of a thread that may already be reaped.
    static PROBE_SP: AtomicU64 = AtomicU64::new(0);

    fn batch_of_eight() {
        let mut tids = [0 as crate::thread::ThreadId; 8];
        for t in &mut tids {
            *t = crate::sched::spawn(|| {}).expect("spawn failed");
        }
        // Let them all run and exit, and let the reaper catch up. Clock-bounded, not yield-bounded:
        // §28 can place these on other cores, and a Finished thread is only removed when its own
        // core switches away from it, so no number of yields *here* can make that happen.
        assert!(
            wait_for(|| tids.iter().all(|&t| !crate::sched::is_thread_present(t))),
            "finished threads were never reaped"
        );
    }

    // The FIRST batch legitimately costs a couple of frames: the stack area is a fresh
    // region of virtual address space, so `map_page` has to build an L2 and an L3 page
    // table for it. Those are a one-time cost, not a leak: `unmap_page` frees the leaf
    // mapping but leaves the intermediate tables standing, on purpose (notes/teardown.md).
    batch_of_eight();

    // Stack reuse is asserted directly; the frame count below is near-blind to the leak in this
    // test's name (eight 28 KiB slots against a 2 MiB table span). One thread spawned after the
    // batch is reaped must land below the watermark that stood before it; one, because the
    // watermark is the high-water mark of concurrent threads and a batch can legitimately raise
    // it. Before the frame baseline, so the settle absorbs its frees. See
    // notes/load-sensitive-assertions/stack-reuse-and-proxy-detectors.md.
    let watermark = crate::thread::stack_area_span().1;
    PROBE_SP.store(0, Ordering::SeqCst);
    let probe = crate::sched::spawn(|| {
        let local = 0u64;
        PROBE_SP.store(&local as *const u64 as u64, Ordering::SeqCst);
    })
    .expect("spawn failed");
    assert!(
        wait_for(|| !crate::sched::is_thread_present(probe)),
        "the stack-reuse probe was never reaped"
    );
    let probe_sp = PROBE_SP.load(Ordering::SeqCst);
    assert!(
        probe_sp != 0,
        "the stack-reuse probe never reported which stack it got"
    );
    assert!(
        probe_sp < watermark,
        "a thread spawned after eight were reaped was given FRESH stack address space: its sp \
         {probe_sp:#x} is at or above the {watermark:#x} watermark that stood before it, so a \
         dead thread's range was not reused and an L2 plus an L3 page table leak per 2 MiB \
         consumed, forever"
    );

    // Sample the frame baseline only once it has STOPPED MOVING: a reaped thread's stack
    // frames are freed by `finish_switch` on whatever core reaps it, a beat after the thread
    // leaves the table, so reading `used` the instant the Tids are gone races the first
    // batch's own frees. Two agreeing samples a yield apart mean nothing is in flight, and
    // `wait_for`'s deadline keeps a genuinely unstable allocator a failure rather than a spin.
    let used = || crate::memory::stats().unwrap().used;
    let mut last = used();
    assert!(
        wait_for(|| {
            crate::sched::yield_now();
            let prev = core::mem::replace(&mut last, used());
            prev == last
        }),
        "frame accounting never settled after the first batch"
    );
    let before = last;

    // The SECOND batch must allocate NOTHING it keeps. The page tables exist, and the dead
    // threads' virtual address ranges went back on the free list, so eight new threads land
    // in the same addresses with the same tables.
    //
    // If this ever regresses, the kernel leaks two frames of page tables per 2 MiB of stack
    // address space consumed, forever, and threads come and go.
    batch_of_eight();

    // `<=`, not `==`: a leak keeps `used` above `before`, and a neighbour's late teardown can only
    // free frames. This catches a per-thread frame the reaper failed to return; the reuse claim
    // above covers the defect in the test's name. The message prints the count the wait decided
    // on, so a `false` return leaves `seen > before` (notes/load-sensitive-assertions.md).
    let mut seen = before;
    let came_back = wait_for(|| {
        seen = used();
        seen <= before
    });
    assert!(
        came_back,
        "a second batch of eight threads leaked {} frames: stack address ranges are not \
         being reused, so page tables accumulate forever",
        seen - before
    );
}

/// Every thread stack has a guard page.
///
/// A thread stack is 24 KiB (under half the boot stack's), and threads are where deep
/// recursion actually happens. Milestone 3's stack overflow hung the machine for 150
/// seconds; a guard page turns the same bug into an instant fault naming the exact byte.
#[test_case]
fn every_thread_stack_has_a_guard_page() {
    use crate::arch::mmu;
    use crate::thread::{KernelStack, STACK_PAGES};

    let stack = KernelStack::new().expect("could not allocate a thread stack");

    assert_eq!(
        mmu::translate(stack.guard()),
        None,
        "a thread stack's guard page IS MAPPED: an overflow would silently eat whatever is \
         below it"
    );

    // And the stack itself is real, writable memory directly above the hole.
    for i in 0..STACK_PAGES as u64 {
        let va = stack.bottom() + i * 4096;
        let (_, flags) = mmu::translate(va).expect("thread stack page is not mapped");
        assert!(flags.is_writable());
        assert!(
            !flags.is_kernel_executable(),
            "a thread stack is EXECUTABLE"
        );
    }
}

/// **A fatal fault can name the stack it fell off**, for all three kinds of kernel stack.
///
/// `guard_page_at` is the slot arithmetic a guard-page fault report needs, so the machine names
/// the stack rather than a reader doing it by hand. The three kinds are allocated three ways (a
/// linker symbol, a `.bss` array, a slot in the virtual area 64 GiB up), so one of each is
/// checked, and so are the negatives: a classifier answering `Thread(0)` everywhere would pass the
/// positives. See notes/stack/kernel-stack-freed-under-its-owner.md.
#[test_case]
fn a_guard_page_fault_names_its_stack() {
    use crate::arch::mmu;
    use crate::stack::{GuardPage, guard_page_at};
    use crate::thread::{KernelStack, STACK_SLOT_SPAN};

    assert_eq!(guard_page_at(mmu::stack_guard()), Some(GuardPage::Boot));
    assert_eq!(
        guard_page_at(mmu::stack_guard() + 4095),
        Some(GuardPage::Boot),
        "the last byte of the guard page is still the guard page"
    );
    assert_eq!(
        guard_page_at(mmu::stack_bottom()),
        None,
        "the first usable byte of the stack is not a guard page"
    );

    // Slot 1's guard address is a static layout fact: the classifier does range math over every
    // slot, online or parked, so this needs no core 1.
    let g = crate::smp::secondary_stack_guard(1);
    assert_eq!(guard_page_at(g), Some(GuardPage::Secondary(1)));

    // A live thread stack, so the watermark provably covers it.
    let stack = KernelStack::new().expect("could not allocate a thread stack");
    let slot = (stack.guard() - crate::thread::stack_area_span().0) / STACK_SLOT_SPAN;
    assert_eq!(guard_page_at(stack.guard()), Some(GuardPage::Thread(slot)));
    assert_eq!(
        guard_page_at(stack.bottom()),
        None,
        "the stack's own first page reported as its guard page"
    );
    assert_eq!(
        guard_page_at(stack.top() - 8),
        None,
        "the top of the stack reported as a guard page"
    );

    // Kernel text is not a stack of any kind.
    assert_eq!(guard_page_at(guard_page_at as *const () as u64), None);
}

/// **A dead thread that has not left its own kernel stack is not reapable**: `depart` publishes a
/// supervised thread as `Dead` and wakes its supervisor before it reaches `switch_to`, and a reap
/// in that window unmaps the stack a core is still running on.
///
/// Stated over `(state, on_cpu)` because the window is a few hundred instructions on two cores and
/// cannot be staged. See notes/stack/kernel-stack-freed-under-its-owner.md.
#[test_case]
fn a_dead_thread_still_standing_on_its_stack_is_not_reapable() {
    use super::{RegionReap, State, region_reap_verdict};

    // Off its stack: the states that mean "never runs again" really are reapable, which is
    // what makes region teardown work at all.
    for state in [State::Dead, State::Finished, State::Embryo] {
        assert_eq!(
            region_reap_verdict(state, false),
            RegionReap::Reap,
            "{state:?} with no core on its stack must be reapable",
        );
    }

    // Still on its stack: refused, and refused WITHOUT arming a kill, because there is nothing
    // left to kill and the condition clears itself one context switch from now.
    for state in [State::Dead, State::Finished] {
        assert_eq!(
            region_reap_verdict(state, true),
            RegionReap::RefuseStanding,
            "{state:?} still standing on its kernel stack must not have that stack unmapped",
        );
    }

    // A thread that can still be scheduled is the older refusal, and it still arms the kill.
    for state in [State::Ready, State::Running] {
        assert_eq!(
            region_reap_verdict(state, false),
            RegionReap::RefuseAndArm,
            "a live {state:?} thread must be refused and armed",
        );
        assert_eq!(
            region_reap_verdict(state, true),
            RegionReap::RefuseAndArm,
            "being on a cpu must not downgrade a live thread's refusal to the passive one",
        );
    }

    // A blocked resident is finished in place, not armed: the arm is spent by `schedule()` only
    // on a `Running` thread, so arming one that never runs again would refuse forever.
    assert_eq!(
        region_reap_verdict(State::Blocked, false),
        RegionReap::FinishInPlace,
        "a blocked resident must be ended, because arming it never reaches it",
    );

    // And a `Blocked` thread still standing on its kernel stack is the one that must NOT be
    // finished in place: freeing its `Thread` unmaps the stack a core is standing on, which is
    // the same bug the `Dead` case above pins. The refusal is passive, so the
    // owner's next retry finds it off its stack and ends it then.
    assert_eq!(
        region_reap_verdict(State::Blocked, true),
        RegionReap::RefuseStanding,
        "a blocked thread mid-switch-out must not have its stack unmapped",
    );
}

/// **The slots are contiguous, so one stack's guard page begins where the previous stack
/// ends**, and a fault report has to say which of the two it is looking at: a guard page's first
/// words are also the first words above the top of the slot below, a different bug.
///
/// `thread_stack_site` places `sp` in the same units as the faulting address. This pins its
/// arithmetic: offsets from a slot's bottom, its guard page as negative offsets, and the join,
/// where slot `n`'s guard base is one past slot `n-1`'s last usable byte.
#[test_case]
fn a_slots_guard_page_begins_where_the_slot_below_it_ends() {
    use crate::stack::thread_stack_site;
    use crate::thread::{KernelStack, STACK_PAGES, STACK_SLOT_SPAN};

    let stack = KernelStack::new().expect("could not allocate a thread stack");
    let (area, _) = crate::thread::stack_area_span();
    let slot = (stack.guard() - area) / STACK_SLOT_SPAN;

    assert_eq!(
        thread_stack_site(stack.bottom()),
        Some((slot, 0)),
        "the lowest usable byte is zero bytes above the bottom"
    );
    assert_eq!(
        thread_stack_site(stack.top() - 8),
        Some((slot, (STACK_PAGES * 4096) as i64 - 8)),
        "the last usable word is one word short of the stack's size above the bottom"
    );
    assert_eq!(
        thread_stack_site(stack.guard()),
        Some((slot, -4096)),
        "the guard page's base is a whole page below the bottom"
    );

    // The join. `top` is exclusive, so it is the next slot's guard base, and the site function
    // must report it as *that* slot's guard rather than as this one's stack.
    assert!(
        slot >= 1,
        "the first stack allocated in the suite is not slot 0"
    );
    assert_eq!(
        thread_stack_site(area + slot * STACK_SLOT_SPAN),
        Some((slot, -4096)),
    );
    assert_eq!(
        thread_stack_site(area + slot * STACK_SLOT_SPAN - 8),
        Some((slot - 1, (STACK_PAGES * 4096) as i64 - 8)),
        "the word below a slot's guard base belongs to the previous slot's stack, and a report \
         that cannot say so cannot tell an overflow from a store past a neighbour's top",
    );

    // Outside the area entirely: kernel text is not a thread stack.
    assert_eq!(
        thread_stack_site(thread_stack_site as *const () as u64),
        None
    );
}

/// **The kernel cannot spend beyond its boot carve, for stacks.** Spawn a batch of threads and
/// let them reap; the frame allocator's free count must return to where it started, because
/// kernel stacks come from the kernel's own budget region (`kmem`, carved once) and recycle
/// within it, not from the allocator.
///
/// The carve itself happens on the very first spawn ever (the idle thread, at boot), so by
/// the time this test runs the region exists and steady state is flat.
#[test_case]
fn kernel_stacks_do_not_touch_the_frame_allocator_in_steady_state() {
    // Each spawn is followed to its own reap by ThreadId, not by the table headcount, which every
    // other test's teardown moves (notes/load-sensitive-assertions.md).
    let settle = |tid| {
        assert!(
            wait_for(|| !crate::sched::is_thread_present(tid)),
            "a spawned thread was never reaped, so the frame count below would be read \
             mid-teardown"
        );
    };

    // Warm up: reach steady state (first spawn after boot may still be settling VAs).
    for _ in 0..2 {
        settle(super::spawn(|| {}).expect("warmup spawn"));
    }

    let free_before = crate::memory::stats().unwrap().free();
    for _ in 0..6 {
        settle(super::spawn(|| {}).expect("spawn failed"));
    }

    // `>=`, not `==`: the defect drives `free` down and keeps it there, and a neighbour's late
    // teardown can only free frames. The message prints the count the wait decided on.
    let mut seen = free_before;
    let recovered = wait_for(|| {
        seen = crate::memory::stats().unwrap().free();
        seen >= free_before
    });
    assert!(
        recovered,
        "six threads came and went and the frame allocator lost {} frames: a kernel stack is \
         still drawing from the allocator instead of the kernel budget",
        free_before - seen,
    );
}

/// A blocked thread is genuinely off the CPU: other threads keep running while it waits.
///
/// If `Blocked` were not respected in `schedule()`, if a blocked thread were helpfully
/// requeued: this would still pass, so it is not the whole story (the two rendezvous tests
/// above are). But it is the cheap, direct statement of what blocking is *for*: a waiting
/// thread must not burn the CPU.
#[test_case]
fn other_threads_run_while_one_is_blocked() {
    static PROGRESS: AtomicU64 = AtomicU64::new(0);
    static STOP: AtomicBool = AtomicBool::new(false);

    let region = crate::memory_region::create(1).expect("no region for a test rendezvous");
    let ep = super::create_rendezvous_from(region).expect("no rendezvous from region");

    PROGRESS.store(0, Ordering::SeqCst);
    STOP.store(false, Ordering::SeqCst);

    let blocked = super::spawn(move || {
        super::ipc_receive(ep); // blocks forever (nobody sends); must not starve the worker
    })
    .expect("spawn failed");

    let worker = super::spawn(|| {
        while !STOP.load(Ordering::SeqCst) {
            PROGRESS.fetch_add(1, Ordering::SeqCst);
            super::yield_now();
        }
    })
    .expect("spawn failed");

    // Clock-bounded, not yield-bounded: see `wait_for`.
    let progressed = wait_for(|| PROGRESS.load(Ordering::SeqCst) > 0);
    STOP.store(true, Ordering::SeqCst);

    assert!(
        progressed,
        "a worker made no progress while another thread was blocked on IPC",
    );

    // Free the blocked receiver and wait for both threads to be gone, so this test's teardown
    // does not land in a later test's accounting.
    super::ipc_send(ep, [0, 0, 0]);
    assert!(
        wait_for(|| !crate::sched::is_thread_present(blocked)
            && !crate::sched::is_thread_present(worker)),
        "this test's own threads had not finished when it returned",
    );
    crate::sched::reclaim_region(region).expect("test region would not reclaim");
}

/// **An interrupt becomes a message.** DECISIONS §10 and notes/interrupts.md, executed.
///
/// A thread blocks on a rendezvous bound to an interrupt, the test raises it from software, the
/// handler signals the rendezvous, and the thread wakes: the path a userspace driver takes. The
/// raise differs per ISA ([`raise_test_irq`]), and RISC-V's leg also covers the PLIC's
/// claim, mask and complete.
#[test_case]
fn an_interrupt_becomes_a_message() {
    static WOKE: AtomicBool = AtomicBool::new(false);

    let region = crate::memory_region::create(1).expect("no region for a test rendezvous");
    let ep = super::create_rendezvous_from(region).expect("no rendezvous from region");
    super::bind_irq(delivery_irq(), ep);
    arm_test_irq(delivery_irq());

    let tid = super::spawn(move || {
        super::ipc_receive(ep); // blocks until the interrupt fires
        WOKE.store(true, Ordering::SeqCst);
    })
    .expect("spawn failed");

    // Let the waiter run and block. It must NOT have woken: no interrupt yet.
    for _ in 0..50 {
        super::yield_now();
    }
    assert!(
        !WOKE.load(Ordering::SeqCst),
        "the thread woke before the interrupt fired",
    );

    // Fire it. The controller delivers it, the handler routes it to `ep`, the waiter wakes.
    raise_test_irq(delivery_irq());

    let woke = spin_until(|| WOKE.load(Ordering::SeqCst));
    quiet_test_irq();
    assert!(
        woke,
        "a hardware interrupt fired and the thread waiting on it never woke",
    );
    assert!(
        wait_for(|| !crate::sched::is_thread_present(tid)),
        "the interrupt waiter never exited",
    );
    crate::sched::reclaim_region(region).expect("test region would not reclaim");
}

/// **A spawn quota caps how many children a spawner can have alive, and replenishes on death.**
///
/// This is the resource-exhaustion bound from the security audit: a process cannot make the
/// kernel spawn without limit. Two threads block on an rendezvous nobody drains, holding their
/// slots; a budget of two is then exhausted and a third spawn is refused. Waking one lets it
/// exit and be reaped, which returns its slot, and a spawn succeeds again.
#[test_case]
fn a_spawn_quota_caps_live_children_and_replenishes_on_reap() {
    use core::sync::atomic::AtomicU32;
    static BUDGET: AtomicU32 = AtomicU32::new(2);

    let region = crate::memory_region::create(1).expect("no region for a test rendezvous");
    let ep = super::create_rendezvous_from(region).expect("no rendezvous from region");

    // Two children that block forever (nobody sends), each holding a quota slot.
    let first = super::spawn_with_quota(&BUDGET, move || {
        super::ipc_receive(ep);
    });
    assert!(first.is_some(), "first child should fit in the budget",);
    let second = super::spawn_with_quota(&BUDGET, move || {
        super::ipc_receive(ep);
    });
    assert!(second.is_some(), "second child should fit in the budget",);

    // Let them run and block, so both slots are genuinely held.
    for _ in 0..50 {
        super::yield_now();
    }

    // The budget is spent: a third spawn is refused, not panicked, not over-committed.
    assert!(
        super::spawn_with_quota(&BUDGET, || {}).is_none(),
        "the budget was exhausted but a third child spawned anyway",
    );

    // Wake one child: it exits, is reaped, and its QuotaToken returns the slot.
    super::ipc_send(ep, [0, 0, 0]);
    // Clock-bounded: the slot comes back when the child is reaped, on whichever core it ran.
    assert!(
        wait_for(|| BUDGET.load(Ordering::Relaxed) > 0),
        "a child exited but its quota slot was never returned to the budget",
    );

    // A slot is free again.
    assert!(
        wait_for(|| super::spawn_with_quota(&BUDGET, || {}).is_some()),
        "a child exited but its quota slot was never returned",
    );

    // Clean up: wake the other blocked child so it does not sit forever.
    super::ipc_send(ep, [0, 0, 0]);
    assert!(
        wait_for(|| !crate::sched::is_thread_present(first.unwrap())
            && !crate::sched::is_thread_present(second.unwrap())),
        "this test's own children had not finished when it returned",
    );
    crate::sched::reclaim_region(region).expect("test region would not reclaim");
}

/// A signal that arrives while nobody is waiting is **remembered, not lost.** An interrupt is
/// not a rendezvous: if it fires a hair before the driver calls `WAIT`, the driver must still
/// see it. The `pending` count is what closes that window.
///
/// Raised the same two ways as `an_interrupt_becomes_a_message`, with the same caveat about
/// what each ISA's raise does and does not cost (see [`raise_test_irq`]).
#[test_case]
fn an_interrupt_that_arrives_before_the_wait_is_not_lost() {
    use crate::arch::exceptions::ROUTED_IRQS;

    let region = crate::memory_region::create(1).expect("no region for a test rendezvous");
    let ep = super::create_rendezvous_from(region).expect("no rendezvous from region");
    super::bind_irq(pending_irq(), ep);
    arm_test_irq(pending_irq());

    // Fire it with NOBODY waiting. The signal must be counted.
    let routed = ROUTED_IRQS.load(Ordering::Relaxed);
    raise_test_irq(pending_irq());
    // Wait for the handler to have actually run, rather than for a fixed number of yields: a
    // yield elapses in no real time on an idle core (DECISIONS §28), and under SMP the interrupt
    // may be taken on another core entirely, so counting yields here would be counting nothing.
    let delivered = spin_until(|| ROUTED_IRQS.load(Ordering::Relaxed) > routed);
    quiet_test_irq();
    assert!(
        delivered,
        "the interrupt was raised but the handler never routed it, so this test could not \
         reach the question it exists to ask",
    );

    static SAW: AtomicBool = AtomicBool::new(false);
    let tid = super::spawn(move || {
        super::ipc_receive(ep); // must return immediately: the signal is pending
        SAW.store(true, Ordering::SeqCst);
    })
    .expect("spawn failed");

    assert!(
        spin_until(|| SAW.load(Ordering::SeqCst)),
        "an interrupt that fired before the WAIT was lost",
    );
    assert!(
        wait_for(|| !crate::sched::is_thread_present(tid)),
        "the interrupt waiter never exited",
    );
    crate::sched::reclaim_region(region).expect("test region would not reclaim");
}

/// **An interrupt raised while its line is masked is delivered when the driver ACKs.** The
/// handler masks a routed line when it fires and `Irq::ACK` unmasks it, so the ACK alone must
/// deliver anything raised meanwhile. On the PLIC the claim is the mask and the ACK the completion
/// (`drivers::plic`), the leg this test exists for. The line is lowered and raised again while
/// masked because QEMU's PLIC latches pending only on a rising edge. On x86_64 the self-IPI has no
/// mask, so that leg proves only two raises, two messages.
///
/// Name: provisional (the USB keyboard lost-wakeup lane, 2026-10-05).
///
/// Falsification: replayable `kernel/falsifications/sched.tests.an_interrupt_raised_while_its_line_is_masked_is_delivered_at_the_ack.patch`
#[test_case]
fn an_interrupt_raised_while_its_line_is_masked_is_delivered_at_the_ack() {
    use crate::arch::exceptions::ROUTED_IRQS;

    let region = crate::memory_region::create(1).expect("no region for a test rendezvous");
    let ep = super::create_rendezvous_from(region).expect("no rendezvous from region");
    super::bind_irq(pending_irq(), ep);
    arm_test_irq(pending_irq());

    // The first raise: the handler claims and routes it, and the claim holds the line until an ACK.
    let routed = ROUTED_IRQS.load(Ordering::Relaxed);
    raise_test_irq(pending_irq());
    let first = spin_until(|| ROUTED_IRQS.load(Ordering::Relaxed) > routed);
    quiet_test_irq();
    assert!(
        first,
        "the first interrupt was never routed, so this test could not reach its question",
    );
    // **Let the first handler finish.** `ROUTED_IRQS` moves before the handler is done with the
    // interrupt (the GIC's end-of-interrupt, for one). If that hart's last write landed after the
    // ACK below, it could deliver the second interrupt itself and the test would pass for a reason
    // that is not the ACK. The handler is a few instructions behind the count; 20 ms is margin,
    // not a guess at it.
    let settle = crate::arch::timer::now() + crate::arch::timer::frequency() / 50;
    while crate::arch::timer::now() < settle {
        super::yield_now();
    }

    // The second raise lands while the line is masked: the device spoke while its driver was
    // busy. Then the driver's ACK, which is `arch::irq::enable`, the same call `Irq::ACK` makes.
    let routed = ROUTED_IRQS.load(Ordering::Relaxed);
    raise_test_irq(pending_irq());
    arm_test_irq(pending_irq());
    let second = spin_until(|| ROUTED_IRQS.load(Ordering::Relaxed) > routed);
    quiet_test_irq();
    assert!(
        second,
        "an interrupt raised while its line was masked was never delivered after the ACK \
         unmasked it: the driver would sleep in WAIT with its device's interrupt pending",
    );

    // Both are messages: a driver waiting now collects two signals and does not block.
    static SAW: AtomicU64 = AtomicU64::new(0);
    let tid = super::spawn(move || {
        super::ipc_receive(ep);
        super::ipc_receive(ep);
        SAW.store(2, Ordering::SeqCst);
    })
    .expect("spawn failed");
    assert!(
        spin_until(|| SAW.load(Ordering::SeqCst) == 2),
        "two interrupts were routed but the waiter did not collect two signals",
    );
    assert!(
        wait_for(|| !crate::sched::is_thread_present(tid)),
        "the interrupt waiter never exited",
    );
    crate::sched::reclaim_region(region).expect("test region would not reclaim");
}

/// **A kill armed while a kernel thread builds a user address space waits until the space is
/// adopted.** `user::run_with` holds the space it is loading as a local on its own stack, and
/// `schedule` never resumes a thread it converts, so a kill carried out mid-load loses that space
/// for the boot.
///
/// The thread is killed inside its window and then passes through `schedule` three times. It must
/// survive all three, and must then die in `allow_kill` rather than run past it.
///
/// Falsification: replayable `kernel/falsifications/sched.tests.a_kill_armed_while_a_space_is_built_waits_for_it_to_be_adopted.patch`
#[test_case]
fn a_kill_armed_while_a_space_is_built_waits_for_it_to_be_adopted() {
    static INSIDE: AtomicBool = AtomicBool::new(false);
    static ARMED: AtomicBool = AtomicBool::new(false);
    static SURVIVED: AtomicBool = AtomicBool::new(false);
    static RAN_PAST: AtomicBool = AtomicBool::new(false);

    let tid = crate::sched::spawn(|| {
        crate::sched::defer_kill();
        INSIDE.store(true, Ordering::SeqCst);
        while !ARMED.load(Ordering::SeqCst) {
            core::hint::spin_loop();
        }
        for _ in 0..3 {
            crate::sched::yield_now();
        }
        SURVIVED.store(true, Ordering::SeqCst);
        crate::sched::allow_kill();
        RAN_PAST.store(true, Ordering::SeqCst);
    })
    .expect("spawn failed");

    assert!(
        wait_for(|| INSIDE.load(Ordering::SeqCst)),
        "the builder never ran"
    );
    assert!(
        crate::sched::kill_thread(tid),
        "the builder was gone before the kill"
    );
    ARMED.store(true, Ordering::SeqCst);
    assert!(
        wait_for(|| !crate::sched::is_thread_present(tid)),
        "a killed builder was never reaped: allow_kill did not carry the kill out"
    );
    assert!(
        SURVIVED.load(Ordering::SeqCst),
        "a kill armed inside defer_kill's window was carried out at a schedule inside it, which \
         is the point where an un-adopted address space is lost"
    );
    assert!(
        !RAN_PAST.load(Ordering::SeqCst),
        "the builder ran past allow_kill with a kill armed on it"
    );
}
