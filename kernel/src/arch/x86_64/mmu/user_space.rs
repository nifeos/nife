//! **The user address spaces** (milestone 161 (the x86_64 kernel port), roadmap item 3): `CR3`, the
//! kernel half each process root carries, and the per-space map, unmap and translate.
//!
//! Cut out of `mmu.rs` unchanged by lane `maintainer/last-three-over-2000`, which left the
//! parent an index (§282 (when a file is split, the parent becomes an index)) and under §266 (a
//! Rust source file stays under 2,000 lines). `super::instructions` names the `x86_64` item the
//! index imports, as in `mmu.rs`.
//!
//! Name: provisional (lane `maintainer/last-three-over-2000`, 2026-10-12 UTC), for the module and
//! the file.

use super::*;

// ---------------------------------------------------------------------------------------------
// The user address spaces (milestone 161, roadmap item 3).
//
// x86 has RISC-V's shape, not aarch64's: `CR3` names the whole address space, as `satp` does, so
// a process's root must carry the kernel's high half ([`share_kernel_half`]). PCID, the ASID
// equivalent, is off (`CR4.PCIDE` clear), so no tag can be written into `CR3` and every `mov cr3`
// discards the whole non-global TLB; [`ttbr0_value`] and [`flush_asid`] say so. See
// notes/x86-port/kernel-page-tables.md.
// ---------------------------------------------------------------------------------------------

/// The raw `CR3`, flag and PCID bits included. [`current_root`] is this with the address masked
/// out; the two differ only once `CR4.PCIDE` or the PWT/PCD bits are used, and neither is today.
/// Kept apart so [`switch_user_root`]'s early return compares what the hardware actually holds.
pub(super) fn read_cr3() -> u64 {
    super::instructions::read_cr3()
}

/// The physical root of the currently installed user address space.
///
/// On this architecture there is only one root, so this is the same register [`current_root`]
/// reads. A thread with no user space of its own is running on [`reserved_root`], whose low half is
/// empty, so "the user root" is always a real answer rather than a `None` in disguise.
pub fn current_user_root() -> u64 {
    current_root()
}

/// **Every entry a walk of user `va` from `root` reads**, for the kill report of a page fault whose
/// cause the error code cannot give (see `exceptions::report_walk`). Reads only tables in frames
/// the allocator manages, because the walk being reported may run through entries somebody wrote
/// over, and the direct map covers RAM and nothing else.
///
/// Name: provisional (lane `maintainer/pipeline-fetch-fault`, 2026-10-14 UTC).
pub fn walk_user(root: u64, va: u64) -> paging::Walk {
    // SAFETY: a walk allocates nothing, so the `|| None` allocator is never called, and it reads
    // only tables `readable` admits: frames the allocator manages, which the direct map covers.
    let mapper = unsafe { Mapper::<_, _, Ia32e>::new(root, Half::Low, || None, phys_to_ptr) };
    mapper.walk(va, |table| memory::frame_is_allocated(table).is_some())
}

/// The root a thread with no user address space runs on.
///
/// **The kernel's own root**, exactly as on RISC-V and for the same reason: one register names the
/// whole space, so there is no separate empty user table to point at. Its low half is zero
/// (`init`'s `verify` asserts it), so every user address faults, which is what a kernel thread
/// should get.
#[allow(dead_code)]
pub(crate) fn reserved_root() -> u64 {
    ttbr0_value(KERNEL_ROOT.load(Ordering::Relaxed), 0)
}

/// Compose the value `CR3` should hold for a root and an address-space id. Named for aarch64's
/// register because that is the arch contract's word.
///
/// The `asid` is dropped: with `CR4.PCIDE` clear, `CR3[11:0]` is not a tag, and bits 2:0 and 11:5
/// are reserved-zero, so `root | asid` would `#GP` on the first switch. [`flush_asid`] pays for
/// that by flushing everything.
#[allow(dead_code)]
pub(crate) fn ttbr0_value(root: u64, asid: u16) -> u64 {
    debug_assert_eq!(
        root & 0xfff,
        0,
        "a page-table root is page-aligned; CR3's low twelve bits are not part of it"
    );
    // The tag has nowhere to go while PCIDE is off; see this function's own doc comment. When it is
    // turned on, this becomes `root | (asid as u64 & 0xfff)` and `crates/address_space_identifier`'s reuse contract
    // acquires a meaning here that it does not have today.
    let _ = asid;
    root
}

/// Populate a fresh process root's **high half** with the kernel's, so one `CR3` sees both the
/// process's user pages and the whole kernel.
///
/// The kernel half is PML4 entries 256..512, where both kernel bases live, so one
/// `copy_from_slice` shares the image, the direct map and every device window. The entries point at
/// kernel intermediate tables, so a page the kernel maps later is visible in every process. Called
/// by `user::AddressSpace::new` right after it allocates a root.
#[allow(dead_code)]
pub(crate) fn share_kernel_half(root: u64) {
    let kernel_root = KERNEL_ROOT.load(Ordering::Relaxed);
    assert_ne!(
        kernel_root, 0,
        "share_kernel_half before mmu::init: there is no kernel map to share yet"
    );
    // SAFETY: both are page-aligned root tables reachable through the direct map, which `boot.s`
    // installed before any Rust ran. Only the high-half entries are written; the low half stays
    // zero for the process's own pages.
    unsafe {
        let dst = &mut (*phys_to_ptr(root)).entries;
        let src = &(*phys_to_ptr(kernel_root)).entries;
        dst[paging::ENTRIES / 2..].copy_from_slice(&src[paging::ENTRIES / 2..]);
    }
}

/// Install the address space named by `cr3`, **unless it is already installed**: with `CR4.PGE`
/// off a `mov cr3` drops every translation, the kernel's included, so a switch that changes nothing
/// would still pay a full refill. The comparison reads the register back.
///
/// # Safety
/// `cr3` must be a value [`ttbr0_value`] composed over a **live** root that carries the kernel's
/// high half, or [`reserved_root`]. Anything else unmaps the instruction after this one: a page
/// fault with tables that cannot describe the handler, then a double and a triple fault.
#[allow(dead_code)]
pub(crate) unsafe fn switch_user_root(cr3: u64) {
    if read_cr3() == cr3 {
        return;
    }
    // SAFETY: this function's own `# Safety` contract is exactly the one `install` needs; it
    // forwards, it does not weaken.
    unsafe { install(cr3) };
}

/// Read the address-space id back out of a composed [`ttbr0_value`]. The inverse of that function,
/// and it exists so a portable test can ask "which tag is this space wearing?" without knowing where
/// this ISA keeps it.
///
/// **It answers 0 for every space, and that is the truth rather than a stub.** `CR4.PCIDE` is off
/// (see [`ttbr0_value`]), so the hardware holds no tag at all: there is nothing in the register for
/// this to decode. A test that asserts two live address spaces wear *different* tags is therefore
/// asserting something this architecture does not do yet, and should say so with `skip!()` rather
/// than read a zero as agreement. The aarch64 twin shifts bits 63:48 out and the RISC-V one
/// `satp[59:44]`.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
pub fn asid_of(cr3: u64) -> u16 {
    let _ = cr3;
    0
}

/// **Test-only: let ring 0 load and store through pages marked user-accessible**, returning whether
/// it was already permitted (always `true`, so the three arch modules share one signature).
///
/// A no-op here: SMAP (`CR4.SMAP`), which would forbid it, is off, as aarch64 never sets
/// `PSTATE.PAN`. RISC-V forbids it unless `sstatus.SUM` is set, which is why this exists.
///
/// # BUGS
/// SMAP is off because nothing has measured its cost on the syscall path; with it on, a kernel bug
/// that strays into the low half would fault rather than succeed quietly.
#[cfg(any(test, feature = "system_tests"))]
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))] // the system tests call it; a unit-test boot on some ISAs does not
pub fn permit_kernel_access_to_user_pages(_allowed: bool) -> bool {
    true
}

/// Install a user address space unconditionally, by writing `CR3`. The running kernel installs
/// through [`switch_user_root`]; this is for a test asserting about one specific address space.
///
/// # Safety
/// `cr3` must be a value [`ttbr0_value`] composed over a live root that carries the kernel's high
/// half. Anything else unmaps the instruction after this one; see [`switch_user_root`].
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
pub unsafe fn activate_user(cr3: u64) {
    // SAFETY: this function's own `# Safety` contract is exactly the one `install` needs; it
    // forwards, it does not weaken.
    unsafe { install(cr3) };
}

/// Remove the user address space from this CPU: fall back to the kernel-only reserved root.
#[allow(dead_code)]
pub fn deactivate_user() {
    // SAFETY: `reserved_root()` composes `KERNEL_ROOT`, the fine map `init` built, verified and is
    // running on. It carries the kernel half by construction and its low half is empty.
    unsafe { switch_user_root(reserved_root()) };
}

/// Discharge every translation belonging to the address space tagged `asid`, on every online CPU.
///
/// It flushes everything: with `CR4.PCIDE` clear the hardware holds no tag to select on, so
/// keeping `crates/address_space_identifier`'s contract (after this call the number may tag
/// someone else) takes a `CR3` reload. The remote half is [`shoot_down_others`]'s NMI, because
/// `invlpg` and a `CR3` reload are both local. Under-flushing would let one process read another's
/// memory with no fault to announce it.
#[allow(dead_code)]
pub fn flush_asid(asid: u16) {
    let _ = asid; // nothing to select on; see this function's doc comment
    // SAFETY: rewriting CR3 with the value it already holds changes no mapping and invalidates
    // every non-global entry, which is exactly the discharge this owes. TLB maintenance is always
    // sound; getting it wrong means a stale translation, which is the unsafety that matters here.
    unsafe { install(read_cr3()) };
    shoot_down_others(SHOOTDOWN_ALL);
}

/// **Map a run of user pages into the current space and invalidate once, when the batch drops**,
/// for `PageFrame::MAP`. Name provisional (lane 2451, 2026-10-20 UTC).
///
/// [`map_current_user_page_frame`] flushes after every page, and here that
/// is an `invlpg` and a cross-core shootdown per page, 683 of them for the scanout. A newly valid
/// leaf needs no invalidate on x86; the one whole flush at the drop keeps that function's guard
/// against a stale translation at a reused `va`. The flush is in `Drop`, so
/// neither the end of a run nor an early return out of one can skip it.
pub(crate) struct UserMapBatch {
    lo: u64,
    hi: u64,
}

impl UserMapBatch {
    pub(crate) const fn new() -> Self {
        Self {
            lo: u64::MAX,
            hi: 0,
        }
    }

    /// Map one page as [`map_current_user_page_frame`] does, leaving its invalidate to the drop.
    pub(crate) fn map(
        &mut self,
        va: u64,
        phys: u64,
        flags: Flags,
        alloc: impl FnMut() -> Option<u64>,
    ) -> Result<(), MapError> {
        let root = current_root();
        // SAFETY: as `map_current_user_page_frame`.
        let mut mapper = unsafe { Mapper::<_, _, Ia32e>::new(root, Half::Low, alloc, phys_to_ptr) };
        mapper.map(va, phys, flags)?;
        self.lo = self.lo.min(va);
        self.hi = self.hi.max(va + PAGE_SIZE);
        Ok(())
    }
}

impl Drop for UserMapBatch {
    fn drop(&mut self) {
        if self.lo >= self.hi {
            return;
        }
        flush_asid(0); // x86 has no tag to select on: this is every core's whole flush
    }
}

/// Map one user page at `va` onto the already-owned physical frame `phys` in the current address
/// space, drawing any intermediate tables from `alloc`.
///
/// The TLB is flushed for `va` afterwards. x86 does not require it for a *newly valid* leaf the way
/// RISC-V does, because the walker is not permitted to cache a not-present entry, but a `va` that
/// was mapped and unmapped inside one address space would otherwise keep the old translation, and
/// distinguishing the two cases here would be a foot gun for one `invlpg`.
#[allow(dead_code)]
pub(crate) fn map_current_user_page_frame(
    va: u64,
    phys: u64,
    flags: paging::Flags,
    alloc: impl FnMut() -> Option<u64>,
) -> Result<(), paging::MapError> {
    let root = current_root();
    // SAFETY: `root` is the live installed root; the direct map makes `phys_to_ptr` valid for every
    // table frame it reaches.
    let mut mapper = unsafe { Mapper::<_, _, Ia32e>::new(root, Half::Low, alloc, phys_to_ptr) };
    mapper.map(va, phys, flags)?;
    flush_tlb(va);
    Ok(())
}

/// Unmap one user page at `va` in the space rooted at `root`, invalidate the TLB, and return the
/// frame it named (the caller's to free; the mapper never owned it).
#[allow(dead_code)]
pub(crate) fn unmap_user_at(root: u64, va: u64) -> Option<u64> {
    // SAFETY: `root` is a low-half-owning root; `unmap` allocates nothing, so the `|| None`
    // allocator is never called; the direct map makes `phys_to_ptr` valid.
    let mut mapper = unsafe { Mapper::<_, _, Ia32e>::new(root, Half::Low, || None, phys_to_ptr) };
    let (pa, flush) = mapper.unmap(va).ok()?;
    flush.flush(flush_tlb);
    Some(pa)
}

/// **Unmap many user pages of one space and invalidate once, by its tag**, for a caller that takes
/// back a whole run at a time ([`crate::revoke::revoke_region`]). `each` is handed an unmap and
/// calls it once per address; pages not mapped are skipped. Name provisional (lane 2451, 2026-10-20
/// UTC).
///
/// [`unmap_user_at`] once per page costs a by-address invalidate per page, and here that is an
/// `invlpg` and a cross-core shootdown per page, 683 of them for the scanout. One
/// [`flush_asid`] after the last unmap discharges every page's obligation, because each of them is
/// a translation tagged `asid`. Until it runs another core may still hit a stale entry, so the
/// caller must not hand any of these frames back before this returns.
pub(crate) fn unmap_user_pages(root: u64, asid: u16, each: impl FnOnce(&mut dyn FnMut(u64))) {
    // SAFETY: as `unmap_user_at`.
    let mut mapper = unsafe { Mapper::<_, _, Ia32e>::new(root, Half::Low, || None, phys_to_ptr) };
    let mut unmapped = false;
    each(&mut |va| {
        if let Ok((_, flush)) = mapper.unmap(va) {
            flush.flush(|_| {}); // owed to, and paid by, the one flush_asid below
            unmapped = true;
        }
    });
    if unmapped {
        flush_asid(asid);
    }
}

/// **Cut the page table at `table` out of the walk that reaches `va` in the space rooted at
/// `root`**, and flush that space's tag. Returns the span of addresses that went with it, as
/// `(base, size)`, or `None` if the walk to `va` no longer passes through `table`. Name
/// provisional (the page-tables-outlive-destroy lane, 2026-10-05 UTC).
///
/// `revoke::revoke_region` calls this for each table a region paid for before the region's pages
/// go back: see `paging::Mapper::unlink_table` for why the cut is one entry. The flush is by ASID
/// rather than by page because the span is up to a whole table's reach and the walk caches above
/// the leaves have to go too, and a by-tag flush reaches both on every core.
pub(crate) fn cut_user_table(root: u64, va: u64, table: u64, asid: u16) -> Option<(u64, u64)> {
    // SAFETY: `root` is a live low-half table (the registry forgets a root before its space frees
    // it); the direct map makes `phys_to_ptr` valid; a cut allocates nothing.
    let mut mapper = unsafe { Mapper::<_, _, Ia32e>::new(root, Half::Low, || None, phys_to_ptr) };
    let (base, span, flush) = mapper.unlink_table(va, table)?;
    flush.flush(|_| flush_asid(asid));
    Some((base, span))
}

/// Translate `va` in the space rooted at physical `root`.
#[allow(dead_code)]
pub fn translate_at(root: u64, va: u64) -> Option<(u64, paging::Flags)> {
    // SAFETY: `root` is a page table; the direct map makes `phys_to_ptr` valid; a translate
    // allocates nothing, so the `|| None` allocator is never called.
    let mapper = unsafe { Mapper::<_, _, Ia32e>::new(root, Half::Low, || None, phys_to_ptr) };
    mapper.translate(va)
}

/// Translate `va` as a user address in the currently installed space.
#[allow(dead_code)]
pub fn translate_user(va: u64) -> Option<(u64, paging::Flags)> {
    translate_at(current_root(), va)
}
