//! Build orchestration for nife: a Rust binary on the *host* for what does not fit `cargo build`
//! (a custom target, a linker script, QEMU's flags), in Rust rather than a Makefile because it
//! composes. See DECISIONS §7.
//!
//! `run` and `test` do not invoke QEMU themselves: cargo runs `helpers/qemu-runner-aarch64.sh`
//! through the runner setting in `.cargo/config.toml`, so the QEMU flags live in one place.

//! # Commands
//!
//!     cargo xtask run      boot the kernel (the milestone tour), print to this terminal
//!     cargo xtask shell    boot straight to the interactive shell (add --hvf for the real core)
//!     cargo xtask swish-check  boot that same shell, type at it, and check what it answered
//!     cargo xtask test     host tests (milliseconds), then the kernel under QEMU
//!                          (--hvf runs the aarch64 kernel leg on the physical core)
//!     cargo xtask gdb      boot paused, waiting for a debugger on :1234
//!     cargo xtask objdump  disassemble the kernel
//!     cargo xtask image    build the flat arm64 Image and dump its header
//!     cargo xtask board-console  read the serial console of a real board, log it, stop on a deadline
//!                                (and, under --stop, send the one byte that ends a rebooting soak)
//!     cargo xtask board-script   write the U-Boot script that boots the board without a person at its prompt

use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};

mod archive;
mod basalt;
mod bench;
mod board;
mod boot_check;
mod card_check;
mod cloud_image;
/// **The rollback gate** (rung 2b of milestone 198 (a package manager, and the trivial install that makes a second customer possible)): a doomed upgrade is tried and the machine comes back on its own.
mod confirm;
mod disk;
mod disk_check;
mod disk_throughput;
mod farm;
mod foreign_note;
mod host;
mod icount;
mod inbound;
mod inspect;
mod install;
mod manual;
mod measure;
mod network_bench;
mod package;
mod portable_executable;
mod restamp;
mod rollback;
mod scanout;
mod screen;
mod soak;
mod stick;
mod suite;
mod swish_check;
mod time_record;
mod uefi;
mod uefi_screen_modes;
mod usage;

use crate::archive::{initial_ramdisk_aarch64, initial_ramdisk_riscv, initial_ramdisk_x86};
use crate::bench::bench;
use crate::board::{board_console, board_script};
use crate::boot_check::boot_check;
use crate::disk::{mkdisk, mkredoxfs, redoxfs_server_build};
use crate::farm::{std_aborts, std_exerciser, std_inputs_stamp, std_src};
use crate::host::cargo;
use crate::icount::icount;
use crate::inspect::{gdb, image, objdump};
use crate::manual::{manual_store, tree_apropos};
use crate::soak::{job_mix_sweep, soak_test};
use crate::suite::{test, undefined_behavior_check};
use crate::swish_check::swish_check;
use crate::uefi::{uefi_boot_command, uefi_image_command, uefi_test_command};

mod command_line;
mod profile;

use command_line::*;
use profile::*;

const TARGET: &str = "aarch64-unknown-none-softfloat";
const RUNNER: &str = "helpers/qemu-runner-aarch64.sh";

/// The RISC-V target, for the second-architecture initial ramdisk (milestone 20 (a portable HAL)).
/// The kernel itself is built and run through cargo + `helpers/qemu-runner-riscv64.sh` directly,
/// not this xtask; this const exists only so `initial-ramdisk-riscv` builds the userspace archive
/// for the matching target.
const RISCV_TARGET: &str = "riscv64imac-unknown-none-elf";

/// The `x86_64` target (milestone 161). The kernel is built and run through cargo +
/// `helpers/qemu-runner-x86_64.sh`, exactly as the RISC-V one is, and since item 4's hand-off this
/// const also builds the third userspace archive: `initial-ramdisk-x86` compiles `user` for it and
/// [`initial_ramdisk_x86`] packs the same programs RISC-V's archive carries. See
/// notes/x86-port/userspace.md.
const X86_TARGET: &str = "x86_64-unknown-none";

/// Whether this run builds optimized binaries. Only `bench --release` sets it (a fair cross-OS
/// comparison wants an optimized kernel and userspace, not the debug default). Everything else stays
/// debug: faster builds, and the tests and the tour want debuginfo and cheap rebuilds.
static RELEASE: AtomicBool = AtomicBool::new(false);

/// Whether this run's userspace carries the confinement tests' attacker roles
/// (`components/confinement_attackers`, milestone 261's DMA-escape test). Only `test` sets it, so
/// the suite and its falsification replays (`cargo xtask test --test ...`) get the attacker
/// `non_volatile_memory_express` server and every other boot (`run`, `stick`, `install`, the
/// benches, the soaks, swish-check) packs the shipped one. Read by `archive::build_programs`. Name
/// provisional.
static CONFINEMENT_ATTACKERS: AtomicBool = AtomicBool::new(false);

fn build() -> bool {
    // The user program and the disk image first: the kernel boots with the program as an initial
    // ramdisk and reads the disk over virtio, so both have to exist before it runs.
    mkdisk() && user() && cargo(&["build", "-p", "kernel", "--target", TARGET])
}

/// Build the userspace ELF that the kernel will load at milestone 7.
///
/// It is a **separate crate with its own linker script** (linked at `address_space_map::IMAGE_BASE`, in the low half,
/// where `TTBR0` lives), so it cannot accidentally share anything with the kernel. And it stays
/// an **ELF**: the kernel's loader wants program headers, unlike the kernel itself, which QEMU
/// wants as a flat image. See notes/elf.md.
pub(crate) fn user() -> bool {
    archive::build_programs(TARGET) && initial_ramdisk_aarch64()
}
