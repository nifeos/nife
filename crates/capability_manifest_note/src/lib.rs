//! A program's manifest, as the bytes of an ELF note it carries: inside the executable behind a
//! `PT_NOTE`, so it is in the bytes the progenitor hashes (§197 (a package is one archive file),
//! calef's option M2, 2026-09-26 UTC). The wire format between [`carry!`] and the shell and
//! progenitor that read it (AGENTS.md rule 7); `crates/elf` finds the note, and this crate starts
//! at the descriptor: owner [`OWNER`], type [`MANIFEST`], version 1 ([`DESCRIPTOR_LEN`] bytes) or
//! [`ZONED_VERSION`] 2. One manifest has exactly one encoding: [`decode`] refuses anything
//! [`encode`] would not write. See notes/capability-manifest-note.md for the layouts.

//! # BUGS
//!
//! - A foreign build carries a note only by linking an object that holds one: `-Clink-arg=note.o`
//!   works and `llvm-objcopy --add-section` makes no program header. Nothing writes that object yet.
//! - Changing a manifest means relinking, and a new digest: M2's accepted cost.
//!
//! Name: ratified 2026-10-18 (calef, "`capability_manifest_note`"). The descriptor is the capabilities
//! a program requests ("manifest" in the Android sense), and "note" separates the ELF-note encoding
//! from `grant_plan::Manifest`, the in-memory type. The owner string, the type number and the
//! encoding are ratified separately; [`carry!`] and the public functions are provisional.

#![no_std]

use grant_plan::{
    DIAGNOSTICS_SLOT, DirectoryGrantDeclaration, FileGrantDeclaration, IndexStoreDeclaration,
    InputDeclaration, MAX_DECLARED_SHORT_OPTIONS, Manifest, MemoryGrantDeclaration,
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
/// field order is in notes/capability-manifest-note.md.
pub const VERSION: u32 = 1;

/// How long a version 1 descriptor is, exactly; notes/capability-manifest-note.md has the layout.
pub const DESCRIPTOR_LEN: usize = 56;

/// The descriptor version that carries resolver zones after version 1's fields (the version 2
/// table in notes/capability-manifest-note.md). Provisional name; the number and layout are
/// calef's 2026-10-19 (UTC) ruling.
///
/// Name: provisional, minted 2026-10-19 (UTC) by milestone/892-installed-programs-hold-the-network-and-zones.
pub const ZONED_VERSION: u32 = 2;

/// The most zones a manifest may ask for: 8, the resolver's
/// `domain_name_resolution_contract::GRANTS_MAX`, since each granted zone is its own badge and a
/// program asking for more could never be granted them all at once.
///
/// Name: provisional, minted 2026-10-19 (UTC) by milestone/892-installed-programs-hold-the-network-and-zones.
pub const MAX_ZONES: usize = domain_name_resolution_contract::GRANTS_MAX;

/// The longest zone, in bytes of dotted text: 253, a host name's limit and the resolver's
/// `domain_name_resolution_contract::NAME_TEXT_MAX`.
///
/// Name: provisional, minted 2026-10-19 (UTC) by milestone/892-installed-programs-hold-the-network-and-zones.
pub const ZONE_MAX: usize = domain_name_resolution_contract::NAME_TEXT_MAX;

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
const MEM_MIN: usize = 8;
const MEM_MAX: usize = 16;
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
const MACHINE: usize = FLAG_LETTERS + MAX_DECLARED_SHORT_OPTIONS;
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
    if let MemoryGrantDeclaration::Required { min, max } = m.memory_grant {
        out[MEM] = 1;
        put8(&mut out, MEM_MIN, min);
        put8(&mut out, MEM_MAX, max);
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
/// later[0] = 3;
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
        _ => return Err(Error::UnknownVersion),
    }
    let d = d.get(..DESCRIPTOR_LEN).ok_or(Error::WrongLength)?;
    let arg = decode_arg(d)?;
    let mem = decode_mem(d)?;
    let file = decode_file(d)?;
    let count = d[FLAG_COUNT] as usize;
    if count > MAX_DECLARED_SHORT_OPTIONS {
        return Err(Error::BadField(FLAG_COUNT));
    }
    let letters = &d[FLAG_LETTERS..FLAG_LETTERS + count];
    let short_options = ShortOptions::try_new(letters).ok_or(Error::BadField(FLAG_LETTERS))?;
    zero(d, FLAG_LETTERS + count, MAX_DECLARED_SHORT_OPTIONS - count)?;
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
            zero(d, MEM_MIN, 16)?;
            MemoryGrantDeclaration::Forbidden
        }
        1 => {
            let (min, max) = (get8(d, MEM_MIN), get8(d, MEM_MAX));
            if min > max {
                return Err(Error::BadField(MEM_MIN));
            }
            MemoryGrantDeclaration::Required { min, max }
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
        !zones.is_empty() && zones.len() <= MAX_ZONES,
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

/// Every zone of a version 2 descriptor, checked: a count of one to [`MAX_ZONES`], each zone
/// canonical (`domain_name_resolution_contract::is_canonical_zone`) and sorted strictly after the one before, and nothing after the
/// last. The offset in a refusal is the first byte at fault.
fn check_zones(d: &[u8]) -> Result<&[u8], Error> {
    let count = *d.get(DESCRIPTOR_LEN).ok_or(Error::WrongLength)? as usize;
    if count == 0 || count > MAX_ZONES {
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
        _ => Ok(Zones { records: &[] }),
    }
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
pub const MAX_VERBS: usize = 256;

/// One verb's record: the word in `grant_plan::MAX_VERB` bytes, zero-padded; then the grants'
/// bits, little-endian; then two zero bytes.
pub const VERB_RECORD: usize = grant_plan::MAX_VERB + 4;

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
/// [`MAX_VERBS`] entries, an empty or too-long word, a word with a zero byte, or a table that is
/// not sorted ascending by its words' bytes with each word once. Sorted is what makes one table
/// one encoding.
pub const fn encode_verbs<const LEN: usize>(verbs: &[grant_plan::VerbGrant]) -> [u8; LEN] {
    assert!(
        verbs.len() <= MAX_VERBS,
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
            !word.is_empty() && word.len() <= grant_plan::MAX_VERB,
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
        out[at + grant_plan::MAX_VERB] = bits[0];
        out[at + grant_plan::MAX_VERB + 1] = bits[1];
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
        let word = &r[..grant_plan::MAX_VERB];
        let len = word.iter().position(|&b| b == 0).unwrap_or(word.len());
        let bits = u16::from_le_bytes([r[grant_plan::MAX_VERB], r[grant_plan::MAX_VERB + 1]]);
        (
            &word[..len],
            grant_plan::ManifestAuthorities::from_bits(bits)
                .unwrap_or(grant_plan::ManifestAuthorities::NONE),
        )
    }
}

/// **A [`VERBS`] descriptor, decoded**, or why not. One table has one encoding, as [`decode`]
/// enforces for a manifest: an unknown version, a count past [`MAX_VERBS`], a length that is not
/// [`verbs_len`] of the count, an empty word, a zero inside a word or a nonzero byte after it, a
/// grant bit that names no authority, a nonzero pad, or records out of order or repeated, all
/// refuse.
pub fn decode_verbs(d: &[u8]) -> Result<Verbs<'_>, Error> {
    let version = d.get(..4).ok_or(Error::NoVersion)?;
    if u32::from_le_bytes([version[0], version[1], version[2], version[3]]) != VERBS_VERSION {
        return Err(Error::UnknownVersion);
    }
    let count = d.get(4..8).ok_or(Error::WrongLength)?;
    let count = u32::from_le_bytes([count[0], count[1], count[2], count[3]]) as usize;
    if count > MAX_VERBS {
        return Err(Error::BadField(4));
    }
    if d.len() != verbs_len(count) {
        return Err(Error::WrongLength);
    }
    let mut previous: Option<&[u8]> = None;
    for i in 0..count {
        let at = 8 + i * VERB_RECORD;
        let record = &d[at..at + VERB_RECORD];
        let word = &record[..grant_plan::MAX_VERB];
        let len = word.iter().position(|&b| b == 0).unwrap_or(word.len());
        if len == 0 {
            return Err(Error::BadField(at));
        }
        if let Some(k) = word[len..].iter().position(|&b| b != 0) {
            return Err(Error::BadField(at + len + k));
        }
        let bits = u16::from_le_bytes([
            record[grant_plan::MAX_VERB],
            record[grant_plan::MAX_VERB + 1],
        ]);
        if grant_plan::ManifestAuthorities::from_bits(bits).is_none() {
            return Err(Error::BadField(at + grant_plan::MAX_VERB));
        }
        if record[grant_plan::MAX_VERB + 2] != 0 || record[grant_plan::MAX_VERB + 3] != 0 {
            return Err(Error::BadField(at + grant_plan::MAX_VERB + 2));
        }
        if previous.is_some_and(|p| !before(p, &word[..len])) {
            return Err(Error::BadField(at));
        }
        previous = Some(&word[..len]);
    }
    Ok(Verbs { records: &d[8..] })
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
    fn a_table_of_exactly_max_verbs_round_trips() {
        const N: usize = MAX_VERBS;
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
mod tests {
    use grant_plan::Program;

    use super::*;

    /// **The subtree-grants note round-trips, and one encoding is the only encoding** (milestone
    /// 606, ruling T1): the note's own descriptor decodes to its scope, and a wrong version, scope
    /// or length is refused rather than read as "eligible".
    #[test]
    fn a_subtree_grants_note_has_one_encoding() {
        let note = SubtreeGrantsNote::of(Scope::SubtreeScope);
        let desc = &note.0[HEADER_LEN + NAME_FIELD..];
        assert_eq!(decode_subtree_grants(desc), Ok(Scope::SubtreeScope));
        assert_eq!(
            u32::from_le_bytes(note.0[8..12].try_into().unwrap()),
            SUBTREE_GRANTS
        );
        let mut bad = [0u8; SUBTREE_GRANTS_LEN];
        bad.copy_from_slice(desc);
        bad[4] = 2;
        assert_eq!(decode_subtree_grants(&bad), Err(Error::BadField(4)));
        bad[4] = 1;
        bad[0] = 2;
        assert_eq!(decode_subtree_grants(&bad), Err(Error::UnknownVersion));
        assert_eq!(decode_subtree_grants(&desc[..7]), Err(Error::WrongLength));
        assert_eq!(decode_subtree_grants(&[1, 0]), Err(Error::NoVersion));
        assert_eq!(Scope::SubtreeScope.declared(), "subtree_scope");
    }

    /// Every manifest the tree compiles in has a spelling, and reads back as itself. A program
    /// added to `Program` with a manifest this layout cannot carry fails here rather than in a build
    /// that tries to carry it.
    #[test]
    fn every_compiled_in_manifest_round_trips() {
        for p in Program::ALL {
            let m = p.manifest();
            // The two manifests with no note, by design (milestones 805 and 809): see `encode`.
            if m.reboot
                || m.sync
                || m.installer
                || m.catalog
                || m.indexes != IndexStoreDeclaration::Forbidden
                || m.activation_directory
            {
                continue;
            }
            assert_eq!(decode(&encode(&m)), Ok(m), "{}", p.name());
        }
        let m = grant_plan::UNVOUCHED_MANIFEST;
        assert_eq!(decode(&encode(&m)), Ok(m));
        // Each word grant a program that hears words may declare (milestone 205).
        for g in [WordGrant::ReadOnly, WordGrant::ReadWrite, WordGrant::Create] {
            let m = Manifest {
                program_argument: ProgramArgumentDeclaration::Words(g),
                ..grant_plan::UNVOUCHED_STD_MANIFEST
            };
            assert_eq!(decode(&encode(&m)), Ok(m), "{g:?}");
        }
    }

    #[test]
    fn a_later_version_is_refused_rather_than_guessed_at() {
        let mut d = encode(&Program::Uptime.manifest());
        d[0] = 3;
        assert_eq!(decode(&d), Err(Error::UnknownVersion));
        assert_eq!(decode(&d[..3]), Err(Error::NoVersion));
    }

    #[test]
    fn a_descriptor_one_byte_long_or_short_is_refused() {
        let d = encode(&Program::Uptime.manifest());
        assert_eq!(decode(&d[..DESCRIPTOR_LEN - 1]), Err(Error::WrongLength));
        let mut long = [0u8; DESCRIPTOR_LEN + 1];
        long[..DESCRIPTOR_LEN].copy_from_slice(&d);
        assert_eq!(decode(&long), Err(Error::WrongLength));
    }

    #[test]
    fn every_field_refuses_a_value_outside_its_range() {
        let base = encode(&Program::Uptime.manifest());
        for at in [
            ARG,
            MEM,
            FILE,
            DIR,
            OUTPUT,
            INPUT,
            REPORTS,
            INTERRUPTIBLE,
            CLOCK,
            DOMAIN,
            CONFIG,
            ENTROPY,
            NETWORK,
            RUNTIME,
        ] {
            let mut d = base;
            d[at] = 0xff;
            assert_eq!(decode(&d), Err(Error::BadField(at)), "offset {at}");
        }
    }

    #[test]
    fn a_byte_the_layout_says_is_zero_is_refused() {
        let base = encode(&Program::Uptime.manifest());
        // Uptime declares no memory, no input, no directory and no options, so all of these are
        // absent fields, and the three trailing bytes are always zero.
        for at in [
            MEM_MIN,
            MEM_MAX + 7,
            WRITES_WHILE_READING,
            SUBTREE,
            FLAG_LETTERS,
            TAIL,
            DESCRIPTOR_LEN - 1,
        ] {
            let mut d = base;
            d[at] = 1;
            assert_eq!(decode(&d), Err(Error::BadField(at)), "offset {at}");
        }
    }

    #[test]
    fn options_are_distinct_letters_and_the_subtree_option_is_one_of_them() {
        let rm = encode(&Program::Rm.manifest());
        assert_eq!(&rm[FLAG_LETTERS..FLAG_LETTERS + 3], b"rfv");
        assert_eq!(rm[SUBTREE], b'r');

        let mut twice = rm;
        twice[FLAG_LETTERS + 1] = b'r';
        assert_eq!(decode(&twice), Err(Error::BadField(FLAG_LETTERS)));

        let mut undeclared = rm;
        undeclared[SUBTREE] = b'q';
        assert_eq!(decode(&undeclared), Err(Error::BadField(SUBTREE)));

        let mut too_many = rm;
        too_many[FLAG_COUNT] = MAX_DECLARED_SHORT_OPTIONS as u8 + 1;
        assert_eq!(decode(&too_many), Err(Error::BadField(FLAG_COUNT)));
    }

    #[test]
    fn a_memory_range_upside_down_is_refused() {
        let m = Manifest {
            memory_grant: MemoryGrantDeclaration::Required { min: 4, max: 8 },
            ..Program::Uptime.manifest()
        };
        let mut d = encode(&m);
        assert_eq!(decode(&d), Ok(m));
        d[MEM_MIN] = 9;
        assert_eq!(decode(&d), Err(Error::BadField(MEM_MIN)));
    }

    #[test]
    fn the_note_is_what_elf_finds() {
        let n = Note::of(&Program::Uptime.manifest());
        assert_eq!(n.0.len(), NOTE_LEN);
        assert_eq!(&n.0[0..4], &5u32.to_le_bytes());
        assert_eq!(&n.0[4..8], &(DESCRIPTOR_LEN as u32).to_le_bytes());
        assert_eq!(&n.0[8..12], &MANIFEST.to_le_bytes());
        assert_eq!(&n.0[12..20], b"nife\0\0\0\0");
        assert_eq!(decode(&n.0[20..]), Ok(Program::Uptime.manifest()));
    }

    /// The manifest every field of which is at a non-default value except those a test sets, so a
    /// field moved to the wrong byte or dropped shows as a different manifest.
    fn base() -> Manifest {
        grant_plan::NO_NOTE_MANIFEST
    }

    /// **Every value of every enumerated field encodes and decodes to itself**, one field at a
    /// time. A `decode` arm for a value no round trip names is an arm nobody would notice losing:
    /// the descriptor would then be refused as malformed, and only for programs that use it.
    #[test]
    fn every_value_of_every_field_round_trips() {
        let args = [
            ProgramArgumentDeclaration::Forbidden,
            ProgramArgumentDeclaration::Required,
            ProgramArgumentDeclaration::Words(WordGrant::ReadOnly),
            ProgramArgumentDeclaration::Words(WordGrant::ReadWrite),
            ProgramArgumentDeclaration::Words(WordGrant::Create),
        ];
        for arg in args {
            let m = Manifest {
                program_argument: arg,
                ..base()
            };
            assert_eq!(decode(&encode(&m)), Ok(m), "{arg:?}");
        }
        for file in [
            FileGrantDeclaration::Forbidden,
            FileGrantDeclaration::Required { writable: false },
            FileGrantDeclaration::Required { writable: true },
        ] {
            let m = Manifest { file, ..base() };
            assert_eq!(decode(&encode(&m)), Ok(m), "{file:?}");
        }
        for dir in [
            DirectoryGrantDeclaration::Forbidden,
            DirectoryGrantDeclaration::Required {
                subtree_option: None,
            },
        ] {
            let m = Manifest {
                directory: dir,
                ..base()
            };
            assert_eq!(decode(&encode(&m)), Ok(m), "{dir:?}");
        }
        for output in [
            OutputDeclaration::Silent,
            OutputDeclaration::Words,
            OutputDeclaration::Bytes,
            OutputDeclaration::BytesAndDiagnostics {
                slot: DIAGNOSTICS_SLOT,
            },
        ] {
            let m = Manifest { output, ..base() };
            assert_eq!(decode(&encode(&m)), Ok(m), "{output:?}");
        }
    }

    /// A second stream is declared at one slot only, and a manifest naming another has no spelling:
    /// the constant evaluation that would reject it is a compile error, and here a panic.
    #[test]
    #[should_panic(expected = "DIAGNOSTICS_SLOT")]
    fn a_second_stream_at_any_other_slot_has_no_encoding() {
        let m = Manifest {
            output: OutputDeclaration::BytesAndDiagnostics {
                slot: DIAGNOSTICS_SLOT + 1,
            },
            ..base()
        };
        let _ = encode(&m);
    }

    /// The reboot object has no spelling in a note (milestone 805): a program carrying one that
    /// asked for it fails to build, rather than shipping a request no reader may grant.
    #[test]
    #[should_panic(expected = "cannot carry `reboot`")]
    fn the_reboot_object_has_no_encoding() {
        let _ = encode(&grant_plan::Program::Reboot.manifest());
    }

    /// A subtree option is accepted when it is one of the declared letters, including when it is
    /// the only one, and has no encoding when it is not.
    #[test]
    fn a_subtree_option_must_be_a_declared_letter_even_the_only_one() {
        let with = |letters: &[u8], flag: u8| Manifest {
            short_options: ShortOptions::try_new(letters).unwrap(),
            directory: DirectoryGrantDeclaration::Required {
                subtree_option: Some(flag),
            },
            ..base()
        };
        let only = with(b"r", b'r');
        assert_eq!(decode(&encode(&only)), Ok(only));
        let second = with(b"fr", b'r');
        assert_eq!(decode(&encode(&second)), Ok(second));
        extern crate std;
        let undeclared = std::panic::catch_unwind(|| encode(&with(b"r", b'x')));
        assert!(undeclared.is_err());
    }

    /// The boundaries of the layout, each at the value that is last to be accepted: a descriptor of
    /// four bytes has a version and nothing else (wrong length, not "no version"), the longest
    /// option list and an empty memory range are accepted, and the last byte of the layout is
    /// checked to be zero.
    #[test]
    fn the_layout_accepts_its_largest_values_and_checks_its_last_byte() {
        assert_eq!(decode(&[1, 2, 3]), Err(Error::NoVersion));
        assert_eq!(decode(&VERSION.to_le_bytes()), Err(Error::WrongLength));
        assert_eq!(decode(&3u32.to_le_bytes()), Err(Error::UnknownVersion));

        let letters: [u8; MAX_DECLARED_SHORT_OPTIONS] = core::array::from_fn(|i| b'a' + i as u8);
        let m = Manifest {
            short_options: ShortOptions::try_new(&letters).unwrap(),
            ..base()
        };
        assert_eq!(decode(&encode(&m)), Ok(m));

        let m = Manifest {
            memory_grant: MemoryGrantDeclaration::Required { min: 8, max: 8 },
            ..base()
        };
        assert_eq!(decode(&encode(&m)), Ok(m));

        let mut d = encode(&base());
        let last = d.len() - 1;
        d[last] = 1;
        assert_eq!(decode(&d), Err(Error::BadField(last)));
    }

    /// The whole note's header says what an ELF reader expects, for both note types: the owner's
    /// length including its NUL, the descriptor's length, and the type.
    #[test]
    fn both_notes_state_their_owner_length_descriptor_length_and_type() {
        let n = SubtreeGrantsNote::of(Scope::SubtreeScope);
        assert_eq!(&n.0[0..4], &5u32.to_le_bytes());
        assert_eq!(&n.0[4..8], &(SUBTREE_GRANTS_LEN as u32).to_le_bytes());
        assert_eq!(&n.0[8..12], &SUBTREE_GRANTS.to_le_bytes());
        assert_eq!(&n.0[12..20], b"nife\0\0\0\0");
        assert_eq!(&n.0[20..24], &SUBTREE_GRANTS_VERSION.to_le_bytes());
        assert_eq!(&n.0[24..28], &1u32.to_le_bytes());
    }

    /// `put4` lays four bytes forward from `at`, in order, touching nothing else. The notes' own
    /// words are all under 256, so only a word with four different bytes tells `at + i` from
    /// `at - i`. Mutation survivor triage, milestone 326 (turn a mutation score upward), batch 3.
    #[test]
    fn put4_writes_four_bytes_forward_and_nothing_else() {
        let mut out = [0u8; 12];
        put4(&mut out, 4, [1, 2, 3, 4]);
        assert_eq!(out, [0, 0, 0, 0, 1, 2, 3, 4, 0, 0, 0, 0]);
    }

    /// **A version 2 descriptor has one reading** (calef's 2026-10-19 (UTC) ruling on #2405): it
    /// decodes to the manifest its first 56 bytes spell, its zones come back in order, and each way
    /// a second spelling of the same ask could arise is refused: no zones (that is version 1),
    /// unsorted or repeated zones, uppercase, an end or doubled dot, more than eight, or a byte past
    /// the last zone.
    #[test]
    fn a_zoned_descriptor_has_one_encoding() {
        extern crate std;
        use std::vec::Vec;
        const ZONES: &[&str] = &["", "basalt.nifeos.org", "example.net"];
        const M: Manifest = Manifest {
            network: true,
            ..grant_plan::NO_NOTE_MANIFEST
        };
        const D: [u8; zoned_len(ZONES)] = encode_zoned(&M, ZONES);
        assert_eq!(decode(&D), Ok(M));
        let z = zones(&D).unwrap();
        assert!(z.has_root());
        assert_eq!(
            z.iter().collect::<Vec<_>>(),
            [b"".as_slice(), b"basalt.nifeos.org", b"example.net"]
        );
        assert!(!z.is_empty());
        assert!(
            zones(&encode(&M)).unwrap().is_empty(),
            "version 1 asks none"
        );
        const NO_ROOT: &[&str] = &["nife.test"];
        const D2: [u8; zoned_len(NO_ROOT)] = encode_zoned(&M, NO_ROOT);
        assert!(!zones(&D2).unwrap().has_root());
        const EIGHT: &[&str] = &["a", "b", "c", "d", "e", "f", "g", "h"];
        const D8: [u8; zoned_len(EIGHT)] = encode_zoned(&M, EIGHT);
        assert_eq!(
            zones(&D8).unwrap().iter().count(),
            MAX_ZONES,
            "eight is the most, and allowed"
        );
        let unsorted = std::panic::catch_unwind(|| {
            encode_zoned::<{ zoned_len(&["b", "a"]) }>(&M, &["b", "a"])
        });
        assert!(
            unsorted.is_err(),
            "encode refuses unsorted zones, as decode does"
        );

        let respell = |zones: &[&[u8]]| {
            let mut d: Vec<u8> = D[..DESCRIPTOR_LEN].to_vec();
            d.push(zones.len() as u8);
            for z in zones {
                d.push(z.len() as u8);
                d.extend_from_slice(z);
            }
            decode(&d)
        };
        assert_eq!(respell(&[b"", b"basalt.nifeos.org", b"example.net"]), Ok(M));
        for bad in [
            &[][..],
            &[b"example.net".as_slice(), b"basalt.nifeos.org"],
            &[b"example.net".as_slice(), b"example.net"],
            &[b"Example.net".as_slice()],
            &[b"example.net.".as_slice()],
            &[b".example.net".as_slice()],
            &[b"example..net".as_slice()],
            &[b"a", b"b", b"c", b"d", b"e", b"f", b"g", b"h", b"i"],
        ] {
            assert!(
                matches!(respell(bad), Err(Error::BadField(_))),
                "{bad:?} must be refused"
            );
        }
        let mut long = D.to_vec();
        long.push(0);
        assert_eq!(decode(&long), Err(Error::WrongLength));
        assert_eq!(decode(&D[..D.len() - 1]), Err(Error::WrongLength));
    }

    /// **The note around a version 2 descriptor is what `elf` finds**: its `descsz` is the
    /// descriptor's exact length, and the padding after it is zero.
    #[test]
    fn a_zoned_note_names_its_exact_length_and_pads_to_four() {
        const ZONES: &[&str] = &["nife.test"];
        const NOTE: ZonedNote<{ zoned_note_len(ZONES) }> =
            ZonedNote::of::<{ zoned_len(ZONES) }>(&grant_plan::NO_NOTE_MANIFEST, ZONES);
        let n = &NOTE.0;
        // 56 + 1 + 1 + 9 = 67 bytes of descriptor after a 20-byte header and owner, rounded to 88.
        assert_eq!(n.len(), 88);
        assert_eq!(
            u32::from_le_bytes([n[0], n[1], n[2], n[3]]),
            5,
            "namesz: `nife` and its NUL"
        );
        let descsz = u32::from_le_bytes([n[4], n[5], n[6], n[7]]) as usize;
        assert_eq!(descsz, 67);
        assert_eq!(descsz, zoned_len(ZONES));
        let d = &n[HEADER_LEN + NAME_FIELD..HEADER_LEN + NAME_FIELD + descsz];
        assert_eq!(
            zones(d).unwrap().iter().next(),
            Some(b"nife.test".as_slice())
        );
        assert!(
            n[HEADER_LEN + NAME_FIELD + descsz..]
                .iter()
                .all(|&b| b == 0)
        );
    }
}
