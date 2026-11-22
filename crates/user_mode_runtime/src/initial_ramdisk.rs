//! **The initial ramdisk archive slice every program receives at spawn.**
//!
//! An initial ramdisk is a filesystem image the bootloader leaves in RAM before the kernel starts,
//! readable before any storage driver exists. This crate calls it the initial ramdisk; the
//! device tree's `linux,initrd-start` and QEMU's `-initrd` spellings stop at the boundary.
//! [`bytes`] holds the one assertion that the kernel mapped it, so each `_start` does not write
//! its own. See `docs/design.md`.

//! Name: ratified 2026-10-08 (calef, #1842, milestone 139 (drive the unsafe count down) round 9's
//! question 2), with [`bytes`]. His words: "Yes". Refused `initrd` (round 6's provisional name).

//! # Examples
//! Bare-metal only, like the rest of this crate (see the crate-level doc for why this is `no_run`).
//!
//! ```no_run
//! fn start(_a0: u64, initial_ramdisk_len: u64, _a2: u64) {
//!     // SAFETY: the kernel maps `initial_ramdisk_len` bytes of the archive, read-only, at
//!     // INITIAL_RAMDISK_VIRTUAL_ADDRESS, before this process's `_start` runs (`bytes`'s own contract).
//!     let archive = unsafe { user_mode_runtime::initial_ramdisk::bytes(initial_ramdisk_len) };
//!     let _ = archive;
//! }
//! # start(0, 0, 0);
//! ```

/// Where the kernel maps the initial ramdisk archive before any program's `_start` runs: the
/// address-space map's constant, which the kernel's own mapping uses too, so the two cannot differ.
pub use address_space_layout::INITIAL_RAMDISK_VIRTUAL_ADDRESS;

/// The initial ramdisk archive bytes this process was handed at spawn. `length` is the value `_start`
/// received in its second argument register (`x1`/`a1`/`rsi`), which the kernel set to the
/// archive's real length.
///
/// # Safety
/// The kernel maps `length` bytes of the archive, read-only, at [`INITIAL_RAMDISK_VIRTUAL_ADDRESS`], before
/// this process's `_start` runs, for the whole lifetime of the process; that reserved RAM outlives
/// every process, which is what makes the `'static` lifetime honest. The caller must pass the
/// `length` its own `_start` actually received: no type can check that, so the obligation stays a
/// sentence rather than an `assert!`.
///
/// Name: ratified 2026-10-08 (calef, #1842, milestone 139 (drive the unsafe count down) round 9's
/// question 2), with its module. His words: "Yes". Refused `initrd_bytes` (round 6's provisional
/// name).
pub unsafe fn bytes(length: u64) -> &'static [u8] {
    // SAFETY: forwarded from this function's own contract, verbatim.
    unsafe {
        core::slice::from_raw_parts(
            INITIAL_RAMDISK_VIRTUAL_ADDRESS as *const u8,
            length as usize,
        )
    }
}
