//! **What the PCH sequences need from the hardware**: the register access, delays and flushes a
//! sequence is written against, so the simulator can stand in for the device.
//!
//! Name: ratified 2026-11-03 (UTC), #3145. Renamed from `hardware`, which did not say that the
//! module holds the access a sequence makes to the hardware.

use super::*;

/// **What the I219 sequences need from whoever holds BAR0.** A kernel implements it over its
/// mapping of the registers; the host tests implement it over a simulated device.
///
/// Name: ratified 2026-10-26 (#2761). Ruled in the ruling on e1000e's names. Refused `Hw`
/// (an abbreviation of hardware).
pub trait Hardware {
    /// Read the 32-bit register at `offset` in BAR0.
    fn read(&mut self, offset: u64) -> u32;
    /// Write it.
    fn write(&mut self, offset: u64, value: u32);
    /// Read the 16-bit register at `offset`. Only the flash status word
    /// ([`non_volatile_memory::HARDWARE_SEQUENCING_FLASH_STATUS_REGISTER`]) is read this way, as FreeBSD reads it.
    fn read16(&mut self, offset: u64) -> u16;
    /// Wait at least `microseconds` microseconds.
    fn delay_microseconds(&mut self, microseconds: u64);
    /// The function's PCI vendor id, read from configuration space. The read is the point: it is
    /// the delay `e1000_reset_hw_ich8lan` needs around the global reset.
    fn pci_vendor_id(&mut self) -> u16;
    /// Something the bench boot should show. Never needed for correctness.
    fn note(&mut self, note: sequence::BringUpObservation);
}

pub(super) fn delay_milliseconds(hardware: &mut impl Hardware, milliseconds: u64) {
    hardware.delay_microseconds(milliseconds * 1000);
}

/// A posted-write flush (`E1000_WRITE_FLUSH`): any read of BAR0 will do; FreeBSD reads `STATUS`.
pub(super) fn flush_writes(hardware: &mut impl Hardware) {
    let _ = hardware.read(crate::registers::DEVICE_STATUS_REGISTER);
}

/// Is `device` a PCH part (an I219) rather than the 82574L? Every I219 id this crate claims is in
/// FreeBSD's `e1000_pch_spt` class (`e1000_api.c`, `e1000_set_mac_type`), which is the class all
/// three ported sequences are written for.
///
/// Name: ratified 2026-11-03 (UTC), #3151. Renamed from `is_pch`; it takes a device id, and the
/// module it serves is the platform controller hub.
pub fn is_platform_controller_hub_device(device: u16) -> bool {
    crate::is_supported_device(crate::VENDOR_INTEL, device) && device != 0x10d3
}
