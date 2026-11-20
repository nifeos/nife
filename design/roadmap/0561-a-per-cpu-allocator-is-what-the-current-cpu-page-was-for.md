---
status: BUILT
raised: 2026-09-21
built: 2026-10-11
promoted_from: a-per-cpu-allocator-is-what-the-current-cpu-page-was-for
milestone_dependencies: 812
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 561. A per-CPU allocator is what the current-CPU page was for

The number is **provisional**: the integrator mints it at merge. Built on branch
`milestone/561-a-per-cpu-allocator` (#1904). Promoted from the proposal `a-per-cpu-allocator-is-what-the-current-cpu-page-was-for` on 2026-09-22, filed 2026-09-21. Raised by the lane that built the current-CPU page, as the work its
own change exists to serve and which nobody is doing.

The page ships; a reader can call `user_mode_runtime::current_cpu` today.

## Why this belongs on the list rather than in a report

The page was justified by a consumer that does not exist yet. That is not a flaw in the reasoning.
The consumer is what decided the shape (a load rather than a crossing, because an allocator asks on
every allocation), and the shape had to be decided before anything could be built on it. But it does
mean the mechanism currently has **no measured benefit at all**, and the numbers quoted for it are
Linux's, about Linux. Until something allocates against it, nife has a fast answer to a question
nothing asks.

The honest version of that in a `BUGS` section is already written. This is the version that gets it
fixed.

## What it would be

A small-object allocator in userspace that keeps a free list per CPU id, reads its own CPU on each
allocation, and falls back correctly when the answer turns out to be the previous core's. The
fallback is the interesting half and the reason this is a milestone rather than an afternoon. The
value can be stale the instruction after it is read. Linux solves that with `rseq`'s restartable
sequences, and this tree deliberately does not have those. So the design question is what
correctness argument replaces them. The candidates (a per-list lock taken only on the slow
path, a compare-and-swap that tolerates the wrong list, an owner check on free) differ in exactly
the cost this page was chosen to avoid.

## What it would prove, which is the point

- A number. Allocations per second against the same allocator with the CPU read removed, on all
  three architectures, which turns the page from an argument into a measurement.
- Whether the staleness matters in practice, which nothing in this tree currently knows.
- Whether `CPU_ID_BOUND` is the right thing to size by. Eight lists per process is the current
  answer and it is a guess; an allocator is what makes the cost of that guess visible.

## What would make it not worth doing

If the answer to the correctness question turns out to want restartable sequences, this stops being
an allocator milestone and becomes an `rseq` milestone, which is a much larger thing and a system call
surface question. Finding that out early is a good outcome of starting it.

## The consumer, measured

2026-10-10 (UTC), by milestone 812 (`std::thread::spawn` runs real threads in one address space):
`std` programs now have threads, and the `std` heap is one spinlock. Allocation is 1.5 to 3.2 times
slower with four threads than with one, across two runs. Each run is 200,000 small allocations and
frees, split four ways. It ran under QEMU's TCG on all three architectures. (This said four
emulated cores on each; `x86_64` has two in turn and one, corrected 2026-10-11 (UTC) below.) The program is `std_exerciser/src/bin/std_heap_contention.rs`. The test
`std_threads_tests::the_std_heap_is_measured_under_four_threads` runs it (pull request #1892). The figures per architecture are in `notes/std/threads.md`. This is the first
consumer this block was waiting for, and the same program is the measure for whatever replaces the
lock.

## What the lane found, 2026-10-10 (UTC)

Without `rseq`, a per-CPU list needs an atomic on every operation, stale id or not. A thread
preempted mid-pop hands the same core, and the same list, to another thread. A cache per thread
needs none, and after milestone 812 reaching it costs a subset of what reaching a per-CPU list
costs. So the lane built a cache per thread in front of the one locked heap, and measured it against
the per-CPU candidate rather than arguing.

## What was built and measured, 2026-10-11 (UTC)

- `crates/allocation_cache` (name provisional), host-tested and clean under Miri, generated into
  `std` and kept in each thread's block (milestone 812's thread-pointer block). A thread's end
  spills it; `std_threads_tests::the_std_heap_is_measured_under_four_threads` proves that with 64
  threads that each end with a full cache, twice the program's heap budget, and a replayable
  falsification.
- The measurement, `std_heap_contention` three times on every leg, millions of allocations per
  second of guest time with four threads (four against one in brackets):

| leg | (a) one lock | (b) per thread | (c) per CPU | (d) per CPU, no CPU read |
|---|---|---|---|---|
| aarch64, 4 cores | 3.31 (0.76) | **13.65 (2.48)** | 8.80 (2.14) | 2.42 (0.51) |
| riscv64, 4 cores | 1.73 (0.87) | **5.49 (2.57)** | 4.53 (2.84) | 3.83 (2.00) |
| `x86_64` PVH, 2 cores in turn | 2.72 (0.56) | **5.84 (1.26)** | 5.17 (1.41) | 3.93 (0.71) |
| `x86_64` OVMF, 1 core | 3.15 (0.61) | 4.64 (1.01) | 2.75 (0.72) | **5.58 (1.06)** |

Per thread wins wherever there is more than one core, and per CPU is not recommended. The page's
own number is (c) against (d): with four threads the CPU read is worth 3.6 times on aarch64 and 1.2
on riscv64, because without it 59 to 69% of list operations find the lock taken. With one thread
the read costs 16 to 50% under TCG. **Staleness did not matter**: an instrumented build counted no
operation whose id went stale, and at most one contended acquire, in about 400,000 per phase on
every leg. **`CPU_ID_BOUND` sizing was right**: eight line-aligned lists are 1.5 KiB, and cached
memory is spent only by lists in use. **For the page (milestone 557 (a thread reads its own CPU from
a page))**: it works, and its CPU word now has no reader outside tests. Whether the kernel keeps
writing it on every switch is calef's, on #1904. This is TCG on a loaded host, and `x86_64` runs its
cores in turn, so read ranks and ratios, not rates. `notes/std/allocation.md` has the method, the
raw times, the caveats and the scratch patch the variants live in.

Reuse: none to take, after surveying `talc`, `linked_list_allocator`, `buddy_system_allocator`,
`dlmalloc`, `rlsf`, `frusa`, `rsbmalloc`, `mimalloc` and `snmalloc` on 2026-10-10 (UTC)
(`notes/std/allocation.md`). The pure-Rust `no_std` ones are one heap behind one lock, which is what
`std` has; the thread-caching ones are C or C++ or need `mmap`, and would replace `user_mode_heap`,
which §31 (the foreign-language seam) rule 4 keeps. The cache adapts glibc's `tcache` design.

## Follow-on

- **Recorded.** The CPU read on silicon, in `notes/std/allocation.md`. Every figure here is TCG;
  the same four variants under HVF on Apple silicon (`notes/benchmarks.md`'s real-time instrument,
  aarch64 only) would put nanoseconds on the read, though not change the staleness or contention
  counts. The scratch patch makes it a rebuild.
- **Recorded.** The page's CPU word has no reader outside tests, in `crates/current_cpu_contract`'s
  `BUGS`. Whether the kernel keeps writing it is calef's question on #1904.
- **Recorded.** Cross-thread frees (a producer freeing what a consumer allocated) were not
  measured, in `notes/std/allocation.md`'s `BUGS`.
- **Recorded.** What the kernel's per-switch write of that word costs in time, below. calef ruled on
  2026-10-11 (UTC) that the write stays.
- **Recorded.** The same A/B on radon's U74, the one in-order core here, when someone is at the desk.

### The write's time cost, measured 2026-10-11 (UTC)

calef asked for this number before ruling on the write (#1955, lane/current-cpu-write-cost). **The
write costs less time than code layout moves.** The instrument is `script/bench --release` under
HVF on patagonia's Apple M3, one hart. Two kernels were built from one commit, one with the
`publish_current_cpu` call in `schedule` compiled out, and booted in shuffled pairs: 500 pairs over
two sessions. Each figure is a median per iteration, then the median paired difference (with minus
without) and its 95% bootstrap interval:

| row | with the write (IQR; min, max) | without | difference | interval |
|---|---|---|---|---|
| `ipc_rtt_el0`, 2 switches | 416.9 ns (393.9 to 420.3; 384, 671) | 416.2 ns | +0.78 ns, 0.19% | +0.25 to +1.43 |
| `ctx_switch`, 2 switches | 126.4 ns (118.1 to 127.7; 117, 202) | 126.7 ns | -0.19 ns | -0.42 to +0.01 |
| `yield_switch`, control | 31.0 ns (29.6 to 31.5) | 31.4 ns | -0.33 ns | -0.44 to -0.21 |
| `call_reply`, control | 69.2 ns (69.0 to 69.7) | 69.0 ns | +0.29 ns | +0.21 to +0.33 |

The two controls never run the changed code, since a kernel thread has no space, yet they move
0.3 ns between the same two builds. That is the layout floor, and the round trip's +0.78 ns is
barely above it.

**A cold line costs no more.** HVF passes through no PMU, so no cache-miss counter is reachable,
and the hypothesis was tested directly instead. A third kernel cleans and invalidates the word's
line to the point of coherency (`dc civac`, `dsb ish`) before the store, and a fourth flushes
without storing. Over 200 shuffled rounds the cold store costs +0.18 ns (-0.60 to +0.74) on the
round trip and +0.12 ns (-0.20 to +0.46) on `ctx_switch`, so the store buffer hides the miss. The
flush alone costs about 7 ns a switch, so it ran natively rather than trapping. In the bench itself
the line is normally hot, since only two spaces alternate.

Caveats:

- HVF is virtualization, not bare metal. Host load was 1.8 to 3.9 on four performance and four
  efficiency cores, and macOS chooses which kind runs the vCPU.
- `ipc_rtt_el0` is bimodal across boots, near 390 and near 420 ns. Both kernels land in the lower
  mode in 36 to 37% of boots.
- The counter ticks at 24 MHz, but each row is one window of thousands of iterations, so the limit
  is boot-to-boot spread rather than the counter.
- riscv64 and `x86_64` were not measured: radon and xenon each need a person at the desk to
  power-cycle. radon's U74 is an in-order core, where a store miss may cost more than on an M3.

### calef's ruling, 2026-10-11 (UTC)

calef ruled (A) on #1955: the kernel keeps writing the CPU id on every switch. Stopping it is not one line. Without the per-switch write, the single write when a space
is adopted goes stale, so `current_cpu` would name the wrong core. Doing it properly retires the
reader, the word, 557's tests and the self-observation half of §204 (how userspace asks where a
thread runs), to save a store and a branch the bench cannot see. Writing only for threads that
register, as Linux `rseq` does, was refused for the same reason: an opt-in interface and a
per-switch check to save nothing measurable.

## Index row

Milestone 557 (a thread reads its own CPU from a page)'s current-CPU page was justified by a consumer that does not exist, so the mechanism has no measured benefit and the numbers quoted for it are Linux's, about Linux. This builds the consumer: a userspace small-object allocator with a free list per CPU id that reads its own CPU on every allocation. The interesting half is the fallback, because the value can be stale the instruction after it is read and this tree deliberately has no `rseq`, so what correctness argument replaces restartable sequences is the question the milestone answers.
