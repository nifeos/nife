//! **Reads a device's identity registers through a mapping it was handed, and reports them** (step
//! 19d.2 of milestone 19d (run a real workload)). `device_child_builder` maps the PL011 UART's
//! registers, device-typed, at [`UART_VA`] before starting this program, and grants it a report
//! endpoint in slot 0. The four PrimeCell identification bytes of every real PL011 read
//! `0xB105F00D`, so sending that word proves the mapping is a real view of the UART rather than
//! normal memory or the wrong page.
//!
//! Name: ratified 2026-10-18 (calef), as the child `DEVICE_CHILD_BUILDER` builds.

#![no_std]
// Program entry points, not the crates/ library surface milestone 68 (code-quality gates)'s ratchet
// tracks (DECISIONS §107 (`missing_docs` moves to the workspace lints)): each `[[bin]]` is its own
// crate root with one `_start`, and 58 of them documenting an OS-facing ABI entry point is not what
// the lint is for.
#![allow(missing_docs)]
#![no_main]

use user_mode_runtime::{exit, send};

const REPORT: u64 = 0;
/// Must match `device_child_builder`'s `CHILD_UART_VA`, where it maps the registers.
const UART_VA: u64 = address_space_map::pair_page(0x0070_0000);

#[unsafe(no_mangle)]
pub extern "C" fn _start(_arg0: u64, _arg1: u64, _arg2: u64) -> ! {
    // The four PrimeCell ID bytes live at 0xFF0, 0xFF4, 0xFF8 and 0xFFC and read 0x0D, 0xF0, 0x05
    // and 0xB1.
    // SAFETY: the builder mapped the UART, device-typed, at UART_VA before starting us; these are
    // read-only identification registers, so reading them has no side effect.
    let id = unsafe {
        let base = UART_VA as *const u32;
        let b0 = base.byte_add(0xFF0).read_volatile() & 0xFF;
        let b1 = base.byte_add(0xFF4).read_volatile() & 0xFF;
        let b2 = base.byte_add(0xFF8).read_volatile() & 0xFF;
        let b3 = base.byte_add(0xFFC).read_volatile() & 0xFF;
        (b3 << 24) | (b2 << 16) | (b1 << 8) | b0
    };
    send(REPORT, id as u64, 0, 0);
    exit();
}

user_mode_runtime::panic_handler!();
