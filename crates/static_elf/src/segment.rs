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
    /// A loader mapping into a fresh address space wants [`vaddr`](Self::vaddr) and never this.
    /// Most linkers write `paddr == vaddr` for a user program, which is the trap: a loader that
    /// reads the wrong one works until it meets a kernel image, whose segments are linked high and
    /// loaded low with no fixed offset between the two. Nothing in the parse checks it.
    pub paddr: u64,

    /// How much **memory** it occupies. May exceed `data.len()`.
    pub memsz: u64,

    /// `PF_R | PF_W | PF_X`.
    pub flags: u32,

    /// The bytes from the file. `data.len()` is `p_filesz`, which can be less than `memsz`; the
    /// loader must zero the tail, which is `.bss`.
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

    /// The page-aligned range this segment touches: `[start, end)`, in pages of `page_bytes`.
    ///
    /// An end past `u64::MAX` saturates there, so the range is never inverted.
    pub fn page_range(&self, page_bytes: PageBytes) -> (u64, u64) {
        let page_bytes = page_bytes.get();
        let start = self.vaddr & !(page_bytes - 1);
        // Saturating: `Segment` is `pub`, so this must be panic-free on a hostile `memsz` without
        // `Elf::parse`'s overflow check. `PageBytes` rules out the zero and the non-power-of-two.
        let end = self
            .vaddr
            .saturating_add(self.memsz)
            .div_ceil(page_bytes)
            .saturating_mul(page_bytes);
        (start, end)
    }
}

/// The size of a page in bytes: a nonzero power of two, checked once when the value is made.
///
/// A page size of zero would divide by zero in [`Segment::page_range`], and one that is not a
/// power of two would return a range that is not page-aligned. Taking this type instead of a bare
/// `u64` makes both impossible without a check at every call.
///
/// ```
/// use static_elf::PageBytes;
/// const FRAME: PageBytes = PageBytes::new(16384).unwrap();
/// assert_eq!(FRAME.get(), 16384);
/// assert_eq!(PageBytes::new(0), None);
/// assert_eq!(PageBytes::new(3000), None);
/// ```
///
/// Name: provisional, minted 2026-11-16 (UTC) by milestone/1088-static-elf-outside-review-fixes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PageBytes(u64);

impl PageBytes {
    /// A 4 KiB page, the smallest page each of aarch64, riscv64 and x86_64 maps.
    ///
    /// Name: provisional, minted 2026-11-16 (UTC) by milestone/1088-static-elf-outside-review-fixes.
    pub const FOUR_KIBIBYTES: PageBytes = PageBytes(4096);

    /// `bytes` as a page size, or `None` when it is zero or not a power of two.
    ///
    /// Name: provisional, minted 2026-11-16 (UTC) by milestone/1088-static-elf-outside-review-fixes.
    pub const fn new(bytes: u64) -> Option<Self> {
        if bytes.is_power_of_two() {
            Some(Self(bytes))
        } else {
            None
        }
    }

    /// The page size in bytes.
    ///
    /// Name: provisional, minted 2026-11-16 (UTC) by milestone/1088-static-elf-outside-review-fixes.
    pub const fn get(self) -> u64 {
        self.0
    }
}
