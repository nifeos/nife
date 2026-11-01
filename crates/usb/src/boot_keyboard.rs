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
//! `include/uapi/linux/input-event-codes.h`, whose name each row carries in a comment. Two rows are
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
    29,  // KEY_LEFTCTRL
    42,  // KEY_LEFTSHIFT
    56,  // KEY_LEFTALT
    125, // KEY_LEFTMETA
    97,  // KEY_RIGHTCTRL
    54,  // KEY_RIGHTSHIFT
    100, // KEY_RIGHTALT
    126, // KEY_RIGHTMETA
];

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
    const TABLE: [u16; 0x60] = [
        30,  // 0x04 a and A: KEY_A
        48,  // 0x05 b and B: KEY_B
        46,  // 0x06 c and C: KEY_C
        32,  // 0x07 d and D: KEY_D
        18,  // 0x08 e and E: KEY_E
        33,  // 0x09 f and F: KEY_F
        34,  // 0x0a g and G: KEY_G
        35,  // 0x0b h and H: KEY_H
        23,  // 0x0c i and I: KEY_I
        36,  // 0x0d j and J: KEY_J
        37,  // 0x0e k and K: KEY_K
        38,  // 0x0f l and L: KEY_L
        50,  // 0x10 m and M: KEY_M
        49,  // 0x11 n and N: KEY_N
        24,  // 0x12 o and O: KEY_O
        25,  // 0x13 p and P: KEY_P
        16,  // 0x14 q and Q: KEY_Q
        19,  // 0x15 r and R: KEY_R
        31,  // 0x16 s and S: KEY_S
        20,  // 0x17 t and T: KEY_T
        22,  // 0x18 u and U: KEY_U
        47,  // 0x19 v and V: KEY_V
        17,  // 0x1a w and W: KEY_W
        45,  // 0x1b x and X: KEY_X
        21,  // 0x1c y and Y: KEY_Y
        44,  // 0x1d z and Z: KEY_Z
        2,   // 0x1e 1: KEY_1
        3,   // 0x1f 2: KEY_2
        4,   // 0x20 3: KEY_3
        5,   // 0x21 4: KEY_4
        6,   // 0x22 5: KEY_5
        7,   // 0x23 6: KEY_6
        8,   // 0x24 7: KEY_7
        9,   // 0x25 8: KEY_8
        10,  // 0x26 9: KEY_9
        11,  // 0x27 0: KEY_0
        28,  // 0x28 Return: KEY_ENTER
        1,   // 0x29 Escape: KEY_ESC
        14,  // 0x2a Delete (Backspace): KEY_BACKSPACE
        15,  // 0x2b Tab: KEY_TAB
        57,  // 0x2c Spacebar: KEY_SPACE
        12,  // 0x2d - and _: KEY_MINUS
        13,  // 0x2e = and +: KEY_EQUAL
        26,  // 0x2f [ and {: KEY_LEFTBRACE
        27,  // 0x30 ] and }: KEY_RIGHTBRACE
        43,  // 0x31 \ and |: KEY_BACKSLASH
        43,  // 0x32 Non-US # and ~: KEY_BACKSLASH
        39,  // 0x33 ; and :: KEY_SEMICOLON
        40,  // 0x34 ' and ": KEY_APOSTROPHE
        41,  // 0x35 Grave Accent and Tilde: KEY_GRAVE
        51,  // 0x36 , and <: KEY_COMMA
        52,  // 0x37 . and >: KEY_DOT
        53,  // 0x38 / and ?: KEY_SLASH
        58,  // 0x39 Caps Lock: KEY_CAPSLOCK
        59,  // 0x3a F1: KEY_F1
        60,  // 0x3b F2: KEY_F2
        61,  // 0x3c F3: KEY_F3
        62,  // 0x3d F4: KEY_F4
        63,  // 0x3e F5: KEY_F5
        64,  // 0x3f F6: KEY_F6
        65,  // 0x40 F7: KEY_F7
        66,  // 0x41 F8: KEY_F8
        67,  // 0x42 F9: KEY_F9
        68,  // 0x43 F10: KEY_F10
        87,  // 0x44 F11: KEY_F11
        88,  // 0x45 F12: KEY_F12
        99,  // 0x46 Print Screen: KEY_SYSRQ
        70,  // 0x47 Scroll Lock: KEY_SCROLLLOCK
        119, // 0x48 Pause: KEY_PAUSE
        110, // 0x49 Insert: KEY_INSERT
        102, // 0x4a Home: KEY_HOME
        104, // 0x4b Page Up: KEY_PAGEUP
        111, // 0x4c Delete Forward: KEY_DELETE
        107, // 0x4d End: KEY_END
        109, // 0x4e Page Down: KEY_PAGEDOWN
        106, // 0x4f Right Arrow: KEY_RIGHT
        105, // 0x50 Left Arrow: KEY_LEFT
        108, // 0x51 Down Arrow: KEY_DOWN
        103, // 0x52 Up Arrow: KEY_UP
        69,  // 0x53 Num Lock: KEY_NUMLOCK
        98,  // 0x54 Keypad /: KEY_KPSLASH
        55,  // 0x55 Keypad *: KEY_KPASTERISK
        74,  // 0x56 Keypad -: KEY_KPMINUS
        78,  // 0x57 Keypad +: KEY_KPPLUS
        96,  // 0x58 Keypad Enter: KEY_KPENTER
        79,  // 0x59 Keypad 1: KEY_KP1
        80,  // 0x5a Keypad 2: KEY_KP2
        81,  // 0x5b Keypad 3: KEY_KP3
        75,  // 0x5c Keypad 4: KEY_KP4
        76,  // 0x5d Keypad 5: KEY_KP5
        77,  // 0x5e Keypad 6: KEY_KP6
        71,  // 0x5f Keypad 7: KEY_KP7
        72,  // 0x60 Keypad 8: KEY_KP8
        73,  // 0x61 Keypad 9: KEY_KP9
        82,  // 0x62 Keypad 0: KEY_KP0
        83,  // 0x63 Keypad .: KEY_KPDOT
    ];
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
