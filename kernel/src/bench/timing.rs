//! **The iteration counts and the timed window** every kernel-thread benchmark shares.
//!
//! Name: provisional (lane `maintainer/last-three-over-2000`, 2026-10-12 UTC), for the module and
//! the file.

use super::*;

/// Iterations per benchmark. Fixed and part of the output, so a baseline is self-describing.
pub(super) const YIELD_ITERS: u64 = 2000;
pub(super) const IPC_ITERS: u64 = 1000;
pub(super) const RELAY_ITERS: u64 = 1000;
pub(super) const CALL_ITERS: u64 = 1000;
pub(super) const SPAWN_ITERS: u64 = 64;
pub(super) const MAP_ITERS: u64 = 64;
pub(super) const COREMARK_ITERS: u64 = 256;

/// Untimed shakeout before each measured loop: thread startup, first rendezvous, cold paths.
pub(super) const WARMUP: u64 = 32;

/// **Time `f` as the row `name`, in a window no preemption tick lands in.** Opens a [`TickWindow`],
/// so the window starts just after a tick, and closes it once the row is printed.
pub(super) fn timed(name: &str, iters: u64, f: impl FnOnce()) {
    let window = TickWindow::open(name);
    measure(name, iters, f);
    window.close();
}

/// **Time `f` and print the row, with no tick alignment of its own.** For a caller that holds a
/// [`TickWindow`] already and must do something between opening it and timing: `map_new` masks
/// interrupts there, and a window cannot wait for a tick with interrupts masked.
pub(super) fn measure(name: &str, iters: u64, f: impl FnOnce()) {
    let t0 = crate::arch::timer::now();
    f();
    let t1 = crate::arch::timer::now();
    println!("bench: {name} {} {iters}", t1 - t0);
}

/// A stretch of a bench run that no preemption tick lands in, and the check that fails if one does.
///
/// [`open`](Self::open) waits for this core's next tick and [`close`](Self::close) panics if
/// another arrived since. Ticks are counted per core, so a `--real --smp` run, whose thread may
/// change cores, prints a probe line instead. See notes/benchmarks/kernel-bench-harness.md.
///
/// Name: provisional (milestone 939, 2026-10-22 UTC).
#[must_use = "a window that is never closed checks nothing"]
pub(super) struct TickWindow<'a> {
    name: &'a str,
    cpu: usize,
    ticks: u64,
}

impl<'a> TickWindow<'a> {
    /// Wait for this core's next preemption tick, then open the window for the row `name`.
    /// Interrupts must be on: the tick is what ends the wait.
    pub(super) fn open(name: &'a str) -> Self {
        let cpu = crate::cpu::id();
        let seen = crate::arch::timer::ticks_on(cpu);
        while crate::arch::timer::ticks_on(cpu) == seen {
            crate::arch::wait_for_interrupt();
        }
        // The tick may have rescheduled this thread onto another core.
        let cpu = crate::cpu::id();
        Self {
            name,
            cpu,
            ticks: crate::arch::timer::ticks_on(cpu),
        }
    }

    /// Close the window.
    ///
    /// # Panics
    /// If a preemption tick arrived on this core since [`open`](Self::open). The bench harness
    /// stops on the panic line and fails the run.
    pub(super) fn close(self) {
        if crate::cpu::id() != self.cpu {
            println!(
                "bench-probe: {} moved cores, so its tick check did not run",
                self.name
            );
            return;
        }
        let landed = crate::arch::timer::ticks_on(self.cpu) - self.ticks;
        assert!(
            landed == 0,
            "{landed} preemption tick(s) landed in {}'s timed window, so the row would read the \
             tick's phase as its cost (milestone 939); the window outgrew a tick period at TICK_HZ {}",
            self.name,
            crate::arch::timer::TICK_HZ,
        );
    }
}
