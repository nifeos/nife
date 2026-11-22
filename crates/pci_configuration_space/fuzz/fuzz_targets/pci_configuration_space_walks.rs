//! Fuzz the walks over an arbitrary configuration space: `walk`, `read_bars`,
//! `virtual_io_capabilities`, `msi_capability` and `msix_capability`.
//!
//! The input is a small machine. Its first byte picks the bus ceiling (1 to 4); the rest fills the
//! 256-byte configuration header of each of 32 functions (buses 0 to 3, devices 0 to 3, functions 0
//! and 1), in that order, padded with zeros. Every other function, and every offset past 255, reads
//! `0xffff_ffff`, the answer of a slot nobody occupies. A write stores the value over the dword's
//! writable bits, and the writable bits of a dword are the bits of the same dword of the next
//! function's header, so the write-ones probe reads back whatever mask the input chose.
//!
//! The walks must not panic, and each answer must keep the rules the crate states: the walk reports
//! each function once and never reads a bus at or above the ceiling; `read_bars` writes nothing but
//! the BAR slots of the header it sizes, restores every one, and returns only power-of-two sizes of
//! at least 16 whose end does not overflow; the capability walks stop within their bound and return
//! offsets inside the header that are multiples of four.

#![no_main]

use std::cell::RefCell;
use std::collections::HashSet;

use libfuzzer_sys::fuzz_target;
use pci_configuration_space::{
    BAR0_REGISTER, BusDeviceFunction, msi_capability, msix_capability, read_bars,
    virtual_io_capabilities, walk,
};

const BUSES: usize = 4;
const DEVICES: usize = 4;
const FUNCTIONS: usize = 2;
const PAGE_BYTES: usize = 256;
const PAGES: usize = BUSES * DEVICES * FUNCTIONS;

/// Where a function's header sits among the input's pages, or `None` for a function the input does
/// not describe.
fn page(bus_device_function: BusDeviceFunction) -> Option<usize> {
    let (bus, device, function) = (
        usize::from(bus_device_function.bus),
        usize::from(bus_device_function.device),
        usize::from(bus_device_function.function),
    );
    (bus < BUSES && device < DEVICES && function < FUNCTIONS)
        .then_some((bus * DEVICES + device) * FUNCTIONS + function)
}

/// A configuration space over the input's bytes, with a record of every read and write.
struct Space {
    bytes: Vec<u8>,
    reads: usize,
    buses_read: HashSet<u8>,
    writes: Vec<(BusDeviceFunction, u64)>,
}

impl Space {
    fn dword(&self, at: usize) -> u32 {
        u32::from_le_bytes(self.bytes[at..at + 4].try_into().unwrap())
    }

    fn read32(&mut self, bus_device_function: BusDeviceFunction, offset: u64) -> u32 {
        self.reads += 1;
        self.buses_read.insert(bus_device_function.bus);
        match page(bus_device_function) {
            Some(page) if offset < PAGE_BYTES as u64 => {
                self.dword(page * PAGE_BYTES + (offset as usize & !3))
            }
            _ => u32::MAX,
        }
    }

    fn write32(&mut self, bus_device_function: BusDeviceFunction, offset: u64, value: u32) {
        self.writes.push((bus_device_function, offset));
        let Some(page) = page(bus_device_function) else {
            return;
        };
        if offset >= PAGE_BYTES as u64 {
            return;
        }
        let at = page * PAGE_BYTES + (offset as usize & !3);
        let writable = self.dword(((page + 1) % PAGES) * PAGE_BYTES + (offset as usize & !3));
        let stored = (self.dword(at) & !writable) | (value & writable);
        self.bytes[at..at + 4].copy_from_slice(&stored.to_le_bytes());
    }
}

fuzz_target!(|data: &[u8]| {
    let Some((&first, data)) = data.split_first() else {
        return;
    };
    let ceiling = u16::from(first % BUSES as u8) + 1;
    let mut bytes = vec![0u8; PAGES * PAGE_BYTES];
    let length = data.len().min(bytes.len());
    bytes[..length].copy_from_slice(&data[..length]);
    let space = RefCell::new(Space {
        bytes,
        reads: 0,
        buses_read: HashSet::new(),
        writes: Vec::new(),
    });
    let read32 = &mut |b: BusDeviceFunction, o: u64| space.borrow_mut().read32(b, o);

    let mut found = Vec::new();
    walk(ceiling, read32, &mut |function| found.push(function));
    let mut seen = HashSet::new();
    for function in &found {
        let bus_device_function = function.bus_device_function;
        assert!(u16::from(bus_device_function.bus) < ceiling);
        assert!(bus_device_function.device < 32 && bus_device_function.function < 8);
        assert!(
            seen.insert((
                bus_device_function.bus,
                bus_device_function.device,
                bus_device_function.function,
            )),
            "a function was reported twice"
        );
    }
    assert!(
        space
            .borrow()
            .buses_read
            .iter()
            .all(|&bus| u16::from(bus) < ceiling),
        "the walk read a bus at or above the ceiling"
    );

    for function in found {
        let bus_device_function = function.bus_device_function;
        let before = space.borrow().bytes.clone();
        space.borrow_mut().writes.clear();
        let bars = read_bars(
            bus_device_function,
            &mut |b, o| space.borrow_mut().read32(b, o),
            &mut |b, o, v| space.borrow_mut().write32(b, o, v),
            &mut |_, _| {},
        );
        for (b, o) in &space.borrow().writes {
            assert_eq!(*b, bus_device_function);
            assert!((BAR0_REGISTER..BAR0_REGISTER + 24).contains(o));
        }
        assert!(
            space.borrow().bytes == before,
            "read_bars left a BAR changed"
        );
        for bar in bars.iter().flatten() {
            assert!(bar.size_bytes.is_power_of_two() && bar.size_bytes >= 16);
            assert!(
                bar.base_physical_address
                    .checked_add(bar.size_bytes)
                    .is_some()
            );
        }

        space.borrow_mut().reads = 0;
        let mut capabilities = 0usize;
        virtual_io_capabilities(
            bus_device_function,
            &mut |b, o| space.borrow_mut().read32(b, o),
            &mut |_| capabilities += 1,
        );
        assert!(capabilities <= 64);
        if let Some(capability) = msi_capability(bus_device_function, &mut |b, o| {
            space.borrow_mut().read32(b, o)
        }) {
            let offset = capability.capability_offset;
            assert!(offset != 0 && offset % 4 == 0 && offset < PAGE_BYTES as u64);
        }
        if let Some(capability) = msix_capability(bus_device_function, &mut |b, o| {
            space.borrow_mut().read32(b, o)
        }) {
            let offset = capability.capability_offset;
            assert!(offset != 0 && offset % 4 == 0 && offset < PAGE_BYTES as u64);
            assert!((1..=2048).contains(&capability.table_entries));
        }
        // Each walk reads the status and the pointer, then at most 64 entries of at most five
        // dwords each.
        assert!(space.borrow().reads <= 3 * (2 + 64 * 5));
    }
});
