//! **From a keyboard's scancodes to the bytes a terminal receives** (milestone 29 (a display
//! terminal: framebuffer, virtio-gpu, and a foreign component)'s input).
//!
//! A virtio-input device sends Linux evdev codes, a key going down or up, not characters. This
//! turns them into the terminal contract's driver-half bytes (notes/terminal-contract.md,
//! `OPERATION_BYTES`) for `components/src/keyboard_driver.rs`, on the host where a wrong row is a
//! failed test rather than a QEMU boot. It covers a US layout's main block and the arrow and
//! navigation clusters, shifted and unshifted, and control on letters; no keypad, function keys,
//! compose, dead keys or other layout. See notes/video-terminal/palette-keymap-and-script.md.

/// `EV_SYN`: an event-batch separator. Carries no key and is ignored here.
pub const EV_SYN: u16 = 0;
/// `EV_KEY`: a key changed state. The only event type this cares about.
pub const EV_KEY: u16 = 1;

/// `KEY_LEFTSHIFT`.
pub(crate) const KEY_LEFTSHIFT: u16 = 42;
/// `KEY_RIGHTSHIFT`.
pub(crate) const KEY_RIGHTSHIFT: u16 = 54;
/// `KEY_LEFTCTRL`.
pub(crate) const KEY_LEFTCTRL: u16 = 29;
/// `KEY_RIGHTCTRL`.
pub(crate) const KEY_RIGHTCTRL: u16 = 97;

/// The highest evdev code this table knows. Above it, [`byte`] answers `None`.
pub const MAX_CODE: u16 = 57;

/// What each key sends with no modifier, indexed by evdev code. `0` means "this key produces no
/// byte", which covers both the unmapped codes and the modifiers themselves.
///
/// Written as one flat table rather than a match, because that is what it is: a keyboard layout is
/// data, and the failure mode of a 58-arm match is a row silently missing.
static UNSHIFTED: [u8; MAX_CODE as usize + 1] =
    *b"\0\x1b1234567890-=\x08\tqwertyuiop[]\r\0asdfghjkl;'`\0\\zxcvbnm,./\0*\0 ";

/// What each key sends with shift held. Same indexing as [`UNSHIFTED`].
static SHIFTED: [u8; MAX_CODE as usize + 1] =
    *b"\0\x1b!@#$%^&*()_+\x08\tQWERTYUIOP{}\r\0ASDFGHJKL:\"~\0|ZXCVBNM<>?\0*\0 ";

// The cursor and navigation clusters' evdev codes. Numerically far above [`MAX_CODE`] (the main
// block's own flat table stops at 57; these start at 102), so they get their own case in
// [`cluster`] rather than a much wider table padded with zeros in between.

/// `KEY_HOME`.
pub(crate) const KEY_HOME: u16 = 102;
/// `KEY_UP`.
pub(crate) const KEY_UP: u16 = 103;
/// `KEY_PAGEUP`.
pub(crate) const KEY_PAGEUP: u16 = 104;
/// `KEY_LEFT`.
pub(crate) const KEY_LEFT: u16 = 105;
/// `KEY_RIGHT`.
pub(crate) const KEY_RIGHT: u16 = 106;
/// `KEY_END`.
pub(crate) const KEY_END: u16 = 107;
/// `KEY_DOWN`.
pub(crate) const KEY_DOWN: u16 = 108;
/// `KEY_PAGEDOWN`.
pub(crate) const KEY_PAGEDOWN: u16 = 109;
/// `KEY_INSERT`.
pub(crate) const KEY_INSERT: u16 = 110;
/// `KEY_DELETE`.
pub(crate) const KEY_DELETE: u16 = 111;

// The clusters sit entirely above the main block's own table, checked at compile time so a future
// edit that moved `MAX_CODE` up past 102 fails the build rather than silently making `byte`'s
// table start answering for a cluster code (`cluster` is matched first in `Keyboard::event`, so
// `byte` would never be reached, but the table itself would then hold a wrong, unreachable entry,
// which is exactly the kind of fact this file's own tests exist to keep visible). `KEY_HOME` is the
// lowest of them.
const _: () = assert!(KEY_HOME > MAX_CODE);

/// Is this key a shift?
pub(crate) const fn is_shift(code: u16) -> bool {
    code == KEY_LEFTSHIFT || code == KEY_RIGHTSHIFT
}

/// **The byte evdev key `code` sends**, or `None` if this layout has nothing for it.
///
/// `None` rather than a placeholder byte: a key with no mapping must produce *nothing*, because a
/// terminal that inserted a substitute character for every unmapped function key would corrupt every
/// line the user typed while reaching for one.
///
/// The main block only: a cluster key is not in `UNSHIFTED`/`SHIFTED` (it sends a sequence, not one
/// byte) and is handled separately, by `cluster` and [`Keyboard::event`].
pub fn byte(code: u16, shift: bool) -> Option<u8> {
    let table = if shift { &SHIFTED } else { &UNSHIFTED };
    match table.get(code as usize) {
        Some(&0) | None => None,
        Some(&b) => Some(b),
    }
}

/// **The `(parameter, final byte)` of one cursor- or navigation-cluster key's `CSI` sequence**, or
/// `None` if `code` is not one of them: xterm's normal cursor mode, which `line_editor` reads.
///
/// | Key | Sends | With shift |
/// |---|---|---|
/// | up, down, right, left | `CSI A/B/C/D` | `CSI 1;2 A/B/C/D` |
/// | home, end | `CSI H`, `CSI F` | `CSI 1;2 H`, `CSI 1;2 F` |
/// | insert, delete | `CSI 2 ~`, `CSI 3 ~` | `CSI 2;2 ~`, `CSI 3;2 ~` |
/// | page up, page down | `CSI 5 ~`, `CSI 6 ~` | `CSI 5;2 ~`, `CSI 6;2 ~` |
const fn cluster(code: u16) -> Option<(u8, u8)> {
    match code {
        KEY_UP => Some((b'1', b'A')),
        KEY_DOWN => Some((b'1', b'B')),
        KEY_RIGHT => Some((b'1', b'C')),
        KEY_LEFT => Some((b'1', b'D')),
        KEY_HOME => Some((b'1', b'H')),
        KEY_END => Some((b'1', b'F')),
        KEY_INSERT => Some((b'2', b'~')),
        KEY_DELETE => Some((b'3', b'~')),
        KEY_PAGEUP => Some((b'5', b'~')),
        KEY_PAGEDOWN => Some((b'6', b'~')),
        _ => None,
    }
}

/// **What one key event sends**: at most six bytes, in a fixed-capacity buffer rather than a `Vec`,
/// because this crate is `no_std` and reaches no allocator. Six is the longest sequence this table
/// emits (`CSI 5;2~`, shift and page up); most keys send exactly one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bytes {
    buf: [u8; 6],
    len: u8,
}

impl Bytes {
    const fn one(b: u8) -> Bytes {
        Bytes {
            buf: [b, 0, 0, 0, 0, 0],
            len: 1,
        }
    }

    /// The `CSI` sequence for a cluster key: `ESC [`, the parameter when the final byte is `~` or a
    /// modifier is held, `;2` when shift is held, then the final byte.
    const fn csi(param: u8, final_byte: u8, shift: bool) -> Bytes {
        let mut buf = [0x1b, b'[', 0, 0, 0, 0];
        let mut len = 2;
        if final_byte == b'~' || shift {
            buf[len] = param;
            len += 1;
        }
        if shift {
            buf[len] = b';';
            buf[len + 1] = b'2';
            len += 2;
        }
        buf[len] = final_byte;
        Bytes {
            buf,
            len: len as u8 + 1,
        }
    }

    /// The bytes this key sent, in order.
    pub fn as_slice(&self) -> &[u8] {
        &self.buf[..self.len as usize]
    }
}

/// **A keyboard's state**: which shift keys are down, and nothing else.
///
/// One bit, and that is the whole reason this is a struct rather than a free function: shift is
/// *held*, so it arrives as its own press and release events and the driver has to remember it
/// between them. Everything else about a key is in the event.
#[derive(Debug, Clone, Copy, Default)]
pub struct Keyboard {
    left: bool,
    right: bool,
    /// Either control key held (milestone 242 (USB host and HID)): what makes `^C` reach the shell
    /// from a keyboard, since a terminal sends control characters, not chords.
    control: bool,
    right_control: bool,
}

impl Keyboard {
    /// Neither shift key held.
    pub const fn new() -> Keyboard {
        Keyboard {
            left: false,
            right: false,
            control: false,
            right_control: false,
        }
    }

    /// Is a shift key held?
    ///
    /// Name: provisional, flagged 2026-09-24 by the boolean-predicate pass
    /// (design/naming/functions-that-answer-yes-or-no-worklist.md). It does not yet follow the Rust predicate
    /// rule calef ratified 2026-09-24; recommended `is_shift_held`, because `is_shift` does not
    /// parse.
    pub const fn shift(&self) -> bool {
        self.left || self.right
    }

    /// **Feed one `virtio_input_event`**, and get back the bytes it produces, if any.
    ///
    /// `value` is 0 for a release, 1 for a press, and 2 for the device's own auto-repeat. **Repeats
    /// count as presses**, which is what makes holding a key type: a driver that only honoured
    /// `value == 1` would look correct in every test and be maddening to use. That includes the
    /// cluster keys: a held arrow repeats the same sequence, exactly as a real terminal's
    /// auto-repeat would.
    pub fn event(&mut self, kind: u16, code: u16, value: u32) -> Option<Bytes> {
        if kind != EV_KEY {
            return None;
        }
        if is_shift(code) {
            let down = value != 0;
            if code == KEY_LEFTSHIFT {
                self.left = down;
            } else {
                self.right = down;
            }
            return None;
        }
        if code == KEY_LEFTCTRL || code == KEY_RIGHTCTRL {
            if code == KEY_LEFTCTRL {
                self.control = value != 0;
            } else {
                self.right_control = value != 0;
            }
            return None;
        }
        if value == 0 {
            return None; // a release types nothing
        }
        if let Some((param, final_byte)) = cluster(code) {
            return Some(Bytes::csi(param, final_byte, self.shift()));
        }
        // Control folds `@` through `_` and their lower-case twins onto 0x00..0x1f, which is what a
        // terminal sends for a control chord: `^C` is 0x03 whether shift is held or not.
        let control = self.control || self.right_control;
        byte(code, self.shift()).map(|b| {
            Bytes::one(if control && (0x40..0x7f).contains(&b) {
                b & 0x1f
            } else {
                b
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    extern crate std;
    use std::vec::Vec;

    /// **Control and a letter is the control character** (milestone 242 (USB host and HID)): `^C`
    /// is 0x03, with or without shift, and releasing control types letters again.
    #[test]
    fn control_and_a_letter_is_a_control_character() {
        let mut k = Keyboard::new();
        assert_eq!(k.event(EV_KEY, KEY_LEFTCTRL, 1), None);
        assert_eq!(k.event(EV_KEY, 46, 1).unwrap().as_slice(), b"\x03"); // KEY_C
        assert_eq!(k.event(EV_KEY, KEY_LEFTSHIFT, 1), None);
        assert_eq!(k.event(EV_KEY, 46, 1).unwrap().as_slice(), b"\x03");
        assert_eq!(k.event(EV_KEY, KEY_LEFTCTRL, 0), None);
        assert_eq!(k.event(EV_KEY, 46, 1).unwrap().as_slice(), b"C");
        assert_eq!(k.event(EV_KEY, KEY_RIGHTCTRL, 1), None);
        assert_eq!(k.event(EV_KEY, 2, 1).unwrap().as_slice(), b"!"); // a digit is not folded
        // Releasing the right-hand control types letters again, as the left-hand one does.
        assert_eq!(k.event(EV_KEY, KEY_RIGHTCTRL, 0), None);
        assert_eq!(k.event(EV_KEY, 46, 1).unwrap().as_slice(), b"C");
    }

    /// The letters, the digits, and the symbols land where the keycaps say, shifted and unshifted.
    /// A table off by one row would put `s` on the `a` key and nothing else would notice.
    #[test]
    fn the_keycaps_say_what_the_table_says() {
        for (code, want, want_shift) in [
            (30u16, b'a', b'A'), // KEY_A: the first row's left-hand home key
            (16, b'q', b'Q'),    // KEY_Q
            (44, b'z', b'Z'),    // KEY_Z
            (2, b'1', b'!'),     // KEY_1
            (11, b'0', b')'),    // KEY_0
            (12, b'-', b'_'),    // KEY_MINUS
            (39, b';', b':'),    // KEY_SEMICOLON
            (53, b'/', b'?'),    // KEY_SLASH
            (57, b' ', b' '),    // KEY_SPACE
            (28, b'\r', b'\r'),  // KEY_ENTER
            (15, b'\t', b'\t'),  // KEY_TAB
            (14, 0x08, 0x08),    // KEY_BACKSPACE
            (1, 0x1b, 0x1b),     // KEY_ESC
        ] {
            assert_eq!(byte(code, false), Some(want), "code {code} unshifted");
            assert_eq!(byte(code, true), Some(want_shift), "code {code} shifted");
        }
    }

    /// **Enter sends CR, not LF.** The line discipline treats either as end-of-line, but a real
    /// terminal sends CR and the VT engine's `\r\n` handling is written for that; a keyboard that
    /// sent LF would move the cursor down a row without returning it, on a screen, visibly.
    #[test]
    fn enter_sends_carriage_return() {
        assert_eq!(byte(28, false), Some(b'\r'));
        assert_ne!(byte(28, false), Some(b'\n'));
    }

    /// Nothing has a mapping it should not: the modifiers type nothing, and a code past the table is
    /// `None` rather than an index panic or a stray byte.
    #[test]
    fn unmapped_keys_type_nothing() {
        assert_eq!(byte(KEY_LEFTSHIFT, false), None);
        assert_eq!(byte(KEY_RIGHTSHIFT, false), None);
        assert_eq!(
            byte(29, false),
            None,
            "left control types nothing on its own"
        );
        assert_eq!(byte(56, false), None, "left alt likewise");
        assert_eq!(byte(0, false), None, "KEY_RESERVED");
        assert_eq!(byte(MAX_CODE + 1, false), None, "past the table");
        assert_eq!(byte(59, false), None, "F1");
        assert_eq!(byte(u16::MAX, true), None, "far past the table");
    }

    /// No two keys send the same byte, so a wrong keycode is a wrong character rather than a
    /// coincidence. (`*` is the exception: the keypad asterisk and shift-8 are the same character on
    /// a real keyboard too, which is why it is named here instead of quietly allowed.)
    #[test]
    fn two_keys_never_send_the_same_byte() {
        for shift in [false, true] {
            let mut seen: Vec<(u16, u8)> = Vec::new();
            for code in 0..=MAX_CODE {
                let Some(b) = byte(code, shift) else { continue };
                if let Some((other, _)) = seen.iter().find(|(_, o)| *o == b) {
                    assert!(
                        b == b'*',
                        "codes {other} and {code} both send {:?}: the table is shifted",
                        b as char,
                    );
                }
                seen.push((code, b));
            }
            assert!(seen.len() > 45, "the table lost most of its keys");
        }
    }

    /// **Shift is held, so it is state.** Press shift, type, release shift, type: the same key sends
    /// two different bytes, and the shift key itself sends none.
    #[test]
    fn shift_is_remembered_between_events() {
        let mut kb = Keyboard::new();
        assert_eq!(kb.event(EV_KEY, 30, 1), Some(Bytes::one(b'a')));
        assert_eq!(kb.event(EV_KEY, KEY_LEFTSHIFT, 1), None);
        assert!(kb.shift());
        assert_eq!(kb.event(EV_KEY, 30, 1), Some(Bytes::one(b'A')));
        assert_eq!(kb.event(EV_KEY, KEY_LEFTSHIFT, 0), None);
        assert!(!kb.shift());
        assert_eq!(kb.event(EV_KEY, 30, 1), Some(Bytes::one(b'a')));

        // Two shifts, released independently: releasing one while the other is held stays shifted.
        kb.event(EV_KEY, KEY_LEFTSHIFT, 1);
        kb.event(EV_KEY, KEY_RIGHTSHIFT, 1);
        kb.event(EV_KEY, KEY_LEFTSHIFT, 0);
        assert_eq!(
            kb.event(EV_KEY, 30, 1),
            Some(Bytes::one(b'A')),
            "right shift is still down"
        );
        kb.event(EV_KEY, KEY_RIGHTSHIFT, 0);
        assert_eq!(kb.event(EV_KEY, 30, 1), Some(Bytes::one(b'a')));

        // Only the state dump can tell the hands apart: typing reads the OR of the two.
        kb.event(EV_KEY, KEY_LEFTSHIFT, 1);
        assert!(
            std::format!("{kb:?}").contains("left: true, right: false"),
            "the left shift key must be recorded as the left one",
        );
    }

    /// A **release types nothing**, an auto-repeat types the same as a press, and a non-key event is
    /// ignored. The release rule is the one that matters: without it every keystroke would arrive
    /// twice, which is the first bug an evdev driver has.
    #[test]
    fn releases_type_nothing_and_repeats_type_again() {
        let mut kb = Keyboard::new();
        assert_eq!(kb.event(EV_KEY, 30, 1), Some(Bytes::one(b'a')), "press");
        assert_eq!(kb.event(EV_KEY, 30, 0), None, "release");
        assert_eq!(
            kb.event(EV_KEY, 30, 2),
            Some(Bytes::one(b'a')),
            "auto-repeat"
        );
        assert_eq!(kb.event(EV_SYN, 0, 0), None, "a batch separator");
        assert_eq!(kb.event(7, 30, 1), None, "some other event type");
    }

    /// **Each arrow sends the standard `CSI` cursor-key sequence**, which is `crates/line_editor`'s
    /// own receiving vocabulary: pointed straight at the pair, so a wrong final byte here fails this
    /// test rather than surfacing three components away as a cursor that moves the wrong direction.
    #[test]
    fn arrow_keys_send_the_csi_sequence_line_editor_understands() {
        let mut kb = Keyboard::new();
        for (code, want) in [
            (KEY_UP, &b"\x1b[A"[..]),
            (KEY_DOWN, b"\x1b[B"),
            (KEY_RIGHT, b"\x1b[C"),
            (KEY_LEFT, b"\x1b[D"),
        ] {
            let got = kb
                .event(EV_KEY, code, 1)
                .unwrap_or_else(|| panic!("arrow code {code} produced nothing"));
            assert_eq!(got.as_slice(), want, "arrow code {code}");
            // A release of the same key types nothing, same as every other key.
            assert_eq!(
                kb.event(EV_KEY, code, 0),
                None,
                "arrow release, code {code}"
            );
            // Auto-repeat resends the same sequence, the same as a held letter key.
            assert_eq!(kb.event(EV_KEY, code, 2).unwrap().as_slice(), want);
        }
    }

    /// **The navigation cluster sends xterm's sequences, and shift adds xterm's modifier
    /// parameter.** Spelled out as bytes rather than built from [`cluster`]'s table, so a wrong row
    /// there is a failure here rather than two copies of the same mistake agreeing.
    #[test]
    fn the_navigation_cluster_sends_xterms_sequences_with_and_without_shift() {
        let mut kb = Keyboard::new();
        let cases: [(u16, &[u8], &[u8]); 10] = [
            (KEY_HOME, b"\x1b[H", b"\x1b[1;2H"),
            (KEY_END, b"\x1b[F", b"\x1b[1;2F"),
            (KEY_INSERT, b"\x1b[2~", b"\x1b[2;2~"),
            (KEY_DELETE, b"\x1b[3~", b"\x1b[3;2~"),
            (KEY_PAGEUP, b"\x1b[5~", b"\x1b[5;2~"),
            (KEY_PAGEDOWN, b"\x1b[6~", b"\x1b[6;2~"),
            (KEY_UP, b"\x1b[A", b"\x1b[1;2A"),
            (KEY_DOWN, b"\x1b[B", b"\x1b[1;2B"),
            (KEY_RIGHT, b"\x1b[C", b"\x1b[1;2C"),
            (KEY_LEFT, b"\x1b[D", b"\x1b[1;2D"),
        ];
        for (code, plain, shifted) in cases {
            assert_eq!(
                kb.event(EV_KEY, code, 1).unwrap().as_slice(),
                plain,
                "code {code}"
            );
            kb.event(EV_KEY, KEY_RIGHTSHIFT, 1);
            assert_eq!(
                kb.event(EV_KEY, code, 1).unwrap().as_slice(),
                shifted,
                "shift and code {code}"
            );
            kb.event(EV_KEY, KEY_RIGHTSHIFT, 0);
            assert_eq!(kb.event(EV_KEY, code, 0), None, "release, code {code}");
        }
    }

    /// **The cluster edits a line through the real line discipline**, which is the claim the
    /// table above makes and the one a person notices: home, end and delete do what their keycaps
    /// say at a prompt. Checked against `crates/line_editor` itself rather than against its
    /// documentation, the same way the VT engine's interoperability test is.
    #[test]
    fn home_end_and_delete_edit_a_line_through_the_line_discipline() {
        struct Discard;
        impl line_editor::Sink for Discard {
            fn put(&mut self, _: &[u8]) {}
        }
        let mut kb = Keyboard::new();
        let mut ld = line_editor::LineDisc::new();
        // "xy", Home, "w", End, "z", Left, Left, Delete, Enter: "wxyz" with the "y" deleted.
        let keys = [
            45u16, 21, KEY_HOME, 17, KEY_END, 44, KEY_LEFT, KEY_LEFT, KEY_DELETE, 28,
        ];
        let mut event = line_editor::Event::None;
        for code in keys {
            for &b in kb.event(EV_KEY, code, 1).unwrap().as_slice() {
                event = ld.feed(b, &mut Discard);
            }
        }
        assert_eq!(event, line_editor::Event::Line);
        assert_eq!(ld.line(), b"wxz");

        // Page up reaches the discipline as a sequence it swallows whole: a key the line has no
        // use for must not type its parameter and final byte into the line as `5~`.
        let mut ld = line_editor::LineDisc::new();
        for code in [30u16, KEY_PAGEUP, KEY_PAGEDOWN, 28] {
            for &b in kb.event(EV_KEY, code, 1).unwrap().as_slice() {
                event = ld.feed(b, &mut Discard);
            }
        }
        assert_eq!(event, line_editor::Event::Line);
        assert_eq!(ld.line(), b"a");
    }

    /// Arrow codes sit well above [`MAX_CODE`] (checked at compile time, below this module): this
    /// proves the runtime half of the same claim, that [`byte`] (the main block's own lookup) does
    /// not accidentally answer for them. If it ever did, [`Keyboard::event`]'s arrow case would be
    /// dead code nobody noticed.
    #[test]
    fn arrow_codes_are_outside_the_main_blocks_table() {
        for code in [
            KEY_UP,
            KEY_DOWN,
            KEY_LEFT,
            KEY_RIGHT,
            KEY_HOME,
            KEY_END,
            KEY_INSERT,
            KEY_DELETE,
            KEY_PAGEUP,
            KEY_PAGEDOWN,
        ] {
            assert_eq!(byte(code, false), None);
            assert_eq!(byte(code, true), None);
        }
    }

    /// A whole word typed the way the device sends it: press and release per key, shift for the
    /// capital. The byte stream is what a terminal would have received from a UART.
    #[test]
    fn typing_a_word_produces_the_bytes_a_terminal_would_have_seen() {
        let mut kb = Keyboard::new();
        let mut out = Vec::new();
        // Shift+h, e, l, l, o, Enter.
        for (code, value) in [
            (KEY_LEFTSHIFT, 1),
            (35, 1),
            (35, 0),
            (KEY_LEFTSHIFT, 0),
            (18, 1),
            (18, 0),
            (38, 1),
            (38, 0),
            (38, 1),
            (38, 0),
            (24, 1),
            (24, 0),
            (28, 1),
            (28, 0),
        ] {
            if let Some(bytes) = kb.event(EV_KEY, code, value) {
                out.extend_from_slice(bytes.as_slice());
            }
        }
        assert_eq!(out, b"Hello\r");
    }
}
