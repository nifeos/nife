//! Machine-checked proofs over the checked read (milestone 423). The parsers beside it are proved
//! in their own modules; this proves the one place firmware's numbers become bytes.
//!
//! Name: provisional, minted 2026-10-27 (UTC) by milestone 423 (a checked direct-map reader for the x86
//! ACPI walk). An architect names things.

use super::*;

/// A region small enough to prove over.
const N: usize = 48;

/// A header and four bytes of body: the checksum loop unwinds once per byte, so this is the
/// cost knob of the table proof.
const TABLE_N: usize = SDT_HEADER_LEN + 4;

/// **No read leaves the region.** For every region base, address and length, including the
/// values that wrap, a read that succeeds returns exactly `len` bytes lying inside the bytes the
/// region holds, at the offset `at - first`. This is the property every firmware number in the
/// walk relies on, and it holds for the full `u64` range of each input.
/// Falsification: replayable `crates/machine_discovery/falsifications/acpi.region.verification.no_read_leaves_the_region.patch`
#[kani::proof]
fn no_read_leaves_the_region() {
    let buffer: [u8; N] = kani::any();
    let first: u64 = kani::any();
    let at: u64 = kani::any();
    let len: u64 = kani::any();
    let region = DirectMapRegion::from_bytes(first, &buffer);
    if let Ok(read) = region.read(at, len) {
        assert!(at != 0);
        assert!(at >= first);
        assert_eq!(read.len() as u64, len);
        let offset = at - first;
        assert!(offset + len <= N as u64);
        assert_eq!(read.as_ptr(), buffer[offset as usize..].as_ptr());
        kani::cover!(len > 0, "a non-empty read is accepted");
    }
}

/// **No table length reaches past the region.** A table's `length` is a number the table wrote,
/// and the whole-table read and the checksum run over it. For every table bytes and every base,
/// a table that is returned has a body exactly as long as the length its header states less the
/// header, and that length fits inside the region from the table's address.
/// Falsification: replayable `crates/machine_discovery/falsifications/acpi.region.verification.no_table_length_reaches_past_the_region.patch`
#[kani::proof]
#[kani::unwind(42)]
fn no_table_length_reaches_past_the_region() {
    let buffer: [u8; TABLE_N] = kani::any();
    let first: u64 = kani::any();
    let at: u64 = kani::any();
    let region = DirectMapRegion::from_bytes(first, &buffer);
    if let Ok(table) = region.table(at) {
        let length = table.header.length as u64;
        assert!(length >= SDT_HEADER_LEN as u64);
        assert_eq!(table.body.len() as u64 + SDT_HEADER_LEN as u64, length);
        assert!(at >= first);
        assert!(at - first + length <= TABLE_N as u64);
        kani::cover!(!table.body.is_empty(), "a table with a body is accepted");
    }
}
