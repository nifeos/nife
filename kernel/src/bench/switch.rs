//! **What a context switch costs**: two threads yielding, and on `x86_64` the I/O-permission
//! bitmap a switch may have to carry.
//!
//! Name: provisional (lane `maintainer/last-three-over-2000`, 2026-10-12 UTC), for the module and
//! the file.

use super::*;

/// **The context switch, round trip.** Two threads yielding to each other; each of our yields
/// is one switch out and (eventually) one switch back in. Ticks/iter ~= two switches.
pub(super) fn yield_switch() {
    static DONE: AtomicBool = AtomicBool::new(false);

    sched::spawn(|| {
        while !DONE.load(Ordering::Relaxed) {
            sched::yield_now();
        }
    })
    .expect("bench: no peer thread");

    for _ in 0..WARMUP {
        sched::yield_now();
    }
    timed("yield_switch", YIELD_ITERS, || {
        for _ in 0..YIELD_ITERS {
            sched::yield_now();
        }
    });
    DONE.store(true, Ordering::Relaxed);
    sched::yield_now(); // let the peer see the flag and exit
}

/// What writing the whole 8 KiB TSS I/O permission bitmap on every switch-in costs, for §121 (what
/// a device capability is when the device has no page: x86 port I/O).
///
/// [`yield_switch`] plus one [`crate::arch::segments::bench_write_io_bitmap`] call on every resume
/// in both threads; read against it from the same boot. See notes/benchmarks/x86-tss-iomap.md.
#[cfg(target_arch = "x86_64")]
pub(super) fn tss_iomap_switch() {
    use core::sync::atomic::AtomicU8;

    static DONE: AtomicBool = AtomicBool::new(false);
    // Folds every write's last byte in, so the optimizer cannot prove the bitmap writes are dead
    // even though nothing ever reads BENCH_IOMAP back through a port instruction.
    static SINK: AtomicU8 = AtomicU8::new(0);

    fn switch_and_write(pattern: &mut u8) {
        sched::yield_now();
        *pattern = pattern.wrapping_add(1);
        SINK.fetch_xor(
            crate::arch::segments::bench_write_io_bitmap(*pattern),
            Ordering::Relaxed,
        );
    }

    sched::spawn(|| {
        let mut pattern = 0u8;
        while !DONE.load(Ordering::Relaxed) {
            switch_and_write(&mut pattern);
        }
    })
    .expect("bench: no peer thread");

    let mut pattern = 0u8;
    for _ in 0..WARMUP {
        switch_and_write(&mut pattern);
    }
    timed("tss_iomap_switch", YIELD_ITERS, || {
        for _ in 0..YIELD_ITERS {
            switch_and_write(&mut pattern);
        }
    });
    DONE.store(true, Ordering::Relaxed);
    sched::yield_now(); // let the peer see the flag and exit
}

/// What the lazy I/O-bitmap write, [`set_port_range_grant`], costs when every switch crosses a port
/// holder: the worst case.
///
/// [`yield_switch`] with the two threads installing different grants on resume, none and COM1's
/// production `(0x3F8, 8)`; read against it from the same boot. Nothing runs `in`/`out`, and the
/// loop restores `None` so the TSS is left denying all ports. See
/// notes/benchmarks/x86-tss-iomap.md.
#[cfg(target_arch = "x86_64")]
pub(super) fn tss_iomap_lazy_switch() {
    const COM1: Option<(u16, u16)> = Some((0x3F8, 8));
    static DONE: AtomicBool = AtomicBool::new(false);

    sched::spawn(|| {
        while !DONE.load(Ordering::Relaxed) {
            sched::yield_now();
            crate::arch::segments::set_port_range_grant(None); // the non-holder side
        }
    })
    .expect("bench: no peer thread");

    for _ in 0..WARMUP {
        sched::yield_now();
        crate::arch::segments::set_port_range_grant(COM1);
    }
    timed("tss_iomap_lazy_switch", YIELD_ITERS, || {
        for _ in 0..YIELD_ITERS {
            sched::yield_now();
            crate::arch::segments::set_port_range_grant(COM1); // the holder side, every iteration a transition
        }
    });
    DONE.store(true, Ordering::Relaxed);
    sched::yield_now(); // let the peer see the flag and exit
    crate::arch::segments::set_port_range_grant(None); // leave the TSS denying all ports
}

/// What the lazy write costs when nothing holds a port, the common case: both threads install
/// `None`, so [`set_port_range_grant`] compares once and writes nothing. Read against
/// [`yield_switch`].
#[cfg(target_arch = "x86_64")]
pub(super) fn tss_iomap_lazy_nop() {
    static DONE: AtomicBool = AtomicBool::new(false);

    sched::spawn(|| {
        while !DONE.load(Ordering::Relaxed) {
            sched::yield_now();
            crate::arch::segments::set_port_range_grant(None);
        }
    })
    .expect("bench: no peer thread");

    for _ in 0..WARMUP {
        sched::yield_now();
        crate::arch::segments::set_port_range_grant(None);
    }
    timed("tss_iomap_lazy_nop", YIELD_ITERS, || {
        for _ in 0..YIELD_ITERS {
            sched::yield_now();
            crate::arch::segments::set_port_range_grant(None);
        }
    });
    DONE.store(true, Ordering::Relaxed);
    sched::yield_now();
}
