//! Tests for the Sv39 MMU: the live page tables, W^X, the guard page, and TLB invalidation.
//!
//! The twins of aarch64's, against the same properties (DECISIONS §19 (architectural parity is a
//! tenet)). `translate` walks the live kernel root, so they inspect the tables the hardware walks.
//! See notes/riscv-port/memory.md.

/// A physical address one gigabyte above the top of every RAM region this machine described, for
/// tests that need a spare page outside the direct map. Read from the machine, since a fixed
/// address that is not RAM on QEMU is RAM on the VisionFive 2.
fn a_physical_address_outside_every_ram_region() -> u64 {
    let top = crate::memory::ram_regions()
        .map(|(start, size)| start + size)
        .max()
        .expect("a booted kernel has at least one RAM region");
    // Page-aligned, because RAM regions are, and clear of generous rounding.
    top + (1 << 30)
}

/// The trust flag agrees with the measured ASID width, whatever that width is.
///
/// RISC-V permits zero bits, and the VisionFive 2's U74 implements zero, so the test asserts that
/// the kernel knows whether the width is enough rather than that it is. See
/// notes/riscv-port/memory.md.
#[test_case]
fn the_hardware_has_at_least_the_asid_bits_the_allocator_assumes() {
    let bits = super::asid_bits();
    assert!(
        bits <= super::SATP_ASID_WIDTH as usize,
        "satp.ASID reported {bits} implemented bits, wider than the architectural 16",
    );
    assert_eq!(
        super::asid_tagging_is_trusted(),
        bits >= super::ASID_BITS_NEEDED as usize,
        "satp.ASID implements {bits} bits, address_space_identifier::ASIDS needs {}: the trust flag must agree \
         with whether this width holds every tag the allocator can hand out. If it does not, \
         either the flush stays where it should have been dropped, or address spaces would \
         alias in the TLB with no flush catching it.",
        address_space_identifier::ASIDS,
    );
}

/// [`is_enabled`](super::is_enabled) reads `satp.MODE` (bits 63:60) correctly. Running at
/// `KERNEL_VA_BASE` already proves paging is on.
#[test_case]
fn mmu_is_enabled() {
    assert!(crate::arch::mmu::is_enabled(), "satp.MODE reads as Bare");
}

/// The kernel's code, stack and statics are in the Sv39 high half, which every process root
/// shares (`share_kernel_half`), so a switch into userspace keeps the kernel mapped.
#[test_case]
fn the_kernel_lives_in_the_high_half() {
    use crate::arch::mmu::KERNEL_VA_BASE;

    let pc = crate::kernel_main as *const () as u64;
    assert!(
        pc >= KERNEL_VA_BASE,
        "kernel .text is at {pc:#x}, not in the high half"
    );

    let sp = crate::arch::current_sp();
    assert!(
        sp >= KERNEL_VA_BASE,
        "the stack is at {sp:#x}, not in the high half"
    );

    static IN_BSS: u64 = 0;
    let addr = (&raw const IN_BSS) as u64;
    assert!(
        addr >= KERNEL_VA_BASE,
        "a static is at {addr:#x}, not in the high half"
    );
}

/// The kernel root's low half is empty, so with no process running a low address faults.
///
/// The addresses are inside Sv39's low half on purpose: the `Mapper` answers `None` outside its
/// half before reading any table, which would prove nothing.
#[test_case]
fn a_low_address_does_not_translate_when_no_process_is_running() {
    use crate::arch::mmu::translate_user;

    for va in [
        0x1000u64,
        0x8020_0000, // where OpenSBI loaded us: the identity map, if it survived
        0x0000_003f_ffff_f000, // the top page of the Sv39 low half
    ] {
        assert!(
            translate_user(va).is_none(),
            "{va:#x} translates through the live satp: the boot table's identity gigapages \
             may still be live",
        );
    }
}

/// A frame the allocator hands out is writable memory at `pa + KERNEL_VA_BASE`.
#[test_case]
fn the_direct_map_reaches_physical_memory() {
    use crate::arch::mmu::{phys_to_virt, virt_to_phys};

    let frame = crate::memory::alloc().expect("out of memory");
    let va = phys_to_virt(frame.addr());

    assert_eq!(
        virt_to_phys(va),
        frame.addr(),
        "the transform is not reversible"
    );

    let (pa, flags) = crate::arch::mmu::translate(va).expect("frame is NOT in the direct map");
    assert_eq!(pa, frame.addr());
    assert!(flags.is_writable());

    // SAFETY: the allocator just gave us this frame exclusively.
    unsafe {
        core::ptr::write_volatile(va as *mut u64, 0xfeed_face_cafe_f00d);
        assert_eq!(
            core::ptr::read_volatile(va as *const u64),
            0xfeed_face_cafe_f00d
        );
    }

    crate::memory::free(frame);
}

/// The guard page is unmapped and the pages either side of it are mapped, so a linker-script edit
/// that moved `__stack_guard` fails here.
#[test_case]
fn the_guard_page_is_a_hole() {
    use crate::arch::mmu;
    assert_eq!(
        mmu::translate(mmu::stack_guard()),
        None,
        "the guard page IS mapped: a stack overflow would silently eat .bss"
    );

    assert!(
        mmu::translate(mmu::stack_guard() - 4096).is_some(),
        "below the guard"
    );
    assert!(
        mmu::translate(mmu::stack_bottom()).is_some(),
        "the stack itself"
    );
}

/// Kernel `.text` is executable, not writable, and not user-executable, in the live tables.
///
/// Sv39 has one `X` bit, so the user-exec assertion cannot fail today; it states the property for a
/// format with separate bits.
#[test_case]
fn kernel_text_is_executable_and_not_writable() {
    use crate::arch::mmu;

    let (pa, flags) = mmu::translate(mmu::text_start()).expect(".text is not mapped");
    assert_eq!(
        pa,
        mmu::virt_to_phys(mmu::text_start()),
        ".text maps to the wrong frame"
    );

    assert!(flags.is_kernel_executable(), ".text is not executable");
    assert!(!flags.is_writable(), ".text is WRITABLE: W^X is broken");
    assert!(!flags.is_user_executable(), ".text is executable by U-mode");
}

/// Constants are read-only, and not executable by anyone.
#[test_case]
fn kernel_rodata_is_read_only_and_not_executable() {
    use crate::arch::mmu;

    let (_, flags) = mmu::translate(mmu::rodata_start()).expect(".rodata is not mapped");
    assert!(!flags.is_writable(), ".rodata is writable");
    assert!(!flags.is_kernel_executable(), ".rodata is executable");
}

/// The stack is writable and NOT executable.
#[test_case]
fn the_stack_is_writable_and_not_executable() {
    use crate::arch::mmu;

    let (_, flags) = mmu::translate(mmu::stack_bottom()).expect("stack is not mapped");
    assert!(flags.is_writable());
    assert!(
        !flags.is_kernel_executable(),
        "the stack is EXECUTABLE: data on the stack could be run as code"
    );
}

/// The UART is mapped as device memory in the kernel's flags.
///
/// Base Sv39 has no memory-type field, so this checks the RSW bit `paging::Sv39` keeps, not what
/// the hardware was told. See notes/riscv-port/memory.md and notes/page-tables.md.
#[test_case]
fn the_uart_is_mapped_as_device_memory() {
    use crate::arch::mmu;

    // Through the direct map: the identity mapping went with the boot table.
    let (_, flags) =
        mmu::translate(mmu::phys_to_virt(super::UART_BASE)).expect("the UART is not mapped");

    assert!(flags.is_device(), "the UART is not device memory");
    assert!(flags.is_writable(), "we do need to write to it");
    assert!(!flags.is_kernel_executable());
}

/// An allocated frame is mapped writable and not executable, so the direct map, built with the
/// kernel image punched out of RAM, has no hole the allocator can reach.
#[test_case]
fn an_allocated_frame_is_reachable_through_the_mmu() {
    use crate::arch::mmu;

    let frame = crate::memory::alloc().expect("out of memory");
    let va = mmu::phys_to_virt(frame.addr());
    let (pa, flags) = mmu::translate(va).expect("allocated frame is NOT MAPPED");

    assert_eq!(pa, frame.addr());
    assert!(flags.is_writable());
    assert!(!flags.is_kernel_executable(), "RAM is executable");

    crate::memory::free(frame);
}

/// Unmapping invalidates the TLB: a spare address mapped to frame A and read, which loads the
/// TLB, then unmapped and mapped to frame B, must read B's pattern. A stale TLB reads A's, the
/// previous owner's data, with correct tables in memory.
#[test_case]
fn unmap_invalidates_the_tlb() {
    use paging::Flags;

    use crate::arch::mmu::{self, phys_to_virt};

    const PATTERN_A: u64 = 0xaaaa_aaaa_aaaa_aaaa;
    const PATTERN_B: u64 = 0xbbbb_bbbb_bbbb_bbbb;

    // Clear of every RAM region and device window, and still nameable in the 256 GiB high half.
    let test_va = mmu::phys_to_virt(a_physical_address_outside_every_ram_region());
    assert_eq!(
        mmu::translate(test_va),
        None,
        "test address is already in use"
    );

    let a = crate::memory::alloc().expect("out of memory");
    let b = crate::memory::alloc().expect("out of memory");

    // SAFETY: two frames the allocator just gave us exclusively, reached via the direct map.
    unsafe {
        core::ptr::write_volatile(phys_to_virt(a.addr()) as *mut u64, PATTERN_A);
        core::ptr::write_volatile(phys_to_virt(b.addr()) as *mut u64, PATTERN_B);
    }

    mmu::map_page(test_va, a.addr(), Flags::kernel_data()).expect("map A");

    // SAFETY: just mapped, writable.
    let seen = unsafe { core::ptr::read_volatile(test_va as *const u64) };
    assert_eq!(seen, PATTERN_A, "the mapping didn't take");
    // That read pulls the translation into the TLB.

    let returned = mmu::unmap_page(test_va).expect("unmap");
    assert_eq!(returned, a.addr(), "unmap returned the wrong frame");

    mmu::map_page(test_va, b.addr(), Flags::kernel_data()).expect("map B");

    // SAFETY: mapped again, to a different frame.
    let seen = unsafe { core::ptr::read_volatile(test_va as *const u64) };

    assert_eq!(
        seen, PATTERN_B,
        "STALE TLB: the same virtual address still reads the OLD frame's data. \
         This is the bug that reads back another process's memory."
    );

    mmu::unmap_page(test_va).expect("cleanup");
    crate::memory::free(a);
    crate::memory::free(b);
}

/// The kernel mapper refuses to overwrite a live leaf, which would leak the old frame unrecorded,
/// so a change goes through break-before-make as on aarch64.
#[test_case]
fn the_kernel_mapper_refuses_to_overwrite() {
    use paging::{Flags, MapError};

    use crate::arch::mmu;

    let va = mmu::phys_to_virt(a_physical_address_outside_every_ram_region());
    let f = crate::memory::alloc().unwrap();

    mmu::map_page(va, f.addr(), Flags::kernel_data()).unwrap();

    assert_eq!(
        mmu::map_page(va, f.addr(), Flags::kernel_data()),
        Err(MapError::AlreadyMapped)
    );

    mmu::unmap_page(va).unwrap();
    crate::memory::free(f);
}

/// `ttbr0_value` puts two live spaces' distinct, nonzero ASIDs at bits 59:44 and leaves MODE
/// and PPN intact.
///
/// That switching between them flushes nothing is
/// `system_tests/src/user/address_space_tests.rs`'s to prove (notes/riscv-tlb-shootdown.md).
#[test_case]
fn the_satp_carries_the_address_spaces_asid() {
    use crate::user::AddressSpace;

    let a = AddressSpace::new(2).expect("no space A");
    let b = AddressSpace::new(2).expect("no space B");

    // satp.ASID is bits 59:44 (16 bits) on rv64 Sv39.
    let asid_of = |satp: u64| (satp >> 44) & 0xffff;
    let (asid_a, asid_b) = (asid_of(a.ttbr0()), asid_of(b.ttbr0()));

    assert_ne!(asid_a, asid_b, "two live spaces share an ASID");
    assert_ne!(asid_a, 0, "a user space got the kernel's ASID 0");
    assert_ne!(asid_b, 0, "a user space got the kernel's ASID 0");

    // MODE, ASID and PPN abut, so a shift off by four lands the tag in a neighbour.
    for satp in [a.ttbr0(), b.ttbr0()] {
        assert_eq!(satp >> 60, 8, "satp.MODE is not Sv39: {satp:#x}");
    }
    assert_ne!(
        a.ttbr0() & super::SATP_PPN_MASK,
        b.ttbr0() & super::SATP_PPN_MASK,
        "two address spaces share a root table",
    );
}
