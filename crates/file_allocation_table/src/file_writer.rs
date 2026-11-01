//! **Creating, writing and deleting files** on a volume whose device writes: directory slots read
//! and written in place, a short entry laid down in the first free slot, and [`FileWriter`], which
//! writes bytes, sets a length and keeps the entry's size and first cluster true.
//!
//! Names here are 8.3 short names; a long name arrives with stage 7. The order of every change is
//! the one `write_cursor.rs` states and `allocation.rs` guarantees: a crash leaks clusters and never
//! shares one, and a deleted entry is marked free before its chain is.
//!
//! Name: ratified 2026-11-01 (UTC), the writer's module, beside `file`, which reads.

use crate::allocation::{AllocationError, LinksMut};
use crate::boot_sector::RootDirectory;
use crate::directory::SLOT_BYTES;
use crate::file::FileOpenError;
use crate::long_name::is_short_byte;
use crate::named_entry::{EntryPlace, NamedEntry};
use crate::reader::{ChainError, Volume, Walk};
use crate::sector::{SectorSize, WritableSectorDevice};
use crate::time::{Clock, Stamp};
use crate::write_cursor::{Medium, WriteError, WriteState};

/// `DIR_Attr`'s archive bit: set on a file a writer has changed.
pub(crate) const ARCHIVE: u8 = 0x20;
/// A FAT directory holds at most this many entries.
pub(crate) const MOST_SLOTS: u32 = 65_536;
/// The first name byte of a free slot.
pub(crate) const FREE: u8 = 0xE5;

impl<D: WritableSectorDevice> Medium for Volume<D> {
    fn read_data(&mut self, sector: u32, out: &mut [u8]) -> Result<(), ChainError<D::Error>> {
        self.read_sector(sector, out)
    }

    fn write_data(&mut self, sector: u32, data: &[u8]) -> Result<(), ChainError<D::Error>> {
        self.cached = None;
        self.device
            .write_sector(u64::from(sector), data)
            .map_err(ChainError::Device)
    }
}

/// The 11 bytes of the short name `name` spells, or `None` when it is not a legal one: one to
/// eight base characters, then optionally a dot and one to three extension characters, each an
/// upper-case letter, a digit or the punctuation a short name admits.
pub(crate) fn short_name_bytes(name: &str) -> Option<[u8; 11]> {
    let (base, extension) = match name.split_once('.') {
        Some((base, extension)) if !extension.is_empty() => (base, extension),
        Some(_) => return None,
        None => (name, ""),
    };
    if base.is_empty() || base.len() > 8 || extension.len() > 3 {
        return None;
    }
    let mut bytes = [b' '; 11];
    for (slot, b) in bytes[..8].iter_mut().zip(base.bytes()) {
        *slot = b;
    }
    for (slot, b) in bytes[8..].iter_mut().zip(extension.bytes()) {
        *slot = b;
    }
    let legal =
        bytes.iter().all(|b| is_short_byte(*b)) && !base.contains(' ') && !extension.contains(' ');
    // A first byte of 0xE5 is spelled 0x05 on disk; names here are ASCII, so it cannot occur.
    legal.then_some(bytes)
}

impl<D: WritableSectorDevice> Volume<D> {
    /// The volume-relative sector and byte offset of slot `index` of directory `directory` (0 for
    /// the root), or `None` when the directory has no such slot.
    pub(crate) fn slot_location(
        &mut self,
        directory: u32,
        index: u32,
    ) -> Result<Option<(u32, usize)>, ChainError<D::Error>> {
        let boot = self.boot;
        let sector_bytes = u64::from(boot.sector_size().bytes() as u32);
        let byte = u64::from(index) * SLOT_BYTES as u64;
        let first = match (directory, boot.root_directory()) {
            (
                0,
                RootDirectory::Region {
                    first_sector,
                    sectors,
                    entries,
                },
            ) => {
                let sector = byte / sector_bytes;
                if index >= u32::from(entries) || sector >= u64::from(sectors) {
                    return Ok(None);
                }
                return Ok(Some((
                    first_sector + sector as u32,
                    (byte % sector_bytes) as usize,
                )));
            }
            (0, RootDirectory::Cluster(root)) => root,
            (other, _) => other,
        };
        let wanted = byte / u64::from(boot.bytes_per_cluster());
        let mut walk = Walk::new(first);
        let mut cluster = 0;
        for _ in 0..=wanted {
            match walk.step(self) {
                Some(next) => cluster = next?,
                None => return Ok(None),
            }
        }
        let within = byte % u64::from(boot.bytes_per_cluster());
        let sector = boot
            .cluster_first_sector(cluster)
            .ok_or(ChainError::NotADataCluster(cluster))?
            + (within / sector_bytes) as u32;
        Ok(Some((sector, (within % sector_bytes) as usize)))
    }

    /// Slot `index` of `directory`, or `None` past its end.
    pub(crate) fn read_slot(
        &mut self,
        directory: u32,
        index: u32,
    ) -> Result<Option<[u8; SLOT_BYTES]>, ChainError<D::Error>> {
        let Some((sector, offset)) = self.slot_location(directory, index)? else {
            return Ok(None);
        };
        let size = self.boot.sector_size().bytes();
        let mut buffer = [0u8; SectorSize::MAX];
        self.read_sector(sector, &mut buffer[..size])?;
        let mut slot = [0u8; SLOT_BYTES];
        slot.copy_from_slice(&buffer[offset..offset + SLOT_BYTES]);
        Ok(Some(slot))
    }

    /// Replace slot `index` of `directory` with `slot`, leaving the rest of its sector. Returns
    /// `false` when the directory has no such slot.
    pub(crate) fn write_slot(
        &mut self,
        directory: u32,
        index: u32,
        slot: &[u8; SLOT_BYTES],
    ) -> Result<bool, ChainError<D::Error>> {
        let Some((sector, offset)) = self.slot_location(directory, index)? else {
            return Ok(false);
        };
        self.begin_change()?;
        let size = self.boot.sector_size().bytes();
        let mut buffer = [0u8; SectorSize::MAX];
        self.read_sector(sector, &mut buffer[..size])?;
        buffer[offset..offset + SLOT_BYTES].copy_from_slice(slot);
        self.write_data(sector, &buffer[..size])?;
        Ok(true)
    }

    /// Add a zeroed cluster to a chain directory. `false` for the fixed root region.
    pub(crate) fn grow_directory(&mut self, directory: u32) -> Result<bool, WriteError<D::Error>> {
        let first = match (directory, self.boot.root_directory()) {
            (0, RootDirectory::Region { .. }) => return Ok(false),
            (0, RootDirectory::Cluster(root)) => root,
            (other, _) => other,
        };
        let mut last = first;
        let mut clusters = 0u64;
        for cluster in self.chain(first) {
            last = cluster?;
            clusters += 1;
        }
        // A directory holds at most `MOST_SLOTS` entries, so it never needs more than this.
        let slots_per_cluster = u64::from(self.boot.bytes_per_cluster()) / SLOT_BYTES as u64;
        if clusters * slots_per_cluster >= u64::from(MOST_SLOTS) {
            return Ok(false);
        }
        self.begin_change()?;
        let Some(new) = self.append(Some(last))? else {
            return Err(WriteError::VolumeFull);
        };
        let boot = self.boot;
        let start = boot
            .cluster_first_sector(new)
            .ok_or(ChainError::NotADataCluster(new))?;
        let zeros = [0u8; SectorSize::MAX];
        for sector in 0..boot.sectors_per_cluster() {
            self.write_data(start + sector, &zeros[..boot.sector_size().bytes()])?;
        }
        Ok(true)
    }

    /// Rewrite the first cluster and size of the short entry `name` at `place`, after checking
    /// that the slot still holds that entry.
    pub(crate) fn update_entry(
        &mut self,
        place: EntryPlace,
        name: [u8; 11],
        first_cluster: u32,
        size: u32,
        stamped: bool,
    ) -> Result<(), WriteError<D::Error>> {
        let mut slot = self
            .read_slot(place.directory, place.short_slot)?
            .ok_or(WriteError::EntryChanged)?;
        if slot[..11] != name || slot[11] & 0x3F == 0x0F {
            return Err(WriteError::EntryChanged);
        }
        slot[11] |= ARCHIVE;
        if self.boot.file_allocation_table_type() == crate::FileAllocationTableType::Fat32 {
            slot[20..22].copy_from_slice(&((first_cluster >> 16) as u16).to_le_bytes());
        }
        slot[26..28].copy_from_slice(&(first_cluster as u16).to_le_bytes());
        slot[28..32].copy_from_slice(&size.to_le_bytes());
        if stamped {
            self.stamp.write_modified(&mut slot);
        }
        self.write_slot(place.directory, place.short_slot, &slot)?;
        Ok(())
    }

    /// **Delete the file `entry` names**: mark its slots free, then free its clusters. Refuses a
    /// directory and the volume label.
    ///
    /// Name: ratified 2026-11-01 (UTC), the verb of `std::fs::remove_file`.
    pub(crate) fn remove_file(&mut self, entry: &NamedEntry) -> Result<(), WriteError<D::Error>> {
        if entry.is_directory() || entry.is_volume_label() {
            return Err(WriteError::NotAFile);
        }
        if entry
            .user_attributes()
            .contains(crate::Attributes::READ_ONLY)
        {
            return Err(WriteError::ReadOnly);
        }
        let place = entry.place();
        let slot = self
            .read_slot(place.directory, place.short_slot)?
            .ok_or(WriteError::EntryChanged)?;
        if slot[..11] != entry.short_name() || slot[11] & 0x3F == 0x0F {
            return Err(WriteError::EntryChanged);
        }
        for index in place.first_slot..=place.short_slot {
            let mut slot = self
                .read_slot(place.directory, index)?
                .ok_or(WriteError::EntryChanged)?;
            slot[0] = FREE;
            self.write_slot(place.directory, index, &slot)?;
        }
        if entry.first_cluster() != 0 {
            self.deallocate_chain(entry.first_cluster())
                .map_err(|error| match error {
                    AllocationError::Chain(error) => WriteError::Chain(error),
                    AllocationError::VolumeFull | AllocationError::NotTheEndOfAChain(_) => {
                        WriteError::EntryChanged
                    }
                })?;
        }
        Ok(())
    }
}

/// **A file open for writing**: what [`crate::WritableVolume::open_file_writer`] returns.
///
/// It writes at a position, extends the file as it writes, and sets its length. After every call
/// that changes the size or the first cluster it rewrites the directory entry, so the entry is
/// true between calls; [`crate::WritableVolume::mark_clean`] writes the volume's own hints and clears the dirty bit.
///
/// Name: ratified 2026-11-01 (UTC), a file being written; named for what it does, as `FileReader`.
#[derive(Debug)]
pub struct FileWriter<'v, D, C> {
    volume: &'v mut Volume<D>,
    clock: &'v mut C,
    place: EntryPlace,
    name: [u8; 11],
    read_only: bool,
    state: WriteState,
    failed: bool,
}

impl<'v, D: WritableSectorDevice, C: Clock> FileWriter<'v, D, C> {
    /// Open `entry` for writing at its start, stamping with `clock`. Refuses a directory and the
    /// volume label.
    pub(crate) fn new(
        volume: &'v mut Volume<D>,
        clock: &'v mut C,
        entry: &NamedEntry,
    ) -> Result<FileWriter<'v, D, C>, FileOpenError> {
        if entry.is_directory() || entry.is_volume_label() {
            return Err(FileOpenError);
        }
        Ok(FileWriter {
            volume,
            clock,
            place: entry.place(),
            name: entry.short_name(),
            read_only: entry
                .user_attributes()
                .contains(crate::Attributes::READ_ONLY),
            state: WriteState::new(entry.first_cluster(), entry.size_bytes()),
            failed: false,
        })
    }

    /// Read the clock once for the operation about to run.
    ///
    /// A time FAT cannot store is refused here, before the writer touches the device, and the
    /// writer stays usable.
    fn tick(&mut self) -> Result<(), WriteError<D::Error>> {
        self.volume.stamp =
            Stamp::pack(self.clock.now()).map_err(|_| WriteError::TimeOutOfRange)?;
        Ok(())
    }

    /// The file's length in bytes.
    ///
    /// Name: ratified 2026-11-01 (UTC), as `FileReader::size_bytes`.
    pub const fn size_bytes(&self) -> u32 {
        self.state.size
    }

    /// The position of the next byte written.
    ///
    /// Name: ratified 2026-11-01 (UTC), as `FileReader::position`.
    pub const fn position(&self) -> u32 {
        self.state.position
    }

    /// Move to byte `position`, or to the end of the file when it is past it.
    ///
    /// Name: ratified 2026-11-01 (UTC), renamed from `seek`: the position it sets, as
    /// `FileReader::set_position`.
    pub fn set_position(&mut self, position: u32) {
        self.state.seek(position);
    }

    fn check(&self) -> Result<(), WriteError<D::Error>> {
        if self.failed {
            Err(WriteError::Failed)
        } else if self.read_only {
            Err(WriteError::ReadOnly)
        } else {
            Ok(())
        }
    }

    /// Rewrite the entry, stamped, when bytes were written or the size or the first cluster
    /// differs from `before`.
    fn settle(&mut self, before: (u32, u32), wrote: bool) -> Result<(), WriteError<D::Error>> {
        if wrote || (self.state.first_cluster, self.state.size) != before {
            self.volume.update_entry(
                self.place,
                self.name,
                self.state.first_cluster,
                self.state.size,
                true,
            )?;
        }
        Ok(())
    }

    /// **Write `data` at the current position**, extending the file past its end, and return how
    /// many bytes were written. It writes fewer than `data.len()` only when the volume fills; the
    /// next call then returns `VolumeFull`. An error ends the writer.
    ///
    /// This is not `std::io::Write::write`: the crate is `no_std` and implements no `std::io`
    /// trait, and the error type is [`WriteError`].
    ///
    /// Name: ratified 2026-11-01 (UTC), the `Write` verb of the standard library's trait.
    pub fn write(&mut self, data: &[u8]) -> Result<usize, WriteError<D::Error>> {
        self.check()?;
        self.tick()?;
        let before = (self.state.first_cluster, self.state.size);
        let boot = self.volume.boot;
        let mut scratch = [0u8; SectorSize::MAX];
        self.volume.begin_change()?;
        let written = self
            .state
            .write(self.volume, &boot, data, &mut scratch)
            .inspect_err(|_| self.failed = true);
        // The entry is brought up to date even when the write stopped part way.
        let settled = self.settle(before, !data.is_empty());
        let written = written?;
        settled.inspect_err(|_| self.failed = true)?;
        Ok(written)
    }

    /// **Set the file's length**, as `std::fs::File::set_len` does. A longer length reads as zeros between the old end and the new;
    /// a shorter one frees the clusters past it. The position moves to the new end if it was
    /// past it.
    ///
    /// Name: ratified 2026-11-01 (UTC), renamed from `set_len`: the length spelled out, with
    /// `set_len` kept as the alias a reader of `File::set_len` searches for.
    #[doc(alias = "set_len")]
    pub fn set_length(&mut self, length: u32) -> Result<(), WriteError<D::Error>> {
        self.check()?;
        self.tick()?;
        let result = self.resize(length);
        if result.is_err() {
            self.failed = true;
        }
        result
    }

    fn resize(&mut self, length: u32) -> Result<(), WriteError<D::Error>> {
        let before = (self.state.first_cluster, self.state.size);
        let resume = self.state.position;
        if length > self.state.size {
            let boot = self.volume.boot;
            let zeros = [0u8; SectorSize::MAX];
            let mut scratch = [0u8; SectorSize::MAX];
            self.volume.begin_change()?;
            self.state.seek(u32::MAX);
            let passes = (length - self.state.size) as usize / zeros.len() + 2;
            let mut outcome = Err(WriteError::VolumeFull);
            for _ in 0..passes {
                let step = ((length - self.state.size) as usize).min(zeros.len());
                if step == 0 {
                    outcome = Ok(());
                    break;
                }
                match self
                    .state
                    .write(self.volume, &boot, &zeros[..step], &mut scratch)
                {
                    Ok(0) => break,
                    Ok(_) => {}
                    Err(error) => {
                        outcome = Err(error);
                        break;
                    }
                }
            }
            let settled = self.settle(before, true);
            outcome?;
            settled?;
            self.state.seek(resume);
        } else if length < self.state.size {
            let per_cluster = u64::from(self.volume.boot.bytes_per_cluster());
            let keep = u32::try_from(u64::from(length).div_ceil(per_cluster)).unwrap_or(u32::MAX);
            let first = self.state.first_cluster;
            self.state.size = length;
            self.state.position = self.state.position.min(length);
            if keep == 0 {
                self.state.first_cluster = 0;
            }
            self.state.forget_cursor();
            // The entry shrinks first, so a crash leaks the tail and never shortens the chain
            // under a size that still claims it.
            self.settle(before, false)?;
            if first != 0 {
                self.volume
                    .truncate_chain(first, keep)
                    .map_err(|error| match error {
                        AllocationError::Chain(error) => WriteError::Chain(error),
                        AllocationError::VolumeFull | AllocationError::NotTheEndOfAChain(_) => {
                            WriteError::EntryChanged
                        }
                    })?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
