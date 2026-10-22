//! **Spawning a thread and mapping a page**, and on `riscv64` what the remote fence a map
//! may fire costs.
//!
//! Cut out of `bench.rs` unchanged by lane `maintainer/last-three-over-2000`, which left the
//! parent an index (§282 (when a file is split, the parent becomes an index)) and under §266 (a
//! Rust source file stays under 2,000 lines).
//!
//! Name: provisional (lane `maintainer/last-three-over-2000`, 2026-10-12 UTC), for the module and
//! the file.

use super::*;

/// **Thread lifecycle, spawn to reap.** Each iteration creates a thread that exits immediately,
/// then yields until the reaper has returned the table to its baseline: TCB pool slot claim and
/// release, stack map and unmap, generational name mint and death.
pub(super) fn spawn_reap() {
    let baseline = sched::thread_count();
    let one = || {
        sched::spawn(|| {}).expect("bench: spawn failed");
        while sched::thread_count() > baseline {
            sched::yield_now();
        }
    };

    for _ in 0..4 {
        one(); // warmup: the first spawn pays for cold stack VAs
    }
    timed("spawn_reap", SPAWN_ITERS, || {
        for _ in 0..SPAWN_ITERS {
            one();
        }
    });
}

/// **Mapping a fresh page into an address space**: retype from the region, walk, write the
/// leaf. The exec path's inner loop.
pub(super) fn map_new() {
    static TOTAL: AtomicU64 = AtomicU64::new(0);

    let mut space = crate::user::AddressSpace::new(MAP_ITERS + 8).expect("bench: no address space");
    let base = 0x40_0000u64;

    // The shootdown probe (notes/benchmarks/riscv-map-new-and-the-rfence-probe.md, the 2026-08-15
    // reading). Counted across exactly the
    // timed window, because the claim under test is about what `map_new` itself issues.
    #[cfg(target_arch = "riscv64")]
    let fences_before = crate::arch::remote_fence_count();

    // Preemption is masked across the window. Milestone 541 (a timed window that excludes
    // preemption) is this, and the masking is the whole of it.
    //
    // **The window is ~250 microseconds of guest time and the scheduler tick is 10 ms** outside
    // the bench build (`TICK_HZ` 100 on all three architectures), so the window covers about 2.5%
    // of a tick period and whether a timer interrupt lands inside it is decided by the phase the
    // boot happened to leave the timer in. That phase moves when unrelated kernel code changes size.
    // On 2026-09-21 a semantically identical rewrite of one expression in `schedule()` moved it,
    // one preemption landed inside this window, and `map_new` read **+26.4%** and failed the 10%
    // tripwire while the map path's object code was byte-identical (notes/benchmarks.md).
    //
    // Masking removes the coin flip rather than diluting it: with the same perturbation applied,
    // `map_new` measures 180,537 either way on x86_64, where it measured 180,604 and 228,356
    // before. The interrupt is not lost, only deferred: it is pending at `restore` below and the
    // preemption is taken there, outside the window, which is why the probe can still report 1.
    //
    // This is a different measurement under `-icount` and the trade is deliberate. All vCPUs
    // share one virtual clock, so a window that masks interrupts is a window that excludes
    // whatever the rest of the machine would have done inside it. That is exactly what is
    // wanted here: the row's job is the cost of mapping a page, not the cost of being
    // descheduled while mapping one. Rows that mean to measure scheduling (`yield_switch`,
    // `spawn_reap`) do not do this and must not.
    //
    // Safe because nothing on this path blocks: `map_new` retypes from a region, walks the
    // table and writes a leaf, taking only spin locks. A row whose body can block cannot use
    // this.
    let preemptions_before = sched::preemptions_here();

    let window = TickWindow::open("map_new");
    let irqs_were_on = crate::arch::interrupts::disable();
    measure("map_new", MAP_ITERS, || {
        for i in 0..MAP_ITERS {
            let page = space
                .map_new(
                    base + i * page_frames::FRAME_SIZE,
                    paging::Flags::user_data(),
                )
                .expect("bench: map failed");
            // Touch it so the compiler cannot dissolve the loop.
            TOTAL.fetch_add(page[0] as u64, Ordering::Relaxed);
        }
    });
    crate::arch::interrupts::restore(irqs_were_on);
    window.close();

    // **A probe, not a row**, for the reason the fence probe below states: it is a count, and a
    // 10% tolerance over a count is not a tripwire. It is what makes the masking above auditable
    // instead of asserted. Read it as "how many preemptions this window did not pay for": a
    // non-zero value here is the deferred one taken at `restore`, not one inside the window.
    println!(
        "bench-probe: map_new_preemptions {} over {MAP_ITERS} iters",
        sched::preemptions_here() - preemptions_before
    );

    // **Not a `timed` line, and deliberately not on the baseline.** These are counts and a bitmask,
    // not durations: putting them through `timed` would invite `--check` to police them with a 10%
    // tolerance, when the only interesting values are exact (`0` fences, one bit set). They are
    // printed next to `map_new` because that is the benchmark whose reading they settle.
    #[cfg(target_arch = "riscv64")]
    {
        println!(
            "bench-probe: map_new_remote_fences {} over {MAP_ITERS} iters",
            crate::arch::remote_fence_count() - fences_before
        );
        println!(
            "bench-probe: online_harts_mask {:#x} ({} online, this hart {})",
            crate::smp::online_harts_mask(),
            crate::smp::online_count(),
            crate::cpu::id()
        );
    }
    drop(space); // teardown outside the timed window; it is spawn_reap's kind of cost, not map's
}

/// **What one remote RFENCE costs, measured on a single hart** (the calibration that settles the
/// 2026-08-15 reading; notes/benchmarks.md).
///
/// The reading said a `map_new` regression of +370 ticks over 64 iterations was remote RFENCEs
/// fired against an over-reporting mask, and it could not rule out the rival reading, that the
/// delta was the *mask arithmetic itself* being consulted per flush. Those two differ by orders of
/// magnitude, so measuring the RFENCE once decides it. This is the only way to get that number on
/// one hart: **a hart may name itself in an SBI hart mask**, which is a legal call the firmware
/// serves with a local fence, and it exercises the whole `ecall`-into-firmware path that the
/// shootdown pays and a local `sfence.vma` does not.
///
/// It must be single-hart. A two-hart comparison would be the 2026-07-28 mistake again: under
/// `-icount` all harts share one virtual clock, so a second hart's idle `wfi` dumps quantized time
/// into whatever window is open, and the delta would measure interleaving rather than the call.
#[cfg(target_arch = "riscv64")]
pub(super) fn rfence_self() {
    const RFENCE_ITERS: u64 = 512;
    // Any mapped kernel address: the firmware fences a range, and which range it is does not change
    // the cost of getting there, which is the whole of what this measures.
    static ANCHOR: AtomicU64 = AtomicU64::new(0);
    let va = &raw const ANCHOR as usize;
    let me = 1usize << crate::cpu::id();

    for _ in 0..WARMUP {
        crate::arch::sbi_remote_sfence_vma(me, va, page_frames::FRAME_SIZE as usize);
    }
    timed("rfence_self", RFENCE_ITERS, || {
        for _ in 0..RFENCE_ITERS {
            crate::arch::sbi_remote_sfence_vma(me, va, page_frames::FRAME_SIZE as usize);
        }
    });
}
