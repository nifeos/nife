#![no_std]
//! **What a USB device says, what a host asks it, and what a boot keyboard's report means**: the
//! half of a USB keyboard driver that is about USB, not the host controller (milestone 242 (USB
//! host and HID)). It touches nothing, and its parsers are total over the device's untrusted bytes,
//! answering a [`Refusal`]; see notes/usb.md, "Untrusted descriptors".
//!
//! - [`descriptor`]: the device descriptor, the configuration header, and
//!   [`descriptor::find_boot_keyboard`], the walk to a boot keyboard's interrupt IN endpoint.
//! - [`request`]: the four standard and class requests the driver sends, as setup packets.
//! - [`boot_keyboard`]: the eight-byte boot report, and two reports' difference as evdev key
//!   presses and releases for `video_terminal::keymap`, the virtio keyboard's layout table.

//! # BUGS
//!
//! - Boot protocol only: a keyboard is found by its interface triple (3, 1, 1), no HID report
//!   descriptor is parsed, and one with no boot interface is [`Refusal::NoBootKeyboard`].
//! - The first boot keyboard interface wins; a second keyboard is not looked for.
//! - No key repeat: holding a key types it once, and typematic repeat is the host's job.
//! - Caps Lock, Num Lock and the LEDs do nothing: `video_terminal::keymap` maps no lock key.
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
