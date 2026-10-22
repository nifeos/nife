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

/// Wait for `cond`, bounded by the CLOCK rather than by a yield count.
///
/// These tests used to spin a fixed number of yields and then assert. A yield count is not a
/// duration: on a loaded host, or once §28's placement scattered work across cores, this core can
/// burn two hundred cheap yields long before the threads it is waiting on have been scheduled at
/// all. That is not a hang, it is an impatient observer, and it fails an assertion that describes
/// the system rather than the test.
///
/// It has now bitten three times in one day, in three different files: the reap waits in
/// `user.rs`, three spin counts in `smp.rs`, and here. The third time was a gate run failing with
/// "finished threads were never reaped, left: 11, right: 5" while a leaked QEMU held 199% of the
/// host, which is exactly the condition a yield count cannot survive and a clock can.
///
/// Two seconds is far beyond any honest completion here (these are milliseconds when the machine
/// is quiet) and well inside the harness's 90 s per-test ceiling, which remains the backstop for a
/// genuine hang. Still a leak trap rather than a masked failure: work that never completes times
/// out, and the caller's assertion then reports what was actually wrong.
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

/// Wait for `cond` **without yielding**, budgeted in timer ticks delivered to this core rather
/// than in wall-clock time.
///
/// For `a_thread_that_never_yields_is_preempted_anyway`, which must not yield (that is the
/// whole point of it) and is waiting for a *preemption*. A tick is when a preemption can
/// happen, so the number of preemption opportunities that went by is what the claim is about.
/// Why that unit survives a contended host where a `timer::now()` deadline does not is
/// [`crate::testing::TickBudget`]'s argument, and the re-anchoring on migration is its
/// mechanism; here the migration is additionally the news this test is waiting for.
///
/// If ticks stop arriving altogether this does not return, and the harness's 90 s per-test
/// ceiling is the backstop: a timer that is not delivering is the arch timer tests' failure to
/// report, not this one's.
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
// The two delivery tests below (`an_interrupt_becomes_a_message` and
// `an_interrupt_that_arrives_before_the_wait_is_not_lost`) are portable: the property they check
// is that the kernel turns an interrupt into a message and does not lose one that arrives early,
// and neither of those is architectural. Only the *trigger* is, and it is genuinely asymmetric,
// so it lives here in three small functions rather than in a comment claiming parity.
//
// aarch64 has a software-generated interrupt (an SGI), so it needs no device whatsoever.
//
// RISC-V has nothing of the kind. `sip.SEIP` is read-only to S-mode (only the PLIC, driven by a
// real wire, sets it), the PLIC's pending block is read-only by specification, and the one
// interrupt S-mode can raise on itself is the SBI's IPI, which arrives as a *software* interrupt
// (`scause` = 1) down a different arm of `riscv_trap_body` than a device's, touching neither
// `irq_route` nor `irq_notify`. Using it would have looked like parity and proved nothing.
//
// So RISC-V uses the smallest real interrupt line it can assert by hand: the console UART's own
// transmit-empty interrupt, which a 16550 raises the instant it is enabled, because the
// transmitter of a polling console is always empty. No transfer, no external stimulus, nothing
// to read back, and it is 16550 architecture rather than a QEMU behaviour, so it should carry to
// a real part. See `console::raise_uart_interrupt` and notes/interrupts.md.

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

/// The NS16550's PLIC source on QEMU `virt`. A board constant, hardcoded identically on
/// main.rs's boot-tour and shell paths; another board would give its UART a different number,
/// and this is one of the places that would have to learn it from the device tree.
///
/// On the VisionFive 2 the boot summary reads `uart irq : source 32 (device tree)`, so
/// `DELIVERY_IRQ = 10` causes `raise_uart_interrupt` on IRQ 10 to never reach the thread
/// waiting on the route bound to the real IRQ 32. The test now reads the DTB-driven value the
/// rest of the kernel already uses (`user::uart_irq_and_source`); on QEMU `virt` that gives 10
/// (via the fallback `UART_RX_INTID`), on the VF2 it gives 32.
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

/// **x86 is aarch64's case, not RISC-V's**, and this is the third answer to the question those
/// two comments have been circling. The local APIC will deliver any vector to its own CPU on
/// demand, through the ICR and a real delivery path (the IRR, the ISR, an EOI), so this ISA
/// needs no device to raise an interrupt by hand and gets two independent sources rather than
/// one shared line. The number **is the vector**, because a local APIC source has no controller
/// input to name; see `arch::x86_64::exceptions::x86_trap_body`.
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

/// **A wake with nothing delivered must not complete a parked receiver's `RECEIVE`** (boot 8,
/// VisionFive 2, 2026-08-14). The bench dump's shape: the boot thread, parked in `ipc_receive`
/// on the report rendezvous, took a `wake:0x0` on a boot where no sender to that rendezvous
/// existed, and its receive neither completed with a message nor re-parked. The receive tail reads
/// the mailbox unconditionally after `schedule()` returns, so an undelivered wake completes
/// the receive with whatever the mailbox happened to hold, and the receiver's TCB is still
/// linked on the rendezvous's wait queue (the waker that owns the unlink never ran), which is
/// the intrusive one-link invariant broken in kernel memory.
///
/// The claim: a `Blocked` IPC thread may only become `Ready` by the hand that completed its
/// rendezvous (message staged, signal counted, or abort flagged). An undelivered wake is
/// refused, the receiver stays parked, and a real sender still reaches it afterwards.
/// The canary tripwire's contract, both halves: an unchanged watched range reports nothing,
/// and a byte flipped behind its back is counted (and printed) on the next check. The scratch
/// is this test's own static, so the live registries are never poked; arming over the real
/// tables is `canary_arm_registries`, which is plain plumbing over the same `arm`.
///
/// Both checks LOOP until a pass actually runs, and the loop is the fix for a real flake
/// (thead-c906, 2026-08-15; notes/cpu-models.md BUGS): `check()` is single-flight, timer
/// ticks on other cores call it too (secondaries are online here), and this test's decisive
/// call used to lose the slot to a tick's pass that had read the scratch byte *before* the
/// flip. The old `check()` swallowed that refusal and the flip went uncounted; now it says
/// `false` and the test insists on a pass of its own.
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

    // Wait ON the property (every thread has run), clock-bounded, rather than asserting after
    // a fixed 300 yields. A yield count is not a duration: §28 scatters these threads across
    // cores, and on a contended host this core can burn its yields before a starved vCPU has
    // run its thread at all, which failed a gate run once as "thread {i} never ran" and passed
    // on the re-run. Widening a wait on the property itself only delays noticing (the smp.rs
    // wait_for argument); the watchdog stays the backstop for a genuine starvation.
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

/// **THE TEST.**
///
/// From DECISIONS §5, written before a single line of this kernel existed:
///
/// > A userspace process is an arbitrary ELF binary. It has its own stack, **it never
/// > yields**, and it will loop forever because we will write a bug. Under cooperative
/// > scheduling, one bad user program hangs the machine permanently.
///
/// So: a thread whose entire body is a tight loop. **No `yield_now`. No syscall. Not even a
/// function call**: nothing a cooperative scheduler could possibly hook.
///
/// Under async/await, or Go before 1.14, or any cooperative runtime, this thread takes the
/// CPU and never gives it back, and the machine is gone. The only thing that can take it
/// back is a timer interrupt landing between two instructions of that loop and switching
/// the stack out from under it.
///
/// If this test passes, the argument was right and the kernel can host untrusted code.
/// If it hangs, it was wrong.
#[test_case]
fn a_thread_that_never_yields_is_preempted_anyway() {
    static SPINNING: AtomicU64 = AtomicU64::new(0);
    static STOP: AtomicBool = AtomicBool::new(false);
    static OTHER_RAN: AtomicBool = AtomicBool::new(false);

    // Pin both threads to THIS core, the one the test thread busy-waits on, so this stays a
    // *same-core* preemption test after DECISIONS §28 made the default `spawn` scatter work
    // across cores. The claim under test is that a never-yielding thread cannot monopolize the
    // core it is on; if the spinner ran on some other idle core the timer would never have to
    // preempt anything here, and the test would prove nothing. `spawn_on(cpu::id())` keeps the
    // spinner, the polite thread, and the waiter contending for one core, as they always did.
    let here = crate::cpu::id();

    // The hostile thread. This is the arbitrary ELF binary, in miniature.
    crate::sched::spawn_on(here, || {
        while !STOP.load(Ordering::Relaxed) {
            SPINNING.fetch_add(1, Ordering::Relaxed);
            // Deliberately nothing else. No yield. No call. Nothing to cooperate with.
        }
    })
    .expect("spawn failed");

    // **Wait for the hostile thread to reach a CPU before the polite one exists.**
    //
    // The order used to be: spawn both, wait for the polite thread, set STOP, and only then
    // sample `SPINNING > 0`. That sample is a race the test creates for itself: if the polite
    // thread gets its turn first, STOP is set before the spinner has ever been scheduled, the
    // spinner exits its loop without incrementing anything, and the run goes red with "the
    // spinner never ran at all" while the kernel did nothing wrong. It failed CI exactly that
    // way on 2026-08-04 (`sifive-u54`), on a pull request that changed no kernel code.
    //
    // The spinner running is a **precondition** of the claim, not the claim, so it is waited
    // on rather than sampled. Waiting for it here also strengthens what follows: the polite
    // thread's turn can now only have come from preempting a thread that was genuinely running.
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

    // **And the spinner has to have actually spun before we stop it**, or the test is vacuous:
    // a polite thread running on a core nobody was monopolizing says nothing about preemption.
    // Stopping it the instant the polite thread reported is a race the two orderings decide,
    // and on the physical core under HVF (milestone 81) it came out the other way: the polite
    // thread ran first, `STOP` was set, and the spinner was killed before its first increment
    // ("the spinner never ran at all"). Its own second, since the wait above may have spent all
    // of the first.
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
    // Reaping is proven per thread, by `is_thread_present` on the Tids THIS test spawned, not
    // by the global table headcount returning to a baseline. `thread_count()` is the size of
    // the whole table, so a neighbour's teardown finishing late moves it: it failed on CI as
    // "left: 5, right: 6", a count BELOW its baseline, which eight reaped threads cannot
    // produce but one baseline-counted thread exiting mid-test does. Same shape as the
    // `reclaim_frees_a_started_then_exited_childs_regions` fix; see
    // notes/load-sensitive-assertions.md.
    //
    // Where the reuse probe below found its own stack, **reported by the thread itself**. A test
    // cannot read the stack out of a thread it spawned, because by the time it looks the thread
    // may already have been reaped, which is the very thing this test waits for. A thread taking
    // the address of one of its own locals has no such race.
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

    // **Reuse is asserted directly, because the frame count below cannot do it.** Measured
    // 2026-08-17: with the `FREE_STACK_ADDRESS_SPACE` push deleted from `KernelStack::drop`, which IS the
    // milestone-6 bug this test is named for, the entire aarch64 leg passed, this test included.
    // The reason is arithmetic rather than luck. A slot is `STACK_SLOT_SPAN`, 28 KiB, so eight
    // of them consume 224 KiB of fresh address space, and a leaked page table costs a *frame*
    // only when the bump crosses a 2 MiB L3 boundary. 224 KiB is 11% of one table's span, so
    // the frame count can see the defect only when the batch happens to straddle a boundary,
    // and that is worse than 11% random: where `NEXT_STACK_VA` stands here is a function of how
    // many threads the tests BEFORE this one spawned, which is fixed for a given tree. So for
    // any given tree the frame count either always catches the defect or always misses it, and
    // which one is decided by unrelated code upstream. The frame assertion below is the outcome;
    // this is the mechanism, and at that batch size only the mechanism is observable.
    //
    // The claim: a thread spawned after the first batch has been reaped lands BELOW the
    // watermark that stood before it, which is what "it reused a dead thread's range" means.
    //
    // **ONE thread, and the count is the whole argument.** The first version of this asserted it
    // for all eight of a batch and failed on a clean kernel, on thread 1, two slots above the
    // watermark. That was correct behaviour and a wrong assertion: the watermark is the
    // high-water mark of *concurrent* live threads, so a batch whose threads happen to be reaped
    // later relative to spawning legitimately needs more slots than the previous batch did, and
    // bumps it. Asserting over a batch conflated reuse with concurrency, which is this
    // milestone's own defect ("a wait written against something wider than the property")
    // committed while fixing it. Recorded in notes/load-sensitive-assertions.md rather than
    // quietly corrected, because reproducing the family from the inside is the useful part.
    //
    // One thread cannot exceed a high-water mark that eight just set. `is_thread_present` going
    // false already implies the push happened (`Threads::remove` runs `KernelStack::drop` before
    // it removes the table entry), so the free list holds up to eight of the first batch's slots
    // when this spawns, and a single pop cannot drain it. A neighbour spawning here can only
    // RAISE the watermark, which makes the claim easier: the failure direction is one-way, which
    // is the discipline the rest of this test was rebuilt for.
    //
    // It runs BEFORE the frame baseline below so that the settle loop absorbs its own stack
    // frees, rather than leaving them in flight inside the window the frame assertion measures.
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

    // `<=`, not `==`, and the direction is the argument: a leak leaves `used` ABOVE `before`
    // and never comes back, so the wait times out and fails. A neighbour's late teardown
    // landing in this window can only FREE frames, pushing `used` below `before`, and holding
    // still is not a property this test can demand of the rest of the machine.
    //
    // **This comment used to end "sensitivity to the milestone-6 bug is unchanged: every
    // leaked frame keeps `used() <= before` false", and that was true of the arithmetic and
    // false about the bug.** It is unchanged from the `==` form, which is what it was written
    // to defend, and both forms are near-blind: the defect leaks page tables per 2 MiB of
    // address space and eight threads consume 224 KiB, so there is usually no leaked frame for
    // either form to see. Corrected 2026-08-17 by deleting the VA push and watching the leg go
    // green. What this assertion is genuinely responsible for is the leak that *does* show at
    // this batch size, a per-thread frame the reaper failed to return; the reuse claim above is
    // what covers the defect in the test's name.
    //
    // The number in the message is the one the wait DECIDED on, not a fresh sample. Re-reading
    // `used()` to format the panic races the frames still arriving, so a genuine timeout could
    // report a delta of zero: `saturating_sub` clamped the negative case away instead of making
    // it impossible, and "leaked 0 frames" is the same unreadable diagnostic as the "-52" this
    // milestone is named for, minus the sign that gave it away. `wait_for` re-evaluates the
    // predicate once after its deadline, so a `false` return leaves `seen > before` and the
    // count below cannot be zero. See notes/load-sensitive-assertions.md.
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
/// Milestone 78. The guard pages already worked; nothing *said* so. Two `cpu matrix` runs died
/// with `unexpected RISC-V trap: scause=0xf stval=0xffffffd0001fe000 from_user=false`, which
/// reads as a memory-system fault, and it took hand arithmetic against `thread.rs`'s slot span
/// to discover that the address was the base of a thread stack's guard page. `guard_page_at` is
/// that arithmetic, in the kernel, so the machine says it instead.
///
/// **That `stval` is one of only two addresses this project's guard-page faults ever used**,
/// and the other is aarch64's `0xffff0010001b3000`. An earlier version of this paragraph read
/// that as proof of a fixed-site writer, on the argument that "a depth-driven overflow does not
/// repeat an address". **It does, and exactly this one** (2026-08-17): a fault that reaches the
/// exception vector's own frame store walks `sp` down a frame at a time and stores upward in
/// aligned steps, so the terminal store lands on the guard base exactly, whatever `sp` was
/// doing. The address carried no information; the *slot number* did, and it survived a change
/// to the slot span. See notes/stack/kernel-stack-freed-under-its-owner.md.
///
/// The three kinds are allocated three different ways (a linker symbol, a `.bss` array, a slot
/// in the virtual area 64 GiB up), so this checks one of each rather than trusting one to stand
/// for the others. It also checks the *negatives*, because a classifier that answered
/// `Thread(0)` for every address would pass the positive half and be worse than nothing.
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

    // Slot 1's guard address is a static layout fact, not a runtime one: the classifier does
    // range math over addresses that exist for every slot, online or parked, so this needs no
    // core 1 at all. (An earlier comment here justified the index by core 1 being online,
    // which was the wrong reason and read as an online-set assumption.)
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

/// **A dead thread that has not left its own kernel stack is not reapable**, and that one
/// clause is the whole of the bug that produced four `*** KERNEL STACK OVERFLOW ***` panics in
/// CI over five days without any stack ever overflowing.
///
/// `depart` publishes a supervised thread as `Dead` and delivers its death message, waking the
/// supervisor, *before* it reaches `switch_to`. A supervisor that reaps in that window used to
/// free the `Thread`, and `KernelStack::drop` unmaps six pages with a real `tlbi` under a core
/// that is still running on them. The corpse's next store faulted, the exception vector's own
/// frame store faulted on the same dead stack, and the vector walked `sp` down one 272-byte
/// frame at a time until it landed in the mapped stack below, which is why the reported address
/// was the slot base every single time and never moved.
///
/// The rule is stated over `(state, on_cpu)` rather than over a live thread table because the
/// window is a few hundred instructions wide on two cores, which is not a thing a test can
/// stage. It reproduced on a desk only with a deliberate spin loop inserted in `depart`; see
/// notes/stack/kernel-stack-freed-under-its-owner.md. What this pins is the claim, so the
/// next person to edit that loop meets `on_cpu` as a requirement rather than as a detail.
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

    // **`Blocked` left the arm on 2026-09-03** (milestone 133, proposal A), and it left for a
    // reason this assertion is the record of: the arm is spent by `schedule()` only for a
    // thread whose state is `Running`, so arming a thread that never runs again armed nothing
    // and refused forever. It is now finished in place.
    assert_eq!(
        region_reap_verdict(State::Blocked, false),
        RegionReap::FinishInPlace,
        "a blocked resident must be ended, because arming it never reaches it",
    );

    // And a `Blocked` thread still standing on its kernel stack is the one that must NOT be
    // finished in place: freeing its `Thread` unmaps the stack a core is standing on, which is
    // the same four-CI-panic bug the `Dead` case above pins. The refusal is passive, so the
    // owner's next retry finds it off its stack and ends it then.
    assert_eq!(
        region_reap_verdict(State::Blocked, true),
        RegionReap::RefuseStanding,
        "a blocked thread mid-switch-out must not have its stack unmapped",
    );
}

/// **The slots are contiguous, so one stack's guard page begins where the previous stack
/// ends**, and a fault report has to be able to say which of the two it is looking at.
///
/// This is the geometry that made two 2026-08-16 guard-page faults ambiguous. Both landed on a
/// slot's guard page at offset 0 and 8, and the report said "sp went 4096 bytes past the
/// bottom" without ever reading `sp`. Those same two addresses are also the first two words
/// **above the top of the stack in the slot below**, which is a completely different bug with
/// a completely different fix, and nothing printed could tell them apart.
///
/// `thread_stack_site` is what lets the report place `sp` in the same units as the faulting
/// address. The assertions below are the arithmetic that claim rests on: a slot's usable span
/// measured from its own bottom, its guard page as negative offsets, and the join, where slot
/// `n`'s guard base is exactly one past slot `n-1`'s last usable byte.
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

/// **Milestone 19c.1: the kernel cannot spend beyond its boot carve, for stacks.** Spawn a
/// batch of threads and let them reap; the frame allocator's free count must return to
/// exactly where it started, because kernel stacks now come from the kernel's own budget
/// region (`kmem`, carved once) and recycle within it, not from the allocator. This is the
/// milestone-14 no-open-ended-kernel-spending thesis extended to the last thing it missed;
/// before 19c.1 this test would show four stacks' worth of frames gone per batch.
///
/// The carve itself happens on the very first spawn ever (the idle thread, at boot), so by
/// the time this test runs the region exists and steady state is flat.
#[test_case]
fn kernel_stacks_do_not_touch_the_frame_allocator_in_steady_state() {
    // Each spawn is followed to its own reap by name. This used to take `thread_count()` as a
    // baseline and spin `while thread_count() > baseline { yield_now() }`, which is the whole
    // family in three lines: the headcount is moved by every other test's teardown, so the loop
    // could exit at once (a neighbour reaping first) and leave this batch's stacks in flight,
    // or never exit at all (a neighbour's thread outliving the batch) with no clock to stop it,
    // spinning until the harness's 90 s ceiling with a message about kernel stacks.
    // `is_thread_present` on the ThreadId each spawn returned asks the narrow question, and
    // `wait_for` supplies the bound the yield loop never had.
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

    // `>=`, not `==`, and the direction is the argument, the same one the reaper test's frame
    // half carries. The defect this guards spends allocator frames on kernel stacks, which
    // drives `free` DOWN and keeps it there, so the wait times out and fails exactly as
    // before. A neighbour's late teardown landing in this window can only FREE frames, pushing
    // `free` ABOVE the baseline, and equality additionally demanded that the rest of the
    // machine hold still for the duration, which is not a property this test is responsible
    // for. See notes/load-sensitive-assertions.md.
    //
    // And the number in the message is the one the wait decided on, for the reason the reaper
    // test's frame half carries at length: a re-sampled `saturating_sub` reports zero when the
    // frames land between the wait giving up and the panic being formatted, which is a red run
    // whose message denies there is anything wrong with it.
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

    // Clock-bounded, not yield-bounded: see `wait_for`. It failed here on `sifive-u54` and
    // `rva22s64` under eight spinning host processes on 2026-08-04, which is the same lesson
    // `threads_round_robin` learned three tests up: a hundred yields is not a duration, and on
    // a contended host this core burns them before the worker's vCPU has run at all.
    let progressed = wait_for(|| PROGRESS.load(Ordering::SeqCst) > 0);
    STOP.store(true, Ordering::SeqCst);

    assert!(
        progressed,
        "a worker made no progress while another thread was blocked on IPC",
    );

    // Free the blocked receiver so it does not sit in the rendezvous queue forever, and wait for
    // BOTH threads to actually be gone. Twenty yields used to be the wait, which is the same
    // "count is not a duration" defect one level down: this test's teardown landing late is
    // precisely the neighbouring state that made other tests' frame and thread accounting fail
    // (notes/load-sensitive-assertions.md).
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
/// A thread blocks waiting on an interrupt it can only name through an rendezvous. We raise the
/// interrupt from software, the kernel's handler turns it into a notification, and the blocked
/// thread wakes. This is the exact path a userspace driver takes when a real device interrupts.
///
/// **The two ISAs raise it differently, and they are not twins in what they cost to raise.** See
/// [`raise_test_irq`]: aarch64 sends itself a GIC SGI, which needs no device at all; RISC-V has
/// no SGI, so it makes the console UART assert its own line into the PLIC with one register
/// write. The kernel path under test is the same on both (the handler routes the interrupt to an
/// rendezvous and signals it), and RISC-V's leg additionally covers the PLIC claim/mask/complete
/// handshake that an SGI on aarch64 does not reach. What RISC-V gives up is aarch64's "minus the
/// device" property. The alternative there was the SBI's IPI, which arrives as a *software*
/// interrupt down a different arm of the trap dispatcher and would not have touched
/// `irq_route`/`irq_notify` at all, so it would have proved less while looking like more.
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

    // Wake one child. It returns from ipc_receive, its closure ends, it exits and is reaped,
    // and its QuotaToken drops, returning the slot. Clock-bounded (milestone 81): the 100
    // yields this used to spend are microseconds on the physical core, well before another
    // core has run the woken child to completion.
    super::ipc_send(ep, [0, 0, 0]);
    // Clock-bounded, not yield-bounded: see `wait_for`. The slot comes back when the child is
    // *reaped*, which happens on whichever core it ran on, so a yield count here measures this
    // core's idleness rather than that child's teardown. Waiting on the budget itself is also
    // the exact property: the assertion below is the confirmation, not the wait.
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
/// third delivery property, and the one a real driver leans on hardest: the handler masks a
/// routed line when it fires and the driver's `Irq::ACK` unmasks it, so anything the device
/// raises while the driver is busy arrives during the mask, and the ACK is the only thing that
/// can deliver it. Nothing else will: the driver goes straight back to `Irq::WAIT`.
///
/// The RISC-V leg is the reason this test exists. On the PLIC the claim is the mask and the ACK
/// is the completion (`drivers::plic`), so an ACK that failed to complete, or completed where the
/// PLIC ignores it, would leave the source claimed and the second raise undelivered for good. This
/// test raises nothing else in between, which is what makes it deterministic.
///
/// The line is lowered and raised again while masked, rather than held, because QEMU's PLIC
/// latches a source's pending bit only when its line rises: a line held high across the
/// claim would never go pending again under the emulator at all, fixed or not, and the test
/// would be asking a different question. On x86_64 the self-IPI has no mask, so the second
/// raise is simply delivered; the property holds there trivially and the leg proves the
/// portable half (two raises, two messages).
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
/// for the boot. `Holding::release` armed exactly that kill on the ntp server whenever a test ended
/// before the server had loaded, and the frame ledger read it as an overrun of 22 frames in about
/// one CI run in eight.
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
