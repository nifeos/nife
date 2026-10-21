---
status: NOT-STARTED
raised: 2026-10-21
milestone_dependencies: 126, 282, 812
decision_dependencies: unwritten
machine_requirements: none
specific_machine: none
needs_person: no
---
# 942. `ps` and `top` show which process each thread belongs to, and how many threads it has

*Name: provisional. The number 942 is provisional until the merge queue lands it, and the title
and slug are too.*

calef approved raising this on 2026-10-21 (UTC), in place of a `BUGS` line. Raised only: a
writing-only lane wrote it and built nothing. Milestones 126 and 282 are BUILT, so this is new work
on their programs, not a reopening of them.

## The gap

`components/src/ps.rs` and `components/src/top.rs` print one row per thread: `TID`, `STATE`,
`TIME(ms)`. Since milestone 812 (`std::thread::spawn` runs real threads in one address space) made `std::thread::spawn` real, a program with several threads is
several rows with nothing joining them. `rg --threads 4` shows five unrelated tids, and a reader
cannot tell that they are one program, or whether the five include a fifth that is somebody else's.
`top` ranks by thread, so a process whose work is spread over eight threads ranks below one that
spent the same time on one.

## What a process is here

- A process is an address space, a shared capability table, and a count of its members
  (§269 (how threads share a process)). `kernel/src/sched/process.rs` holds it: `members`, which
  falls to zero as threads leave, and `started`, which only grows.
- It has no name. `ps`'s `BUGS` says so: "No `CMD` column: there is no display name, and no
  design for whether a confined viewer may see one." That gap is its own and this milestone does
  not close it.
- It has no pid space. A kernel-internal `ProcessId` exists and no program holds one or can pass
  one to a call (`notes/process-view.md`: no call in a program's reach takes a process id it was
  not shown).
- `ps` sees only its supervision domain: the threads whose deaths arrive on the endpoint it holds
  with `ENUMERATE` in `grant_plan::DOMAIN_SLOT` (§223 (the process view is the supervision
  domain)). So "the process a thread belongs to" can only mean the processes that own the threads
  it can already name.
- The kernel already counts. `sched::process_threads_started` answers how many threads a process
  has ever started, and `system_tests/src/user/ripgrep_threads_tests.rs` reads it. It is
  `#[cfg(feature = "system_tests")]`, so no program can reach it, and "ever started" is not "has
  now" (`members` is that).

## What `ps` and `top` should show

A process grouping and a thread count. The simplest form is one column on every row, a process
label shared by the threads of one process, plus a `THR` count column. The alternatives are a
process header row with the threads indented under it, or `top` summing `TIME` per process with a
flag for the per-thread view. Which is an open question below. Whatever the form:

- Two threads show the same label if and only if they share an address space and capability
  table.
- The count is live members (`members`), not threads ever started. A thread that has exited
  and not been reaped is `DEAD` in the existing `STATE` column, and the count says what a reader
  expects: it equals the number of rows the group has.
- The label is a name for the viewer's snapshot, not the kernel's `ProcessId`. It need only be
  stable across one program run's walks (the `TIME` join relies on the same).
- `top` ranks by process when asked, summing member `CPU_TIME`; the per-thread ranking milestone
  282 built stays.

## The authority question

Does `ENUMERATE` on a domain already cover learning which of its threads share a process?

The case for yes, under §150's reasoning. §150 (how does a thread's CPU time reach userspace?) and
§204 (how userspace asks where a thread runs) ruled that a new per-thread fact is a new `SURVEY`
record selector. `CPU_TIME` needed no new right because the second walk uses the
same endpoint and the same right as the first. Membership is a fact about threads the viewer can
already name, and it reveals no thread, process or capability the viewer could not already see.
Grouping tids the viewer already holds adds a relation among them and no new object.

The case for asking: §150 was about a number that describes one thread. Process membership tells
the viewer that two tids share memory and capabilities, which is a statement about their
confinement boundary, and `notes/process-view.md` is deliberate about what a domain may name. A
thread's endpoint is fixed at `START` (§223), so siblings may have different supervisors. A viewer
holding only part of a process would learn that a sibling exists outside its domain if the count
were the process's total. The count should then be the count within its domain. Whether that is
acceptable, or the count must be the true total, is the open question below.

This is a wire-and-authority question on the syscall surface, so it is calef's (§10 (process model: capability-based, microkernel); rule 3 in
`AGENTS.md`), and unwritten.

## Where the data comes from

`abi::rendezvous::SURVEY` today, one record per call, selected by `abi::survey::record`
(`STATE`, `PLACEMENT`, `CPU_TIME`). The cheapest shape is a new selector, `record::PROCESS`, that
answers a per-snapshot process label for a tid, on the walk `ps` already does for `TIME`
(`Survey::join_cpu_time` in `crates/ps`). The thread count is then computed in the program by
counting rows per label, so the kernel exposes no count and `members` stays internal.

The kernel side is in `sched::survey_supervised` (`kernel/src/sched/region_reaping.rs`), which
already finds a thread's slot. It needs the thread's process; `process_of` in
`kernel/src/sched/process.rs` goes from a `ProcessId` to the record, and the lane finds the way from
a thread slot to it (not checked here). The label must not be that page's address or the `ProcessId`, since
either would publish the kernel's layout. A per-walk ordinal (the first distinct process the walk
meets is 1) would reveal nothing but the grouping and costs the kernel one comparison. It
makes the label differ between the two walks if a process ends in between. The label design is
for the lane that builds this, inside whatever the ruling allows.

A `SURVEY` selector adds no register and no syscall, so it does not widen the surface beyond
the rule §204 already set. Adding a value to `is_known` and an arm in `survey_supervised` is the
change.

## The dependency on the `CMD` gap

The table would show a label and a count, not a name. `rg --threads 4` would read as "process 3,
5 threads", which is better than five rows and still not "rg". The `CMD` column needs a design
this tree does not have (`ps`'s `BUGS`): where a process's display name lives, who sets it, and
whether a confined viewer may read it. That has no milestone and this one does not raise it. The
two are separable: the grouping is useful with an opaque label, and `CMD` slots in later as another
column on the same record walk. If calef wants `CMD` first, this one waits on it. That is a
dependency on a milestone that does not yet exist, so `milestone_dependencies` does not list it.

## Open questions for calef

1. Authority. Does `ENUMERATE` on a domain cover learning thread-to-process membership, as it
   covered `CPU_TIME` (§150, §204), or does a process grouping need its own right or its own
   decision?
2. A partial process. When a process has threads in several supervision domains, does the
   count mean the threads this viewer can see (its domain only), or the process's true member
   count? The first leaks nothing and can undercount; the second tells a confined viewer that a
   sibling exists elsewhere.
3. Form. A column on every row, grouped sections with the threads indented under the process,
   or a flag? And does `top` rank by process by default, by thread by default, or on a flag?
4. The label. An opaque per-walk ordinal, or something stable across runs of `ps`? Stable
   across runs means the kernel hands out a process identity a program can hold, which `ps` has
   deliberately never been given.
5. Order against `CMD`. Build this first with an opaque label, or wait and ship the grouping
   and the name together?

## Exit criteria, once the decision is written

1. A new `SURVEY` selector (or whatever the ruling chooses) answers a process label for every
   thread in a domain, in `abi::survey::record`, `is_known` and `sched::survey_supervised`, on
   aarch64, riscv64 and x86_64.
2. `crates/ps` carries the label and the per-process count on its rows, host-tested. A table with
   two processes of three and one threads prints the right grouping, and a thread that dies
   between walks degrades the way `TIME` does (`-`) rather than miscounting.
3. `components/src/ps.rs` and `components/src/top.rs` print it, and their module docs and `BUGS`
   say what is shown.
4. A kernel test spawns one process with four threads under `std::thread::spawn` and a second
   with one, and surveys their shared domain. It asserts that exactly two labels appear, that the
   four tids share one, and that the count per label is four and one. It runs under QEMU on all
   three architectures. This is the test that nothing else would catch: milestone 812's tests read
   `process_threads_started` inside the kernel, and none of them goes through what a program can see.
5. A user-visible check in `script/swish-check` or the basalt integration tests (milestone 916
   (integration tests live in basalt) moves them there). `ps` or `top` under `rg --threads 4` shows the five
   threads as one group with a count of five (the main thread and four workers). Until 916 moves
   the harness, the kernel test in item 4 is the proof and this one waits.
6. A thread of a process outside the viewer's domain does not appear, and under the ruling for
   question 2 the count says what the ruling says. The test names which.

## BUGS

- The decision is unwritten. The authority question and the five above are open.
- **The process page is `#[cfg(feature = "system_tests")]`-only to userspace today.**
  `process_threads_started` is a test hook and is not what `ps` would read.
- **`CMD` is a separate gap.** An opaque label is the most this can show.

Reuse: the tree's own `SURVEY` selector (§204) and `Survey::join_cpu_time` are the mechanism, so
nothing outside is taken. Prior art, recalled rather than read. Linux `ps` and `top` group by tgid
and show `NLWP` (threads in the process), with `-L` or `H` for the per-thread view. Zircon's
`zx_object_get_info` lists a process's threads from a process handle, so grouping is the handle the
caller holds; seL4 has no enumeration at all.

## Index row

`ps` and `top` list threads one per row, so a program with several threads reads as unrelated
rows. This groups them by process and shows a thread count, from a new `SURVEY` record, and
waits on a ruling about whether `ENUMERATE` covers learning that threads share a process.
