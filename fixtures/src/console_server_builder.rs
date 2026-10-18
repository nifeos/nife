//! **Builds the real console server and then plays its client** (step 19d.2b of milestone 19d (run
//! a real workload)). `kernel::user::spawn_elf_child_builder` enters this program holding a
//! building budget (slot 0), a report endpoint (slot 1) and the console UART's registers as a
//! device capability (slot 2). It reads `console` out of the archive by name and builds it with the
//! four things a console server needs, all out of this program's own budget and capabilities: a
//! request endpoint, a reply endpoint, a shared page and the UART's registers. Then it writes
//! [`capability_witness_contract::CONSOLE_SERVER_BUILDER_LINE`] into the shared page, sends its
//! length, and reports the length the server acknowledges after printing it. That report proves a
//! userspace-built console works end to end: a driver this program constructed, on a channel it
//! created, driving hardware it delegated.
//!
//! A failed lookup, parse or build reports word 0.
//!
//! # BUGS
//!
//! The child is loaded unmeasured (notes/trusted-init.md). On `x86_64` the UART is port-mapped
//! and there is no device page to delegate, so the kernel's test skips there.
//!
//! Name: ratified 2026-10-18 (calef), as `CONSOLE_SERVER_BUILDER`, written here in program form.

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

use supervision_protocol::{
    ChildEndowment, Retention, build_child, retype_obj_from, retype_page_frame_from, start_child,
};
use user_mode_runtime::{exit, map_page_frame, receive, require, send};

const MEMORY_REGION: u64 = 0;
const REPORT: u64 = 1;
const UART_DEVICE: u64 = 2;
const CHILD: &str = "console";
/// Must match the console server's `SHARED_VA`.
const SHARED_VA: u64 = address_space_map::pair_page(0x0060_0000);
/// Must match the console server's `UART_VA`.
const CHILD_UART_VA: u64 = address_space_map::pair_page(0x0070_0000);

#[unsafe(no_mangle)]
pub extern "C" fn _start(_arg0: u64, initial_ramdisk_len: u64, _arg2: u64) -> ! {
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

    let Ok(request) = retype_obj_from(MEMORY_REGION, abi::objtype::RENDEZVOUS) else {
        refuse()
    };
    let Ok(reply) = retype_obj_from(MEMORY_REGION, abi::objtype::RENDEZVOUS) else {
        refuse()
    };
    let Ok(shared) = retype_page_frame_from(MEMORY_REGION) else {
        refuse()
    };
    // Read/write here, where this program writes the line; read-only in the server.
    if !map_page_frame(shared, SHARED_VA, true, MEMORY_REGION) {
        refuse();
    }

    // The server receives on slot 0 and acknowledges on slot 1.
    let grants = [(request, abi::rights::READ), (reply, abi::rights::WRITE)];
    let maps = [
        (SHARED_VA, shared, abi::address_space::MAP_RO),
        (CHILD_UART_VA, UART_DEVICE, abi::address_space::MAP_RO),
    ];
    let endowment = ChildEndowment {
        caps: &grants,
        maps: &maps,
        stack_pages: system_initializer::CHILD_STACK_PAGES,
        // `Retention::Nothing`: nothing here ever stops the server, and this program exits once
        // it has its answer (DECISIONS §142 (what a spawner retains over a child after `START`)).
        ..ChildEndowment::new(Retention::Nothing)
    };
    let Ok(child) = build_child(MEMORY_REGION, MEMORY_REGION, &elf, &endowment) else {
        refuse()
    };
    require(start_child(child, 0, 0, 0));

    let line = capability_witness_contract::CONSOLE_SERVER_BUILDER_LINE;
    // SAFETY: the shared page was mapped read/write at SHARED_VA above, and the line is shorter
    // than a page.
    unsafe {
        core::slice::from_raw_parts_mut(SHARED_VA as *mut u8, line.len()).copy_from_slice(line);
    }
    require(send(request, line.len() as u64, 0, 0) == 0);
    let (acked, _, _) = receive(reply);
    send(REPORT, acked, 0, 0);
    exit();
}

/// Report word 0, which no server acknowledges for a line this long, and exit.
fn refuse() -> ! {
    send(REPORT, 0, 0, 0);
    exit();
}

user_mode_runtime::panic_handler!();
