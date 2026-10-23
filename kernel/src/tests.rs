//! Tests for the boot path itself: the things `boot.s` and the boot protocol had to get
//! right before any other code could run at all.
//!
//! Everything else lives beside the code it tests. `cargo test -p kernel` still collects
//! all of them: `custom_test_frameworks` gathers every `#[test_case]` in the crate,
//! wherever it is.

/// Proves the harness itself works. If this fails, nothing else is meaningful.
#[test_case]
fn harness_runs() {
    // black_box so this is a real runtime check, not a constant clippy folds to `2 == 2`.
    let two = core::hint::black_box(1) + core::hint::black_box(1);
    assert_eq!(two, 2);
}

/// Proves `boot.s` zeroed `.bss`.
///
/// A zero-initialized static lands in `.bss`, which occupies no bytes in the
/// ELF file. Nobody loaded it. If our zeroing loop were wrong, this would hold
/// whatever garbage was in RAM at power-on. See notes/elf.md.
#[test_case]
fn bss_was_zeroed() {
    use core::sync::atomic::{AtomicU64, Ordering};
    static CANARY: AtomicU64 = AtomicU64::new(0);
    assert_eq!(CANARY.load(Ordering::Relaxed), 0);
}

/// Proves `boot.s` gave us a usable, correctly aligned stack.
///
/// Both aarch64 and RISC-V require a 16-byte-aligned `sp`, and both fault or corrupt
/// silently on a misaligned one, so a bug here would show up as a mysterious early crash
/// rather than as anything legible. Reads `sp` through `arch::current_sp`, so it is portable.
/// See notes/stack.md.
#[test_case]
fn stack_pointer_is_16_byte_aligned() {
    let sp = crate::arch::current_sp();
    assert_eq!(sp % 16, 0, "sp = {sp:#x}");
}

/// Proves we are where we think we are.
///
/// We run at EL1, whether the machine started us there or at EL2 and `boot.s` dropped down
/// (`virt,virtualization=on`, or U-Boot on a board; `arch::entry_el` records which). See
/// notes/aarch64.md.
///
/// RISC-V has no twin: S-mode cannot read its own privilege level, and
/// `arch::riscv64::exceptions`'s `breakpoint_is_caught_and_execution_resumes` proves it sideways,
/// since an M-mode `ebreak` would go to OpenSBI and never reach us.
#[cfg(target_arch = "aarch64")]
#[test_case]
fn running_at_el1() {
    use aarch64_cpu::registers::CurrentEL;
    use tock_registers::interfaces::Readable;
    assert_eq!(CurrentEL.read(CurrentEL::EL), 1);
}

/// Proves the boot protocol actually delivered a device tree.
///
/// The flat binary's arm64 Image header makes QEMU follow the Linux boot protocol and pass a
/// pointer. A zero means the runner has regressed to shipping an ELF, which QEMU loads on its
/// bare-metal path with `x0` zero. See notes/boot-protocol.md.
#[test_case]
fn device_tree_pointer_was_provided() {
    use core::sync::atomic::Ordering;
    assert_ne!(
        crate::DTB.load(Ordering::Relaxed),
        0,
        "no DTB pointer in x0: did we fall back to booting as an ELF?"
    );
}

/// Proves the pointer points at an actual device tree, not just at something.
///
/// Every flattened device tree begins with the magic `0xd00dfeed`, stored big-endian, so it is
/// byte-swapped on the way in. Not on `x86_64`, whose one pointer is PVH's `hvm_start_info`;
/// `machine_discovery::x86_64` decodes that, host-tested against a real dump.
#[test_case]
fn device_tree_has_the_right_magic() {
    use core::sync::atomic::Ordering;

    if cfg!(target_arch = "x86_64") {
        crate::testing::skip!(
            "this machine hands over PVH hvm_start_info, not a device tree (see \
             machine_discovery::x86_64, host-tested)"
        );
    }

    // DTB holds the physical address from x0; with TTBR0 disabled, only the direct map reaches it.
    let pa = crate::DTB.load(Ordering::Relaxed) as u64;
    let ptr = crate::arch::mmu::phys_to_virt(pa) as *const u32;

    // SAFETY: QEMU put a device tree at that physical address, and the direct map makes
    // it readable.
    let magic = unsafe { core::ptr::read_volatile(ptr) };

    assert_eq!(
        u32::from_be(magic),
        0xd00d_feed,
        "no device tree magic at {ptr:p}"
    );
}
