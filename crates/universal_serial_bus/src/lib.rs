#![no_std]
#![forbid(unsafe_code)]
//! **What a Universal Serial Bus (USB) device says, what a host asks it, and what a boot keyboard's report means**: the
//! half of a USB keyboard driver that is about USB, not the host controller. It touches nothing.
//! Every descriptor and every report is whatever the device chose to send, and a device is whatever
//! a stranger plugged in, so its parsers are total: they bound every walk, check every length
//! before reading through it, and answer a [`Refusal`] that names what was wrong rather than
//! panicking or guessing. Its Kani harnesses prove that for every input up to a bound.
//!
//! The bounds are these. Every byte string of up to 20 bytes, for the device descriptor and
//! configuration header parsers. Every configuration of up to 25 bytes, the shortest that reaches
//! an endpoint, for [`descriptor::find_boot_keyboard`]; and up to 41 bytes in one fixed shape (a
//! header, then two interfaces each followed by an endpoint, every byte but the lengths and types
//! symbolic), for the paths that need a second interface. Every pair of eight-byte reports, for
//! [`boot_keyboard::changes`]: at most twenty events, in the documented order, each naming a key
//! that changed. Two things lie outside them: a configuration longer than 25 bytes in any other
//! shape, which is what a real composite keyboard sends (the unit tests walk several), and the
//! converse of the report proof, that every key which changed produces an event.
//!
//! - [`descriptor`]: the device descriptor, the configuration header, and
//!   [`descriptor::find_boot_keyboard`], the walk to a boot keyboard's interrupt endpoint, the direction `IN`, device to host.
//! - [`request`]: the four standard and class requests the driver sends, as setup packets.
//! - [`boot_keyboard`]: the eight-byte boot report, and two reports' difference as key presses
//!   and releases in Human Interface Device (HID) usages. [`boot_keyboard::evdev_code`] maps a usage to an evdev code, for a
//!   keymap that already reads those (as from a virtio keyboard).
//!
//! Reuse: the usage to evdev table takes its usages and names from the USB Implementers Forum (USB-IF)
//! HID Usage Tables 1.5 and its numbers from the `KEY_*` names of Linux's `input-event-codes.h`, an
//! Application Binary Interface (ABI) header. It copies no driver. Six mappings the sources leave open are this crate's choices, which
//! [`boot_keyboard`]'s documentation states.

//! # BUGS
//!
//! - Boot protocol only: a keyboard is found by its interface triple (3, 1, 1), no HID report
//!   descriptor is parsed, and one with no boot interface is [`Refusal::NoBootKeyboard`].
//! - The first boot keyboard interface wins; a second keyboard is not looked for.
//! - No key repeat: a boot keyboard reports state, not events, and typematic repeat is the
//!   host's job.
//! - No lock state and no light-emitting diodes (LEDs): the lock keys are reported as key presses, and the crate keeps no
//!   Caps Lock or Num Lock state and builds no request that sets a keyboard's LEDs.
//!

#[doc(alias = "usb")]
pub mod boot_keyboard;
#[doc(alias = "usb")]
pub mod descriptor;
#[doc(alias = "usb")]
pub mod request;

pub use descriptor::Refusal;

#[cfg(kani)]
mod verification;
