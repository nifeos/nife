//! The entry every core reaches from its boot code, the test table, and the panic handler.
//!
//! Name: provisional, minted 2026-10-24 (UTC) by maintainer/measure-arm64-host-reordering.

use core::hint::spin_loop;
use core::sync::atomic::Ordering;

use crate::cores::{CORES, PAIRS, REORDERS, SAMPLES, barrier, scale};
use crate::{arch, console, message_passing as mp, store_buffering as sb};

struct Test {
    name: &'static str,
    ordering: &'static str,
    /// True when Rust's memory model forbids the outcome counted.
    forbidden: bool,
    /// Messages for MP; for SB the samples the cores report.
    messages: Option<u64>,
    run: fn(usize),
}

const MP_TOTAL: Option<u64> = Some(mp::MESSAGES * PAIRS as u64);

const TESTS: &[Test] = &[
    Test {
        name: "MP",
        ordering: "release-acquire",
        forbidden: true,
        messages: MP_TOTAL,
        run: mp::test::<true, true>,
    },
    Test {
        name: "MP",
        ordering: "relaxed-relaxed",
        forbidden: false,
        messages: MP_TOTAL,
        run: mp::test::<false, false>,
    },
    Test {
        name: "MP",
        ordering: "release-relaxed",
        forbidden: false,
        messages: MP_TOTAL,
        run: mp::test::<true, false>,
    },
    Test {
        name: "MP",
        ordering: "relaxed-acquire",
        forbidden: false,
        messages: MP_TOTAL,
        run: mp::test::<false, true>,
    },
    Test {
        name: "SB",
        ordering: "relaxed",
        forbidden: false,
        messages: None,
        run: sb::test::<{ sb::RELAXED }>,
    },
    Test {
        name: "SB",
        ordering: "seqcst",
        forbidden: true,
        messages: None,
        run: sb::test::<{ sb::SEQ_CST }>,
    },
    Test {
        name: "SB",
        ordering: "fence-seqcst",
        forbidden: true,
        messages: None,
        run: sb::test::<{ sb::FENCED }>,
    },
];

#[cfg(target_arch = "aarch64")]
const ARCH: &str = "aarch64";
#[cfg(target_arch = "riscv64")]
const ARCH: &str = "riscv64";

/// Runs every test on core `id`. Core 0 starts the others, prints, and ends QEMU.
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
        if id != 0 {
            continue;
        }
        let ms = (arch::ticks() - t0) * 1000 / arch::ticks_per_second();
        let samples: u64 = SAMPLES.iter().map(|s| s.load(Ordering::Relaxed)).sum();
        let reorders: u64 = REORDERS.iter().map(|s| s.load(Ordering::Relaxed)).sum();
        let iterations = t.messages.unwrap_or(samples);
        let ppm = reorders * 1_000_000 / iterations.max(1);
        console::line(format_args!(
            "RESULT arch={ARCH} test={} ordering={} forbidden={} iterations={iterations}              samples={samples} observed={reorders} per_million={ppm} ms={ms}",
            t.name, t.ordering, t.forbidden
        ));
        if t.forbidden && reorders != 0 {
            violations += 1;
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
