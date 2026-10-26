//! **What the data plane needs from the hardware**, so the simulator can stand in for the MAC.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

/// **The controller's register window, as a driver reaches it.** The kernel's implementation is a
/// pair of volatile accesses into the direct map; the tests' is `sim`. Offsets are bytes from the
/// window's base, and every one this crate passes is a constant from
/// [`registers`](crate::registers), below [`registers::WINDOW`](crate::registers::WINDOW).
///
/// Name: provisional. Ruled 2026-10-26 (UTC): renamed by the crate's rename table, replacing `Hw`.
pub trait Hardware {
    /// Read the 32-bit register at `offset`.
    fn read(&mut self, offset: u32) -> u32;
    /// Write `v` to the 32-bit register at `offset`.
    fn write(&mut self, offset: u32, v: u32);
    /// Wait at least `us` microseconds.
    fn delay_us(&mut self, us: u64);
}
