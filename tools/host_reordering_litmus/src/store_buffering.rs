//! SB (store buffering), in batches.
//!
//! Each core of a pair stores 1 to its own slot, then loads the other core's slot, across
//! [`SLOTS`] fresh slots per batch. Both loads reading 0 needs a store to pass a later load. TSO
//! permits that, so on an x86_64 host this is the positive control showing the guest cores run in
//! parallel. Rust's SeqCst on both accesses, or a SeqCst fence between them, forbids it.
//!
//! Name: provisional, minted 2026-10-24 (UTC) by maintainer/measure-arm64-host-reordering.

use core::sync::atomic::{AtomicU64, Ordering, fence};

use crate::cores::{CORES, Line, PAIRS, REORDERS, SAMPLES, barrier, scale};

pub(crate) const SLOTS: usize = 256;
pub(crate) const BATCHES: u64 = 2_000 * scale();

/// The store and load each core issues: both Relaxed, both SeqCst, or Relaxed around a SeqCst
/// fence.
pub(crate) const RELAXED: u8 = 0;
pub(crate) const SEQ_CST: u8 = 1;
pub(crate) const FENCED: u8 = 2;

struct Pair {
    x: [Line; SLOTS],
    y: [Line; SLOTS],
}

static PAIR: [Pair; PAIRS] = [const {
    Pair {
        x: [const { Line::new() }; SLOTS],
        y: [const { Line::new() }; SLOTS],
    }
}; PAIRS];

/// What each core read in the current batch.
static SEEN: [[AtomicU64; SLOTS]; CORES] = [const { [const { AtomicU64::new(0) }; SLOTS] }; CORES];

#[inline(never)]
fn side<const ORDER: u8>(mine: &[Line; SLOTS], theirs: &[Line; SLOTS], seen: &[AtomicU64]) {
    for k in 0..SLOTS {
        let r = match ORDER {
            RELAXED => {
                mine[k].0.store(1, Ordering::Relaxed);
                theirs[k].0.load(Ordering::Relaxed)
            }
            SEQ_CST => {
                mine[k].0.store(1, Ordering::SeqCst);
                theirs[k].0.load(Ordering::SeqCst)
            }
            _ => {
                mine[k].0.store(1, Ordering::Relaxed);
                // PAIR: the other core's fence(SeqCst) in this same function, between its store
                // and its load; SB needs one on each side.
                fence(Ordering::SeqCst);
                theirs[k].0.load(Ordering::Relaxed)
            }
        };
        seen[k].store(r, Ordering::Relaxed);
    }
}

/// Runs one SB test on core `id`; the even core of each pair counts the pair's outcomes.
pub(crate) fn test<const ORDER: u8>(id: usize) {
    let p = &PAIR[id / 2];
    let (mine, theirs) = if id % 2 == 0 {
        (&p.x, &p.y)
    } else {
        (&p.y, &p.x)
    };
    let mut both_zero = 0;
    for _ in 0..BATCHES {
        barrier();
        side::<ORDER>(mine, theirs, &SEEN[id]);
        barrier();
        if id % 2 == 0 {
            for k in 0..SLOTS {
                let a = SEEN[id][k].load(Ordering::Relaxed);
                let b = SEEN[id + 1][k].load(Ordering::Relaxed);
                if a == 0 && b == 0 {
                    both_zero += 1;
                }
                p.x[k].0.store(0, Ordering::Relaxed);
                p.y[k].0.store(0, Ordering::Relaxed);
            }
        }
    }
    let samples = if id % 2 == 0 {
        BATCHES * SLOTS as u64
    } else {
        0
    };
    SAMPLES[id].store(samples, Ordering::Relaxed);
    REORDERS[id].store(both_zero, Ordering::Relaxed);
}
