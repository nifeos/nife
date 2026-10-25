//! **Builds a driver out of an ELF and hands it a device's registers** (step 19d.2 of milestone 19d
//! (run a real workload)). `kernel::user::spawn_elf_child_builder` enters this program holding a
//! building budget (slot 0), a report endpoint (slot 1) and the console UART's registers as a
//! device capability (slot 2). It reads `device_child` out of the archive by name, builds it,
//! grants it the report endpoint and maps the UART into it, device-typed, at [`CHILD_UART_VA`]. The
//! child reads the PL011's identification registers and reports them, which proves device access is
//! a capability this program could delegate and that the mapping it made is a real view of the
//! hardware.
//!
//! A failed lookup, parse or build reports word 0.

//! # BUGS
//!
//! The child is loaded unmeasured (notes/trusted-init.md). Only aarch64's kernel test runs this
//! program, because the identification constant is a PL011's and the other two machines have none.
//!
//! Name: ratified 2026-10-18 (calef), as `DEVICE_CHILD_BUILDER`, written here in program form.

#![no_std]
// Program entry points, not the crates/ library surface milestone 68 (code-quality gates)'s ratchet
// tracks (DECISIONS §107 (`missing_docs` moves to the workspace lints)): each `[[bin]]` is its own
// crate root with one `_start`, and 58 of them documenting an OS-facing ABI entry point is not what
// the lint is for.
#![allow(missing_docs)]
#![no_main]

use supervision_protocol::{ChildEndowment, Retention, build_child, start_child};
use user_mode_runtime::{exit, require, send};

const MEMORY_REGION: u64 = 0;
const REPORT: u64 = 1;
const UART_DEVICE: u64 = 2;
const CHILD: &str = "device_child";
/// Must match `device_child`'s `UART_VA`, where it reads the registers.
const CHILD_UART_VA: u64 = address_space_map::pair_page(0x0070_0000);

user_mode_runtime::program_entry!(start);

fn start(_arg0: u64, initial_ramdisk_len: u64, _arg2: u64) -> ! {
    // SAFETY: forwarded from user_mode_runtime::initial_ramdisk::bytes's own contract: the kernel
    // mapped the archive at INITIAL_RAMDISK_VA and passed its length in x1.
    let archive = unsafe { user_mode_runtime::initial_ramdisk::bytes(initial_ramdisk_len) };
    let Some(elf) = nifefs::Fs::parse(archive)
        .ok()
        .and_then(|fs| fs.read(CHILD))
        .and_then(|bytes| elf::Elf::parse(bytes).ok())
    else {
        refuse()
    };
    let grants = [(REPORT, abi::rights::WRITE)];
    let maps = [(CHILD_UART_VA, UART_DEVICE, abi::address_space::MAP_RO)];
    let endowment = ChildEndowment {
        caps: &grants,
        maps: &maps,
        stack_pages: system_initializer::CHILD_STACK_PAGES,
        // `Retention::Nothing`: this program exits a few lines after `START`, so keeping the
        // child's thread control block capability would be keeping it for nobody (DECISIONS §142
        // (what a spawner retains over a child after `START`)).
        ..ChildEndowment::new(Retention::Nothing)
    };
    let Ok(child) = build_child(MEMORY_REGION, MEMORY_REGION, &elf, &endowment) else {
        refuse()
    };
    require(start_child(child, 0, 0, 0));
    exit();
}

/// Report word 0, which no PL011 returns as its identity, and exit.
fn refuse() -> ! {
    send(REPORT, 0, 0, 0);
    exit();
}

user_mode_runtime::panic_handler!();
