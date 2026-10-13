---
status: NOT-STARTED
raised: 2026-10-07
promoted_from: the-same-storage-benchmark-on-nife-and-linux
milestone_dependencies: 261, 812, 835, 809, 801
decision_dependencies: 262, 265
machine_requirements: x86_64 silicon with VT-d and an NVMe drive
specific_machine: xenon (the Micron 2450 the first comparison was taken on)
needs_person: yes
---
# 833. The same storage benchmark on nife and Linux, by porting real fio

*(The title and slug are provisional.)*

It follows from fatal risk 6's GREEN verdict (`design/fatal-risks/6-a-confined-driver-is-too-slow.md`)
and milestone 261 (the NVMe driver leaves the kernel). calef, 2026-10-07 (UTC): *"Of course we should port fio. That makes perfect sense."*

**In brief.** Port real `fio` to nife and run the same binary and the same job file on both systems.
Risk 6 went GREEN on one pair of numbers: nife's confined driver at 4 KiB, queue depth 1, against a
Linux `fio` run photographed the same evening. The nife read was 2.16x Linux's, which nobody can
explain, and the figures were hand-transcribed. The port serves two risks at once: risk 6 (a
comparison nobody has to take on trust) and risk 1 (only software written for nife runs on nife),
because a real third-party program would then run on nife.

## What it builds

- One reference job file. `randread`, `randwrite`, `read` and `write`; 4 KiB and 128 KiB; queue
  depth 1, 4 and 32; `direct`; a fixed offset and size inside the window the confined driver owns.
- Linux side. `fio` with `io_uring`, polled and interrupt-driven. Also SPDK's `spdk_nvme_perf`,
  the like-for-like design: it is Linux's polled userspace NVMe driver, which is what nife's server is.
- nife side. The same `fio` source, built for nife and run against the confined NVMe server, reading
  the same job file and emitting the same JSON.
- A per-I/O latency histogram on each OS. This is the part that explains the read gap. A mean
  and a median cannot say whether nife's reads are fast or Linux's are slow in a tail.

## What the port needs from nife, to be discovered

These are questions the port answers, not assumptions. Each is a dependency found by trying.

- Threads. `fio` runs jobs as processes or threads and times them with its own clock threads.
  Whether nife's thread and timer surface covers what fio uses is not known.
- Which `ioengine` maps to the confined driver. `fio` has no engine for a block server reached over
  an endpoint. The candidates are `psync` over a file-like layer on `blk`, or a new engine written for
  nife's block contract. Which one measures the driver rather than the layer above it is the open question.
- Direct I/O and a block-device view. `direct=1` and an offset window have to mean the same on
  both sides, or the two numbers are not comparable.
- What std and libc fio's build wants that nife lacks, found by the first compile.

Answered in part on 2026-10-12 (UTC) by compiling fio 3.40 against the C library without linking
it: `notes/fio-on-nife.md` has the list. fio needs no C thread-locals, since its `configure` falls
back to `pthread_getspecific`, which costs nife one key lookup per timestamp that Linux does not pay.
On the benchmark's path it needs `sys/resource.h`, `libgen.h`, `select`, two header fixes and an
`os/os-nife.h`; the rest can stay stubs with `thread=1` and `--disable-shm`. The C library side
was built the same day by lane `lane/fio-c-library-pieces`: every piece on that list, with the off-path
calls refusing honestly rather than missing, checked by `fixtures/c/c_library_contract.c` on all three
architectures.

## What it must answer

Why the 2026-10-04 read was 2.16x Linux's (272 MB/s against 126 MB/s) when polled Linux reads were
no
faster than interrupt-driven ones. Either nife's number is wrong, which the histogram will show, or
the gap is real and has a cause worth knowing. Queue depths 4 and 32 are where the comparison stops
being one command in flight and "real speed" can be claimed beyond depth 1.

Reuse: `fio` itself is taken, ported rather than rewritten, and `spdk_nvme_perf` is taken as is for
the Linux side. No nife-side benchmark program is written, so there is no shim to keep.

## What it waits on

The C library is §265 (a C library started from relibc), and its first stage, milestone 835 (a C
library, stage 1: files, clock and memory), is built. This program's other needs are below.

Threads. fio times its jobs with its own threads even when jobs are processes. Threads are milestone
812 (`std::thread::spawn` runs real threads in one address space), option A of §105
(`std::thread::spawn` stays declined, until a customer needs it), reopened 2026-10-07 and not yet
scheduled. Until then, milestone 834 (ioping) answers risk 6's read gap at queue depth 1 without
threads.

## Done when

*§297 (software reaches a nife system as a package `jig` installs from basalt's index, the kernel
included) binds this. calef, 2026-10-12 (UTC): "They should all be installed via jig through
basalt."*

fio must run on nife installed by `jig install` from basalt's package index. basalt builds the
package against the nife commit it pins. The install waits on milestone 809 (the package client
becomes a program, `jig`, with the verbs an index needs). It also waits on milestone 801 (packages
over the internet: a host name, HTTPS and the distribution's index). The rest of the port does not
start until those milestones are built (calef, 2026-10-12 UTC: "For our other programs let's block
start on packaging. That was the point but if we are done with fio let's not throw out the work.").
The C library work already done for fio stays: the needs #2038 recorded and the pieces #2040 built
(`getrusage`, `basename`, `select`, `O_SYNC` and a `stdlib.h` fix).

## Index row

Real fio, one job file on nife and Linux, queue depths 1, 4 and 32 with per-I/O histograms, so risk
6's read gap is explained or corrected. Waits on a C library and threads.
