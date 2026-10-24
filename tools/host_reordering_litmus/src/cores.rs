//! What every test shares: the core count, the scale, cache-line slots, tallies and a barrier.
//!
//! Name: provisional, minted 2026-10-24 (UTC) by maintainer/measure-arm64-host-reordering.

use core::hint::spin_loop;
use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

pub(crate) const CORES: usize = 4;
pub(crate) const PAIRS: usize = CORES / 2;

/// `LITMUS_SCALE` at build time, or 1.
pub(crate) const fn scale() -> u64 {
    match option_env!("LITMUS_SCALE") {
        None => 1,
        Some(s) => {
            let b = s.as_bytes();
            let mut i = 0;
            let mut n = 0u64;
            while i < b.len() {
                assert!(b[i].is_ascii_digit(), "LITMUS_SCALE is a decimal number");
                n = n * 10 + (b[i] - b'0') as u64;
                i += 1;
            }
            n
        }
    }
}

/// One atomic on its own 128-byte line, so no two slots share a line or an adjacent-line pair.
#[repr(align(128))]
pub(crate) struct Line(pub(crate) AtomicU64);

impl Line {
    pub(crate) const fn new() -> Self {
        Line(AtomicU64::new(0))
    }
}

/// Per-core tallies for the current test: samples taken, and reorderings observed.
pub(crate) static SAMPLES: [AtomicU64; CORES] = [const { AtomicU64::new(0) }; CORES];
pub(crate) static REORDERS: [AtomicU64; CORES] = [const { AtomicU64::new(0) }; CORES];

static ARRIVED: AtomicU32 = AtomicU32::new(0);
static GENERATION: AtomicU32 = AtomicU32::new(0);

/// Waits until all [`CORES`] cores have arrived; everything a core wrote before it is visible to
/// every core after it.
pub(crate) fn barrier() {
    let generation = GENERATION.load(Ordering::Acquire);
    if ARRIVED.fetch_add(1, Ordering::AcqRel) == CORES as u32 - 1 {
        ARRIVED.store(0, Ordering::Relaxed);
        // Release pairs with the waiters' Acquire load of GENERATION below.
        GENERATION.store(generation.wrapping_add(1), Ordering::Release);
    } else {
        while GENERATION.load(Ordering::Acquire) == generation {
            spin_loop();
        }
    }
}
