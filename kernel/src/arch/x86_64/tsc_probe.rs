//! Does the TSC tick at a constant rate under QEMU TCG? A measurement behind the off-by-default
//! `tsc_probe` feature, not a feature.
//!
//! TCG never advertises the invariant-TSC bit, and all time on x86 flows from `timer.rs`'s one
//! measured rate. So this times the TSC against the CMOS RTC's seconds edge: QEMU drives the RTC
//! from host time, where the PIT shares the TSC's clock. Windows alternate a cheap `alu` burn with
//! a costly `port` burn (`out 0x80`); an instruction-derived TSC would give the two different
//! rates. Each edge is timestamped to one poll iteration, measured and printed. See
//! notes/tsc-under-tcg.md.

use super::port::out8;
use super::timer;
use crate::{print, println};

/// The CMOS index port.
const CMOS_INDEX: u16 = 0x70;
/// The CMOS data port.
const CMOS_DATA: u16 = 0x71;
/// The CMOS seconds register.
const CMOS_SECONDS: u8 = 0x00;
/// The POST diagnostic port. Writes are swallowed by QEMU and by real chipsets alike; this is the
/// classic "I/O delay" write, used here as a guest instruction that costs a lot of host time.
const POST_PORT: u16 = 0x80;

/// How many RTC seconds one measurement window spans.
const WINDOW_SECONDS: u64 = 4;
/// How many windows to run. Eight windows of four seconds is about half a minute of boot.
const WINDOWS: u64 = 8;

/// Status register B.
const CMOS_STATUS_B: u8 = 0x0B;
/// Status register B, bit 2: the time fields are binary (set) rather than BCD (clear).
const STATUS_B_BINARY: u8 = 1 << 2;

/// Read one CMOS register.
fn cmos(reg: u8) -> u8 {
    // SAFETY: 0x70/0x71 are the CMOS index/data ports on every PC-compatible machine, driven only
    // from ring 0, and nothing in userspace can name them: §121 (what a device capability is when
    // the device has no page: x86 port I/O) keeps the legacy ports kernel-side. This is the same
    // access `arch::x86_64::rtc` makes.
    unsafe {
        out8(CMOS_INDEX, reg);
        super::port::in8(CMOS_DATA)
    }
}

/// The seconds register **decoded**, 0..=59.
///
/// The decode is not a detail: QEMU's CMOS is BCD by default, so the raw bytes step 0x09 -> 0x10
/// across every decade and a subtraction of two raw readings overstates the elapsed seconds. The
/// first version of this harness subtracted the raw bytes and reported windows of ten seconds that
/// had taken four.
fn seconds() -> u8 {
    let raw = cmos(CMOS_SECONDS);
    if BINARY.load(core::sync::atomic::Ordering::Relaxed) {
        raw
    } else {
        (raw & 0x0F) + 10 * (raw >> 4)
    }
}

/// Whether this CMOS reports binary rather than BCD, read once by [`probe`]. A static rather than a
/// second port read per poll, because every poll is on the edge-detection path and the resolution
/// bound this harness reports is the cost of one poll.
static BINARY: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

/// Spin until the CMOS seconds register changes, and return the TSC reading taken immediately
/// after the change was observed, along with the number of poll iterations it took.
fn wait_for_edge(previous: u8) -> (u64, u8, u64) {
    let mut iterations = 0u64;
    loop {
        let now = seconds();
        if now != previous {
            return (timer::now(), now, iterations);
        }
        iterations += 1;
    }
}

/// A register-only burn: many guest instructions, cheap for TCG to emulate.
fn burn_alu(rounds: u64) -> u64 {
    let mut x = 1u64;
    for _ in 0..rounds {
        x = x.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        core::hint::black_box(&mut x);
    }
    x
}

/// A port-I/O burn: few guest instructions, each an exit into device emulation, so it is expensive
/// for TCG in host time.
fn burn_port(rounds: u64) {
    for _ in 0..rounds {
        // SAFETY: 0x80 is the POST diagnostic port. Writes to it are ignored by QEMU and by every
        // chipset this kernel can run on, and nothing in this tree drives it.
        unsafe { out8(POST_PORT, 0) };
    }
}

/// Run the measurement and print one line per window. Called from the x86 tour when the kernel is
/// built with the `tsc_probe` feature; absent from every other build.
pub(crate) fn probe() {
    BINARY.store(
        cmos(CMOS_STATUS_B) & STATUS_B_BINARY != 0,
        core::sync::atomic::Ordering::Relaxed,
    );
    let hz = timer::frequency();
    println!(
        "tscprobe: calibrated tsc {hz} Hz (source {}), reference CMOS RTC seconds edge",
        timer::frequency_source(),
    );

    // Every calibration window, so a run shows the smallest near the RTC's rate and the largest
    // far above it: the min-of-N argument demonstrated, not asserted.
    print!("tscprobe: calibration windows");
    for window in timer::calibration().windows() {
        print!(" {window}");
    }
    println!();

    // How long one poll iteration takes, in TSC counts. This is the resolution bound on every edge
    // timestamp below: the edge happened somewhere inside the last iteration.
    let before = timer::now();
    let mut probe_seconds = seconds();
    for _ in 0..1000 {
        probe_seconds = core::hint::black_box(seconds());
    }
    let poll_cost = (timer::now() - before) / 1000;
    println!(
        "tscprobe: one rtc poll costs {poll_cost} tsc counts, so each edge is timestamped to \
         within that ({} ns at the calibrated rate)",
        poll_cost.saturating_mul(1_000_000_000) / hz.max(1),
    );

    // Sync to an edge so the first window starts aligned.
    let (mut tsc_at_edge, mut last, _) = wait_for_edge(probe_seconds);

    let mut lowest = u64::MAX;
    let mut highest = 0u64;
    let first_tsc = tsc_at_edge;
    let first_seconds = last;
    let mut reference_seconds_total = 0u64;

    for window in 0..WINDOWS {
        let alu = window % 2 == 0;
        let start = tsc_at_edge;
        let start_seconds = last;

        // Burn, then tight-poll for the ending edge. The burn's length uses the calibrated rate,
        // the number under suspicion, so it does not enter the result; the RTC counts the span.
        let burn_until = start + hz.saturating_mul(WINDOW_SECONDS - 1);
        let mut work = 0u64;
        while timer::now() < burn_until {
            if alu {
                core::hint::black_box(burn_alu(10_000));
            } else {
                burn_port(1_000);
            }
            work += 1;
        }

        // `elapsed` is read off the RTC, not assumed: the burn may cross more or fewer seconds.
        let current = seconds();
        let (tsc, seen, poll_iterations) = wait_for_edge(current);
        last = seen;
        tsc_at_edge = tsc;
        let elapsed = u64::from((60 + seen - start_seconds) % 60);
        reference_seconds_total += elapsed;

        let delta = tsc_at_edge - start;
        let implied = delta / elapsed.max(1);
        lowest = lowest.min(implied);
        highest = highest.max(implied);
        println!(
            "tscprobe: window {window} burn {} work {work} rtc-seconds {elapsed} delta {delta} \
             implied {implied} Hz (tail poll {poll_iterations} iterations)",
            if alu { "alu " } else { "port" },
        );
    }

    let total = tsc_at_edge - first_tsc;
    let wrapped = u64::from((60 + last - first_seconds) % 60);
    let implied = total / reference_seconds_total.max(1);
    let spread = highest - lowest;
    println!(
        "tscprobe: over {reference_seconds_total} rtc seconds (register moved {wrapped} mod 60) \
         the tsc advanced {total}, implied {implied} Hz"
    );
    println!(
        "tscprobe: per-window implied rate lowest {lowest} highest {highest} spread {spread} Hz \
         ({} ppm of the measured rate)",
        spread.saturating_mul(1_000_000) / implied.max(1),
    );
    println!(
        "tscprobe: calibrated {hz} Hz vs measured {implied} Hz, calibration error {} ppm",
        hz.abs_diff(implied).saturating_mul(1_000_000) / implied.max(1),
    );
    println!("tscprobe: done");
}
