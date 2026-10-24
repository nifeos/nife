//! Line output on the machine's UART.
//!
//! Name: provisional, minted 2026-10-24 (UTC) by maintainer/measure-arm64-host-reordering.

use core::fmt::Write;

struct Uart;

impl Write for Uart {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for b in s.bytes() {
            if b == b'\n' {
                crate::arch::putc(b'\r');
            }
            crate::arch::putc(b);
        }
        Ok(())
    }
}

/// Prints `args` and a newline. Only core 0 prints, so lines never interleave.
pub(crate) fn line(args: core::fmt::Arguments) {
    let _ = Uart.write_fmt(args);
    let _ = Uart.write_str("\n");
}
