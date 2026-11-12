# Prior art

An appendix to [the milestone](../1077-allocation-cache-outside-review.md).

How the crate sits against the thread caches a reader already knows. The figures in this table come from
memory of the sources named; this review did not verify them.

| system | where the cache lives | bins and depth | what moves under the lock | what a reader coming from it will miss here |
|---|---|---|---|---|
| glibc `tcache` (`malloc/malloc.c`) | inside malloc, one per thread | 64 bins, 7 entries each, up to about 1 KiB | one chunk at a time; the arena lock is taken per miss | more bins and a larger ceiling; a count-per-bin tunable; a shutdown flush that the library runs itself |
| jemalloc `tcache` | inside jemalloc, one per thread | per size class, `ncached_max`, flushes half when full and fills by a shrinking fraction | a batch | a GC that shrinks idle bins; large-class caching; `opt.tcache_max` |
| TCMalloc `ThreadCache` and `CpuCache` | inside tcmalloc | per size class, adaptive `max_length`, slow start | a batch of `num_objects_to_move` per class | adaptive depth; a per-CPU variant that needs `rseq` |
| Linux SLUB `kmem_cache_cpu` | the kernel, one per CPU | one active slab and a partial list per cache | a slab | a lockless per-CPU fast path with `this_cpu_cmpxchg`, which this crate does not need because one owner holds it |
| FreeBSD UMA buckets, NetBSD `pool_cache` groups | the kernel, one per CPU | a bucket or group of fixed size per zone | a whole bucket or group, swapped under the zone lock | two-bucket (allocate and free) design that avoids the boundary thrash this crate solves with `BATCH = DEPTH / 2` |
| `heapless::Vec::push` (Rust) | a bounded collection | fixed capacity | none | the `Result<(), T>` shape on a full push is the same as `put`'s, and the std-spelled names `push` and `pop` |

Where this crate differs: it is the algorithm only. It never locks, never reads an owner id, never
frees, and compiles in `no_std` with no dependency. The owner (a `std` port, a `GlobalAlloc`
wrapper, a kernel) decides who touches a `Cache`, holds whatever lock the heap needs around `refill`
and `spill`, and frees every block a `drain` hands back. Of the systems above, none publishes its
thread cache as a separate library (unverified: this review knows of none); each keeps it inside
the allocator. The batch transfer of half a class under one acquisition is jemalloc's and the
BSDs' idea; the fixed depth of 16 is a choice the crate's own `BUGS` says is not measured per
workload. The host test that drives a random walk of allocations and frees over a real heap and then demands one coalesced fragment is the concrete example of what the crate promises. No block goes out twice. None comes up shorter than its class. Every byte comes back.
