---
status: BUILT
raised: 2026-10-08
built: 2026-10-12
milestone_dependencies: 835, 812
decision_dependencies: 265
machine_requirements: none
specific_machine: none
needs_person: no
---
# 836. A C library, stage 2: threads

*(The title and slug are provisional.)*

Stage 2 of §265 (a C library started from relibc): `pthread_create`, `pthread_join`, mutexes,
condition variables, thread-local storage for more than one thread, and the thread-safe `stdio`
locking relibc already has. It rides on milestone 812 (`std::thread::spawn` runs real threads in one
address space), which PR #1856 rules is to be built.

## First consumers

hackbench (827) in its thread mode, iperf3 (828) and the storage benchmark (833), all on PR
#1854.

Reuse: relibc's `src/pthread/` and `src/sync/` (MIT), seeded as stage 1 seeded the headers.

## What it builds

relibc's `src/pthread/` and `src/sync/`, seeded as stage 1 seeded the headers, over two platform
calls: thread creation (relibc's `rlct_clone`) and a wait-on-address pair (its `futex_wait` and
`futex_wake`). Milestone 812 provides that pair, so `src/sync/` is kept nearly whole: corrected
2026-10-11 (UTC), since 812 (#1892) built `abi::address_space::WAIT` and `WAKE`, an address-keyed
futex (`notes/futex.md`). It has no timeout yet, which a timed wait such as
`pthread_cond_timedwait` will meet.
`TPIDR_EL0`, `tp` and the x86_64 FS base become per-thread state, which is 812's to switch.

## Done when

hackbench's thread mode builds unmodified and runs on all three architectures. It is this block's
one program. Not schbench, because the C library does not build `syscall()` and §265 (a C library
started from relibc) stands as written (calef, 2026-10-12 UTC, on #2009: "c, and drop schbench").
Milestone 830 (wakeup latency on nife and Linux, from each kernel's own scheduler trace) measures
the tails by tracing instead.

## What is built

hackbench (rt-tests 2.11, `helpers/build-hackbench.sh`) runs unmodified in its thread mode on
aarch64, riscv64 and x86_64 under QEMU (`system_tests/src/user/c_thread_tests.rs`, two tests,
skipping where the helper was not run). Over pipes, two groups of three senders and three
receivers; over its default `socketpair(AF_UNIX, SOCK_STREAM)`, one group of seven and seven,
fifteen threads with the main one. Every sender's 100 messages reach every receiver of its group,
every thread is joined, and the process ends with every thread reaped. Each test has an attested
falsification. SQLite's `speedtest1` and ioping still pass on the threaded library.

The library ([`c_library/README.md`](../../c_library/README.md) is the guide, with its `BUGS`):

- relibc's `pthread.h` (attributes, mutexes, condition variables, read-write locks, barriers, spin
  locks, keys, `pthread_once`) and its `sync` module are seeded. So are `sched.h`, `semaphore.h`,
  `poll.h`, `sys/socket.h`, `netinet/in.h`, `arpa/inet.h` and `sys/un.h`, and `setjmp.h` with
  musl's assembly, less the float registers a soft-float process has none of.
- `pthread_create` is a `std` spawn (`c_library/src/pthread.rs`), not relibc's `clone` and
  dynamic-linker control block. `pthread_exit` `longjmp`s back to `c_library/c/thread_start.c`.
- relibc's wait-on-address pair is built on `std`'s thread parking (`futex.rs`), a parking lot
  keyed by address, so the crate still makes no system call of its own.
- `errno`, the locale, cleanup handlers and key values are per thread, on `std`'s
  `thread_local!`. relibc's `#[thread_local]` statics cannot work on nife, whose thread pointer
  names `std`'s block.
- The descriptor table is a `std::sync::Mutex`. A pipe or socket read or write takes its object
  out of the table and waits with the lock released.
- Pipes, `socketpair` and `poll` live inside the process (`stream.rs`).

The `helpers/build-ioping.sh` and `build-speedtest1.sh` fix: each read only the first of the two
lines `helpers/c-library-cflags.sh` prints, so neither got nife's headers. Both now read both.

## What this block decided

Each is reversible:

- The thread is `std`'s. The platform layer is on `std`, as milestone 835 (a C library, stage
  1: files, clock and memory) decided, and `std` already has the thread, its stack and its thread
  pointer, so no second spawn path was written.
- The futex is a parking lot on `std`, not a new `std` export of the kernel's
  `AddressSpace::WAIT` and `WAKE`. It costs a second wake per handoff (`futex.rs`'s `BUGS`) and
  adds no public API to `std`.
- Pipes and socket pairs are process-local queues. No kernel object is needed while both ends
  are in one process.
- `FIONBIO` is the one `ioctl` kept, as `fcntl`'s `O_NONBLOCK`.
- hackbench's build passes `-Wno-incompatible-pointer-types`. glibc's socket calls take a
  transparent union under `_GNU_SOURCE`, and relibc's take `struct sockaddr *`.

## BUGS

- The rest of this stage's limits are in `c_library/README.md`'s `BUGS`: C thread-locals do not
  link, 16 threads a process, stacks from the heap, deferred cancellation only at
  `pthread_testcancel`, and pipes that cannot leave the process.

## Follow-on

- **Milestone 912.** Milestone 912 (C thread-locals on nife's thread pointer): C's own
  `__thread`, which fio (milestone 833 (the same storage benchmark on nife and Linux, by porting
  real fio)) uses only when it links. Without it fio falls back to `pthread_getspecific`.
- **Milestone 827.** Milestone 827 (hackbench on nife and Linux, as a scheduler benchmark and a
  multicore soak): the same program on silicon beside Linux, now that its thread mode runs here.
- **Recorded.** The per-call limits, in `c_library/README.md`'s `BUGS`.

## Index row

POSIX threads for nife's C library, on milestone 812's threads, so the scheduling benchmarks can
run as their authors wrote them.
