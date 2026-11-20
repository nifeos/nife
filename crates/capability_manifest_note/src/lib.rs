//! A program's manifest, as the bytes of an ELF (Executable and Linkable Format) note it carries:
//! inside the executable behind a `PT_NOTE`, so it is in the bytes a loader hashes, and changing
//! what a program asks for changes its digest. The wire format between [`carry!`] and the loader
//! and shell that read it; an ELF reader finds the note, and this crate starts at the descriptor:
//! owner [`OWNER`], type [`MANIFEST`], version 1 ([`DESCRIPTOR_BYTES`] bytes), [`ZONED_VERSION`] 2
//! or [`SERVICED_VERSION`] 3. One manifest has exactly one encoding: [`decode`] refuses anything
//! [`encode`] would not write. The layouts are below; the README has the reasons.

//! # BUGS
//!
//! - A foreign build carries a note only by linking an object that holds one: `-Clink-arg=note.o`
//!   works and `llvm-objcopy --add-section` makes no program header. [`Note::of`] gives the bytes
//!   that object holds; writing the object is the build's own step.
//! - Changing a manifest means relinking, and a new digest: the cost of keeping the request inside
//!   the hashed bytes.
//!

//! # The descriptor, version 1
//!
//! A version word, then little-endian fields in a fixed order. Every field is one of
//! `grant_contract::Manifest`'s, in bytes; nothing here is a new authority.
//!
//! | offset | size | field | values |
//! |---|---|---|---|
//! | 0 | 4 | version | `1` |
//! | 4 | 1 | `program_argument` | 0 forbidden, 1 required; hears words: 2 read-only, 3 read-write, 4 create |
//! | 5 | 1 | `memory_grant` | 0 forbidden, 1 required |
//! | 6 | 1 | `file` | 0 forbidden, 1 read-only, 2 read-write |
//! | 7 | 1 | `directory` | 0 forbidden, 1 required |
//! | 8 | 8 | `memory_grant` minimum pages | 0 unless `memory_grant` is required |
//! | 16 | 8 | `memory_grant` maximum pages | 0 unless `memory_grant` is required; at least the minimum |
//! | 24 | 1 | `output` | 0 silent, 1 words, 2 bytes, 3 bytes and diagnostics (at `grant_contract::DIAGNOSTICS_SLOT`) |
//! | 25 | 1 | `input` | 0 forbidden, 1 required |
//! | 26 | 1 | `input` writes while reading | 0 or 1; 0 unless `input` is required |
//! | 27 | 1 | `directory`'s subtree option | 0 for none, else one of the declared option letters; 0 unless `directory` is required |
//! | 28 | 1 | `reports` | 0 or 1 |
//! | 29 | 1 | `interruptible` | 0 or 1 |
//! | 30 | 1 | `clock` | 0 or 1 |
//! | 31 | 1 | `domain` | 0 or 1 |
//! | 32 | 1 | `configuration` | 0 or 1 |
//! | 33 | 1 | `entropy` | 0 or 1 |
//! | 34 | 1 | `network` | 0 or 1 |
//! | 35 | 1 | `startup` | 0 native, 1 std |
//! | 36 | 1 | option count | at most `grant_contract::MAXIMUM_DECLARED_SHORT_OPTIONS` (16) |
//! | 37 | 16 | option letters | the first *count* are the letters in bit order, the rest 0 |
//! | 53 | 1 | `machine` | 0 or 1, the machine statistics page |
//! | 54 | 1 | `share` | 0 or 1, a view of the job budget |
//! | 55 | 1 | zero | |
//!
//! Byte 55 is the last zero.

//! # The descriptor, version 2: a manifest and the resolver zones it asks for
//!
//! Version 1's 56 bytes at the same offsets, with `2` in the version word, then:
//!
//! | offset | size | field | values |
//! |---|---|---|---|
//! | 56 | 1 | zone count | 1 to [`MAXIMUM_ZONES`] |
//! | 57 | 1 + *n* | each zone: its length *n*, then *n* bytes of dotted ASCII | *n* at most
//! [`MAXIMUM_ZONE_BYTES`]; 0 is the root zone |
//!
//! The descriptor ends with the last zone, so `descsz` need not be a multiple of four; [`ZonedNote`]
//! pads after it. **One request is still one encoding**: a manifest that asks for no zone is
//! version 1, so a version 2 count of 0 is refused, and the zones are canonical ([`zones`] says
//! how). Zones are what the program asks for; the owner grants them at install, one zone to each
//! badge the resolver hands out, and nothing here grants anything.
//!
//! # The descriptor, version 3: the service ask
//!
//! The owner grants a program at install what its manifest asks for, and running as a service is
//! one of those asks. Version 3 is version 2's layout with the zone count allowed to be zero and
//! one byte after the zones:
//!
//! | offset | size | field | values |
//! |---|---|---|---|
//! | 56 | 1 | zone count | 0 to [`MAXIMUM_ZONES`] |
//! | 57 | 1 + *n* | each zone, as version 2's | |
//! | after | 1 | service | `0` or `1` |
//!
//! A manifest that asks for zones and no service stays version 2, so no existing digest changes;
//! the service ask is the one thing only version 3 can say. Nothing here starts a service.
//!
//! Name: ratified 2026-10-18 (calef, "`capability_manifest_note`"). The descriptor is the capabilities
//! a program requests ("manifest" in the Android sense), and "note" separates the ELF-note encoding
//! from `grant_contract::Manifest`, the in-memory type. The owner string, the type number and the
//! encoding are ratified separately; [`carry!`] and the public functions are provisional. Version
//! 3 is provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.

#![no_std]

use grant_contract::{
    DIAGNOSTICS_SLOT, DirectoryGrantDeclaration, FileGrantDeclaration, IndexStoreDeclaration,
    InputDeclaration, MAXIMUM_DECLARED_SHORT_OPTIONS, Manifest, MemoryGrantDeclaration,
    OutputDeclaration, ProgramArgumentDeclaration, ShortOptions, StartupContract, WordGrant,
};

/// The note's owner string, without its terminating null (NUL) byte: the name of the operating
/// system that defines these notes, in lowercase, as `FreeBSD` and `GNU` name the owners of theirs.
///
/// Name: ratified 2026-09-26 (calef, answering "Yes" to the maintainer, who relayed it to milestone 597's lane).
pub const OWNER: &[u8] = b"nife"; // wire literal

/// The note type that carries a manifest: `1`, "manifest".
///
/// Types are per owner, so `1` collides with no other owner's notes.
///
/// Name: ratified 2026-09-26 (calef, answering "Yes" to the maintainer, who relayed it to milestone 597's lane).
pub const MANIFEST: u32 = 1;

/// The first descriptor version: a version word, then little-endian fields in a fixed order, with
/// no zones and no service byte. [`decode`] reads this version and [`ZONED_VERSION`] and
/// [`SERVICED_VERSION`], and refuses any later one.
///
/// Name: ratified 2026-11-19. `BARE` says it is the version with nothing after the fixed fields, beside
/// [`ZONED_VERSION`] and [`SERVICED_VERSION`]; a bare `VERSION` read as the current version of the
/// format or of the crate, which it is not.
pub const BARE_VERSION: u32 = 1;

/// The version of the set of manifest note types and their descriptors: 1. A break bumps it, with
/// the dated ledger entry the interface-stability discipline owes; an addition an older peer
/// ignores does not.
///
/// Name: provisional, minted 2026-11-01 (UTC) by lane lane/956-interface-ids-for-the-owed-formats.
pub const NOTE_SET_VERSION: u32 = 1;

/// This interface's identity in a program's `INTERFACES` note: 7, naming the set of manifest note
/// types and their descriptors. The number is this interface's for good: no other interface takes
/// it, and a format change is a new version, never a new number. The project that links these
/// crates keeps the table of numbers and holds it to that rule. A program declares the pair of
/// this id and [`NOTE_SET_VERSION`].
///
/// Name: provisional, minted 2026-11-01 (UTC) by lane lane/956-interface-ids-for-the-owed-formats.
pub const INTERFACE_ID: u64 = 7;

/// How long a version 1 descriptor is, exactly; the module documentation has the layout.
///
/// Name: provisional. A length carries its unit (`design/naming.md`, units).
pub const DESCRIPTOR_BYTES: usize = 56;

/// The descriptor version that carries resolver zones after version 1's fields (the module's
/// version 2 table). The number and the layout are ratified; the name is provisional.
///
/// Name: provisional, minted 2026-10-19 (UTC) by milestone/892-installed-programs-hold-the-network-and-zones.
pub const ZONED_VERSION: u32 = 2;

/// The descriptor version that carries the service ask after the zones: version 2's layout with
/// the zone count allowed to be zero, then one byte, `1` when the manifest asks to run as a
/// service. A manifest that asks for zones and no service stays version 2, so no existing digest
/// changes; the service ask is the one thing only version 3 can say. This is the ask the owner
/// grants at install; starting the service is the system's, not this crate's.
///
/// Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.
pub const SERVICED_VERSION: u32 = 3;

/// The most zones a manifest may ask for: 8, the resolver's
/// `domain_name_resolution_contract::MAXIMUM_GRANTS`, since each granted zone is its own badge and
/// a program asking for more could never be granted them all at once.
///
/// Name: provisional, minted 2026-10-19 (UTC) by milestone/892-installed-programs-hold-the-network-and-zones.
pub const MAXIMUM_ZONES: usize = domain_name_resolution_contract::MAXIMUM_GRANTS;

/// The longest zone, in bytes of dotted text: 253, a host name's limit and the resolver's
/// `domain_name_resolution_contract::MAXIMUM_NAME_TEXT_BYTES`.
///
/// Name: provisional. A length carries its unit (`design/naming.md`, units).
pub const MAXIMUM_ZONE_BYTES: usize = domain_name_resolution_contract::MAXIMUM_NAME_TEXT_BYTES;

/// `OWNER` with its NUL, padded to the note's 4-byte alignment.
const NAME_FIELD_BYTES: usize = 8;

/// The note's three header words.
const HEADER_BYTES: usize = 12;

/// How long a whole manifest note is: header, owner, descriptor. Every field is already a multiple
/// of four, so there is no trailing pad.
///
/// Name: provisional. A length carries its unit (`design/naming.md`, units).
pub const NOTE_BYTES: usize = HEADER_BYTES + NAME_FIELD_BYTES + DESCRIPTOR_BYTES;

/// Why a descriptor was refused. Provisional names.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Error {
    /// Fewer than four bytes, so not even a version.
    NoVersion,
    /// A version this reader does not know. A later layout is refused rather than guessed at.
    UnknownVersion,
    /// A descriptor cut short, or with bytes after its last field: version 1 is exactly
    /// [`DESCRIPTOR_BYTES`] bytes, and the later versions add their zone and service bytes.
    WrongLength,
    /// A field holds a value outside its range, or a byte the layout says is zero is not. The
    /// offset is where the field at fault starts, not the byte inside it: the first option letter
    /// for a list of letters that is refused, a zone's length byte for anything wrong inside that
    /// zone, and a byte's own offset for a one-byte field or a byte that should be zero.
    BadField(usize),
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Error::NoVersion => f.write_str("fewer than four bytes, so no version word"),
            Error::UnknownVersion => f.write_str("a descriptor version this reader does not know"),
            Error::WrongLength => f.write_str("a descriptor cut short or with bytes after its end"),
            Error::BadField(at) => {
                write!(f, "a field at offset {at} holds a value outside its range")
            }
        }
    }
}

impl core::error::Error for Error {}

// Offsets, named once so `encode` and `decode` cannot disagree about where a field is.
const PROGRAM_ARGUMENT: usize = 4;
const MEMORY_GRANT: usize = 5;
const FILE: usize = 6;
const DIRECTORY: usize = 7;
const MEMORY_MINIMUM_PAGES: usize = 8;
const MEMORY_MAXIMUM_PAGES: usize = 16;
const OUTPUT: usize = 24;
const INPUT: usize = 25;
const WRITES_WHILE_READING: usize = 26;
const SUBTREE: usize = 27;
const REPORTS: usize = 28;
const INTERRUPTIBLE: usize = 29;
const CLOCK: usize = 30;
const DOMAIN: usize = 31;
const CONFIGURATION: usize = 32;
const ENTROPY: usize = 33;
const NETWORK: usize = 34;
const STARTUP: usize = 35;
const OPTION_COUNT: usize = 36;
const OPTION_LETTERS: usize = 37;
const MACHINE: usize = OPTION_LETTERS + MAXIMUM_DECLARED_SHORT_OPTIONS;
const SHARE: usize = MACHINE + 1;
const TAIL: usize = SHARE + 1;

const _: () = assert!(TAIL + 1 == DESCRIPTOR_BYTES);

/// Four bytes at `at`, in order. `const` because a note is built at compile time, and a helper
/// because four hand-written copies of this loop each hid an `at - i` that no value in the notes
/// could tell from `at + i` (every word written is under 256 and lands on zeros), so the loop is
/// written once and tested with a word whose four bytes differ.
const fn put4(out: &mut [u8], at: usize, v: [u8; 4]) {
    let mut i = 0;
    while i < 4 {
        out[at + i] = v[i];
        i += 1;
    }
}

const fn put8(out: &mut [u8; DESCRIPTOR_BYTES], at: usize, v: u64) {
    let b = v.to_le_bytes();
    let mut i = 0;
    while i < 8 {
        out[at + i] = b[i];
        i += 1;
    }
}

/// **One note, written once for every note type**: the three header words (`namesz` is the owner
/// and its NUL, `descsz` the descriptor's exact length, then the type), the owner with its NUL and
/// zero padding, then the descriptor. `out` is zeroed and as long as the note, so whatever this
/// does not write is the padding after the descriptor.
const fn write_note(out: &mut [u8], kind: u32, descriptor: &[u8]) {
    put4(out, 0, ((OWNER.len() + 1) as u32).to_le_bytes());
    put4(out, 4, (descriptor.len() as u32).to_le_bytes());
    put4(out, 8, kind.to_le_bytes());
    let mut i = 0;
    while i < OWNER.len() {
        out[HEADER_BYTES + i] = OWNER[i];
        i += 1;
    }
    let mut i = 0;
    while i < descriptor.len() {
        out[HEADER_BYTES + NAME_FIELD_BYTES + i] = descriptor[i];
        i += 1;
    }
}

/// **The grants a note has no spelling for**, refused at compile time in a constant: each is the
/// owner's console's to grant, or no image's at all, so no installed bytes may ask for it.
const fn refuse_what_no_note_carries(m: &Manifest) {
    assert!(
        !m.reboot,
        "a manifest note cannot carry `reboot`: no image may be endowed the reboot object"
    );
    assert!(
        !m.sync,
        "a manifest note cannot carry `sync`: no image may be endowed a sync-only capability"
    );
    assert!(
        !m.installer && !m.catalog,
        "a manifest note cannot carry `installer` or `catalog`: only the owner's console grants them, \
         and only to a program the image names"
    );
    // The index store likewise, for the same reason.
    assert!(
        matches!(m.indexes, IndexStoreDeclaration::Forbidden),
        "a manifest note cannot carry `indexes`: only the owner's console grants the index store"
    );
    // And the activation directory.
    assert!(
        !m.activation_directory,
        "a manifest note cannot carry `activation_directory`: only the owner's console grants it"
    );
    // And standard input from the console.
    assert!(
        !m.console_input,
        "a manifest note cannot carry `console_input`: only the owner's console grants it"
    );
}

/// `m` as a version 1 descriptor. A `const fn`, so a program's note is computed by the compiler
/// from the constant its source declares ([`carry!`]). Panics, a compile error in a constant, on
/// what the layout cannot spell: a second stream off `grant_contract::DIAGNOSTICS_SLOT`, a subtree
/// option that is not a declared letter or lacks a directory grant, and `reboot`, `sync`,
/// `installer` or `catalog`, which no image may be endowed, and the other grants only the owner's
/// console gives (the README says why they have no byte).
pub const fn encode(m: &Manifest) -> [u8; DESCRIPTOR_BYTES] {
    refuse_what_no_note_carries(m);
    let mut out = [0u8; DESCRIPTOR_BYTES];
    let v = BARE_VERSION.to_le_bytes();
    out[0] = v[0];
    out[1] = v[1];
    out[2] = v[2];
    out[3] = v[3];
    out[PROGRAM_ARGUMENT] = match m.program_argument {
        ProgramArgumentDeclaration::Forbidden => 0,
        ProgramArgumentDeclaration::Required => 1,
        // A program whose line is its argv; see the module's "one encoding" paragraph.
        ProgramArgumentDeclaration::Words(WordGrant::ReadOnly) => 2,
        ProgramArgumentDeclaration::Words(WordGrant::ReadWrite) => 3,
        ProgramArgumentDeclaration::Words(WordGrant::Create) => 4,
    };
    if let MemoryGrantDeclaration::Required { minimum, maximum } = m.memory_grant {
        assert!(
            minimum <= maximum,
            "a required memory grant has a minimum no greater than its maximum"
        );
        out[MEMORY_GRANT] = 1;
        put8(&mut out, MEMORY_MINIMUM_PAGES, minimum);
        put8(&mut out, MEMORY_MAXIMUM_PAGES, maximum);
    }
    out[FILE] = match m.file {
        FileGrantDeclaration::Forbidden => 0,
        FileGrantDeclaration::Required { writable: false } => 1,
        FileGrantDeclaration::Required { writable: true } => 2,
    };
    let letters = m.short_options.letters();
    if let DirectoryGrantDeclaration::Required { subtree_option } = m.directory {
        out[DIRECTORY] = 1;
        if let Some(letter) = subtree_option {
            let mut i = 0;
            let mut declared = false;
            while i < letters.len() {
                declared |= letters[i] == letter;
                i += 1;
            }
            assert!(
                declared,
                "a subtree option must be one of the declared letters"
            );
            out[SUBTREE] = letter;
        }
    }
    out[OUTPUT] = match m.output {
        OutputDeclaration::Silent => 0,
        OutputDeclaration::Words => 1,
        OutputDeclaration::Bytes => 2,
        OutputDeclaration::BytesAndDiagnostics { slot } => {
            assert!(
                slot == DIAGNOSTICS_SLOT,
                "a second stream lands at grant_contract::DIAGNOSTICS_SLOT"
            );
            3
        }
    };
    if let InputDeclaration::Required {
        writes_while_reading,
    } = m.input
    {
        out[INPUT] = 1;
        out[WRITES_WHILE_READING] = writes_while_reading as u8;
    }
    out[REPORTS] = m.reports as u8;
    out[INTERRUPTIBLE] = m.interruptible as u8;
    out[CLOCK] = m.clock as u8;
    out[DOMAIN] = m.domain as u8;
    out[CONFIGURATION] = m.configuration as u8;
    out[ENTROPY] = m.entropy as u8;
    out[NETWORK] = m.network as u8;
    out[MACHINE] = m.machine as u8;
    out[SHARE] = m.share as u8;
    out[STARTUP] = match m.startup {
        StartupContract::Native => 0,
        StartupContract::Std => 1,
    };
    out[OPTION_COUNT] = letters.len() as u8;
    let mut i = 0;
    while i < letters.len() {
        out[OPTION_LETTERS + i] = letters[i];
        i += 1;
    }
    out
}

fn get8(d: &[u8], at: usize) -> u64 {
    let mut b = [0u8; 8];
    b.copy_from_slice(&d[at..at + 8]);
    u64::from_le_bytes(b)
}

/// A byte that must be 0 or 1.
fn flag(d: &[u8], at: usize) -> Result<bool, Error> {
    match d[at] {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(Error::BadField(at)),
    }
}

/// A byte that must be zero, because the field it belongs to is absent.
///
/// A loop over the indices rather than `iter().position`, which reaches `memchr`; the meaning is
/// the same, and the proofs run through this function on every descriptor they choose.
fn zero(d: &[u8], at: usize, len: usize) -> Result<(), Error> {
    let field = &d[at..at + len];
    match (0..field.len()).find(|&i| field[i] != 0) {
        Some(i) => Err(Error::BadField(at + i)),
        None => Ok(()),
    }
}

/// The manifest a descriptor of any version spells, or why it spells none. Strict: whatever this
/// accepts, the encoder of its version turns back into the same bytes, so a descriptor has one
/// reading.
///
/// # Examples
///
/// ```
/// const M: grant_contract::Manifest = grant_contract::Manifest {
///     clock: true,
///     ..program_table::NO_NOTE_MANIFEST
/// };
/// let m = M;
/// let bytes = capability_manifest_note::encode(&m);
/// assert_eq!(capability_manifest_note::decode(&bytes), Ok(m));
///
/// let mut later = bytes;
/// later[0] = 9;
/// assert_eq!(capability_manifest_note::decode(&later), Err(capability_manifest_note::Error::UnknownVersion));
/// ```
///
/// A program carries its note with one line at module scope:
///
/// ```ignore
/// capability_manifest_note::carry!(M);
/// ```
///
/// A version 2 or version 3 descriptor decodes to the manifest its first 56 bytes spell. Its zones
/// and its service byte are checked here too, so a refusal is the whole descriptor's, and they are
/// read with [`zones`] and [`asks_service`].
pub fn decode(d: &[u8]) -> Result<Manifest, Error> {
    if d.len() < 4 {
        return Err(Error::NoVersion);
    }
    match u32::from_le_bytes([d[0], d[1], d[2], d[3]]) {
        BARE_VERSION if d.len() != DESCRIPTOR_BYTES => return Err(Error::WrongLength),
        BARE_VERSION => {}
        ZONED_VERSION => {
            check_zones(d)?;
        }
        SERVICED_VERSION => {
            check_serviced(d)?;
        }
        _ => return Err(Error::UnknownVersion),
    }
    let d = d.get(..DESCRIPTOR_BYTES).ok_or(Error::WrongLength)?;
    let program_argument = decode_program_argument(d)?;
    let memory_grant = decode_memory_grant(d)?;
    let file = decode_file(d)?;
    let count = d[OPTION_COUNT] as usize;
    if count > MAXIMUM_DECLARED_SHORT_OPTIONS {
        return Err(Error::BadField(OPTION_COUNT));
    }
    let letters = &d[OPTION_LETTERS..OPTION_LETTERS + count];
    let short_options = ShortOptions::try_new(letters).ok_or(Error::BadField(OPTION_LETTERS))?;
    zero(
        d,
        OPTION_LETTERS + count,
        MAXIMUM_DECLARED_SHORT_OPTIONS - count,
    )?;
    let directory = decode_directory(d, letters)?;
    let output = decode_output(d)?;
    let input = decode_input(d)?;
    let startup = decode_startup(d)?;
    zero(d, TAIL, DESCRIPTOR_BYTES - TAIL)?;
    Ok(Manifest {
        program_argument,
        memory_grant,
        file,
        directory,
        short_options,
        output,
        input,
        reports: flag(d, REPORTS)?,
        interruptible: flag(d, INTERRUPTIBLE)?,
        clock: flag(d, CLOCK)?,
        domain: flag(d, DOMAIN)?,
        configuration: flag(d, CONFIGURATION)?,
        entropy: flag(d, ENTROPY)?,
        network: flag(d, NETWORK)?,
        machine: flag(d, MACHINE)?,
        share: flag(d, SHARE)?,
        // Never from a note: see `encode`.
        reboot: false,
        sync: false,
        installer: false,
        catalog: false,
        indexes: IndexStoreDeclaration::Forbidden,
        activation_directory: false,
        console_input: false,
        startup,
    })
}

/// The `program_argument` field.
fn decode_program_argument(d: &[u8]) -> Result<ProgramArgumentDeclaration, Error> {
    Ok(match d[PROGRAM_ARGUMENT] {
        0 => ProgramArgumentDeclaration::Forbidden,
        1 => ProgramArgumentDeclaration::Required,
        2 => ProgramArgumentDeclaration::Words(WordGrant::ReadOnly),
        3 => ProgramArgumentDeclaration::Words(WordGrant::ReadWrite),
        4 => ProgramArgumentDeclaration::Words(WordGrant::Create),
        _ => return Err(Error::BadField(PROGRAM_ARGUMENT)),
    })
}

/// The `memory_grant` field and its bounds, which are zero when it is absent.
fn decode_memory_grant(d: &[u8]) -> Result<MemoryGrantDeclaration, Error> {
    Ok(match d[MEMORY_GRANT] {
        0 => {
            zero(d, MEMORY_MINIMUM_PAGES, 16)?;
            MemoryGrantDeclaration::Forbidden
        }
        1 => {
            let (minimum, maximum) = (get8(d, MEMORY_MINIMUM_PAGES), get8(d, MEMORY_MAXIMUM_PAGES));
            if minimum > maximum {
                return Err(Error::BadField(MEMORY_MINIMUM_PAGES));
            }
            MemoryGrantDeclaration::Required { minimum, maximum }
        }
        _ => return Err(Error::BadField(MEMORY_GRANT)),
    })
}

/// The `file` field.
fn decode_file(d: &[u8]) -> Result<FileGrantDeclaration, Error> {
    Ok(match d[FILE] {
        0 => FileGrantDeclaration::Forbidden,
        1 => FileGrantDeclaration::Required { writable: false },
        2 => FileGrantDeclaration::Required { writable: true },
        _ => return Err(Error::BadField(FILE)),
    })
}

/// The `directory` field and its subtree letter, which must be one of the declared `letters`.
fn decode_directory(d: &[u8], letters: &[u8]) -> Result<DirectoryGrantDeclaration, Error> {
    Ok(match d[DIRECTORY] {
        0 => {
            zero(d, SUBTREE, 1)?;
            DirectoryGrantDeclaration::Forbidden
        }
        1 => DirectoryGrantDeclaration::Required {
            subtree_option: match d[SUBTREE] {
                0 => None,
                l if letters.contains(&l) => Some(l),
                _ => return Err(Error::BadField(SUBTREE)),
            },
        },
        _ => return Err(Error::BadField(DIRECTORY)),
    })
}

/// The `output` field.
fn decode_output(d: &[u8]) -> Result<OutputDeclaration, Error> {
    Ok(match d[OUTPUT] {
        0 => OutputDeclaration::Silent,
        1 => OutputDeclaration::Words,
        2 => OutputDeclaration::Bytes,
        3 => OutputDeclaration::BytesAndDiagnostics {
            slot: DIAGNOSTICS_SLOT,
        },
        _ => return Err(Error::BadField(OUTPUT)),
    })
}

/// The `input` field and whether it writes while reading, which is zero when it is absent.
fn decode_input(d: &[u8]) -> Result<InputDeclaration, Error> {
    Ok(match d[INPUT] {
        0 => {
            zero(d, WRITES_WHILE_READING, 1)?;
            InputDeclaration::Forbidden
        }
        1 => InputDeclaration::Required {
            writes_while_reading: flag(d, WRITES_WHILE_READING)?,
        },
        _ => return Err(Error::BadField(INPUT)),
    })
}

/// The `startup` field.
fn decode_startup(d: &[u8]) -> Result<StartupContract, Error> {
    Ok(match d[STARTUP] {
        0 => StartupContract::Native,
        1 => StartupContract::Std,
        _ => return Err(Error::BadField(STARTUP)),
    })
}

/// **A whole manifest note**, header and owner and descriptor, aligned as a note must be. What
/// [`carry!`] places in the program's note section. Provisional name.
#[repr(C, align(4))]
pub struct Note(pub [u8; NOTE_BYTES]);

impl Note {
    /// The note carrying `m`.
    pub const fn of(m: &Manifest) -> Self {
        let mut out = [0u8; NOTE_BYTES];
        write_note(&mut out, MANIFEST, &encode(m));
        Self(out)
    }
}

/// **A version 2 descriptor's length** for these zones: version 1's 56 bytes, the count, and each
/// zone's length byte and text.
///
/// Name: provisional. A length carries its unit (`design/naming.md`, units), and `descriptor`
/// says which length, beside the whole note's.
pub const fn zoned_descriptor_bytes(zones: &[&str]) -> usize {
    let mut len = DESCRIPTOR_BYTES + 1;
    let mut i = 0;
    while i < zones.len() {
        len += 1 + zones[i].len();
        i += 1;
    }
    len
}

/// `m` and the zones it asks for, as a version 2 descriptor. `LEN` is [`zoned_descriptor_bytes`] of
/// `zones`, named by the caller's type, as [`encode_verbs`] does:
///
/// ```
/// const ZONES: &[&str] = &["", "example.net"];
/// const M: grant_contract::Manifest = program_table::NO_NOTE_MANIFEST;
/// const D: [u8; capability_manifest_note::zoned_descriptor_bytes(ZONES)] = capability_manifest_note::encode_zoned(&M, ZONES);
/// assert_eq!(capability_manifest_note::decode(&D), Ok(M));
/// let zones = capability_manifest_note::zones(&D).unwrap();
/// assert_eq!(zones.iter().collect::<Vec<_>>(), [b"".as_slice(), b"example.net"]);
/// ```
///
/// It panics, a compile error in a constant, on everything [`zones`] refuses, and on no zones at
/// all: a manifest that asks for none is [`encode`]'s version 1.
///
/// Name: provisional, minted 2026-10-19 (UTC) by milestone/892-installed-programs-hold-the-network-and-zones.
pub const fn encode_zoned<const LEN: usize>(m: &Manifest, zones: &[&str]) -> [u8; LEN] {
    assert!(
        !zones.is_empty() && zones.len() <= MAXIMUM_ZONES,
        "a version 2 manifest asks for one to eight zones; none is version 1"
    );
    assert!(
        LEN == zoned_descriptor_bytes(zones),
        "LEN must be zoned_descriptor_bytes(zones)"
    );
    let mut out = [0u8; LEN];
    let fixed = encode(m);
    let mut i = 0;
    while i < DESCRIPTOR_BYTES {
        out[i] = fixed[i];
        i += 1;
    }
    put4(&mut out, 0, ZONED_VERSION.to_le_bytes());
    write_zones(&mut out, zones);
    out
}

/// **Write a zone count and its length-prefixed zones after the fixed fields**, checking each zone
/// as [`decode`] will: canonical, at most 253 bytes (so its length fits the prefix byte), and
/// sorted strictly after the one before. Returns the offset after the last zone. One body for
/// [`encode_zoned`] and [`encode_serviced`], so the two cannot disagree about what they accept.
const fn write_zones(out: &mut [u8], zones: &[&str]) -> usize {
    out[DESCRIPTOR_BYTES] = zones.len() as u8;
    let mut at = DESCRIPTOR_BYTES + 1;
    let mut z = 0;
    while z < zones.len() {
        let zone = zones[z].as_bytes();
        assert!(
            domain_name_resolution_contract::is_canonical_zone(zone),
            "a zone is lowercase dotted ASCII, at most 253 bytes, with no empty label"
        );
        if z > 0 {
            assert!(
                before(zones[z - 1].as_bytes(), zone),
                "zones are sorted ascending, each once"
            );
        }
        out[at] = zone.len() as u8;
        let mut b = 0;
        while b < zone.len() {
            out[at + 1 + b] = zone[b];
            b += 1;
        }
        at += 1 + zone.len();
        z += 1;
    }
    at
}

/// Every zone of a version 2 descriptor, checked: a count of one to [`MAXIMUM_ZONES`], each zone
/// canonical (`domain_name_resolution_contract::is_canonical_zone`) and sorted strictly after the one before, and nothing after the
/// last. A refusal's offset is the count byte, or the length byte of the zone that breaks.
fn check_zones(d: &[u8]) -> Result<&[u8], Error> {
    let count = *d.get(DESCRIPTOR_BYTES).ok_or(Error::WrongLength)? as usize;
    if count == 0 || count > MAXIMUM_ZONES {
        return Err(Error::BadField(DESCRIPTOR_BYTES));
    }
    let mut at = DESCRIPTOR_BYTES + 1;
    let mut previous: Option<&[u8]> = None;
    for _ in 0..count {
        let len = *d.get(at).ok_or(Error::WrongLength)? as usize;
        let zone = d.get(at + 1..at + 1 + len).ok_or(Error::WrongLength)?;
        if !domain_name_resolution_contract::is_canonical_zone(zone)
            || previous.is_some_and(|p| !before(p, zone))
        {
            return Err(Error::BadField(at));
        }
        previous = Some(zone);
        at += 1 + len;
    }
    if at != d.len() {
        return Err(Error::WrongLength);
    }
    Ok(&d[DESCRIPTOR_BYTES..])
}

/// Every zone and the service byte of a version 3 descriptor, checked: a count of zero to
/// [`MAXIMUM_ZONES`] (zero asks no zone, which version 2 refuses), each zone canonical and sorted
/// strictly after the one before, then exactly one byte, `0` or `1`, and nothing after it. A
/// refusal's offset is the count byte, the length byte of the zone that breaks, or the service
/// byte; a descriptor that ends before its service byte is cut short, as any other.
///
/// Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.
fn check_serviced(d: &[u8]) -> Result<&[u8], Error> {
    let count = *d.get(DESCRIPTOR_BYTES).ok_or(Error::WrongLength)? as usize;
    if count > MAXIMUM_ZONES {
        return Err(Error::BadField(DESCRIPTOR_BYTES));
    }
    let mut at = DESCRIPTOR_BYTES + 1;
    let mut previous: Option<&[u8]> = None;
    for _ in 0..count {
        let len = *d.get(at).ok_or(Error::WrongLength)? as usize;
        let zone = d.get(at + 1..at + 1 + len).ok_or(Error::WrongLength)?;
        if !domain_name_resolution_contract::is_canonical_zone(zone)
            || previous.is_some_and(|p| !before(p, zone))
        {
            return Err(Error::BadField(at));
        }
        previous = Some(zone);
        at += 1 + len;
    }
    match d.get(at) {
        Some(0 | 1) => {}
        Some(_) => return Err(Error::BadField(at)),
        None => return Err(Error::WrongLength),
    }
    if at + 1 != d.len() {
        return Err(Error::WrongLength);
    }
    Ok(&d[DESCRIPTOR_BYTES..])
}

/// **Whether the manifest asks to run as a service**, the version 3 ask, read from a checked
/// descriptor.
/// `false` for version 1 and version 2, whose grammar cannot say it.
///
/// Name: ratified 2026-11-19. A question, so a caller's `if asks_service(d)? { ... }` reads as the
/// condition it tests; `service` read as a noun.
pub fn asks_service(d: &[u8]) -> Result<bool, Error> {
    decode(d)?;
    if u32::from_le_bytes([d[0], d[1], d[2], d[3]]) != SERVICED_VERSION {
        return Ok(false);
    }
    let count = d[DESCRIPTOR_BYTES] as usize;
    let mut at = DESCRIPTOR_BYTES + 1;
    for _ in 0..count {
        at += 1 + d[at] as usize;
    }
    Ok(d[at] == 1)
}

/// **The resolver zones a manifest asks for**, as a view of the descriptor, checked whole. Empty for
/// version 1, which asks for none. Refused as [`decode`] refuses the descriptor.
///
/// A zone is dotted text, the resolver's own spelling (`domain_name_resolution_contract`): `b""` is
/// the root zone, any name at all, which the install confirmation flags apart from the others.
///
/// Name: provisional, minted 2026-10-19 (UTC) by milestone/892-installed-programs-hold-the-network-and-zones.
pub fn zones(d: &[u8]) -> Result<Zones<'_>, Error> {
    decode(d)?;
    match u32::from_le_bytes([d[0], d[1], d[2], d[3]]) {
        ZONED_VERSION => Ok(Zones {
            records: &d[DESCRIPTOR_BYTES + 1..],
        }),
        SERVICED_VERSION => {
            let count = d[DESCRIPTOR_BYTES] as usize;
            Ok(Zones {
                records: &d[DESCRIPTOR_BYTES + 1..DESCRIPTOR_BYTES + 1 + zones_bytes(d, count)],
            })
        }
        _ => Ok(Zones { records: &[] }),
    }
}

/// How many bytes the first `count` zones of a version 3 descriptor occupy, its count byte
/// apart: what [`zones`] and [`asks_service`] both walk.
fn zones_bytes(d: &[u8], count: usize) -> usize {
    let mut at = 0usize;
    for _ in 0..count {
        at += 1 + d[DESCRIPTOR_BYTES + 1 + at] as usize;
    }
    at
}

/// **The zones a manifest asks for**: [`zones`]'s view, which allocates nothing.
///
/// Name: provisional, minted 2026-10-19 (UTC) by milestone/892-installed-programs-hold-the-network-and-zones.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Zones<'a> {
    records: &'a [u8],
}

impl<'a> Zones<'a> {
    /// The zones in their sorted order, each as dotted text; the root zone is empty.
    ///
    /// Name: provisional, minted 2026-10-19 (UTC) by milestone/892-installed-programs-hold-the-network-and-zones.
    pub fn iter(&self) -> ZonesIter<'a> {
        ZonesIter { rest: self.records }
    }

    /// How many zones the manifest asks for. A walk of the length-prefixed records, so linear in
    /// the count; [`decode`] bounds the count at 255.
    ///
    /// Name: provisional. std's spelling and meaning (`design/naming.md`), beside `is_empty`.
    pub fn len(&self) -> usize {
        self.iter().count()
    }

    /// Whether the manifest asks for no zone.
    ///
    /// Name: provisional, minted 2026-10-19 (UTC) by milestone/892-installed-programs-hold-the-network-and-zones.
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Whether one of the zones is the root zone: any name at all.
    ///
    /// Name: provisional, minted 2026-10-19 (UTC) by milestone/892-installed-programs-hold-the-network-and-zones.
    pub fn has_root(&self) -> bool {
        self.records.first() == Some(&0)
    }
}

impl<'a> IntoIterator for &Zones<'a> {
    type Item = &'a [u8];
    type IntoIter = ZonesIter<'a>;

    fn into_iter(self) -> ZonesIter<'a> {
        self.iter()
    }
}

/// **The zones of a [`Zones`], in their sorted order**, each as dotted text. The items borrow the
/// descriptor, not the view, so they outlive both.
///
/// Name: provisional. The collection's name and `Iter`, as std names an iterator beside its
/// collection; one crate holds two such views, so the collection's name comes first.
#[derive(Clone, Debug)]
pub struct ZonesIter<'a> {
    rest: &'a [u8],
}

impl<'a> Iterator for ZonesIter<'a> {
    type Item = &'a [u8];

    fn next(&mut self) -> Option<&'a [u8]> {
        // `decode` walked these records already, so each length fits; `get` keeps a short
        // record an end rather than a panic.
        let (&len, tail) = self.rest.split_first()?;
        let zone = tail.get(..len as usize)?;
        self.rest = &tail[len as usize..];
        Some(zone)
    }
}

/// **A whole version 2 manifest note**, padded after its descriptor to the note's 4-byte
/// alignment. `N` is [`zoned_note_bytes`] of the zones. What [`carry!`]'s `zones =` form places.
///
/// Name: provisional, minted 2026-10-19 (UTC) by milestone/892-installed-programs-hold-the-network-and-zones.
#[repr(C, align(4))]
pub struct ZonedNote<const N: usize>(pub [u8; N]);

/// How long a whole version 2 note is for these zones: header, owner, the descriptor, and the
/// zero bytes that round it up to four.
///
/// Name: provisional. A length carries its unit (`design/naming.md`, units).
pub const fn zoned_note_bytes(zones: &[&str]) -> usize {
    (HEADER_BYTES + NAME_FIELD_BYTES + zoned_descriptor_bytes(zones)).next_multiple_of(4)
}

impl<const N: usize> ZonedNote<N> {
    /// The note carrying `m` and the zones it asks for.
    ///
    /// Name: provisional, minted 2026-10-19 (UTC) by milestone/892-installed-programs-hold-the-network-and-zones.
    pub const fn of<const LEN: usize>(m: &Manifest, zones: &[&str]) -> Self {
        assert!(
            N == zoned_note_bytes(zones),
            "N must be zoned_note_bytes(zones)"
        );
        let mut out = [0u8; N];
        let d: [u8; LEN] = encode_zoned(m, zones);
        write_note(&mut out, MANIFEST, &d);
        Self(out)
    }
}

/// **A whole version 3 manifest note**, padded after its descriptor to the note's 4-byte
/// alignment. `N` is [`serviced_note_bytes`] of the zones. What [`carry!`]'s `service =` form
/// places, with or without `zones =`.
///
/// Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.
#[repr(C, align(4))]
pub struct ServicedNote<const N: usize>(pub [u8; N]);

/// How long a version 3 descriptor is for these zones: the zone block of version 2's layout,
/// which may be empty here, and the service byte.
///
/// Name: provisional. A length carries its unit (`design/naming.md`, units), and `descriptor`
/// says which length, beside the whole note's.
pub const fn serviced_descriptor_bytes(zones: &[&str]) -> usize {
    let mut len = DESCRIPTOR_BYTES + 1 + 1;
    let mut i = 0;
    while i < zones.len() {
        len += 1 + zones[i].len();
        i += 1;
    }
    len
}

/// How long a whole version 3 note is for these zones: header, owner, the descriptor, and the
/// zero bytes that round it up to four.
///
/// Name: provisional. A length carries its unit (`design/naming.md`, units).
pub const fn serviced_note_bytes(zones: &[&str]) -> usize {
    (HEADER_BYTES + NAME_FIELD_BYTES + serviced_descriptor_bytes(zones)).next_multiple_of(4)
}

/// `m`, the zones it asks for (none allowed here, which only version 3 can say), and whether it
/// asks to run as a service, as a version 3 descriptor. `LEN` is [`serviced_descriptor_bytes`] of `zones`.
/// It panics, a compile error in a constant, on what [`decode`] refuses: more than
/// [`MAXIMUM_ZONES`] zones, zones not sorted ascending each once, or a zone the resolver would
/// not spell.
///
/// Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.
pub const fn encode_serviced<const LEN: usize>(
    m: &Manifest,
    zones: &[&str],
    service: bool,
) -> [u8; LEN] {
    assert!(
        zones.len() <= MAXIMUM_ZONES,
        "a version 3 manifest asks for at most eight zones"
    );
    assert!(
        LEN == serviced_descriptor_bytes(zones),
        "LEN must be serviced_descriptor_bytes(zones)"
    );
    let mut out = [0u8; LEN];
    // The first 56 bytes are the manifest, as version 1 spells it; `encode` wrote them into
    // `head`, and a const fn copies field by field (`IndexMut` is not const).
    let head = encode(m);
    let mut i = 0;
    while i < DESCRIPTOR_BYTES {
        out[i] = head[i];
        i += 1;
    }
    let v = SERVICED_VERSION.to_le_bytes();
    out[0] = v[0];
    out[1] = v[1];
    out[2] = v[2];
    out[3] = v[3];
    let at = write_zones(&mut out, zones);
    out[at] = service as u8;
    out
}

impl<const N: usize> ServicedNote<N> {
    /// The note carrying `m`, the zones it asks for, and its service ask.
    ///
    /// Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.
    pub const fn of<const LEN: usize>(m: &Manifest, zones: &[&str], service: bool) -> Self {
        assert!(
            N == serviced_note_bytes(zones),
            "N must be serviced_note_bytes(zones)"
        );
        let mut out = [0u8; N];
        let d: [u8; LEN] = encode_serviced(m, zones, service);
        write_note(&mut out, MANIFEST, &d);
        Self(out)
    }
}

// The owner and its NUL fit the name field, which keeps the descriptor 4-aligned. `write_note`
// copies the owner whole, so an owner of any length this allows is written in full.
const _: () = assert!(OWNER.len() < NAME_FIELD_BYTES && NAME_FIELD_BYTES.is_multiple_of(4));

/// Carry this manifest in the program's own bytes: one line at module scope, any constant
/// `grant_contract::Manifest`, and optionally its resolver zones, sorted, `""` for the root
/// ([`ZonedNote`]). It places one [`Note`] in the program's note section, which the program's
/// linker script must keep in a `PT_NOTE`; used twice it is a duplicate symbol, and a reader should
/// refuse a file with two manifest notes.
///
/// ```ignore
/// capability_manifest_note::carry!(MANIFEST, zones = &["example.net"]);
/// ```
///
/// Name: provisional. The static it emits is `MANIFEST_NOTE`, the note's own name with no
/// project word, so the crate can leave nife.
#[macro_export]
macro_rules! carry {
    ($manifest:expr) => {
        #[used]
        #[unsafe(link_section = ".note.nife.manifest")] // wire literal
        static MANIFEST_NOTE: $crate::Note = $crate::Note::of(&$manifest);
    };
    ($manifest:expr, zones = $zones:expr) => {
        #[used]
        #[unsafe(link_section = ".note.nife.manifest")] // wire literal
        static MANIFEST_NOTE: $crate::ZonedNote<{ $crate::zoned_note_bytes($zones) }> =
            $crate::ZonedNote::of::<{ $crate::zoned_descriptor_bytes($zones) }>(&$manifest, $zones);
    };
    ($manifest:expr, service = true) => {
        #[used]
        #[unsafe(link_section = ".note.nife.manifest")] // wire literal
        static MANIFEST_NOTE: $crate::ServicedNote<{ $crate::serviced_note_bytes(&[]) }> =
            $crate::ServicedNote::of::<{ $crate::serviced_descriptor_bytes(&[]) }>(
                &$manifest,
                &[],
                true,
            );
    };
    ($manifest:expr, zones = $zones:expr, service = true) => {
        #[used]
        #[unsafe(link_section = ".note.nife.manifest")] // wire literal
        static MANIFEST_NOTE: $crate::ServicedNote<{ $crate::serviced_note_bytes($zones) }> =
            $crate::ServicedNote::of::<{ $crate::serviced_descriptor_bytes($zones) }>(
                &$manifest, $zones, true,
            );
    };
}

// -------------------------------------------------------------------------------------------
// The second note type: a file server that enforces subtree grants itself.
// -------------------------------------------------------------------------------------------

/// **The note type that says a file server enforces subtree grants itself**: `2`. A server carries
/// it when it resolves every path and every handle through a scope crate, and it is what the
/// loader reads to choose a bound badge over a caretaker for that server's grants. Absent means
/// "give its clients a caretaker", which is every server but an eligible one.
///
/// Name and number provisional.
pub const SUBTREE_GRANTS: u32 = 2;

/// The version of the [`SUBTREE_GRANTS`] descriptor this crate writes and reads.
pub const SUBTREE_GRANTS_VERSION: u32 = 1;

/// How long a version 1 [`SUBTREE_GRANTS`] descriptor is: a version word, then a scope word.
///
/// Name: provisional. A length carries its unit (`design/naming.md`, units).
pub const SUBTREE_GRANTS_BYTES: usize = 8;

/// **Which scope crate a server enforces grants through.** One value today, so the word is room for
/// a second rather than a flag that would need a second field.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Scope {
    /// The `subtree_scope` crate, the one scope crate there is.
    SubtreeScope,
}

impl Scope {
    const fn word(self) -> u32 {
        match self {
            Scope::SubtreeScope => 1,
        }
    }

    /// The package declaration's spelling of this scope, the value of its `subtree_grants` field,
    /// which a build can check against the note a server carries.
    pub const fn declared(self) -> &'static str {
        match self {
            Scope::SubtreeScope => "subtree_scope",
        }
    }
}

/// **A [`SUBTREE_GRANTS`] descriptor, decoded**, or why not. One encoding for one scope, as
/// [`decode`] enforces for a manifest: an unknown version or scope, or the wrong length, refuses.
pub fn decode_subtree_grants(d: &[u8]) -> Result<Scope, Error> {
    let version = d.get(..4).ok_or(Error::NoVersion)?;
    if u32::from_le_bytes([version[0], version[1], version[2], version[3]])
        != SUBTREE_GRANTS_VERSION
    {
        return Err(Error::UnknownVersion);
    }
    if d.len() != SUBTREE_GRANTS_BYTES {
        return Err(Error::WrongLength);
    }
    match u32::from_le_bytes([d[4], d[5], d[6], d[7]]) {
        1 => Ok(Scope::SubtreeScope),
        _ => Err(Error::BadField(4)),
    }
}

/// How long a whole [`SUBTREE_GRANTS`] note is.
///
/// Name: provisional. A length carries its unit (`design/naming.md`, units).
pub const SUBTREE_GRANTS_NOTE_BYTES: usize = HEADER_BYTES + NAME_FIELD_BYTES + SUBTREE_GRANTS_BYTES;

/// **A whole [`SUBTREE_GRANTS`] note**, what [`carry_subtree_grants!`] places. Provisional name.
#[repr(C, align(4))]
pub struct SubtreeGrantsNote(pub [u8; SUBTREE_GRANTS_NOTE_BYTES]);

impl SubtreeGrantsNote {
    /// The note declaring `scope`.
    pub const fn of(scope: Scope) -> Self {
        let mut d = [0u8; SUBTREE_GRANTS_BYTES];
        put4(&mut d, 0, SUBTREE_GRANTS_VERSION.to_le_bytes());
        put4(&mut d, 4, scope.word().to_le_bytes());
        let mut out = [0u8; SUBTREE_GRANTS_NOTE_BYTES];
        write_note(&mut out, SUBTREE_GRANTS, &d);
        Self(out)
    }
}

/// **Declare that this file server enforces subtree grants through `scope`.** One line at module
/// scope in the server's root, beside nothing else: a server carries no [`Manifest`]. It places one
/// [`SubtreeGrantsNote`] in the same note section and `PT_NOTE` a manifest uses.
///
/// Carrying it is a claim the loader acts on, so a build should hold it to account: the crate
/// must depend on `subtree_scope`, and its package declaration must say so too.
///
/// Name: provisional. The static it emits is `SUBTREE_GRANTS_NOTE`, the note's own name with no
/// project word, so the crate can leave nife.
#[macro_export]
macro_rules! carry_subtree_grants {
    ($scope:expr) => {
        #[used]
        #[unsafe(link_section = ".note.nife.manifest")] // wire literal
        static SUBTREE_GRANTS_NOTE: $crate::SubtreeGrantsNote =
            $crate::SubtreeGrantsNote::of($scope);
    };
}

// -------------------------------------------------------------------------------------------
// The third note type: a program's verbs and what each is granted.
// -------------------------------------------------------------------------------------------

/// The note type that carries a program's verb table: `3`. Each entry names one word a line may
/// give after the program's name and which loader-endowed authorities that line is granted
/// (`grant_contract::narrow_by_verb`); a word not in the table is granted none.
///
/// Name and number provisional.
pub const VERBS: u32 = 3;

/// The version of the [`VERBS`] descriptor this crate writes and reads.
pub const VERBS_VERSION: u32 = 1;

/// The most verbs a [`VERBS`] descriptor may hold: 256. Room for a program the size of git
/// (175 subcommands in git 2.56.0), and a bound on what a malformed count
/// can make a reader walk.
///
/// Name: ratified 2026-10-27 (#2797).
pub const MAXIMUM_VERBS: usize = 256;

/// One verb's record: the word in `grant_contract::MAXIMUM_VERB` bytes, zero-padded; then the grants'
/// bits, little-endian; then two zero bytes.
///
/// Name: provisional. A length carries its unit (`design/naming.md`, units).
pub const VERB_RECORD_BYTES: usize = grant_contract::MAXIMUM_VERB + 4;

/// A [`VERBS`] descriptor's length for `count` verbs: the version word, the count word, and
/// exactly `count` records. Variable in length; one table still has one encoding, because the
/// records are sorted and each verb appears once.
///
/// Name: provisional. A length carries its unit (`design/naming.md`, units), and `descriptor`
/// says which length, beside the whole note's.
pub const fn verbs_descriptor_bytes(count: usize) -> usize {
    8 + count * VERB_RECORD_BYTES
}

/// The bytes of a verb table. A `const fn`, so a program's table is computed by the compiler;
/// `LEN` is [`verbs_descriptor_bytes`] of the table's length, named by the caller's type:
///
/// ```
/// use grant_contract::{ManifestAuthorities, VerbGrant};
/// const TABLE: &[VerbGrant] = &[
///     VerbGrant { verb: "fetch", authorities: ManifestAuthorities::NETWORK },
///     VerbGrant { verb: "list", authorities: ManifestAuthorities::NONE },
/// ];
/// const BYTES: [u8; capability_manifest_note::verbs_descriptor_bytes(TABLE.len())] = capability_manifest_note::encode_verbs(TABLE);
/// assert_eq!(capability_manifest_note::decode_verbs(&BYTES).unwrap().len(), TABLE.len());
/// ```
///
/// It panics, a compile error in a constant, on a `LEN` that is not that length, more than
/// [`MAXIMUM_VERBS`] entries, an empty or too-long word, a word with a zero byte, or a table that
/// is not sorted ascending by its words' bytes with each word once. Sorted is what makes one table
/// one encoding.
pub const fn encode_verbs<const LEN: usize>(verbs: &[grant_contract::VerbGrant]) -> [u8; LEN] {
    assert!(
        verbs.len() <= MAXIMUM_VERBS,
        "a verb table holds at most 256 verbs"
    );
    assert!(
        LEN == verbs_descriptor_bytes(verbs.len()),
        "LEN must be verbs_descriptor_bytes(table.len())"
    );
    let mut out = [0u8; LEN];
    let version = VERBS_VERSION.to_le_bytes();
    let count = (verbs.len() as u32).to_le_bytes();
    let mut k = 0;
    while k < 4 {
        out[k] = version[k];
        out[4 + k] = count[k];
        k += 1;
    }
    let mut i = 0;
    while i < verbs.len() {
        let word = verbs[i].verb.as_bytes();
        assert!(
            !word.is_empty() && word.len() <= grant_contract::MAXIMUM_VERB,
            "a verb is one to sixteen bytes"
        );
        if i > 0 {
            assert!(
                before(verbs[i - 1].verb.as_bytes(), word),
                "a verb table is sorted ascending by its words, each word once"
            );
        }
        let at = 8 + i * VERB_RECORD_BYTES;
        let mut b = 0;
        while b < word.len() {
            assert!(word[b] != 0, "a verb holds no zero byte");
            out[at + b] = word[b];
            b += 1;
        }
        let bits = verbs[i].authorities.bits().to_le_bytes();
        out[at + grant_contract::MAXIMUM_VERB] = bits[0];
        out[at + grant_contract::MAXIMUM_VERB + 1] = bits[1];
        i += 1;
    }
    out
}

/// Whether `a` sorts strictly before `b`, bytewise, a shorter prefix first.
const fn before(a: &[u8], b: &[u8]) -> bool {
    let mut i = 0;
    while i < a.len() && i < b.len() {
        if a[i] != b[i] {
            return a[i] < b[i];
        }
        i += 1;
    }
    a.len() < b.len()
}

/// **A decoded verb table**: a view of the descriptor [`decode_verbs`] checked whole, so a reader
/// allocates nothing. Provisional name.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Verbs<'a> {
    records: &'a [u8],
}

impl<'a> Verbs<'a> {
    /// How many verbs.
    pub fn len(&self) -> usize {
        self.records.len() / VERB_RECORD_BYTES
    }

    /// Whether the table is empty, which a descriptor may say: a program whose every line is
    /// granted nothing chosen by a verb.
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// What `verb` is granted, or `None` for a word that is no verb in the table. A binary search,
    /// since the records are sorted: it halves the run left to search until one record is left,
    /// and then asks whether that record is `verb`. How many times it halves depends on the table's
    /// length alone, not on the words, as in `slice::binary_search`, which is what lets the Kani
    /// proof that it finds each verb finish in a minute; the count is a bound, not a condition, so
    /// no change to it can loop forever.
    pub fn grants_for(&self, verb: &[u8]) -> Option<grant_contract::ManifestAuthorities> {
        let mut size = self.len();
        if size == 0 {
            return None;
        }
        let mut base = 0;
        // Each halving leaves `size - size / 2` records, so `ilog2 + 1` of them reach one; a
        // halving of one record is none.
        for _ in 0..size.ilog2() + 1 {
            let half = size / 2;
            if !before(verb, self.record(base + half).0) {
                base += half;
            }
            size -= half;
        }
        // The record left is the last one at or before `verb`, or the first one when `verb` sorts
        // before them all; it is `verb` when neither sorts before the other.
        let (word, grants) = self.record(base);
        (!before(word, verb) && !before(verb, word)).then_some(grants)
    }

    /// The verbs, in their sorted order, with their grants. The items borrow the descriptor, not
    /// the view, as [`Zones::iter`]'s do.
    pub fn iter(&self) -> VerbsIter<'a> {
        VerbsIter {
            verbs: *self,
            next: 0,
        }
    }

    /// Record `i`, which [`decode_verbs`] has already checked.
    fn record(&self, i: usize) -> (&'a [u8], grant_contract::ManifestAuthorities) {
        let r = &self.records[i * VERB_RECORD_BYTES..(i + 1) * VERB_RECORD_BYTES];
        let word = &r[..grant_contract::MAXIMUM_VERB];
        let len = word.iter().position(|&b| b == 0).unwrap_or(word.len());
        let bits = u16::from_le_bytes([
            r[grant_contract::MAXIMUM_VERB],
            r[grant_contract::MAXIMUM_VERB + 1],
        ]);
        (
            &word[..len],
            grant_contract::ManifestAuthorities::from_bits(bits)
                .unwrap_or(grant_contract::ManifestAuthorities::NONE),
        )
    }
}

impl<'a> IntoIterator for &Verbs<'a> {
    type Item = (&'a [u8], grant_contract::ManifestAuthorities);
    type IntoIter = VerbsIter<'a>;

    fn into_iter(self) -> VerbsIter<'a> {
        self.iter()
    }
}

/// **The verbs of a [`Verbs`], in their sorted order**, with their grants.
///
/// Name: provisional. The collection's name and `Iter`, as [`ZonesIter`] is named.
#[derive(Clone, Debug)]
pub struct VerbsIter<'a> {
    verbs: Verbs<'a>,
    next: usize,
}

impl<'a> Iterator for VerbsIter<'a> {
    type Item = (&'a [u8], grant_contract::ManifestAuthorities);

    fn next(&mut self) -> Option<Self::Item> {
        if self.next == self.verbs.len() {
            return None;
        }
        let record = self.verbs.record(self.next);
        self.next += 1;
        Some(record)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let left = self.verbs.len() - self.next;
        (left, Some(left))
    }
}

impl ExactSizeIterator for VerbsIter<'_> {}

/// **A [`VERBS`] descriptor, decoded**, or why not. One table has one encoding, as [`decode`]
/// enforces for a manifest: an unknown version, a count past [`MAXIMUM_VERBS`], a length that is
/// not [`verbs_descriptor_bytes`] of the count, an empty word, a zero inside a word or a nonzero byte after it,
/// a grant bit that names no authority, a nonzero pad, or records out of order or repeated, all
/// refuse.
pub fn decode_verbs(d: &[u8]) -> Result<Verbs<'_>, Error> {
    let version = d.get(..4).ok_or(Error::NoVersion)?;
    if u32::from_le_bytes([version[0], version[1], version[2], version[3]]) != VERBS_VERSION {
        return Err(Error::UnknownVersion);
    }
    let count = d.get(4..8).ok_or(Error::WrongLength)?;
    let count = u32::from_le_bytes([count[0], count[1], count[2], count[3]]) as usize;
    if count > MAXIMUM_VERBS {
        return Err(Error::BadField(4));
    }
    if d.len() != verbs_descriptor_bytes(count) {
        return Err(Error::WrongLength);
    }
    let mut previous: Option<&[u8]> = None;
    for i in 0..count {
        let at = 8 + i * VERB_RECORD_BYTES;
        let record = &d[at..at + VERB_RECORD_BYTES];
        let word = &record[..grant_contract::MAXIMUM_VERB];
        let len = word.iter().position(|&b| b == 0).unwrap_or(word.len());
        if len == 0 {
            return Err(Error::BadField(at));
        }
        if let Some(k) = word[len..].iter().position(|&b| b != 0) {
            return Err(Error::BadField(at + len + k));
        }
        let bits = u16::from_le_bytes([
            record[grant_contract::MAXIMUM_VERB],
            record[grant_contract::MAXIMUM_VERB + 1],
        ]);
        if grant_contract::ManifestAuthorities::from_bits(bits).is_none() {
            return Err(Error::BadField(at + grant_contract::MAXIMUM_VERB));
        }
        if record[grant_contract::MAXIMUM_VERB + 2] != 0
            || record[grant_contract::MAXIMUM_VERB + 3] != 0
        {
            return Err(Error::BadField(at + grant_contract::MAXIMUM_VERB + 2));
        }
        if previous.is_some_and(|p| !before(p, &word[..len])) {
            return Err(Error::BadField(at));
        }
        previous = Some(&word[..len]);
    }
    Ok(Verbs { records: &d[8..] })
}

#[cfg(kani)]
mod verification;

// The tests and the verb tests use `std` (a panic caught, a table built at run time); one line here
// puts it in scope for every test module. It sits with the test modules, below the crate's code,
// which the interface check reads up to the first `#[cfg(test)]`.
#[cfg(test)]
extern crate std;

#[cfg(test)]
mod verb_tests {
    use grant_contract::{ManifestAuthorities, VerbGrant};
    use program_table::Program;

    use super::*;

    const JIG: &[VerbGrant] = Program::Jig.verbs();
    const JIG_BYTES: [u8; verbs_descriptor_bytes(JIG.len())] = encode_verbs(JIG);

    /// **A program's verb table round-trips, and so does an empty one**. What a distribution writes
    /// is what the decoder reads back, entry for entry, and the length is exactly the count's.
    #[test]
    fn a_verb_table_round_trips() {
        assert_eq!(JIG_BYTES.len(), 8 + JIG.len() * VERB_RECORD_BYTES);
        let v = decode_verbs(&JIG_BYTES).unwrap();
        assert_eq!(v.len(), JIG.len());
        for (want, (word, grants)) in JIG.iter().zip(v.iter()) {
            assert_eq!(word, want.verb.as_bytes());
            assert_eq!(grants, want.authorities);
            assert_eq!(v.grants_for(word), Some(want.authorities));
        }
        assert_eq!(v.grants_for(b"upgrade"), None);
        // The iterator ends after the last verb, which `take` checks without waiting on one that
        // never ends, and it counts down as it goes: each step leaves one fewer, the last none.
        assert_eq!(v.iter().take(JIG.len() + 1).count(), JIG.len());
        let mut walk = v.iter();
        for left in (0..JIG.len()).rev() {
            assert!(walk.next().is_some());
            assert_eq!(walk.len(), left);
        }
        assert_eq!(walk.next(), None);
        let empty: [u8; verbs_descriptor_bytes(0)] = encode_verbs(&[]);
        assert!(decode_verbs(&empty).unwrap().is_empty());
    }

    /// **A table the size of git's**: 175 sorted verbs encode, decode, and are each found; the cap
    /// holds at 256.
    #[test]
    fn a_table_the_size_of_gits_round_trips() {
        let words: std::vec::Vec<std::string::String> =
            (0..175).map(|i| std::format!("verb{i:03}")).collect();
        let mut d = std::vec![0u8; verbs_descriptor_bytes(words.len())];
        d[..4].copy_from_slice(&VERBS_VERSION.to_le_bytes());
        d[4..8].copy_from_slice(&(words.len() as u32).to_le_bytes());
        for (i, w) in words.iter().enumerate() {
            let at = 8 + i * VERB_RECORD_BYTES;
            d[at..at + w.len()].copy_from_slice(w.as_bytes());
            d[at + 16..at + 18].copy_from_slice(&ManifestAuthorities::CATALOG.bits().to_le_bytes());
        }
        let v = decode_verbs(&d).unwrap();
        assert_eq!(v.len(), 175);
        // Every verb is found, and no word between, before or after them is: the first, the last
        // and each run the search halves to.
        for w in &words {
            assert_eq!(
                v.grants_for(w.as_bytes()),
                Some(ManifestAuthorities::CATALOG)
            );
            assert_eq!(v.grants_for(std::format!("{w}a").as_bytes()), None);
            assert_eq!(v.grants_for(&w.as_bytes()[..6]), None);
        }
        assert_eq!(v.grants_for(b"a"), None);
        assert_eq!(v.grants_for(b"verb175"), None);
        assert_eq!(v.grants_for(b"z"), None);
        let one = Verbs {
            records: &d[8..8 + VERB_RECORD_BYTES],
        };
        assert_eq!(
            one.grants_for(b"verb000"),
            Some(ManifestAuthorities::CATALOG)
        );
        assert_eq!(one.grants_for(b"verb001"), None);
        assert_eq!(one.grants_for(b"a"), None);
        assert_eq!(Verbs { records: &[] }.grants_for(b"verb000"), None);
        let mut over = d.clone();
        over[4..8].copy_from_slice(&257u32.to_le_bytes());
        assert_eq!(decode_verbs(&over), Err(Error::BadField(4)));
    }

    /// **One table, one encoding**: every byte the layout fixes is checked, and the order too, so
    /// no second spelling of a table decodes. Falsified: decoding `jig`'s records swapped, which
    /// `encode_verbs` would refuse to write, is refused here.
    #[test]
    fn an_unsorted_or_respelled_table_is_refused() {
        let good = JIG_BYTES;
        let refused = |at: usize, b: u8| {
            let mut d = good;
            d[at] = b;
            decode_verbs(&d).err()
        };
        assert_eq!(
            refused(0, 2),
            Some(Error::UnknownVersion),
            "a later version"
        );
        assert_eq!(
            refused(4, 9),
            Some(Error::WrongLength),
            "a count that is not the length's"
        );
        assert_eq!(refused(8, 0), Some(Error::BadField(8)), "an empty word");
        // Each refusal names the byte at fault, which is what a person reading a bad note needs.
        let jig = JIG[0].verb.len();
        assert_eq!(
            refused(8 + jig + 1, b'x'),
            Some(Error::BadField(8 + jig + 1)),
            "a byte after the word's end"
        );
        assert_eq!(
            refused(8 + 17, 0x80),
            Some(Error::BadField(8 + 16)),
            "a grant bit that names nothing"
        );
        assert_eq!(
            refused(8 + 18, 1),
            Some(Error::BadField(8 + 18)),
            "a nonzero pad"
        );
        assert_eq!(
            refused(8 + 19, 1),
            Some(Error::BadField(8 + 18)),
            "the pad's second byte"
        );
        // The first two records swapped: every byte valid, the order not.
        let mut swapped = good;
        let (a, b) = (8, 8 + VERB_RECORD_BYTES);
        let first: [u8; VERB_RECORD_BYTES] = good[a..b].try_into().unwrap();
        swapped.copy_within(b..b + VERB_RECORD_BYTES, a);
        swapped[b..b + VERB_RECORD_BYTES].copy_from_slice(&first);
        assert_eq!(decode_verbs(&swapped), Err(Error::BadField(b)));
        // The same verb twice.
        let mut twice = good;
        twice.copy_within(a..b, b);
        assert!(decode_verbs(&twice).is_err());
        assert_eq!(decode_verbs(&good[..10]), Err(Error::WrongLength));
        assert_eq!(
            decode_verbs(&good[..good.len() - 1]),
            Err(Error::WrongLength)
        );
    }

    /// **A record is the word's sixteen bytes, its grant word and two zero bytes**: twenty, after
    /// an eight-byte head. The layout is a wire format between the build and the loader.
    #[test]
    fn a_verb_record_is_twenty_bytes() {
        assert_eq!(verbs_descriptor_bytes(0), 8);
        assert_eq!(verbs_descriptor_bytes(1), 8 + 16 + 2 + 2);
    }

    /// **The full 256 verbs encode and decode**, the cap itself rather than one under it. The
    /// count's second byte is nonzero only here, so this is also what proves the count is written
    /// where the decoder reads it.
    #[test]
    fn a_table_of_exactly_maximum_verbs_round_trips() {
        const N: usize = MAXIMUM_VERBS;
        // `verb000` to `verb255`, spelled at compile time: a `VerbGrant` borrows its word for
        // `'static`, and a static gives that without a leaked allocation, which Miri reports.
        static WORDS: [[u8; 7]; N] = {
            let mut words = [*b"verb000"; N];
            let mut i = 0;
            while i < N {
                words[i][4] = b'0' + (i / 100) as u8;
                words[i][5] = b'0' + (i / 10 % 10) as u8;
                words[i][6] = b'0' + (i % 10) as u8;
                i += 1;
            }
            words
        };
        let table: std::vec::Vec<VerbGrant> = (0..N)
            .map(|i| VerbGrant {
                verb: core::str::from_utf8(&WORDS[i]).unwrap(),
                authorities: ManifestAuthorities::CATALOG,
            })
            .collect();
        let bytes: [u8; verbs_descriptor_bytes(N)] = encode_verbs(&table);
        let v = decode_verbs(&bytes).unwrap();
        assert_eq!(v.len(), N);
        assert!(!v.is_empty());
        let every = v.iter();
        assert_eq!(every.len(), N);
        // The items outlive the view: the iterator is taken from a view that is then dropped.
        let words: std::vec::Vec<&[u8]> = { decode_verbs(&bytes).unwrap() }
            .iter()
            .map(|(word, _)| word)
            .collect();
        assert_eq!(words.first(), Some(&b"verb000".as_slice()));
        assert_eq!(
            (&v).into_iter().last().map(|(w, _)| w),
            Some(b"verb255".as_slice())
        );
        assert_eq!(v.grants_for(b"verb255"), Some(ManifestAuthorities::CATALOG));
    }

    /// **A prefix sorts first**: `ab` before `abc`, never after, and neither before itself.
    #[test]
    fn a_shorter_prefix_sorts_first() {
        assert!(before(b"ab", b"abc"));
        assert!(!before(b"abc", b"ab"));
        assert!(!before(b"ab", b"ab"));
        assert!(before(b"ab", b"b"));
    }

    /// **`encode_verbs` refuses an unsorted table**, which in a constant is a compile error.
    #[test]
    #[should_panic(expected = "sorted")]
    fn an_unsorted_table_is_not_encoded() {
        let unsorted = [
            VerbGrant {
                verb: "remove",
                authorities: ManifestAuthorities::INSTALLER,
            },
            VerbGrant {
                verb: "list",
                authorities: ManifestAuthorities::CATALOG,
            },
        ];
        let _: [u8; verbs_descriptor_bytes(2)] = encode_verbs(&unsorted);
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
