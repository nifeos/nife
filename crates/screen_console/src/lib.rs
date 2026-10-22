//! **Text on a screen, for a machine with no serial port.** Given a linear
//! framebuffer the previous boot stage found ([`machine_discovery::framebuffer`]) and the tree's 7x8
//! font ([`bitmap_font`]), put bytes on the screen. See notes/serial-less-output.md.
//!
//! It is not `video_terminal`, which is a several-hundred-kilobyte value with an escape-sequence
//! parser: too big for a kernel static and the wrong thing to put in the TCB. What it reports is
//! often why the machine is broken, so it holds the least state that could work: a cursor, a
//! geometry and a scale, plus a caller-kept [`Cells`] that a scroll redraws from. No colour
//! changes, escapes, cursor, scrollback or reflow, so it cannot be put in a surprising state. The
//! font is shared, so early-boot letters match the graphical terminal's.
//!
//! Each font pixel is drawn as a square block at the largest integer scale that leaves
//! [`MIN_COLUMNS`] columns (120, calef's choice on 2026-10-04): 1920x1080 gets two, anything under
//! 1680 wide gets one. Scrolling redraws only the cells that changed, through a [`PixelSink`] that
//! cannot be read, because reading the aperture back is very slow on real silicon.
//!
//! # BUGS
//!
//! - A screen taller than [`Cells::CAPACITY`] holds is drawn on its top rows only: the grid is a
//!   fixed 36,000 bytes because it exists before any allocator does.
//! - A width or height that is not a whole number of cells, or of the scale, leaves an unpainted
//!   sliver at the right and bottom edges ([`Aperture`] covers whole surface pixels).
//! - Only 32-bit pixels: [`machine_discovery::framebuffer::PixelOrder`] has UEFI's two orders.
//! - On xenon, the one real machine that has run it, the screen shows a grid instead of text; the
//!   finding and the experiment that would measure it are in
//!   `notes/serial-less-output/the-xenon-grid.md`.
//! - Nothing here is proved on real silicon, only on the host and under OVMF.
//!
//! Name: ratified 2026-09-13 (calef), half of a pair with `serial_console`: two consoles named for
//! where the text comes out. Refused `framebuffer_console` (the thing it writes into),
//! `pixel_console` (the unit, not the surface), `early_console` (when, not what) and
//! `video_terminal` (the terminal emulator this deliberately is not; `script/names` reports it as
//! refused here and live there, which is true).

#![no_std]

use machine_discovery::framebuffer::{Framebuffer, PixelOrder};

/// **The fewest columns a console is scaled down to**: 120. A screen is drawn at the largest
/// integer scale that still leaves this many columns (see [`ScreenConsole::scale_for`]). A choice,
/// not a derivation, and this is the one place it lives: calef's starting choice on 2026-10-04
/// (UTC), "80 is a good place to start", then 120 the same evening ("80 is too big") after seeing
/// 80 on xenon's monitor at 3x. 80 was the Kaypro II's width, whose character cell §100 (the
/// terminal font) took the font from. Introduced that day on the xenon bench of milestone 261 (the
/// NVMe driver leaves the kernel), when the 1:1 font on a 1920-wide monitor could not be read or
/// photographed.
pub const MIN_COLUMNS: u32 = 120;

/// **Where a surface lands on a firmware screen**: the arithmetic a driver needs to copy a
/// rectangle of pixels it was handed onto a screen the firmware set up (the shell on the firmware
/// screen, rung 1b of milestone 198; `components/src/framebuffer_driver.rs` is its one caller).
///
/// It lives here rather than in the driver because a stride bug is only catchable on the host: this
/// is a pure function of five numbers. It holds the surface's size clipped to the screen's, placed
/// top-left, the screen's stride and byte order, and **the scale every surface pixel is drawn at**:
/// [`ScreenConsole::scale_for_cell`] of the screen's width for the surface's cell, the kernel
/// console's rule, so the shell after the handover has at most the boot tour's columns. Clipping is in surface pixels against the
/// screen divided by the scale; a screen not a whole multiple of its scale leaves fewer than
/// `scale` pixels uncovered at its right and bottom edges (see BUGS).
///
/// Name: provisional (the shell on the firmware screen's lane). "Aperture" is the tree's word for
/// the firmware's framebuffer as a device window (`kernel/src/arch/x86_64/machine.rs`).
///
/// # Examples
///
/// ```
/// use machine_discovery::framebuffer::{Framebuffer, PixelOrder};
/// use screen_console::Aperture;
///
/// // A 4x2 screen with four bytes of padding per row, and a 3x3 surface: the surface is clipped
/// // to three columns and two rows. A screen this narrow draws at scale one.
/// let screen = Framebuffer { base: 0, width: 4, height: 2, stride: 20, order: PixelOrder::Rgbx };
/// let aperture = Aperture::new(&screen, 7, 3, 3).expect("a screen and a surface both bigger than 0");
/// assert_eq!(aperture.size(), (3, 2));
///
/// // Copy the whole visible part of a surface whose every pixel is pure red, 0x00ff0000.
/// let mut pixels = [0u8; 40];
/// assert!(aperture.copy(0, 0, 3, 2, |_, _| 0x00ff_0000, |at, word| {
///     pixels[at..at + 4].copy_from_slice(&word.to_le_bytes());
/// }));
/// // An rgbx screen stores red in its first byte, and the fourth column was never touched.
/// assert_eq!(&pixels[0..4], &[0xff, 0, 0, 0]);
/// assert_eq!(&pixels[12..16], &[0, 0, 0, 0]);
///
/// // A rectangle that leaves the visible part is refused rather than clipped.
/// assert!(!aperture.copy(0, 0, 4, 2, |_, _| 0, |_, _| {}));
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Aperture {
    /// The covered part, in **surface** pixels.
    width: u32,
    height: u32,
    stride: u32,
    order: PixelOrder,
    /// Screen pixels per surface pixel, in each direction. At least one.
    scale: u32,
}

impl Aperture {
    /// The part of `screen` a `surface_width` by `surface_height` surface covers, drawn at
    /// [`ScreenConsole::scale_for_cell`]'s scale for the surface's `cell_w`-pixel cells, or `None`
    /// when the screen's own geometry does not close ([`Framebuffer::span`]) or either size is zero.
    #[must_use]
    pub fn new(
        screen: &Framebuffer,
        cell_w: u32,
        surface_width: u32,
        surface_height: u32,
    ) -> Option<Self> {
        screen.span()?;
        let scale = ScreenConsole::scale_for_cell(screen.width, cell_w);
        Self::checked(
            (screen.width / scale).min(surface_width),
            (screen.height / scale).min(surface_height),
            screen.stride,
            screen.order,
            scale,
        )
    }

    /// The one validation both constructors share: a non-empty rectangle whose every scaled row
    /// fits in the stride, and whose last byte is addressable.
    fn checked(
        width: u32,
        height: u32,
        stride: u32,
        order: PixelOrder,
        scale: u32,
    ) -> Option<Self> {
        let screen_width = (width as u64).checked_mul(scale as u64)?;
        if width == 0 || height == 0 || scale == 0 || (stride as u64) < screen_width * 4 {
            return None;
        }
        let aperture = Self {
            width,
            height,
            stride,
            order,
            scale,
        };
        aperture.checked_span()?;
        Some(aperture)
    }

    /// The covered part, in **surface** pixels: `(width, height)`. This is what the driver answers
    /// a client's `INFO` with, so the client lays its grid out over what can actually be seen, and
    /// it is the surface's size too: the screen terminal's surface is allocated at exactly this.
    #[must_use]
    pub const fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// How many bytes from pixel (0, 0) a copy can reach: the last covered row's last pixel, plus
    /// one. **This, and not the screen's whole span, is what the driver needs mapped**, so a driver
    /// is never handed the rows of the screen it will never paint.
    #[must_use]
    pub fn span(&self) -> usize {
        self.checked_span().unwrap_or(0)
    }

    fn checked_span(&self) -> Option<usize> {
        let rows = (self.height as u64).checked_mul(self.scale as u64)?;
        let bytes = (self.stride as u64)
            .checked_mul(rows - 1)?
            .checked_add(self.width as u64 * self.scale as u64 * 4)?;
        usize::try_from(bytes).ok()
    }

    /// Screen pixels per surface pixel. See [`ScreenConsole::scale_for`].
    #[must_use]
    pub const fn scale(&self) -> u32 {
        self.scale
    }

    /// **Two words that carry this across a spawn**, for a driver that is told its geometry in
    /// registers because it holds no capability that could describe it. [`Self::from_words`] is
    /// the other half, and the pair lives here so the kernel that packs and the driver that
    /// unpacks are one definition rather than two that could drift (AGENTS.md rule 7).
    #[must_use]
    pub const fn to_words(&self) -> (u64, u64) {
        let order = match self.order {
            PixelOrder::Bgrx => 0,
            PixelOrder::Rgbx => 1,
        };
        (
            self.width as u64 | (self.height as u64) << 32,
            self.stride as u64 | order << 32 | (self.scale as u64) << 40,
        )
    }

    /// The inverse of [`Self::to_words`], validated the same way [`Self::new`] is, because the
    /// words crossed a process boundary and the driver should not paint on the strength of a
    /// geometry it cannot check.
    #[must_use]
    pub fn from_words(size: u64, layout: u64) -> Option<Self> {
        let order = match (layout >> 32) & 0xff {
            0 => PixelOrder::Bgrx,
            1 => PixelOrder::Rgbx,
            _ => return None,
        };
        if layout >> 48 != 0 {
            return None;
        }
        let scale = ((layout >> 40) & 0xff) as u32;
        Self::checked(
            size as u32,
            (size >> 32) as u32,
            layout as u32,
            order,
            scale,
        )
    }

    /// **Copy one rectangle of a surface onto the screen.** `read(x, y)` is the surface's pixel in
    /// the tree's usual `0x00RRGGBB` spelling; `write(offset, word)` stores `word` at `offset`
    /// bytes from the screen's pixel (0, 0), already in the screen's byte order.
    ///
    /// Returns `false`, having written nothing, when the rectangle is empty or leaves the covered
    /// part: **refused rather than clamped**, the same stance the framebuffer contract takes
    /// (`graphics_contract::rect_in_surface`), because a clamp would absorb a client's coordinate
    /// bug silently.
    pub fn copy(
        &self,
        x: u32,
        y: u32,
        w: u32,
        h: u32,
        read: impl Fn(u32, u32) -> u32,
        mut write: impl FnMut(usize, u32),
    ) -> bool {
        if !self.fits(x, y, w, h) {
            return false;
        }
        let k = self.scale;
        for row in y * k..(y + h) * k {
            let line = row as usize * self.stride as usize;
            for col in x * k..(x + w) * k {
                write(
                    line + col as usize * 4,
                    self.order.store(read(col / k, row / k)),
                );
            }
        }
        true
    }

    /// Whether a surface rectangle is non-empty and inside the covered part.
    fn fits(&self, x: u32, y: u32, w: u32, h: u32) -> bool {
        let fits = |start: u32, len: u32, limit: u32| {
            len > 0 && start.checked_add(len).is_some_and(|end| end <= limit)
        };
        fits(x, w, self.width) && fits(y, h, self.height)
    }

    /// **[`Self::copy`] with qword stores where the path permits them** (the paint lane,
    /// 2026-09-30). `write64(offset, pair)` receives two already-staged pixels in one `u64` and is
    /// called only at offsets where `(misalign + offset) % 8 == 0`; `write32` receives the head and
    /// tail pixels a row's alignment leaves over, exactly as [`Self::copy`] would. Every other
    /// word, including the byte order and the refusal stance, is [`Self::copy`]'s own.
    ///
    /// **Why the store width is the point.** The aperture a driver copies into is mapped
    /// uncacheable (x86: no PAT programming yet), and under QEMU's TCG store-heavy guest code runs
    /// ~3.8x slower per instruction than arithmetic
    /// (`notes/benchmarks/icount-tick-scales.md`), which made the one-word-per-pixel copy most of
    /// the `x86_64` swish leg's 321 s. Pairing the pixels halves the store count and nothing else
    /// changes. A `u64` is also the widest store this tree's `x86_64` target can legalise: the
    /// kernel and initial ramdisk build for `x86_64-unknown-none`, whose spec is
    /// `-mmx,-sse,+soft-float`, so there is no vector store to reach for (`xtask/src/archive.rs`
    /// records the target).
    ///
    /// `misalign` is the aperture's own base misalignment in bytes: the offset of pixel (0, 0)
    /// from an 8-aligned address, which only the driver that holds the mapping can know. With it,
    /// every geometry still takes its widest path: a padded or odd stride merely earns head and
    /// tail words on some rows.
    ///
    /// **The pair is byte-identical to two word stores on a little-endian machine**, which all
    /// three of this tree's targets are, and the host test that proves `copy_wide` equals `copy`
    /// pixel for pixel runs on one. `lo` is the earlier pixel.
    #[allow(clippy::too_many_arguments)] // the argument list is the copy contract: a rectangle,
    // the base's misalignment, a read, and the two store widths the caller's hardware offers
    pub fn copy_wide(
        &self,
        x: u32,
        y: u32,
        w: u32,
        h: u32,
        misalign: u32,
        read: impl Fn(u32, u32) -> u32,
        write64: impl FnMut(usize, u64),
        write32: impl FnMut(usize, u32),
    ) -> bool {
        if !self.fits(x, y, w, h) {
            return false;
        }
        // At scale one the screen rectangle is the surface rectangle and the read is the caller's
        // own, so the path the TCG-bound swish leg pays is the path it always paid: no division
        // per pixel. Above one, the same loop runs over the scaled rectangle and every read finds
        // the surface pixel under it.
        let k = self.scale;
        if k == 1 {
            self.copy_wide_screen(x, y, w, h, misalign, read, write64, write32);
        } else {
            self.copy_wide_screen(
                x * k,
                y * k,
                w * k,
                h * k,
                misalign,
                |col, row| read(col / k, row / k),
                write64,
                write32,
            );
        }
        true
    }

    /// [`Self::copy_wide`] over a rectangle already in **screen** pixels, which `copy_wide` has
    /// checked; `read` is asked for screen coordinates.
    #[allow(clippy::too_many_arguments)] // `copy_wide`'s own contract, one level down
    fn copy_wide_screen(
        &self,
        x: u32,
        y: u32,
        w: u32,
        h: u32,
        misalign: u32,
        read: impl Fn(u32, u32) -> u32,
        mut write64: impl FnMut(usize, u64),
        mut write32: impl FnMut(usize, u32),
    ) {
        for row in y..y + h {
            let line = row as usize * self.stride as usize;
            // The run's byte extent. A qword has two homes to satisfy at once: it must cover
            // exactly two pixels (`at` a multiple of 4 from `line`), and the store must be
            // 8-aligned where the pixels live (`(misalign + at) % 8 == 0`). Both are homes for
            // `at = line + 4k` exactly when `(misalign + line) % 4 == 0`, and then k's parity is
            // forced; a stride that is not itself 4-aligned (no framebuffer this tree has booted
            // reports one, but `checked` does not refuse it) has no such `at`, and that row takes
            // word stores alone.
            let run = line + x as usize * 4..line + (x + w) as usize * 4;
            let mis = misalign as usize;
            let qw = if (mis + line).is_multiple_of(4) && run.len() >= 8 {
                run.start
                    + if (mis + run.start).is_multiple_of(8) {
                        0
                    } else {
                        4
                    }
            } else {
                run.end
            };
            // Head words up to the first aligned pair (at most one: `qw` is 0 or 4 past the
            // start, or the whole run when the row has no aligned pair).
            for at in (run.start..qw.min(run.end)).step_by(4) {
                let col = (at - line) as u32 / 4;
                write32(at, self.order.store(read(col, row)));
            }
            // Staged pairs: two reads from cacheable RAM, one qword store into the aperture.
            let pairs = run.end.saturating_sub(qw) / 8;
            for p in 0..pairs {
                let at = qw + p * 8;
                let col = (at - line) as u32 / 4;
                let lo = self.order.store(read(col, row)) as u64;
                let hi = self.order.store(read(col + 1, row)) as u64;
                write64(at, lo | hi << 32);
            }
            // Tail words after the last whole pair.
            for at in (qw + pairs * 8..run.end).step_by(4) {
                let col = (at - line) as u32 / 4;
                write32(at, self.order.store(read(col, row)));
            }
        }
    }
}

/// **The framebuffer, as something this crate can only write.**
///
/// A framebuffer aperture on real hardware is mapped uncacheable or write-combining, and reading
/// either is slow by orders of magnitude: a read has to cross to the device and back, one word at a
/// time, where a write is posted and forgotten. Until 2026-10-04 the scroll moved the picture up
/// with `copy_within` over the aperture, reading most of 8 MB back for every new line on xenon,
/// whose console calef saw redraw in sections. **So the console's only handle on the screen is this
/// type, and it has no way to read.** The slice is private to this module and the one method is a
/// store, so a painter that reads the screen back does not compile (rung 1: the wrong state is
/// unrepresentable, and no test has to remember to look for it).
///
/// ```compile_fail
/// let mut bytes = [0u8; 8];
/// let sink = screen_console::PixelSink::new(&mut bytes);
/// let _ = sink.bytes[0]; // private: there is no read path
/// ```
///
/// Name: provisional (the console-scroll lane, 2026-10-04). "Sink" for the one direction data
/// flows; `Framebuffer` is taken by the description this paints into.
pub struct PixelSink<'a> {
    bytes: &'a mut [u8],
    /// How many pixels have been stored, for the tests that measure what a scroll costs.
    #[cfg(test)]
    stores: usize,
}

impl<'a> PixelSink<'a> {
    /// Wrap the framebuffer's bytes. In a host test that is ordinary memory; in the kernel and the
    /// loader it is the aperture.
    #[must_use]
    pub const fn new(bytes: &'a mut [u8]) -> Self {
        Self {
            bytes,
            #[cfg(test)]
            stores: 0,
        }
    }

    /// A run of whole pixels, already in the screen's byte order, at `at` bytes from pixel (0, 0).
    /// `false` when the run does not fit, having stored the whole pixels of it that do: a short
    /// slice truncates the picture rather than panicking (see [`ScreenConsole::write`]).
    ///
    /// **A run rather than a pixel, for the emulator's sake** (2026-10-04). One bounds check and
    /// one `copy_from_slice` per row of a glyph, instead of a check, a font lookup and a four-byte
    /// copy per pixel, is what kept the OVMF kernel suite inside its 90 s bound under TCG once a
    /// scroll became a redraw; see `ScreenConsole::draw`.
    fn store_run(&mut self, at: usize, run: &[u8]) -> bool {
        let room = self.bytes.len().saturating_sub(at);
        let fits = run.len().min(room) / 4 * 4;
        if fits > 0 {
            self.bytes[at..at + fits].copy_from_slice(&run[..fits]);
            #[cfg(test)]
            {
                self.stores += fits / 4;
            }
        }
        fits == run.len()
    }

    /// How many bytes of screen this covers.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Whether it covers nothing at all.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
}

/// **What the screen says, kept in RAM, one byte per character cell.**
///
/// The console redraws from this rather than from the screen, which is what makes a scroll cost
/// writes and no reads. A byte is the glyph's ASCII code, a space where nothing was drawn. One
/// colour pair and no attributes, so a byte is the whole cell.
///
/// **A separate value from [`ScreenConsole`] rather than a field of it**, because of its size:
/// [`Cells::CAPACITY`] bytes, about 35 KiB, and the kernel's boot stack is 64 KiB with a recorded
/// incident from a 16 KiB array on it (`notes/stack.md`). The kernel keeps this in its console's
/// `static`, initialised at compile time, so it is never built on a stack; [`ScreenConsole`] stays
/// the small `Copy` value it always was.
///
/// Name: provisional (the console-scroll lane, 2026-10-04).
pub struct Cells {
    text: [u8; Cells::CAPACITY],
}

impl Cells {
    /// **How many cells there is room for**: 240 columns by 150 rows.
    ///
    /// 240 is the widest any screen gets. [`ScreenConsole::scale_for`] picks the largest scale
    /// that leaves [`MIN_COLUMNS`] columns, so a screen gets fewer than `2 * MIN_COLUMNS` columns
    /// at every scale: at one, because it is narrower than two scales' worth; at a larger scale,
    /// because the next scale up did not fit. 150 rows is a 1200-pixel-tall screen at scale one,
    /// the tallest under 1680 wide this tree has a reason to expect. A screen with more rows than
    /// this leaves room for is drawn on the top rows that fit; see this crate's BUGS.
    pub const CAPACITY: usize = 2 * MIN_COLUMNS as usize * 150;

    /// A blank grid: every cell a space.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            text: [b' '; Self::CAPACITY],
        }
    }
}

impl Default for Cells {
    fn default() -> Self {
        Self::new()
    }
}

/// A cursor on a screen, and the arithmetic that puts a byte under it.
///
/// It holds no picture. The screen's text lives in a [`Cells`] and the screen itself is a
/// [`PixelSink`], both passed in on every call rather than held, so that this type is a plain value
/// a caller can put in a `static` without a pointer to device memory living inside it.
///
/// # Examples
///
/// A whole console, painted into ordinary memory, which is what the host tests in `tests.rs` do and
/// what makes this crate provable without a screen:
///
/// ```
/// use machine_discovery::framebuffer::{Framebuffer, PixelOrder};
/// use screen_console::{Cells, PixelSink, ScreenConsole};
///
/// // Two cells wide, one tall: 14x8 pixels at four bytes each.
/// let screen = Framebuffer { base: 0, width: 14, height: 8, stride: 56, order: PixelOrder::Bgrx };
/// let mut pixels = [0u8; 56 * 8];
/// let mut console = ScreenConsole::new(screen).expect("a screen big enough for one cell");
///
/// assert_eq!(console.size(), (2, 1));
/// let mut cells = Cells::new();
/// console.clear(&mut cells, &mut PixelSink::new(&mut pixels));
/// console.write(&mut cells, &mut PixelSink::new(&mut pixels), "F");
///
/// // The top-left pixel of an `F` is background and the one beside it is ink, which is the
/// // property a mirrored or transposed painter gets wrong. See `bitmap_font`'s own example.
/// let pixel = |x: usize, y: usize| u32::from_le_bytes(
///     pixels[y * 56 + x * 4..y * 56 + x * 4 + 4].try_into().unwrap(),
/// );
/// assert_eq!(pixel(0, 0), ScreenConsole::BACKGROUND);
/// assert_eq!(pixel(1, 0), ScreenConsole::FOREGROUND);
/// ```
#[derive(Clone, Copy, Debug)]
pub struct ScreenConsole {
    screen: Framebuffer,
    cols: u32,
    rows: u32,
    col: u32,
    row: u32,
    /// How many screen pixels each font pixel becomes, in each direction. See [`Self::scale_for`].
    scale: u32,
}

impl ScreenConsole {
    /// Ink. A light grey rather than white: white on black at a 1:1 pixel scale is glare on a real
    /// monitor, and every other console in this tree is a terminal's default rather than maximum
    /// contrast.
    pub const FOREGROUND: u32 = 0x00c8_c8c8;

    /// Paper. Black, because the alternative is repainting a firmware logo one cell at a time and
    /// getting a boot tour written over a splash screen.
    pub const BACKGROUND: u32 = 0x0000_0000;

    /// A console over `screen`, or `None` if it cannot hold a single character cell.
    ///
    /// The geometry is validated by [`Framebuffer::span`], which is the same check the loader
    /// applied before writing the description down. Doing it twice is deliberate: this crate is
    /// handed a description that came across a boot handoff, and the cost of believing a bad one is
    /// a kernel writing over memory it does not own.
    #[must_use]
    pub fn new(screen: Framebuffer) -> Option<Self> {
        screen.span()?;
        let scale = Self::scale_for(screen.width);
        let cols = screen.width / (bitmap_font::GLYPH_W * scale);
        let fit = u32::try_from(Cells::CAPACITY / cols.max(1) as usize).unwrap_or(u32::MAX);
        let rows = (screen.height / (bitmap_font::GLYPH_H * scale)).min(fit);
        if cols == 0 || rows == 0 {
            return None;
        }
        Some(Self {
            screen,
            cols,
            rows,
            col: 0,
            row: 0,
            scale,
        })
    }

    /// **The integer scale a screen `width` pixels wide is drawn at**: the largest that still
    /// leaves [`MIN_COLUMNS`] columns, never less than one. Each font pixel becomes a
    /// scale-by-scale block.
    ///
    /// The font's cell is 7 pixels wide, so a 1920x1080 monitor drew 274 columns of 8-pixel-tall
    /// glyphs at one, and on xenon's bench on 2026-10-04 calef could neither read nor photograph
    /// them. At two it draws 14x16 cells, 137 columns by 67 rows. Anything under 1680 wide
    /// (QEMU's 1280x800 OVMF screen among them) stays at one, as before.
    #[must_use]
    pub const fn scale_for(width: u32) -> u32 {
        Self::scale_for_cell(width, bitmap_font::GLYPH_W)
    }

    /// **[`Self::scale_for`]'s rule for a cell `cell_w` pixels wide**: the largest integer scale
    /// that still leaves [`MIN_COLUMNS`] columns of such cells, never less than one. The one rule
    /// both consoles draw by, so the shell's terminal (14-pixel cells from `glyph_atlas`) and this
    /// console's 7-pixel bitmap give a screen of the same width the same columns at most: a
    /// 1920-wide screen is 137 columns of either, at scale one for the terminal and two here.
    ///
    /// Name: provisional.
    #[must_use]
    pub const fn scale_for_cell(width: u32, cell_w: u32) -> u32 {
        let s = width / (cell_w * MIN_COLUMNS);
        if s == 0 { 1 } else { s }
    }

    /// The grid, in character cells: `(columns, rows)`.
    #[must_use]
    pub const fn size(&self) -> (u32, u32) {
        (self.cols, self.rows)
    }

    /// The screen this console paints, as the boot handoff described it.
    ///
    /// For the one caller that gives the screen away (the kernel's console, when a userspace
    /// terminal takes it over): what it hands on is the description it was itself given, so the
    /// driver that paints next and the console that painted before cannot disagree about the
    /// geometry. **Name provisional** (the shell on the firmware screen).
    #[must_use]
    pub const fn screen(&self) -> Framebuffer {
        self.screen
    }

    /// How many bytes of framebuffer this console addresses, which is what a caller has to map and
    /// how long the slice behind the [`PixelSink`] it passes to [`Self::write`] must be.
    #[must_use]
    pub fn span(&self) -> usize {
        self.screen.span().unwrap_or(0)
    }

    /// Paint the whole surface in [`Self::BACKGROUND`], blank `cells`, and put the cursor at the top
    /// left.
    ///
    /// Called once when a console is armed, because whatever the firmware left on the screen is not
    /// this kernel's and text drawn over a logo is text nobody can read. **This is what makes the
    /// grid and the screen agree**: every cell a space, every pixel paper, which is exactly the
    /// picture a grid of spaces draws.
    pub fn clear(&mut self, cells: &mut Cells, pixels: &mut PixelSink<'_>) {
        let paper = self.screen.order.store(Self::BACKGROUND).to_le_bytes();
        // 512 bytes, not a page: a 4 KiB local put this frame over the 4 KiB guard under every
        // kernel thread stack (`script/stack-frame-check`, #1645's second CI run).
        let mut run = [0u8; 512];
        for pixel in run.as_chunks_mut::<4>().0 {
            *pixel = paper;
        }
        for at in (0..pixels.len()).step_by(run.len()) {
            pixels.store_run(at, &run[..run.len().min(pixels.len() - at)]);
        }
        cells.text.fill(b' ');
        self.col = 0;
        self.row = 0;
    }

    /// Write `text` at the cursor, advancing it.
    ///
    /// `cells` must be the grid this console was last [`cleared`](Self::clear) with, and `pixels`
    /// the screen. A screen shorter than [`Self::span`] simply truncates the picture rather than
    /// panicking: this is the code that runs when something else has already gone wrong, and a
    /// bounds panic inside the console would take the message with it.
    pub fn write(&mut self, cells: &mut Cells, pixels: &mut PixelSink<'_>, text: &str) {
        for byte in text.bytes() {
            self.put(cells, pixels, byte);
        }
    }

    /// One byte.
    fn put(&mut self, cells: &mut Cells, pixels: &mut PixelSink<'_>, byte: u8) {
        match byte {
            b'\n' => {
                self.col = 0;
                self.newline(cells, pixels);
                return;
            }
            b'\r' => {
                self.col = 0;
                return;
            }
            _ => {}
        }
        if self.col >= self.cols {
            self.col = 0;
            self.newline(cells, pixels);
        }
        // A byte with no glyph is drawn as a space rather than dropped, so that a run of them still
        // occupies the columns it occupies on the UART and the two transcripts line up.
        let glyph = if (0x20..0x7f).contains(&byte) {
            byte
        } else {
            b' '
        };
        cells.text[self.cell(self.col, self.row)] = glyph;
        self.draw(pixels, self.col, self.row, glyph);
        self.col += 1;
    }

    /// Where cell (`col`, `row`) lives in a [`Cells`]. In range by construction: `new` keeps
    /// `cols * rows` within [`Cells::CAPACITY`].
    const fn cell(&self, col: u32, row: u32) -> usize {
        (row * self.cols + col) as usize
    }

    /// Move to the next row, scrolling when there is not one.
    fn newline(&mut self, cells: &mut Cells, pixels: &mut PixelSink<'_>) {
        if self.row + 1 < self.rows {
            self.row += 1;
        } else {
            self.scroll(cells, pixels);
        }
    }

    /// Move the text up by one row of cells and blank the row that opens at the bottom.
    ///
    /// **Redrawn from the grid, and only where it changed.** The picture this produces is exactly
    /// the one the old copy-up of the screen produced (a host test holds the two side by side), but
    /// it reads nothing from the screen and writes only the cells whose glyph differs from the one
    /// already there. A boot transcript is mostly short lines over blank paper, so most cells are a
    /// space moving onto a space and cost nothing (the test
    /// `a_scroll_writes_only_the_cells_that_changed` measures how much on a real transcript). A
    /// one-row console has nowhere to scroll to and is left alone, as it always was.
    fn scroll(&mut self, cells: &mut Cells, pixels: &mut PixelSink<'_>) {
        if self.rows < 2 {
            return;
        }
        let cols = self.cols as usize;
        let live = cols * self.rows as usize;
        for row in 0..self.rows {
            for col in 0..self.cols {
                let at = self.cell(col, row);
                let next = if row + 1 < self.rows {
                    cells.text[at + cols]
                } else {
                    b' '
                };
                if cells.text[at] != next {
                    self.draw(pixels, col, row, next);
                }
            }
        }
        cells.text.copy_within(cols..live, 0);
        cells.text[live - cols..live].fill(b' ');
    }

    /// Paint one glyph into cell (`col`, `row`).
    ///
    /// Each font row is built once, as one screen row's run of `GLYPH_W * scale` pixels, and that
    /// run is stored `scale` times. Per pixel there is no font lookup and no bounds check, which
    /// matters more than it looks: under QEMU's TCG an unoptimised per-pixel loop made the
    /// redrawing scroll slower than the copy it replaced, and the OVMF kernel suite overran its
    /// bound (#1645's first CI run, 2026-10-04).
    fn draw(&self, pixels: &mut PixelSink<'_>, col: u32, row: u32, glyph: u8) {
        /// Room for a glyph row up to scale 16, a screen 13,440 pixels wide (448 bytes, kept small
        /// because this is a frame on a kernel thread stack). Past that the cell is left unpainted
        /// rather than drawn wrong.
        const RUN: usize = bitmap_font::GLYPH_W as usize * 16 * 4;
        let stride = self.screen.stride as usize;
        let k = self.scale as usize;
        let len = bitmap_font::GLYPH_W as usize * k * 4;
        if len > RUN {
            return;
        }
        let left = (col * bitmap_font::GLYPH_W) as usize * k * 4;
        let top = (row * bitmap_font::GLYPH_H) as usize * k;
        let ink = self.screen.order.store(Self::FOREGROUND).to_le_bytes();
        let paper = self.screen.order.store(Self::BACKGROUND).to_le_bytes();
        let mut run = [0u8; RUN];
        for (fy, &bits) in bitmap_font::glyph(glyph as char).iter().enumerate() {
            for (x, pixel) in run[..len].as_chunks_mut::<4>().0.iter_mut().enumerate() {
                *pixel = if bits >> (x / k) & 1 != 0 { ink } else { paper };
            }
            for dy in 0..k {
                if !pixels.store_run((top + fy * k + dy) * stride + left, &run[..len]) {
                    return;
                }
            }
        }
    }
}

#[cfg(test)]
extern crate std;

#[cfg(test)]
mod tests;
