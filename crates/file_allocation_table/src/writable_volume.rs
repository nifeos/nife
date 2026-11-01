//! **A volume open for writing**: a [`Volume`] and the [`Clock`] its writes stamp with.
//!
//! The clock is required: a read-only [`Volume`] has no write methods, and the only way to the
//! write methods is a `WritableVolume`, which cannot exist without a clock. A caller with no time
//! source passes [`NoClock`](crate::NoClock) and says so. A `WritableVolume` derefs to its
//! `Volume`, so every read (lookup, listing, file reads, the label, the clean bit) works on it
//! unchanged. A read stamps nothing, so no read takes the clock and no access date changes on a
//! read.
//!
//! The clock is read once per operation, and every entry the operation writes carries that one
//! reading. A new file or directory is created, written and accessed at that time. A write or a
//! resize sets the write time, the write date and the access date. A rename keeps the entry's
//! times. Attribute and label changes stamp nothing except a label's first entry.
//!
//! Name: provisional. The writing half of `Volume`, whose clock the ruling of 2026-11-01 (UTC)
//! makes required.

use core::ops::{Deref, DerefMut};

use crate::allocation::AllocationError;
use crate::attributes::Attributes;
use crate::file::NotAFile;
use crate::file_write::WriteError;
use crate::file_writer::FileWriter;
use crate::named_entry::NamedEntry;
use crate::reader::{ChainError, Volume};
use crate::sector::WritableSectorDevice;
use crate::time::{Clock, Stamp};

/// **A volume open for writing.**
///
/// # Examples
///
/// ```
/// use file_allocation_table::{
///     Format, FormatOptions, NoClock, SectorDevice, SectorSize, WritableSectorDevice,
///     WritableVolume,
/// };
///
/// struct Memory(Vec<u8>);
/// impl SectorDevice for Memory {
///     type Error = ();
///     fn sector_size(&self) -> SectorSize { SectorSize::Bytes512 }
///     fn read_sector(&mut self, index: u64, buffer: &mut [u8]) -> Result<(), ()> {
///         let start = index as usize * 512;
///         buffer.copy_from_slice(self.0.get(start..start + 512).ok_or(())?);
///         Ok(())
///     }
/// }
/// impl WritableSectorDevice for Memory {
///     fn write_sector(&mut self, index: u64, buffer: &[u8]) -> Result<(), ()> {
///         let start = index as usize * 512;
///         self.0.get_mut(start..start + 512).ok_or(())?.copy_from_slice(buffer);
///         Ok(())
///     }
/// }
///
/// let sectors = 16 * 1024;
/// let format = Format::new(FormatOptions::new(SectorSize::Bytes512, sectors), &mut NoClock).unwrap();
/// let mut device = Memory(vec![0; sectors as usize * 512]);
/// format.write_to(&mut device).unwrap();
/// // No time source here, so the caller says so.
/// let mut volume = WritableVolume::open(device, NoClock).unwrap();
/// volume.create_file(0, "HELLO.TXT").unwrap();
/// ```
///
/// Name: provisional. A volume plus the clock its writes need.
#[derive(Debug)]
pub struct WritableVolume<D, C> {
    volume: Volume<D>,
    clock: C,
}

impl<D: WritableSectorDevice, C: Clock> WritableVolume<D, C> {
    /// **Open the volume on `device` for writing**, stamping with `clock`.
    ///
    /// Name: provisional. [`Volume::open`] with the clock the writes need.
    pub fn open(device: D, clock: C) -> Result<WritableVolume<D, C>, crate::ReadError<D::Error>> {
        Ok(WritableVolume {
            volume: Volume::open(device)?,
            clock,
        })
    }

    /// Give the device back.
    ///
    /// Name: provisional. As `Volume::into_device`.
    pub fn into_device(self) -> D {
        self.volume.into_device()
    }

    /// Give the volume back without its clock, for reading.
    ///
    /// Name: provisional. The `into_` convention for giving back what a value owns.
    pub fn into_volume(self) -> Volume<D> {
        self.volume
    }

    /// Read the clock once for the operation about to run.
    fn tick(&mut self) {
        self.volume.stamp = Stamp::pack(self.clock.now());
    }

    /// **Take one free cluster** for the end of a chain: linked after `extending` when given.
    ///
    /// Name: provisional. As the former `Volume::allocate_cluster`.
    pub fn allocate_cluster(
        &mut self,
        extending: Option<u32>,
    ) -> Result<u32, AllocationError<D::Error>> {
        self.volume.allocate_cluster(extending)
    }

    /// **Free every cluster of the chain that starts at `first`**, and return how many.
    ///
    /// Name: provisional. As the former `Volume::free_chain`.
    pub fn free_chain(&mut self, first: u32) -> Result<u32, AllocationError<D::Error>> {
        self.volume.free_chain(first)
    }

    /// **Cut the chain that starts at `first` to its first `keep` clusters**, and return how many
    /// clusters it freed.
    ///
    /// Name: provisional. As the former `Volume::truncate_chain`.
    pub fn truncate_chain(
        &mut self,
        first: u32,
        keep: u32,
    ) -> Result<u32, AllocationError<D::Error>> {
        self.volume.truncate_chain(first, keep)
    }

    /// **Write the `FSInfo` hints and set the clean-shutdown bit again.**
    ///
    /// Name: provisional. As the former `Volume::sync`.
    pub fn sync(&mut self) -> Result<(), ChainError<D::Error>> {
        self.volume.sync()
    }

    /// **Create an empty file called `name`** in `directory` (0 for the root), created, written
    /// and accessed now.
    ///
    /// Name: provisional. As the former `Volume::create_file`.
    pub fn create_file(
        &mut self,
        directory: u32,
        name: &str,
    ) -> Result<NamedEntry, WriteError<D::Error>> {
        self.tick();
        self.volume.create_file(directory, name)
    }

    /// **Make an empty directory called `name`** in `directory` (0 for the root), created,
    /// written and accessed now, with the same times on its `.` and `..`.
    ///
    /// Name: provisional. As the former `Volume::make_directory`.
    pub fn make_directory(
        &mut self,
        directory: u32,
        name: &str,
    ) -> Result<NamedEntry, WriteError<D::Error>> {
        self.tick();
        self.volume.make_directory(directory, name)
    }

    /// **Remove the empty directory `entry` names.**
    ///
    /// Name: provisional. As the former `Volume::remove_directory`.
    pub fn remove_directory(&mut self, entry: &NamedEntry) -> Result<(), WriteError<D::Error>> {
        self.volume.remove_directory(entry)
    }

    /// **Rename `entry`, and move it to `directory`**; the entry keeps its times.
    ///
    /// Name: provisional. As the former `Volume::rename`.
    pub fn rename(
        &mut self,
        entry: &NamedEntry,
        directory: u32,
        new_name: &str,
    ) -> Result<NamedEntry, WriteError<D::Error>> {
        self.volume.rename(entry, directory, new_name)
    }

    /// **Delete the file `entry` names.**
    ///
    /// Name: provisional. As the former `Volume::remove_file`.
    pub fn remove_file(&mut self, entry: &NamedEntry) -> Result<(), WriteError<D::Error>> {
        self.volume.remove_file(entry)
    }

    /// **Set `entry`'s read-only, hidden, system and archive bits to exactly `attributes`.** The
    /// entry's times do not change.
    ///
    /// Name: provisional. As the former `Volume::set_attributes`.
    pub fn set_attributes(
        &mut self,
        entry: &NamedEntry,
        attributes: Attributes,
    ) -> Result<NamedEntry, WriteError<D::Error>> {
        self.volume.set_attributes(entry, attributes)
    }

    /// **Set the volume label.**
    ///
    /// Name: provisional. As the former `Volume::set_volume_label`.
    pub fn set_volume_label(&mut self, label: &str) -> Result<(), WriteError<D::Error>> {
        self.tick();
        self.volume.set_volume_label(label)
    }

    /// **Open the file `entry` names for writing**, at its start. The writer reads this volume's
    /// clock on each write. Refuses a directory and the volume label.
    ///
    /// Name: provisional. Beside `Volume::open_file`, which reads and takes no clock.
    pub fn open_file_writer(
        &mut self,
        entry: &NamedEntry,
    ) -> Result<FileWriter<'_, D, C>, NotAFile> {
        FileWriter::new(&mut self.volume, &mut self.clock, entry)
    }
}

impl<D, C> Deref for WritableVolume<D, C> {
    type Target = Volume<D>;

    fn deref(&self) -> &Volume<D> {
        &self.volume
    }
}

impl<D, C> DerefMut for WritableVolume<D, C> {
    fn deref_mut(&mut self) -> &mut Volume<D> {
        &mut self.volume
    }
}
