//! **A file's attribute bits** the user sets: read-only, hidden, system and archive.
//!
//! `DIR_Attr` holds six bits. Two of them say what the entry is (a directory, the volume label) and
//! the writer owns them; the other four are the user's, and [`WritableVolume::set_attributes`] changes only
//! those. The result can never read as a long-name piece, whose attribute is all four low bits
//! together with the label bit, because the label bit is never set by this module and an entry
//! that already carries it is refused.
//!
//! Name: ratified 2026-11-01 (UTC), the attribute half of `directory_writer`, with no clock in it.

use crate::named_entry::NamedEntry;
use crate::reader::Volume;
use crate::sector::WritableSectorDevice;
use crate::write_cursor::WriteError;

/// The four bits a user sets: read-only, hidden, system, archive.
const USER_BITS: u8 = 0x27;

/// **A set of the four user-settable attribute bits.**
///
/// # Examples
///
/// ```
/// use file_allocation_table::Attributes;
///
/// let hidden_log = Attributes::HIDDEN.union(Attributes::ARCHIVE);
/// assert!(hidden_log.contains(Attributes::HIDDEN));
/// assert!(!hidden_log.contains(Attributes::READ_ONLY));
/// assert_eq!(hidden_log.difference(Attributes::HIDDEN), Attributes::ARCHIVE);
/// ```
///
/// Name: ratified 2026-11-01 (UTC), a set of `ATTR_READ_ONLY`, `ATTR_HIDDEN`, `ATTR_SYSTEM` and
/// `ATTR_ARCHIVE`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Attributes(u8);

impl Attributes {
    /// No bit set.
    ///
    /// Name: ratified 2026-11-01 (UTC), renamed from `NONE`: the set holding no bit, named as a
    /// collection is named when it holds nothing, and not as an absent value.
    pub const EMPTY: Attributes = Attributes(0);
    /// `ATTR_READ_ONLY`: the file may not be written or removed.
    ///
    /// Name: ratified 2026-11-01 (UTC), the specification's bit.
    pub const READ_ONLY: Attributes = Attributes(0x01);
    /// `ATTR_HIDDEN`: a listing leaves the file out unless asked.
    ///
    /// Name: ratified 2026-11-01 (UTC), the specification's bit.
    pub const HIDDEN: Attributes = Attributes(0x02);
    /// `ATTR_SYSTEM`: the file belongs to the operating system.
    ///
    /// Name: ratified 2026-11-01 (UTC), the specification's bit.
    pub const SYSTEM: Attributes = Attributes(0x04);
    /// `ATTR_ARCHIVE`: the file changed since a backup last cleared the bit.
    ///
    /// Name: ratified 2026-11-01 (UTC), the specification's bit.
    pub const ARCHIVE: Attributes = Attributes(0x20);

    /// The set with every bit of `self` and of `other`.
    ///
    /// Name: ratified 2026-11-01 (UTC), set union.
    #[must_use]
    pub const fn union(self, other: Attributes) -> Attributes {
        Attributes(self.0 | other.0)
    }

    /// The set without the bits of `other`.
    ///
    /// Name: ratified 2026-11-01 (UTC), renamed from `without`: the set operation, beside `union`.
    #[must_use]
    pub const fn difference(self, other: Attributes) -> Attributes {
        Attributes(self.0 & !other.0)
    }

    /// Whether every bit of `other` is in the set.
    ///
    /// Name: ratified 2026-11-01 (UTC), set containment.
    pub const fn contains(self, other: Attributes) -> bool {
        self.0 & other.0 == other.0
    }

    /// The bits as `DIR_Attr` stores them.
    ///
    /// Name: ratified 2026-11-01 (UTC), the raw byte.
    pub const fn bits(self) -> u8 {
        self.0
    }

    /// The user bits of a stored `DIR_Attr`.
    pub(crate) const fn of_stored(stored: u8) -> Attributes {
        Attributes(stored & USER_BITS)
    }
}

/// **`DIR_Attr` after the user bits become `new`**: the directory and label bits and the two bits
/// the specification reserves keep their value.
pub(crate) const fn apply(stored: u8, new: Attributes) -> u8 {
    (stored & !USER_BITS) + new.0
}

impl NamedEntry {
    /// The user-settable attribute bits of the entry.
    ///
    /// Name: ratified 2026-11-01 (UTC), renamed from `file_attributes`: the user-settable four-bit
    /// set, apart from `attributes`, the raw `DIR_Attr` byte.
    pub const fn user_attributes(&self) -> Attributes {
        Attributes::of_stored(self.attributes())
    }
}

impl<D: WritableSectorDevice> Volume<D> {
    /// **Set `entry`'s read-only, hidden, system and archive bits to exactly `attributes`**, and
    /// return the entry as stored. The directory bit and every other bit stay. Refuses the volume
    /// label, `.` and `..`.
    ///
    /// Name: ratified 2026-11-01 (UTC), the `chmod` and `attrib` of the directory tree; it takes no
    /// time.
    pub(crate) fn set_attributes(
        &mut self,
        entry: &NamedEntry,
        attributes: Attributes,
    ) -> Result<NamedEntry, WriteError<D::Error>> {
        if entry.is_dot_entry() || entry.is_volume_label() {
            return Err(WriteError::NotAFile);
        }
        self.verify(entry)?;
        let place = entry.place();
        let mut slot = self
            .read_slot(place.directory, place.short_slot)?
            .ok_or(WriteError::EntryChanged)?;
        slot[11] = apply(slot[11], attributes);
        self.write_slot(place.directory, place.short_slot, &slot)?;
        self.entry_at(place.directory, place.short_slot)
    }
}

#[cfg(kani)]
mod verification;

#[cfg(test)]
mod tests;
