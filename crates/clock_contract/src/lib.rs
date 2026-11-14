//! The wall-clock contract (reading the clock is a page, setting it is a page you may write,
//! proposing is an endpoint): the clock page's layout, the propose protocol and the policy the
//! service applies, defined once for the clock service, its readers, the standard library's
//! platform abstraction layer and the tests.
//!
//! Reading is a read-only mapping of [`ClockPage`] plus the monotonic counter, setting is a
//! read/write mapping of the same page, and proposing is an endpoint [`policy::decide`] answers.
//! Wall-clock time is [`wall_nanoseconds`]: the counter plus an offset, and only the offset is ever
//! written, so a clock step cannot perturb `Instant`. Every time is nanoseconds since the Unix
//! epoch in a `u64`, which runs out in 2554.
//!
//! # Identity
//!
//! The page's identity is frozen: [`MAGIC`] is the ASCII `CLOCKv01`, [`VERSION`] is 1 and
//! [`INTERFACE_ID`] is 8. So is its layout: [`WORDS`] is seven 64-bit words, the magic, the
//! publish count, the writers' claim, then the state and the offset of copy 0 and of copy 1, in
//! that order. The id stays 8 for good. A layout break ships as a new magic and a new version
//! together, never a new id, and a retired value is never reused.
//!
//! # BUGS
//!
//! - `NANOS_PER_SEC` and `wall_nanos` are older spellings of `NS_PER_SECOND` and `wall_nanoseconds`, kept for
//!   a standard-library overlay, which reads them by those names. Use the newer spellings.
//!
//! Name: ratified 2026-08-23 (calef): the service's own word, `clock`, with the contract suffix
//! `_contract` (calef's 2026-10-18 ruling).

#![cfg_attr(not(test), no_std)]

#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeExample;

// The atomics are loom's under `--cfg loom` and the real ones otherwise, which is what lets
// the `loom` model checker replay the clock page under every interleaving the C11 model permits
// (build with `RUSTFLAGS="--cfg loom"`). The algorithms below are written once and are the ones the
// clock service runs: only where the words live differs, and that is not what is being checked.
#[cfg(not(loom))]
use core::sync::atomic::{AtomicU64, Ordering, fence};

#[cfg(loom)]
use loom::sync::atomic::{AtomicU64, Ordering, fence};

/// Give the model checker a scheduling point inside a spin, and the CPU a hint outside one.
///
/// A writer spins while another writer holds the claim. On real hardware that is a few
/// instructions and `spin_loop` is the right hint. Under loom it is a **liveness hazard**: loom's
/// scheduler is cooperative, and a thread that spins without yielding can starve the writer whose
/// progress the spin is waiting for, so the model never terminates. `yield_now` tells loom this is
/// a point where the other thread must be allowed to run, which is exactly what the hardware does
/// for free. A reader never spins, so it never calls this.
#[inline]
fn spin_hint() {
    #[cfg(loom)]
    loom::thread::yield_now();
    #[cfg(not(loom))]
    core::hint::spin_loop();
}

/// (`NS_PER_SECOND`, the unit-symbol ruling of 2026-10-27, which names this one definition.)
/// Nanoseconds in a second, the conversion every reader does exactly once.
///
/// Name: ratified 2026-10-27 (calef): the one definition the unit-symbol ruling of that date names.
#[doc(alias = "nanoseconds per second")]
pub const NS_PER_SECOND: u64 = 1_000_000_000;

/// Wall-clock nanoseconds, from the offset the clock page carries and the monotonic nanoseconds
/// the reader measured for itself.
///
/// Saturating rather than wrapping: an implausible offset should read as an implausible time, not
/// as a plausible one on the other side of the wrap.
///
/// Name: provisional, minted 2026-10-27 (UTC) by lane/unit-symbols-sweep-1: `wall_nanos` under the
/// unit-symbol ruling of that date.
///
/// # Examples
///
/// Setting the wall clock is writing an offset; the monotonic counter is not in the expression:
///
/// ```
/// use clock_contract::{NS_PER_SECOND, offset_for, wall_nanoseconds};
///
/// // The machine has been up for 5 seconds and the service has just learned the wall time.
/// let monotonic = 5 * NS_PER_SECOND;
/// let now = 1_800_000_000 * NS_PER_SECOND;
/// let offset = offset_for(now, monotonic);
/// assert_eq!(wall_nanoseconds(offset, monotonic), now);
///
/// // Two seconds later, with nobody having touched anything.
/// let monotonic = 7 * NS_PER_SECOND;
/// assert_eq!(wall_nanoseconds(offset, monotonic), now + 2 * NS_PER_SECOND);
///
/// // An operator steps the clock a minute forward. Only the offset changes.
/// let stepped = offset_for(now + 60 * NS_PER_SECOND, monotonic);
/// assert_eq!(wall_nanoseconds(stepped, monotonic), now + 60 * NS_PER_SECOND);
/// assert_eq!(monotonic, 7 * NS_PER_SECOND); // untouched, and there is no way to touch it
/// ```
pub const fn wall_nanoseconds(offset_nanoseconds: u64, monotonic_nanoseconds: u64) -> u64 {
    offset_nanoseconds.saturating_add(monotonic_nanoseconds)
}

/// (`NANOS_PER_SEC`, a standard-library overlay, which reads this name from its generated copy
/// of this file.)
/// The spelling of [`NS_PER_SECOND`] that the overlay reads.
///
/// Name: provisional. Kept for that reader; the crate's own spelling is `NS_PER_SECOND`, and the
/// overlay moves to it with its next edit.
#[doc(alias = "nanoseconds per second, overlay spelling")]
pub const NANOS_PER_SEC: u64 = NS_PER_SECOND;

/// (`wall_nanos`, a standard-library overlay, which reads this name from its generated copy of
/// this file.)
/// The spelling of [`wall_nanoseconds`] that the overlay reads; see [`NANOS_PER_SEC`].
///
/// Name: provisional. Kept for the overlay, as [`NANOS_PER_SEC`] is.
#[doc(alias = "wall clock nanoseconds, overlay spelling")]
pub const fn wall_nanos(offset_nanoseconds: u64, monotonic_nanoseconds: u64) -> u64 {
    wall_nanoseconds(offset_nanoseconds, monotonic_nanoseconds)
}

/// The offset that makes `monotonic_nanoseconds` read as `wall_nanoseconds`: what the clock service writes
/// when it learns the time. The inverse of [`wall_nanoseconds`].
pub const fn offset_for(wall_nanoseconds: u64, monotonic_nanoseconds: u64) -> u64 {
    wall_nanoseconds.saturating_sub(monotonic_nanoseconds)
}

// ================================================================================================
// What the machine knows about the time, which includes "nothing".
// ================================================================================================

/// **The states the wall clock can be in, and one of them is "I do not know".**
///
/// This is the no-silent-degradation rule on a second axis. A machine with no RTC, or
/// with an RTC reporting something impossible, must say so; confidently reporting 1970 plus uptime
/// is the failure, because the caller cannot tell it from a real answer.
pub mod state {
    /// **The wall clock is unknown.** No clock page, no clock service, an RTC that is absent, or an
    /// RTC whose reading failed [`super::policy::is_plausible`]. The offset is meaningless and must
    /// not be used. This is also what a zeroed page reads as, so a page nobody has published to is
    /// honest by default rather than by initialization.
    pub const UNKNOWN: u64 = 0;
    /// Set once at startup from the hardware RTC. As good as the battery-backed clock on the board,
    /// which on QEMU is the host's clock and on a real board is whatever the coin cell managed.
    pub const RTC: u64 = 1;
    /// Set outright by an authority holding the page read/write: an operator, or the service's own
    /// startup on a machine where that is the only source.
    pub const SET: u64 = 2;
    /// Set from a **proposal the service accepted**, which is where a network time client's work
    /// lands. Distinguished from [`SET`] because "an external source I bounded" and "a human told
    /// me" are different provenance, and the difference is exactly what a caller deciding whether
    /// to trust a certificate expiry wants.
    pub const SYNCED: u64 = 3;

    /// Whether a state means the machine actually knows the time.
    pub const fn is_known(state: u64) -> bool {
        state != UNKNOWN
    }
}

// ================================================================================================
// The clock page: the read authority, and the set authority, are mappings of these fifty-six bytes.
// ================================================================================================

/// The page's first word, so a reader can tell a published clock page from a zeroed frame or from
/// somebody else's page. ASCII `CLOCKv01`, big-endian in the source so it reads in a hex dump.
///
/// Name: ratified 2026-11-14 (UTC, #3576), with its value `CLOCKv01`: the identity freeze of that
/// date. The name every contract crate uses for its page's first word.
pub const MAGIC: u64 = 0x434c_4f43_4b76_3031;

/// The clock page layout's version, the digits [`MAGIC`] carries. A reader's check is the magic
/// itself: a reader built against one generation reads a page of another as no clock page at all,
/// and a break bumps this and re-mints the magic's digits in one change, which the changelog
/// records.
///
/// Name: ratified 2026-11-14 (UTC, #3576), with its value 1: the identity freeze of that date. The
/// name every contract crate uses for its layout version.
pub const VERSION: u32 = 1;

/// This interface's identity in a program's `INTERFACES` note: 8, naming the clock page and its
/// propose call. The number is this interface's for good: no other interface takes it, and a format
/// change is a new version, never a new number. The project that links these crates keeps the table of numbers and holds it to that rule. A
/// program declares the pair of this id and [`VERSION`].
///
/// Name: ratified 2026-11-14 (UTC, #3576), with its value 8: the identity freeze of that date. The
/// name every contract crate uses for its interface identity.
pub const INTERFACE_ID: u64 = 8;

/// The words the clock page uses: seven, in the order the word indices below list them. The rest of
/// the frame is reserved and must read as zero: a future field is a new index here, and an old
/// reader that never learned about it is unaffected.
pub const WORDS: usize = 7;

// The layout, one list rather than seven scattered offsets. Word 0 is the magic, word 1 the publish
// count, word 2 the writers' claim, then two copies of the state and the offset: copy 0 in words 3
// and 4, copy 1 in words 5 and 6. The publish count's low bit names the copy readers use.
const MAGIC_WORD_INDEX: usize = 0;
const PUBLISH_WORD_INDEX: usize = 1;
const CLAIM_WORD_INDEX: usize = 2;
const STATE_WORD_INDEX: [usize; 2] = [3, 5];
const OFFSET_WORD_INDEX: [usize; 2] = [4, 6];

/// The claim word's two values. A writer moves it from free to held with a compare-exchange before
/// it touches anything else, and back to free once its publish is visible. No reader looks at it.
const CLAIM_FREE: u64 = 0;
const CLAIM_HELD: u64 = 1;

/// The copy that holds the publish numbered `generation`: its low bit.
const fn copy_for(generation: u64) -> usize {
    (generation & 1) as usize
}

/// What a reader gets out of the page in one consistent look.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reading {
    /// One of [`state`]'s values.
    pub state: u64,
    /// (`offset_nanos`, a standard-library overlay, which reads this field from its generated
    /// copy of this file.)
    /// Wall-clock nanoseconds at monotonic zero. Meaningless unless [`state::is_known`].
    #[doc(alias = "offset in nanoseconds")]
    pub offset_nanos: u64,
    /// How many times the page has been published to: the publish count this reading came with. A
    /// reader that cares whether the clock stepped under it (a log, a cache with an expiry)
    /// compares this across two readings instead of comparing timestamps and guessing.
    pub generation: u64,
}

/// **The clock page**, as seen through one process's mapping of it. Readers are many, writers are
/// several (any process holding the page read/write), and no lock can be held across an
/// address-space boundary, so the page keeps two copies of the time and a publish count, and **a
/// reader never waits on a writer**.
///
/// The writer fills the copy the current count does not name, then advances the count, which moves
/// every later reader to the new copy in one store. A reader loads the count, reads the copy it
/// names, and loads the count again: equal means the pair is whole. Unequal means a publish
/// finished during the read, and the reader goes again; a writer that has stopped finishes nothing,
/// so it never makes a reader retry. Writers exclude each other through a separate claim word,
/// which readers never touch, so a writer that stops holding the claim blocks only later writers.
///
/// This is Linux's seqcount latch (`raw_write_seqcount_latch` in `include/linux/seqlock.h`)
/// with one copy written per publish instead of both, and FreeBSD's `timehands` ring
/// (`sys/kern/kern_tc.c`) at its default of two records, with the generation kept in one word
/// rather than in each record. The orderings, and what each pairs with, are at the stores and loads below and in
/// notes/clock.md.
///
/// # BUGS
///
/// - The orderings are checked against C11 by loom, not against Arm's or RISC-V's model.
/// - A writer stopped while it holds the claim blocks every later writer until something clears the
///   claim. Readers keep reading the last publish. Nothing in this crate can tell a writer that
///   stopped for good from one that is only slow, so nothing here clears a claim it did not take.
/// - The publish count is a `u64` and wraps after 2^64 publishes, at which point a reader that slept
///   across exactly 2^64 of them would accept a torn pair. A publish a nanosecond would take five
///   centuries to get there.
#[derive(Debug, Clone, Copy)]
pub struct ClockPage {
    base: *const AtomicU64,
}

// SAFETY: the whole point of the page is that several address spaces share it; every access below
// goes through atomics, so there is no non-atomic aliasing to protect against.
unsafe impl Send for ClockPage {}
// SAFETY: as for `Send` above: the page is shared by construction and every access goes through atomics, so there is no non-atomic aliasing to protect against.
unsafe impl Sync for ClockPage {}

impl ClockPage {
    /// Name the clock page mapped at `va`.
    ///
    /// # Safety
    ///
    /// `va` must be a mapped, 8-byte-aligned frame that is the clock page (or, for a writer about
    /// to call [`initialize`](Self::initialize), a zeroed frame that is about to become one), and
    /// it must stay mapped for as long as this value is used. Read-only is enough for
    /// [`read`](Self::read); [`publish`](Self::publish) and [`initialize`](Self::initialize) need
    /// it mapped read/write, and calling them on a read-only mapping faults the caller, which is
    /// the correct outcome: a process without the set authority cannot set the clock, and finds out
    /// immediately.
    pub const unsafe fn new(va: u64) -> Self {
        ClockPage {
            base: va as *const AtomicU64,
        }
    }

    fn word(&self, i: usize) -> &AtomicU64 {
        // SAFETY: `new`'s contract is a mapped frame; `i` is always one of the `*_WORD_INDEX`
        // constants, all of which are below WORDS and therefore inside the frame.
        unsafe { &*self.base.add(i) }
    }

    /// Stamp a fresh frame as a clock page in the unknown state. The writer does this once, before
    /// anyone else has a mapping, so it needs no claim.
    ///
    /// The magic goes down **last**, with a release, so a reader that races the first publish sees
    /// either a page it does not recognize (and reports unknown) or a fully initialized one. Never
    /// a recognized page with garbage in it.
    ///
    /// Name: provisional. Spelled out under the `init` ruling of 2026-10-26 (UTC), in
    /// design/naming/acronyms-and-abbreviations.md.
    pub fn initialize(&self) {
        self.word(PUBLISH_WORD_INDEX).store(0, Ordering::Relaxed);
        self.word(CLAIM_WORD_INDEX)
            .store(CLAIM_FREE, Ordering::Relaxed);
        for copy in 0..2 {
            self.word(STATE_WORD_INDEX[copy])
                .store(state::UNKNOWN, Ordering::Relaxed);
            self.word(OFFSET_WORD_INDEX[copy])
                .store(0, Ordering::Relaxed);
        }
        // Release: a reader whose acquire load in `read` sees the magic sees every store above.
        self.word(MAGIC_WORD_INDEX).store(MAGIC, Ordering::Release);
    }

    /// One consistent look at the page. Never blocks a writer, never waits on one, and never
    /// fails: a page without the magic reads as [`state::UNKNOWN`], which is the truth about a
    /// frame nobody has published to.
    ///
    /// It loops only while publishes finish during its own look, which a stopped writer cannot do.
    pub fn read(&self) -> Reading {
        const UNKNOWN: Reading = Reading {
            state: state::UNKNOWN,
            offset_nanos: 0,
            generation: 0,
        };
        // Acquire: pairs with the release store of the magic in `initialize`.
        if self.word(MAGIC_WORD_INDEX).load(Ordering::Acquire) != MAGIC {
            return UNKNOWN;
        }
        loop {
            if let Some(reading) = self.read_once() {
                return reading;
            }
        }
    }

    /// One attempt at a reading: `None` when a publish finished while it read, so the copy it read
    /// may have been rewritten under it.
    fn read_once(&self) -> Option<Reading> {
        // Acquire: pairs with the release store of the publish count in `flip`, so the copy this
        // count names holds at least the pair that publish wrote.
        let generation = self.word(PUBLISH_WORD_INDEX).load(Ordering::Acquire);
        let copy = copy_for(generation);
        let st = self.word(STATE_WORD_INDEX[copy]).load(Ordering::Relaxed);
        let off = self.word(OFFSET_WORD_INDEX[copy]).load(Ordering::Relaxed);
        // Keep the two data loads above the second load of the count. Without it the compiler or
        // the machine may move them after it, and the check would validate nothing.
        //
        // PAIR: the writer's `fence(Ordering::Release)` before its data stores in `fill`. If
        // either data load above read a store made after that fence, the two fences synchronize,
        // and the count the writer read before its fence (one below its own publish, and above the
        // `generation` this reader holds) is visible to the load below. So a copy rewritten under
        // this reader always shows as a moved count. Both halves must be here, because a reader is
        // in a different address space and reaches this page with no syscall: no message
        // rendezvous underneath supplies the edge.
        fence(Ordering::Acquire);
        // Equal, not "within one": a count one above ours can belong to a writer that is already
        // filling our copy for the publish after it, and equality is what proves no publish
        // finished. The full 64-bit count, not the copy bit, is what lets a reader that slept
        // across two publishes see that it did.
        (self.word(PUBLISH_WORD_INDEX).load(Ordering::Relaxed) == generation).then_some(Reading {
            state: st,
            offset_nanos: off,
            generation,
        })
    }

    /// Write a new state and offset. **This is the set authority**: it needs nothing but a
    /// read/write mapping, because being able to write the offset is what setting the clock means.
    ///
    /// Waits while another writer holds the claim, and never makes a reader wait. Returns the new
    /// generation.
    pub fn publish(&self, new_state: u64, offset_nanoseconds: u64) -> u64 {
        self.claim();
        let generation = self.fill(new_state, offset_nanoseconds);
        self.flip(generation);
        self.release();
        generation
    }

    /// Take the writers' claim, waiting while another writer holds it.
    fn claim(&self) {
        while !self.try_claim() {
            spin_hint();
        }
    }

    /// Take the writers' claim if it is free.
    fn try_claim(&self) -> bool {
        // Acquire on success: pairs with the release store in `release`, so this writer sees the
        // last writer's publish count and its copy's stores, and its own stores to that copy two
        // publishes later are ordered after them.
        self.word(CLAIM_WORD_INDEX)
            .compare_exchange_weak(CLAIM_FREE, CLAIM_HELD, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
    }

    /// Write the pair into the copy readers are not using, and return the generation it is for.
    /// The caller holds the claim.
    fn fill(&self, new_state: u64, offset_nanoseconds: u64) -> u64 {
        // Relaxed: only a claim holder stores the count, and the claim's acquire already made the
        // last holder's store visible here.
        let generation = self
            .word(PUBLISH_WORD_INDEX)
            .load(Ordering::Relaxed)
            .wrapping_add(1);
        let copy = copy_for(generation);
        // **The count read above must be visible before the data stores below**, to any reader
        // that sees one of them. Those stores rewrite the copy that readers holding the count two
        // below `generation` read, and such a reader detects it only by finding the count moved.
        // A store-store barrier, Linux's `smp_wmb()` at the head of `raw_write_seqcount_latch`.
        // The claim's acquire does not do this: an acquire orders later accesses after itself,
        // not earlier stores ahead of later ones.
        //
        // PAIR: the reader's `fence(Ordering::Acquire)` in `read_once`.
        fence(Ordering::Release);
        self.word(STATE_WORD_INDEX[copy])
            .store(new_state, Ordering::Relaxed);
        self.word(OFFSET_WORD_INDEX[copy])
            .store(offset_nanoseconds, Ordering::Relaxed);
        generation
    }

    /// Move readers to the copy `fill` wrote.
    fn flip(&self, generation: u64) {
        // Release: pairs with the reader's acquire load of the count in `read_once`, so a reader
        // that sees `generation` sees the pair `fill` stored for it.
        self.word(PUBLISH_WORD_INDEX)
            .store(generation, Ordering::Release);
    }

    /// Give the claim back.
    fn release(&self) {
        // Release: pairs with the next writer's acquire in `try_claim`, carrying this publish's
        // count and copy to it. Without it the next writer may read a count older than this
        // publish's and fill the copy readers are using. Loom does not fail when this is relaxed,
        // so this one rests on the argument alone.
        self.word(CLAIM_WORD_INDEX)
            .store(CLAIM_FREE, Ordering::Release);
    }
}

// ================================================================================================
// The propose protocol: two words out, two words back, over one endpoint.
// ================================================================================================

/// **The propose protocol** (a proposer → the clock service), spoken over an endpoint `CALL`.
///
/// Deliberately tiny, and deliberately **not** a way to set the clock. Everything that arrives here
/// is a request the service is free to refuse, which is what makes the endpoint safe to hand to a
/// network time client: a compromised one can lie inside [`policy`]'s bounds and can do nothing
/// else at all.
pub mod propose {
    /// Where the opcode sits in the first `CALL` word: bits 63:56, the same position its sibling
    /// message contracts use, so the contracts read alike.
    pub const OPERATION_SHIFT: u32 = 56;

    /// Build a request's first word.
    pub const fn req(operation: u64) -> u64 {
        operation << OPERATION_SHIFT
    }

    /// The opcode of a request word.
    pub const fn operation(w0: u64) -> u64 {
        w0 >> OPERATION_SHIFT
    }

    /// `CALL(req(PROPOSE), proposed_unix_nanos)`. Ask the service to move the wall clock to
    /// `proposed_unix_nanos`. Reply `r0` is one of the `status` codes and `r1` is
    /// the wall-clock nanoseconds in force afterwards (0 when the state is unknown), so a proposer
    /// learns what happened without needing a read mapping.
    pub const PROPOSE: u64 = 1;

    /// `CALL(req(STATE), 0)`. Ask what the clock knows. Reply `r0` is a [`state`](super::state)
    /// value and `r1` the wall-clock nanoseconds now (0 when unknown).
    ///
    /// Redundant for anyone holding the page, and that is fine: a proposer is exactly the process
    /// that may hold the endpoint and no mapping, and it has to know whether the clock is unknown
    /// (in which case its proposal bootstraps) or already running (in which case a step applies).
    pub const STATE: u64 = 2;
}

/// The reply's first word for a [`propose::PROPOSE`]. Not an errno space: this contract has no
/// POSIX behind it, and every refusal here is a policy answer rather than a failure.
pub mod status {
    /// The proposal was applied. The clock is now [`super::state::SYNCED`].
    pub const ACCEPTED: u64 = 0;
    /// Outside the sanity window entirely: [`super::policy::is_plausible`] says no machine running
    /// this code is at that instant. A proposal of 1970, or of 2038, lands here.
    pub const REFUSED_IMPLAUSIBLE: u64 = 1;
    /// Plausible in the absolute, but more than [`super::policy::MAXIMUM_STEP_FORWARD_NANOSECONDS`] ahead
    /// of what the clock already believes.
    pub const REFUSED_TOO_FAR_FORWARD: u64 = 2;
    /// Plausible in the absolute, but more than [`super::policy::MAXIMUM_STEP_BACKWARD_NANOSECONDS`]
    /// behind what the clock already believes. The asymmetry with forward is deliberate; see
    /// [`policy`](super::policy).
    pub const REFUSED_TOO_FAR_BACKWARD: u64 = 3;
    /// The request was not one this contract defines.
    pub const BAD_REQUEST: u64 = 4;
}

// ================================================================================================
// The policy: what the service does with a proposal, as a pure function.
// ================================================================================================

/// **The policy a proposal is judged by.**
///
/// It lives in the contract crate rather than inside the service for two reasons. It is the part
/// worth testing on the host in milliseconds, and it is the part a proposer needs in order to be a
/// well-behaved one: a network time client that can predict the answer can decline to ask rather
/// than hammering the endpoint with proposals that will be refused. Stating the bounds publicly
/// costs nothing, because the authority was never secrecy about the bounds; it is that the
/// proposer cannot write the page.
pub mod policy {
    use super::{state, status};

    /// **The build-era floor: 2026-01-01T00:00:00Z.** No machine running this code existed before
    /// it, so any claimed time below this is wrong no matter who said it.
    ///
    /// It is the escape from the network-time chicken-and-egg (TLS needs a roughly correct clock,
    /// and a correct clock needs TLS), chosen here on purpose. It is a **floor on plausibility, not a
    /// claim of accuracy**: passing it means "not obviously a lie", never "trustworthy".
    pub const NOT_BEFORE_NANOSECONDS: u64 = 1_767_225_600 * super::NS_PER_SECOND;

    /// The ceiling: 2100-01-01T00:00:00Z. Far enough out to be uncontroversial and near enough to
    /// catch the classic attacks, which push the clock past a certificate's expiry rather than
    /// nudging it.
    pub const NOT_AFTER_NANOSECONDS: u64 = 4_102_444_800 * super::NS_PER_SECOND;

    /// How far forward one accepted proposal may move a clock that already knows the time: an hour.
    /// Enough to absorb a machine that has been asleep, or an RTC an hour out because somebody set
    /// it to local time; not enough to walk the clock past an expiry in one step.
    ///
    /// Name: ratified 2026-10-27 (calef): `MAXIMUM` from the minimum, maximum and operating-system
    /// ruling of 2026-10-26 (UTC), and `NS` from the unit-symbol ruling of 2026-10-27 (UTC).
    pub const MAXIMUM_STEP_FORWARD_NANOSECONDS: u64 = 3600 * super::NS_PER_SECOND;

    /// How far **backward** one accepted proposal may move a clock that already knows the time: one
    /// second.
    ///
    /// The asymmetry is the whole point and it is not timidity. Moving forward skips over instants
    /// nobody has observed yet; moving backward makes instants happen twice, which is what breaks
    /// log ordering, cache expiries, build stamps and anything that recorded a timestamp and
    /// assumed it would not be reissued. Unix reaches for `adjtime` slewing largely because of
    /// this, and here the same conservatism is one constant instead of a mechanism, because
    /// `Instant` is never affected by any of it.
    ///
    /// Name: ratified 2026-10-27 (calef): `MAXIMUM` from the minimum, maximum and operating-system
    /// ruling of 2026-10-26 (UTC), and `NS` from the unit-symbol ruling of 2026-10-27 (UTC).
    pub const MAXIMUM_STEP_BACKWARD_NANOSECONDS: u64 = super::NS_PER_SECOND;

    // The asymmetry is a decision, not an accident, so it is a build-time fact rather than
    // something a reader has to notice: anyone "tidying" the two constants into one fails to
    // compile rather than quietly making backwards steps as free as forwards ones.
    const _: () = assert!(
        MAXIMUM_STEP_BACKWARD_NANOSECONDS < MAXIMUM_STEP_FORWARD_NANOSECONDS,
        "moving the clock backwards must stay far tighter than moving it forwards",
    );

    /// Whether an absolute instant is one a machine running this code could be at. The sanity
    /// window, applied to an RTC reading as well as to a proposal: an RTC that fails this is an RTC
    /// the service refuses to believe, and the clock stays [`state::UNKNOWN`] rather than becoming
    /// confidently wrong.
    pub const fn is_plausible(unix_nanoseconds: u64) -> bool {
        unix_nanoseconds >= NOT_BEFORE_NANOSECONDS && unix_nanoseconds < NOT_AFTER_NANOSECONDS
    }

    /// **The decision.** `current_state` and `current_nanoseconds` are what the clock believes now;
    /// `proposed_nanoseconds` is what the proposer asked for. The answer is one of [`status`]'s codes.
    ///
    /// The bootstrap case is the interesting one: when the clock is [`state::UNKNOWN`] there is
    /// nothing to step *from*, so a plausible proposal is accepted outright. That is not a hole,
    /// because a machine that does not know the time has no belief for a step limit to protect;
    /// the sanity window is the only guard that means anything, and it is applied.
    ///
    /// # Examples
    ///
    /// A compromised time client can lie inside these bounds and do nothing else:
    ///
    /// ```
    /// use clock_contract::{NS_PER_SECOND, policy, state, status};
    ///
    /// let now = 1_800_000_000 * NS_PER_SECOND;
    ///
    /// // A small correction, in either direction, is what a time client is for.
    /// assert_eq!(policy::decide(state::SYNCED, now, now + NS_PER_SECOND / 2), status::ACCEPTED);
    ///
    /// // Walking the clock past a certificate expiry in one step is the classic attack.
    /// assert_eq!(
    ///     policy::decide(state::SYNCED, now, now + 2 * policy::MAXIMUM_STEP_FORWARD_NANOSECONDS),
    ///     status::REFUSED_TOO_FAR_FORWARD,
    /// );
    ///
    /// // And the tighter half: a second forward is fine, a second and a half backward is not.
    /// assert_eq!(policy::decide(state::SYNCED, now, now + 3 * NS_PER_SECOND / 2), status::ACCEPTED);
    /// assert_eq!(
    ///     policy::decide(state::SYNCED, now, now - 3 * NS_PER_SECOND / 2),
    ///     status::REFUSED_TOO_FAR_BACKWARD,
    /// );
    ///
    /// // The bootstrap case: a machine that does not know the time has no belief for a step limit to
    /// // protect, so a plausible proposal is accepted outright. The sanity window still applies.
    /// assert_eq!(policy::decide(state::UNKNOWN, 0, now), status::ACCEPTED);
    /// assert_eq!(policy::decide(state::UNKNOWN, 0, 0), status::REFUSED_IMPLAUSIBLE);
    /// assert!(!policy::is_plausible(0)); // 1970 plus uptime is not a time this code can be running at
    /// ```
    pub const fn decide(
        current_state: u64,
        current_nanoseconds: u64,
        proposed_nanoseconds: u64,
    ) -> u64 {
        if !is_plausible(proposed_nanoseconds) {
            return status::REFUSED_IMPLAUSIBLE;
        }
        if !state::is_known(current_state) {
            return status::ACCEPTED;
        }
        if proposed_nanoseconds > current_nanoseconds {
            if proposed_nanoseconds - current_nanoseconds > MAXIMUM_STEP_FORWARD_NANOSECONDS {
                return status::REFUSED_TOO_FAR_FORWARD;
            }
        } else if current_nanoseconds - proposed_nanoseconds > MAXIMUM_STEP_BACKWARD_NANOSECONDS {
            return status::REFUSED_TOO_FAR_BACKWARD;
        }
        status::ACCEPTED
    }
}

// ================================================================================================
// The RTC bindings, named so the driver picks a register layout (or, for CMOS, a pair of I/O
// ports) from what the machine said.
// ================================================================================================

/// **Which RTC the machine has**, discovered from the device tree's `compatible` (or, on `x86_64`,
/// the CMOS clock behind two fixed I/O ports) and passed to the clock service at spawn.
///
/// The service could have keyed the register layout off `target_arch`, which is what the console
/// driver does for its UART. It does not, because the RTC is where that shortcut runs out: the
/// VisionFive 2 is riscv64 and has neither of these devices, so an ISA-keyed driver would compile
/// clean and read garbage on the first real board. The binding is what the driver actually knows
/// how to drive, so the binding is what it is told. `CMOS` names a register map behind two I/O
/// ports rather than behind a page, and the service reads it the same way it reads the other two.
pub mod rtc {
    /// No RTC in the device tree. The clock service still runs and still serves proposals; it just
    /// starts out not knowing what time it is, which is a state the contract has.
    pub const NONE: u64 = 0;
    /// `arm,pl031`, QEMU `virt` on aarch64 at `0x9010000`. One 32-bit register at offset 0, `DR`,
    /// reading **seconds** since the Unix epoch.
    pub const PL031: u64 = 1;
    /// `google,goldfish-rtc`, QEMU `virt` on riscv64 at `0x101000`. Two 32-bit registers,
    /// `TIME_LOW` at 0 and `TIME_HIGH` at 4, together **nanoseconds** since the Unix epoch. Read
    /// LOW first: it latches HIGH, and reading them the other way round gives a value that is
    /// correct except across the low word's wrap, which is a bug that shows up once every four
    /// seconds and looks like a 4-second jump.
    pub const GOLDFISH: u64 = 2;
    /// **The `x86_64` CMOS RTC**, a clock behind two fixed I/O ports, [`INDEX_PORT`] and
    /// [`DATA_PORT`], with no page. `PL031` and `GOLDFISH` name a register layout the clock
    /// service reads through a mapping it holds; this one names the two ports the service reads
    /// through a `PortRange` capability over both, which the wiring grants with `WRITE` because
    /// the I/O permission bitmap cannot permit `in` without `out`.
    ///
    /// Name and value provisional.
    pub const CMOS: u64 = 3;

    /// The CMOS index port: write a register number here before reaching it at [`DATA_PORT`].
    /// The lower of the two ports a [`CMOS`] binding's `PortRange` names.
    ///
    /// Name: provisional, minted 2026-11-10 (UTC) by lane/cmos-read-in-the-clock-service for
    /// milestone 1062 (the device control planes leave the kernel), step 6.
    pub const INDEX_PORT: u16 = 0x70;

    /// The CMOS data port: reads and writes the register [`INDEX_PORT`] last named.
    ///
    /// Name: provisional, minted 2026-11-10 (UTC) by lane/cmos-read-in-the-clock-service for
    /// milestone 1062 (the device control planes leave the kernel), step 6.
    pub const DATA_PORT: u16 = 0x71;
}

#[cfg(all(test, not(loom)))]
mod tests {
    use super::*;

    /// `buffer` read as a mapped [`ClockPage`] for the length of `f`, and the one place these tests
    /// argue `ClockPage::new`'s contract. The page is lent to `f` by reference; a test that copied it
    /// out would break the argument, and none does.
    fn with_page<R>(buffer: &[AtomicU64; WORDS], f: impl FnOnce(&ClockPage) -> R) -> R {
        // SAFETY: `buffer` is live, sized and aligned for the page by its type, and borrowed for
        // the whole call, so it outlives every use of the page inside `f`.
        f(&unsafe { ClockPage::new(buffer.as_ptr() as u64) })
    }

    /// A time inside the sanity window, used as "now" by the policy tests.
    const NOW: u64 = 1_800_000_000 * NS_PER_SECOND; // 2027-01-15-ish

    #[test]
    fn the_sanity_window_rejects_the_two_lies_that_matter() {
        // 1970 is what a machine with no clock reports, and it is exactly what this crate exists
        // to stop being mistaken for an answer.
        assert!(!policy::is_plausible(0));
        // The far future is where a clock attack aims: past every certificate's expiry.
        assert!(!policy::is_plausible(policy::NOT_AFTER_NANOSECONDS));
        assert!(policy::is_plausible(NOW));
        // 2038, where a 32-bit `time_t` wraps, is deliberately INSIDE the window. It is a real
        // instant this machine may run at, and refusing it would be treating a C bug as a fact
        // about time. Asserted so a future tightening of the ceiling has to mean it.
        assert!(policy::is_plausible(2_147_483_647 * NS_PER_SECOND));
    }

    #[test]
    fn an_unknown_clock_accepts_any_plausible_proposal() {
        // The bootstrap: nothing to step from, so only the window applies.
        assert_eq!(
            policy::decide(state::UNKNOWN, 0, NOW),
            status::ACCEPTED,
            "a machine that does not know the time has no belief a step limit could protect"
        );
        assert_eq!(
            policy::decide(state::UNKNOWN, 0, 0),
            status::REFUSED_IMPLAUSIBLE,
            "but the window still applies, so 1970 is still refused"
        );
    }

    #[test]
    fn a_known_clock_is_stepped_only_within_the_bounds() {
        let known = state::RTC;
        for (delta, want) in [
            (0i64, status::ACCEPTED),
            (
                policy::MAXIMUM_STEP_FORWARD_NANOSECONDS as i64,
                status::ACCEPTED, // exactly the limit is inside it
            ),
            (
                policy::MAXIMUM_STEP_FORWARD_NANOSECONDS as i64 + 1,
                status::REFUSED_TOO_FAR_FORWARD,
            ),
            (
                -(policy::MAXIMUM_STEP_BACKWARD_NANOSECONDS as i64),
                status::ACCEPTED,
            ),
            (
                -(policy::MAXIMUM_STEP_BACKWARD_NANOSECONDS as i64) - 1,
                status::REFUSED_TOO_FAR_BACKWARD,
            ),
        ] {
            let proposed = (NOW as i64 + delta) as u64;
            assert_eq!(
                policy::decide(known, NOW, proposed),
                want,
                "a step of {delta} ns from a known clock"
            );
        }
    }

    /// The std overlay reads the older spellings; they must stay the same value and the same sum.
    #[test]
    fn the_overlay_spellings_agree_with_the_ruled_ones() {
        assert_eq!(NANOS_PER_SEC, NS_PER_SECOND);
        assert_eq!(wall_nanos(40, 2), wall_nanoseconds(40, 2));
        assert_eq!(wall_nanos(u64::MAX, 1), u64::MAX);
    }

    /// The asymmetry is a decision, not an accident, so it gets a test that fails if someone
    /// "tidies" the two constants into one.
    #[test]
    fn backwards_is_held_far_tighter_than_forwards() {
        let known = state::SYNCED;
        let ten_minutes = 600 * NS_PER_SECOND;
        assert_eq!(
            policy::decide(known, NOW, NOW + ten_minutes),
            status::ACCEPTED
        );
        assert_eq!(
            policy::decide(known, NOW, NOW - ten_minutes),
            status::REFUSED_TOO_FAR_BACKWARD,
            "the same magnitude backwards makes instants happen twice, and is refused"
        );
    }

    #[test]
    fn the_offset_round_trips_through_the_wall_clock() {
        let monotonic = 42 * NS_PER_SECOND;
        let offset = offset_for(NOW, monotonic);
        assert_eq!(wall_nanoseconds(offset, monotonic), NOW);
        // And the property the whole design is for: the offset changes, the monotonic input does
        // not, and the two are independent by construction.
        let stepped = offset_for(NOW + 5 * NS_PER_SECOND, monotonic);
        assert_eq!(
            wall_nanoseconds(stepped, monotonic) - NOW,
            5 * NS_PER_SECOND
        );
        assert_eq!(monotonic, 42 * NS_PER_SECOND);
    }

    /// A zeroed frame is a frame nobody published to, and it must read as "unknown" rather than as
    /// "1970". This is the default-honest property, and it is the one a reader gets for free when a
    /// clock service failed to start at all.
    #[test]
    fn a_zeroed_page_reads_as_unknown() {
        let frame = [const { AtomicU64::new(0) }; WORDS];
        with_page(&frame, |page| {
            assert_eq!(
                page.read(),
                Reading {
                    state: state::UNKNOWN,
                    offset_nanos: 0,
                    generation: 0,
                }
            );
            assert!(!state::is_known(page.read().state));
        });
    }

    #[test]
    fn a_published_page_reads_back_and_counts_generations() {
        let frame = [const { AtomicU64::new(0) }; WORDS];
        with_page(&frame, |page| {
            page.initialize();
            assert_eq!(
                page.read().state,
                state::UNKNOWN,
                "init is honest, not 1970"
            );

            assert_eq!(page.publish(state::RTC, 7), 1);
            assert_eq!(
                page.read(),
                Reading {
                    state: state::RTC,
                    offset_nanos: 7,
                    generation: 1,
                }
            );

            assert_eq!(page.publish(state::SYNCED, 9), 2);
            assert_eq!(
                page.read().generation,
                2,
                "a reader can see the clock moved"
            );
        });
    }

    /// The page's whole reason for two copies, stated where a refactor will trip over it: a writer
    /// that stops anywhere inside a publish leaves the last publish readable, at once and whole. A
    /// single-threaded test can call `read` here because a reader no longer waits on anything.
    #[test]
    fn a_writer_stopped_inside_a_publish_leaves_the_last_publish_readable() {
        let frame = [const { AtomicU64::new(0) }; WORDS];
        with_page(&frame, |page| {
            page.initialize();
            page.publish(state::RTC, 7);
            let before = page.read();

            // A writer takes the claim and fills the other copy, then stops for good.
            page.claim();
            assert_eq!(page.fill(state::SYNCED, 11), 2);
            assert_eq!(
                page.read_once(),
                Some(before),
                "a stopped writer must not make a reader retry, let alone wait"
            );
            assert!(
                !page.try_claim(),
                "and it still holds the claim against writers"
            );

            // The writer that resumes finishes the publish the way `publish` would.
            page.flip(2);
            page.release();
            assert_eq!(
                page.read(),
                Reading {
                    state: state::SYNCED,
                    offset_nanos: 11,
                    generation: 2,
                }
            );
        });
    }

    /// The frozen layout, word by word, as literals: a publish writes the copy the new count
    /// names and leaves the other copy holding the publish before it.
    #[test]
    fn the_layout_keeps_its_frozen_words() {
        let frame = [const { AtomicU64::new(0) }; WORDS];
        let words = || frame.each_ref().map(|w| w.load(Ordering::Relaxed));
        with_page(&frame, |page| {
            page.initialize();
            assert_eq!(words(), [MAGIC, 0, 0, 0, 0, 0, 0]);
            page.publish(state::RTC, 7);
            assert_eq!(words(), [MAGIC, 1, 0, 0, 0, state::RTC, 7]);
            page.publish(state::SYNCED, 9);
            assert_eq!(words(), [MAGIC, 2, 0, state::SYNCED, 9, state::RTC, 7]);
        });
        assert_eq!(WORDS, 7);
    }

    /// The request word is a wire format shared with the clock service at runtime; `req` and `operation`
    /// must be inverses AND put the opcode in bits 63:56 exactly, because the service on the other
    /// side of the endpoint decodes with its own copy of the constant. A mutation run
    /// once showed the shift direction and the whole function bodies were pinned by nothing.
    #[test]
    fn the_request_word_is_the_wire_format_it_claims() {
        assert_eq!(propose::req(propose::PROPOSE), 1u64 << 56);
        assert_eq!(
            propose::operation(propose::req(propose::STATE)),
            propose::STATE
        );
        assert_eq!(propose::operation(0), 0);
    }

    /// The frozen identity, written out as literals. A reader built against another value reads
    /// the page as no clock page at all, so a change here is a new version of the interface, never
    /// an edit: a layout break bumps [`VERSION`] and the magic's digits together, and this test
    /// with them, and the id never changes.
    #[test]
    fn the_identity_is_the_one_readers_were_built_against() {
        assert_eq!(MAGIC.to_be_bytes(), *b"CLOCKv01");
        assert_eq!(VERSION, 1);
        assert_eq!(INTERFACE_ID, 8);
    }

    /// The plausibility window's endpoints are seconds times a billion, and the multiplication is
    /// exactly the kind of arithmetic a mutant can quietly break (a `/` there made the floor
    /// microscopic, and every implausible proposal became plausible). Pin the products.
    #[test]
    // Asserting on constants is this test's entire purpose: the constant is the thing a mutant
    // rewrites, and the assertion is what notices.
    #[allow(clippy::assertions_on_constants)]
    fn the_sanity_window_is_where_it_says() {
        assert_eq!(policy::NOT_BEFORE_NANOSECONDS, 1_767_225_600_000_000_000);
        assert_eq!(policy::NOT_AFTER_NANOSECONDS, 4_102_444_800_000_000_000);
        assert!(policy::NOT_BEFORE_NANOSECONDS < policy::NOT_AFTER_NANOSECONDS);
    }
}

// ================================================================================================
// The clock page under loom: every interleaving, and every reordering C11 permits.
// ================================================================================================
#[cfg(all(test, loom))]
mod interleavings {
    //! **The clock page is a sharp weak-memory target.** It is a hand-rolled lock-free protocol
    //! whose reader and writers run in *different address spaces*, with no lock available between
    //! them, and its own documentation says the memory ordering is the point rather than
    //! decoration. On x86 a sloppy version passes forever; on the aarch64 and riscv64 this project
    //! targets it does not.
    //!
    //! The models hold three claims: a reader never returns a torn pair (two harnesses, one of
    //! them across two publishes), a stopped writer never makes a reader wait, and two writers
    //! never interleave.
    //!
    //! Each ordering on the page was weakened in turn and the run repeated: every one fails some
    //! harness except the relaxed release of the claim, which loom passes. That one rests on the
    //! argument at `release`.
    //!
    //! Run with `RUSTFLAGS="--cfg loom" cargo test --release`.

    use loom::thread;

    use super::*;

    /// What a frame holds before `initialize`: whatever its last owner left there, which is
    /// neither the magic nor any state a reader accepts.
    const LEFTOVER: u64 = 0xA5A5_A5A5_A5A5_A5A5;

    /// A clock page backed by loom atomics rather than by a mapped frame.
    ///
    /// The crate's own `ClockPage` is a raw pointer into a shared page, which is exactly right for
    /// something two address spaces map and exactly wrong for a model checker, whose atomics carry
    /// per-execution state and cannot be conjured from an address. Leaking a real array of loom
    /// atomics and pointing the ordinary `ClockPage` at it means **the algorithm under test is the
    /// algorithm that ships**: `read`, `publish` and `initialize` are not reimplemented here, only
    /// the addressing is. The leak is per model execution and is seven words wide.
    ///
    /// Every word starts as [`LEFTOVER`], not zero. A zeroed copy reads as `UNKNOWN` with offset
    /// zero, which is also what `initialize` writes, so on a zeroed frame a reader that got past
    /// the magic too early would read a plausible page and no harness could tell.
    fn page() -> ClockPage {
        let words: &'static [AtomicU64; WORDS] =
            Box::leak(Box::new(core::array::from_fn(|_| AtomicU64::new(LEFTOVER))));
        // SAFETY: `ClockPage::new` wants a mapped, 8-byte-aligned frame of at least `WORDS` words
        // that stays mapped for the value's life. A leaked array of exactly `WORDS` atomics is all
        // three: it is aligned by the type, it is never freed, and `word()` indexes it with the
        // same pointer arithmetic it would use on a frame.
        unsafe { ClockPage::new(words.as_ptr() as u64) }
    }

    /// Runs `read` on a thread of its own, and hands back its handle.
    ///
    /// The reader never stores and never spins, so on the model's main thread it finishes before
    /// a spawned writer has a pending operation, and loom's partial-order reduction then sees no
    /// race to explore: a harness written that way ran once. A reader on its own thread is
    /// pending beside the writer, so every interleaving of the two is explored. Making the flip a
    /// relaxed store is caught this way and was missed the other way.
    fn spawn_reader<T: 'static>(
        page: ClockPage,
        read: impl FnOnce(ClockPage) -> T + 'static,
    ) -> thread::JoinHandle<T> {
        thread::spawn(move || read(page))
    }

    /// Non-vacuity, across executions rather than within one. A model checker explores every
    /// interleaving, and a harness whose interesting branch no execution reaches passes while
    /// checking nothing; each harness marks the branches it expects and asserts, after the model
    /// ends, that some execution reached each one.
    struct Reached(core::sync::atomic::AtomicBool);
    impl Reached {
        const fn new() -> Self {
            Self(core::sync::atomic::AtomicBool::new(false))
        }
        fn mark(&self) {
            self.0.store(true, core::sync::atomic::Ordering::Relaxed);
        }
        fn assert(&self, what: &str) {
            assert!(
                self.0.load(core::sync::atomic::Ordering::Relaxed),
                "vacuous harness: no execution ever reached {what}, so nothing was checked"
            );
        }
    }

    /// **A reader never sees half a publish.**
    ///
    /// The state and the offset are a matched pair (a state of `SYNCED` with the offset from
    /// before the sync is a wrong wall clock, not a crash, which is the worst kind of bug to leave
    /// possible). A reader that races a publish gets the publish before it or the publish itself,
    /// never a blend.
    #[test]
    fn a_reader_never_sees_half_a_publish() {
        static SAW_THE_OLD: Reached = Reached::new();
        static SAW_THE_NEW: Reached = Reached::new();

        loom::model(|| {
            let page = page();
            page.initialize();
            page.publish(state::RTC, 1_000);

            let writer = {
                let p = page;
                thread::spawn(move || p.publish(state::SYNCED, 2_000))
            };

            let r = spawn_reader(page, |p| p.read()).join().unwrap();
            match (r.state, r.offset_nanos) {
                (state::RTC, 1_000) => SAW_THE_OLD.mark(),
                (state::SYNCED, 2_000) => SAW_THE_NEW.mark(),
                other => panic!("a torn reading: {other:?} is neither publish"),
            }

            writer.join().unwrap();
        });

        SAW_THE_OLD.assert("an execution where the reader gets in before the publish");
        SAW_THE_NEW.assert("an execution where the reader gets in after the publish");
    }

    /// **A reader that sleeps across two publishes never returns a torn pair.**
    ///
    /// The second of two publishes rewrites the copy a slow reader is reading, so this is the case
    /// the two copies alone cannot protect: only the full publish count can. The reader here is
    /// `read`'s own loop over `read_once`, unrolled so the harness can mark that a retry happened.
    /// Removing the writer's release fence in `fill`, or the reader's acquire fence in
    /// `read_once`, fails this harness.
    #[test]
    fn a_reader_across_two_publishes_never_returns_a_torn_pair() {
        static RETRIED: Reached = Reached::new();
        static SAW_THE_LAST: Reached = Reached::new();

        loom::model(|| {
            let page = page();
            page.initialize();
            page.publish(state::RTC, 1_000);

            let writer = {
                let p = page;
                thread::spawn(move || {
                    p.publish(state::SYNCED, 2_000);
                    p.publish(state::SET, 3_000);
                })
            };

            let r = spawn_reader(page, |p| {
                loop {
                    match p.read_once() {
                        Some(r) => break r,
                        None => RETRIED.mark(),
                    }
                }
            })
            .join()
            .unwrap();
            match (r.state, r.offset_nanos, r.generation) {
                (state::RTC, 1_000, 1) | (state::SYNCED, 2_000, 2) => {}
                (state::SET, 3_000, 3) => SAW_THE_LAST.mark(),
                other => panic!("a torn reading across two publishes: {other:?}"),
            }

            writer.join().unwrap();
        });

        RETRIED.assert("an execution where a publish finished during a read");
        SAW_THE_LAST.assert("an execution where the reader gets in after both publishes");
    }

    /// **The generation is what a reader compares, so it must count publishes and not retries.**
    ///
    /// `Reading::generation` exists so a log or an expiring cache can ask "did the clock step under
    /// me" without comparing timestamps and guessing. That makes it a value the reader depends on,
    /// not a diagnostic, and it is the same publish count that names the copy and checks the read.
    #[test]
    fn the_generation_a_reader_sees_matches_the_pair_it_read() {
        loom::model(|| {
            let page = page();
            page.initialize();

            let writer = {
                let p = page;
                thread::spawn(move || p.publish(state::RTC, 1_000))
            };

            let r = spawn_reader(page, |p| p.read()).join().unwrap();
            // Generation 0 is the initialized page nobody has published to; 1 is the one publish.
            let expected = if r.state == state::UNKNOWN { 0 } else { 1 };
            assert_eq!(
                r.generation, expected,
                "the generation disagrees with the reading it came with: {r:?}"
            );

            writer.join().unwrap();
        });
    }

    /// **Two writers serialize rather than corrupt.**
    ///
    /// Several processes may hold a read/write mapping, so writers take a claim word with a
    /// compare-exchange before they touch the count or a copy. Two writers that both read the same
    /// count would fill the same copy and publish the same generation; "would corrupt silently" is
    /// a claim about interleavings, which only a model checker can check.
    #[test]
    fn two_writers_serialise_rather_than_corrupt_the_page() {
        static BOTH_LANDED: Reached = Reached::new();

        loom::model(|| {
            let page = page();
            page.initialize();

            let first = {
                let p = page;
                thread::spawn(move || p.publish(state::RTC, 1_000))
            };
            let second = {
                let p = page;
                thread::spawn(move || p.publish(state::SYNCED, 2_000))
            };

            let g1 = first.join().unwrap();
            let g2 = second.join().unwrap();
            assert_ne!(g1, g2, "two publishes were handed the same generation");

            let r = page.read();
            assert_eq!(
                r.generation, 2,
                "two publishes did not advance the page twice"
            );
            let last = if g1 == 2 {
                (state::RTC, 1_000)
            } else {
                (state::SYNCED, 2_000)
            };
            assert_eq!(
                (r.state, r.offset_nanos),
                last,
                "the page does not hold the pair of the writer that published last: {r:?}"
            );
            assert!(
                page.try_claim(),
                "the claim was left held after both writers finished"
            );
            BOTH_LANDED.mark();
        });

        BOTH_LANDED.assert("any execution at all");
    }

    /// **A stopped writer never makes a reader wait.**
    ///
    /// The writer stops for good at each point inside a publish: holding the claim, after filling
    /// the spare copy, and after the flip but before it gives the claim back. While it holds the
    /// claim and has not flipped, a reader's very first attempt succeeds and returns the last
    /// publish. Once it has flipped, the reader may retry once, because a publish finished, and
    /// then reads one publish or the other whole. In every case the claim stays held, so the
    /// stopped writer blocks later writers and nobody else.
    #[test]
    fn a_stopped_writer_never_makes_a_reader_wait() {
        #[derive(Clone, Copy, PartialEq)]
        enum StopsAfter {
            Claiming,
            Filling,
            Flipping,
        }
        static SAW_THE_NEW: Reached = Reached::new();

        for stops_after in [
            StopsAfter::Claiming,
            StopsAfter::Filling,
            StopsAfter::Flipping,
        ] {
            loom::model(move || {
                let page = page();
                page.initialize();
                page.publish(state::RTC, 1_000);
                let before = page.read();

                let writer = {
                    let p = page;
                    thread::spawn(move || {
                        p.claim();
                        if stops_after == StopsAfter::Claiming {
                            return;
                        }
                        let generation = p.fill(state::SYNCED, 2_000);
                        if stops_after == StopsAfter::Filling {
                            return;
                        }
                        p.flip(generation);
                        // And the writer stops here, the claim never given back.
                    })
                };

                if stops_after == StopsAfter::Flipping {
                    let r = spawn_reader(page, |p| p.read()).join().unwrap();
                    match (r.state, r.offset_nanos, r.generation) {
                        (state::RTC, 1_000, 1) => {}
                        (state::SYNCED, 2_000, 2) => SAW_THE_NEW.mark(),
                        other => panic!("a torn reading beside a stopped writer: {other:?}"),
                    }
                } else {
                    assert_eq!(
                        spawn_reader(page, |p| p.read_once()).join().unwrap(),
                        Some(before),
                        "a writer that never flipped made a reader retry"
                    );
                }

                writer.join().unwrap();
                assert!(
                    !page.try_claim(),
                    "a later writer took the claim a stopped writer still holds"
                );
            });
        }

        SAW_THE_NEW.assert("an execution where the reader gets in after the stopped writer's flip");
    }

    /// **A page is recognizable only once it is whole.**
    ///
    /// `initialize` writes the magic last, with a release, precisely so a reader racing the first
    /// publish sees either a page it does not recognize (and reports `UNKNOWN`, which is the truth
    /// about a frame nobody has published to) or a fully initialized one. Never a recognized page
    /// with garbage in it. The release is the whole mechanism, and this harness checks it.
    #[test]
    fn a_racing_reader_sees_an_unrecognised_page_or_a_whole_one() {
        static SAW_UNRECOGNISED: Reached = Reached::new();
        static SAW_INITIALISED: Reached = Reached::new();

        loom::model(|| {
            let page = page();

            let writer = {
                let p = page;
                thread::spawn(move || {
                    p.initialize();
                    p.publish(state::RTC, 1_000);
                })
            };

            let r = spawn_reader(page, |p| p.read()).join().unwrap();
            match (r.state, r.offset_nanos) {
                (state::UNKNOWN, 0) => SAW_UNRECOGNISED.mark(),
                (state::RTC, 1_000) => SAW_INITIALISED.mark(),
                other => panic!("a reader saw a recognized page with garbage in it: {other:?}"),
            }

            writer.join().unwrap();
        });

        SAW_UNRECOGNISED.assert("an execution where the reader arrives before the magic lands");
        SAW_INITIALISED.assert("an execution where the reader arrives after the publish");
    }
}
