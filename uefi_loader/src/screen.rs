//! **The last word anything says before the kernel exists**, the early-boot half of
//! milestone 243 (a machine with no serial port).
//!
//! From `ExitBootServices` to the kernel's own `console::attach_screen` a machine with no serial
//! port has no console, and everything in between (the mode-switch trampoline, `boot.s`'s 32-bit
//! half, the page tables, the long-mode jump) runs with no IDT, so a fault is a silent reset. That
//! window cannot be narrated, but it can be **bounded**: the screen is cleared and given one
//! sentence, so a person at a monitor can tell "the kernel never reached its first statement" from
//! "the kernel armed its console and died after", which would otherwise be one black rectangle.

//! # Why it lives here rather than in `main.rs`
//!
//! Because it can be proved on the host. The painting is `screen_console`'s, the same crate and the
//! same font the kernel uses, so the loader's last word and the kernel's first are literally the
//! same letters; what this module adds is the text and the one call, and the test below reads it
//! back out of a RAM framebuffer rather than trusting that it was written.

//! # EXAMPLES
//!
//! ```
//! use machine_discovery::framebuffer::{Framebuffer, PixelOrder};
//!
//! // A screen wide enough for the banner's longest line and two cells tall.
//! let screen = Framebuffer { base: 0, width: 560, height: 16, stride: 2240, order: PixelOrder::Bgrx };
//! let mut pixels = vec![0u8; 2240 * 16];
//! assert!(uefi_loader::screen::paint_handoff(screen, &mut pixels));
//!
//! // There is ink on it now, which a black screen at the same moment would not have.
//! assert!(pixels.chunks(4).any(|p| u32::from_le_bytes(p.try_into().unwrap())
//!     == screen_console::ScreenConsole::FOREGROUND));
//! ```

//! # BUGS
//!
//! - **It is unobservable in a healthy boot.** The kernel clears the screen within milliseconds of
//!   this being painted, so no gate can assert it on a working machine. What proves it is the
//!   halted-loader experiment in `notes/serial-less-output.md`.
//! - **Only ASCII**, like everything else `screen_console` draws.
//! - **It puts a 36,000-byte `screen_console::Cells` on the firmware's stack** for two lines that
//!   never scroll. UEFI promises an x64 application at least 128 KiB of stack (recalled from the
//!   specification, not re-read); a `static` would avoid it at the price of `unsafe` shared state.

use machine_discovery::framebuffer::Framebuffer;
use screen_console::{Cells, PixelSink, ScreenConsole};

/// **What the screen says while the kernel is coming up, and what it still says if it does not.**
///
/// Written to be read by a person standing at a monitor with no other instrument, which is the
/// whole situation this milestone exists for, so it says what to conclude rather than only what
/// happened.
pub const HANDOFF: &str = concat!(
    "nife loader: firmware released, entering the kernel.\n",
    "If this line is still here, the kernel stopped before its console came up.\n",
);

/// Clear `pixels` and write [`HANDOFF`] into it. `false` when the geometry cannot hold a cell.
///
/// `pixels` must be the framebuffer `screen` describes, `screen.span()` bytes of it.
#[must_use]
pub fn paint_handoff(screen: Framebuffer, pixels: &mut [u8]) -> bool {
    let Some(mut console) = ScreenConsole::new(screen) else {
        return false;
    };
    if pixels.len() < console.span() {
        return false;
    }
    // The grid a scroll would redraw from. Two lines never scroll, so this is pure overhead here,
    // and it is 36,000 bytes of the firmware's stack; see this module's BUGS.
    let mut cells = Cells::new();
    let mut sink = PixelSink::new(pixels);
    console.clear(&mut cells, &mut sink);
    console.write(&mut cells, &mut sink, HANDOFF);
    true
}

#[cfg(test)]
mod tests {
    use machine_discovery::framebuffer::PixelOrder;

    use super::*;

    fn screen(width: u32, height: u32) -> Framebuffer {
        Framebuffer {
            base: 0,
            width,
            height,
            stride: width * 4,
            order: PixelOrder::Bgrx,
        }
    }

    /// The claim the halted-loader experiment makes on a real machine, made here in milliseconds:
    /// ink appears, and it appears only in the rows the two lines occupy.
    #[test]
    fn the_banner_paints_two_rows_of_cells_and_no_more() {
        let s = screen(560, 80);
        let mut pixels = vec![0u8; (s.stride * s.height) as usize];
        assert!(paint_handoff(s, &mut pixels));

        let row_has_ink = |y: u32| {
            (0..s.width).any(|x| {
                let at = (y * s.stride + x * 4) as usize;
                u32::from_le_bytes(pixels[at..at + 4].try_into().unwrap())
                    == ScreenConsole::FOREGROUND
            })
        };
        assert!(
            (0..16).any(row_has_ink),
            "the two lines are in the first two cell rows"
        );
        assert!(
            !(16..s.height).any(row_has_ink),
            "nothing below them, which is what makes a stuck banner readable as one",
        );
    }

    /// A screen too small for a character cell is refused rather than written past.
    #[test]
    fn a_screen_with_no_room_for_a_cell_is_refused() {
        let s = screen(4, 4);
        let mut pixels = vec![0u8; 64];
        assert!(!paint_handoff(s, &mut pixels));
    }

    /// The caller owns the slice, so a caller that sized it wrongly must be refused rather than
    /// trusted: this is the one thing between a stray `stride` and a write past a real aperture.
    #[test]
    fn a_buffer_shorter_than_the_screen_is_refused() {
        let s = screen(560, 80);
        let mut pixels = vec![0u8; 16];
        assert!(!paint_handoff(s, &mut pixels));
    }
}
