//! **Futex wait queues: threads parked on a word, found again by its key** (milestone 812
//! (`std::thread::spawn` runs real threads in one address space), §269 (how threads share a
//! process) fork 2).
//!
//! A futex is a key, `(space, address)`, not an object, which is what lets `std`'s `Mutex::new`
//! stay `const` and syscall-free. This module is the queueing: buckets of intrusive FIFOs, a key
//! hashed to a bucket, oldest waiter first within a key. Keys can collide in a bucket, so lookups
//! ask the caller each waiter's key (`key_of`), which is the kernel's own record. Reading memory,
//! deciding to park and waking threads are the kernel's (`kernel/src/sched/futex.rs`);
//! `notes/futex.md` has the argument and the open limits.
//!
//! # EXAMPLES
//!
//! ```
//! use core::ptr::NonNull;
//! use intrusive_fifo::{Node, Unqueued};
//! use inter_process_communication::futex::{Futexes, Key};
//!
//! struct Thread { next: Option<NonNull<Thread>>, key: Key }
//! // SAFETY: plain field storage, which is the whole of the `Node` contract.
//! unsafe impl Node for Thread {
//!     fn next(&self) -> Option<NonNull<Self>> { self.next }
//!     fn set_next(&mut self, next: Option<NonNull<Self>>) { self.next = next; }
//! }
//!
//! let word = Key { space: 7, address: 0x1000 };
//! let mut a = Thread { next: None, key: word };
//! let mut futexes: Futexes<Thread, 4> = Futexes::new();
//! // SAFETY: `a` is a live local declared before `futexes`, on no queue, minted once.
//! futexes.park(word, unsafe { Unqueued::new(NonNull::from(&mut a)) });
//! // SAFETY: every queued pointer is the live `Thread` above.
//! let mut woken = futexes.take(word, usize::MAX, |t| unsafe { t.as_ref().key });
//! assert!(woken.pop_front().is_some_and(|t| t == NonNull::from(&mut a)));
//! ```
//!
//! # BUGS
//!
//! - A wake walks its whole bucket, so it costs the bucket's length, not the number woken.
//!
//! Name: provisional (milestone 812's lane, 2026-10-10 UTC), the field's word for this.

use core::ptr::NonNull;

use intrusive_fifo::{Fifo, Node, Unqueued};

/// **What a waiter is parked under**: the address space it waits in, by the kernel's generational
/// name for it, and the virtual address of the 32-bit word in that space.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Key {
    /// The address space's name, as `kernel::user`'s registry hands it out.
    pub space: u64,
    /// The word's virtual address in that space, 4-aligned.
    pub address: u64,
}

/// **The wait queues**, `B` buckets of intrusive FIFOs.
pub struct Futexes<T: Node, const B: usize> {
    buckets: [Fifo<T>; B],
}

impl<T: Node, const B: usize> Default for Futexes<T, B> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Node, const B: usize> Futexes<T, B> {
    /// No one waiting on anything.
    pub const fn new() -> Self {
        Self {
            buckets: [const { Fifo::new() }; B],
        }
    }

    /// The bucket `key` hashes to. Fibonacci hashing over both halves of the key: the address's low
    /// two bits are always zero and its page offset is shared by every word at the same place in
    /// different pages, so the multiply has to spread the high bits down.
    fn bucket(key: Key) -> usize {
        const GOLDEN: u64 = 0x9E37_79B9_7F4A_7C15;
        let mixed = (key.address >> 2) ^ key.space.wrapping_mul(GOLDEN);
        (mixed.wrapping_mul(GOLDEN) >> 32) as usize % B
    }

    /// **Park `waiter` under `key`**, at the back of its bucket.
    pub fn park(&mut self, key: Key, waiter: Unqueued<T>) {
        self.buckets[Self::bucket(key)].push_back(waiter);
    }

    /// **Take up to `max` waiters parked under `key`**, first parked first, and hand them back as a
    /// queue of their own so the caller can wake each one without holding a borrow of this table.
    /// Waiters under other keys in the same bucket stay, in their order.
    pub fn take(&mut self, key: Key, max: usize, key_of: impl Fn(NonNull<T>) -> Key) -> Fifo<T> {
        let bucket = &mut self.buckets[Self::bucket(key)];
        let mut kept = Fifo::new();
        let mut taken = Fifo::new();
        let mut count = 0;
        while let Some(waiter) = bucket.pop_front() {
            if count < max && key_of(waiter.as_non_null()) == key {
                taken.push_back(waiter);
                count += 1;
            } else {
                kept.push_back(waiter);
            }
        }
        *bucket = kept;
        taken
    }

    /// **Unlink one waiter**, the teardown of a thread that will never be woken (its region is being
    /// destroyed). `None` if it was not parked under `key`.
    pub fn remove(&mut self, key: Key, victim: NonNull<T>) -> Option<Unqueued<T>> {
        let bucket = &mut self.buckets[Self::bucket(key)];
        let mut kept = Fifo::new();
        let mut found = None;
        while let Some(waiter) = bucket.pop_front() {
            if waiter == victim {
                found = Some(waiter);
            } else {
                kept.push_back(waiter);
            }
        }
        *bucket = kept;
        found
    }

    /// **How many waiters are linked anywhere in the table**, whatever key they are under or
    /// whether they still have one. What catches a teardown that forgot to unlink a thread: a dead
    /// TCB has no key to count it by, but it is still a link.
    pub fn queued(&self) -> usize {
        self.buckets.iter().map(Fifo::len).sum()
    }

    /// How many waiters are parked under `key`. For tests and diagnostics; a wake does not need it.
    /// `&mut` because the queue is intrusive and walked by popping, in order, then put back as it was.
    pub fn waiters(&mut self, key: Key, key_of: impl Fn(NonNull<T>) -> Key) -> usize {
        let bucket = &mut self.buckets[Self::bucket(key)];
        let mut kept = Fifo::new();
        let mut count = 0;
        while let Some(waiter) = bucket.pop_front() {
            if key_of(waiter.as_non_null()) == key {
                count += 1;
            }
            kept.push_back(waiter);
        }
        *bucket = kept;
        count
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

    fn key(space: u64, address: u64) -> Key {
        Key { space, address }
    }

    /// A one-bucket table forces every key to collide, which is the case the key check exists for.
    type OneBucket = Futexes<T, 1>;

    fn key_of(t: NonNull<T>) -> Key {
        // SAFETY: every pointer these tests queue is one of the test's own live locals.
        unsafe { t.as_ref().key }
    }

    fn park_all(f: &mut OneBucket, threads: &mut [T]) {
        for t in threads.iter_mut() {
            let k = t.key;
            // SAFETY: a live local of the caller's, on no queue, minted once.
            f.park(k, unsafe { Unqueued::new(NonNull::from(t)) });
        }
    }

    fn drain(mut q: Fifo<T>) -> usize {
        let mut n = 0;
        while q.pop_front().is_some() {
            n += 1;
        }
        n
    }

    #[test]
    fn a_wake_takes_only_its_own_key_in_order_and_at_most_max() {
        let a = key(1, 0x1000);
        let b = key(1, 0x1004);
        let c = key(2, 0x1000); // the same address in another space is another futex
        let mut threads = [a, b, a, c, a].map(|key| T { next: None, key });
        let ptrs: Vec<NonNull<T>> = threads.iter_mut().map(NonNull::from).collect();
        let mut f = OneBucket::new();
        park_all(&mut f, &mut threads);

        let mut two = f.take(a, 2, key_of);
        assert!(two.pop_front().is_some_and(|t| t == ptrs[0]));
        assert!(two.pop_front().is_some_and(|t| t == ptrs[2]));
        assert!(two.pop_front().is_none());
        assert_eq!(f.waiters(a, key_of), 1);
        assert_eq!(f.waiters(b, key_of), 1);
        assert_eq!(f.waiters(c, key_of), 1);

        // The collided keys kept their order around the ones taken.
        assert_eq!(drain(f.take(c, usize::MAX, key_of)), 1);
        assert_eq!(drain(f.take(a, usize::MAX, key_of)), 1);
        assert_eq!(drain(f.take(b, 0, key_of)), 0, "max 0 takes nothing");
        assert_eq!(drain(f.take(b, 1, key_of)), 1);
        assert_eq!(drain(f.take(a, usize::MAX, key_of)), 0);
    }

    #[test]
    fn remove_unlinks_exactly_the_victim() {
        let a = key(1, 0x2000);
        let mut threads = [a, a, a].map(|key| T { next: None, key });
        let ptrs: Vec<NonNull<T>> = threads.iter_mut().map(NonNull::from).collect();
        let mut f = OneBucket::new();
        park_all(&mut f, &mut threads);
        assert!(f.remove(a, ptrs[1]).is_some_and(|t| t == ptrs[1]));
        assert!(
            f.remove(a, ptrs[1]).is_none(),
            "a second remove finds nothing"
        );
        let mut rest = f.take(a, usize::MAX, key_of);
        assert!(rest.pop_front().is_some_and(|t| t == ptrs[0]));
        assert!(rest.pop_front().is_some_and(|t| t == ptrs[2]));
        assert!(rest.pop_front().is_none());
    }

    #[test]
    fn keys_spread_over_the_buckets() {
        // Consecutive words of one page, and one word in each of many spaces, are the two shapes a
        // real program makes. Neither should land in one bucket.
        let mut hit = [false; 64];
        for i in 0..64 {
            hit[Futexes::<T, 64>::bucket(key(9, 0x4000 + 4 * i))] = true;
        }
        assert!(hit.iter().filter(|&&h| h).count() > 32);
        let mut hit = [false; 64];
        for s in 0..64 {
            hit[Futexes::<T, 64>::bucket(key(s << 32 | 3, 0x4000))] = true;
        }
        assert!(hit.iter().filter(|&&h| h).count() > 32);
    }
}
