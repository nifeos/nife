//! **A directory read as files**: each short entry with the long name its pieces spell, checked
//! against the entry's checksum.
//!
//! [`Directory`] reports slots as stored. [`Entries`] wraps it and joins every run of long-name
//! pieces to the short entry after it, through [`LongNameAssembler`]. A run the assembler refuses
//! does not stop the listing: the short entry is still yielded, with no long name and the reason
//! the run was refused, because a stranger's volume may hold a stale piece and its files remain
//! reachable by their short names.
//!
//! Name: provisional. The entries of a directory, with their names.

use crate::directory::{Directory, DirectoryEntry, ShortEntry};
use crate::long_name_assembly::{LongFileName, LongNameAssembler, LongNameError};
use crate::reader::ChainError;
use crate::sector::SectorDevice;

/// **A directory entry with its names**: the short entry, and the long name when the entry has one
/// that passed its checks.
///
/// Name: provisional. A short entry that carries its long name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NamedEntry {
    short: ShortEntry,
    long_name: Result<Option<LongFileName>, LongNameError>,
}

impl NamedEntry {
    /// The short entry as stored.
    ///
    /// Name: provisional. The `ShortEntry` inside.
    pub const fn short_entry(&self) -> &ShortEntry {
        &self.short
    }

    /// The eleven short-name bytes as stored: eight of base and three of extension, space padded.
    ///
    /// Name: provisional. Matches `ShortEntry::name`.
    pub const fn short_name(&self) -> [u8; 11] {
        self.short.name()
    }

    /// The long name, when the entry has one and its run was whole, ordered and bound to this
    /// entry by its checksum.
    ///
    /// Name: provisional. The assembled name.
    pub const fn long_name(&self) -> Option<&LongFileName> {
        match &self.long_name {
            Ok(Some(name)) => Some(name),
            _ => None,
        }
    }

    /// Why the long-name pieces ahead of this entry were refused, or `None` when there were none
    /// or they were accepted.
    ///
    /// Name: provisional. The refusal, kept so a caller can tell a stale run from no run.
    pub const fn long_name_refusal(&self) -> Option<LongNameError> {
        match &self.long_name {
            Err(error) => Some(*error),
            Ok(_) => None,
        }
    }

    /// `DIR_Attr`, every bit as stored.
    ///
    /// Name: provisional. Matches `ShortEntry::attributes`.
    pub const fn attributes(&self) -> u8 {
        self.short.attributes()
    }

    /// The first cluster of the entry's chain, or 0 for an empty file. Not checked against the
    /// volume.
    ///
    /// Name: provisional. Matches `ShortEntry::first_cluster`.
    pub const fn first_cluster(&self) -> u32 {
        self.short.first_cluster()
    }

    /// The file's length in bytes; 0 for a directory.
    ///
    /// Name: provisional. Matches `ShortEntry::size_bytes`.
    pub const fn size_bytes(&self) -> u32 {
        self.short.size_bytes()
    }

    /// Whether the entry is a directory.
    ///
    /// Name: provisional. Matches `ShortEntry::is_directory`.
    pub const fn is_directory(&self) -> bool {
        self.short.is_directory()
    }

    /// Whether the entry is the volume label, which names no file.
    ///
    /// Name: provisional. Matches `ShortEntry::is_volume_label`.
    pub const fn is_volume_label(&self) -> bool {
        self.short.is_volume_label()
    }

    /// Whether the entry is `.` or `..`, the links a subdirectory begins with.
    ///
    /// Name: provisional. The specification's "dot" and "dotdot" entries.
    pub fn is_dot_entry(&self) -> bool {
        let name = self.short.name();
        self.short.is_directory() && (name == *b".          " || name == *b"..         ")
    }
}

/// **A directory, read as named entries**: what [`Volume::root_entries`] and [`Volume::entries`]
/// return.
///
/// It yields every used short entry in order, `.`, `..` and the volume label included, each with
/// its long name. It ends where [`Directory`] ends, and after an error it yields nothing more.
///
/// Name: provisional. The iterator over `NamedEntry`.
///
/// [`Volume::root_entries`]: crate::Volume::root_entries
/// [`Volume::entries`]: crate::Volume::entries
#[derive(Debug)]
pub struct Entries<'v, D> {
    directory: Directory<'v, D>,
    assembler: LongNameAssembler,
}

impl<'v, D> Entries<'v, D> {
    /// Read `directory` as named entries.
    pub(crate) const fn new(directory: Directory<'v, D>) -> Entries<'v, D> {
        Entries {
            directory,
            assembler: LongNameAssembler::new(),
        }
    }
}

impl<D: SectorDevice> Iterator for Entries<'_, D> {
    type Item = Result<NamedEntry, ChainError<D::Error>>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.directory.next()? {
                Err(error) => return Some(Err(error)),
                Ok(DirectoryEntry::LongName(piece)) => self.assembler.push(&piece),
                Ok(DirectoryEntry::Short(short)) => {
                    let long_name = self.assembler.finish(&short.name());
                    return Some(Ok(NamedEntry { short, long_name }));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
