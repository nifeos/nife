//! The `x86_64` four-level format, [`Ia32e`], and the IOMMU formats of its shape, [`Vtd`] and
//! [`AmdVi`]. See notes/paging.md, "x86_64", for the inherited permissions and the NX bit.
//!
//! ```text
//!  47      39 38      30 29      21 20      12 11         0
//! ┌──────────┬──────────┬──────────┬──────────┬────────────┐
//! │  PML4    │  PDPT    │   PD     │   PT     │   offset   │
//! │  9 bits  │  9 bits  │  9 bits  │  9 bits  │  12 bits   │
//! └──────────┴──────────┴──────────┴──────────┴────────────┘
//! ```

use crate::{
    CAP_DEVICE, CAP_GLOBAL, CAP_KERNEL_EXEC, CAP_USER, CAP_USER_EXEC, CAP_WRITE, CAP_WRITE_COMBINE,
    Flags, PageFormat, PageSize,
};

/// **What the eight PAT entries mean**, as `arch::x86_64::init` programs `IA32_PAT` on every core.
/// The same role as aarch64's `mair` module: the encoder below selects an entry by its `PWT`,
/// `PCD` and `PAT` bits, and the value here is what that entry then means, so the two are one
/// definition.
pub mod pat {
    /// `IA32_PAT`, MSR 0x277.
    pub const MSR: u32 = 0x277;

    /// The power-on value, entries 0 to 7: WB, WT, UC-, UC, WB, WT, UC-, UC (one byte each, low
    /// byte first). From the Intel SDM's PAT chapter, recalled rather than re-read; the kernel
    /// prints what it finds before it writes, so a machine that disagrees says so.
    pub const RESET: u64 = 0x0007_0406_0007_0406;

    /// **The value this kernel programs: the power-on table with entry 1 changed from
    /// write-through to write-combining (type 0x01).** Entry 1 is what `PWT` alone selects, and
    /// only a write-combining leaf sets `PWT` alone (device memory is `PCD | PWT`, entry 3, strong
    /// UC, and everything else is entry 0). Linux makes the same choice for the same entry.
    pub const VALUE: u64 = (RESET & !0xff00) | 0x01 << 8;
}

const P: u64 = 1 << 0; // Present
const RW: u64 = 1 << 1; // Read/Write (clear = read-only)
const US: u64 = 1 << 2; // User/Supervisor (set = user may access)
const PWT: u64 = 1 << 3; // Page-level Write-Through
const PCD: u64 = 1 << 4; // Page-level Cache Disable
const A: u64 = 1 << 5; // Accessed
const D: u64 = 1 << 6; // Dirty
const G: u64 = 1 << 8; // Global (ignored unless CR4.PGE)
/// Bit 7, Page Size, in a PDPT or PD entry: set, the entry is a 1 GiB or 2 MiB leaf rather than a
/// pointer to the next table. **The same bit is `PAT` in a bottom-level entry**, which is why
/// [`PageFormat::is_block`] is only ever asked above the bottom, and it is reserved-must-be-zero in
/// a PML4 entry, which is why [`Ia32e::block_entry`] declines anything larger than 1 GiB.
const PS: u64 = 1 << 7;
/// Bit 63, execute disable, and only while `IA32_EFER.NXE` is set: with NXE clear it is reserved
/// and faults the entry. The boot trampoline sets NXE before it builds any table
/// (`arch/x86_64/boot.s`), which this crate cannot check.
const XD: u64 = 1 << 63;

/// Bit 9, the first of the three "available to software" bits (11:9) in every entry. Used to record
/// that a leaf was created as **kernel**-executable, which the hardware itself cannot express: x86
/// has one execute permission per entry (the absence of XD) and decides which ring it applies to
/// from `U/S`, exactly as Sv39 does with its `U` bit. That much round-trips without help. What does
/// not is the difference between "kernel data" and "kernel code", both of which are supervisor-only
/// and only one of which should be executable, so the encoder needs to remember which it was told.
const SW_KERNEL_EXEC: u64 = 1 << 9;

/// Bit 10, the second software bit: this leaf is device memory. x86 expresses uncacheable memory
/// through PWT/PCD (and the PAT), which [`Ia32e::attrs`] does set, but those bits are also what a
/// cacheability policy would use for ordinary memory, so they are not a reliable read-back of the
/// portable [`Flags::is_device`] intent. Same role as Sv39's `SW_DEVICE`.
const SW_DEVICE: u64 = 1 << 10;

/// The physical address occupies bits [51:12]: 40 bits. Bits 62:52 are reserved or software-use,
/// and bit 63 is XD, so the mask must not be widened without checking both ends.
const ADDR_MASK: u64 = 0x000f_ffff_ffff_f000;

/// The `x86_64` four-level (IA-32e) page-table format. Named for Intel's own term, "IA-32e
/// paging" (SDM volume 3, chapter 4), since `X86_64` is not camel case. Name provisional.
pub struct Ia32e;

impl Ia32e {
    /// Encode the permission/attribute bits (not the address or P) for a leaf with `flags`.
    ///
    /// A and D are set eagerly for the same reason Sv39 does it: hardware that has to set them
    /// itself takes a fault on the first touch of every page, and there is nothing here that wants
    /// to know about first touches. D is set only where a write is possible at all.
    const fn attrs(flags: Flags) -> u64 {
        let mut bits = A;
        if flags.is_writable() {
            bits |= RW | D;
        }
        if flags.is_user_accessible() {
            bits |= US;
        }
        if flags.is_global() {
            bits |= G;
        }
        if flags.is_write_combining() {
            // `PWT` alone: PAT entry 1, which [`pat::VALUE`] makes write-combining. A PAT type of
            // WC wins over any MTRR, so this is WC whatever the firmware's MTRRs say about the
            // range. `SW_DEVICE` as for any device page, and the missing `PCD` is what the decoder
            // reads back as the combining.
            bits |= PWT | SW_DEVICE;
        } else if flags.is_device() {
            // Uncacheable, write-through: `PCD | PWT` selects PAT entry 3, strong uncacheable,
            // which [`pat::VALUE`] leaves as it was at reset. A device register that gets cached
            // reads back a value the device never produced.
            bits |= PCD | PWT | SW_DEVICE;
        }
        // One execute permission, at whichever ring `U/S` names: XD only for a page nobody
        // executes, as on Sv39. A user page is not kernel-executable only while `CR4.SMEP` is set,
        // which `arch::x86_64::init` sets on every core that offers it; this crate cannot check.
        if flags.is_kernel_executable() {
            bits |= SW_KERNEL_EXEC;
        } else if !flags.is_user_executable() {
            bits |= XD;
        }
        bits
    }
}

impl PageFormat for Ia32e {
    const LEVELS: usize = 4;

    /// Bit 47. An `x86_64` virtual address is canonical when bits 63:47 are all equal, which is the
    /// same shape as Sv39's sign extension from bit 38, one bit position over.
    const SPLIT_SHIFT: u32 = 47;

    fn is_present(entry: u64) -> bool {
        entry & P != 0
    }

    fn entry_pa(entry: u64) -> u64 {
        entry & ADDR_MASK
    }

    fn table_entry(pa: u64, _level: usize) -> u64 {
        // Maximally permissive, because x86 ANDs the levels together and the leaf must be able to
        // grant anything. See notes/paging.md for what that trades away.
        (pa & ADDR_MASK) | P | RW | US
    }

    fn leaf_entry(pa: u64, flags: Flags) -> u64 {
        (pa & ADDR_MASK) | P | Self::attrs(flags)
    }

    fn leaf_flags(entry: u64) -> Flags {
        let mut caps = 0;
        if entry & RW != 0 {
            caps |= CAP_WRITE;
        }
        if entry & US != 0 {
            caps |= CAP_USER;
        }
        if entry & XD == 0 {
            if entry & US != 0 {
                caps |= CAP_USER_EXEC;
            } else if entry & SW_KERNEL_EXEC != 0 {
                caps |= CAP_KERNEL_EXEC;
            }
        }
        if entry & G != 0 {
            caps |= CAP_GLOBAL;
        }
        if entry & SW_DEVICE != 0 {
            caps |= CAP_DEVICE;
            if entry & PCD == 0 {
                caps |= CAP_WRITE_COMBINE;
            }
        }
        Flags::from_caps(caps)
    }

    /// A PD entry (2 MiB) or a PDPT entry (1 GiB) with `PS` set, and otherwise the page leaf's
    /// bits, every one of which sits in the same position at all three levels.
    ///
    /// Bit 12 is `PAT` in a large-page entry, and bits 20:13 (2 MiB) or 29:13 (1 GiB) are reserved,
    /// so the address is masked to the block's own alignment: a stray low bit is a reserved-bit
    /// fault. `attrs` never sets bit 12, so the memory type is the one a page leaf selects.
    ///
    /// A 1 GiB leaf is usable only where `CPUID` leaf 0x80000001 reports `Page1GB` (EDX bit 26).
    /// This crate cannot ask, so the caller passes the largest size it has checked for
    /// ([`Mapper::map_span`](crate::Mapper::map_span)).
    fn block_entry(pa: u64, flags: Flags, size: PageSize) -> Option<u64> {
        match size {
            PageSize::Size4KiB => Some(Self::leaf_entry(pa, flags)),
            PageSize::Size2MiB | PageSize::Size1GiB => {
                Some((pa & ADDR_MASK & !(size.bytes() - 1)) | P | PS | Self::attrs(flags))
            }
        }
    }

    fn is_block(entry: u64) -> bool {
        entry & PS != 0
    }
}

/// **VT-d's second-level page-table format**: what an Intel IOMMU walks to translate a device's
/// DMA address. The shape of [`Ia32e`], but a leaf has exactly two meaningful bits, Read (0) and
/// Write (1); bits 2 through 10 are reserved and checked (QEMU's `VTD_SPTE_PAGE_L1_RSVD_MASK`), so
/// `Ia32e`'s `US` and `XD` would fault every DMA. A device has no privilege level and no execute
/// permission here, and [`leaf_flags`](Vtd::leaf_flags) reports only write. See notes/paging.md,
/// "VT-d".
pub struct Vtd;

/// Read: the transaction may read through this entry. Bit 0, same position as `Ia32e`'s `P`,
/// which is a coincidence worth naming: VT-d has no separate present bit, an entry with both `R`
/// and `W` clear is simply not present.
const VTD_R: u64 = 1 << 0;
/// Write: the transaction may write through this entry.
const VTD_W: u64 = 1 << 1;
/// Bits 51:12: the physical address of the next table or, at a leaf, the mapped frame. The same
/// width `Ia32e::ADDR_MASK` uses, because nothing this kernel runs names a wider physical address
/// yet; VT-d's actual width is `CAP_REG.MGAW`-defined and this driver has not needed to narrow to
/// it (`kernel/src/arch/x86_64/iommu.rs`'s BUGS says why).
const VTD_ADDR_MASK: u64 = 0x000f_ffff_ffff_f000;

/// **Every bit a VT-d second-level entry may carry**: R (bit 0), W (bit 1) and the address field
/// (bits 51:12), per QEMU's `VTD_SPTE_PAGE_L1_RSVD_MASK`; any other bit faults the transaction.
///
/// A literal, because the encoder is built from `VTD_R`, `VTD_W` and `VTD_ADDR_MASK`, and a check
/// through them holds for every value they take. Gated to the test configurations, so no
/// implementation can reach it. See notes/paging.md, "Reserved bits are spelled as literals".
#[cfg(any(test, kani))]
const VTD_PERMITTED_BITS: u64 = 0x000f_ffff_ffff_f003;

impl PageFormat for Vtd {
    const LEVELS: usize = 4;

    /// Not a real VT-d concept (IOVAs are not split into a low and a high half the way CPU virtual
    /// addresses are); kept at the same value `Ia32e` uses so
    /// [`is_in_half`](PageFormat::is_in_half) admits exactly the addresses this driver ever asks it
    /// about, every physical address the frame allocator hands out on a machine with less than 128
    /// TiB of RAM.
    const SPLIT_SHIFT: u32 = 47;

    fn is_present(entry: u64) -> bool {
        entry & (VTD_R | VTD_W) != 0
    }

    fn entry_pa(entry: u64) -> u64 {
        entry & VTD_ADDR_MASK
    }

    /// Both bits set: VT-d ANDs permissions down the walk exactly as the CPU's own tables do (an
    /// intermediate entry's `R`/`W` gate every leaf beneath it), so an intermediate must grant
    /// everything and let the leaf decide, the same reasoning `Ia32e::table_entry` documents.
    fn table_entry(pa: u64, _level: usize) -> u64 {
        (pa & VTD_ADDR_MASK) | VTD_R | VTD_W
    }

    /// Read is unconditional (every domain this seam builds is at least readable) and write
    /// follows `flags.is_writable()`. Nothing else: see this type's own doc for why `US` and `XD`
    /// would be reserved-bit violations here rather than the extra permissiveness they are on the
    /// CPU's own format.
    fn leaf_entry(pa: u64, flags: Flags) -> u64 {
        let mut bits = VTD_R;
        if flags.is_writable() {
            bits |= VTD_W;
        }
        (pa & VTD_ADDR_MASK) | bits
    }

    /// The only two facts a second-level leaf carries: read (always, once present) and write.
    fn leaf_flags(entry: u64) -> Flags {
        let mut caps = 0;
        if entry & VTD_W != 0 {
            caps |= CAP_WRITE;
        }
        Flags::from_caps(caps)
    }

    /// **Pages only.** VT-d has second-level superpages (bit 7, `SP`), but whether this unit
    /// walks them is `CAP_REG.SLLPS`'s answer, which this crate cannot read and the driver does not
    /// check; and bit 7 is outside `VTD_PERMITTED_BITS`, in the reserved range, for exactly that reason.
    /// A DMA domain is a few granted pages, not a direct map, so there is nothing to win yet.
    fn block_entry(pa: u64, flags: Flags, size: PageSize) -> Option<u64> {
        match size {
            PageSize::Size4KiB => Some(Self::leaf_entry(pa, flags)),
            PageSize::Size2MiB | PageSize::Size1GiB => None,
        }
    }

    /// Bit 7 is what the hardware would read as a superpage, so a walk agrees with it on an entry
    /// somebody else wrote; this format never writes one.
    fn is_block(entry: u64) -> bool {
        entry & (1 << 7) != 0
    }
}

/// **AMD-Vi's host page-table format**: what an AMD IOMMU walks to translate a device's DMA
/// address. The shape of [`Vtd`] and [`Ia32e`] and an encoding that is neither, every bit from AMD
/// document 48882 revision 2.62, section 2.2.3 (Figures 9 and 10, Tables 17 and 18).
///
/// A directory entry written at walk level `L` names the AMD level it points at in `NextLevel`
/// (bits 11:9), `LEVELS - 1 - L`, and a leaf says 0. Permissions (`IR`, `IW`) combine by AND down the
/// walk, so a directory grants both. `FC` (bit 60, leaves only) keeps a device's DMA coherent
/// whatever it asks for. See notes/paging.md, "AMD-Vi". Name provisional.
pub struct AmdVi;

/// `PR`: present. Bit 0, at every level.
const AMD_VI_PR: u64 = 1 << 0;
/// `NextLevel`, bits 11:9.
const AMD_VI_NEXT_LEVEL_SHIFT: u64 = 9;
const AMD_VI_NEXT_LEVEL_MASK: u64 = 0b111 << AMD_VI_NEXT_LEVEL_SHIFT;
/// `FC`: force coherent. Leaf only.
const AMD_VI_FC: u64 = 1 << 60;
/// `IR`: the device may read through this entry.
const AMD_VI_IR: u64 = 1 << 61;
/// `IW`: the device may write through this entry.
const AMD_VI_IW: u64 = 1 << 62;
/// Bits 51:12, the next table's or the page's physical address.
const AMD_VI_ADDR_MASK: u64 = 0x000f_ffff_ffff_f000;

/// **Every bit an AMD-Vi leaf this format writes may carry**: `PR`, the address, `FC`, `IR`, `IW`.
/// `NextLevel` is zero in a 4 KiB leaf, and bits 58:52 are reserved and fault the walk with
/// `IO_PAGE_FAULT` (Table 17). Spelled as a literal for the reason [`VTD_PERMITTED_BITS`] gives:
/// written through the encoder's own constants, the check would move with them.
#[cfg(test)]
const AMD_VI_LEAF_PERMITTED_BITS: u64 = 0x700f_ffff_ffff_f001;
/// The same for a directory entry: `PR`, `NextLevel`, the address, `IR`, `IW`. Bit 60 is reserved
/// here (Table 18), which is the one place the two entry kinds differ.
#[cfg(test)]
const AMD_VI_DIRECTORY_PERMITTED_BITS: u64 = 0x600f_ffff_ffff_fe01;

impl PageFormat for AmdVi {
    const LEVELS: usize = 4;

    /// Not an AMD-Vi concept either; kept equal to [`Vtd`]'s for the same reason [`Vtd`] gives.
    const SPLIT_SHIFT: u32 = 47;

    fn is_present(entry: u64) -> bool {
        entry & AMD_VI_PR != 0
    }

    fn entry_pa(entry: u64) -> u64 {
        entry & AMD_VI_ADDR_MASK
    }

    /// A directory entry at walk level `level`, naming the AMD level of the table at `pa`
    /// (`LEVELS - 1 - level`), granting read and write so the leaf decides.
    fn table_entry(pa: u64, level: usize) -> u64 {
        let next = (Self::LEVELS - 1 - level) as u64;
        (pa & AMD_VI_ADDR_MASK)
            | AMD_VI_PR
            | (next << AMD_VI_NEXT_LEVEL_SHIFT)
            | AMD_VI_IR
            | AMD_VI_IW
    }

    /// A 4 KiB leaf: present, `NextLevel` 0, force coherent, readable, and writable when `flags`
    /// says so. A device has no privilege level and no execute permission in this mode, so `US`
    /// and `XD` have nowhere to go, exactly as on [`Vtd`].
    fn leaf_entry(pa: u64, flags: Flags) -> u64 {
        let mut bits = AMD_VI_PR | AMD_VI_FC | AMD_VI_IR;
        if flags.is_writable() {
            bits |= AMD_VI_IW;
        }
        (pa & AMD_VI_ADDR_MASK) | bits
    }

    fn leaf_flags(entry: u64) -> Flags {
        let mut caps = 0;
        if entry & AMD_VI_IW != 0 {
            caps |= CAP_WRITE;
        }
        Flags::from_caps(caps)
    }

    /// **Pages only**, the same call [`Vtd`] makes and for the same reason: a DMA domain is a few
    /// granted pages. AMD-Vi could say 2 MiB with a level-2 entry whose `NextLevel` is 0; nothing
    /// here needs it yet.
    fn block_entry(pa: u64, flags: Flags, size: PageSize) -> Option<u64> {
        match size {
            PageSize::Size4KiB => Some(Self::leaf_entry(pa, flags)),
            PageSize::Size2MiB | PageSize::Size1GiB => None,
        }
    }

    /// Above the bottom level, an entry whose `NextLevel` is 0 or 7 is a page translation entry
    /// rather than a directory (section 2.2.3's definition), so a walk stops at it.
    fn is_block(entry: u64) -> bool {
        let next = (entry & AMD_VI_NEXT_LEVEL_MASK) >> AMD_VI_NEXT_LEVEL_SHIFT;
        next == 0 || next == 7
    }
}

// --- The AMD-Vi device table entry (AMD 48882 rev 2.62, section 2.2.2.1, Table 7). ---
//
// Here rather than in the kernel's driver so Kani can prove it on any host: the kernel's x86_64
// proofs compile only on an x86_64 host (notes/kernel-proofs.md).

/// `V`, bit 0: the entry's bits 127:1 are valid.
const DTE_V: u64 = 1 << 0;
/// `TV`, bit 1: the translation fields are valid.
const DTE_TV: u64 = 1 << 1;
/// `Mode` (bits 11:9) = 100b: a four-level host table, the depth [`AmdVi::LEVELS`] builds.
const DTE_MODE_4_LEVEL: u64 = 4 << 9;
/// `IR` (bit 61) and `IW` (bit 62): the device may read and write, as far as the table allows.
const DTE_IR: u64 = 1 << 61;
const DTE_IW: u64 = 1 << 62;

/// **Every bit of a device table entry's first qword this crate may set**: `V`, `TV`, `Mode`, the
/// page table root pointer (51:12), `IR` and `IW`. Bits 8:2, 54:52 and 63 are reserved and reported
/// as an `ILLEGAL_DEV_TABLE_ENTRY` event when `V` = 1; bits 60:55 are guest-translation fields
/// that must be zero when `GTSup` is clear, and this driver never uses them. Written out as a
/// literal for the reason [`VTD_PERMITTED_BITS`] gives.
#[cfg(any(test, kani))]
const AMD_VI_DTE_PERMITTED_BITS: u64 = 0x600f_ffff_ffff_fe03;

impl AmdVi {
    /// **The device table entry of a device confined to the table at `root`**: valid, translating,
    /// four levels, read and write allowed so the leaves decide, tagged `domain` (Table 7). Every
    /// other bit is zero: no interrupt remapping (`IV`, bit 128, clear), no IOTLB, no exclusion
    /// (`EX`), events not suppressed. `None` when `root` is not a 4 KiB-aligned address the 40-bit
    /// root field can hold, because the alternative is a silent mask that points the device at a
    /// different table. Name: provisional.
    pub const fn device_table_entry(root: u64, domain: u16) -> Option<[u64; 4]> {
        if root & !AMD_VI_ADDR_MASK != 0 {
            return None;
        }
        Some([
            DTE_V | DTE_TV | DTE_MODE_4_LEVEL | root | DTE_IR | DTE_IW,
            domain as u64,
            0,
            0,
        ])
    }

    /// **The device table entry that denies everything**: a four-level walk to `empty_root`, an
    /// all-zero table, with `IR` and `IW` clear, in domain 0. Every access target-aborts and is
    /// logged. Why not `Mode` 0 with `IR` = `IW` = 0 is the kernel driver's header. `None` as for
    /// [`device_table_entry`](Self::device_table_entry). Name: provisional.
    pub const fn blocked_device_table_entry(empty_root: u64) -> Option<[u64; 4]> {
        if empty_root & !AMD_VI_ADDR_MASK != 0 {
            return None;
        }
        Some([DTE_V | DTE_TV | DTE_MODE_4_LEVEL | empty_root, 0, 0, 0])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The two device table entries, spelled against Table 7**: the host-speed twin of
    /// `no_amd_vi_device_table_entry_sets_a_reserved_bit`, over the roots where a mask goes wrong
    /// (the top and bottom of the field, one bit past each end, an unaligned one).
    #[test]
    fn the_amd_vi_device_table_entries_are_what_table_7_says() {
        assert_eq!(
            AmdVi::device_table_entry(0x1234_5000, 7),
            Some([0x6000_0000_1234_5803, 7, 0, 0])
        );
        assert_eq!(
            AmdVi::blocked_device_table_entry(0x9000),
            Some([0x0000_0000_0000_9803, 0, 0, 0])
        );
        for root in [0x1000, 0x000f_ffff_ffff_f000] {
            let e = AmdVi::device_table_entry(root, 0xffff).unwrap();
            assert_eq!(e[0] & !AMD_VI_DTE_PERMITTED_BITS, 0);
            assert_eq!(e[0] & 0x000f_ffff_ffff_f000, root);
        }
        for root in [0x0010_0000_0000_0000, 0x1234_5800, u64::MAX] {
            assert_eq!(AmdVi::device_table_entry(root, 1), None, "{root:#x}");
            assert_eq!(AmdVi::blocked_device_table_entry(root), None, "{root:#x}");
        }
    }

    /// **Every leaf is present and accessed**, so no page faults on its own first touch.
    #[test]
    fn every_leaf_is_present_and_accessed() {
        for flags in [
            Flags::kernel_code(),
            Flags::kernel_data(),
            Flags::user_code(),
            Flags::user_data(),
        ] {
            let leaf = Ia32e::leaf_entry(0x10_0000, flags);
            assert_ne!(leaf & P, 0, "not present: {flags:?}");
            assert_ne!(leaf & A, 0, "accessed bit not set: {flags:?}");
        }
    }

    /// **A writable leaf is pre-dirtied, and a read-only one has nothing to dirty.** Neither bit
    /// reaches the portable `Flags`, so the round-trip test below cannot see D go missing.
    #[test]
    fn a_writable_leaf_is_pre_dirtied() {
        assert_ne!(Ia32e::leaf_entry(0x10_0000, Flags::kernel_data()) & D, 0);
        assert_eq!(Ia32e::leaf_entry(0x10_0000, Flags::kernel_code()) & D, 0);
    }

    /// **Nothing writable is also executable.** W^X is a property of the portable `Flags`
    /// constructors, and this asserts the encoder does not manufacture the combination on the way
    /// down: every entry that lacks XD must lack RW.
    #[test]
    fn no_encoded_leaf_is_both_writable_and_executable() {
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
            let leaf = Ia32e::leaf_entry(0x10_0000, flags);
            if leaf & XD == 0 {
                assert_eq!(leaf & RW, 0, "writable and executable: {flags:?}");
            }
        }
    }

    /// **A table-pointer entry grants everything**, because on x86 the levels are `AND`ed and a
    /// restrictive intermediate entry would silently override the leaf. This is the one place the
    /// three formats genuinely disagree, so it gets its own test rather than a comment.
    #[test]
    fn a_table_entry_grants_everything_and_leaves_the_leaf_to_decide() {
        let e = Ia32e::table_entry(0x20_0000, 0);
        assert_ne!(e & P, 0);
        assert_ne!(
            e & RW,
            0,
            "a read-only intermediate would veto a writable leaf"
        );
        assert_ne!(
            e & US,
            0,
            "a supervisor intermediate would veto a user leaf"
        );
        assert_eq!(
            e & XD,
            0,
            "an XD intermediate would veto an executable leaf"
        );
        assert_eq!(Ia32e::entry_pa(e), 0x20_0000);
    }

    /// **Every constructor round-trips through encode/decode**, including the two software bits
    /// that exist precisely because the hardware encoding loses the distinction.
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
            Flags::write_combining(),
            Flags::user_write_combining(),
        ] {
            let leaf = Ia32e::leaf_entry(0x10_0000, flags);
            assert_eq!(Ia32e::entry_pa(leaf), 0x10_0000);
            assert_eq!(
                Ia32e::leaf_flags(leaf),
                flags,
                "round-trip failed for {flags:?}"
            );
        }
    }

    /// **Kernel code and kernel rodata differ only in the software bit**, which is the whole reason
    /// that bit exists: both are supervisor-only, and x86 has no per-ring execute bit to tell them
    /// apart the way it would need to.
    #[test]
    fn kernel_code_and_rodata_are_distinguished_by_the_software_bit() {
        let code = Ia32e::leaf_entry(0x10_0000, Flags::kernel_code());
        let rodata = Ia32e::leaf_entry(0x10_0000, Flags::kernel_rodata());
        assert_ne!(code & SW_KERNEL_EXEC, 0);
        assert_eq!(rodata & SW_KERNEL_EXEC, 0);
        assert_eq!(code & XD, 0, "kernel code must be executable");
        assert_ne!(rodata & XD, 0, "kernel rodata must not be executable");
    }

    /// **Device memory is uncacheable.** A cached device register reads back a value the device
    /// never produced, and that failure looks like a driver bug for a very long time.
    #[test]
    fn device_memory_is_uncacheable() {
        for flags in [Flags::device(), Flags::user_device()] {
            let leaf = Ia32e::leaf_entry(0xfee0_0000, flags);
            assert_ne!(leaf & PCD, 0, "cacheable device page: {flags:?}");
            // PCD alone selects PAT entry 2, which is UC-, and an MTRR can weaken UC- to
            // write-combining. PWT with it selects entry 3, strong UC, which nothing overrides.
            assert_ne!(leaf & PWT, 0, "UC- rather than UC: {flags:?}");
        }
    }

    /// **A write-combining page selects PAT entry 1, and entry 1 is write-combining.** Checked as
    /// a pair because either half alone proves nothing: the encoder could pick an entry the
    /// programmed value never changed, or the value could change an entry nobody selects. The other
    /// entries this tree uses (0 for memory, 3 for devices) must be what they were at reset.
    #[test]
    fn a_write_combining_page_selects_the_entry_that_is_write_combining() {
        let leaf = Ia32e::leaf_entry(0xd000_0000, Flags::write_combining());
        let entry = (leaf & PWT != 0) as u64 | ((leaf & PCD != 0) as u64) << 1;
        assert_eq!(entry, 1, "PWT alone");
        let memory_type = |table: u64, entry: u64| (table >> (entry * 8)) & 0xff;
        assert_eq!(memory_type(pat::VALUE, 1), 0x01, "WC");
        for unchanged in [0, 2, 3, 4, 5, 6, 7] {
            assert_eq!(
                memory_type(pat::VALUE, unchanged),
                memory_type(pat::RESET, unchanged),
                "entry {unchanged}"
            );
        }
        let device = Ia32e::leaf_entry(0xd000_0000, Flags::device());
        assert_ne!(device & PCD, 0, "a register stays strong UC");
    }

    /// **A leaf's only bits are R, W and the address.** This is the property that matters most for
    /// [`Vtd`]: verified against QEMU's `VTD_SPTE_PAGE_L1_RSVD_MASK`, any bit outside `R`/`W`/the
    /// address field is reserved-must-be-zero at a second-level leaf, and QEMU's model checks it.
    /// Every `Flags` constructor is tried, including the ones (`user_code`, `kernel_data`, ...)
    /// that would set `US` or `XD` on `Ia32e`'s encoding of the same flags.
    ///
    /// The permitted set is a literal, [`VTD_PERMITTED_BITS`], which says why.
    #[test]
    fn a_vtd_leaf_sets_no_bit_outside_read_write_and_address() {
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
            for pa in [
                0x10_0000,
                // Bits above 51 set: with a small address `(pa & M) == pa` for every wide enough
                // M, and a widened `VTD_ADDR_MASK` would go unseen.
                0x00ff_ffff_ffff_f000,
                0xffff_ffff_ffff_f000,
            ] {
                let leaf = Vtd::leaf_entry(pa, flags);
                assert_eq!(
                    leaf & !VTD_PERMITTED_BITS,
                    0,
                    "a reserved bit was set for {flags:?} at {pa:#x}: {leaf:#x}"
                );
            }
        }
    }

    /// **The domain builder's only flags, `Flags::user_data`, round-trips on what VT-d actually
    /// has to say: writable.** `CAP_USER` has no VT-d encoding (see [`Vtd`]'s own doc) and is not
    /// expected back.
    #[test]
    fn a_vtd_leaf_is_present_and_writable_for_user_data() {
        let leaf = Vtd::leaf_entry(0x10_0000, Flags::user_data());
        assert!(Vtd::is_present(leaf));
        assert_eq!(Vtd::entry_pa(leaf), 0x10_0000);
        assert!(Vtd::leaf_flags(leaf).is_writable());
    }

    /// A read-only mapping is present (R alone means present) but not writable, and an entry with
    /// neither bit is not present at all: VT-d has no separate present bit, so this is the whole
    /// present/absent story.
    #[test]
    fn read_alone_is_present_but_not_writable_and_zero_is_absent() {
        let ro = Vtd::leaf_entry(0x10_0000, Flags::kernel_rodata());
        assert!(Vtd::is_present(ro));
        assert!(!Vtd::leaf_flags(ro).is_writable());
        assert!(!Vtd::is_present(0), "R and W both clear: not present");
    }

    /// **A VT-d intermediate entry grants everything**, the same reasoning as `Ia32e`'s table
    /// entries and for the same reason: VT-d ANDs `R`/`W` down the walk, so a restrictive
    /// intermediate would veto a writable leaf beneath it.
    #[test]
    fn a_vtd_table_entry_grants_everything() {
        let e = Vtd::table_entry(0x20_0000, 0);
        assert_eq!(e & (VTD_R | VTD_W), VTD_R | VTD_W);
        assert_eq!(Vtd::entry_pa(e), 0x20_0000);
        assert_eq!(
            e & !(VTD_ADDR_MASK | VTD_R | VTD_W),
            0,
            "no reserved bit in a table entry either"
        );
    }

    /// **An AMD-Vi leaf sets no bit outside `PR`, the address, `FC`, `IR` and `IW`**, for every
    /// `Flags` constructor and for addresses with bits above 51 set, which is the input that asks
    /// the encoder to mask (the same reasoning as the VT-d test above). Reserved bits 58:52 fault
    /// the walk, and a nonzero `NextLevel` in a bottom-level entry would read as a directory.
    #[test]
    fn an_amd_vi_leaf_sets_no_bit_outside_its_permitted_set() {
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
            for pa in [0x10_0000, 0x00ff_ffff_ffff_f000, 0xffff_ffff_ffff_f000] {
                let leaf = AmdVi::leaf_entry(pa, flags);
                assert_eq!(
                    leaf & !AMD_VI_LEAF_PERMITTED_BITS,
                    0,
                    "a reserved bit was set for {flags:?} at {pa:#x}: {leaf:#x}"
                );
                assert_eq!(leaf & 0xe00, 0, "a 4 KiB leaf's NextLevel is 0");
                assert_eq!(leaf & 1, 1, "a leaf is present");
                assert_eq!(leaf & (1 << 61), 1 << 61, "every leaf is readable");
                assert_eq!(leaf & (1 << 60), 1 << 60, "every leaf forces coherence");
            }
        }
    }

    /// **A directory entry names the level it points at**, 3 from the top table down to 1 from
    /// the last directory, grants both permissions, and sets no reserved bit (bit 60 included,
    /// which a leaf may carry and a directory may not). The level arithmetic is the thing a wrong
    /// value would turn into a skipped level, so each level is spelled out rather than derived.
    #[test]
    fn an_amd_vi_directory_entry_names_the_next_level() {
        for (walk_level, next_level) in [(0usize, 3u64), (1, 2), (2, 1)] {
            for pa in [0x20_0000, 0xffff_ffff_ffff_f000] {
                let e = AmdVi::table_entry(pa, walk_level);
                assert_eq!((e >> 9) & 7, next_level, "walk level {walk_level}");
                assert_eq!(e & !AMD_VI_DIRECTORY_PERMITTED_BITS, 0, "{e:#x}");
                assert_eq!(e & (3 << 61), 3 << 61, "a directory grants read and write");
                assert_eq!(AmdVi::entry_pa(e), pa & 0x000f_ffff_ffff_f000);
                assert!(AmdVi::is_present(e));
                assert!(!AmdVi::is_block(e), "a directory is not a page");
            }
        }
    }

    /// **The permissions round-trip, and only pages are written.** `IW` is what `Flags` gets back;
    /// a page translation entry met above the bottom (`NextLevel` 0 or 7) stops a walk.
    #[test]
    fn an_amd_vi_leaf_reads_back_and_pages_are_the_only_size() {
        let rw = AmdVi::leaf_entry(0x10_0000, Flags::user_data());
        let ro = AmdVi::leaf_entry(0x10_0000, Flags::kernel_rodata());
        assert!(AmdVi::leaf_flags(rw).is_writable());
        assert!(!AmdVi::leaf_flags(ro).is_writable());
        assert!(!AmdVi::is_present(0));
        assert!(AmdVi::is_block(rw));
        assert!(AmdVi::is_block(1 | (7 << 9)));
        let data = Flags::user_data();
        assert_eq!(
            AmdVi::block_entry(0x10_0000, data, PageSize::Size4KiB),
            Some(rw)
        );
        assert_eq!(
            AmdVi::block_entry(0x20_0000, data, PageSize::Size2MiB),
            None
        );
    }

    /// **Execute permission reads back from an `Ia32e` leaf**, both rings. The software bit that
    /// tells kernel code from kernel rodata is only consulted when `XD` is clear, so an `XD` test
    /// that is always true or always false loses exactly the pages that run.
    #[test]
    fn execute_permission_reads_back_from_a_leaf() {
        for flags in [
            Flags::kernel_code(),
            Flags::user_code(),
            Flags::kernel_rodata(),
            Flags::user_rodata(),
        ] {
            assert_eq!(
                Ia32e::leaf_flags(Ia32e::leaf_entry(0x10_0000, flags)),
                flags
            );
        }
    }

    /// **VT-d writes pages only, and reads bit 7 as a superpage.** `block_entry` refusing a 4 KiB
    /// "block" would make every DMA grant fail; `is_block` must agree with the hardware about an
    /// entry this format never writes but a walk may still meet.
    #[test]
    fn vtd_writes_pages_only_and_knows_a_superpage_when_it_sees_one() {
        let data = Flags::user_data();
        assert_eq!(
            Vtd::block_entry(0x10_0000, data, PageSize::Size4KiB),
            Some(Vtd::leaf_entry(0x10_0000, data))
        );
        assert_eq!(Vtd::block_entry(0x20_0000, data, PageSize::Size2MiB), None);
        assert!(Vtd::is_block(VTD_R | (1 << 7)));
        assert!(!Vtd::is_block(Vtd::leaf_entry(0x10_0000, data)));
        assert!(!Vtd::is_block(Vtd::table_entry(0x10_0000, 0)));
    }

    /// A leaf with no software kernel-exec bit is not kernel code, however its other bits sit. The
    /// bit is the only thing that says a supervisor page is executable.
    #[test]
    fn a_kernel_leaf_without_the_exec_bit_is_not_executable() {
        assert_eq!(Ia32e::leaf_flags(P | RW | G), Flags::kernel_data());
        assert_eq!(Ia32e::leaf_flags(P | G), Flags::kernel_rodata());
        assert_eq!(
            Ia32e::leaf_flags(P | G | SW_KERNEL_EXEC),
            Flags::kernel_code()
        );
    }
}

/// Machine-checked proofs of the `x86_64` format, mirroring the other two modules'. The shared
/// `Mapper` walk inherits all three formats' guarantees. See notes/verification.md.
#[cfg(kani)]
mod verification {
    use super::*;
    use crate::{Half, PAGE_SIZE, PageSize};

    /// **No AMD-Vi device table entry ever sets a reserved bit, and none loses its root**, over
    /// every `u64` root and every domain id (provisional milestone 767 (AMD-Vi hardening before the
    /// first AMD boot)). The twin of `no_vtd_entry_ever_sets_a_reserved_bit`, and spelled the same
    /// way: every assertion is a literal from Table 7 rather than one of the constants the encoder
    /// is built from, so widening the root mask or moving a field fails here instead of moving the
    /// check with it. A refused root must be one the 51:12 field cannot hold; an accepted one must
    /// read back whole, so a mask that narrowed the root silently fails too.
    /// Falsification: replayable `crates/paging/falsifications/x86_64.verification.no_amd_vi_device_table_entry_sets_a_reserved_bit.patch`
    #[kani::proof]
    fn no_amd_vi_device_table_entry_sets_a_reserved_bit() {
        const ROOT_BITS: u64 = 0x000f_ffff_ffff_f000;
        let root: u64 = kani::any();
        let domain: u16 = kani::any();
        let fits = root & !ROOT_BITS == 0;

        match AmdVi::device_table_entry(root, domain) {
            None => assert!(!fits, "a root the field can hold was refused"),
            Some(e) => {
                assert!(fits, "a root the field cannot hold was accepted");
                assert_eq!(
                    e[0] & !AMD_VI_DTE_PERMITTED_BITS,
                    0,
                    "a reserved bit is set"
                );
                assert_eq!(e[0] & ROOT_BITS, root, "the root did not read back whole");
                assert_eq!(e[0] & 0b11, 0b11, "V and TV");
                assert_eq!((e[0] >> 9) & 0b111, 0b100, "Mode is not four levels");
                assert_eq!((e[0] >> 61) & 0b11, 0b11, "IR and IW");
                assert_eq!(e[1], domain as u64, "anything but DomainID in bits 127:64");
                assert_eq!((e[2], e[3]), (0, 0), "interrupt remapping or reserved bits");
            }
        }
        match AmdVi::blocked_device_table_entry(root) {
            None => assert!(!fits, "a root the field can hold was refused"),
            Some(e) => {
                assert!(fits, "a root the field cannot hold was accepted");
                assert_eq!(
                    e[0] & !AMD_VI_DTE_PERMITTED_BITS,
                    0,
                    "a reserved bit is set"
                );
                assert_eq!(e[0] & ROOT_BITS, root, "the root did not read back whole");
                assert_eq!((e[0] >> 61) & 0b11, 0, "the blocked entry grants IR or IW");
                assert_eq!((e[0] >> 9) & 0b111, 0b100, "Mode is not four levels");
                assert_eq!((e[1], e[2], e[3]), (0, 0, 0));
            }
        }
    }

    /// **The walk never indexes past a table** (four levels here).
    /// Falsification: replayable `crates/paging/falsifications/x86_64.verification.index_is_always_in_bounds.patch`
    #[kani::proof]
    fn index_is_always_in_bounds() {
        let va: u64 = kani::any();
        let level: usize = kani::any();
        kani::assume(level < Ia32e::LEVELS);
        assert!(Ia32e::index(va, level) < crate::ENTRIES);
    }

    /// **The four indices and the offset tile the low 48 bits exactly.**
    /// Falsification: replayable `crates/paging/falsifications/x86_64.verification.the_indices_and_offset_tile_the_address.patch`
    #[kani::proof]
    fn the_indices_and_offset_tile_the_address() {
        let va: u64 = kani::any();
        let reconstructed = ((Ia32e::index(va, 0) as u64) << 39)
            | ((Ia32e::index(va, 1) as u64) << 30)
            | ((Ia32e::index(va, 2) as u64) << 21)
            | ((Ia32e::index(va, 3) as u64) << 12)
            | (va & (PAGE_SIZE - 1));
        assert_eq!(reconstructed, va & 0x0000_ffff_ffff_ffff);
    }

    /// **Distinct pages take distinct paths** within the 48-bit VA.
    /// Falsification: replayable `crates/paging/falsifications/x86_64.verification.distinct_pages_take_distinct_paths.patch`
    #[kani::proof]
    fn distinct_pages_take_distinct_paths() {
        let a: u64 = kani::any::<u64>() & 0x0000_ffff_ffff_f000;
        let b: u64 = kani::any::<u64>() & 0x0000_ffff_ffff_f000;
        kani::assume(
            Ia32e::index(a, 0) == Ia32e::index(b, 0)
                && Ia32e::index(a, 1) == Ia32e::index(b, 1)
                && Ia32e::index(a, 2) == Ia32e::index(b, 2)
                && Ia32e::index(a, 3) == Ia32e::index(b, 3),
        );
        assert_eq!(a, b);
    }

    /// **The two halves are disjoint** at the canonical split (bit 47).
    /// Falsification: replayable `crates/paging/falsifications/x86_64.verification.the_two_halves_are_disjoint.patch`
    #[kani::proof]
    fn the_two_halves_are_disjoint() {
        let va: u64 = kani::any();
        assert!(!(Ia32e::is_in_half(Half::Low, va) && Ia32e::is_in_half(Half::High, va)));
    }

    /// **The user-VA gate admits exactly the aligned low half**, never the high one.
    /// Falsification: replayable `crates/paging/falsifications/x86_64.verification.the_user_va_gate_admits_only_the_aligned_low_half.patch`
    #[kani::proof]
    fn the_user_va_gate_admits_only_the_aligned_low_half() {
        let va: u64 = kani::any();
        assert_eq!(
            crate::is_user_page_va::<Ia32e>(va),
            va & 0xfff == 0 && va >> 47 == 0
        );
        if crate::is_user_page_va::<Ia32e>(va) {
            assert!(Ia32e::is_in_half(Half::Low, va) && !Ia32e::is_in_half(Half::High, va));
        }
    }

    /// **A leaf keeps the address and the permissions apart, and the permissions round-trip.**
    /// Falsification: replayable `crates/paging/falsifications/x86_64.verification.the_leaf_keeps_address_and_permissions_apart.patch`
    #[kani::proof]
    fn the_leaf_keeps_address_and_permissions_apart() {
        let pa: u64 = kani::any();
        kani::assume(pa & !ADDR_MASK == 0);

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

        let leaf = Ia32e::leaf_entry(pa, flags);
        // **The address field read out of the word, not through `entry_pa`** (milestone 211).
        // `leaf_entry` and `entry_pa` are an encoder and its own decoder, so a round trip
        // between them is satisfied by any pair that agree: a shift wrong in both would leave
        // this green while the hardware read the wrong page. The line below states where the
        // architecture puts the address, which is the one thing the implementation does not
        // get to choose, and it is spelled as a literal rather than through this crate's own
        // ADDR_MASK or PPN_SHIFT: a defect in one of those constants moves the implementation
        // and any harness that cited it together, which is the same trap one level down.
        //
        // **This comment used to say `no_vtd_entry_ever_sets_a_reserved_bit` already worked this
        // way. It did not** (milestone 307): that harness stated its address property through
        // `VTD_ADDR_MASK`, which is the constant its encoder builds the entry out of, so the
        // property held for every value of the constant. The citation was the only place in the
        // tree claiming the trap had been avoided there, and it was written by a lane that had
        // just avoided it here. Both are literals now.
        assert_eq!(
            leaf & 0x000f_ffff_ffff_f000,
            pa,
            "the address left bits 12..52 of the entry"
        );
        assert_eq!(leaf & 1, 1, "a leaf must have the present bit set");
        assert_eq!(Ia32e::entry_pa(leaf), pa);
        assert_eq!(Ia32e::leaf_flags(leaf), flags);
    }

    /// **No encoded leaf is both writable and executable**, over every constructor. The W^X
    /// property is enforced by the portable `Flags` constructors; this proves the x86 encoder does
    /// not reintroduce the combination while translating them.
    /// Falsification: replayable `crates/paging/falsifications/x86_64.verification.no_encoded_leaf_is_both_writable_and_executable.patch`
    #[kani::proof]
    fn no_encoded_leaf_is_both_writable_and_executable() {
        let pa: u64 = kani::any();
        kani::assume(pa & !ADDR_MASK == 0);

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

        let leaf = Ia32e::leaf_entry(pa, all[i]);
        assert!(leaf & XD != 0 || leaf & RW == 0);
    }

    /// **A block keeps the address and the permissions apart, carries `PS`, and never sets a bit
    /// the large-page format reserves**, for every physical address and every `Flags` constructor,
    /// at both block sizes. Every bit position is a literal, for the reason
    /// `the_leaf_keeps_address_and_permissions_apart` gives: the architecture chooses them.
    ///
    /// No assumption on `pa`: masking it down to the block's alignment and the 52-bit field is the
    /// encoder's job, and a stray bit below the alignment is a reserved-bit fault (bits 20:13 or
    /// 29:13) or a different memory type (bit 12, `PAT`), so the claim is over every `u64`.
    /// Falsification: replayable `crates/paging/falsifications/x86_64.verification.a_block_keeps_address_and_permissions_apart.patch`
    #[kani::proof]
    fn a_block_keeps_address_and_permissions_apart() {
        let pa: u64 = kani::any();
        let two_mib: bool = kani::any();
        let (size, address_bits) = if two_mib {
            (PageSize::Size2MiB, 0x000f_ffff_ffe0_0000u64)
        } else {
            (PageSize::Size1GiB, 0x000f_ffff_c000_0000u64)
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

        let block = Ia32e::block_entry(pa, flags, size).expect("x86 encodes both block sizes");
        assert_eq!(
            block & address_bits,
            pa & address_bits,
            "the address left its field"
        );
        assert_eq!(
            block & 0x000f_ffff_ffff_f000 & !address_bits,
            0,
            "a bit below the block's alignment is set: reserved, or PAT"
        );
        assert_eq!(block & 1, 1, "a block must be present");
        assert_eq!(block & (1 << 7), 1 << 7, "a block must carry PS");
        assert!(Ia32e::is_block(block));
        assert_eq!(Ia32e::leaf_flags(block), flags);
        // W^X survives the move up a level, stated on the hardware bits: executable (XD clear)
        // means not writable (RW clear).
        assert!(block & (1 << 63) != 0 || block & (1 << 1) == 0);
    }

    /// **No table pointer ever reads as a block**, for every address: `table_entry` keeps `PS`
    /// clear, so the walk never stops early at an entry that points at a table and never takes a
    /// table's frame for 2 MiB of mapped memory.
    /// Falsification: replayable `crates/paging/falsifications/x86_64.verification.a_table_entry_is_never_a_block.patch`
    #[kani::proof]
    fn a_table_entry_is_never_a_block() {
        let pa: u64 = kani::any();
        assert!(!Ia32e::is_block(Ia32e::table_entry(pa, 0)));
    }

    /// **No `Vtd` leaf or table entry ever sets a bit VT-d treats as reserved**, over every
    /// physical address and every portable `Flags` constructor. This is the property
    /// `a_vtd_leaf_sets_no_bit_outside_read_write_and_address` checks by example; Kani closes it
    /// for every address and every flag combination the type accepts, which matters here more than
    /// on `Ia32e` because QEMU's model (and real silicon) faults a transaction over a reserved bit
    /// rather than merely ignoring it.
    ///
    /// # Every assertion here is spelled in literals, and milestone 307 is why
    ///
    /// This harness used to state all three of its assertions, and its `assume`, through
    /// `VTD_ADDR_MASK`, `VTD_R` and `VTD_W`: the same three constants `Vtd::leaf_entry` and
    /// `Vtd::table_entry` build their result out of. `(pa & M) | bits` sets no bit outside
    /// `M | VTD_R | VTD_W` **for every value of M**, so the address half of the reserved-bit claim
    /// was a tautology and this harness could not fail on it. Widening `VTD_ADDR_MASK` over bits
    /// 52..63, which this crate's own note says has never been narrowed to the `CAP_REG.MGAW` the
    /// hardware reports, moves the encoder and the assertion together; every DMA the IOMMU
    /// translates then faults on a reserved bit while the proof stays green, and the symptom is a
    /// device that reads as broken rather than a table that reads as wrong.
    ///
    /// It was found by asking which assertion fires when the claim is broken, and it is worth the
    /// paragraph because the tree had recorded the opposite: the comment on
    /// `the_leaf_keeps_address_and_permissions_apart` cited *this* harness as already avoiding the
    /// trap it was demonstrating. [`VTD_PERMITTED_BITS`] carries the argument in full.
    /// Falsification: replayable `crates/paging/falsifications/x86_64.verification.no_vtd_entry_ever_sets_a_reserved_bit.patch`
    #[kani::proof]
    fn no_vtd_entry_ever_sets_a_reserved_bit() {
        // The address field, bits 51:12, written out rather than cited.
        const ADDRESS_BITS: u64 = 0x000f_ffff_ffff_f000;

        // **No assumption on `pa`, and that is half the fix.** This harness used to open with
        // `kani::assume(pa & !VTD_ADDR_MASK == 0)`, which defines the input space in terms of the
        // constant under test: widen the mask and the harness quietly admits the very addresses
        // that would now reach the entry, so the defect and the guard against it move together and
        // the proof stays green. Masking the address down is `leaf_entry`'s own job, so the claim
        // is over *every* `u64`, and the address assertions below say `pa & ADDRESS_BITS` rather
        // than `pa` because that is what a correct encoder keeps.
        let pa: u64 = kani::any();

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

        let leaf = Vtd::leaf_entry(pa, all[i]);
        assert_eq!(
            leaf & !VTD_PERMITTED_BITS,
            0,
            "a VT-d leaf set a bit the hardware reserves",
        );
        // Where the architecture puts the address, not where this crate says it does. The
        // `entry_pa` round trip below is an encoder and its own decoder, so a shift wrong in both
        // satisfies it; this line is what that round trip is checked against.
        assert_eq!(
            leaf & ADDRESS_BITS,
            pa & ADDRESS_BITS,
            "the address left bits 12..52",
        );
        assert_eq!(Vtd::entry_pa(leaf), pa & ADDRESS_BITS);

        let table = Vtd::table_entry(pa, 0);
        assert_eq!(
            table & !VTD_PERMITTED_BITS,
            0,
            "a VT-d table entry set a bit the hardware reserves",
        );
        assert_eq!(
            table & ADDRESS_BITS,
            pa & ADDRESS_BITS,
            "the address left bits 12..52",
        );
    }
}

/// **The eighth outsider pass's host attack on the VT-d encoder** (claim 12): a physical address
/// built to carry a payload in every reserved position. Name: provisional (this pass's lane,
/// 2026-10-13 UTC).
#[cfg(test)]
mod eighth_pass_attacks {
    use super::*;

    /// **A hostile physical address cannot smuggle reserved bits into an entry.**
    ///
    /// The address is built with a payload in every bit the format reserves: the low nibble's
    /// bits 2 and 3, bits 4 through 9, and everything above bit 51, the set QEMU's
    /// `VTD_SPTE_PAGE_L1_RSVD_MASK` faults on. `leaf_entry` must mask the address down to
    /// `VTD_ADDR_MASK` and leave the payload nowhere: the encoded leaf is exactly the permitted
    /// bits, and `entry_pa` reads back the masked address rather than the hostile one.
    #[test]
    fn a_hostile_address_cannot_smuggle_reserved_bits_through_vtd() {
        let payload = !VTD_ADDR_MASK;
        let pa = (0x0040_0000_0000_0000 & VTD_ADDR_MASK) | payload;
        assert_ne!(
            pa & VTD_ADDR_MASK,
            pa,
            "the fixture: the payload is in the mask"
        );

        for flags in [Flags::user_data(), Flags::kernel_rodata()] {
            let leaf = Vtd::leaf_entry(pa, flags);
            assert_eq!(
                leaf & !VTD_PERMITTED_BITS,
                0,
                "a reserved bit survived the encode for {flags:?}: {leaf:#x}",
            );
            assert_eq!(
                Vtd::entry_pa(leaf),
                pa & VTD_ADDR_MASK,
                "the hostile payload reached entry_pa for {flags:?}",
            );
        }
    }
}
