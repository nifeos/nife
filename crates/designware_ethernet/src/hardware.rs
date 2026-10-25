//! **What the data plane needs from the hardware**, so the simulator can stand in for the MAC.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

/// **The controller's register window, as a driver reaches it.** The kernel's implementation is a
/// pair of volatile accesses into the direct map; the tests' is `sim`. Offsets are bytes from the
/// window's base, and every one this crate passes is a constant from [`regs`](crate::regs), below
/// [`regs::WINDOW`](crate::regs::WINDOW).
pub trait Hw {
    /// Read the 32-bit register at `off`.
    fn read(&mut self, off: u32) -> u32;
    /// Write `v` to the 32-bit register at `off`.
    fn write(&mut self, off: u32, v: u32);
    /// Wait at least `us` microseconds.
    fn delay_us(&mut self, us: u64);
}
