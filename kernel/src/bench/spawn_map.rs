//! **Spawning a thread and mapping a page**, and on `riscv64` what the remote fence a map
//! may fire costs.
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

    // Counted across exactly the window: the claim is about what `map_new` itself issues.
    #[cfg(target_arch = "riscv64")]
    let fences_before = crate::arch::remote_fence_count();

    // Preemption masked across the window, so a tick's phase cannot land in it; the deferred tick
    // is taken at `restore`. Safe only because nothing here blocks: `map_new` takes spin locks.
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

    // A probe, not a row: a count. Nonzero is the tick deferred to `restore`, not one inside.
    println!(
        "bench-probe: map_new_preemptions {} over {MAP_ITERS} iters",
        sched::preemptions_here() - preemptions_before
    );

    // Probes, not rows: counts and a bitmask whose only interesting values are exact.
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

/// What one remote RFENCE costs, measured on a single hart by naming itself in the SBI hart mask.
///
/// That legal call takes the whole `ecall`-into-firmware path a shootdown pays. It must be
/// single-hart, because under `-icount` a second hart's idle time lands in the window. See
/// notes/benchmarks/kernel-bench-harness.md.
#[cfg(target_arch = "riscv64")]
pub(super) fn rfence_self() {
    const RFENCE_ITERS: u64 = 512;
    // Any mapped address: which range is fenced does not change the cost of getting there.
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
