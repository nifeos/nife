//! **Why a file was refused.**
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

/// Why [`Elf::parse`](crate::Elf::parse) refused a file.
///
/// # Examples
///
/// ```
/// use static_elf::*;
/// # fn image(flags: u32, entry: u64) -> Vec<u8> {
/// #     const EHDR: usize = 64; const PHDR: usize = 56;
/// #     let mut v = vec![0u8; EHDR + PHDR + 16]; // 16 bytes of segment body
/// #     v[0..4].copy_from_slice(&[0x7f, b'E', b'L', b'F']);
/// #     v[4] = 2; v[5] = 1; v[6] = 1; // ELFCLASS64, ELFDATA2LSB, EV_CURRENT
/// #     v[16..18].copy_from_slice(&2u16.to_le_bytes()); // ET_EXEC
/// #     v[18..20].copy_from_slice(&NATIVE_MACHINE.to_le_bytes());
/// #     v[24..32].copy_from_slice(&entry.to_le_bytes()); // e_entry
/// #     v[32..40].copy_from_slice(&(EHDR as u64).to_le_bytes()); // e_phoff
/// #     v[54..56].copy_from_slice(&(PHDR as u16).to_le_bytes()); v[56] = 1; // e_phentsize, e_phnum
/// #     let p = EHDR;
/// #     v[p..p + 4].copy_from_slice(&1u32.to_le_bytes()); // PT_LOAD
/// #     v[p + 4..p + 8].copy_from_slice(&flags.to_le_bytes());
/// #     v[p + 8..p + 16].copy_from_slice(&((EHDR + PHDR) as u64).to_le_bytes()); // p_offset
/// #     v[p + 16..p + 24].copy_from_slice(&0x40_0000u64.to_le_bytes()); // p_vaddr
/// #     v[p + 24..p + 32].copy_from_slice(&0x40_0000u64.to_le_bytes()); // p_paddr, equal here
/// #     v[p + 32..p + 48].copy_from_slice(&[16u64.to_le_bytes(), 4096u64.to_le_bytes()].concat());
/// #     v
/// # }
/// // W^X: a writable and executable segment is how a buffer overflow becomes code execution.
/// let bad = image(READABLE | WRITABLE | EXECUTABLE, 0x40_0000);
/// assert!(matches!(Elf::parse(&bad), Err(Error::WritableAndExecutable)));
///
/// // An entry point outside every executable segment is a program that cannot start.
/// let nowhere = image(READABLE | EXECUTABLE, 0x99_0000);
/// assert!(matches!(Elf::parse(&nowhere), Err(Error::EntryNotExecutable)));
///
/// // A binary for another machine than this build's, which `parse_for_machine` accepts by name.
/// let other = if NATIVE_MACHINE == MACHINE_X86_64 { MACHINE_AARCH64 } else { MACHINE_X86_64 };
/// let mut foreign = image(READABLE | EXECUTABLE, 0x40_0000);
/// foreign[18..20].copy_from_slice(&other.to_le_bytes());
/// assert!(matches!(Elf::parse(&foreign), Err(Error::WrongMachine)));
/// assert!(Elf::parse_for_machine(&foreign, other).is_ok());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Error {
    /// Smaller than an ELF header. Not even worth looking at.
    TooSmall,
    /// No `\x7fELF`.
    BadMagic,
    /// 32-bit. We do not have a 32-bit anything.
    Not64Bit,
    /// Big-endian. `aarch64` can be, and ours is not.
    NotLittleEndian,
    /// `e_version` is not 1.
    BadVersion,
    /// `e_machine` is not the machine asked for: a riscv binary handed to an `aarch64` loader, say,
    /// caught here rather than as an illegal-instruction fault when the program starts.
    WrongMachine,
    /// A PIE or shared object. It expects a dynamic linker to relocate it. We are not one.
    NeedsRelocation,
    /// Not an executable at all (a relocatable object, a core dump).
    NotExecutable,
    /// The program header table runs off the end of the file.
    BadProgramHeaders,
    /// A segment's file contents run off the end of the file. `p_offset + p_filesz` is hostile,
    /// and a loader that trusted it would map whatever follows the buffer into a process.
    SegmentOutOfBounds,
    /// `p_memsz < p_filesz`: the segment claims to occupy less memory than it has bytes.
    SegmentTruncated,
    /// A segment that is both writable and executable. A page that is both is how a buffer
    /// overflow becomes code execution, so a loader holding W^X refuses the file asking for one.
    WritableAndExecutable,
    /// A segment that is neither readable nor executable. Nothing can ever touch it.
    SegmentUnreachable,
    /// Two segments want the same page, on pages of the size the parse was given (4 KiB unless
    /// [`Elf::parse_with`](crate::Elf::parse_with) named another). Legal ELF, and common when
    /// `.text` and `.rodata` share a page, but refused: a loader that gives each segment its own
    /// permissions cannot give one page two, so a linker script for this loader page-aligns every
    /// segment.
    SegmentsOverlap,
    /// The entry point is not inside the file bytes of any executable segment. Outside every one,
    /// the program cannot start. Inside one but past its `p_filesz`, in the zero-filled tail, it
    /// starts by executing zeros, which faults on every architecture this crate reads.
    EntryNotExecutable,
    /// `p_vaddr + p_memsz` overflows, caught so the entry check and page math cannot wrap.
    AddressOverflow,
    /// More than `MAXIMUM_PHNUM` (64) program headers, the bound on the O(n^2) overlap check.
    TooManyProgramHeaders,
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Error::TooSmall => "smaller than an ELF header",
            Error::BadMagic => "not an ELF file",
            Error::Not64Bit => "not a 64-bit ELF file",
            Error::NotLittleEndian => "not a little-endian ELF file",
            Error::BadVersion => "an ELF version other than 1",
            Error::WrongMachine => "built for another machine",
            Error::NeedsRelocation => {
                "a position-independent or shared object, which needs relocation"
            }
            Error::NotExecutable => "not an executable",
            Error::BadProgramHeaders => "the program header table runs off the end of the file",
            Error::SegmentOutOfBounds => "a segment's file contents run off the end of the file",
            Error::SegmentTruncated => "a segment claims less memory than it has bytes on disk",
            Error::WritableAndExecutable => "a segment is both writable and executable",
            Error::SegmentUnreachable => "a segment is neither readable nor executable",
            Error::SegmentsOverlap => "two segments want the same page",
            Error::EntryNotExecutable => "the entry point is not inside an executable segment",
            Error::AddressOverflow => "a segment's address range overflows",
            Error::TooManyProgramHeaders => "more program headers than the parser accepts",
        })
    }
}

impl core::error::Error for Error {}
