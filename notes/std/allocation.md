# The `std` heap under threads: a cache per thread in front of one lock

Milestone 561 (a per-CPU allocator is what the current-CPU page was for) asked for a free list per
CPU id. This appendix records why it builds a cache per thread instead, what was surveyed before
writing anything, and what was measured against the per-CPU design. Written 2026-10-10 (UTC) by the
milestone's lane; the wiring and the measurement were added 2026-10-11 (UTC).

## The problem, measured by milestone 812

`std`'s allocator (`patches/std-nife/overlay/std/src/sys/alloc/nife/mod.rs`) is
one heap behind one spinlock. Milestone 812 (`std::thread::spawn` runs real threads
in one address space) made `std::thread::spawn` real and measured it with `std_heap_contention`: the
same 200,000 small allocations take 2.1 to 3.2 times longer split across four threads than done by
one (`notes/std/threads.md` on #1892). A thread preempted holding the lock makes the others spin out
their slices. (812 described all three architectures as four emulated cores. `x86_64` has two in
its PVH leg and one in its OVMF leg, and runs them in turn on one host thread; see below.)

## Why per thread and not per CPU

The current-CPU page was built for a per-CPU allocator, and the block named the hard part: the id
can be stale the instruction after it is read, and this tree has no restartable sequences (`rseq`).

Staleness is the smaller half. A thread can be preempted between two instructions of a pop, and the
next thread on that core reads the same, fresh, id and pops the same list. User mode cannot turn
preemption off, so without `rseq` a per-CPU list is shared by every thread that may run on that core,
and each operation needs an atomic read-modify-write. The block's three candidates, checked:

| candidate | correct without `rseq`? | fast-path cost |
|---|---|---|
| per-list lock, slow path only | no: preemption mid-pop corrupts the list | none, which is why it fails |
| per-list lock on every operation | yes: a stale id locks another core's list, which is contention, not corruption | one uncontended acquire and release |
| compare-and-swap pop tolerating the wrong list | only with ABA protection: a double-width CAS `riscv64` lacks, or a tag in the pointer | one CAS, plus the tag scheme |
| owner check on free | not needed here | (blocks are fungible, see below) |
| **cache per thread** | yes: only its thread touches it, on any core | none |

An owner check is for lists that own address ranges. These blocks are fungible: any block of a size
class serves any request for it. So a free goes to the freeing thread's cache, as glibc's `tcache`
does (`malloc/malloc.c`, `_int_free` calling `tcache_put`; `tcache_thread_shutdown` returns a dying
thread's entries to the arena).

Cost of reaching each, after milestone 812, where each thread's current-CPU page is found through its
thread-pointer block (`sys/thread_local/key/nife.rs`):

- per thread: read the thread pointer, load the cache from the block.
- per CPU: read the thread pointer, load the page's address, load the CPU id, index the arenas, then
  the lock.

Per CPU is a superset of per thread. It wins on memory only when threads far outnumber cores (many
threads, few caches), and a process here has at most sixteen threads. Prior art agrees: TCMalloc runs
per-CPU only with `rseq` and "if RSEQ is unavailable ... reverts to" per-thread caches
([TCMalloc overview](https://github.com/google/tcmalloc/blob/master/docs/overview.md), read
2026-10-10); jemalloc's `percpu_arena` is the per-list-lock row above, an arena with its lock per
CPU ([jemalloc manual, `opt.percpu_arena`](https://jemalloc.net/jemalloc.3.html)).

Nothing here needs `rseq`, a new syscall or a new method. That was the argument; the measurement
below tests it rather than trusting it.

## What ships

The cache is `crates/allocation_cache` (name provisional), generated into `std` as
`sys/alloc/nife/allocation_cache.rs` beside the heap, `talc/`. Each thread's block
(`sys/thread_local/key/nife.rs`, milestone 812's thread-pointer block) holds one, zeroed and so
empty. A request of at most 256 bytes and alignment at most 16 pops from it; a miss takes the heap's
lock once and refills eight; a free into a full class spills eight under one acquisition. Before the
main thread's block exists every request takes the lock, which also keeps the block's own making
from recursing into the allocator.

A thread's end spills its cache. `destroy_tls` runs the thread-local destructors, closes the
cache (later frees go straight to the heap) and gives every cached block back under one lock. The
block is freed last, by `free_own_block`, just before `SYS_EXIT_THREAD`, because a detached thread
still frees its packet after `destroy_tls`, and with a cache in the block that free would otherwise
write into freed memory. A `const` assert keeps the block larger than any class, so its own free
cannot land in the cache it holds. `std_threads_tests::the_std_heap_is_measured_under_four_threads`
proves the spill: 64 threads in turn each end with a full cache, 2.2 MB against the program's 1 MiB
budget. Its falsification drops the spill, and the program then dies out of memory.

## What was measured, 2026-10-11 (UTC)

The same program as 812, `std_heap_contention`: 200,000 allocations of a 48-byte `Box`, each freed
at once, by one thread and then split across four. Four allocators, chosen at build time by a switch
in the scratch patch [allocation-variants.scratch.patch](allocation-variants.scratch.patch), which
applies to this tree and does not ship:

- (a) **one lock**: the heap behind its spinlock, as on `main` before this milestone.
- (b) **per thread**: what ships.
- (c) **per CPU**: `CPU_ID_BOUND` caches, each on its own cache line behind its own spinlock taken on
  every operation (the cheapest correct row of the table above), indexed by the CPU id read from the
  thread's current-CPU page with `CurrentCpuPage::cpu`.
- (d) **per CPU, the CPU read removed**: (c) with every thread on list 0, which the block's "What it
  would prove" asks for.

Three rounds of every variant on every leg, run by `script/test --test
the_std_heap_is_measured_under_four_threads`, under QEMU's TCG emulation. Figures are the median of
the three, in millions of allocations (each with its free) per second of guest time; "four against
one" is the four-thread rate over the one-thread rate, so above 1 means four threads got more done.

| allocator | leg | one thread | four threads | four against one |
|---|---|---|---|---|
| (a) one lock | aarch64, 4 cores | 4.39 | 3.31 | 0.76 |
| (b) per thread | aarch64, 4 cores | 5.50 | **13.65** | **2.48** |
| (c) per CPU | aarch64, 4 cores | 4.11 | 8.80 | 2.14 |
| (d) per CPU, no read | aarch64, 4 cores | 4.78 | 2.42 | 0.51 |
| (a) one lock | riscv64, 4 cores | 2.00 | 1.73 | 0.87 |
| (b) per thread | riscv64, 4 cores | 2.14 | **5.49** | 2.57 |
| (c) per CPU | riscv64, 4 cores | 1.60 | 4.53 | **2.84** |
| (d) per CPU, no read | riscv64, 4 cores | 1.92 | 3.83 | 2.00 |
| (a) one lock | `x86_64` PVH, 2 cores in turn | 4.82 | 2.72 | 0.56 |
| (b) per thread | `x86_64` PVH, 2 cores in turn | 4.64 | **5.84** | 1.26 |
| (c) per CPU | `x86_64` PVH, 2 cores in turn | 3.67 | 5.17 | **1.41** |
| (d) per CPU, no read | `x86_64` PVH, 2 cores in turn | 5.50 | 3.93 | 0.71 |
| (a) one lock | `x86_64` OVMF, 1 core | 5.21 | 3.15 | 0.61 |
| (b) per thread | `x86_64` OVMF, 1 core | 4.58 | 4.64 | 1.01 |
| (c) per CPU | `x86_64` OVMF, 1 core | 3.82 | 2.75 | 0.72 |
| (d) per CPU, no read | `x86_64` OVMF, 1 core | 5.24 | **5.58** | **1.06** |

The raw times, in milliseconds (one thread, four threads) for rounds one to three:

| allocator | aarch64 | riscv64 | `x86_64` PVH | `x86_64` OVMF |
|---|---|---|---|---|
| (a) | 45.6/50.8, 44.3/60.3, 47.6/71.7 | 106.6/162.4, 100.2/115.8, 94.7/88.4 | 42.0/97.0, 41.5/73.6, 39.9/47.5 | 38.4/36.6, 40.0/93.4, 37.6/63.4 |
| (b) | 34.6/14.7, 36.6/15.6, 36.4/13.2 | 92.9/42.4, 93.6/35.0, 95.1/36.4 | 47.4/36.6, 43.1/34.2, 40.6/22.2 | 43.7/42.1, 42.9/43.1, 56.8/84.8 |
| (c) | 47.9/18.8, 48.9/22.7, 48.7/29.6 | 164.3/211.8, 125.2/44.2, 118.0/38.2 | 54.8/39.2, 54.5/36.8, 53.1/38.7 | 52.6/72.7, 50.6/80.0, 52.4/51.6 |
| (d) | 39.9/82.7, 144.4/104.1, 41.9/50.0 | 105.8/71.7, 104.3/49.1, 101.9/52.2 | 35.5/60.4, 38.4/50.9, 36.4/41.6 | 38.1/37.3, 38.4/35.8, 37.5/34.7 |

**The verdict: per thread wins, and per CPU does not.** Per thread does the most work with four
threads on every leg that has more than one core, by 55% on aarch64, 21% on riscv64 and 13% on
`x86_64`'s PVH leg. It is also faster than per CPU with one thread everywhere. Per CPU's better ratio
on riscv64 and `x86_64` PVH comes from a slower one-thread figure, not a faster four-thread one. Both
replace a lock that made four threads slower than one (812's finding, reproduced in ten of (a)'s twelve
runs) with something that scales: 2.5 times on four emulated cores. So the shipped design
stands, and this does not recommend per CPU.

**What the CPU read buys, and what it costs.** (c) against (d) is the page's own number. With four
threads on four cores the read is worth 3.6 times on aarch64 and 1.2 times on riscv64: without it
every thread fights for list 0. With one thread it costs: (c) is 16% slower than (d) on aarch64, 20%
on riscv64 and 37 to 50% on `x86_64`. The only difference is `CurrentCpuPage::cpu`: two dependent
loads to reach the page, its eight-byte magic check and the word. Under TCG that is a
cost in emulated instructions; on silicon those are cache hits, and nothing here says what they cost
there.

**Staleness did not matter.** A separate instrumented build (the patch's `INSTRUMENT`; its times
are not used above) counted three things for (c). They were every list operation, every acquire that
found the lock taken, and every operation after which the thread's CPU id named another list. The last is
an id gone stale mid-operation. On all four legs, across about 400,000 operations per phase: **no stale id**,
and at most one contended acquire. The scheduler leaves a running thread on its core, and the window
is a few dozen instructions. On `x86_64`'s one-core leg all four threads legitimately share list 0,
and even there no acquire found the lock taken, because a thread is rarely preempted inside so short
a lock. (d), for contrast, found the lock taken on 69% of operations on aarch64 (240,255 of 346,837
in one thread's report) and 59% on riscv64 (191,939 of 327,726). The reports are written from four
threads at once and interleave, so only the whole lines are quoted. That staleness was never seen
does not remove the atomic: correctness still needs it whenever two threads share a list, by a
stale id or by sharing a core, and the one-core leg is the second case on every operation.

**`CPU_ID_BOUND` was the right thing to size by.** Sizing by it is what makes an id an index without
a check that the online set can change under; (c) also reduced the id modulo the bound, which never
fired. Its cost at eight is small: eight lists of one 192-byte, line-aligned cache, 1.5 KiB of
static memory, of which these machines touch four, two or one. The memory a list keeps out of the
heap, up to 34,816 bytes, is spent only by lists a thread touches, so a larger bound would cost
static bytes rather than cached ones. Per thread does not depend on it at all.

**What the numbers can and cannot show.** This is TCG on a laptop that was running another lane's
gate (load average 6 to 20), not hardware. It is not `-icount` either, so guest time follows host
time and moves with load: (d)'s one-thread aarch64 round at 144 ms against 40 and 42 is that, and
so is (c)'s first riscv64 round. Read the ratios and the ranking, not the rates. `x86_64` is weaker still. Its
runner leaves QEMU's round-robin TCG in place (`helpers/qemu-runner-x86_64.sh`), so its two cores take
turns on one host thread and can show what a lock costs but not parallel scaling. The OVMF leg has
one core. On that leg (d) edges (b), within the spread of (b)'s runs; and (c)'s four-thread
slowdown there is unexplained, since its lists were uncontended. Every figure is one access pattern
(allocate, then free the same size at once, on the same thread), which is any cache's best case. A
producer on one thread freeing to a consumer on another would move blocks between per-thread caches,
and was not measured. Apple silicon under HVF (`notes/benchmarks.md`'s other instrument) would put
real nanoseconds on the CPU read for aarch64, and is the follow-on that could change the cost half
of this, though not the staleness or contention counts.

**What this means for the current-CPU page (milestone 557 (a thread reads its own CPU from a
page)).** The page works as designed. Its id was never stale here, and reading it takes four threads
from fighting over one list to not contending at all. But the allocator it was built for loses to
one that never reads it, so the page's CPU word has no consumer in the tree; only tests and the
`current_cpu_reader` fixture read it. Its allowance word keeps its consumer,
`available_parallelism`. Whether the kernel should keep writing the CPU word on every switch is
calef's question, on the milestone's pull request.

## Reuse: what was surveyed, 2026-10-10 (UTC)

Read from crates.io's API and each crate's source or README. The question for each: does it give
`std` a heap whose allocations stop taking one lock, on three bare targets, with no C and no `mmap`?

| candidate | license, latest release | what it is | why not |
|---|---|---|---|
| `talc` 5.1.1 | MIT, 2026-09-09 | `no_std` heap, `TalcLock` over `lock_api` | one heap, one lock; its README sends concurrent users to jemalloc or mimalloc |
| `linked_list_allocator` 0.10.6 | MIT/Apache-2.0, 2026-04-14 | first-fit list, spinlocked `LockedHeap` | same lock |
| `buddy_system_allocator` 0.13.0 | MIT, 2026-03-30 | buddy heap, spinlocked `LockedHeap` | same lock |
| `dlmalloc` 0.2.14 | MIT/Apache-2.0, 2026-05-16 | already in `std`'s own `Cargo.lock` (xous, wasm, SGX); an `Allocator` trait for system memory | one global lock (`std`'s `sys/alloc/xous.rs` wraps it in one); would replace the one heap algorithm [§31 (the foreign-language seam)](../../design/decisions/0031-foreign-language-seam.md) rule 4 and `notes/std.md` keep |
| `rlsf` 0.2.3 | MIT/Apache-2.0, 2026-07-27 | TLSF | "does not support concurrent access"; its README recommends a thread-caching allocator |
| `frusa` 0.1.3 (Motor OS) | MIT/Apache-2.0, 2025-11-22 | size-class allocator over a back end | its own benchmark falls from 16.8 to 8.6 operations per microsecond going from one thread to four; needs nightly `feature(test)`; tested on x86_64 only |
| `rsbmalloc` | (not checked further) | thread-cached in its `std` build | its `no_std` build drops the caches for spinlocks, and it needs `mmap` |
| `mimalloc` 0.1.52 | MIT, 2026-05-22 | C; per-thread heaps, cross-thread frees to a separate atomic list | a C toolchain inside `std`'s build on three targets, an OS layer over `mmap` that §31 forbids C to call, a TLS slot model this tree lacks, and it would replace `user_mode_heap` |
| `snmalloc-rs` 0.7.6 | MIT, 2026-10-02 | C++; per-thread allocators, remote frees by message passing | a C++ toolchain and a platform layer to port, for the same replacement |

Verdict: nothing to take. Every pure-Rust `no_std` allocator is one heap behind one lock, which is
what we have; the ones with per-thread caches are C or C++, or need `libc`. The layer that is missing
is small and sits in front of the heap we keep: `crates/allocation_cache` (name provisional),
about 150 lines. Its shape is glibc's `tcache`: bounded bins of 16, frees to the freeing thread, a
flush at the thread's end. It adds refills and spills batched under one lock acquisition. That
adapts a design and takes no dependency, so no ruling is owed.

## BUGS

- A cached block cannot coalesce until it is spilled: up to 34,816 bytes per thread, about half a
  mebibyte for sixteen threads (`crates/allocation_cache`'s `BUGS`).
- A block freed on another thread than the one that allocated it lands in the freeing thread's
  cache, as in glibc. A producer and consumer pair therefore drains one thread's cache into the
  other's, bounded by the depth and spilled at the bound. Not measured.
- The main thread's cache is never spilled, because the process ends with it.
