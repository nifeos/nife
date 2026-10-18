//! **The initial ramdisk archive slice every program receives at spawn** (milestone 139).
//!
//! **An initial ramdisk is a filesystem image the bootloader leaves in RAM before the kernel
//! starts, readable before any storage driver exists.** The bootstrap problem it solves is
//! circular: starting the first userspace program means reading a file, which needs a filesystem,
//! which needs a block driver, which in this system is itself a userspace program. The bootloader
//! breaking that circle is the whole mechanism.
//!
//! **The foreign spelling stops at the boundary**, as `design/naming.md` records: the kernel finds
//! this region by reading `linux,initrd-start` out of a device tree QEMU generates, and every run
//! passes QEMU's own `-initrd`. Both spellings are somebody else's; inside, it is the initial
//! ramdisk.
//!
//! Seven programs (`builder`, `c_confiner`, `hello`, `login`, `root_supervisor`, `swapper`,
//! `timetable`) each declared their own `const INITRD_VA: u64 = 0x2000_0000` and their own
//! `unsafe { core::slice::from_raw_parts(INITRD_VA as *const u8, initrd_len as usize) }`, one
//! hand-written `// SAFETY:` comment per file asserting the identical invariant: the kernel maps
//! `initrd_len` bytes of the initrd archive, read-only, at a fixed VA, before `_start` runs.
//! `timetable.rs`'s own comment had already named the duplication out loud ("the same contract
//! `components/src/builder.rs` is started under") without anyone lifting it out, the same shape
//! `ntp.rs`'s comment named for the [`mapped_window`](crate::mapped_window) cluster.
//!
//! **Seven is a count of 2026-09-02 and six of them are left** (milestone 295): `builder` was
//! retired on 2026-09-14 when the RISC-V tour stopped composing a child of its own, so the file
//! that phrase quotes is gone from the tree. The count and the quotation both stay, because they
//! are an account of what this module was written to collapse rather than a description of today,
//! and rewriting either would make milestone 139's own reasoning unreadable.
//!
//! [`bytes`] holds that one assertion instead of seven. This does not remove the `unsafe`
//! block at each call site the way `MappedWindow` did: this hands back a whole `'static` slice
//! rather than a bounds-checked per-offset accessor, and `len` cannot be validated by any
//! type (any `u64` compiles), so the obligation stays a sentence rather than becoming a checked
//! `assert!`. But it is still a real reduction by this milestone's own criterion 1: collapsing
//! seven hand-written copies of one invariant into one declaration, the §94 shape
//! (`design/decisions/0094-what-may-live-in-a-library.md`: "a per-binary item whose body is copied
//! verbatim into every binary is not per-binary; only its declaration is"), the same "flat block
//! count, still a real reduction" case `smb_server.rs`, `swish.rs`'s terminal pair and
//! `display_terminal.rs`'s `paint` already were in this milestone's earlier rounds.
//!

//! Name: ratified 2026-10-08 (calef, #1842, milestone 139 (drive the unsafe count down) round 9's
//! question 2), with [`bytes`]. His words: "Yes". Refused `initrd` (round 6's provisional name).
//!
//! # Examples
//!
//! Bare-metal only, like the rest of this crate (see the crate-level doc for why this is `no_run`).
//!
//! ```no_run
//! #[unsafe(no_mangle)]
//! pub extern "C" fn _start(_a0: u64, initial_ramdisk_len: u64, _a2: u64) -> ! {
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
/// the tests) maps this many bytes at, read-only, before starting the process. Every `_start` that
/// receives the archive gets it at this same VA; the seven callers this module replaces had each
/// hard-coded it under this exact name.
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
/// `len` its own `_start` actually received: nothing here can check that, because any `u64`
/// compiles, the same "the parameter could have been produced without meeting this" case
/// `notes/unsafe-obligations.md` (milestone 112) already names for `switch_user_root`.
///
/// Name: ratified 2026-10-08 (calef, #1842, milestone 139 (drive the unsafe count down) round 9's
/// question 2), with its module. His words: "Yes". Refused `initrd_bytes` (round 6's provisional
/// name).
pub unsafe fn bytes(len: u64) -> &'static [u8] {
    // SAFETY: forwarded from this function's own contract, verbatim.
    unsafe { core::slice::from_raw_parts(INITIAL_RAMDISK_VA as *const u8, len as usize) }
}
