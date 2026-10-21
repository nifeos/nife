//! **Turning a position-independent ELF into the PE/COFF image UEFI firmware loads.**
//!
//! UEFI loads applications in Microsoft's PE/COFF format. rustc builds PE directly for the three
//! UEFI targets it has (`aarch64-`, `i686-`, `x86_64-unknown-uefi`) and has **no riscv64 UEFI
//! target**. So the riscv64 boot file is built as an ordinary static-PIE ELF for
//! `riscv64imac-unknown-none-elf` and converted here, as systemd's `elf2efi` does, rather than
//! hand-writing a PE header in assembly (U-Boot, gnu-efi).
//!
//! **`object` reads the ELF and writes the PE; the conversion is ours** (milestone 784 (reuse what
//! the tree wrote), item (g), ruled by calef on 2026-10-21 (UTC): "B"). `object::read::elf` parses
//! the file header, program headers, dynamic section and `Elf64_Rela` with every offset checked, and
//! `object::write::pe::Writer` lays out the headers, the section table and `.reloc`. What this file
//! decides is what no library knows: which ELFs it will convert, where each segment goes, and what a
//! relocation becomes.
//!
//! # What goes in and what comes out
//!
//! In: a 64-bit little-endian ELF of type `ET_DYN` (a PIE), linked with its segments page-aligned
//! (`-z separate-loadable-segments`) and its first segment at or above `0x1000`, whose only dynamic
//! relocations are the architecture's `RELATIVE` kind. That is what this file's caller links, and
//! anything else is refused by name rather than converted wrongly.
//!
//! Out: a PE32+ image whose section RVAs **are** the ELF's virtual addresses (image base 0), one
//! section per `PT_LOAD`, and a `.reloc` section holding one `IMAGE_REL_BASED_DIR64` fixup per ELF
//! relocation. The firmware loads the image at some base `B` and adds `B` to each fixed-up word;
//! this has already written each relocation's addend into its word, so the result is `B + addend`,
//! which is exactly what `R_*_RELATIVE` means.
//!
//! # BUGS
//!
//! - **Only `RELATIVE` relocations are converted.** A static PIE with no dynamic symbols produces
//!   nothing else, and anything else is refused with its type number.
//! - **The image carries no debug directory and no symbols**, so a firmware debugger sees addresses
//!   only. The ELF beside it has them.
//! - **The PE timestamp is zero**, deliberately: the image is a function of the ELF, so two builds of
//!   one ELF are byte-identical.
//!
//! Name: ratified 2026-10-07 (calef, pull request #1806's publish-ours review: "Yes, one per
//! crate"), the format's own name spelled out under §154 (the acronym test is whether the phrase
//! is spoken): "portable executable" is the specification title's phrase.

use object::elf::{self, FileHeader64, ProgramHeader64};
use object::pe;
use object::read::elf::{Dyn as _, FileHeader as _, ProgramHeader as _, Rela as _};
use object::write::pe::{NtHeaders, Writer};
use object::{LittleEndian as LE, pod};

/// `IMAGE_FILE_MACHINE_*` and the `R_*_RELATIVE` type, for each machine this converts.
fn machine(elf_machine: elf::Machine) -> Option<(pe::Machine, elf::RelocationType)> {
    match elf_machine {
        elf::EM_X86_64 => Some((pe::IMAGE_FILE_MACHINE_AMD64, elf::R_X86_64_RELATIVE)),
        elf::EM_AARCH64 => Some((pe::IMAGE_FILE_MACHINE_ARM64, elf::R_AARCH64_RELATIVE)),
        elf::EM_RISCV => Some((pe::IMAGE_FILE_MACHINE_RISCV64, elf::R_RISCV_RELATIVE)),
        _ => None,
    }
}

/// Why an ELF could not be converted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Error {
    /// `object` could not read it: not a 64-bit little-endian ELF, or a header or table that runs
    /// past the end of the file.
    Elf(object::read::Error),
    /// A `PT_LOAD` past the end of the file, or a `DT_RELA` table past the end of the image.
    Truncated,
    /// `object` could not write the PE.
    Write(object::write::Error),
    /// Not `ET_DYN`: a position-dependent image cannot be loaded where the firmware chooses.
    NotPositionIndependent,
    /// An `e_machine` this does not know a PE machine number for.
    UnknownMachine(u16),
    /// A `PT_LOAD` whose address is not a multiple of 4 KiB, that starts below `0x1000` where the
    /// PE headers go, or that overlaps another; or an image past what PE's 32-bit fields hold.
    SegmentLayout,
    /// A dynamic relocation other than `RELATIVE`, by type number.
    UnsupportedRelocation(u32),
    /// `DT_REL`, `DT_RELR` or `DT_JMPREL` tables, which this does not convert.
    UnsupportedRelocationTable,
    /// A relocation outside every segment's file bytes.
    RelocationOutsideImage(u64),
}

impl From<object::read::Error> for Error {
    fn from(e: object::read::Error) -> Self {
        Error::Elf(e)
    }
}

const PAGE: u64 = 0x1000;
const FILE_ALIGNMENT: u32 = 0x200;

fn align(value: u64, to: u64) -> u64 {
    value.div_ceil(to) * to
}
fn narrow(v: u64) -> Result<u32, Error> {
    u32::try_from(v).map_err(|_| Error::SegmentLayout)
}

/// One `PT_LOAD`, with its file bytes.
#[derive(Debug, Clone, Copy)]
struct Load<'a> {
    vaddr: u64,
    memsz: u64,
    bytes: &'a [u8],
    flags: elf::ProgramFlags,
}

/// What [`from_elf`] reads from the ELF.
struct Header<'a> {
    pe_machine: pe::Machine,
    relative: elf::RelocationType,
    entry: u64,
    /// The `PT_LOAD`s, sorted by address.
    loads: Vec<Load<'a>>,
    /// `PT_DYNAMIC`'s entries, if there is one.
    dynamic: &'a [elf::Dyn64<LE>],
}

/// **Convert** (see the module documentation).
pub(crate) fn from_elf(elf: &[u8]) -> Result<Vec<u8>, Error> {
    let header = read_header(elf)?;
    let image_end = check_layout(&header.loads)?;
    let mut image = memory_image(&header.loads, image_end);
    let mut fixups = apply_relocations(&mut image, &header)?;
    fixups.sort_unstable();
    fixups.dedup();
    write_pe(&header, &image, &fixups)
}

/// The ELF identification, type and machine, then every program header this converts.
fn read_header(elf: &[u8]) -> Result<Header<'_>, Error> {
    let file = FileHeader64::<LE>::parse(elf)?;
    let endian = file.endian()?;
    if file.e_type(endian) != elf::ET_DYN {
        return Err(Error::NotPositionIndependent);
    }
    let e_machine = file.e_machine(endian);
    let (pe_machine, relative) = machine(e_machine).ok_or(Error::UnknownMachine(e_machine.0))?;
    let mut loads = Vec::new();
    let mut dynamic: &[elf::Dyn64<LE>] = &[];
    for segment in file.program_headers(endian, elf)? {
        let segment: &ProgramHeader64<LE> = segment;
        match segment.p_type(endian) {
            elf::PT_LOAD => loads.push(Load {
                vaddr: segment.p_vaddr(endian),
                memsz: segment.p_memsz(endian),
                bytes: segment
                    .data(endian, elf)
                    .map_err(|()| Error::Truncated)?,
                flags: segment.p_flags(endian),
            }),
            elf::PT_DYNAMIC => dynamic = segment.dynamic(endian, elf)?.unwrap_or(&[]),
            _ => {}
        }
    }
    loads.sort_by_key(|l| l.vaddr);
    Ok(Header {
        pe_machine,
        relative,
        entry: file.e_entry(endian),
        loads,
        dynamic,
    })
}

/// Refuse segments that are unaligned, below the headers or overlapping, and return the address
/// where the last one ends.
fn check_layout(loads: &[Load]) -> Result<u64, Error> {
    for load in loads {
        if load.vaddr % PAGE != 0 || load.vaddr < PAGE || load.bytes.len() as u64 > load.memsz {
            return Err(Error::SegmentLayout);
        }
    }
    for pair in loads.windows(2) {
        if pair[0].vaddr + pair[0].memsz > pair[1].vaddr {
            return Err(Error::SegmentLayout);
        }
    }
    let end = loads
        .last()
        .map(|l| l.vaddr + l.memsz)
        .ok_or(Error::SegmentLayout)?;
    // SizeOfImage is 32 bits, and `.reloc` takes the page after the last segment.
    narrow(align(end, PAGE) + PAGE)?;
    Ok(end)
}

/// The memory image, from address 0, holding every segment's file bytes where it will run.
fn memory_image(loads: &[Load], image_end: u64) -> Vec<u8> {
    let mut image = vec![0u8; image_end as usize];
    for load in loads {
        let at = load.vaddr as usize;
        image[at..at + load.bytes.len()].copy_from_slice(load.bytes);
    }
    image
}

/// The dynamic relocations: walk `PT_DYNAMIC` for the `DT_RELA` table, write each `RELATIVE`
/// addend into `image`, and return the addresses of the words written, unsorted.
fn apply_relocations(image: &mut [u8], header: &Header) -> Result<Vec<u64>, Error> {
    let (mut rela, mut relasz, mut relaent) = (None, 0u64, 24u64);
    for entry in header.dynamic {
        let value = entry.d_val(LE);
        match entry.d_tag(LE) {
            elf::DT_NULL => break,
            elf::DT_RELA => rela = Some(value),
            elf::DT_RELASZ => relasz = value,
            elf::DT_RELAENT => relaent = value,
            elf::DT_REL | elf::DT_RELR => return Err(Error::UnsupportedRelocationTable),
            elf::DT_PLTRELSZ | elf::DT_JMPREL if value != 0 => {
                return Err(Error::UnsupportedRelocationTable);
            }
            _ => {}
        }
    }
    let mut fixups = Vec::new();
    let Some(rela) = rela else {
        return Ok(fixups);
    };
    // `DT_RELA` is a virtual address; its bytes are in the image built above.
    let stride = relaent.max(1);
    for i in 0..relasz / stride {
        let at = rela
            .checked_add(i * stride)
            .and_then(|at| usize::try_from(at).ok())
            .and_then(|at| image.get(at..))
            .ok_or(Error::Truncated)?;
        let (entry, _) =
            pod::from_bytes::<elf::Rela64<LE>>(at).map_err(|()| Error::Truncated)?;
        let (r_offset, r_addend) = (entry.r_offset(LE), entry.r_addend(LE) as u64);
        let kind = entry.r_type(LE, false);
        if kind == elf::R_RISCV_NONE {
            continue; // R_*_NONE is 0 on every machine
        }
        if kind != header.relative || entry.r_sym(LE, false) != 0 {
            return Err(Error::UnsupportedRelocation(kind.0));
        }
        // Inside the segment's FILE bytes, not its memory size: the word is written into
        // `image` here, but each section's raw data is only its first `filesz` bytes, so a
        // word in `.bss` would lose its addend while its fixup stayed in `.reloc`, and the
        // firmware would add the load base to zero.
        let inside = header.loads.iter().any(|l| {
            r_offset >= l.vaddr && r_offset.saturating_add(8) <= l.vaddr + l.bytes.len() as u64
        });
        if !inside {
            return Err(Error::RelocationOutsideImage(r_offset));
        }
        let word = r_offset as usize;
        image[word..word + 8].copy_from_slice(&r_addend.to_le_bytes());
        fixups.push(r_offset);
    }
    Ok(fixups)
}

/// A section's name and flags, from its segment's permissions.
fn section_kind(flags: elf::ProgramFlags) -> ([u8; 8], pe::SectionFlags) {
    if flags.contains(elf::PF_X) {
        let code = pe::IMAGE_SCN_CNT_CODE | pe::IMAGE_SCN_MEM_EXECUTE | pe::IMAGE_SCN_MEM_READ;
        (*b".text\0\0\0", code)
    } else if flags.contains(elf::PF_W) {
        let data =
            pe::IMAGE_SCN_CNT_INITIALIZED_DATA | pe::IMAGE_SCN_MEM_READ | pe::IMAGE_SCN_MEM_WRITE;
        (*b".data\0\0\0", data)
    } else {
        let rdata = pe::IMAGE_SCN_CNT_INITIALIZED_DATA | pe::IMAGE_SCN_MEM_READ;
        (*b".rdata\0\0", rdata)
    }
}

/// **Lay out the PE file** with `object`'s writer: headers in the first page, a section per
/// segment at the segment's own address, then `.reloc` on the page after the last.
fn write_pe(header: &Header, image: &[u8], fixups: &[u64]) -> Result<Vec<u8>, Error> {
    let mut out = Vec::new();
    let mut writer = Writer::new(true, PAGE as u32, FILE_ALIGNMENT, &mut out);
    writer.reserve_dos_header();
    writer.reserve_nt_headers(pe::IMAGE_NUMBEROF_DIRECTORY_ENTRIES);
    let sections = header.loads.len() + usize::from(!fixups.is_empty());
    writer.reserve_section_headers(u16::try_from(sections).map_err(|_| Error::SegmentLayout)?);
    // The first segment is at 0x1000 or above, so headers past a page would be overwritten once
    // the image is loaded.
    if writer.reserved_len() > PAGE {
        return Err(Error::SegmentLayout);
    }
    let mut ranges = Vec::new();
    for load in &header.loads {
        let (name, flags) = section_kind(load.flags);
        writer.reserve_virtual_until(narrow(load.vaddr)?);
        let range = writer.reserve_section(
            name,
            flags,
            narrow(load.memsz)?,
            narrow(load.bytes.len() as u64)?,
        );
        let at = load.vaddr as usize;
        ranges.push((range.file_offset, &image[at..at + load.bytes.len()]));
    }
    for &fixup in fixups {
        writer.add_reloc(narrow(fixup)?, pe::IMAGE_REL_BASED_DIR64);
    }
    if writer.has_relocs() {
        writer.reserve_reloc_section();
    }

    writer.write_empty_dos_header().map_err(Error::Write)?;
    writer.write_nt_headers(NtHeaders {
        machine: header.pe_machine,
        time_date_stamp: 0,
        characteristics: pe::IMAGE_FILE_EXECUTABLE_IMAGE
            | pe::IMAGE_FILE_LARGE_ADDRESS_AWARE
            | pe::IMAGE_FILE_DEBUG_STRIPPED,
        major_linker_version: 0,
        minor_linker_version: 0,
        address_of_entry_point: narrow(header.entry)?,
        // The RVAs are the ELF's addresses from zero.
        image_base: 0,
        major_operating_system_version: 0,
        minor_operating_system_version: 0,
        major_image_version: 0,
        minor_image_version: 0,
        major_subsystem_version: 0,
        minor_subsystem_version: 0,
        subsystem: pe::IMAGE_SUBSYSTEM_EFI_APPLICATION,
        // What the image is: relocatable anywhere, and nothing executes from a data page.
        dll_characteristics: pe::IMAGE_DLLCHARACTERISTICS_HIGH_ENTROPY_VA
            | pe::IMAGE_DLLCHARACTERISTICS_DYNAMIC_BASE
            | pe::IMAGE_DLLCHARACTERISTICS_NX_COMPAT,
        size_of_stack_reserve: 0,
        size_of_stack_commit: 0,
        size_of_heap_reserve: 0,
        size_of_heap_commit: 0,
    });
    writer.write_section_headers();
    for (offset, bytes) in ranges {
        writer.write_section(offset, bytes);
    }
    writer.write_reloc_section();
    Ok(out)
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
        assert!(matches!(from_elf(b"MZ not an elf"), Err(Error::Elf(_))));
        let mut odd = tiny_pie(3);
        odd[64 + 16..64 + 24].copy_from_slice(&0x1234u64.to_le_bytes());
        assert_eq!(from_elf(&odd), Err(Error::SegmentLayout));
    }

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
        // `object` sums each section's virtual size at file alignment, so `.data` counts its
        // `.bss` too. Firmware does not read either field to load the image.
        assert_eq!(u32le(&pe, opt + 4), 0x200, "SizeOfCode: .text only");
        assert_eq!(
            u32le(&pe, opt + 8),
            0x400 + 0x1200 + 0x200,
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
        assert!(matches!(from_elf(&e), Err(Error::Elf(_))), "ELFCLASS32");
        let mut e = three_kinds();
        e[5] = 2;
        assert!(matches!(from_elf(&e), Err(Error::Elf(_))), "big-endian");
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
}
