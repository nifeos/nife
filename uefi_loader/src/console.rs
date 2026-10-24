//! **Saying things on the firmware console** without an allocator or `core::fmt`, and the
//! panic handler that says the last one.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

/// Write `ascii` into `out` as NUL-terminated UTF-16, and answer how many units that took, the NUL
/// included. Every character of a removable-media path is ASCII, so this is a widening and not an
/// encoding.
#[cfg(target_arch = "x86_64")]
pub(crate) fn utf16(ascii: &str, out: &mut [u16; 64]) -> usize {
    let mut n = 0;
    for byte in ascii.bytes() {
        out[n] = u16::from(byte);
        n += 1;
    }
    out[n] = 0;
    n + 1
}

/// Print one unsigned value in decimal, without an allocator and without `core::fmt`.
#[cfg_attr(target_arch = "aarch64", allow(dead_code))]
pub(crate) fn say_decimal(table: &SystemTable, value: u32) {
    let mut buffer = [0u8; 10];
    let mut n = 0;
    let mut digits = [0u8; 10];
    let mut left = value;
    loop {
        digits[n] = b'0' + (left % 10) as u8;
        left /= 10;
        n += 1;
        if left == 0 {
            break;
        }
    }
    for i in 0..n {
        buffer[i] = digits[n - 1 - i];
    }
    // SAFETY: every byte written above is an ASCII digit.
    say(table, unsafe {
        core::str::from_utf8_unchecked(&buffer[..n])
    });
}

/// Print a `[start, end)` physical range after a caller-supplied phrase.
///
/// Its own function rather than a format string because there is no allocator here and
/// `core::fmt` on the firmware console would pull in machinery this binary otherwise does not have.
pub(crate) fn say_span(table: &SystemTable, what: &str, start: u64, end: u64) {
    say(table, what);
    say_hex(table, start);
    say(table, "..");
    say_hex(table, end);
    say(table, "\r\n");
}

/// Print one 64-bit value as `0x...`, without an allocator and without `core::fmt`.
pub(crate) fn say_hex(table: &SystemTable, value: u64) {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut buffer = [0u8; 18];
    buffer[0] = b'0';
    buffer[1] = b'x';
    for i in 0..16 {
        buffer[2 + i] = DIGITS[((value >> (60 - 4 * i)) & 0xf) as usize];
    }
    // SAFETY: every byte written above is ASCII.
    say(table, unsafe { core::str::from_utf8_unchecked(&buffer) });
}

/// **Say which memory the firmware is already using inside a range it refused** (milestone 195).
///
/// `AllocatePages(AllocateAddress)` reports one status and no address, so on its own it cannot tell
/// "your kernel is too big" from "your kernel overlaps a firmware volume". This prints every map
/// descriptor in the range that is not free RAM, with its UEFI type number
/// (`r_efi::efi::CONVENTIONAL_MEMORY` and its siblings). It runs only on the failure path, so its
/// allocation cannot disturb the map key `ExitBootServices` later checks: there is no later.
///
/// # BUGS
/// - **A firmware too broken to report a memory map prints nothing here**: no second source.
pub(crate) fn say_conflict(table: &SystemTable, services: &BootServices, start: u64, end: u64) {
    let Some((bytes, descriptor_size)) = boot_services::memory_map_size(services) else {
        return;
    };
    // Slack for the pool allocation itself, the same reason `MAP_SLACK_DESCRIPTORS` exists.
    let capacity = bytes + MAP_SLACK_DESCRIPTORS * descriptor_size;
    let Some(buffer) = boot_services::allocate_pool(services, capacity) else {
        return;
    };
    // SAFETY: `buffer` is `capacity` bytes from the firmware's pool, used for nothing else.
    if let Some(map) = unsafe { boot_services::memory_map(services, buffer, capacity) } {
        let descriptor_size = map.descriptor_size;
        let got = map.bytes;
        for i in 0..got / descriptor_size {
            // SAFETY: `buffer` holds `got` bytes of descriptors and `i` is inside that count. Read
            // unaligned because the firmware chooses `descriptor_size` and nothing promises the
            // stride keeps 8-byte alignment.
            let descriptor = unsafe {
                ptr::read_unaligned(
                    (buffer as usize + i * descriptor_size) as *const efi::MemoryDescriptor,
                )
            };
            let first = descriptor.physical_start;
            let last = first + descriptor.number_of_pages * PAGE;
            if descriptor.r#type == efi::CONVENTIONAL_MEMORY || last <= start || first >= end {
                continue;
            }
            say_span(table, "uefi_loader:   in the way: ", first, last);
            say(table, "uefi_loader:   memory type ");
            say_hex(table, u64::from(descriptor.r#type));
            say(table, "\r\n");
        }
    }
    // SAFETY: `buffer` came from `allocate_pool` above, and the walk that read it is done.
    unsafe { boot_services::free_pool(services, buffer) };
}

/// Print one ASCII line on the firmware console, if there is one.
///
/// It exists for the person standing at the machine: the `OptiPlex`'s serial port carries the
/// kernel's output but not the firmware's, so without this a loader that failed before
/// `ExitBootServices` would look exactly like a stick the firmware never read. Non-ASCII input is
/// not this loader's problem; every string it prints is a literal in this binary.
pub(crate) fn say(table: &SystemTable, text: &str) {
    if table.con_out.is_null() {
        return;
    }
    // 128 is longer than any message here and keeps this off the heap, of which there is none.
    let mut utf16 = [0u16; 128];
    for (n, byte) in text.bytes().enumerate() {
        if n + 1 >= utf16.len() {
            break;
        }
        utf16[n] = u16::from(byte);
    }
    // SAFETY: `con_out` is non-null and the firmware owns it until `ExitBootServices`; the buffer
    // is NUL-terminated because `n + 1 < len` leaves at least one zero behind.
    // `OutputString` takes `*mut` and only reads it.
    unsafe { ((*table.con_out).output_string)(table.con_out, utf16.as_mut_ptr()) };
}

/// There is nothing to unwind to, and by the time most failures here can happen there is no console
/// left to say anything on, so a panic parks the core.
#[panic_handler]
pub(crate) fn panic(_info: &core::panic::PanicInfo) -> ! {
    arch::park()
}
