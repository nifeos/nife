//! The tables behind `IPC_TABLES`: [`IpcTables`] itself, the one lock it sits behind, the limits
//! that size it ([`MAX_THREADS`], [`MAX_RENDEZVOUS`], the futex buckets and the kernel's endpoint
//! chunks), and the rendezvous-pressure instrument. See notes/ipc-tables-lock-inventory.md.
//!
//! The fields are `pub(super)`, which in a child of `sched` reaches `sched` and its children, the
//! set §271 (the scheduler file splits into submodules of `sched`) keeps the table to. The
//! functions that read the tables on the IPC fastpath are in `sched.rs`, beside `schedule`.
//!
//! Name: provisional (lane maintainer/near-ceiling-splits, 2026-10-12 UTC), for the module and the
//! file.

use super::*;

/// A synchronous IPC rendezvous point: the two wait queues and the pending-signal count.
///
/// The state machine is the `inter_process_communication` crate, which proves its one invariant,
/// "at most one wait queue is ever non-empty" (§14 (the project's direction),
/// notes/verification.md); the kernel's IPC functions add only the bookkeeping the queues cannot
/// express. A wait-queue entry is the TCB itself, threaded through the run queues' link, so
/// blocking cannot allocate and a thread waits on one rendezvous at a time. The pointers' contract
/// is at [`thread_control_block_ptr`].
pub(super) type Rendezvous = inter_process_communication::Rendezvous<Thread>;

/// The most threads that can be alive at once, whole machine: a documented limit of the image, so
/// spawn past it fails cleanly, as out-of-memory does. About twice the peak [`PEAK_THREADS`]
/// reports. What scales with it, and why 256, are in notes/ipc-tables-lock-inventory.md ("What the
/// tables hold").
///
/// # BUGS
///
/// - Revocation still scans every live thread; milestone 183 (a physical-range index for
///   capability holders) is the structural fix.
/// - The peak is a whole-boot high-water and cannot name the test that pushed it.
pub const MAX_THREADS: usize = 256;

pub(super) struct IpcTables {
    /// The thread table: generational names over page-resident TCBs. See [`Threads`] and
    /// notes/thread-control-block.md.
    pub(super) threads: Threads,
    /// The rendezvous registry: generational name to the physical address of the page the
    /// rendezvous lives at, retyped from a region that stays pinned while it lives.
    pub(super) rendezvous_table: generational_table::Table<u64, MAX_RENDEZVOUS>,
    /// The kernel's current object chunk, where its own endpoints (boot services, tests) are
    /// retyped from. Carved on the first [`create_rendezvous`] and replaced when it fills; a filled
    /// chunk's handle is forgotten, because no path tears a kernel chunk down.
    pub(super) kernel_ep_region: Option<u64>,
    /// How many chunks have been carved, so growth is bounded.
    pub(super) kernel_ep_chunks: usize,
    /// The notification registry (§101 (notification objects)): name to the page a
    /// [`NotificationPage`] lives at, retyped from its creator's region.
    pub(super) notification_table: generational_table::Table<u64, MAX_NOTIFICATIONS>,
    /// The timer registry (§147 (a timer a userspace service cannot hold)): name to the page a
    /// [`TimerPage`] lives at. The expiry walk iterates it.
    pub(super) timer_table: generational_table::Table<u64, MAX_TIMERS>,
    /// The futex wait queues (§269 (how threads share a process) fork 2): threads parked in
    /// `AddressSpace::WAIT`, by `(space, address)`. Under this lock so that the check of the word,
    /// the park, and a waker's take are one order. See [`futex_wait`].
    pub(super) futexes: inter_process_communication::futex::Futexes<Thread, FUTEX_BUCKETS>,
    /// The process registry: name to the physical address of a process's page.
    pub(super) processes: generational_table::Table<u64, { process::MAX_PROCESSES }>,
}

/// How many buckets the futex table hashes into: a wake walks one, four waiters each on average
/// with the machine full. Twenty-four bytes each.
pub(super) const FUTEX_BUCKETS: usize = 64;

/// The most endpoints that can exist at once: the registry's bound. Region teardown frees a
/// rendezvous's slot, so this bounds concurrency, not creations.
pub(crate) const MAX_RENDEZVOUS: usize = 512;

/// The most rendezvous that were ever live at once on this boot, and how many of those sat on the
/// kernel's own chunks, which only rises. [`MAX_RENDEZVOUS`]'s instrument; it reports and does not
/// gate. The measured peaks are in notes/ipc-tables-lock-inventory.md.
///
/// Context: updated at the one insert, under `IPC_TABLES`.
pub(super) static PEAK_RENDEZVOUS: core::sync::atomic::AtomicUsize =
    core::sync::atomic::AtomicUsize::new(0);
pub(super) static KERNEL_CHUNK_RENDEZVOUS: core::sync::atomic::AtomicUsize =
    core::sync::atomic::AtomicUsize::new(0);

/// `(peak live, created on kernel chunks so far)`: see [`PEAK_RENDEZVOUS`]. Printed by the test
/// suite's closing summary.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))] // the closing summary is the only reader
pub fn rendezvous_pressure() -> (usize, usize) {
    (
        PEAK_RENDEZVOUS.load(Ordering::Relaxed),
        KERNEL_CHUNK_RENDEZVOUS.load(Ordering::Relaxed),
    )
}

/// A rendezvous's name: a generational `slots` name over the rendezvous registry, what an
/// `Object::Rendezvous` capability carries. `u64` like a `ThreadId`, and stale-safe the same way.
pub type RendezvousId = u64;

/// The pages in one of the kernel's rendezvous chunks: a batch size, not a ceiling, since
/// [`create_rendezvous`] carves another when one fills. A normal boot carves one chunk of 128 KiB.
pub(super) const KERNEL_EP_CHUNK_PAGES: u64 = 32;

/// How many chunks the kernel will carve before refusing, derived so the chunks can host
/// [`MAX_RENDEZVOUS`] one-page endpoints: exhaustion always reports the registry full.
pub(super) const MAX_KERNEL_EP_CHUNKS: usize =
    MAX_RENDEZVOUS.div_ceil(KERNEL_EP_CHUNK_PAGES as usize);

/// The lock over every table in [`IpcTables`].
///
/// Ranked above the allocators, because the reaper (`finish_switch`) drops a dead `Thread` while
/// holding it and that drop frees through the kernel MMU lock. Nothing under it allocates, so §9
/// (locking)'s no-allocation-in-IRQ rule holds. See notes/ipc-tables-lock-inventory.md.
pub(super) static IPC_TABLES: IrqSafeMutex<Option<IpcTables>> =
    IrqSafeMutex::new(rank::IPC_TABLES, None);

/// The scheduler before anything is in it, a `.rodata` aggregate [`init`] copies from instead of a
/// value built on the boot stack, which keeps the boot frame under its gate at [`MAX_THREADS`].
pub(super) const EMPTY_TABLES: IpcTables = IpcTables {
    threads: Threads::new(),
    rendezvous_table: generational_table::Table::new(),
    kernel_ep_region: None,
    kernel_ep_chunks: 0,
    notification_table: generational_table::Table::new(),
    timer_table: generational_table::Table::new(),
    futexes: inter_process_communication::futex::Futexes::new(),
    processes: generational_table::Table::new(),
};
