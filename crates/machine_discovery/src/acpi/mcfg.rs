//! **The MCFG**: where the PCIe ECAM windows are.
//!
//! Name: provisional (milestone 875's lane, 2026-10-12 UTC).

use super::*;

/// One ECAM window: a range of buses on one PCI segment, and the physical address their
/// configuration space is memory-mapped at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct McfgEntry {
    pub base: u64,
    pub segment: u16,
    pub start_bus: u8,
    pub end_bus: u8,
}

impl McfgEntry {
    /// How many bytes of configuration space this window covers. One bus is 1 MiB (32 devices x 8
    /// functions x 4 KiB), and the range is inclusive at both ends.
    ///
    /// **A window that ends before it begins covers nothing, and says so rather than panicking.**
    /// Nothing in the MCFG's encoding stops firmware writing `start_bus = 255, end_bus = 0`. Zero
    /// is the honest answer: there is no bus in the range, so no configuration space to map.
    pub const fn size(&self) -> u64 {
        if self.end_bus < self.start_bus {
            return 0;
        }
        (self.end_bus as u64 - self.start_bus as u64 + 1) * 0x10_0000
    }
}

/// The MCFG's eight reserved bytes before its entry list.
const MCFG_FIXED_LEN: usize = 8;
/// The size of one MCFG allocation entry.
const MCFG_ENTRY_LEN: usize = 16;

/// Decode MCFG entry `index`. `body` begins after the SDT header.
pub fn mcfg_entry(body: &[u8], index: usize) -> Option<McfgEntry> {
    let at = MCFG_FIXED_LEN + index.checked_mul(MCFG_ENTRY_LEN)?;
    if body.len() < at.checked_add(MCFG_ENTRY_LEN)? {
        return None;
    }
    Some(McfgEntry {
        base: u64(body, at),
        segment: u16(body, at + 8),
        start_bus: body[at + 10],
        end_bus: body[at + 11],
    })
}

/// Machine-checked proofs over the MCFG (milestone 319). Why these tables carry harnesses is on
/// `acpi::root`'s `verification`.
///
/// Names: provisional (milestone 319 (the crate that parses firmware had no proofs)). An architect
/// names things.
#[cfg(kani)]
mod verification {
    use super::*;

    /// **An ECAM window's size is total, and counts exactly one mebibyte per bus it covers.** A
    /// window whose end precedes its start answers zero rather than underflowing. The second
    /// clause pins the arithmetic to 32 devices x 8 functions x 4 KiB per bus, inclusive at both
    /// ends, so an off-by-one either way fails it.
    /// Falsification: replayable `crates/machine_discovery/falsifications/acpi.mcfg.verification.an_ecam_windows_size_is_total_and_counts_one_mebibyte_per_bus.patch`
    #[kani::proof]
    fn an_ecam_windows_size_is_total_and_counts_one_mebibyte_per_bus() {
        let start_bus: u8 = kani::any();
        let end_bus: u8 = kani::any();
        let e = McfgEntry {
            base: 0,
            segment: 0,
            start_bus,
            end_bus,
        };
        let size = e.size();
        if start_bus <= end_bus {
            let buses = end_bus as u64 - start_bus as u64 + 1;
            assert_eq!(size, buses * 0x10_0000);
        } else {
            assert_eq!(size, 0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **A window's size counts the buses between its ends, inclusive at both.**
    ///
    /// Three cases, and the single-bus one is the point: equal bus numbers are a window of one
    /// mebibyte rather than an empty one, which is what separates "ends before it begins" from
    /// "ends where it begins". The four-bus case starts away from bus zero, where a sum and a
    /// difference stop agreeing.
    #[test]
    fn an_ecam_window_counts_the_buses_between_its_ends_inclusive() {
        let one = McfgEntry {
            base: 0,
            segment: 0,
            start_bus: 7,
            end_bus: 7,
        };
        assert_eq!(one.size(), 0x10_0000, "one bus is one mebibyte");

        let four = McfgEntry {
            base: 0,
            segment: 0,
            start_bus: 2,
            end_bus: 5,
        };
        assert_eq!(four.size(), 4 * 0x10_0000, "buses 2, 3, 4 and 5");

        let backwards = McfgEntry {
            base: 0,
            segment: 0,
            start_bus: 8,
            end_bus: 7,
        };
        assert_eq!(
            backwards.size(),
            0,
            "nothing in the MCFG's encoding stops firmware writing this",
        );
    }

    /// **A second window, on a segment that is not zero, read from its own offsets.**
    ///
    /// One window at bus 0 on segment 0 hides every offset inside the entry, because each byte a
    /// wrong offset would reach is also zero: the segment can be read from the base's low half and
    /// still answer 0. Distinct values in every field are what make the offsets load-bearing.
    #[test]
    fn a_second_ecam_window_is_read_from_its_own_offsets() {
        let mut body = [0u8; MCFG_FIXED_LEN + 2 * MCFG_ENTRY_LEN];
        body[8..16].copy_from_slice(&0xb000_0000u64.to_le_bytes());
        body[16..18].copy_from_slice(&0x0102u16.to_le_bytes());
        body[18] = 0;
        body[19] = 255;
        let at = MCFG_FIXED_LEN + MCFG_ENTRY_LEN;
        body[at..at + 8].copy_from_slice(&0xc000_0000u64.to_le_bytes());
        body[at + 8..at + 10].copy_from_slice(&0x0304u16.to_le_bytes());
        body[at + 10] = 16;
        body[at + 11] = 31;

        let first = mcfg_entry(&body, 0).expect("two windows");
        assert_eq!(
            first.segment, 0x0102,
            "the segment is its own field, not the base's low half",
        );

        let second = mcfg_entry(&body, 1).expect("two windows");
        assert_eq!(second.base, 0xc000_0000);
        assert_eq!(second.segment, 0x0304);
        assert_eq!(second.start_bus, 16);
        assert_eq!(second.end_bus, 31);
        assert_eq!(second.size(), 16 * 0x10_0000);
        assert_eq!(mcfg_entry(&body, 2), None);
    }

    /// The MCFG entry q35 produces: bus 0 through 255 at 0xb0000000.
    #[test]
    fn the_mcfg_gives_the_ecam_window_and_its_size() {
        let mut body = [0u8; MCFG_FIXED_LEN + MCFG_ENTRY_LEN];
        body[8..16].copy_from_slice(&0xb000_0000u64.to_le_bytes());
        body[16..18].copy_from_slice(&0u16.to_le_bytes());
        body[18] = 0;
        body[19] = 255;
        let e = mcfg_entry(&body, 0).expect("one window");
        assert_eq!(e.base, 0xb000_0000);
        assert_eq!(e.start_bus, 0);
        assert_eq!(e.end_bus, 255);
        assert_eq!(e.size(), 256 * 0x10_0000, "256 buses of 1 MiB each");
        assert_eq!(mcfg_entry(&body, 1), None);
    }
}
