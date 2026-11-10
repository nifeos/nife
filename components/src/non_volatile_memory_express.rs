//! **The NVMe driver, at EL0, as two processes from one image** (§86 (whether an NVMe driver can
//! leave the kernel), notes/non-volatile-memory-express.md). `arg0` picks the role, and the
//! capabilities each spawn grants are what separate them. [`admin`] owns the controller and grants
//! the data plane its pages; [`data_plane`] is the block server. All the arithmetic is in
//! `crates/non_volatile_memory_express`, host-tested and Kani-reachable; what is left here is
//! volatile access, two barriers and a bounded poll.
//!
//! Name: ratified 2026-09-17 (calef, §154 (the acronym test is whether the phrase is spoken)), the
//! program's half 2026-09-18: it shares the crate's name, as `compositor` does. Refused
//! `nvme_server`, `nvme_driver` and `non_volatile_memory_express_server` (34 bytes, past the
//! 32-byte `NAME_LEN`, which was not raised for it).

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

#[path = "non_volatile_memory_express_admin.rs"]
mod admin;
#[path = "non_volatile_memory_express_data_plane.rs"]
mod data_plane;

use driver_process_contract::non_volatile_memory_express::{ADMINISTRATION_ROLE, DATA_PLANE_ROLE};

// Index exception (2026-11-10): the queue, the namespace, the poll bound and the barrier are the
// four facts both roles drive the controller by, so they sit where both children read them.
/// The one I/O queue pair the admin process creates. Ringing any other queue's doorbell
/// would be ringing a queue that does not exist.
const IO_QID: u16 = 1;

/// The one namespace QEMU's `-device nvme,drive=` carries; the admin process identifies it.
const NSID: u32 = 1;

/// How many polls of the completion ring before this server declares the command lost. QEMU
/// completes synchronously inside the doorbell write, so this is paranoia there; on hardware it is
/// roughly a second of spinning. **Not a measured bound**, and it is the only thing standing
/// between a dead controller and a server that never answers, because `CSTS` is unreachable from
/// here. See `data_plane`'s `BUGS`.
const SPIN_BOUND: u32 = 50_000_000;

/// **Order our writes to the rings before the doorbell that announces them, and the phase-tag read
/// before the payload it gates.** The controller is another observer of this memory, and on a
/// weakly-ordered CPU nothing else stops a doorbell write hoisting above the command it announces.
///
/// Stronger than [`user_mode_runtime::virtio::virtio_ring_barrier`] on purpose: those drivers
/// notify through a syscall instruction, so `x86_64` needs only a compiler fence and aarch64 only
/// `dmb ish`. This doorbell is a direct store to a device-typed page with no syscall between the
/// ring stores and it, so it keeps `dmb sy` and a real `mfence`. Sharing the two would be an
/// ordering change, not deduplication.
fn barrier() {
    // Shared with the `e1000e` data plane (milestone 494 (a driver for the network card a PC
    // actually has)); the paragraph above is the reason for its instructions.
    user_mode_runtime::mapped_window::doorbell_barrier();
}

user_mode_runtime::program_entry!(start);

// Index exception (2026-11-10): the entry point only picks the role, and each role is a child.
fn start(role: u64, first: u64, second: u64) -> ! {
    match role {
        // The run's physical base.
        ADMINISTRATION_ROLE => admin::run(first),
        // Where to map the data-plane run and the doorbell page.
        DATA_PLANE_ROLE => data_plane::run(first, second),
        _ => user_mode_runtime::exit(),
    }
}

user_mode_runtime::panic_handler!();
