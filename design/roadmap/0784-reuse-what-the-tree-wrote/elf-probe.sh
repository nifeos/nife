#!/bin/sh
# Probes behind elf-against-three-readers.md, beside this script. Name provisional.
#
#   elf-probe.sh write            # write the scratch crates under $WORK (every mode does this first)
#   elf-probe.sh agree DIR...     # the four readers on program images, then their parse time
#   elf-probe.sh edge             # crafted note and alignment cases, and goblin's misaligned header
#   elf-probe.sh fit              # each reader, no allocator, on the three nife targets; code size
#   elf-probe.sh fuzz [SECONDS]   # one cargo-fuzz target per reader, run one after another
#   elf-probe.sh kani             # our eight properties asked of each candidate
#
# Every probe is generated OUTSIDE this repository, under $WORK, so this tree's Cargo.lock and
# rust-toolchain.toml stay untouched and nothing here is a dependency of nife. Versions are pinned
# with `=`. DIR is a directory of built program images, such as target/<triple>/release; `agree`
# rewrites each image's e_machine to the host's in memory, so every image reaches the same paths.
#
# BUGS: needs the network. Host figures only; nothing here runs on target. `fuzz` and `kani` are
# heavy: run them one at a time on a shared machine.
set -eu
HERE=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
ROOT=$(CDPATH='' cd -- "$HERE/../../.." && pwd)
WORK="${WORK:-${TMPDIR:-/tmp}/nife-784-elf-probe}"
MODE=${1:?usage: elf-probe.sh write|agree|edge|fit|fuzz|kani}
shift
PIN=$(sed -n 's/^channel = "\(.*\)"$/\1/p' "$ROOT/rust-toolchain.toml")
mkdir -p "$WORK"

write() {
    mkdir -p "$WORK/adapters"
    cat > "$WORK/adapters/Cargo.toml" <<TOML
[package]
name = "adapters"
version = "0.0.0"
edition = "2024"
publish = false

[features]
default = []
alloc = []
ours = ["dep:elf_ours"]
cole = ["dep:elf"]
object = ["dep:object"]
object_unaligned = ["object", "object/unaligned"]
goblin_noalloc = ["dep:goblin"]
goblin = ["alloc", "dep:goblin", "goblin/elf32", "goblin/elf64", "goblin/endian_fd"]

[dependencies]
elf_ours = { package = "elf", path = "$ROOT/crates/elf", optional = true }
elf = { version = "=0.8.0", default-features = false, optional = true }
object = { version = "=0.40.0", default-features = false, features = ["read_core", "elf"], optional = true }
goblin = { version = "=0.10.7", default-features = false, features = ["elf64"], optional = true }
TOML
    mkdir -p "$WORK/adapters/src"
    cat > "$WORK/adapters/src/lib.rs" <<'SOURCE'
//! What each candidate needs around it to do our loader's job.
//!
//! `policy` is the part we keep whichever parser we take: the loader's own rules (one machine,
//! ET_EXEC only, W^X, no page shared, the entry in an executable segment, at most 64 headers,
//! duplicate notes refused). Each candidate module only reads fields and hands them to it, so the
//! line count of a module is the glue that candidate costs.
#![no_std]
#[cfg(feature = "alloc")]
extern crate alloc;

pub const MAX_PHNUM: usize = 64;
pub const PT_LOAD: u32 = 1;
pub const PT_NOTE: u32 = 4;
pub const ET_EXEC: u16 = 2;
pub const EM_AARCH64: u16 = 183;
pub const EM_RISCV: u16 = 243;
pub const EM_X86_64: u16 = 62;

#[cfg(target_arch = "riscv64")]
pub const NATIVE: u16 = EM_RISCV;
#[cfg(target_arch = "x86_64")]
pub const NATIVE: u16 = EM_X86_64;
#[cfg(not(any(target_arch = "riscv64", target_arch = "x86_64")))]
pub const NATIVE: u16 = EM_AARCH64;

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct Seg {
    pub vaddr: u64,
    pub paddr: u64,
    pub memsz: u64,
    pub flags: u32,
    pub off: usize,
    pub filesz: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    /// The candidate's own parser refused the bytes.
    Parse,
    /// Our policy refused what the candidate read.
    Policy(&'static str),
}

pub struct Loaded {
    pub entry: u64,
    pub segs: [Seg; MAX_PHNUM],
    pub n: usize,
}

impl Loaded {
    pub fn segments(&self) -> &[Seg] {
        &self.segs[..self.n]
    }
}

pub mod policy {
    use super::*;

    pub fn header(e_type: u16, e_machine: u16, phnum: usize) -> Result<(), Refused> {
        if e_machine != NATIVE {
            return Err(Refused::Policy("machine"));
        }
        if e_type != ET_EXEC {
            return Err(Refused::Policy("type"));
        }
        if phnum > MAX_PHNUM {
            return Err(Refused::Policy("phnum"));
        }
        Ok(())
    }

    pub fn segment(s: &Seg, file_len: usize) -> Result<(), Refused> {
        if (s.memsz as usize) < s.filesz {
            return Err(Refused::Policy("truncated"));
        }
        let end = s.off.checked_add(s.filesz).ok_or(Refused::Policy("bounds"))?;
        if end > file_len {
            return Err(Refused::Policy("bounds"));
        }
        s.vaddr.checked_add(s.memsz).ok_or(Refused::Policy("overflow"))?;
        let w = s.flags & 2 != 0;
        let x = s.flags & 1 != 0;
        let r = s.flags & 4 != 0;
        if w && x {
            return Err(Refused::Policy("wx"));
        }
        if !r && !x {
            return Err(Refused::Policy("unreachable"));
        }
        Ok(())
    }

    pub fn page_range(s: &Seg, page: u64) -> (u64, u64) {
        let start = s.vaddr & !(page - 1);
        let end = s.vaddr.saturating_add(s.memsz).div_ceil(page).saturating_mul(page);
        (start, end)
    }

    pub fn whole(l: &Loaded) -> Result<(), Refused> {
        let segs = l.segments();
        let mut entry_ok = false;
        for (i, s) in segs.iter().enumerate() {
            if s.flags & 1 != 0 && l.entry >= s.vaddr && l.entry < s.vaddr + s.memsz {
                entry_ok = true;
            }
            let (a0, a1) = page_range(s, 4096);
            for o in &segs[..i] {
                let (b0, b1) = page_range(o, 4096);
                if a0 < b1 && b0 < a1 {
                    return Err(Refused::Policy("overlap"));
                }
            }
        }
        if !entry_ok {
            return Err(Refused::Policy("entry"));
        }
        Ok(())
    }

    /// One note seen during a walk: our rule is the NUL-terminated owner must match exactly, and a
    /// second match is refused.
    pub fn note_match<'a>(
        name_with_nul: &[u8],
        ntype: u32,
        desc: &'a [u8],
        owner: &[u8],
        kind: u32,
        found: &mut Option<&'a [u8]>,
    ) -> Result<(), Refused> {
        if ntype == kind
            && name_with_nul.len() == owner.len() + 1
            && name_with_nul[owner.len()] == 0
            && &name_with_nul[..owner.len()] == owner
        {
            if found.is_some() {
                return Err(Refused::Policy("duplicate"));
            }
            *found = Some(desc);
        }
        Ok(())
    }

    pub fn push(l: &mut Loaded, s: Seg, file_len: usize) -> Result<(), Refused> {
        segment(&s, file_len)?;
        if l.n == MAX_PHNUM {
            return Err(Refused::Policy("phnum"));
        }
        l.segs[l.n] = s;
        l.n += 1;
        Ok(())
    }

    pub fn empty(entry: u64) -> Loaded {
        Loaded { entry, segs: [Seg::default(); MAX_PHNUM], n: 0 }
    }
}

#[cfg(feature = "ours")]
pub mod ours {
    use super::*;
    pub fn load(b: &[u8]) -> Result<Loaded, Refused> {
        let e = elf_ours::Elf::parse(b).map_err(|_| Refused::Parse)?;
        let mut l = policy::empty(e.entry());
        for s in e.segments() {
            let off = s.data.as_ptr() as usize - b.as_ptr() as usize;
            l.segs[l.n] = Seg { vaddr: s.vaddr, paddr: s.paddr, memsz: s.memsz, flags: s.flags, off, filesz: s.data.len() };
            l.n += 1;
        }
        Ok(l)
    }
    pub fn note<'a>(b: &'a [u8], owner: &[u8], kind: u32) -> Result<Option<&'a [u8]>, Refused> {
        let e = elf_ours::Elf::parse(b).map_err(|_| Refused::Parse)?;
        e.note(owner, kind).map_err(|_| Refused::Parse)
    }
}

#[cfg(feature = "cole")]
pub mod cole {
    use super::*;
    use elf::{ElfBytes, endian::LittleEndian, file::Class, note::Note};

    fn open(b: &[u8]) -> Result<ElfBytes<'_, LittleEndian>, Refused> {
        let f = ElfBytes::<LittleEndian>::minimal_parse(b).map_err(|_| Refused::Parse)?;
        if f.ehdr.class != Class::ELF64 {
            return Err(Refused::Parse);
        }
        Ok(f)
    }

    pub fn load(b: &[u8]) -> Result<Loaded, Refused> {
        let f = open(b)?;
        let h = &f.ehdr;
        policy::header(h.e_type, h.e_machine, h.e_phnum as usize)?;
        let mut l = policy::empty(h.e_entry);
        let phs = f.segments().ok_or(Refused::Parse)?;
        for ph in phs.iter() {
            if ph.p_type != PT_LOAD {
                continue;
            }
            let s = Seg {
                vaddr: ph.p_vaddr,
                paddr: ph.p_paddr,
                memsz: ph.p_memsz,
                flags: ph.p_flags,
                off: ph.p_offset as usize,
                filesz: ph.p_filesz as usize,
            };
            policy::push(&mut l, s, b.len())?;
        }
        policy::whole(&l)?;
        Ok(l)
    }

    pub fn note<'a>(b: &'a [u8], owner: &[u8], kind: u32) -> Result<Option<&'a [u8]>, Refused> {
        let f = open(b)?;
        if f.ehdr.e_phnum as usize > MAX_PHNUM {
            return Err(Refused::Policy("phnum"));
        }
        let mut found = None;
        let Some(phs) = f.segments() else { return Ok(None) };
        for ph in phs.iter() {
            if ph.p_type != PT_NOTE {
                continue;
            }
            // The iterator turns a malformed note into the end of the walk (`.ok()`), so a
            // truncated note is indistinguishable from the last one here.
            let it = f.segment_data_as_notes(&ph).map_err(|_| Refused::Parse)?;
            for n in it {
                // GnuAbiTag and GnuBuildId hide the name; ours is never "GNU".
                if let Note::Unknown(a) = n {
                    policy::note_match(a.name, a.n_type as u32, a.desc, owner, kind, &mut found)?;
                }
            }
        }
        Ok(found)
    }
}

#[cfg(feature = "object")]
pub mod obj {
    use super::*;
    use object::LittleEndian as LE;
    use object::elf::FileHeader64;
    use object::read::elf::{FileHeader, ProgramHeader};

    pub fn load(b: &[u8]) -> Result<Loaded, Refused> {
        let h = FileHeader64::<LE>::parse(b).map_err(|_| Refused::Parse)?;
        let e = h.endian().map_err(|_| Refused::Parse)?;
        policy::header(h.e_type.get(e).0, h.e_machine.get(e).0, h.e_phnum.get(e) as usize)?;
        let mut l = policy::empty(h.e_entry.get(e));
        for ph in h.program_headers(e, b).map_err(|_| Refused::Parse)? {
            if ph.p_type(e).0 != PT_LOAD {
                continue;
            }
            let s = Seg {
                vaddr: ph.p_vaddr(e),
                paddr: ph.p_paddr(e),
                memsz: ph.p_memsz(e),
                flags: ph.p_flags(e).0,
                off: ph.p_offset(e) as usize,
                filesz: ph.p_filesz(e) as usize,
            };
            policy::push(&mut l, s, b.len())?;
        }
        policy::whole(&l)?;
        Ok(l)
    }

    pub fn note<'a>(b: &'a [u8], owner: &[u8], kind: u32) -> Result<Option<&'a [u8]>, Refused> {
        let h = FileHeader64::<LE>::parse(b).map_err(|_| Refused::Parse)?;
        let e = h.endian().map_err(|_| Refused::Parse)?;
        if h.e_phnum.get(e) as usize > MAX_PHNUM {
            return Err(Refused::Policy("phnum"));
        }
        let mut found = None;
        for ph in h.program_headers(e, b).map_err(|_| Refused::Parse)? {
            let Some(mut it) = ph.notes(e, b).map_err(|_| Refused::Parse)? else { continue };
            while let Some(n) = it.next().map_err(|_| Refused::Parse)? {
                policy::note_match(n.name_bytes(), n.n_type(e).0, n.desc(), owner, kind, &mut found)?;
            }
        }
        Ok(found)
    }
}

#[cfg(all(feature = "goblin", feature = "alloc"))]
pub mod gob {
    use super::*;
    use goblin::container::{Container, Ctx};
    use goblin::elf::Elf;
    use goblin::elf::program_header::ProgramHeader;

    pub fn load(b: &[u8]) -> Result<Loaded, Refused> {
        let h = Elf::parse_header(b).map_err(|_| Refused::Parse)?;
        if h.e_ident[4] != 2 || h.e_ident[5] != 1 {
            return Err(Refused::Parse);
        }
        policy::header(h.e_type, h.e_machine, h.e_phnum as usize)?;
        let ctx = Ctx::new(Container::Big, goblin::container::Endian::Little);
        let phs = ProgramHeader::parse(b, h.e_phoff as usize, h.e_phnum as usize, ctx)
            .map_err(|_| Refused::Parse)?;
        let mut l = policy::empty(h.e_entry);
        for ph in &phs {
            if ph.p_type != PT_LOAD {
                continue;
            }
            let s = Seg {
                vaddr: ph.p_vaddr,
                paddr: ph.p_paddr,
                memsz: ph.p_memsz,
                flags: ph.p_flags,
                off: ph.p_offset as usize,
                filesz: ph.p_filesz as usize,
            };
            policy::push(&mut l, s, b.len())?;
        }
        policy::whole(&l)?;
        Ok(l)
    }

    pub fn note<'a>(b: &'a [u8], owner: &[u8], kind: u32) -> Result<Option<&'a [u8]>, Refused> {
        let h = Elf::parse_header(b).map_err(|_| Refused::Parse)?;
        if h.e_phnum as usize > MAX_PHNUM {
            return Err(Refused::Policy("phnum"));
        }
        let mut elf = Elf::lazy_parse(h).map_err(|_| Refused::Parse)?;
        let ctx = Ctx::new(Container::Big, goblin::container::Endian::Little);
        elf.program_headers = ProgramHeader::parse(b, h.e_phoff as usize, h.e_phnum as usize, ctx)
            .map_err(|_| Refused::Parse)?;
        let mut found = None;
        let Some(it) = elf.iter_note_headers(b) else { return Ok(None) };
        for n in it {
            let n = n.map_err(|_| Refused::Parse)?;
            // goblin hands back the name as a `&str` without its NUL, and never checks the byte it
            // skipped was a NUL; rebuild what our rule compares.
            let mut name = [0u8; 64];
            if n.name.len() + 1 > name.len() {
                continue;
            }
            name[..n.name.len()].copy_from_slice(n.name.as_bytes());
            policy::note_match(&name[..n.name.len() + 1], n.n_type, n.desc, owner, kind, &mut found)?;
        }
        Ok(found)
    }
}

#[cfg(feature = "goblin_noalloc")]
pub mod gob_noalloc {
    use super::*;
    /// Everything goblin offers for an ELF64 file with `alloc` off: the header as a plain struct,
    /// cast from bytes. Program headers have only an `unsafe fn from_raw_parts`.
    pub fn header(b: &[u8]) -> Result<(u16, u16, u64, u64, u16), Refused> {
        let a: &[u8; 64] = b.get(..64).ok_or(Refused::Parse)?.try_into().unwrap();
        let h = goblin::elf64::header::Header::from_bytes(a);
        Ok((h.e_type, h.e_machine, h.e_entry, h.e_phoff, h.e_phnum))
    }
}
SOURCE
    mkdir -p "$WORK/bench"
    cat > "$WORK/bench/Cargo.toml" <<TOML
[package]
name = "bench"
version = "0.0.0"
edition = "2024"
publish = false

[dependencies]
adapters = { path = "../adapters", features = ["ours", "cole", "object", "goblin", "goblin_noalloc"] }

[profile.release]
debug = false
TOML
    mkdir -p "$WORK/bench/src"
    cat > "$WORK/bench/src/main.rs" <<'SOURCE'
//! Agreement and parse time on real program images.
//! Usage: bench <dir>...  (each dir's files are patched to the host machine, aarch64, in memory, so
//! every image reaches the same paths past the one-u16 machine check.)
use adapters::*;
use std::time::Instant;

type Load = fn(&[u8]) -> Result<Loaded, Refused>;
type Note = for<'a> fn(&'a [u8], &[u8], u32) -> Result<Option<&'a [u8]>, Refused>;

const CANDS: [(&str, Load, Note); 4] = [
    ("ours", ours::load, ours::note),
    ("cole", cole::load, cole::note),
    ("object", obj::load, obj::note),
    ("goblin", gob::load, gob::note),
];

fn summary(r: &Result<Loaded, Refused>) -> String {
    match r {
        Ok(l) => format!("ok entry={:x} {:?}", l.entry, l.segments()),
        Err(_) => "refused".into(),
    }
}

fn main() {
    let mut files = vec![];
    for d in std::env::args().skip(1) {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            let mut b = std::fs::read(&p).unwrap();
            if b.len() > 20 {
                b[18..20].copy_from_slice(&EM_AARCH64.to_le_bytes());
            }
            files.push((p, b));
        }
    }
    files.sort();
    let mut disagree = 0;
    let mut notes_found = [0usize; 4];
    let mut accepted = [0usize; 4];
    for (p, b) in &files {
        let base = summary(&ours::load(b));
        let base_note = ours::note(b, b"nife", 1);
        for (i, (name, load, note)) in CANDS.iter().enumerate() {
            let s = summary(&load(b));
            if s.starts_with("ok") {
                accepted[i] += 1;
            }
            let n = note(b, b"nife", 1);
            if matches!(n, Ok(Some(_))) {
                notes_found[i] += 1;
            }
            if s != base || n.map(|x| x.map(|d| d.as_ptr())) != base_note.map(|x| x.map(|d| d.as_ptr())) {
                disagree += 1;
                eprintln!("DISAGREE {name} {}: {s} vs {base}", p.display());
            }
        }
    }
    println!("images: {}", files.len());
    for (i, (name, _, _)) in CANDS.iter().enumerate() {
        println!("{name}: accepted {} manifest note found {}", accepted[i], notes_found[i]);
    }
    println!("disagreements: {disagree}");

    // Misaligned: the same bytes one byte off an 8-byte boundary, as a member inside an archive is.
    for (name, load, note) in CANDS.iter() {
        let mut ok = 0;
        for (_, b) in &files {
            let mut v = vec![0u8; b.len() + 8];
            let base = v.as_ptr() as usize;
            let skew = (9 - base % 8) % 8; // start at an address that is 1 mod 8
            let skew = if (base + skew) % 8 == 1 { skew } else { (base + skew + 1) % 8 };
            v[skew..skew + b.len()].copy_from_slice(b);
            let s = &v[skew..skew + b.len()];
            assert_eq!(s.as_ptr() as usize % 8 != 0, true);
            if load(s).is_ok() && note(s, b"nife", 1).is_ok() {
                ok += 1;
            }
        }
        println!("misaligned {name}: {ok}/{} load and note as when aligned-accepted", files.len());
    }

    // Parse time: load + manifest-note lookup, per image, best of 7 rounds of 200.
    let rounds = 7;
    let reps = 200;
    for (name, load, note) in CANDS.iter() {
        let mut best = vec![u128::MAX; files.len()];
        for _ in 0..rounds {
            for (k, (_, b)) in files.iter().enumerate() {
                let t = Instant::now();
                for _ in 0..reps {
                    let _ = std::hint::black_box(load(std::hint::black_box(b)).is_ok());
                    let _ = std::hint::black_box(note(std::hint::black_box(b), b"nife", 1).is_ok());
                }
                best[k] = best[k].min(t.elapsed().as_nanos() / reps as u128);
            }
        }
        let mut sorted = best.clone();
        sorted.sort();
        let total: u128 = best.iter().sum();
        println!(
            "time {name}: median {} ns, p90 {} ns, max {} ns, all {} images {} us",
            sorted[sorted.len() / 2],
            sorted[sorted.len() * 9 / 10],
            sorted[sorted.len() - 1],
            files.len(),
            total / 1000
        );
    }
}
SOURCE
    mkdir -p "$WORK/fit"
    cat > "$WORK/fit/Cargo.toml" <<TOML
[package]
name = "fit"
version = "0.0.0"
edition = "2024"
publish = false

[features]
ours = ["adapters/ours"]
cole = ["adapters/cole"]
object = ["adapters/object"]
goblin = ["adapters/goblin"]
goblin_noalloc = ["adapters/goblin_noalloc"]
heap = []

[dependencies]
adapters = { path = "../adapters" }

[profile.release]
panic = "abort"
overflow-checks = true
[profile.dev]
panic = "abort"
TOML
    mkdir -p "$WORK/fit/src"
    cat > "$WORK/fit/src/main.rs" <<'SOURCE'
//! The loader's calls (parse, walk the segments, find the manifest note) in a freestanding binary
//! with no global allocator unless `heap` is on, so the link answers "does this need an allocator?"
#![no_std]
#![no_main]
use adapters::*;

#[cfg(feature = "ours")]
use adapters::ours as c;
#[cfg(feature = "cole")]
use adapters::cole as c;
#[cfg(feature = "object")]
use adapters::obj as c;
#[cfg(feature = "goblin")]
use adapters::gob as c;

#[cfg(feature = "heap")]
mod heap {
    // A refusing allocator: enough to link, and it says nothing about what a real heap costs.
    struct Never;
    unsafe impl core::alloc::GlobalAlloc for Never {
        unsafe fn alloc(&self, _: core::alloc::Layout) -> *mut u8 { core::ptr::null_mut() }
        unsafe fn dealloc(&self, _: *mut u8, _: core::alloc::Layout) {}
    }
    #[global_allocator]
    static A: Never = Never;
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[unsafe(no_mangle)]
pub extern "C" fn _start(p: *const u8, n: usize) -> u64 {
    let b = unsafe { core::slice::from_raw_parts(core::ptr::read_volatile(&p), n) };
    #[cfg(feature = "goblin_noalloc")]
    {
        return match gob_noalloc::header(b) { Ok(h) => h.2, Err(_) => 0 };
    }
    #[cfg(not(feature = "goblin_noalloc"))]
    {
        let mut sum = 0u64;
        if let Ok(l) = c::load(b) {
            sum = l.entry;
            for s in l.segments() {
                sum ^= s.vaddr ^ s.memsz ^ s.flags as u64;
            }
        }
        if let Ok(Some(d)) = c::note(b, b"nife", 1) {
            sum ^= d.len() as u64;
        }
        sum
    }
}
SOURCE
    mkdir -p "$WORK/probe"
    cat > "$WORK/probe/Cargo.toml" <<TOML
[package]
name = "probe"
version = "0.0.0"
edition = "2024"
[dependencies]
adapters = { path = "../adapters", features = ["ours", "cole", "object", "goblin", "goblin_noalloc"] }
TOML
    mkdir -p "$WORK/probe/src"
    cat > "$WORK/probe/src/main.rs" <<'SOURCE'
use adapters::*;
fn main() {
    if std::env::var("MIS").is_ok() { misaligned_goblin_header(); return; }
    for p in std::env::args().skip(1) {
        let mut b = std::fs::read(&p).unwrap();
        b[18..20].copy_from_slice(&EM_AARCH64.to_le_bytes());
        println!("{p}");
        println!("  ours   {:?}", ours::note(&b, b"nife", 1));
        println!("  cole   {:?}", cole::note(&b, b"nife", 1));
        println!("  object {:?}", obj::note(&b, b"nife", 1));
        println!("  goblin {:?}", gob::note(&b, b"nife", 1));
    }
}

#[allow(dead_code)]
pub fn misaligned_goblin_header() {
    let b = std::fs::read("../seeds/minimal_rx_aarch64.elf").unwrap();
    let mut v = vec![0u8; b.len() + 8];
    let skew = (0..8).find(|k| (v.as_ptr() as usize + k) % 8 == 1).unwrap();
    v[skew..skew + b.len()].copy_from_slice(&b);
    println!("aligned: {:?}", gob_noalloc::header(&b).map(|h| h.2));
    println!("misaligned:");
    println!("{:?}", gob_noalloc::header(&v[skew..skew + b.len()]).map(|h| h.2));
}
SOURCE
    mkdir -p "$WORK/sizes"
    cat > "$WORK/sizes/Cargo.toml" <<TOML
[package]
name = "sizes"
version = "0.0.0"
edition = "2024"
[dependencies]
elf_ours = { package = "elf", path = "$ROOT/crates/elf" }
elf = { version = "=0.8.0", default-features = false }
object = { version = "=0.40.0", default-features = false, features = ["read_core", "elf"] }
goblin = { version = "=0.10.7", default-features = false, features = ["elf32", "elf64", "endian_fd"] }
[workspace]
TOML
    mkdir -p "$WORK/sizes/src"
    cat > "$WORK/sizes/src/main.rs" <<'SOURCE'
use core::mem::size_of;
fn main() {
    println!("ours Elf: {}", size_of::<Option<elf_ours::Elf<'static>>>());
    println!("cole ElfBytes<LE>: {}", size_of::<Option<elf::ElfBytes<'static, elf::endian::LittleEndian>>>());
    println!("object ElfFile64<LE,&[u8]>: {}", size_of::<Option<object::read::elf::ElfFile64<'static, object::LittleEndian, &'static [u8]>>>());
    println!("object (&FileHeader64, &[ProgramHeader64]): {}", size_of::<Option<(&'static object::elf::FileHeader64<object::LittleEndian>, &'static [object::elf::ProgramHeader64<object::LittleEndian>], &'static [u8])>>());
    println!("goblin Elf: {} (+ heap)", size_of::<Option<goblin::elf::Elf<'static>>>());
    println!("goblin (Header, Vec<ProgramHeader>): {} (+ heap)", size_of::<Option<(goblin::elf::Header, Vec<goblin::elf::ProgramHeader>, &'static [u8])>>());
}
SOURCE
    mkdir -p "$WORK/fz"
    cat > "$WORK/fz/Cargo.toml" <<TOML
[package]
name="fzroot"
version="0.0.0"
edition="2024"
[workspace]
members=[]
exclude=["fuzz"]
TOML
    mkdir -p "$WORK/fz/src"
    cat > "$WORK/fz/src/lib.rs" <<'SOURCE'
SOURCE
    mkdir -p "$WORK/fz/fuzz"
    cat > "$WORK/fz/fuzz/Cargo.toml" <<TOML
[package]
name = "elfm-fuzz"
version = "0.0.0"
edition = "2024"
publish = false

[package.metadata]
cargo-fuzz = true

[dependencies]
libfuzzer-sys = "0.4"
adapters = { path = "../../adapters", features = ["ours", "cole", "object", "goblin", "goblin_noalloc"] }
elf_ours = { package = "elf", path = "$ROOT/crates/elf" }
elf = { version = "=0.8.0", features = ["std"] }
object = { version = "=0.40.0", default-features = false, features = ["read", "std"] }
goblin = { version = "=0.10.7" }

[[bin]]
name = "ours"
path = "fuzz_targets/ours.rs"
test = false
doc = false
bench = false

[[bin]]
name = "cole"
path = "fuzz_targets/cole.rs"
test = false
doc = false
bench = false

[[bin]]
name = "object"
path = "fuzz_targets/object.rs"
test = false
doc = false
bench = false

[[bin]]
name = "goblin"
path = "fuzz_targets/goblin.rs"
test = false
doc = false
bench = false

[profile.release]
debug = 1

[workspace]
TOML
    mkdir -p "$WORK/fz/fuzz/fuzz_targets"
    cat > "$WORK/fz/fuzz/fuzz_targets/ours.rs" <<'SOURCE'
//! Fuzz the ELF loader's front half with arbitrary bytes.
//!
//! **Why this target exists.** `crates/elf` is the only parser in the tree that *loads what it
//! parses*. Everything else answers a question about some bytes; this one hands the kernel a list of
//! (vaddr, memsz, flags, data) tuples that become page-table entries in a fresh address space. It is
//! also the parse with the widest supply of hostile inputs, because "run this binary" is a thing a
//! user asks for by name.
//!
//! **What it adds over the Kani proofs.** The crate's own doc comment says it plainly: the harnesses
//! prove `check_segment_bounds`, the leaf arithmetic, total and in-bounds for every field
//! combination, and the whole-parse totality proof **did not return**. An O(n^2) overlap loop over
//! up to 64 program headers, unrolled 64 deep against symbolic slice offsets, is past what the
//! solver can do at a useful input size, and that is written up in notes/verification.md rather than
//! hidden. So the crate has a proved leaf and an unproved shell, and the shell is what this reaches:
//! the header-count and table-size arithmetic, the entry-point check, the overlap loop, and the
//! `segments()` iterator a caller drives after `parse` returns.
//!
//! **The machine check is a compile-time constant** (`EXPECTED_MACHINE`), so a host fuzz build
//! accepts ELFs for the host's own machine, which is what that machine's kernel accepts. The builds
//! for the other two differ in exactly that one `u16`, so the paths past it are the same paths.
//! `fuzz/seeds/elf_parse/` therefore holds one seed per machine and this build gets past the check
//! on exactly one of them; see `crates/elf/tests/fuzz_seed.rs`, which fails if that count is ever
//! zero.

#![no_main]

use elf_ours::Elf;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // The shell's streaming note reader runs on bytes nobody has parsed as a program yet, so it is
    // fuzzed before `parse` gets a chance to refuse them (milestone 597 (a program carries its
    // manifest in an ELF note), provisional).
    if let Ok(segs) = elf_ours::NoteSegments::from_head(data) {
        let mut search = elf_ours::NoteSearch::new(b"nife", 1);
        for seg in segs {
            let Ok(r) = seg.range(data.len()) else { break };
            if search.segment(&data[r], seg.p_align).is_err() {
                break;
            }
        }
    }
    let Ok(elf) = Elf::parse(data) else {
        return;
    };

    // Everything the kernel's loader does with a parsed ELF, in the order it does it. Validation
    // already ran inside `parse`; this is the walk that happens afterwards, when the loader is
    // mapping and is past the point where refusing is easy.
    let _ = elf.entry();

    // The note lookup the progenitor makes for a program's manifest (milestone 597, provisional):
    // every `PT_NOTE` walked in full, on the same hostile bytes the loader accepted.
    let _ = elf.note(b"nife", 1);
    for seg in elf.segments() {
        let _ = seg.is_readable();
        let _ = seg.is_writable();
        let _ = seg.is_executable();
        // Both page sizes the tree can boot with. 4 KiB is what both ISAs use today; 16 KiB is on
        // the aarch64 roadmap and the arithmetic differs (`div_ceil` then `saturating_mul`), so
        // fuzzing only the one we ship would prove the less interesting half.
        let _ = seg.page_range(4096);
        let _ = seg.page_range(16384);
        // The tail the loader must zero: `memsz` beyond `data.len()` is `.bss`. Recomputed here
        // because a segment whose `memsz` is under its `filesz` would make this wrap, and the check
        // that forbids it is in `check_segment_bounds` rather than in this type.
        let _ = seg.memsz - seg.data.len() as u64;
    }
});
SOURCE
    mkdir -p "$WORK/fz/fuzz/fuzz_targets"
    cat > "$WORK/fz/fuzz/fuzz_targets/cole.rs" <<'SOURCE'
#![no_main]
//! cole14 `elf` 0.8.0: the loader path through our policy, then the crate's own surface a caller
//! of a whole-file reader would drive (sections, string tables, symbols, notes, dynamic).
use elf::{ElfBytes, endian::AnyEndian};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = adapters::cole::load(data);
    let _ = adapters::cole::note(data, b"nife", 1);

    let Ok(f) = ElfBytes::<AnyEndian>::minimal_parse(data) else { return };
    if let Some(phs) = f.segments() {
        for ph in phs.iter() {
            let _ = f.segment_data(&ph);
            if let Ok(notes) = f.segment_data_as_notes(&ph) {
                for n in notes {
                    let _ = core::hint::black_box(n);
                }
            }
        }
    }
    if let Ok((shdrs, strtab)) = f.section_headers_with_strtab() {
        if let (Some(shdrs), Some(strtab)) = (shdrs, strtab) {
            for sh in shdrs.iter() {
                let _ = strtab.get(sh.sh_name as usize);
                let _ = f.section_data(&sh);
                if let Ok(notes) = f.section_data_as_notes(&sh) {
                    for n in notes {
                        let _ = core::hint::black_box(n);
                    }
                }
            }
        }
    }
    if let Ok(common) = f.find_common_data() {
        if let Some(syms) = common.symtab {
            for s in syms.iter() {
                if let Some(st) = &common.symtab_strs {
                    let _ = st.get(s.st_name as usize);
                }
            }
        }
        if let Some(d) = common.dynamic {
            for e in d.iter() {
                let _ = core::hint::black_box(e);
            }
        }
    }
    let _ = f.symbol_version_table();
});
SOURCE
    mkdir -p "$WORK/fz/fuzz/fuzz_targets"
    cat > "$WORK/fz/fuzz/fuzz_targets/object.rs" <<'SOURCE'
#![no_main]
//! `object` 0.40.0's reader: the loader path through our policy, then `ElfFile64` (the unified
//! `Object` API) and the ELF-specific walks.
use libfuzzer_sys::fuzz_target;
use object::read::elf::{ElfFile64, FileHeader, ProgramHeader, SectionHeader};
use object::{Object, ObjectSection, ObjectSegment, ObjectSymbol};

fuzz_target!(|data: &[u8]| {
    let _ = adapters::obj::load(data);
    let _ = adapters::obj::note(data, b"nife", 1);

    let Ok(f) = ElfFile64::<object::Endianness>::parse(data) else { return };
    let e = f.endian();
    for seg in f.segments() {
        let _ = seg.data();
        let _ = seg.name();
    }
    for ph in f.elf_program_headers() {
        if let Ok(Some(mut notes)) = ph.notes(e, data) {
            while let Ok(Some(n)) = notes.next() {
                let _ = core::hint::black_box(n);
            }
        }
    }
    for s in f.sections() {
        let _ = s.name();
        let _ = s.data();
        let _ = s.uncompressed_data();
    }
    for sh in f.elf_section_table().iter() {
        if let Ok(Some(mut notes)) = sh.notes(e, data) {
            while let Ok(Some(n)) = notes.next() {
                let _ = core::hint::black_box(n);
            }
        }
    }
    for s in f.symbols() {
        let _ = s.name();
    }
    for s in f.dynamic_symbols() {
        let _ = s.name();
    }
    let _ = f.imports();
    let _ = f.exports();
    let _ = f.build_id();
    let _ = f.entry();
    let _ = f.elf_header().e_entry(e);
});
SOURCE
    mkdir -p "$WORK/fz/fuzz/fuzz_targets"
    cat > "$WORK/fz/fuzz/fuzz_targets/goblin.rs" <<'SOURCE'
#![no_main]
//! `goblin` 0.10.7: the loader path through our policy, then `Elf::parse` (goblin's one entry
//! point for a whole ELF file) and its note walk; and the no-`alloc` header cast.
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = adapters::gob::load(data);
    let _ = adapters::gob::note(data, b"nife", 1);

    if let Ok(elf) = goblin::elf::Elf::parse(data) {
        if let Some(notes) = elf.iter_note_headers(data) {
            for n in notes {
                let _ = core::hint::black_box(n);
            }
        }
        if let Some(notes) = elf.iter_note_sections(data, None) {
            for n in notes {
                let _ = core::hint::black_box(n);
            }
        }
        for s in elf.syms.iter() {
            let _ = elf.strtab.get_at(s.st_name);
        }
        for s in elf.dynsyms.iter() {
            let _ = elf.dynstrtab.get_at(s.st_name);
        }
    }
});
SOURCE
    mkdir -p "$WORK/kh"
    cat > "$WORK/kh/Cargo.toml" <<TOML
[package]
name = "kh"
version = "0.0.0"
edition = "2024"
publish = false

[dependencies]
elf = { version = "=0.8.0", default-features = false }
object = { version = "=0.40.0", default-features = false, features = ["read_core", "elf"] }
goblin = { version = "=0.10.7", default-features = false, features = ["elf32", "elf64", "endian_fd"] }

[workspace]
TOML
    mkdir -p "$WORK/kh/src"
    cat > "$WORK/kh/src/lib.rs" <<'SOURCE'
//! Our eight `crates/elf` properties, asked of each candidate's nearest function.
//!
//! P1-P3: one program header's file range. P5-P6/P8: a note walk. P7: the header and program
//! header table read from a hostile head. P4 (`page_range`) has no counterpart in any candidate.
#![no_std]
extern crate alloc;

#[cfg(kani)]
mod cole {
    use elf::{ElfBytes, endian::LittleEndian, file::Class, note::{Note, NoteIterator}};

    fn inside(outer: &[u8], inner: &[u8]) -> bool {
        let o = outer.as_ptr() as usize;
        let i = inner.as_ptr() as usize;
        i >= o && i + inner.len() <= o + outer.len()
    }

    /// P1+P2: `segment_data` (the only public road to `get_file_data_range`) never panics and
    /// an `Ok` slice lies inside the file. The header is fixed and the one program header is
    /// symbolic, so this is the leaf property and not a whole-parse one.
    #[kani::proof]
    #[kani::unwind(9)]
    fn p1_p2_segment_data_is_in_bounds() {
        const LEN: usize = 64 + 56;
        let mut b: [u8; LEN] = kani::any();
        b[..16].copy_from_slice(&[0x7f, b'E', b'L', b'F', 2, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        b[32..40].copy_from_slice(&64u64.to_le_bytes()); // e_phoff
        b[54..56].copy_from_slice(&56u16.to_le_bytes()); // e_phentsize
        b[56..58].copy_from_slice(&1u16.to_le_bytes()); // e_phnum
        b[58..60].copy_from_slice(&0u16.to_le_bytes()); // e_shentsize
        b[60..62].copy_from_slice(&0u16.to_le_bytes()); // e_shnum
        let cut: usize = kani::any();
        kani::assume(cut <= LEN);
        let Ok(f) = ElfBytes::<LittleEndian>::minimal_parse(&b[..cut]) else { return };
        let Some(phs) = f.segments() else { return };
        for ph in phs.iter() {
            if let Ok(d) = f.segment_data(&ph) {
                assert!(inside(&b[..cut], d));
            }
        }
    }

    /// P7: the header and program header table from a hostile 176-byte head, walked. The reader
    /// also takes ELF32, whose 32-byte headers fit five times in 176 bytes; the bound covers that.
    #[kani::proof]
    #[kani::unwind(9)]
    fn p7_head_never_panics() {
        const LEN: usize = 64 + 2 * 56;
        let b: [u8; LEN] = kani::any();
        let cut: usize = kani::any();
        kani::assume(cut <= LEN);
        if let Ok(f) = ElfBytes::<LittleEndian>::minimal_parse(&b[..cut]) {
            if let Some(phs) = f.segments() {
                for ph in phs.iter() {
                    let _ = f.segment_data(&ph);
                }
            }
        }
    }

    /// P5: one note's arithmetic never panics, for any sizes and either padding.
    #[kani::proof]
    #[kani::unwind(3)]
    fn p5_one_note_never_panics() {
        const LEN: usize = 24;
        let b: [u8; LEN] = kani::any();
        let align: usize = if kani::any() { 4 } else { 8 };
        let mut it = NoteIterator::new(LittleEndian, Class::ELF64, align, &b);
        let _ = it.next();
    }

    /// P6+P8: everything a walk of a 40-byte segment hands back lies inside it, and the walk
    /// ends within the three notes 40 bytes can hold.
    #[kani::proof]
    #[kani::unwind(5)]
    fn p6_p8_walk_is_inside_and_ends() {
        const LEN: usize = 40;
        let b: [u8; LEN] = kani::any();
        let align: usize = if kani::any() { 4 } else { 8 };
        for n in NoteIterator::new(LittleEndian, Class::ELF64, align, &b) {
            if let Note::Unknown(a) = n {
                assert!(inside(&b, a.name));
                assert!(inside(&b, a.desc));
            }
        }
    }
}

#[cfg(kani)]
mod obj {
    use object::LittleEndian as LE;
    use object::elf::{FileHeader64, ProgramHeader64};
    use object::read::elf::{FileHeader, NoteIterator, ProgramHeader};

    #[repr(C, align(8))]
    struct Aligned<const N: usize>([u8; N]);

    fn inside(outer: &[u8], inner: &[u8]) -> bool {
        let o = outer.as_ptr() as usize;
        let i = inner.as_ptr() as usize;
        i >= o && i + inner.len() <= o + outer.len()
    }

    /// P1+P2: `ProgramHeader::data` never panics, and an `Ok` slice lies inside the file, for any
    /// field values and file length.
    #[kani::proof]
    fn p1_p2_segment_data_is_in_bounds() {
        let raw = Aligned::<56>(kani::any());
        let (ph, _) = object::pod::from_bytes::<ProgramHeader64<LE>>(&raw.0).unwrap();
        let file = [0u8; 32];
        let cut: usize = kani::any();
        kani::assume(cut <= file.len());
        if let Ok(d) = ph.data(LE, &file[..cut]) {
            assert!(inside(&file[..cut], d));
        }
    }

    /// P2 for a segment with bytes: `read_bytes_at` answers `Ok(&[])` for size 0 at any offset,
    /// so P2 above fails on empty segments only. This is the same property with `p_filesz > 0`.
    #[kani::proof]
    fn p2_nonempty_segment_data_is_in_bounds() {
        let raw = Aligned::<56>(kani::any());
        let (ph, _) = object::pod::from_bytes::<ProgramHeader64<LE>>(&raw.0).unwrap();
        kani::assume(ph.p_filesz(LE) > 0);
        let file = [0u8; 32];
        let cut: usize = kani::any();
        kani::assume(cut <= file.len());
        if let Ok(d) = ph.data(LE, &file[..cut]) {
            assert!(inside(&file[..cut], d));
        }
    }

    /// P7: header and program header table from a hostile 176-byte head, walked.
    #[kani::proof]
    #[kani::unwind(5)]
    fn p7_head_never_panics() {
        let b = Aligned::<{ 64 + 2 * 56 }>(kani::any());
        let cut: usize = kani::any();
        kani::assume(cut <= b.0.len());
        let data = &b.0[..cut];
        if let Ok(h) = FileHeader64::<LE>::parse(data) {
            if let Ok(e) = h.endian() {
                if let Ok(phs) = h.program_headers(e, data) {
                    for ph in phs {
                        let _ = ph.data(e, data);
                    }
                }
            }
        }
    }

    /// P5: one note's arithmetic never panics, for any sizes and either padding.
    #[kani::proof]
    #[kani::unwind(3)]
    fn p5_one_note_never_panics() {
        let b = Aligned::<24>(kani::any());
        let align: u64 = if kani::any() { 4 } else { 8 };
        let Ok(mut it) = NoteIterator::<FileHeader64<LE>>::new(LE, align, &b.0) else { return };
        let _ = it.next();
    }

    /// P6+P8: everything a walk of a 40-byte segment hands back lies inside it, and the walk ends.
    #[kani::proof]
    #[kani::unwind(5)]
    fn p6_p8_walk_is_inside_and_ends() {
        let b = Aligned::<40>(kani::any());
        let align: u64 = if kani::any() { 4 } else { 8 };
        let Ok(mut it) = NoteIterator::<FileHeader64<LE>>::new(LE, align, &b.0) else { return };
        while let Ok(Some(n)) = it.next() {
            assert!(inside(&b.0, n.name_bytes()));
            assert!(inside(&b.0, n.desc()));
        }
    }
}

#[cfg(kani)]
mod gob {
    use goblin::container::{Container, Ctx, Endian};
    use goblin::elf::Elf;
    use goblin::elf::note::NoteDataIterator;
    use goblin::elf::program_header::ProgramHeader;

    fn inside(outer: &[u8], inner: &[u8]) -> bool {
        let o = outer.as_ptr() as usize;
        let i = inner.as_ptr() as usize;
        i >= o && i + inner.len() <= o + outer.len()
    }

    fn any_ph() -> ProgramHeader {
        ProgramHeader {
            p_type: kani::any(),
            p_flags: kani::any(),
            p_offset: kani::any(),
            p_vaddr: kani::any(),
            p_paddr: kani::any(),
            p_filesz: kani::any(),
            p_memsz: kani::any(),
            p_align: kani::any(),
        }
    }

    /// P1: `file_range` and `vm_range` never panic (they saturate).
    #[kani::proof]
    fn p1_ranges_never_panic() {
        let ph = any_ph();
        let _ = ph.file_range();
        let _ = ph.vm_range();
    }

    /// P2: goblin has no checked accessor for a segment's bytes; the caller slices the file with
    /// `file_range()`. This asks whether that range is in bounds for a file `file_len` long.
    #[kani::proof]
    fn p2_file_range_is_in_bounds() {
        let ph = any_ph();
        let file_len: usize = kani::any();
        let r = ph.file_range();
        assert!(r.start <= r.end && r.end <= file_len);
    }

    /// P3: does `vm_range` mean the address range did not wrap?
    #[kani::proof]
    fn p3_vm_range_has_no_address_overflow() {
        let ph = any_ph();
        let r = ph.vm_range();
        assert!((ph.p_vaddr as usize).checked_add(ph.p_memsz as usize) == Some(r.end));
    }

    /// P7: header and program header table from a hostile 176-byte head.
    #[kani::proof]
    #[kani::unwind(5)]
    fn p7_head_never_panics() {
        const LEN: usize = 64 + 2 * 56;
        let b: [u8; LEN] = kani::any();
        let cut: usize = kani::any();
        kani::assume(cut <= LEN);
        let data = &b[..cut];
        if let Ok(h) = Elf::parse_header(data) {
            kani::assume(h.e_phnum <= 2);
            let ctx = Ctx::new(Container::Big, Endian::Little);
            let _ = ProgramHeader::parse(data, h.e_phoff as usize, h.e_phnum as usize, ctx);
        }
    }

    fn walk(b: &[u8], size: usize, align: usize) -> NoteDataIterator<'_> {
        NoteDataIterator { data: b, size, offset: 0, ctx: (align, Ctx::new(Container::Big, Endian::Little)) }
    }

    /// P5: one note's arithmetic never panics.
    #[kani::proof]
    #[kani::unwind(10)]
    fn p5_one_note_never_panics() {
        const LEN: usize = 24;
        let b: [u8; LEN] = kani::any();
        let align: usize = if kani::any() { 4 } else { 8 };
        let _ = walk(&b, LEN, align).next();
    }

    /// P6+P8: everything a walk of a 40-byte segment hands back lies inside the segment. The
    /// segment is the first `seg` bytes of the 40-byte file, as `iter_note_headers` builds it.
    #[kani::proof]
    #[kani::unwind(10)]
    fn p6_p8_walk_is_inside_and_ends() {
        const LEN: usize = 40;
        let b: [u8; LEN] = kani::any();
        let seg: usize = kani::any();
        kani::assume(seg <= LEN);
        let align: usize = if kani::any() { 4 } else { 8 };
        for n in walk(&b, seg, align) {
            if let Ok(n) = n {
                assert!(inside(&b[..seg], n.desc));
            }
        }
    }
}
SOURCE
    mkdir -p "$WORK/."
    cat > "$WORK/mkseed.py" <<'SOURCE'
import struct, sys, os
# Minimal static ELF64 images with one PT_LOAD and one PT_NOTE holding a "nife" note (and a GNU
# build-id note), one per machine and padding, as seeds for the note walks.
out = sys.argv[1]
def image(machine, align, notes):
    ehdr, phdr = 64, 56
    nph = 2
    note_off = ehdr + nph * phdr
    pad = lambda n: (n + align - 1) // align * align
    body = b''
    for owner, kind, desc in notes:
        name = owner + b'\0'
        n = struct.pack('<III', len(name), len(desc), kind) + name
        n += b'\0' * (pad(len(n)) - len(n))
        n += desc
        n += b'\0' * (pad(len(n)) - len(n))
        body += n
    total = note_off + len(body)
    e = bytearray(b'\x7fELF' + bytes([2, 1, 1]) + b'\0' * 9)
    e += struct.pack('<HHIQQQIHHHHHH', 2, machine, 1, 0x400000, ehdr, 0, 0, ehdr, phdr, nph, 64, 0, 0)
    e += struct.pack('<IIQQQQQQ', 1, 5, 0, 0x400000, 0x400000, total, total, 0x1000)
    e += struct.pack('<IIQQQQQQ', 4, 4, note_off, 0x400000 + note_off, 0x400000 + note_off, len(body), len(body), align)
    return bytes(e) + body
for m, n in ((183, 'aarch64'), (243, 'riscv64'), (62, 'x86_64')):
    for a in (4, 8):
        b = image(m, a, [(b'GNU', 3, b'\x11' * 20), (b'nife', 1, b'manifest-bytes!')])
        open(os.path.join(out, f'note_{n}_a{a}.elf'), 'wb').write(b)
SOURCE
    mkdir -p "$WORK/."
    cat > "$WORK/mkedge.py" <<'SOURCE'
import struct, sys, os
out = sys.argv[1]
def note(owner_bytes, kind, desc, align, namesz=None):
    pad = lambda n: (n + align - 1) // align * align
    n = struct.pack('<III', len(owner_bytes) if namesz is None else namesz, len(desc), kind) + owner_bytes
    n += b'\0' * (pad(len(n)) - len(n))
    n += desc
    n += b'\0' * (pad(len(n)) - len(n))
    return n
def image(body, p_align, filesz=None, tail=b''):
    ehdr, phdr, nph = 64, 56, 2
    off = ehdr + nph * phdr
    total = off + len(body) + len(tail)
    e = bytearray(b'\x7fELF' + bytes([2, 1, 1]) + b'\0' * 9)
    e += struct.pack('<HHIQQQIHHHHHH', 2, 183, 1, 0x400000, ehdr, 0, 0, ehdr, phdr, nph, 64, 0, 0)
    e += struct.pack('<IIQQQQQQ', 1, 5, 0, 0x400000, 0x400000, total, total, 0x1000)
    fs = len(body) if filesz is None else filesz
    e += struct.pack('<IIQQQQQQ', 4, 4, off, 0x400000 + off, 0x400000 + off, fs, fs, p_align)
    return bytes(e) + body + tail
cases = {
 # the owner's last byte is not a NUL: "nifeX", namesz 5
 'no_nul': image(note(b'nifeX', 1, b'D' * 8, 4), 4),
 # the last note's padding runs past the segment: desc 5 bytes, segment cut before padding
 'pad_past_end': image(note(b'nife\0', 1, b'D' * 5, 4)[:-3], 4),
 # p_align 16
 'align16': image(note(b'nife\0', 1, b'D' * 8, 4), 16),
 # p_align 1: binutils and LLVM read 4-byte padding
 'align1': image(note(b'nife\0', 1, b'D' * 8, 4), 1),
 # the note runs past p_filesz but the bytes are in the file
 'past_filesz': image(note(b'nife\0', 1, b'D' * 16, 4)[:20], 4, tail=note(b'nife\0', 1, b'D' * 16, 4)[20:]),
 # duplicate in one segment
 'duplicate': image(note(b'nife\0', 1, b'A' * 8, 4) + note(b'nife\0', 1, b'B' * 8, 4), 4),
 # a malformed note after the match
 'trailing_garbage': image(note(b'nife\0', 1, b'A' * 8, 4) + b'\xff' * 12, 4),
}
for k, v in cases.items():
    open(os.path.join(out, k + '.elf'), 'wb').write(v)
SOURCE
}

write
case "$MODE" in
write) ;;
agree)
    (cd "$WORK/bench" && CARGO_TARGET_DIR=../target cargo "+$PIN" run -q --release -- "$@")
    (cd "$WORK/sizes" && CARGO_TARGET_DIR=../target cargo "+$PIN" run -q)
    ;;
edge)
    mkdir -p "$WORK/seeds" "$WORK/edge"
    cp "$ROOT"/fuzz/seeds/elf_parse/* "$WORK/seeds/"
    python3 -I "$WORK/mkseed.py" "$WORK/seeds"
    python3 -I "$WORK/mkedge.py" "$WORK/edge"
    (cd "$WORK/probe" && CARGO_TARGET_DIR=../target cargo "+$PIN" run -q -- ../edge/*.elf)
    (cd "$WORK/probe" && MIS=1 CARGO_TARGET_DIR=../target cargo "+$PIN" run -q) || true
    ;;
fit)
    BIN=$(rustc "+$PIN" --print sysroot)/lib/rustlib/$(rustc "+$PIN" -vV | sed -n 's/^host: //p')/bin
    for t in aarch64-unknown-none-softfloat riscv64imac-unknown-none-elf x86_64-unknown-none; do
        for f in ours cole object goblin "object heap" "goblin heap" goblin_noalloc; do
            if (cd "$WORK/fit" && CARGO_TARGET_DIR=../target-fit cargo "+$PIN" build -q --release \
                --target "$t" --features "$f" 2>/dev/null); then
                echo "$t [$f] links; text, rodata: $("$BIN/llvm-size" -A "$WORK/target-fit/$t/release/fit" \
                    | awk '$1==".text"{t=$2} $1==".rodata"{r=$2} END{print t+0, r+0}')"
            else
                echo "$t [$f] does not link"
            fi
        done
    done
    ;;
fuzz)
    SECS=${1:-1200}
    "$0" edge >/dev/null
    cd "$WORK/fz"
    cargo "+$PIN" fuzz build -O -a
    for t in ours cole object goblin; do
        mkdir -p "corpus/$t" "artifacts/$t"
        # fork mode with crashes, timeouts and OOMs ignored, so a run counts failures instead of
        # stopping at the first; each one lands in artifacts/<target>/.
        "./fuzz/target/$(rustc "+$PIN" -vV | sed -n 's/^host: //p')/release/$t" "corpus/$t" "$WORK/seeds" \
            -dict="$ROOT/fuzz/dictionaries/elf_parse.dict" -artifact_prefix="artifacts/$t/" \
            -max_total_time="$SECS" -timeout=10 -rss_limit_mb=2048 -malloc_limit_mb=512 -max_len=16384 \
            -fork=1 -ignore_crashes=1 -ignore_timeouts=1 -ignore_ooms=1 -print_final_stats=1 \
            > "$t.log" 2>&1 || true
        echo "$t: $(find "artifacts/$t" -type f | wc -l | tr -d ' ') artifacts; $(grep -o 'cov: [0-9]*' "$t.log" | tail -1)"
    done
    ;;
kani)
    (cd "$ROOT/crates/elf" && cargo kani)
    (cd "$WORK/kh" && cargo kani)
    ;;
*) echo "elf-probe.sh: unknown mode '$MODE'" >&2; exit 2 ;;
esac
