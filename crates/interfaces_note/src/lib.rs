//! **The `INTERFACES` note**: the interface versions a program was built against.
//!
//! A package declares what it links by carrying one note in `.note.nife.manifest`, the section and
//! `PT_NOTE` that `capability_manifest_note` uses for its own types. The progenitor reads it at
//! spawn and refuses a program built against an interface version it does not serve (milestone
//! 956 (the linked interfaces carry checkable versions) step 3). The note has a type of its own,
//! `4`, on the pattern of `SUBTREE_GRANTS` and `VERBS`, and not a descriptor version.
//!
//! The descriptor, version 1, little-endian: a version word, a count word, then `count` records of
//! an interface id (`u64`, each crate's `INTERFACE_ID`, strictly ascending) and that interface's
//! version (`u32`). One list has one encoding.
//!
//! The note's header (owner `nife`, then the type and the descriptor) is the one every nife
//! manifest note has; `capability_manifest_note`'s tests hold the two crates to the same header.
//!
//! BUGS: a program names its interfaces by hand in [`carry_interfaces!`], although each version
//! comes from the crate's constant. Deriving the list from the dependency graph at package build
//! waits for the first package that leaves the tree.
//!
//! Name: provisional. Moved here from `capability_manifest_note` on 2026-11-01 (UTC) so that
//! `disk-tools` need not link `grant_plan`; the items keep the names they had there. The crate
//! name is the architect's call under the two-way rule (reversing it costs one pull request).

#![no_std]

/// The note's owner string, without its NUL: the same `nife` that `capability_manifest_note::OWNER`
/// holds. Duplicated so that this crate depends on nothing; a test in that crate holds the two equal.
const OWNER: &[u8] = b"nife";

/// `OWNER` with its NUL, padded to the note's 4-byte alignment.
const NAME_FIELD: usize = 8;

/// The note's three header words.
const HEADER_LEN: usize = 12;

/// Why a descriptor was refused.
///
/// Name: provisional, minted 2026-11-01 (UTC) by lane milestone/956-the-interfaces-note-leaves-the-manifest-note-crate.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Error {
    /// Fewer than four bytes, so not even a version.
    NoVersion,
    /// A version this reader does not know. A later layout is refused rather than guessed at.
    UnknownVersion,
    /// A descriptor that is not exactly as long as its count says: cut short, or with bytes after
    /// the last record.
    WrongLength,
    /// A field holds a value outside its range, or ids that do not strictly ascend. The offset is
    /// the first such byte.
    BadField(usize),
}

/// Four bytes at `at`, in order. `const` because a note is built at compile time.
const fn put4(out: &mut [u8], at: usize, v: [u8; 4]) {
    let mut i = 0;
    while i < 4 {
        out[at + i] = v[i];
        i += 1;
    }
}

/// The note type that carries the interfaces a program was built against: `4`.
///
/// Name: provisional; the number and the placement are calef's ruling of 2026-10-31 (UTC).
pub const INTERFACES: u32 = 4;

/// **This interface's number**: `21`. It is this interface's for good: no other interface takes
/// it, and a format change is a new version ([`INTERFACES_VERSION`]), never a new number. Milestone
/// 956 (the linked interfaces carry checkable versions) holds the table of numbers, and
/// `script/lint` holds the tree to it. A program declares the pair of this id and
/// [`INTERFACES_VERSION`].
///
/// Name: provisional, minted 2026-11-01 (UTC) by lane milestone/956-the-interfaces-note-leaves-the-manifest-note-crate.
pub const INTERFACE_ID: u64 = 21;

/// The version of the [`INTERFACES`] descriptor this crate writes and reads.
///
/// Name: provisional, minted 2026-10-31 (UTC) by lane milestone/956-checkable-interface-versions.
pub const INTERFACES_VERSION: u32 = 1;

/// The most interfaces one note may declare: 64, far past the handful a program links, and a bound
/// on what a malformed count can make a reader walk.
///
/// Name: provisional, minted 2026-10-31 (UTC) by lane milestone/956-checkable-interface-versions.
pub const MAXIMUM_INTERFACES: usize = 64;

/// One record: the interface id, then its version, little-endian.
///
/// Name: provisional, minted 2026-10-31 (UTC) by lane milestone/956-checkable-interface-versions.
pub const INTERFACE_RECORD: usize = 12;

/// One declared interface: the id its crate owns and the version the program was built against.
///
/// Name: provisional, minted 2026-10-31 (UTC) by lane milestone/956-checkable-interface-versions.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Declared {
    /// The crate's `INTERFACE_ID`.
    pub id: u64,
    /// The crate's version constant when the program was built.
    pub version: u32,
}

/// An [`INTERFACES`] descriptor's length for `count` records.
///
/// Name: provisional, minted 2026-10-31 (UTC) by lane milestone/956-checkable-interface-versions.
pub const fn interfaces_len(count: usize) -> usize {
    8 + count * INTERFACE_RECORD
}

/// The bytes of an interface list. A `const fn`, so a program's list is computed by the compiler;
/// `LEN` is [`interfaces_len`] of the list's length. It panics, a compile error in a constant, on a
/// `LEN` that is not that length, more than [`MAXIMUM_INTERFACES`] entries, or ids that do not
/// strictly ascend.
///
/// Name: provisional, minted 2026-10-31 (UTC) by lane milestone/956-checkable-interface-versions.
pub const fn encode_interfaces<const LEN: usize>(list: &[Declared]) -> [u8; LEN] {
    assert!(
        list.len() <= MAXIMUM_INTERFACES,
        "an interface list holds at most 64 entries"
    );
    assert!(
        LEN == interfaces_len(list.len()),
        "LEN must be interfaces_len(list.len())"
    );
    let mut out = [0u8; LEN];
    put4(&mut out, 0, INTERFACES_VERSION.to_le_bytes());
    put4(&mut out, 4, (list.len() as u32).to_le_bytes());
    let mut i = 0;
    while i < list.len() {
        if i > 0 {
            assert!(
                list[i - 1].id < list[i].id,
                "interface ids strictly ascend, each once"
            );
        }
        let at = 8 + i * INTERFACE_RECORD;
        let id = list[i].id.to_le_bytes();
        put4(&mut out, at, [id[0], id[1], id[2], id[3]]);
        put4(&mut out, at + 4, [id[4], id[5], id[6], id[7]]);
        put4(&mut out, at + 8, list[i].version.to_le_bytes());
        i += 1;
    }
    out
}

/// **A decoded interface list**: a view of the descriptor [`decode_interfaces`] checked whole.
///
/// Name: provisional, minted 2026-10-31 (UTC) by lane milestone/956-checkable-interface-versions.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Interfaces<'a> {
    records: &'a [u8],
}

impl<'a> Interfaces<'a> {
    /// How many interfaces.
    ///
    /// Name: provisional, minted 2026-10-31 (UTC) by lane milestone/956-checkable-interface-versions.
    pub fn len(&self) -> usize {
        self.records.len() / INTERFACE_RECORD
    }

    /// Whether the list is empty, which a descriptor may say.
    ///
    /// Name: provisional, minted 2026-10-31 (UTC) by lane milestone/956-checkable-interface-versions.
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// The declared interfaces in ascending id order.
    ///
    /// Name: provisional, minted 2026-10-31 (UTC) by lane milestone/956-checkable-interface-versions.
    pub fn iter(&self) -> impl Iterator<Item = Declared> + use<'a> {
        let records = self.records;
        (0..records.len() / INTERFACE_RECORD)
            .map(move |i| record(&records[i * INTERFACE_RECORD..(i + 1) * INTERFACE_RECORD]))
    }

    /// The version declared for `id`, or `None` when the program declares nothing about it. A
    /// linear walk: a list holds at most 64 records.
    ///
    /// Name: provisional, minted 2026-10-31 (UTC) by lane milestone/956-checkable-interface-versions.
    pub fn version_of(&self, id: u64) -> Option<u32> {
        self.iter().find(|d| d.id == id).map(|d| d.version)
    }
}

fn record(r: &[u8]) -> Declared {
    Declared {
        id: u64::from_le_bytes([r[0], r[1], r[2], r[3], r[4], r[5], r[6], r[7]]),
        version: u32::from_le_bytes([r[8], r[9], r[10], r[11]]),
    }
}

/// **An [`INTERFACES`] descriptor, decoded**, or why not. An unknown version, a count past
/// [`MAXIMUM_INTERFACES`] or one that disagrees with the length, and ids that do not strictly
/// ascend all refuse, so no second spelling of a list decodes.
///
/// Name: provisional, minted 2026-10-31 (UTC) by lane milestone/956-checkable-interface-versions.
pub fn decode_interfaces(d: &[u8]) -> Result<Interfaces<'_>, Error> {
    let version = d.get(..4).ok_or(Error::NoVersion)?;
    if u32::from_le_bytes([version[0], version[1], version[2], version[3]]) != INTERFACES_VERSION {
        return Err(Error::UnknownVersion);
    }
    let count = d.get(4..8).ok_or(Error::WrongLength)?;
    let count = u32::from_le_bytes([count[0], count[1], count[2], count[3]]) as usize;
    if count > MAXIMUM_INTERFACES {
        return Err(Error::BadField(4));
    }
    if d.len() != interfaces_len(count) {
        return Err(Error::WrongLength);
    }
    let mut previous: Option<u64> = None;
    for i in 0..count {
        let at = 8 + i * INTERFACE_RECORD;
        let id = record(&d[at..at + INTERFACE_RECORD]).id;
        if previous.is_some_and(|p| p >= id) {
            return Err(Error::BadField(at));
        }
        previous = Some(id);
    }
    Ok(Interfaces { records: &d[8..] })
}

/// **A program was built against a version of an interface that its host does not serve**, with
/// both sides: what the program declared and what the host serves. What the progenitor refuses a
/// spawn with (milestone 956 step 3), so the refusal names both.
///
/// Name: provisional, minted 2026-10-31 (UTC) by lane milestone/956-checkable-interface-versions.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Refusal {
    /// The interface's `INTERFACE_ID`.
    pub interface: u64,
    /// The version the program declared.
    pub declared: u32,
    /// The version the host serves.
    pub served: u32,
}

impl Interfaces<'_> {
    /// **Whether this list's entry for `interface` is the version the host serves.** `Ok` when the
    /// entry equals `served`, and also when the program declares nothing about `interface`: a
    /// program without the entry runs as every program did before the note existed, and refusing it
    /// waits on packages carrying the stamp. A different version is a [`Refusal`] naming both. This
    /// is equality, not set membership: the syscall ABI never dual-serves.
    ///
    /// Name: provisional, minted 2026-10-31 (UTC) by lane milestone/956-checkable-interface-versions.
    pub fn check_served(&self, interface: u64, served: u32) -> Result<(), Refusal> {
        match self.version_of(interface) {
            Some(declared) if declared != served => Err(Refusal {
                interface,
                declared,
                served,
            }),
            _ => Ok(()),
        }
    }
}

/// How long a whole [`INTERFACES`] note is for `count` records: header, owner and descriptor. Every
/// part is a multiple of four, so there is no trailing pad.
///
/// Name: provisional, minted 2026-10-31 (UTC) by lane milestone/956-checkable-interface-versions.
pub const fn interfaces_note_len(count: usize) -> usize {
    HEADER_LEN + NAME_FIELD + interfaces_len(count)
}

/// **A whole [`INTERFACES`] note**, what [`carry_interfaces!`] places. `N` is
/// [`interfaces_note_len`] of the list.
///
/// Name: provisional, minted 2026-10-31 (UTC) by lane milestone/956-checkable-interface-versions.
#[repr(C, align(4))]
pub struct InterfacesNote<const N: usize>(pub [u8; N]);

impl<const N: usize> InterfacesNote<N> {
    /// The note declaring `list`.
    ///
    /// Name: provisional, minted 2026-10-31 (UTC) by lane milestone/956-checkable-interface-versions.
    pub const fn of<const LEN: usize>(list: &[Declared]) -> Self {
        assert!(
            N == interfaces_note_len(list.len()),
            "N must be interfaces_note_len(list.len())"
        );
        let mut out = [0u8; N];
        put4(&mut out, 0, ((OWNER.len() + 1) as u32).to_le_bytes());
        put4(&mut out, 4, (LEN as u32).to_le_bytes());
        put4(&mut out, 8, INTERFACES.to_le_bytes());
        put4(
            &mut out,
            HEADER_LEN,
            [OWNER[0], OWNER[1], OWNER[2], OWNER[3]],
        );
        let d: [u8; LEN] = encode_interfaces(list);
        let mut i = 0;
        while i < LEN {
            out[HEADER_LEN + NAME_FIELD + i] = d[i];
            i += 1;
        }
        Self(out)
    }
}

/// **Declare the interface versions this program was built against.** One line at module scope in
/// the program's root. It takes a `&[Declared]` in ascending id order, each version read from the
/// interface crate's constant, and places one [`InterfacesNote`] in `.note.nife.manifest`, the
/// section and `PT_NOTE` a manifest uses:
///
/// ```ignore
/// interfaces_note::carry_interfaces!(&[
///     interfaces_note::Declared { id: abi::INTERFACE_ID, version: abi::ABI_VERSION },
/// ]);
/// ```
///
/// An unsorted list is a compile error.
///
/// Name: provisional, minted 2026-10-31 (UTC) by lane milestone/956-checkable-interface-versions.
#[macro_export]
macro_rules! carry_interfaces {
    ($list:expr) => {
        #[used]
        #[unsafe(link_section = ".note.nife.manifest")]
        static NIFE_INTERFACES_NOTE: $crate::InterfacesNote<
            { $crate::interfaces_note_len($list.len()) },
        > = $crate::InterfacesNote::of::<{ $crate::interfaces_len($list.len()) }>($list);
    };
}

#[cfg(kani)]
mod interface_verification {
    use super::*;

    /// **Whatever `decode_interfaces` accepts, `encode_interfaces` spells back byte for byte**,
    /// for a list of two. This is the one-encoding claim of the manifest note's own harness,
    /// carried to this note.
    ///
    /// Falsification: unfalsified. Kani does not run in the lane that wrote this; the gate does.
    #[kani::proof]
    #[kani::unwind(33)]
    fn a_decoded_interface_list_encodes_back_to_its_bytes() {
        let d: [u8; interfaces_len(2)] = kani::any();
        if let Ok(list) = decode_interfaces(&d) {
            let mut declared = [Declared { id: 0, version: 0 }; 2];
            let mut n = 0;
            for entry in list.iter() {
                declared[n] = entry;
                n += 1;
            }
            assert!(n == 2);
            assert!(encode_interfaces::<{ interfaces_len(2) }>(&declared) == d);
        }
    }

    /// **`decode_interfaces` never panics, at any length up to one past a list of two.**
    ///
    /// Falsification: unfalsified. Kani does not run in the lane that wrote this; the gate does.
    #[kani::proof]
    #[kani::unwind(34)]
    fn decode_interfaces_never_panics() {
        let d: [u8; interfaces_len(2) + 1] = kani::any();
        let len: usize = kani::any();
        kani::assume(len <= d.len());
        let _ = decode_interfaces(&d[..len]);
    }
}

#[cfg(test)]
mod interface_tests {
    extern crate std;

    use super::*;

    const LIST: [Declared; 3] = [
        Declared { id: 1, version: 1 },
        Declared { id: 4, version: 7 },
        Declared {
            id: 0x0102_0304_0506_0708,
            version: 0x0a0b_0c0d,
        },
    ];

    /// **A list round-trips, and the wire bytes are where the table says.** An id whose eight bytes
    /// all differ and a version whose four differ, so a swapped or shifted byte shows.
    #[test]
    fn a_list_round_trips_at_the_documented_offsets() {
        let bytes: [u8; interfaces_len(3)] = encode_interfaces(&LIST);
        assert_eq!(&bytes[..8], &[1, 0, 0, 0, 3, 0, 0, 0]);
        assert_eq!(
            &bytes[8 + 24..],
            &[8, 7, 6, 5, 4, 3, 2, 1, 0x0d, 0x0c, 0x0b, 0x0a]
        );
        let list = decode_interfaces(&bytes).unwrap();
        assert_eq!(list.len(), 3);
        assert!(list.iter().eq(LIST.iter().copied()));
        assert_eq!(list.version_of(4), Some(7));
        assert_eq!(list.version_of(0x0102_0304_0506_0708), Some(0x0a0b_0c0d));
        assert_eq!(list.version_of(2), None);
        assert_eq!(list.version_of(u64::MAX), None);
    }

    /// **An empty list is a list**: a program may declare nothing and still carry the note.
    #[test]
    fn an_empty_list_round_trips() {
        let bytes: [u8; interfaces_len(0)] = encode_interfaces(&[]);
        let list = decode_interfaces(&bytes).unwrap();
        assert!(list.is_empty());
        assert_eq!(list.version_of(1), None);
        let one: [u8; interfaces_len(1)] = encode_interfaces(&[Declared { id: 9, version: 2 }]);
        assert!(!decode_interfaces(&one).unwrap().is_empty());
    }

    /// **The largest list decodes and is searched whole**, and one more record is refused: the
    /// bound is on the count, `MAXIMUM_INTERFACES` itself being allowed.
    #[test]
    fn a_list_of_exactly_the_maximum_round_trips() {
        let mut list = [Declared { id: 0, version: 0 }; MAXIMUM_INTERFACES];
        for (i, entry) in list.iter_mut().enumerate() {
            *entry = Declared {
                id: i as u64 + 1,
                version: i as u32 + 100,
            };
        }
        let bytes: [u8; interfaces_len(MAXIMUM_INTERFACES)] = encode_interfaces(&list);
        let decoded = decode_interfaces(&bytes).unwrap();
        assert_eq!(decoded.len(), MAXIMUM_INTERFACES);
        assert_eq!(decoded.version_of(1), Some(100));
        assert_eq!(decoded.version_of(64), Some(163));
        assert_eq!(decoded.version_of(65), None);
        let mut over = [0u8; interfaces_len(MAXIMUM_INTERFACES + 1)];
        over[..bytes.len()].copy_from_slice(&bytes);
        over[0..4].copy_from_slice(&INTERFACES_VERSION.to_le_bytes());
        over[4..8].copy_from_slice(&(MAXIMUM_INTERFACES as u32 + 1).to_le_bytes());
        assert_eq!(decode_interfaces(&over), Err(Error::BadField(4)));
    }

    /// **One list, one encoding**: each byte the layout fixes is checked, and so is the order.
    #[test]
    fn a_respelled_list_is_refused() {
        let good: [u8; interfaces_len(3)] = encode_interfaces(&LIST);
        let refused = |at: usize, b: u8| {
            let mut d = good;
            d[at] = b;
            decode_interfaces(&d).err()
        };
        assert_eq!(refused(0, 2), Some(Error::UnknownVersion));
        assert_eq!(refused(4, 2), Some(Error::WrongLength));
        assert_eq!(
            refused(5, 1),
            Some(Error::BadField(4)),
            "a count past the maximum"
        );
        // The first two records swapped, and the first one repeated: every byte valid, the order not.
        let mut swapped = good;
        swapped.copy_within(8 + 12..8 + 24, 8);
        swapped[8 + 12..8 + 24].copy_from_slice(&good[8..8 + 12]);
        assert_eq!(decode_interfaces(&swapped), Err(Error::BadField(8 + 12)));
        let mut twice = good;
        twice.copy_within(8..8 + 12, 8 + 12);
        assert_eq!(decode_interfaces(&twice), Err(Error::BadField(8 + 12)));
        assert_eq!(decode_interfaces(&good[..3]), Err(Error::NoVersion));
        assert_eq!(decode_interfaces(&good[..6]), Err(Error::WrongLength));
        assert_eq!(
            decode_interfaces(&good[..good.len() - 1]),
            Err(Error::WrongLength)
        );
    }

    /// **The encoder refuses an unsorted or repeated list**, which in a constant is a compile error.
    #[test]
    #[should_panic(expected = "ascend")]
    fn an_unsorted_list_is_not_encoded() {
        let _: [u8; interfaces_len(2)] = encode_interfaces(&[
            Declared { id: 2, version: 1 },
            Declared { id: 1, version: 1 },
        ]);
    }

    /// **A whole note has the header, the owner, the type and the descriptor** that `crates/elf`
    /// finds, and nothing after.
    #[test]
    fn a_whole_note_carries_its_header_and_descriptor() {
        const N: usize = interfaces_note_len(3);
        let note = InterfacesNote::<N>::of::<{ interfaces_len(3) }>(&LIST);
        let b = note.0;
        assert_eq!(&b[..4], &5u32.to_le_bytes());
        assert_eq!(&b[4..8], &(interfaces_len(3) as u32).to_le_bytes());
        assert_eq!(&b[8..12], &INTERFACES.to_le_bytes());
        assert_eq!(&b[12..16], b"nife");
        assert_eq!(&b[16..20], &[0, 0, 0, 0]);
        assert_eq!(b.len(), 20 + interfaces_len(3));
        assert_eq!(decode_interfaces(&b[20..]).unwrap().version_of(4), Some(7));
    }

    /// **The spawn-time check** (milestone 956 step 3): a program that declares the version the
    /// host serves is accepted, one that declares another is refused with both numbers, and one
    /// that says nothing about the interface is accepted. Either side of the served version
    /// refuses, so an older program is not read as compatible with a newer host.
    #[test]
    fn a_declared_version_must_be_the_one_served() {
        let bytes: [u8; interfaces_len(3)] = encode_interfaces(&LIST);
        let list = decode_interfaces(&bytes).unwrap();
        assert_eq!(list.check_served(4, 7), Ok(()));
        let refused = |served| list.check_served(4, served);
        assert_eq!(
            refused(8),
            Err(Refusal {
                interface: 4,
                declared: 7,
                served: 8
            })
        );
        assert_eq!(
            refused(6),
            Err(Refusal {
                interface: 4,
                declared: 7,
                served: 6
            })
        );
        assert_eq!(
            list.check_served(2, 99),
            Ok(()),
            "no entry, nothing to refuse"
        );
        let empty: [u8; interfaces_len(0)] = encode_interfaces(&[]);
        assert_eq!(
            decode_interfaces(&empty).unwrap().check_served(1, 1),
            Ok(())
        );
    }
}
