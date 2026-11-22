//! **Address wait queues: threads parked on a word, found again by its key** (milestone 812
//! (`std::thread::spawn` runs real threads in one address space), §269 (how threads share a
//! process) fork 2).
//!
//! An address wait (Linux calls it a fast user-space mutex, "futex") is a key, `(address_space, virtual_address)`, not an object. This module is the queuing: buckets of
//! intrusive FIFOs, oldest waiter first within a key, with each waiter's key read back through
//! `key_of` because keys collide in a bucket. The kernel side is `kernel/src/sched/address_wait.rs`; see
//! `notes/address-wait.md` and `notes/inter-process-communication.md`.
//!
//! # Examples
//!
//! ```
//! use core::ptr::NonNull;
//! use intrusive_queue::{Node, Unqueued};
//! use inter_process_communication::address_wait::{AddressWaitTable, Key};
//!
//! struct Thread { next: Option<NonNull<Thread>>, key: Key }
//! // SAFETY: plain field storage, which is the whole of the `Node` contract.
//! unsafe impl Node for Thread {
//!     fn next(&self) -> Option<NonNull<Self>> { self.next }
//!     fn set_next(&mut self, next: Option<NonNull<Self>>) { self.next = next; }
//! }
//!
//! let word = Key { address_space: 7, virtual_address: 0x1000 };
//! let mut a = Thread { next: None, key: word };
//! // The token's pointer comes from the place itself, and `a` is not named again.
//! let pointer_a = NonNull::new(&raw mut a).unwrap();
//! let mut address_waits: AddressWaitTable<Thread, 4> = AddressWaitTable::new();
//! // SAFETY: `a` is a live local declared before `address_waits`, on no queue, minted once.
//! address_waits.park(word, unsafe { Unqueued::new(pointer_a) });
//! // SAFETY: every queued pointer is the live `Thread` above.
//! let mut woken = address_waits.take(word, usize::MAX, |t| unsafe { t.as_ref().key });
//! assert!(woken.pop_front().is_some_and(|t| t == pointer_a));
//! ```
//!
//! # BUGS
//!
//! - A wake walks its whole bucket, so it costs the bucket's length, not the number woken.
//!
//! Name: ratified 2026-11-21 (UTC). The module and `AddressWaitTable` say what the queues are for, a wait on an
//! address, and not the Linux word for it. `AddressWaitTable` names the thing (a plural noun named
//! a collection of objects, and an address wait is a key). `Key::address_space` and
//! `Key::virtual_address` say which kind of name and which kind of address the fields hold.
//! `BUCKETS` is the word the documentation uses for the const parameter. `len` is the standard
//! library's spelling of a count, and `waiter_count` counts the waiters under one key. Refused:
//! keeping the plural noun, and `waiters` for the per-key count.

use core::ptr::NonNull;

use intrusive_queue::{Node, Queue, Unqueued};

/// **What a waiter is parked under**: the address space it waits in, by the kernel's generational
/// name for it, and the virtual address of the 32-bit word in that space.
///
/// Name: ratified 2026-11-21 (UTC).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Key {
    /// The address space's name, as `kernel::user`'s registry hands it out.
    pub address_space: u64,
    /// The word's virtual address in that space, 4-aligned, which the caller checks: nothing in
    /// the type enforces it.
    pub virtual_address: u64,
}

/// **The wait queues**, `BUCKETS` buckets of intrusive FIFOs.
///
/// Name: ratified 2026-11-21 (UTC). A table of address wait queues, named for the thing; the module header
/// gives what was refused.
#[doc(alias = "futex")]
pub struct AddressWaitTable<T: Node, const BUCKETS: usize> {
    buckets: [Queue<T>; BUCKETS],
}

impl<T: Node, const BUCKETS: usize> Default for AddressWaitTable<T, BUCKETS> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Node, const BUCKETS: usize> AddressWaitTable<T, BUCKETS> {
    /// No one waiting on anything.
    ///
    /// Name: ratified 2026-11-21 (UTC).
    pub const fn new() -> Self {
        Self {
            buckets: {
                const {
                    assert!(
                        BUCKETS > 0,
                        "an address wait table needs at least one bucket"
                    );
                };
                [const { Queue::new() }; BUCKETS]
            },
        }
    }

    /// The bucket `key` hashes to. Fibonacci hashing over both halves of the key: the address's low
    /// two bits are always zero and its page offset is shared by every word at the same place in
    /// different pages, so the multiply has to spread the high bits down.
    fn bucket(key: Key) -> usize {
        const GOLDEN: u64 = 0x9E37_79B9_7F4A_7C15;
        let mixed = (key.virtual_address >> 2) ^ key.address_space.wrapping_mul(GOLDEN);
        (mixed.wrapping_mul(GOLDEN) >> 32) as usize % BUCKETS
    }

    /// **Park `waiter` under `key`**, at the back of its bucket.
    ///
    /// Name: ratified 2026-11-21 (UTC).
    pub fn park(&mut self, key: Key, waiter: Unqueued<T>) {
        self.buckets[Self::bucket(key)].push_back(waiter);
    }

    /// **Take up to `maximum` waiters parked under `key`**, first parked first, and hand them back
    /// as a queue of their own so the caller can wake each one without holding a borrow of this
    /// table. Waiters under other keys in the same bucket stay, in their order.
    ///
    /// Name: ratified 2026-11-21 (UTC).
    pub fn take(
        &mut self,
        key: Key,
        maximum: usize,
        key_of: impl Fn(NonNull<T>) -> Key,
    ) -> Queue<T> {
        let mut left = maximum;
        self.buckets[Self::bucket(key)].extract_if(|waiter| {
            let pick = left > 0 && key_of(waiter) == key;
            left -= usize::from(pick);
            pick
        })
    }

    /// **Unlink one waiter**, the teardown of a thread that will never be woken (its region is being
    /// destroyed). `None` if it was not parked under `key`.
    ///
    /// Name: ratified 2026-11-21 (UTC).
    pub fn remove(&mut self, key: Key, victim: NonNull<T>) -> Option<Unqueued<T>> {
        self.buckets[Self::bucket(key)].remove(victim)
    }

    /// **How many waiters are linked anywhere in the table**, whatever key they are under or
    /// whether they still have one. What catches a teardown that forgot to unlink a thread: a dead
    /// TCB has no key to count it by, but it is still a link.
    ///
    /// Name: ratified 2026-11-21 (UTC). The standard library's spelling of a count.
    pub fn len(&self) -> usize {
        self.buckets.iter().map(Queue::len).sum()
    }

    /// **Whether no waiter is linked anywhere in the table**, the pair of [`len`](Self::len).
    ///
    /// Name: ratified 2026-11-21 (UTC). The standard library's spelling, which a public `len` brings with it.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// How many waiters are parked under `key`. For tests and diagnostics; a wake does not need it.
    /// A read-only walk of the key's bucket, which leaves every queue as it was.
    ///
    /// Name: ratified 2026-11-21 (UTC).
    pub fn waiter_count(&self, key: Key, key_of: impl Fn(NonNull<T>) -> Key) -> usize {
        self.buckets[Self::bucket(key)]
            .iter()
            .filter(|&waiter| key_of(waiter) == key)
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct T {
        next: Option<NonNull<T>>,
        key: Key,
    }

    // SAFETY: plain field storage.
    unsafe impl Node for T {
        fn next(&self) -> Option<NonNull<Self>> {
            self.next
        }
        fn set_next(&mut self, next: Option<NonNull<Self>>) {
            self.next = next;
        }
    }

    fn key(address_space: u64, virtual_address: u64) -> Key {
        Key {
            address_space,
            virtual_address,
        }
    }

    /// A one-bucket table forces every key to collide, which is the case the key check exists for.
    type OneBucket = AddressWaitTable<T, 1>;

    fn key_of(t: NonNull<T>) -> Key {
        // SAFETY: every pointer these tests queue is one of the test's own live locals.
        unsafe { t.as_ref().key }
    }

    /// A pointer to each element of `threads`, all derived from the one pointer to the array, so
    /// a token minted from one is not invalidated by a later access (`Unqueued::new`'s provenance
    /// clause). The caller passes `&raw mut` on its local and does not name the local again.
    fn element_pointers<const N: usize>(threads: *mut [T; N]) -> Vec<NonNull<T>> {
        (0..N)
            .map(|i| NonNull::new(threads.cast::<T>().wrapping_add(i)).unwrap())
            .collect()
    }

    fn park_all(f: &mut OneBucket, threads: &[NonNull<T>]) {
        for &t in threads {
            // SAFETY: a live local of the caller's, on no queue, minted once from the pointer
            // every other access to it derives from.
            f.park(key_of(t), unsafe { Unqueued::new(t) });
        }
    }

    fn drain(mut q: Queue<T>) -> usize {
        let mut n = 0;
        while q.pop_front().is_some() {
            n += 1;
        }
        n
    }

    #[test]
    fn a_wake_takes_only_its_own_key_in_order_and_at_most_maximum() {
        let a = key(1, 0x1000);
        let b = key(1, 0x1004);
        let c = key(2, 0x1000); // the same address in another space is another address wait
        let mut threads = [a, b, a, c, a].map(|key| T { next: None, key });
        let pointers = element_pointers(&raw mut threads);
        let mut f = OneBucket::new();
        park_all(&mut f, &pointers);
        assert_eq!(f.len(), 5);
        assert_eq!(f.waiter_count(a, key_of), 3);

        let mut two = f.take(a, 2, key_of);
        assert!(two.pop_front().is_some_and(|t| t == pointers[0]));
        assert!(two.pop_front().is_some_and(|t| t == pointers[2]));
        assert!(two.pop_front().is_none());
        assert_eq!(f.waiter_count(a, key_of), 1);
        assert_eq!(f.waiter_count(b, key_of), 1);
        assert_eq!(f.waiter_count(c, key_of), 1);

        // The collided keys kept their order around the ones taken.
        assert_eq!(drain(f.take(c, usize::MAX, key_of)), 1);
        assert_eq!(drain(f.take(a, usize::MAX, key_of)), 1);
        assert_eq!(drain(f.take(b, 0, key_of)), 0, "max 0 takes nothing");
        assert_eq!(drain(f.take(b, 1, key_of)), 1);
        assert_eq!(drain(f.take(a, usize::MAX, key_of)), 0);
        assert_eq!(f.len(), 0);
        assert!(f.is_empty());
    }

    #[test]
    fn remove_unlinks_exactly_the_victim() {
        let a = key(1, 0x2000);
        let mut threads = [a, a, a].map(|key| T { next: None, key });
        let pointers = element_pointers(&raw mut threads);
        let mut f = OneBucket::new();
        assert!(f.is_empty(), "a new table has no waiter");
        park_all(&mut f, &pointers);
        assert!(!f.is_empty(), "a table with parked waiters is not empty");
        assert!(f.remove(a, pointers[1]).is_some_and(|t| t == pointers[1]));
        assert!(
            f.remove(a, pointers[1]).is_none(),
            "a second remove finds nothing"
        );
        let mut rest = f.take(a, usize::MAX, key_of);
        assert!(rest.pop_front().is_some_and(|t| t == pointers[0]));
        assert!(rest.pop_front().is_some_and(|t| t == pointers[2]));
        assert!(rest.pop_front().is_none());
    }

    #[test]
    fn keys_spread_over_the_buckets() {
        // Consecutive words of one page, and one word in each of many spaces, are the two shapes a
        // real program makes. Neither should land in one bucket.
        let mut hit = [false; 64];
        for i in 0..64 {
            hit[AddressWaitTable::<T, 64>::bucket(key(9, 0x4000 + 4 * i))] = true;
        }
        assert!(hit.iter().filter(|&&h| h).count() > 32);
        let mut hit = [false; 64];
        for s in 0..64 {
            hit[AddressWaitTable::<T, 64>::bucket(key(s << 32 | 3, 0x4000))] = true;
        }
        assert!(hit.iter().filter(|&&h| h).count() > 32);
    }
}
