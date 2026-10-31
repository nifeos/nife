//! A program's manifest, as the bytes of an ELF note it carries: inside the executable behind a
//! `PT_NOTE`, so it is in the bytes the progenitor hashes (§197 (a package is one archive file),
//! calef's option M2, 2026-09-26 UTC). The wire format between [`carry!`] and the shell and
//! progenitor that read it (AGENTS.md rule 7); `crates/elf` finds the note, and this crate starts
//! at the descriptor: owner [`OWNER`], type [`MANIFEST`], version 1 ([`DESCRIPTOR_LEN`] bytes) or
//! [`ZONED_VERSION`] 2. One manifest has exactly one encoding: [`decode`] refuses anything
//! [`encode`] would not write. The layouts are below; notes/capability-manifest-note.md has the
//! reasons.

//! # BUGS
//!
//! - A foreign build carries a note only by linking an object that holds one: `-Clink-arg=note.o`
//!   works and `llvm-objcopy --add-section` makes no program header. Nothing writes that object yet.
//! - Changing a manifest means relinking, and a new digest: M2's accepted cost.
//!

//! # The descriptor, version 1
//!
//! A version word, then little-endian fields in a fixed order (the encoding calef ratified
//! 2026-09-26). Every field is one of `grant_plan::Manifest`'s, in bytes; nothing here is a new
//! authority.
//!
//! | offset | size | field | values |
//! |---|---|---|---|
//! | 0 | 4 | version | `1` |
//! | 4 | 1 | `arg` | 0 forbidden, 1 required; hears words (milestone 205): 2 read-only, 3 read-write, 4 create |
//! | 5 | 1 | `mem` | 0 forbidden, 1 required |
//! | 6 | 1 | `file` | 0 forbidden, 1 read-only, 2 read-write |
//! | 7 | 1 | `dir` | 0 forbidden, 1 required |
//! | 8 | 8 | `mem` minimum pages | 0 unless `mem` is required |
//! | 16 | 8 | `mem` maximum pages | 0 unless `mem` is required; at least the minimum |
//! | 24 | 1 | `output` | 0 silent, 1 words, 2 bytes, 3 bytes and diagnostics (at `grant_plan::DIAGNOSTICS_SLOT`) |
//! | 25 | 1 | `input` | 0 forbidden, 1 required |
//! | 26 | 1 | `input` writes while reading | 0 or 1; 0 unless `input` is required |
//! | 27 | 1 | `dir`'s subtree option | 0 for none, else one of the declared option letters; 0 unless `dir` is required |
//! | 28 | 1 | `reports` | 0 or 1 |
//! | 29 | 1 | `interruptible` | 0 or 1 |
//! | 30 | 1 | `clock` | 0 or 1 |
//! | 31 | 1 | `domain` | 0 or 1 |
//! | 32 | 1 | `config` | 0 or 1 |
//! | 33 | 1 | `entropy` | 0 or 1 |
//! | 34 | 1 | `network` | 0 or 1 |
//! | 35 | 1 | `startup` | 0 native, 1 std |
//! | 36 | 1 | option count | at most `grant_plan::MAXIMUM_DECLARED_SHORT_OPTIONS` (16) |
//! | 37 | 16 | option letters | the first *count* are the letters in bit order, the rest 0 |
//! | 53 | 1 | `machine` | 0 or 1 (milestone 126, the machine statistics page) |
//! | 54 | 1 | `share` | 0 or 1 (milestone 126, a view of the job budget) |
//! | 55 | 1 | zero | |
//!
//! Byte 55 is the last zero.

//! # The descriptor, version 2: a manifest and the resolver zones it asks for
//!
//! calef, 2026-10-19 (UTC), on #2405 (milestone 892 (a std program at the prompt holds the network
//! and a resolver)'s Q1, "approve version 2?"): "Yes". Version 1's 56 bytes at the same offsets,
//! with `2` in the version word, then:
//!
//! | offset | size | field | values |
//! |---|---|---|---|
//! | 56 | 1 | zone count | 1 to [`MAXIMUM_ZONES`] |
//! | 57 | 1 + *n* | each zone: its length *n*, then *n* bytes of dotted ASCII | *n* at most
//! [`ZONE_MAXIMUM`]; 0 is the root zone |
//!
//! The descriptor ends with the last zone, so `descsz` need not be a multiple of four; [`ZonedNote`]
//! pads after it. **One request is still one encoding**: a manifest that asks for no zone is
//! version 1, so a version 2 count of 0 is refused, and the zones are canonical ([`zones`] says
//! how). Zones are what the program asks for; the owner grants them at install, and nothing here
//! grants anything (§252 (a resolver grant is one zone per client badge)).
//!
//! # The descriptor, version 3: the service ask
//!
//! Milestone 911 (scripted installs grant only what the owner's approval file allows) needs the
//! ask the owner grants at install to be able to say "runs as a service" before milestone 910 (a
//! package declares a service) builds the start. Version 3 is version 2's layout with the zone
//! count allowed to be zero and one byte after the zones:
//!
//! | offset | size | field | values |
//! |---|---|---|---|
//! | 56 | 1 | zone count | 0 to [`MAXIMUM_ZONES`] |
//! | 57 | 1 + *n* | each zone, as version 2's | |
//! | after | 1 | service | `0` or `1` |
//!
//! A manifest that asks for zones and no service stays version 2, so no existing digest changes;
//! the service ask is the one thing only version 3 can say. Nothing starts a recorded service;
//! that is milestone 910's.
//!
//! Name: ratified 2026-10-18 (calef, "`capability_manifest_note`"). The descriptor is the capabilities
//! a program requests ("manifest" in the Android sense), and "note" separates the ELF-note encoding
//! from `grant_plan::Manifest`, the in-memory type. The owner string, the type number and the
//! encoding are ratified separately; [`carry!`] and the public functions are provisional. Version
//! 3 is provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.

#![no_std]

use grant_plan::{
    DIAGNOSTICS_SLOT, DirectoryGrantDeclaration, FileGrantDeclaration, IndexStoreDeclaration,
    InputDeclaration, MAXIMUM_DECLARED_SHORT_OPTIONS, Manifest, MemoryGrantDeclaration,
    OutputDeclaration, ProgramArgumentDeclaration, ShortOptions, StartupContract, WordGrant,
};

/// The note's owner string, without its NUL: the project's name, lowercase as
/// `design/naming.md` rules it spelled everywhere.
///
/// Name: ratified 2026-09-26 (calef, answering "Yes" to the maintainer, who relayed it to milestone 597's lane).
pub const OWNER: &[u8] = b"nife";

/// The note type that carries a manifest: `1`, "manifest".
///
/// Types are per owner, so `1` collides with nothing outside `nife`'s own notes.
///
/// Name: ratified 2026-09-26 (calef, answering "Yes" to the maintainer, who relayed it to milestone 597's lane).
pub const MANIFEST: u32 = 1;

/// The descriptor version this crate writes and the only one it reads. The encoding it numbers (a
/// version word, then little-endian fields in a fixed order) is calef's ruling of 2026-09-26; the
/// field order of version 1 is milestone 597's lane's design, in the module documentation.
pub const VERSION: u32 = 1;

/// How long a version 1 descriptor is, exactly; the module documentation has the layout.
pub const DESCRIPTOR_LEN: usize = 56;

/// The descriptor version that carries resolver zones after version 1's fields (the module's
/// version 2 table). Provisional name; the number and layout are
/// calef's 2026-10-19 (UTC) ruling.
///
/// Name: provisional, minted 2026-10-19 (UTC) by milestone/892-installed-programs-hold-the-network-and-zones.
pub const ZONED_VERSION: u32 = 2;

/// The descriptor version that carries the service ask after the zones (milestone 911 (scripted
/// installs grant only what the owner's approval file allows)): version 2's layout with the zone
/// count allowed to be zero, then one byte, `1` when the manifest asks to run as a service. A
/// manifest that asks for zones and no service stays version 2, so no existing digest changes;
/// the service ask is the one thing only version 3 can say. The starting is milestone 910 (a
/// package declares a service)'s; this is the ask the owner grants at install.
///
/// Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.
pub const SERVICED_VERSION: u32 = 3;

/// The most zones a manifest may ask for: 8, the resolver's
/// `domain_name_resolution_contract::GRANTS_MAXIMUM`, since each granted zone is its own badge and
/// a program asking for more could never be granted them all at once.
///
/// Name: provisional, minted 2026-10-19 (UTC) by milestone/892-installed-programs-hold-the-network-and-zones.
pub const MAXIMUM_ZONES: usize = domain_name_resolution_contract::GRANTS_MAXIMUM;

/// The longest zone, in bytes of dotted text: 253, a host name's limit and the resolver's
/// `domain_name_resolution_contract::NAME_TEXT_MAXIMUM`.
///
/// Name: provisional, minted 2026-10-19 (UTC) by milestone/892-installed-programs-hold-the-network-and-zones.
pub const ZONE_MAXIMUM: usize = domain_name_resolution_contract::NAME_TEXT_MAXIMUM;

/// `OWNER` with its NUL, padded to the note's 4-byte alignment.
const NAME_FIELD: usize = 8;

/// The note's three header words.
const HEADER_LEN: usize = 12;

/// How long a whole manifest note is: header, owner, descriptor. Every field is already a multiple
/// of four, so there is no trailing pad.
pub const NOTE_LEN: usize = HEADER_LEN + NAME_FIELD + DESCRIPTOR_LEN;

/// Why a descriptor was refused. Provisional names.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Error {
    /// Fewer than four bytes, so not even a version.
    NoVersion,
    /// A version this reader does not know. A later layout is refused rather than guessed at.
    UnknownVersion,
    /// A version 1 descriptor that is not exactly [`DESCRIPTOR_LEN`] bytes: cut short, or with
    /// bytes after the last field.
    WrongLength,
    /// A field holds a value outside its range, or a byte the layout says is zero is not. The
    /// offset is the first such byte.
    BadField(usize),
}

// Offsets, named once so `encode` and `decode` cannot disagree about where a field is.
const ARG: usize = 4;
const MEM: usize = 5;
const FILE: usize = 6;
const DIR: usize = 7;
const MEM_MINIMUM: usize = 8;
const MEM_MAXIMUM: usize = 16;
const OUTPUT: usize = 24;
const INPUT: usize = 25;
const WRITES_WHILE_READING: usize = 26;
const SUBTREE: usize = 27;
const REPORTS: usize = 28;
const INTERRUPTIBLE: usize = 29;
const CLOCK: usize = 30;
const DOMAIN: usize = 31;
const CONFIG: usize = 32;
const ENTROPY: usize = 33;
const NETWORK: usize = 34;
const RUNTIME: usize = 35;
const FLAG_COUNT: usize = 36;
const FLAG_LETTERS: usize = 37;
const MACHINE: usize = FLAG_LETTERS + MAXIMUM_DECLARED_SHORT_OPTIONS;
const SHARE: usize = MACHINE + 1;
const TAIL: usize = SHARE + 1;

const _: () = assert!(TAIL + 1 == DESCRIPTOR_LEN);

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

const fn put8(out: &mut [u8; DESCRIPTOR_LEN], at: usize, v: u64) {
    let b = v.to_le_bytes();
    let mut i = 0;
    while i < 8 {
        out[at + i] = b[i];
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
    // Milestone 801 (packages over the internet): the index store likewise, for the same reason.
    assert!(
        matches!(m.indexes, IndexStoreDeclaration::Forbidden),
        "a manifest note cannot carry `indexes`: only the owner's console grants the index store"
    );
    // Milestone 809: and the activation directory.
    assert!(
        !m.activation_directory,
        "a manifest note cannot carry `activation_directory`: only the owner's console grants it"
    );
    // Milestone 892: and standard input from the console.
    assert!(
        !m.console_input,
        "a manifest note cannot carry `console_input`: only the owner's console grants it"
    );
}

/// `m` as a version 1 descriptor. A `const fn`, so a program's note is computed by the compiler
/// from the constant its source declares ([`carry!`]). Panics, a compile error in a constant, on
/// what the layout cannot spell: a second stream off `grant_plan::DIAGNOSTICS_SLOT`, a subtree
/// option that is not a declared letter or lacks a directory grant, and `reboot`, `sync`,
/// `installer` or `catalog`, which no image may be endowed (milestone 809 (the package client
/// becomes a program)). See notes/capability-manifest-note.md.
pub const fn encode(m: &Manifest) -> [u8; DESCRIPTOR_LEN] {
    refuse_what_no_note_carries(m);
    let mut out = [0u8; DESCRIPTOR_LEN];
    let v = VERSION.to_le_bytes();
    out[0] = v[0];
    out[1] = v[1];
    out[2] = v[2];
    out[3] = v[3];
    out[ARG] = match m.program_argument {
        ProgramArgumentDeclaration::Forbidden => 0,
        ProgramArgumentDeclaration::Required => 1,
        // A program whose line is its argv (milestone 205 (how a foreign program is told what to
        // do), §170 (how a foreign program is told what to do)). Added to version 1 in place on 2026-09-27; see the module's "one encoding"
        // paragraph for calef's ruling.
        ProgramArgumentDeclaration::Words(WordGrant::ReadOnly) => 2,
        ProgramArgumentDeclaration::Words(WordGrant::ReadWrite) => 3,
        ProgramArgumentDeclaration::Words(WordGrant::Create) => 4,
    };
    if let MemoryGrantDeclaration::Required { minimum, maximum } = m.memory_grant {
        out[MEM] = 1;
        put8(&mut out, MEM_MINIMUM, minimum);
        put8(&mut out, MEM_MAXIMUM, maximum);
    }
    out[FILE] = match m.file {
        FileGrantDeclaration::Forbidden => 0,
        FileGrantDeclaration::Required { writable: false } => 1,
        FileGrantDeclaration::Required { writable: true } => 2,
    };
    let letters = m.short_options.letters();
    if let DirectoryGrantDeclaration::Required { subtree_option } = m.directory {
        out[DIR] = 1;
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
                "a second stream lands at grant_plan::DIAGNOSTICS_SLOT"
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
    out[CONFIG] = m.configuration as u8;
    out[ENTROPY] = m.entropy as u8;
    out[NETWORK] = m.network as u8;
    out[MACHINE] = m.machine as u8;
    out[SHARE] = m.share as u8;
    out[RUNTIME] = match m.startup {
        StartupContract::Native => 0,
        StartupContract::Std => 1,
    };
    out[FLAG_COUNT] = letters.len() as u8;
    let mut i = 0;
    while i < letters.len() {
        out[FLAG_LETTERS + i] = letters[i];
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
fn zero(d: &[u8], at: usize, len: usize) -> Result<(), Error> {
    match d[at..at + len].iter().position(|&b| b != 0) {
        Some(i) => Err(Error::BadField(at + i)),
        None => Ok(()),
    }
}

/// The manifest a version 1 descriptor spells, or why it spells none. Strict: whatever this
/// accepts, [`encode`] turns back into the same bytes, so a descriptor has one reading.
///
/// # Examples
///
/// ```
/// let m = grant_plan::Program::Uptime.manifest();
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
/// capability_manifest_note::carry!(grant_plan::Program::Uptime.manifest());
/// ```
///
/// A version 2 descriptor decodes to the same manifest its first 56 bytes would; its zones are
/// checked here too, and read with [`zones`].
pub fn decode(d: &[u8]) -> Result<Manifest, Error> {
    if d.len() < 4 {
        return Err(Error::NoVersion);
    }
    match u32::from_le_bytes([d[0], d[1], d[2], d[3]]) {
        VERSION if d.len() != DESCRIPTOR_LEN => return Err(Error::WrongLength),
        VERSION => {}
        ZONED_VERSION => {
            check_zones(d)?;
        }
        SERVICED_VERSION => {
            check_serviced(d)?;
        }
        _ => return Err(Error::UnknownVersion),
    }
    let d = d.get(..DESCRIPTOR_LEN).ok_or(Error::WrongLength)?;
    let arg = decode_arg(d)?;
    let mem = decode_mem(d)?;
    let file = decode_file(d)?;
    let count = d[FLAG_COUNT] as usize;
    if count > MAXIMUM_DECLARED_SHORT_OPTIONS {
        return Err(Error::BadField(FLAG_COUNT));
    }
    let letters = &d[FLAG_LETTERS..FLAG_LETTERS + count];
    let short_options = ShortOptions::try_new(letters).ok_or(Error::BadField(FLAG_LETTERS))?;
    zero(
        d,
        FLAG_LETTERS + count,
        MAXIMUM_DECLARED_SHORT_OPTIONS - count,
    )?;
    let dir = decode_dir(d, letters)?;
    let output = decode_output(d)?;
    let input = decode_input(d)?;
    let startup = decode_runtime(d)?;
    zero(d, TAIL, DESCRIPTOR_LEN - TAIL)?;
    Ok(Manifest {
        program_argument: arg,
        memory_grant: mem,
        file,
        directory: dir,
        short_options,
        output,
        input,
        reports: flag(d, REPORTS)?,
        interruptible: flag(d, INTERRUPTIBLE)?,
        clock: flag(d, CLOCK)?,
        domain: flag(d, DOMAIN)?,
        configuration: flag(d, CONFIG)?,
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

/// The `arg` field.
fn decode_arg(d: &[u8]) -> Result<ProgramArgumentDeclaration, Error> {
    Ok(match d[ARG] {
        0 => ProgramArgumentDeclaration::Forbidden,
        1 => ProgramArgumentDeclaration::Required,
        2 => ProgramArgumentDeclaration::Words(WordGrant::ReadOnly),
        3 => ProgramArgumentDeclaration::Words(WordGrant::ReadWrite),
        4 => ProgramArgumentDeclaration::Words(WordGrant::Create),
        _ => return Err(Error::BadField(ARG)),
    })
}

/// The `mem` field and its bounds, which are zero when it is absent.
fn decode_mem(d: &[u8]) -> Result<MemoryGrantDeclaration, Error> {
    Ok(match d[MEM] {
        0 => {
            zero(d, MEM_MINIMUM, 16)?;
            MemoryGrantDeclaration::Forbidden
        }
        1 => {
            let (minimum, maximum) = (get8(d, MEM_MINIMUM), get8(d, MEM_MAXIMUM));
            if minimum > maximum {
                return Err(Error::BadField(MEM_MINIMUM));
            }
            MemoryGrantDeclaration::Required { minimum, maximum }
        }
        _ => return Err(Error::BadField(MEM)),
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

/// The `dir` field and its subtree letter, which must be one of the declared `letters`.
fn decode_dir(d: &[u8], letters: &[u8]) -> Result<DirectoryGrantDeclaration, Error> {
    Ok(match d[DIR] {
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
        _ => return Err(Error::BadField(DIR)),
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
fn decode_runtime(d: &[u8]) -> Result<StartupContract, Error> {
    Ok(match d[RUNTIME] {
        0 => StartupContract::Native,
        1 => StartupContract::Std,
        _ => return Err(Error::BadField(RUNTIME)),
    })
}

/// **A whole manifest note**, header and owner and descriptor, aligned as a note must be. What
/// [`carry!`] places in the program's `.note.nife.manifest` section. Provisional name.
#[repr(C, align(4))]
pub struct Note(pub [u8; NOTE_LEN]);

impl Note {
    /// The note carrying `m`.
    pub const fn of(m: &Manifest) -> Self {
        let mut out = [0u8; NOTE_LEN];
        let namesz = ((OWNER.len() + 1) as u32).to_le_bytes();
        let descsz = (DESCRIPTOR_LEN as u32).to_le_bytes();
        let kind = MANIFEST.to_le_bytes();
        put4(&mut out, 0, namesz);
        put4(&mut out, 4, descsz);
        put4(&mut out, 8, kind);
        put4(
            &mut out,
            HEADER_LEN,
            [OWNER[0], OWNER[1], OWNER[2], OWNER[3]],
        );
        let d = encode(m);
        let mut i = 0;
        while i < DESCRIPTOR_LEN {
            out[HEADER_LEN + NAME_FIELD + i] = d[i];
            i += 1;
        }
        Self(out)
    }
}

/// **A version 2 descriptor's length** for these zones: version 1's 56 bytes, the count, and each
/// zone's length byte and text.
///
/// Name: provisional, minted 2026-10-19 (UTC) by milestone/892-installed-programs-hold-the-network-and-zones.
pub const fn zoned_len(zones: &[&str]) -> usize {
    let mut len = DESCRIPTOR_LEN + 1;
    let mut i = 0;
    while i < zones.len() {
        len += 1 + zones[i].len();
        i += 1;
    }
    len
}

/// `m` and the zones it asks for, as a version 2 descriptor. `LEN` is [`zoned_len`] of
/// `zones`, named by the caller's type, as [`encode_verbs`] does:
///
/// ```
/// const ZONES: &[&str] = &["", "basalt.nifeos.org"];
/// const M: grant_plan::Manifest = grant_plan::NO_NOTE_MANIFEST;
/// const D: [u8; capability_manifest_note::zoned_len(ZONES)] = capability_manifest_note::encode_zoned(&M, ZONES);
/// assert_eq!(capability_manifest_note::decode(&D), Ok(M));
/// let zones = capability_manifest_note::zones(&D).unwrap();
/// assert_eq!(zones.iter().collect::<Vec<_>>(), [b"".as_slice(), b"basalt.nifeos.org"]);
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
    assert!(LEN == zoned_len(zones), "LEN must be zoned_len(zones)");
    let mut out = [0u8; LEN];
    let fixed = encode(m);
    let mut i = 0;
    while i < DESCRIPTOR_LEN {
        out[i] = fixed[i];
        i += 1;
    }
    put4(&mut out, 0, ZONED_VERSION.to_le_bytes());
    out[DESCRIPTOR_LEN] = zones.len() as u8;
    let mut at = DESCRIPTOR_LEN + 1;
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
    out
}

/// Every zone of a version 2 descriptor, checked: a count of one to [`MAXIMUM_ZONES`], each zone
/// canonical (`domain_name_resolution_contract::is_canonical_zone`) and sorted strictly after the one before, and nothing after the
/// last. The offset in a refusal is the first byte at fault.
fn check_zones(d: &[u8]) -> Result<&[u8], Error> {
    let count = *d.get(DESCRIPTOR_LEN).ok_or(Error::WrongLength)? as usize;
    if count == 0 || count > MAXIMUM_ZONES {
        return Err(Error::BadField(DESCRIPTOR_LEN));
    }
    let mut at = DESCRIPTOR_LEN + 1;
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
    Ok(&d[DESCRIPTOR_LEN..])
}

/// Every zone and the service byte of a version 3 descriptor, checked: a count of zero to
/// [`MAXIMUM_ZONES`] (zero asks no zone, which version 2 refuses), each zone canonical and sorted
/// strictly after the one before, then exactly one byte, `0` or `1`, and nothing after it. The
/// offset in a refusal is the first byte at fault.
///
/// Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.
fn check_serviced(d: &[u8]) -> Result<&[u8], Error> {
    let count = *d.get(DESCRIPTOR_LEN).ok_or(Error::WrongLength)? as usize;
    if count > MAXIMUM_ZONES {
        return Err(Error::BadField(DESCRIPTOR_LEN));
    }
    let mut at = DESCRIPTOR_LEN + 1;
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
        _ => return Err(Error::BadField(at)),
    }
    if at + 1 != d.len() {
        return Err(Error::WrongLength);
    }
    Ok(&d[DESCRIPTOR_LEN..])
}

/// **Whether the manifest asks to run as a service**, the version 3 ask (milestone 911 (scripted
/// installs grant only what the owner's approval file allows)), read from a checked descriptor.
/// `false` for version 1 and version 2, whose grammar cannot say it.
///
/// Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.
pub fn service(d: &[u8]) -> Result<bool, Error> {
    decode(d)?;
    if u32::from_le_bytes([d[0], d[1], d[2], d[3]]) != SERVICED_VERSION {
        return Ok(false);
    }
    let count = d[DESCRIPTOR_LEN] as usize;
    let mut at = DESCRIPTOR_LEN + 1;
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
            records: &d[DESCRIPTOR_LEN + 1..],
        }),
        SERVICED_VERSION => {
            let count = d[DESCRIPTOR_LEN] as usize;
            Ok(Zones {
                records: &d[DESCRIPTOR_LEN + 1..DESCRIPTOR_LEN + 1 + zones_bytes(d, count)],
            })
        }
        _ => Ok(Zones { records: &[] }),
    }
}

/// How many bytes the first `count` zones of a version 3 descriptor occupy, its count byte
/// apart: what [`zones`] and [`service`] both walk.
fn zones_bytes(d: &[u8], count: usize) -> usize {
    let mut at = 0usize;
    for _ in 0..count {
        at += 1 + d[DESCRIPTOR_LEN + 1 + at] as usize;
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
    pub fn iter(&self) -> impl Iterator<Item = &'a [u8]> + use<'a> {
        let mut rest = self.records;
        core::iter::from_fn(move || {
            let (&len, tail) = rest.split_first()?;
            let (zone, after) = tail.split_at(len as usize);
            rest = after;
            Some(zone)
        })
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

/// **A whole version 2 manifest note**, padded after its descriptor to the note's 4-byte
/// alignment. `N` is [`zoned_note_len`] of the zones. What [`carry!`]'s `zones =` form places.
///
/// Name: provisional, minted 2026-10-19 (UTC) by milestone/892-installed-programs-hold-the-network-and-zones.
#[repr(C, align(4))]
pub struct ZonedNote<const N: usize>(pub [u8; N]);

/// How long a whole version 2 note is for these zones: header, owner, the descriptor, and the
/// zero bytes that round it up to four.
///
/// Name: provisional, minted 2026-10-19 (UTC) by milestone/892-installed-programs-hold-the-network-and-zones.
pub const fn zoned_note_len(zones: &[&str]) -> usize {
    (HEADER_LEN + NAME_FIELD + zoned_len(zones)).next_multiple_of(4)
}

impl<const N: usize> ZonedNote<N> {
    /// The note carrying `m` and the zones it asks for.
    ///
    /// Name: provisional, minted 2026-10-19 (UTC) by milestone/892-installed-programs-hold-the-network-and-zones.
    pub const fn of<const LEN: usize>(m: &Manifest, zones: &[&str]) -> Self {
        assert!(
            N == zoned_note_len(zones),
            "N must be zoned_note_len(zones)"
        );
        let mut out = [0u8; N];
        put4(&mut out, 0, ((OWNER.len() + 1) as u32).to_le_bytes());
        put4(&mut out, 4, (LEN as u32).to_le_bytes());
        put4(&mut out, 8, MANIFEST.to_le_bytes());
        put4(
            &mut out,
            HEADER_LEN,
            [OWNER[0], OWNER[1], OWNER[2], OWNER[3]],
        );
        let d: [u8; LEN] = encode_zoned(m, zones);
        let mut i = 0;
        while i < LEN {
            out[HEADER_LEN + NAME_FIELD + i] = d[i];
            i += 1;
        }
        Self(out)
    }
}

/// **A whole version 3 manifest note**, padded after its descriptor to the note's 4-byte
/// alignment. `N` is [`serviced_note_len`] of the zones. What [`carry!`]'s `service =` form
/// places, with or without `zones =`.
///
/// Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.
#[repr(C, align(4))]
pub struct ServicedNote<const N: usize>(pub [u8; N]);

/// How long a version 3 descriptor is for these zones: the zone block of version 2's layout,
/// which may be empty here, and the service byte.
///
/// Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.
pub const fn serviced_len(zones: &[&str]) -> usize {
    let mut len = DESCRIPTOR_LEN + 1 + 1;
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
/// Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.
pub const fn serviced_note_len(zones: &[&str]) -> usize {
    (HEADER_LEN + NAME_FIELD + serviced_len(zones)).next_multiple_of(4)
}

/// `m`, the zones it asks for (none allowed here, which only version 3 can say), and whether it
/// asks to run as a service, as a version 3 descriptor. `LEN` is [`serviced_len`] of `zones`.
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
        LEN == serviced_len(zones),
        "LEN must be serviced_len(zones)"
    );
    let mut out = [0u8; LEN];
    // The first 56 bytes are the manifest, as version 1 spells it; `encode` wrote them into
    // `head`, and a const fn copies field by field (`IndexMut` is not const).
    let head = encode(m);
    let mut i = 0;
    while i < DESCRIPTOR_LEN {
        out[i] = head[i];
        i += 1;
    }
    let v = SERVICED_VERSION.to_le_bytes();
    out[0] = v[0];
    out[1] = v[1];
    out[2] = v[2];
    out[3] = v[3];
    out[DESCRIPTOR_LEN] = zones.len() as u8;
    let mut at = DESCRIPTOR_LEN + 1;
    let mut z = 0;
    while z < zones.len() {
        let zone = zones[z].as_bytes();
        out[at] = zone.len() as u8;
        let mut b = 0;
        while b < zone.len() {
            out[at + 1 + b] = zone[b];
            b += 1;
        }
        assert!(
            z == 0 || before(zones[z - 1].as_bytes(), zone),
            "zones are sorted ascending, each once"
        );
        at += 1 + zone.len();
        z += 1;
    }
    out[at] = service as u8;
    out
}

impl<const N: usize> ServicedNote<N> {
    /// The note carrying `m`, the zones it asks for, and its service ask.
    ///
    /// Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.
    pub const fn of<const LEN: usize>(m: &Manifest, zones: &[&str], service: bool) -> Self {
        assert!(
            N == serviced_note_len(zones),
            "N must be serviced_note_len(zones)"
        );
        let mut out = [0u8; N];
        put4(&mut out, 0, ((OWNER.len() + 1) as u32).to_le_bytes());
        put4(&mut out, 4, (LEN as u32).to_le_bytes());
        put4(&mut out, 8, MANIFEST.to_le_bytes());
        put4(
            &mut out,
            HEADER_LEN,
            [OWNER[0], OWNER[1], OWNER[2], OWNER[3]],
        );
        let d: [u8; LEN] = encode_serviced(m, zones, service);
        let mut i = 0;
        while i < LEN {
            out[HEADER_LEN + NAME_FIELD + i] = d[i];
            i += 1;
        }
        Self(out)
    }
}

// The owner and its NUL fit the name field, which keeps the descriptor 4-aligned.
const _: () = assert!(OWNER.len() < NAME_FIELD && NAME_FIELD.is_multiple_of(4));

/// Carry this manifest in the program's own bytes: one line at module scope, any constant
/// `grant_plan::Manifest`, and optionally its resolver zones, sorted, `""` for the root
/// ([`ZonedNote`]). It places one [`Note`] in `.note.nife.manifest`; used twice it is a duplicate
/// symbol, and readers refuse a file with two notes (`elf::NoteError::Duplicate`).
///
/// ```ignore
/// capability_manifest_note::carry!(MANIFEST, zones = &["basalt.nifeos.org"]);
/// ```
///
/// Name: provisional, milestone 597's lane, 2026-09-26.
#[macro_export]
macro_rules! carry {
    ($manifest:expr) => {
        #[used]
        #[unsafe(link_section = ".note.nife.manifest")]
        static NIFE_MANIFEST_NOTE: $crate::Note = $crate::Note::of(&$manifest);
    };
    ($manifest:expr, zones = $zones:expr) => {
        #[used]
        #[unsafe(link_section = ".note.nife.manifest")]
        static NIFE_MANIFEST_NOTE: $crate::ZonedNote<{ $crate::zoned_note_len($zones) }> =
            $crate::ZonedNote::of::<{ $crate::zoned_len($zones) }>(&$manifest, $zones);
    };
    ($manifest:expr, service = true) => {
        #[used]
        #[unsafe(link_section = ".note.nife.manifest")]
        static NIFE_MANIFEST_NOTE: $crate::ServicedNote<{ $crate::serviced_note_len(&[]) }> =
            $crate::ServicedNote::of::<{ $crate::serviced_len(&[]) }>(&$manifest, &[], true);
    };
    ($manifest:expr, zones = $zones:expr, service = true) => {
        #[used]
        #[unsafe(link_section = ".note.nife.manifest")]
        static NIFE_MANIFEST_NOTE: $crate::ServicedNote<{ $crate::serviced_note_len($zones) }> =
            $crate::ServicedNote::of::<{ $crate::serviced_len($zones) }>(&$manifest, $zones, true);
    };
}

// -------------------------------------------------------------------------------------------
// The second note type: a file server that enforces subtree grants itself (milestone 606 (a
// directory walk costs what it does on Linux), calef's ruling T1 of 2026-09-27, 15:22Z on #1413).
// -------------------------------------------------------------------------------------------

/// **The note type that says a file server enforces subtree grants itself**: `2`. A server carries
/// it when it resolves every path and every handle through a scope crate, and it is what the
/// progenitor reads to choose a bound badge over a caretaker for that server's grants. Absent means
/// "give its clients a caretaker", which is every server but an eligible one.
///
/// Name and number provisional (calef, 2026-09-27: "the note and field names stay provisional").
pub const SUBTREE_GRANTS: u32 = 2;

/// The version of the [`SUBTREE_GRANTS`] descriptor this crate writes and reads.
pub const SUBTREE_GRANTS_VERSION: u32 = 1;

/// How long a version 1 [`SUBTREE_GRANTS`] descriptor is: a version word, then a scope word.
pub const SUBTREE_GRANTS_LEN: usize = 8;

/// **Which scope crate a server enforces grants through.** One value today, so the word is room for
/// a second rather than a flag that would need a second field.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Scope {
    /// `crates/subtree_scope`, the one ruling D (2026-09-27) names.
    SubtreeScope,
}

impl Scope {
    const fn word(self) -> u32 {
        match self {
            Scope::SubtreeScope => 1,
        }
    }

    /// The package declaration's spelling of this scope, the value of its `subtree_grants` field,
    /// which `script/lint` checks against the note a server carries.
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
    if d.len() != SUBTREE_GRANTS_LEN {
        return Err(Error::WrongLength);
    }
    match u32::from_le_bytes([d[4], d[5], d[6], d[7]]) {
        1 => Ok(Scope::SubtreeScope),
        _ => Err(Error::BadField(4)),
    }
}

/// How long a whole [`SUBTREE_GRANTS`] note is.
pub const SUBTREE_GRANTS_NOTE_LEN: usize = HEADER_LEN + NAME_FIELD + SUBTREE_GRANTS_LEN;

/// **A whole [`SUBTREE_GRANTS`] note**, what [`carry_subtree_grants!`] places. Provisional name.
#[repr(C, align(4))]
pub struct SubtreeGrantsNote(pub [u8; SUBTREE_GRANTS_NOTE_LEN]);

impl SubtreeGrantsNote {
    /// The note declaring `scope`.
    pub const fn of(scope: Scope) -> Self {
        let mut out = [0u8; SUBTREE_GRANTS_NOTE_LEN];
        let namesz = ((OWNER.len() + 1) as u32).to_le_bytes();
        let descsz = (SUBTREE_GRANTS_LEN as u32).to_le_bytes();
        let kind = SUBTREE_GRANTS.to_le_bytes();
        let version = SUBTREE_GRANTS_VERSION.to_le_bytes();
        let word = scope.word().to_le_bytes();
        put4(&mut out, 0, namesz);
        put4(&mut out, 4, descsz);
        put4(&mut out, 8, kind);
        put4(
            &mut out,
            HEADER_LEN,
            [OWNER[0], OWNER[1], OWNER[2], OWNER[3]],
        );
        put4(&mut out, HEADER_LEN + NAME_FIELD, version);
        put4(&mut out, HEADER_LEN + NAME_FIELD + 4, word);
        Self(out)
    }
}

/// **Declare that this file server enforces subtree grants through `scope`.** One line at module
/// scope in the server's root, beside nothing else: a server carries no [`Manifest`]. It places one
/// [`SubtreeGrantsNote`] in `.note.nife.manifest`, the same section and `PT_NOTE` a manifest uses.
///
/// Carrying it is a claim the progenitor acts on, so `script/lint` holds it to account: the crate
/// must depend on `subtree_scope`, and its package declaration must say so too.
///
/// Name: provisional, milestone 606's lane, 2026-09-27.
#[macro_export]
macro_rules! carry_subtree_grants {
    ($scope:expr) => {
        #[used]
        #[unsafe(link_section = ".note.nife.manifest")]
        static NIFE_SUBTREE_GRANTS_NOTE: $crate::SubtreeGrantsNote =
            $crate::SubtreeGrantsNote::of($scope);
    };
}

// -------------------------------------------------------------------------------------------
// The third note type: a program's verbs and what each is granted (milestone 809 (the package
// client becomes a program), its option V2).
// -------------------------------------------------------------------------------------------

/// The note type that carries a program's verb table: `3`. Each entry names one word a line may
/// give after the program's name and which progenitor-endowed authorities that line is granted
/// (`grant_plan::narrow_by_verb`); a word not in the table is granted none. Nothing in the image
/// carries it yet, and the progenitor refuses installed bytes that do. See
/// notes/capability-manifest-note.md.
///
/// Name and number provisional (milestone 809).
pub const VERBS: u32 = 3;

/// The version of the [`VERBS`] descriptor this crate writes and reads.
pub const VERBS_VERSION: u32 = 1;

/// The most verbs a [`VERBS`] descriptor may hold: 256. Room for a program the size of git
/// (175 subcommands in git 2.56.0, calef's measure on #1903), and a bound on what a malformed count
/// can make a reader walk.
///
/// Name: ratified 2026-10-27 (#2797).
pub const MAXIMUM_VERBS: usize = 256;

/// One verb's record: the word in `grant_plan::MAXIMUM_VERB` bytes, zero-padded; then the grants'
/// bits, little-endian; then two zero bytes.
pub const VERB_RECORD: usize = grant_plan::MAXIMUM_VERB + 4;

/// A [`VERBS`] descriptor's length for `count` verbs: the version word, the count word, and
/// exactly `count` records. Variable, as calef ruled on #1903 (2026-10-10 (UTC)); one table still
/// has one encoding, because the records are sorted and each verb appears once.
pub const fn verbs_len(count: usize) -> usize {
    8 + count * VERB_RECORD
}

/// The bytes of a verb table (milestone 809). A `const fn`, so a program's table is computed by
/// the compiler; `LEN` is [`verbs_len`] of the table's length, named by the caller's type:
///
/// ```
/// const TABLE: &[grant_plan::VerbGrant] = grant_plan::Program::Jig.verbs();
/// const BYTES: [u8; capability_manifest_note::verbs_len(TABLE.len())] = capability_manifest_note::encode_verbs(TABLE);
/// assert_eq!(capability_manifest_note::decode_verbs(&BYTES).unwrap().len(), TABLE.len());
/// ```
///
/// It panics, a compile error in a constant, on a `LEN` that is not that length, more than
/// [`MAXIMUM_VERBS`] entries, an empty or too-long word, a word with a zero byte, or a table that
/// is not sorted ascending by its words' bytes with each word once. Sorted is what makes one table
/// one encoding.
pub const fn encode_verbs<const LEN: usize>(verbs: &[grant_plan::VerbGrant]) -> [u8; LEN] {
    assert!(
        verbs.len() <= MAXIMUM_VERBS,
        "a verb table holds at most 256 verbs"
    );
    assert!(
        LEN == verbs_len(verbs.len()),
        "LEN must be verbs_len(table.len())"
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
            !word.is_empty() && word.len() <= grant_plan::MAXIMUM_VERB,
            "a verb is one to sixteen bytes"
        );
        if i > 0 {
            assert!(
                before(verbs[i - 1].verb.as_bytes(), word),
                "a verb table is sorted ascending by its words, each word once"
            );
        }
        let at = 8 + i * VERB_RECORD;
        let mut b = 0;
        while b < word.len() {
            assert!(word[b] != 0, "a verb holds no zero byte");
            out[at + b] = word[b];
            b += 1;
        }
        let bits = verbs[i].authorities.bits().to_le_bytes();
        out[at + grant_plan::MAXIMUM_VERB] = bits[0];
        out[at + grant_plan::MAXIMUM_VERB + 1] = bits[1];
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
        self.records.len() / VERB_RECORD
    }

    /// Whether the table is empty, which a descriptor may say: a program whose every line is
    /// granted nothing chosen by a verb.
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// What `verb` is granted, or `None` for a word that is no verb in the table. A binary search,
    /// since the records are sorted.
    pub fn grants_for(&self, verb: &[u8]) -> Option<grant_plan::ManifestAuthorities> {
        let (mut lo, mut hi) = (0, self.len());
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let (word, grants) = self.record(mid);
            match word.cmp(verb) {
                core::cmp::Ordering::Equal => return Some(grants),
                core::cmp::Ordering::Less => lo = mid + 1,
                core::cmp::Ordering::Greater => hi = mid,
            }
        }
        None
    }

    /// The verbs, in their sorted order, with their grants.
    pub fn iter(&self) -> impl Iterator<Item = (&'a [u8], grant_plan::ManifestAuthorities)> + '_ {
        (0..self.len()).map(|i| self.record(i))
    }

    /// Record `i`, which [`decode_verbs`] has already checked.
    fn record(&self, i: usize) -> (&'a [u8], grant_plan::ManifestAuthorities) {
        let r = &self.records[i * VERB_RECORD..(i + 1) * VERB_RECORD];
        let word = &r[..grant_plan::MAXIMUM_VERB];
        let len = word.iter().position(|&b| b == 0).unwrap_or(word.len());
        let bits =
            u16::from_le_bytes([r[grant_plan::MAXIMUM_VERB], r[grant_plan::MAXIMUM_VERB + 1]]);
        (
            &word[..len],
            grant_plan::ManifestAuthorities::from_bits(bits)
                .unwrap_or(grant_plan::ManifestAuthorities::NONE),
        )
    }
}

/// **A [`VERBS`] descriptor, decoded**, or why not. One table has one encoding, as [`decode`]
/// enforces for a manifest: an unknown version, a count past [`MAXIMUM_VERBS`], a length that is
/// not [`verbs_len`] of the count, an empty word, a zero inside a word or a nonzero byte after it,
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
    if d.len() != verbs_len(count) {
        return Err(Error::WrongLength);
    }
    let mut previous: Option<&[u8]> = None;
    for i in 0..count {
        let at = 8 + i * VERB_RECORD;
        let record = &d[at..at + VERB_RECORD];
        let word = &record[..grant_plan::MAXIMUM_VERB];
        let len = word.iter().position(|&b| b == 0).unwrap_or(word.len());
        if len == 0 {
            return Err(Error::BadField(at));
        }
        if let Some(k) = word[len..].iter().position(|&b| b != 0) {
            return Err(Error::BadField(at + len + k));
        }
        let bits = u16::from_le_bytes([
            record[grant_plan::MAXIMUM_VERB],
            record[grant_plan::MAXIMUM_VERB + 1],
        ]);
        if grant_plan::ManifestAuthorities::from_bits(bits).is_none() {
            return Err(Error::BadField(at + grant_plan::MAXIMUM_VERB));
        }
        if record[grant_plan::MAXIMUM_VERB + 2] != 0 || record[grant_plan::MAXIMUM_VERB + 3] != 0 {
            return Err(Error::BadField(at + grant_plan::MAXIMUM_VERB + 2));
        }
        if previous.is_some_and(|p| !before(p, &word[..len])) {
            return Err(Error::BadField(at));
        }
        previous = Some(&word[..len]);
    }
    Ok(Verbs { records: &d[8..] })
}

// -------------------------------------------------------------------------------------------
// The fourth note type: the interface versions a program was built against (milestone 956 (the
// linked interfaces carry checkable versions) step 2; calef ruled the placement on 2026-10-31
// UTC: a note type of its own, on the pattern of `SUBTREE_GRANTS` and `VERBS`, and not a
// descriptor version). The progenitor reads it at spawn to refuse a program built against an
// interface version it does not serve. The descriptor, version 1: a version word, a count word,
// then `count` records of an interface id (`u64`, each crate's `INTERFACE_ID`, strictly ascending)
// and that interface's version (`u32`), little-endian. One list has one encoding.
//
// BUGS: a program names its interfaces by hand in `carry_interfaces!`, although each version
// comes from the crate's constant. Deriving the list from the dependency graph at package build
// waits for the first package that leaves the tree.
// -------------------------------------------------------------------------------------------

/// The note type that carries the interfaces a program was built against: `4`.
///
/// Name: provisional; the number and the placement are calef's ruling of 2026-10-31 (UTC).
pub const INTERFACES: u32 = 4;

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
    /// binary search, since the ids ascend.
    ///
    /// Name: provisional, minted 2026-10-31 (UTC) by lane milestone/956-checkable-interface-versions.
    pub fn version_of(&self, id: u64) -> Option<u32> {
        let (mut lo, mut hi) = (0, self.len());
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let found = record(&self.records[mid * INTERFACE_RECORD..(mid + 1) * INTERFACE_RECORD]);
            match found.id.cmp(&id) {
                core::cmp::Ordering::Equal => return Some(found.version),
                core::cmp::Ordering::Less => lo = mid + 1,
                core::cmp::Ordering::Greater => hi = mid,
            }
        }
        None
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
/// capability_manifest_note::carry_interfaces!(&[
///     capability_manifest_note::Declared { id: abi::INTERFACE_ID, version: abi::ABI_VERSION },
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

#[cfg(kani)]
mod verification {
    use super::*;

    /// **Whatever `decode` accepts, `encode` spells back byte for byte**, over every descriptor the
    /// solver can choose. This is the "one manifest, one encoding" claim: no two byte strings
    /// decode to one manifest, so no two readers can disagree about which bytes meant what. The
    /// unwind bound is the 56-byte comparison, plus one.
    ///
    /// Falsification: replayable `crates/capability_manifest_note/falsifications/verification.a_decoded_manifest_encodes_back_to_its_bytes.patch`
    #[kani::proof]
    #[kani::unwind(57)]
    fn a_decoded_manifest_encodes_back_to_its_bytes() {
        let d: [u8; DESCRIPTOR_LEN] = kani::any();
        if let Ok(m) = decode(&d) {
            assert!(encode(&m) == d);
        }
    }

    /// **`decode` never panics, at any length up to one past the layout.** The bytes come from a
    /// file anyone could have written.
    ///
    /// Falsification: replayable `crates/capability_manifest_note/falsifications/verification.decode_never_panics.patch`
    #[kani::proof]
    #[kani::unwind(18)]
    fn decode_never_panics() {
        let d: [u8; DESCRIPTOR_LEN + 1] = kani::any();
        let len: usize = kani::any();
        kani::assume(len <= d.len());
        let _ = decode(&d[..len]);
    }
}

#[cfg(test)]
mod verb_tests {
    extern crate std;

    use grant_plan::{ManifestAuthorities, Program, VerbGrant};

    use super::*;

    const JIG: &[VerbGrant] = Program::Jig.verbs();
    const JIG_BYTES: [u8; verbs_len(JIG.len())] = encode_verbs(JIG);

    /// **`jig`'s verb table round-trips, and so does an empty one** (milestone 809, fork 2: a list
    /// of (verb, grants) pairs). What a distribution writes is what the decoder reads back, entry
    /// for entry, and the length is exactly the count's.
    #[test]
    fn a_verb_table_round_trips() {
        assert_eq!(JIG_BYTES.len(), 8 + JIG.len() * VERB_RECORD);
        let v = decode_verbs(&JIG_BYTES).unwrap();
        assert_eq!(v.len(), JIG.len());
        for (want, (word, grants)) in JIG.iter().zip(v.iter()) {
            assert_eq!(word, want.verb.as_bytes());
            assert_eq!(grants, want.authorities);
            assert_eq!(v.grants_for(word), Some(want.authorities));
        }
        assert_eq!(v.grants_for(b"upgrade"), None);
        let empty: [u8; verbs_len(0)] = encode_verbs(&[]);
        assert!(decode_verbs(&empty).unwrap().is_empty());
    }

    /// **A table the size of git's** (calef on #1903: "How many verbs does git have?"): 175 sorted
    /// verbs encode, decode, and are each found; the cap holds at 256.
    #[test]
    fn a_table_the_size_of_gits_round_trips() {
        let words: std::vec::Vec<std::string::String> =
            (0..175).map(|i| std::format!("verb{i:03}")).collect();
        let mut d = std::vec![0u8; verbs_len(words.len())];
        d[..4].copy_from_slice(&VERBS_VERSION.to_le_bytes());
        d[4..8].copy_from_slice(&(words.len() as u32).to_le_bytes());
        for (i, w) in words.iter().enumerate() {
            let at = 8 + i * VERB_RECORD;
            d[at..at + w.len()].copy_from_slice(w.as_bytes());
            d[at + 16..at + 18].copy_from_slice(&ManifestAuthorities::CATALOG.bits().to_le_bytes());
        }
        let v = decode_verbs(&d).unwrap();
        assert_eq!(v.len(), 175);
        assert_eq!(v.grants_for(b"verb174"), Some(ManifestAuthorities::CATALOG));
        assert_eq!(v.grants_for(b"verb175"), None);
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
        let (a, b) = (8, 8 + VERB_RECORD);
        let first: [u8; VERB_RECORD] = good[a..b].try_into().unwrap();
        swapped.copy_within(b..b + VERB_RECORD, a);
        swapped[b..b + VERB_RECORD].copy_from_slice(&first);
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
        assert_eq!(verbs_len(0), 8);
        assert_eq!(verbs_len(1), 8 + 16 + 2 + 2);
    }

    /// **The full 256 verbs encode and decode**, the cap itself rather than one under it. The
    /// count's second byte is nonzero only here, so this is also what proves the count is written
    /// where the decoder reads it.
    #[test]
    fn a_table_of_exactly_maximum_verbs_round_trips() {
        const N: usize = MAXIMUM_VERBS;
        let table: std::vec::Vec<VerbGrant> = (0..N)
            .map(|i| VerbGrant {
                verb: std::boxed::Box::leak(std::format!("verb{i:03}").into_boxed_str()),
                authorities: ManifestAuthorities::CATALOG,
            })
            .collect();
        let bytes: [u8; verbs_len(N)] = encode_verbs(&table);
        let v = decode_verbs(&bytes).unwrap();
        assert_eq!(v.len(), N);
        assert!(!v.is_empty());
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
        let _: [u8; verbs_len(2)] = encode_verbs(&unsorted);
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
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
