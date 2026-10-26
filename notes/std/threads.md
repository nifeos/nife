# `std` threads on nife

*Written 2026-10-10 (UTC) by milestone 812 (`std::thread::spawn` runs real threads in one address
space)'s lane. The kernel side is in `notes/processes.md`, `notes/futex.md` and
`notes/thread-pointer.md`; this note is the `std` PAL.*

## What a program needs, and where it gets it

Three slots of `std_runtime_contract`, all of them the program's own:

| slot | capability | used for |
|---|---|---|
| 10 | its process, `BIND` only | `thread::spawn` joins each new thread to it |
| 11 | its first thread, `WRITE` | the main thread sets its own thread pointer |
| 12 | its address space, `READ` | the futex waits `std`'s locks make |

The progenitor gives all three to every `std` program it builds (`StdLayout`), through
`supervision_protocol`'s `CHILDS_OWN_PROCESS`, `CHILDS_OWN_THREAD` and `CHILDS_OWN_SPACE`
markers. Threads add no authority beyond the program's own memory and time: a thread's TCB, stack and
thread-local block all come from the heap's region.

A program without slot 10 cannot spawn (`Unsupported`, as before 812). Without slot 12 its locks yield
instead of sleeping, which is correct and slower.

## The pieces

- **`sys/thread/nife.rs`.** A spawn retypes a TCB from slot 0 above the fixed slots (`RETYPE_OBJ`'s
  floor), allocates a stack and a thread-local block from the heap, and calls `CONFIGURE` with the
  process capability and the block as the thread pointer. `CONFIGURE` answers the thread's
  current-CPU page, which goes in its block before `START`. A thread ends with `SYS_EXIT_THREAD`
  naming its packet's word. The kernel clears that word once the thread has left user mode, so a
  `join` that sees it cleared may free the stack.
- `sys/thread_local/key/nife.rs`. Thread-local keys, xous's shape. The thread pointer names the
  thread's block of key slots, whose first word is the block itself for `x86_64`'s `fs:[0]`. The
  main thread makes its block on first use, before any second thread exists.
- `sys/sync/futex/nife.rs`. `AddressSpace::WAIT` and `WAKE` on slot 12, under the futex locks
  `std` already has for Linux.
- `available_parallelism` reads the allowance on the thread's current-CPU page: the online core
  count today.

## How it is proved

`system_tests/src/user/std_threads_tests.rs` runs `std_exerciser/src/bin/std_threads.rs` on all three
architectures. Four threads add to an `AtomicU64` and a `Mutex<u64>` a million times each, every
thread keeps its own `thread_local!`, the space outlives the first thread to exit, and
`available_parallelism` is the online count.

A second test runs `std_rayon.rs`, unmodified `rayon` from crates.io. Its global pool must have one
worker per online core, its parallel count over four million items must come to four million, and
`rayon::broadcast` must reach every worker. `rayon` is a dependency of `std_exerciser` only (calef,
#1892, 2026-10-10 UTC), and `deny.toml` bans it everywhere else.

## The heap under four threads

The block's item 12 asked for a measurement before any change to the heap's spinlock.
`std_heap_contention` does 200,000 small allocations and frees with one thread, then splits the same
work across four. Measured 2026-10-10 (UTC) under QEMU's TCG emulation by
`std_threads_tests::the_std_heap_is_measured_under_four_threads`. aarch64 and riscv64 have four
emulated cores. `x86_64` has two in its PVH leg, taken in turn on one host thread, and one in its OVMF leg
(`helpers/qemu-runner-x86_64.sh`). Its figures show what the lock costs and not how it scales.

| architecture | one thread | four threads | four against one |
|---|---|---|---|
| aarch64 | 63,442 us | 152,530 us | 2.4 times slower |
| riscv64 | 91,426 us | 189,922 us | 2.1 times slower |
| `x86_64` (two firmware legs) | 38,393 and 63,726 us | 123,778 and 132,899 us | 2.1 to 3.2 times slower |

Emulated time is a ratio to read, not a speed. The ratio says the lock is contended badly enough
that four threads do the same work slower than one: a thread preempted holding it makes the others
spin out their time slices.

A second run at commit `691cb473c`, on a quieter host, read 46.0 ms against 116.2 ms on aarch64
(2.5 times), 65.1 ms against 96.5 ms on riscv64 (1.5 times) and 29.6 ms against 70.7 ms on
`x86_64` (2.4 times). The ratio moves with the host's load. That four threads are slower than one
does not.

Milestone 561 (a per-CPU allocator is what the current-CPU page was for) put a cache per thread in
front of the lone lock. Four threads now do 2.5 times the work of one on aarch64 and riscv64
(`notes/std/allocation.md`). Its twelve further runs of the lone lock mostly agree with the
paragraph above, but not always. Four threads were slower than one in ten of them, and slightly
faster in two (riscv64 at 1.07 times, `x86_64`'s one-core leg at 1.05).

## BUGS

- A spawned thread's stack has no guard page. It is heap memory, so an overflow writes into the
  heap. A stack mapped from the region with an unmapped page below it is the fix. Until then a
  program that parses hostile input by recursion on a spawned thread turns deep input into heap
  corruption rather than a fault, so such a parser bounds its own depth.
- A detached thread's stack is never freed, because nothing remains to free it.
- A timed wait spins on `yield` until its deadline, because the kernel's `WAIT` has no timeout.
- One TCB page per spawn is spent for good, and 16 threads at most per process
  (`notes/processes.md`).
- **Fixed 2026-10-12 (UTC): the file page's lock spun.** `std::fs` shares one page per process with
  the file server and held a spinlock on it across each `CALL`, so under `rg`'s parallel walk the
  waiting workers spun while the holder slept in `CALL`. It is a futex mutex now
  (`sys/fs/nife/page.rs`, lane milestone/121-rg-threads).
