//! **The seam between the shared walk and a hardware page-table format**, the address-space
//! halves it splits on, and the user-VA gate built from them.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

/// Which half of the address space a set of tables serves: the low half (user) or the high half
/// (kernel). The bit boundary between them is per-format ([`PageFormat::SPLIT_SHIFT`]); this enum is
/// just the marker, and the format supplies the geometry via [`PageFormat::is_in_half`] and
/// [`PageFormat::half_base`].
///
/// **The top bits of a virtual address are not translated**: they select which set of tables to
/// use (aarch64 `TTBR0`/`TTBR1`; on RISC-V one `satp` per space, the sign-extended top bit telling
/// the halves apart; on x86 one `CR3` per space, with the sign-extended bit 47 telling the halves
/// apart), and the index comes from the lower bits identically for both halves. A
/// higher-half kernel is a separate set of tables. See `nifeos/record:notes/page-tables.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Half {
    /// The low half: user space.
    Low,
    /// The high half: the kernel.
    High,
}

/// **The user-VA gate: may a user process ask for a mapping at `virtual_address` at all?**
///
/// True exactly when `virtual_address` is page-aligned and in the low half for format `F`, so an admitted
/// request walks only the process's own tables (the halves are proved disjoint per format). The
/// system call layer runs it before a page is retyped for a user `MAP` request.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane/1104-renames-package.
///
/// # Examples
///
/// A conjunction: the aligned high-half address is what tells `&&` from `||`.
///
/// ```
/// use page_tables::{Aarch64, PAGE_SIZE, is_user_page_virtual_address};
///
/// assert!(is_user_page_virtual_address::<Aarch64>(0x40_0000));
/// assert!(!is_user_page_virtual_address::<Aarch64>(0x40_0001)); // not page-aligned
/// assert!(!is_user_page_virtual_address::<Aarch64>(0xffff_0000_0000_0000)); // aligned, and the kernel's half
/// assert_eq!(PAGE_SIZE, 4096);
/// ```
pub fn is_user_page_virtual_address<F: PageFormat>(virtual_address: u64) -> bool {
    F::is_in_half(Half::Low, virtual_address) && virtual_address.is_multiple_of(PAGE_SIZE)
}

/// **The seam between the shared walk and a hardware page-table format.**
///
/// [`Mapper`] descends the levels, allocates intermediate tables, and writes a leaf, using nothing
/// architecture-specific except what a format supplies here: how many levels, where the halves
/// split, and the four bit operations (is-present, extract-address, encode-a-table-pointer,
/// encode-and-decode-a-leaf). Every format here is 512-entry / 4 KiB / 4 KiB, so [`ENTRIES`] and
/// [`PAGE_SIZE`] are shared and not part of the trait.
pub trait PageFormat {
    /// Translation levels. aarch64: 4. Sv39: 3. `Ia32e`, `Vtd` and `AmdVi`: 4.
    const LEVELS: usize;

    /// The bit position that splits the low (user) half from the high (kernel) half. aarch64: 48
    /// (bits 63:48 select the half). Sv39: 38 (bits 63:38, the sign-extended top VA bit and above). `Ia32e`: 47 (bit 47 sign-extended
    /// through bit 63). `Vtd` and `AmdVi`: 47 as a ceiling and not a split, since I/O virtual
    /// addresses have no halves: every address below 128 TiB counts as the low half.
    const SPLIT_SHIFT: u32;

    /// Is this entry present (valid)?
    fn is_present(entry: u64) -> bool;

    /// The physical address a table-pointer or leaf entry refers to.
    fn entry_physical_address(entry: u64) -> u64;

    /// Encode an intermediate entry pointing at `physical_address` (a next-level table). No permissions: a leaf
    /// is the single source of truth for access rights, so intermediate entries carry none.
    ///
    /// `level` is the level of the table this entry is written INTO, counted the way
    /// [`index`](Self::index) counts it (0 = the top), so the table `physical_address` names is at `level + 1`.
    /// Only AMD-Vi reads it: its `NextLevel` field must name the level of the table it points at,
    /// and a wrong value is a skipped level. Required, for the reason
    /// [`block_entry`](Self::block_entry) gives.
    fn table_entry(physical_address: u64, level: usize) -> u64;

    /// Encode a leaf entry mapping physical `physical_address` with `flags`.
    fn leaf_entry(physical_address: u64, flags: Flags) -> u64;

    /// Decode a leaf entry's permission/attribute bits back into portable [`Flags`]. Works on a
    /// block as well as a page: every format keeps its permission bits in the same positions at
    /// every level.
    fn leaf_flags(entry: u64) -> Flags;

    /// **Encode a block**: a leaf at the level `size` names (one level up for 2 MiB, two for
    /// 1 GiB), mapping the naturally aligned span at `physical_address` with `flags`. `None` when this format has
    /// no leaf of that size, which is how a format declines rather than being mis-encoded.
    ///
    /// Given [`PageSize::FourKibibytes`] this is [`leaf_entry`](Self::leaf_entry). Required, with no
    /// default: `None` would let a new format silently decline blocks, and reusing `leaf_entry`
    /// would write a table pointer on aarch64 (`0b11` at L2 is a table; a block is `0b01`).
    fn block_entry(physical_address: u64, flags: Flags, size: PageSize) -> Option<u64>;

    /// **Is this present entry, found above the bottom level, a block rather than a table
    /// pointer?** Only ever asked of an entry above the bottom level: at the bottom every present
    /// entry is a page, and on x86 the bit this reads there means something else entirely (PAT).
    fn is_block(entry: u64) -> bool;

    /// Which 9-bit slice of `virtual_address` selects an entry at `level` (0 = the top level). The top level
    /// covers the highest translated bits; each lower level shifts down by 9. Derived from
    /// [`LEVELS`](Self::LEVELS) so it is right for both a 4- and a 3-level walk.
    fn index(virtual_address: u64, level: usize) -> usize {
        let shift = 9 * (Self::LEVELS - 1 - level) as u32 + 12;
        ((virtual_address >> shift) & 0x1ff) as usize
    }

    /// Does `virtual_address` lie in `half` for this format? The top bits (above [`SPLIT_SHIFT`](Self::SPLIT_SHIFT))
    /// must be all-zero for the low half or all-one for the high half; anything between is
    /// non-canonical and faults.
    fn is_in_half(half: Half, virtual_address: u64) -> bool {
        let top = virtual_address >> Self::SPLIT_SHIFT;
        match half {
            Half::Low => top == 0,
            Half::High => top == (u64::MAX >> Self::SPLIT_SHIFT),
        }
    }

    /// The base virtual address of `half` (0 for the low half; the sign-extended high base for the
    /// high half). Exists to make a reader's mental model explicit; the walk does not use it.
    fn half_base(half: Half) -> u64 {
        match half {
            Half::Low => 0,
            Half::High => (u64::MAX >> Self::SPLIT_SHIFT) << Self::SPLIT_SHIFT,
        }
    }
}
