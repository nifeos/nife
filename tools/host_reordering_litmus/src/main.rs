//! Counts the memory reorderings a guest can see under QEMU's multi-threaded TCG.
//!
//! Four cores run two litmus shapes, two pairs at a time (cores 0 and 1, cores 2 and 3), each
//! pair on its own cache lines:
//!
//! - MP (message passing), streamed. The writer stores `data = i` then `flag = i`; the reader
//!   loads `flag` then `data`. Coherence keeps each location monotonic, so `data < flag` is
//!   possible only if the writer's two stores or the reader's two loads were reordered. Run with
//!   every combination of Release or Relaxed on the flag store and Acquire or Relaxed on the flag
//!   load.
//! - SB (store buffering), in batches. Each core stores 1 to its own slot then loads the other's;
//!   both loads reading 0 needs a store to pass a later load. TSO permits it, so on an x86_64 host
//!   it is the positive control that shows the guest cores really run in parallel.
//!
//! Each test prints one `RESULT` line. `LITMUS_SCALE` (build time, default 1) multiplies every
//! iteration count. See notes/benchmarks/host-reordering-under-tcg.md.

#![no_std]
#![no_main]

#[cfg(target_arch = "aarch64")]
#[path = "aarch64.rs"]
mod arch;
#[cfg(target_arch = "riscv64")]
#[path = "riscv64.rs"]
mod arch;

use core::hint::spin_loop;
use core::sync::atomic::{AtomicU32, AtomicU64, Ordering, fence};

const CORES: usize = 4;
const PAIRS: usize = CORES / 2;

const fn scale() -> u64 {
    match option_env!("LITMUS_SCALE") {
        None => 1,
        Some(s) => {
            let b = s.as_bytes();
            let mut i = 0;
            let mut n = 0u64;
            while i < b.len() {
                n = n * 10 + (b[i] - b'0') as u64;
                i += 1;
            }
            n
        }
    }
}

/// Messages each MP writer sends per test.
const MP_MESSAGES: u64 = 2_000_000 * scale();
/// SB slots per batch, and batches per test.
const SB_SLOTS: usize = 256;
const SB_BATCHES: u64 = 2_000 * scale();

#[repr(align(128))]
struct Line(AtomicU64);

impl Line {
    const fn new() -> Self {
        Line(AtomicU64::new(0))
    }
}

struct MpPair {
    data: Line,
    flag: Line,
}

static MP: [MpPair; PAIRS] = [const {
    MpPair {
        data: Line::new(),
        flag: Line::new(),
    }
}; PAIRS];

struct SbPair {
    x: [Line; SB_SLOTS],
    y: [Line; SB_SLOTS],
}

static SB: [SbPair; PAIRS] = [const {
    SbPair {
        x: [const { Line::new() }; SB_SLOTS],
        y: [const { Line::new() }; SB_SLOTS],
    }
}; PAIRS];

/// What each core read in the current SB batch.
static SB_SEEN: [[AtomicU64; SB_SLOTS]; CORES] =
    [const { [const { AtomicU64::new(0) }; SB_SLOTS] }; CORES];

/// Per-core tallies for the current test: samples, then reorderings observed.
static SAMPLES: [AtomicU64; CORES] = [const { AtomicU64::new(0) }; CORES];
static REORDERS: [AtomicU64; CORES] = [const { AtomicU64::new(0) }; CORES];

static ARRIVED: AtomicU32 = AtomicU32::new(0);
static GENERATION: AtomicU32 = AtomicU32::new(0);

/// A central barrier for all [`CORES`] cores.
fn barrier() {
    let generation = GENERATION.load(Ordering::Acquire);
    if ARRIVED.fetch_add(1, Ordering::AcqRel) == CORES as u32 - 1 {
        ARRIVED.store(0, Ordering::Relaxed);
        // Release pairs with the waiters' Acquire load, publishing everything before the barrier.
        GENERATION.store(generation.wrapping_add(1), Ordering::Release);
    } else {
        while GENERATION.load(Ordering::Acquire) == generation {
            spin_loop();
        }
    }
}

mod console {
    use core::fmt::Write;

    struct Uart;

    impl Write for Uart {
        fn write_str(&mut self, s: &str) -> core::fmt::Result {
            for b in s.bytes() {
                if b == b'\n' {
                    crate::arch::putc(b'\r');
                }
                crate::arch::putc(b);
            }
            Ok(())
        }
    }

    pub fn line(args: core::fmt::Arguments) {
        let _ = Uart.write_fmt(args);
        let _ = Uart.write_str("\n");
    }
}

#[inline(always)]
fn store_order(release: bool) -> Ordering {
    if release {
        Ordering::Release
    } else {
        Ordering::Relaxed
    }
}

#[inline(always)]
fn load_order(acquire: bool) -> Ordering {
    if acquire {
        Ordering::Acquire
    } else {
        Ordering::Relaxed
    }
}

#[inline(never)]
fn mp_writer<const RELEASE: bool>(p: &MpPair) {
    for i in 1..=MP_MESSAGES {
        p.data.0.store(i, Ordering::Relaxed);
        p.flag.0.store(i, store_order(RELEASE));
    }
}

#[inline(never)]
fn mp_reader<const ACQUIRE: bool>(p: &MpPair) -> (u64, u64) {
    let mut samples = 0;
    let mut reorders = 0;
    loop {
        let f = p.flag.0.load(load_order(ACQUIRE));
        let d = p.data.0.load(Ordering::Relaxed);
        samples += 1;
        if d < f {
            reorders += 1;
        }
        if f == MP_MESSAGES {
            return (samples, reorders);
        }
    }
}

fn mp<const RELEASE: bool, const ACQUIRE: bool>(id: usize) {
    let p = &MP[id / 2];
    if id == 0 {
        for q in &MP {
            q.data.0.store(0, Ordering::Relaxed);
            q.flag.0.store(0, Ordering::Relaxed);
        }
    }
    barrier();
    if id % 2 == 0 {
        mp_writer::<RELEASE>(p);
        SAMPLES[id].store(0, Ordering::Relaxed);
        REORDERS[id].store(0, Ordering::Relaxed);
    } else {
        let (s, r) = mp_reader::<ACQUIRE>(p);
        SAMPLES[id].store(s, Ordering::Relaxed);
        REORDERS[id].store(r, Ordering::Relaxed);
    }
}

/// How each SB core orders its store before its load.
#[derive(Clone, Copy, PartialEq)]
enum SbOrder {
    Relaxed,
    SeqCst,
    Fence,
}

#[inline(never)]
fn sb_side<const ORDER: u8>(
    mine: &[Line; SB_SLOTS],
    theirs: &[Line; SB_SLOTS],
    seen: &[AtomicU64],
) {
    for k in 0..SB_SLOTS {
        let r = match ORDER {
            0 => {
                mine[k].0.store(1, Ordering::Relaxed);
                theirs[k].0.load(Ordering::Relaxed)
            }
            1 => {
                mine[k].0.store(1, Ordering::SeqCst);
                theirs[k].0.load(Ordering::SeqCst)
            }
            _ => {
                mine[k].0.store(1, Ordering::Relaxed);
                fence(Ordering::SeqCst);
                theirs[k].0.load(Ordering::Relaxed)
            }
        };
        seen[k].store(r, Ordering::Relaxed);
    }
}

fn sb<const ORDER: u8>(id: usize) {
    let p = &SB[id / 2];
    let (mine, theirs) = if id % 2 == 0 {
        (&p.x, &p.y)
    } else {
        (&p.y, &p.x)
    };
    let mut both_zero = 0;
    for _ in 0..SB_BATCHES {
        barrier();
        sb_side::<ORDER>(mine, theirs, &SB_SEEN[id]);
        barrier();
        if id % 2 == 0 {
            for k in 0..SB_SLOTS {
                let a = SB_SEEN[id][k].load(Ordering::Relaxed);
                let b = SB_SEEN[id + 1][k].load(Ordering::Relaxed);
                if a == 0 && b == 0 {
                    both_zero += 1;
                }
                p.x[k].0.store(0, Ordering::Relaxed);
                p.y[k].0.store(0, Ordering::Relaxed);
            }
        }
    }
    let samples = if id % 2 == 0 {
        SB_BATCHES * SB_SLOTS as u64
    } else {
        0
    };
    SAMPLES[id].store(samples, Ordering::Relaxed);
    REORDERS[id].store(both_zero, Ordering::Relaxed);
}

struct Test {
    name: &'static str,
    ordering: &'static str,
    /// True when the language memory model forbids the outcome counted.
    forbidden: bool,
    run: fn(usize),
}

const TESTS: &[Test] = &[
    Test {
        name: "MP",
        ordering: "release-acquire",
        forbidden: true,
        run: mp::<true, true>,
    },
    Test {
        name: "MP",
        ordering: "relaxed-relaxed",
        forbidden: false,
        run: mp::<false, false>,
    },
    Test {
        name: "MP",
        ordering: "release-relaxed",
        forbidden: false,
        run: mp::<true, false>,
    },
    Test {
        name: "MP",
        ordering: "relaxed-acquire",
        forbidden: false,
        run: mp::<false, true>,
    },
    Test {
        name: "SB",
        ordering: "relaxed",
        forbidden: false,
        run: sb::<0>,
    },
    Test {
        name: "SB",
        ordering: "seqcst",
        forbidden: true,
        run: sb::<1>,
    },
    Test {
        name: "SB",
        ordering: "fence-seqcst",
        forbidden: true,
        run: sb::<2>,
    },
];

const _: () = assert!(SbOrder::Relaxed as u8 == 0 && SbOrder::SeqCst as u8 == 1);
const _: () = assert!(SbOrder::Fence as u8 == 2);

#[cfg(target_arch = "aarch64")]
const ARCH: &str = "aarch64";
#[cfg(target_arch = "riscv64")]
const ARCH: &str = "riscv64";

#[unsafe(no_mangle)]
extern "C" fn litmus_main(id: usize) -> ! {
    if id == 0 {
        console::line(format_args!(
            "host_reordering_litmus: {ARCH}, {CORES} cores, scale {}",
            scale()
        ));
        arch::start_secondaries(CORES);
    }
    let mut violations = 0u64;
    for t in TESTS {
        barrier();
        let t0 = arch::ticks();
        (t.run)(id);
        barrier();
        if id == 0 {
            let ms = (arch::ticks() - t0) * 1000 / arch::ticks_per_second();
            let samples: u64 = SAMPLES.iter().map(|s| s.load(Ordering::Relaxed)).sum();
            let reorders: u64 = REORDERS.iter().map(|s| s.load(Ordering::Relaxed)).sum();
            let messages = if t.name == "MP" {
                MP_MESSAGES * PAIRS as u64
            } else {
                samples
            };
            let ppm = reorders * 1_000_000 / messages.max(1);
            console::line(format_args!(
                "RESULT arch={ARCH} test={} ordering={} forbidden={} iterations={messages} \
                 samples={samples} observed={reorders} per_million={ppm} ms={ms}",
                t.name, t.ordering, t.forbidden
            ));
            if t.forbidden && reorders != 0 {
                violations += 1;
            }
        }
    }
    if id == 0 {
        console::line(format_args!(
            "DONE arch={ARCH} forbidden_outcomes_seen_in={violations}_tests"
        ));
        arch::exit(0);
    }
    loop {
        spin_loop();
    }
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    console::line(format_args!("PANIC {info}"));
    arch::exit(1)
}
