//! **A loadable segment**: its permission bits, and what the loader reads off one.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

/// `p_type`: a segment the loader must actually put in memory. The only one we care about.
pub(crate) const PT_LOAD: u32 = 1;

/// `PF_X` in `p_flags`: the segment holds code.
///
/// Name: ratified 2026-11-21 (UTC).
#[doc(alias = "PF_X")]
pub const EXECUTABLE: u32 = 1;

/// `PF_W` in `p_flags`: the segment may be written.
///
/// Name: ratified 2026-11-21 (UTC).
#[doc(alias = "PF_W")]
pub const WRITABLE: u32 = 2;

/// `PF_R` in `p_flags`: the segment may be read.
///
/// Name: ratified 2026-11-21 (UTC).
#[doc(alias = "PF_R")]
pub const READABLE: u32 = 4;

/// One `PT_LOAD` segment: what to map, where, and how.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Segment<'a> {
    /// `p_vaddr`: the virtual address the program wants this at. Its choice, and the loader
    /// honours it or refuses.
    ///
    /// Name: ratified 2026-11-21 (UTC).
    #[doc(alias = "p_vaddr")]
    pub virtual_address: u64,

    /// `p_paddr`: where the segment wants to sit in physical memory, for a loader that places an
    /// image at fixed physical addresses (a firmware loader, a bootloader).
    ///
    /// A loader mapping into a fresh address space wants
    /// [`virtual_address`](Self::virtual_address) and never this. Most linkers write the two equal
    /// for a user program, which is the trap: a loader that reads the wrong one works until it
    /// meets a kernel image, whose segments are linked high and loaded low with no fixed offset
    /// between the two. Nothing in the parse checks it.
    ///
    /// Name: ratified 2026-11-21 (UTC).
    #[doc(alias = "p_paddr")]
    pub physical_address: u64,

    /// `p_memsz`: how many bytes of **memory** it occupies. May exceed `data.len()`.
    ///
    /// Name: ratified 2026-11-21 (UTC).
    #[doc(alias = "p_memsz")]
    pub memory_bytes: u64,

    /// `p_flags`: [`READABLE`], [`WRITABLE`] and [`EXECUTABLE`], and any other bits the file set.
    #[doc(alias = "p_flags")]
    pub flags: u32,

    /// The bytes from the file. `data.len()` is `p_filesz`, which can be less than
    /// [`memory_bytes`](Self::memory_bytes); the loader must zero the tail, which is `.bss`.
    #[doc(alias = "p_filesz")]
    pub data: &'a [u8],
}

impl Segment<'_> {
    /// Whether [`READABLE`] is set in [`flags`](Self::flags).
    pub fn is_readable(&self) -> bool {
        self.flags & READABLE != 0
    }
    /// Whether [`WRITABLE`] is set in [`flags`](Self::flags).
    pub fn is_writable(&self) -> bool {
        self.flags & WRITABLE != 0
    }
    /// Whether [`EXECUTABLE`] is set in [`flags`](Self::flags).
    pub fn is_executable(&self) -> bool {
        self.flags & EXECUTABLE != 0
    }

    /// The page-aligned range this segment touches: `[start, end)`, in pages of `page_bytes`.
    ///
    /// An end past `u64::MAX` saturates there, so the range is never inverted.
    pub fn page_range(&self, page_bytes: PageBytes) -> (u64, u64) {
        let page_bytes = page_bytes.get();
        let start = self.virtual_address & !(page_bytes - 1);
        // Saturating: `Segment` is `pub`, so this must be panic-free on a hostile `memory_bytes`
        // without `Elf::parse`'s overflow check. `PageBytes` rules out the zero and the non-power-of-two.
        let end = self
            .virtual_address
            .saturating_add(self.memory_bytes)
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
/// Name: ratified 2026-11-21 (UTC).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PageBytes(u64);

impl PageBytes {
    /// A 4 KiB page, the smallest page each of `aarch64`, `riscv64` and `x86_64` maps.
    ///
    /// Name: ratified 2026-11-21 (UTC).
    pub const FOUR_KIBIBYTES: PageBytes = PageBytes(4096);

    /// `bytes` as a page size, or `None` when it is zero or not a power of two.
    ///
    /// Name: ratified 2026-11-21 (UTC).
    pub const fn new(bytes: u64) -> Option<Self> {
        if bytes.is_power_of_two() {
            Some(Self(bytes))
        } else {
            None
        }
    }

    /// The page size in bytes.
    ///
    /// Name: ratified 2026-11-21 (UTC).
    pub const fn get(self) -> u64 {
        self.0
    }
}
