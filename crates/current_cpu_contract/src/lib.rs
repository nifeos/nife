//! A thread's own CPU, as a page rather than a crossing (§204 (how userspace asks where a
//! thread runs)): the page's one definition, for the kernel (its one writer) and
//! `user_mode_runtime::current_cpu` (every reader). See notes/current-cpu-page.md.
//!
//! The kernel writes the word on the reader's own core in `schedule()`, and the scheduler's
//! release/acquire pair on the run queue orders a migration's handoff, so both accesses are
//! `Relaxed`; an aligned 64-bit access does not tear. A reader may be stale the instruction after
//! it reads, so a per-CPU cache built on it must stay correct on the last core's answer. Unset
//! reads as [`None`], never zero. The word is a cpu id, not an index into `0..count`: size
//! per-CPU arrays by [`CPU_ID_BOUND`].

//! # BUGS
//!
//! - Nothing outside tests reads the word yet; the write costs under 1.5 ns on an M3 (#1904).
//! - The reader forms an `&AtomicU64` over read-only memory, which Miri rejects; see the note.
//! - At most [`THREAD_PAGE_SLOTS`] threads a process (§269 fork 6); see notes/current-cpu-page.md.
//! - An address space never bound to a TCB has no page, so `current_cpu` there faults.
//! - [`CPU_ID_BOUND`] is the compile-time `MAX_CPUS`: right for sizing, useless for iterating.
//!
//! Name: ratified 2026-10-18 (calef): "Yes". `current_cpu` is the field's own word for this
//! quantity (`cpu` is ruled in design/naming.md, 2026-10-03); the suffix follows its sibling.

#![cfg_attr(not(test), no_std)]

use core::sync::atomic::{AtomicU64, Ordering};

/// The page's first eight bytes: what tells a prepared page from a frame nobody has written. ASCII,
/// unpadded, the same shape `counter_frequency_contract::MAGIC` and `clock_contract::MAGIC` use.
///
/// The layout grows only by appending a word that reads zero where a kernel never wrote it, so a
/// new field never changes the magic. Every published program's `std` carries its own copy of this
/// value and compares it before reading the CPU or the allowance: basalt's `rg` built against
/// `CURRCPU1` reads a page under any other magic as having no allowance and walks on one thread.
/// The test `the_magic_is_the_one_published_programs_compare` holds it.
pub const MAGIC: [u8; 8] = *b"CURRCPU1";

/// What the CPU word holds before the kernel has ever switched this thread in. Not zero, because
/// zero is a valid CPU id, and "a wrong number is worse than no number" (calef, 2026-09-21).
///
/// # Examples
///
/// Neither a zeroed frame nor a thread that has never run fabricates an answer:
///
/// ```
/// use current_cpu_contract::{CurrentCpuPage, PAGE_BYTES, build_page};
///
/// # #[repr(align(8))]
/// # struct Aligned([u8; PAGE_BYTES]);
/// let mut zeroed = Aligned([0u8; PAGE_BYTES]);
/// // SAFETY: a live, aligned buffer of exactly `PAGE_BYTES` for this block.
/// assert_eq!(unsafe { CurrentCpuPage::new(zeroed.0.as_mut_ptr() as u64) }.cpu(), None);
///
/// let mut fresh = Aligned(build_page());
/// // SAFETY: as above.
/// assert_eq!(unsafe { CurrentCpuPage::new(fresh.0.as_mut_ptr() as u64) }.cpu(), None);
/// ```
pub const UNSCHEDULED: u64 = u64::MAX;

/// One past the highest CPU id this kernel will ever hand out: the size of a per-CPU array in
/// userspace. The kernel's `cpu::MAX_CPUS` is this constant (AGENTS.md rule 7). Not the online
/// count; see the crate's `BUGS` section, and `crates/cpu_set` for why counting is not indexing.
///
/// # Examples
///
/// Sizing a per-CPU array, which is the consumer this page exists for:
///
/// ```
/// # use current_cpu_contract::{CurrentCpuPage, build_page, publish, CPU_ID_BOUND};
/// let mut caches = [0u64; CPU_ID_BOUND]; // never `[0; cores_i_have_seen]`
/// # #[repr(align(8))]
/// # struct Aligned([u8; current_cpu_contract::PAGE_BYTES]);
/// # let mut bytes = Aligned(build_page());
/// # let va = bytes.0.as_mut_ptr() as u64;
/// # // SAFETY: as in `CurrentCpuPage`'s example.
/// # unsafe { publish(va, 3) };
/// # // SAFETY: as in `CurrentCpuPage`'s example.
/// # let page = unsafe { CurrentCpuPage::new(va) };
/// if let Some(cpu) = page.cpu() {
///     caches[cpu] += 1;
/// }
/// ```
pub const CPU_ID_BOUND: usize = 8;

const OFF_MAGIC: usize = 0;
const OFF_CPU: usize = OFF_MAGIC + 8;
const OFF_ALLOWANCE: usize = OFF_CPU + 8;
const OFF_TICK_HZ: usize = OFF_ALLOWANCE + 8;

/// The whole page's size in bytes: the magic, then three little-endian `u64`s, the CPU, the
/// allowance and the scheduler tick. Under one frame, the unit it is mapped as, and inside one
/// cache line, so the reader is a single load on a line nobody else writes.
pub const PAGE_BYTES: usize = OFF_TICK_HZ + 8;

/// The CPU word's offset in the page, for a reader that cannot call [`CurrentCpuPage::cpu`]:
/// hand-assembled machine code loading the word by absolute address.
///
/// Name: provisional, minted 2026-10-21 (UTC) by milestone 53 (the board's own peripherals)'s
/// receive-rate lane.
pub const CPU_WORD_OFFSET: u64 = OFF_CPU as u64;

/// How many threads of one process can each have a page: the slots below [`PAGE_VA`] in
/// `address_space_map::PROCESS_PAGES` (milestone 812 (`std::thread::spawn` runs real threads in one
/// address space), §269 (how threads share a process) fork 6). Slot `k` is at [`thread_page_va`]`(k)`;
/// slot 0 is [`PAGE_VA`] itself, so a process's first thread reads where every thread read before.
pub const THREAD_PAGE_SLOTS: usize =
    (address_space_map::PROCESS_PAGES.bytes() / address_space_map::PAGE) as usize;

/// Where thread page slot `k` is mapped: [`PAGE_VA`] for slot 0, and one page lower for each
/// slot after it, inside `PROCESS_PAGES` and so inside the stack's last-level table. `None` past
/// [`THREAD_PAGE_SLOTS`].
pub const fn thread_page_va(k: usize) -> Option<u64> {
    if k < THREAD_PAGE_SLOTS {
        Some(PAGE_VA - k as u64 * address_space_map::PAGE)
    } else {
        None
    }
}

/// The fixed virtual address the kernel maps this page at in every process, the same on all three
/// architectures: `address_space_map::CURRENT_CPU_PAGE`, the last page of `PROCESS_PAGES`. One page
/// above the stack's top page, it shares the stack's L1, L2 and L3 tables, so the mapping buys only
/// its leaf; a page alone in its tables cost 1,245 ticks per spawn (notes/benchmarks/spawn-el0.md).
/// The map is milestone 206 (a program image has under 896 KiB) and DECISIONS §171 (where a
/// program image starts) option D. See notes/current-cpu-page.md.
pub const PAGE_VA: u64 = address_space_map::CURRENT_CPU_PAGE;

/// Build a fresh page's bytes: the magic, and [`UNSCHEDULED`] for a thread that has not run yet.
/// The kernel writes this into a zeroed frame before mapping it read-only into the process, and
/// nothing else ever constructs one.
///
/// There is no `build_page(cpu)`: the CPU is not known at build time and is not the kind of thing
/// a builder should be able to assert. [`publish`] is the only way the word ever changes.
pub fn build_page() -> [u8; PAGE_BYTES] {
    build_page_allowing(0, 0)
}

/// [`build_page`], carrying the thread's **allowance**: how many cores its process may use
/// (milestone 812 (`std::thread::spawn` runs real threads in one address space), §269 (how threads
/// share a process) fork 7). Not how many the machine has: it holds the online count today, and a
/// later spawner grant or affinity budget lowers it without changing what it means. `tick_hz` is
/// the scheduler tick ([`CurrentCpuPage::tick_hz`]). Zero reads as unknown for either, which is
/// what [`build_page`] writes.
pub fn build_page_allowing(allowance: u64, tick_hz: u64) -> [u8; PAGE_BYTES] {
    let mut page = [0u8; PAGE_BYTES];
    page[OFF_MAGIC..OFF_MAGIC + 8].copy_from_slice(&MAGIC);
    page[OFF_CPU..OFF_CPU + 8].copy_from_slice(&UNSCHEDULED.to_le_bytes());
    page[OFF_ALLOWANCE..OFF_ALLOWANCE + 8].copy_from_slice(&allowance.to_le_bytes());
    page[OFF_TICK_HZ..OFF_TICK_HZ + 8].copy_from_slice(&tick_hz.to_le_bytes());
    page
}

/// Publish the core a thread is about to run on. The kernel calls it from the context switch with
/// the switching core's id. One `Relaxed` store, with no magic check, on the hottest path this
/// crate touches. It pairs with the scheduler's release/acquire handoff of the thread to the core,
/// which orders everything else about the move, so it needs no fence of its own.
///
/// # Safety
///
/// `page_va` must name a mapped, 8-byte-aligned buffer of at least [`PAGE_BYTES`] bytes that this
/// caller is the only writer of, live for the duration of the call. In the kernel that is the
/// direct-map view of a frame the address space owns and nothing else can reach.
#[inline]
pub unsafe fn publish(page_va: u64, cpu: u64) {
    // SAFETY: the caller's contract gives us a live, aligned, exclusively written buffer of at
    // least `PAGE_BYTES`, and `OFF_CPU + 8` is inside it.
    // `wrapping_add` because the wrap cannot happen and this runs on every context switch (release
    // builds check overflow, notes/overflow-checks.md): the contract says `page_va` starts a live
    // buffer of `PAGE_BYTES`, and no live buffer runs past the top of the address space.
    let word = unsafe { &*(page_va.wrapping_add(OFF_CPU as u64) as *const AtomicU64) };
    word.store(cpu, Ordering::Relaxed);
}

/// The current-CPU page, as seen through one thread's read-only mapping of it at [`PAGE_VA`].
///
/// # Examples
///
/// A built page reads back the core the kernel last published:
///
/// ```
/// use current_cpu_contract::{CurrentCpuPage, build_page, publish};
///
/// # #[repr(align(8))]
/// # struct Aligned([u8; current_cpu_contract::PAGE_BYTES]);
/// // The kernel's page is a frame; a test's must be given the frame's alignment by hand.
/// let mut bytes = Aligned(build_page());
/// let va = bytes.0.as_mut_ptr() as u64;
/// // SAFETY: `bytes` is a live, 8-aligned buffer of exactly `PAGE_BYTES` for this block, built
/// // by `build_page`, and this thread is its only writer.
/// unsafe { publish(va, 3) };
/// // SAFETY: as above.
/// let page = unsafe { CurrentCpuPage::new(va) };
/// assert_eq!(page.cpu(), Some(3));
/// ```
#[derive(Debug, Clone, Copy)]
pub struct CurrentCpuPage {
    base: *const u8,
}

// Deliberately neither `Send` nor `Sync`, unlike `counter_frequency_contract::TimebasePage`: this
// is this thread's answer, and carrying it to another thread carries a wrong one. The raw pointer
// makes the type neither by default. See notes/current-cpu-page.md.

impl CurrentCpuPage {
    /// Name the current-CPU page mapped at `va`.
    ///
    /// # Safety
    ///
    /// `va` must be a mapped, 8-byte-aligned buffer of at least [`PAGE_BYTES`] bytes (a page
    /// [`build_page`] wrote, or a zeroed frame, which reads as "unknown"), and it must stay mapped
    /// for as long as this value is used. The kernel maps exactly such a page, read-only, at
    /// [`PAGE_VA`]; see the crate's `BUGS` section for the shape that does not get one.
    pub const unsafe fn new(va: u64) -> Self {
        CurrentCpuPage {
            base: va as *const u8,
        }
    }

    /// The little-endian word at `off`, or `None` if this page is unrecognized (a frame nobody
    /// prepared). The magic is checked on every call rather than once at startup because the page
    /// is one cache line, so the check is in the line the answer is in and costs a compare rather
    /// than a miss.
    fn word(&self, off: usize) -> Option<u64> {
        // SAFETY: `new`'s contract: `base` names at least `PAGE_BYTES` mapped, stable bytes.
        let magic = unsafe { core::slice::from_raw_parts(self.base, 8) };
        if magic != MAGIC {
            return None;
        }
        // SAFETY: `new`'s contract again, plus its alignment clause: every word is 8-aligned
        // because `base` is and every offset is a multiple of 8. Reached through an integer rather
        // than by offsetting the `*const u8` so the cast is not one clippy has to take on trust,
        // which is the same shape `publish` uses on the writing side.
        let word = unsafe { &*((self.base as u64 + off as u64) as *const AtomicU64) };
        Some(word.load(Ordering::Relaxed))
    }

    /// How many cores this thread's process may use, `std::thread::available_parallelism`'s
    /// answer (milestone 812, §269 fork 7), or `None` for an unrecognized page or one built without
    /// an allowance. One load, never written after the page is built, so it needs no ordering.
    pub fn allowance(&self) -> Option<core::num::NonZeroUsize> {
        core::num::NonZeroUsize::new(self.word(OFF_ALLOWANCE)? as usize)
    }

    /// The scheduler tick's rate in hertz, and so the resolution of every timed wait, or
    /// `None` for an unrecognized page or one built without it. The kernel checks deadlines only
    /// from the tick (DECISIONS §147 (a timer a userspace service cannot hold)), so a `Timer::ARM`
    /// fires at the first tick at or after its deadline: a wait asked for 1 ms lasts up to
    /// `1 / tick_hz` s. The rate is the kernel's to choose and it publishes it here, the one page
    /// every process on every architecture has (calef, 2026-10-21 UTC, ruling C on #2489). One
    /// load, never written after the page is built.
    ///
    /// Name: provisional, minted 2026-10-21 (UTC) by milestone 53 (the board's own peripherals)'s
    /// receive-rate lane.
    pub fn tick_hz(&self) -> Option<core::num::NonZeroU64> {
        core::num::NonZeroU64::new(self.word(OFF_TICK_HZ)?)
    }

    /// Which CPU this thread is running on, or `None` if this page is unrecognized (a frame
    /// nobody prepared) or its thread has never been switched in. Never a fabricated 0.
    ///
    /// Two aligned loads and two comparisons, no syscall.
    pub fn cpu(&self) -> Option<usize> {
        match self.word(OFF_CPU)? {
            UNSCHEDULED => None,
            cpu => Some(cpu as usize),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `build_page`'s bytes at the alignment `publish` and `new` require; a bare `[u8; PAGE_BYTES]`
    /// is 1-aligned, and `script/undefined-behavior-check` (Miri) reports the unaligned
    /// `&AtomicU64`. Buffers are reached through `as_mut_ptr`, read-only tests included, because
    /// Miri refuses an `&AtomicU64` over an immutable binding (the crate's BUGS). Every call is a
    /// method that borrows the buffer for exactly that call, the one place the tests argue safety.
    #[repr(C, align(8))]
    struct Aligned([u8; PAGE_BYTES]);

    impl Aligned {
        fn va(&mut self) -> u64 {
            self.0.as_mut_ptr() as u64
        }

        fn publish(&mut self, cpu: u64) {
            // SAFETY: `self` is a live, 8-aligned buffer of exactly `PAGE_BYTES`, borrowed
            // mutably for the call, so this thread is its only writer.
            unsafe { publish(self.va(), cpu) }
        }

        /// [`CurrentCpuPage::cpu`] over this buffer. The page value never leaves the call, so it
        /// cannot outlive the buffer it names.
        fn cpu(&mut self) -> Option<usize> {
            // SAFETY: as `publish`; the page is read and dropped while `self` is borrowed.
            unsafe { CurrentCpuPage::new(self.va()) }.cpu()
        }

        /// [`CurrentCpuPage::allowance`] over this buffer, under `cpu`'s argument.
        fn allowance(&mut self) -> Option<core::num::NonZeroUsize> {
            // SAFETY: as `cpu`.
            unsafe { CurrentCpuPage::new(self.va()) }.allowance()
        }

        /// [`CurrentCpuPage::tick_hz`] over this buffer, under `cpu`'s argument.
        fn tick_hz(&mut self) -> Option<core::num::NonZeroU64> {
            // SAFETY: as `cpu`.
            unsafe { CurrentCpuPage::new(self.va()) }.tick_hz()
        }
    }

    fn aligned_page() -> Aligned {
        Aligned(build_page())
    }

    /// The round trip: a fresh page, a published core, the id read back the way a mapped frame
    /// would be named.
    #[test]
    fn a_published_cpu_reads_back() {
        let mut bytes = aligned_page();
        bytes.publish(2);
        assert_eq!(bytes.cpu(), Some(2));
    }

    /// **Zero is an answer, not an absence**, which is the whole reason the unset state is a magic
    /// and a sentinel rather than a zeroed word.
    #[test]
    fn cpu_zero_is_a_real_answer() {
        let mut bytes = aligned_page();
        bytes.publish(0);
        assert_eq!(bytes.cpu(), Some(0));
    }

    /// A frame nobody prepared reads as unknown, not as CPU 0.
    #[test]
    fn a_zeroed_frame_reads_as_unknown() {
        let mut zeroed = Aligned([0u8; PAGE_BYTES]);
        assert_eq!(zeroed.cpu(), None);
    }

    /// A prepared page whose thread has never been switched in reads as unknown, and the sentinel
    /// that makes that true is written by `build_page` rather than left to a zeroed frame.
    #[test]
    fn a_thread_that_never_ran_reads_as_unknown() {
        let mut bytes = aligned_page();
        assert_eq!(&bytes.0[OFF_CPU..OFF_CPU + 8], &UNSCHEDULED.to_le_bytes());
        assert_eq!(bytes.cpu(), None);
    }

    /// An unrecognized magic refuses the answer even when the word after it is a plausible core
    /// id: the magic check runs first, the discipline `counter_frequency_contract`'s equivalent
    /// test pins.
    #[test]
    fn an_unrecognized_magic_reads_as_unknown() {
        let mut bytes = aligned_page();
        bytes.0[OFF_MAGIC] ^= 0xff;
        bytes.publish(1);
        assert_eq!(bytes.cpu(), None);
    }

    /// Publishing twice is what a migration looks like from this page's side: the second write
    /// wins and nothing carries over from the first.
    #[test]
    fn a_migration_overwrites_rather_than_accumulates() {
        let mut bytes = aligned_page();
        bytes.publish(3);
        bytes.publish(1);
        assert_eq!(bytes.cpu(), Some(1));
    }

    /// Every CPU id this kernel can hand out survives the round trip, including the top of the
    /// range, so a sentinel chosen badly (say `CPU_ID_BOUND`) would fail here.
    #[test]
    fn every_id_in_the_bound_round_trips() {
        let mut bytes = aligned_page();
        for id in 0..CPU_ID_BOUND {
            bytes.publish(id as u64);
            assert_eq!(bytes.cpu(), Some(id));
        }
    }

    /// **The allowance reads back, and a page built without one has no answer** (milestone 812
    /// (`std::thread::spawn` runs real threads in one address space)), which is what keeps
    /// `available_parallelism` from fabricating a count on a page nobody gave one.
    #[test]
    fn the_allowance_reads_back_and_none_is_unknown() {
        let mut with = Aligned(build_page_allowing(4, 100));
        let mut without = aligned_page();
        assert_eq!(with.allowance().map(|n| n.get()), Some(4));
        assert_eq!(without.allowance(), None);
        assert_eq!(with.cpu(), None, "an allowance is not a CPU");
    }

    /// **The tick reads back from its own word, and a page built without one has no answer**, so
    /// a reader never divides a counter by a fabricated zero, and never takes the allowance for it.
    #[test]
    fn the_tick_reads_back_and_none_is_unknown() {
        let mut with = Aligned(build_page_allowing(4, 100));
        let mut without = Aligned(build_page_allowing(4, 0));
        assert_eq!(with.tick_hz().map(|n| n.get()), Some(100));
        assert_eq!(without.tick_hz(), None);
        let mut zeroed = aligned_page();
        assert_eq!(zeroed.tick_hz(), None);
    }

    /// Every thread page slot is inside `PROCESS_PAGES`, slot 0 is the page every thread read
    /// before 812, and there is no slot past the band.
    #[test]
    fn the_thread_page_slots_fill_the_process_pages_band() {
        assert_eq!(thread_page_va(0), Some(PAGE_VA));
        for k in 0..THREAD_PAGE_SLOTS {
            let va = thread_page_va(k).unwrap();
            assert!(address_space_map::PROCESS_PAGES.contains(va));
        }
        assert_eq!(thread_page_va(THREAD_PAGE_SLOTS), None);
        assert_eq!(THREAD_PAGE_SLOTS, 16);
        // One page apart, so no two threads share a slot's page.
        assert_eq!(thread_page_va(1), Some(PAGE_VA - address_space_map::PAGE));
    }

    /// The magic is the one every `std` already published in basalt compares: no nife-built
    /// reader would notice a change, since each takes its copy from this crate, and only the
    /// non-required `rg-threads` job runs a published one.
    #[test]
    fn the_magic_is_the_one_published_programs_compare() {
        assert_eq!(&MAGIC, b"CURRCPU1");
    }

    /// The layout constants do not overlap and the page fits in one frame and one cache line,
    /// pinned so a mutant swapping an offset is caught here rather than by a wrong core id on real
    /// hardware. The shape `counter_frequency_contract::the_layout_offsets_do_not_overlap` uses.
    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn the_layout_offsets_do_not_overlap() {
        assert_eq!(OFF_CPU, 8);
        assert_eq!(OFF_ALLOWANCE, 16);
        assert_eq!(OFF_TICK_HZ, 24);
        assert_eq!(CPU_WORD_OFFSET, 8);
        assert_eq!(PAGE_BYTES, 32);
        assert!(PAGE_BYTES <= 64, "the page must fit in one cache line");
        assert!(PAGE_BYTES < 4096, "the page must fit in one frame");
    }

    /// **The page shares the stack's last-level table**, which is what makes the mapping cost only
    /// its leaf. A 4 KiB-granule L3 covers 2 MiB on all three architectures, so two addresses share
    /// one when they agree above bit 21.
    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn the_page_shares_the_stacks_last_level_table() {
        assert_eq!(PAGE_VA % 4096, 0);
        assert!(address_space_map::PROCESS_PAGES.contains(PAGE_VA));
        assert_eq!(
            PAGE_VA >> 21,
            address_space_map::STACK_TOP_PAGE >> 21,
            "not in the stack's 2 MiB, so the mapping buys an L3 of its own"
        );
    }
}
