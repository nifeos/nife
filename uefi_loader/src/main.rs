//! **The boot entry real firmware can start** (milestone 87; aarch64 and riscv64 since the stick
//! milestone).
//!
//! A UEFI application the firmware loads from the removable-media path of a FAT volume
//! (`\EFI\BOOT\BOOTX64.EFI`, `BOOTAA64.EFI`, `BOOTRISCV64.EFI`), carrying the kernel and the
//! userspace archive inside itself. It asks the firmware what a kernel cannot ask once the firmware
//! is gone, and hands over in the state QEMU's `-kernel` or U-Boot's `booti` would have: **the
//! kernel is not modified and cannot tell which loader started it**. One source, three
//! architectures; how each kernel is told about the machine lives under `src/arch/`. **Read
//! notes/x86-uefi-boot.md and notes/boot-stick.md before changing anything here.**

//! | Architecture | The kernel's entry contract | Told about the machine by |
//! |---|---|---|
//! | `x86_64` | PVH: 32-bit protected mode, `ebx` = `hvm_start_info` | a synthesised `hvm_start_info` (`arch/x86_64`) |
//! | aarch64 | Linux arm64 `booti`: MMU off, `x0` = device tree | the firmware's device tree, with `/chosen` naming the archive (`arch/aarch64`) |
//! | riscv64 | Linux riscv `booti`: paging off, `a0` = hart, `a1` = device tree | the same, plus the boot hart from the firmware (`arch/riscv64`) |

//! # The order of operations is rigid, and two rules are why
//!
//! `ExitBootServices` takes a `map_key` from a `GetMemoryMap` with **no allocation in between**, so
//! every buffer is allocated before that call and filled in after it. And **after
//! `ExitBootServices` there is no console**: a failure past that point can only halt, so everything
//! checkable is checked while there is still something to report it on.

//! # BUGS (each one's full account is in notes/x86-uefi-boot/limitations.md)
//!
//! - **Secure Boot must be off**: nothing signs this image (notes/turning-secure-boot-off.md).
//! - **The kernel is embedded rather than loaded from the filesystem**, so it rebuilds with it.
//! - **Nothing validates the UEFI tables' header CRCs**: the firmware is the trust root here.
//! - **Nothing verifies what it hands over at boot**: the trust boundary is the build (`build.rs`).
//! - **AP bring-up under UEFI is proved on OVMF only** (milestone 195 (finish the UEFI boot path)).
//! - **The `hvm_start_info` command line holds only the screen** (milestone 243 (no serial port)).
//! - **The screen is taken as the firmware left it**, and aarch64 and riscv64 do not hand it over.
//! - **A `PixelBitMask` or `PixelBltOnly` adapter gets no screen** on `x86_64`; the loader says so.

#![no_std]
#![no_main]

use core::ptr;

// **UEFI through `r-efi`** (milestone 784 (reuse what the tree wrote), item (e), ruled by calef on
// 2026-10-20 UTC under §46 (thin primitives or whole subsystems)). Its tables are complete and in
// specification order, so no field here is found by counting. Every function pointer in them is an
// `unsafe extern "efiapi" fn`. `boot_services` wraps the calls made more than once; every other
// call site states why its arguments are what the specification asks for. All of them run before
// `ExitBootServices`, through the table the firmware handed to `efi_main`.
use r_efi::efi::{
    self, ALLOCATE_ADDRESS, ALLOCATE_MAX_ADDRESS, BootServices, ConfigurationTable, Handle, Status,
    SystemTable,
};
#[cfg(target_arch = "x86_64")]
use r_efi::protocols::{file, loaded_image, simple_file_system};
#[cfg(not(target_arch = "x86_64"))]
use uefi_loader::device_tree_patch;
use uefi_loader::image;

mod arch;
mod boot_services;
/// **Which of the two boot images on the disk to start**, so a bad upgrade cannot brick the machine (milestone 198's rung 2b).
#[cfg(target_arch = "x86_64")]
mod chooser;

mod console;
mod loading;
mod placement;

use console::*;
use loading::*;
pub use placement::Placed;
use placement::*;

/// The kernel ELF and the userspace archive, embedded by `build.rs`.
mod embedded {
    include!(concat!(env!("OUT_DIR"), "/embedded.rs"));
}

/// `EFI_SUCCESS`, spelled once for the comparisons every boot-service call ends in.
const SUCCESS: Status = Status::SUCCESS;

/// UEFI's page size, and the granularity of every `AllocatePages` call.
const PAGE: u64 = 4096;

/// Slack, in descriptors, added to the memory map buffer between the sizing call and the real one.
///
/// The UEFI specification says so outright: allocating memory to hold the map can itself change the
/// map. This loader allocates exactly once between the two calls, and that allocation can split one
/// free range into as many as three; thirty-two is far more than that and costs 4 KiB.
const MAP_SLACK_DESCRIPTORS: usize = 32;
