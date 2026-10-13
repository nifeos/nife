---
status: AMENDED
raised: 2026-10-08
decided: 2026-10-08
ratified_by: calef
---

# 263. Threads are built: more than one thread in an address space

## The ruling

§105 (`std::thread::spawn` stays declined, until a customer needs it) declined threads, and its
2026-10-07 amendment reopened it for read-write ZFS without scheduling it. calef, 2026-10-08 (UTC):

> We should do threads. We deferred them because we didn't have a use. Now we've backed up uses.

So §105's condition is met, and this section supersedes it. Option A of
[`notes/thread-spawn-fork.md`](../../notes/thread-spawn-fork.md) is to be built: several threads,
each its own TCB, sharing one address space. Milestone 812 (`std::thread::spawn` runs real threads
in one address space) is the build, and it is now scheduled work rather than a parked block.

## The uses that are waiting

Each is a stranger's program or a benchmark the field already reads:

- `fio`, for risk 6 (a confined driver at real speed) and for risk 1 (only software written for
  nife runs on nife). It runs jobs as threads and keeps its own clock thread.
- `schbench` and `hackbench`'s thread mode, for risk 5 (not reliable on multicore). They measure
  wakeups between threads of one process, which nife cannot host today.
- `iperf3` 3.16 and later, which is multithreaded.
- `ripgrep`'s parallel walk, risk 1's third gap. Its threads have never run, because nife answers
  `available_parallelism()` with `Ok(1)`.
- Read-write ZFS through `libzpool`, `tough`, and parallel `rustc` and `cargo`, which §105's
  amendment already listed.

## What this decides, and what it leaves open

It decides that threads exist, on aarch64, riscv64 and x86_64 together (§19 (architectural parity is
a tenet)). It does not decide the syscall shape. Binding a space without consuming it changes what
`Tcb::CONFIGURE` promises, and a wait/wake primitive is new surface, so each owes a ruling before
code (§10 (process model: capability-based, microkernel) keeps the syscall surface narrow).
Milestone 812's block lists those forks with options and a recommendation for each.

## What it changes in other sections

- §105 is superseded. Its costing stays the starting point.
- §249 (a running address space stays nameable, and a capability never decides when it dies),
  amendment (b), refuses a second bind of a bound space and says §105 rests on that refusal. The
  refusal has to become a permission that can be granted, which is milestone 812's first fork.
- §22 (Rust `std` on the native ABI, the Hermit way) set `"singlethread": true` in the three target
  specifications and said it "flips off when `thread::spawn` becomes real". Milestone 812 flips it.
- §33 (the compositor's authority is memory, not messages) recorded the constraint as a structural
  fact: two threads cannot share an address space, so a process has one blocking wait point. Ten
  comments in `components/` and `crates/user_mode_runtime` cite §33 for that fact to justify a
  `static mut` or a shared page. The fact stops holding for any program that starts a second thread,
  so each site is a work item in milestone 812. §33's own reasoning about wait-any is unchanged,
  since §101 (notification objects) answered that separately.

## Amendment, 2026-10-12 (UTC): schbench is no longer waiting

`schbench` leaves the uses waiting on threads. calef dropped it on #2009
([his comment](https://github.com/nifeos/nife/pull/2009#issuecomment-6130684610)): *"c, and drop schbench"*. He ruled on #2021 that this section be amended to
match: *"Amend"*. Risk 5 (it cannot be made reliable on multicore, and the bugs appear only on
silicon) now takes its wakeup-latency tails from each kernel's own scheduler trace. That is
milestone 830 (wakeup latency on nife and Linux, from each kernel's own scheduler trace), and it
needs no threads in C. `hackbench`'s thread mode still waits on threads, as listed above.
