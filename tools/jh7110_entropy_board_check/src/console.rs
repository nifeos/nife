//! Record output on UART0, one checksummed line per record.
//!
//! A record is `board-check: <payload> *<crc32>`, where the checksum is the IEEE CRC-32 (zlib's) of
//! the payload bytes, as eight lower-case hex digits. The serial link to a VisionFive 2 runs at
//! 115200 baud with no flow control and has been seen to damage bytes under sustained output
//! (notes/bench-runbook.md, BUGS), so the checker keeps only lines whose checksum matches.
//!
//! The UART is an 8250-compatible block whose register stride is the device tree's `reg-shift`:
//! 2 on the VisionFive 2's `snps,dw-apb-uart`, 0 on QEMU `virt`'s `ns16550a`. U-Boot leaves it
//! configured at 115200 8N1, so this only writes the transmit register.
//!
//! Name: provisional.

use core::fmt::Write;
use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// UART0's base on both machines, used until the device tree says otherwise.
static UART_BASE: AtomicU64 = AtomicU64::new(0x1000_0000);
/// The register stride, `1 << shift` bytes. The VisionFive 2's until the tree says otherwise, so a
/// board that fails to parse its tree still prints.
static UART_SHIFT: AtomicU32 = AtomicU32::new(2);

/// Points output at the UART the device tree describes: `base` and its `reg-shift`.
pub(crate) fn configure(base: u64, shift: u32) {
    UART_BASE.store(base, Ordering::Relaxed);
    UART_SHIFT.store(shift, Ordering::Relaxed);
}

/// The 8250 transmit holding register's index, and the line status register's.
const TRANSMIT_HOLDING: u64 = 0;
const LINE_STATUS: u64 = 5;
/// `LSR.THRE`: the transmit holding register is empty.
const LINE_STATUS_TRANSMIT_EMPTY: u32 = 1 << 5;

fn put(byte: u8) {
    let base = UART_BASE.load(Ordering::Relaxed);
    let shift = UART_SHIFT.load(Ordering::Relaxed);
    let status = (base + (LINE_STATUS << shift)) as *const u32;
    let transmit = (base + (TRANSMIT_HOLDING << shift)) as *mut u32;
    if shift == 0 {
        // A byte-wide 16550 (QEMU `virt`): 32-bit accesses would touch the neighbouring registers.
        // SAFETY: `base` is the console UART the tree named; these are its LSR and THR.
        unsafe {
            while core::ptr::read_volatile(status.cast::<u8>()) as u32 & LINE_STATUS_TRANSMIT_EMPTY
                == 0
            {}
            core::ptr::write_volatile(transmit.cast::<u8>(), byte);
        }
    } else {
        // SAFETY: as above, with the 32-bit accesses the board's `reg-io-width = <4>` asks for.
        unsafe {
            while core::ptr::read_volatile(status) & LINE_STATUS_TRANSMIT_EMPTY == 0 {}
            core::ptr::write_volatile(transmit, u32::from(byte));
        }
    }
}

/// The IEEE CRC-32 of `bytes`, bit by bit: zlib's `crc32`, which `results.py` recomputes.
fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

/// A record being formatted. Longer than the longest record the checks print (a capture line is
/// about 90 bytes); a record that overflows it is cut short and its checksum then covers what fit,
/// so the checker sees a truncated but honest line.
struct Line {
    bytes: [u8; 512],
    length: usize,
}

impl Write for Line {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let room = self.bytes.len() - self.length;
        let take = s.len().min(room);
        self.bytes[self.length..self.length + take].copy_from_slice(&s.as_bytes()[..take]);
        self.length += take;
        if take < s.len() {
            Err(core::fmt::Error)
        } else {
            Ok(())
        }
    }
}

/// Prints one record: `board-check: ` then `args`, a space, `*` and the payload's CRC-32.
pub(crate) fn record(args: core::fmt::Arguments) {
    let mut line = Line {
        bytes: [0; 512],
        length: 0,
    };
    let _ = line.write_fmt(args);
    let payload = &line.bytes[..line.length];
    let mut digits = [0u8; 8];
    let crc = crc32(payload);
    for (i, digit) in digits.iter_mut().enumerate() {
        *digit = b"0123456789abcdef"[((crc >> (28 - 4 * i)) & 0xf) as usize];
    }
    for &byte in b"board-check: "
        .iter()
        .chain(payload)
        .chain(b" *")
        .chain(&digits)
        .chain(b"\r\n")
    {
        put(byte);
    }
}

/// Formats 32 bytes as 64 lower-case hex digits, for a record.
pub(crate) struct Hex<'a>(pub(crate) &'a [u8]);

impl core::fmt::Display for Hex<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}
