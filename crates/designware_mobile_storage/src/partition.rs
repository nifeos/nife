//! Where on a card a write cannot hurt: the MBR, and the gap before its first partition. radon
//! boots from its microSD card, one FAT32 partition
//! in an MBR, so the only sectors this crate names for a write are before the first partition and
//! after sector 0: the alignment gap no filesystem owns and U-Boot does not read when the board
//! boots from its QSPI flash, as radon does. A card with no MBR signature, a protective MBR (GPT,
//! whose header and table live in that gap), no partitions, or a first partition at sector 1 gets
//! no scratch range, and the bench probe refuses to write and says why. A Kani harness (`proofs` in
//! the crate root) proves the range never touches a partition or sector 0, for every table.

/// The MBR's last two bytes on a valid table.
pub const SIGNATURE: [u8; 2] = [0x55, 0xaa];
/// The partition type a GPT disk's protective MBR carries.
pub(crate) const TYPE_PROTECTIVE_GPT: u8 = 0xee;

/// One of the four primary partition entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Entry {
    /// The type byte; 0 is an unused entry.
    pub kind: u8,
    /// First sector.
    pub start: u32,
    /// Length in sectors.
    pub sectors: u32,
}

impl Entry {
    /// Is this entry a partition at all? Unused entries are type 0 or zero length.
    #[must_use]
    pub const fn is_used(&self) -> bool {
        self.kind != 0 && self.sectors != 0
    }

    /// Does this partition contain `sector`? Computed in 64 bits, so an entry whose end passes
    /// 2^32 still contains what it claims to.
    #[must_use]
    pub const fn contains(&self, sector: u64) -> bool {
        self.is_used()
            && sector >= self.start as u64
            && sector < self.start as u64 + self.sectors as u64
    }
}

/// A parsed MBR: the four primary entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mbr {
    /// Entries 0 to 3, as on disk.
    pub entries: [Entry; 4],
}

impl Mbr {
    /// **Parse sector 0**, or `None` without the signature.
    ///
    /// ```
    /// use designware_mobile_storage::partition::Mbr;
    ///
    /// let mut s = [0u8; 512];
    /// s[446 + 4] = 0x0b; // FAT32
    /// s[446 + 8..446 + 12].copy_from_slice(&2048u32.to_le_bytes());
    /// s[446 + 12..446 + 16].copy_from_slice(&61_000_000u32.to_le_bytes());
    /// s[510] = 0x55;
    /// s[511] = 0xaa;
    /// let mbr = Mbr::parse(&s).unwrap();
    /// assert_eq!(mbr.entries[0].start, 2048);
    /// // Eight sectors ending just before the partition.
    /// assert_eq!(mbr.scratch(8), Some((2040, 8)));
    /// ```
    #[must_use]
    pub fn parse(sector: &[u8; 512]) -> Option<Mbr> {
        if sector[510..512] != SIGNATURE {
            return None;
        }
        let mut entries = [Entry::default(); 4];
        for (i, e) in entries.iter_mut().enumerate() {
            let at = 446 + 16 * i;
            let le = |o: usize| {
                u32::from_le_bytes([
                    sector[at + o],
                    sector[at + o + 1],
                    sector[at + o + 2],
                    sector[at + o + 3],
                ])
            };
            *e = Entry {
                kind: sector[at + 4],
                start: le(8),
                sectors: le(12),
            };
        }
        Some(Mbr { entries })
    }

    /// The first used partition's start, or `None` when nothing is partitioned.
    #[must_use]
    pub(crate) fn first_start(&self) -> Option<u32> {
        self.entries
            .iter()
            .filter(|e| e.is_used())
            .map(|e| e.start)
            .min()
    }

    /// **Up to `want` sectors that belong to nothing**: `(first, count)`, ending at the sector
    /// before the first partition, starting no lower than sector 1, `count` between 1 and `want`.
    /// `None` for a protective MBR, a table with no partitions, a first partition at sector 0 or
    /// 1, or `want` of 0.
    ///
    /// # Examples
    ///
    /// A write test on radon's boot card may only land before the first partition:
    ///
    /// ```
    /// use designware_mobile_storage::partition::{Entry, Mbr};
    ///
    /// let fat = Entry { kind: 0x0b, start: 2048, sectors: 1 << 20 };
    /// let mbr = Mbr { entries: [fat, Entry::default(), Entry::default(), Entry::default()] };
    /// assert_eq!(mbr.scratch(8), Some((2040, 8)));
    /// ```
    #[must_use]
    pub fn scratch(&self, want: u32) -> Option<(u32, u32)> {
        if want == 0 || self.entries.iter().any(|e| e.kind == TYPE_PROTECTIVE_GPT) {
            return None;
        }
        let first = self.first_start()?;
        // The gap is [1, first); empty when first is 0 or 1.
        let gap = first.checked_sub(1)?;
        if gap == 0 {
            return None;
        }
        let count = if want < gap { want } else { gap };
        Some((first - count, count))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sector(entries: &[(u8, u32, u32)]) -> [u8; 512] {
        let mut s = [0u8; 512];
        for (i, &(kind, start, len)) in entries.iter().enumerate() {
            let at = 446 + 16 * i;
            s[at + 4] = kind;
            s[at + 8..at + 12].copy_from_slice(&start.to_le_bytes());
            s[at + 12..at + 16].copy_from_slice(&len.to_le_bytes());
        }
        s[510] = 0x55;
        s[511] = 0xaa;
        s
    }

    #[test]
    fn no_signature_no_table() {
        let mut s = sector(&[(0x0b, 2048, 100)]);
        s[511] = 0;
        assert_eq!(Mbr::parse(&s), None);
    }

    #[test]
    fn the_scratch_range_ends_before_the_lowest_partition_whatever_its_slot() {
        let mbr = Mbr::parse(&sector(&[(0x83, 1_000_000, 10), (0x0b, 2048, 900_000)])).unwrap();
        assert_eq!(mbr.scratch(8), Some((2040, 8)));
        // A gap smaller than the ask is used whole, and never reaches sector 0.
        let mbr = Mbr::parse(&sector(&[(0x0b, 4, 100)])).unwrap();
        assert_eq!(mbr.scratch(8), Some((1, 3)));
    }

    #[test]
    fn a_gpt_disk_or_a_partition_at_sector_one_has_no_scratch() {
        let gpt = Mbr::parse(&sector(&[(TYPE_PROTECTIVE_GPT, 1, u32::MAX)])).unwrap();
        assert_eq!(gpt.scratch(1), None);
        let tight = Mbr::parse(&sector(&[(0x0b, 1, 100)])).unwrap();
        assert_eq!(tight.scratch(1), None);
        let empty = Mbr::parse(&sector(&[])).unwrap();
        assert_eq!(empty.scratch(1), None);
        let mbr = Mbr::parse(&sector(&[(0x0b, 2048, 100)])).unwrap();
        assert_eq!(mbr.scratch(0), None);
    }

    #[test]
    fn unused_entries_with_a_start_do_not_count() {
        // Type 0 with a start of 16 is an unused slot, not a partition at 16.
        let mbr = Mbr::parse(&sector(&[(0, 16, 100), (0x0b, 2048, 100)])).unwrap();
        assert_eq!(mbr.scratch(4), Some((2044, 4)));
    }

    /// Every byte of the start and length words is read, little-endian, from its own entry: the
    /// CHS bytes between them are not.
    #[test]
    fn an_entry_is_read_from_all_four_bytes_of_each_word() {
        let mut s = sector(&[(0, 0, 0), (0x83, 0x1234_5678, 0x0abc_def0)]);
        // CHS end address bytes (offsets 5 to 7) set, so a misplaced read cannot pass by zeros.
        s[446 + 16 + 5..446 + 16 + 8].copy_from_slice(&[0xfe, 0xff, 0xff]);
        let e = Mbr::parse(&s).unwrap().entries[1];
        assert_eq!(
            (e.kind, e.start, e.sectors),
            (0x83, 0x1234_5678, 0x0abc_def0)
        );
    }

    #[test]
    fn a_partition_contains_its_own_sectors_and_no_others() {
        let e = Entry {
            kind: 0x0b,
            start: 100,
            sectors: 50,
        };
        assert!(!e.contains(99));
        assert!(e.contains(100));
        assert!(e.contains(149));
        assert!(!e.contains(150));
        // An unused slot contains nothing, whatever its start says.
        assert!(!Entry { kind: 0, ..e }.contains(120));
        // The end is computed in 64 bits, so an entry crossing 2^32 still contains its tail.
        let high = Entry {
            kind: 0x83,
            start: u32::MAX - 1,
            sectors: 10,
        };
        assert!(high.contains(u64::from(u32::MAX) + 5));
        assert!(!high.contains(u64::from(u32::MAX) + 9));
    }
}
