//! **What the PCH sequences need from the hardware**: the register access, delays and flushes a
//! sequence is written against, so the simulator can stand in for the device.

use super::*;

/// **What the I219 sequences need from whoever holds BAR0.** A kernel implements it over its
/// mapping of the registers; the host tests implement it over a simulated device.
pub trait Hw {
    /// Read the 32-bit register at `off` in BAR0.
    fn read(&mut self, off: u64) -> u32;
    /// Write it.
    fn write(&mut self, off: u64, v: u32);
    /// Read the 16-bit register at `off`. Only the flash status word ([`nvm::HSFSTS`]) is read
    /// this way, as FreeBSD reads it.
    fn read16(&mut self, off: u64) -> u16;
    /// Wait at least `us` microseconds.
    fn delay_us(&mut self, us: u64);
    /// The function's PCI vendor id, read from configuration space. The read is the point: it is
    /// the delay `e1000_reset_hw_ich8lan` needs around the global reset.
    fn pci_vendor_id(&mut self) -> u16;
    /// Something the bench boot should show. Never needed for correctness.
    fn note(&mut self, n: sequence::Note);
}

pub(super) fn delay_ms(hw: &mut impl Hw, ms: u64) {
    hw.delay_us(ms * 1000);
}

/// A posted-write flush (`E1000_WRITE_FLUSH`): any read of BAR0 will do; FreeBSD reads `STATUS`.
pub(super) fn flush_writes(hw: &mut impl Hw) {
    let _ = hw.read(crate::regs::STATUS);
}

/// Is `device` a PCH part (an I219) rather than the 82574L? Every I219 id this crate claims is in
/// FreeBSD's `e1000_pch_spt` class (`e1000_api.c`, `e1000_set_mac_type`), which is the class all
/// three ported sequences are written for.
pub fn is_pch(device: u16) -> bool {
    crate::is_supported(crate::VENDOR_INTEL, device) && device != 0x10d3
}
