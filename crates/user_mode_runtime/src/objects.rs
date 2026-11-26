//! **Kernel objects**: page frames and memory regions retyped, sliced, mapped and revoked,
//! address-space mappings, thread control blocks configured and started, and deleting a capability.
//!
//! The crate root re-exports every public item, so each keeps its crate-root path.
//!
//! Name: ratified 2026-11-20 (UTC); first minted, lane maintainer/files-over-1500 (#2017), 2026-10-12 (UTC), for the module and
//! the file. Named for the calls it holds, one module per seam of the old crate root; the pull
//! request records no other name considered.

use moho_abi::system_call as typed;

use crate::system_call::{invoke, trap_call};

/// **Map the `PageFrame` capability in `frame_slot` at `virtual_address`**, drawing the page tables from the untyped
/// in `memory_region_slot`. `true` if the page is now there.
///
/// The verb a process that holds a page uses to put it in its own address space. Unlike a page
/// wired in at spawn, the mapping is in the revocation database, so `PageFrame::REVOKE` can take it
/// back.
///
/// `writable` needs `WRITE` on the frame; a read-only mapping needs `READ`. A caller handed a
/// narrowed view that asks for more than it holds gets `false` and no mapping, which is the rights
/// ladder doing its job rather than an error to route around.
pub fn map_page_frame(
    frame_slot: u64,
    virtual_address: u64,
    writable: bool,
    memory_region_slot: u64,
) -> bool {
    // SAFETY: `svc`/`ecall`. The kernel validates the frame capability, the rights, the address and
    // the untyped before it touches a page table.
    unsafe {
        trap_call(typed::page_frame::map(
            frame_slot,
            virtual_address,
            writable,
            memory_region_slot,
        ))[0]
            == 0
    }
}

/// Whether a capability is in `slot`, without touching whatever it names.
/// Invoke a method number no object type defines, so the call can only be refused, and read
/// *which* refusal came back: an empty slot answers `NoSuchSlot`, and a real object answers
/// `BadMethod`, a refusal from something that exists.
///
/// One copy for every program that probes a slot before it uses it, rather than one per program:
/// a pure wrapper over a system call, the shape the crate root describes.
///
/// Name: ratified 2026-10-08 (calef, #1842, milestone 139 (drive the unsafe count down) round 9's
/// question 2). His words: "Ratify ... `is_granted`." Refused `holds_capability` (the
/// boolean-predicate worklist had already given this probe its `is_` shape).
///
/// Relies on the kernel answering an unknown method on a held slot with `BadMethod` and on an
/// empty slot with `NoSuchSlot`, which is what tells the two apart.
pub fn is_granted(slot: u64) -> bool {
    /// A method number no object type defines, so the invocation can only ever be refused.
    const NO_SUCH_METHOD: u64 = 0xffff;
    // SAFETY: a system call that cannot succeed; the kernel validates the slot before the method.
    let r = unsafe { invoke(slot, NO_SUCH_METHOD, 0, 0, 0) };
    r != moho_abi::Error::NoSuchSlot as i64
}

/// `RETYPE` one page out of the untyped in `memory_region_slot` into a `PageFrame` capability the
/// caller now holds. Returns the slot the frame landed in, or a negative `moho_abi::Error`
/// (`OutOfMemory` when the untyped is exhausted or the caller's table is full).
///
/// Name: ratified 2026-10-08 (calef, #1842, milestone 139 (drive the unsafe count down) round 9's
/// question 2), with [`retype_into_kernel_object`]. His words: "`retype_into_page_frame` and
/// `retype_into_kernel_object`". Refused `retype_page_frame` (round 7's provisional name).
pub fn retype_into_page_frame(memory_region_slot: u64) -> i64 {
    // SAFETY: `svc`/`ecall`. The kernel validates the untyped capability and its remaining budget.
    unsafe { trap_call(typed::memory_region::retype(memory_region_slot, 0))[0] as i64 }
}

/// `RETYPE` a run of `pages` contiguous pages into one `PageFrame` capability (a frame names a run
/// of pages), `0` meaning one. Returns the slot, or a negative `moho_abi::Error`: `OutOfMemory` when the run does not
/// fit, in which case the region's budget is untouched.
///
/// **The one call site in this crate that passes a count other than `0`**: every other `RETYPE`
/// passes `0`, so a caller cannot ask for a run by accident through a raw `invoke`.
///
/// Name: ratified 2026-11-20 (UTC); first minted (the lane for milestone 23 (a capability-routed component OS with live
/// replacement), 2026-09-26).
pub fn retype_page_frame_run(memory_region_slot: u64, pages: u64) -> i64 {
    // SAFETY: as `retype_into_page_frame`; the kernel checks the count against the remaining budget.
    unsafe { trap_call(typed::memory_region::retype(memory_region_slot, pages))[0] as i64 }
}

/// **`page_frame::SLICE`: a capability naming `count` pages of the run in `frame_slot`, from page
/// `first`**, with that capability's rights. The new slot, or a negative `moho_abi::Error`. Needs
/// `GRANT`. A loader that reads a package into one run hands the installer a page of it at a time
/// this way. Name follows the method.
pub fn slice_page_frame(frame_slot: u64, first: u64, count: u64) -> i64 {
    // SAFETY: `svc`/`ecall`; the kernel validates the capability, the range and the right.
    unsafe { trap_call(typed::page_frame::slice(frame_slot, first, count))[0] as i64 }
}

/// `RETYPE_OBJECT` one page out of the untyped in `region` into a kernel object of
/// `object_type` (see [`moho_abi::object_type`]). Returns the slot holding a full-rights capability to the new
/// object, or a negative `moho_abi::Error` (`BadMethod` for an unknown `object_type`, `OutOfMemory` when the
/// untyped or either table is exhausted).
///
/// Name: ratified 2026-10-08 (calef, #1842, milestone 139 (drive the unsafe count down) round 9's
/// question 2), with [`retype_into_page_frame`]. His words: "`retype_into_page_frame` and
/// `retype_into_kernel_object`". Refused `retype_object` (round 7's provisional name),
/// `retype_kernel_object` (ratified earlier the same day, then superseded by this ruling).
pub fn retype_into_kernel_object(region: u64, object_type: u64) -> i64 {
    // SAFETY: `svc`/`ecall`. The kernel validates the untyped, the object_type and the budget before it
    // touches a page.
    unsafe {
        trap_call(typed::memory_region::retype_object(
            region,
            object_type,
            0,
            0,
        ))[0] as i64
    }
}

/// `SPLIT` `pages` off the untyped's unspent budget in `memory_region_slot` into a new child
/// untyped. Returns the slot holding a full-rights capability to the child, or a negative
/// `moho_abi::Error` (`NotPermitted` without `WRITE`, `OutOfMemory` when the budget or a table is
/// exhausted).
///
/// Name: ratified 2026-10-08 (calef, #1842, milestone 139 (drive the unsafe count down) round 9's
/// question 2). His words: "Ratify `split_memory_region`, `destroy_memory_region`,
/// `map_memory_region_page`, `revoke_page_frame`, ..." Refused `split_region` (round 7's
/// provisional name; the object is a memory region).
pub fn split_memory_region(memory_region_slot: u64, pages: u64) -> i64 {
    // SAFETY: `svc`/`ecall`. The kernel validates `WRITE` and the remaining budget before it splits
    // anything off.
    unsafe { trap_call(typed::memory_region::split(memory_region_slot, pages))[0] as i64 }
}

/// `DESTROY` the region in `memory_region_slot`: reclaim it and every object retyped from it
/// (object revocation, the region-owner's half). `0` on success; a negative `moho_abi::Error`
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
    unsafe { trap_call(typed::memory_region::destroy(memory_region_slot))[0] as i64 }
}

/// `USAGE`: how many pages of the region in `memory_region_slot` are spent on `record`, one of
/// [`moho_abi::usage`]'s selectors. The page count, or a negative `moho_abi::Error` (`NotPermitted` without
/// `ENUMERATE`, `BadMethod` for an unknown record).
///
/// Name: ratified 2026-11-20 (UTC). The method and its object, the shape [`split_memory_region`] has.
pub fn memory_region_usage(memory_region_slot: u64, record: u64) -> i64 {
    // SAFETY: `svc`/`ecall`. USAGE reads one record and writes nothing of the caller's; the kernel
    // validates `ENUMERATE` and the record before it answers.
    unsafe { trap_call(typed::memory_region::usage(memory_region_slot, record))[0] as i64 }
}

/// `MAP` (untyped): retype one page out of the untyped in `memory_region_slot` and map it,
/// writable, at `virtual_address` in the caller's own address space, in one step. `0` on success; a negative
/// `moho_abi::Error` (`OutOfMemory` when the untyped is exhausted).
///
/// The one-step twin of [`retype_into_page_frame`] followed by [`map_page_frame`]: this never produces
/// a `PageFrame` capability the caller can delegate or revoke, it just spends the untyped's budget
/// directly on a mapped page.
///
/// Name: ratified 2026-10-08 (calef, #1842, milestone 139 (drive the unsafe count down) round 9's
/// question 2). His words: "Ratify `split_memory_region`, `destroy_memory_region`,
/// `map_memory_region_page`, `revoke_page_frame`, ..." Refused `map_region_page` (round 7's
/// provisional name).
pub fn map_memory_region_page(memory_region_slot: u64, virtual_address: u64) -> i64 {
    // SAFETY: `svc`/`ecall`. The kernel validates the untyped, the address, and maps a fresh page
    // from its own budget.
    unsafe {
        trap_call(typed::memory_region::map(
            memory_region_slot,
            virtual_address,
        ))[0] as i64
    }
}

/// `REVOKE` the `PageFrame` in `frame_slot`: un-share it (or, on a device capability, take it back;
/// see `moho_abi::page_frame::REVOKE`'s own doc for the asymmetry). `0` on success; a negative
/// `moho_abi::Error` (`NotPermitted` without `GRANT`).
///
/// Name: ratified 2026-10-08 (calef, #1842, milestone 139 (drive the unsafe count down) round 9's
/// question 2). His words: "Ratify `split_memory_region`, `destroy_memory_region`,
/// `map_memory_region_page`, `revoke_page_frame`, ..." Refused `revoke_frame` (round 7's
/// provisional name; the kernel already spells the object `page_frame`).
pub fn revoke_page_frame(frame_slot: u64) -> i64 {
    // SAFETY: `svc`/`ecall`. The kernel validates `GRANT` before it unmaps and deletes every
    // capability to the page.
    unsafe { trap_call(typed::page_frame::revoke(frame_slot))[0] as i64 }
}

/// `MAP_INTO`: map the frame in `frame_slot` into the address space named by `virtual_address_space_slot`, at
/// `virtual_address`, per `mode` (`moho_abi::address_space::MAP_READ_ONLY`/`MAP_READ_WRITE`/`MAP_CODE`). `0` on success; a negative
/// `moho_abi::Error`.
///
/// Safe for the reason [`map_page_frame`] is: the kernel checks the address-space capability's
/// `WRITE`, the frame's rights against `mode`, and the address before it touches a page table. A
/// caller racing what this changes has a bug in its own code, not a Rust-safety obligation. See
/// `docs/design.md`.
///
/// Name: ratified 2026-10-08 (calef, #1842, milestone 139 (drive the unsafe count down) round 9's
/// question 2). His words: "Ratify `retype_kernel_object` `address_space_layout_into`." Refused
/// `map_into` (round 7's provisional name; the method is `address_space::MAP_INTO`).
pub fn address_space_layout_into(
    virtual_address_space_slot: u64,
    virtual_address: u64,
    frame_slot: u64,
    mode: u64,
) -> i64 {
    // SAFETY: `svc`/`ecall`. The kernel validates the address-space capability, the frame's rights
    // against `mode`, and the address before it touches a page table.
    unsafe {
        trap_call(typed::address_space::map_into(
            virtual_address_space_slot,
            virtual_address,
            frame_slot,
            mode,
        ))[0] as i64
    }
}

/// `CAPABILITY_INSERT`: copy the capability in the caller's `capability_slot`, narrowed to `rights`, into the
/// child TCB's capability table. `target = 0` places it in the first free slot; `target = n` places
/// it in slot `n - 1` (see `moho_abi::thread_control_block::CAPABILITY_INSERT`'s own doc for the supervision-slot
/// use of an explicit target). Returns the slot it landed in, or a negative `moho_abi::Error`.
///
/// Name: ratified 2026-10-08 (calef, #1842, milestone 139 (drive the unsafe count down) round 9's
/// question 2). His words: "Ratify ... `thread_control_block_capability_insert`,
/// `thread_control_block_configure`, `thread_control_block_start`, ..." Refused `tcb_cap_insert`
/// (round 7's provisional name; §154 (the acronym test is whether the phrase is spoken) spells out
/// thread control block, and capability is spelled out).
pub fn thread_control_block_capability_insert(
    thread_control_block_slot: u64,
    capability_slot: u64,
    rights: u64,
    target: u64,
) -> i64 {
    // SAFETY: `svc`/`ecall`. The kernel validates `WRITE` on the TCB and `GRANT` on the inserted
    // capability before it copies anything.
    unsafe {
        trap_call(typed::thread_control_block::capability_insert(
            thread_control_block_slot,
            capability_slot,
            rights,
            target,
        ))[0] as i64
    }
}

/// `CONFIGURE`: bind the address space in `virtual_address_space_slot` to the (embryo) TCB in `thread_control_block_slot`, and set
/// where EL0 execution begins (`entry`) and on what user stack (`user_stack_pointer`). The capability in
/// `virtual_address_space_slot` is consumed; the space keeps its name, so a copy made first still names it while
/// the thread runs. On success, the address of the child's current-CPU page, which lets several
/// threads share one address space, never negative; a negative `moho_abi::Error` otherwise, `WrongObject` for a space already
/// bound.
///
/// Name: ratified 2026-10-08 (calef, #1842, milestone 139 (drive the unsafe count down) round 9's
/// question 2). His words: "Ratify ... `thread_control_block_capability_insert`,
/// `thread_control_block_configure`, `thread_control_block_start`, ..." Refused `tcb_configure`
/// (round 7's provisional name).
pub fn thread_control_block_configure(
    thread_control_block_slot: u64,
    entry: u64,
    user_stack_pointer: u64,
    virtual_address_space_slot: u64,
) -> i64 {
    // SAFETY: `svc`/`ecall`. The kernel validates `WRITE` on both capabilities before it binds them.
    unsafe {
        trap_call(typed::thread_control_block::configure(
            thread_control_block_slot,
            entry,
            user_stack_pointer,
            virtual_address_space_slot,
            0,
        ))[0] as i64
    }
}

/// `START`: make the TCB in `thread_control_block_slot` runnable. `argument_zero`, `argument_one` and `argument_two` seed the child's first three argument registers (`x0`/`x1`/`x2` on
/// aarch64, `a0`/`a1`/`a2` on RISC-V) on its first instruction, the kernel-side spelling of "this
/// thread's input"; the kernel forwards all three unconditionally. Refuses a half-built thread (no bound address space, or no entry). `0` on
/// success; a negative `moho_abi::Error`.
///
/// Name: ratified 2026-10-08 (calef, #1842, milestone 139 (drive the unsafe count down) round 9's
/// question 2). His words: "Ratify ... `thread_control_block_capability_insert`,
/// `thread_control_block_configure`, `thread_control_block_start`, ..." Refused `tcb_start` (round
/// 7's provisional name).
pub fn thread_control_block_start(
    thread_control_block_slot: u64,
    argument_zero: u64,
    argument_one: u64,
    argument_two: u64,
) -> i64 {
    // SAFETY: `svc`/`ecall`. The kernel validates `WRITE` and that the TCB is whole before it joins
    // the run queue.
    unsafe {
        trap_call(typed::thread_control_block::start(
            thread_control_block_slot,
            argument_zero,
            argument_one,
            argument_two,
        ))[0] as i64
    }
}

/// `SUSPEND`: stop the thread whose TCB is in `thread_control_block_slot` until a [`thread_control_block_resume`],
/// keeping its registers. Silent: nobody is told. When this returns 0 the thread
/// is off every core. `0` on success; a negative `moho_abi::Error`.
///
/// Name: ratified 2026-11-20 (UTC); first minted (lane/48-tcb-suspend-resume, 2026-10-18 UTC).
pub fn thread_control_block_suspend(thread_control_block_slot: u64) -> i64 {
    // SAFETY: `svc`/`ecall`. The kernel validates `WRITE`; nothing in this address space changes.
    unsafe {
        trap_call(typed::thread_control_block::suspend(
            thread_control_block_slot,
        ))[0] as i64
    }
}

/// `RESUME`: let the thread whose TCB is in `thread_control_block_slot` run on from where it was suspended
/// `0` on success, also when it was not suspended; a negative `moho_abi::Error`.
///
/// Name: ratified 2026-11-20 (UTC); first minted (lane/48-tcb-suspend-resume, 2026-10-18 UTC).
pub fn thread_control_block_resume(thread_control_block_slot: u64) -> i64 {
    // SAFETY: as `thread_control_block_suspend`.
    unsafe {
        trap_call(typed::thread_control_block::resume(
            thread_control_block_slot,
        ))[0] as i64
    }
}

/// Drop the capability in `slot` from this thread's capability table (`SYSTEM_CALL_CAPABILITY_DELETE`). Deleting an empty
/// slot is a no-op. A program that retypes many objects (a loader, a spawner) frees each slot as
/// soon as it is done with it, so its fixed capability table does not fill. Through the six-word
/// trap for [`yield_now`](crate::yield_now)'s reason: the kernel answers in the register `slot` went in on.
///
/// Name: ratified 2026-11-20 (UTC); first minted, minted 2026-10-27 (UTC) by lane capability-sweep-6. Spelled out from
/// `cap_delete` under the capability ruling of 2026-10-06 (UTC).
pub fn capability_delete(slot: u64) {
    // SAFETY: SYSTEM_CALL_CAPABILITY_DELETE frees a slot in the caller's own capability table, nothing to clean up.
    let _ = unsafe { trap_call(typed::capability_delete(slot)) };
}
