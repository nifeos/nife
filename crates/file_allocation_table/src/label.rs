//! **The volume label**: read from the root directory's label entry or, failing that, the boot
//! sector, and changed in both places.
//!
//! A FAT volume keeps its label twice. The root directory's volume-label entry is the one the
//! tools show and change; the boot sector's `BS_VolLab` is a copy that older drivers read. A format
//! without a label leaves the boot sector at `NO NAME` and the root without an entry, and that
//! reads as no label.
//!
//! Name: provisional. The label half of the volume's identity, beside the serial number.

use crate::boot_sector::FileAllocationTableType;
use crate::file_write::{Medium, WriteError};
use crate::format::label_bytes;
use crate::reader::{ChainError, Volume};
use crate::sector::{SectorDevice, SectorSize, WritableSectorDevice};

/// `ATTR_VOLUME_ID`.
const LABEL_ENTRY: u8 = 0x08;
/// The boot sector's label when a format was given none.
const NO_NAME: [u8; 11] = *b"NO NAME    ";

/// **A volume label**: eleven bytes, the name then spaces.
///
/// Name: provisional. The specification's `BS_VolLab` and label entry as one value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VolumeLabel([u8; 11]);

impl VolumeLabel {
    /// The eleven bytes as stored, padded with spaces.
    ///
    /// Name: provisional. The stored form.
    pub const fn as_bytes(&self) -> &[u8; 11] {
        &self.0
    }

    /// The name without its trailing spaces.
    ///
    /// Name: provisional. The form a person reads.
    pub fn trimmed(&self) -> &[u8] {
        let end = self.0.iter().rposition(|b| *b != b' ').map_or(0, |i| i + 1);
        &self.0[..end]
    }
}

impl<D: SectorDevice> Volume<D> {
    /// Where the boot sector keeps its label, and the signature byte that says it is there.
    const fn boot_label_at(kind: FileAllocationTableType) -> (usize, usize) {
        match kind {
            FileAllocationTableType::Fat32 => (66, 71),
            _ => (38, 43),
        }
    }

    /// **The volume's label**, or `None` when it has none: the root's label entry if there is one,
    /// else the boot sector's if it is set and is not `NO NAME`.
    ///
    /// Name: provisional. The `label` of `fatlabel` and `diskutil info`.
    pub fn volume_label(&mut self) -> Result<Option<VolumeLabel>, ChainError<D::Error>> {
        for entry in self.root_entries() {
            let entry = entry?;
            if entry.is_volume_label() {
                return Ok(Some(VolumeLabel(entry.short_name())));
            }
        }
        let size = self.boot.sector_size().bytes();
        let mut sector = [0u8; SectorSize::MAX];
        self.read_sector(0, &mut sector[..size])?;
        let (signature, label) = Self::boot_label_at(self.boot.file_allocation_table_type());
        if sector[signature] != 0x29 {
            return Ok(None);
        }
        let mut bytes = [0u8; 11];
        bytes.copy_from_slice(&sector[label..label + 11]);
        Ok((bytes != NO_NAME && bytes != [b' '; 11]).then_some(VolumeLabel(bytes)))
    }
}

impl<D: WritableSectorDevice> Volume<D> {
    /// **Set the label** to `label`: up to eleven upper-case letters, digits, spaces and the
    /// punctuation a short name admits. The root's label entry changes, or is made in the first free
    /// slot, and then the boot sector's copy (and FAT32's backup boot sector) follows.
    ///
    /// Name: provisional. The `fatlabel` of the directory tree.
    pub fn set_volume_label(&mut self, label: &str) -> Result<(), WriteError<D::Error>> {
        let bytes = label_bytes(label).ok_or(WriteError::InvalidName)?;
        self.begin_change()?;
        let mut existing = None;
        for entry in self.root_entries() {
            let entry = entry?;
            if entry.is_volume_label() {
                existing = Some(entry.place().short_slot);
                break;
            }
        }
        let index = match existing {
            Some(index) => index,
            None => self.find_run(0, 1)?.0,
        };
        let mut slot = [0u8; 32];
        slot[..11].copy_from_slice(&bytes);
        slot[11] = LABEL_ENTRY;
        self.write_slot(0, index, &slot)?;
        let mut copies = [Some(0), None];
        if let Some(backup) = self.boot.backup_boot_sector() {
            copies[1] = Some(u32::from(backup));
        }
        let size = self.boot.sector_size().bytes();
        let (signature, at) = Self::boot_label_at(self.boot.file_allocation_table_type());
        for sector_index in copies.into_iter().flatten() {
            let mut sector = [0u8; SectorSize::MAX];
            self.read_sector(sector_index, &mut sector[..size])?;
            // A boot sector with no extended signature has no label field to fill.
            if sector[signature] == 0x29 {
                sector[at..at + 11].copy_from_slice(&bytes);
                self.write_data(sector_index, &sector[..size])?;
            }
        }
        Ok(())
    }
}

#[cfg(kani)]
mod verification;

#[cfg(test)]
mod tests;
