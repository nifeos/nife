//! MP (message passing), streamed.
//!
//! The writer stores `data = i` then `flag = i` for every i; the reader loads `flag` then
//! `data` until it has seen the last message. Coherence keeps each location monotonic, so
//! `data < flag` is possible only if the writer's two stores or the reader's two loads were
//! reordered. The flag store is Release or Relaxed and the flag load Acquire or Relaxed; only
//! Release with Acquire forbids the outcome.
//!
//! Name: provisional, minted 2026-10-24 (UTC) by maintainer/measure-arm64-host-reordering.

use core::sync::atomic::Ordering;

use crate::cores::{Line, PAIRS, REORDERS, SAMPLES, barrier, scale};

/// Messages each writer sends per test.
pub(crate) const MESSAGES: u64 = 2_000_000 * scale();

struct Pair {
    data: Line,
    flag: Line,
}

static PAIR: [Pair; PAIRS] = [const {
    Pair {
        data: Line::new(),
        flag: Line::new(),
    }
}; PAIRS];

#[inline(never)]
fn writer<const RELEASE: bool>(p: &Pair) {
    let order = if RELEASE {
        Ordering::Release
    } else {
        Ordering::Relaxed
    };
    for i in 1..=MESSAGES {
        p.data.0.store(i, Ordering::Relaxed);
        // When RELEASE, pairs with reader's Acquire load of flag.
        p.flag.0.store(i, order);
    }
}

#[inline(never)]
fn reader<const ACQUIRE: bool>(p: &Pair) -> (u64, u64) {
    let order = if ACQUIRE {
        Ordering::Acquire
    } else {
        Ordering::Relaxed
    };
    let mut samples = 0;
    let mut reorders = 0;
    loop {
        // When ACQUIRE, pairs with writer's Release store of flag.
        let f = p.flag.0.load(order);
        let d = p.data.0.load(Ordering::Relaxed);
        samples += 1;
        if d < f {
            reorders += 1;
        }
        if f == MESSAGES {
            return (samples, reorders);
        }
    }
}

/// Runs one MP test on core `id`: even cores write, odd cores read their pair's lines.
pub(crate) fn test<const RELEASE: bool, const ACQUIRE: bool>(id: usize) {
    if id == 0 {
        for p in &PAIR {
            p.data.0.store(0, Ordering::Relaxed);
            p.flag.0.store(0, Ordering::Relaxed);
        }
    }
    barrier();
    let p = &PAIR[id / 2];
    let (s, r) = if id % 2 == 0 {
        writer::<RELEASE>(p);
        (0, 0)
    } else {
        reader::<ACQUIRE>(p)
    };
    SAMPLES[id].store(s, Ordering::Relaxed);
    REORDERS[id].store(r, Ordering::Relaxed);
}
