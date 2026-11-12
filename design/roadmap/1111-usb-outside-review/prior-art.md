# Prior art

An appendix to [the milestone](../1111-usb-outside-review.md).

How the crate sits against what a user already knows. Downloads are from the crates.io API at the
time of the review (all-time / recent 90 days).

| implementation | scope | how it solves the same problems | what a user coming from it will miss here |
|---|---|---|---|
| Linux `drivers/usb/core/config.c`, `drivers/hid/usbhid`, `drivers/hid/hid-input.c` | every device class | the configuration walk skips an endpoint descriptor shorter than 7 bytes with a notice, skips a descriptor for endpoint zero, skips a duplicate endpoint address, and repairs some `wMaxPacketSize` values; the keyboard is driven through the HID report descriptor, and `hid_keyboard[]` maps usages 0x00 to 0xff to `KEY_*` codes | report protocol, every usage page, LEDs, key repeat, quirks |
| OpenBSD `sys/dev/hid/hidkbd.c`, `sys/dev/usb/ukbd.c` | keyboards, boot and report protocol | `hidkbd_decode` ignores a report whose first key slot is `KEY_ERROR` (0x01), then emits modifier changes, then releases, then presses: the same order as `changes`; `ukbd` sends `SET_IDLE` 0 at attach | the translation table lives in `wskbd`, so the HID layer emits usages and not key codes |
| U-Boot `common/usb_kbd.c` | boot keyboards only, polled | requires subclass 1 protocol 1 and the first interrupt IN endpoint; treats a slot value of 0 to 3 as no key; sends `SET_PROTOCOL` 0 and `SET_IDLE`; implements typematic repeat and the LED report in 730 lines | repeat and LEDs, which this crate's BUGS section leaves to the host |
| `usb` 0.3.0 (21.4k / 226) | constants only | holds the crates.io name; defines descriptor type and request constants with no parser | nothing: it is what a search for the name finds first |
| `usb-host` 0.1.3 (8.0k / 83), `usbh` 0.1.0 (1.5k / 7) | host-side traits and an experimental stack | trait shapes a host driver implements; both dormant (unverified beyond the release dates) | an implementation of either trait |
| `usb-device` 0.3.2, `usbd-hid` 0.10.2 (1.57M / 316k), `embassy-usb` | the device side | `usbd-hid` builds the boot keyboard report a device sends, the mirror of what this crate parses | nothing that applies: they are the other end of the cable |
| `keycode` 1.0.0 (224k / 65k) | key-code translation | maps between HID usages, evdev, Windows, macOS and web codes as data tables | a second numbering, if the consumer is not Linux-shaped |

Where this crate differs: it parses only what a boot keyboard needs, in 1,100 lines, with no
allocation and no `unsafe`. It proves with Kani that the walk over device-chosen lengths is total
and accepts only an interrupt IN endpoint of at least eight bytes. No implementation above
carries a machine-checked proof of its descriptor walk. What it costs: boot protocol only, one
keyboard per configuration, a usage table that stops at 0x63 (item 6), and a key vocabulary that is
Linux's (Decision 2).

Concrete example. OpenBSD's `hidkbd_decode` ignores a phantom report only when slot 0 holds 0x01;
U-Boot treats each slot independently and ignores values 0 to 3. This crate refuses the whole report
when any slot holds 1, 2 or 3 (`BootReport::parse`), which is stricter than both and is the right
reading of HID 1.11 Appendix B: a roll-over report describes no state at all.
