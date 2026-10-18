//! **A program's manifest, as the bytes of an ELF note it carries.** §197 (a package is one archive
//! file) asked where a manifest travels, and calef ruled option M2 on 2026-09-26 (UTC): inside the
//! executable, as a note behind a `PT_NOTE` program header, so it is in the bytes the progenitor
//! hashes and cannot drift from the code. This crate is the wire format between the program that
//! carries a note ([`carry!`]) and the two that read one: the shell, which binds a line against
//! it, and the progenitor, which endows from it (AGENTS.md rule 7). Finding the note is
//! `crates/elf`'s job (`elf::Elf::note`, `elf::NoteSearch`); this crate starts at the descriptor,
//! whose layout is below.
//!
//! | | |
//! |---|---|
//! | owner | [`OWNER`], `nife` (with its NUL, `namesz` 5) |
//! | type | [`MANIFEST`], `1`, "manifest" |
//! | descriptor | [`DESCRIPTOR_LEN`] bytes, the layout below |
//! | section | `.note.nife.manifest`, which `crates/user_mode_runtime/link.ld` keeps in a `PT_NOTE` inside the read-only load segment |
//!
//! **One manifest has exactly one encoding**, and [`decode`] enforces it: an unknown version, an
//! out-of-range value, a nonzero reserved byte or a wrong length all refuse, and whatever it
//! accepts [`encode`] turns back into the same bytes
//! (`verification::a_decoded_manifest_encodes_back_to_its_bytes`), so two readers cannot
//! disagree. calef amended version 1 in place while nothing outside the tree had acted on it; from
//! here a change is a new version word, not a tolerated extension.
//!
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
//! | 35 | 1 | `runtime` | 0 native, 1 std |
//! | 36 | 1 | option count | at most `grant_plan::MAX_DECLARED_SHORT_OPTIONS` (16) |
//! | 37 | 16 | option letters | the first *count* are the letters in bit order, the rest 0 |
//! | 53 | 1 | `machine` | 0 or 1 (milestone 126, the machine statistics page) |
//! | 54 | 1 | `share` | 0 or 1 (milestone 126, a view of the job budget) |
//! | 55 | 1 | zero | |
//!
//! Byte 55 is the last zero, so the next field is likely version 2.
//!
//! Name: ratified 2026-10-18 (calef, "`capability_manifest_note`"). The descriptor is the capabilities
//! a program requests ("manifest" in the Android sense), and "note" separates the ELF-note encoding
//! from `grant_plan::Manifest`, the in-memory type. The owner string, the type number and the
//! encoding are ratified separately; [`carry!`] and the public functions are provisional.

#![no_std]

use grant_plan::{
    ArgSpec, DIAGNOSTICS_SLOT, DirSpec, FileSpec, IndexStore, InputSpec,
    MAX_DECLARED_SHORT_OPTIONS, Manifest, MemSpec, OutputSpec, Runtime, ShortOptions, WordGrant,
};

/// **The note's owner string**, without its NUL: the project's name, lowercase as
/// `design/naming.md` rules it spelled everywhere.
///
/// Name: ratified 2026-09-26 (calef, answering "Yes" to the maintainer, who relayed it to milestone 597's lane).
pub const OWNER: &[u8] = b"nife";

/// **The note type that carries a manifest**: `1`, "manifest".
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

/// **`m` as a version 1 descriptor.**
///
/// A `const fn`, so a program's note is computed by the compiler from the same constant its source
/// declares ([`carry!`]). It panics on the manifests the layout has no spelling for, and in
/// a constant that panic is a compile error rather than a note nobody can read: a declared second
/// stream at a slot other than `grant_plan::DIAGNOSTICS_SLOT`, a subtree option that is not one of
/// the declared letters (or is declared without a directory grant),
/// `grant_plan::Manifest::reboot` or `grant_plan::Manifest::sync`, and since milestone 809 (the
/// package client becomes a program) `installer` and `catalog`, for the same reason: no image may
/// be endowed them, and `jig` is endowed from the table.
///
/// **`reboot` and `sync` have no byte on purpose** (milestone 805 (`reboot` at the prompt), DECISIONS §251
/// (restarting the machine is a kernel object the progenitor hands out)). A note is how installed
/// bytes ask for authority, and `grant_plan::image_can_carry` refuses both to every image, so a
/// byte for either would spell a request no reader may grant. Leaving it out keeps the
/// layout's last zero byte for version 2 and keeps this wire format unamended; the one program that
/// declares them is endowed from `grant_plan`'s table, which carries no note.
pub const fn encode(m: &Manifest) -> [u8; DESCRIPTOR_LEN] {
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
        matches!(m.indexes, IndexStore::Forbidden),
        "a manifest note cannot carry `indexes`: only the owner's console grants the index store"
    );
    let mut out = [0u8; DESCRIPTOR_LEN];
    let v = VERSION.to_le_bytes();
    out[0] = v[0];
    out[1] = v[1];
    out[2] = v[2];
    out[3] = v[3];
    out[ARG] = match m.arg {
        ArgSpec::Forbidden => 0,
        ArgSpec::Required => 1,
        // A program whose line is its argv (milestone 205 (how a foreign program is told what to
        // do), §170 (how a foreign program is told what to do)). Added to version 1 in place on 2026-09-27; see the module's "one encoding"
        // paragraph for calef's ruling.
        ArgSpec::Words(WordGrant::ReadOnly) => 2,
        ArgSpec::Words(WordGrant::ReadWrite) => 3,
        ArgSpec::Words(WordGrant::Create) => 4,
    };
    if let MemSpec::Required { min, max } = m.mem {
        out[MEM] = 1;
        put8(&mut out, MEM_MIN, min);
        put8(&mut out, MEM_MAX, max);
    }
    out[FILE] = match m.file {
        FileSpec::Forbidden => 0,
        FileSpec::Required { writable: false } => 1,
        FileSpec::Required { writable: true } => 2,
    };
    let letters = m.short_options.letters();
    if let DirSpec::Required { subtree_option } = m.dir {
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
        OutputSpec::Silent => 0,
        OutputSpec::Words => 1,
        OutputSpec::Bytes => 2,
        OutputSpec::BytesAndDiagnostics { slot } => {
            assert!(
                slot == DIAGNOSTICS_SLOT,
                "a second stream lands at grant_plan::DIAGNOSTICS_SLOT"
            );
            3
        }
    };
    if let InputSpec::Required {
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
    out[CONFIG] = m.config as u8;
    out[ENTROPY] = m.entropy as u8;
    out[NETWORK] = m.network as u8;
    out[MACHINE] = m.machine as u8;
    out[SHARE] = m.share as u8;
    out[RUNTIME] = match m.runtime {
        Runtime::Native => 0,
        Runtime::Std => 1,
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

/// **The manifest a version 1 descriptor spells**, or why it spells none.
///
/// Strict on purpose, for the module documentation's reason: whatever this accepts, [`encode`]
/// turns back into the same bytes, so a descriptor has one reading.
///
/// # Examples
///
/// ```
/// let m = grant_plan::Prog::Uptime.manifest();
/// let bytes = capability_manifest_note::encode(&m);
/// assert_eq!(capability_manifest_note::decode(&bytes), Ok(m));
///
/// let mut later = bytes;
/// later[0] = 2;
/// assert_eq!(capability_manifest_note::decode(&later), Err(capability_manifest_note::Error::UnknownVersion));
/// ```
///
/// A program carries its note with one line at module scope:
///
/// ```ignore
/// capability_manifest_note::carry!(grant_plan::Prog::Uptime.manifest());
/// ```
pub fn decode(d: &[u8]) -> Result<Manifest, Error> {
    if d.len() < 4 {
        return Err(Error::NoVersion);
    }
    if u32::from_le_bytes([d[0], d[1], d[2], d[3]]) != VERSION {
        return Err(Error::UnknownVersion);
    }
    if d.len() != DESCRIPTOR_LEN {
        return Err(Error::WrongLength);
    }
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
    let runtime = decode_runtime(d)?;
    zero(d, TAIL, DESCRIPTOR_LEN - TAIL)?;
    Ok(Manifest {
        arg,
        mem,
        file,
        dir,
        short_options,
        output,
        input,
        reports: flag(d, REPORTS)?,
        interruptible: flag(d, INTERRUPTIBLE)?,
        clock: flag(d, CLOCK)?,
        domain: flag(d, DOMAIN)?,
        config: flag(d, CONFIG)?,
        entropy: flag(d, ENTROPY)?,
        network: flag(d, NETWORK)?,
        machine: flag(d, MACHINE)?,
        share: flag(d, SHARE)?,
        // Never from a note: see `encode`.
        reboot: false,
        sync: false,
        installer: false,
        catalog: false,
        indexes: IndexStore::Forbidden,
        runtime,
    })
}

/// The `arg` field.
fn decode_arg(d: &[u8]) -> Result<ArgSpec, Error> {
    Ok(match d[ARG] {
        0 => ArgSpec::Forbidden,
        1 => ArgSpec::Required,
        2 => ArgSpec::Words(WordGrant::ReadOnly),
        3 => ArgSpec::Words(WordGrant::ReadWrite),
        4 => ArgSpec::Words(WordGrant::Create),
        _ => return Err(Error::BadField(ARG)),
    })
}

/// The `mem` field and its bounds, which are zero when it is absent.
fn decode_mem(d: &[u8]) -> Result<MemSpec, Error> {
    Ok(match d[MEM] {
        0 => {
            zero(d, MEM_MIN, 16)?;
            MemSpec::Forbidden
        }
        1 => {
            let (min, max) = (get8(d, MEM_MIN), get8(d, MEM_MAX));
            if min > max {
                return Err(Error::BadField(MEM_MIN));
            }
            MemSpec::Required { min, max }
        }
        _ => return Err(Error::BadField(MEM)),
    })
}

/// The `file` field.
fn decode_file(d: &[u8]) -> Result<FileSpec, Error> {
    Ok(match d[FILE] {
        0 => FileSpec::Forbidden,
        1 => FileSpec::Required { writable: false },
        2 => FileSpec::Required { writable: true },
        _ => return Err(Error::BadField(FILE)),
    })
}

/// The `dir` field and its subtree letter, which must be one of the declared `letters`.
fn decode_dir(d: &[u8], letters: &[u8]) -> Result<DirSpec, Error> {
    Ok(match d[DIR] {
        0 => {
            zero(d, SUBTREE, 1)?;
            DirSpec::Forbidden
        }
        1 => DirSpec::Required {
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
fn decode_output(d: &[u8]) -> Result<OutputSpec, Error> {
    Ok(match d[OUTPUT] {
        0 => OutputSpec::Silent,
        1 => OutputSpec::Words,
        2 => OutputSpec::Bytes,
        3 => OutputSpec::BytesAndDiagnostics {
            slot: DIAGNOSTICS_SLOT,
        },
        _ => return Err(Error::BadField(OUTPUT)),
    })
}

/// The `input` field and whether it writes while reading, which is zero when it is absent.
fn decode_input(d: &[u8]) -> Result<InputSpec, Error> {
    Ok(match d[INPUT] {
        0 => {
            zero(d, WRITES_WHILE_READING, 1)?;
            InputSpec::Forbidden
        }
        1 => InputSpec::Required {
            writes_while_reading: flag(d, WRITES_WHILE_READING)?,
        },
        _ => return Err(Error::BadField(INPUT)),
    })
}

/// The `runtime` field.
fn decode_runtime(d: &[u8]) -> Result<Runtime, Error> {
    Ok(match d[RUNTIME] {
        0 => Runtime::Native,
        1 => Runtime::Std,
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

// The owner and its NUL fit the name field, which keeps the descriptor 4-aligned.
const _: () = assert!(OWNER.len() < NAME_FIELD && NAME_FIELD.is_multiple_of(4));

/// **Carry this manifest in the program's own bytes.** One line at module scope in a program's
/// root; the argument is any constant expression of type `grant_plan::Manifest`.
///
/// It places one [`Note`] in `.note.nife.manifest`, which `crates/user_mode_runtime/link.ld` keeps
/// in a `PT_NOTE` inside the read-only load segment. Used twice in one program it is a duplicate
/// symbol, and if two notes reached one file anyway the readers refuse the file
/// (`elf::NoteError::Duplicate`).
///
/// Name: provisional, milestone 597's lane, 2026-09-26.
#[macro_export]
macro_rules! carry {
    ($manifest:expr) => {
        #[used]
        #[unsafe(link_section = ".note.nife.manifest")]
        static NIFE_MANIFEST_NOTE: $crate::Note = $crate::Note::of(&$manifest);
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

/// **The note type that carries a program's verb table**: `3`. A list of (verb, grants) pairs,
/// `grant_plan::VerbGrant`'s bytes: each entry names one word a line may give after the program's
/// name and which of the manifest's progenitor-endowed authorities that line is granted
/// (`grant_plan::narrow_by_verb`). A word not in the table is granted none of them. It is its own
/// note rather than a field of [`MANIFEST`] because a manifest is one fixed-length value every
/// reader copies, and a table of words is not; most programs carry none.
///
/// The image's own `jig` is endowed from `grant_plan`'s table, which carries the same list, so
/// nothing in the image carries this note yet. It is written here, with its decoder, so a
/// distribution can declare one for a program it packages (basalt, at milestone 810 (`ripgrep` is
/// packaged in basalt and installed with `jig`)). Until a reader honors it, the progenitor refuses
/// installed bytes that carry it rather than endow them with every verb's grants at once.
///
/// Name and number provisional (milestone 809).
pub const VERBS: u32 = 3;

/// The version of the [`VERBS`] descriptor this crate writes and reads.
pub const VERBS_VERSION: u32 = 1;

/// **The most verbs a [`VERBS`] descriptor may hold**: 256. Room for a program the size of git
/// (175 subcommands in git 2.56.0, calef's measure on #1903), and a bound on what a malformed count
/// can make a reader walk.
pub const MAX_VERBS: usize = 256;

/// One verb's record: the word in `grant_plan::MAX_VERB` bytes, zero-padded; then the grants'
/// bits, little-endian; then two zero bytes.
pub const VERB_RECORD: usize = grant_plan::MAX_VERB + 4;

/// **A [`VERBS`] descriptor's length for `count` verbs**: the version word, the count word, and
/// exactly `count` records. Variable, as calef ruled on #1903 (2026-10-10 (UTC)); one table still
/// has one encoding, because the records are sorted and each verb appears once.
pub const fn verbs_len(count: usize) -> usize {
    8 + count * VERB_RECORD
}

/// **The bytes of a verb table** (milestone 809). A `const fn`, so a program's table is computed by
/// the compiler; `LEN` is [`verbs_len`] of the table's length, named by the caller's type:
///
/// ```
/// const TABLE: &[grant_plan::VerbGrant] = grant_plan::Prog::Jig.verbs();
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
        let bits = verbs[i].grants.bits().to_le_bytes();
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
    pub fn grants_for(&self, verb: &[u8]) -> Option<grant_plan::Grants> {
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
    pub fn iter(&self) -> impl Iterator<Item = (&'a [u8], grant_plan::Grants)> + '_ {
        (0..self.len()).map(|i| self.record(i))
    }

    /// Record `i`, which [`decode_verbs`] has already checked.
    fn record(&self, i: usize) -> (&'a [u8], grant_plan::Grants) {
        let r = &self.records[i * VERB_RECORD..(i + 1) * VERB_RECORD];
        let word = &r[..grant_plan::MAX_VERB];
        let len = word.iter().position(|&b| b == 0).unwrap_or(word.len());
        let bits = u16::from_le_bytes([r[grant_plan::MAX_VERB], r[grant_plan::MAX_VERB + 1]]);
        (
            &word[..len],
            grant_plan::Grants::from_bits(bits).unwrap_or(grant_plan::Grants::NONE),
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
        if grant_plan::Grants::from_bits(bits).is_none() {
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

    use grant_plan::{Grants, Prog, VerbGrant};

    use super::*;

    const JIG: &[VerbGrant] = Prog::Jig.verbs();
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
            assert_eq!(grants, want.grants);
            assert_eq!(v.grants_for(word), Some(want.grants));
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
            d[at + 16..at + 18].copy_from_slice(&Grants::CATALOG.bits().to_le_bytes());
        }
        let v = decode_verbs(&d).unwrap();
        assert_eq!(v.len(), 175);
        assert_eq!(v.grants_for(b"verb174"), Some(Grants::CATALOG));
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
                grants: Grants::CATALOG,
            })
            .collect();
        let bytes: [u8; verbs_len(N)] = encode_verbs(&table);
        let v = decode_verbs(&bytes).unwrap();
        assert_eq!(v.len(), N);
        assert!(!v.is_empty());
        assert_eq!(v.grants_for(b"verb255"), Some(Grants::CATALOG));
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
                grants: Grants::INSTALLER,
            },
            VerbGrant {
                verb: "list",
                grants: Grants::CATALOG,
            },
        ];
        let _: [u8; verbs_len(2)] = encode_verbs(&unsorted);
    }
}

#[cfg(test)]
mod tests {
    use grant_plan::Prog;

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
    /// added to `Prog` with a manifest this layout cannot carry fails here rather than in a build
    /// that tries to carry it.
    #[test]
    fn every_compiled_in_manifest_round_trips() {
        for p in Prog::ALL {
            let m = p.manifest();
            // The two manifests with no note, by design (milestones 805 and 809): see `encode`.
            if m.reboot || m.sync || m.installer || m.catalog || m.indexes != IndexStore::Forbidden
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
                arg: ArgSpec::Words(g),
                ..grant_plan::UNVOUCHED_STD_MANIFEST
            };
            assert_eq!(decode(&encode(&m)), Ok(m), "{g:?}");
        }
    }

    #[test]
    fn a_later_version_is_refused_rather_than_guessed_at() {
        let mut d = encode(&Prog::Uptime.manifest());
        d[0] = 2;
        assert_eq!(decode(&d), Err(Error::UnknownVersion));
        assert_eq!(decode(&d[..3]), Err(Error::NoVersion));
    }

    #[test]
    fn a_descriptor_one_byte_long_or_short_is_refused() {
        let d = encode(&Prog::Uptime.manifest());
        assert_eq!(decode(&d[..DESCRIPTOR_LEN - 1]), Err(Error::WrongLength));
        let mut long = [0u8; DESCRIPTOR_LEN + 1];
        long[..DESCRIPTOR_LEN].copy_from_slice(&d);
        assert_eq!(decode(&long), Err(Error::WrongLength));
    }

    #[test]
    fn every_field_refuses_a_value_outside_its_range() {
        let base = encode(&Prog::Uptime.manifest());
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
        let base = encode(&Prog::Uptime.manifest());
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
        let rm = encode(&Prog::Rm.manifest());
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
            mem: MemSpec::Required { min: 4, max: 8 },
            ..Prog::Uptime.manifest()
        };
        let mut d = encode(&m);
        assert_eq!(decode(&d), Ok(m));
        d[MEM_MIN] = 9;
        assert_eq!(decode(&d), Err(Error::BadField(MEM_MIN)));
    }

    #[test]
    fn the_note_is_what_elf_finds() {
        let n = Note::of(&Prog::Uptime.manifest());
        assert_eq!(n.0.len(), NOTE_LEN);
        assert_eq!(&n.0[0..4], &5u32.to_le_bytes());
        assert_eq!(&n.0[4..8], &(DESCRIPTOR_LEN as u32).to_le_bytes());
        assert_eq!(&n.0[8..12], &MANIFEST.to_le_bytes());
        assert_eq!(&n.0[12..20], b"nife\0\0\0\0");
        assert_eq!(decode(&n.0[20..]), Ok(Prog::Uptime.manifest()));
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
            ArgSpec::Forbidden,
            ArgSpec::Required,
            ArgSpec::Words(WordGrant::ReadOnly),
            ArgSpec::Words(WordGrant::ReadWrite),
            ArgSpec::Words(WordGrant::Create),
        ];
        for arg in args {
            let m = Manifest { arg, ..base() };
            assert_eq!(decode(&encode(&m)), Ok(m), "{arg:?}");
        }
        for file in [
            FileSpec::Forbidden,
            FileSpec::Required { writable: false },
            FileSpec::Required { writable: true },
        ] {
            let m = Manifest { file, ..base() };
            assert_eq!(decode(&encode(&m)), Ok(m), "{file:?}");
        }
        for dir in [
            DirSpec::Forbidden,
            DirSpec::Required {
                subtree_option: None,
            },
        ] {
            let m = Manifest { dir, ..base() };
            assert_eq!(decode(&encode(&m)), Ok(m), "{dir:?}");
        }
        for output in [
            OutputSpec::Silent,
            OutputSpec::Words,
            OutputSpec::Bytes,
            OutputSpec::BytesAndDiagnostics {
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
            output: OutputSpec::BytesAndDiagnostics {
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
        let _ = encode(&grant_plan::Prog::Reboot.manifest());
    }

    /// A subtree option is accepted when it is one of the declared letters, including when it is
    /// the only one, and has no encoding when it is not.
    #[test]
    fn a_subtree_option_must_be_a_declared_letter_even_the_only_one() {
        let with = |letters: &[u8], flag: u8| Manifest {
            short_options: ShortOptions::try_new(letters).unwrap(),
            dir: DirSpec::Required {
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
        assert_eq!(decode(&2u32.to_le_bytes()), Err(Error::UnknownVersion));

        let letters: [u8; MAX_DECLARED_SHORT_OPTIONS] = core::array::from_fn(|i| b'a' + i as u8);
        let m = Manifest {
            short_options: ShortOptions::try_new(&letters).unwrap(),
            ..base()
        };
        assert_eq!(decode(&encode(&m)), Ok(m));

        let m = Manifest {
            mem: MemSpec::Required { min: 8, max: 8 },
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
}
