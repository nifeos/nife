//! The RISC-V Sv39 format: three levels, 4 KiB pages, 39-bit virtual addresses. A PTE is a leaf
//! when any of R/W/X is set, and one RSW bit (9:8) carries "device memory", which base Sv39 lacks.
//!
//! ```text
//!  38      30 29      21 20      12 11         0
//! ┌──────────┬──────────┬──────────┬────────────┐
//! │ VPN[2]   │ VPN[1]   │ VPN[0]   │   offset   │
//! │  9 bits  │  9 bits  │  9 bits  │  12 bits   │
//! └──────────┴──────────┴──────────┴────────────┘
//! ```

use crate::{
    CAP_DEVICE, CAP_GLOBAL, CAP_KERNEL_EXEC, CAP_USER, CAP_USER_EXEC, CAP_WRITE, Flags, PageFormat,
    PageSize,
};

const V: u64 = 1 << 0; // Valid
const R: u64 = 1 << 1; // Readable
const W: u64 = 1 << 2; // Writable
const X: u64 = 1 << 3; // eXecutable
const U: u64 = 1 << 4; // User-accessible
const G: u64 = 1 << 5; // Global
const A: u64 = 1 << 6; // Accessed
const D: u64 = 1 << 7; // Dirty

/// Bit 8, one of the two RSW (reserved-for-software) bits. We use it to record device-typed memory,
/// which base Sv39 has no PTE encoding for (real hardware would use Svpbmt's bits 62:61). On QEMU's
/// `virt` the memory type actually comes from the physical address, so this bit is bookkeeping that
/// keeps the portable [`Flags`] round-trip exact rather than something the hardware reads.
const SW_DEVICE: u64 = 1 << 8;

/// The PPN occupies bits [53:10] of a PTE: 44 bits.
const PPN_SHIFT: u32 = 10;
const PPN_MASK: u64 = (1 << 44) - 1;

/// The RISC-V Sv39 three-level page-table format.
pub struct Sv39;

impl Sv39 {
    /// Encode the permission/attribute bits (not the PPN or V) for a leaf with `flags`. A leaf is
    /// always readable (R), and we set A/D eagerly so the hardware never faults to update them (the
    /// Sv39 analog of always setting aarch64's Access Flag). D is set only for writable pages.
    const fn attrs(flags: Flags) -> u64 {
        let mut bits = R | A;
        if flags.is_writable() {
            bits |= W | D;
        }
        // Sv39 has one execute bit; the U bit decides at which privilege it applies (U-mode can
        // execute a U page, S-mode a non-U page, and never the other's). So the aarch64 PXN/UXN
        // split maps to X together with U.
        if flags.is_user_executable() || flags.is_kernel_executable() {
            bits |= X;
        }
        if flags.is_user_accessible() {
            bits |= U;
        }
        if flags.is_global() {
            bits |= G;
        }
        if flags.is_device() {
            bits |= SW_DEVICE;
        }
        bits
    }
}

impl PageFormat for Sv39 {
    const LEVELS: usize = 3;
    const SPLIT_SHIFT: u32 = 38;

    fn is_present(entry: u64) -> bool {
        entry & V != 0
    }

    fn entry_pa(entry: u64) -> u64 {
        ((entry >> PPN_SHIFT) & PPN_MASK) << 12
    }

    fn table_entry(pa: u64, _level: usize) -> u64 {
        // A pointer PTE: PPN plus V, and R=W=X=0 (that zero is what marks it a pointer, not a leaf).
        (((pa >> 12) & PPN_MASK) << PPN_SHIFT) | V
    }

    fn leaf_entry(pa: u64, flags: Flags) -> u64 {
        (((pa >> 12) & PPN_MASK) << PPN_SHIFT) | V | Self::attrs(flags)
    }

    fn leaf_flags(entry: u64) -> Flags {
        let mut caps = 0;
        if entry & W != 0 {
            caps |= CAP_WRITE;
        }
        if entry & U != 0 {
            caps |= CAP_USER;
        }
        if entry & X != 0 {
            // Execute permission belongs to whichever privilege the U bit names.
            if entry & U != 0 {
                caps |= CAP_USER_EXEC;
            } else {
                caps |= CAP_KERNEL_EXEC;
            }
        }
        if entry & G != 0 {
            caps |= CAP_GLOBAL;
        }
        if entry & SW_DEVICE != 0 {
            caps |= CAP_DEVICE;
        }
        Flags::from_caps(caps)
    }

    /// A megapage (2 MiB, at level 1) or gigapage (1 GiB, at level 0): **the same leaf encoding at
    /// a higher level**, since Sv39 marks a leaf by R/W/X rather than by position. What changes is
    /// the PPN: its low 9 (megapage) or 18 (gigapage) bits must be zero, or the hardware raises a
    /// misaligned-superpage page fault, so the address is masked to the block's alignment.
    fn block_entry(pa: u64, flags: Flags, size: PageSize) -> Option<u64> {
        Some(Self::leaf_entry(pa & !(size.bytes() - 1), flags))
    }

    /// R, W or X set: a leaf, at whatever level. All three clear: a pointer.
    fn is_block(entry: u64) -> bool {
        entry & (R | W | X) != 0
    }
}

/// **Sv39 with T-Head's memory-type bits** (`XTheadMae`): the format a C906 or C910 walks once
/// firmware has set `MAEE` in `th.mxstatus`, as OpenSBI does on the TH1520. The core then reads
/// PTE bits 63:59, reserved in standard Sv39, on every leaf (SO, C, B, SH below; SEC unused), and
/// all five clear is undefined for normal memory. The values are Linux's (`_PAGE_PMA_THEAD`,
/// `_PAGE_IO_THEAD`): memory is C, B and SH, a device SO and SH, and pointer entries carry none.
///
/// Everything else is delegated to [`Sv39`]. A type rather than a runtime switch: the kernel picks
/// it per machine at compile time (`board_th1520`), keeping this crate free of global state, and
/// checks at boot that the hart agrees.
///
/// Name: provisional, milestone 89 (Scaleway EM-RV1)'s lane, 2026-10-06 (UTC).
pub struct Sv39Mae;

/// Strongly ordered: no speculation, no reordering, which a device register needs.
const MAE_SO: u64 = 1 << 63;
/// Cacheable.
const MAE_C: u64 = 1 << 62;
/// Bufferable: writes may be merged and posted.
const MAE_B: u64 = 1 << 61;
/// Shareable: coherent across harts.
const MAE_SH: u64 = 1 << 60;
/// All five memory-type bits, which a decoder must mask and a pointer must never set.
const MAE_MASK: u64 = 0b11111 << 59;

impl Sv39Mae {
    /// The memory type for a leaf with `flags`. The RSW device bit [`Sv39`] already keeps is what
    /// decides, so the portable [`Flags::device`] is the one input on both formats.
    const fn memory_type(flags: Flags) -> u64 {
        if flags.is_device() {
            MAE_SO | MAE_SH
        } else {
            MAE_C | MAE_B | MAE_SH
        }
    }

    /// The memory-type field of a leaf, for a test or a bench line that wants to show it.
    pub const fn memory_type_of(entry: u64) -> u64 {
        entry & MAE_MASK
    }
}

impl PageFormat for Sv39Mae {
    const LEVELS: usize = Sv39::LEVELS;
    const SPLIT_SHIFT: u32 = Sv39::SPLIT_SHIFT;

    fn is_present(entry: u64) -> bool {
        Sv39::is_present(entry)
    }

    fn entry_pa(entry: u64) -> u64 {
        // The PPN mask already stops at bit 53, so the memory type cannot leak into the address.
        Sv39::entry_pa(entry)
    }

    fn table_entry(pa: u64, level: usize) -> u64 {
        Sv39::table_entry(pa, level)
    }

    fn leaf_entry(pa: u64, flags: Flags) -> u64 {
        Sv39::leaf_entry(pa, flags) | Self::memory_type(flags)
    }

    fn leaf_flags(entry: u64) -> Flags {
        Sv39::leaf_flags(entry & !MAE_MASK)
    }

    fn block_entry(pa: u64, flags: Flags, size: PageSize) -> Option<u64> {
        Some(Sv39::block_entry(pa, flags, size)? | Self::memory_type(flags))
    }

    fn is_block(entry: u64) -> bool {
        Sv39::is_block(entry)
    }
}

#[cfg(test)]
mod mae_tests {
    use super::*;

    const ALL: [Flags; 8] = [
        Flags::kernel_code(),
        Flags::kernel_rodata(),
        Flags::kernel_data(),
        Flags::device(),
        Flags::user_code(),
        Flags::user_rodata(),
        Flags::user_data(),
        Flags::user_device(),
    ];

    /// **Linux's two values, bit for bit.** Written as literals rather than through this file's
    /// constants, so a wrong constant cannot agree with itself: memory is `0x7 << 60`
    /// (`_PAGE_PMA_THEAD`), a device `(1 << 63) | (1 << 60)` (`_PAGE_IO_THEAD`).
    #[test]
    fn memory_and_devices_carry_linuxs_memory_types() {
        for flags in ALL {
            let want = if flags.is_device() {
                0x9000_0000_0000_0000
            } else {
                0x7000_0000_0000_0000
            };
            let leaf = Sv39Mae::leaf_entry(0xff_e701_4000, flags);
            assert_eq!(leaf >> 59 << 59, want, "{flags:?}");
            assert_eq!(Sv39Mae::memory_type_of(leaf), want, "{flags:?}");
            for size in [PageSize::Size2MiB, PageSize::Size1GiB] {
                let block = Sv39Mae::block_entry(0x8000_0000, flags, size).unwrap();
                assert_eq!(block >> 59 << 59, want, "{flags:?} block");
            }
        }
    }

    /// **Below bit 59 the word is plain Sv39's**, so everything already proved about that format
    /// (W^X, the address field, the A and D bits) holds here unchanged; and the flags round-trip.
    #[test]
    fn below_the_memory_type_it_is_sv39() {
        for flags in ALL {
            for pa in [0, 0x8020_0000, 0xff_e701_4000, 0x3_ffff_f000] {
                let leaf = Sv39Mae::leaf_entry(pa, flags);
                assert_eq!(leaf & !MAE_MASK, Sv39::leaf_entry(pa, flags));
                assert_eq!(Sv39Mae::entry_pa(leaf), pa);
                assert_eq!(Sv39Mae::leaf_flags(leaf), flags);
                assert!(Sv39Mae::is_block(leaf));
            }
        }
    }

    /// **A pointer entry carries no memory type.** T-Head defines the bits on leaves only, and
    /// Linux sets them only through a leaf's protection bits.
    #[test]
    fn a_pointer_carries_no_memory_type() {
        let e = Sv39Mae::table_entry(0x8100_0000, 0);
        assert_eq!(e, Sv39::table_entry(0x8100_0000, 0));
        assert!(Sv39Mae::is_present(e) && !Sv39Mae::is_present(0));
        assert_eq!(Sv39Mae::memory_type_of(e), 0);
        assert!(!Sv39Mae::is_block(e));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Every leaf is present, readable, and accessed.** V and R make it a valid readable leaf; A
    /// set eagerly avoids an access fault on first touch (the Sv39 analog of aarch64's AF).
    #[test]
    fn every_leaf_is_valid_readable_and_accessed() {
        for flags in [
            Flags::kernel_code(),
            Flags::kernel_data(),
            Flags::user_code(),
            Flags::user_data(),
        ] {
            let leaf = Sv39::leaf_entry(0x8020_0000, flags);
            assert_ne!(leaf & V, 0, "not valid: {flags:?}");
            assert_ne!(leaf & R, 0, "not readable: {flags:?}");
            assert_ne!(leaf & A, 0, "accessed bit not set: {flags:?}");
        }
    }

    /// **A writable leaf is pre-dirtied, and a read-only one has nothing to dirty.** D is set for
    /// the same reason as A: hardware that has to set it itself takes a store fault on the first
    /// write to every page. Neither bit reaches the portable [`Flags`], so the round-trip test below
    /// cannot see D go missing.
    #[test]
    fn a_writable_leaf_is_pre_dirtied() {
        assert_ne!(
            Sv39::leaf_entry(0x8020_0000, Flags::kernel_data()) & D,
            0,
            "a writable leaf faults on its first store to set D",
        );
        assert_eq!(
            Sv39::leaf_entry(0x8020_0000, Flags::kernel_code()) & D,
            0,
            "a read-only page cannot be dirtied",
        );
    }

    /// **A table-pointer entry has R=W=X=0** (that is what distinguishes it from a leaf) and V=1.
    #[test]
    fn a_table_entry_is_a_pointer_not_a_leaf() {
        let e = Sv39::table_entry(0x8100_0000, 0);
        assert_ne!(e & V, 0);
        assert_eq!(e & (R | W | X), 0, "a pointer must have R=W=X clear");
        assert_eq!(Sv39::entry_pa(e), 0x8100_0000);
    }

    /// **Every constructor round-trips through encode/decode**, including the RSW device bit.
    #[test]
    fn flags_round_trip_through_a_leaf() {
        for flags in [
            Flags::kernel_code(),
            Flags::kernel_rodata(),
            Flags::kernel_data(),
            Flags::device(),
            Flags::user_code(),
            Flags::user_rodata(),
            Flags::user_data(),
            Flags::user_device(),
        ] {
            let leaf = Sv39::leaf_entry(0x8020_0000, flags);
            assert_eq!(Sv39::entry_pa(leaf), 0x8020_0000);
            assert_eq!(
                Sv39::leaf_flags(leaf),
                flags,
                "round-trip failed for {flags:?}"
            );
        }
    }

    /// **Any one of R, W and X makes an entry a leaf.** Every walk above the bottom level asks
    /// this before it descends, so an entry it misreads as a pointer is a megapage whose data is
    /// walked as though it were a page table.
    #[test]
    fn any_permission_bit_makes_a_leaf_and_none_makes_a_pointer() {
        for bits in [R, W, X, R | W, R | X] {
            assert!(Sv39::is_block(V | bits), "{bits:#x}");
        }
        assert!(!Sv39::is_block(Sv39::table_entry(0x8020_0000, 0)));
    }

    /// **A write-combining request is encoded as plain device memory here**: correct, because
    /// device memory is the stricter of the two, and slower.
    /// Nothing on this architecture maps a device aperture as a screen yet; when something does,
    /// this is the test that says the request is being ignored.
    #[test]
    fn write_combining_is_encoded_as_device_memory() {
        assert_eq!(
            Sv39::leaf_entry(0x1000_0000, Flags::write_combining()),
            Sv39::leaf_entry(0x1000_0000, Flags::device()),
        );
    }
}

/// Machine-checked proofs of the Sv39 format, mirroring the aarch64 module's. The shared `Mapper`
/// walk inherits both formats' guarantees. See notes/verification.md.
#[cfg(kani)]
mod verification {
    use super::*;
    use crate::{Half, PAGE_SIZE, PageSize};

    /// **The walk never indexes past a table** (three levels here).
    /// Falsification: replayable `crates/paging/falsifications/sv39.verification.index_is_always_in_bounds.patch`
    #[kani::proof]
    fn index_is_always_in_bounds() {
        let va: u64 = kani::any();
        let level: usize = kani::any();
        kani::assume(level < Sv39::LEVELS);
        assert!(Sv39::index(va, level) < crate::ENTRIES);
    }

    /// **The three indices and the offset tile the low 39 bits exactly.**
    /// Falsification: replayable `crates/paging/falsifications/sv39.verification.the_indices_and_offset_tile_the_address.patch`
    #[kani::proof]
    fn the_indices_and_offset_tile_the_address() {
        let va: u64 = kani::any();
        let reconstructed = ((Sv39::index(va, 0) as u64) << 30)
            | ((Sv39::index(va, 1) as u64) << 21)
            | ((Sv39::index(va, 2) as u64) << 12)
            | (va & (PAGE_SIZE - 1));
        assert_eq!(reconstructed, va & 0x0000_007f_ffff_ffff);
    }

    /// **Distinct pages take distinct paths** within the 39-bit VA.
    /// Falsification: replayable `crates/paging/falsifications/sv39.verification.distinct_pages_take_distinct_paths.patch`
    #[kani::proof]
    fn distinct_pages_take_distinct_paths() {
        let a: u64 = kani::any::<u64>() & 0x0000_007f_ffff_f000;
        let b: u64 = kani::any::<u64>() & 0x0000_007f_ffff_f000;
        kani::assume(
            Sv39::index(a, 0) == Sv39::index(b, 0)
                && Sv39::index(a, 1) == Sv39::index(b, 1)
                && Sv39::index(a, 2) == Sv39::index(b, 2),
        );
        assert_eq!(a, b);
    }

    /// **The two halves are disjoint** at the Sv39 split (bit 38).
    /// Falsification: replayable `crates/paging/falsifications/sv39.verification.the_two_halves_are_disjoint.patch`
    #[kani::proof]
    fn the_two_halves_are_disjoint() {
        let va: u64 = kani::any();
        assert!(!(Sv39::is_in_half(Half::Low, va) && Sv39::is_in_half(Half::High, va)));
    }

    /// **The user-VA gate admits exactly the aligned low half**, never the high one.
    /// Falsification: replayable `crates/paging/falsifications/sv39.verification.the_user_va_gate_admits_only_the_aligned_low_half.patch`
    #[kani::proof]
    fn the_user_va_gate_admits_only_the_aligned_low_half() {
        let va: u64 = kani::any();
        assert_eq!(
            crate::is_user_page_va::<Sv39>(va),
            va & 0xfff == 0 && va >> 38 == 0
        );
        if crate::is_user_page_va::<Sv39>(va) {
            assert!(Sv39::is_in_half(Half::Low, va) && !Sv39::is_in_half(Half::High, va));
        }
    }

    /// **No encoded leaf is both writable and executable**, over every constructor. A leaf with `W` (bit 2) set
    /// has `X` (bit 3) clear.
    ///
    /// Stated in raw bits written as literals, never through `leaf_flags` or this file's own
    /// constants: a decoder or a constant that moved with the encoder would otherwise hide the
    /// defect (the trap `the_leaf_keeps_address_and_permissions_apart` records). Milestone 718
    /// (provisional) added it so §19 (architectural parity is a tenet) gate hold: this claim is proved on all three ISAs.
    /// Falsification: replayable `crates/paging/falsifications/sv39.verification.no_encoded_leaf_is_both_writable_and_executable.patch`
    #[kani::proof]
    fn no_encoded_leaf_is_both_writable_and_executable() {
        let pa: u64 = kani::any();
        kani::assume(pa & !0x00ff_ffff_ffff_f000 == 0);

        let all = [
            Flags::kernel_code(),
            Flags::kernel_rodata(),
            Flags::kernel_data(),
            Flags::device(),
            Flags::user_code(),
            Flags::user_rodata(),
            Flags::user_data(),
            Flags::user_device(),
        ];
        let i: usize = kani::any();
        kani::assume(i < all.len());

        let leaf = Sv39::leaf_entry(pa, all[i]);
        assert!(leaf & (1 << 2) == 0 || leaf & (1 << 3) == 0);
    }

    /// **A megapage or gigapage keeps the address and the permissions apart and is a leaf**, for
    /// every physical address and every `Flags` constructor. Literals for every bit position: the
    /// PPN is bits [53:10], and its low 9 (megapage) or 18 (gigapage) bits must be zero, or the
    /// hardware raises a misaligned-superpage fault on the first access.
    /// Falsification: replayable `crates/paging/falsifications/sv39.verification.a_block_keeps_address_and_permissions_apart.patch`
    #[kani::proof]
    fn a_block_keeps_address_and_permissions_apart() {
        let pa: u64 = kani::any();
        kani::assume(pa >> 56 == 0); // Sv39 physical addresses are 56 bits
        let two_mib: bool = kani::any();
        let (size, pa_bits) = if two_mib {
            (PageSize::Size2MiB, 0x00ff_ffff_ffe0_0000u64)
        } else {
            (PageSize::Size1GiB, 0x00ff_ffff_c000_0000u64)
        };
        let all = [
            Flags::kernel_code(),
            Flags::kernel_rodata(),
            Flags::kernel_data(),
            Flags::device(),
            Flags::user_code(),
            Flags::user_rodata(),
            Flags::user_data(),
            Flags::user_device(),
        ];
        let i: usize = kani::any();
        kani::assume(i < all.len());
        let flags = all[i];

        let block = Sv39::block_entry(pa, flags, size).expect("Sv39 encodes both sizes");
        // PPN at [53:10] is pa[55:12]; read it out of the word by hand.
        let ppn_as_address = ((block >> 10) & 0x0000_0fff_ffff_ffff) << 12;
        assert_eq!(ppn_as_address, pa & pa_bits, "the address left its field");
        assert_eq!(block & 1, 1, "a block must be valid");
        assert_ne!(
            block & 0b1110,
            0,
            "R, W or X must be set, or this is a pointer"
        );
        assert!(Sv39::is_block(block));
        assert_eq!(Sv39::leaf_flags(block), flags);
    }

    /// **No pointer PTE ever reads as a leaf**, for every address.
    /// Falsification: replayable `crates/paging/falsifications/sv39.verification.a_table_entry_is_never_a_block.patch`
    #[kani::proof]
    fn a_table_entry_is_never_a_block() {
        let pa: u64 = kani::any();
        assert!(!Sv39::is_block(Sv39::table_entry(pa, 0)));
    }

    /// **A leaf keeps the address and the permissions apart, and the permissions round-trip.**
    /// Falsification: replayable `crates/paging/falsifications/sv39.verification.the_leaf_keeps_address_and_permissions_apart.patch`
    #[kani::proof]
    fn the_leaf_keeps_address_and_permissions_apart() {
        // Page-aligned physical address within Sv39's 56-bit physical space.
        let pa: u64 = kani::any();
        kani::assume(pa & !(PPN_MASK << 12) == 0);

        let all = [
            Flags::kernel_code(),
            Flags::kernel_rodata(),
            Flags::kernel_data(),
            Flags::device(),
            Flags::user_code(),
            Flags::user_rodata(),
            Flags::user_data(),
            Flags::user_device(),
        ];
        let i: usize = kani::any();
        kani::assume(i < all.len());
        let flags = all[i];

        let leaf = Sv39::leaf_entry(pa, flags);
        // **The address field read out of the word, not through `entry_pa`** (milestone 211).
        // `leaf_entry` and `entry_pa` are an encoder and its own decoder, so a round trip
        // between them is satisfied by any pair that agree: a shift wrong in both would leave
        // this green while the hardware read the wrong page. The line below states where the
        // architecture puts the address, which is the one thing the implementation does not
        // get to choose, and it is spelled as a literal rather than through this crate's own
        // ADDR_MASK or PPN_SHIFT: a defect in one of those constants moves the implementation
        // and any harness that cited it together, which is the same trap one level down.
        // `no_vtd_entry_ever_sets_a_reserved_bit` in this crate already works this way; this
        // is the same move on the portable leaf.
        assert_eq!(
            ((leaf >> 10) & ((1 << 44) - 1)) << 12,
            pa,
            "the address left bits 10..54 of the PTE",
        );
        assert_eq!(leaf & 1, 1, "a leaf must have the valid bit set");
        assert_eq!(Sv39::entry_pa(leaf), pa);
        assert_eq!(Sv39::leaf_flags(leaf), flags);
    }
}
