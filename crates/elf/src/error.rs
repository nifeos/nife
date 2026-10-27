//! **Why a file was refused.**
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

/// Why [`Elf::parse`](crate::Elf::parse) refused a file.
///
/// # Examples
///
/// ```
/// use elf::{Elf, Error, FOREIGN_MACHINES, NATIVE_MACHINE, PF_R, PF_W, PF_X};
/// # fn image(flags: u32, entry: u64) -> Vec<u8> {
/// #     const EHDR: usize = 64; const PHDR: usize = 56;
/// #     let mut v = vec![0u8; EHDR + PHDR];
/// #     v[0..4].copy_from_slice(&[0x7f, b'E', b'L', b'F']);
/// #     v[4] = 2; v[5] = 1; v[6] = 1; // ELFCLASS64, ELFDATA2LSB, EV_CURRENT
/// #     v[16..18].copy_from_slice(&2u16.to_le_bytes()); // ET_EXEC
/// #     v[18..20].copy_from_slice(&NATIVE_MACHINE.to_le_bytes());
/// #     v[24..32].copy_from_slice(&entry.to_le_bytes()); // e_entry
/// #     v[32..40].copy_from_slice(&(EHDR as u64).to_le_bytes()); // e_phoff
/// #     v[54..56].copy_from_slice(&(PHDR as u16).to_le_bytes()); // e_phentsize
/// #     v[56..58].copy_from_slice(&1u16.to_le_bytes()); // e_phnum
/// #     let p = EHDR;
/// #     v[p..p + 4].copy_from_slice(&1u32.to_le_bytes()); // PT_LOAD
/// #     v[p + 4..p + 8].copy_from_slice(&flags.to_le_bytes());
/// #     v[p + 8..p + 16].copy_from_slice(&((EHDR + PHDR) as u64).to_le_bytes()); // p_offset
/// #     v[p + 16..p + 24].copy_from_slice(&0x40_0000u64.to_le_bytes()); // p_vaddr
/// #     v[p + 24..p + 32].copy_from_slice(&0x40_0000u64.to_le_bytes()); // p_paddr, equal here
/// #     v[p + 40..p + 48].copy_from_slice(&4096u64.to_le_bytes()); // p_memsz; p_filesz stays 0
/// #     v
/// # }
/// // W^X: a writable and executable segment is how a buffer overflow becomes code execution.
/// let bad = image(PF_R | PF_W | PF_X, 0x40_0000);
/// assert!(matches!(Elf::parse(&bad), Err(Error::WritableAndExecutable)));
///
/// // An entry point outside every executable segment is a program that cannot start.
/// let nowhere = image(PF_R | PF_X, 0x99_0000);
/// assert!(matches!(Elf::parse(&nowhere), Err(Error::EntryNotExecutable)));
///
/// // A foreign binary (which machines are foreign depends on the build).
/// let mut foreign = image(PF_R | PF_X, 0x40_0000);
/// foreign[18..20].copy_from_slice(&FOREIGN_MACHINES[0].to_le_bytes());
/// assert!(matches!(Elf::parse(&foreign), Err(Error::WrongMachine)));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    /// `e_machine` is not this build's machine: a riscv binary handed to the `aarch64` kernel, say,
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
    /// A segment that is both writable and executable: the W^X rule `paging::Flags` holds by
    /// having no `writable_and_executable()` constructor. See notes/elf/parser.md.
    WritableAndExecutable,
    /// A segment that is neither readable nor executable. Nothing can ever touch it.
    SegmentUnreachable,
    /// Two segments want the same page. Legal ELF, refused because
    /// `crates/user_mode_runtime/link.ld` page-aligns every segment. See notes/elf/parser.md.
    SegmentsOverlap,
    /// The entry point is not inside any executable segment. The program cannot start.
    EntryNotExecutable,
    /// `p_vaddr + p_memsz` overflows, caught so the entry check and page math cannot wrap.
    AddressOverflow,
    /// More than `MAX_PHNUM` (64) program headers, the bound on the O(n^2) overlap check.
    TooManyProgramHeaders,
}
