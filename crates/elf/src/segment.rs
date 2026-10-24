//! **A loadable segment**: its `p_flags`, and what the loader reads off one.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

/// `p_type`: a segment the loader must actually put in memory. The only one we care about.
pub(crate) const PT_LOAD: u32 = 1;

/// `p_flags`.
pub const PF_X: u32 = 1;

/// `p_flags`: writable.
pub const PF_W: u32 = 2;

/// `p_flags`: readable.
pub const PF_R: u32 = 4;

/// One `PT_LOAD` segment: what to map, where, and how.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Segment<'a> {
    /// The virtual address the program wants this at. Its choice, and we honour it or refuse.
    pub vaddr: u64,

    /// `p_paddr`: where the segment wants to sit in physical memory, for a loader that places an
    /// image at fixed physical addresses (a firmware loader, a bootloader).
    ///
    /// A loader mapping into a fresh address space, such as `kernel/src/user.rs`, wants
    /// [`vaddr`](Self::vaddr) and never this. Every user program here has `paddr == vaddr`; the
    /// kernel image does not, and no fixed offset relates the two (`kernel/link-x86_64.ld`). A plain
    /// `u64` until milestone 200 (a virtual address and a physical address stop being the same type).
    /// See notes/elf.md ("`p_paddr` is not `p_vaddr`, and the trap is that it usually is").
    pub paddr: u64,

    /// How much **memory** it occupies. May exceed `data.len()`.
    pub memsz: u64,

    /// `PF_R | PF_W | PF_X`.
    pub flags: u32,

    /// The bytes from the file. `data.len()` is `p_filesz`, which can be less than `memsz`; the
    /// loader must zero the tail, which is `.bss`. See notes/elf.md.
    pub data: &'a [u8],
}

impl Segment<'_> {
    /// Whether `PF_R` is set in [`flags`](Self::flags).
    pub fn is_readable(&self) -> bool {
        self.flags & PF_R != 0
    }
    /// Whether `PF_W` is set in [`flags`](Self::flags).
    pub fn is_writable(&self) -> bool {
        self.flags & PF_W != 0
    }
    /// Whether `PF_X` is set in [`flags`](Self::flags).
    pub fn is_executable(&self) -> bool {
        self.flags & PF_X != 0
    }

    /// The page-aligned range this segment touches: `[start, end)`.
    pub fn page_range(&self, page_size: u64) -> (u64, u64) {
        let start = self.vaddr & !(page_size - 1);
        // Saturating: `Segment` is `pub`, so this must be panic-free on a hostile `memsz` without
        // `Elf::parse`'s overflow check.
        let end = self
            .vaddr
            .saturating_add(self.memsz)
            .div_ceil(page_size)
            .saturating_mul(page_size);
        (start, end)
    }
}
