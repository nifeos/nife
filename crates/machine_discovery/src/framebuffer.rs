//! Where the pixels are, told to the kernel by whoever booted it (milestone 243 (a machine with no
//! serial port has no way to say anything)).
//!
//! The description rides the boot command line as one `screen=` token. [`Framebuffer::encode`] is
//! what `uefi_loader` calls and [`Framebuffer::parse`] what the kernel calls, so neither keeps its
//! own copy of the spelling. See `notes/serial-less-output.md` for why a token and not a field,
//! and `notes/machine-discovery.md` ("The screen token").

//! # BUGS
//!
//! - Only the two 32-bit-per-pixel orders are expressible. A loader meeting `PixelBitMask` or
//!   `PixelBltOnly` writes no token, and the kernel keeps whatever console it had.
//! - Nothing validates that the address is a framebuffer. The trust boundary is the boot chain,
//!   where the memory map already sits.
//! - The aperture's size is not carried; a firmware `framebuffer_size` larger than
//!   `stride * height` goes unnoticed.

/// The byte order of one 32-bit pixel, as it lies in memory.
///
/// **Named for the memory order, not the `u32` value**, because the memory order is the thing a
/// writer has to get right and the `u32` spelling of it is the thing that flips between them. UEFI
/// names these the same way (`PixelBlueGreenRedReserved8BitPerColor` lists the bytes in order).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PixelOrder {
    /// Bytes B, G, R, unused. A little-endian `u32` is `0x00RRGGBB`, which is how every colour
    /// constant in this tree is already written, so no swizzle is needed. It is also what OVMF and
    /// essentially every PC display adapter report.
    Bgrx,
    /// Bytes R, G, B, unused. A little-endian `u32` is `0x00BBGGRR`, so a colour written in the
    /// tree's usual spelling has to have its red and blue exchanged before it is stored.
    Rgbx,
}

impl PixelOrder {
    /// The token this order is spelled with on the command line.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::Bgrx => "bgrx",
            Self::Rgbx => "rgbx",
        }
    }

    /// Turn a colour written as `0x00RRGGBB` into the `u32` this order wants stored.
    ///
    /// The whole of the byte-order question, in one place, so that no painter has to hold it.
    #[must_use]
    pub const fn store(self, rrggbb: u32) -> u32 {
        match self {
            Self::Bgrx => rrggbb,
            // Exchange the red and blue bytes; green and the unused byte stay where they are.
            Self::Rgbx => {
                (rrggbb & 0x0000_ff00) | ((rrggbb & 0xff) << 16) | ((rrggbb >> 16) & 0xff)
            }
        }
    }
}

/// A linear framebuffer, as one boot stage describes it to the next.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Framebuffer {
    /// The physical address of pixel (0, 0).
    pub base: u64,
    /// Visible width, in pixels.
    pub width: u32,
    /// Visible height, in pixels.
    pub height: u32,
    /// **Bytes from one row to the next**, which is not `width * 4`. Firmware is free to pad rows
    /// out to a convenient pitch, and a painter that multiplies by the width instead paints a
    /// picture that shears progressively down the screen.
    pub stride: u32,
    /// How one pixel's four bytes are arranged.
    pub order: PixelOrder,
}

impl Framebuffer {
    /// The key this token opens with. `screen` rather than Linux's `video`, because Linux's takes a
    /// *mode request* (`video=1024x768`) and this takes an *answer*; borrowing the spelling would
    /// borrow the wrong meaning.
    pub const KEY: &'static str = "screen=";

    /// The longest token [`Self::encode`] can produce, for a caller sizing a buffer.
    ///
    /// The worst case is 63 bytes: `screen=`, `0x` and sixteen hex digits, three ten-digit decimals
    /// with their commas, and a four-character order. Rounded up rather than derived, so nothing
    /// ties it to the encoder: when the key grew from `fb=` to `screen=`, the worst case reached 63
    /// of the old 64 unnoticed, and one byte of headroom is a trap for whoever adds a field.
    pub const MAX_LEN: usize = 72;

    /// How many bytes the console may touch: `stride * height`.
    ///
    /// Returns `None` when the arithmetic overflows or the geometry is degenerate, including a
    /// stride shorter than one row of pixels. The row width is compared in `u64`, because in `u32`
    /// a saturated `width * 4` lets a too-short stride through. This is the only validation a
    /// description read out of a boot handoff gets.
    #[must_use]
    pub const fn span(&self) -> Option<usize> {
        if self.width == 0 || self.height == 0 || (self.stride as u64) < self.width as u64 * 4 {
            return None;
        }
        match (self.stride as u64).checked_mul(self.height as u64) {
            Some(bytes) if bytes <= usize::MAX as u64 => Some(bytes as usize),
            _ => None,
        }
    }

    /// Write this description into `out` as one command-line token, returning its length. Written
    /// by hand rather than with `core::fmt`, for a `no_std` caller with no allocator.
    ///
    /// # Panics
    ///
    /// If `out` is shorter than [`Self::MAX_LEN`].
    ///
    /// # Examples
    ///
    /// The token survives the round trip unchanged:
    ///
    /// ```
    /// use machine_discovery::framebuffer::{Framebuffer, PixelOrder};
    ///
    /// let found = Framebuffer {
    ///     base: 0x8000_0000,
    ///     width: 800,
    ///     height: 600,
    ///     stride: 3200,
    ///     order: PixelOrder::Bgrx,
    /// };
    ///
    /// let mut buffer = [0u8; Framebuffer::MAX_LEN];
    /// let written = found.encode(&mut buffer);
    /// assert_eq!(&buffer[..written], b"screen=0x80000000,800,600,3200,bgrx");
    ///
    /// let text = core::str::from_utf8(&buffer[..written]).expect("ASCII");
    /// assert_eq!(Framebuffer::parse(text), Some(found));
    /// ```
    pub fn encode(&self, out: &mut [u8]) -> usize {
        assert!(out.len() >= Self::MAX_LEN, "encode needs MAX_LEN bytes");
        let mut n = 0;
        let mut put = |bytes: &[u8], n: &mut usize| {
            out[*n..*n + bytes.len()].copy_from_slice(bytes);
            *n += bytes.len();
        };
        put(Self::KEY.as_bytes(), &mut n);
        let mut hex = [0u8; 18];
        let hex_len = write_hex(self.base, &mut hex);
        put(&hex[..hex_len], &mut n);
        for value in [self.width, self.height, self.stride] {
            put(b",", &mut n);
            let mut decimal = [0u8; 10];
            let len = write_decimal(value, &mut decimal);
            put(&decimal[..len], &mut n);
        }
        put(b",", &mut n);
        put(self.order.token().as_bytes(), &mut n);
        n
    }

    /// Find and read the token on a boot command line, or `None` if there is not one.
    ///
    /// A malformed token reads as absent: a display description the kernel could do without must
    /// not stop the boot.
    ///
    /// # Examples
    ///
    /// Other words on the line are ignored, and no token is the ordinary case, not an error:
    ///
    /// ```
    /// use machine_discovery::framebuffer::Framebuffer;
    ///
    /// assert!(Framebuffer::parse("quiet screen=0x1000,64,32,256,rgbx debug").is_some());
    /// assert_eq!(Framebuffer::parse("quiet debug"), None);
    /// ```
    #[must_use]
    pub fn parse(cmdline: &str) -> Option<Self> {
        let token = cmdline
            .split_ascii_whitespace()
            .find_map(|word| word.strip_prefix(Self::KEY))?;
        let mut fields = token.split(',');
        let base = parse_hex(fields.next()?)?;
        let width = parse_decimal(fields.next()?)?;
        let height = parse_decimal(fields.next()?)?;
        let stride = parse_decimal(fields.next()?)?;
        let order = match fields.next()? {
            "bgrx" => PixelOrder::Bgrx,
            "rgbx" => PixelOrder::Rgbx,
            _ => return None,
        };
        if fields.next().is_some() {
            return None;
        }
        let found = Self {
            base,
            width,
            height,
            stride,
            order,
        };
        found.span()?;
        Some(found)
    }
}

/// `0x` followed by the value's hex digits, shortest form, uppercase never (the tree prints hex
/// lowercase everywhere).
fn write_hex(value: u64, out: &mut [u8; 18]) -> usize {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    out[0] = b'0';
    out[1] = b'x';
    if value == 0 {
        out[2] = b'0';
        return 3;
    }
    let mut digits = [0u8; 16];
    let mut count = 0;
    let mut left = value;
    while left != 0 {
        digits[count] = DIGITS[(left & 0xf) as usize];
        left >>= 4;
        count += 1;
    }
    for i in 0..count {
        out[2 + i] = digits[count - 1 - i];
    }
    2 + count
}

/// The value's decimal digits, shortest form.
fn write_decimal(value: u32, out: &mut [u8; 10]) -> usize {
    if value == 0 {
        out[0] = b'0';
        return 1;
    }
    let mut digits = [0u8; 10];
    let mut count = 0;
    let mut left = value;
    while left != 0 {
        digits[count] = b'0' + (left % 10) as u8;
        left /= 10;
        count += 1;
    }
    for i in 0..count {
        out[i] = digits[count - 1 - i];
    }
    count
}

/// `0x`-prefixed hex, which is what [`write_hex`] produces and what a person hand-editing a boot
/// line would write. A bare number is refused rather than guessed at.
fn parse_hex(text: &str) -> Option<u64> {
    let digits = text.strip_prefix("0x")?;
    if digits.is_empty() || digits.len() > 16 {
        return None;
    }
    let mut value = 0u64;
    for byte in digits.bytes() {
        let digit = match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'f' => byte - b'a' + 10,
            _ => return None,
        };
        value = (value << 4) | u64::from(digit);
    }
    Some(value)
}

/// Decimal, with no sign and no separators.
fn parse_decimal(text: &str) -> Option<u32> {
    if text.is_empty() {
        return None;
    }
    let mut value = 0u32;
    for byte in text.bytes() {
        let digit = byte.checked_sub(b'0').filter(|d| *d < 10)?;
        value = value.checked_mul(10)?.checked_add(u32::from(digit))?;
    }
    Some(value)
}

/// Machine-checked proofs over the screen token, a wire format between two binaries: the loader
/// writes it and the kernel reads it. See milestone 319 (the crate that parses firmware had no
/// proofs) for what each harness restores.
///
/// Names: provisional (milestone 319).
#[cfg(kani)]
mod verification {
    use super::*;

    /// **An encoded token never exceeds the maximum it advertises.** Nothing in the type system
    /// ties [`Framebuffer::MAX_LEN`] to what [`Framebuffer::encode`] writes, and an overrun is a
    /// panic in a `no_std` UEFI application. One more field, a longer order token or non-shortest
    /// hex breaks it, and no test can enumerate the widest `u64`.
    /// Falsification: replayable `crates/machine_discovery/falsifications/framebuffer.verification.an_encoded_token_never_exceeds_the_maximum_it_advertises.patch`
    #[kani::proof]
    #[kani::unwind(18)]
    fn an_encoded_token_never_exceeds_the_maximum_it_advertises() {
        let screen = Framebuffer {
            base: kani::any(),
            width: kani::any(),
            height: kani::any(),
            stride: kani::any(),
            order: if kani::any() {
                PixelOrder::Bgrx
            } else {
                PixelOrder::Rgbx
            },
        };
        let mut out = [0u8; Framebuffer::MAX_LEN];
        let n = screen.encode(&mut out);
        assert!(n <= Framebuffer::MAX_LEN);
        kani::cover!(n >= 60, "the worst case really does get close to the bound");
    }

    /// **A span is refused exactly when the geometry cannot be trusted, and never overflows.** The
    /// console paints inside whatever [`Framebuffer::span`] returns, so the claim is that a `Some`
    /// is sound: the stride covers a row of pixels, for every `u32` triple, including those where
    /// `width * 4` overflows a `u32`. A red result here once came from a `u64` product that reached
    /// `2^64` against correct code, so read the counterexample before trusting the colour.
    /// Falsification: replayable `crates/machine_discovery/falsifications/framebuffer.verification.an_accepted_span_covers_every_pixel_the_geometry_describes.patch`
    #[kani::proof]
    fn an_accepted_span_covers_every_pixel_the_geometry_describes() {
        let screen = Framebuffer {
            base: 0,
            width: kani::any(),
            height: kani::any(),
            stride: kani::any(),
            order: PixelOrder::Bgrx,
        };
        if let Some(span) = screen.span() {
            assert!(screen.width > 0 && screen.height > 0);
            // Only the guard half: `span == stride * height` restates `span`, and the full
            // `span >= width * 4 * height` needs `u128` and does not finish (milestone 319).
            assert!(screen.stride as u64 >= screen.width as u64 * 4);
            kani::cover!(span > 0, "a real geometry is accepted");
        }
    }

    /// **Every hex field the loader can write, the kernel reads back unchanged.** The writer emits
    /// shortest-form digits behind `0x` and the reader refuses more than sixteen digits; nothing
    /// else says the two bounds are one bound. A reader one digit short would read the token as no
    /// screen at all, and a base above 2^60 is real on firmware that relocates its windows.
    /// Falsification: replayable `crates/machine_discovery/falsifications/framebuffer.verification.every_hex_field_the_loader_writes_is_one_the_kernel_reads_back.patch`
    #[kani::proof]
    #[kani::unwind(18)]
    fn every_hex_field_the_loader_writes_is_one_the_kernel_reads_back() {
        let value: u64 = kani::any();
        let mut out = [0u8; 18];
        let n = write_hex(value, &mut out);
        assert!(n <= 18);
        let text = core::str::from_utf8(&out[..n]).expect("the writer emits ASCII");
        assert_eq!(parse_hex(text), Some(value));
        kani::cover!(
            n == 18,
            "the widest base really does use all sixteen digits"
        );
    }
}

/// The word that asks the kernel to hold the screen at the handover, so a host watching the
/// framebuffer is told when to look (milestone 445 (the screen check stops sampling and starts
/// asking)). A debugging affordance: the kernel stops between painting the tour and clearing the
/// screen, says so on the serial line and waits for a byte back (`kernel::console::yield_screen`).
/// Without the word the handover is unchanged. A bare word, because it carries nothing.
///
/// Name: provisional (`AGENTS.md`: calef names what a reader meets).
pub const SCREEN_HOLD: &str = "screen-hold";

/// Whether a boot command line carries [`SCREEN_HOLD`].
///
/// A whole-word match, so a longer token that merely starts this way (a future `screen-hold=5`,
/// say) does not read as this one. Absence is the answer for an empty line, which is what every
/// boot that is not a screen gate hands over.
#[must_use]
pub fn has_screen_hold(cmdline: &str) -> bool {
    cmdline.split_ascii_whitespace().any(|w| w == SCREEN_HOLD)
}

#[cfg(test)]
mod tests {
    use super::{Framebuffer, PixelOrder, has_screen_hold};

    /// The token has to survive sitting *beside* the framebuffer description, because that is the
    /// only command line it is ever written on: the loader emits both or neither.
    #[test]
    fn the_hold_token_is_found_beside_a_framebuffer_and_nowhere_else() {
        let line = "screen=0x80000000,800,600,3200,bgrx screen-hold";
        assert!(has_screen_hold(line), "the token is on this line");
        assert!(
            Framebuffer::parse(line).is_some(),
            "and the framebuffer beside it still parses"
        );
        assert!(
            !has_screen_hold(""),
            "an empty command line asks for nothing"
        );
        assert!(
            !has_screen_hold("screen=0x80000000,800,600,3200,bgrx"),
            "a framebuffer alone asks for nothing"
        );
        assert!(
            !has_screen_hold("screen-holder"),
            "a longer word that merely starts this way is not this word"
        );
    }

    /// The writer and the reader are one crate, so the property worth testing is that they agree.
    /// A loader whose token the kernel cannot read is the failure this exists to make impossible,
    /// and it is the failure nobody can see on a machine with no serial port.
    #[test]
    fn every_description_survives_the_round_trip() {
        for found in [
            Framebuffer {
                base: 0x8000_0000,
                width: 800,
                height: 600,
                stride: 3200,
                order: PixelOrder::Bgrx,
            },
            // A padded stride, which is the case a painter that multiplies by the width gets wrong.
            Framebuffer {
                base: 0xffff_ffff_0000_0000,
                width: 1366,
                height: 768,
                stride: 5504,
                order: PixelOrder::Rgbx,
            },
            Framebuffer {
                base: 0x1000,
                width: 1,
                height: 1,
                stride: 4,
                order: PixelOrder::Bgrx,
            },
        ] {
            let mut buffer = [0u8; Framebuffer::MAX_LEN];
            let n = found.encode(&mut buffer);
            let text = core::str::from_utf8(&buffer[..n]).expect("the encoder writes ASCII");
            assert_eq!(
                Framebuffer::parse(text),
                Some(found),
                "round trip of {text}"
            );
        }
    }

    /// `MAX_LEN` is a constant a caller sizes a stack buffer with, so it has to hold for the widest
    /// value the encoder can meet rather than for the values anyone expects.
    #[test]
    fn the_widest_token_fits_the_advertised_maximum() {
        let widest = Framebuffer {
            base: u64::MAX,
            width: u32::MAX,
            height: 1,
            stride: u32::MAX,
            order: PixelOrder::Bgrx,
        };
        let mut buffer = [0u8; Framebuffer::MAX_LEN];
        assert!(widest.encode(&mut buffer) <= Framebuffer::MAX_LEN);
    }

    /// A description that does not parse must read as "this machine said nothing about a screen",
    /// never as a boot failure. See the note on `parse`.
    #[test]
    fn a_malformed_token_reads_as_no_screen_at_all() {
        for bad in [
            "",
            "screen=",
            "screen=80000000,800,600,3200,bgrx", // no 0x
            "screen=0x8000,800,600,3200",        // no order
            "screen=0x8000,800,600,3200,cmyk",   // an order nobody has
            "screen=0x8000,800,600,3200,bgrx,7", // a field nobody wrote
            "screen=0x8000,0,600,3200,bgrx",     // no pixels
            "screen=0x8000,800,600,100,bgrx",    // a stride narrower than a row
            "screen=0x,800,600,3200,bgrx",       // the prefix and no digits after it
            "screen=0x8000,800,60:,3200,bgrx",   // the character after '9' is not a tenth digit
        ] {
            assert_eq!(Framebuffer::parse(bad), None, "{bad:?} should not parse");
        }
    }

    /// The token is found among others and does not have to be first, because a boot command line
    /// is a place other things will eventually be written too.
    #[test]
    fn the_token_is_found_among_other_words() {
        let found = Framebuffer::parse("console=uart screen=0x1000,64,32,256,rgbx smp=2");
        assert_eq!(found.map(|f| f.width), Some(64));
    }

    /// The `u32` a painter stores differs between the two orders, and this is the only place in the
    /// tree that knows how. Red must come out red on both.
    #[test]
    fn red_is_stored_as_red_in_both_orders() {
        const RED: u32 = 0x00ff_0000;
        assert_eq!(PixelOrder::Bgrx.store(RED), 0x00ff_0000);
        assert_eq!(PixelOrder::Rgbx.store(RED), 0x0000_00ff);
        // Green is in the same byte either way, which is what makes a mirrored painter hard to see.
        const GREEN: u32 = 0x0000_ff00;
        assert_eq!(PixelOrder::Bgrx.store(GREEN), GREEN);
        assert_eq!(PixelOrder::Rgbx.store(GREEN), GREEN);
        // And blue, which is the only one of the three that travels UP the word. Red's byte is
        // already at the top, so red alone cannot tell a sixteen-bit shift from either direction.
        const BLUE: u32 = 0x0000_00ff;
        assert_eq!(PixelOrder::Bgrx.store(BLUE), 0x0000_00ff);
        assert_eq!(PixelOrder::Rgbx.store(BLUE), 0x00ff_0000);
    }

    /// The span is what bounds every write a console makes, so a geometry whose arithmetic
    /// overflows has to be refused rather than truncated.
    ///
    /// A screen `u32::MAX` pixels wide with a stride of `u32::MAX` is refused: four bytes per pixel
    /// need four times that stride. A saturated `width * 4` would make the comparison vacuous above
    /// `2^30 - 1`, and a test reading the same product would agree with it.
    #[test]
    fn a_span_that_cannot_be_computed_is_refused() {
        let absurd = Framebuffer {
            base: 0,
            width: u32::MAX,
            height: u32::MAX,
            stride: u32::MAX,
            order: PixelOrder::Bgrx,
        };
        assert_eq!(
            absurd.span(),
            None,
            "one row of u32::MAX pixels needs four times this stride"
        );

        // A stride that does cover the row is computed, and on a 64-bit host it fits.
        let wide = Framebuffer {
            // The widest row a u32 stride can cover: four bytes each, so (2^32 - 4) bytes.
            width: (1 << 30) - 1,
            stride: u32::MAX,
            height: 4,
            ..absurd
        };
        assert_eq!(wide.span(), Some(u32::MAX as usize * 4));
    }
}
