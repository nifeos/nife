//! **Placing a validated ELF by its *physical* addresses**, which is the one thing a boot loader
//! needs and an ordinary program loader never does.
//!
//! The kernel is linked high with `AT()`, so `p_vaddr` and `p_paddr` are unrelated even within one
//! file (`.text` at `p_vaddr` `0xffffffff8010a000` and `p_paddr` `0x10a000`, `.ap_trampoline` at
//! `0x8000` and `0x166000`). The headers are read by `elf::Elf::parse`, with `elf::Segment::paddr`
//! (milestone 196 (a physical address on `elf::Segment`)), which validates as it reads: bounds,
//! overlap, W^X, and the entry inside an executable segment. So a kernel that regressed to an `RWX`
//! segment fails to boot on real firmware with a printed reason, a second mechanism behind
//! `script/image-permissions`. See notes/x86-uefi-boot/loader-design.md.

//! # BUGS
//!
//! - **Segments must not overlap the loader's own image**, and nothing here checks it: the firmware
//!   is asked for exactly the kernel's `p_paddr` span (`kernel/link-x86_64.ld`'s `PHYS_START`) with
//!   `AllocateAddress`, so a conflict is an allocation failure with the range and the descriptors
//!   in the way printed, not corruption. That check is the firmware's.
//! - **`Elf::parse`'s overlap check is over `p_vaddr`, not `p_paddr`**, so a linker script that
//!   made two segments' physical ranges collide would pass, and the later would overwrite the
//!   earlier. No linker script in this tree can produce that (one script, one contiguous physical
//!   layout).

use elf::{Elf, Segment};

/// The `[first, last)` physical range every `PT_LOAD` falls in, page-aligned outwards.
///
/// The firmware is asked for it in one `AllocatePages(AllocateAddress)` call: the segments are
/// contiguous by construction (one linker script), and one call is one failure to report instead
/// of thirteen. **Over `paddr`, not `vaddr`**: a program loader maps at `p_vaddr` and a firmware
/// loader places at `p_paddr`, and on this image they are unrelated (`elf::Segment::paddr`). `None`
/// when there is no `PT_LOAD`, which `Elf::parse` cannot produce; a return value rather than an
/// `unwrap`, because this function is public and must be panic-free on its own terms.
pub fn physical_span<'a>(
    segments: impl Iterator<Item = Segment<'a>>,
    page_size: u64,
) -> Option<(u64, u64)> {
    let mut first = u64::MAX;
    let mut last = 0u64;
    let mut any = false;
    for segment in segments {
        any = true;
        first = first.min(segment.paddr);
        last = last.max(segment.paddr.saturating_add(segment.memsz));
    }
    if !any {
        return None;
    }
    Some((
        first & !(page_size - 1),
        last.div_ceil(page_size).saturating_mul(page_size),
    ))
}

/// **The physical address the firmware should jump to**: `e_entry` translated through the segment
/// that contains it.
///
/// On `x86_64` this is the identity, because `_start` lives in `.boot`, linked where it is loaded.
/// The aarch64 and riscv64 kernels are linked in the high half with `AT()` placing them low
/// (`kernel/link-aarch64.ld`, `kernel/link-riscv64.ld`), so `e_entry` is a virtual address nothing
/// maps yet and the jump goes to `_start`'s bytes, as QEMU's `-kernel` and U-Boot's `booti` do.
///
/// `None` when no segment contains the entry, which `Elf::parse` refuses before this runs
/// (`EntryNotExecutable`); a return value rather than a panic for [`physical_span`]'s reason.
pub fn physical_entry<'a>(entry: u64, segments: impl Iterator<Item = Segment<'a>>) -> Option<u64> {
    segments
        .filter(|s| entry >= s.vaddr && entry - s.vaddr < s.memsz)
        .map(|s| s.paddr + (entry - s.vaddr))
        .next()
}

/// A sentence for the firmware console, for every way [`Elf::parse`] can refuse the kernel this
/// binary was built around.
///
/// Worded for somebody standing at a machine that will not boot: every one means **the build is
/// wrong**, not that a hostile file arrived, since the only ELF this loader sees is the kernel
/// `build.rs` embedded in it. Exhaustive on purpose, with no `_` arm: `elf::Error` is a shared
/// definition, so growing it should break this build rather than silently print the wrong sentence
/// (rung one of AGENTS.md's ladder, for the price of a match).
pub const fn refusal(error: elf::Error) -> &'static str {
    use elf::Error::*;
    match error {
        TooSmall => "the embedded kernel is smaller than an ELF header",
        BadMagic => "the embedded kernel is not an ELF file",
        Not64Bit => "the embedded kernel is not ELF64",
        NotLittleEndian => "the embedded kernel is not little-endian",
        BadVersion => "the embedded kernel's ELF version is not 1",
        WrongMachine => "the embedded kernel is for a different architecture than this loader",
        NeedsRelocation => "the embedded kernel is a PIE and nothing here relocates it",
        NotExecutable => "the embedded kernel is not an executable",
        BadProgramHeaders => "the embedded kernel's program header table is out of bounds",
        SegmentOutOfBounds => "an embedded kernel segment runs past the end of the file",
        SegmentTruncated => "an embedded kernel segment has more bytes than memory",
        WritableAndExecutable => {
            "the embedded kernel ships a writable-executable segment (see kernel/link-x86_64.ld)"
        }
        SegmentUnreachable => "an embedded kernel segment is neither readable nor executable",
        SegmentsOverlap => "two embedded kernel segments want the same page",
        EntryNotExecutable => "the embedded kernel's entry point is not in an executable segment",
        AddressOverflow => "an embedded kernel segment's address arithmetic overflows",
        TooManyProgramHeaders => "the embedded kernel has more program headers than we will read",
    }
}

/// **Does this ELF still carry DWARF?** True if any section is named `.debug_*`, and also true when
/// the section table cannot be read, because "could not prove it stripped" must not read as
/// "stripped".
///
/// A debug image cannot be installed (calef's ruling of 2026-10-03 UTC on the install boot slots: a
/// slot holds a release image), so the stick of a debug build offers no install. A debug kernel is
/// about 7.5 MB of which 5.8 MB is DWARF, and a slot is a fixed 64 MiB (`BOOT_FILE_MAX`).
/// `crates/elf` reads program headers only, so this reads the section headers itself (ELF64,
/// little-endian). `cargo xtask`'s release path strips with `--strip-debug`, which removes every
/// `.debug_*` section and keeps the symbol table.
pub fn carries_debug_info(elf: &[u8]) -> bool {
    fn u16_at(b: &[u8], at: usize) -> Option<usize> {
        Some(u16::from_le_bytes(b.get(at..at.checked_add(2)?)?.try_into().ok()?) as usize)
    }
    fn u32_at(b: &[u8], at: usize) -> Option<usize> {
        Some(u32::from_le_bytes(b.get(at..at.checked_add(4)?)?.try_into().ok()?) as usize)
    }
    fn u64_at(b: &[u8], at: usize) -> Option<usize> {
        usize::try_from(u64::from_le_bytes(
            b.get(at..at.checked_add(8)?)?.try_into().ok()?,
        ))
        .ok()
    }
    fn scan(elf: &[u8]) -> Option<bool> {
        let shoff = u64_at(elf, 0x28)?;
        let entsize = u16_at(elf, 0x3a)?;
        let count = u16_at(elf, 0x3c)?;
        let names = u16_at(elf, 0x3e)?;
        if count == 0 {
            return Some(false);
        }
        if entsize < 0x28 || names >= count {
            return None;
        }
        let header = |i: usize| shoff.checked_add(i.checked_mul(entsize)?);
        if elf.len() < header(count)? {
            return None;
        }
        let strtab_at = header(names)?;
        let strtab_off = u64_at(elf, strtab_at.checked_add(0x18)?)?;
        let strtab_len = u64_at(elf, strtab_at.checked_add(0x20)?)?;
        let strtab = elf.get(strtab_off..strtab_off.checked_add(strtab_len)?)?;
        for i in 0..count {
            let name = u32_at(elf, header(i)?)?;
            if strtab.get(name..)?.starts_with(b".debug_") {
                return Some(true);
            }
        }
        Some(false)
    }
    scan(elf).unwrap_or(true)
}

/// Parse and validate the embedded kernel, with this loader's wording on a refusal.
pub fn parse(bytes: &[u8]) -> Result<Elf<'_>, &'static str> {
    Elf::parse(bytes).map_err(refusal)
}

#[cfg(test)]
mod tests {
    use elf::{NATIVE_MACHINE, PF_R, PF_W, PF_X};

    use super::*;

    /// `Segment`'s fields are all public, so the span can be tested against hand-built segments
    /// rather than against a forged ELF. The forging belongs to `crates/elf`'s own tests, which is
    /// where `Elf::parse` is proved; what is under test here is the arithmetic over `paddr`.
    fn seg(paddr: u64, memsz: u64) -> Segment<'static> {
        Segment {
            vaddr: paddr + 0xffff_ffff_8000_0000,
            paddr,
            memsz,
            flags: PF_R | PF_X,
            data: &[],
        }
    }

    #[test]
    fn a_high_half_entry_is_entered_where_its_bytes_are() {
        // aarch64's shape: `_start` at 0xffff_0000_4008_0000, loaded at 0x4008_0000.
        let text = Segment {
            vaddr: 0xffff_0000_4008_0000,
            paddr: 0x4008_0000,
            memsz: 0x2_0000,
            flags: PF_R | PF_X,
            data: &[],
        };
        assert_eq!(
            physical_entry(0xffff_0000_4008_0040, [text].into_iter()),
            Some(0x4008_0040)
        );
        // x86_64's shape: `.boot` is linked where it loads, so the translation is the identity.
        let boot = Segment {
            vaddr: 0x200_0000,
            paddr: 0x200_0000,
            memsz: 0x1000,
            flags: PF_R | PF_X,
            data: &[],
        };
        assert_eq!(
            physical_entry(0x200_0010, [boot].into_iter()),
            Some(0x200_0010)
        );
        assert_eq!(physical_entry(0x10, [boot, text].into_iter()), None);
    }

    /// **A segment contains its entry only below its end**: an entry exactly at one segment's end
    /// is the next segment's first byte, and translating it through the first would jump into the
    /// wrong physical page. Milestone 326 (turn a mutation score upward), 2026-09-24.
    #[test]
    fn an_entry_at_a_segments_end_belongs_to_the_next_segment() {
        let first = Segment {
            vaddr: 0xffff_0000_4008_0000,
            paddr: 0x4008_0000,
            memsz: 0x1000,
            flags: PF_R | PF_X,
            data: &[],
        };
        let second = Segment {
            vaddr: 0xffff_0000_4008_1000,
            paddr: 0x5000_0000,
            memsz: 0x1000,
            flags: PF_R | PF_X,
            data: &[],
        };
        assert_eq!(
            physical_entry(0xffff_0000_4008_1000, [first, second].into_iter()),
            Some(0x5000_0000)
        );
        assert_eq!(
            physical_entry(0xffff_0000_4008_1000, [first].into_iter()),
            None
        );
    }

    /// The property this module exists for: the span is over **`p_paddr`**, so an image whose
    /// virtual addresses are in the high half still reports a low physical span.
    #[test]
    fn the_span_is_physical_and_page_aligned_outwards() {
        let segments = [
            seg(0x10_0000, 0x14),
            seg(0x10_9000, 0x4c000),
            seg(0x23_8123, 0x100),
        ];
        assert_eq!(
            physical_span(segments.into_iter(), 4096),
            Some((0x10_0000, 0x239_000))
        );
    }

    /// A `NOLOAD` section arrives as a segment with no bytes and a real size, and it has to be
    /// counted in the span or the firmware hands its memory to something else.
    #[test]
    fn a_noload_segment_still_reserves_its_memory() {
        let segments = [seg(0x10_2000, 0x7000)];
        assert_eq!(
            physical_span(segments.into_iter(), 4096),
            Some((0x10_2000, 0x109_000))
        );
    }

    /// The trampoline's shape: its `p_vaddr` is *below* every other segment's while its `p_paddr`
    /// is in the middle of the image. A span taken over `vaddr` would start at 0x8000 and ask the
    /// firmware for a megabyte of real-mode memory it does not own.
    #[test]
    fn a_segment_that_ships_high_and_runs_low_is_spanned_where_it_ships() {
        let trampoline = Segment {
            vaddr: 0x8000,
            paddr: 0x16_6000,
            memsz: 0xe0,
            flags: PF_R | PF_X,
            data: &[],
        };
        assert_eq!(
            physical_span([seg(0x10_0000, 0x14), trampoline].into_iter(), 4096),
            Some((0x10_0000, 0x167_000))
        );
    }

    #[test]
    fn an_image_with_no_load_segments_has_no_span() {
        assert_eq!(physical_span([].into_iter(), 4096), None);
    }

    /// **The rounding is outwards at BOTH ends, and only the low end had ever been asked.** Every
    /// other test here starts its lowest segment on a page boundary, where `first & !(page_size-1)`
    /// is the identity and any mask at all gives the right answer; the mutation run found two
    /// survivors on that expression for exactly that reason (`page_size + 1` and `page_size / 1`
    /// both clear bits the aligned inputs never had set).
    ///
    /// A segment that starts mid-page is not hypothetical bookkeeping. The firmware is asked for
    /// this range with one `AllocatePages(AllocateAddress)`, which takes a **page** number, so a
    /// span that begins above the segment's first byte asks for memory that starts after the bytes
    /// about to be written into it.
    #[test]
    fn a_segment_starting_mid_page_pulls_the_span_down_to_its_page() {
        let segments = [seg(0x10_0800, 0x100)];
        assert_eq!(
            physical_span(segments.into_iter(), 4096),
            Some((0x10_0000, 0x10_1000))
        );
    }

    /// Every refusal has its own sentence, and none of them is empty. The value of the exhaustive
    /// match is that adding a variant to `elf::Error` fails this build; the value of this test is
    /// that nobody satisfies that by pasting the same string.
    #[test]
    fn every_refusal_says_something_of_its_own() {
        use elf::Error::*;
        let all = [
            TooSmall,
            BadMagic,
            Not64Bit,
            NotLittleEndian,
            BadVersion,
            WrongMachine,
            NeedsRelocation,
            NotExecutable,
            BadProgramHeaders,
            SegmentOutOfBounds,
            SegmentTruncated,
            WritableAndExecutable,
            SegmentUnreachable,
            SegmentsOverlap,
            EntryNotExecutable,
            AddressOverflow,
            TooManyProgramHeaders,
        ];
        let mut seen: Vec<&str> = all.iter().map(|e| refusal(*e)).collect();
        assert!(seen.iter().all(|s| !s.is_empty()));
        seen.sort_unstable();
        let before = seen.len();
        seen.dedup();
        assert_eq!(seen.len(), before, "two refusals share a sentence");
    }

    /// A real property of the pair, cheap to state and easy to get wrong: a W^X violation is
    /// refused, and the sentence a person reads names the file to go and edit.
    #[test]
    fn a_writable_executable_segment_is_named_by_the_file_that_causes_it() {
        assert!(refusal(elf::Error::WritableAndExecutable).contains("link-x86_64.ld"));
        let s = Segment {
            vaddr: 0x1000,
            paddr: 0x1000,
            memsz: 0x1000,
            flags: PF_R | PF_W | PF_X,
            data: &[],
        };
        assert!(s.is_writable() && s.is_executable());
    }

    /// One `PT_LOAD` segment, forged by hand, with the flags and the physical address the caller
    /// asks for. Adapted from `crates/elf`'s own doc example, which is where the format is
    /// explained; what differs here is that `p_paddr` and `p_vaddr` are **not** equal, because a
    /// physical address unrelated to the virtual one is the whole shape this module deals in.
    ///
    /// `NATIVE_MACHINE` rather than a literal 62: `elf::Elf::parse` checks the machine of the build
    /// it is compiled into, and this test runs on whatever the developer's laptop is. The x86-64
    /// wording in [`refusal`] is about the kernel this loader ships, not about this check.
    fn forge(flags: u32, paddr: u64) -> Vec<u8> {
        const EHDR: usize = 64;
        const PHDR: usize = 56;
        let vaddr = paddr + 0xffff_ffff_8000_0000;
        let mut v = vec![0u8; EHDR + PHDR + 0x20];
        v[0..4].copy_from_slice(&[0x7f, b'E', b'L', b'F']);
        v[4] = 2; // ELFCLASS64
        v[5] = 1; // ELFDATA2LSB
        v[6] = 1; // EV_CURRENT
        v[16..18].copy_from_slice(&2u16.to_le_bytes()); // ET_EXEC
        v[18..20].copy_from_slice(&NATIVE_MACHINE.to_le_bytes());
        v[24..32].copy_from_slice(&vaddr.to_le_bytes()); // e_entry
        v[32..40].copy_from_slice(&(EHDR as u64).to_le_bytes()); // e_phoff
        v[54..56].copy_from_slice(&(PHDR as u16).to_le_bytes()); // e_phentsize
        v[56..58].copy_from_slice(&1u16.to_le_bytes()); // e_phnum

        let p = EHDR;
        v[p..p + 4].copy_from_slice(&1u32.to_le_bytes()); // PT_LOAD
        v[p + 4..p + 8].copy_from_slice(&flags.to_le_bytes());
        v[p + 8..p + 16].copy_from_slice(&((EHDR + PHDR) as u64).to_le_bytes()); // p_offset
        v[p + 16..p + 24].copy_from_slice(&vaddr.to_le_bytes());
        v[p + 24..p + 32].copy_from_slice(&paddr.to_le_bytes());
        v[p + 32..p + 40].copy_from_slice(&0x20u64.to_le_bytes()); // p_filesz
        v[p + 40..p + 48].copy_from_slice(&0x20u64.to_le_bytes()); // p_memsz
        v
    }

    /// **Nothing called [`parse`] until this test, which is the hole the mutation run could not
    /// even report.** Its one mutant (`Ok(Default::default())`) is unviable because `elf::Elf` has
    /// no `Default`, so the tool scored the function as if it did not exist; see the note in
    /// notes/mutation-testing.md for why deriving one here would be the wrong repair.
    ///
    /// What this asserts is the thing an unviable mutant hides: an accepted image comes back
    /// *carrying its contents*, so a wrapper that returned an empty `Elf` would fail here rather
    /// than boot a machine into a kernel with no segments.
    #[test]
    fn an_accepted_image_arrives_with_its_segments_and_its_entry() {
        let bytes = forge(PF_R | PF_X, 0x10_2000);
        let elf = match parse(&bytes) {
            Ok(elf) => elf,
            Err(why) => panic!("a well-formed executable was refused: {why}"),
        };
        assert_eq!(elf.entry(), 0x10_2000 + 0xffff_ffff_8000_0000);
        let segments: Vec<_> = elf.segments().collect();
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].paddr, 0x10_2000);
        assert_eq!(segments[0].memsz, 0x20);
        assert_eq!(
            physical_span(elf.segments(), 4096),
            Some((0x10_2000, 0x10_3000))
        );
    }

    /// The other half of the same wrapper: a refusal comes back as **this loader's sentence**, not
    /// as `elf::Error`'s `Debug`. The audience is somebody standing at a machine that will not
    /// boot, so the mapping through [`refusal`] is the behaviour under test rather than an
    /// implementation detail.
    #[test]
    fn a_refused_image_arrives_as_this_loaders_sentence() {
        let bytes = forge(PF_R | PF_W | PF_X, 0x10_2000);
        // Matched rather than unwrapped: `elf::Elf` is deliberately neither `Debug` nor
        // `PartialEq`, since it is a validated token rather than a value, and every `Result`
        // helper that reports a failure wants to print the `Ok` side.
        let sentence = |bytes: &[u8]| match parse(bytes) {
            Ok(_) => panic!("accepted an image that must be refused"),
            Err(why) => why,
        };
        assert_eq!(sentence(&bytes), refusal(elf::Error::WritableAndExecutable));
        assert_eq!(sentence(&[]), refusal(elf::Error::TooSmall));
    }

    /// A minimal ELF64 with a section table: a null section, the string table, and one named
    /// section per entry of `names`.
    fn with_sections(names: &[&str]) -> std::vec::Vec<u8> {
        with_sections_of(names, 64)
    }

    /// [`with_sections`] with section headers `entsize` bytes apart, at least the 0x28 this reads.
    fn with_sections_of(names: &[&str], entsize: usize) -> std::vec::Vec<u8> {
        let mut strtab = std::vec![0u8];
        let mut offsets = std::vec![];
        for n in names {
            offsets.push(strtab.len() as u32);
            strtab.extend_from_slice(n.as_bytes());
            strtab.push(0);
        }
        let shstr_name = strtab.len() as u32;
        strtab.extend_from_slice(b".shstrtab\0");
        let shoff = 64 + strtab.len();
        let count = names.len() + 2;
        let mut out = std::vec![0u8; 64];
        out[0x28..0x30].copy_from_slice(&(shoff as u64).to_le_bytes());
        out[0x3a..0x3c].copy_from_slice(&(entsize as u16).to_le_bytes());
        out[0x3c..0x3e].copy_from_slice(&(count as u16).to_le_bytes());
        out[0x3e..0x40].copy_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&strtab);
        let mut header = |name: u32, off: u64, len: u64| {
            let mut h = std::vec![0u8; entsize];
            h[0..4].copy_from_slice(&name.to_le_bytes());
            h[0x18..0x20].copy_from_slice(&off.to_le_bytes());
            h[0x20..0x28].copy_from_slice(&len.to_le_bytes());
            out.extend_from_slice(&h);
        };
        header(0, 0, 0);
        header(shstr_name, 64, strtab.len() as u64);
        for o in offsets {
            header(o, 0, 0);
        }
        out
    }

    #[test]
    fn a_kernel_with_dwarf_sections_is_a_debug_image() {
        assert!(carries_debug_info(&with_sections(&[
            ".text",
            ".debug_info"
        ])));
        assert!(carries_debug_info(&with_sections(&[".debug_line"])));
    }

    #[test]
    fn a_stripped_kernel_is_not() {
        assert!(!carries_debug_info(&with_sections(&[
            ".text", ".rodata", ".symtab"
        ])));
        // A section that merely mentions the word later in its name is not DWARF.
        assert!(!carries_debug_info(&with_sections(&[".not.debug_x"])));
        // No section table at all.
        assert!(!carries_debug_info(&[0u8; 64]));
    }

    /// The smallest section header this reads, 0x28 bytes, is read; one smaller is not.
    #[test]
    fn a_section_table_of_the_smallest_entry_size_is_read() {
        assert!(!carries_debug_info(&with_sections_of(&[".text"], 0x28)));
        assert!(carries_debug_info(&with_sections_of(
            &[".debug_info"],
            0x28
        )));
    }

    /// A string-table index equal to the section count is unreadable even when bytes follow the
    /// table that would parse as one more header: the count bounds the table, not the file.
    #[test]
    fn a_string_table_index_past_the_table_is_unreadable() {
        let mut image = with_sections(&[".text"]);
        let count = u16::from_le_bytes([image[0x3c], image[0x3d]]);
        image[0x3e..0x40].copy_from_slice(&count.to_le_bytes());
        // A copy of the real string table's header, one slot past the end of the table.
        let shoff = image.len() - usize::from(count) * 64;
        let real = image[shoff + 64..shoff + 128].to_vec();
        image.extend_from_slice(&real);
        assert!(carries_debug_info(&image));
    }

    #[test]
    fn a_section_table_that_cannot_be_read_does_not_count_as_stripped() {
        let mut image = with_sections(&[".text"]);
        image.truncate(image.len() - 40);
        assert!(carries_debug_info(&image));
        assert!(carries_debug_info(&[]));
    }
}
