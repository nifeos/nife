//! **The JH7110's watchdog, the volatile half** (milestone 593 (a wedged kernel resets itself),
//! provisional number).
//!
//! `crates/jh7110_watchdog` owns the offsets, the sequences and the arithmetic. This walks the
//! sequences it is handed against a base it is handed, and reads two words back. It reaches into
//! no kernel global (DECISIONS §4 (kernel shape: monolithic, deferred, with two cheap rules)), and the clocks behind it are the caller's to turn on, through
//! `drivers::jh7110_clock_and_reset`.
//!
//! Kernel-resident for the reason that module gives and one more: the only thing that pets it is
//! the soak's supervisor, and a watchdog a user program pets resets the machine when that program
//! is starved, which is a different experiment.
//!
//! Name: provisional (milestone 593's lane, 2026-10-20 UTC), for the module and [`Jh7110Watchdog`].

use jh7110_watchdog::{Access, CONTROL, CONTROL_ENABLE, VALUE};

/// An armed or armable JH7110 watchdog: the window it lives in and the count each pet reloads.
///
/// Name: provisional, minted 2026-10-20 (UTC) by milestone/593-radon-watchdog.
pub struct Jh7110Watchdog {
    base: usize,
    load: u32,
}

impl Jh7110Watchdog {
    /// Hold the watchdog at `base`.
    ///
    /// # Safety
    ///
    /// `base` must be the kernel virtual address of a mapped, device-typed window of at least
    /// `jh7110_watchdog::WINDOW_USED` bytes belonging to a JH7110 watchdog whose clocks run and
    /// whose resets are released. A store to that window with its clock gated may hang the bus.
    ///
    /// Name: provisional, minted 2026-10-20 (UTC) by milestone/593-radon-watchdog.
    pub const unsafe fn new(base: usize) -> Self {
        Self { base, load: 0 }
    }

    fn read(&self, offset: usize) -> u32 {
        // SAFETY: `new`'s contract; every offset the crate names is inside `WINDOW_USED`.
        unsafe { core::ptr::read_volatile((self.base + offset) as *const u32) }
    }

    fn walk(&self, sequence: &[Access]) {
        for access in sequence {
            let (offset, value) = match *access {
                Access::Write { offset, value } => (offset, value),
                Access::Set { offset, mask } => (offset, self.read(offset) | mask),
                Access::Clear { offset, mask } => (offset, self.read(offset) & !mask),
            };
            // SAFETY: as `read`. The sequences unlock first and lock last, so a store between them
            // reaches the register and nothing outside them can.
            unsafe { core::ptr::write_volatile((self.base + offset) as *mut u32, value) };
        }
    }

    /// Start the timer with `load` ticks per count. The reset follows two counts after the last
    /// pet.
    ///
    /// Name: provisional, minted 2026-10-20 (UTC) by milestone/593-radon-watchdog.
    pub fn arm(&mut self, load: u32) {
        self.load = load;
        self.walk(&jh7110_watchdog::arm_sequence(load));
    }

    /// Reload the count, and clear a first expiry if one happened.
    ///
    /// Name: provisional, minted 2026-10-20 (UTC) by milestone/593-radon-watchdog.
    pub fn pet(&self) {
        self.walk(&jh7110_watchdog::pet_sequence(self.load));
    }

    /// The count as the timer holds it now, in core-clock ticks.
    ///
    /// Name: provisional, minted 2026-10-20 (UTC) by milestone/593-radon-watchdog.
    pub fn ticks_left(&self) -> u32 {
        self.read(VALUE)
    }

    /// The count each pet reloads.
    ///
    /// Name: provisional, minted 2026-10-20 (UTC) by milestone/593-radon-watchdog.
    pub fn reload(&self) -> u32 {
        self.load
    }

    /// Stop the timer. [`Self::is_counting`] says whether it took.
    ///
    /// Name: provisional, minted 2026-10-20 (UTC) by milestone/593-radon-watchdog.
    pub fn disarm(&self) {
        self.walk(&jh7110_watchdog::disarm_sequence());
    }

    /// Whether the enable bit reads set, so the timer is still running toward a reset.
    ///
    /// Name: provisional, minted 2026-10-20 (UTC) by milestone/593-radon-watchdog.
    pub fn is_counting(&self) -> bool {
        self.read(CONTROL) & CONTROL_ENABLE != 0
    }
}
