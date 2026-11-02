#![no_std]
// `Result<_, ()>` is deliberate: every failure here is a syscall that already returned its own
// error through the ABI, so the unit means "the syscall said no". Per-crate because it is one
// decision about one calling convention. §38 (a suppression is scoped to an item and carries a
// reason)'s rule that a dead-code suppression is an item attribute, never an inner one (milestone
// 41 (dead code: triage the suppressions)), does not reach a style lint about an error type.
#![allow(clippy::result_unit_err)]
//! **The process builder: the runtime half of supervision.** The tree's only userspace ELF
//! loader, and the scratch window a builder fills child pages through. `spawner`,
//! `system_initializer` and the fixtures' builders call these; the protocol words they exchange
//! stay in `supervision_contract`, which this crate depends on and never the other way round
//! (DECISIONS §235 (the OS is built and updated from packages, and its source divides by purpose) P3: a contract never links the runtime).
//!
//! The examples run under `cargo test --doc -p process_builder` on an aarch64 host.
//!
//! Name: provisional, minted 2026-10-29 (UTC) by lane
//! `milestone/607-item-3-contracts-drop-the-runtime`. It splits `supervision_contract`, whose own
//! name was ratified 2026-10-28 and whose doc named this split as open
//! (design/naming/convention-rulings.md). calef names it.

use supervision_contract::{
    CHILD_STACK_VA, CHILDS_OWN_PROCESS, CHILDS_OWN_SPACE, CHILDS_OWN_THREAD, Child, ChildEndowment,
    Retention,
};
use user_mode_runtime::{
    address_space_map_into, capability_delete, destroy_memory_region, invoke,
    retype_into_kernel_object, retype_into_page_frame, send_capability, split_memory_region,
    thread_control_block_capability_insert, thread_control_block_configure,
    thread_control_block_start,
};

// ===========================================================================================
// The loader.
// ===========================================================================================

/// Name: provisional, minted 2026-10-29 (UTC) by lane `milestone/607-item-3-contracts-drop-the-runtime`; it moved here from
/// `supervision_contract` with the split, and calef names it.
/// The page size the loader maps in.
pub const PAGE: u64 = 4096;

/// **The builder's scratch window**: where a builder maps each frame it fills for a child, and each
/// frame of somebody else's it reads (the image request of DECISIONS §219 (how the shell names an
/// installed program to the spawner)). 64 MiB, 16,384 pages, from the bottom of the map's runtime
/// windows.
///
/// [`map_scratch`] wraps around it, since destroying a child's region revokes the builder's
/// mappings of it (milestone 604 (provisional), DECISIONS §13 (frame revocation)). The progenitor
/// checks its size against its own budgets at compile time. See notes/progenitor-and-loading.md.
///
/// Name: provisional (milestone 604, 2026-09-26).
pub const SCRATCH_WINDOW: address_space_map::Band = address_space_map::Band {
    start: address_space_map::runtime_window(0x1000_0000),
    end: address_space_map::runtime_window(0x1400_0000),
};

/// Name: provisional, minted 2026-10-29 (UTC) by lane `milestone/607-item-3-contracts-drop-the-runtime`; it moved here from
/// `supervision_contract` with the split, and calef names it.
/// [`SCRATCH_WINDOW`] in pages. Name: provisional (milestone 604).
pub const SCRATCH_WINDOW_PAGES: u64 = SCRATCH_WINDOW.bytes() / PAGE;

/// Name: provisional, minted 2026-10-29 (UTC) by lane `milestone/607-item-3-contracts-drop-the-runtime`; it moved here from
/// `supervision_contract` with the split, and calef names it.
/// **The most last-level page tables [`SCRATCH_WINDOW`] can cost a builder**: one per 512 pages,
/// which is a 4 KiB table of 8-byte entries on all three architectures. A builder pays them out of
/// the budget it passes as `own_ut`, once, as the cursor first reaches each 2 MiB; after the first
/// lap every table already exists. The levels above are one or two tables shared with everything
/// else the builder maps under 1 GiB. Name: provisional (milestone 604).
pub const SCRATCH_TABLE_PAGES: u64 = SCRATCH_WINDOW_PAGES / 512;

const _: () = assert!(
    SCRATCH_WINDOW.start.is_multiple_of(512 * PAGE)
        && SCRATCH_WINDOW.end.is_multiple_of(512 * PAGE)
);

/// The next page [`map_scratch`] tries. Starts at the window's base and wraps.
static SCRATCH_NEXT: core::sync::atomic::AtomicU64 =
    core::sync::atomic::AtomicU64::new(SCRATCH_WINDOW.start);

/// **Map `frame` at the next free page of [`SCRATCH_WINDOW`]**, with page tables from `own_ut`,
/// read/write if `writable`; `Err` after a full lap. See notes/progenitor-and-loading.md.
///
/// # BUGS
/// - One thread per builder: the cursor takes no lock, so two building threads waste a probe.
/// - `BadPointer` means "taken" only because every address tried is a valid window page.
/// - Without [`give_up_scratch_through`] (§249 (a running address space stays nameable)), live
///   children's pages stay mapped read/write in the builder.
/// - The progenitor's use is computed, not measured (milestone 604's block has the arithmetic).
///
/// Name: provisional (milestone 604). It replaces `scratch_pages`, which handed out a contiguous
/// run that was never reused.
pub fn map_scratch(frame: u64, writable: bool, own_ut: u64) -> Result<u64, ()> {
    use core::sync::atomic::Ordering::Relaxed;
    let mut va = SCRATCH_NEXT.load(Relaxed);
    for _ in 0..SCRATCH_WINDOW_PAGES {
        let next = if va + PAGE >= SCRATCH_WINDOW.end {
            SCRATCH_WINDOW.start
        } else {
            va + PAGE
        };
        // SAFETY: `invoke` is the syscall; the kernel validates the capability, the rights and the
        // address, and maps nothing on any refusal.
        let r = unsafe { invoke(frame, abi::page_frame::MAP, va, u64::from(writable), own_ut) };
        if r == 0 {
            SCRATCH_NEXT.store(next, Relaxed);
            return Ok(va);
        }
        if r != abi::Error::BadPointer as i64 {
            SCRATCH_NEXT.store(va, Relaxed);
            return Err(());
        }
        va = next;
    }
    SCRATCH_NEXT.store(va, Relaxed);
    Err(())
}

/// **The builder's own address space**, or [`NO_OWN_SPACE`] when it was never named (§249 (a running
/// address space stays nameable); milestone 95 (an unmap primitive)). Set once, by
/// [`give_up_scratch_through`], and read by [`give_up_own_page`].
static OWN_SPACE: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(NO_OWN_SPACE);

/// [`OWN_SPACE`]'s "never named". `u64::MAX` is no capability slot.
const NO_OWN_SPACE: u64 = u64::MAX;

/// **Name the slot holding a `WRITE` capability to this builder's own address space**, so the
/// scratch window is given back page by page rather than held for the life of the child (milestone
/// 95 (an unmap primitive), DECISIONS §162 (whether a holder can give up a mapping), §249 (a running
/// address space stays nameable)).
///
/// The progenitor is the one caller: the kernel grants it a capability to its own space at slot 28
/// on every boot. A builder that never calls this keeps today's behaviour, which is a scratch page
/// that stays mapped until the child's region is destroyed and revokes it (DECISIONS §13).
///
/// Name: provisional (milestone 95's §249 lane, 2026-10-05 UTC).
pub fn give_up_scratch_through(own_space: u64) {
    OWN_SPACE.store(own_space, core::sync::atomic::Ordering::Relaxed);
}

/// **Give up one page this process mapped for itself**: `AddressSpace::UNMAP` of `va` in its own
/// space, through the capability [`give_up_scratch_through`] named: a filled scratch page, or
/// another window a builder opened onto memory it hands on (the progenitor's view of the shell's
/// output page, and of two devices' DMA pages). `true` if the page is gone, `false` if no
/// capability was named or the kernel refused; a scratch page is then taken back when the child's
/// region is.
///
/// Afterward a write to `va` faults (milestone 95's negative control, `system_tests`'
/// `running_space_tests`), and [`map_scratch`]'s probe finds the address free.
///
/// Name: provisional (milestone 95's §249 lane, 2026-10-05 UTC).
pub fn give_up_own_page(va: u64) -> bool {
    let own = OWN_SPACE.load(core::sync::atomic::Ordering::Relaxed);
    if own == NO_OWN_SPACE {
        return false;
    }
    // SAFETY: `invoke` is the syscall; the kernel validates the capability, the right and the
    // address, and changes nothing on a refusal. The caller is done with every byte at `va`.
    unsafe { invoke(own, abi::address_space::UNMAP, va, 0, 0) == 0 }
}

/// Name: provisional, minted 2026-10-29 (UTC) by lane `milestone/607-item-3-contracts-drop-the-runtime`; it moved here from
/// `supervision_contract` with the split, and calef names it.
/// Build a child from `elf` and configure it, ready for [`start_child`]. The whole job in one call,
/// which is what every caller but the hot-swap operator wants.
///
/// `own_ut` pays for **our** scratch mappings (they are ours, and a child's region must not have our
/// page tables freed under it when the child is reaped). `build_ut` is what the child itself is made
/// of: its address space, frames, stack, and TCB. Passing a per-child region as `build_ut` is what
/// makes a single `DESTROY` reap the whole instance.
///
/// **`build_ut` must be budget the caller owns** (DECISIONS §303 (the retention declaration says
/// nothing about the region)): a region it created, a `SPLIT` child of its own region, or a root
/// handed to it as its whole budget. Building from a retained, delegated region is forbidden,
/// because the reap of §40 (a supervisor's death is its subtree's death) holds only when a child is
/// built from its supervisor's own region. Nothing enforces this yet: the kernel check that
/// compares `build_ut`'s recorded parent with a presented owning region is the work of milestone 356
/// (retention declares the thread capability and says nothing about the region).
///
/// # Examples
///
/// ```no_run
/// # use supervision_contract::{ChildEndowment, Retention};
/// # use process_builder::{build_child, start_child};
/// # fn demo(own: u64, per_child: u64, elf: &elf::Elf) -> Result<(), ()> {
/// let endow = ChildEndowment { fault: Some(7), ..ChildEndowment::new(Retention::Nothing) };
///
/// // `own` pays for OUR scratch mappings; `per_child` is what the child is made of. Passing a
/// // per-child region as the second argument is what makes a single `DESTROY` reap the whole
/// // instance, and passing our own would free our page tables under the child.
/// let child = build_child(own, per_child, elf, &endow)?;
/// start_child(child, 0, 0, 0); // and the tcb capability goes back here, as declared
/// # Ok(())
/// # }
/// ```
pub fn build_child(
    own_ut: u64,
    build_ut: u64,
    elf: &elf::Elf,
    endow: &ChildEndowment,
) -> Result<Child, ()> {
    let (child, aspace) = build_child_space(own_ut, build_ut, elf, endow)?;
    let process = endow
        .placed
        .iter()
        .find(|&&(_, ours, _)| ours == CHILDS_OWN_PROCESS);
    let configured = match process {
        None => configure_child(child.tcb, aspace, elf.entry()),
        Some(&(slot, _, rights)) => {
            configure_child_as_process(build_ut, child.tcb, aspace, elf.entry(), slot, rights)
        }
    };
    if configured.is_err() {
        // `CONFIGURE` consumes the address space only when it succeeds, so both are still ours.
        capability_delete(child.tcb);
        capability_delete(aspace);
        return Err(());
    }
    Ok(child)
}

/// Name: provisional, minted 2026-10-29 (UTC) by lane `milestone/607-item-3-contracts-drop-the-runtime`; it moved here from
/// `supervision_contract` with the split, and calef names it.
/// Everything [`build_child`] does **except** the final `CONFIGURE`: lay each segment W^X at the VA
/// it names, map a stack, copy the blobs in, map `maps`, retype a TCB, insert the endowment.
/// Returns `(child, address space)`, both still held by us; the [`Child`] carries the retention.
///
/// Split out for milestone 23 (a capability-routed component OS with live replacement)'s live
/// replacement (DECISIONS §41 (the endpoint is the broker)), which maps a device in before
/// `CONFIGURE` consumes the address space (notes/live-replacement.md). A failure gives back every
/// capability slot it took (milestone 757 (a test kernel fails a process on its Nth retype),
/// provisional); the memory is the caller's, given back by destroying `build_ut`.
pub fn build_child_space(
    own_ut: u64,
    build_ut: u64,
    elf: &elf::Elf,
    endow: &ChildEndowment,
) -> Result<(Child, u64), ()> {
    // The map's image band, checked before anything is retyped, the same check `kernel::user::load`
    // makes. This loader has no channel for the reason (every refusal is `Err(())`, see BUGS above),
    // so a too-large image still reads as "could not build"; what the check buys is that it fails
    // here, cleanly, rather than half-built at the first stack page it would have collided with.
    let (mut lo, mut hi) = (u64::MAX, 0);
    for seg in elf.segments() {
        let (start, end) = seg.page_range(PAGE);
        lo = lo.min(start);
        hi = hi.max(end);
    }
    if address_space_map::check_image(lo, hi).is_err()
        || endow.stack_pages > address_space_map::MAXIMUM_STACK_PAGES
    {
        return Err(());
    }

    let aspace = retype_obj_from(build_ut, abi::objtype::ADDRESS_SPACE)?;
    match lay_out_child(own_ut, build_ut, elf, endow, aspace) {
        Ok(tcb) => Ok((
            Child {
                tcb,
                retention: endow.retention,
            },
            aspace,
        )),
        Err(()) => {
            capability_delete(aspace);
            Err(())
        }
    }
}

/// [`build_child_space`] after the address space exists: the segments, the stack, the blobs, the
/// maps, then the TCB and its endowment. Returns the TCB's slot. On `Err` it holds nothing it made;
/// the address space is its caller's to delete.
fn lay_out_child(
    own_ut: u64,
    build_ut: u64,
    elf: &elf::Elf,
    endow: &ChildEndowment,
    aspace: u64,
) -> Result<u64, ()> {
    for seg in elf.segments() {
        let mode = if seg.is_executable() {
            abi::address_space::MAP_CODE
        } else if seg.is_writable() {
            abi::address_space::MAP_RW
        } else {
            abi::address_space::MAP_RO
        };
        let (start, end) = seg.page_range(PAGE);
        let mut va = start;
        while va < end {
            let file_lo = seg.vaddr;
            let file_hi = seg.vaddr + seg.data.len() as u64;
            let lo = va.max(file_lo);
            let hi = (va + PAGE).min(file_hi);
            let src = if lo < hi {
                Some((
                    (lo - va) as usize,
                    &seg.data[(lo - file_lo) as usize..(hi - file_lo) as usize],
                ))
            } else {
                None
            };
            fill_and_map(own_ut, build_ut, aspace, va, src, mode)?;
            va += PAGE;
        }
    }

    for k in 0..endow.stack_pages {
        let stack_frame = retype_page_frame_from(build_ut)?;
        let va = CHILD_STACK_VA - k * PAGE;
        if address_space_map_into(aspace, va, stack_frame, abi::address_space::MAP_RW) != 0 {
            capability_delete(stack_frame);
            return Err(());
        }
        capability_delete(stack_frame);
    }

    // The timebase page: the builder's own rate, written into a fresh page from `build_ut`, so
    // every child of this, the only userspace loader, inherits it. A builder that does not know
    // the rate writes a zeroed page and the child refuses (calef, 2026-09-21: only accurate
    // numbers). See notes/progenitor-and-loading.md.
    #[cfg(any(target_arch = "x86_64", target_arch = "riscv64"))]
    {
        let page = user_mode_runtime::cntfrq_checked().map(counter_frequency_contract::build_page);
        fill_and_map(
            own_ut,
            build_ut,
            aspace,
            counter_frequency_contract::PAGE_VIRTUAL_ADDRESS,
            page.as_ref().map(|bytes| (0usize, &bytes[..])),
            abi::address_space::MAP_RO,
        )?;
    }

    // The blobs: fresh read-only pages carrying bytes we chose. Read-only because a program image
    // is data the child reads, and a child that could rewrite the image it was handed could hand a
    // different one on.
    for &(base, bytes) in endow.blobs {
        let mut off = 0usize;
        while off < bytes.len() {
            let n = core::cmp::min(PAGE as usize, bytes.len() - off);
            fill_and_map(
                own_ut,
                build_ut,
                aspace,
                base + off as u64,
                Some((0, &bytes[off..off + n])),
                abi::address_space::MAP_RO,
            )?;
            off += n;
        }
    }

    for &(va, our_slot, mode) in endow.maps {
        if address_space_map_into(aspace, va, our_slot, mode) != 0 {
            return Err(());
        }
    }

    let tcb = retype_obj_from(build_ut, abi::objtype::THREAD_CONTROL_BLOCK)?;
    if endow_child(tcb, endow, aspace).is_err() {
        capability_delete(tcb);
        return Err(());
    }
    Ok(tcb)
}

/// Insert the endowment's capabilities into the embryo `tcb`: [`ChildEndowment::capabilities`] first-free,
/// then the placed ones (one of which may be the child's own address space, [`CHILDS_OWN_SPACE`]),
/// then the fault endpoint in its reserved slot.
fn endow_child(tcb: u64, endow: &ChildEndowment, aspace: u64) -> Result<(), ()> {
    for &(our_slot, rights) in endow.capabilities {
        if thread_control_block_capability_insert(tcb, our_slot, rights, 0) < 0 {
            return Err(());
        }
    }
    for &(child_slot, our_slot, rights) in endow.placed {
        let source = if our_slot == CHILDS_OWN_SPACE {
            // `WRITE` to shape it, or `READ` alone for the futex waits a `std` program's locks make
            // on it (milestone 812); never `GRANT`.
            if rights != abi::rights::WRITE && rights != abi::rights::READ {
                return Err(());
            }
            aspace
        } else if our_slot == CHILDS_OWN_THREAD {
            if rights != abi::rights::WRITE {
                return Err(());
            }
            tcb
        } else if our_slot == CHILDS_OWN_PROCESS {
            continue; // placed once the process exists, by `configure_child_as_process`
        } else {
            our_slot
        };
        // `target = n` lands the capability in slot `n - 1`; 0 would mean "first free", which is the
        // behaviour this call exists to avoid.
        if thread_control_block_capability_insert(tcb, source, rights, child_slot + 1) < 0 {
            return Err(());
        }
    }
    if let Some(fault) = endow.fault {
        // The spawn-slot convention: target slot `n + 1` means "slot n", so the supervision endpoint
        // lands in the reserved last slot rather than wherever first-free fell.
        let target = abi::fault::FAULT_EP_SLOT + 1;
        if thread_control_block_capability_insert(tcb, fault, abi::rights::READ, target) < 0 {
            return Err(());
        }
    }
    Ok(())
}

/// **[`configure_child`] for a child that is a process** (milestone 812): the process is made in
/// `build_ut` around `aspace` (which that consumes, as `CONFIGURE` would have), its capability goes to
/// the child at `slot` with `rights` (exactly `BIND`), and the child joins it. The builder keeps no
/// capability to the process.
fn configure_child_as_process(
    build_ut: u64,
    tcb: u64,
    aspace: u64,
    entry: u64,
    slot: u64,
    rights: u64,
) -> Result<(), ()> {
    if rights != abi::rights::BIND {
        return Err(());
    }
    // SAFETY: `invoke` is the syscall; the kernel validates the region, the objtype and the
    // address space before it builds anything. Raw because a process takes the space as its second
    // argument, which `retype_into_kernel_object` does not pass.
    let process = unsafe {
        invoke(
            build_ut,
            abi::memory_region::RETYPE_OBJ,
            abi::objtype::PROCESS,
            aspace,
            0,
        )
    };
    if process < 0 {
        return Err(());
    }
    let process = process as u64;
    let joined = thread_control_block_capability_insert(tcb, process, rights, slot + 1) >= 0
        && thread_control_block_configure(tcb, entry, CHILD_STACK_VA + PAGE, process) >= 0;
    capability_delete(process);
    if joined { Ok(()) } else { Err(()) }
}

/// Name: provisional, minted 2026-10-29 (UTC) by lane `milestone/607-item-3-contracts-drop-the-runtime`; it moved here from
/// `supervision_contract` with the split, and calef names it.
/// Bind the address space and set the entry point: the last step before [`start_child`]. The `aspace`
/// capability is **consumed** by the kernel here, so this is the moment after which the builder can
/// no longer shape the child's memory.
pub fn configure_child(tcb: u64, aspace: u64, entry: u64) -> Result<(), ()> {
    if thread_control_block_configure(tcb, entry, CHILD_STACK_VA + PAGE, aspace) < 0 {
        // A negative answer is a refusal. Since milestone 812 (`std::thread::spawn` runs real
        // threads in one address space) a success answers the child's current-CPU page address
        // rather than zero.
        return Err(());
    }
    Ok(())
}

/// Retype one page out of `build_ut`, fill it (zeroed, then `src` bytes at an offset) through our own
/// scratch window, and map it into `aspace` at `va`. The one place a page's contents are written, so
/// segments and blobs cannot drift apart.
fn fill_and_map(
    own_ut: u64,
    build_ut: u64,
    aspace: u64,
    va: u64,
    src: Option<(usize, &[u8])>,
    mode: u64,
) -> Result<(), ()> {
    let frame = retype_page_frame_from(build_ut)?;
    let Ok(scratch) = map_scratch(frame, true, own_ut) else {
        capability_delete(frame);
        return Err(());
    };
    // SAFETY: `scratch` is a page we just mapped read/write in our own address space.
    let dst = unsafe { core::slice::from_raw_parts_mut(scratch as *mut u8, PAGE as usize) };
    dst.fill(0);
    if let Some((at, bytes)) = src {
        dst[at..at + bytes.len()].copy_from_slice(bytes);
    }
    let mapped = address_space_map_into(aspace, va, frame, mode) == 0;
    capability_delete(frame);
    // The page is written and in the child (or the build failed): either way this builder is done
    // with its own window onto it. Given up here, one page at a time, rather than after the child
    // is built, so the window is never wider than the page being filled.
    give_up_own_page(scratch);
    if mapped { Ok(()) } else { Err(()) }
}

/// Name: provisional, minted 2026-10-29 (UTC) by lane `milestone/607-item-3-contracts-drop-the-runtime`; it moved here from
/// `supervision_contract` with the split, and calef names it.
/// Retype one page of `ut` into a kernel object of `objtype` (`abi::objtype`), returning the
/// capability slot it landed in.
pub fn retype_obj_from(ut: u64, objtype: u64) -> Result<u64, ()> {
    let r = retype_into_kernel_object(ut, objtype);
    if r < 0 { Err(()) } else { Ok(r as u64) }
}

/// Name: provisional, minted 2026-10-29 (UTC) by lane `milestone/607-item-3-contracts-drop-the-runtime`; it moved here from
/// `supervision_contract` with the split, and calef names it.
/// Retype one page of `ut` into a plain `PageFrame` capability, unmapped, returning the slot it
/// landed in.
pub fn retype_page_frame_from(ut: u64) -> Result<u64, ()> {
    let r = retype_into_page_frame(ut);
    if r < 0 { Err(()) } else { Ok(r as u64) }
}

/// Name: provisional, minted 2026-10-29 (UTC) by lane `milestone/607-item-3-contracts-drop-the-runtime`; it moved here from
/// `supervision_contract` with the split, and calef names it.
/// Carve `pages` off `ut` into a new child untyped we hold in full: the region a single `DESTROY`
/// reclaims. `Err` carries the negated error code, which is how the dropped-authority proof reports
/// *why* a retype from a deleted budget failed.
pub fn memory_region_split(ut: u64, pages: u64) -> Result<u64, i64> {
    let r = split_memory_region(ut, pages);
    if r < 0 { Err(r) } else { Ok(r as u64) }
}

/// Name: provisional, minted 2026-10-29 (UTC) by lane `milestone/607-item-3-contracts-drop-the-runtime`; it moved here from
/// `supervision_contract` with the split, and calef names it.
/// §16 object revocation: reclaim a region and every object retyped from it, by its **owner**. The
/// stronger of the two reaps, because `WRITE` on a region is also what builds a process out of it,
/// and the only one that can tear down a *live* thread (§16's amendment arms the kill). A supervisor
/// collecting a dead child wants `user_mode_runtime::reap` instead (§32).
pub fn memory_region_destroy(ut: u64) -> bool {
    destroy_memory_region(ut) == 0
}

/// Name: provisional, minted 2026-10-29 (UTC) by lane `milestone/607-item-3-contracts-drop-the-runtime`; it moved here from
/// `supervision_contract` with the split, and calef names it.
/// **Make `child` runnable and then keep or drop its capability as its endowment declared**, with
/// `a0`, `a1`, `a2` in its entry registers. `false` if the kernel refused (no bound address space
/// or no entry point). The only way to `START` a child here (DECISIONS §142); the disposal holds
/// whether or not `START` succeeded.
///
/// # Examples
///
/// ```no_run
/// # use supervision_contract::{ChildEndowment, Retention};
/// # use process_builder::{build_child, start_child};
/// # fn demo(own: u64, per_child: u64, elf: &elf::Elf) -> Result<(), ()> {
/// let endow = ChildEndowment { fault: Some(7), ..ChildEndowment::new(Retention::Nothing) };
/// let child = build_child(own, per_child, elf, &endow)?;
/// // The capability is gone after this line, because the endowment said so.
/// start_child(child, 0, 0, 0);
/// # Ok(())
/// # }
/// ```
///
/// # BUGS
///
/// - [`Retention::ThreadControlBlock`]'s arm has no caller and therefore no test: every spawn site
///   declares [`Retention::Nothing`] or, in `system_initializer`'s supervised-job spawn,
///   [`Retention::Requester`] (§309). The first site that declares it exercises it first.
pub fn start_child(child: Child, a0: u64, a1: u64, a2: u64) -> bool {
    let started = thread_control_block_start(child.tcb, a0, a1, a2) == 0;
    match child.retention {
        Retention::Nothing => forget_thread_control_block(child.tcb),
        // The reason is the caller's, recorded at the call site; nothing here reads it.
        Retention::ThreadControlBlock { reason: _ } | Retention::Requester { reason: _ } => {}
    }
    started
}

/// **Hand a started child's `ThreadControlBlock` to whoever asked for it**, the second half of
/// [`Retention::Requester`] (DECISIONS §309 (a supervised job's `ThreadControlBlock` goes to the
/// shell that asked for it)). The capability goes on `endpoint` narrowed to `WRITE`, which is what
/// `SUSPEND` and `RESUME` need, and without `GRANT`, so the requester can pause the child and
/// cannot pass that on. The builder's own slot is deleted whether or not the send succeeded: a
/// send fails only when nobody is left on `endpoint` to want the capability.
///
/// Name: provisional, minted 2026-10-18 (UTC) by lane/48-ctrl-z.
pub fn hand_to_requester(tcb: u64, endpoint: u64) {
    let _ = send_capability(endpoint, tcb, abi::rights::WRITE, 0);
    forget_thread_control_block(tcb);
}

/// **Delete the builder's slot for a child's TCB**: [`start_child`]'s [`Retention::Nothing`],
/// [`hand_to_requester`] after its send, and a builder whose child never started, which owes its
/// requester nothing.
///
/// Name: provisional, minted 2026-10-18 (UTC) by lane/48-ctrl-z.
pub fn forget_thread_control_block(tcb: u64) {
    capability_delete(tcb);
}

/// Name: provisional, minted 2026-10-29 (UTC) by lane `milestone/607-item-3-contracts-drop-the-runtime`; it moved here from
/// `supervision_contract` with the split, and calef names it.
/// Trap. A half-built system is not worth limping along, and a fault is legible: the kernel prints
/// the pc and the process dies where the mistake was.
///
/// Named rather than a bare [`user_mode_runtime::trap`] because a caller here means a
/// supervision-protocol step failed, and limping on would build half a system.
pub fn fail() -> ! {
    user_mode_runtime::trap()
}
