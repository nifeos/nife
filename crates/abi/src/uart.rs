//! **The console UART's register shape, carried as one start word** (milestone 890 (riscv64 and
//! `x86_64` join the shared boot path)).
//!
//! The NS16550's register stride is the machine's: one byte on QEMU `virt`, four with 32-bit
//! access on the JH7110 (`reg-shift = <2>`, `reg-io-width = <4>`), where a driver assuming QEMU's
//! would read line status from the wrong register and wait forever. The kernel reads it from the
//! device tree (`console::configure_from_dtb`) and passes it to the progenitor as its first start
//! word, and the progenitor to the console and input drivers, which read no tree. The word is
//! `reg_shift | reg_io_width << 8`; 0, which aarch64 and `x86_64` pass, is QEMU's byte layout.
//!
//! Name: provisional (milestone 890's radon lane, 2026-10-20).

/// How far apart a UART's registers are and how wide an access to one is.
///
/// Name: provisional, minted 2026-10-20 (UTC) by milestone/890-radon-console-shape.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct RegisterShape {
    reg_shift: u8,
    reg_io_width: u8,
}

/// The device frame both drivers map is one page, and register 7 at this shift is the last that
/// still lands inside it. A larger shift from a malformed word is clamped rather than shifted off
/// the page.
const MAX_SHIFT: u8 = 9;

impl RegisterShape {
    /// Consecutive byte registers: QEMU `virt`, and the meaning of a zero word.
    ///
    /// Name: provisional, minted 2026-10-20 (UTC) by milestone/890-radon-console-shape.
    pub const BYTES: Self = Self {
        reg_shift: 0,
        reg_io_width: 1,
    };

    /// The shape a device tree states. A width of 0 means unstated, which is byte access.
    ///
    /// Name: provisional, minted 2026-10-20 (UTC) by milestone/890-radon-console-shape.
    pub const fn new(reg_shift: u8, reg_io_width: u8) -> Self {
        Self {
            reg_shift: if reg_shift > MAX_SHIFT {
                MAX_SHIFT
            } else {
                reg_shift
            },
            reg_io_width: if reg_io_width == 0 { 1 } else { reg_io_width },
        }
    }

    /// The start word that carries this shape.
    ///
    /// Name: provisional, minted 2026-10-20 (UTC) by milestone/890-radon-console-shape.
    pub const fn to_word(self) -> u64 {
        self.reg_shift as u64 | (self.reg_io_width as u64) << 8
    }

    /// The shape a start word carries; any bits above the two bytes are ignored.
    ///
    /// Name: provisional, minted 2026-10-20 (UTC) by milestone/890-radon-console-shape.
    pub const fn from_word(word: u64) -> Self {
        Self::new(word as u8, (word >> 8) as u8)
    }

    /// The byte offset of register `index` from the block's base.
    ///
    /// Name: provisional, minted 2026-10-20 (UTC) by milestone/890-radon-console-shape.
    pub const fn offset(self, index: u64) -> u64 {
        index << self.reg_shift
    }

    /// Whether every access is 32 bits wide. As in the kernel's driver, a 4-byte width is honored
    /// only when the stride keeps every register 4-byte aligned, because an unaligned 32-bit
    /// access traps; anything else is byte access.
    ///
    /// Name: provisional, minted 2026-10-20 (UTC) by milestone/890-radon-console-shape.
    pub const fn is_word_access(self) -> bool {
        self.reg_io_width == 4 && self.reg_shift >= 2
    }
}
