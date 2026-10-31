//! **Making and removing directory entries**: files and directories with short or long names, the
//! alias numbered against the names already in the directory, and rename within or across
//! directories.
//!
//! A name that is already a legal upper-case 8.3 name takes one slot. Any other legal name takes a
//! run of long-name slots and then a short entry whose alias is numbered `~1`, `~2` and so on until
//! no entry in the directory holds it. The run and its short entry go down in directory order,
//! the short entry last, so a crash leaves orphan pieces that no reader joins and never a short
//! entry with a wrong name. [`RunFinder`] picks the slots; `verification.rs` proves it first-fit.
//!
//! Name: provisional. The write half of `directory` and `named_entry`.

use crate::boot_sector::FileAllocationTableType;
use crate::directory::SLOT_BYTES;
use crate::file_write::WriteError;
use crate::file_writer::{ARCHIVE, FREE, MOST_SLOTS, short_name_bytes};
use crate::long_name::{self, LongName};
use crate::lookup::LookupError;
use crate::named_entry::{EntryPlace, NamedEntry};
use crate::reader::{ChainError, Volume};
use crate::sector::{SectorSize, WritableSectorDevice};

/// `DIR_Attr`'s directory bit.
const DIRECTORY: u8 = 0x10;

/// What a directory slot is, for choosing where to write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SlotKind {
    /// In use: a short entry or a long-name piece.
    Used,
    /// Deleted: free, with used slots possibly after it.
    Free,
    /// The end marker: this slot and every slot after it is free.
    End,
}

/// **Finds the first run of `count` free slots** as the slots are fed in directory order.
#[derive(Debug, Clone, Copy)]
pub(crate) struct RunFinder {
    count: u32,
    at: u32,
    start: u32,
    len: u32,
}

impl RunFinder {
    /// A finder for `count` slots, which is at least 1.
    pub(crate) const fn new(count: u32) -> RunFinder {
        RunFinder {
            count,
            at: 0,
            start: 0,
            len: 0,
        }
    }

    /// **Take the next slot**, and return where the run starts once it is complete. An end marker
    /// completes the run at once, since every slot after it is free too: the caller makes sure the
    /// directory has the slots.
    pub(crate) fn push(&mut self, kind: SlotKind) -> Option<u32> {
        let index = self.at;
        self.at += 1;
        match kind {
            SlotKind::Used => {
                self.len = 0;
                None
            }
            SlotKind::Free | SlotKind::End => {
                if self.len == 0 {
                    self.start = index;
                }
                self.len += 1;
                (kind == SlotKind::End || self.len == self.count).then_some(self.start)
            }
        }
    }
}

impl<D: WritableSectorDevice> Volume<D> {
    /// The root as 0 for the one place a FAT32 root may be named two ways.
    fn directory_number(&self, directory: u32) -> u32 {
        match self.boot.root_directory() {
            crate::RootDirectory::Cluster(root) if root == directory => 0,
            _ => directory,
        }
    }

    /// Make sure `directory` has a slot `upto - 1`, growing a chain directory as needed.
    fn ensure_slots(&mut self, directory: u32, upto: u32) -> Result<(), WriteError<D::Error>> {
        if upto > MOST_SLOTS {
            return Err(WriteError::DirectoryFull);
        }
        // A run is at most 21 slots, so a few clusters at the smallest size always suffice.
        for _ in 0..=upto {
            if self.slot_location(directory, upto - 1)?.is_some() {
                return Ok(());
            }
            if !self.grow_directory(directory)? {
                return Err(WriteError::DirectoryFull);
            }
        }
        Err(WriteError::DirectoryFull)
    }

    /// The first slot of a run of `count` free slots in `directory`, and whether the run reaches
    /// the directory's end marker.
    fn find_run(
        &mut self,
        directory: u32,
        count: u32,
    ) -> Result<(u32, bool), WriteError<D::Error>> {
        let mut finder = RunFinder::new(count);
        let mut index = 0;
        let mut grown_at = None;
        for _ in 0..2 * MOST_SLOTS {
            if index >= MOST_SLOTS {
                break;
            }
            match self.read_slot(directory, index)? {
                Some(slot) => {
                    let kind = match slot[0] {
                        0 => SlotKind::End,
                        FREE => SlotKind::Free,
                        _ => SlotKind::Used,
                    };
                    if let Some(start) = finder.push(kind) {
                        self.ensure_slots(directory, start + count)?;
                        return Ok((start, kind == SlotKind::End));
                    }
                    index += 1;
                }
                // A directory that grew and still has no slot at `index` is damaged.
                None if grown_at != Some(index) && self.grow_directory(directory)? => {
                    grown_at = Some(index);
                }
                None => break,
            }
        }
        Err(WriteError::DirectoryFull)
    }

    /// Whether any entry of `directory` other than the one at `except` holds the short name.
    fn short_name_taken(
        &mut self,
        directory: u32,
        short: [u8; 11],
        except: Option<EntryPlace>,
    ) -> Result<bool, WriteError<D::Error>> {
        for entry in self.entries(directory) {
            let entry = entry?;
            if entry.short_name() == short && Some(entry.place()) != except {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Refuse `name` when it is not a legal name or an entry other than `except` already has it.
    fn check_new_name(
        &mut self,
        directory: u32,
        name: &str,
        except: Option<EntryPlace>,
    ) -> Result<(), WriteError<D::Error>> {
        if short_name_bytes(name).is_none() && LongName::new(name).is_none() {
            return Err(WriteError::InvalidName);
        }
        for entry in self.entries(directory) {
            let entry = entry?;
            if entry.is_called(name) && Some(entry.place()) != except {
                return Err(WriteError::AlreadyExists);
            }
        }
        Ok(())
    }

    /// The entry whose short slot is `short_slot` in `directory`.
    fn entry_at(
        &mut self,
        directory: u32,
        short_slot: u32,
    ) -> Result<NamedEntry, WriteError<D::Error>> {
        for entry in self.entries(directory) {
            let entry = entry?;
            if entry.place().short_slot == short_slot {
                return Ok(entry);
            }
        }
        Err(WriteError::EntryChanged)
    }

    /// Write an entry called `name` into `directory` and return it as stored.
    fn write_entry(
        &mut self,
        directory: u32,
        name: &str,
        attributes: u8,
        first_cluster: u32,
        size: u32,
        except: Option<EntryPlace>,
    ) -> Result<NamedEntry, WriteError<D::Error>> {
        let directory = self.directory_number(directory);
        self.check_new_name(directory, name, except)?;
        let long = match short_name_bytes(name) {
            Some(_) => None,
            None => Some(LongName::new(name).ok_or(WriteError::InvalidName)?),
        };
        let short = match (&long, short_name_bytes(name)) {
            (_, Some(short)) => short,
            (Some(long), None) => self.unused_alias(directory, long, except)?,
            (None, None) => return Err(WriteError::InvalidName),
        };
        let pieces = long.as_ref().map_or(0, LongName::entries) as u32;
        let (start, reached_end) = self.find_run(directory, pieces + 1)?;
        self.begin_change()?;
        let sum = long_name::checksum(&short);
        for piece in 0..pieces {
            let mut slot = [0u8; SLOT_BYTES];
            if let Some(long) = &long {
                long.entry(piece as usize, sum, &mut slot);
            }
            self.write_slot(directory, start + piece, &slot)?;
        }
        let mut slot = [0u8; SLOT_BYTES];
        slot[..11].copy_from_slice(&short);
        slot[11] = attributes;
        if self.boot.file_allocation_table_type() == FileAllocationTableType::Fat32 {
            slot[20..22].copy_from_slice(&((first_cluster >> 16) as u16).to_le_bytes());
        }
        slot[26..28].copy_from_slice(&(first_cluster as u16).to_le_bytes());
        slot[28..32].copy_from_slice(&size.to_le_bytes());
        let short_slot = start + pieces;
        self.write_slot(directory, short_slot, &slot)?;
        if reached_end
            && let Some(next) = self.read_slot(directory, short_slot + 1)?
            && next[0] != 0
        {
            // Slots after the end marker may hold stale bytes; end the directory after the new one.
            let mut end = next;
            end[0] = 0;
            self.write_slot(directory, short_slot + 1, &end)?;
        }
        self.entry_at(directory, short_slot)
    }

    /// The first alias of `long`, numbered from 1, that no other entry of `directory` holds.
    fn unused_alias(
        &mut self,
        directory: u32,
        long: &LongName,
        except: Option<EntryPlace>,
    ) -> Result<[u8; 11], WriteError<D::Error>> {
        for number in 1..=999_999 {
            let alias = long.alias_numbered(number).ok_or(WriteError::InvalidName)?;
            if !self.short_name_taken(directory, alias, except)? {
                return Ok(alias);
            }
        }
        Err(WriteError::DirectoryFull)
    }

    /// **Create an empty file called `name` in the directory whose first cluster is `directory`**
    /// (0 for the root), and return its entry.
    ///
    /// A name that is a legal upper-case 8.3 name is written as a short entry alone. Any other
    /// legal name (up to 255 UCS-2 units, mixed case, spaces inside, several dots) gets long-name
    /// slots ahead of a short alias numbered against the directory. The entry has the archive
    /// attribute, no clusters, size 0 and zero timestamps. A full directory grows by a cluster
    /// unless it is the fixed root of a FAT12 or FAT16 volume.
    ///
    /// Name: provisional. What a caller asks of a volume to make a file.
    pub fn create_file(
        &mut self,
        directory: u32,
        name: &str,
    ) -> Result<NamedEntry, WriteError<D::Error>> {
        self.write_entry(directory, name, ARCHIVE, 0, 0, None)
    }

    /// **Make an empty directory called `name`** in `directory` (0 for the root), and return its
    /// entry. The new directory holds `.` and `..` in a cluster of its own, zeroed past them.
    ///
    /// Name: provisional. The `mkdir` of the directory tree.
    pub fn make_directory(
        &mut self,
        directory: u32,
        name: &str,
    ) -> Result<NamedEntry, WriteError<D::Error>> {
        let parent = self.directory_number(directory);
        self.check_new_name(parent, name, None)?;
        self.begin_change()?;
        let cluster = self.append_cluster()?.ok_or(WriteError::VolumeFull)?;
        let made = self
            .write_dot_entries(cluster, parent)
            .and_then(|()| self.write_entry(parent, name, DIRECTORY, cluster, 0, None));
        if made.is_err() {
            let _ = self.free_chain(cluster);
        }
        made
    }

    /// Take one free cluster as the head of a new chain.
    fn append_cluster(&mut self) -> Result<Option<u32>, WriteError<D::Error>> {
        Ok(crate::allocation::LinksMut::append(self, None)?)
    }

    /// Fill `cluster` with `.` and `..` (whose cluster is `parent`, 0 for the root) and zeros.
    fn write_dot_entries(&mut self, cluster: u32, parent: u32) -> Result<(), WriteError<D::Error>> {
        let boot = self.boot;
        let first = boot
            .cluster_first_sector(cluster)
            .ok_or(ChainError::NotADataCluster(cluster))?;
        let size = boot.sector_size().bytes();
        let mut sector = [0u8; SectorSize::MAX];
        for (at, (name, target)) in [(b".          ", cluster), (b"..         ", parent)]
            .iter()
            .enumerate()
        {
            let slot = &mut sector[at * SLOT_BYTES..(at + 1) * SLOT_BYTES];
            slot[..11].copy_from_slice(*name);
            slot[11] = DIRECTORY;
            if boot.file_allocation_table_type() == FileAllocationTableType::Fat32 {
                slot[20..22].copy_from_slice(&((*target >> 16) as u16).to_le_bytes());
            }
            slot[26..28].copy_from_slice(&(*target as u16).to_le_bytes());
        }
        crate::file_write::Medium::write_data(self, first, &sector[..size])?;
        let zeros = [0u8; SectorSize::MAX];
        for later in 1..boot.sectors_per_cluster() {
            crate::file_write::Medium::write_data(self, first + later, &zeros[..size])?;
        }
        Ok(())
    }

    /// Check the slots `entry` names still hold it, and return its short slot's bytes.
    fn verify(&mut self, entry: &NamedEntry) -> Result<(), WriteError<D::Error>> {
        let place = entry.place();
        let slot = self
            .read_slot(place.directory, place.short_slot)?
            .ok_or(WriteError::EntryChanged)?;
        if slot[..11] != entry.short_name() || slot[11] & 0x3F == 0x0F {
            return Err(WriteError::EntryChanged);
        }
        Ok(())
    }

    /// Mark every slot of `entry`, its long-name run and its short entry, free.
    fn free_slots(&mut self, entry: &NamedEntry) -> Result<(), WriteError<D::Error>> {
        let place = entry.place();
        for index in place.first_slot..=place.short_slot {
            let mut slot = self
                .read_slot(place.directory, index)?
                .ok_or(WriteError::EntryChanged)?;
            slot[0] = FREE;
            self.write_slot(place.directory, index, &slot)?;
        }
        Ok(())
    }

    /// **Remove the empty directory `entry` names**: mark its slots free, then free its cluster.
    /// Refuses a file, `.` and `..`, and a directory that holds anything else.
    ///
    /// Name: provisional. The `rmdir` of the directory tree.
    pub fn remove_directory(&mut self, entry: &NamedEntry) -> Result<(), WriteError<D::Error>> {
        if !entry.is_directory() || entry.is_dot_entry() {
            return Err(WriteError::NotADirectory);
        }
        self.verify(entry)?;
        for inner in self.entries(entry.first_cluster()) {
            if !inner?.is_dot_entry() {
                return Err(WriteError::DirectoryNotEmpty);
            }
        }
        self.begin_change()?;
        self.free_slots(entry)?;
        self.free_chain(entry.first_cluster())
            .map_err(|_| WriteError::EntryChanged)?;
        Ok(())
    }

    /// Whether `ancestor` is `directory` or one of the directories above it, by `..` links.
    fn is_inside(&mut self, ancestor: u32, directory: u32) -> Result<bool, WriteError<D::Error>> {
        let mut current = self.directory_number(directory);
        for _ in 0..=self.boot.clusters() {
            if current == ancestor {
                return Ok(true);
            }
            if current == 0 {
                return Ok(false);
            }
            let Some(dotdot) = self.read_slot(current, 1)? else {
                return Ok(false);
            };
            let high = if self.boot.file_allocation_table_type() == FileAllocationTableType::Fat32 {
                u32::from(u16::from_le_bytes([dotdot[20], dotdot[21]]))
            } else {
                0
            };
            current = self.directory_number(
                (high << 16) | u32::from(u16::from_le_bytes([dotdot[26], dotdot[27]])),
            );
        }
        Err(WriteError::Chain(ChainError::TooLong))
    }

    /// **Rename `entry`, and move it to `directory`** (0 for the root; its own directory to rename
    /// in place), and return the entry as it now stands. The data does not move: a new entry
    /// points at the same clusters, then the old one is freed. A directory moved to another parent
    /// has its `..` updated. Refuses a name another entry holds, a file or directory moved into
    /// itself or below itself, and `.` and `..`.
    ///
    /// Name: provisional. The `rename` of the directory tree.
    pub fn rename(
        &mut self,
        entry: &NamedEntry,
        directory: u32,
        new_name: &str,
    ) -> Result<NamedEntry, WriteError<D::Error>> {
        if entry.is_dot_entry() || entry.is_volume_label() {
            return Err(WriteError::NotAFile);
        }
        self.verify(entry)?;
        let target = self.directory_number(directory);
        if entry.is_directory() && self.is_inside(entry.first_cluster(), target)? {
            return Err(WriteError::MovedIntoItself);
        }
        let moved = self.write_entry(
            target,
            new_name,
            entry.attributes(),
            entry.first_cluster(),
            entry.size_bytes(),
            Some(entry.place()),
        )?;
        if entry.is_directory() && entry.place().directory != target {
            let first = entry.first_cluster();
            let mut dotdot = self.read_slot(first, 1)?.ok_or(WriteError::EntryChanged)?;
            if self.boot.file_allocation_table_type() == FileAllocationTableType::Fat32 {
                dotdot[20..22].copy_from_slice(&((target >> 16) as u16).to_le_bytes());
            }
            dotdot[26..28].copy_from_slice(&(target as u16).to_le_bytes());
            self.write_slot(first, 1, &dotdot)?;
        }
        self.free_slots(entry)?;
        Ok(moved)
    }
}

/// Keeps the lookup error convertible where the directory tree reports one.
impl<E> From<LookupError<E>> for WriteError<E> {
    fn from(error: LookupError<E>) -> Self {
        match error {
            LookupError::Chain(error) => WriteError::Chain(error),
            LookupError::NotFound | LookupError::NoName => WriteError::EntryChanged,
            LookupError::NotADirectory => WriteError::NotADirectory,
        }
    }
}

#[cfg(kani)]
mod verification;

#[cfg(test)]
mod tests;
