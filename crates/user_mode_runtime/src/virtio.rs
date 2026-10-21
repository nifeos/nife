//! **Thin safe wrappers over the four `Virtio` capability methods** (milestone 139 round 7).
//!
//! Four drivers (`gpu_driver.rs`, `entropy.rs`, `keyboard_driver.rs`, `virtio_net_transport.rs`) each called
//! `invoke(VIRTIO, abi::virtio::READ_REG/WRITE_REG/SETUP_QUEUE/NOTIFY, ...)` directly, eighteen
//! call sites in all, every one carrying the identical `// SAFETY:` comment: "the kernel validates
//! the capability and the method before acting." That comment is `invoke`'s own contract, not a
//! per-call obligation any of these four methods add: `abi::virtio`'s own doc already states what
//! the kernel checks (`WRITE_REGISTER` refuses the queue-address and notify registers;
//! `SETUP_QUEUE` and `NOTIFY` are bounds-checked against the DMA region and its high-water mark),
//! so the raw `invoke` site could not add anything a wrapper cannot add just as well.
//!
//! Scoped to a module, not lifted into the crate root, because unlike [`super::send`] or
//! [`super::reply`] these four methods are meaningful to exactly the programs that own a `Virtio`
//! capability: the same "opt in by importing the module" shape
//! [`super::initial_ramdisk`] and [`super::mapped_window`] already use for functionality most programs
//! never touch.

/// `READ_REGISTER` at `off` on the `Virtio` capability in `virtio_slot`. Reads are DMA-safe, so any
/// aligned register in the device's own slot. Returns the register value, or a negative
/// `abi::Error`.
///
/// Name: ratified 2026-10-08 (calef, #1842, milestone 139 (drive the unsafe count down) round 9's
/// question 2), with [`write_register`], [`setup_queue`] and [`notify`]. His words: "Yes". Refused
/// `virtio_read_reg` (round 7's provisional name; the module already says virtio, and reg is an
/// abbreviation).
pub fn read_register(virtio_slot: u64, off: u64) -> i64 {
    // SAFETY: `svc`/`ecall`. The kernel validates the Virtio capability before it reads the
    // register and refuses an offset outside the device's slot.
    unsafe { super::invoke(virtio_slot, abi::virtio::READ_REGISTER, off, 0, 0) }
}

/// `WRITE_REGISTER` `val` at `off` on the `Virtio` capability in `virtio_slot`. `0` on success; a
/// negative `abi::Error` (`NotPermitted` for a queue-address or notify register, which only
/// [`setup_queue`]/[`notify`] may touch).
///
/// Name: ratified 2026-10-08 (calef, #1842, milestone 139 (drive the unsafe count down) round 9's
/// question 2), with [`read_register`]. His words: "Yes". Refused `virtio_write_reg` (round 7's
/// provisional name).
pub fn write_register(virtio_slot: u64, off: u64, val: u64) -> i64 {
    // SAFETY: `svc`/`ecall`. The kernel validates the Virtio capability and refuses any register
    // this call is not DMA-safe to write.
    unsafe { super::invoke(virtio_slot, abi::virtio::WRITE_REGISTER, off, val, 0) }
}

/// `SETUP_QUEUE` virtqueue `queue` (0-indexed; a virtio-net device uses 0 = receive, 1 = transmit)
/// with `num` descriptors, on the `Virtio` capability in `virtio_slot`. The kernel programs the
/// ring addresses to the fixed offsets of that queue's ring block in the driver's own DMA region;
/// the driver never chooses them. `0` on success; a negative `abi::Error` (`BadQueue`/`WrongObject`
/// if `queue` is out of range or the block does not fit the region).
///
/// Name: ratified 2026-10-08 (calef, #1842, milestone 139 (drive the unsafe count down) round 9's
/// question 2), with [`read_register`]. His words: "Yes". Refused `virtio_setup_queue` (round 7's
/// provisional name).
pub fn setup_queue(virtio_slot: u64, num: u64, queue: u64) -> i64 {
    // SAFETY: `svc`/`ecall`. The kernel validates the Virtio capability, `queue`, and that the
    // ring block fits the driver's DMA region before it programs anything.
    unsafe { super::invoke(virtio_slot, abi::virtio::SETUP_QUEUE, num, queue, 0) }
}

/// `NOTIFY` the device that virtqueue `queue` has new descriptors, on the `Virtio` capability in
/// `virtio_slot`. `0` on success; a negative `abi::Error` (`DeviceRefused` if a newly-published
/// descriptor points outside the driver's DMA region, in which case the device is not told to go).
///
/// Name: ratified 2026-10-08 (calef, #1842, milestone 139 (drive the unsafe count down) round 9's
/// question 2), with [`read_register`]. His words: "Yes". Refused `virtio_notify` (round 7's
/// provisional name).
pub fn notify(virtio_slot: u64, queue: u64) -> i64 {
    // SAFETY: `svc`/`ecall`. The kernel validates every published descriptor on `queue` against the
    // driver's own high-water mark before it lets the device see them.
    unsafe { super::invoke(virtio_slot, abi::virtio::NOTIFY, queue, 0, 0) }
}

/// **Order this driver's normal-memory accesses to a virtqueue against the device's.** Call it
/// between publishing a descriptor and bumping the available index that advertises it, between
/// that index and the notify, and between reading a used index and the payload that index gates.
///
/// Name is **provisional** (a lane's, not ratified): it sits in this module beside the four
/// capability wrappers and took the `virtio_` prefix they then carried. calef's ruling of
/// 2026-10-08 (UTC), #1842, dropped that prefix from those four ([`read_register`] and its
/// siblings); whether this one follows is not yet ruled.
///
/// **Why one function rather than five.** This body was copied into `crates/virtio` and four
/// programs under `components/src/`, and every copy had an `aarch64` arm, a `riscv64` arm, and no
/// third arm, so on `x86_64-unknown-none` the whole function compiled to nothing: not the machine
/// barrier that `TSO` makes unnecessary, and not the compiler barrier that nothing makes
/// unnecessary. An empty function is not a warning, so `script/lint`'s `x86_64` user pass, written
/// to catch exactly this, could not. Five copies meant five places to be wrong and five places to
/// fix; the `compile_error!` below now exists once, and a sixth driver gets it by calling this
/// rather than by remembering. See notes/architecture-list-sweep.md, finding 9.
///
/// **What each architecture needs, and why they differ.** aarch64 and riscv64 are weakly ordered,
/// so both halves (machine and compiler) need a real instruction: `dmb ish` orders the inner
/// shareable domain, which is where a coherent DMA agent observes, and RISC-V takes one full
/// `fence`, the same conservative choice `kernel::arch::direct_memory_access_write_barrier` makes.
/// `x86_64` is TSO, and the only orderings this function is ever asked for are store-store
/// (descriptor before index, index before notify) and load-load (used index before payload), both
/// of which the machine already guarantees for the write-back memory a coherent device shares. The
/// store-load case TSO does
/// *not* give is not reachable here: every notify and every register access in these drivers
/// leaves the process through a syscall instruction, which is serializing. So on `x86_64` the
/// compiler is the entire exposure and [`core::sync::atomic::compiler_fence`] is the whole answer.
/// `components/src/non_volatile_memory_express.rs` keeps its own stronger barrier because its
/// doorbell is a direct store to a device-typed page in this address space rather than a syscall.
///
/// Name: provisional, the lane for milestone 186 (derive the architecture list), #1143, 2026-09-23
/// (UTC); marker written 2026-10-12 (UTC). Milestone 139 (drive the unsafe count down) round 11
/// recorded it as outside calef's 2026-10-08 (UTC) renames of this module's wrappers. No other
/// name is recorded.
pub fn virtio_ring_barrier() {
    // SAFETY: a barrier has no operands and cannot be unsound; it only constrains ordering.
    #[cfg(target_arch = "aarch64")]
    unsafe {
        core::arch::asm!("dmb ish", options(nostack, nomem, preserves_flags));
    }
    // SAFETY: as above. `fence` carries no `nomem`, deliberately: ordering memory is its job.
    #[cfg(target_arch = "riscv64")]
    unsafe {
        core::arch::asm!("fence", options(nostack, preserves_flags));
    }
    #[cfg(target_arch = "x86_64")]
    // PAIR: none in this tree, and that is the whole point. The other observer is the virtio
    // device itself, which reads the descriptor table and the available index by DMA and has no
    // fence to name. So this is deliberately one-sided: the machine half of the pairing is TSO
    // plus cache coherence, and this fence supplies only the half a compiler could break, which is
    // sinking a descriptor store past the index store that advertises it. The aarch64 and riscv64
    // arms above are one-sided for the same reason and against the same non-fence partner. See
    // notes/memory-ordering.md.
    core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);

    // A fourth architecture fails to build here rather than failing to order, which is the whole
    // point of this arm: the bug this function was lifted to fix was a `cfg` chain with no `else`
    // compiling to an empty body. `entropy_backend`'s backend ladder is the in-tree precedent for
    // ending in `compile_error!` rather than a fallback.
    #[cfg(not(any(
        target_arch = "aarch64",
        target_arch = "riscv64",
        target_arch = "x86_64"
    )))]
    compile_error!(
        "virtio_ring_barrier(): this architecture has no ordering named here. Decide what orders a \
         virtqueue publish against a device's read on it, and add an arm; do not let this fall \
         through to an empty body."
    );
}
