//! **Starting a second logical CPU: INIT-SIPI-SIPI and the real-mode trampoline it needs.**
//!
//! x86 has no firmware call for this (aarch64 has PSCI `CPU_ON`, RISC-V SBI `hart_start`): an
//! INIT resets the target into wait-for-SIPI, and a STARTUP IPI names a physical page below 1 MiB
//! where it begins in 16-bit real mode (`irq::send_init`, `irq::send_startup`, `boot.s`'s
//! `secondary_boot`). This module copies the trampoline onto that page and hands it the one
//! per-core value, the stack top. `NIFE_SMP` defaults to 2. See notes/x86-port/smp-bring-up.md.

use super::mmu::phys_to_virt;

unsafe extern "C" {
    /// The trampoline's own low VMA (`AP_TRAMPOLINE_PHYS`, link-x86_64.ld): where it executes from,
    /// and so also where its bytes must be copied to before a `STARTUP` IPI can name it.
    static __ap_trampoline_start: core::ffi::c_void;
    /// One past the trampoline's last byte, at the same low VMA. `__ap_trampoline_end -
    /// __ap_trampoline_start` is exactly how much [`prepare`] copies.
    static __ap_trampoline_end: core::ffi::c_void;
    /// Where the trampoline's bytes actually sit in the *loaded* image: link-x86_64.ld gives
    /// `.ap_trampoline` a low VMA but leaves its load address (LMA) an ordinary spot beside
    /// `.rodata`, which is where the loader actually put the bytes.
    static __ap_trampoline_lma: core::ffi::c_void;
    /// The one value the trampoline cannot know at link time: the target core's stack top.
    /// [`prepare`] writes it; `boot.s`'s 64-bit tail reads it back RIP-relative.
    static ap_trampoline_stack_top: u64;
}

/// The physical page a `STARTUP` IPI names: `secondary_boot`'s own address, which is also
/// `__ap_trampoline_start`'s, since the trampoline's VMA *is* where it runs from.
///
/// Page-aligned by construction (`link-x86_64.ld`'s `.ap_trampoline` names `AP_TRAMPOLINE_PHYS`,
/// itself a multiple of 4096), which is what lets [`super::cpu_start`] derive a `STARTUP` vector
/// from it with a plain shift.
pub(crate) fn trampoline_phys() -> u64 {
    (&raw const __ap_trampoline_start) as u64
}

/// How many bytes [`prepare`] copies: `__ap_trampoline_end - __ap_trampoline_start`.
pub(crate) fn trampoline_size() -> u64 {
    (&raw const __ap_trampoline_end) as u64 - trampoline_phys()
}

/// **Where the trampoline's bytes live in the loaded image**, physically: link-x86_64.ld places
/// `.ap_trampoline`'s LMA in the gap it opens between `.rodata` and `.data`.
///
/// Neither general mapping rule covers that gap: the direct map skips the image (no second
/// writable alias), and the gap lies between two sections rather than inside either. So
/// `mmu::map_everything` direct-maps it by name, as `map_firmware_regions` covers other gaps.
pub(crate) fn trampoline_lma() -> u64 {
    (&raw const __ap_trampoline_lma) as u64
}

/// **Copy the trampoline to the page it has to execute from, and hand it this core's stack top.**
///
/// # Safety
/// Must not run while another core might still be reading the SAME copy: there is one trampoline
/// scratch page, shared by every `STARTUP` IPI this kernel ever sends. [`super::cpu_start`] is the
/// only caller, and it does not return until the core it just started is either fully online (past
/// the point it could still be reading this page) or given up on, which is what makes calling this
/// again, for the next core, safe.
pub(crate) unsafe fn prepare(stack_top: u64) {
    let dst = trampoline_phys();
    let src = trampoline_lma();
    let len = trampoline_size();

    // SAFETY: `src..src+len` is part of the loaded image (ordinary, file-backed bytes in the gap
    // between `.rodata` and `.data`; see the linker script's comment on why NOT `.boot_scratch` or
    // the secondary stacks, both of which are mutated at runtime), and `mmu::map_everything` maps
    // exactly this range into the direct map by name ([`trampoline_lma`]'s own doc explains why it
    // needs to: neither the general direct-map rule nor either neighboring section's own mapping
    // reaches it). `dst..dst+len` is the low megabyte, which `mmu::init`'s `map_firmware_regions`
    // maps writable (`Flags::kernel_data()`) and which the frame allocator never hands out
    // (`machine::LOW_MEGABYTE`), so nothing else owns those bytes. Non-overlapping: `dst` is a
    // fixed low page (`AP_TRAMPOLINE_PHYS`) and `src` sits beside `.rodata`, comfortably above 1 MiB.
    unsafe {
        core::ptr::copy_nonoverlapping(
            phys_to_virt(src) as *const u8,
            phys_to_virt(dst) as *mut u8,
            len as usize,
        );
    }

    let stack_top_slot = (&raw const ap_trampoline_stack_top) as u64;
    // SAFETY: `stack_top_slot` is inside the page just copied, reached through the direct map, and
    // this is the one caller (see this function's own safety contract).
    unsafe {
        core::ptr::write_volatile(phys_to_virt(stack_top_slot) as *mut u64, stack_top);
    }
}
