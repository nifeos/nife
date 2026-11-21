use super::*;
extern crate std;
use std::vec;
use std::vec::Vec;

/// Build an ELF64 image by hand, so the tests can lie about anything they like. A forged hostile
/// file is a few lines here; one from a real toolchain would need that toolchain to cooperate.
struct Builder {
    e_type: u16,
    e_machine: u16,
    class: u8,
    data: u8,
    version: u8,
    magic: [u8; 4],
    entry: u64,
    segments: Vec<(u32, u64, u64, Vec<u8>, u64)>, // flags, vaddr, paddr, bytes, memsz
    lie_about_filesz: Option<u64>,
    lie_about_offset: Option<u64>,
}

impl Builder {
    fn new() -> Self {
        Builder {
            e_type: ET_EXEC,
            // The machine this build accepts, so every forgery reaches the property past the
            // machine check on any host.
            e_machine: NATIVE_MACHINE,
            class: ELFCLASS64,
            data: ELFDATA2LSB,
            version: EV_CURRENT,
            magic: MAGIC,
            entry: 0x40_0000,
            segments: vec![],
            lie_about_filesz: None,
            lie_about_offset: None,
        }
    }

    /// A segment whose physical address equals its virtual one, which is what every real
    /// user program looks like. Use `seg_at_paddr` for the kernel-image shape.
    fn seg(self, flags: u32, vaddr: u64, bytes: &[u8], memsz: u64) -> Self {
        self.seg_at_paddr(flags, vaddr, vaddr, bytes, memsz)
    }

    /// A segment whose `p_paddr` and `p_vaddr` genuinely differ, the way a kernel image linked
    /// high and loaded low does.
    fn seg_at_paddr(
        mut self,
        flags: u32,
        vaddr: u64,
        paddr: u64,
        bytes: &[u8],
        memsz: u64,
    ) -> Self {
        self.segments
            .push((flags, vaddr, paddr, bytes.to_vec(), memsz));
        self
    }

    fn build(self) -> Vec<u8> {
        let phnum = self.segments.len();
        let phoff = EHDR_SIZE;
        let mut body_off = EHDR_SIZE + phnum * PHDR_SIZE;

        let mut ehdr = vec![0u8; EHDR_SIZE];
        ehdr[0..4].copy_from_slice(&self.magic);
        ehdr[4] = self.class;
        ehdr[5] = self.data;
        ehdr[6] = self.version;
        ehdr[16..18].copy_from_slice(&self.e_type.to_le_bytes());
        ehdr[18..20].copy_from_slice(&self.e_machine.to_le_bytes());
        ehdr[24..32].copy_from_slice(&self.entry.to_le_bytes());
        ehdr[32..40].copy_from_slice(&(phoff as u64).to_le_bytes());
        ehdr[54..56].copy_from_slice(&(PHDR_SIZE as u16).to_le_bytes());
        ehdr[56..58].copy_from_slice(&(phnum as u16).to_le_bytes());

        let mut phdrs = vec![];
        let mut body = vec![];
        for (flags, vaddr, paddr, bytes, memsz) in &self.segments {
            let mut ph = vec![0u8; PHDR_SIZE];
            ph[0..4].copy_from_slice(&PT_LOAD.to_le_bytes());
            ph[4..8].copy_from_slice(&flags.to_le_bytes());
            let off = self.lie_about_offset.unwrap_or(body_off as u64);
            ph[8..16].copy_from_slice(&off.to_le_bytes());
            ph[16..24].copy_from_slice(&vaddr.to_le_bytes());
            ph[24..32].copy_from_slice(&paddr.to_le_bytes());
            let fsz = self.lie_about_filesz.unwrap_or(bytes.len() as u64);
            ph[32..40].copy_from_slice(&fsz.to_le_bytes());
            ph[40..48].copy_from_slice(&memsz.to_le_bytes());
            phdrs.extend_from_slice(&ph);
            body.extend_from_slice(bytes);
            body_off += bytes.len();
        }

        let mut out = ehdr;
        out.extend_from_slice(&phdrs);
        out.extend_from_slice(&body);
        out
    }
}

/// The happy path: two segments, code and data, and the entry point lands in the code.
fn good() -> Vec<u8> {
    Builder::new()
        .seg(READABLE | EXECUTABLE, 0x40_0000, &[0xaa; 16], 16)
        .seg(READABLE | WRITABLE, 0x41_0000, &[0xbb; 8], 4096) // memsz > filesz: .bss
        .build()
}

#[test]
fn a_good_binary_parses() {
    let bytes = good();
    let elf = Elf::parse(&bytes).expect("should parse");

    assert_eq!(elf.entry(), 0x40_0000);

    let segs: Vec<_> = elf.segments().collect();
    assert_eq!(segs.len(), 2);

    assert_eq!(segs[0].virtual_address, 0x40_0000);
    assert_eq!(segs[0].physical_address, 0x40_0000);
    assert!(segs[0].is_executable() && !segs[0].is_writable());
    assert_eq!(segs[0].data, &[0xaa; 16]);

    assert!(segs[1].is_writable() && !segs[1].is_executable());
}

/// **`memsz > filesz` is `.bss`, and forgetting it is the classic ELF loader bug.**
///
/// The file carries 8 bytes; the program expects 4096, with the rest zeroed. A loader that
/// copies `filesz` and stops hands the program 4088 bytes of whoever used that frame last.
#[test]
fn bss_is_the_difference_between_memsz_and_filesz() {
    let bytes = good();
    let elf = Elf::parse(&bytes).unwrap();
    let data = elf.segments().nth(1).unwrap();

    assert_eq!(data.data.len(), 8, "filesz");
    assert_eq!(data.memory_bytes, 4096, "memsz");
    assert!(
        data.memory_bytes as usize > data.data.len(),
        "the loader must zero {} bytes the file does not contain",
        data.memory_bytes as usize - data.data.len(),
    );
}

/// **W^X, refused at the door.**
///
/// An ELF can simply *ask* for a page that is both writable and executable, and a loader
/// that grants it has handed the program the thing every exploit wants. This is the check that
/// stops a file talking a loader into building one.
#[test]
fn a_writable_executable_segment_is_refused() {
    let bytes = Builder::new()
        .seg(READABLE | WRITABLE | EXECUTABLE, 0x40_0000, &[0xaa; 16], 16)
        .build();

    assert_eq!(Elf::parse(&bytes).err(), Some(Error::WritableAndExecutable),);
}

/// A segment whose contents run off the end of the file.
///
/// `p_offset` and `p_filesz` are attacker-controlled. A loader that trusts them reads
/// whatever is after the buffer and then **maps it into a process**.
#[test]
fn a_segment_that_runs_off_the_end_is_refused() {
    // memsz is a match for the lie, so `MemorySmallerThanFile` does not fire first and we
    // genuinely exercise the bounds check on the FILE.
    let mut b = Builder::new().seg(READABLE | EXECUTABLE, 0x40_0000, &[0xaa; 16], 0x1000_0000);
    b.lie_about_filesz = Some(0x1000_0000);
    assert_eq!(
        Elf::parse(&b.build()).err(),
        Some(Error::SegmentOutOfBounds)
    );
}

#[test]
fn an_offset_that_overflows_is_refused() {
    let mut b = Builder::new().seg(READABLE | EXECUTABLE, 0x40_0000, &[0xaa; 16], 16);
    b.lie_about_offset = Some(u64::MAX - 3); // p_offset + p_filesz wraps
    assert_eq!(
        Elf::parse(&b.build()).err(),
        Some(Error::SegmentOutOfBounds)
    );
}

#[test]
fn memsz_smaller_than_filesz_is_refused() {
    let bytes = Builder::new()
        .seg(READABLE | EXECUTABLE, 0x40_0000, &[0xaa; 16], 4) // memsz 4 < filesz 16
        .build();
    assert_eq!(Elf::parse(&bytes).err(), Some(Error::MemorySmallerThanFile));
}

/// A binary for a machine the crate does not name at all is refused. SPARC (2) goes through
/// [`unknown_machine`] in a `const`, so if it ever joins `KNOWN_MACHINES` this fails to compile
/// instead of asserting something false.
#[test]
fn a_binary_for_another_machine_is_refused() {
    const EM_SPARC: u16 = unknown_machine(2);
    let mut b = Builder::new().seg(READABLE | EXECUTABLE, 0x40_0000, &[0xaa; 16], 16);
    b.e_machine = EM_SPARC;
    assert_eq!(Elf::parse(&b.build()).err(), Some(Error::WrongMachine));
}

/// A binary for either of the other named architectures is refused: the check is symmetric.
/// [`FOREIGN_MACHINES`] rather than a written-out pair, which would be true only on one host.
#[test]
fn a_binary_for_the_other_supported_machine_is_refused() {
    for machine in FOREIGN_MACHINES {
        let mut b = Builder::new().seg(READABLE | EXECUTABLE, 0x40_0000, &[0xaa; 16], 16);
        b.e_machine = machine;
        assert_eq!(Elf::parse(&b.build()).err(), Some(Error::WrongMachine));
    }
}

/// `parse_for_machine` accepts the machine it is given and refuses every other, this build's own
/// included, so a tool on one machine validates a program for another with the same checks.
#[test]
fn a_named_machine_is_the_one_accepted() {
    for machine in KNOWN_MACHINES {
        let mut b = Builder::new().seg(READABLE | EXECUTABLE, 0x40_0000, &[0xaa; 16], 16);
        b.e_machine = machine;
        let bytes = b.build();
        for asked in KNOWN_MACHINES {
            assert_eq!(
                Elf::parse_for_machine(&bytes, asked).is_ok(),
                asked == machine,
                "file for {machine}, asked for {asked}"
            );
        }
    }
}

/// `FOREIGN_MACHINES` is every known machine except this build's. The test above cannot say so,
/// since every wrong number is refused: an array naming no architecture (`[0; _]`, `[1; _]`)
/// survived it in a mutation run.
#[test]
fn foreign_machines_is_every_known_machine_but_this_builds() {
    assert_eq!(FOREIGN_MACHINES.len(), KNOWN_MACHINES.len() - 1);
    for machine in KNOWN_MACHINES {
        assert_eq!(
            machine == NATIVE_MACHINE,
            !FOREIGN_MACHINES.contains(&machine),
            "{machine:#x} must be in exactly one of native and foreign"
        );
    }
}

/// **The derivation gives the same answer when it runs**, not only when the compiler folds it.
/// `FOREIGN_MACHINES` is evaluated in a `const`, so its build-time asserts are the real check, and
/// line coverage cannot see a `const` evaluation: without a runtime call, `foreign_machines` reads
/// as never executed. This runs it, and holds the result to the constant the rest of the suite
/// iterates.
#[test]
fn foreign_machines_derives_the_same_list_at_runtime() {
    assert_eq!(core::hint::black_box(foreign_machines)(), FOREIGN_MACHINES);
}

/// The compile-time guard hands back what it was given, the half a caller relies on. A constant
/// return survives every other test, because every number it could return is foreign too.
#[test]
fn a_machine_no_build_accepts_is_handed_back_unchanged() {
    const EM_SPARC: u16 = unknown_machine(2);
    assert_eq!(EM_SPARC, 2);
    assert_eq!(unknown_machine(0x3fff), 0x3fff);
}

/// The guard refuses a machine the crate names. Without a runtime rejection, a loop bound mutated so the
/// loop never runs (`i < len` as `==` or `>`) checks nothing and no test sees it.
#[test]
#[should_panic(expected = "some build accepts it")]
fn a_machine_this_tree_runs_on_is_refused_by_the_guard() {
    let _ = unknown_machine(core::hint::black_box(NATIVE_MACHINE));
}

/// A PIE expects a dynamic linker to relocate it. We are not one, and loading it as if we
/// were means jumping to an address that means nothing.
#[test]
fn a_position_independent_executable_is_refused() {
    let mut b = Builder::new().seg(READABLE | EXECUTABLE, 0x40_0000, &[0xaa; 16], 16);
    b.e_type = ET_DYN;
    assert_eq!(Elf::parse(&b.build()).err(), Some(Error::NeedsRelocation));
}

/// The entry point must be somewhere we can actually execute.
#[test]
fn an_entry_point_outside_every_executable_segment_is_refused() {
    let mut b = Builder::new().seg(READABLE | EXECUTABLE, 0x40_0000, &[0xaa; 16], 16);
    b.entry = 0x41_0000; // not in the code segment
    assert_eq!(
        Elf::parse(&b.build()).err(),
        Some(Error::EntryNotExecutable)
    );
}

/// An entry point past the file bytes of its code segment, in the zero-filled tail, would start
/// the program on zeros. The last file byte is still a valid entry.
#[test]
fn an_entry_point_in_the_zero_filled_tail_is_refused() {
    let tail = |entry| {
        let mut b = Builder::new().seg(READABLE | EXECUTABLE, 0x40_0000, &[0xaa; 16], 0x1000);
        b.entry = entry;
        Elf::parse(&b.build()).err()
    };
    assert_eq!(tail(0x40_0010), Some(Error::EntryNotExecutable));
    assert_eq!(tail(0x40_0fff), Some(Error::EntryNotExecutable));
    assert_eq!(tail(0x40_000f), None);
}

/// An entry point inside a segment that is readable but NOT executable.
#[test]
fn an_entry_point_in_a_data_segment_is_refused() {
    let mut b = Builder::new().seg(READABLE | WRITABLE, 0x40_0000, &[0xaa; 16], 16);
    b.entry = 0x40_0000;
    assert_eq!(
        Elf::parse(&b.build()).err(),
        Some(Error::EntryNotExecutable)
    );
}

/// **Execute-only code is legal, and its flags read back exactly.** Every other fixture sets
/// `READABLE`, so this is the only place the readable bit is ever *clear*: it proves the
/// unreachable check keys on "neither readable nor executable" rather than on readability
/// alone, and that `is_readable` tests its own bit (a mask read as `|` or `^` reports a
/// EXECUTABLE-only segment readable; both are wrong the same way, visible only here).
#[test]
fn an_execute_only_segment_is_accepted_and_is_not_readable() {
    let bytes = Builder::new()
        .seg(EXECUTABLE, 0x40_0000, &[0xaa; 16], 16)
        .build();
    let elf = Elf::parse(&bytes).expect("execute-only code is reachable");
    let seg = elf.segments().next().unwrap();
    assert!(seg.is_executable());
    assert!(!seg.is_readable());
    assert!(!seg.is_writable());
}

/// `e_phentsize` is checked against the size of a program header from both sides: one byte short
/// is refused, and a larger entry (which the format allows) is not refused for its size.
#[test]
fn the_program_header_entry_size_boundary_is_one_whole_header() {
    let mut bytes = good();
    bytes[54..56].copy_from_slice(&((PHDR_SIZE - 1) as u16).to_le_bytes());
    assert_eq!(
        Elf::parse(&bytes).err(),
        Some(Error::ProgramHeadersOutOfBounds)
    );
    let mut bytes = good();
    bytes[54..56].copy_from_slice(&((PHDR_SIZE + 8) as u16).to_le_bytes());
    assert_ne!(
        Elf::parse(&bytes).err(),
        Some(Error::ProgramHeadersOutOfBounds)
    );
}

/// `e_phentsize` smaller than a program header means the table entries we would read do not
/// exist as described. The builder always writes 56, so this patches the header bytes to lie.
#[test]
fn a_short_phentsize_is_refused() {
    let mut bytes = good();
    bytes[54..56].copy_from_slice(&8u16.to_le_bytes());
    assert_eq!(
        Elf::parse(&bytes).err(),
        Some(Error::ProgramHeadersOutOfBounds)
    );
}

/// The header-count cap is exclusive: exactly `MAXIMUM_PHNUM` (64) headers must still parse,
/// because the cap exists to stop a stalling count, not to shrink what a real file may hold.
#[test]
fn exactly_the_maximum_header_count_is_accepted() {
    let mut b = Builder::new();
    for i in 0..64u64 {
        // Distinct pages, so the overlap check stays quiet; the first segment holds the entry.
        b = b.seg(READABLE | EXECUTABLE, 0x40_0000 + i * 0x1000, &[0xaa; 8], 8);
    }
    let bytes = b.build();
    let elf = Elf::parse(&bytes).expect("64 headers is within the cap");
    assert_eq!(elf.segments().count(), 64);
}

/// The entry range is half-open: an entry at exactly `vaddr + memsz` is one past the last
/// byte of the segment, which is not a place execution can begin.
#[test]
fn an_entry_one_past_the_code_segment_is_refused() {
    let mut b = Builder::new().seg(READABLE | EXECUTABLE, 0x40_0000, &[0xaa; 16], 16);
    b.entry = 0x40_0010; // vaddr + memsz, the first address outside
    assert_eq!(
        Elf::parse(&b.build()).err(),
        Some(Error::EntryNotExecutable)
    );
}

/// Two segments in *adjacent* pages share an edge, not a page: `[0x400000, 0x401000)` and
/// `[0x401000, 0x402000)` must not be called an overlap. Both declaration orders, because the
/// half-open comparison has one strict `<` per side and each order exercises a different side.
#[test]
fn segments_in_adjacent_pages_are_accepted() {
    let ascending = Builder::new()
        .seg(READABLE | EXECUTABLE, 0x40_0000, &[0xaa; 16], 16)
        .seg(READABLE | WRITABLE, 0x40_1000, &[0xbb; 16], 16)
        .build();
    Elf::parse(&ascending).expect("adjacent pages, code first");

    let descending = Builder::new()
        .seg(READABLE | WRITABLE, 0x40_1000, &[0xbb; 16], 16)
        .seg(READABLE | EXECUTABLE, 0x40_0000, &[0xaa; 16], 16)
        .build();
    Elf::parse(&descending).expect("adjacent pages, code second");
}

/// Two segments one 4 KiB page apart share a 16 KiB page. `parse` accepts them, and a parse told
/// the loader maps 16 KiB pages refuses them, so the overlap check follows the page size given.
#[test]
fn the_overlap_check_follows_the_page_size_given() {
    let bytes = Builder::new()
        .seg(READABLE | EXECUTABLE, 0x40_0000, &[0xaa; 16], 16)
        .seg(READABLE | WRITABLE, 0x40_1000, &[0xbb; 16], 16)
        .build();
    Elf::parse(&bytes).expect("different 4 KiB pages");
    let sixteen = PageBytes::new(0x4000).unwrap();
    assert_eq!(
        Elf::parse_with(&bytes, NATIVE_MACHINE, sixteen).err(),
        Some(Error::SegmentsOverlap)
    );
    Elf::parse_with(&bytes, NATIVE_MACHINE, PageBytes::FOUR_KIBIBYTES).expect("as parse");
}

/// A header offset is returned only when the whole 56-byte header ends inside the table, and an
/// index or entry size large enough to wrap is `None` rather than a panic.
#[test]
fn a_program_header_offset_holds_a_whole_header() {
    assert_eq!(program_header_offset(64, 0, 56, 120), Some(64));
    assert_eq!(program_header_offset(64, 1, 56, 176), Some(120));
    assert_eq!(
        program_header_offset(64, 1, 56, 175),
        None,
        "one byte short"
    );
    assert_eq!(program_header_offset(0, 0, 56, 55), None);
    assert_eq!(program_header_offset(0, usize::MAX, 56, usize::MAX), None);
    assert_eq!(program_header_offset(usize::MAX, 0, 56, usize::MAX), None);
}

/// Pins the byte order of the field reader with a value whose two bytes differ. Every field
/// the parser checks happens to have a zero high byte in the other fixtures, so a reader that
/// picked up the wrong neighbouring byte would still pass them.
#[test]
fn u16le_reads_the_two_bytes_in_little_endian_order() {
    assert_eq!(u16le(&[0x00, 0xcd, 0xab], 1), 0xabcd);
}

#[test]
fn two_segments_in_the_same_page_are_refused() {
    let bytes = Builder::new()
        .seg(READABLE | EXECUTABLE, 0x40_0000, &[0xaa; 16], 16)
        .seg(READABLE | WRITABLE, 0x40_0800, &[0xbb; 16], 16) // same 4 KiB page
        .build();
    assert_eq!(Elf::parse(&bytes).err(), Some(Error::SegmentsOverlap));
}

#[test]
fn a_segment_whose_address_range_overflows_is_refused() {
    // filesz small (passes the file-bounds check), memsz enormous (passes memsz >= filesz), so
    // only the vaddr+memsz overflow guard stands between this and a panic in the loader.
    let mut b = Builder::new().seg(READABLE | EXECUTABLE, 0x40_0000, &[0xaa; 16], u64::MAX);
    b.entry = 0x40_0000;
    // Must return an Err, and crucially must NOT panic on the overflow.
    assert_eq!(Elf::parse(&b.build()).err(), Some(Error::AddressOverflow));
}

#[test]
fn too_many_program_headers_are_refused() {
    let mut b = Builder::new();
    for _ in 0..65 {
        b = b.seg(READABLE | EXECUTABLE, 0x40_0000, &[0xaa; 8], 8);
    }
    assert_eq!(
        Elf::parse(&b.build()).err(),
        Some(Error::TooManyProgramHeaders)
    );
}

#[test]
fn junk_is_refused() {
    assert_eq!(Elf::parse(&[]).err(), Some(Error::TooSmall));
    assert_eq!(Elf::parse(&[0u8; 64]).err(), Some(Error::BadMagic));

    let mut b = Builder::new().seg(READABLE | EXECUTABLE, 0x40_0000, &[0xaa; 16], 16);
    b.class = 1; // ELFCLASS32
    assert_eq!(Elf::parse(&b.build()).err(), Some(Error::Not64Bit));

    let mut b = Builder::new().seg(READABLE | EXECUTABLE, 0x40_0000, &[0xaa; 16], 16);
    b.data = 2; // ELFDATA2MSB
    assert_eq!(Elf::parse(&b.build()).err(), Some(Error::NotLittleEndian));
}

/// A shell script, a JPEG, and a flat kernel image are all not-an-ELF.
#[test]
fn a_file_that_is_not_an_elf_at_all_is_refused() {
    assert_eq!(
        Elf::parse(b"#!/bin/sh\necho hello\n#####################################################")
            .err(),
        Some(Error::BadMagic),
    );
}

/// A kernel image's shape, and the reason this field exists at all: `p_paddr` and
/// `p_vaddr` are separately readable, and neither is derived from the other. Every other test in
/// this file has them equal, which is exactly the hazard the field's doc comment names.
#[test]
fn a_physical_address_may_differ_from_the_virtual_one() {
    let bytes = Builder::new()
        .seg_at_paddr(READABLE | EXECUTABLE, 0x40_0000, 0x40_0000, &[0xaa; 16], 16)
        .seg_at_paddr(READABLE | WRITABLE, 0x16_5000, 0x8000, &[0xbb; 8], 4096)
        .build();
    let elf = Elf::parse(&bytes).expect("should parse");
    let segs: Vec<_> = elf.segments().collect();

    assert_eq!(segs[1].virtual_address, 0x16_5000);
    assert_eq!(segs[1].physical_address, 0x8000);
}

/// **Nothing validates `physical_address`, on purpose.** Every check in `parse` (bounds, overlap,
/// the entry point, the `p_vaddr + p_memsz` overflow guard) is about the virtual layout, because
/// that is what a loader that maps into a fresh address space reads. A physical address that
/// overlaps another segment's, or that is zero because the file never set the field, is a fact
/// about the file rather than an error, and a loader that places images physically owns that
/// judgment.
#[test]
fn a_nonsense_physical_address_is_not_a_parse_error() {
    let bytes = Builder::new()
        .seg_at_paddr(READABLE | EXECUTABLE, 0x40_0000, u64::MAX, &[0xaa; 16], 16)
        .seg_at_paddr(READABLE | WRITABLE, 0x41_0000, 0, &[0xbb; 8], 8)
        .build();
    let elf = Elf::parse(&bytes).expect("paddr is carried, not judged");
    let segs: Vec<_> = elf.segments().collect();

    assert_eq!(segs[0].physical_address, u64::MAX);
    assert_eq!(segs[1].physical_address, 0);
    // And `page_range` is the *virtual* range, which is the other half of the same trap.
    assert_eq!(
        segs[0].page_range(PageBytes::FOUR_KIBIBYTES),
        (0x40_0000, 0x40_1000)
    );
}

#[test]
fn page_range_covers_the_whole_segment() {
    let seg = Segment {
        virtual_address: 0x40_0800,
        physical_address: 0x40_0800,
        memory_bytes: 0x900,
        flags: READABLE,
        data: &[],
    };
    // 0x400800..0x401100 spans two pages: 0x400000 and 0x401000.
    assert_eq!(
        seg.page_range(PageBytes::FOUR_KIBIBYTES),
        (0x40_0000, 0x40_2000)
    );
}

/// The same segment rounds to the page size it is given: one 16 KiB page holds what took two
/// 4 KiB pages.
#[test]
fn page_range_rounds_to_the_page_size_given() {
    let seg = Segment {
        virtual_address: 0x40_0800,
        physical_address: 0x40_0800,
        memory_bytes: 0x900,
        flags: READABLE,
        data: &[],
    };
    let sixteen = PageBytes::new(0x4000).unwrap();
    assert_eq!(seg.page_range(sixteen), (0x40_0000, 0x40_4000));
    assert_eq!(PageBytes::FOUR_KIBIBYTES.get(), 4096);
    assert_eq!(PageBytes::new(1 << 63).map(PageBytes::get), Some(1 << 63));
    assert_eq!(PageBytes::new(0x3000), None);
}

/// A 64-bit offset or size the target cannot hold is refused as out of bounds, never truncated
/// onto other bytes, and the error types print, box and hash.
#[test]
fn errors_print_and_a_64_bit_field_is_never_truncated() {
    extern crate std;
    use std::string::ToString;
    let error: &dyn core::error::Error = &Error::SegmentOutOfBounds;
    assert!(error.to_string().contains("off the end"));
    let note: &dyn core::error::Error = &NoteError::Duplicate;
    assert!(note.to_string().contains("more than once"));
    // `memsz` too large for `usize` is not "less than" any `filesz` that fits.
    assert_ne!(
        check_segment_bounds(1 << 20, 0, 16, 0x40_0000, u64::MAX),
        Err(Error::MemorySmallerThanFile)
    );
}

/// Every refusal prints its own sentence, so a message never stands for two causes.
#[test]
fn every_refusal_prints_a_distinct_sentence() {
    extern crate std;
    use std::collections::HashSet;
    use std::string::ToString;
    let errors = [
        Error::TooSmall,
        Error::BadMagic,
        Error::Not64Bit,
        Error::NotLittleEndian,
        Error::BadVersion,
        Error::WrongMachine,
        Error::NeedsRelocation,
        Error::NotExecutable,
        Error::ProgramHeadersOutOfBounds,
        Error::SegmentOutOfBounds,
        Error::MemorySmallerThanFile,
        Error::WritableAndExecutable,
        Error::SegmentUnreachable,
        Error::SegmentsOverlap,
        Error::EntryNotExecutable,
        Error::AddressOverflow,
        Error::TooManyProgramHeaders,
    ];
    let notes = [
        NoteError::NotReadable,
        NoteError::SegmentOutOfBounds,
        NoteError::BadAlignment,
        NoteError::Truncated,
        NoteError::Duplicate,
    ];
    let mut seen = HashSet::new();
    for text in errors
        .iter()
        .map(ToString::to_string)
        .chain(notes.iter().map(ToString::to_string))
    {
        assert!(!text.is_empty());
        assert!(seen.insert(text));
    }
}
