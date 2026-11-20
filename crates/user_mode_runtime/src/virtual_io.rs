//! **Thin safe wrappers over the four `VirtualIo` capability methods**, and the barrier that orders
//! a virtqueue against the device.
//!
//! Each wrapper's only obligation is `invoke`'s own; `abi::virtual_io` states what the kernel checks
//! (`WRITE_REGISTER` refuses the queue-address and notify registers; `SETUP_QUEUE` and `NOTIFY` are
//! bounds-checked against the DMA region and its high-water mark). A module rather than crate-root
//! items, because only a program that owns a `VirtualIo` capability needs them. See
//! notes/user-mode-runtime.md.

/// `READ_REGISTER` at `offset` on the `VirtualIo` capability in `virtual_io_slot`. Reads are DMA-safe, so any
/// aligned register in the device's own slot. Returns the register value, or a negative
/// `abi::Error`.
///
/// Name: ratified 2026-10-08 (calef, #1842, milestone 139 (drive the unsafe count down) round 9's
/// question 2), with [`write_register`], [`setup_queue`] and [`notify`]. His words: "Yes". Refused
/// `virtio_read_reg` (round 7's provisional name; the module already says virtio, and reg is an
/// abbreviation).
pub fn read_register(virtual_io_slot: u64, offset: u64) -> i64 {
    // SAFETY: `svc`/`ecall`. The kernel validates the VirtualIo capability before it reads the
    // register and refuses an offset outside the device's slot.
    unsafe {
        super::invoke(
            virtual_io_slot,
            abi::virtual_io::READ_REGISTER,
            offset,
            0,
            0,
        )
    }
}

/// `WRITE_REGISTER` `value` at `offset` on the `VirtualIo` capability in `virtual_io_slot`. `0` on success; a
/// negative `abi::Error` (`NotPermitted` for a queue-address or notify register, which only
/// [`setup_queue`]/[`notify`] may touch).
///
/// Name: ratified 2026-10-08 (calef, #1842, milestone 139 (drive the unsafe count down) round 9's
/// question 2), with [`read_register`]. His words: "Yes". Refused `virtio_write_reg` (round 7's
/// provisional name).
pub fn write_register(virtual_io_slot: u64, offset: u64, value: u64) -> i64 {
    // SAFETY: `svc`/`ecall`. The kernel validates the VirtualIo capability and refuses any register
    // this call is not DMA-safe to write.
    unsafe {
        super::invoke(
            virtual_io_slot,
            abi::virtual_io::WRITE_REGISTER,
            offset,
            value,
            0,
        )
    }
}

/// `SETUP_QUEUE` virtqueue `queue` (0-indexed; a virtio-net device uses 0 = receive, 1 = transmit)
/// with `descriptors` descriptors, on the `VirtualIo` capability in `virtual_io_slot`. The kernel programs the
/// ring addresses to the fixed offsets of that queue's ring block in the driver's own DMA region;
/// the driver never chooses them. `0` on success; a negative `abi::Error` (`BadQueue`/`WrongObject`
/// if `queue` is out of range or the block does not fit the region).
///
/// Name: ratified 2026-10-08 (calef, #1842, milestone 139 (drive the unsafe count down) round 9's
/// question 2), with [`read_register`]. His words: "Yes". Refused `virtio_setup_queue` (round 7's
/// provisional name).
pub fn setup_queue(virtual_io_slot: u64, descriptors: u64, queue: u64) -> i64 {
    // SAFETY: `svc`/`ecall`. The kernel validates the VirtualIo capability, `queue`, and that the
    // ring block fits the driver's DMA region before it programs anything.
    unsafe {
        super::invoke(
            virtual_io_slot,
            abi::virtual_io::SETUP_QUEUE,
            descriptors,
            queue,
            0,
        )
    }
}

/// `NOTIFY` the device that virtqueue `queue` has new descriptors, on the `VirtualIo` capability in
/// `virtual_io_slot`. `0` on success; a negative `abi::Error` (`DeviceRefused` if a newly-published
/// descriptor points outside the driver's DMA region, in which case the device is not told to go).
///
/// Name: ratified 2026-10-08 (calef, #1842, milestone 139 (drive the unsafe count down) round 9's
/// question 2), with [`read_register`]. His words: "Yes". Refused `virtio_notify` (round 7's
/// provisional name).
pub fn notify(virtual_io_slot: u64, queue: u64) -> i64 {
    // SAFETY: `svc`/`ecall`. The kernel validates every published descriptor on `queue` against the
    // driver's own high-water mark before it lets the device see them.
    unsafe { super::invoke(virtual_io_slot, abi::virtual_io::NOTIFY, queue, 0, 0) }
}

/// **Order this driver's ring writes before a doorbell store to device memory**: what a driver
/// granted direct needs where a mediated one has a system call. `dmb oshst` on aarch64, the
/// barrier Linux's `writel` takes, since `dmb ish` orders against other CPUs only; `fence w,o` on
/// riscv64; on `x86_64` the device mapping is uncacheable and total store ordering (TSO) keeps the stores in order, so
/// only a compiler fence is needed.
///
/// Name: ratified 2026-11-20 (UTC); first minted, minted 2026-11-11 (UTC) by lane/1062-virtio-direct-transport.
pub fn doorbell_barrier() {
    // SAFETY: a barrier has no operands and cannot be unsound; it only constrains ordering.
    #[cfg(target_arch = "aarch64")]
    unsafe {
        core::arch::asm!("dmb oshst", options(nostack, preserves_flags));
    }
    // SAFETY: as above.
    #[cfg(target_arch = "riscv64")]
    unsafe {
        core::arch::asm!("fence w, o", options(nostack, preserves_flags));
    }
    #[cfg(target_arch = "x86_64")]
    // PAIR: none in software. The partner is the device, which reads the rings after it sees the
    // doorbell store; x86_64 keeps stores in order to memory and to MMIO alike, so only the
    // compiler must not move the ring writes past the doorbell.
    core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
    #[cfg(not(any(
        target_arch = "aarch64",
        target_arch = "riscv64",
        target_arch = "x86_64"
    )))]
    compile_error!("doorbell_barrier(): this architecture has no ordering named here.");
}

/// **Order this driver's normal-memory accesses to a virtqueue against the device's**: a descriptor
/// before its available index, that index before the notify, a used index before its payload.
///
/// `dmb ish` on aarch64, `fence` on riscv64; on `x86_64`, TSO and a serializing system call leave only a
/// compiler fence to supply. A fourth architecture fails to build. See notes/user-mode-runtime.md
/// and notes/architecture-list-sweep.md, finding 9.
///
/// Name: ratified 2026-11-20 (UTC); first minted, the lane for milestone 186 (derive the architecture list), #1143, 2026-09-23
/// (UTC); marker written 2026-10-12 (UTC). Milestone 139 (drive the unsafe count down) round 11
/// recorded it as outside calef's 2026-10-08 (UTC) renames of this module's wrappers. No other
/// name is recorded.
#[doc(alias = "virtio")]
pub fn virtual_io_ring_barrier() {
    // No `nomem`. The Rust reference (Inline assembly, Options) defines it as a block that "does not
    // read or write to any memory accessible outside of the `asm!` block", which lets the compiler
    // cache values in registers and move loads and stores across it, and it adds that the compiler
    // "also assumes that this `asm!` block does not perform any kind of synchronization with other
    // threads, e.g. via fences". A barrier exists to order memory, so it must not claim either.
    // SAFETY: a barrier has no operands and cannot be unsound; it only constrains ordering.
    #[cfg(target_arch = "aarch64")]
    unsafe {
        core::arch::asm!("dmb ish", options(nostack, preserves_flags));
    }
    // SAFETY: as above.
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

    // A `cfg` chain with no `else` compiles to an empty body, so a fourth architecture fails to
    // build here rather than failing to order.
    #[cfg(not(any(
        target_arch = "aarch64",
        target_arch = "riscv64",
        target_arch = "x86_64"
    )))]
    compile_error!(
        "virtual_io_ring_barrier(): this architecture has no ordering named here. Decide what orders a \
         virtqueue publish against a device's read on it, and add an arm; do not let this fall \
         through to an empty body."
    );
}
