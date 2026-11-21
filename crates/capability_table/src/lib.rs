//! Capability tables: a file descriptor table that can point at anything.
//!
//! The table lives in the kernel's memory and a program sees only a slot number, so the
//! unforgeability is an array and a bounds check. Pure logic, host-tested and Kani-proved; the
//! owner supplies the object type and picks the size, which is part of the type, so creating a table
//! allocates nothing.
//!
//! # Examples
//!
//! Rights narrow by intersection, so a capability passed on never carries more than was held.
//!
//! ```
//! use capability_table::Rights;
//!
//! let held = Rights::READ.union(Rights::WRITE);
//! assert!(held.allows(Rights::READ));
//! assert!(!held.allows(Rights::GRANT));
//!
//! // Handing it on with a mask can only ever narrow it.
//! let passed_on = held.intersect(Rights::READ.union(Rights::GRANT));
//! assert_eq!(passed_on, Rights::READ);
//! assert!(passed_on.is_subset_of(held));
//! ```
//!
//! No operation widens:
//!
//! ```
//! use capability_table::Rights;
//!
//! let read_only = Rights::READ;
//! // The best you can do with ALL on the other side is get back what you already had.
//! assert_eq!(read_only.intersect(Rights::ALL), read_only);
//! assert!(!read_only.intersect(Rights::ALL).allows(Rights::WRITE));
//! ```
//!
//! Name: ratified 2026-08-01 (calef, milestone 63), replacing `caps`. Refused `caps` (a container's
//! name for what is the capability model: it exports `Capability`, `Rights`, `Object`, `Reap` and
//! `CapabilityTable`) and `cap_space` (names one of five exports, and stutters as `cap_space::CapabilityTable`).
//! The exported type was originally kept as `CSpace`, seL4's own spelling; DECISIONS §113's
//! amendment (calef, 2026-08-25) renamed it to `CapabilityTable`, the plain term this crate's own
//! prose already reached for ("A thread's capability table," above).

#![no_std]

/// What you may do with a capability.
///
/// Rights only narrow: [`CapabilityTable::derive`] cannot widen them, because the type offers no
/// operation that would.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rights(u32);

impl Rights {
    /// No rights at all.
    pub const NONE: Rights = Rights(0);
    /// The right to read the object's contents.
    pub const READ: Rights = Rights(1 << 0);
    /// The right to modify the object's contents.
    pub const WRITE: Rights = Rights(1 << 1);

    /// The right to **pass this capability on**. Without it, you may use a thing and not lend it,
    /// which makes delegation a decision rather than an accident.
    pub const GRANT: Rights = Rights(1 << 2);

    /// The right to **learn what exists**, as distinct from acting on it: the table's twin of a
    /// directory's `ENUMERATE`.
    ///
    /// A domain names its members and never acts on them, so a survey takes this rather than the
    /// `READ` that a receive and a reap take. An owner decides which objects consult it.
    pub const ENUMERATE: Rights = Rights(1 << 3);

    /// **The right to add a thread to a process.** Held on a process capability, it lets a thread
    /// under construction join that process without consuming the capability. An owner decides
    /// which objects consult it.
    pub const BIND: Rights = Rights(1 << 4);

    /// **Every defined bit.** [`from_bits`](Self::from_bits) masks against this, so a right that is
    /// missing here is silently dropped at every delegation: adding a constant above without
    /// widening this mask produces a right that cannot survive being passed on.
    pub const ALL: Rights = Rights::READ
        .union(Rights::WRITE)
        .union(Rights::GRANT)
        .union(Rights::ENUMERATE)
        .union(Rights::BIND);

    /// The raw bit pattern, for carrying rights across a boundary as an integer (a system call
    /// register). [`from_bits`](Self::from_bits) is the inverse.
    pub const fn bits(self) -> u32 {
        self.0
    }

    /// Rebuild rights from a raw bit pattern, keeping only defined bits: the inverse of
    /// [`bits`](Self::bits), for rights that crossed a boundary in a system call register. Undefined
    /// bits are dropped, so a caller cannot conjure a right that does not exist.
    pub const fn from_bits(bits: u32) -> Rights {
        Rights(bits & Rights::ALL.0)
    }

    /// Every right either side holds.
    pub const fn union(self, other: Rights) -> Rights {
        Rights(self.0 | other.0)
    }

    /// Only the rights both sides hold. The one operation on the delegation path: it can only
    /// narrow, never widen, which is the monotonicity the whole model rests on.
    pub const fn intersect(self, other: Rights) -> Rights {
        Rights(self.0 & other.0)
    }

    /// Do I hold *at least* these rights?
    pub const fn allows(self, needed: Rights) -> bool {
        self.0 & needed.0 == needed.0
    }

    /// Is `self` no more than `other`? The subset test that [`CapabilityTable::derive`] turns on.
    pub const fn is_subset_of(self, other: Rights) -> bool {
        self.0 & !other.0 == 0
    }
}

/// A capability: **an object, and what you may do with it.**
///
/// Name: provisional, minted 2026-10-27 (UTC) by lane capability-sweep-6. Spelled out from
/// `Cap` under the capability ruling of 2026-10-06 (UTC).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capability<O> {
    /// What this capability names.
    pub object: O,
    /// What the holder may do with it.
    pub rights: Rights,
}

impl<O: Copy> Capability<O> {
    /// **Pass this capability on with `rights`, or refuse.** The one delegation rule: the holder
    /// must hold [`Rights::GRANT`], and the copy may only narrow. The copy names the same object.
    /// [`CapabilityTable::derive`] files the result, and an owner that reads its source under a
    /// lock of its own calls this on what it read, so the rule has one body.
    ///
    /// Without `GRANT` the answer is [`Error::GrantRequired`], whatever `rights` asks for. With it,
    /// asking for a right the source lacks is [`Error::CannotWiden`], not a silent clamp.
    ///
    /// Name: ratified 2026-11-19 (UTC), with `Error::GrantRequired`. Refused `derive`, which is the
    /// table's narrowing copy into a slot; this one is a capability's copy of itself.
    pub fn delegate(&self, rights: Rights) -> Result<Capability<O>, Error> {
        if !self.rights.allows(Rights::GRANT) {
            return Err(Error::GrantRequired);
        }
        if !rights.is_subset_of(self.rights) {
            return Err(Error::CannotWiden);
        }
        Ok(Capability {
            object: self.object,
            rights,
        })
    }
}

impl<O> Capability<O> {
    /// **Mint a fresh capability to `object` with exactly this one's rights.** The mint site for
    /// authority created outside [`CapabilityTable::derive`], such as carving a child budget out of
    /// a parent. The harness `split_never_widens_rights` proves it, so a spend-only (`GRANT`-less)
    /// budget cannot split itself a `GRANT`-bearing child.
    pub fn mint_child(&self, object: O) -> Capability<O> {
        Capability {
            object,
            rights: self.rights,
        }
    }
}

/// Why a capability table operation was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Error {
    /// **The slot is empty.** Note what this is *not*: it is not "permission denied." There is
    /// nothing there. The thing you tried to name does not exist, for you.
    NoSuchSlot,
    /// You asked to derive a capability with rights you do not hold.
    CannotWiden,
    /// **The capability may not be passed on.** Its holder lacks [`Rights::GRANT`].
    ///
    /// Name: ratified 2026-11-19 (UTC).
    GrantRequired,
    /// The table is full.
    NoFreeSlot,
    /// **The slot asked for is already occupied.** Distinct from [`NoFreeSlot`](Self::NoFreeSlot),
    /// which says the whole table is full.
    ///
    /// Name: ratified 2026-11-19 (UTC).
    SlotOccupied,
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Error::NoSuchSlot => "the slot is empty or out of range",
            Error::CannotWiden => "the rights asked for are not all held",
            Error::GrantRequired => "the capability may not be passed on",
            Error::NoFreeSlot => "the table is full",
            Error::SlotOccupied => "the slot is already occupied",
        })
    }
}

impl core::error::Error for Error {}

/// **The most slots a [`CapabilityTable`] may have: one per bit of its free-slot word.**
///
/// The word is a `u64`, so [`CapabilityTable::new`] asserts `N <= 64` at compile time, and an owner
/// that fixes its own table size asserts it again beside that constant. Growing past 64 is a wider
/// word or a second one.
///
/// Name: ratified 2026-10-27 (#2797).
pub const MAXIMUM_SLOTS: usize = u64::BITS as usize;

pub use storage::CapabilityTable;

/// **The only code that can touch a slot**, and that is the mechanism rather than a convention.
///
/// The free-slot word, the occupancy count and the high-water mark are private to this module,
/// which changes a slot only through [`fill`](CapabilityTable::fill) and
/// [`empty`](CapabilityTable::empty). Every other mutator is written in terms of those two, so it
/// cannot forget the bookkeeping. The harnesses `the_free_mask_is_the_empty_slots` and
/// `the_count_is_the_slots` check the two doors against the array.
mod storage {
    #[cfg(target_has_atomic = "32")]
    use super::note_peak;
    use super::{Capability, MAXIMUM_SLOTS};

    /// A thread's capability table: a flat array, an fd table with a type tag on each entry,
    /// rather than seL4's tree of CNodes.
    pub struct CapabilityTable<O, const N: usize> {
        slots: [Option<Capability<O>>; N],
        /// **Bit `i` set means slot `i` is empty**, so `insert`'s first free slot is one
        /// `trailing_zeros`, and bits at and above `N` stay clear, so a full table is a zero word.
        /// A `u64`; the table is `N` slots, this word and two `u16` counts, rounded to eight bytes.
        free: u64,
        /// How many of the slots are occupied right now. Maintained by the two doors rather than
        /// counted on demand, because the thing that wants it is a high-water mark and a mark
        /// sampled at a convenient moment is not one.
        used: u16,
        /// The largest `used` this table has ever held. **Never decreases**, which is the whole
        /// point: the number a boot reports is what it *reached*, not what it happens to be holding
        /// when somebody looks. See [`peak`](Self::peak) and [`highest_seen`](super::highest_seen).
        peak: u16,
    }

    impl<O: Copy, const N: usize> CapabilityTable<O, N> {
        /// Every slot free: the low `N` bits. The assertion is the compile-time wall
        /// [`MAXIMUM_SLOTS`] describes, evaluated for each `N` that is instantiated.
        const ALL_FREE: u64 = {
            assert!(
                N <= MAXIMUM_SLOTS,
                "a capability table has one free-slot bit per slot, and the word is a u64"
            );
            if N == MAXIMUM_SLOTS {
                u64::MAX
            } else {
                (1u64 << N) - 1
            }
        };

        /// **A brand-new process holds nothing**: it can name nothing until somebody hands it
        /// something.
        pub const fn new() -> Self {
            CapabilityTable {
                slots: [const { None }; N],
                free: Self::ALL_FREE,
                used: 0,
                peak: 0,
            }
        }

        /// The capability in `slot`, or `None` for an empty or out-of-range one.
        pub(super) fn slot(&self, slot: usize) -> Option<Capability<O>> {
            self.slots.get(slot).copied().flatten()
        }

        /// The lowest-numbered empty slot, or `None` when the table is full. One count of trailing
        /// zeros, whatever `N` is.
        pub(super) fn first_free(&self) -> Option<usize> {
            if self.free == 0 {
                None
            } else {
                Some(self.free.trailing_zeros() as usize)
            }
        }

        /// The lowest empty slot at or above `floor`, or `None` when there is none.
        pub(super) fn first_free_from(&self, floor: usize) -> Option<usize> {
            let above = if floor >= 64 {
                0
            } else {
                self.free & (u64::MAX << floor)
            };
            (above != 0).then(|| above.trailing_zeros() as usize)
        }

        /// The free-slot word itself, for the proofs and the tests that check it against the array.
        #[cfg_attr(not(any(test, kani)), allow(dead_code))]
        pub(super) fn free_mask(&self) -> u64 {
            self.free
        }

        /// The occupied slots, as a mask: `free`'s complement, restricted to the `N` bits that
        /// exist (inverting `free` alone would set the bits above `N`).
        /// [`delete_matching`](super::CapabilityTable::delete_matching) walks this so a sweep
        /// costs a table's occupancy, not its capacity.
        pub(super) fn occupied_mask(&self) -> u64 {
            !self.free & Self::ALL_FREE
        }

        /// Whether every slot is empty: one compare, because the word already knows.
        pub(super) fn all_free(&self) -> bool {
            self.free == Self::ALL_FREE
        }

        /// **Door one: put `capability` in `slot`**, occupied or not. `None` if `slot` is out of range;
        /// otherwise whether it was empty before. Clears the slot's free bit, and if the slot was
        /// empty, counts the growth and raises the high-water mark.
        pub(super) fn fill(&mut self, slot: usize, capability: Capability<O>) -> Option<bool> {
            let s = self.slots.get_mut(slot)?;
            let was_empty = s.is_none();
            *s = Some(capability);
            self.free &= !(1u64 << slot);
            if was_empty {
                // Wrapping, because the wrap cannot happen and this is on every reply-capability
                // mint (release builds check overflow). `used` counts
                // occupied slots (`the_count_is_the_slots` proves it), this slot was empty, so the
                // result is at most `N`, which the `u64` free mask bounds at 64.
                self.used = self.used.wrapping_add(1);
                if self.used > self.peak {
                    self.peak = self.used;
                    #[cfg(target_has_atomic = "32")]
                    note_peak(self.peak, N);
                }
            }
            Some(was_empty)
        }

        /// **Door two: take whatever is in `slot`**, leaving it empty. `None` if `slot` is out of
        /// range or already empty, and then nothing changes. Sets the slot's free bit and counts
        /// the loss. `peak` is untouched: a mark that fell when the table drained would report
        /// what a boot holds, not what it reached.
        pub(super) fn empty(&mut self, slot: usize) -> Option<Capability<O>> {
            let capability = self.slots.get_mut(slot)?.take()?;
            self.free |= 1u64 << slot;
            self.used -= 1;
            Some(capability)
        }

        /// How many slots are occupied right now.
        pub fn used(&self) -> usize {
            self.used as usize
        }

        /// **The most slots this table has ever held at once**, which is the number a capacity
        /// decision is actually made against. `used` at any given moment is whatever the last few
        /// calls left behind; this is the wall the table came closest to.
        pub fn peak(&self) -> usize {
            self.peak as usize
        }
    }
}

impl<O: Copy, const N: usize> Default for CapabilityTable<O, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<O: Copy, const N: usize> CapabilityTable<O, N> {
    /// The table's fixed capacity, `N`: how many slots it has, occupied or not. Never changes; a
    /// capability table's size is part of its type. [`used`](Self::used) counts the occupied ones.
    ///
    /// Name: ratified 2026-11-19 (UTC), as std's `Vec::capacity`. Refused `len`, which in std counts
    /// the elements held.
    pub const fn capacity(&self) -> usize {
        N
    }

    /// Whether every slot is currently empty.
    pub fn is_empty(&self) -> bool {
        self.all_free()
    }

    /// Look up a slot. **The entire security mechanism, and it is a bounds check.**
    pub fn get(&self, slot: u64) -> Result<Capability<O>, Error> {
        // A slot that does not fit `usize` names nothing, on any target: truncating it would let
        // `2^32 + k` name slot `k` where `usize` is 32 bits.
        let slot = usize::try_from(slot).map_err(|_| Error::NoSuchSlot)?;
        self.slot(slot).ok_or(Error::NoSuchSlot)
    }

    /// Put a capability in the first free slot. Used at spawn, to hand a process its world, and
    /// on every `CALL` for the one-shot Reply, which is why it is one `trailing_zeros` and not a
    /// scan (see the free-slot word's own doc).
    pub fn insert(&mut self, capability: Capability<O>) -> Result<u64, Error> {
        let slot = self.first_free().ok_or(Error::NoFreeSlot)?;
        self.fill(slot, capability);
        Ok(slot as u64)
    }

    /// Put a capability in the **first free slot at or above `floor`**. [`insert`](Self::insert) is
    /// `floor` 0. A program's low slots can be fixed by number, with an empty one meaning "not
    /// granted", so what the program makes for itself must land above them.
    pub fn insert_at_or_above(
        &mut self,
        floor: u64,
        capability: Capability<O>,
    ) -> Result<u64, Error> {
        let floor = usize::try_from(floor).unwrap_or(usize::MAX);
        let slot = self.first_free_from(floor).ok_or(Error::NoFreeSlot)?;
        self.fill(slot, capability);
        Ok(slot as u64)
    }

    /// Put a capability in a **specific free slot**, refusing an occupied or out-of-range one.
    ///
    /// The targeted counterpart of [`insert`](Self::insert). A supervisor uses it to place a
    /// child's supervision rendezvous in a reserved slot rather than wherever first-free would fall,
    /// and refusing an occupied slot keeps that reservation honest: two things must never share the
    /// slot the owner reads when the child starts.
    pub fn insert_at(&mut self, slot: u64, capability: Capability<O>) -> Result<u64, Error> {
        if slot >= N as u64 {
            return Err(Error::NoSuchSlot);
        }
        if self.slot(slot as usize).is_some() {
            return Err(Error::SlotOccupied);
        }
        self.fill(slot as usize, capability);
        Ok(slot)
    }

    /// Put a capability in a specific slot, replacing whatever was there. This is the door for
    /// building a table to a known shape (a loader, the proof builder); it consults no rights, so
    /// authority moves through [`derive`](Self::derive) and not through this.
    pub fn put(&mut self, slot: u64, capability: Capability<O>) -> Result<(), Error> {
        let slot = usize::try_from(slot).map_err(|_| Error::NoSuchSlot)?;
        self.fill(slot, capability).ok_or(Error::NoSuchSlot)?;
        Ok(())
    }

    /// **Copy a capability into another slot, with rights that are no greater.**
    ///
    /// The only way authority moves between slots, by [`Capability::delegate`]: the source must
    /// hold [`Rights::GRANT`] ([`Error::GrantRequired`]), and asking for rights it does not hold is
    /// [`Error::CannotWiden`], not a silent clamp: a loader that quietly grants less than was asked
    /// for is a loader nobody can reason about.
    pub fn derive(&mut self, from: u64, to: u64, rights: Rights) -> Result<(), Error> {
        let copy = self.get(from)?.delegate(rights)?;
        self.put(to, copy)
    }

    /// Drop a capability. The object may still exist; **we simply can no longer name it.**
    pub fn delete(&mut self, slot: u64) -> Result<(), Error> {
        let slot = usize::try_from(slot).map_err(|_| Error::NoSuchSlot)?;
        self.empty(slot).ok_or(Error::NoSuchSlot)?;
        Ok(())
    }

    /// **The occupied slots, lowest first, as `(slot, capability)`**: a read-only walk that costs a
    /// table's occupancy and not its capacity, because it follows the occupied mask as
    /// [`delete_matching`](Self::delete_matching) does.
    ///
    /// Name: ratified 2026-11-19 (UTC).
    pub fn iter(&self) -> impl Iterator<Item = (u64, Capability<O>)> + '_ {
        let mut occupied = self.occupied_mask();
        core::iter::from_fn(move || {
            while occupied != 0 {
                let slot = occupied.trailing_zeros() as usize;
                occupied &= occupied - 1; // clear the lowest set bit; each slot visited once
                if let Some(capability) = self.slot(slot) {
                    return Some((slot as u64, capability));
                }
            }
            None
        })
    }

    /// Delete every occupied slot whose object satisfies `matches`, in place: the revocation
    /// sweeps' primitive, behaviorally a `get`-then-`delete` per slot. It reads each slot once and
    /// walks the occupied mask, not `0..N`, because a sweep visits every thread's table for every
    /// frame a revocation touches and should cost what a table holds, not its capacity.
    pub fn delete_matching(&mut self, mut matches: impl FnMut(&O) -> bool) {
        let mut occupied = self.occupied_mask();
        while occupied != 0 {
            let slot = occupied.trailing_zeros() as usize;
            occupied &= occupied - 1; // clear the lowest set bit; each slot visited once
            if matches!(self.slot(slot), Some(c) if matches(&c.object)) {
                self.empty(slot);
            }
        }
    }
}

/// **The high-water mark across every capability table in this binary, and the ceiling it was
/// measured against**, packed as `peak << 16 | ceiling` so a reader gets a pair true at one instant.
///
/// A reader sees the mark move without walking every table under a lock. [`note_peak`] saturates.
///
/// # BUGS
/// - It does not say which table: a `static` cannot be keyed by a const generic. An owner with one
///   instantiation reads the mark directly; a host test binary sees whichever climbed highest, and
///   `ceiling` tells which.
#[cfg(target_has_atomic = "32")]
static PEAK: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);

/// Raise [`PEAK`] if `peak` beats what is recorded. Called from [`CapabilityTable`]'s one private
/// growth path, so it cannot be forgotten.
#[cfg(target_has_atomic = "32")]
fn note_peak(peak: u16, ceiling: usize) {
    use core::sync::atomic::Ordering;
    let ceiling = core::cmp::min(ceiling, u16::MAX as usize) as u32;
    let packed = ((peak as u32) << 16) | ceiling;
    // `fetch_max` rather than a compare-exchange loop: the packed word puts the peak in the high
    // half, so numeric max IS max-by-peak, and the ceiling that rides along is the one belonging to
    // the table that set the record.
    PEAK.fetch_max(packed, Ordering::Relaxed);
}

/// **The highest occupancy any capability table has reached, and that table's capacity.**
///
/// `(0, 0)` before anything has ever been inserted. One number for every instantiation linked
/// into the binary; see `PEAK`'s BUGS.
#[cfg(target_has_atomic = "32")]
pub fn highest_seen() -> (usize, usize) {
    let packed = PEAK.load(core::sync::atomic::Ordering::Relaxed);
    ((packed >> 16) as usize, (packed & 0xffff) as usize)
}

/// Machine-checked proofs of the capability model.
///
/// Each harness proves its property for every input, symbolically: `derive_never_widens_rights`
/// covers all 2^32 source and requested rights at once. Behind `#[cfg(kani)]`, so only `cargo
/// kani` builds it.
#[cfg(kani)]
mod verification {
    use super::*;

    /// Every capability is a subset of itself: the reflexive base case of the derivation order.
    /// Falsification: replayable `crates/capability_table/falsifications/verification.subset_is_reflexive.patch`
    #[kani::proof]
    fn subset_is_reflexive() {
        let a = Rights(kani::any());
        assert!(a.is_subset_of(a));
    }

    /// **Rights cannot be laundered through a chain.** If B is derived from A and C from B, then C
    /// is no more than A. This is why a *flat* subset check suffices and we never need to walk a
    /// derivation tree to bound a capability: subset is transitive, so the chain can only narrow.
    /// Falsification: replayable `crates/capability_table/falsifications/verification.subset_is_transitive.patch`
    #[kani::proof]
    fn subset_is_transitive() {
        let (a, b, c) = (
            Rights(kani::any()),
            Rights(kani::any()),
            Rights(kani::any()),
        );
        kani::assume(a.is_subset_of(b));
        kani::assume(b.is_subset_of(c));
        assert!(a.is_subset_of(c));
    }

    /// **Userspace cannot forge a right.** `from_bits` takes an attacker-controlled system call register
    /// (any u32) and the result holds only defined rights, for every possible input.
    /// Falsification: replayable `crates/capability_table/falsifications/verification.from_bits_cannot_forge_a_right.patch`
    #[kani::proof]
    fn from_bits_cannot_forge_a_right() {
        let raw: u32 = kani::any();
        assert!(Rights::from_bits(raw).is_subset_of(Rights::ALL));
    }

    /// The two ways of asking the question agree: "a is no more than b" is exactly "b holds at
    /// least a". Proving them equivalent means a bug in one would show up against the other.
    /// Falsification: replayable `crates/capability_table/falsifications/verification.subset_matches_allows.patch`
    #[kani::proof]
    fn subset_matches_allows() {
        let (a, b) = (Rights(kani::any()), Rights(kani::any()));
        assert_eq!(a.is_subset_of(b), b.allows(a));
    }

    /// A small table in an arbitrary state: every slot independently empty or holding a capability
    /// with symbolic object and rights. Three slots is enough to exhibit "the slot in question, a
    /// different slot, and an empty one" in every combination.
    fn any_small_capability_table() -> CapabilityTable<u8, 3> {
        let mut cs: CapabilityTable<u8, 3> = CapabilityTable::new();
        for slot in 0..3u64 {
            if kani::any() {
                cs.put(
                    slot,
                    Capability {
                        object: kani::any(),
                        rights: Rights(kani::any()),
                    },
                )
                .unwrap();
            }
        }
        cs
    }

    /// **The occupancy count is the slots, for every reachable table state.**
    ///
    /// A drifted count reports a high-water mark that was never true. Covers both freeing paths,
    /// `delete` and `delete_matching`; `any_small_capability_table` builds through `put`.
    ///
    /// `#[kani::unwind(4)]` is N + 1 for this harness's N = 3: every loop is an exact `0..3` or the
    /// sweep's bit-scan, which clears one of the N occupied bits per iteration, and the unwinding
    /// assertion fails if any loop could run a fourth time.
    ///
    /// Falsification: replayable `crates/capability_table/falsifications/verification.the_count_is_the_slots.patch`
    #[kani::proof]
    #[kani::unwind(4)]
    fn the_count_is_the_slots() {
        fn occupied(cs: &CapabilityTable<u8, 3>) -> usize {
            (0..3).filter(|&i| cs.slot(i).is_some()).count()
        }
        let mut cs = any_small_capability_table();
        assert_eq!(cs.used(), occupied(&cs), "the count is wrong at the start");

        // The floor insert and a second `put` over a slot that may be occupied.
        let _ = cs.insert_at_or_above(
            kani::any(),
            Capability {
                object: kani::any(),
                rights: Rights(kani::any()),
            },
        );
        assert_eq!(
            cs.used(),
            occupied(&cs),
            "insert_at_or_above left the count behind"
        );
        let _ = cs.put(
            kani::any(),
            Capability {
                object: kani::any(),
                rights: Rights(kani::any()),
            },
        );
        assert_eq!(
            cs.used(),
            occupied(&cs),
            "a second put left the count behind"
        );

        let slot: u64 = kani::any();
        let _ = cs.delete(slot);
        assert_eq!(cs.used(), occupied(&cs), "delete left the count behind");

        // The sweep, which frees any number of slots in one pass and is the other freeing path.
        let wanted: u8 = kani::any();
        cs.delete_matching(|o| *o == wanted);
        assert_eq!(
            cs.used(),
            occupied(&cs),
            "delete_matching left the count behind"
        );

        // And the mark never fell while all that was happening, which is what makes it a mark.
        assert!(cs.peak() >= cs.used());
    }

    /// **The free-slot word is the set of empty slots, after every mutator.**
    ///
    /// `insert` believes the word: "free" over an occupied slot overwrites a live capability, and
    /// "occupied" over an empty one leaks it. Bits at and above `N` must stay clear too, or a full
    /// table would not be a zero word. Checked over every small-table state and slot index, through
    /// every mutator. `#[kani::unwind(4)]`, by `the_count_is_the_slots`'s argument.
    ///
    /// Falsification: replayable `crates/capability_table/falsifications/verification.the_free_mask_is_the_empty_slots.patch`
    #[kani::proof]
    #[kani::unwind(4)]
    fn the_free_mask_is_the_empty_slots() {
        fn empties(cs: &CapabilityTable<u8, 3>) -> u64 {
            (0..3)
                .filter(|&i| cs.slot(i).is_none())
                .fold(0, |m, i| m | (1 << i))
        }
        let capability = || Capability {
            object: kani::any(),
            rights: Rights(kani::any()),
        };
        let mut cs = any_small_capability_table();
        assert_eq!(cs.free_mask(), empties(&cs), "wrong after put");

        let _ = cs.insert(capability());
        assert_eq!(cs.free_mask(), empties(&cs), "wrong after insert");

        let _ = cs.insert_at_or_above(kani::any(), capability());
        assert_eq!(
            cs.free_mask(),
            empties(&cs),
            "wrong after insert_at_or_above"
        );

        let _ = cs.put(kani::any(), capability());
        assert_eq!(cs.free_mask(), empties(&cs), "wrong after a second put");

        let _ = cs.delete(kani::any());
        assert_eq!(cs.free_mask(), empties(&cs), "wrong after delete");

        let _ = cs.insert_at(kani::any(), capability());
        assert_eq!(cs.free_mask(), empties(&cs), "wrong after insert_at");

        let _ = cs.derive(kani::any(), kani::any(), Rights(kani::any()));
        assert_eq!(cs.free_mask(), empties(&cs), "wrong after derive");

        let wanted: u8 = kani::any();
        cs.delete_matching(|o| *o == wanted);
        assert_eq!(cs.free_mask(), empties(&cs), "wrong after delete_matching");
        assert_eq!(cs.is_empty(), empties(&cs) == 0b111);
    }

    /// **The high-water mark never falls across any operation, and a floor insert never lands
    /// below its floor.** One symbolic operation of any kind, applied to a symbolic table.
    /// `#[kani::unwind(4)]`, by `the_count_is_the_slots`'s argument.
    ///
    /// Falsification: replayable `crates/capability_table/falsifications/verification.the_peak_never_falls_and_a_floor_insert_stays_above_its_floor.patch`
    #[kani::proof]
    #[kani::unwind(4)]
    fn the_peak_never_falls_and_a_floor_insert_stays_above_its_floor() {
        let capability = Capability {
            object: kani::any(),
            rights: Rights(kani::any()),
        };
        let mut cs = any_small_capability_table();
        let before = cs.peak();
        let floor: u64 = kani::any();
        let wanted: u8 = kani::any();
        let operation: u8 = kani::any();
        kani::assume(operation < 5);
        let mut landed = None;
        match operation {
            0 => landed = cs.insert_at_or_above(floor, capability).ok(),
            1 => drop(cs.insert(capability)),
            2 => drop(cs.delete(kani::any())),
            3 => drop(cs.insert_at(kani::any(), capability)),
            _ => cs.delete_matching(|o| *o == wanted),
        }
        assert!(cs.peak() >= before, "the mark fell");
        if let Some(slot) = landed {
            assert!(slot >= floor, "a floor insert landed below its floor");
        }
    }

    /// **Consume-on-use is final.** For every table state and every slot (in bounds or not), once
    /// `delete` succeeds the slot answers `NoSuchSlot` to both `get` and a second `delete`. This is
    /// the mechanism that makes a one-shot reply capability one-shot: the caller of `delete` removes
    /// it the instant it is invoked.
    /// Falsification: replayable `crates/capability_table/falsifications/verification.a_deleted_capability_stays_deleted.patch`
    #[kani::proof]
    fn a_deleted_capability_stays_deleted() {
        let mut cs = any_small_capability_table();
        let slot: u64 = kani::any();
        if cs.delete(slot).is_ok() {
            // The storage itself, before the accessors, so the claim rests on the array rather than
            // on two readers of it.
            assert!(
                cs.slot(slot as usize).is_none(),
                "the slot still holds a capability"
            );
            assert_eq!(cs.get(slot).err(), Some(Error::NoSuchSlot));
            assert_eq!(cs.delete(slot).err(), Some(Error::NoSuchSlot));
        }
    }

    /// **Delete is slot-local.** Deleting any slot, in bounds or not, leaves every other slot
    /// exactly as it was. A server holding one-shot Reply capabilities for two callers consumes
    /// one and must still hold the other, or answering caller A would silently orphan caller B.
    /// Falsification: replayable `crates/capability_table/falsifications/verification.delete_touches_only_its_slot.patch`
    #[kani::proof]
    fn delete_touches_only_its_slot() {
        let mut cs = any_small_capability_table();
        let victim: u64 = kani::any();
        let other: u64 = kani::any();
        kani::assume(victim != other);
        kani::assume(other < 3);

        let before = cs.get(other);
        let _ = cs.delete(victim);
        assert_eq!(cs.get(other), before);
    }

    /// **The central theorem, on the real `CapabilityTable::derive`.** For any source rights and any request,
    /// if the derive succeeds then the capability it stored holds no more than the source did, and
    /// holds exactly what was asked (no silent grant of more). There is no reachable input that
    /// widens authority.
    /// Falsification: replayable `crates/capability_table/falsifications/verification.derive_never_widens_rights.patch`
    #[kani::proof]
    fn derive_never_widens_rights() {
        let source_rights = Rights(kani::any());
        let requested = Rights(kani::any());

        let mut cs: CapabilityTable<u8, 2> = CapabilityTable::new();
        cs.put(
            0,
            Capability {
                object: 0u8,
                rights: source_rights,
            },
        )
        .unwrap();

        if cs.derive(0, 1, requested).is_ok() {
            let derived = cs
                .slot(1)
                .expect("a successful derive filled the destination slot");
            // In raw bits, not through `is_subset_of`: `derive` guards on that predicate, so a
            // claim phrased through it would hold for any consistently wrong `is_subset_of`.
            assert_eq!(
                derived.rights.0 & !source_rights.0,
                0,
                "a derived capability holds a right the source did not",
            );
            assert_eq!(
                requested.0 & !source_rights.0,
                0,
                "a request wider than the source got past the guard",
            );
            // And it holds exactly what was asked: no silent grant of more, no silent clamp to
            // less. Field equality, so no predicate stands between the claim and the bits.
            assert_eq!(derived.rights.0, requested.0);
            assert_eq!(derived.object, 0u8);
        }
    }

    /// **Delegation needs `GRANT`, and never widens.** For every one of the 2^32 source rights and
    /// every request, `delegate` succeeds only when the source held the `GRANT` bit and the copy's
    /// bits sit inside the source's, and the copy holds exactly what was asked. The other way too:
    /// a source with `GRANT` and a request inside it is never refused. Stated in raw bits, as
    /// `derive_never_widens_rights` is, so no predicate of the crate stands between the claim and
    /// the bits.
    /// Falsification: replayable `crates/capability_table/falsifications/verification.delegation_needs_grant_and_never_widens.patch`
    #[kani::proof]
    fn delegation_needs_grant_and_never_widens() {
        const GRANT_BIT: u32 = 1 << 2;
        let source = Capability {
            object: 5u8,
            rights: Rights(kani::any()),
        };
        let requested = Rights(kani::any());

        match source.delegate(requested) {
            Ok(copy) => {
                assert_ne!(
                    source.rights.0 & GRANT_BIT,
                    0,
                    "a capability without GRANT was passed on"
                );
                assert_eq!(copy.rights.0 & !source.rights.0, 0);
                assert_eq!(copy.rights.0, requested.0);
                assert_eq!(copy.object, 5u8);
            }
            Err(Error::GrantRequired) => assert_eq!(source.rights.0 & GRANT_BIT, 0),
            Err(Error::CannotWiden) => {
                assert_ne!(source.rights.0 & GRANT_BIT, 0);
                assert_ne!(requested.0 & !source.rights.0, 0);
            }
            Err(_) => panic!("delegate gave an answer it has no reason for"),
        }
    }

    /// **Authority never widens at the other mint site either: `split` inherits, it does not
    /// grant.** `MemoryRegion::SPLIT` mints outside [`CapabilityTable::derive`] through
    /// [`Capability::mint_child`], and this proves the child's rights are exactly the parent's,
    /// which implies the subset relation. Equality is the precise claim because a split passes
    /// `GRANT` down only when the parent holds it, so delegability comes from the root's mint, and
    /// rights down a budget tree never increase.
    /// Falsification: replayable `crates/capability_table/falsifications/verification.split_never_widens_rights.patch`
    #[kani::proof]
    fn split_never_widens_rights() {
        let parent = Capability {
            object: 7u8,
            rights: Rights(kani::any()),
        };
        let child_object: u8 = kani::any();

        let child = parent.mint_child(child_object);

        // Exactly the parent's, for every one of the 2^32 rights patterns: inheritance, not a grant.
        assert_eq!(
            child.rights, parent.rights,
            "an inheriting mint changed the child's rights",
        );
        // The weaker relation the capability model is phrased in, which the above implies.
        assert!(child.rights.is_subset_of(parent.rights));
        assert_eq!(child.object, child_object);
    }
}

#[cfg(test)]
mod tests;
