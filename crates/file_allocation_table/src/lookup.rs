//! **Finding a file by path**: names compared the way FAT compares them, without regard to case,
//! against both the long name and the short name of every entry.
//!
//! A path is components joined by `/` or `\`. Empty components (a leading, trailing or doubled
//! separator) are skipped. `.` and `..` are looked up like any other name: a subdirectory holds
//! entries for them, the root does not, and a `..` that leads to the root has first cluster 0,
//! which [`Volume::entries`] reads as the root.
//!
//! A long name matches when its UTF-16 units equal the query's after each unit is upper-cased by
//! its simple (one-to-one) Unicode mapping. A short name matches when its dotted form equals the
//! query's ASCII after the same fold; a query with a non-ASCII character matches only a long name.
//! The first matching entry in directory order wins, which is the one a deleted-then-recreated
//! duplicate would hide.
//!
//! Name: provisional. The module that finds an entry by name.

use crate::named_entry::NamedEntry;
use crate::reader::{ChainError, Volume};
use crate::sector::SectorDevice;

/// **Why a lookup found nothing.**
///
/// Name: provisional. Beside `ChainError`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LookupError<E> {
    /// A directory could not be read; its own error, unchanged.
    ///
    /// Name: provisional. The chain's own error, unchanged.
    Chain(ChainError<E>),
    /// No entry matched a component.
    ///
    /// Name: provisional. The condition.
    NotFound,
    /// A component that is not the last names a file, not a directory.
    ///
    /// Name: provisional. The condition.
    NotADirectory,
    /// The path has no component, so it names the root, which has no entry.
    ///
    /// Name: provisional. The root has no entry to return; read it with `root_entries`.
    NoName,
}

/// The simple upper-case mapping of one UTF-16 unit: the unit itself unless it is a character
/// whose upper case is one character in the Basic Multilingual Plane.
fn fold_unit(unit: u16) -> u16 {
    let Some(c) = char::from_u32(u32::from(unit)) else {
        return unit;
    };
    let mut upper = c.to_uppercase();
    match (upper.next(), upper.next()) {
        (Some(u), None) => u16::try_from(u32::from(u)).unwrap_or(unit),
        _ => unit,
    }
}

/// Whether `units` and the UTF-16 encoding of `query` are equal after folding each unit.
fn long_name_matches(units: &[u16], query: &str) -> bool {
    let mut query = query.encode_utf16();
    for unit in units {
        match query.next() {
            Some(q) if fold_unit(q) == fold_unit(*unit) => {}
            _ => return false,
        }
    }
    query.next().is_none()
}

/// The length of `part` without its trailing spaces.
fn trimmed_len(part: &[u8]) -> usize {
    part.iter().rposition(|b| *b != b' ').map_or(0, |i| i + 1)
}

/// Whether the dotted form of the eleven short-name bytes equals `query` in ASCII, ignoring case.
fn short_name_matches(name: &[u8; 11], query: &str) -> bool {
    let mut shown = [0u8; 12];
    let base = trimmed_len(&name[..8]);
    shown[..base].copy_from_slice(&name[..base]);
    let mut len = base;
    let extension = trimmed_len(&name[8..]);
    if extension > 0 {
        shown[len] = b'.';
        shown[len + 1..len + 1 + extension].copy_from_slice(&name[8..8 + extension]);
        len += 1 + extension;
    }
    shown[..len].eq_ignore_ascii_case(query.as_bytes())
}

impl NamedEntry {
    /// **Whether this entry is called `name`**, by its long name or its short name, without regard
    /// to case. The volume label is called nothing.
    ///
    /// Name: provisional. The question a lookup asks of each entry.
    pub fn is_called(&self, name: &str) -> bool {
        if self.is_volume_label() || name.is_empty() {
            return false;
        }
        let long = self
            .long_name()
            .is_some_and(|long| long_name_matches(long.code_units(), name));
        long || short_name_matches(&self.short_name(), name)
    }
}

impl<D: SectorDevice> Volume<D> {
    /// **The entry called `name` in the directory whose first cluster is `directory`** (0 for the
    /// root), or `Ok(None)`.
    ///
    /// Name: provisional. One component of a lookup, by the directory's first cluster as
    /// `Volume::entries`.
    pub fn find(
        &mut self,
        directory: u32,
        name: &str,
    ) -> Result<Option<NamedEntry>, ChainError<D::Error>> {
        for entry in self.entries(directory) {
            let entry = entry?;
            if entry.is_called(name) {
                return Ok(Some(entry));
            }
        }
        Ok(None)
    }

    /// **The entry at `path`**, from the root.
    ///
    /// Every component but the last must name a directory. A path that names the root itself has
    /// no entry and is [`LookupError::NoName`]. Each component reads one directory to its end or
    /// its match, bounded by the volume's cluster count, and the number of directories read is
    /// the number of components.
    ///
    /// Name: provisional. A path lookup; `find` for one component.
    pub fn lookup(&mut self, path: &str) -> Result<NamedEntry, LookupError<D::Error>> {
        let mut directory = 0;
        let mut components = path.split(['/', '\\']).filter(|part| !part.is_empty());
        let Some(mut name) = components.next() else {
            return Err(LookupError::NoName);
        };
        loop {
            let entry = self
                .find(directory, name)
                .map_err(LookupError::Chain)?
                .ok_or(LookupError::NotFound)?;
            let Some(next) = components.next() else {
                return Ok(entry);
            };
            if !entry.is_directory() {
                return Err(LookupError::NotADirectory);
            }
            directory = entry.first_cluster();
            name = next;
        }
    }
}

#[cfg(test)]
mod tests;
