//! **Thin safe wrappers over the four `Virtio` capability methods**, and the barrier that orders
//! a virtqueue against the device.
//!
//! Each wrapper's only obligation is `invoke`'s own; `abi::virtio` states what the kernel checks
//! (`WRITE_REGISTER` refuses the queue-address and notify registers; `SETUP_QUEUE` and `NOTIFY` are
//! bounds-checked against the DMA region and its high-water mark). A module rather than crate-root
//! items, because only a program that owns a `Virtio` capability needs them. See
//! notes/user-mode-runtime.md.

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
/// `dmb ish` on aarch64 and `fence` on riscv64, both weakly ordered. On `x86_64`, TSO already gives
/// the store-store and load-load orders asked for, and every notify leaves through a serializing
/// syscall, so a compiler fence is the whole answer. A fourth architecture fails to build. See
/// notes/user-mode-runtime.md and notes/architecture-list-sweep.md, finding 9.

/// Name is **provisional** (a lane's, not ratified): it sits in this module beside the four
/// capability wrappers and took the `virtio_` prefix they then carried. calef's ruling of
/// 2026-10-08 (UTC), #1842, dropped that prefix from those four ([`read_register`] and its
/// siblings); whether this one follows is not yet ruled.
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

    // A `cfg` chain with no `else` compiles to an empty body, so a fourth architecture fails to
    // build here rather than failing to order.
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
