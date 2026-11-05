#![no_std]
//! **What a USB device says, what a host asks it, and what a boot keyboard's report means**: the
//! half of a USB keyboard driver that is about USB, not the host controller. It touches nothing.
//! Every descriptor and every report is whatever the device chose to send, and a device is whatever
//! a stranger plugged in, so its parsers are total: they bound every walk, check every length
//! before reading through it, and answer a [`Refusal`] that names what was wrong rather than
//! panicking or guessing. Its Kani harnesses prove that for every input up to a bound.
//!
//! - [`descriptor`]: the device descriptor, the configuration header, and
//!   [`descriptor::find_boot_keyboard`], the walk to a boot keyboard's interrupt IN endpoint.
//! - [`request`]: the four standard and class requests the driver sends, as setup packets.
//! - [`boot_keyboard`]: the eight-byte boot report, and two reports' difference as evdev key
//!   presses and releases, so a keymap that already reads evdev codes (as from a virtio keyboard)
//!   does the rest.
//!
//! Reuse: the usage to evdev table takes its usages and names from the USB-IF HID Usage Tables 1.5
//! and its numbers from the `KEY_*` names of Linux's `input-event-codes.h`, an ABI header. It
//! copies no driver. Three mappings the sources leave open (usages 0x32 and 0x46, and two-legend
//! usage names) are this crate's choices, which [`boot_keyboard`]'s documentation states.

//! # BUGS
//!
//! - Boot protocol only: a keyboard is found by its interface triple (3, 1, 1), no HID report
//!   descriptor is parsed, and one with no boot interface is [`Refusal::NoBootKeyboard`].
//! - The first boot keyboard interface wins; a second keyboard is not looked for.
//! - No key repeat: a boot keyboard reports state, not events, and typematic repeat is the
//!   host's job.
//! - No lock state and no LEDs: the lock keys are reported as key presses, and the crate keeps no
//!   Caps Lock or Num Lock state and builds no request that sets a keyboard's LEDs.
//!

//! Name: provisional, for milestone 242 (USB host and HID). `usb` and not `universal_serial_bus`
//! by DECISIONS §154 (the acronym test is whether the phrase is spoken), as `pci` was: nobody says
//! the expansion.

pub mod boot_keyboard;
pub mod descriptor;
pub mod request;

pub use descriptor::Refusal;

#[cfg(kani)]
mod verification;
