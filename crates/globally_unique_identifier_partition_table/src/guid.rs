//! GPT's GUIDs: how the GUID Partition Table stores a UUID on disk, and the partition types worth
//! recognizing by name.
//!
//! The identifier itself is [`Uuid`], from the `uuid` crate, in RFC 9562's
//! byte order. This module keeps only what is GPT's: the on-disk byte layout and the type constants.
//! It keeps the spec's word, GUID, because the UEFI fields it serves are spelled that way
//! (`PartitionTypeGUID`, `UniquePartitionGUID`, `DiskGUID`). Everything else says UUID, RFC 9562's
//! term (calef approved the split on 2026-10-06 (UTC)).
//!
//! # The mixed-endian trap
//!
//! A GUID is 16 bytes and everybody writes it as `C12A7328-F81F-11D2-BA4B-00A0C93EC93B`. The trap is
//! that **the five groups are not stored the same way on a GPT**. The first three are integers and
//! go on disk little-endian; the last two are byte strings and go on disk in the order written. So
//! the bytes of that GUID on disk are:
//!
//! ```text
//!   28 73 2A C1  1F F8  D2 11  BA 4B  00 A0 C9 3E C9 3B
//!   \--------/   \---/  \---/  \---/  \-----------------/
//!    u32 LE       u16 LE u16 LE  as written
//! ```
//!
//! where RFC 9562 order (what [`Uuid::to_bytes`] returns) is `C1 2A 73 28 F8 1F 11 D2 BA 4B ...`.
//! This is Microsoft's `GUID` struct, which UEFI inherited; RFC 9562 section 4 names it as the one known
//! exception to big-endian storage. Getting it wrong gives you a GUID that looks plausible, matches
//! nothing, and is byte-reversed in three places out of five. [`from_disk`] and [`to_disk`] are the
//! only two functions that see the on-disk order, and [`Entry`](crate::Entry) and
//! [`Header`](crate::Header) call nothing else, so a [`Uuid`] in this crate is always canonical and
//! always prints right.
//!
//! # Type GUIDs versus unique GUIDs
//!
//! Every entry carries two. The **type** GUID says what the partition is for and is a shared
//! constant ([`types`]); the **unique** GUID names this particular partition and is generated once,
//! never reused. Confusing them is how you get two partitions the OS thinks are the same one.

pub use uuid::Uuid;

/// Which RFC 9562 byte each on-disk byte holds: the first three fields reversed, the last two not.
/// The same table read either way, because each swap is its own inverse.
const SWAP: [usize; 16] = [3, 2, 1, 0, 5, 4, 7, 6, 8, 9, 10, 11, 12, 13, 14, 15];

/// The UUID that sixteen bytes read from a GPT (an entry's type or unique GUID, or the header's
/// disk GUID) stand for. Every bit pattern is one, so this cannot fail.
///
/// Name: ratified 2026-10-07 (calef, #1795). His word: "Yes". Named for where the bytes come from,
/// since the disk order is the only thing that distinguishes them from a `Uuid`'s own.
pub const fn from_disk(bytes: [u8; 16]) -> Uuid {
    let mut canonical = [0u8; 16];
    let mut i = 0;
    while i < 16 {
        canonical[i] = bytes[SWAP[i]];
        i += 1;
    }
    Uuid::from_bytes(canonical)
}

/// The sixteen bytes a GPT stores for `id`. Inverse of [`from_disk`], proved.
///
/// Name: ratified 2026-10-07 (calef, #1795). His word: "Yes". The inverse of `from_disk`, named to
/// match it.
pub const fn to_disk(id: Uuid) -> [u8; 16] {
    let canonical = id.into_bytes();
    let mut bytes = [0u8; 16];
    let mut i = 0;
    while i < 16 {
        bytes[SWAP[i]] = canonical[i];
        i += 1;
    }
    bytes
}

/// Partition **type** GUIDs, and a name for each.
///
/// Not an attempt at the full registry, which runs to several hundred entries and is a data problem
/// rather than a kernel problem. These are the ones that answer a question this OS actually asks:
/// which partition do I boot from, which one holds a Linux filesystem, which one is ours, and (for
/// the recovery story in milestone 57) what are the two things a Mac will have put on a disk.
///
/// Every value here was read back out of `sgdisk` on the machine rather than typed from memory, and
/// four of them are pinned by the committed fixtures.
pub mod types {
    use super::Uuid;

    /// An unused entry. The type GUID is what marks an entry live, not the LBA fields.
    pub const UNUSED: Uuid = Uuid::nil();

    /// EFI System Partition: the FAT volume firmware loads a bootloader from.
    pub const EFI_SYSTEM: Uuid = Uuid::from_fields(
        0xC12A_7328,
        0xF81F,
        0x11D2,
        &[0xBA, 0x4B, 0x00, 0xA0, 0xC9, 0x3E, 0xC9, 0x3B],
    );

    /// BIOS boot partition, where GRUB puts its core image on a legacy-booted GPT disk. The GUID
    /// spells `Hah!IdontNeedEFI` in ASCII, which is the only joke in the UEFI ecosystem.
    pub(crate) const BIOS_BOOT: Uuid = Uuid::from_fields(
        0x2168_6148,
        0x6449,
        0x6E6F,
        &[0x74, 0x4E, 0x65, 0x65, 0x64, 0x45, 0x46, 0x49],
    );

    /// Microsoft basic data. Also what macOS's `diskutil` labels a FAT partition it creates, which
    /// is why the Apple fixture carries it.
    pub const MICROSOFT_BASIC_DATA: Uuid = Uuid::from_fields(
        0xEBD0_A0A2,
        0xB9E5,
        0x4433,
        &[0x87, 0xC0, 0x68, 0xB6, 0xB7, 0x26, 0x99, 0xC7],
    );

    /// Linux filesystem data, `sgdisk` type code `8300`. The generic "there is a Linux filesystem
    /// here" marker and by far the most common type on a Linux disk.
    pub const LINUX_FILESYSTEM: Uuid = Uuid::from_fields(
        0x0FC6_3DAF,
        0x8483,
        0x4772,
        &[0x8E, 0x79, 0x3D, 0x69, 0xD8, 0x47, 0x7D, 0xE4],
    );

    /// Linux swap.
    pub(crate) const LINUX_SWAP: Uuid = Uuid::from_fields(
        0x0657_FD6D,
        0xA4AB,
        0x43C4,
        &[0x84, 0xE5, 0x09, 0x33, 0xC8, 0x4B, 0x4F, 0x4F],
    );

    /// Linux root filesystem, arm64. From the Discoverable Partitions Specification, which is how a
    /// systemd machine finds its root without a kernel command line. Here because arm64 is the
    /// architecture this OS runs on and a disk shared with Linux may carry one.
    pub(crate) const LINUX_ROOT_ARM64: Uuid = Uuid::from_fields(
        0xB921_B045,
        0x1DF0,
        0x41C3,
        &[0xAF, 0x44, 0x4C, 0x6F, 0x28, 0x0D, 0x3F, 0xAE],
    );

    /// LUKS, Linux's encrypted-volume container. Recognised so that a tool can say "encrypted, and
    /// this crate does not open it" instead of reporting an unreadable filesystem.
    pub(crate) const LINUX_LUKS: Uuid = Uuid::from_fields(
        0xCA7D_7CCB,
        0x63ED,
        0x4C53,
        &[0x86, 0x1C, 0x17, 0x42, 0x53, 0x60, 0x59, 0xCC],
    );

    /// Apple HFS+.
    pub const APPLE_HFS_PLUS: Uuid = Uuid::from_fields(
        0x4846_5300,
        0x0000,
        0x11AA,
        &[0xAA, 0x11, 0x00, 0x30, 0x65, 0x43, 0xEC, 0xAC],
    );

    /// Apple APFS, which is what any Mac disk made since 2017 is.
    pub const APPLE_APFS: Uuid = Uuid::from_fields(
        0x7C34_57EF,
        0x0000,
        0x11AA,
        &[0xAA, 0x11, 0x00, 0x30, 0x65, 0x43, 0xEC, 0xAC],
    );

    /// **A nife data partition**, holding a RedoxFS volume (DECISIONS §34).
    ///
    /// A random version-4 GUID, generated once on 2026-07-30 and fixed forever. It has to be random:
    /// a type GUID's whole job is to not collide with anybody else's, and there is no registry to
    /// ask. **Never change this value.** A disk written by one release and read by another has only
    /// this number to agree on, and the recovery story (milestone 57's "the board is dead, can I get
    /// my data") depends on a `sgdisk -p` five years from now still showing it.
    pub const NIFE_DATA: Uuid = Uuid::from_fields(
        0xEC5C_C08B,
        0xD749,
        0x4434,
        &[0xAC, 0x38, 0xA2, 0x74, 0xC5, 0x03, 0x85, 0xBA],
    );

    /// **A nife boot slot**, holding one copy of the image the firmware's chooser starts
    /// (rung 2b of milestone 198 (a package manager, and the trivial install that makes a second customer possible)).
    ///
    /// A random version-4 GUID, generated once on 2026-09-21 and fixed forever, for the same
    /// reason [`NIFE_DATA`] is: there is no registry to ask and a disk written by one release has
    /// only this number to agree on with the next. **Never change this value.**
    ///
    /// It has a second job [`NIFE_DATA`] does not, and it is the reason a boot slot is a partition
    /// of its own rather than a file. UEFI 2.11 5.3.3 reserves attribute bits 48 to 63 for
    /// "GUID specific use" and says *"Only the owner of the `PartitionTypeGUID` is allowed to modify
    /// these bits"*. Those bits carry the priority, the tries and the confirmed flag that decide
    /// which image boots (`crates/boot_slot`), and they are ours to define **because this GUID is
    /// ours**. On a partition of somebody else's type they would be somebody else's bits.
    ///
    /// **Ratified by calef on 2026-09-21** ("Ratify the GUID and bit positions"), the same day it
    /// was minted. A type GUID is a value two programs agree on, so ratification is what moves it
    /// from a lane's proposal to a number this project has committed to; from here it changes only
    /// the way [`NIFE_DATA`] would, which is to say not at all.
    pub const NIFE_BOOT: Uuid = Uuid::from_fields(
        0x1163_1EE3,
        0xE18F,
        0x4AFC,
        &[0x9D, 0x7F, 0x17, 0x15, 0x72, 0x63, 0x56, 0x29],
    );

    /// A short name for a type GUID, or `None` for one this crate does not recognise.
    ///
    /// Deliberately returns `None` rather than a placeholder: a partition tool should print the raw
    /// GUID for an unknown type, because that is the string a person can look up.
    pub fn name(guid: Uuid) -> Option<&'static str> {
        Some(match guid {
            UNUSED => "unused",
            EFI_SYSTEM => "EFI system",
            BIOS_BOOT => "BIOS boot",
            MICROSOFT_BASIC_DATA => "Microsoft basic data",
            LINUX_FILESYSTEM => "Linux filesystem",
            LINUX_SWAP => "Linux swap",
            LINUX_ROOT_ARM64 => "Linux root (arm64)",
            LINUX_LUKS => "Linux LUKS",
            APPLE_HFS_PLUS => "Apple HFS+",
            APPLE_APFS => "Apple APFS",
            NIFE_DATA => "nife data",
            NIFE_BOOT => "nife boot slot",
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The EFI System Partition GUID as it sits on disk, which is offset 0 of entry 0 in the
    /// committed `sgdisk` fixture. The one known vector for GPT's layout: the first three groups
    /// little-endian, the last two as written.
    const EFI_SYSTEM_ON_DISK: [u8; 16] = [
        0x28, 0x73, 0x2A, 0xC1, // C12A7328, little-endian
        0x1F, 0xF8, // F81F, little-endian
        0xD2, 0x11, // 11D2, little-endian
        0xBA, 0x4B, // BA4B, as written
        0x00, 0xA0, 0xC9, 0x3E, 0xC9, 0x3B, // node, as written
    ];

    /// Every type GUID this crate names says its name, and one it does not know says nothing. A
    /// deleted arm would hand a person a raw GUID for a partition type this crate was written to
    /// recognize.
    #[test]
    fn every_known_type_has_its_name() {
        let known = [
            (types::UNUSED, "unused"),
            (types::EFI_SYSTEM, "EFI system"),
            (types::BIOS_BOOT, "BIOS boot"),
            (types::MICROSOFT_BASIC_DATA, "Microsoft basic data"),
            (types::LINUX_FILESYSTEM, "Linux filesystem"),
            (types::LINUX_SWAP, "Linux swap"),
            (types::LINUX_ROOT_ARM64, "Linux root (arm64)"),
            (types::LINUX_LUKS, "Linux LUKS"),
            (types::APPLE_HFS_PLUS, "Apple HFS+"),
            (types::APPLE_APFS, "Apple APFS"),
            (types::NIFE_DATA, "nife data"),
            (types::NIFE_BOOT, "nife boot slot"),
        ];
        for (guid, name) in known {
            assert_eq!(types::name(guid), Some(name));
        }
        assert_eq!(types::name(Uuid::from_bytes([0xab; 16])), None);
    }

    /// **The two byte orders, side by side, on one known GUID.** The type constant is canonical
    /// (RFC 9562 order, the printed digits), `to_disk` is GPT's mixed-endian order, and
    /// `from_disk` takes the fixture's bytes back to the constant. This is the one place the
    /// mixed-endian rule can be wrong without anything else noticing.
    #[test]
    fn the_efi_system_partition_guid_in_both_byte_orders() {
        assert_eq!(
            types::EFI_SYSTEM.into_bytes(),
            [
                0xC1, 0x2A, 0x73, 0x28, 0xF8, 0x1F, 0x11, 0xD2, 0xBA, 0x4B, 0x00, 0xA0, 0xC9, 0x3E,
                0xC9, 0x3B,
            ],
            "canonical order is the printed digits",
        );
        assert_eq!(to_disk(types::EFI_SYSTEM), EFI_SYSTEM_ON_DISK);
        assert_eq!(from_disk(EFI_SYSTEM_ON_DISK), types::EFI_SYSTEM);
        assert_eq!(
            &*from_disk(EFI_SYSTEM_ON_DISK)
                .hyphenated()
                .encode_upper(&mut Uuid::encode_buffer()),
            "C12A7328-F81F-11D2-BA4B-00A0C93EC93B"
        );
    }

    #[test]
    fn printing_a_type_constant_gives_its_registered_string() {
        assert_eq!(
            &*types::NIFE_DATA
                .hyphenated()
                .encode_upper(&mut Uuid::encode_buffer()),
            "EC5CC08B-D749-4434-AC38-A274C50385BA"
        );
        assert_eq!(
            &*types::NIFE_BOOT
                .hyphenated()
                .encode_upper(&mut Uuid::encode_buffer()),
            "11631EE3-E18F-4AFC-9D7F-171572635629"
        );
    }

    /// **RFC 9562 Appendix A.3, end to end, through the calls this tree makes**: the sixteen
    /// random bytes the RFC starts from, stamped by `uuid::Builder::from_random_bytes` (what
    /// `disk_partitioner`, `system_installer` and the `uuid` program call), printed by the uppercase
    /// hyphenated encoder (what the `uuid` program prints), and read back in lower case. A `uuid`
    /// release that moved the stamp or the case would fail here and nowhere else on the host.
    #[test]
    fn the_rfc_9562_version_4_example_comes_out_as_printed() {
        let random = [
            0x91, 0x91, 0x08, 0xF7, 0x52, 0xD1, 0x33, 0x20, 0x5B, 0xAC, 0xF8, 0x47, 0xDB, 0x41,
            0x48, 0xA8,
        ];
        let id = uuid::Builder::from_random_bytes(random).into_uuid();
        assert_eq!(
            &*id.hyphenated().encode_upper(&mut Uuid::encode_buffer()),
            "919108F7-52D1-4320-9BAC-F847DB4148A8"
        );
        assert_eq!(
            Uuid::try_parse_ascii(b"919108f7-52d1-4320-9bac-f847db4148a8"),
            Ok(id)
        );
    }

    /// A version-4 UUID written to a GPT has its version nibble in on-disk byte 7, where `sgdisk`
    /// and UEFI read the third group's high nibble from. Before 2026-10-06 the stamp itself wrote
    /// byte 7 because the type held on-disk bytes; now the stamp writes RFC byte 6 and `to_disk`
    /// moves it, and this is the check that the two together land where the old one did.
    #[test]
    fn a_version_4_uuid_lands_its_version_in_on_disk_byte_7() {
        for fill in [0x00u8, 0xff, 0x5a] {
            let disk = to_disk(uuid::Builder::from_random_bytes([fill; 16]).into_uuid());
            assert_eq!(disk[7] >> 4, 4, "version nibble, from a {fill:#04x} fill");
            assert_eq!(disk[8] >> 6, 0b10, "variant bits, from a {fill:#04x} fill");
        }
    }

    #[test]
    fn every_named_type_is_distinct() {
        let all = [
            types::EFI_SYSTEM,
            types::BIOS_BOOT,
            types::MICROSOFT_BASIC_DATA,
            types::LINUX_FILESYSTEM,
            types::LINUX_SWAP,
            types::LINUX_ROOT_ARM64,
            types::LINUX_LUKS,
            types::APPLE_HFS_PLUS,
            types::APPLE_APFS,
            types::NIFE_DATA,
            types::NIFE_BOOT,
        ];
        for (i, a) in all.iter().enumerate() {
            assert!(types::name(*a).is_some(), "{a} has no name");
            for b in &all[i + 1..] {
                assert_ne!(a, b, "two type constants collide");
            }
        }
    }
}
