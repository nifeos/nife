use super::*;
extern crate std;
use std::vec;
use std::vec::Vec;

/// Build an ELF64 image by hand, so the tests can lie about anything they like. See
/// notes/elf/parser.md for why forging here beats booting a real one.
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
    /// program in this tree looks like. Use `seg_at_paddr` for the kernel-image shape.
    fn seg(self, flags: u32, vaddr: u64, bytes: &[u8], memsz: u64) -> Self {
        self.seg_at_paddr(flags, vaddr, vaddr, bytes, memsz)
    }

    /// A segment whose `p_paddr` and `p_vaddr` genuinely differ, the way the kernel image's
    /// `.ap_trampoline` does.
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
        .seg(PF_R | PF_X, 0x40_0000, &[0xaa; 16], 16)
        .seg(PF_R | PF_W, 0x41_0000, &[0xbb; 8], 4096) // memsz > filesz: .bss
        .build()
}

#[test]
fn a_good_binary_parses() {
    let bytes = good();
    let elf = Elf::parse(&bytes).expect("should parse");

    assert_eq!(elf.entry(), 0x40_0000);

    let segs: Vec<_> = elf.segments().collect();
    assert_eq!(segs.len(), 2);

    assert_eq!(segs[0].vaddr, 0x40_0000);
    assert_eq!(segs[0].paddr, 0x40_0000);
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
    assert_eq!(data.memsz, 4096, "memsz");
    assert!(
        data.memsz as usize > data.data.len(),
        "the loader must zero {} bytes the file does not contain",
        data.memsz as usize - data.data.len(),
    );
}

/// **W^X, refused at the door.**
///
/// An ELF can simply *ask* for a page that is both writable and executable, and a loader
/// that grants it has handed the program the thing every exploit wants. `paging::Flags` has
/// no `writable_and_executable()` constructor for the same reason; this is the check that
/// stops a file talking us into building one.
#[test]
fn a_writable_executable_segment_is_refused() {
    let bytes = Builder::new()
        .seg(PF_R | PF_W | PF_X, 0x40_0000, &[0xaa; 16], 16)
        .build();

    assert_eq!(Elf::parse(&bytes).err(), Some(Error::WritableAndExecutable),);
}

/// A segment whose contents run off the end of the file.
///
/// `p_offset` and `p_filesz` are attacker-controlled. A loader that trusts them reads
/// whatever is after the buffer and then **maps it into a process**.
#[test]
fn a_segment_that_runs_off_the_end_is_refused() {
    // memsz is a match for the lie, so `SegmentTruncated` does not fire first and we
    // genuinely exercise the bounds check on the FILE.
    let mut b = Builder::new().seg(PF_R | PF_X, 0x40_0000, &[0xaa; 16], 0x1000_0000);
    b.lie_about_filesz = Some(0x1000_0000);
    assert_eq!(
        Elf::parse(&b.build()).err(),
        Some(Error::SegmentOutOfBounds)
    );
}

#[test]
fn an_offset_that_overflows_is_refused() {
    let mut b = Builder::new().seg(PF_R | PF_X, 0x40_0000, &[0xaa; 16], 16);
    b.lie_about_offset = Some(u64::MAX - 3); // p_offset + p_filesz wraps
    assert_eq!(
        Elf::parse(&b.build()).err(),
        Some(Error::SegmentOutOfBounds)
    );
}

#[test]
fn memsz_smaller_than_filesz_is_refused() {
    let bytes = Builder::new()
        .seg(PF_R | PF_X, 0x40_0000, &[0xaa; 16], 4) // memsz 4 < filesz 16
        .build();
    assert_eq!(Elf::parse(&bytes).err(), Some(Error::SegmentTruncated));
}

/// A binary for a machine nife does not run at all is refused. SPARC (2) goes through
/// [`machine_no_nife_build_accepts`] in a `const`, so if it ever joins `KNOWN_MACHINES` this fails
/// to compile instead of asserting something false.
#[test]
fn a_binary_for_another_machine_is_refused() {
    const EM_SPARC: u16 = machine_no_nife_build_accepts(2);
    let mut b = Builder::new().seg(PF_R | PF_X, 0x40_0000, &[0xaa; 16], 16);
    b.e_machine = EM_SPARC;
    assert_eq!(Elf::parse(&b.build()).err(), Some(Error::WrongMachine));
}

/// A binary for either of the other nife architectures is refused: the check is symmetric.
/// [`FOREIGN_MACHINES`] rather than a written-out pair, which would be true only on one host.
#[test]
fn a_binary_for_the_other_supported_machine_is_refused() {
    for machine in FOREIGN_MACHINES {
        let mut b = Builder::new().seg(PF_R | PF_X, 0x40_0000, &[0xaa; 16], 16);
        b.e_machine = machine;
        assert_eq!(Elf::parse(&b.build()).err(), Some(Error::WrongMachine));
    }
}

/// `FOREIGN_MACHINES` is every known machine except this build's. The test above cannot say so,
/// since every wrong number is refused: an array naming no architecture (`[0; _]`, `[1; _]`)
/// survived it in a mutation run (milestone 326 (nobody has been assigned to turn a mutation score
/// upward)).
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
    const EM_SPARC: u16 = machine_no_nife_build_accepts(2);
    assert_eq!(EM_SPARC, 2);
    assert_eq!(machine_no_nife_build_accepts(0x3fff), 0x3fff);
}

/// The guard refuses a machine nife runs. Without a runtime rejection, a loop bound mutated so the
/// loop never runs (`i < len` as `==` or `>`) checks nothing and no test sees it.
#[test]
#[should_panic(expected = "some build accepts it")]
fn a_machine_this_tree_runs_on_is_refused_by_the_guard() {
    let _ = machine_no_nife_build_accepts(core::hint::black_box(NATIVE_MACHINE));
}

/// A PIE expects a dynamic linker to relocate it. We are not one, and loading it as if we
/// were means jumping to an address that means nothing.
#[test]
fn a_position_independent_executable_is_refused() {
    let mut b = Builder::new().seg(PF_R | PF_X, 0x40_0000, &[0xaa; 16], 16);
    b.e_type = ET_DYN;
    assert_eq!(Elf::parse(&b.build()).err(), Some(Error::NeedsRelocation));
}

/// The entry point must be somewhere we can actually execute.
#[test]
fn an_entry_point_outside_every_executable_segment_is_refused() {
    let mut b = Builder::new().seg(PF_R | PF_X, 0x40_0000, &[0xaa; 16], 16);
    b.entry = 0x41_0000; // not in the code segment
    assert_eq!(
        Elf::parse(&b.build()).err(),
        Some(Error::EntryNotExecutable)
    );
}

/// An entry point inside a segment that is readable but NOT executable.
#[test]
fn an_entry_point_in_a_data_segment_is_refused() {
    let mut b = Builder::new().seg(PF_R | PF_W, 0x40_0000, &[0xaa; 16], 16);
    b.entry = 0x40_0000;
    assert_eq!(
        Elf::parse(&b.build()).err(),
        Some(Error::EntryNotExecutable)
    );
}

/// **Execute-only code is legal, and its flags read back exactly.** Every other fixture sets
/// `PF_R`, so this is the only place the readable bit is ever *clear*: it proves the
/// unreachable check keys on "neither readable nor executable" rather than on readability
/// alone, and that `is_readable` tests its own bit (a mask read as `|` or `^` reports a
/// PF_X-only segment readable; both are wrong the same way, visible only here).
#[test]
fn an_execute_only_segment_is_accepted_and_is_not_readable() {
    let bytes = Builder::new().seg(PF_X, 0x40_0000, &[0xaa; 16], 16).build();
    let elf = Elf::parse(&bytes).expect("execute-only code is reachable");
    let seg = elf.segments().next().unwrap();
    assert!(seg.is_executable());
    assert!(!seg.is_readable());
    assert!(!seg.is_writable());
}

/// `e_phentsize` smaller than a program header means the table entries we would read do not
/// exist as described. The builder always writes 56, so this patches the header bytes to lie.
#[test]
fn a_short_phentsize_is_refused() {
    let mut bytes = good();
    bytes[54..56].copy_from_slice(&8u16.to_le_bytes());
    assert_eq!(Elf::parse(&bytes).err(), Some(Error::BadProgramHeaders));
}

/// The header-count cap is exclusive: exactly `MAX_PHNUM` (64) headers must still parse,
/// because the cap exists to stop a stalling count, not to shrink what a real file may hold.
#[test]
fn exactly_the_maximum_header_count_is_accepted() {
    let mut b = Builder::new();
    for i in 0..64u64 {
        // Distinct pages, so the overlap check stays quiet; the first segment holds the entry.
        b = b.seg(PF_R | PF_X, 0x40_0000 + i * 0x1000, &[0xaa; 8], 8);
    }
    let bytes = b.build();
    let elf = Elf::parse(&bytes).expect("64 headers is within the cap");
    assert_eq!(elf.segments().count(), 64);
}

/// The entry range is half-open: an entry at exactly `vaddr + memsz` is one past the last
/// byte of the segment, which is not a place execution can begin.
#[test]
fn an_entry_one_past_the_code_segment_is_refused() {
    let mut b = Builder::new().seg(PF_R | PF_X, 0x40_0000, &[0xaa; 16], 16);
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
        .seg(PF_R | PF_X, 0x40_0000, &[0xaa; 16], 16)
        .seg(PF_R | PF_W, 0x40_1000, &[0xbb; 16], 16)
        .build();
    Elf::parse(&ascending).expect("adjacent pages, code first");

    let descending = Builder::new()
        .seg(PF_R | PF_W, 0x40_1000, &[0xbb; 16], 16)
        .seg(PF_R | PF_X, 0x40_0000, &[0xaa; 16], 16)
        .build();
    Elf::parse(&descending).expect("adjacent pages, code second");
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
        .seg(PF_R | PF_X, 0x40_0000, &[0xaa; 16], 16)
        .seg(PF_R | PF_W, 0x40_0800, &[0xbb; 16], 16) // same 4 KiB page
        .build();
    assert_eq!(Elf::parse(&bytes).err(), Some(Error::SegmentsOverlap));
}

#[test]
fn a_segment_whose_address_range_overflows_is_refused() {
    // filesz small (passes the file-bounds check), memsz enormous (passes memsz >= filesz), so
    // only the vaddr+memsz overflow guard stands between this and a kernel panic.
    let mut b = Builder::new().seg(PF_R | PF_X, 0x40_0000, &[0xaa; 16], u64::MAX);
    b.entry = 0x40_0000;
    // Must return an Err, and crucially must NOT panic on the overflow.
    assert_eq!(Elf::parse(&b.build()).err(), Some(Error::AddressOverflow));
}

#[test]
fn too_many_program_headers_are_refused() {
    let mut b = Builder::new();
    for _ in 0..65 {
        b = b.seg(PF_R | PF_X, 0x40_0000, &[0xaa; 8], 8);
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

    let mut b = Builder::new().seg(PF_R | PF_X, 0x40_0000, &[0xaa; 16], 16);
    b.class = 1; // ELFCLASS32
    assert_eq!(Elf::parse(&b.build()).err(), Some(Error::Not64Bit));

    let mut b = Builder::new().seg(PF_R | PF_X, 0x40_0000, &[0xaa; 16], 16);
    b.data = 2; // ELFDATA2MSB
    assert_eq!(Elf::parse(&b.build()).err(), Some(Error::NotLittleEndian));
}

/// A shell script, a JPEG, and the kernel's own flat image are all not-an-ELF.
#[test]
fn a_file_that_is_not_an_elf_at_all_is_refused() {
    assert_eq!(
        Elf::parse(b"#!/bin/sh\necho hello\n#####################################################")
            .err(),
        Some(Error::BadMagic),
    );
}

/// The kernel image's own shape, and the reason this field exists at all: `p_paddr` and
/// `p_vaddr` are separately readable, and neither is derived from the other. Every other test in
/// this file has them equal, which is exactly the hazard the field's doc comment names.
#[test]
fn a_physical_address_may_differ_from_the_virtual_one() {
    let bytes = Builder::new()
        .seg_at_paddr(PF_R | PF_X, 0x40_0000, 0x40_0000, &[0xaa; 16], 16)
        .seg_at_paddr(PF_R | PF_W, 0x16_5000, 0x8000, &[0xbb; 8], 4096)
        .build();
    let elf = Elf::parse(&bytes).expect("should parse");
    let segs: Vec<_> = elf.segments().collect();

    assert_eq!(segs[1].vaddr, 0x16_5000);
    assert_eq!(segs[1].paddr, 0x8000);
}

/// **Nothing validates `paddr`, on purpose.** Every check in `parse` (bounds, overlap, the entry
/// point, the `vaddr + memsz` overflow guard) is about the virtual layout, because that is what
/// this kernel's loader maps. A physical address that overlaps another segment's, or that is
/// zero because the file never set the field, is a fact about the file rather than an error, and
/// a loader that places images physically owns that judgment.
#[test]
fn a_nonsense_physical_address_is_not_a_parse_error() {
    let bytes = Builder::new()
        .seg_at_paddr(PF_R | PF_X, 0x40_0000, u64::MAX, &[0xaa; 16], 16)
        .seg_at_paddr(PF_R | PF_W, 0x41_0000, 0, &[0xbb; 8], 8)
        .build();
    let elf = Elf::parse(&bytes).expect("paddr is carried, not judged");
    let segs: Vec<_> = elf.segments().collect();

    assert_eq!(segs[0].paddr, u64::MAX);
    assert_eq!(segs[1].paddr, 0);
    // And `page_range` is the *virtual* range, which is the other half of the same trap.
    assert_eq!(segs[0].page_range(4096), (0x40_0000, 0x40_1000));
}

#[test]
fn page_range_covers_the_whole_segment() {
    let seg = Segment {
        vaddr: 0x40_0800,
        paddr: 0x40_0800,
        memsz: 0x900,
        flags: PF_R,
        data: &[],
    };
    // 0x400800..0x401100 spans two pages: 0x400000 and 0x401000.
    assert_eq!(seg.page_range(4096), (0x40_0000, 0x40_2000));
}
