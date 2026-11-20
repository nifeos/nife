//! **The `INTERFACES` note**: the interface versions a program was built against.
//!
//! A package declares what it links by carrying one note in its manifest note section, the section
//! and `PT_NOTE` that a family of manifest notes shares. The host that spawns the program reads it
//! and refuses a program whose note does not decode, or declares a version of an interface that the
//! host serves at another version. The other interfaces a note lists are carried and not compared
//! at spawn. The note has a type of its own, `4`, and not a descriptor version.
//!
//! The descriptor, version 1, little-endian: a version word, a count word, then `count` records of
//! an interface id (`u64`, each interface crate's `INTERFACE_ID`, strictly ascending) and that
//! interface's version (`u32`). One list has one encoding.
//!
//! The note's header (the owner string, then the type and the descriptor) is the one every note in
//! that section has. The owner string names the operating system that reads the note.
//!
//! # What a stranger needs to place and read the note
//!
//! - Every word is little-endian, whatever the target's byte order: the encoder writes each one
//!   with `to_le_bytes`, and an ELF note's own header is in the file's byte order, so a big-endian
//!   file would carry a header this crate does not write.
//! - The linker must keep the section: the link script has to `KEEP` the input section
//!   the macro names into an output section inside a `PT_NOTE` segment, and that segment
//!   has to lie in a readable `PT_LOAD`. A program whose link script drops it carries no note,
//!   which the next rule reads as declaring nothing.
//! - An absent note, and an absent entry for an interface, pass [`Interfaces::check_served`]: only
//!   a different version refuses.
//! - Each interface crate owns its id for good; the registry of ids lives with the system that
//!   reads the note, and this crate reserves `21` for itself.
//!
//! BUGS: a program names its interfaces by hand in [`carry_interfaces!`], although each version
//! comes from the crate's constant. Deriving the list from the dependency graph at package build
//! waits for a package that links its interfaces as separate crates.
//!
//! BUGS: a program with no `INTERFACES` note declares nothing and is spawned unchecked. The note is
//! a compatibility check and not an authority check, so leaving it out costs a hostile package
//! nothing it could have gained.
//!
//! Name: ratified 2026-11-14 (UTC) for the crate and repository name `interfaces_note`. The
//! crates.io name is asked again with the naming-layers decision.

#![no_std]

/// The note's owner string, without its NUL terminator. The sibling manifest-note crate holds the
/// same bytes; this crate repeats them so that it depends on nothing.
const OWNER: &[u8] = b"nife"; // wire literal

/// `OWNER` with its NUL terminator, padded to the note's 4-byte alignment.
const NAME_FIELD_BYTES: usize = 8;

/// The note's three header words.
const HEADER_BYTES: usize = 12;

/// Why a descriptor was refused.
///
/// Name: ratified 2026-11-19 (UTC) with the review package's rulings, as built.
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
/// Name: ratified 2026-11-19 (UTC) with the review package's rulings, as built.
pub const INTERFACES: u32 = 4;

/// **This interface's number**: `21`. It is this interface's for good: no other interface takes
/// it, and a format change is a new version ([`INTERFACES_VERSION`]), never a new number. The
/// system that reads the note holds the table of numbers. A program declares the pair of this id and
/// [`INTERFACES_VERSION`].
///
/// Name: ratified 2026-11-19 (UTC) with the review package's rulings, as built.
pub const INTERFACE_ID: u64 = 21;

/// The version of the [`INTERFACES`] descriptor this crate writes and reads.
///
/// Name: ratified 2026-11-19 (UTC) with the review package's rulings, as built.
pub const INTERFACES_VERSION: u32 = 1;

/// The most interfaces one note may declare: 64, far past the handful a program links, and a bound
/// on what a malformed count can make a reader walk.
///
/// Name: ratified 2026-11-19 (UTC) with the review package's rulings, as built.
pub const MAXIMUM_INTERFACES: usize = 64;

/// One record: the interface id, then its version, little-endian.
///
/// Name: ratified 2026-11-19 (UTC) with the review package's rulings, as built.
pub const INTERFACE_RECORD_BYTES: usize = 12;

/// One declared interface: the id its crate owns and the version the program was built against.
///
/// Name: ratified 2026-11-19 (UTC) with the review package's rulings, as built.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DeclaredInterface {
    /// The crate's `INTERFACE_ID`.
    pub id: u64,
    /// The crate's version constant when the program was built.
    pub version: u32,
}

/// An [`INTERFACES`] descriptor's length for `count` records.
///
/// Name: ratified 2026-11-19 (UTC) with the review package's rulings, as built.
pub const fn descriptor_bytes(count: usize) -> usize {
    8 + count * INTERFACE_RECORD_BYTES
}

/// The bytes of an interface list. A `const fn`, so a program's list is computed by the compiler;
/// `LEN` is [`descriptor_bytes`] of the list's length. It panics, a compile error in a constant, on a
/// `LEN` that is not that length, more than [`MAXIMUM_INTERFACES`] entries, or ids that do not
/// strictly ascend.
///
/// # Examples
///
/// ```
/// use interfaces_note::{DeclaredInterface, decode_interfaces, encode_interfaces, descriptor_bytes};
/// let bytes: [u8; descriptor_bytes(2)] = encode_interfaces(&[
///     DeclaredInterface { id: 1, version: 3 },
///     DeclaredInterface { id: 9, version: 1 },
/// ]);
/// assert_eq!(decode_interfaces(&bytes).unwrap().len(), 2);
/// ```
///
/// Name: ratified 2026-11-19 (UTC) with the review package's rulings, as built.
pub const fn encode_interfaces<const LEN: usize>(list: &[DeclaredInterface]) -> [u8; LEN] {
    assert!(
        list.len() <= MAXIMUM_INTERFACES,
        "an interface list holds at most 64 entries"
    );
    assert!(
        LEN == descriptor_bytes(list.len()),
        "LEN must be descriptor_bytes(list.len())"
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
        let at = 8 + i * INTERFACE_RECORD_BYTES;
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
/// Name: ratified 2026-11-19 (UTC) with the review package's rulings, as built.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Interfaces<'a> {
    records: &'a [u8],
}

impl<'a> Interfaces<'a> {
    /// How many interfaces.
    ///
    /// Name: ratified 2026-11-19 (UTC) with the review package's rulings, as built.
    pub fn len(&self) -> usize {
        self.records.len() / INTERFACE_RECORD_BYTES
    }

    /// Whether the list is empty, which a descriptor may say.
    ///
    /// Name: ratified 2026-11-19 (UTC) with the review package's rulings, as built.
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// The declared interfaces in ascending id order. They come by value, not by reference: the
    /// iterator decodes each record from the descriptor's bytes as it goes, so none is stored to
    /// borrow.
    ///
    /// Name: ratified 2026-11-19 (UTC) with the review package's rulings, as built.
    pub fn iter(&self) -> impl Iterator<Item = DeclaredInterface> + use<'a> {
        let records = self.records;
        (0..records.len() / INTERFACE_RECORD_BYTES).map(move |i| {
            record(&records[i * INTERFACE_RECORD_BYTES..(i + 1) * INTERFACE_RECORD_BYTES])
        })
    }

    /// The version declared for `id`, or `None` when the program declares nothing about it. A
    /// linear walk: a list holds at most 64 records.
    ///
    /// ```
    /// use interfaces_note::{DeclaredInterface, decode_interfaces, encode_interfaces, descriptor_bytes};
    /// let bytes: [u8; descriptor_bytes(1)] = encode_interfaces(&[DeclaredInterface { id: 4, version: 7 }]);
    /// let list = decode_interfaces(&bytes).unwrap();
    /// assert_eq!(list.version_of(4), Some(7));
    /// assert_eq!(list.version_of(5), None);
    /// ```
    ///
    /// Name: ratified 2026-11-19 (UTC) with the review package's rulings, as built.
    pub fn version_of(&self, id: u64) -> Option<u32> {
        self.iter().find(|d| d.id == id).map(|d| d.version)
    }
}

fn record(r: &[u8]) -> DeclaredInterface {
    DeclaredInterface {
        id: u64::from_le_bytes([r[0], r[1], r[2], r[3], r[4], r[5], r[6], r[7]]),
        version: u32::from_le_bytes([r[8], r[9], r[10], r[11]]),
    }
}

/// **An [`INTERFACES`] descriptor, decoded**, or why not. An unknown version, a count past
/// [`MAXIMUM_INTERFACES`] or one that disagrees with the length, and ids that do not strictly
/// ascend all refuse, so no second spelling of a list decodes.
///
/// # Examples
///
/// ```
/// use interfaces_note::{Error, decode_interfaces};
/// assert_eq!(decode_interfaces(&[1, 0]), Err(Error::NoVersion));
/// assert_eq!(decode_interfaces(&[7, 0, 0, 0, 0, 0, 0, 0]), Err(Error::UnknownVersion));
/// assert!(decode_interfaces(&[1, 0, 0, 0, 0, 0, 0, 0]).unwrap().is_empty());
/// ```
///
/// Name: ratified 2026-11-19 (UTC) with the review package's rulings, as built.
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
    if d.len() != descriptor_bytes(count) {
        return Err(Error::WrongLength);
    }
    let mut previous: Option<u64> = None;
    for i in 0..count {
        let at = 8 + i * INTERFACE_RECORD_BYTES;
        let id = record(&d[at..at + INTERFACE_RECORD_BYTES]).id;
        if previous.is_some_and(|p| p >= id) {
            return Err(Error::BadField(at));
        }
        previous = Some(id);
    }
    Ok(Interfaces { records: &d[8..] })
}

/// **A program was built against a version of an interface that its host does not serve**, with
/// both sides: what the program declared and what the host serves. The host refuses a spawn with
/// it, so the refusal names both.
///
/// Name: ratified 2026-11-19 (UTC) with the review package's rulings, as built.
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
    /// ```
    /// use interfaces_note::{DeclaredInterface, decode_interfaces, encode_interfaces, descriptor_bytes};
    /// let bytes: [u8; descriptor_bytes(1)] = encode_interfaces(&[DeclaredInterface { id: 4, version: 7 }]);
    /// let list = decode_interfaces(&bytes).unwrap();
    /// assert!(list.check_served(4, 7).is_ok());
    /// assert!(list.check_served(4, 8).is_err());
    /// assert!(list.check_served(5, 1).is_ok(), "no entry, nothing to refuse");
    /// ```
    ///
    /// Name: ratified 2026-11-19 (UTC) with the review package's rulings, as built.
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
/// Name: ratified 2026-11-19 (UTC) with the review package's rulings, as built.
pub const fn note_bytes(count: usize) -> usize {
    HEADER_BYTES + NAME_FIELD_BYTES + descriptor_bytes(count)
}

/// **A whole [`INTERFACES`] note**, what [`carry_interfaces!`] places. `N` is
/// [`note_bytes`] of the list.
///
/// Name: ratified 2026-11-19 (UTC) with the review package's rulings, as built.
#[repr(C, align(4))]
pub struct InterfacesNote<const N: usize>(pub [u8; N]);

impl<const N: usize> InterfacesNote<N> {
    /// The note declaring `list`.
    ///
    /// Name: ratified 2026-11-19 (UTC) with the review package's rulings, as built.
    pub const fn of<const LEN: usize>(list: &[DeclaredInterface]) -> Self {
        assert!(
            N == note_bytes(list.len()),
            "N must be note_bytes(list.len())"
        );
        let mut out = [0u8; N];
        put4(&mut out, 0, ((OWNER.len() + 1) as u32).to_le_bytes());
        put4(&mut out, 4, (LEN as u32).to_le_bytes());
        put4(&mut out, 8, INTERFACES.to_le_bytes());
        put4(
            &mut out,
            HEADER_BYTES,
            [OWNER[0], OWNER[1], OWNER[2], OWNER[3]],
        );
        let d: [u8; LEN] = encode_interfaces(list);
        let mut i = 0;
        while i < LEN {
            out[HEADER_BYTES + NAME_FIELD_BYTES + i] = d[i];
            i += 1;
        }
        Self(out)
    }
}

/// **Declare the interface versions this program was built against.** One line at module scope in
/// the program's root. It takes a `&[DeclaredInterface]` in ascending id order, each version read from the
/// interface crate's constant, and places one [`InterfacesNote`] in the manifest note section, the
/// section and `PT_NOTE` a manifest uses:
///
/// ```
/// interfaces_note::carry_interfaces!(&[
///     interfaces_note::DeclaredInterface { id: 1, version: 1 },
/// ]);
/// ```
///
/// An unsorted list is a compile error. The static it declares is named `CARRIED_INTERFACES_NOTE`,
/// so a program carries one list.
///
/// Name: ratified 2026-11-19 (UTC) with the review package's rulings, as built.
#[macro_export]
macro_rules! carry_interfaces {
    ($list:expr) => {
        #[used]
        #[unsafe(link_section = ".note.nife.manifest")] // wire literal
        static CARRIED_INTERFACES_NOTE: $crate::InterfacesNote<
            { $crate::note_bytes($list.len()) },
        > = $crate::InterfacesNote::of::<{ $crate::descriptor_bytes($list.len()) }>($list);
    };
}

#[cfg(kani)]
mod interface_verification {
    use super::*;

    /// **Whatever `decode_interfaces` accepts, `encode_interfaces` spells back byte for byte**,
    /// for a list of two. This is the one-encoding claim: one list has one encoding.
    ///
    /// Falsification: replayable `crates/interfaces_note/falsifications/interface_verification.a_decoded_interface_list_encodes_back_to_its_bytes.patch`
    #[kani::proof]
    #[kani::unwind(33)]
    fn a_decoded_interface_list_encodes_back_to_its_bytes() {
        let d: [u8; descriptor_bytes(2)] = kani::any();
        if let Ok(list) = decode_interfaces(&d) {
            let mut declared = [DeclaredInterface { id: 0, version: 0 }; 2];
            let mut n = 0;
            for entry in list.iter() {
                declared[n] = entry;
                n += 1;
            }
            assert!(n == 2);
            assert!(encode_interfaces::<{ descriptor_bytes(2) }>(&declared) == d);
        }
    }

    /// **Whatever `encode_interfaces` writes, `decode_interfaces` reads back**, for a list of two
    /// with symbolic versions and symbolic ascending ids: the other direction of the one-encoding
    /// claim.
    ///
    /// Falsification: replayable `crates/interfaces_note/falsifications/interface_verification.an_encoded_interface_list_decodes_back_to_itself.patch`
    #[kani::proof]
    #[kani::unwind(33)]
    fn an_encoded_interface_list_decodes_back_to_itself() {
        let (first_id, second_id): (u64, u64) = (kani::any(), kani::any());
        kani::assume(first_id < second_id);
        let list = [
            DeclaredInterface {
                id: first_id,
                version: kani::any(),
            },
            DeclaredInterface {
                id: second_id,
                version: kani::any(),
            },
        ];
        let bytes: [u8; descriptor_bytes(2)] = encode_interfaces(&list);
        let Ok(decoded) = decode_interfaces(&bytes) else {
            panic!("an encoded list was refused");
        };
        let mut n = 0;
        for entry in decoded.iter() {
            assert!(n < 2 && entry == list[n]);
            n += 1;
        }
        assert!(n == 2);
    }

    /// **`decode_interfaces` never panics, at any length up to one past a list of two.**
    ///
    /// Falsification: replayable `crates/interfaces_note/falsifications/interface_verification.decode_interfaces_never_panics.patch`
    #[kani::proof]
    #[kani::unwind(34)]
    fn decode_interfaces_never_panics() {
        let d: [u8; descriptor_bytes(2) + 1] = kani::any();
        let len: usize = kani::any();
        kani::assume(len <= d.len());
        let _ = decode_interfaces(&d[..len]);
    }
}

#[cfg(test)]
mod interface_tests {
    extern crate std;

    use super::*;

    const LIST: [DeclaredInterface; 3] = [
        DeclaredInterface { id: 1, version: 1 },
        DeclaredInterface { id: 4, version: 7 },
        DeclaredInterface {
            id: 0x0102_0304_0506_0708,
            version: 0x0a0b_0c0d,
        },
    ];

    /// **A list round-trips, and the wire bytes are where the table says.** An id whose eight bytes
    /// all differ and a version whose four differ, so a swapped or shifted byte shows.
    #[test]
    fn a_list_round_trips_at_the_documented_offsets() {
        let bytes: [u8; descriptor_bytes(3)] = encode_interfaces(&LIST);
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
        let bytes: [u8; descriptor_bytes(0)] = encode_interfaces(&[]);
        let list = decode_interfaces(&bytes).unwrap();
        assert!(list.is_empty());
        assert_eq!(list.version_of(1), None);
        let one: [u8; descriptor_bytes(1)] =
            encode_interfaces(&[DeclaredInterface { id: 9, version: 2 }]);
        assert!(!decode_interfaces(&one).unwrap().is_empty());
    }

    /// **The largest list decodes and is searched whole**, and one more record is refused: the
    /// bound is on the count, `MAXIMUM_INTERFACES` itself being allowed.
    #[test]
    fn a_list_of_exactly_the_maximum_round_trips() {
        let mut list = [DeclaredInterface { id: 0, version: 0 }; MAXIMUM_INTERFACES];
        for (i, entry) in list.iter_mut().enumerate() {
            *entry = DeclaredInterface {
                id: i as u64 + 1,
                version: i as u32 + 100,
            };
        }
        let bytes: [u8; descriptor_bytes(MAXIMUM_INTERFACES)] = encode_interfaces(&list);
        let decoded = decode_interfaces(&bytes).unwrap();
        assert_eq!(decoded.len(), MAXIMUM_INTERFACES);
        assert_eq!(decoded.version_of(1), Some(100));
        assert_eq!(decoded.version_of(64), Some(163));
        assert_eq!(decoded.version_of(65), None);
        let mut over = [0u8; descriptor_bytes(MAXIMUM_INTERFACES + 1)];
        over[..bytes.len()].copy_from_slice(&bytes);
        over[0..4].copy_from_slice(&INTERFACES_VERSION.to_le_bytes());
        over[4..8].copy_from_slice(&(MAXIMUM_INTERFACES as u32 + 1).to_le_bytes());
        assert_eq!(decode_interfaces(&over), Err(Error::BadField(4)));
    }

    /// **One list, one encoding**: each byte the layout fixes is checked, and so is the order.
    #[test]
    fn a_respelled_list_is_refused() {
        let good: [u8; descriptor_bytes(3)] = encode_interfaces(&LIST);
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
    fn a_list_out_of_order_is_not_encoded() {
        let _: [u8; descriptor_bytes(2)] = encode_interfaces(&[
            DeclaredInterface { id: 2, version: 1 },
            DeclaredInterface { id: 1, version: 1 },
        ]);
    }

    /// **A whole note has the header, the owner, the type and the descriptor** that a loader
    /// finds, and nothing after.
    #[test]
    fn a_whole_note_carries_its_header_and_descriptor() {
        const N: usize = note_bytes(3);
        let note = InterfacesNote::<N>::of::<{ descriptor_bytes(3) }>(&LIST);
        let b = note.0;
        assert_eq!(&b[..4], &5u32.to_le_bytes());
        assert_eq!(&b[4..8], &(descriptor_bytes(3) as u32).to_le_bytes());
        assert_eq!(&b[8..12], &INTERFACES.to_le_bytes());
        assert_eq!(&b[12..16], b"nife"); // wire literal
        assert_eq!(&b[16..20], &[0, 0, 0, 0]);
        assert_eq!(b.len(), 20 + descriptor_bytes(3));
        assert_eq!(decode_interfaces(&b[20..]).unwrap().version_of(4), Some(7));
    }

    /// **The spawn-time check**: a program that declares the version the
    /// host serves is accepted, one that declares another is refused with both numbers, and one
    /// that says nothing about the interface is accepted. Either side of the served version
    /// refuses, so an older program is not read as compatible with a newer host.
    #[test]
    fn a_declared_version_must_be_the_one_served() {
        let bytes: [u8; descriptor_bytes(3)] = encode_interfaces(&LIST);
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
        let empty: [u8; descriptor_bytes(0)] = encode_interfaces(&[]);
        assert_eq!(
            decode_interfaces(&empty).unwrap().check_served(1, 1),
            Ok(())
        );
    }
}
