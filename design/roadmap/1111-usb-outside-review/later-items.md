# P2: later

An appendix to [the milestone](../1111-usb-outside-review.md).

## P2: after the first release, or when a lane is there anyway

11. `control_max_packet` is looser than the specification at two speeds.
    Finding: below SuperSpeed the function accepts 8, 16, 32 and 64 for any speed. USB 2.0 section
    5.5.3 allows only 8 at low speed and only 64 at high speed; the four values are legal at full
    speed alone. The function takes a boolean, so it cannot know. The function accepts a low-speed
    keyboard that declares     64, and the controller hears a packet size the device cannot carry. Linux's
    handling of this case was not checked (unverified).
    Evidence: `crates/usb/src/descriptor.rs` lines 103 to 117.
    Change: take the four-way `Speed` instead of a boolean, as Decision 3 decides, provisionally.
    Door: two-way before the first release. Effort: S.

12. Reserved bits of `bEndpointAddress` pass through to the caller.
    Finding: `BootKeyboard::endpoint` is the byte as sent. Scratch test: the crate returns `0xf1` as
    `0xf1`. The specification reserves bits 6:4 and says they reset to zero. The xHCI crate's
    `endpoint_index` masks them, so nothing breaks today, but the published value should be the
    number and the direction and nothing else.
    Evidence: `crates/usb/src/descriptor.rs` lines 234 and 246.
    Change: `endpoint: address & 0x8f`, and one sentence in the field's documentation.
    Door: two-way. Effort: XS.

13. Documentation gaps a stranger meets.
    Finding: `find_boot_keyboard` does not say the walk begins at the header's own `bLength`, so a
    header of 18 bytes skips what follows it by the device's own claim (scratch test: answers
    `NoBootKeyboard`, which is right but surprising). `BootReport::parse` does not say that it
    accepts a     buffer longer than eight bytes and ignores its tail (scratch test: it does). The
    `the_table_matches_its_linux_cross_check` test comment carries a date and a commit, which is
    the right place for them, but the README (item 4) should carry the same two facts. `get_descriptor`'s
    parameter `index` is the descriptor index in `wValue` while the struct field `index` is
    `wIndex`; one sentence would prevent the misread.
    Evidence: `crates/usb/src/descriptor.rs` lines 165 to 173 and 208; `crates/usb/src/boot_keyboard.rs`
    lines 70 to 73; `crates/usb/src/request.rs` lines 43 to 46.
    Change: four sentences.
    Door: two-way. Effort: XS.

14. Private names the report-only gate lists.
    Finding: `abbreviations.py` reports `QEMU_USB_KBD`, `dev`, `diff`, `ab`, `shift_ab`, `keycaps`
    and `evdev`. The first five are test locals and the maintainer's; `keycaps` is a word in this
    domain; `evdev` is Linux's own name for its input event interface and stays, with the plain
    name in the function's documentation.
    Evidence: `crates/usb/src/descriptor.rs` lines 275 and 415; `crates/usb/src/boot_keyboard.rs`
    lines 121, 286, 294, 380 and 389.
    Change: `QEMU_USB_KEYBOARD`, `device`, `difference`, `shift_a_b`, `a_b`; the rest stay.
    Door: two-way. Effort: XS.
