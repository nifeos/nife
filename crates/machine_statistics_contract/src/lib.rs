//! **The machine statistics page: how the machine is doing, held as a capability rather than
//! ambient.**
//!
//! One read-only page in which a kernel keeps its machine-wide counters: physical frames, and per
//! cpu the ticks spent busy and idle, the context switches, the interrupts taken and the run queue.
//! This crate is the one definition of where each word lives, so the kernel (the one writer) and
//! every reader agree through a dependency and not through a copy of the layout. The page is
//! granted to a program, never ambient.
//!
//! Each word is an `AtomicU64` the writer updates in place, one writer per word, with relaxed
//! stores and no sequence counter. A reader loads the words one at a time, so **the words are not
//! a consistent snapshot of each other**: a sum of busy and idle ticks may mix two instants. The
//! design is deliberate. A reader that needs only each word's own value never waits for the
//! writer, and the page needs no lock a read-only mapping could not take.
//!
//! # Layout
//!
//! The page is [`WORDS`] words of eight bytes: a header line of [`word::LINE`] words, then one such
//! line per possible cpu. The header holds the magic at word 0, the frame size at 1, the tick rate
//! at 2, the total and free frame counts at 3 and 4, and the version at 5. Words 6 and 7 of the
//! header and words 6 and 7 of every cpu line are reserved and read zero. A later writer may assign
//! one under the same [`VERSION`] only if a reader that does not know it is still right to read
//! zero there; any other change bumps [`VERSION`] and re-mints [`MAGIC`]'s digit in one change.
//!
//! # BUGS
//!
//! - The words are not consistent with each other, and no sequence counter lets a reader detect
//!   a torn read.
//! - Busy time is not split into user and system time.
//! - A sample's age is bounded by the writer's tick and not by the page: a counter the writer
//!   publishes once per tick is up to one tick old, and a parked cpu keeps its last sample.
//! - The line count is fixed at [`CPU_ID_BOUND`]; a machine with more cpus does not fit.
//! - There is no swap, page-cache or block I/O counter.
//!
//! Name: ratified 2026-09-27 (calef, #1360's table), for the crate and for "the machine statistics
//! page". §225 called it the machine memory page; it carries the scheduler's counters too, because
//! `vmstat` needs them and a second page would be a second grant for one question.

#![no_std]

use core::sync::atomic::{AtomicU64, Ordering};

/// The bound on cpu ids, re-exported from the current-cpu page. This page's layout depends on it
/// (see [`WORDS`]), so its identity follows the same rule as that page's: any change to the value
/// re-mints [`MAGIC`] and bumps [`VERSION`] in the same change.
pub use current_cpu_contract::CPU_ID_BOUND;

/// The page's first word: what tells a prepared page from a frame nobody has written.
pub const MAGIC: [u8; 8] = *b"MACHSTA1";

/// The page layout's version, written into the header line at [`word::VERSION`] and checked by
/// every reader before it trusts any other word. A break bumps this and re-mints [`MAGIC`]'s digit
/// in the same change; a shared page serves one generation, so one word carries it whole. The
/// line count ([`CPU_ID_BOUND`]) is part of what the version names.
///
/// Name: provisional, minted 2026-10-29 (UTC) by lane milestone/956-checkable-interface-versions.
pub const VERSION: u32 = 1;

/// This interface's identity in a program's `INTERFACES` note: 4, naming the machine statistics
/// page. The number is the crate's to keep across a rename and no other interface ever reuses it. A
/// program declares the pair of this id and [`VERSION`].
///
/// Name: provisional, minted 2026-10-31 (UTC) by lane milestone/956-checkable-interface-versions.
pub const INTERFACE_ID: u64 = 4;

/// The words the page carries: one header line, then one line per possible cpu.
pub const WORDS: usize = word::LINE * (1 + CPU_ID_BOUND);

/// The page's size in bytes. Far under one frame, which is the unit it is mapped as.
pub const PAGE_BYTES: usize = WORDS * 8;

const _: () = assert!(
    PAGE_BYTES <= 4096,
    "the page must fit the frame it is mapped as"
);
const _: () = assert!(
    PAGE_BYTES == core::mem::size_of::<[u64; WORDS]>(),
    "the page is exactly the words the kernel writes, eight bytes each"
);
const _: () = assert!(
    PAGE_VA.is_multiple_of(4096),
    "a mapping starts on a page boundary"
);

/// **Where a child that declares `machine` finds the page**, read-only. A pair page on the
/// address-space map (`address_space_map::pair_page`): the program that maps the page and the one
/// child it builds agree on the address, the way they agree on the clock page's address. Under the
/// map it costs what any pair page does.
///
/// Name: ratified 2026-09-27 (calef, #1360's table).
pub const PAGE_VA: u64 = address_space_map::pair_page(0x005F_F000);

/// **Word indices into the page.** A per-cpu word is `cpu(id) + OFFSET`.
pub mod word {
    /// Words per 64-byte line.
    pub const LINE: usize = 8;

    /// [`MAGIC`](super::MAGIC), as a little-endian word.
    pub const MAGIC: usize = 0;
    /// Bytes per frame, so a reader can print bytes without assuming a page size.
    pub const FRAME_BYTES: usize = 1;
    /// How many ticks make a second, so a reader can turn tick counts into time without a clock.
    pub const TICK_HERTZ: usize = 2;
    /// Frames the allocator tracks in total.
    pub const TOTAL_FRAMES: usize = 3;
    /// Frames free right now.
    pub const FREE_FRAMES: usize = 4;
    /// The page layout's version, [`VERSION`](super::VERSION) as a word. A reader that finds the
    /// magic but not its version refuses the page.
    ///
    /// Name: provisional, minted 2026-10-29 (UTC) by lane milestone/956-checkable-interface-versions.
    pub const VERSION: usize = 5;

    /// The first word of cpu `id`'s line. `id` is a cpu id, a name and not an index into the
    /// online set.
    #[must_use]
    pub const fn cpu(id: usize) -> usize {
        LINE * (1 + id)
    }

    /// 1 once the cpu has taken a tick. A cpu never seen stays 0.
    pub const ONLINE: usize = 0;
    /// Ticks this cpu spent running something other than its idle thread.
    pub const BUSY_TICKS: usize = 1;
    /// Ticks this cpu spent in its idle thread.
    pub const IDLE_TICKS: usize = 2;
    /// Context switches this cpu has made.
    pub const CONTEXT_SWITCHES: usize = 3;
    /// Interrupts this cpu has taken: timer, cross-cpu and device.
    pub const INTERRUPTS: usize = 4;
    /// Threads runnable on this cpu at its last tick, counting the running one unless it is idle.
    pub const RUNNABLE: usize = 5;
}

/// Build a fresh page's header: the magic, the version, the frame size and the tick rate. The
/// writer puts this into a zeroed frame once, before any counter moves.
#[must_use]
pub fn build_header(frame_bytes: u64, tick_hertz: u64) -> [u64; word::LINE] {
    let mut line = [0u64; word::LINE];
    line[word::MAGIC] = u64::from_le_bytes(MAGIC);
    line[word::VERSION] = u64::from(VERSION);
    line[word::FRAME_BYTES] = frame_bytes;
    line[word::TICK_HERTZ] = tick_hertz;
    line
}

/// **One word of the page, for its one writer.** Every access is relaxed, and the reason is in the
/// crate docs: each word has one writer and no reader needs the words consistent with each other.
///
/// The reference lives for `'a`, which the caller chooses and must not let outlast the mapping. A
/// reader holding a read-only mapping may only load through it, and only with
/// [`Ordering::Relaxed`]: the standard library guarantees nothing else for atomic access to
/// read-only memory. A store or a read-modify-write through a read-only page faults.
///
/// # Safety
///
/// `page_va` must name a mapped, 8-byte-aligned buffer of at least [`PAGE_BYTES`] bytes that stays
/// mapped for `'a`, and `index` must be under [`WORDS`].
#[inline]
pub unsafe fn counter<'a>(page_va: u64, index: usize) -> &'a AtomicU64 {
    // The address is a `u64` and must become a pointer: refuse a target whose pointer is narrower.
    const {
        assert!(
            usize::BITS >= u64::BITS,
            "the page address must fit a pointer"
        );
    };
    let word_virtual_address = (page_va + (index as u64) * 8) as usize;
    // SAFETY: the caller's contract: in bounds, aligned, mapped for `'a`.
    unsafe { AtomicU64::from_ptr(word_virtual_address as *mut u64) }
}

/// One cpu's line, as a reader sees it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cpu {
    /// The cpu has taken at least one tick.
    pub online: bool,
    /// Ticks spent running something other than the idle thread.
    pub busy_ticks: u64,
    /// Ticks spent idle.
    pub idle_ticks: u64,
    /// Context switches.
    pub context_switches: u64,
    /// Interrupts taken.
    pub interrupts: u64,
    /// Runnable threads at the last tick.
    pub runnable: u64,
}

/// **The page, read once.** Each field is one word as it stood when it was read; see the crate docs
/// for why the fields are not consistent with each other. It is a 416-byte `Copy` value, copied
/// once by a reader and then passed by reference.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Snapshot {
    /// Bytes per frame.
    pub frame_bytes: u64,
    /// Ticks per second.
    pub tick_hertz: u64,
    /// Frames the allocator tracks.
    pub total_frames: u64,
    /// Frames free.
    pub free_frames: u64,
    /// Every possible cpu's line, indexed by cpu id.
    pub cpus: [Cpu; CPU_ID_BOUND],
}

impl Snapshot {
    /// Read a page from its words, or `None` if the magic or the version is missing: a frame nobody
    /// prepared reads as zeroes, and zero free memory is a plausible, wrong answer.
    ///
    /// # Examples
    ///
    /// A reader over a page a kernel filled, here built on the host from words:
    ///
    /// ```
    /// use machine_statistics_contract::{Snapshot, VERSION, WORDS, word};
    ///
    /// let mut words = [0u64; WORDS];
    /// words[word::MAGIC] = u64::from_le_bytes(*b"MACHSTA1");
    /// words[word::VERSION] = u64::from(VERSION);
    /// words[word::FRAME_BYTES] = 4096;
    /// words[word::TICK_HERTZ] = 100;
    /// words[word::TOTAL_FRAMES] = 32768;
    /// words[word::FREE_FRAMES] = 30000;
    /// words[word::cpu(0) + word::ONLINE] = 1;
    /// words[word::cpu(0) + word::BUSY_TICKS] = 25;
    /// words[word::cpu(0) + word::IDLE_TICKS] = 75;
    ///
    /// let s = Snapshot::from_words(&words).expect("a page the kernel prepared");
    /// assert_eq!(s.total_bytes(), 32768 * 4096);
    /// assert_eq!(s.online_cpus(), 1);
    /// assert_eq!(s.busy_ticks(), 25);
    ///
    /// // A frame nobody prepared has no answer, rather than an answer of zero.
    /// assert!(Snapshot::from_words(&[0u64; WORDS]).is_none());
    /// ```
    #[must_use]
    pub fn from_words(words: &[u64; WORDS]) -> Option<Snapshot> {
        if words[word::MAGIC] != u64::from_le_bytes(MAGIC) {
            return None;
        }
        // The version check beside the magic, before any counter word is trusted: a page written
        // by another generation reads as no page at all rather than as words in the wrong places.
        if words[word::VERSION] != u64::from(VERSION) {
            return None;
        }
        let mut cpus = [Cpu::default(); CPU_ID_BOUND];
        for (id, c) in cpus.iter_mut().enumerate() {
            let at = |offset: usize| words[word::cpu(id) + offset];
            *c = Cpu {
                online: at(word::ONLINE) != 0,
                busy_ticks: at(word::BUSY_TICKS),
                idle_ticks: at(word::IDLE_TICKS),
                context_switches: at(word::CONTEXT_SWITCHES),
                interrupts: at(word::INTERRUPTS),
                runnable: at(word::RUNNABLE),
            };
        }
        Some(Snapshot {
            frame_bytes: words[word::FRAME_BYTES],
            tick_hertz: words[word::TICK_HERTZ],
            total_frames: words[word::TOTAL_FRAMES],
            free_frames: words[word::FREE_FRAMES],
            cpus,
        })
    }

    /// Read the page mapped at `va`.
    ///
    /// Every load is [`Ordering::Relaxed`]. The mapping is read-only, and the standard library
    /// guarantees only a relaxed load of at most eight bytes for atomic access to read-only memory;
    /// the words need no ordering with each other, because they are not consistent with each other
    /// by design.
    ///
    /// # Safety
    ///
    /// `va` must be a mapped, 8-byte-aligned buffer of at least [`PAGE_BYTES`] bytes that stays
    /// mapped for the call. A parent maps exactly such a page, read-only, at [`PAGE_VA`].
    #[must_use]
    pub unsafe fn read(va: u64) -> Option<Snapshot> {
        let mut words = [0u64; WORDS];
        for (i, w) in words.iter_mut().enumerate() {
            // SAFETY: the caller's contract, and `i < WORDS`.
            *w = unsafe { counter(va, i) }.load(Ordering::Relaxed);
        }
        Snapshot::from_words(&words)
    }

    /// Bytes the allocator tracks.
    #[must_use]
    pub fn total_bytes(&self) -> u64 {
        self.total_frames.saturating_mul(self.frame_bytes)
    }

    /// Bytes free.
    #[must_use]
    pub fn free_bytes(&self) -> u64 {
        self.free_frames.saturating_mul(self.frame_bytes)
    }

    /// The sum of one counter over the online cpus, saturating: a page the reader accepted can hold
    /// any word, and no accessor panics on it.
    fn sum(&self, f: impl Fn(&Cpu) -> u64) -> u64 {
        self.cpus
            .iter()
            .filter(|c| c.online)
            .map(f)
            .fold(0, u64::saturating_add)
    }

    /// Cpus that have ticked. A count that indexes an array is a `usize`, where every other
    /// accessor returns the page's own `u64` words.
    #[must_use]
    pub fn online_cpus(&self) -> usize {
        self.cpus.iter().filter(|c| c.online).count()
    }

    /// Busy ticks over every online cpu.
    #[must_use]
    pub fn busy_ticks(&self) -> u64 {
        self.sum(|c| c.busy_ticks)
    }

    /// Idle ticks over every online cpu.
    #[must_use]
    pub fn idle_ticks(&self) -> u64 {
        self.sum(|c| c.idle_ticks)
    }

    /// Context switches over every online cpu.
    #[must_use]
    pub fn context_switches(&self) -> u64 {
        self.sum(|c| c.context_switches)
    }

    /// Interrupts over every online cpu.
    #[must_use]
    pub fn interrupts(&self) -> u64 {
        self.sum(|c| c.interrupts)
    }

    /// Runnable threads over every online cpu, as of each cpu's last tick.
    #[must_use]
    pub fn runnable(&self) -> u64 {
        self.sum(|c| c.runnable)
    }
}

#[cfg(kani)]
mod verification {
    use super::*;

    /// A page of any 64 words.
    fn any_words() -> [u64; WORDS] {
        kani::any()
    }

    /// **The reader admits a page exactly when its magic and version words match, and copies
    /// every field from its word.** Over every page: `None` exactly when the magic word or the
    /// version word differs, and otherwise each header field and each cpu field equals its word,
    /// with `online` the nonzero test of the first word of the line.
    /// Falsification: replayable `crates/machine_statistics_contract/falsifications/verification.the_reader_admits_exactly_the_pages_it_names.patch`
    #[kani::proof]
    #[kani::unwind(10)]
    fn the_reader_admits_exactly_the_pages_it_names() {
        let words = any_words();
        let admitted = words[word::MAGIC] == u64::from_le_bytes(MAGIC)
            && words[word::VERSION] == u64::from(VERSION);
        match Snapshot::from_words(&words) {
            None => assert!(!admitted),
            Some(s) => {
                assert!(admitted);
                assert_eq!(s.frame_bytes, words[word::FRAME_BYTES]);
                assert_eq!(s.tick_hertz, words[word::TICK_HERTZ]);
                assert_eq!(s.total_frames, words[word::TOTAL_FRAMES]);
                assert_eq!(s.free_frames, words[word::FREE_FRAMES]);
                for (id, c) in s.cpus.iter().enumerate() {
                    let at = word::cpu(id);
                    assert_eq!(c.online, words[at + word::ONLINE] != 0);
                    assert_eq!(c.busy_ticks, words[at + word::BUSY_TICKS]);
                    assert_eq!(c.idle_ticks, words[at + word::IDLE_TICKS]);
                    assert_eq!(c.context_switches, words[at + word::CONTEXT_SWITCHES]);
                    assert_eq!(c.interrupts, words[at + word::INTERRUPTS]);
                    assert_eq!(c.runnable, words[at + word::RUNNABLE]);
                }
            }
        }
    }

    /// **Every accessor of a snapshot of any admitted page returns without panicking**, and the
    /// byte totals saturate where the product would overflow.
    /// Falsification: replayable `crates/machine_statistics_contract/falsifications/verification.every_reading_is_total_over_an_admitted_page.patch`
    #[kani::proof]
    #[kani::unwind(10)]
    fn every_reading_is_total_over_an_admitted_page() {
        let mut words = any_words();
        words[word::MAGIC] = u64::from_le_bytes(MAGIC);
        words[word::VERSION] = u64::from(VERSION);
        let s = Snapshot::from_words(&words).unwrap();
        let _ = (s.busy_ticks(), s.idle_ticks(), s.context_switches());
        let _ = (s.interrupts(), s.runnable(), s.online_cpus());
        let (total, free) = (s.total_bytes(), s.free_bytes());
        assert_eq!(total, s.total_frames.saturating_mul(s.frame_bytes));
        assert_eq!(free, s.free_frames.saturating_mul(s.frame_bytes));
    }

    /// **A built header followed by zeroes reads back as an empty page**: the frame size and tick
    /// rate it was built with, no online cpu, every total zero.
    /// Falsification: replayable `crates/machine_statistics_contract/falsifications/verification.a_built_header_reads_back_as_an_empty_page.patch`
    #[kani::proof]
    #[kani::unwind(10)]
    fn a_built_header_reads_back_as_an_empty_page() {
        let (frame_bytes, tick_hertz): (u64, u64) = (kani::any(), kani::any());
        let mut words = [0u64; WORDS];
        words[..word::LINE].copy_from_slice(&build_header(frame_bytes, tick_hertz));
        let s = Snapshot::from_words(&words).unwrap();
        assert_eq!(s.frame_bytes, frame_bytes);
        assert_eq!(s.tick_hertz, tick_hertz);
        assert_eq!(s.online_cpus(), 0);
        assert_eq!(s.total_frames, 0);
        assert_eq!(s.free_frames, 0);
        assert_eq!(s.busy_ticks(), 0);
        assert_eq!(s.total_bytes(), 0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prepared() -> [u64; WORDS] {
        let mut w = [0u64; WORDS];
        w[..word::LINE].copy_from_slice(&build_header(4096, 100));
        w
    }

    #[test]
    fn every_line_is_its_own_cache_line() {
        assert_eq!(word::cpu(0), 8);
        assert_eq!(word::cpu(CPU_ID_BOUND - 1) + word::LINE, WORDS);
    }

    #[test]
    fn an_offline_cpu_counts_for_nothing_even_if_its_words_are_not_zero() {
        let mut w = prepared();
        w[word::cpu(1) + word::ONLINE] = 1;
        w[word::cpu(1) + word::CONTEXT_SWITCHES] = 7;
        w[word::cpu(3) + word::CONTEXT_SWITCHES] = 1000; // never ticked
        let s = Snapshot::from_words(&w).unwrap();
        assert_eq!(s.online_cpus(), 1);
        assert_eq!(s.context_switches(), 7);
    }

    #[test]
    fn the_header_carries_what_a_reader_needs_to_print_units() {
        let s = Snapshot::from_words(&prepared()).unwrap();
        assert_eq!((s.frame_bytes, s.tick_hertz), (4096, 100));
    }

    #[test]
    fn a_page_without_the_magic_has_no_answer() {
        let mut w = prepared();
        w[word::MAGIC] ^= 1;
        assert!(Snapshot::from_words(&w).is_none());
    }

    /// **A page of another generation reads as no page at all** (the interface-version check): the
    /// magic alone no longer admits a page; the reader checks the version word beside it, so a
    /// reader built against one layout cannot print another layout's words in its own places.
    #[test]
    fn a_page_of_another_version_has_no_answer() {
        let mut w = prepared();
        w[word::VERSION] = u64::from(VERSION) + 1;
        assert!(Snapshot::from_words(&w).is_none());
        // A page that predates the word (all zeroes there) is refused too, not read as version 0.
        let mut old = prepared();
        old[word::VERSION] = 0;
        assert!(Snapshot::from_words(&old).is_none());
    }

    /// **Every accessor sums only the online cpus, and each answers its own word.** Two online
    /// cpus carry distinct values in every counter and a third, offline, carries values that would
    /// show if it were counted. Distinct non-0, non-1 values separate an accessor from a constant
    /// and from its neighbours. Found by mutation testing.
    #[test]
    fn each_total_is_its_own_counter_summed_over_online_cores_only() {
        let mut w = prepared();
        w[word::TOTAL_FRAMES] = 11;
        w[word::FREE_FRAMES] = 5;
        for (id, base) in [(0, 10u64), (2, 100)] {
            let at = word::cpu(id);
            w[at + word::ONLINE] = 1;
            w[at + word::BUSY_TICKS] = base + 1;
            w[at + word::IDLE_TICKS] = base + 2;
            w[at + word::CONTEXT_SWITCHES] = base + 3;
            w[at + word::INTERRUPTS] = base + 4;
            w[at + word::RUNNABLE] = base + 5;
        }
        let off = word::cpu(1);
        for k in 1..=5 {
            w[off + k] = 1_000_000;
        }
        let s = Snapshot::from_words(&w).unwrap();
        assert_eq!(s.total_bytes(), 11 * 4096);
        assert_eq!(s.free_bytes(), 5 * 4096);
        assert_eq!(s.online_cpus(), 2);
        assert_eq!(s.busy_ticks(), 11 + 101);
        assert_eq!(s.idle_ticks(), 12 + 102);
        assert_eq!(s.context_switches(), 13 + 103);
        assert_eq!(s.interrupts(), 14 + 104);
        assert_eq!(s.runnable(), 15 + 105);
    }

    /// **No accessor panics, and each saturates, on a page the reader accepted.** Every counter at
    /// `u64::MAX` under a valid header is a page a foreign or corrupted writer could leave.
    #[test]
    fn a_page_of_maximal_counters_saturates_every_reading() {
        let mut w = prepared();
        w[word::TOTAL_FRAMES] = u64::MAX;
        w[word::FREE_FRAMES] = u64::MAX;
        for id in 0..CPU_ID_BOUND {
            for offset in 0..=word::RUNNABLE {
                w[word::cpu(id) + offset] = u64::MAX;
            }
        }
        let s = Snapshot::from_words(&w).unwrap();
        assert_eq!(s.online_cpus(), CPU_ID_BOUND);
        assert_eq!(s.total_bytes(), u64::MAX);
        assert_eq!(s.free_bytes(), u64::MAX);
        assert_eq!(s.busy_ticks(), u64::MAX);
        assert_eq!(s.idle_ticks(), u64::MAX);
        assert_eq!(s.context_switches(), u64::MAX);
        assert_eq!(s.interrupts(), u64::MAX);
        assert_eq!(s.runnable(), u64::MAX);
    }

    /// A page frame, aligned as a mapping is, for the tests that go through the `unsafe` reader.
    #[repr(align(4096))]
    struct Frame([u64; WORDS]);

    /// **The `unsafe` reader over a real buffer.** A header and one cpu line are written through
    /// the same atomic view the kernel's writer uses, and `read` returns exactly them. The frame is
    /// reached through `as_mut_ptr` so the pointer carries provenance over the whole buffer.
    #[test]
    fn read_returns_what_the_writer_stored_through_counter() {
        let mut frame = Frame([0; WORDS]);
        let frame_virtual_address = frame.0.as_mut_ptr() as u64;
        for (i, word) in build_header(4096, 100).iter().enumerate() {
            // SAFETY: `frame` is aligned, `WORDS` long and lives past every use below.
            unsafe { counter(frame_virtual_address, i) }.store(*word, Ordering::Relaxed);
        }
        let line = word::cpu(2);
        for (offset, value) in [
            (word::ONLINE, 1),
            (word::BUSY_TICKS, 9),
            (word::RUNNABLE, 3),
        ] {
            // SAFETY: as above; `line + offset < WORDS`.
            unsafe { counter(frame_virtual_address, line + offset) }.store(value, Ordering::Relaxed);
        }
        // SAFETY: the same buffer, still live.
        let s = unsafe { Snapshot::read(frame_virtual_address) }.expect("a prepared frame");
        assert_eq!((s.frame_bytes, s.tick_hertz), (4096, 100));
        assert_eq!(s.online_cpus(), 1);
        assert_eq!((s.busy_ticks(), s.runnable()), (9, 3));
    }

    /// A frame nobody prepared reads as no page through the `unsafe` reader.
    #[test]
    fn read_of_an_unprepared_frame_is_none() {
        let mut frame = Frame([0; WORDS]);
        // SAFETY: an aligned, live frame of `WORDS` words.
        assert!(unsafe { Snapshot::read(frame.0.as_mut_ptr() as u64) }.is_none());
    }
}
