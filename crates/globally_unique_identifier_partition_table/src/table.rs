//! **A parsed and checked GUID partition table**: the header and entry array a caller supplies,
//! checked against each other and against the backup before anything is trusted.
//!
//! Name: ratified 2026-11-20 (#3864).

use super::*;

/// A parsed or freshly built GUID Partition Table.
///
/// Borrows the entry array rather than copying it: 16 KiB is too much to move around a kernel, and
/// the caller already has the bytes.
///
/// Name: ratified 2026-11-20 (#3864).
#[derive(Clone, Copy)]
pub struct PartitionTable<'a> {
    header: Header,
    /// Exactly `entry_count * entry_bytes` bytes, the span the array CRC covers. Trimmed on the way
    /// in so nothing downstream has to remember to.
    entries: &'a [u8],
    block_bytes: usize,
}

/// The header, the block size and how many entries are in use: the 16 KiB array stays out of a
/// `{:?}`, which would otherwise fill a console or a kernel log.
impl core::fmt::Debug for PartitionTable<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("PartitionTable")
            .field("header", &self.header)
            .field("block_bytes", &self.block_bytes)
            .field("used_entries", &self.partitions().count())
            .finish_non_exhaustive()
    }
}

impl<'a> PartitionTable<'a> {
    /// Parse a primary GPT.
    ///
    /// `header_block` is the block read from LBA 1, and its length is the logical block size.
    /// `entry_array` is read from the header's `PartitionEntryLBA` and may run past the array. That
    /// field is not always block 2: [`Header::decode`] the block from LBA 1 first, read
    /// [`Header::entry_array_logical_block_address`] and as many blocks as the array needs, then call `parse`.
    /// Bytes read from the wrong block fail as [`Error::EntryArrayCrc`] or
    /// [`Error::TableOverlapsUsable`].
    ///
    /// `parse` takes the disk's size from the header (`alternate_logical_block_address + 1`) and never sees the
    /// device. After it succeeds a caller runs three more checks: [`Self::check_disk_size`]
    /// against the device, [`Self::check_backup`] against the last block, and
    /// [`Self::check_protective_master_boot_record`] against LBA 0.
    ///
    /// Refuses a bad header ([`Header::decode`]), a bad array CRC, a table that intrudes on the
    /// usable range, a used partition outside it, and two partitions that overlap. The backup is
    /// a second read, left to [`PartitionTable::check_backup`].
    ///
    /// # Examples
    ///
    /// ```
    /// # use globally_unique_identifier_partition_table::PartitionTable;
    /// # use globally_unique_identifier_partition_table::guid::types;
    /// # let disk = globally_unique_identifier_partition_table::testing::sample_disk();
    /// # let (header_block, entry_array) = (&disk[512..1024], &disk[1024..1024 + 16384]);
    /// let table = PartitionTable::parse(header_block, entry_array)?;
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
    pub fn parse(header_block: &[u8], entry_array: &'a [u8]) -> Result<PartitionTable<'a>, Error> {
        let block_bytes = header_block.len();
        let header = Header::decode(header_block)?;

        if header.my_logical_block_address != PRIMARY_HEADER_LOGICAL_BLOCK_ADDRESS {
            return Err(Error::NotPrimary {
                my_logical_block_address: header.my_logical_block_address,
            });
        }

        Self::checked(header, entry_array, block_bytes)
    }

    /// Parse the backup GPT of a disk whose primary is damaged, as UEFI 2.10 section 5.3.2
    /// requires a reader to be able to.
    ///
    /// `backup_block` is the last block of the disk, `backup_entries` is read from the backup
    /// header's `PartitionEntryLBA`, and `block_count` is the device's size. The header must say
    /// it lives at the last block and points back at LBA 1, and its array must lie after the last
    /// usable block and end before it. The table returned is normalized to the primary's
    /// orientation (`MyLBA` 1, `AlternateLBA` the last block, the array at block 2), so
    /// [`Self::write_primary_header`] and [`Self::entry_array`] restore the primary, and
    /// [`Self::check_backup`] against the real primary says whether a restore is needed. The
    /// normalization assumes the primary array sits at block 2, which every tool writes and the
    /// specification does not require.
    ///
    /// Name: ratified 2026-11-20 (#3864).
    pub fn parse_backup(
        backup_block: &[u8],
        backup_entries: &'a [u8],
        block_count: u64,
    ) -> Result<PartitionTable<'a>, Error> {
        let block_bytes = backup_block.len();
        let backup = Header::decode(backup_block)?;
        let last = block_count
            .checked_sub(1)
            .ok_or(Error::TableOverlapsUsable)?;
        if backup.my_logical_block_address != last {
            return Err(Error::NotBackup {
                claimed_block: backup.my_logical_block_address,
            });
        }
        if backup.alternate_logical_block_address != PRIMARY_HEADER_LOGICAL_BLOCK_ADDRESS {
            return Err(Error::Backup(BackupMismatch::AlternateLogicalBlockAddress));
        }
        check_entry_array(&backup, backup_entries)?;
        let array_blocks = array_blocks(&backup, block_bytes)?;
        let array_end = backup
            .entry_array_logical_block_address
            .checked_add(array_blocks)
            .ok_or(Error::TableOverlapsUsable)?;
        if backup.entry_array_logical_block_address <= backup.last_usable_logical_block_address
            || array_end > backup.my_logical_block_address
        {
            return Err(Error::TableOverlapsUsable);
        }
        Self::checked(
            primary_from_backup(backup, last),
            backup_entries,
            block_bytes,
        )
    }

    /// The geometry and partition rules every table meets, whichever copy it was read from.
    fn checked(
        header: Header,
        entry_array: &'a [u8],
        block_bytes: usize,
    ) -> Result<PartitionTable<'a>, Error> {
        let entries = check_entry_array(&header, entry_array)?;
        let array_blocks = array_blocks(&header, block_bytes)?;

        // The disk's size, as the primary header understands it: the backup header is on the last
        // block, so there is one more block than its address.
        let block_count = header
            .alternate_logical_block_address
            .checked_add(1)
            .ok_or(Error::TableOverlapsUsable)?;

        if header.first_usable_logical_block_address > header.last_usable_logical_block_address {
            return Err(Error::UsableRange {
                first: header.first_usable_logical_block_address,
                last: header.last_usable_logical_block_address,
            });
        }
        // The primary table (MBR, header, array) must end before the usable range starts, and the
        // usable range must end before the backup array starts. Written as additions on the low
        // side and subtractions guarded by comparison on the high side, because a hostile header
        // can put any u64 in any of these fields.
        let primary_end = header
            .entry_array_logical_block_address
            .checked_add(array_blocks)
            .ok_or(Error::TableOverlapsUsable)?;
        let backup_reserved = array_blocks
            .checked_add(1)
            .ok_or(Error::TableOverlapsUsable)?;
        // `>=`, not `>`: `block_count - backup_reserved` IS the backup array's first block (what
        // `backup_entry_array_logical_block_address` computes), so equality puts a usable block inside the backup array.
        if header.entry_array_logical_block_address <= PRIMARY_HEADER_LOGICAL_BLOCK_ADDRESS
            || primary_end > header.first_usable_logical_block_address
            || block_count < backup_reserved
            || header.last_usable_logical_block_address >= block_count - backup_reserved
        {
            return Err(Error::TableOverlapsUsable);
        }

        check_partitions(entries, header.entry_bytes as usize, &header)?;

        Ok(PartitionTable {
            header,
            entries,
            block_bytes,
        })
    }

    /// Build a new table.
    ///
    /// `entry_array` must be a whole number of entries, and its size sets
    /// `NumberOfPartitionEntries`: pass `[0u8; `[`ENTRY_ARRAY_BYTES`]`]` for the usual 128. It is
    /// overwritten completely, so no entry of a previous table comes back. The partitions are
    /// checked as [`PartitionTable::parse`] checks a disk's, and placing
    /// them (the 1 MiB alignment convention included) is the caller's policy.
    ///
    /// # Examples
    ///
    /// ```
    /// # use globally_unique_identifier_partition_table::Entry;
    /// # use globally_unique_identifier_partition_table::PartitionTable;
    /// # use globally_unique_identifier_partition_table::guid::types;
    /// # use uuid::Uuid;
    /// # let disk_guid = Uuid::nil();
    /// # let part_guid = Uuid::nil();
    /// let mut array = [0u8; globally_unique_identifier_partition_table::ENTRY_ARRAY_BYTES];
    /// let table = PartitionTable::create(
    ///     disk_guid,
    ///     512,
    ///     131_072, // a 64 MiB disk
    ///     &[Entry::new(types::LINUX_FILESYSTEM, part_guid, 2048, 131_038).with_name("data")?],
    ///     &mut array,
    /// )?;
    ///
    /// let mut block = [0u8; 512];
    /// table.write_protective_master_boot_record(&mut block)?; // to LBA 0
    /// table.write_primary_header(&mut block)?; // to LBA 1
    /// table.write_backup_header(&mut block)?;  // to table.backup_header_logical_block_address()
    /// let bytes = table.entry_array();          // to LBA 2 and to table.backup_entry_array_logical_block_address()
    /// # Ok::<(), globally_unique_identifier_partition_table::Error>(())
    /// ```
    pub fn create(
        disk_guid: Uuid,
        block_bytes: usize,
        block_count: u64,
        partitions: &[Entry],
        entry_array: &'a mut [u8],
    ) -> Result<PartitionTable<'a>, Error> {
        if !is_block_size_ok(block_bytes) {
            return Err(Error::BlockSize(block_bytes));
        }
        if entry_array.len() < entry::ENTRY_BYTES
            || !entry_array.len().is_multiple_of(entry::ENTRY_BYTES)
        {
            return Err(Error::EntryArrayShape {
                have: entry_array.len(),
            });
        }
        let entry_count = u32::try_from(entry_array.len() / entry::ENTRY_BYTES).map_err(|_| {
            Error::EntryArrayShape {
                have: entry_array.len(),
            }
        })?;
        if partitions.len() > entry_count as usize {
            return Err(Error::TooManyPartitions {
                given: partitions.len(),
                room: entry_count,
            });
        }

        let array_bytes = entry_array.len() as u64;
        let array_blocks = array_bytes.div_ceil(block_bytes as u64);
        // LBA 0 is the protective MBR, LBA 1 the header, then the array; the backup is the array
        // again and then the header, at the very end. Everything between is usable.
        let first_usable = PRIMARY_HEADER_LOGICAL_BLOCK_ADDRESS + 1 + array_blocks;
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
            entry_array[i * entry::ENTRY_BYTES..(i + 1) * entry::ENTRY_BYTES]
                .copy_from_slice(&part.encode());
        }

        let header = Header {
            my_logical_block_address: PRIMARY_HEADER_LOGICAL_BLOCK_ADDRESS,
            alternate_logical_block_address: block_count - 1,
            first_usable_logical_block_address: first_usable,
            last_usable_logical_block_address: last_usable,
            disk_guid,
            entry_array_logical_block_address: PRIMARY_HEADER_LOGICAL_BLOCK_ADDRESS + 1,
            entry_count,
            entry_bytes: entry::ENTRY_BYTES_U32,
            entry_array_crc: crc32(entry_array),
        };

        check_partitions(entry_array, entry::ENTRY_BYTES, &header)?;

        Ok(PartitionTable {
            header,
            entries: entry_array,
            block_bytes,
        })
    }

    /// Check the backup copy against this one.
    ///
    /// `backup_block` is the last block of the disk; `backup_entries` is read from
    /// [`PartitionTable::backup_entry_array_logical_block_address`]. Every field the two headers
    /// must share is compared, and the two arrays are compared by CRC.
    ///
    /// This is stricter than UEFI 2.10 section 5.3.1, which lets the backup array sit anywhere after
    /// the last usable block and before the backup header: here it must end exactly at the backup
    /// header, which is where every tool this crate was tested against writes it.
    ///
    /// The CRC is what the format itself binds a header to its array with, so arrays that CRC
    /// alike and differ would be a CRC-32 collision; and the caller need not hold both 16 KiB
    /// arrays at once.
    pub fn check_backup(&self, backup_block: &[u8], backup_entries: &[u8]) -> Result<(), Error> {
        let backup = Header::decode(backup_block)?;

        let mismatch = |m: BackupMismatch| Err(Error::Backup(m));
        if backup.my_logical_block_address != self.header.alternate_logical_block_address {
            return mismatch(BackupMismatch::MyLogicalBlockAddress);
        }
        if backup.alternate_logical_block_address != self.header.my_logical_block_address {
            return mismatch(BackupMismatch::AlternateLogicalBlockAddress);
        }
        if backup.disk_guid != self.header.disk_guid {
            return mismatch(BackupMismatch::DiskGuid);
        }
        if backup.first_usable_logical_block_address
            != self.header.first_usable_logical_block_address
        {
            return mismatch(BackupMismatch::FirstUsableLogicalBlockAddress);
        }
        if backup.last_usable_logical_block_address != self.header.last_usable_logical_block_address
        {
            return mismatch(BackupMismatch::LastUsableLogicalBlockAddress);
        }
        if backup.entry_count != self.header.entry_count {
            return mismatch(BackupMismatch::EntryCount);
        }
        if backup.entry_bytes != self.header.entry_bytes {
            return mismatch(BackupMismatch::EntryBytes);
        }
        if backup.entry_array_crc != self.header.entry_array_crc {
            return mismatch(BackupMismatch::EntryArrayCrc);
        }
        if backup.entry_array_logical_block_address
            != self.backup_entry_array_logical_block_address()
        {
            return mismatch(BackupMismatch::EntryArrayLogicalBlockAddress);
        }

        // Validates the bytes the caller actually read, which is the point: the field comparison
        // above only proves the two headers agree, not that the backup array on the disk is intact.
        check_entry_array(&backup, backup_entries)?;
        Ok(())
    }

    /// Check the device is the size the header says it is.
    ///
    /// `device_blocks` is the device's own block count. Equality is required: a table cloned onto
    /// a smaller or larger disk, or a disk grown under its table, is a resize for a tool to
    /// perform, and not a table to accept. [`PartitionTable::parse`] cannot
    /// make this check because it never sees the device.
    ///
    /// Name: ratified 2026-11-20 (#3864).
    pub fn check_disk_size(&self, device_blocks: u64) -> Result<(), Error> {
        if self.block_count() == device_blocks {
            Ok(())
        } else {
            Err(Error::DiskSize {
                header: self.block_count(),
                device: device_blocks,
            })
        }
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
    ///
    /// Name: ratified 2026-11-20 (#3864).
    pub fn block_bytes(&self) -> usize {
        self.block_bytes
    }

    /// How many blocks the disk has, according to the header (`alternate_logical_block_address + 1`).
    pub fn block_count(&self) -> u64 {
        self.header.alternate_logical_block_address + 1
    }

    /// First block a partition may occupy.
    ///
    /// Name: ratified 2026-11-20 (#3864).
    pub fn first_usable_logical_block_address(&self) -> u64 {
        self.header.first_usable_logical_block_address
    }

    /// Last block a partition may occupy, inclusive.
    ///
    /// Name: ratified 2026-11-20 (#3864).
    pub fn last_usable_logical_block_address(&self) -> u64 {
        self.header.last_usable_logical_block_address
    }

    /// Where the backup header goes: the last block of the disk.
    ///
    /// Name: ratified 2026-11-20 (#3864).
    pub fn backup_header_logical_block_address(&self) -> u64 {
        self.header.alternate_logical_block_address
    }

    /// Where the primary entry array goes.
    ///
    /// Name: ratified 2026-11-20 (#3864).
    pub fn primary_entry_array_logical_block_address(&self) -> u64 {
        self.header.entry_array_logical_block_address
    }

    /// Where the backup entry array goes: immediately below the backup header.
    ///
    /// Name: ratified 2026-11-20 (#3864).
    pub fn backup_entry_array_logical_block_address(&self) -> u64 {
        self.header.alternate_logical_block_address - self.entry_array_blocks()
    }

    /// How many blocks the entry array occupies. 32 for the usual 16 KiB array on a 512-byte disk,
    /// 4 on a 4K-native one.
    pub fn entry_array_blocks(&self) -> u64 {
        (self.entries.len() as u64).div_ceil(self.block_bytes as u64)
    }

    /// How many entries the array holds, used or not.
    pub fn entry_count(&self) -> u32 {
        self.header.entry_count
    }

    /// The entry array bytes, exactly `entry_count * entry_bytes` of them: what to write at
    /// [`PartitionTable::primary_entry_array_logical_block_address`] and again at
    /// [`PartitionTable::backup_entry_array_logical_block_address`]. Both copies are the same
    /// bytes, which is why there is one accessor.
    pub fn entry_array(&self) -> &'a [u8] {
        self.entries
    }

    /// Entry `index`, used or not. `None` past the end of the array.
    ///
    /// Name: ratified 2026-11-20 (#3864). `get` is the standard library's word for a checked lookup
    /// by index.
    pub fn get(&self, index: usize) -> Option<Entry> {
        let start = index.checked_mul(self.header.entry_bytes as usize)?;
        let end = start.checked_add(entry::ENTRY_BYTES)?;
        let bytes: &[u8; entry::ENTRY_BYTES] = self.entries.get(start..end)?.try_into().ok()?;
        Some(Entry::decode(bytes))
    }

    /// The used partitions, with their zero-based index in the array.
    ///
    /// The index matters and is not just a counter: it is what `sgdisk` and `parted` print as the
    /// partition *number* (plus one), and what Linux turns into `/dev/sda1`. A table may have a
    /// hole, an unused entry 1 between used entries 0 and 2, and the numbering survives it.
    pub fn partitions(&self) -> impl Iterator<Item = (usize, Entry)> + '_ {
        (0..self.header.entry_count as usize)
            .filter_map(|i| self.get(i).map(|e| (i, e)))
            .filter(|(_, e)| e.is_used())
    }

    /// Check the block read from LBA 0 is a protective MBR for this disk.
    ///
    /// Separate from [`PartitionTable::parse`] for the same reason
    /// [`PartitionTable::check_backup`] is: it is a different block, and a
    /// caller that has only read LBA 1 and the array should not be forced to read another. See
    /// [`crate::master_boot_record`] for what "protective" means and why a hybrid MBR is refused.
    ///
    /// Name: ratified 2026-11-20 (#3864).
    #[doc(alias = "MBR")]
    pub fn check_protective_master_boot_record(&self, block: &[u8]) -> Result<(), Error> {
        master_boot_record::validate(block, self.block_count())
    }

    /// Write the protective MBR for this disk into `block` (destined for LBA 0).
    ///
    /// Name: ratified 2026-11-20 (#3864).
    #[doc(alias = "MBR")]
    pub fn write_protective_master_boot_record(&self, block: &mut [u8]) -> Result<(), Error> {
        master_boot_record::write(block, self.block_count())
    }

    /// Write the primary header into `block` (destined for LBA 1).
    pub fn write_primary_header(&self, block: &mut [u8]) -> Result<(), Error> {
        self.header.encode_into(block)
    }

    /// Write the backup header into `block` (destined for
    /// [`PartitionTable::backup_header_logical_block_address`]).
    ///
    /// The same header with the two LBA fields swapped and the array pointer moved. Everything else
    /// is identical, which is what [`PartitionTable::check_backup`] relies
    /// on.
    pub fn write_backup_header(&self, block: &mut [u8]) -> Result<(), Error> {
        self.backup_header().encode_into(block)
    }

    /// The header [`PartitionTable::write_backup_header`] encodes. Split out so that
    /// `verification::a_backup_header_restores_the_table_it_was_written_from` proves the field
    /// mapping on symbolic disk sizes without a CRC, which `encode_into` adds and the header round
    /// trip proves separately.
    pub(crate) fn backup_header(&self) -> Header {
        Header {
            my_logical_block_address: self.header.alternate_logical_block_address,
            alternate_logical_block_address: self.header.my_logical_block_address,
            entry_array_logical_block_address: self.backup_entry_array_logical_block_address(),
            ..self.header
        }
    }
}

/// The primary header a backup header at `last` describes, in the orientation
/// [`PartitionTable::parse_backup`] documents: `MyLBA` 1, `AlternateLBA` the last block, the array at
/// block 2.
pub(crate) fn primary_from_backup(backup: Header, last: u64) -> Header {
    Header {
        my_logical_block_address: PRIMARY_HEADER_LOGICAL_BLOCK_ADDRESS,
        alternate_logical_block_address: last,
        entry_array_logical_block_address: PRIMARY_HEADER_LOGICAL_BLOCK_ADDRESS + 1,
        ..backup
    }
}

/// Trim the caller's buffer to the array the header describes, and check its CRC.
///
/// Shared by [`PartitionTable::parse`] and
/// [`PartitionTable::check_backup`] so that the backup's array is held to
/// the same standard as the primary's, which is the whole point of having a backup.
pub(crate) fn check_entry_array<'a>(
    header: &Header,
    supplied: &'a [u8],
) -> Result<&'a [u8], Error> {
    if header.entry_bytes < entry::ENTRY_BYTES_U32 || !header.entry_bytes.is_multiple_of(8) {
        return Err(Error::EntryBytes(header.entry_bytes));
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
pub(crate) fn array_blocks(header: &Header, block_bytes: usize) -> Result<u64, Error> {
    let bytes = header
        .entry_array_bytes()
        .ok_or(Error::EntryCount(header.entry_count))? as u64;
    Ok(bytes.div_ceil(block_bytes as u64))
}

/// Every rule about where partitions may sit, applied to an entry array.
///
/// The overlap check is the quadratic one and it is deliberately not clever: 128 entries is 8,128
/// comparisons, which is nothing, and a sort would need a scratch buffer this crate does not have.
pub(crate) fn check_partitions(
    entries: &[u8],
    entry_bytes: usize,
    header: &Header,
) -> Result<(), Error> {
    let count = entries.len() / entry_bytes;
    let at = |i: usize| -> Entry {
        let start = i * entry_bytes;
        Entry::decode(
            entries[start..start + entry::ENTRY_BYTES]
                .try_into()
                .unwrap(),
        )
    };

    for i in 0..count {
        let e = at(i);
        if !e.is_used() {
            continue;
        }
        if e.last_logical_block_address < e.first_logical_block_address {
            return Err(Error::PartitionRange { index: i });
        }
        if e.first_logical_block_address < header.first_usable_logical_block_address
            || e.last_logical_block_address > header.last_usable_logical_block_address
        {
            return Err(Error::PartitionOutsideUsable { index: i });
        }
        for j in (i + 1)..count {
            if e.overlaps(&at(j)) {
                return Err(Error::PartitionsOverlap {
                    first: i,
                    second: j,
                });
            }
        }
    }
    Ok(())
}
