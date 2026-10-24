//! LOCK: four cores increment one counter under a spinlock built the way `spin` 0.12.3 builds
//! `SpinMutex`, which is what the kernel's IrqSafeMutex wraps.
//!
//! The lock takes with `compare_exchange_weak(false, true, Acquire, Relaxed)` and spins on a
//! Relaxed load. The counter is read and written with plain Relaxed loads and stores inside the
//! critical section, as protected data is. A lost update (the final count short of
//! `CORES * INCREMENTS`) means the next holder read the counter before the previous holder's
//! store reached it. Unlocking with Release forbids that; the weakened unlock is Relaxed, the
//! change tools/host_reordering_litmus/weaken-spin.sh makes to `spin`.
//!
//! Name: provisional, minted 2026-10-24 (UTC) by maintainer/measure-arm64-host-reordering.

use core::hint::spin_loop;
use core::sync::atomic::{AtomicBool, Ordering};

use crate::cores::{CORES, Line, REORDERS, SAMPLES, barrier, scale};

/// Increments each core makes per test.
pub(crate) const INCREMENTS: u64 = 200_000 * scale();

#[repr(align(128))]
struct Lock(AtomicBool);

static LOCK: Lock = Lock(AtomicBool::new(false));
static COUNTER: Line = Line::new();

#[inline(never)]
fn hammer<const RELEASE: bool>() {
    let unlock = if RELEASE {
        Ordering::Release
    } else {
        Ordering::Relaxed
    };
    for _ in 0..INCREMENTS {
        // Acquire pairs with the previous holder's unlock store below (when it is Release).
        while LOCK
            .0
            .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            while LOCK.0.load(Ordering::Relaxed) {
                spin_loop();
            }
        }
        let n = COUNTER.0.load(Ordering::Relaxed);
        COUNTER.0.store(n + 1, Ordering::Relaxed);
        LOCK.0.store(false, unlock);
    }
}

/// Runs one LOCK test on core `id`; core 0 reports the lost updates.
pub(crate) fn test<const RELEASE: bool>(id: usize) {
    if id == 0 {
        COUNTER.0.store(0, Ordering::Relaxed);
    }
    barrier();
    hammer::<RELEASE>();
    barrier();
    let (samples, lost) = if id == 0 {
        let total = CORES as u64 * INCREMENTS;
        (total, total - COUNTER.0.load(Ordering::Relaxed))
    } else {
        (0, 0)
    };
    SAMPLES[id].store(samples, Ordering::Relaxed);
    REORDERS[id].store(lost, Ordering::Relaxed);
}
