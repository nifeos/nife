//! The virtio-blk driver as its own binary (parity C).
//!
//! The driver logic is the `block_driver_roles` module of this package's library; this file is only the skeleton a dedicated
//! binary needs (`_start` dispatching the role, a panic handler, runtime re-exports). Every
//! architecture spawns it with the same role numbers and capability slots. The roles are the honest
//! driver, the block server, the net driver, the write path, and the two attackers the
//! DMA-confinement tests spawn (a descriptor aimed at kernel memory, and the indirect-descriptor
//! escape). The attackers share this binary on purpose: they differ from the honest driver by one
//! descriptor, so the attack is a fair test. See notes/virtual-io.md and notes/dma/shadow-ring.md.
//!
//! Name: ratified 2026-08-27 (calef). Renamed from `blk` to `block_driver`, matching the
//! `<device>_driver` shape given to `gpu_driver` and `keyboard_driver` in the same pass. Not
//! `disk_driver`: virtio-blk operates on the block-device abstraction, which is medium-agnostic
//! (RAM-backed, network-backed and virtual block devices are all real, not just physical disks),
//! so `block` is the accurate term and `disk` would be a narrower, slightly false claim. `blk`
//! itself was ratified 2026-07-30 (DECISIONS §39) as an abbreviation that was the ordinary name
//! of the thing; this supersedes that entry with the fuller, consistent name.

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

// The runtime re-exports the calls this skeleton's own code once spelled `crate::`'s; since 291
// the driver logic is the library's `block_driver_roles` and takes `user_mode_runtime` itself.
use components::block_driver_roles as roles;
pub use user_mode_runtime::{invoke, send};

/// Role numbers, matching `kernel/src/user/virtual_io_service.rs`. This binary is the only thing that
/// dispatches on them since milestone 291.
const VIRTUAL_IO_BLK: u64 = 3;
const VIRTUAL_IO_ATTACK: u64 = 8;
const VIRTUAL_IO_ATTACK_INDIRECT: u64 = 13;
const VIRTUAL_IO_BLK_WRITE: u64 = 30;
const VIRTUAL_IO_BLK_WRITE_ABANDON: u64 = 31;
const VIRTUAL_IO_NET: u64 = 40;
/// The block server (milestone 32 phase 2): serves blocks over blk IPC to the FS server.
const VIRTUAL_IO_BLK_SERVER: u64 = 32;

user_mode_runtime::program_entry!(start);

fn start(arg0: u64, arg1: u64, arg2: u64) -> ! {
    // A direct grant's three words are its handoff, and the role travels in it; a mediated
    // grant's first word is the role and its second the DMA page's physical base.
    let (role, direct_memory_access_physical_address) =
        match roles::direct::take([arg0, arg1, arg2]) {
            Some(granted) => (granted.role, granted.physical_address),
            None => (arg0, arg1),
        };
    match role {
        VIRTUAL_IO_BLK => roles::run(direct_memory_access_physical_address),
        VIRTUAL_IO_ATTACK => roles::run_attack(direct_memory_access_physical_address),
        VIRTUAL_IO_ATTACK_INDIRECT => {
            roles::run_attack_indirect(direct_memory_access_physical_address)
        }
        VIRTUAL_IO_BLK_WRITE => roles::run_write(direct_memory_access_physical_address),
        VIRTUAL_IO_BLK_WRITE_ABANDON => {
            roles::run_write_abandon(direct_memory_access_physical_address)
        }
        VIRTUAL_IO_NET => roles::run_net(direct_memory_access_physical_address),
        VIRTUAL_IO_BLK_SERVER => roles::run_block_server(direct_memory_access_physical_address),
        _ => panic!(),
    }
}

user_mode_runtime::panic_handler!();
