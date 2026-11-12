# Prior art

An appendix to [the milestone](../1095-inter-process-communication-outside-review.md).

This table sets the four parts against what a stranger already knows. Downloads are all-time / last 90
days from crates.io at the time of the review. A figure not fetched says so.

| what | where | how it solves the same problem | where this crate differs |
|---|---|---|---|
| the synchronous rendezvous | seL4 endpoints | one wait queue and a three-state tag (`Idle`, `Send`, `Recv`); the queue holds senders or receivers, never both, by construction | two queues and a proved invariant that at most one is non-empty (item 11); a pending-signal count on the same object, which seL4 keeps on the notification |
| | QNX `MsgSend`/`MsgReceive`, L4Re IPC gates | blocked senders queue on the channel in priority or FIFO order; the receiver collects one | FIFO only; priority is the kernel's business and the crate never sees it |
| the notification | seL4 notification objects | a word, a wait queue and a bound TCB; a signal with a waiter wakes it, with a bound TCB in receive delivers there, otherwise ORs into the word (`Idle`, `Waiting`, `Active`) | the same three rules in the same order (§101 (notification objects)), proved; the bound check arrives as a boolean the kernel computes, so the crate never reads another object's queue; a signal of zero bits is a distinct verdict that touches nothing, where seL4 makes the object `Active` with a zero word |
| | Zircon signals and ports | a bitmask per object, observed through a port | no port; the waiter is a thread on the object |
| the futex wait queues | Linux `futex_hash_bucket` | a global table of `roundup_pow_of_two(256 * cpus)` buckets hashed by `jhash2` over the key; waiters in a bucket carry their key and a wake walks the bucket comparing keys. Newer kernels add a private per-process hash (unverified) | a fixed `B` of buckets chosen at compile time (64 in the kernel), Fibonacci hashing over `(space, address)`, the same key-compare walk; the key is read back through a caller closure because the crate never dereferences a node |
| | `parking_lot_core` (figures not fetched) | a hash table of buckets keyed by address, FIFO within a key, `park`, `unpark_one`, `unpark_all`, `unpark_requeue`; resizes with the thread count | no requeue and no resize; `take(key, maximum, key_of)` is `unpark_one` and `unpark_all` in one call |
| | `futex` 0.1 (7,978 / 55), Zephyr `k_futex` | thin wrappers over a kernel futex, or an object with a wait queue | this crate is the queue the kernel keeps, not the user-side wrapper |
| the timer | Linux `hrtimer` | per-CPU red-black `timerqueue` with a cached leftmost node and a per-base `expires_next`; the tick compares against the cache | one deadline per timer, a cached earliest that is never raised except by a full walk, and an O(timers) walk per firing; sound for the kernel's 1,024-timer bound, and the kernel documents the cost |
| | FreeBSD `callout`, Zephyr timeouts, Zircon timer dispatchers | a wheel, a sorted list, a per-CPU queue | no ordering structure at all; the crate proves the cache invariant instead of a data structure |
| the intrusive node and token | `intrusive-collections` 0.10 (26.2M / 9.0M), `cordyceps` (figures not fetched) | an `Adapter` or an unsafe `Linked` trait, and an `UnsafeRef` or `Pin` as the membership proof | `intrusive_queue`'s `Node` and `Unqueued<T>`: a token that is proof of non-membership, consumed by a push and returned by a pop, so none of this crate's shipped operations is `unsafe` |

The concrete example of where the shape pays: `Rendezvous::send` on an endpoint bound to an
interrupt answers `Send::Refused` with the sender's own token and nothing touched. The proof
`send_rendezvous_iff_a_receiver_waited` turns red at two assertions if that branch goes away. In
seL4 the equivalent protection is the capability rights on the endpoint. Here it is a verdict a
kernel path cannot forget to handle, because the `match` is exhaustive.
