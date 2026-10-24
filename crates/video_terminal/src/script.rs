//! **What milestone 29 (a display terminal: framebuffer, virtio-gpu, and a foreign component)'s
//! tests put on the screen**, in one place so every party agrees.
//!
//! The kernel-side test sends these bytes and predicts the framebuffer by running the engine; the
//! host-side scanout check runs the same engine and grades what QEMU displays. Neither reads the
//! other's answer. See notes/video-terminal/palette-keymap-and-script.md, "The test script".

/// The columns of a terminal that owns the whole scanout: `graphics_contract::WIDTH / `[`CELL_W`].
///
/// A literal, because this crate does not depend on the framebuffer contract;
/// `components/src/display_terminal.rs` and the kernel wiring assert the two agree at compile time.
/// 80 by 24 at the 14x26 cell is exactly the 1120x624 scanout.
///
/// [`CELL_W`]: crate::CELL_W
pub const COLS: u32 = 80;
/// The rows of a terminal that owns the whole scanout. See [`COLS`].
pub const ROWS: u32 = 24;

/// **What the application prints.** Delivered as `OPERATION_WRITE`, the terminal contract's application
/// half (notes/terminal-contract.md), exactly as a program printing to a serial console would.
///
/// Chosen so the picture is hard to produce by accident: four rows, five renditions (a 24-bit
/// Solarized yellow that no indexed colour matches among them), a `\r\n` pair as
/// `line_editor::expand_output` writes it, a bold italic word and a curly underline in SGR 58's
/// red, and descenders. Each one's reason is in notes/video-terminal/palette-keymap-and-script.md.
pub const GREETING: &[u8] =
    b"\x1b[38;2;181;137;0mnife\x1b[0m\r\n\x1b[32mglyphs_ok\x1b[0m\r\nby a \x1b[4mvt\x1b[24m \x1b[1;3mbold italic\x1b[22;23m \x1b[4:3;58;5;1mcurly\x1b[0m\r\n\x1b[7mFOCUS\x1b[0m";

/// **What the user types.** Delivered as `OPERATION_BYTES`, the terminal contract's driver half, which is
/// the same framing the compositor uses to route a keystroke to the focused client (DECISIONS §33).
/// Echoed into the grid, so a keystroke that never arrived is a missing letter on the screen rather
/// than a silent nothing.
pub const TYPED: &[u8] = b"\r\n> hi";

/// **A wrong picture, for the negative control.** The greeting with one letter changed.
///
/// The host-side scanout check has to reject this, and that is the assertion that makes the whole
/// text proof mean something: a checker that only asked "is the screen not blank?" would pass every
/// run, including the ones where the terminal drew the wrong thing. One letter, because a wholly
/// different screen would be rejected by a much weaker check.
pub const GREETING_TYPO: &[u8] =
    b"\x1b[38;2;181;137;0mnife\x1b[0m\r\n\x1b[32mglyphs_0k\x1b[0m\r\nby a \x1b[4mvt\x1b[24m \x1b[1;3mbold italic\x1b[22;23m \x1b[4:3;58;5;1mcurly\x1b[0m\r\n\x1b[7mFOCUS\x1b[0m";

/// **The banner each terminal in the compositor test prints**, indexed by window.
///
/// Different text per window on purpose: it is what turns "the screen has text on it" into "each
/// window has *its own* text on it", so an `OPERATION_WRITE` delivered to the wrong terminal is a wrong
/// picture rather than a duplicate one.
pub const WINDOW_BANNER: [&[u8]; 2] = [b"term0", b"term1"];

/// **What is typed at each terminal in the compositor test** after focus moves to it.
///
/// The letters differ so that focus routing is checkable from the *picture*: an `A` appearing in
/// window 1, or a `B` in both, is a compositor that sent a keystroke to a client that should not
/// have received it. Since a client can only be sent input because it *holds* an input endpoint
/// (DECISIONS §33), this is the capability claim made visible in pixels.
pub const WINDOW_TYPED: [&[u8]; 2] = [b"\r\nA", b"\r\nB"];

/// **The key the host presses on the real keyboard device**, as QEMU's monitor `sendkey` names it.
///
/// The keyboard test's one genuinely host-driven input: nothing in the guest can press a key, so
/// `cargo xtask` sends this on the monitor beside the suite, the same connection the scanout check
/// already uses. Shared here so the side that presses and the side that asserts cannot disagree
/// about which key it was.
pub const HOST_KEY: &str = "a";

/// The byte [`HOST_KEY`] must arrive as, once the driver's evdev event has been through
/// [`crate::keymap`]. Unshifted, so this is the plain letter.
pub const HOST_KEY_BYTE: u8 = b'a';

/// **How many scroller lines the full-scanout script feeds after the greeting and the typing**
/// (the paint path, 2026-09-30).
///
/// Five rows are in use when the scroller starts, so 48 lines plus the 5-line tail below scroll a
/// 24-row grid 34 times. The driver side feeds them one `OPERATION_WRITE` per line, so every
/// present moves the surface one row; the oracle ignores write boundaries, so the tail tests
/// several scrolls in one present without a second oracle.
/// See notes/video-terminal/palette-keymap-and-script.md.
pub const SCROLL_LINES: usize = 48;

/// **How many more lines arrive in one write**, after the per-line ones: five scrolls inside a
/// single present, the shape a program printing a paragraph produces. See [`SCROLL_LINES`].
pub(crate) const SCROLL_TAIL_LINES: usize = 5;

/// One scroller line, `scroll NN\r\n` with `NN` zero-padded: written into `out`, returning its
/// length. The number is the line's own index, so a picture scrolled by the wrong number of rows
/// disagrees with the oracle at every cell that faces the wrong number, which is the failure this
/// content exists to catch.
pub(crate) fn scroll_line(i: usize, out: &mut [u8]) -> usize {
    const HEAD: &[u8] = b"scroll ";
    out[..HEAD.len()].copy_from_slice(HEAD);
    out[HEAD.len()] = b'0' + (i / 10 % 10) as u8;
    out[HEAD.len() + 1] = b'0' + (i % 10) as u8;
    out[HEAD.len() + 2..HEAD.len() + 4].copy_from_slice(b"\r\n");
    HEAD.len() + 4
}

/// **Hand the scroller to `write` in the writes a driver side sends**: [`SCROLL_LINES`] writes of
/// one line each, then one write carrying the [`SCROLL_TAIL_LINES`]-line tail. The byte stream is
/// exactly what [`feed_scroller`] feeds the oracle with `typo_line: None`.
///
/// One function rather than a loop at each call site because the call sites drifted: the tail
/// was added to the oracle and to one witness, and the firmware-screen witness kept writing only
/// the per-line half, so its oracle ended five scrolls ahead of its screen (the paint lane,
/// 2026-10-02). Name: provisional, the paint lane's.
pub fn write_scroller(mut write: impl FnMut(&[u8])) {
    for i in 0..SCROLL_LINES {
        let mut line = [0u8; 12];
        let n = scroll_line(i, &mut line);
        write(&line[..n]);
    }
    let mut tail = [0u8; 12 * SCROLL_TAIL_LINES];
    let mut used = 0;
    for i in 0..SCROLL_TAIL_LINES {
        used += scroll_line(SCROLL_LINES + i, &mut tail[used..]);
    }
    write(&tail[..used]);
}

/// Feed the whole scroller ([`SCROLL_LINES`] lines then [`SCROLL_TAIL_LINES`] more) into `vt`.
///
/// `typo_line` mangles one line's counter (`99` in place of its own number), for the negative
/// control: a scrolled picture that is one digit off must fail the comparison, or the comparison
/// proves nothing about scrolls specifically.
pub fn feed_scroller(vt: &mut crate::Vt, typo_line: Option<usize>) {
    for i in 0..SCROLL_LINES + SCROLL_TAIL_LINES {
        let mut line = [0u8; 12];
        let n = match typo_line {
            Some(t) if t == i => {
                const MANGLED: &[u8] = b"scroll 99\r\n";
                line[..MANGLED.len()].copy_from_slice(MANGLED);
                MANGLED.len()
            }
            _ => scroll_line(i, &mut line),
        };
        vt.feed(&line[..n]);
    }
}

/// **Feed the greeting, the typing and the scroller into `vt`**: the full-scanout wiring's final
/// picture. See [`full_screen`] (which this builds on) for why it takes `&mut Vt` rather than
/// returning one, and [`SCROLL_LINES`] for why the scroller is here at all.
pub fn scrolled_full_screen(vt: &mut crate::Vt, typo_line: Option<usize>) {
    full_screen(vt);
    feed_scroller(vt, typo_line);
}

/// **How many lines each compositor window prints past its own height** (the paint path,
/// 2026-09-30): seven, which scrolls each 2-row window at least once a line. See
/// [`SCROLL_LINES`], the full-scanout twin, for why windows need their own scroller at all: until
/// this, the [`MODE_WINDOW`](crate::status::MODE_WINDOW) present path (a scroll publishes
/// whole-window damage to the compositor) had never run once.
pub const WINDOW_SCROLL_LINES: usize = 7;

/// One window's scroller line, `wN-MM\r\n` with `MM` zero-padded, written into `out`. The window's
/// own digit leads, so two windows scrolling the same count still disagree pixel for pixel and
/// the focus test's routing check keeps its teeth in the scrolled picture.
pub fn window_scroll_line(window: usize, i: usize, out: &mut [u8]) -> usize {
    out[0] = b'w';
    out[1] = b'0' + window as u8;
    out[2] = b'-';
    out[3] = b'0' + (i / 10 % 10) as u8;
    out[4] = b'0' + (i % 10) as u8;
    out[5..7].copy_from_slice(b"\r\n");
    7
}

/// **Feed the greeting, then the typing, into `vt`**: what a full-scanout wiring shows after the
/// whole script. The caller constructs `vt` (typically `Vt::new(COLS, ROWS)`, compile-time
/// constants, so a `static` costs nothing at runtime) rather than this function returning one.
///
/// **Takes `&mut Vt`, not `-> Vt`, since milestone 142's grid growth.** A `Vt` is now hundreds of
/// KiB (`Vt`'s own doc comment), so a function that built one and returned it by value would need
/// that whole value to exist somewhere at the call site; a kernel test calling this on a 24 KiB
/// thread stack is exactly the caller this signature protects (`script/stack-frame-check` is the
/// gate that would have caught the old shape once the grid grew, and this signature is why it does
/// not have to).
pub fn full_screen(vt: &mut crate::Vt) {
    vt.feed(GREETING);
    vt.feed(TYPED);
}

/// **Feed window `i`'s banner, its typed text and its scroller into `vt`**: what its terminal
/// shows in the compositor test. `vt` must already be at the right geometry (`Vt::reset_to(cols,
/// rows)`, since a window's size is not known until runtime, unlike [`full_screen`]'s fixed one);
/// this only feeds the script, for [`full_screen`]'s own reason (a `Vt`-by-value return is no
/// longer a cheap thing to hand back).
///
/// **The scroller is part of the window script since the paint path (2026-09-30)**: the compositor
/// test drives each real terminal with the same lines, one `OPERATION_WRITE` each, so the window-mode
/// present path runs its scroll fast path under a pixel-exact witness.
pub fn window(vt: &mut crate::Vt, i: usize) {
    vt.feed(WINDOW_BANNER[i]);
    vt.feed(WINDOW_TYPED[i]);
    let mut line = [0u8; 8];
    for n in 0..WINDOW_SCROLL_LINES {
        let len = window_scroll_line(i, n, &mut line);
        vt.feed(&line[..len]);
    }
}

#[cfg(test)]
mod scroll_tests {
    extern crate std;
    use std::format;

    use super::*;

    /// Every line the scroller feeds is the same shape, and the counter is decimal, so a
    /// witness can predict the picture without calling this module.
    #[test]
    fn scroll_line_shapes_every_index() {
        let mut line = [0u8; 12];
        for i in [0usize, 1, 9, 10, 11, 47, 52] {
            let n = scroll_line(i, &mut line);
            assert_eq!(n, "scroll ".len() + 4);
            assert_eq!(&line[..7], b"scroll ");
            assert_eq!(&line[7..9], format!("{:02}", i).as_bytes());
            assert_eq!(&line[9..11], b"\r\n");
        }
    }

    fn screen(vt: &crate::Vt) -> std::vec::Vec<std::vec::Vec<u8>> {
        (0..vt.rows())
            .map(|r| {
                let mut b = [0u8; 256];
                let n = vt.row_bytes(r, &mut b);
                b[..n].to_vec()
            })
            .collect()
    }

    /// The writes a driver sends are one per line and then one tail, and together they are the
    /// bytes the oracle is fed, so the two cannot drift apart again.
    #[test]
    fn the_scroller_is_written_one_line_at_a_time_and_then_a_tail() {
        let mut writes: std::vec::Vec<std::vec::Vec<u8>> = std::vec::Vec::new();
        write_scroller(|b| writes.push(b.to_vec()));
        assert_eq!(writes.len(), SCROLL_LINES + 1);
        assert_eq!(writes[0], b"scroll 00\r\n");
        assert_eq!(writes[SCROLL_LINES - 1], b"scroll 47\r\n");
        let mut tail = std::vec::Vec::new();
        for i in 0..SCROLL_TAIL_LINES {
            tail.extend_from_slice(format!("scroll {:02}\r\n", SCROLL_LINES + i).as_bytes());
        }
        assert_eq!(writes[SCROLL_LINES], tail);
        let mut wire = crate::Vt::new(40, 10);
        for w in &writes {
            wire.feed(w);
        }
        let mut oracle = crate::Vt::new(40, 10);
        feed_scroller(&mut oracle, None);
        assert_eq!(screen(&wire), screen(&oracle));
    }

    /// The oracle feeds every line, and the negative control mangles exactly the one it is told to.
    #[test]
    fn the_oracle_feeds_every_line_and_mangles_only_the_chosen_one() {
        let total = SCROLL_LINES + SCROLL_TAIL_LINES;
        let mut by_hand = crate::Vt::new(40, 10);
        for i in 0..total {
            by_hand.feed(format!("scroll {i:02}\r\n").as_bytes());
        }
        let mut oracle = crate::Vt::new(40, 10);
        feed_scroller(&mut oracle, None);
        assert_eq!(screen(&oracle), screen(&by_hand));

        let mut typo = crate::Vt::new(40, 10);
        feed_scroller(&mut typo, Some(total - 2));
        let mut by_hand = crate::Vt::new(40, 10);
        for i in 0..total {
            let line = if i == total - 2 {
                "scroll 99\r\n".into()
            } else {
                format!("scroll {i:02}\r\n")
            };
            by_hand.feed(line.as_bytes());
        }
        assert_eq!(screen(&typo), screen(&by_hand));
        assert_ne!(screen(&typo), screen(&oracle));
        // The full-screen picture is the greeting and the typing, then the scroller.
        let mut whole = crate::Vt::new(40, 10);
        scrolled_full_screen(&mut whole, None);
        let mut parts = crate::Vt::new(40, 10);
        full_screen(&mut parts);
        feed_scroller(&mut parts, None);
        assert_eq!(screen(&whole), screen(&parts));
    }

    #[test]
    fn a_window_line_names_its_window_and_its_count() {
        let mut out = [0u8; 8];
        assert_eq!(window_scroll_line(3, 47, &mut out), 7);
        assert_eq!(&out[..7], b"w3-47\r\n");
        assert_eq!(window_scroll_line(2, 5, &mut out), 7);
        assert_eq!(&out[..7], b"w2-05\r\n");
        assert_eq!(window_scroll_line(0, 123, &mut out), 7);
        assert_eq!(&out[..7], b"w0-23\r\n");
    }

    /// The typo control and the window scroller execute their whole paths and leave the
    /// terminal holding a nonempty, scrolled picture: every line of both functions runs.
    #[test]
    fn feed_and_window_scroller_execute_their_paths() {
        let mut vt = crate::Vt::new(80, 24);
        feed_scroller(&mut vt, Some(SCROLL_LINES + 1));
        assert!(vt.rows() > 0);
        let mut b = [0u8; 256];
        assert!(vt.row_bytes(0, &mut b) > 0);
        scrolled_full_screen(&mut vt, Some(WINDOW_SCROLL_LINES));
        assert!(vt.row_bytes(vt.rows() - 1, &mut b) > 0);
        // Determinism, the property the scanout check depends on: same inputs, same first row.
        let mut other = crate::Vt::new(80, 24);
        feed_scroller(&mut other, None);
        let mut c = [0u8; 256];
        let n1 = vt.row_bytes(0, &mut b);
        let n2 = other.row_bytes(0, &mut c);
        assert_eq!(n1, n2);
    }
}
