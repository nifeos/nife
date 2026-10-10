---
status: PARTIAL
raised: 2026-10-07
milestone_dependencies: none
decision_dependencies: 16, 19, 263, 269
machine_requirements: none
specific_machine: none
needs_person: no
---
# 812. `std::thread::spawn` runs real threads in one address space

*(Minted 2026-10-07 (UTC) by lane/filesystem-crate-probe from fork 5 of pull request #1803. The
number is provisional until the merge queue lands it; the title and slug are drafts. Scheduled
2026-10-08 (UTC) by §263 (threads are built: more than one thread in an address space), recorded by
lane/threads-and-fork.)*

calef reopened DECISIONS §105 (`std::thread::spawn` stays declined, until a customer needs it) on
2026-10-07 (UTC) for read-write ZFS, and did not schedule it. On 2026-10-08 (UTC) he did: *"We
should do threads. We deferred them because we didn't have a use. Now we've backed up uses."* §263
records that ruling and supersedes §105. This block is §105's option A, as
`notes/thread-spawn-fork.md` costed it.

## What exists, checked 2026-10-07 (UTC)

- `std::thread::spawn` returns `Unsupported` unconditionally
  (`patches/std-nife/overlay/std/src/sys/thread/nife.rs`).
- All three `targets/*-unknown-nife.json` set `"singlethread": true`, which routes `std` to its
  `no_threads` locks and a single static for thread-local storage.
- `Tcb::CONFIGURE` consumes the address-space capability it binds, and `kernel/src/thread.rs` owns
  that `AddressSpace` outright so the reaper's drop frees it. No two TCBs can share one today.
- `rayon`, `crossbeam-channel`, `tokio` and `ignore` already compile and link against the PAL. They
  fail at the first spawn, at run time.

Reuse: the kernel half is ours to write, since §46 (thin primitives or whole subsystems; we write everything in between) has the kernel written here. Its design
is seL4's, where `TCB_SetSpace` binds a VSpace without consuming it. The `std` half reuses std's
own threaded `sys` code for every platform shape it can, and writes only the nife arm of the PAL.

## The work

Kernel, on all three architectures:

1. An address-space capability can be bound to a TCB without being consumed. This changes what an
   existing method promises, so it is a syscall-surface change and owes a `design/decisions/`
   section ruled by calef before code. Whether the change is a new rights bit or a second
   `CONFIGURE` form is part of that ruling.
2. `AddressSpace` gains multi-holder liveness, the shape `Endpoint` already has under §16 (object
   revocation): teardown waits for the last TCB, not the first.
3. The reap path (`reap_region_objects`) learns "still referenced" beside live and dead.
4. Whether sibling threads share one `CapabilityTable` is the same fork one level down. seL4 answers
   it the same way, and the section in step 1 should rule both together.

`std`, in the overlay:

5. `Thread::new`, `join`, and thread exit, on the kernel's TCB methods.
6. Thread-local storage per thread, set up by the PAL on each architecture's thread pointer
   (`TPIDR_EL0`, `tp`, `FS` base).
7. Real locks: park and unpark on a kernel primitive that blocks one thread and wakes another, so
   `std::sync` stops using its `no_threads` variants.
8. `"singlethread": false` in all three target specifications, landed with step 7 and never
   before it, or every lock in `std` silently becomes a no-op under contention.

Weak memory ordering is the point of the exercise on aarch64 and riscv64, so every lock and the
TLS setup carries its ordering argument in a comment, per the codebase rule.

## What it breaks, each a work item

Each of these rests on one thread per address space and stops being true when a program starts a
second. Search for more before building; this list is what a grep found on 2026-10-08 (UTC).

9. `MappedWindow::as_slice` and `as_mut_slice` (`crates/user_mode_runtime/src/mapped_window.rs`).
   Their safety contract leans on "one thread per address space (DECISIONS §33)" for no concurrent
   writer. It must become a per-call-site claim that no other thread of this process touches the
   range.
10. Ten `static mut` buffers and shared pages in `components/` (`disk_surveyor`, `disk_partitioner`,
    `system_installer`, `login`, `line_editor`, `rmle`, `display_terminal`) argue soundness the same
    way. They stay sound while each program starts no thread, and each comment has to say that
    instead. Nine cite §33 (the compositor's authority is memory, not messages), which recorded
    the constraint as a fact.
11. The current-CPU page (`crates/current_cpu_protocol`, `kernel/src/user.rs`'s `current_cpu_page`).
    It is per address space, which is per thread only because of §105. The crate's `BUGS` names
    the change: per thread by something other than the space.
12. `MemoryRegionHeap`'s spinlock (`crates/user_mode_runtime/src/heap.rs`). Correct under
    contention, by its own argument, and not efficient. Measure it under four threads before
    replacing it.
13. §249 (a running address space stays nameable) amendment (b): a second bind of a bound space is
    refused with `WrongObject`. Step 1 replaces that refusal.
14. The per-thread user registers. aarch64's `TPIDR_EL0` and x86_64's FS base are not saved or
    restored by the kernel today, since nothing in userspace sets them. riscv64's `tp` is a general
    register the trap frame already saves, and the kernel borrows it for its own per-CPU pointer
    through `sscratch` (`kernel/src/arch/riscv64/trap.s`), which a TLS test must cover.
15. Two cores editing one space's page tables at once. A parent holding a child's space can already
    map into it while the child runs, so a lock may cover this; check rather than assume.
16. `available_parallelism()` answers `Ok(1)`
    (`patches/std-nife/overlay/std/src/sys/thread/nife.rs`). It must report the cores the process
    can run on, counted from the online set and not from `CPU_ID_BOUND` (`crates/cpu_set` says why
    counting is not indexing).

## The forks, for calef

Ruled 2026-10-09 (UTC) by calef, all seven, in [§269 (how threads share a process)](../decisions/0269-how-threads-share-a-process.md). Forks 4, 5 and 6 went differently from the recommendations below, and forks 2 and 7 were widened; §269 is the record, and the list below is kept as the question calef was asked. One consequence is open: fork 5 makes a thread join a process object, so whether fork 1's `BIND` right moves onto it comes back to calef before that code.

As first written: none is decided here. Each needs a ruling before its code, since 1, 2, 3 and 6 are syscall surface
under §10 (process model: capability-based, microkernel), and the rest are agreements between the
kernel and `std`.

1. How a second TCB joins a space. (a) A right on the address-space capability, say `BIND`, which
   lets `CONFIGURE` bind without consuming. (b) A second `CONFIGURE` form that never consumes. (c) A
   new method on the space that makes a thread inside it. Recommended: (a). Adding a thread to a
   space is running code inside it, so it should be an authority that can be withheld and narrowed
   on delegation, which a right is and a method form is not. seL4's `TCB_SetSpace` does not
   consume either, from memory and not reread.
2. The wait/wake primitive under `std`'s locks. (a) Futex-shaped wait and wake, keyed by a virtual
   address in the caller's own space, as methods on that space's capability. (b) A capability object
   per lock, such as §101 (notification objects). (c) Spin and yield in userspace only. Recommended:
   (a). `std`'s `Mutex::new` is `const` and makes no syscall, which a capability per lock cannot
   honor, and a key inside one's own memory carries no authority beyond that memory. Zircon, also
   handle-based, chose address-keyed futexes (from memory). (c) Burns a core under exactly the
   contention schbench measures. If a method on the space is refused, the fallback is a new syscall
   number, which is a bigger ask.
3. Whether sibling threads share one capability table. (a) Shared. (b) One each. Recommended: (a).
   A `File` or `TcpStream` is `Send` in `std`, and a handle moved to another thread must still name
   the same slot. POSIX threads share descriptors for the same reason.
4. Thread-local storage on each architecture. (a) The register is writable from userspace on all
   three (`TPIDR_EL0` already is; `tp` is a general register; x86_64 needs `CR4.FSGSBASE`), and the
   kernel saves it per thread. (b) The kernel sets it, through a TCB method, and userspace never
   writes it. (c) `TPIDRRO_EL0` on aarch64, read-only to userspace. Recommended: (a), with the
   initial value passed at `CONFIGURE` so a thread's first instruction already has its block. If a
   board lacks `FSGSBASE`, x86_64 falls back to (b) on that board, which the parity suite must
   show.
5. What ends a process. (a) The space lives until its last thread, and `std`'s `exit` destroys its
   sibling threads through the TCB capabilities the PAL holds, which milestone 133 (ending a
   permanently blocked thread) made possible. (b) A kernel thread-group object that dies as one.
   Recommended: (a). No new object, and the supervisor's view stays one capability per thread.
6. The current-CPU page once threads share a space (item 11). (a) One page with a slot per thread,
   one cache line each, the slot learned at thread start. (b) A page per thread at a per-thread
   address. Recommended: (a), as the crate's `BUGS` suggests, with false sharing measured rather
   than assumed away by the padding.
7. Where `available_parallelism()` reads the count (item 16). (a) A field in a read-only page the
   kernel already maps. (b) Milestone 527 (the `SURVEY` selector). (c) A number the spawner grants.
   Recommended: (a). No crossing, and a later affinity budget can lower the same field.

## Exit

A test in `script/test`'s suite runs on aarch64, riscv64 and x86_64 and fails if any step is
missing. A program spawns four threads that each increment one shared `AtomicU64` and one
`Mutex<u64>` a million times, joins them, and checks both read four million. It also checks each
thread saw its own thread-local value, and that the address space outlived the first thread to
exit. A second check runs `rayon`'s parallel sum to the same answer.

## What it unlocks

- The standard benchmarks that need threads, minted by lane/standard-benchmarks (#1854, numbers
  provisional). 833 (`fio`) serves risk 6 (a confined driver at real speed) and risk 1 (only
  software written for nife runs on nife). 827 (`hackbench`) and 830 (`schbench`) serve risk 5 (not
  reliable on multicore). 828 is `iperf3` 3.16 and later, which is multithreaded. 832 (STREAM)
  needs a thread per core for its per-core figure.
- Risk 1's third gap: `ripgrep`'s threads, which nife has never run.
- Read-write ZFS through OpenZFS's `libzpool`, hosted as a confined CDDL program. A later
  milestone, not prioritized; see notes/filesystem-crates-2026-10-07.md.
- Read-write btrfs through LKL (Linux Kernel Library), recorded as an alternative in the same note.
- `tough`, the TUF client pull request #1805 names, whose `tokio` `fs` feature runs file I/O on a
  blocking thread pool.
- `ripgrep`'s parallel directory walk; milestone 121 (`ripgrep` on nife: enumeration as a capability) is single-threaded today.
- Parallel `rustc` and `cargo` for milestone 173 (`rustc`/`cargo`/LLVM natively on nife), which
  today plans on the single-threaded fallback.
- The Rayon-parallel variants of milestone 149 (a common HPC benchmark suite, in Rust).

## BUGS

- **Built so far, on all three architectures** (pull request #1892, 2026-10-10 UTC): item 14, each
  thread's own thread pointer, set by the kernel (`notes/thread-pointer.md`). And the kernel half of
  item 7: futex `WAIT` and `WAKE` on the address-space capability (`notes/futex.md`). Each has
  system tests and replayable falsifications. Neither is used by `std` yet.
- **Not built:** items 1 to 6, 8 to 13, 15 and 16, and the exit test. The join (items 1 to 4)
  waits on calef: §269's open consequence asks where the `BIND` right lives, and the question is
  posted on #1892 with two smaller ones about `CONFIGURE`'s words and a thread-only exit.
- **The futex has no timeout**, so `std`'s `park_timeout` and `Condvar::wait_timeout` cannot be
  written on it yet. The exit test does not need them; `notes/futex.md`'s `BUGS` has the plan.
- This block gives Rust programs threads. `fio`, `hackbench`, `schbench` and `iperf3` are C, and
  want pthreads from a C library nife does not have. §31 (the foreign-language seam: C holds no
  capabilities and makes no syscalls) bars C from syscalls. Full POSIX, milestone 478 (tier three),
  is refused. How those benchmarks get threads is not decided here; the maintainer is taking it to
  calef.
- The costing in `notes/thread-spawn-fork.md` is from 2026-08-22; the kernel has changed since,
  and the first act of a lane here is to re-check its file references.

## Follow-on

- **Outstanding.** The join, items 1 to 4: the process object (§269 forks 3 and 5), multi-holder
  liveness for the space, and the `BIND` right. Blocked on calef's ruling on #1892; checked on
  2026-10-10 (UTC) by reading `thread_control_block_configure` in `kernel/src/syscall.rs`, which
  still consumes the space's capability and refuses a bound space.
- **Outstanding.** The `std` half, items 5 to 8, and the exit test. They need the join; checked the
  same day in `patches/std-nife/overlay/std/src/sys/thread/nife.rs`, which still returns
  `Unsupported`.
- **Outstanding.** Items 9 to 13, 15 and 16. Item 11 and item 16 (the per-thread current-CPU page
  and its allowance field, §269 forks 6 and 7) wait on the same `CONFIGURE` question as the join.
  The rest are audits of code that stays sound while no program starts a second thread.
- **Recorded.** `notes/futex.md` carries the futex's open limits (no timeout, an unargued
  page-walk race, whole-bucket wakes); `notes/thread-pointer.md` carries the thread pointer's
  asymmetry and the `TPIDRRO_EL0` alternative.

## Index row

Real shared-memory threads: `std::thread::spawn` on all three architectures, by letting several
TCBs bind one address space (§105's option A, scheduled by §263 on 2026-10-08). It unlocks `fio`,
`schbench`, `hackbench`, `iperf3`, parallel `rg`, read-write ZFS and parallel builds. §269 ruled
the seven forks; where the `BIND` right lives awaits calef. The thread pointer and the futex are
built.
