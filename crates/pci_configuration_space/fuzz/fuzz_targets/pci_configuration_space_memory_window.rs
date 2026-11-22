//! Fuzz `memory_window_32_bit` over any `ranges` bytes.
//!
//! The parse must not panic, and a window it returns must be one entry of the property as the
//! bytes state it: a 32-bit, non-prefetchable memory entry whose PCI and processor addresses are
//! equal and whose size is not zero, the first such entry in the property. A length that is not a
//! whole number of seven-cell entries returns `None`.

#![no_main]

use libfuzzer_sys::fuzz_target;
use pci_configuration_space::memory_window_32_bit;

/// One `ranges` entry: three PCI address cells, two processor address cells, two size cells.
const ENTRY_BYTES: usize = 7 * 4;

fuzz_target!(|ranges: &[u8]| {
    let window = memory_window_32_bit(ranges);
    if ranges.is_empty() || !ranges.len().is_multiple_of(ENTRY_BYTES) {
        assert_eq!(window, None);
        return;
    }

    let cell = |entry: &[u8], index: usize| -> u64 {
        u64::from(u32::from_be_bytes(
            entry[index * 4..index * 4 + 4].try_into().unwrap(),
        ))
    };
    let expected = ranges
        .as_chunks::<ENTRY_BYTES>()
        .0
        .iter()
        .find_map(|entry| {
            let high = cell(entry, 0);
            let pci_address = (cell(entry, 1) << 32) | cell(entry, 2);
            let processor_address = (cell(entry, 3) << 32) | cell(entry, 4);
            let size = (cell(entry, 5) << 32) | cell(entry, 6);
            let qualifies = (high >> 24) & 0b11 == 0b10
                && high & (1 << 30) == 0
                && pci_address == processor_address
                && size != 0;
            qualifies.then_some((processor_address, size))
        });
    assert_eq!(window, expected);
});
