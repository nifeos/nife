---
status: NOT-STARTED
raised: 2026-10-08
milestone_dependencies: 261, 835, 809, 801, 906
decision_dependencies: 262, 265
machine_requirements: x86_64 silicon with VT-d and an NVMe drive
specific_machine: xenon (the Micron 2450 that risk 6's first comparison was taken on)
needs_person: yes
---
# 834. ioping on nife and Linux, storage latency one request at a time

*(The title and slug are provisional.)*

calef, 2026-10-08 (UTC): *"I'm leaning ioping now ...
ioping beats SQLite to move our risks, but SQLite is way more useful."*

In brief. Port unmodified `ioping` and run it on nife and on Linux on xenon, against the same files,
with real logs, repeated passes and a latency for every request. It is the lightest standard storage
benchmark there is: one process, one thread, one request in flight. So it can run before threads
exist, which milestone 833 (real fio) cannot.

## What it measures

ioping issues one I/O at a time, like `ping`, and prints the latency of each one, then the minimum,
mean, maximum and deviation. With direct I/O it times the driver and the device rather than a
cache. Request size, count and read or write are flags, so a run can match risk 6's first
comparison: 4 KiB, queue depth 1, a fixed window on the Micron 2450.

## Which fatal risks it informs

Risk 6 (a confined driver at real speed). calef put it back to amber on 2026-10-08 because nife's
read was 2.16x Linux's and nobody can explain it. Those figures came from one pass per boot,
transcribed from photographs. A latency per request, from logs on both systems, shows whether
nife's reads are fast or Linux's are slow, and in which part of the distribution.

Risk 1 (only software written for nife runs on nife): a third-party C program running unmodified.

## What nife must supply, as questions

- What the target is. ioping takes a file, a directory or a device. "The same files" means the same
  size at the same offset inside the window the confined driver owns. Is that a file on nife's
  filesystem, or the block window itself? Linux gets the same choice, and both sides make the same
  one.
- What direct I/O means on nife. The block server keeps no cache, but the filesystem server may. If
  the file path goes through a cache, the run uses the block window instead, and the row says so.
- A clock with microsecond resolution or better, reachable from C.

## The target and direct I/O on nife

calef ruled both on nifeos/nife#2371 (2026-10-18 UTC): *"Yes, read-only at prompt"* (21:36) and
*"Per-client window"* (23:06).

- **The target is the block window, through the disk tree.** `components/src/disk_tree.rs` serves
  the whole NVMe namespace as one read-only file, `/disk/nvme0/n1`, with no cache: every read is
  one device read (`crates/disk_tree`'s tests hold it). It is the installed system's second mounted
  tree. ioping names the window by flag on both systems, `-o 1m -S 64m`, against `n1` on nife and
  `/dev/nvme0n1` on Linux.
- **The disk tree is the NVMe server's second client.** The NVMe server stages each request in the
  transfer window its endpoint's badge names (`non_volatile_memory_express::CLIENT_WINDOWS`), so
  the disk tree reads beside a live filesystem server, and the disk tree's window cannot write.
- **Direct I/O is `O_DIRECT` in the C library** (`c_library/README.md`'s BUGS says what it does
  not promise). basalt's recipe builds ioping with `-DHAVE_DIRECT_IO`, ioping's own Linux
  definition, so `-D` is accepted and the "non-cached I/O not supported" warning is not printed.
- **The file path stays cached.** A file on `redoxfs_server` is served from its memory after the
  first read, so a run against one measures that memory, and the row says so.

The run at the prompt:

```text
cd /disk/nvme0
ioping -D -c 1000 -i 0 -s 4k -S 64m -o 1m n1
```

Reuse: the disk tree is written. Nothing in the tree or in a dependency serves a raw block range
over the file contract, and the server is a namespace of two names over `file_system_contract::blk`.

The names `/disk`, `nvme0`, `n1` and `disk_tree` are provisional.

## The Linux comparison

The same ioping release, pinned by tag. The same flags on both sides: direct I/O, 4 KiB, a fixed
count, reads and then writes. The same disk and window as `bench/xenon-2026-10-04/`. Linux runs from
the Fedora live USB used on 2026-10-04 or a pinned successor, and its kernel version, I/O scheduler
and polling mode are recorded. Each system boots at least three times and runs at least five passes
per boot. Every pass is reported, not only a median.

The logs are committed whole under `bench/xenon-<date>/`, as the program printed them. Nothing is
transcribed by hand.

## What it waits on

The C library is §265 (a C library started from relibc), built as milestone 835 (a C library,
stage 1: files, clock and memory). ioping 1.3 runs unmodified on nife under QEMU on all three
architectures. What remains is the comparison on xenon.

It needs nothing else that is missing. From memory, to be checked against the pinned source, ioping
uses `open`, `pread` and `pwrite`, `clock_gettime` and `getopt`, with no threads and no `fork`. Of
the benchmarks §262 lists, it asks the least of nife.

Reuse: ioping is taken unmodified; no nife-side benchmark program is written.

## How it runs continuously

QEMU TCG wall-clock timings are meaningless, so nothing here gates on them. Where a host-side
harness allows, CI gates on deterministic instruction counts through `script/bench` (a pinned
icount baseline per workload step, `--check` failing on drift). The wall-clock runs happen on
silicon, and write dated rows to `notes/benchmarks.md` with the Linux row beside them. A run that
cannot reach the board writes nothing rather than a TCG number.

## Done when

*§297 (software reaches a nife system as a package `jig` installs from basalt's index, the kernel
included) binds this. calef, 2026-10-12 (UTC): "They should all be installed via jig through
basalt."*

ioping must run on nife installed by `jig install` from basalt's package index. basalt builds the
package against the nife commit it pins. The install waits on milestone 809 (the package client
becomes a program, `jig`, with the verbs an index needs). It also waits on milestone 801 (packages
over the internet: a host name, HTTPS and the distribution's index). The package itself is milestone
906 (ioping is packaged in basalt, and its test installs it with `jig`). No work toward that starts
until those milestones are built (calef, 2026-10-12 UTC: "For our other programs let's block start
on packaging.").

## BUGS

- ioping is a common operator's tool, but papers cite it far less often than fio. It is here
  because it moves risk 6 soonest, not because it carries the most weight. Milestone 833 is the
  comparison with weight behind it.
- Queue depth 1 only. "Real speed" beyond one request in flight is milestone 833's question.

## Index row

The lightest standard storage benchmark, unmodified on nife and Linux on xenon with real logs and a
latency per request, so risk 6's unexplained 2.16x read is explained or corrected before fio can
run. Its C library is milestone 835, built.
