//! **A parsed and checked GUID partition table**: the header and entry array a caller supplies,
//! checked against each other and against the backup before anything is trusted.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

/// A parsed or freshly built GUID Partition Table.
///
/// Borrows the entry array rather than copying it: 16 KiB is too much to move around a kernel, and
/// the caller already has the bytes.
#[derive(Clone, Copy, Debug)]
pub struct GloballyUniqueIdentifierPartitionTable<'a> {
    header: Header,
    /// Exactly `entry_count * entry_size` bytes, the span the array CRC covers. Trimmed on the way
    /// in so nothing downstream has to remember to.
    entries: &'a [u8],
    block_size: usize,
}

impl<'a> GloballyUniqueIdentifierPartitionTable<'a> {
    /// Parse a primary GPT.
    ///
    /// `header_block` is the block read from LBA 1, and its length is the logical block size.
    /// `entry_array` is read from the header's `PartitionEntryLBA` and may run past the array.
    /// Refuses a bad header ([`Header::decode`]), a bad array CRC, a table that intrudes on the
    /// usable range, a used partition outside it, and two partitions that overlap. The backup is
    /// a second read, left to [`GloballyUniqueIdentifierPartitionTable::check_backup`].
    ///
    /// # Examples
    ///
    /// ```
    /// # use globally_unique_identifier_partition_table::GloballyUniqueIdentifierPartitionTable;
    /// # use globally_unique_identifier_partition_table::guid::types;
    /// # let disk = globally_unique_identifier_partition_table::testing::sample_disk();
    /// # let (header_block, entry_array) = (&disk[512..1024], &disk[1024..1024 + 16384]);
    /// let table = GloballyUniqueIdentifierPartitionTable::parse(header_block, entry_array)?;
    /// for (index, part) in table.partitions() {
    ///     let mut label = [0u8; 4 * globally_unique_identifier_partition_table::entry::NAME_UNITS];
    ///     let n = part.name_utf8(&mut label)?;
    ///     println!(
    ///         "{}: {} blocks of {}, {:?}",
    ///         index + 1,
    ///         part.blocks().unwrap(),
    ///         types::name(part.type_guid).unwrap_or("unknown"),
    ///         core::str::from_utf8(&label[..n]).unwrap(),
    ///     );
    /// }
    /// # Ok::<(), globally_unique_identifier_partition_table::Error>(())
    /// ```
    pub fn parse(
        header_block: &[u8],
        entry_array: &'a [u8],
    ) -> Result<GloballyUniqueIdentifierPartitionTable<'a>, Error> {
        let block_size = header_block.len();
        let header = Header::decode(header_block)?;

        if header.my_lba != PRIMARY_HEADER_LBA {
            return Err(Error::NotPrimary {
                my_lba: header.my_lba,
            });
        }

        let entries = check_entry_array(&header, entry_array)?;
        let array_blocks = array_blocks(&header, block_size)?;

        // The disk's size, as the primary header understands it: the backup header is on the last
        // block, so there is one more block than its address.
        let block_count = header
            .alternate_lba
            .checked_add(1)
            .ok_or(Error::TableOverlapsUsable)?;

        if header.first_usable_lba > header.last_usable_lba {
            return Err(Error::UsableRange {
                first: header.first_usable_lba,
                last: header.last_usable_lba,
            });
        }
        // The primary table (MBR, header, array) must end before the usable range starts, and the
        // usable range must end before the backup array starts. Written as additions on the low
        // side and subtractions guarded by comparison on the high side, because a hostile header
        // can put any u64 in any of these fields.
        let primary_end = header
            .entry_array_lba
            .checked_add(array_blocks)
            .ok_or(Error::TableOverlapsUsable)?;
        let backup_reserved = array_blocks
            .checked_add(1)
            .ok_or(Error::TableOverlapsUsable)?;
        // `>=`, not `>`: `block_count - backup_reserved` IS the backup array's first block (what
        // `backup_entry_lba` computes), so equality puts a usable block inside the backup array.
        // See notes/mutation-testing.md.
        if header.entry_array_lba <= PRIMARY_HEADER_LBA
            || primary_end > header.first_usable_lba
            || block_count < backup_reserved
            || header.last_usable_lba >= block_count - backup_reserved
        {
            return Err(Error::TableOverlapsUsable);
        }

        check_partitions(entries, header.entry_size as usize, &header)?;

        Ok(GloballyUniqueIdentifierPartitionTable {
            header,
            entries,
            block_size,
        })
    }

    /// Build a new table. The `mkpart` half of the milestone.
    ///
    /// `entry_array` must be a whole number of entries, and its size sets
    /// `NumberOfPartitionEntries`: pass `[0u8; `[`ENTRY_ARRAY_BYTES`]`]` for the usual 128. It is
    /// overwritten completely, so no entry of a previous table comes back. The partitions are
    /// checked as [`GloballyUniqueIdentifierPartitionTable::parse`] checks a disk's, and placing
    /// them (the 1 MiB alignment convention included) is the caller's policy.
    ///
    /// # Examples
    ///
    /// ```
    /// # use globally_unique_identifier_partition_table::Entry;
    /// # use globally_unique_identifier_partition_table::GloballyUniqueIdentifierPartitionTable;
    /// # use globally_unique_identifier_partition_table::guid::types;
    /// # use uuid::Uuid;
    /// # let disk_guid = Uuid::nil();
    /// # let part_guid = Uuid::nil();
    /// let mut array = [0u8; globally_unique_identifier_partition_table::ENTRY_ARRAY_BYTES];
    /// let table = GloballyUniqueIdentifierPartitionTable::create(
    ///     disk_guid,
    ///     512,
    ///     131_072, // a 64 MiB disk
    ///     &[Entry::new(types::NIFE_DATA, part_guid, 2048, 131_038).with_name("nife data")?],
    ///     &mut array,
    /// )?;
    ///
    /// let mut block = [0u8; 512];
    /// table.write_protective_mbr(&mut block)?; // to LBA 0
    /// table.write_primary_header(&mut block)?; // to LBA 1
    /// table.write_backup_header(&mut block)?;  // to table.backup_header_lba()
    /// let bytes = table.entry_array();          // to LBA 2 and to table.backup_entry_lba()
    /// # Ok::<(), globally_unique_identifier_partition_table::Error>(())
    /// ```
    pub fn create(
        disk_guid: Uuid,
        block_size: usize,
        block_count: u64,
        partitions: &[Entry],
        entry_array: &'a mut [u8],
    ) -> Result<GloballyUniqueIdentifierPartitionTable<'a>, Error> {
        if !is_block_size_ok(block_size) {
            return Err(Error::BlockSize(block_size));
        }
        if entry_array.len() < entry::SIZE || !entry_array.len().is_multiple_of(entry::SIZE) {
            return Err(Error::EntryArrayShape {
                have: entry_array.len(),
            });
        }
        let entry_count = (entry_array.len() / entry::SIZE) as u32;
        if partitions.len() > entry_count as usize {
            return Err(Error::TooManyPartitions {
                given: partitions.len(),
                room: entry_count,
            });
        }

        let array_bytes = entry_array.len() as u64;
        let array_blocks = array_bytes.div_ceil(block_size as u64);
        // LBA 0 is the protective MBR, LBA 1 the header, then the array; the backup is the array
        // again and then the header, at the very end. Everything between is usable.
        let first_usable = PRIMARY_HEADER_LBA + 1 + array_blocks;
        // The smallest disk this table fits on: the primary table, one usable block, the backup
        // array and the backup header.
        let need = first_usable + array_blocks + 2;
        if block_count < need {
            return Err(Error::DiskTooSmall {
                blocks: block_count,
                need,
            });
        }
        let last_usable = block_count - array_blocks - 2;

        entry_array.fill(0);
        for (i, part) in partitions.iter().enumerate() {
            entry_array[i * entry::SIZE..(i + 1) * entry::SIZE].copy_from_slice(&part.encode());
        }

        let header = Header {
            my_lba: PRIMARY_HEADER_LBA,
            alternate_lba: block_count - 1,
            first_usable_lba: first_usable,
            last_usable_lba: last_usable,
            disk_guid,
            entry_array_lba: PRIMARY_HEADER_LBA + 1,
            entry_count,
            entry_size: entry::SIZE_U32,
            entry_array_crc: crc32(entry_array),
        };

        check_partitions(entry_array, entry::SIZE, &header)?;

        Ok(GloballyUniqueIdentifierPartitionTable {
            header,
            entries: entry_array,
            block_size,
        })
    }

    /// Check the backup copy against this one.
    ///
    /// `backup_block` is the last block of the disk; `backup_entries` is read from
    /// [`GloballyUniqueIdentifierPartitionTable::backup_entry_lba`]. Every field the two headers
    /// must share is compared, and the two arrays are compared by CRC.
    ///
    /// The CRC is what the format itself binds a header to its array with, so arrays that CRC
    /// alike and differ would be a CRC-32 collision; and the caller need not hold both 16 KiB
    /// arrays at once.
    pub fn check_backup(&self, backup_block: &[u8], backup_entries: &[u8]) -> Result<(), Error> {
        let backup = Header::decode(backup_block)?;

        let mismatch = |m: BackupMismatch| Err(Error::Backup(m));
        if backup.my_lba != self.header.alternate_lba {
            return mismatch(BackupMismatch::MyLba);
        }
        if backup.alternate_lba != self.header.my_lba {
            return mismatch(BackupMismatch::AlternateLba);
        }
        if backup.disk_guid != self.header.disk_guid {
            return mismatch(BackupMismatch::DiskGuid);
        }
        if backup.first_usable_lba != self.header.first_usable_lba {
            return mismatch(BackupMismatch::FirstUsableLba);
        }
        if backup.last_usable_lba != self.header.last_usable_lba {
            return mismatch(BackupMismatch::LastUsableLba);
        }
        if backup.entry_count != self.header.entry_count {
            return mismatch(BackupMismatch::EntryCount);
        }
        if backup.entry_size != self.header.entry_size {
            return mismatch(BackupMismatch::EntrySize);
        }
        if backup.entry_array_crc != self.header.entry_array_crc {
            return mismatch(BackupMismatch::EntryArrayCrc);
        }
        if backup.entry_array_lba != self.backup_entry_lba() {
            return mismatch(BackupMismatch::EntryArrayLba);
        }

        // Validates the bytes the caller actually read, which is the point: the field comparison
        // above only proves the two headers agree, not that the backup array on the disk is intact.
        check_entry_array(&backup, backup_entries)?;
        Ok(())
    }

    /// The decoded primary header.
    pub fn header(&self) -> &Header {
        &self.header
    }

    /// The GUID naming this disk.
    pub fn disk_guid(&self) -> Uuid {
        self.header.disk_guid
    }

    /// The logical block size, taken from the length of the header block.
    pub fn block_size(&self) -> usize {
        self.block_size
    }

    /// How many blocks the disk has, according to the header (`alternate_lba + 1`).
    pub fn block_count(&self) -> u64 {
        self.header.alternate_lba + 1
    }

    /// First block a partition may occupy.
    pub fn first_usable_lba(&self) -> u64 {
        self.header.first_usable_lba
    }

    /// Last block a partition may occupy, inclusive.
    pub fn last_usable_lba(&self) -> u64 {
        self.header.last_usable_lba
    }

    /// Where the backup header goes: the last block of the disk.
    pub fn backup_header_lba(&self) -> u64 {
        self.header.alternate_lba
    }

    /// Where the primary entry array goes.
    pub fn primary_entry_lba(&self) -> u64 {
        self.header.entry_array_lba
    }

    /// Where the backup entry array goes: immediately below the backup header.
    pub fn backup_entry_lba(&self) -> u64 {
        self.header.alternate_lba - self.entry_array_blocks()
    }

    /// How many blocks the entry array occupies. 32 for the usual 16 KiB array on a 512-byte disk,
    /// 4 on a 4K-native one.
    pub fn entry_array_blocks(&self) -> u64 {
        (self.entries.len() as u64).div_ceil(self.block_size as u64)
    }

    /// How many entries the array holds, used or not.
    pub fn entry_count(&self) -> u32 {
        self.header.entry_count
    }

    /// The entry array bytes, exactly `entry_count * entry_size` of them: what to write at
    /// [`GloballyUniqueIdentifierPartitionTable::primary_entry_lba`] and again at
    /// [`GloballyUniqueIdentifierPartitionTable::backup_entry_lba`]. Both copies are the same
    /// bytes, which is why there is one accessor.
    pub fn entry_array(&self) -> &'a [u8] {
        self.entries
    }

    /// Entry `index`, used or not. `None` past the end of the array.
    pub fn entry(&self, index: usize) -> Option<Entry> {
        let start = index.checked_mul(self.header.entry_size as usize)?;
        let end = start.checked_add(entry::SIZE)?;
        let bytes: &[u8; entry::SIZE] = self.entries.get(start..end)?.try_into().ok()?;
        Some(Entry::decode(bytes))
    }

    /// The used partitions, with their zero-based index in the array.
    ///
    /// The index matters and is not just a counter: it is what `sgdisk` and `parted` print as the
    /// partition *number* (plus one), and what Linux turns into `/dev/sda1`. A table may have a
    /// hole, an unused entry 1 between used entries 0 and 2, and the numbering survives it.
    pub fn partitions(&self) -> impl Iterator<Item = (usize, Entry)> + '_ {
        (0..self.header.entry_count as usize)
            .filter_map(|i| self.entry(i).map(|e| (i, e)))
            .filter(|(_, e)| e.is_used())
    }

    /// Check the block read from LBA 0 is a protective MBR for this disk.
    ///
    /// Separate from [`GloballyUniqueIdentifierPartitionTable::parse`] for the same reason
    /// [`GloballyUniqueIdentifierPartitionTable::check_backup`] is: it is a different block, and a
    /// caller that has only read LBA 1 and the array should not be forced to read another. See
    /// [`mbr`] for what "protective" means and why a hybrid MBR is refused.
    pub fn check_protective_mbr(&self, block: &[u8]) -> Result<(), Error> {
        mbr::validate(block, self.block_count())
    }

    /// Write the protective MBR for this disk into `block` (destined for LBA 0).
    pub fn write_protective_mbr(&self, block: &mut [u8]) -> Result<(), Error> {
        mbr::write(block, self.block_count())
    }

    /// Write the primary header into `block` (destined for LBA 1).
    pub fn write_primary_header(&self, block: &mut [u8]) -> Result<(), Error> {
        self.header.encode_into(block)
    }

    /// Write the backup header into `block` (destined for
    /// [`GloballyUniqueIdentifierPartitionTable::backup_header_lba`]).
    ///
    /// The same header with the two LBA fields swapped and the array pointer moved. Everything else
    /// is identical, which is what [`GloballyUniqueIdentifierPartitionTable::check_backup`] relies
    /// on.
    pub fn write_backup_header(&self, block: &mut [u8]) -> Result<(), Error> {
        Header {
            my_lba: self.header.alternate_lba,
            alternate_lba: self.header.my_lba,
            entry_array_lba: self.backup_entry_lba(),
            ..self.header
        }
        .encode_into(block)
    }
}

/// Trim the caller's buffer to the array the header describes, and check its CRC.
///
/// Shared by [`GloballyUniqueIdentifierPartitionTable::parse`] and
/// [`GloballyUniqueIdentifierPartitionTable::check_backup`] so that the backup's array is held to
/// the same standard as the primary's, which is the whole point of having a backup.
pub(crate) fn check_entry_array<'a>(
    header: &Header,
    supplied: &'a [u8],
) -> Result<&'a [u8], Error> {
    if header.entry_size < entry::SIZE_U32 || !header.entry_size.is_multiple_of(8) {
        return Err(Error::EntrySize(header.entry_size));
    }
    if header.entry_count == 0 {
        return Err(Error::EntryCount(header.entry_count));
    }
    let need = header
        .entry_array_bytes()
        .ok_or(Error::EntryCount(header.entry_count))?;
    let entries = supplied.get(..need).ok_or(Error::EntryArrayLen {
        need,
        have: supplied.len(),
    })?;

    let computed = crc32(entries);
    if computed != header.entry_array_crc {
        return Err(Error::EntryArrayCrc {
            stored: header.entry_array_crc,
            computed,
        });
    }
    Ok(entries)
}

/// How many blocks the entry array occupies, from a header and a block size.
pub(crate) fn array_blocks(header: &Header, block_size: usize) -> Result<u64, Error> {
    let bytes = header
        .entry_array_bytes()
        .ok_or(Error::EntryCount(header.entry_count))? as u64;
    Ok(bytes.div_ceil(block_size as u64))
}

/// Every rule about where partitions may sit, applied to an entry array.
///
/// The overlap check is the quadratic one and it is deliberately not clever: 128 entries is 8,128
/// comparisons, which is nothing, and a sort would need a scratch buffer this crate does not have.
pub(crate) fn check_partitions(
    entries: &[u8],
    entry_size: usize,
    header: &Header,
) -> Result<(), Error> {
    let count = entries.len() / entry_size;
    let at = |i: usize| -> Entry {
        let start = i * entry_size;
        Entry::decode(entries[start..start + entry::SIZE].try_into().unwrap())
    };

    for i in 0..count {
        let e = at(i);
        if !e.is_used() {
            continue;
        }
        if e.last_lba < e.first_lba {
            return Err(Error::PartitionRange { index: i });
        }
        if e.first_lba < header.first_usable_lba || e.last_lba > header.last_usable_lba {
            return Err(Error::PartitionOutsideUsable { index: i });
        }
        for j in (i + 1)..count {
            if e.overlaps(&at(j)) {
                return Err(Error::PartitionsOverlap { a: i, b: j });
            }
        }
    }
    Ok(())
}
