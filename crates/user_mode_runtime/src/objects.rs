//! **Kernel objects**: page frames and memory regions retyped, sliced, mapped and revoked,
//! address-space mappings, thread control blocks configured and started, and deleting a capability.
//!
//! The crate root re-exports every public item, so each keeps its crate-root path.
//!
//! Name: provisional, lane maintainer/files-over-1500 (#2017), 2026-10-12 (UTC), for the module and
//! the file. Named for the calls it holds, one module per seam of the old crate root; the pull
//! request records no other name considered.

use crate::syscall::{invoke, trap6};

/// **Map the `PageFrame` capability in `frame_slot` at `va`**, drawing the page tables from the untyped
/// in `memory_region_slot`. `true` if the page is now there.
///
/// The verb a process that holds a page uses to put it in its own address space (milestone 108
/// (the drivers move onto frame capabilities)). Unlike a page wired in at spawn, the mapping is in
/// the revocation database, so `PageFrame::REVOKE` can take it back. See notes/frames.md.
///
/// `writable` needs `WRITE` on the frame; a read-only mapping needs `READ`. A caller handed a
/// narrowed view that asks for more than it holds gets `false` and no mapping, which is the rights
/// ladder doing its job rather than an error to route around.
pub fn map_page_frame(frame_slot: u64, va: u64, writable: bool, memory_region_slot: u64) -> bool {
    // SAFETY: `svc`/`ecall`. The kernel validates the frame capability, the rights, the address and
    // the untyped before it touches a page table.
    unsafe {
        invoke(
            frame_slot,
            abi::page_frame::MAP,
            va,
            writable as u64,
            memory_region_slot,
        ) == 0
    }
}

/// Whether a capability is in `slot`, without touching whatever it names (milestone 139 round 7).
/// Invoke a method number no object type defines, so the call can only be refused, and read
/// *which* refusal came back: an empty slot answers `NoSuchSlot`, and a real object answers
/// `BadMethod`, a refusal from something that exists.
///
/// Lifted out of `date.rs`'s `clock_page` probe, which four more programs (`pgrep`, `pmap`, `ps`,
/// `watch`) had each copied verbatim, one of them naming the duplication out loud in its own doc
/// comment without anyone lifting it. The exact §94 (what may live in a userspace library) shape
/// the crate-level docs in `lib.rs` describe.
///
/// Name: ratified 2026-10-08 (calef, #1842, milestone 139 (drive the unsafe count down) round 9's
/// question 2). His words: "Ratify ... `is_granted`." Refused `holds_capability` (the
/// boolean-predicate worklist had already given this probe its `is_` shape).
pub fn is_granted(slot: u64) -> bool {
    /// A method number no object type defines, so the invocation can only ever be refused.
    const NO_SUCH_METHOD: u64 = 0xffff;
    // SAFETY: a syscall that cannot succeed; the kernel validates the slot before the method.
    let r = unsafe { invoke(slot, NO_SUCH_METHOD, 0, 0, 0) };
    r != abi::Error::NoSuchSlot as i64
}

/// `RETYPE` one page out of the untyped in `memory_region_slot` into a `PageFrame` capability the
/// caller now holds. Returns the slot the frame landed in, or a negative `abi::Error`
/// (`OutOfMemory` when the untyped is exhausted or the caller's table is full).
///
/// Name: ratified 2026-10-08 (calef, #1842, milestone 139 (drive the unsafe count down) round 9's
/// question 2), with [`retype_into_kernel_object`]. His words: "`retype_into_page_frame` and
/// `retype_into_kernel_object`". Refused `retype_page_frame` (round 7's provisional name).
pub fn retype_into_page_frame(memory_region_slot: u64) -> i64 {
    // SAFETY: `svc`/`ecall`. The kernel validates the untyped capability and its remaining budget.
    unsafe { invoke(memory_region_slot, abi::memory_region::RETYPE, 0, 0, 0) }
}

/// `RETYPE` a run of `pages` contiguous pages into one `PageFrame` capability, per §102 (a Frame
/// names a run of pages), `0` meaning one. Returns the slot, or a negative `abi::Error`: `OutOfMemory` when the run does not
/// fit, in which case the region's budget is untouched.
///
/// **The one call site in the tree that passes a count other than `0`**, and `script/lint` holds
/// it to that ("every RETYPE outside the run wrapper passes 0"), so a caller cannot ask for a run
/// by accident through a raw `invoke`.
///
/// Name: provisional (the lane for milestone 23 (a capability-routed component OS with live
/// replacement), 2026-09-26).
pub fn retype_page_frame_run(memory_region_slot: u64, pages: u64) -> i64 {
    // SAFETY: as `retype_into_page_frame`; the kernel checks the count against the remaining budget.
    unsafe { invoke(memory_region_slot, abi::memory_region::RETYPE, pages, 0, 0) } // run wrapper
}

/// **`page_frame::SLICE`: a capability naming `count` pages of the run in `frame_slot`, from page
/// `first`**, with that capability's rights (milestone 599 (a frame per filesystem client channel)).
/// The new slot, or a negative `abi::Error`. Needs `GRANT`. `jig` reads a package into one run and
/// hands the installer a page of it at a time (milestone 809 (the package client becomes a
/// program)). Name follows the method; provisional.
pub fn slice_page_frame(frame_slot: u64, first: u64, count: u64) -> i64 {
    // SAFETY: `svc`/`ecall`; the kernel validates the capability, the range and the right.
    unsafe { invoke(frame_slot, abi::page_frame::SLICE, first, count, 0) }
}

/// `RETYPE_OBJ` one page out of the untyped in `region` into a kernel object of
/// `objtype` (see [`abi::objtype`]). Returns the slot holding a full-rights capability to the new
/// object, or a negative `abi::Error` (`BadMethod` for an unknown `objtype`, `OutOfMemory` when the
/// untyped or either table is exhausted).
///
/// Name: ratified 2026-10-08 (calef, #1842, milestone 139 (drive the unsafe count down) round 9's
/// question 2), with [`retype_into_page_frame`]. His words: "`retype_into_page_frame` and
/// `retype_into_kernel_object`". Refused `retype_object` (round 7's provisional name),
/// `retype_kernel_object` (ratified earlier the same day, then superseded by this ruling).
pub fn retype_into_kernel_object(region: u64, objtype: u64) -> i64 {
    // SAFETY: `svc`/`ecall`. The kernel validates the untyped, the objtype and the budget before it
    // touches a page.
    unsafe { invoke(region, abi::memory_region::RETYPE_OBJ, objtype, 0, 0) }
}

/// `SPLIT` `pages` off the untyped's unspent budget in `memory_region_slot` into a new child
/// untyped. Returns the slot holding a full-rights capability to the child, or a negative
/// `abi::Error` (`NotPermitted` without `WRITE`, `OutOfMemory` when the budget or a table is
/// exhausted).
///
/// Name: ratified 2026-10-08 (calef, #1842, milestone 139 (drive the unsafe count down) round 9's
/// question 2). His words: "Ratify `split_memory_region`, `destroy_memory_region`,
/// `map_memory_region_page`, `revoke_page_frame`, ..." Refused `split_region` (round 7's
/// provisional name; the object is a memory region).
pub fn split_memory_region(memory_region_slot: u64, pages: u64) -> i64 {
    // SAFETY: `svc`/`ecall`. The kernel validates `WRITE` and the remaining budget before it splits
    // anything off.
    unsafe { invoke(memory_region_slot, abi::memory_region::SPLIT, pages, 0, 0) }
}

/// `DESTROY` the region in `memory_region_slot`: reclaim it and every object retyped from it
/// (object revocation, the region-owner's half). `0` on success; a negative `abi::Error`
/// (`NotPermitted` while a live thread still occupies it, or if it has been `SPLIT` into children,
/// or without `WRITE`).
///
/// Name: ratified 2026-10-08 (calef, #1842, milestone 139 (drive the unsafe count down) round 9's
/// question 2). His words: "Ratify `split_memory_region`, `destroy_memory_region`,
/// `map_memory_region_page`, `revoke_page_frame`, ..." Refused `destroy_region` (round 7's
/// provisional name).
pub fn destroy_memory_region(memory_region_slot: u64) -> i64 {
    // SAFETY: `svc`/`ecall`. The kernel validates `WRITE` and that nothing still occupies or splits
    // from the region before it reclaims anything.
    unsafe { invoke(memory_region_slot, abi::memory_region::DESTROY, 0, 0, 0) }
}

/// `USAGE`: how many pages of the region in `memory_region_slot` are spent on `record`, one of
/// [`abi::usage`]'s selectors. The page count, or a negative `abi::Error` (`NotPermitted` without
/// `ENUMERATE`, `BadMethod` for an unknown record).
///
/// Name: provisional. The method and its object, the shape [`split_memory_region`] has.
pub fn memory_region_usage(memory_region_slot: u64, record: u64) -> i64 {
    // SAFETY: `svc`/`ecall`. USAGE reads one record and writes nothing of the caller's; the kernel
    // validates `ENUMERATE` and the record before it answers.
    unsafe { invoke(memory_region_slot, abi::memory_region::USAGE, record, 0, 0) }
}

/// `MAP` (untyped): retype one page out of the untyped in `memory_region_slot` and map it,
/// writable, at `va` in the caller's own address space, in one step. `0` on success; a negative
/// `abi::Error` (`OutOfMemory` when the untyped is exhausted).
///
/// The one-step twin of [`retype_into_page_frame`] followed by [`map_page_frame`]: this never produces
/// a `PageFrame` capability the caller can delegate or revoke, it just spends the untyped's budget
/// directly on a mapped page.
///
/// Name: ratified 2026-10-08 (calef, #1842, milestone 139 (drive the unsafe count down) round 9's
/// question 2). His words: "Ratify `split_memory_region`, `destroy_memory_region`,
/// `map_memory_region_page`, `revoke_page_frame`, ..." Refused `map_region_page` (round 7's
/// provisional name).
pub fn map_memory_region_page(memory_region_slot: u64, va: u64) -> i64 {
    // SAFETY: `svc`/`ecall`. The kernel validates the untyped, the address, and maps a fresh page
    // from its own budget.
    unsafe { invoke(memory_region_slot, abi::memory_region::MAP, va, 0, 0) }
}

/// `REVOKE` the `PageFrame` in `frame_slot`: un-share it (or, on a device capability, take it back;
/// see `abi::page_frame::REVOKE`'s own doc for the asymmetry). `0` on success; a negative
/// `abi::Error` (`NotPermitted` without `GRANT`).
///
/// Name: ratified 2026-10-08 (calef, #1842, milestone 139 (drive the unsafe count down) round 9's
/// question 2). His words: "Ratify `split_memory_region`, `destroy_memory_region`,
/// `map_memory_region_page`, `revoke_page_frame`, ..." Refused `revoke_frame` (round 7's
/// provisional name; the kernel already spells the object `page_frame`).
pub fn revoke_page_frame(frame_slot: u64) -> i64 {
    // SAFETY: `svc`/`ecall`. The kernel validates `GRANT` before it unmaps and deletes every
    // capability to the page.
    unsafe { invoke(frame_slot, abi::page_frame::REVOKE, 0, 0, 0) }
}

/// `MAP_INTO`: map the frame in `frame_slot` into the address space named by `aspace`, at
/// `va`, per `mode` (`abi::address_space::MAP_RO`/`MAP_RW`/`MAP_CODE`). `0` on success; a negative
/// `abi::Error`.
///
/// Safe for the reason [`map_page_frame`] is: the kernel checks the address-space capability's
/// `WRITE`, the frame's rights against `mode`, and the address before it touches a page table. A
/// caller racing what this changes has a bug in its own code, not a Rust-safety obligation. See
/// notes/user-mode-runtime.md.
///
/// Name: ratified 2026-10-08 (calef, #1842, milestone 139 (drive the unsafe count down) round 9's
/// question 2). His words: "Ratify `retype_kernel_object` `address_space_map_into`." Refused
/// `map_into` (round 7's provisional name; the method is `address_space::MAP_INTO`).
pub fn address_space_map_into(aspace: u64, va: u64, frame_slot: u64, mode: u64) -> i64 {
    // SAFETY: `svc`/`ecall`. The kernel validates the address-space capability, the frame's rights
    // against `mode`, and the address before it touches a page table.
    unsafe { invoke(aspace, abi::address_space::MAP_INTO, va, frame_slot, mode) }
}

/// `CAP_INSERT`: copy the capability in the caller's `capability_slot`, narrowed to `rights`, into the
/// child TCB's capability table. `target = 0` places it in the first free slot; `target = n` places
/// it in slot `n - 1` (see `abi::thread_control_block::CAP_INSERT`'s own doc for the supervision-slot
/// use of an explicit target). Returns the slot it landed in, or a negative `abi::Error`.
///
/// Name: ratified 2026-10-08 (calef, #1842, milestone 139 (drive the unsafe count down) round 9's
/// question 2). His words: "Ratify ... `thread_control_block_capability_insert`,
/// `thread_control_block_configure`, `thread_control_block_start`, ..." Refused `tcb_cap_insert`
/// (round 7's provisional name; §154 (the acronym test is whether the phrase is spoken) spells out
/// thread control block, and capability is spelled out).
pub fn thread_control_block_capability_insert(
    tcb_slot: u64,
    capability_slot: u64,
    rights: u64,
    target: u64,
) -> i64 {
    // SAFETY: `svc`/`ecall`. The kernel validates `WRITE` on the TCB and `GRANT` on the inserted
    // capability before it copies anything.
    unsafe {
        invoke(
            tcb_slot,
            abi::thread_control_block::CAP_INSERT,
            capability_slot,
            rights,
            target,
        )
    }
}

/// `CONFIGURE`: bind the address space in `aspace_slot` to the (embryo) TCB in `tcb_slot`, and set
/// where EL0 execution begins (`entry`) and on what user stack (`user_sp`). The capability in
/// `aspace_slot` is consumed; the space keeps its name, so a copy made first still names it while
/// the thread runs (§249 (a running address space stays nameable)). On success, the address of the
/// child's current-CPU page (milestone 812 (`std::thread::spawn` runs real threads in one address
/// space)), never negative; a negative `abi::Error` otherwise, `WrongObject` for a space already
/// bound.
///
/// Name: ratified 2026-10-08 (calef, #1842, milestone 139 (drive the unsafe count down) round 9's
/// question 2). His words: "Ratify ... `thread_control_block_capability_insert`,
/// `thread_control_block_configure`, `thread_control_block_start`, ..." Refused `tcb_configure`
/// (round 7's provisional name).
pub fn thread_control_block_configure(
    tcb_slot: u64,
    entry: u64,
    user_sp: u64,
    aspace_slot: u64,
) -> i64 {
    // SAFETY: `svc`/`ecall`. The kernel validates `WRITE` on both capabilities before it binds them.
    unsafe {
        invoke(
            tcb_slot,
            abi::thread_control_block::CONFIGURE,
            entry,
            user_sp,
            aspace_slot,
        )
    }
}

/// `START`: make the TCB in `tcb_slot` runnable. `x0`, `x1`, `x2` seed the child's own `x0`/`x1`/
/// `x2` (`a0`/`a1`/`a2` on RISC-V) on its first instruction, the kernel-side spelling of "this
/// thread's input" (`abi::thread_control_block::START`'s own doc comment says `_, _, _`, which is
/// stale: `kernel/src/syscall.rs`'s `START` arm forwards all three to `sched::start_thread_control_block`
/// unconditionally). Refuses a half-built thread (no bound address space, or no entry). `0` on
/// success; a negative `abi::Error`.
///
/// Name: ratified 2026-10-08 (calef, #1842, milestone 139 (drive the unsafe count down) round 9's
/// question 2). His words: "Ratify ... `thread_control_block_capability_insert`,
/// `thread_control_block_configure`, `thread_control_block_start`, ..." Refused `tcb_start` (round
/// 7's provisional name).
pub fn thread_control_block_start(tcb_slot: u64, x0: u64, x1: u64, x2: u64) -> i64 {
    // SAFETY: `svc`/`ecall`. The kernel validates `WRITE` and that the TCB is whole before it joins
    // the run queue.
    unsafe { invoke(tcb_slot, abi::thread_control_block::START, x0, x1, x2) }
}

/// `SUSPEND`: stop the thread whose TCB is in `tcb_slot` until a [`thread_control_block_resume`],
/// keeping its registers (DECISIONS §300 (the paused state, a TCB verb, silent)). Silent: nobody is told. When this returns 0 the thread
/// is off every core. `0` on success; a negative `abi::Error`.
///
/// Name: provisional (lane/48-tcb-suspend-resume, 2026-10-18 UTC).
pub fn thread_control_block_suspend(tcb_slot: u64) -> i64 {
    // SAFETY: `svc`/`ecall`. The kernel validates `WRITE`; nothing in this address space changes.
    unsafe { invoke(tcb_slot, abi::thread_control_block::SUSPEND, 0, 0, 0) }
}

/// `RESUME`: let the thread whose TCB is in `tcb_slot` run on from where it was suspended
/// (DECISIONS §300). `0` on success, also when it was not suspended; a negative `abi::Error`.
///
/// Name: provisional (lane/48-tcb-suspend-resume, 2026-10-18 UTC).
pub fn thread_control_block_resume(tcb_slot: u64) -> i64 {
    // SAFETY: as `thread_control_block_suspend`.
    unsafe { invoke(tcb_slot, abi::thread_control_block::RESUME, 0, 0, 0) }
}

/// Drop the capability in `slot` from this thread's capability table (`SYS_CAP_DELETE`). Deleting an empty
/// slot is a no-op. A program that retypes many objects (a loader, a spawner) frees each slot as
/// soon as it is done with it, so its fixed capability table does not fill. Through [`trap6`] for
/// [`yield_now`](crate::yield_now)'s reason: the kernel answers in the register `slot` went in on.
pub fn cap_delete(slot: u64) {
    // SAFETY: SYS_CAP_DELETE frees a slot in the caller's own capability table, nothing to clean up.
    let _ = unsafe { trap6(abi::SYS_CAP_DELETE, [slot, 0, 0, 0, 0, 0]) };
}
