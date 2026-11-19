//! Kani harnesses: the outbound window encoder and the root port's bus and window words, for every
//! input rather than the handful the tests name.
//!
//! Name: provisional, minted 2026-11-19 (UTC) by lane milestone/163-jh7110-pcie-root-complex.

use crate::{Window, root_port, xpressrich};

/// Any accepted window encodes an enabled entry whose size field decodes back to the size, whose
/// source address keeps its 4 KiB granule, and whose words stay inside the entry's 32 bytes.
#[kani::proof]
fn outbound_window_round_trips_its_size() {
    let index: u32 = kani::any();
    let processor_address: u64 = kani::any();
    let pci_address: u64 = kani::any();
    let size_bytes: u64 = kani::any();
    if let Some(entry) =
        xpressrich::outbound_window(index, processor_address, pci_address, size_bytes)
    {
        assert!(index < xpressrich::OUTBOUND_ENTRIES);
        let base = index * xpressrich::TRANSLATION_ENTRY_BYTES;
        for (offset, _) in entry {
            assert!(offset >= base + xpressrich::OUTBOUND_SOURCE_ADDRESS_AND_SIZE);
            assert!(
                offset
                    < base
                        + xpressrich::OUTBOUND_SOURCE_ADDRESS_AND_SIZE
                        + xpressrich::TRANSLATION_ENTRY_BYTES
            );
        }
        let source = entry[1].1;
        assert_eq!(xpressrich::translated_size_bytes(source), Some(size_bytes));
        assert_eq!(source & 0xfff & !(xpressrich::TRANSLATION_SIZE_MASK | 1), 0);
        assert_eq!(source & !0xfff, (processor_address as u32) & !0xfff);
        assert_eq!(entry[2].1, (processor_address >> 32) as u32);
        assert_eq!(
            entry[0].1 == xpressrich::INTERFACE_CONFIGURATION,
            index == 0
        );
    }
}

/// Every power of two from 4 KiB up is accepted at every index.
#[kani::proof]
fn outbound_window_accepts_every_power_of_two_size() {
    let index: u32 = kani::any();
    let shift: u32 = kani::any();
    kani::assume(index < xpressrich::OUTBOUND_ENTRIES);
    kani::assume((12..64).contains(&shift));
    assert!(xpressrich::outbound_window(index, kani::any(), kani::any(), 1u64 << shift).is_some());
}

/// The bus numbers word always reads primary 0, secondary 1, subordinate 1, and keeps the latency
/// byte.
#[kani::proof]
fn bus_numbers_forward_bus_one_only() {
    let before: u32 = kani::any();
    let word = root_port::bus_numbers(before);
    assert_eq!(word & 0xff, 0);
    assert_eq!((word >> 8) & 0xff, u32::from(root_port::SECONDARY_BUS));
    assert_eq!((word >> 16) & 0xff, u32::from(root_port::SECONDARY_BUS));
    assert_eq!(word >> 24, before >> 24);
}

/// An accepted memory window decodes, as a bridge reads the word, to exactly that window.
#[kani::proof]
fn memory_base_and_limit_forwards_exactly_the_window() {
    let window = Window {
        physical_address: kani::any(),
        size_bytes: kani::any(),
    };
    if let Some(word) = root_port::memory_base_and_limit(window) {
        let base = u64::from(word & 0xfff0) << 16;
        let limit = (u64::from(word >> 16 & 0xfff0) << 16) | 0xf_ffff;
        assert_eq!(base, window.physical_address);
        assert_eq!(limit + 1, window.physical_address + window.size_bytes);
    }
}
