---
status: NOT-STARTED
raised: 2026-10-08
promoted_from: sqlite-speedtest1-on-nife-and-linux
milestone_dependencies: 835, 809, 801, 897
decision_dependencies: 262, 265
machine_requirements: silicon for wall-clock rows, radon first, then xenon
specific_machine: none
needs_person: yes
---
# 831. SQLite's speedtest1 on nife and Linux, a real database workload

*(The title and slug are provisional.)*

Model: milestone 833 (the same storage benchmark on nife and Linux, by porting real fio).

**In brief.** Port SQLite's `speedtest1` (one C file beside the amalgamation) and run it on nife and
Linux with identical parameters. It is the cheapest real database workload there is.

## What it measures

About 100 numbered SQL tests (inserts, indexed and unindexed selects, updates, joins, `VACUUM`),
each timed. It stresses the C library, the allocator, `mmap` or file I/O and the clock far more
than the CPU. A separate in-memory run (`:memory:`) isolates compute and allocator from storage.

## Which fatal risk it informs

Risk 1 (`design/fatal-risks/1-only-software-written-for-nife.md`): SQLite is the canonical C you
cannot rewrite (milestone 36 (a foreign-language component, seam first) says so). A passing
`speedtest1` is a third-party
program running unmodified on nife. Risk 4 (the per-crossing cost): each test's file I/O count is
known, so the time per crossing falls out.

## What nife must supply, as questions

- Does nife's libc surface (or the C toolchain path of milestone 36) build the amalgamation at all?
- Which VFS does SQLite get: unix over a file capability, or a small custom VFS over the fs server?
  Locking and `fsync` semantics decide whether the durable tests are comparable.
- Is there a monotonic clock fine enough for sub-millisecond tests, and `mmap` or a substitute?
- How much heap can a C program draw (some tests allocate tens of MB)?

## The Linux comparison

The same `speedtest1.c` and SQLite version, pinned by tag and amalgamation hash, same `--size`,
`--journal`, `--pagesize` and `--cachesize`, run on the same board against the same storage
(ext4 on the NVMe Linux sees, nife's filesystem on the same drive). Linux kernel, compiler and flags
are recorded beside nife's numbers. A durable run (`synchronous=FULL`) and a non-durable one are
reported separately, because nife's write path commits per request without a device flush unless
asked (see `notes/benchmarks.md`, "What is not apples to apples").

Reuse: SQLite itself is taken unmodified; no benchmark program is written.

## What it waits on

The C library is §265 (a C library started from relibc), built as milestone 835 (a C library,
stage 1: files, clock and memory). `speedtest1` runs unmodified on nife under QEMU on all three
architectures. What remains is the Linux comparison on silicon.

Neither threads nor fork. SQLite builds single-threaded (`SQLITE_THREADSAFE=0`), and `speedtest1`
runs in one process. Of the C programs here, this is the one the most people would run for its own
sake.

## How it runs continuously

QEMU TCG wall-clock timings are meaningless, so nothing here gates on them. Where a host-side
harness allows, CI gates on deterministic instruction counts through `script/bench` (a pinned
icount baseline per workload step, `--check` failing on drift). The wall-clock runs happen on
silicon on a schedule, as the runbook does for the soak, and write dated rows to
`notes/benchmarks.md` with the Linux row beside them. A run that cannot reach the board writes
nothing rather than a TCG number.

## Done when

*§297 (software reaches a nife system as a package `jig` installs from basalt's index, the kernel
included) binds this. calef, 2026-10-12 (UTC): "They should all be installed via jig through
basalt."*

`speedtest1` must run on nife installed by `jig install` from basalt's package index. basalt builds
the package against the nife commit it pins. The install waits on milestone 809 (the package client
becomes a program, `jig`, with the verbs an index needs). It also waits on milestone 801 (packages
over the internet: a host name, HTTPS and the distribution's index). The package itself is milestone
897 (SQLite is packaged in basalt, and its tests run in basalt's package build). No work toward that
starts until those milestones are built (calef, 2026-10-12 UTC: "For our other programs let's block
start on packaging.").

## Index row

SQLite's speedtest1, the cheapest real database workload, unmodified on nife and Linux. Needs only a
C library: no threads, no fork.
