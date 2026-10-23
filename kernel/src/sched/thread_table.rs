//! The thread table: the fixed array of thread control block pointers, generational over
//! [`MAX_THREADS`] slots, that every thread id names. See notes/thread-control-block.md ("The
//! thread table").
//!
//! Name: provisional (milestone 840's lane, 2026-10-11 UTC; §271 (the scheduler file splits into
//! submodules of `sched`)).

use super::*;

/// The most threads that were ever alive at once on this boot, which makes [`MAX_THREADS`] a
/// measured number: a refused spawn cannot say how close the suite came.
///
/// Context: updated with `fetch_max` by both inserts, under `IPC_TABLES`. See
/// notes/thread-control-block.md ("The peak").
pub(super) static PEAK_THREADS: core::sync::atomic::AtomicUsize =
    core::sync::atomic::AtomicUsize::new(0);

/// Scheduled on-CPU time, one timer tick at a time, keyed by thread slot (§150 (how does a thread's
/// CPU time reach userspace?)). `abi::survey::record::CPU_TIME` reads it.
///
/// An array beside the table, not a field on `Thread` or `cpu::PerCpu`: [`on_tick`] knows its
/// running thread only as a tid, so the slot needs no lock. Each core writes only its running
/// thread's slot; a survey's relaxed read of a slot in flight is stale, never torn. Zeroed when a
/// slot is filled, so a corpse keeps its time until it is reaped. See
/// notes/thread-control-block.md ("CPU time, per slot").
pub(super) static CPU_TICKS: [core::sync::atomic::AtomicU64; MAX_THREADS] =
    [const { core::sync::atomic::AtomicU64::new(0) }; MAX_THREADS];

/// The slot a generational thread name occupies, for indexing [`CPU_TICKS`].
///
/// `generational_table` packs `(generation << 32) | slot`, so the slot is the low word. Unchecked:
/// every caller indexes with `get`, so `cpu::NO_TID` (`u64::MAX`) is out of range and `None`.
pub(super) const fn slot_of(tid: ThreadId) -> usize {
    (tid & 0xffff_ffff) as usize
}

/// Charge one timer tick to the thread this core is running.
///
/// One relaxed load of this core's `current`, one bounds-checked index, one relaxed increment; a
/// core running nothing (`cpu::NO_TID`) charges nobody. The idle thread is charged like any other,
/// and no survey reports it.
///
/// Context: called from [`on_tick`] in interrupt context. Takes no lock and never blocks.
pub(super) fn charge_tick() {
    if let Some(counter) = CPU_TICKS.get(slot_of(current_thread_id())) {
        counter.fetch_add(1, Ordering::Relaxed);
    }
}

/// Start a freshly filled slot's CPU time at zero. Called by both inserts, under `IPC_TABLES`.
pub(super) fn clear_cpu_ticks(tid: ThreadId) {
    if let Some(counter) = CPU_TICKS.get(slot_of(tid)) {
        counter.store(0, Ordering::Relaxed);
    }
}

/// The high-water mark [`PEAK_THREADS`] holds. Printed by the test suite's closing summary.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))] // the closing summary is the only reader
pub fn peak_thread_count() -> usize {
    PEAK_THREADS.load(Ordering::Relaxed)
}

/// A TCB pointer that may cross cores. The `Thread` it names is touched only under `IPC_TABLES`,
/// and its queue link only under the intrusive discipline at [`thread_control_block_ptr`].
#[derive(Clone, Copy)]
pub(super) struct ThreadControlBlockPointer(*mut Thread);

// SAFETY: see the type's doc; sending the pointer is sound because dereferencing it is gated.
unsafe impl Send for ThreadControlBlockPointer {}

/// The thread table: generational names (`crates/slots`, notes/generational-names.md) over
/// page-resident TCBs, each at the start of one page that never moves, so the context-switch
/// assembly and the intrusive queues may hold pointers into it. See notes/thread-control-block.md.
pub(super) struct Threads {
    pub(super) table: generational_table::Table<ThreadControlBlockPointer, MAX_THREADS>,
}

impl Threads {
    pub(super) const fn new() -> Self {
        Self {
            table: generational_table::Table::new(),
        }
    }

    pub(super) fn get(&self, tid: ThreadId) -> Option<&Thread> {
        let p = self.table.get(tid)?.0;
        // SAFETY: a pointer we stored at insert, into a live kmem page not yet recycled (remove
        // kills the name before recycling); IPC_TABLES serializes access.
        Some(unsafe { &*p })
    }

    pub(super) fn get_mut(&mut self, tid: ThreadId) -> Option<&mut Thread> {
        let p = self.table.get(tid)?.0;
        // SAFETY: as `get`, and `&mut self` carries IPC_TABLES's exclusivity.
        Some(unsafe { &mut *p })
    }

    /// **A thread's capability table**, for a caller holding `IPC_TABLES` that needs no other part
    /// of the thread. The table is past the `Thread` in its page (`thread::capability_table_of`), so
    /// the reference is derived from the page pointer, never from a `&Thread`.
    pub(super) fn capabilities(&self, tid: ThreadId) -> Option<&CapabilityTableLock> {
        let p = self.table.get(tid)?.0;
        // SAFETY: the stored page pointer of a live thread; the page outlives `&self`, because
        // only `remove` (which takes `&mut self`) recycles it.
        Some(unsafe { &*crate::thread::capability_table_of(p) })
    }

    /// `get_mut` and [`capabilities`](Self::capabilities) at once, for the sites that write the
    /// `Thread` and its table together. Disjoint by construction: the table is past the struct's
    /// end, so the `&mut Thread` does not cover it.
    pub(super) fn get_mut_with_capabilities(
        &mut self,
        tid: ThreadId,
    ) -> Option<(&mut Thread, &CapabilityTableLock)> {
        let p = self.table.get(tid)?.0;
        // SAFETY: as `get_mut` for the `Thread` and `capabilities` for the table; the two do not
        // overlap.
        Some(unsafe { (&mut *p, &*crate::thread::capability_table_of(p)) })
    }

    /// The raw pointer the table stores, the start of the thread's TCB page, unnarrowed.
    ///
    /// The FP register file lives past the struct's end in the same page, so a pointer derived
    /// from a `&Thread` would reach outside its provenance (`thread::fp_state_of`'s safety note).
    pub(super) fn pointer(&self, tid: ThreadId) -> Option<*mut Thread> {
        Some(self.table.get(tid)?.0)
    }

    /// Insert: claim a page from the kernel budget, build the `Thread` (carrying its own minted
    /// name) into it, and store the pointer under that name. `None` (page recycled, `f` never
    /// run) if the budget or the table is exhausted.
    pub(super) fn insert_with(&mut self, f: impl FnOnce(ThreadId) -> Thread) -> Option<ThreadId> {
        let page = crate::kmem::page()?;
        // A kernel thread's TCB page is `kmem`'s and comes home to it at death; if the table is
        // full it never held a Thread, so recycle now.
        let name = self.insert_at(page, f);
        if name.is_none() {
            crate::kmem::recycle(page);
        }
        name
    }

    /// `insert_with`'s place-writing twin: claim a page from the kernel budget and let `build`
    /// construct the Thread into it. Recycles the page on any failure, exactly as `insert_with`
    /// does, so a refused spawn costs nothing.
    pub(super) fn insert_in_place(
        &mut self,
        build: impl FnOnce(ThreadId, *mut Thread) -> bool,
    ) -> Option<ThreadId> {
        let page = crate::kmem::page()?;
        let name = self.insert_at_in_place(page, build);
        if name.is_none() {
            crate::kmem::recycle(page);
        }
        name
    }

    /// Insert a Thread that already has a page (milestone 19c.3): a user-retyped TCB, whose page
    /// is its creator's region's, not `kmem`'s. On a full table the page is the region's to
    /// account (spend-only), so nothing is recycled here.
    pub(super) fn insert_from_page(
        &mut self,
        page: u64,
        f: impl FnOnce(ThreadId) -> Thread,
    ) -> Option<ThreadId> {
        self.insert_at(page, f)
    }

    /// The place-writing insert: `build` receives the minted name and the TCB page and writes the
    /// Thread there itself, so a large `Thread` is not copied through a return value. `false` from
    /// `build` means it wrote nothing, and the slot is left as it was. See `Thread::spawn_into`.
    fn insert_at_in_place(
        &mut self,
        page: u64,
        build: impl FnOnce(ThreadId, *mut Thread) -> bool,
    ) -> Option<ThreadId> {
        let ptr = crate::arch::mmu::phys_to_virt(page) as *mut Thread;
        let mut built = false;
        let name = self.table.insert_with(|tid| {
            built = build(tid, ptr);
            if built {
                // In place, not in a constructor: `kmem` pages are not zeroed, so the register
                // file's `live` flag must be written before anything reads it.
                //
                // SAFETY: `ptr` is the start of a page this insert exclusively owns, and `build`
                // has just written a live `Thread` at it.
                unsafe { crate::thread::init_fp_state(ptr) };
                // SAFETY: as above; the name is not yet handed to anyone, so nothing else can
                // reach the table.
                unsafe { crate::thread::init_capability_table(ptr) };
            }
            ThreadControlBlockPointer(ptr)
        })?;
        if !built {
            // `build` declined (no kernel stack), so the slot never held a Thread. `Table::remove`
            // drops only the stored pointer and bumps the generation; the page is the caller's
            // to recycle.
            self.table.remove(name);
            return None;
        }
        self.note_peak();
        self.mint_token(name);
        // A reused slot starts its CPU accounting at zero; see `CPU_TICKS`.
        clear_cpu_ticks(name);
        Some(name)
    }

    /// The shared engine: write the built Thread into `page` and name it. The Thread carries its
    /// own `thread_control_block_kmem`, which `remove` reads to decide whether the page returns to `kmem`.
    fn insert_at(&mut self, page: u64, f: impl FnOnce(ThreadId) -> Thread) -> Option<ThreadId> {
        let ptr = crate::arch::mmu::phys_to_virt(page) as *mut Thread;
        let name = self.table.insert_with(|tid| {
            // SAFETY: a fresh, exclusively-ours page; `write` moves the Thread in, no drop of
            // uninitialized bytes.
            unsafe { ptr.write(f(tid)) };
            // And its register file beside it; see `insert_at_in_place` for why here.
            //
            // SAFETY: as above, with the `Thread` now live at `ptr`.
            unsafe { crate::thread::init_fp_state(ptr) };
            // And its capability table, past the register file, for the same reason.
            //
            // SAFETY: as above; the name is not yet handed to anyone.
            unsafe { crate::thread::init_capability_table(ptr) };
            ThreadControlBlockPointer(ptr)
        });
        if let Some(name) = name {
            self.note_peak();
            self.mint_token(name);
            // A reused slot starts its CPU accounting at zero; see `CPU_TICKS`.
            clear_cpu_ticks(name);
        }
        name
    }

    /// Mint a new thread's queue token, the only place in the kernel a token is made. Both inserts
    /// call it once on the name they minted, so every thread has one token for its whole life.
    ///
    /// The `unsafe` discharges `Unqueued::new`'s three obligations: the `Thread` lives until
    /// `Threads::remove`, reached only for a thread on no queue; the name is a moment old, so no
    /// other token for its page exists; and every queue holding a `Thread` is behind `IPC_TABLES`
    /// or an inbox lock. See notes/thread-control-block.md ("The thread table").
    fn mint_token(&mut self, name: ThreadId) {
        if let Some(t) = self.get_mut(name) {
            let node = core::ptr::NonNull::from(&mut *t);
            // SAFETY: the three obligations above.
            t.own_token = Some(unsafe { Unqueued::new(node) });
        }
    }

    /// Remove and destroy: drop the TCB in place (its stack, address space, and quota token go
    /// with it), kill the name so no copy of the `ThreadId` ever resolves again, then recycle the page.
    pub(super) fn remove(&mut self, tid: ThreadId) {
        let Some(&ThreadControlBlockPointer(ptr)) = self.table.get(tid) else {
            return;
        };
        // Read the page's origin before the drop consumes the Thread: a kernel TCB's page goes
        // home to `kmem`, a user TCB's belongs to its region.
        // SAFETY: live per the table, exclusive per `&mut self`.
        let from_kmem = unsafe { (*ptr).thread_control_block_kmem };
        // SAFETY: as above. Drop first (KernelStack's unmap-and-recycle, AddressSpace teardown,
        // the QuotaToken), then kill the name, then the page goes home: nothing can reach the
        // dropped Thread afterward.
        unsafe { core::ptr::drop_in_place(ptr) };
        self.table.remove(tid);
        if from_kmem {
            crate::kmem::recycle(crate::arch::mmu::virt_to_phys(ptr as u64));
        }
    }

    pub(super) fn len(&self) -> usize {
        self.table.len()
    }

    /// Record the table's occupancy against [`PEAK_THREADS`]. Every insert reaches it through
    /// `insert_at` or `insert_at_in_place`.
    fn note_peak(&self) {
        PEAK_THREADS.fetch_max(self.table.len(), Ordering::Relaxed);
    }

    /// Every live TCB, for whole-table sweeps (revocation). Each live name resolves to a
    /// distinct page pointer, so the `&mut`s are disjoint.
    pub(super) fn iter_mut(&mut self) -> impl Iterator<Item = &mut Thread> + '_ {
        // SAFETY: each stored pointer is a distinct live page (one page per thread), and
        // `&mut self` carries IPC_TABLES's exclusivity across the whole sweep.
        self.table
            .values()
            .map(|&ThreadControlBlockPointer(p)| unsafe { &mut *p })
    }

    /// [`iter_mut`](Self::iter_mut) with each thread's capability table beside it, for the
    /// revocation sweeps. Disjoint as [`get_mut_with_capabilities`](Self::get_mut_with_capabilities).
    pub(super) fn iter_mut_with_capabilities(
        &mut self,
    ) -> impl Iterator<Item = (&mut Thread, &CapabilityTableLock)> + '_ {
        // SAFETY: as `iter_mut`, and the table is past each `Thread`'s end in its own page.
        self.table
            .values()
            .map(|&ThreadControlBlockPointer(p)| unsafe {
                (&mut *p, &*crate::thread::capability_table_of(p))
            })
    }

    /// Every live TCB from slot `from` onward, with its slot index, for a resumable sweep
    /// (`rendezvous::SURVEY`). The slot is the caller's cursor; see
    /// `generational_table::Table::iter_from` for why a position would not do.
    pub(super) fn iter_from(&self, from: usize) -> impl Iterator<Item = (usize, &Thread)> + '_ {
        // SAFETY: as `iter_mut`, and shared rather than exclusive: each stored pointer is a
        // distinct live page, and `&self` carries IPC_TABLES for the walk.
        self.table
            .iter_from(from)
            .map(|(slot, _, &ThreadControlBlockPointer(p))| (slot, unsafe { &*p }))
    }
}
