//! **The address-space registry**: every space a capability or a thread can name, and what holds
//! it (§249 (a running address space stays nameable)). Moved out of `user.rs` unchanged by
//! milestone 812 (`std::thread::spawn` runs real threads in one address space), for §266 (a Rust
//! source file stays under 2,000 lines).
//!
//! Name: provisional (milestone 812's lane, 2026-10-10 UTC).

use super::*;

/// **How many address spaces the registry can name at once: every one the machine can hold.**
///
/// §249 (a running address space stays nameable) made the registry the owner of every space, bound
/// to a thread or not, so a running process now holds an entry for as long as it lives. Until then
/// this was 32 and named only spaces under construction, which was right while `CONFIGURE` moved a
/// space out into its thread.
///
/// **The number is [`crate::revoke::MAX_SPACES`], and that is an argument rather than a
/// coincidence.** Every [`AddressSpace`] registers with the revocation registry when it is built
/// and leaves it only when it drops, so no more than that many spaces can exist at once, and an
/// entry here holds exactly one. Sized the same, this table cannot fill while a space that wants an
/// entry exists, which is what lets [`register_bound_address_space`] treat a full table as a broken
/// invariant rather than an error every kernel spawn path would have to carry. The `const` assert
/// below keeps the two from drifting apart; the revocation registry's own constant is the thread
/// ceiling plus headroom, so the argument follows the thread ceiling when it moves.
///
/// What it costs, measured from the type rather than estimated: `registry_footprint`, which
/// `system_tests`' `running_space_tests` prints, is 21,904 bytes of `.bss` on all three
/// architectures at 288 slots (at 32 it was about a ninth of that; not measured then): 72 bytes an entry and a 4-byte
/// generation each. That is the whole of what every process holding an entry needs, because the
/// space's pages were always paid from its own region. Nothing walks the empty slots:
/// `generational_table::Table` bounds every sweep by its highest live slot, and the context switch
/// never reads this table at all ([`BoundSpace`]).
const MAX_USER_SPACES: usize = crate::revoke::MAX_SPACES;
const _: () = assert!(MAX_USER_SPACES >= crate::revoke::MAX_SPACES);

/// **One registry entry: a space, and what holds it if anything does** (§249; name provisional).
///
/// One holder per space. Since milestone 812 (`std::thread::spawn` runs real threads in one address
/// space) the holder is a process, and many threads share a space by joining that process
/// (`BIND`); a thread configured with a space is a process of its own and holds the space itself,
/// as every thread did before. So §249's amendment (b), which refused a second thread a bound space
/// and on which §105 (`std::thread::spawn` stays declined) rested, retires as calef ruled on pull
/// request #1892: threads share through their process, and a second *holder* is still refused.
struct Registered {
    space: AddressSpace,
    bound: Option<Holder>,
}

/// **What holds a space** (milestone 812; name provisional): a thread that is a process of its own,
/// or a process object whose members run in it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Holder {
    /// A thread configured with the space directly.
    Thread(crate::thread::ThreadId),
    /// A process created holding the space (`RETYPE_OBJ` of `objtype::PROCESS`).
    Process(crate::sched::ProcessId),
}

/// **What a thread keeps of the space it is bound to** (§249; name provisional): the registry's
/// name for it, and the three values the context switch reads, copied out once at bind time.
///
/// The space itself lives in the registry, which owns it. The switch reads this instead, because
/// it runs under `IPC_TABLES` and the registry's lock ranks above that one, so the switch could not
/// take it, and should not want to on the hottest path in the kernel. All three are immutable for
/// the life of the space: a space's root and its TLB tag are fixed at creation, and its current-CPU
/// page is attached before the bind and freed only by its `Drop`.
///
/// **What makes the copy safe is who may drop the space**, and that is [`reap_address_spaces_in_region`]'s
/// and the reaper's rule, not this type's: a bound space is dropped only once its thread can never
/// be switched in again (it is gone from the thread table, or it is a corpse no core is standing
/// on). So no switch ever reads a `BoundSpace` whose space has been freed.
#[derive(Clone, Copy)]
pub struct BoundSpace {
    name: u64,
    root: u64,
    ttbr0: u64,
    current_cpu_page: Option<u64>,
}

impl BoundSpace {
    fn of(name: u64, space: &AddressSpace) -> Self {
        BoundSpace {
            name,
            root: space.root(),
            ttbr0: space.ttbr0(),
            current_cpu_page: space.current_cpu_page.map(|(_, kernel_va)| kernel_va),
        }
    }

    /// **This copy with a thread's own current-CPU page** (milestone 812): a member of a process
    /// takes the process's copy and replaces the page with the one its slot holds.
    pub(crate) fn with_current_cpu_page(self, kernel_va: u64) -> Self {
        BoundSpace {
            current_cpu_page: Some(kernel_va),
            ..self
        }
    }

    /// Whether this copy carries a current-CPU page, which `CONFIGURE`'s result reports.
    pub(crate) fn has_current_cpu_page(&self) -> bool {
        self.current_cpu_page.is_some()
    }

    /// The registry's name for the space, which is what a thread's reaper takes it out by.
    pub fn name(&self) -> u64 {
        self.name
    }

    /// The physical address of the space's root table.
    pub fn root(&self) -> u64 {
        self.root
    }

    /// The composed `TTBR0_EL1` (or `satp`, or `CR3`) value the context switch installs.
    pub(crate) fn ttbr0(&self) -> u64 {
        self.ttbr0
    }

    /// `AddressSpace::publish_current_cpu`, from the copy: one branch and one relaxed store.
    #[inline]
    pub(crate) fn publish_current_cpu(&self, cpu: u64) {
        if let Some(kernel_va) = self.current_cpu_page {
            // SAFETY: the frame is the bound space's own, freed only by that space's `Drop`, and
            // the space is dropped only after its thread can never be switched in again (this
            // type's own doc). The caller is the one core switching this thread in, so it is the
            // only writer for as long as the store takes.
            unsafe { current_cpu_contract::publish(kernel_va, cpu) };
        }
    }
}

/// **The address-space registry** (milestone 19b (run a real workload); since §249, the owner of every space a capability
/// or a thread can name): the kernel-side records behind `Object::AddressSpace` capabilities, named
/// generationally like everything since milestone 14 (kernel objects from untyped). The `AddressSpace` in the slot is the same
/// type exec builds, so every mechanism that works on a process's space (region-paid tables,
/// revocation logs, ASID tagging) works on a user-built one identically.
///
/// **Two removers, and a removal is take-once.** A space leaves when its thread is reaped
/// (`sched::reap_switched_out`, by the name in the thread's [`BoundSpace`]) or when the region
/// sweep takes it ([`reap_address_spaces_in_region`]). Both go through `Table::remove`, which hands
/// the space out once and makes the name dead in the same step, so whichever comes second finds
/// nothing and a double free is not representable. Deleting a capability removes nothing (§249's
/// amendment (a)): a capability is a name, and it simply stops resolving once the space is gone.
static USER_SPACES: crate::sync::IrqSafeMutex<
    generational_table::Table<Registered, MAX_USER_SPACES>,
> = crate::sync::IrqSafeMutex::new(
    crate::sync::rank::ADDRESS_SPACES,
    generational_table::Table::new(),
);

/// **The registry's static footprint in bytes**, for the boot that reports it (§249's
/// `MAX_USER_SPACES` raise; name provisional).
#[cfg(any(test, feature = "system_tests"))]
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
pub const fn registry_footprint() -> usize {
    size_of::<generational_table::Table<Registered, MAX_USER_SPACES>>()
}

/// Create an address space **in and backed by** `region` (the `RETYPE_OBJ(ADDRESS_SPACE)` engine): the
/// root page is retyped from it (pinning it, atomically with the carve), and the region becomes
/// the space's table-and-record budget, exactly as for an exec-built space. `None` on an
/// exhausted region, a full registry, or ASID exhaustion (unreachable; the type is honest).
pub fn user_address_space_create(region: u64) -> Option<u64> {
    let root = crate::memory_region::retype_object_page(
        region,
        crate::memory_region::ObjectKind::AddressSpace,
    )?;
    mmu::share_kernel_half(root); // RISC-V single-satp: the process root carries the kernel high half

    // The tag first, because the registry records it (see `AddressSpace::new`).
    let asid = ASIDS.lock().alloc()?;
    if !crate::revoke::register_space(root, region, asid) {
        ASIDS.lock().free(asid);
        return None; // registry full; the carved page is spent, the caller's own loss (B.4 rule)
    }

    let space = AddressSpace {
        root: PageFrame::from_addr(root),
        asid,
        // Lent, not owned: the caller holds the `MemoryRegion` capability to this region and reclaims
        // it with `DESTROY`. See `Backing` for the double free that taught us to say so.
        backing: Backing::Lent(region),
        // **Not attached here**, for the same reason the timebase page is not mapped here (the
        // comment below): this syscall serves every purpose that wants a bare address-space
        // object, most of which never run a thread and some of which are sized to the page. The
        // space gets its page when a TCB binds it, in `bind_user_address_space`,
        // which is the moment it becomes a thread's space and therefore the moment the question
        // "which CPU am I on" starts having an answer.
        current_cpu_page: None,
        thread_pages: [0; current_cpu_contract::THREAD_PAGE_SLOTS],
        thread_pages_made: 0,
        thread_pages_in_use: 0,
    };

    // The timebase page is **not** mapped unconditionally here (an earlier version of this
    // lane's work did, and a full-suite run under `script/test --arch x86_64` caught two
    // regressions: the hand-sized demo region of the test now named
    // `a_process_composed_from_two_capabilities_runs_in_the_space_it_built` ran out of table
    // budget, and it makes no sense for the many callers of this syscall that build
    // nothing resembling a real ELF process at all). This syscall is shared by every purpose that
    // needs a bare address space object, not only the userspace ELF loader
    // (`supervision_protocol::build_child_space`), and the loader is where this page actually
    // belongs: see that crate's own code for the targeted fix, which writes the page from the rate
    // the *parent* already holds, so a child reads its parent's measured number and a parent that
    // knows nothing hands down nothing rather than a plausible constant.
    let name = USER_SPACES
        .lock()
        .insert_with(|_| Registered { space, bound: None });
    if name.is_none() {
        // Undo the bookkeeping; the page stays spent on the caller's budget. (Unreachable while
        // `MAX_USER_SPACES` covers the revocation registry, which `register_space` above already
        // admitted this space to; kept because the value would otherwise be dropped unregistered.)
        crate::revoke::forget_root(root);
        ASIDS.lock().free(asid);
    }
    name
}

/// Map `phys` into the user-built space `name` at `va`, one page under one hold. Tables and the
/// §13 record come from the space's own backing region; an unrecordable mapping is unmapped and
/// refused, exactly as at the `page_frame::MAP` syscall, because a mapping revocation cannot see is
/// the §13 use-after-free.
///
/// `under` says which capability's authority this mapping was made with, which is what scopes a
/// later `PageFrame::REVOKE` to that capability's derivation family rather than to the physical
/// page (DECISIONS §132). The `MAP_INTO` syscall passes the invoked frame capability's object; the
/// kernel's own callers, which build a space directly out of a region, pass
/// `PageMapSource::NoCapability`.
// Not `MAP_INTO`'s engine since 2026-10-04 (it maps under one `MappingHold` through
// `with_user_address_space`); the kernel's own wiring calls it only in test builds, and the system
// tests build spaces with it.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub fn user_address_space_map(
    name: u64,
    va: u64,
    phys: u64,
    flags: Flags,
    under: crate::revoke::PageMapSource,
) -> Result<(), MapError> {
    let mut spaces = USER_SPACES.lock();
    let space = &mut spaces.get_mut(name).ok_or(MapError::NotMapped)?.space;

    // `map_physical` maps and records in one step since 2026-09-21, including the unmap-and-refuse
    // on an unrecordable mapping that used to live here: this function was the one caller that
    // remembered to record, which is exactly why it is now the one caller with nothing extra to
    // remember. See that function's own docs for the defect the other callers carried.
    space.map_physical(va, phys, flags, under)?;
    // A code page a loader just filled via data writes (milestone 19d): the instruction fetcher
    // has its own cache and has never heard of those bytes. On aarch64 the I-cache is not
    // coherent with the D-cache, so make it so now, via the frame's direct-map VA (any VA that
    // maps the physical page works; caches are PIPT to the point of unification). Without this,
    // the child fetches whatever was in the frame before the loader wrote its program.
    if flags.is_user_executable() {
        sync_icache(mmu::phys_to_virt(phys), FRAME_SIZE as usize);
    }
    Ok(())
}

/// The root table of a space the registry names, bound or not.
///
/// Built for tests (so a walker can ask what a space really maps) and now also
/// `abi::address_space::LIST`'s way in (milestone 126's `pmap`, DECISIONS §114): the syscall handler
/// resolves the capability's `name` to a root here before consulting `revoke::list_mapping` and
/// `arch::mmu::translate_at`. `None` once the space is gone: its thread was reaped or its region
/// destroyed (§249). A `LIST` against a capability that outlived its space reads as "nothing to
/// report," the same as an empty space, because the capability itself was never refused and the
/// kernel has nothing left to say about where it used to point.
pub fn user_address_space_root(name: u64) -> Option<u64> {
    USER_SPACES.lock().get(name).map(|e| e.space.root())
}

/// **Run `f` with the space `name` under the registry lock**, `None` if the name does not
/// resolve. `AddressSpace::MAP_INTO`'s and `UNMAP`'s way in (the map-revocation-window lane,
/// 2026-10-04 UTC; name provisional): it must hold this registry (`ADDRESS_SPACES`, 61) *above* the
/// mapping registry (`MAPPINGS`, 59) for its whole run, so it cannot go through
/// [`user_address_space_map`], which takes and drops this lock once per page. The `Option` is
/// handed in rather than checked here so the caller keeps its own refusal order: a frame that is
/// not there answers before a space that is not there, as it always has.
///
/// Since §249 a running space resolves here too, so a mapping changed through it may be one a core
/// is translating through right now. That is why `UNMAP` unmaps with the function every revoke uses,
/// whose TLB obligation reaches every core.
pub fn with_user_address_space<R>(name: u64, f: impl FnOnce(Option<&mut AddressSpace>) -> R) -> R {
    let mut spaces = USER_SPACES.lock();
    f(spaces.get_mut(name).map(|e| &mut e.space))
}

/// **Bind the space `name` to the embryo `tid`** (`ThreadControlBlock::CONFIGURE`'s engine since
/// §249; name provisional). The space stays in the registry and keeps its name, which is the whole
/// of §249's option A: a copy of the capability made before `CONFIGURE` still names the space while
/// its thread runs.
///
/// `NoSuchSlot` if the name does not resolve. **`WrongObject` if the space already has a holder**,
/// which is §249's amendment (b), the answer a second `CONFIGURE` of a started thread already gives.
/// Since milestone 812 (`std::thread::spawn` runs real threads in one address space) it keeps a space
/// to one holder; threads share it by joining the holder's process.
///
/// `bind` is called with the copy the thread will keep, under this registry's lock, and takes
/// `IPC_TABLES` itself (60, below this one's 61, so the nesting is the rank order's own direction).
/// That makes the bind one critical section across both tables: no region sweep can see a space
/// marked bound to a thread that does not have it, or the reverse. If `bind` refuses, the entry is
/// left unbound. The current-CPU page is attached first, because the copy carries its address; a
/// space that goes on to be refused keeps it, which is harmless (the attach is idempotent, and the
/// page dies with the space).
///
/// `bind` returns the holder to record: the embryo itself, or (milestone 812) the process that
/// `RETYPE_OBJ` is creating around the space.
pub(crate) fn bind_user_address_space(
    name: u64,
    bind: impl FnOnce(BoundSpace) -> Result<Holder, abi::Error>,
) -> Result<(), abi::Error> {
    let mut spaces = USER_SPACES.lock();
    let entry = spaces.get_mut(name).ok_or(abi::Error::NoSuchSlot)?;
    if entry.bound.is_some() {
        return Err(abi::Error::WrongObject);
    }
    entry.space.attach_current_cpu_page();
    let holder = bind(BoundSpace::of(name, &entry.space))?;
    // A thread that is a process of its own reads slot 0, the space's own page; a process's first
    // member takes slot 0 when it joins (`take_thread_page`).
    if matches!(holder, Holder::Thread(_)) {
        entry.space.thread_pages_in_use |= 1;
    }
    entry.bound = Some(holder);
    Ok(())
}

/// **A current-CPU page for a thread joining the process that holds `name`** (milestone 812
/// (`std::thread::spawn` runs real threads in one address space), §269 (how threads share a
/// process) fork 6): the lowest free slot, its page made and mapped read-only if the slot has
/// never been used, and rebuilt (unscheduled, with today's allowance) if it has. Returns
/// `(slot, user address, kernel address)`. `None` when every slot is taken, which is the
/// per-process thread limit (`current_cpu_contract::THREAD_PAGE_SLOTS`), or no frame or table can
/// be had.
pub(crate) fn take_thread_page(name: u64) -> Option<(u8, u64, u64)> {
    let mut spaces = USER_SPACES.lock();
    let space = &mut spaces.get_mut(name)?.space;
    let k = (!space.thread_pages_in_use).trailing_zeros() as usize;
    let va = current_cpu_contract::thread_page_va(k)?;
    let bytes = current_cpu_contract::build_page_allowing(
        u64::from(crate::smp::online_harts_mask().count_ones()),
        crate::arch::timer::TICK_HZ,
    );
    let kernel_va = if k == 0 {
        space.attach_current_cpu_page();
        space.current_cpu_page?.1
    } else if space.thread_pages_made & (1 << k) != 0 {
        mmu::phys_to_virt(space.thread_pages[k])
    } else {
        let frame = crate::memory::alloc_zeroed()?;
        let kernel_va = mmu::phys_to_virt(frame.addr());
        if space
            .map_physical(
                va,
                frame.addr(),
                Flags::user_rodata(),
                crate::revoke::PageMapSource::NoCapability,
            )
            .is_err()
        {
            crate::memory::free(frame);
            return None;
        }
        space.thread_pages[k] = frame.addr();
        space.thread_pages_made |= 1 << k;
        kernel_va
    };
    // SAFETY: the slot's own frame, owned by this space and not read by any thread while the slot
    // is free; `PAGE_BYTES` is far under a frame.
    unsafe {
        core::ptr::copy_nonoverlapping(bytes.as_ptr(), kernel_va as *mut u8, bytes.len());
    }
    space.thread_pages_in_use |= 1 << k;
    Some((k as u8, va, kernel_va))
}

/// **Give a thread's current-CPU page slot back** when the thread is reaped (milestone 812). The
/// frame stays mapped, read-only and unread, for the next thread to take the slot. Nothing if the
/// space is gone.
pub(crate) fn give_back_thread_page(name: u64, slot: u8) {
    if let Some(entry) = USER_SPACES.lock().get_mut(name) {
        entry.space.thread_pages_in_use &= !(1u16 << slot);
    }
}

/// **Put a space the kernel built into the registry, already bound to `tid`** (`sched::adopt_address_space`'s
/// half, §249; name provisional), and return the copy the thread keeps.
///
/// Infallible because it cannot fail: [`MAX_USER_SPACES`] covers every space the revocation
/// registry can hold, and this one is registered there. A full table here is a broken invariant,
/// not a resource limit, so it panics with that sentence rather than handing every kernel spawn
/// path an error it could never act on.
pub(crate) fn register_bound_address_space(
    space: AddressSpace,
    tid: crate::thread::ThreadId,
) -> BoundSpace {
    let mut spaces = USER_SPACES.lock();
    let mut copy = None;
    spaces
        .insert_with(|name| {
            copy = Some(BoundSpace::of(name, &space));
            Registered {
                space: {
                    let mut space = space;
                    space.thread_pages_in_use |= 1; // the adopting thread reads slot 0
                    space
                },
                bound: Some(Holder::Thread(tid)),
            }
        })
        .expect("the address-space registry is full, which MAX_USER_SPACES says cannot happen");
    copy.expect("insert_with names the entry before it stores it")
}

/// **Take a space out of the registry**: the reaper's half of the take-once removal (§249), and the
/// system tests' way of ending a space without a thread. `None` if the name does not resolve, which
/// for the reaper means the region sweep took it first. The space drops in the caller, outside this
/// registry's lock, because its `Drop` takes the revocation, region and ASID locks.
pub fn take_user_address_space(name: u64) -> Option<AddressSpace> {
    USER_SPACES.lock().remove(name).map(|e| e.space)
}

/// **Tear down every space a region's destruction ends** (object revocation, the address-space
/// case; widened by §249). Each removed `AddressSpace` drops here, and its `Drop` forgets its
/// revocation records and frees its ASID (its region's memory comes back at the enclosing
/// `reclaim_region`, which unpins after this).
///
/// Three kinds of entry go, and the registry owning bound spaces (§249) is what made the second and
/// third reachable here at all:
///
/// 1. **An unbound space whose root is in `[base, end)`**: built from the region, never bound.
/// 2. **A bound space whose thread is gone from the thread table**, wherever its root lives. This is
///    a thread `reap_region_objects` just removed because its TCB was in the region: removing a
///    `Thread` drops nothing of its space now, so the space is collected here, in the same
///    `reclaim_region`, before the region is unpinned.
/// 3. **A bound space whose root is in the span and whose thread is a corpse no core stands on**
///    (`Dead` or `Finished`, off its stack). The corpse can never be switched in again, so taking its
///    space is safe, and it is what closes the gap `notes/naming-a-running-address-space.md` found
///    by reading: a corpse whose TCB is outside the region used to keep its space, so the region came
///    back while the corpse still owned the root, and the corpse's later drop forgot the revocation
///    records of whoever was given that page next. Its reaper now finds the name dead and drops
///    nothing.
///
/// **A bound space whose thread can still run is left alone**, root in the span or not. That is the
/// hole milestone 765 (a destroyed region cannot free the root a running thread walks) closes, by
/// making `reap_region_objects` refuse and kill such a thread first; until then it stands exactly as
/// it stood before §249, recorded at `revoke::revoke_region`. A corpse still standing on its stack
/// (`on_cpu`) is counted with the runnable ones for the same reason: a core still has its root
/// installed.
///
/// Takes the registry lock and, under it, `IPC_TABLES` once per scan to ask about the threads (61
/// then 60, the rank order's direction). Never drops an `AddressSpace` under either.
pub(crate) fn reap_address_spaces_in_region(base: u64, end: u64) {
    loop {
        let victim = {
            let spaces = USER_SPACES.lock();
            crate::sched::with_holders(|binder| {
                spaces.iter().find_map(|(name, entry)| {
                    let root = entry.space.root.addr();
                    let in_span = base <= root && root < end;
                    let goes = match entry.bound {
                        None => in_span,
                        Some(holder) => match binder(holder) {
                            crate::sched::Binder::Gone => true,
                            crate::sched::Binder::Corpse => in_span,
                            crate::sched::Binder::CanRun => false,
                        },
                    };
                    goes.then_some(name)
                })
            })
        };
        let Some(name) = victim else { break };
        // `remove` returns the entry; the registry lock is released at the `;`, then the space drops.
        let space = USER_SPACES.lock().remove(name);
        drop(space);
    }
}

/// Put a space the kernel built into the registry, unbound, and return its name: how a kernel
/// spawn path that builds a process the way userspace does (`CONFIGURE` by name) gets one. Named for
/// milestone 19c.3's unwind path, which it no longer serves: since §249 a refused bind leaves the
/// space where it was.
pub fn readopt_user_address_space(space: AddressSpace) -> Option<u64> {
    USER_SPACES
        .lock()
        .insert_with(|_| Registered { space, bound: None })
}
