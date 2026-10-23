//! **The address-space registry**: every space a capability or a thread can name, and what holds
//! it (§249 (a running address space stays nameable)). See notes/user-address-spaces.md.
//!
//! Name: provisional (milestone 812's lane, 2026-10-10 UTC).

use super::*;

/// **How many address spaces the registry can name at once: every one the machine can hold.**
///
/// [`crate::revoke::MAX_SPACES`], because every [`AddressSpace`] is in the revocation registry
/// from build to drop and holds one entry here, so this table cannot fill while a space wanting an
/// entry exists. That lets [`register_bound_address_space`] treat a full table as a broken
/// invariant; the `const` assert keeps the two together. It is 21,904 bytes of `.bss` at 288 slots
/// (`registry_footprint`); see notes/user-address-spaces.md.
const MAX_USER_SPACES: usize = crate::revoke::MAX_SPACES;
const _: () = assert!(MAX_USER_SPACES >= crate::revoke::MAX_SPACES);

/// **One registry entry: a space, and what holds it if anything does** (§249; name provisional).
///
/// One holder per space: a thread that is a process of its own, or a process whose members share
/// the space by joining it (`BIND`, milestone 812). A second holder is refused.
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
/// name for it, and the three values the context switch reads, copied once at bind time.
///
/// The switch holds `IPC_TABLES`, which ranks below the registry's lock, so it reads this copy
/// instead; all three values are fixed for the space's life. The copy is safe because a bound space
/// is dropped only once its thread can never be switched in again, the rule
/// [`reap_address_spaces_in_region`] and the reaper keep.
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

/// **The address-space registry**: the kernel-side records behind `Object::AddressSpace`
/// capabilities, named generationally, and since §249 the owner of every space a capability or a
/// thread can name.
///
/// Two removers, and a removal is take-once: the reaper, by the name in a thread's [`BoundSpace`],
/// and the region sweep ([`reap_address_spaces_in_region`]). `Table::remove` hands the space out
/// once and kills the name in the same step, so a double free is not representable. Deleting a
/// capability removes nothing (§249's amendment (a)).
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
        // Lent: the caller holds this region's `MemoryRegion` capability (`Backing`).
        backing: Backing::Lent(region),
        // Attached when a TCB binds the space (`bind_user_address_space`): most callers of this
        // syscall never run a thread, and some are sized to the page.
        current_cpu_page: None,
        thread_pages: [0; current_cpu_contract::THREAD_PAGE_SLOTS],
        thread_pages_made: 0,
        thread_pages_in_use: 0,
    };

    // No timebase page either: the userspace loader (`supervision_protocol::build_child_space`)
    // maps one from its parent's rate, and most callers build nothing like an ELF process.
    let name = USER_SPACES
        .lock()
        .insert_with(|_| Registered { space, bound: None });
    if name.is_none() {
        // Undo the bookkeeping; the page stays spent on the caller's budget. Unreachable while
        // `MAX_USER_SPACES` covers the revocation registry.
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
// The kernel's own wiring calls this only in test builds; the system tests build spaces with it.
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

    space.map_physical(va, phys, flags, under)?;
    // The I-cache is not coherent with the D-cache on aarch64, so a code page a loader filled with
    // data writes is synced through its direct-map VA (caches are PIPT to the point of
    // unification), or the child fetches the frame's old contents.
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

/// **Run `f` with the space `name` under the registry lock**, `None` if the name does not resolve:
/// `AddressSpace::MAP_INTO`'s and `UNMAP`'s way in (name provisional).
///
/// Context: holds `ADDRESS_SPACES` (61) for the whole of `f`, above `MAPPINGS` (59), which is why
/// the caller cannot use [`user_address_space_map`]. The caller checks the `Option` itself, to keep
/// its own refusal order. A running space resolves here too, so `UNMAP` unmaps with the function
/// every revoke uses,
/// whose TLB obligation reaches every core.
pub fn with_user_address_space<R>(name: u64, f: impl FnOnce(Option<&mut AddressSpace>) -> R) -> R {
    let mut spaces = USER_SPACES.lock();
    f(spaces.get_mut(name).map(|e| &mut e.space))
}

/// **Bind the space `name` to the embryo `tid`** (`ThreadControlBlock::CONFIGURE`'s engine since
/// §249; name provisional). The space stays in the registry under its name (§249's option A).
///
/// `bind` gets the copy the thread will keep and returns the holder to record: the embryo, or the
/// process `RETYPE_OBJ` is creating. `NoSuchSlot` if the name does not resolve; `WrongObject` if
/// the space already has a holder (§249's amendment (b)); `bind`'s own refusal leaves it unbound.
///
/// Context: holds `ADDRESS_SPACES` (61), and `bind` takes `IPC_TABLES` (60) under it, so the bind
/// is one critical section across both tables.
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
/// case). Each removed space drops here; its memory comes back at the enclosing `reclaim_region`.
///
/// It takes an unbound space rooted in `[base, end)`, a bound space whose holder is gone, and a
/// bound space rooted in the span whose thread is a corpse no core stands on. A bound space whose
/// thread can still run is left alone (milestone 765 (a destroyed region cannot free the root a
/// running thread walks)). See notes/user-address-spaces.md.
///
/// Context: takes the registry lock and, under it, `IPC_TABLES` once per scan (61 then 60). Never
/// drops an `AddressSpace` under either.
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
