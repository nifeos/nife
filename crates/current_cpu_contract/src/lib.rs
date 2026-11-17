//! **The current-CPU page**: where a thread is running, read from memory rather than asked of the
//! kernel. The kernel maps one read-only page into every process, at [`PAGE_VIRTUAL_ADDRESS`], and
//! writes the id of the core into it at every switch-in, on the destination core. A reader in the
//! process loads the word with no system call. The page also carries two words the kernel writes
//! once when it builds the page: the thread's **allowance** (how many cores its process may use)
//! and the scheduler's tick rate. This crate is the page's one definition, for its one writer and
//! every reader.
//!
//! The writer's store and the reader's load are both `Relaxed`: an aligned 64-bit access does not
//! tear, and the scheduler's release/acquire handoff of a thread to a core orders everything about
//! a migration, so the page needs no fence of its own. A reader may be stale one instruction after
//! it reads, so a per-core cache built on the answer must stay correct when the thread has already
//! moved. Each word reads as [`None`] when it is unknown, never as a fabricated zero: for a frame
//! without the [`MAGIC`], for a thread that was never switched in (the CPU), and for a zero the
//! kernel never wrote (the allowance and the tick). The CPU word is an id, not an index into
//! `0..count`: size per-core arrays by [`CPU_ID_BOUND`].
//!
//! The layout grows only by appending a word that reads zero where a kernel never wrote it, so a
//! new field never changes the magic.
//!
//! # BUGS
//!
//! - The reader forms an `&AtomicU64` over a read-only mapping, a shape the Rust memory model has
//!   not settled; the fallback is a raw atomic load that forms no reference.
//! - A process has at most [`THREAD_PAGE_SLOTS`] threads, one page each.
//! - An address space never bound to a thread has no page, so a reader there faults.
//! - [`CPU_ID_BOUND`] sizes per-core arrays and does not enumerate the cores that exist.
//! - A consumer may copy this crate's single source file into a standard library: keep the crate to
//!   one file with fully qualified paths.
//!
//! Name: ratified 2026-10-18. `current_cpu` is the field's own word for this quantity; the suffix
//! follows its sibling `counter_frequency_contract`.

#![no_std]

#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeExample;

use core::num::{NonZeroU64, NonZeroUsize};
use core::sync::atomic::{AtomicU64, Ordering};

/// The page's first eight bytes: what tells a prepared page from a frame nobody has written. ASCII,
/// unpadded, the same shape every sibling page contract uses.
///
/// A published program's standard library carries its own copy of this value and compares it
/// before reading the CPU or the allowance: a program built against `CURRCPU1` reads a page under
/// any other magic as having no allowance and walks on one thread. The layout grows only by
/// appending a word that reads zero where a kernel never wrote it, so a new field never changes the
/// magic. A change to [`CPU_ID_BOUND`] does, and so does a layout break.
pub const MAGIC: [u8; 8] = *b"CURRCPU1";

/// The page layout's version, the digit [`MAGIC`] carries. A reader's check is the magic itself;
/// a break bumps this and re-mints the magic in one change.
///
/// Name: provisional.
pub const VERSION: u32 = 1;

/// This interface's identity in a program's interface note: 10, naming the current-CPU page. The
/// number is this interface's for good: no other interface takes it, and a format change is a new
/// version, never a new number. A program declares the pair of this id and [`VERSION`].
///
/// Name: provisional.
pub const INTERFACE_ID: u64 = 10;

/// What the CPU word holds before the kernel has ever switched this thread in. Not zero, because
/// zero is a valid CPU id, so the unset state cannot be a zeroed word.
///
/// # Examples
///
/// Neither a zeroed frame nor a thread that has never run fabricates an answer:
///
/// ```
/// use current_cpu_contract::{CurrentCpuPage, PAGE_CONTENT_BYTES, build_page};
///
/// # #[repr(align(8))]
/// # struct Aligned([u8; PAGE_CONTENT_BYTES]);
/// let mut zeroed = Aligned([0u8; PAGE_CONTENT_BYTES]);
/// // SAFETY: a live, aligned buffer of exactly `PAGE_CONTENT_BYTES` for this block.
/// assert_eq!(unsafe { CurrentCpuPage::new(zeroed.0.as_mut_ptr() as u64) }.cpu(), None);
///
/// let mut fresh = Aligned(build_page());
/// // SAFETY: as above.
/// assert_eq!(unsafe { CurrentCpuPage::new(fresh.0.as_mut_ptr() as u64) }.cpu(), None);
/// ```
pub const UNSCHEDULED: u64 = u64::MAX;

/// One past the highest CPU id a kernel will ever hand out: the size of a per-core array in
/// userspace. A kernel sizes its own per-core tables with this same constant. Not the online
/// count; see the crate's `BUGS` section.
///
/// **The bound is part of this page's identity.** The page's layout does not change when the bound
/// does, so a reader built with one bound would otherwise accept a page from a writer built with
/// another. Any change to this value therefore re-mints [`MAGIC`] and bumps [`VERSION`] in the
/// same change. The reader backs this up: [`CurrentCpuPage::cpu`] reads an id at or past the bound
/// as unknown.
///
/// # Examples
///
/// Sizing a per-core array, which is the consumer this page exists for:
///
/// ```
/// # use current_cpu_contract::{CurrentCpuPage, build_page, publish, CPU_ID_BOUND};
/// let mut caches = [0u64; CPU_ID_BOUND]; // never `[0; cores_i_have_seen]`
/// # #[repr(align(8))]
/// # struct Aligned([u8; current_cpu_contract::PAGE_CONTENT_BYTES]);
/// # let mut bytes = Aligned(build_page());
/// # let page_virtual_address = bytes.0.as_mut_ptr() as u64;
/// # // SAFETY: as in `CurrentCpuPage`'s example.
/// # unsafe { publish(page_virtual_address, 3) };
/// # // SAFETY: as in `CurrentCpuPage`'s example.
/// # let page = unsafe { CurrentCpuPage::new(page_virtual_address) };
/// if let Some(cpu) = page.cpu() {
///     caches[cpu] += 1;
/// }
/// ```
pub const CPU_ID_BOUND: usize = 8;

const MAGIC_OFFSET_BYTES: usize = 0;

/// The CPU word's offset in the page, for a reader that cannot call [`CurrentCpuPage::cpu`]:
/// hand-assembled machine code loading the word by absolute address. A `u64`, so it adds to an
/// address without a cast.
///
/// Name: provisional.
pub const CPU_OFFSET_BYTES: u64 = MAGIC_OFFSET_BYTES as u64 + 8;

const CPU_OFFSET: usize = CPU_OFFSET_BYTES as usize;
const ALLOWANCE_OFFSET_BYTES: usize = CPU_OFFSET + 8;
const TICK_HERTZ_OFFSET_BYTES: usize = ALLOWANCE_OFFSET_BYTES + 8;

/// The whole page's size in bytes: the magic, then three little-endian `u64`s, the CPU, the
/// allowance and the scheduler tick. Under one frame, the unit it is mapped as, and inside one
/// cache line, so the reader is a single load on a line nobody else writes.
///
/// Name: provisional. The bytes are the page's content, not the whole page it is mapped as.
pub const PAGE_CONTENT_BYTES: usize = TICK_HERTZ_OFFSET_BYTES + 8;

/// How many threads of one process can each have a page: the slots below
/// [`PAGE_VIRTUAL_ADDRESS`] in `address_space_layout::PROCESS_PAGES`. Slot `k` is at
/// [`thread_page_virtual_address`]`(k)`; slot 0 is [`PAGE_VIRTUAL_ADDRESS`] itself, so a process's
/// first thread reads where every thread read before slots existed.
pub const THREAD_PAGE_SLOTS: usize =
    (address_space_layout::PROCESS_PAGES.bytes() / address_space_layout::PAGE) as usize;

/// Where thread page slot `k` is mapped: [`PAGE_VIRTUAL_ADDRESS`] for slot 0, and one page lower
/// for each slot after it, inside `PROCESS_PAGES` and so inside the stack's last-level table.
/// `None` past [`THREAD_PAGE_SLOTS`].
///
/// Name: provisional.
pub const fn thread_page_virtual_address(k: usize) -> Option<u64> {
    if k < THREAD_PAGE_SLOTS {
        Some(PAGE_VIRTUAL_ADDRESS - k as u64 * address_space_layout::PAGE)
    } else {
        None
    }
}

/// The fixed virtual address the kernel maps this page at in every process, the same on all three
/// architectures: `address_space_layout::CURRENT_CPU_PAGE`, the last page of `PROCESS_PAGES`. One
/// page above the stack's top page, it shares the stack's three upper-level page tables, so the
/// mapping costs only its leaf entry; a page alone in its tables costs a table frame at each level
/// on every spawn.
///
/// Name: provisional.
pub const PAGE_VIRTUAL_ADDRESS: u64 = address_space_layout::CURRENT_CPU_PAGE;

/// Build a fresh page's bytes: the magic, and [`UNSCHEDULED`] for a thread that has not run yet,
/// with no allowance and no tick. The kernel writes this into a zeroed frame before mapping it
/// read-only into the process.
///
/// There is no `build_page(cpu)`: the CPU is not known at build time and is not the kind of thing
/// a builder should be able to assert. [`publish`] is the only way the word ever changes.
pub fn build_page() -> [u8; PAGE_CONTENT_BYTES] {
    build_page_with(None, None)
}

/// [`build_page`], carrying the thread's **allowance**: how many cores its process may use. Not
/// how many the machine has: it holds the online count today, and a later grant or affinity budget
/// lowers it without changing what it means. `tick_hertz` is the scheduler tick
/// ([`CurrentCpuPage::tick_hertz`]). [`None`] for either is the unknown the readers answer, which
/// is what [`build_page`] writes.
///
/// Name: provisional.
pub fn build_page_with(
    allowance: Option<NonZeroU64>,
    tick_hertz: Option<NonZeroU64>,
) -> [u8; PAGE_CONTENT_BYTES] {
    let mut page = [0u8; PAGE_CONTENT_BYTES];
    page[MAGIC_OFFSET_BYTES..MAGIC_OFFSET_BYTES + 8].copy_from_slice(&MAGIC);
    page[CPU_OFFSET..CPU_OFFSET + 8].copy_from_slice(&UNSCHEDULED.to_le_bytes());
    page[ALLOWANCE_OFFSET_BYTES..ALLOWANCE_OFFSET_BYTES + 8]
        .copy_from_slice(&allowance.map_or(0, NonZeroU64::get).to_le_bytes());
    page[TICK_HERTZ_OFFSET_BYTES..TICK_HERTZ_OFFSET_BYTES + 8]
        .copy_from_slice(&tick_hertz.map_or(0, NonZeroU64::get).to_le_bytes());
    page
}

/// Publish the core a thread is about to run on. The kernel calls it from the context switch with
/// the switching core's id. One `Relaxed` store, with no magic check, on the hottest path this
/// crate touches. It pairs with the scheduler's release/acquire handoff of the thread to the core,
/// which orders everything else about the move, so it needs no fence of its own.
///
/// # Safety
///
/// `page_virtual_address` must name a mapped, 8-byte-aligned buffer of at least
/// [`PAGE_CONTENT_BYTES`] bytes that this caller is the only writer of, live for the duration of
/// the call. In a kernel that is its direct-map view of a frame the address space owns and nothing
/// else can reach.
///
/// Name: provisional (the parameter).
#[inline]
pub unsafe fn publish(page_virtual_address: u64, cpu: u64) {
    debug_assert!(
        page_virtual_address.is_multiple_of(8),
        "the page is 8-aligned"
    );
    debug_assert!(cpu != UNSCHEDULED, "publishing the sentinel un-publishes");
    debug_assert!(cpu < CPU_ID_BOUND as u64, "the id is inside the bound");
    // SAFETY: the caller's contract gives us a live, aligned, exclusively written buffer of at
    // least `PAGE_CONTENT_BYTES`, and `CPU_OFFSET_BYTES + 8` is inside it.
    // `wrapping_add` because the wrap cannot happen and this runs on every context switch (release
    // builds check overflow): the contract says `page_virtual_address` starts a live buffer of
    // `PAGE_CONTENT_BYTES`, and no live buffer runs past the top of the address space.
    let word =
        unsafe { &*(page_virtual_address.wrapping_add(CPU_OFFSET_BYTES) as *const AtomicU64) };
    word.store(cpu, Ordering::Relaxed);
}

/// The current-CPU page, as seen through one thread's read-only mapping of it at
/// [`PAGE_VIRTUAL_ADDRESS`].
///
/// # Examples
///
/// A built page reads back the core the kernel last published:
///
/// ```
/// use current_cpu_contract::{CurrentCpuPage, build_page, publish};
///
/// # #[repr(align(8))]
/// # struct Aligned([u8; current_cpu_contract::PAGE_CONTENT_BYTES]);
/// // The kernel's page is a frame; a test's must be given the frame's alignment by hand.
/// let mut bytes = Aligned(build_page());
/// let page_virtual_address = bytes.0.as_mut_ptr() as u64;
/// // SAFETY: `bytes` is a live, 8-aligned buffer of exactly `PAGE_CONTENT_BYTES` for this block,
/// // built by `build_page`, and this thread is its only writer.
/// unsafe { publish(page_virtual_address, 3) };
/// // SAFETY: as above.
/// let page = unsafe { CurrentCpuPage::new(page_virtual_address) };
/// assert_eq!(page.cpu(), Some(3));
/// ```
#[derive(Debug, Clone, Copy)]
pub struct CurrentCpuPage {
    base: *const u8,
}

// Deliberately neither `Send` nor `Sync`, unlike `counter_frequency_contract::CounterFrequencyPage`:
// this is this thread's answer, and carrying it to another thread carries a wrong one. The raw
// pointer makes the type neither by default.

impl CurrentCpuPage {
    /// Name the current-CPU page mapped at `virtual_address`.
    ///
    /// # Safety
    ///
    /// `virtual_address` must be a mapped, 8-byte-aligned buffer of at least
    /// [`PAGE_CONTENT_BYTES`] bytes (a page [`build_page`] wrote, or a zeroed frame, which reads as
    /// "unknown"), and it must stay mapped for as long as this value is used. A kernel maps exactly
    /// such a page, read-only, at [`PAGE_VIRTUAL_ADDRESS`]; see the crate's `BUGS` section for the
    /// shape that does not get one.
    ///
    /// Name: provisional (the parameter).
    pub const unsafe fn new(virtual_address: u64) -> Self {
        debug_assert!(virtual_address.is_multiple_of(8), "the page is 8-aligned");
        CurrentCpuPage {
            base: virtual_address as *const u8,
        }
    }

    /// The little-endian word at `off`, or `None` if this page is unrecognized (a frame nobody
    /// prepared). The magic is checked on every call rather than once at startup because the page
    /// is one cache line, so the check is in the line the answer is in and costs a compare rather
    /// than a miss.
    fn word(&self, off: usize) -> Option<u64> {
        // SAFETY: `new`'s contract: `base` names at least `PAGE_CONTENT_BYTES` mapped, stable bytes.
        let magic = unsafe { core::slice::from_raw_parts(self.base, 8) };
        if magic != MAGIC {
            return None;
        }
        // SAFETY: `new`'s contract again, plus its alignment clause: every word is 8-aligned
        // because `base` is and every offset is a multiple of 8. The offset keeps the pointer's
        // provenance, so a checker can tie the load to the buffer it came from.
        #[allow(clippy::cast_ptr_alignment)]
        let word = unsafe { &*self.base.add(off).cast::<AtomicU64>() };
        Some(word.load(Ordering::Relaxed))
    }

    /// How many cores this thread's process may use (the answer a standard library gives for
    /// available parallelism), or `None` for an unrecognized page or one built without an
    /// allowance. An allowance the target cannot hold reads as unknown, never as a truncated count.
    /// One load, never written after the page is built, so it needs no ordering.
    pub fn allowance(&self) -> Option<NonZeroUsize> {
        usize::try_from(self.word(ALLOWANCE_OFFSET_BYTES)?)
            .ok()
            .and_then(NonZeroUsize::new)
    }

    /// The scheduler tick's rate in hertz, and so the resolution of every timed wait, or `None` for
    /// an unrecognized page or one built without it. A kernel that checks deadlines only from its
    /// tick fires a timer at the first tick at or after its deadline: a wait asked for 1 ms lasts
    /// up to `1 / tick_hertz` seconds. The rate is the kernel's to choose and it publishes it here,
    /// the one page every process on every architecture has. One load, never written after the page
    /// is built.
    ///
    /// Name: provisional.
    pub fn tick_hertz(&self) -> Option<NonZeroU64> {
        NonZeroU64::new(self.word(TICK_HERTZ_OFFSET_BYTES)?)
    }

    /// Which CPU this thread is running on, or `None` if this page is unrecognized (a frame
    /// nobody prepared), its thread has never been switched in, or the published id is one this
    /// crate's bound cannot index. Never a fabricated 0.
    ///
    /// An id at or past [`CPU_ID_BOUND`] reads as unknown, so the answer always indexes an array
    /// of `CPU_ID_BOUND` elements.
    ///
    /// Two aligned loads and three comparisons, no syscall.
    pub fn cpu(&self) -> Option<usize> {
        match self.word(CPU_OFFSET)? {
            UNSCHEDULED => None,
            cpu if cpu >= CPU_ID_BOUND as u64 => None,
            cpu => Some(cpu as usize),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `build_page`'s bytes at the alignment `publish` and `new` require; a bare
    /// `[u8; PAGE_CONTENT_BYTES]` is 1-aligned, and Miri reports the unaligned `&AtomicU64`.
    /// Buffers are reached through `as_mut_ptr`, read-only tests included, because Miri refuses an
    /// `&AtomicU64` over an immutable binding (the crate's BUGS). Every call is a
    /// method that borrows the buffer for exactly that call, the one place the tests argue safety.
    #[repr(C, align(8))]
    struct Aligned([u8; PAGE_CONTENT_BYTES]);

    impl Aligned {
        fn va(&mut self) -> u64 {
            self.0.as_mut_ptr() as u64
        }

        /// Store `word` in the CPU word the way a writer built with another bound or an older
        /// layout could, which `publish` refuses in a debug build.
        fn write_cpu_word(&mut self, word: u64) {
            let at = CPU_OFFSET;
            self.0[at..at + 8].copy_from_slice(&word.to_le_bytes());
        }

        fn publish(&mut self, cpu: u64) {
            // SAFETY: `self` is a live, 8-aligned buffer of exactly `PAGE_CONTENT_BYTES`, borrowed
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

        /// [`CurrentCpuPage::tick_hertz`] over this buffer, under `cpu`'s argument.
        fn tick_hertz(&mut self) -> Option<core::num::NonZeroU64> {
            // SAFETY: as `cpu`.
            unsafe { CurrentCpuPage::new(self.va()) }.tick_hertz()
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
        let mut zeroed = Aligned([0u8; PAGE_CONTENT_BYTES]);
        assert_eq!(zeroed.cpu(), None);
    }

    /// A prepared page whose thread has never been switched in reads as unknown, and the sentinel
    /// that makes that true is written by `build_page` rather than left to a zeroed frame.
    #[test]
    fn a_thread_that_never_ran_reads_as_unknown() {
        let mut bytes = aligned_page();
        assert_eq!(
            &bytes.0[CPU_OFFSET..CPU_OFFSET + 8],
            &UNSCHEDULED.to_le_bytes()
        );
        assert_eq!(bytes.cpu(), None);
    }

    /// An unrecognized magic refuses the answer even when the word after it is a plausible core
    /// id: the magic check runs first, the discipline `counter_frequency_contract`'s equivalent
    /// test pins.
    #[test]
    fn an_unrecognized_magic_reads_as_unknown() {
        let mut bytes = aligned_page();
        bytes.0[MAGIC_OFFSET_BYTES] ^= 0xff;
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

    /// An id one below the bound is accepted and an id at the bound is refused, so a writer built
    /// with a larger bound degrades to "unknown" instead of handing out an index past the array.
    #[test]
    fn an_id_past_the_bound_reads_as_unknown() {
        let mut bytes = aligned_page();
        bytes.publish(CPU_ID_BOUND as u64 - 1);
        assert_eq!(bytes.cpu(), Some(CPU_ID_BOUND - 1));
        bytes.write_cpu_word(CPU_ID_BOUND as u64);
        assert_eq!(bytes.cpu(), None);
        bytes.write_cpu_word(CPU_ID_BOUND as u64 + 1);
        assert_eq!(bytes.cpu(), None);
        bytes.write_cpu_word(UNSCHEDULED);
        assert_eq!(bytes.cpu(), None);
    }

    /// **The allowance reads back, and a page built without one has no answer**, which is what keeps
    /// a standard library's available parallelism from fabricating a count on a page nobody gave one.
    #[test]
    fn the_allowance_reads_back_and_none_is_unknown() {
        let mut with = Aligned(build_page_with(NonZeroU64::new(4), NonZeroU64::new(100)));
        let mut without = aligned_page();
        assert_eq!(with.allowance().map(|n| n.get()), Some(4));
        assert_eq!(without.allowance(), None);
        assert_eq!(with.cpu(), None, "an allowance is not a CPU");
    }

    /// **The tick reads back from its own word, and a page built without one has no answer**, so
    /// a reader never divides a counter by a fabricated zero, and never takes the allowance for it.
    #[test]
    fn the_tick_reads_back_and_none_is_unknown() {
        let mut with = Aligned(build_page_with(NonZeroU64::new(4), NonZeroU64::new(100)));
        let mut without = Aligned(build_page_with(NonZeroU64::new(4), None));
        assert_eq!(with.tick_hertz().map(|n| n.get()), Some(100));
        assert_eq!(without.tick_hertz(), None);
        let mut zeroed = aligned_page();
        assert_eq!(zeroed.tick_hertz(), None);
    }

    /// Every thread page slot is inside `PROCESS_PAGES`, slot 0 is the page every thread read
    /// before 812, and there is no slot past the band.
    #[test]
    fn the_thread_page_slots_fill_the_process_pages_band() {
        assert_eq!(thread_page_virtual_address(0), Some(PAGE_VIRTUAL_ADDRESS));
        for k in 0..THREAD_PAGE_SLOTS {
            let va = thread_page_virtual_address(k).unwrap();
            assert!(address_space_layout::PROCESS_PAGES.contains(va));
        }
        assert_eq!(thread_page_virtual_address(THREAD_PAGE_SLOTS), None);
        assert_eq!(THREAD_PAGE_SLOTS, 16);
        // One page apart, so no two threads share a slot's page.
        assert_eq!(
            thread_page_virtual_address(1),
            Some(PAGE_VIRTUAL_ADDRESS - address_space_layout::PAGE)
        );
    }

    /// The magic is the one every published program's standard library compares, so a change to it
    /// is a break that no reader built from this crate would notice, since each takes its copy from
    /// here: only a program built earlier does.
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
        assert_eq!(CPU_OFFSET_BYTES, 8);
        assert_eq!(ALLOWANCE_OFFSET_BYTES, 16);
        assert_eq!(TICK_HERTZ_OFFSET_BYTES, 24);
        assert_eq!(PAGE_CONTENT_BYTES, 32);
        assert!(
            PAGE_CONTENT_BYTES <= 64,
            "the page must fit in one cache line"
        );
        assert!(PAGE_CONTENT_BYTES < 4096, "the page must fit in one frame");
    }

    /// **The page shares the stack's last-level table**, which is what makes the mapping cost only
    /// its leaf. A 4 KiB-granule L3 covers 2 MiB on all three architectures, so two addresses share
    /// one when they agree above bit 21.
    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn the_page_shares_the_stacks_last_level_table() {
        assert_eq!(PAGE_VIRTUAL_ADDRESS % 4096, 0);
        assert!(address_space_layout::PROCESS_PAGES.contains(PAGE_VIRTUAL_ADDRESS));
        assert_eq!(
            PAGE_VIRTUAL_ADDRESS >> 21,
            address_space_layout::STACK_TOP_PAGE >> 21,
            "not in the stack's 2 MiB, so the mapping buys an L3 of its own"
        );
    }
}

/// Machine-checked proofs, run with `cargo kani`. The reader's decisions are the magic compare, the
/// sentinel and the bound; the writer's one promise is that the reader gets its words back. No proof
/// covers the caller's promise that the address names a mapped page, which Kani cannot see.
#[cfg(kani)]
mod verification {
    use super::*;

    /// A frame at the alignment the reader requires.
    #[repr(C, align(8))]
    struct Aligned([u8; PAGE_CONTENT_BYTES]);

    fn word(bytes: &[u8; PAGE_CONTENT_BYTES], at: usize) -> u64 {
        let mut raw = [0u8; 8];
        raw.copy_from_slice(&bytes[at..at + 8]);
        u64::from_le_bytes(raw)
    }

    /// Falsification: replayable `crates/current_cpu_contract/falsifications/verification.the_readers_answer_only_what_the_page_holds.patch`
    /// **Each reader answers `Some` exactly when the magic matches and its word is a real answer**,
    /// over every 32-byte frame: the CPU when it is neither the sentinel nor at or past the bound,
    /// the allowance and the tick when they are nonzero.
    #[kani::proof]
    fn the_readers_answer_only_what_the_page_holds() {
        let mut frame = Aligned(kani::any());
        let bytes = frame.0;
        // SAFETY: `frame` is a live, 8-aligned buffer of exactly `PAGE_CONTENT_BYTES`.
        let page = unsafe { CurrentCpuPage::new(frame.0.as_mut_ptr() as u64) };
        let magic_matches = bytes[..8] == MAGIC;
        let cpu = word(&bytes, CPU_OFFSET);
        let allowance = word(&bytes, ALLOWANCE_OFFSET_BYTES);
        let tick = word(&bytes, TICK_HERTZ_OFFSET_BYTES);
        if magic_matches && cpu != UNSCHEDULED && cpu < CPU_ID_BOUND as u64 {
            assert_eq!(page.cpu(), Some(cpu as usize));
        } else {
            assert_eq!(page.cpu(), None);
        }
        if magic_matches && allowance != 0 {
            assert_eq!(page.allowance().map(|n| n.get() as u64), Some(allowance));
        } else {
            assert_eq!(page.allowance(), None);
        }
        if magic_matches && tick != 0 {
            assert_eq!(page.tick_hertz().map(NonZeroU64::get), Some(tick));
        } else {
            assert_eq!(page.tick_hertz(), None);
        }
    }

    /// Falsification: replayable `crates/current_cpu_contract/falsifications/verification.a_built_page_reads_back_its_words.patch`
    /// **A built page reads back its allowance and tick, and reads the CPU as unknown**, for every
    /// pair of values, absent or not.
    #[kani::proof]
    fn a_built_page_reads_back_its_words() {
        let allowance = NonZeroU64::new(kani::any());
        let tick = NonZeroU64::new(kani::any());
        let mut frame = Aligned(build_page_with(allowance, tick));
        // SAFETY: `frame` is a live, 8-aligned buffer of exactly `PAGE_CONTENT_BYTES`.
        let page = unsafe { CurrentCpuPage::new(frame.0.as_mut_ptr() as u64) };
        assert_eq!(
            page.allowance().map(|n| n.get() as u64),
            allowance.map(NonZeroU64::get)
        );
        assert_eq!(page.tick_hertz(), tick);
        assert_eq!(page.cpu(), None);
    }
}
