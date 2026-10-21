//! **Turning a position-independent ELF into the PE/COFF image UEFI firmware loads.**
//!
//! UEFI loads applications in Microsoft's PE/COFF format. rustc builds PE directly for the three
//! UEFI targets it has (`aarch64-`, `i686-`, `x86_64-unknown-uefi`) and has **no riscv64 UEFI
//! target**. So the riscv64 boot file is built as an ordinary static-PIE ELF for
//! `riscv64imac-unknown-none-elf` and converted here, as systemd's `elf2efi` does, rather than
//! hand-writing a PE header in assembly (U-Boot, gnu-efi): pure data transformation, host-tested.
//!
//! # What goes in and what comes out
//!
//! In: a 64-bit little-endian ELF of type `ET_DYN` (a PIE), linked with its segments page-aligned
//! (`-z separate-loadable-segments`) and its first segment at or above `0x1000`, whose only dynamic
//! relocations are the architecture's `RELATIVE` kind. That is what `cargo xtask`'s riscv64 build
//! produces, and anything else is refused by name rather than converted wrongly.
//!
//! Out: a PE32+ image whose section RVAs **are** the ELF's virtual addresses (image base 0), one
//! section per `PT_LOAD`, and a `.reloc` section holding one `IMAGE_REL_BASED_DIR64` fixup per ELF
//! relocation. The firmware loads the image at some base `B` and adds `B` to each fixed-up word;
//! this converter has already written each relocation's addend into its word, so the result is
//! `B + addend`, which is exactly what `R_*_RELATIVE` means.
//!

//! # Why the ELF is read here and not with `crates/elf`
//!
//! `crates/elf`'s `Elf::parse` refuses any machine but `NATIVE_MACHINE` and refuses `ET_DYN`
//! (`NeedsRelocation`), correctly for a loader. This reads a different architecture's PIE on the
//! build host, so it reads the four structures it needs (the file header, program headers, the
//! dynamic section and `Elf64_Rela`) itself, and every read is checked.
//!
//! # BUGS
//!
//! - **It reads its own build product and is not hardened against a hostile ELF** (2026-09-24
//!   security audit). The one caller is `xtask/src/stick.rs`, on the ELF `xtask` linked seconds
//!   earlier, so the trust boundary is the build host and an attacker who can rewrite that file
//!   already runs code there. "Every read is checked" above is true of reads and not of the
//!   arithmetic around them: `phoff` near `u64::MAX` overflows before `u32_at` can refuse it, a
//!   `PT_DYNAMIC` offset is never bounds-checked the way `PT_LOAD`s are, a `memsz` of 2^44 with
//!   `filesz` 0 passes every check and sizes the image `Vec` from it, and `DT_RELAENT = 0` with a
//!   large `DT_RELASZ` loops at one address pushing fixups until memory runs out. xtask builds in
//!   the dev profile, so the overflows are panics rather than wraps. Hardening is about twenty lines
//!   of `checked_*` and a cap on `image_end`; it is not done because nothing hostile reaches this
//!   crate, and this entry is what has to change first if that stops being true.
//! - **Only `RELATIVE` relocations are converted.** A static PIE with no dynamic symbols produces
//!   nothing else, and anything else is refused with its type number.
//! - **The image carries no debug directory and no symbols**, so a firmware debugger sees addresses
//!   only. The ELF beside it has them.
//! - **The PE timestamp is zero**, deliberately: the image is a function of the ELF, so two builds of
//!   one ELF are byte-identical.
//!
//! Name: ratified 2026-10-07 (calef, pull request #1806's publish-ours review: "Yes, one per
//! crate"), the format's own name spelled out under §154 (the acronym test is whether the phrase is
//! spoken): "portable executable" is the specification title's phrase. Set aside `elf_to_efi` (a
//! verb phrase, and systemd's tool name) and `pe_image` (the acronym). It is not published:
//! calef refused that on 2026-10-21 (UTC), ruling milestone 784 (reuse what the tree wrote)'s
//! item (g).

#![cfg_attr(not(test), no_std)]

// `no_std` with `alloc`, like the tree's other pure crates: nothing here needs an operating system,
// so it builds and documents for the bare-metal targets alongside them rather than joining the
// host-only set `script/lint` has to name.
extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;

/// `IMAGE_FILE_MACHINE_*` for the machines this converts, by ELF `e_machine`.
fn pe_machine(elf_machine: u16) -> Option<u16> {
    match elf_machine {
        62 => Some(0x8664),  // EM_X86_64 -> IMAGE_FILE_MACHINE_AMD64
        183 => Some(0xaa64), // EM_AARCH64 -> IMAGE_FILE_MACHINE_ARM64
        243 => Some(0x5064), // EM_RISCV -> IMAGE_FILE_MACHINE_RISCV64
        _ => None,
    }
}

/// The `R_*_RELATIVE` relocation type for each machine.
fn relative_type(elf_machine: u16) -> u32 {
    match elf_machine {
        62 => 8,     // R_X86_64_RELATIVE
        183 => 1027, // R_AARCH64_RELATIVE
        _ => 3,      // R_RISCV_RELATIVE
    }
}

/// Why an ELF could not be converted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// Not a 64-bit little-endian ELF.
    NotElf64,
    /// Not `ET_DYN`: a position-dependent image cannot be loaded where the firmware chooses.
    NotPositionIndependent,
    /// An `e_machine` this does not know a PE machine number for.
    UnknownMachine(u16),
    /// A header, table or segment runs past the end of the file.
    Truncated,
    /// A `PT_LOAD` whose address is not a multiple of 4 KiB, or that starts below `0x1000`, where
    /// the PE headers go.
    SegmentLayout,
    /// A dynamic relocation other than `RELATIVE`, by type number.
    UnsupportedRelocation(u32),
    /// `DT_REL`, `DT_RELR` or `DT_JMPREL` tables, which this does not convert.
    UnsupportedRelocationTable,
    /// A relocation outside every segment's file bytes.
    RelocationOutsideImage(u64),
}

const PAGE: u64 = 0x1000;
const FILE_ALIGNMENT: u64 = 0x200;

fn u16_at(b: &[u8], at: usize) -> Result<u16, Error> {
    let s = b.get(at..at + 2).ok_or(Error::Truncated)?;
    Ok(u16::from_le_bytes([s[0], s[1]]))
}
fn u32_at(b: &[u8], at: usize) -> Result<u32, Error> {
    let s = b.get(at..at + 4).ok_or(Error::Truncated)?;
    Ok(u32::from_le_bytes(s.try_into().unwrap_or([0; 4])))
}
fn u64_at(b: &[u8], at: usize) -> Result<u64, Error> {
    let s = b.get(at..at + 8).ok_or(Error::Truncated)?;
    Ok(u64::from_le_bytes(s.try_into().unwrap_or([0; 8])))
}
fn align(value: u64, to: u64) -> u64 {
    value.div_ceil(to) * to
}
fn usize_of(v: u64) -> Result<usize, Error> {
    usize::try_from(v).map_err(|_| Error::Truncated)
}

/// One `PT_LOAD`.
#[derive(Debug, Clone, Copy)]
struct Load {
    offset: u64,
    vaddr: u64,
    filesz: u64,
    memsz: u64,
    flags: u32,
}

/// **Convert** (see the module documentation).
pub fn from_elf(elf: &[u8]) -> Result<Vec<u8>, Error> {
    let header = read_header(elf)?;
    let image_end = check_layout(elf, &header.loads)?;
    let mut image = memory_image(elf, &header.loads, image_end)?;
    let mut fixups = match header.dynamic {
        Some(dynamic) => apply_relocations(elf, &mut image, &header, dynamic)?,
        None => Vec::new(),
    };
    fixups.sort_unstable();
    fixups.dedup();
    let relocations = base_relocations(&fixups);
    let reloc_va = align(image_end, PAGE);
    let sections = sections_of(&header.loads, &image, &relocations, reloc_va);
    write_pe(&header, &sections, &relocations, reloc_va)
}

/// What [`from_elf`] reads from the ELF header and the program headers.
struct Header {
    machine: u16,
    pe_machine: u16,
    entry: u64,
    /// The `PT_LOAD`s, sorted by address.
    loads: Vec<Load>,
    /// `PT_DYNAMIC`'s file offset and size, if there is one.
    dynamic: Option<(u64, u64)>,
}

/// The ELF identification, type and machine, then every program header this converts.
fn read_header(elf: &[u8]) -> Result<Header, Error> {
    if elf.get(0..4) != Some(b"\x7fELF".as_slice())
        || elf.get(4) != Some(&2)
        || elf.get(5) != Some(&1)
    {
        return Err(Error::NotElf64);
    }
    if u16_at(elf, 16)? != 3 {
        return Err(Error::NotPositionIndependent);
    }
    let machine = u16_at(elf, 18)?;
    let pe_machine = pe_machine(machine).ok_or(Error::UnknownMachine(machine))?;
    let entry = u64_at(elf, 24)?;
    let phoff = usize_of(u64_at(elf, 32)?)?;
    let phentsize = usize::from(u16_at(elf, 54)?);
    let phnum = usize::from(u16_at(elf, 56)?);

    let mut loads = Vec::new();
    let mut dynamic: Option<(u64, u64)> = None;
    for i in 0..phnum {
        let at = phoff + i * phentsize;
        let kind = u32_at(elf, at)?;
        let flags = u32_at(elf, at + 4)?;
        let offset = u64_at(elf, at + 8)?;
        let vaddr = u64_at(elf, at + 16)?;
        let filesz = u64_at(elf, at + 32)?;
        let memsz = u64_at(elf, at + 40)?;
        match kind {
            1 => loads.push(Load {
                offset,
                vaddr,
                filesz,
                memsz,
                flags,
            }),
            2 => dynamic = Some((offset, filesz)),
            _ => {}
        }
    }
    loads.sort_by_key(|l| l.vaddr);
    Ok(Header {
        machine,
        pe_machine,
        entry,
        loads,
        dynamic,
    })
}

/// Refuse segments that are unaligned, below the headers, past the file's end or overlapping, and
/// return the address where the last one ends.
fn check_layout(elf: &[u8], loads: &[Load]) -> Result<u64, Error> {
    for load in loads {
        if load.vaddr % PAGE != 0 || load.vaddr < PAGE || load.filesz > load.memsz {
            return Err(Error::SegmentLayout);
        }
        let end = load
            .offset
            .checked_add(load.filesz)
            .ok_or(Error::Truncated)?;
        if usize_of(end)? > elf.len() {
            return Err(Error::Truncated);
        }
    }
    for pair in loads.windows(2) {
        if pair[0].vaddr + pair[0].memsz > pair[1].vaddr {
            return Err(Error::SegmentLayout);
        }
    }
    loads
        .last()
        .map(|l| l.vaddr + l.memsz)
        .ok_or(Error::SegmentLayout)
}

/// The memory image, from address 0, holding every segment's file bytes where it will run.
fn memory_image(elf: &[u8], loads: &[Load], image_end: u64) -> Result<Vec<u8>, Error> {
    let mut image = vec![0u8; usize_of(image_end)?];
    for load in loads {
        let (from, to) = (usize_of(load.offset)?, usize_of(load.vaddr)?);
        let len = usize_of(load.filesz)?;
        image[to..to + len].copy_from_slice(&elf[from..from + len]);
    }
    Ok(image)
}

/// The dynamic relocations: read `PT_DYNAMIC` for the `DT_RELA` table, write each `RELATIVE`
/// addend into `image`, and return the addresses of the words written, unsorted.
fn apply_relocations(
    elf: &[u8],
    image: &mut [u8],
    header: &Header,
    (offset, size): (u64, u64),
) -> Result<Vec<u64>, Error> {
    let mut fixups: Vec<u64> = Vec::new();
    let (mut rela, mut relasz, mut relaent) = (None, 0u64, 24u64);
    let mut at = usize_of(offset)?;
    let end = at + usize_of(size)?;
    while at + 16 <= end {
        let tag = u64_at(elf, at)?;
        let value = u64_at(elf, at + 8)?;
        at += 16;
        match tag {
            0 => break,
            7 => rela = Some(value),
            8 => relasz = value,
            9 => relaent = value,
            17 | 36 => return Err(Error::UnsupportedRelocationTable),
            2 | 23 if value != 0 => return Err(Error::UnsupportedRelocationTable),
            _ => {}
        }
    }
    let Some(rela) = rela else {
        return Ok(fixups);
    };
    // `DT_RELA` is a virtual address; its bytes are in the image built above.
    let base = usize_of(rela)?;
    let count = usize_of(relasz / relaent.max(1))?;
    for i in 0..count {
        let at = base + i * usize_of(relaent)?;
        let r_offset = u64_at(image, at)?;
        let r_info = u64_at(image, at + 8)?;
        let r_addend = u64_at(image, at + 16)?;
        let kind = (r_info & 0xffff_ffff) as u32;
        if kind == 0 {
            continue; // R_*_NONE
        }
        if kind != relative_type(header.machine) || r_info >> 32 != 0 {
            return Err(Error::UnsupportedRelocation(kind));
        }
        // Inside the segment's FILE bytes, not its memory size: the word is written into
        // `image` here, but each section's raw data is only its first `filesz` bytes, so a
        // word in `.bss` would lose its addend while its fixup stayed in `.reloc`, and the
        // firmware would add the load base to zero. This read `memsz` until milestone 326's
        // mutation triage (2026-09-24) wrote the boundary test and it accepted such a word.
        let inside = header
            .loads
            .iter()
            .any(|l| r_offset >= l.vaddr && r_offset + 8 <= l.vaddr + l.filesz);
        if !inside {
            return Err(Error::RelocationOutsideImage(r_offset));
        }
        let word = usize_of(r_offset)?;
        image[word..word + 8].copy_from_slice(&r_addend.to_le_bytes());
        fixups.push(r_offset);
    }
    Ok(fixups)
}

/// One PE section: a segment's file bytes, or the `.reloc` table.
struct Section {
    name: [u8; 8],
    virtual_address: u64,
    virtual_size: u64,
    data: Vec<u8>,
    characteristics: u32,
}

/// A section per segment, named and flagged by its permissions, then `.reloc` if any fixups.
fn sections_of(loads: &[Load], image: &[u8], relocations: &[u8], reloc_va: u64) -> Vec<Section> {
    let mut sections: Vec<Section> = loads
        .iter()
        .map(|l| {
            let (name, characteristics) = if l.flags & 1 != 0 {
                (*b".text\0\0\0", 0x6000_0020) // CODE | EXECUTE | READ
            } else if l.flags & 2 != 0 {
                (*b".data\0\0\0", 0xc000_0040) // INITIALIZED_DATA | READ | WRITE
            } else {
                (*b".rdata\0\0", 0x4000_0040) // INITIALIZED_DATA | READ
            };
            let from = l.vaddr as usize;
            Section {
                name,
                virtual_address: l.vaddr,
                virtual_size: l.memsz,
                data: image[from..from + l.filesz as usize].to_vec(),
                characteristics,
            }
        })
        .collect();
    if !relocations.is_empty() {
        sections.push(Section {
            name: *b".reloc\0\0",
            virtual_address: reloc_va,
            virtual_size: relocations.len() as u64,
            data: relocations.to_vec(),
            characteristics: 0x4200_0040, // INITIALIZED_DATA | DISCARDABLE | READ
        });
    }
    sections
}

const DOS_LEN: usize = 0x40;
const COFF_LEN: usize = 20;
const OPTIONAL_LEN: usize = 240;

fn put16(out: &mut [u8], at: usize, v: u16) {
    out[at..at + 2].copy_from_slice(&v.to_le_bytes());
}
fn put32(out: &mut [u8], at: usize, v: u32) {
    out[at..at + 4].copy_from_slice(&v.to_le_bytes());
}
fn put64(out: &mut [u8], at: usize, v: u64) {
    out[at..at + 8].copy_from_slice(&v.to_le_bytes());
}
fn narrow(v: u64) -> Result<u32, Error> {
    u32::try_from(v).map_err(|_| Error::SegmentLayout)
}

/// **Lay out the PE file**: headers in the first page, then each section's raw data at file
/// alignment.
fn write_pe(
    header: &Header,
    sections: &[Section],
    relocations: &[u8],
    reloc_va: u64,
) -> Result<Vec<u8>, Error> {
    let headers = DOS_LEN + 4 + COFF_LEN + OPTIONAL_LEN + 40 * sections.len();
    let size_of_headers = align(headers as u64, FILE_ALIGNMENT);
    if size_of_headers > PAGE {
        return Err(Error::SegmentLayout);
    }
    let size_of_image = align(
        sections
            .iter()
            .map(|s| s.virtual_address + s.virtual_size)
            .max()
            .unwrap_or(PAGE),
        PAGE,
    );

    let mut raw_at = size_of_headers;
    let mut placements = Vec::new();
    for s in sections {
        let raw_size = align(s.data.len() as u64, FILE_ALIGNMENT);
        placements.push((raw_at, raw_size));
        raw_at += raw_size;
    }
    let mut out = vec![0u8; usize_of(raw_at)?];

    // DOS header: the magic and the offset of the PE signature, nothing else.
    out[0..2].copy_from_slice(b"MZ");
    put32(&mut out, 0x3c, DOS_LEN as u32);
    out[DOS_LEN..DOS_LEN + 4].copy_from_slice(b"PE\0\0");

    let coff = DOS_LEN + 4;
    put16(&mut out, coff, header.pe_machine);
    put16(&mut out, coff + 2, sections.len() as u16);
    // TimeDateStamp, PointerToSymbolTable, NumberOfSymbols: zero.
    put16(&mut out, coff + 16, OPTIONAL_LEN as u16);
    // EXECUTABLE_IMAGE | LARGE_ADDRESS_AWARE | DEBUG_STRIPPED.
    put16(&mut out, coff + 18, 0x0002 | 0x0020 | 0x0200);

    let opt = coff + COFF_LEN;
    write_optional_header(
        &mut out[opt..opt + OPTIONAL_LEN],
        header.entry,
        sections,
        [size_of_image, size_of_headers],
        (reloc_va, relocations.len()),
    )?;

    let table = opt + OPTIONAL_LEN;
    for (i, (s, (raw_at, raw_size))) in sections.iter().zip(&placements).enumerate() {
        let h = table + i * 40;
        out[h..h + 8].copy_from_slice(&s.name);
        put32(&mut out, h + 8, narrow(s.virtual_size)?);
        put32(&mut out, h + 12, narrow(s.virtual_address)?);
        put32(&mut out, h + 16, narrow(*raw_size)?);
        put32(&mut out, h + 20, narrow(*raw_at)?);
        put32(&mut out, h + 36, s.characteristics);
        let at = usize_of(*raw_at)?;
        out[at..at + s.data.len()].copy_from_slice(&s.data);
    }
    Ok(out)
}

/// The PE32+ optional header, written into its own `OPTIONAL_LEN` bytes. `sizes` is
/// `[SizeOfImage, SizeOfHeaders]`; `reloc` is the `.reloc` section's address and length, a zero
/// length leaving data directory 5 empty.
fn write_optional_header(
    opt: &mut [u8],
    entry: u64,
    sections: &[Section],
    [size_of_image, size_of_headers]: [u64; 2],
    (reloc_va, reloc_len): (u64, usize),
) -> Result<(), Error> {
    let code: u64 = sections
        .iter()
        .filter(|s| s.characteristics & 0x20 != 0)
        .map(|s| align(s.data.len() as u64, FILE_ALIGNMENT))
        .sum();
    let data: u64 = sections
        .iter()
        .filter(|s| s.characteristics & 0x20 == 0)
        .map(|s| align(s.data.len() as u64, FILE_ALIGNMENT))
        .sum();
    let base_of_code = sections
        .iter()
        .find(|s| s.characteristics & 0x20 != 0)
        .map_or(0, |s| s.virtual_address);
    put16(opt, 0, 0x20b); // PE32+
    put32(opt, 4, narrow(code)?);
    put32(opt, 8, narrow(data)?);
    put32(opt, 16, narrow(entry)?);
    put32(opt, 20, narrow(base_of_code)?);
    put64(opt, 24, 0); // ImageBase: the RVAs are the ELF's addresses from zero
    put32(opt, 32, PAGE as u32); // SectionAlignment
    put32(opt, 36, FILE_ALIGNMENT as u32);
    put32(opt, 56, narrow(size_of_image)?);
    put32(opt, 60, narrow(size_of_headers)?);
    put16(opt, 68, 10); // IMAGE_SUBSYSTEM_EFI_APPLICATION
    // DllCharacteristics: HIGH_ENTROPY_VA | DYNAMIC_BASE | NX_COMPAT, which is what the image is.
    put16(opt, 70, 0x0020 | 0x0040 | 0x0100);
    put32(opt, 108, 16); // NumberOfRvaAndSizes
    if reloc_len != 0 {
        // Data directory 5: the base relocation table.
        put32(opt, 112 + 5 * 8, narrow(reloc_va)?);
        put32(opt, 112 + 5 * 8 + 4, reloc_len as u32);
    }
    Ok(())
}

/// **The `.reloc` section**: fixups grouped into one block per 4 KiB page, each block a page RVA,
/// a block size, and a 16-bit entry per fixup (type in the top four bits, offset in the low
/// twelve), padded to four bytes with an `IMAGE_REL_BASED_ABSOLUTE` entry, which is a no-op.
fn base_relocations(sorted: &[u64]) -> Vec<u8> {
    const DIR64: u16 = 10;
    let mut out = Vec::new();
    let mut i = 0;
    while i < sorted.len() {
        let page = sorted[i] & !(PAGE - 1);
        let mut entries: Vec<u16> = Vec::new();
        while i < sorted.len() && sorted[i] & !(PAGE - 1) == page {
            entries.push((DIR64 << 12) | (sorted[i] & (PAGE - 1)) as u16);
            i += 1;
        }
        if entries.len() % 2 == 1 {
            entries.push(0);
        }
        out.extend_from_slice(&(page as u32).to_le_bytes());
        out.extend_from_slice(&((8 + 2 * entries.len()) as u32).to_le_bytes());
        for e in entries {
            out.extend_from_slice(&e.to_le_bytes());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal riscv64 PIE: a code segment at 0x1000, a data segment at 0x2000 holding one pointer
    /// to 0x1010, a dynamic section and one `R_RISCV_RELATIVE` for that pointer.
    fn tiny_pie(reloc_type: u32) -> Vec<u8> {
        let mut e = vec![0u8; 0x3000];
        e[0..4].copy_from_slice(b"\x7fELF");
        e[4] = 2;
        e[5] = 1;
        e[6] = 1;
        e[16..18].copy_from_slice(&3u16.to_le_bytes()); // ET_DYN
        e[18..20].copy_from_slice(&243u16.to_le_bytes()); // EM_RISCV
        e[24..32].copy_from_slice(&0x1004u64.to_le_bytes()); // entry
        e[32..40].copy_from_slice(&64u64.to_le_bytes()); // phoff
        e[54..56].copy_from_slice(&56u16.to_le_bytes());
        e[56..58].copy_from_slice(&3u16.to_le_bytes());
        let ph = |e: &mut Vec<u8>,
                  i: usize,
                  kind: u32,
                  flags: u32,
                  off: u64,
                  va: u64,
                  fsz: u64,
                  msz: u64| {
            let at = 64 + i * 56;
            e[at..at + 4].copy_from_slice(&kind.to_le_bytes());
            e[at + 4..at + 8].copy_from_slice(&flags.to_le_bytes());
            e[at + 8..at + 16].copy_from_slice(&off.to_le_bytes());
            e[at + 16..at + 24].copy_from_slice(&va.to_le_bytes());
            e[at + 32..at + 40].copy_from_slice(&fsz.to_le_bytes());
            e[at + 40..at + 48].copy_from_slice(&msz.to_le_bytes());
        };
        ph(&mut e, 0, 1, 5, 0x1000, 0x1000, 0x20, 0x20); // R X
        ph(&mut e, 1, 1, 6, 0x2000, 0x2000, 0x100, 0x1100); // R W, with .bss
        ph(&mut e, 2, 2, 6, 0x2040, 0x2040, 0x40, 0x40); // DYNAMIC
        e[0x1000..0x1004].copy_from_slice(&0x0000_0013u32.to_le_bytes()); // nop
        // The pointer word at 0x2008 holds zero; the relocation's addend is 0x1010.
        let dyn_at = 0x2040;
        for (i, (tag, val)) in [(7u64, 0x2080u64), (8, 24), (9, 24), (0, 0)]
            .iter()
            .enumerate()
        {
            e[dyn_at + i * 16..dyn_at + i * 16 + 8].copy_from_slice(&tag.to_le_bytes());
            e[dyn_at + i * 16 + 8..dyn_at + i * 16 + 16].copy_from_slice(&val.to_le_bytes());
        }
        e[0x2080..0x2088].copy_from_slice(&0x2008u64.to_le_bytes());
        e[0x2088..0x2090].copy_from_slice(&u64::from(reloc_type).to_le_bytes());
        e[0x2090..0x2098].copy_from_slice(&0x1010u64.to_le_bytes());
        e
    }

    fn u16le(b: &[u8], at: usize) -> u16 {
        u16::from_le_bytes([b[at], b[at + 1]])
    }
    fn u32le(b: &[u8], at: usize) -> u32 {
        u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
    }

    #[test]
    fn a_pie_becomes_a_pe_whose_rvas_are_its_addresses() {
        let pe = from_elf(&tiny_pie(3)).unwrap();
        assert_eq!(&pe[0..2], b"MZ");
        let pe_at = u32le(&pe, 0x3c) as usize;
        assert_eq!(&pe[pe_at..pe_at + 4], b"PE\0\0");
        let coff = pe_at + 4;
        assert_eq!(u16le(&pe, coff), 0x5064, "IMAGE_FILE_MACHINE_RISCV64");
        assert_eq!(u16le(&pe, coff + 2), 3, ".text, .data, .reloc");
        let opt = coff + 20;
        assert_eq!(u16le(&pe, opt), 0x20b);
        assert_eq!(u32le(&pe, opt + 16), 0x1004, "the entry point is the ELF's");
        assert_eq!(u16le(&pe, opt + 68), 10, "an EFI application");
        assert_eq!(
            u32le(&pe, opt + 56),
            0x5000,
            "SizeOfImage covers .bss and then .reloc on its own page"
        );

        let sections = opt + 240;
        let section = |i: usize| {
            let h = sections + i * 40;
            (
                &pe[h..h + 8],
                u32le(&pe, h + 8),
                u32le(&pe, h + 12),
                u32le(&pe, h + 20),
            )
        };
        let (name, vsize, va, raw) = section(0);
        assert_eq!((name, vsize, va), (b".text\0\0\0".as_slice(), 0x20, 0x1000));
        assert_eq!(&pe[raw as usize..raw as usize + 4], &0x13u32.to_le_bytes());
        let (name, vsize, va, raw) = section(1);
        assert_eq!(
            (name, vsize, va),
            (b".data\0\0\0".as_slice(), 0x1100, 0x2000)
        );
        // The pointer word now holds the addend, so firmware adding the load base makes it right.
        assert_eq!(
            u64::from_le_bytes(pe[raw as usize + 8..raw as usize + 16].try_into().unwrap()),
            0x1010
        );
        let (name, _, va, raw) = section(2);
        assert_eq!(name, b".reloc\0\0");
        assert_eq!(va, 0x4000);
        let block = &pe[raw as usize..];
        assert_eq!(u32le(block, 0), 0x2000, "the page");
        assert_eq!(
            u32le(block, 4),
            12,
            "header plus two entries (one real, one pad)"
        );
        assert_eq!(u16le(block, 8), (10 << 12) | 0x008, "DIR64 at offset 8");
        assert_eq!(u16le(block, 10), 0, "ABSOLUTE padding");
        assert_eq!(
            u32le(&pe, opt + 112 + 40),
            0x4000,
            "data directory 5 names .reloc"
        );
    }

    #[test]
    fn converting_twice_is_byte_identical() {
        assert_eq!(from_elf(&tiny_pie(3)), from_elf(&tiny_pie(3)));
    }

    #[test]
    fn refuses_what_it_cannot_convert_correctly() {
        assert_eq!(from_elf(&tiny_pie(2)), Err(Error::UnsupportedRelocation(2)));
        let mut exec = tiny_pie(3);
        exec[16] = 2;
        assert_eq!(from_elf(&exec), Err(Error::NotPositionIndependent));
        assert_eq!(from_elf(b"MZ not an elf"), Err(Error::NotElf64));
        let mut odd = tiny_pie(3);
        odd[64 + 16..64 + 24].copy_from_slice(&0x1234u64.to_le_bytes());
        assert_eq!(from_elf(&odd), Err(Error::SegmentLayout));
    }

    #[test]
    fn relocation_blocks_split_by_page_and_pad_to_four_bytes() {
        let r = base_relocations(&[0x2000, 0x2008, 0x3ff8]);
        assert_eq!(u32le(&r, 0), 0x2000);
        assert_eq!(u32le(&r, 4), 12);
        assert_eq!(u32le(&r, 12), 0x3000);
        assert_eq!(u32le(&r, 16), 12, "one entry padded to two");
        assert_eq!(r.len(), 24);
    }

    // --- The tests below are from milestone 326 (turn a mutation score upward), 2026-09-24. Each
    // --- names the property it pins; notes/mutation-testing/portable-executable.md has the
    // --- mutants each one was seen to kill.

    fn put(e: &mut [u8], at: usize, bytes: &[u8]) {
        e[at..at + bytes.len()].copy_from_slice(bytes);
    }

    /// An ELF header with `phnum` program headers at 64, for `machine`, `len` bytes long.
    fn header(machine: u16, phnum: u16, len: usize) -> Vec<u8> {
        let mut e = vec![0u8; len];
        put(&mut e, 0, b"\x7fELF");
        e[4] = 2;
        e[5] = 1;
        e[6] = 1;
        put(&mut e, 16, &3u16.to_le_bytes());
        put(&mut e, 18, &machine.to_le_bytes());
        put(&mut e, 24, &0x2000u64.to_le_bytes()); // entry
        put(&mut e, 32, &64u64.to_le_bytes());
        put(&mut e, 54, &56u16.to_le_bytes());
        put(&mut e, 56, &phnum.to_le_bytes());
        e
    }

    /// Program header `i`: (kind, flags, offset = vaddr, filesz, memsz).
    fn phdr(e: &mut [u8], i: usize, kind: u32, flags: u32, va: u64, fsz: u64, msz: u64) {
        let at = 64 + i * 56;
        put(e, at, &kind.to_le_bytes());
        put(e, at + 4, &flags.to_le_bytes());
        put(e, at + 8, &va.to_le_bytes());
        put(e, at + 16, &va.to_le_bytes());
        put(e, at + 32, &fsz.to_le_bytes());
        put(e, at + 40, &msz.to_le_bytes());
    }

    fn dynamic(e: &mut [u8], at: usize, entries: &[(u64, u64)]) {
        for (i, (tag, value)) in entries.iter().enumerate() {
            put(e, at + i * 16, &tag.to_le_bytes());
            put(e, at + i * 16 + 8, &value.to_le_bytes());
        }
    }

    fn rela(e: &mut [u8], at: usize, offset: u64, kind: u64, addend: u64) {
        put(e, at, &offset.to_le_bytes());
        put(e, at + 8, &kind.to_le_bytes());
        put(e, at + 16, &addend.to_le_bytes());
    }

    /// Three kinds of segment, deliberately out of the order a linker usually emits them so that
    /// "the first section" and "the first code section" differ: a read-only segment at 0x1000
    /// (0x300 bytes), code at 0x2000 (0x20), and data at 0x3000 (0x100 bytes, 0x1100 in memory)
    /// holding a six-entry dynamic section at 0x3040 (four used, then zeros) and one
    /// `R_RISCV_RELATIVE` at 0x30c0 for the word at 0x3008. Offsets equal addresses, so a byte's
    /// file position is its address.
    fn three_kinds() -> Vec<u8> {
        let mut e = header(243, 4, 0x3100);
        phdr(&mut e, 0, 1, 4, 0x1000, 0x300, 0x300); // R
        phdr(&mut e, 1, 1, 5, 0x2000, 0x20, 0x20); // R X
        phdr(&mut e, 2, 1, 6, 0x3000, 0x100, 0x1100); // R W, with .bss
        phdr(&mut e, 3, 2, 6, 0x3040, 0x60, 0x60);
        put(&mut e, 0x1000, b"rodata");
        put(&mut e, 0x2000, &0x13u32.to_le_bytes());
        dynamic(&mut e, 0x3040, &[(7, 0x30c0), (8, 24), (9, 24), (0, 0)]);
        rela(&mut e, 0x30c0, 0x3008, 3, 0x1234);
        e
    }

    /// **Every header field the firmware reads, with the value it must have.** The first test
    /// above checks the fields a reader looks at first; this one checks the rest, because the
    /// firmware reads all of them and a wrong one is a boot that fails with no console to say so.
    #[test]
    fn every_header_field_the_firmware_reads_has_its_value() {
        let pe = from_elf(&three_kinds()).unwrap();
        let coff = 0x44;
        assert_eq!(u16le(&pe, coff + 2), 4, ".rdata, .text, .data, .reloc");
        assert_eq!(u16le(&pe, coff + 16), 240, "SizeOfOptionalHeader");
        assert_eq!(
            u16le(&pe, coff + 18),
            0x0222,
            "EXECUTABLE_IMAGE | LARGE_ADDRESS_AWARE | DEBUG_STRIPPED"
        );
        let opt = coff + 20;
        assert_eq!(
            u32le(&pe, opt + 4),
            0x200,
            "SizeOfCode: .text's raw size only"
        );
        assert_eq!(
            u32le(&pe, opt + 8),
            0x400 + 0x200 + 0x200,
            "SizeOfInitializedData: .rdata, .data and .reloc"
        );
        assert_eq!(u32le(&pe, opt + 16), 0x2000, "AddressOfEntryPoint");
        assert_eq!(
            u32le(&pe, opt + 20),
            0x2000,
            "BaseOfCode is .text, not the first section"
        );
        assert_eq!(&pe[opt + 24..opt + 32], &[0; 8], "ImageBase");
        assert_eq!(u32le(&pe, opt + 32), 0x1000, "SectionAlignment");
        assert_eq!(u32le(&pe, opt + 36), 0x200, "FileAlignment");
        assert_eq!(
            u32le(&pe, opt + 56),
            0x6000,
            "SizeOfImage: .reloc's page at 0x5000, whole"
        );
        assert_eq!(u32le(&pe, opt + 60), 0x200, "SizeOfHeaders");
        assert_eq!(u16le(&pe, opt + 68), 10, "EFI_APPLICATION");
        assert_eq!(
            u16le(&pe, opt + 70),
            0x0160,
            "HIGH_ENTROPY_VA | DYNAMIC_BASE | NX_COMPAT"
        );
        assert_eq!(u32le(&pe, opt + 108), 16, "NumberOfRvaAndSizes");
        assert_eq!(
            u32le(&pe, opt + 112 + 40),
            0x5000,
            "BASERELOC directory RVA"
        );
        assert_eq!(u32le(&pe, opt + 112 + 44), 12, "BASERELOC directory size");

        // (name, VirtualSize, VirtualAddress, SizeOfRawData, PointerToRawData, Characteristics)
        type Row = (&'static [u8; 8], u32, u32, u32, u32, u32);
        let expected: [Row; 4] = [
            (b".rdata\0\0", 0x300, 0x1000, 0x400, 0x200, 0x4000_0040),
            (b".text\0\0\0", 0x20, 0x2000, 0x200, 0x600, 0x6000_0020),
            (b".data\0\0\0", 0x1100, 0x3000, 0x200, 0x800, 0xc000_0040),
            (b".reloc\0\0", 12, 0x5000, 0x200, 0xa00, 0x4200_0040),
        ];
        for (i, want) in expected.iter().enumerate() {
            let h = opt + 240 + i * 40;
            let got = (
                &pe[h..h + 8],
                u32le(&pe, h + 8),
                u32le(&pe, h + 12),
                u32le(&pe, h + 16),
                u32le(&pe, h + 20),
                u32le(&pe, h + 36),
            );
            assert_eq!(
                got,
                (want.0.as_slice(), want.1, want.2, want.3, want.4, want.5),
                "section {i}"
            );
        }
        assert_eq!(pe.len(), 0xc00, "the last section's raw bytes end the file");
        assert_eq!(
            &pe[0x200..0x206],
            b"rodata",
            ".rdata's bytes at its PointerToRawData"
        );
        assert_eq!(&pe[0x600..0x604], &0x13u32.to_le_bytes());
        assert_eq!(
            &pe[0x808..0x810],
            &0x1234u64.to_le_bytes(),
            "the addend, written in place"
        );
    }

    /// **Each machine gets its own PE machine number and is held to its own `RELATIVE` type.** A
    /// relocation type is only meaningful per machine: 3 is `R_RISCV_RELATIVE` but `R_AARCH64_*`
    /// something else entirely, so accepting another machine's number would convert a relocation
    /// this does not understand.
    #[test]
    fn each_machine_gets_its_pe_number_and_its_own_relative_type() {
        for (machine, relative, pe_machine) in [
            (62u16, 8u64, 0x8664u16),
            (183, 1027, 0xaa64),
            (243, 3, 0x5064),
        ] {
            let mut e = three_kinds();
            put(&mut e, 18, &machine.to_le_bytes());
            rela(&mut e, 0x30c0, 0x3008, relative, 0x1234);
            let pe = from_elf(&e).unwrap_or_else(|err| panic!("machine {machine}: {err:?}"));
            assert_eq!(u16le(&pe, 0x44), pe_machine, "machine {machine}");
            for other in [8u64, 1027, 3].into_iter().filter(|&t| t != relative) {
                rela(&mut e, 0x30c0, 0x3008, other, 0x1234);
                assert_eq!(
                    from_elf(&e),
                    Err(Error::UnsupportedRelocation(other as u32)),
                    "machine {machine} must refuse type {other}"
                );
            }
        }
        let mut e = three_kinds();
        put(&mut e, 18, &40u16.to_le_bytes()); // EM_ARM
        assert_eq!(from_elf(&e), Err(Error::UnknownMachine(40)));
    }

    /// **The dynamic walk stops at `DT_NULL`, and it stops at the end of its segment.** What
    /// follows either is not the dynamic section, so a tag found there must not be obeyed. Both
    /// fixtures put a `DT_REL`, which would be refused, just past the real end.
    #[test]
    fn the_dynamic_walk_ends_at_dt_null_or_at_its_segment_end() {
        let mut e = three_kinds();
        dynamic(&mut e, 0x3040 + 0x40, &[(17, 0x3000)]);
        assert!(from_elf(&e).is_ok(), "DT_NULL ends the walk");

        let mut e = three_kinds();
        phdr(&mut e, 3, 2, 6, 0x3040, 0x30, 0x30); // three entries and no DT_NULL
        dynamic(&mut e, 0x3040 + 0x30, &[(17, 0x3000)]);
        assert!(from_elf(&e).is_ok(), "the segment's end ends the walk");
    }

    /// **Which relocation tables are refused, and which are only refused when non-empty.**
    /// `DT_REL` and `DT_RELR` hold relocations this does not convert, so their presence is enough.
    /// `DT_PLTRELSZ` and `DT_JMPREL` are emitted as zero by linkers that have no PLT, and a zero one
    /// holds nothing to convert.
    #[test]
    fn relocation_tables_it_cannot_convert_are_refused_by_tag() {
        let with = |tag: u64, value: u64| {
            let mut e = three_kinds();
            dynamic(&mut e, 0x3040 + 0x30, &[(tag, value), (0, 0)]);
            from_elf(&e)
        };
        assert_eq!(
            with(17, 0x3000),
            Err(Error::UnsupportedRelocationTable),
            "DT_REL"
        );
        assert_eq!(
            with(36, 0x3000),
            Err(Error::UnsupportedRelocationTable),
            "DT_RELR"
        );
        assert_eq!(
            with(2, 24),
            Err(Error::UnsupportedRelocationTable),
            "DT_PLTRELSZ"
        );
        assert_eq!(
            with(23, 0x3000),
            Err(Error::UnsupportedRelocationTable),
            "DT_JMPREL"
        );
        assert!(with(2, 0).is_ok(), "an empty DT_PLTRELSZ");
        assert!(with(23, 0).is_ok(), "an empty DT_JMPREL");
    }

    /// **`DT_RELAENT` is the stride, and every entry is read at `base + i * stride`.** A linker
    /// is free to pad `Elf64_Rela`; one that does, read at a fixed 24, reads the second entry
    /// from the middle of the first.
    #[test]
    fn dt_relaent_is_the_stride_between_relocations() {
        let mut e = three_kinds();
        dynamic(&mut e, 0x3040, &[(7, 0x30c0), (8, 64), (9, 32), (0, 0)]);
        rela(&mut e, 0x30c0, 0x3008, 3, 0x1111);
        rela(&mut e, 0x30e0, 0x3010, 3, 0x2222);
        let pe = from_elf(&e).unwrap();
        assert_eq!(&pe[0x808..0x810], &0x1111u64.to_le_bytes());
        assert_eq!(&pe[0x810..0x818], &0x2222u64.to_le_bytes());
        let block = &pe[0xa00..];
        assert_eq!(u32le(block, 4), 12, "two fixups");
        assert_eq!(u16le(block, 8), (10 << 12) | 0x008);
        assert_eq!(u16le(block, 10), (10 << 12) | 0x010);
    }

    /// **A relocated word must lie wholly inside a segment's file bytes.** The word is written into
    /// the section's raw data, so a word past `filesz` (in `.bss`) would have its addend dropped
    /// while its fixup stayed in `.reloc`, and the firmware would add the load base to zero. The
    /// last whole word before `filesz` is accepted; a word straddling it, a word in `.bss`, and a
    /// word below every segment are refused.
    #[test]
    fn a_relocated_word_lies_wholly_inside_a_segments_file_bytes() {
        let at = |offset: u64| {
            let mut e = three_kinds();
            rela(&mut e, 0x30c0, offset, 3, 0x1234);
            from_elf(&e)
        };
        let pe = at(0x30f8).unwrap();
        assert_eq!(
            &pe[0x8f8..0x900],
            &0x1234u64.to_le_bytes(),
            "the last whole word"
        );
        assert_eq!(
            at(0x30fc),
            Err(Error::RelocationOutsideImage(0x30fc)),
            "straddles filesz"
        );
        assert_eq!(
            at(0x3200),
            Err(Error::RelocationOutsideImage(0x3200)),
            "in .bss"
        );
        assert_eq!(
            at(0x0800),
            Err(Error::RelocationOutsideImage(0x0800)),
            "below every segment"
        );
    }

    /// **The segment layout checks at their exact boundaries.** A segment may end at the last byte
    /// of the file and not one past it; two segments may touch and not overlap; `filesz` may not
    /// exceed `memsz`.
    #[test]
    fn segment_layout_is_checked_at_its_exact_boundaries() {
        let mut e = three_kinds();
        e.truncate(0x3100);
        assert!(
            from_elf(&e).is_ok(),
            "a segment ending exactly at the file's end"
        );
        e.truncate(0x30ff);
        assert_eq!(from_elf(&e), Err(Error::Truncated), "one byte short");

        let mut e = three_kinds();
        phdr(&mut e, 0, 1, 4, 0x1000, 0x300, 0x1000);
        assert!(from_elf(&e).is_ok(), "segments that touch");
        phdr(&mut e, 0, 1, 4, 0x1000, 0x300, 0x1001);
        assert_eq!(
            from_elf(&e),
            Err(Error::SegmentLayout),
            "segments that overlap by a byte"
        );

        let mut e = three_kinds();
        phdr(&mut e, 1, 1, 5, 0x2000, 0x21, 0x20);
        assert_eq!(
            from_elf(&e),
            Err(Error::SegmentLayout),
            "filesz above memsz"
        );
    }

    /// **Only a 64-bit little-endian ELF is read**, and each of the two ident bytes is checked on
    /// its own.
    #[test]
    fn only_elfclass64_little_endian_is_read() {
        let mut e = three_kinds();
        e[4] = 1;
        assert_eq!(from_elf(&e), Err(Error::NotElf64), "ELFCLASS32");
        let mut e = three_kinds();
        e[5] = 2;
        assert_eq!(from_elf(&e), Err(Error::NotElf64), "big-endian");
    }

    /// **The section table must fit in the first page, and 94 sections exactly fill it.** The
    /// first segment starts at 0x1000, so headers past a page would overwrite it once loaded. The
    /// header length is 0x40 + 4 + 20 + 240 + 40 per section: 94 is 4,088 bytes, aligned to exactly
    /// 0x1000, and 95 is 4,128.
    #[test]
    fn the_headers_round_up_to_file_alignment_and_fit_in_one_page() {
        let loads = |n: u16| {
            let mut e = header(243, n, 0x1000 * (usize::from(n) + 1));
            for i in 0..usize::from(n) {
                let va = 0x1000 * (i as u64 + 1);
                phdr(&mut e, i, 1, 4, va, 0x10, 0x10);
            }
            e
        };
        // 56 sections is 2,568 bytes of header, eight past 0xa00: a count short by those eight
        // would place the first section's raw bytes over the last section header.
        let pe = from_elf(&loads(56)).unwrap();
        assert_eq!(
            u32le(&pe, 0x44 + 20 + 60),
            0xc00,
            "SizeOfHeaders, rounded up"
        );
        assert_eq!(
            u32le(&pe, 0x44 + 20 + 240 + 20),
            0xc00,
            "the first section's raw data"
        );
        let pe = from_elf(&loads(94)).unwrap();
        assert_eq!(
            u32le(&pe, 0x44 + 20 + 60),
            0x1000,
            "SizeOfHeaders, exactly a page"
        );
        assert_eq!(
            u32le(&pe, 0x44 + 20 + 240 + 20),
            0x1000,
            "the first section's raw data"
        );
        assert_eq!(from_elf(&loads(95)), Err(Error::SegmentLayout));
    }

    /// **A header field is read from exactly its own bytes.** An ELF cut off right after
    /// `e_phnum` has every field this reads, so it reaches the layout check rather than being
    /// called truncated.
    #[test]
    fn a_header_field_needs_exactly_its_own_bytes() {
        let e = header(243, 0, 58);
        assert_eq!(from_elf(&e), Err(Error::SegmentLayout), "no PT_LOAD at all");
    }

    /// **A block's size counts its header and two bytes per entry.** Four entries in one page is
    /// the case that tells `8 + 2 * n` from `8 + 2 + n`, which agree at two.
    #[test]
    fn a_relocation_block_is_eight_bytes_plus_two_per_entry() {
        let r = base_relocations(&[0x2000, 0x2008, 0x2010, 0x2018]);
        assert_eq!(u32le(&r, 4), 16);
        assert_eq!(r.len(), 16);
    }
}
