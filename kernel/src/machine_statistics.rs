//! The machine statistics page, the kernel's half: the writes to the frame `free`, `vmstat` and
//! `top` read, for §225 (`free` sees the machine and your share) part 2.
//!
//! Each per-core word is written only by its own core, on a cache line of its own; the frame count
//! under the allocator's lock. Before [`publish`] the writes land in a static sink, so a writer
//! never tests whether the page exists. The layout is `crates/machine_statistics_contract`'s. See
//! notes/machine-statistics.md.
//!
//! # BUGS
//! - A counter written before [`publish`] is lost.
//! - The switch count on the page is up to one tick old.
//! - A spurious or unrouted interrupt is not counted.
//! - The soak build's timer-driven `irq_notify` counts as a device interrupt.
//!
//! Name: provisional, minted 2026-09-26 by milestone 126's `free` lane.

use core::sync::atomic::{AtomicPtr, AtomicU64, Ordering};

use machine_statistics_contract::{WORDS, build_header, word};

use crate::cpu;

/// Where writes go until the page exists. Aligned like the page, so a word is never torn.
#[repr(C, align(64))]
struct Sink([AtomicU64; WORDS]);

static SINK: Sink = Sink([const { AtomicU64::new(0) }; WORDS]);

/// The page's first word, as the kernel reaches it: the sink until [`publish`], then the frame's
/// direct-map address.
static PAGE: AtomicPtr<AtomicU64> =
    AtomicPtr::new(&SINK.0 as *const [AtomicU64; WORDS] as *mut AtomicU64);

/// The frame's physical address once published, for the one capability minted to it.
static PHYS: AtomicU64 = AtomicU64::new(0);

#[inline(always)]
fn at(index: usize) -> &'static AtomicU64 {
    debug_assert!(index < WORDS);
    // SAFETY: `PAGE` names either `SINK` or a frame `publish` allocated and never frees, both at
    // least `WORDS` words long, and `index < WORDS` at every call site (each is a protocol
    // constant, and `cpu::id()` is under `CPU_ID_BOUND`, which `word::cpu` is sized by).
    unsafe { &*PAGE.load(Ordering::Relaxed).add(index) }
}

#[inline(always)]
fn mine(offset: usize) -> &'static AtomicU64 {
    at(word::cpu(cpu::id()) + offset)
}

/// Make the page: a zeroed frame, the header and the current frame counts, after which every
/// counter lands in it. Called once, after the frame allocator and before the scheduler. Returns
/// the frame's physical address.
pub fn publish() -> u64 {
    let frame = crate::memory::alloc_zeroed().expect("no frame for the machine statistics page");
    let phys = frame.addr();
    let base = crate::arch::mmu::phys_to_virt(phys) as *mut AtomicU64;
    let header = build_header(page_frames::FRAME_SIZE, crate::arch::timer::TICK_HZ);
    for (i, w) in header.iter().enumerate() {
        // SAFETY: a frame just allocated and zeroed for us alone, reached through the direct map,
        // and the header is one line, far under a frame.
        unsafe { (*base.add(i)).store(*w, Ordering::Relaxed) };
    }
    PHYS.store(phys, Ordering::Relaxed);
    // Release: a core that sees the new pointer sees the header written above.
    PAGE.store(base, Ordering::Release);
    if let Some(s) = crate::memory::stats() {
        frames(s.total, s.total - s.used);
    }
    phys
}

/// The page's physical address, or 0 before [`publish`].
pub fn page_phys() -> u64 {
    PHYS.load(Ordering::Relaxed)
}

/// The allocator's counts, stored by whoever just changed them, under the allocator's lock.
#[inline]
pub(crate) fn frames(total: usize, free: usize) {
    at(word::TOTAL_FRAMES).store(total as u64, Ordering::Relaxed);
    at(word::FREE_FRAMES).store(free as u64, Ordering::Relaxed);
}

/// This core took an interrupt other than its tick (the tick counts itself in [`tick`]). Out of
/// line because its callers sit in the exception dispatchers `script/fastpath-footprint` measures.
#[inline(never)]
pub fn interrupt() {
    mine(word::INTERRUPTS).fetch_add(1, Ordering::Relaxed);
}

/// This core took a tick with `idle` running or not, `runnable` threads waiting, and `switches`
/// context switches since boot (`cpu::PerCpu::switches`, which this publishes).
#[inline]
pub(crate) fn tick(switches: u64, idle: bool, runnable: u64) {
    mine(word::ONLINE).store(1, Ordering::Relaxed);
    mine(word::CONTEXT_SWITCHES).store(switches, Ordering::Relaxed);
    let spent = if idle {
        word::IDLE_TICKS
    } else {
        word::BUSY_TICKS
    };
    mine(spent).fetch_add(1, Ordering::Relaxed);
    mine(word::INTERRUPTS).fetch_add(1, Ordering::Relaxed);
    mine(word::RUNNABLE).store(runnable, Ordering::Relaxed);
}
