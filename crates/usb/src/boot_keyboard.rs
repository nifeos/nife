//! **The boot keyboard report, and what changed between two of them** (HID 1.11 Appendix B.1,
//! HID Usage Tables 1.12 section 10).
//!
//! A boot keyboard does not send key events. It sends its **state**: eight bytes, the modifier
//! keys as a bitmap in byte 0, a reserved byte, then up to six usage ids for the other keys held
//! down, in no promised order. So a keystroke is a *difference* between two reports, and
//! [`changes`] computes it, in the evdev numbering so that `video_terminal::keymap` (which already
//! turns the virtio keyboard's evdev codes into terminal bytes) does the rest.
//!
//! # Where the table comes from
//!
//! [`evdev_code`]'s table is read off two documents: each row's usage and name from the HID Usage
//! Tables 1.12 keyboard page (section 10), and each code's number from the `KEY_*` names in
//! `include/uapi/linux/input-event-codes.h`, whose names the table spells out. Two rows are
//! nife's own choices, since the Usage Tables name no evdev key for them: usage 0x32 (Non-US `#`
//! and `~`) is reported as `KEY_BACKSLASH`, the key the usage sits beside, and usage 0x46 (Print
//! Screen) is reported as `KEY_SYSRQ`, the key a terminal keyboard labels that way.
//!
//! One comparison was run, once, against Linux's `hid_keyboard[]` in `drivers/hid/hid-input.c` at
//! `torvalds/linux@cbbddb6b08cc9900a87cccaa9b22bbe06aa4f50a`, fetched 2026-11-01 (UTC). The two
//! agree on every usage from 0x04 to 0x63, which is expected: both place one physical key at one
//! number. The longest run of identical lower-case word tokens between this file and
//! `hid-input.c` is 6, a row of test report bytes, and between this file and the header it is 4,
//! the header's own path. No test carries the Linux table.

/// How long a boot report is.
pub const REPORT_LEN: usize = 8;

/// The usage id a keyboard puts in every key slot when more keys are held than it can report
/// (`ErrorRollOver`, Usage Tables section 10). A report carrying it describes no real state.
pub const ERROR_ROLL_OVER: u8 = 0x01;

/// The evdev codes of the eight modifier keys, in the bit order of byte 0: left control, shift,
/// alt, GUI, then the right-hand four.
pub const MODIFIER_CODES: [u16; 8] = [
    evdev::KEY_LEFTCTRL,
    evdev::KEY_LEFTSHIFT,
    evdev::KEY_LEFTALT,
    evdev::KEY_LEFTMETA,
    evdev::KEY_RIGHTCTRL,
    evdev::KEY_RIGHTSHIFT,
    evdev::KEY_RIGHTALT,
    evdev::KEY_RIGHTMETA,
];

/// The evdev key codes this module reports, under the names of
/// `include/uapi/linux/input-event-codes.h`.
#[allow(non_upper_case_globals)]
mod evdev {
    pub const KEY_ESC: u16 = 1;
    pub const KEY_1: u16 = 2;
    pub const KEY_2: u16 = 3;
    pub const KEY_3: u16 = 4;
    pub const KEY_4: u16 = 5;
    pub const KEY_5: u16 = 6;
    pub const KEY_6: u16 = 7;
    pub const KEY_7: u16 = 8;
    pub const KEY_8: u16 = 9;
    pub const KEY_9: u16 = 10;
    pub const KEY_0: u16 = 11;
    pub const KEY_MINUS: u16 = 12;
    pub const KEY_EQUAL: u16 = 13;
    pub const KEY_BACKSPACE: u16 = 14;
    pub const KEY_TAB: u16 = 15;
    pub const KEY_Q: u16 = 16;
    pub const KEY_W: u16 = 17;
    pub const KEY_E: u16 = 18;
    pub const KEY_R: u16 = 19;
    pub const KEY_T: u16 = 20;
    pub const KEY_Y: u16 = 21;
    pub const KEY_U: u16 = 22;
    pub const KEY_I: u16 = 23;
    pub const KEY_O: u16 = 24;
    pub const KEY_P: u16 = 25;
    pub const KEY_LEFTBRACE: u16 = 26;
    pub const KEY_RIGHTBRACE: u16 = 27;
    pub const KEY_ENTER: u16 = 28;
    pub const KEY_LEFTCTRL: u16 = 29;
    pub const KEY_A: u16 = 30;
    pub const KEY_S: u16 = 31;
    pub const KEY_D: u16 = 32;
    pub const KEY_F: u16 = 33;
    pub const KEY_G: u16 = 34;
    pub const KEY_H: u16 = 35;
    pub const KEY_J: u16 = 36;
    pub const KEY_K: u16 = 37;
    pub const KEY_L: u16 = 38;
    pub const KEY_SEMICOLON: u16 = 39;
    pub const KEY_APOSTROPHE: u16 = 40;
    pub const KEY_GRAVE: u16 = 41;
    pub const KEY_LEFTSHIFT: u16 = 42;
    pub const KEY_BACKSLASH: u16 = 43;
    pub const KEY_Z: u16 = 44;
    pub const KEY_X: u16 = 45;
    pub const KEY_C: u16 = 46;
    pub const KEY_V: u16 = 47;
    pub const KEY_B: u16 = 48;
    pub const KEY_N: u16 = 49;
    pub const KEY_M: u16 = 50;
    pub const KEY_COMMA: u16 = 51;
    pub const KEY_DOT: u16 = 52;
    pub const KEY_SLASH: u16 = 53;
    pub const KEY_RIGHTSHIFT: u16 = 54;
    pub const KEY_KPASTERISK: u16 = 55;
    pub const KEY_LEFTALT: u16 = 56;
    pub const KEY_SPACE: u16 = 57;
    pub const KEY_CAPSLOCK: u16 = 58;
    pub const KEY_F1: u16 = 59;
    pub const KEY_F2: u16 = 60;
    pub const KEY_F3: u16 = 61;
    pub const KEY_F4: u16 = 62;
    pub const KEY_F5: u16 = 63;
    pub const KEY_F6: u16 = 64;
    pub const KEY_F7: u16 = 65;
    pub const KEY_F8: u16 = 66;
    pub const KEY_F9: u16 = 67;
    pub const KEY_F10: u16 = 68;
    pub const KEY_NUMLOCK: u16 = 69;
    pub const KEY_SCROLLLOCK: u16 = 70;
    pub const KEY_KP7: u16 = 71;
    pub const KEY_KP8: u16 = 72;
    pub const KEY_KP9: u16 = 73;
    pub const KEY_KPMINUS: u16 = 74;
    pub const KEY_KP4: u16 = 75;
    pub const KEY_KP5: u16 = 76;
    pub const KEY_KP6: u16 = 77;
    pub const KEY_KPPLUS: u16 = 78;
    pub const KEY_KP1: u16 = 79;
    pub const KEY_KP2: u16 = 80;
    pub const KEY_KP3: u16 = 81;
    pub const KEY_KP0: u16 = 82;
    pub const KEY_KPDOT: u16 = 83;
    pub const KEY_F11: u16 = 87;
    pub const KEY_F12: u16 = 88;
    pub const KEY_KPENTER: u16 = 96;
    pub const KEY_RIGHTCTRL: u16 = 97;
    pub const KEY_KPSLASH: u16 = 98;
    pub const KEY_SYSRQ: u16 = 99;
    pub const KEY_RIGHTALT: u16 = 100;
    pub const KEY_HOME: u16 = 102;
    pub const KEY_UP: u16 = 103;
    pub const KEY_PAGEUP: u16 = 104;
    pub const KEY_LEFT: u16 = 105;
    pub const KEY_RIGHT: u16 = 106;
    pub const KEY_END: u16 = 107;
    pub const KEY_DOWN: u16 = 108;
    pub const KEY_PAGEDOWN: u16 = 109;
    pub const KEY_INSERT: u16 = 110;
    pub const KEY_DELETE: u16 = 111;
    pub const KEY_PAUSE: u16 = 119;
    pub const KEY_LEFTMETA: u16 = 125;
    pub const KEY_RIGHTMETA: u16 = 126;
}

/// **One boot report**: the modifier bitmap and the six key slots.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BootReport {
    /// Byte 0: one bit per modifier key, in [`MODIFIER_CODES`]' order.
    pub modifiers: u8,
    /// Bytes 2..8: usage ids of the keys held, zero for an empty slot.
    pub keys: [u8; 6],
}

impl BootReport {
    /// Nothing held: the state before the first report.
    pub const RELEASED: BootReport = BootReport {
        modifiers: 0,
        keys: [0; 6],
    };

    /// Parse a report. `None` for fewer than [`REPORT_LEN`] bytes, and for a **phantom** report
    /// (any slot holding [`ERROR_ROLL_OVER`] or the two other error usages, 2 and 3), which a
    /// driver must ignore rather than read as every key released.
    pub fn parse(b: &[u8]) -> Option<BootReport> {
        if b.len() < REPORT_LEN {
            return None;
        }
        let mut keys = [0u8; 6];
        keys.copy_from_slice(&b[2..8]);
        if keys.iter().any(|&k| (1..=3).contains(&k)) {
            return None;
        }
        Some(BootReport {
            modifiers: b[0],
            keys,
        })
    }

    fn holds(&self, usage: u8) -> bool {
        usage != 0 && self.keys.contains(&usage)
    }
}

/// **Usage id to evdev code**, for the keyboard page's usages 0x04 through 0x63: the letters,
/// digits, the main block, the function keys, the navigation cluster, the arrows and the keypad.
/// `None` for anything else, including 0 (no key). The module documentation says where the table
/// comes from.
pub fn evdev_code(usage: u8) -> Option<u16> {
    /// Indexed by usage id from 0x04. Every usage in the range has a code.
    const TABLE: [u16; 0x60] = {
        use self::evdev::*;
        [
            KEY_A,
            KEY_B,
            KEY_C,
            KEY_D,
            KEY_E,
            KEY_F,
            KEY_G,
            KEY_H,
            KEY_I,
            KEY_J,
            KEY_K,
            KEY_L,
            KEY_M,
            KEY_N,
            KEY_O,
            KEY_P,
            KEY_Q,
            KEY_R,
            KEY_S,
            KEY_T,
            KEY_U,
            KEY_V,
            KEY_W,
            KEY_X,
            KEY_Y,
            KEY_Z, // 0x04 Keyboard a and A .. 0x1d z and Z
            KEY_1,
            KEY_2,
            KEY_3,
            KEY_4,
            KEY_5,
            KEY_6,
            KEY_7,
            KEY_8,
            KEY_9,
            KEY_0, // 0x1e 1 .. 0x27 0
            KEY_ENTER,
            KEY_ESC,
            KEY_BACKSPACE,
            KEY_TAB,
            KEY_SPACE, // 0x28 Return, Escape, Delete (Backspace), Tab, Spacebar
            KEY_MINUS,
            KEY_EQUAL,
            KEY_LEFTBRACE,
            KEY_RIGHTBRACE,
            KEY_BACKSLASH,
            KEY_BACKSLASH, // 0x2d - _, = +, [ {, ] }, \ |, Non-US # ~ (0x32 is nife's choice, see above)
            KEY_SEMICOLON,
            KEY_APOSTROPHE,
            KEY_GRAVE,
            KEY_COMMA,
            KEY_DOT,
            KEY_SLASH,    // 0x33 ; :, ' ", ` ~, comma, period, slash
            KEY_CAPSLOCK, // 0x39 Caps Lock
            KEY_F1,
            KEY_F2,
            KEY_F3,
            KEY_F4,
            KEY_F5,
            KEY_F6,
            KEY_F7,
            KEY_F8,
            KEY_F9,
            KEY_F10,
            KEY_F11,
            KEY_F12, // 0x3a F1 .. 0x45 F12
            KEY_SYSRQ,
            KEY_SCROLLLOCK,
            KEY_PAUSE, // 0x46 Print Screen (nife's choice), Scroll Lock, Pause
            KEY_INSERT,
            KEY_HOME,
            KEY_PAGEUP,
            KEY_DELETE,
            KEY_END,
            KEY_PAGEDOWN, // 0x49 Insert, Home, Page Up, Delete Forward, End, Page Down
            KEY_RIGHT,
            KEY_LEFT,
            KEY_DOWN,
            KEY_UP, // 0x4f Right, Left, Down, Up arrow
            KEY_NUMLOCK,
            KEY_KPSLASH,
            KEY_KPASTERISK,
            KEY_KPMINUS,
            KEY_KPPLUS,
            KEY_KPENTER, // 0x53 Num Lock, keypad / * - + Enter
            KEY_KP1,
            KEY_KP2,
            KEY_KP3,
            KEY_KP4,
            KEY_KP5,
            KEY_KP6,
            KEY_KP7,
            KEY_KP8,
            KEY_KP9,
            KEY_KP0,
            KEY_KPDOT, // 0x59 keypad 1 .. 9, 0, period
        ]
    };
    TABLE.get(usize::from(usage).checked_sub(4)?).copied()
}

/// **What changed from `before` to `after`**, as `(evdev code, pressed)` calls to `event`, in the
/// order that types correctly: every modifier change first, then the keys released, then the keys
/// pressed. Shift and a letter arriving in one report must reach the keymap shift first, which is
/// why modifiers lead; a key released and another pressed in one report is a roll, and the release
/// first is what a person did. A usage with no evdev code is skipped.
///
/// At most twenty calls: eight modifiers, six releases, six presses.
///
/// # Examples
///
/// Shift and `h` arrive together in one report, so the shift press is delivered first:
///
/// ```
/// use usb::boot_keyboard::{BootReport, changes};
///
/// let before = BootReport::parse(&[0, 0, 0, 0, 0, 0, 0, 0]).unwrap();
/// let after = BootReport::parse(&[0x02, 0, 0x0b, 0, 0, 0, 0, 0]).unwrap(); // left shift, usage 0x0b (h)
/// let mut seen = Vec::new();
/// changes(&before, &after, &mut |code, pressed| seen.push((code, pressed)));
/// assert_eq!(seen, [(42, true), (35, true)]); // KEY_LEFTSHIFT, then KEY_H
/// ```
pub fn changes(before: &BootReport, after: &BootReport, event: &mut dyn FnMut(u16, bool)) {
    let flipped = before.modifiers ^ after.modifiers;
    for (bit, &code) in MODIFIER_CODES.iter().enumerate() {
        if flipped & (1 << bit) != 0 {
            event(code, after.modifiers & (1 << bit) != 0);
        }
    }
    for &usage in &before.keys {
        if usage != 0
            && !after.holds(usage)
            && let Some(code) = evdev_code(usage)
        {
            event(code, false);
        }
    }
    for (i, &usage) in after.keys.iter().enumerate() {
        // `after.keys[..i]` so a usage a confused device reports twice is pressed once.
        if usage != 0
            && !before.holds(usage)
            && !after.keys[..i].contains(&usage)
            && let Some(code) = evdev_code(usage)
        {
            event(code, true);
        }
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::vec::Vec;

    use super::*;

    fn report(modifiers: u8, keys: &[u8]) -> BootReport {
        let mut b = [0u8; 8];
        b[0] = modifiers;
        b[2..2 + keys.len()].copy_from_slice(keys);
        BootReport::parse(&b).unwrap()
    }

    fn diff(a: &BootReport, b: &BootReport) -> Vec<(u16, bool)> {
        let mut v = Vec::new();
        changes(a, b, &mut |c, p| v.push((c, p)));
        v
    }

    /// The table's rows land where the keycaps say: an off-by-one row puts `s` on the `a` key.
    #[test]
    fn the_usage_table_matches_the_keycaps() {
        for (usage, code) in [
            (0x04, 30), // a
            (0x1d, 44), // z
            (0x1e, 2),  // 1
            (0x27, 11), // 0
            (0x28, 28), // enter
            (0x2a, 14), // backspace
            (0x2c, 57), // space
            (0x38, 53), // slash
            (0x4f, 106),
            (0x52, 103), // up
            (0x63, 83),  // keypad dot, the table's last row
        ] {
            assert_eq!(evdev_code(usage), Some(code), "usage {usage:#x}");
        }
        assert_eq!(evdev_code(0), None);
        assert_eq!(evdev_code(3), None);
        assert_eq!(evdev_code(0x64), None);
        assert_eq!(evdev_code(0xff), None);
    }

    /// **`echo hello`, typed the way QEMU's `sendkey` presses it**: one key down, then up, eleven
    /// times. What the end-to-end gate types, decoded on the host.
    #[test]
    fn echo_hello_decodes_key_by_key() {
        let mut state = BootReport::RELEASED;
        let mut presses = Vec::new();
        for usage in [
            0x08u8, 0x06, 0x0b, 0x12, 0x2c, 0x0b, 0x08, 0x0f, 0x0f, 0x12, 0x28,
        ] {
            for next in [report(0, &[usage]), BootReport::RELEASED] {
                changes(&state, &next, &mut |code, pressed| {
                    if pressed {
                        presses.push(code);
                    }
                });
                state = next;
            }
        }
        // e c h o space h e l l o enter, in evdev codes.
        assert_eq!(presses, [18, 46, 35, 24, 57, 35, 18, 38, 38, 24, 28]);
    }

    #[test]
    fn shift_and_a_letter_together_deliver_shift_first() {
        assert_eq!(
            diff(&BootReport::RELEASED, &report(0x20, &[0x04])), // right shift + a
            [(54, true), (30, true)]
        );
    }

    /// A modifier held across two reports changes nothing, and letting it go is a release.
    #[test]
    fn a_held_modifier_is_quiet_and_its_release_is_a_release() {
        let shift_a = report(0x20, &[0x04]);
        let shift_ab = report(0x20, &[0x04, 0x05]);
        assert_eq!(diff(&shift_a, &shift_ab), [(48, true)]);
        assert_eq!(diff(&shift_ab, &report(0, &[0x04, 0x05])), [(54, false)]);
    }

    /// A roll: `a` still held, `b` pressed, then `a` released while `b` stays.
    #[test]
    fn a_roll_presses_each_key_once_and_releases_it_once() {
        let a = report(0, &[0x04]);
        let ab = report(0, &[0x04, 0x05]);
        let b = report(0, &[0x05]);
        assert_eq!(diff(&a, &ab), [(48, true)]);
        // Slot order is not promised, so `b` moving to slot 0 must not read as a new press.
        assert_eq!(diff(&ab, &b), [(30, false)]);
    }

    #[test]
    fn a_phantom_report_is_refused_rather_than_read_as_a_release() {
        assert_eq!(BootReport::parse(&[0, 0, 1, 1, 1, 1, 1, 1]), None);
        assert_eq!(BootReport::parse(&[0, 0, 0, 0]), None);
    }

    /// A device that reports one usage in two slots presses it once.
    #[test]
    fn a_duplicated_usage_presses_once() {
        assert_eq!(
            diff(&BootReport::RELEASED, &report(0, &[0x04, 0x04])),
            [(30, true)]
        );
    }
}
