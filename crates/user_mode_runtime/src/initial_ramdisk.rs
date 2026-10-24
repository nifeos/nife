//! **The initial ramdisk archive slice every program receives at spawn.**
//!
//! An initial ramdisk is a filesystem image the bootloader leaves in RAM before the kernel starts,
//! readable before any storage driver exists. Inside this tree it is the initial ramdisk; the
//! device tree's `linux,initrd-start` and QEMU's `-initrd` spellings stop at the boundary
//! (`design/naming.md`). [`bytes`] holds the one assertion that the kernel mapped it, so each
//! `_start` does not write its own. See notes/user-mode-runtime.md.

//! Name: ratified 2026-10-08 (calef, #1842, milestone 139 (drive the unsafe count down) round 9's
//! question 2), with [`bytes`]. His words: "Yes". Refused `initrd` (round 6's provisional name).

//! # Examples
//! Bare-metal only, like the rest of this crate (see the crate-level doc for why this is `no_run`).
//!
//! ```no_run
//! user_mode_runtime::program_entry!(start);
//!
//! fn start(_a0: u64, initial_ramdisk_len: u64, _a2: u64) -> ! {
//!     // SAFETY: the kernel maps `initial_ramdisk_len` bytes of the archive, read-only, at
//!     // INITIAL_RAMDISK_VA, before this process's `_start` runs (`bytes`'s own contract).
//!     let archive = unsafe { user_mode_runtime::initial_ramdisk::bytes(initial_ramdisk_len) };
//!     let _ = archive;
//!     loop {}
//! }
//! ```

/// Where the kernel maps the initial ramdisk archive before any program's `_start` runs. Must match
/// `kernel::user::INITIAL_RAMDISK_VA`, the kernel-side constant this value mirrors, which every
/// kernel-side spawn path (`kernel::user::boot_progenitor` at boot, `spawn_elf_child_builder` in
/// the tests) maps this many bytes at, read-only, before starting the process.
///
/// Name: ratified 2026-10-12 (calef, #1998, milestone 139 (drive the unsafe count down) round 11).
/// His words: "B for #1998." Refused `INITRD_VA` (the foreign spelling, which stops at the boundary
/// where it arrives).
pub const INITIAL_RAMDISK_VA: u64 = 0x2000_0000;

/// The initial ramdisk archive bytes this process was handed at spawn. `len` is the value `_start`
/// received in its second argument register (`x1`/`a1`/`rsi`), which the kernel set to the
/// archive's real length.
///
/// # Safety
/// The kernel maps `len` bytes of the archive, read-only, at [`INITIAL_RAMDISK_VA`], before
/// this process's `_start` runs, for the whole lifetime of the process; that reserved RAM outlives
/// every process, which is what makes the `'static` lifetime honest. The caller must pass the
/// `len` its own `_start` actually received: no type can check that (notes/unsafe-obligations.md).
///
/// Name: ratified 2026-10-08 (calef, #1842, milestone 139 (drive the unsafe count down) round 9's
/// question 2), with its module. His words: "Yes". Refused `initrd_bytes` (round 6's provisional
/// name).
pub unsafe fn bytes(len: u64) -> &'static [u8] {
    // SAFETY: forwarded from this function's own contract, verbatim.
    unsafe { core::slice::from_raw_parts(INITIAL_RAMDISK_VA as *const u8, len as usize) }
}
