//! **The partition types nife writes to a disk**, and a name for each.
//!
//! An entry in a disk's partition table names the kind of partition with a type Globally Unique
//! Identifier (GUID). These two are nife's own: the data volume and the boot slot. Every
//! installed disk carries them, so their values never change. The well-known types (the
//! Extensible Firmware Interface (EFI) system partition, Linux filesystem and the rest) are in
//! [`globally_unique_identifier_partition_table::guid::types`], and [`name`] answers for both
//! sets.
//!
//! Name: provisional. The crate keeps nife in its name and its constants while it is unpublished;
//! whether the identifiers say nife or Moho stays open and two-way.

#![no_std]
#![forbid(unsafe_code)]

use globally_unique_identifier_partition_table::guid::types;
use uuid::Uuid;

/// **A nife data partition**, holding a RedoxFS volume.
///
/// A random version-4 GUID, generated once on 2026-07-30 and fixed forever. It has to be random:
/// a type GUID's whole job is to not collide with anybody else's, and there is no registry to
/// ask. **Never change this value.** A disk written by one release and read by another has only
/// this number to agree on, and the recovery story ("the board is dead, can I get my
/// data") depends on a `sgdisk -p` five years from now still showing it.
///
/// Name: provisional, moved from `globally_unique_identifier_partition_table::guid::types`.
pub const NIFE_DATA: Uuid = Uuid::from_fields(
    0xEC5C_C08B,
    0xD749,
    0x4434,
    &[0xAC, 0x38, 0xA2, 0x74, 0xC5, 0x03, 0x85, 0xBA],
);

/// **A nife boot slot**, holding one copy of the image the firmware's chooser starts
/// (one slot of an A/B install).
///
/// A random version-4 GUID generated on 2026-09-21 and fixed. **Never change this value**, for [`NIFE_DATA`]'s reason. Unified Extensible Firmware Interface (UEFI) 2.11
/// 5.3.3 reserves attribute bits 48 to 63 for "GUID specific use": *"Only the owner of the
/// `PartitionTypeGUID` is allowed to modify these bits"*. They carry a boot slot's
/// priority, tries and confirmed flag, and are ours because this GUID is, which is why a boot
/// slot is a partition and not a file.
///
/// Name: provisional, moved from `globally_unique_identifier_partition_table::guid::types`.
pub const NIFE_BOOT: Uuid = Uuid::from_fields(
    0x1163_1EE3,
    0xE18F,
    0x4AFC,
    &[0x9D, 0x7F, 0x17, 0x15, 0x72, 0x63, 0x56, 0x29],
);

/// A short name for a partition type, or `None` for one neither this crate nor
/// [`types::name`] recognizes.
///
/// Answers for the two nife types, and for every other GUID asks [`types::name`]. It returns
/// `None` rather than a placeholder: a partition tool should print the raw GUID for an unknown
/// type, because that is the string a person can look up.
///
/// Name: provisional.
pub fn name(type_identifier: Uuid) -> Option<&'static str> {
    match type_identifier {
        NIFE_DATA => Some("nife data"),
        NIFE_BOOT => Some("nife boot slot"),
        other => types::name(other),
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;

    /// The values are on every installed disk, so the printed form is pinned.
    #[test]
    fn printing_a_type_constant_gives_its_registered_string() {
        assert_eq!(
            &*NIFE_DATA
                .hyphenated()
                .encode_upper(&mut Uuid::encode_buffer()),
            "EC5CC08B-D749-4434-AC38-A274C50385BA"
        );
        assert_eq!(
            &*NIFE_BOOT
                .hyphenated()
                .encode_upper(&mut Uuid::encode_buffer()),
            "11631EE3-E18F-4AFC-9D7F-171572635629"
        );
    }

    /// Both nife types say their names, a well-known type still says its own, and an unknown one
    /// says nothing. A deleted arm would hand a person a raw GUID for a partition this system wrote.
    #[test]
    fn every_known_type_has_its_name() {
        assert_eq!(name(NIFE_DATA), Some("nife data"));
        assert_eq!(name(NIFE_BOOT), Some("nife boot slot"));
        assert_eq!(name(types::EFI_SYSTEM), Some("EFI system"));
        assert_eq!(name(Uuid::from_bytes([0xab; 16])), None);
    }

    /// The two types differ from each other and from every well-known type, since a collision would
    /// make a foreign partition look like ours.
    #[test]
    fn the_two_types_collide_with_nothing_named() {
        assert_ne!(NIFE_DATA, NIFE_BOOT);
        for known in [
            types::EFI_SYSTEM,
            types::MICROSOFT_BASIC_DATA,
            types::LINUX_FILESYSTEM,
            types::APPLE_HFS_PLUS,
            types::APPLE_APFS,
        ] {
            assert_ne!(NIFE_DATA, known);
            assert_ne!(NIFE_BOOT, known);
        }
    }
}
