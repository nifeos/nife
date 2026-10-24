//! The virtio-blk driver as its own binary (parity C).
//!
//! The driver logic is the shared `virtio` module; this file is only the skeleton a dedicated
//! binary needs (`_start` dispatching the role, a panic handler, runtime re-exports). Every
//! architecture spawns it with the same role numbers and capability slots. The roles are the honest
//! driver, the block server, the net driver, the write path, and the two attackers the
//! DMA-confinement tests spawn (a descriptor aimed at kernel memory, and the indirect-descriptor
//! escape). The attackers share this binary on purpose: they differ from the honest driver by one
//! descriptor, so the attack is a fair test. See notes/virtio.md and notes/dma/shadow-ring.md.
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
// the driver logic is the `virtio` crate and takes `user_mode_runtime` itself.
pub use user_mode_runtime::{invoke, send};

/// Role numbers, matching `kernel/src/user/virtio_service.rs`. This binary is the only thing that
/// dispatches on them since milestone 291.
const VIRTIO_BLK: u64 = 3;
const VIRTIO_ATTACK: u64 = 8;
const VIRTIO_ATTACK_INDIRECT: u64 = 13;
const VIRTIO_BLK_WRITE: u64 = 30;
const VIRTIO_BLK_WRITE_ABANDON: u64 = 31;
const VIRTIO_NET: u64 = 40;
/// The block server (milestone 32 phase 2): serves blocks over blk IPC to the FS server.
const VIRTIO_BLK_SERVER: u64 = 32;

#[unsafe(no_mangle)]
pub extern "C" fn _start(role: u64, direct_memory_access_phys: u64, _arg2: u64) -> ! {
    match role {
        VIRTIO_BLK => virtio::run(direct_memory_access_phys),
        VIRTIO_ATTACK => virtio::run_attack(direct_memory_access_phys),
        VIRTIO_ATTACK_INDIRECT => virtio::run_attack_indirect(direct_memory_access_phys),
        VIRTIO_BLK_WRITE => virtio::run_write(direct_memory_access_phys),
        VIRTIO_BLK_WRITE_ABANDON => virtio::run_write_abandon(direct_memory_access_phys),
        VIRTIO_NET => virtio::run_net(direct_memory_access_phys),
        VIRTIO_BLK_SERVER => virtio::run_blk_server(direct_memory_access_phys),
        _ => panic!(),
    }
}

user_mode_runtime::panic_handler!();
