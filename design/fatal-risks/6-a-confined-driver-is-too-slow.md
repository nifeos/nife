---
experiment_status: RUN
experiment_run: 2026-10-04
---
# 6. A capability-confined userspace driver cannot drive real hardware at real speed

*Risk 6 of [the nine](README.md). The status vocabulary, the rule an entry meets and the running order are there.*

The claim: the thing that makes the thesis interesting, drivers outside the kernel behind an IOMMU,
does not survive contact with a real device.

AMBER (calef, 2026-10-08: "Risk 6 back to amber."); GREEN (calef, 2026-10-07); amber 2026-10-04. The first silicon evidence was the
TRNG, 2026-09-16: its three parts were measured on silicon, and they were never one claim. On radon, milestone 159 (a real hardware entropy source: the JH7110's TRNG)'s
driver is an EL0 process reaching the TRNG through a capability that names no device. Confined,
2026-09-03. Driving real hardware, 2026-09-04, reproducibly. At real speed, MEASURED 2026-09-16 at
about 8.4 us per round trip. The committed boots read 973,384 to 992,248 bytes/s (four boots; the
first figure, 955,223 bytes/s, has no committed transcript; #1495).

**The decisive experiment: RUN, 2026-10-04, on xenon.** An EL0 process holding one page of the Micron 2450's BAR0 and a DMA window confined by VT-d drove the real NVMe in three boots, all `CONFINED-AT-RATE`: both preflights PASS (the catch-all unit `0xfed91000` owns 01:00.0; 512-byte LBAs), and 16384 of 16384 blocks verified each time. Medians: write 458142471 B/s (range 457744197 to 474990381), read 271854622 B/s (237098519 to 281608311, a 16% spread), and an IPC floor of 1197 ns per round trip, which is 13% of a write block and 8% of a read block. Getting there found and fixed a kernel defect: xenon's VT-d units do not snoop the CPU caches (`ECAP.C` = 0), and the kernel never wrote its tables back, which QEMU cannot show (`bench/xenon-2026-10-04/`, milestone 261 (the NVMe driver leaves the kernel)).

**Verdict: AMBER (calef, 2026-10-08: "Risk 6 back to amber."); GREEN (calef, 2026-10-07); amber 2026-10-04.** Real hardware (a real NVMe with DMA), driven correctly (16384 of 16384 verified, three of three boots), from EL0 behind a translating VT-d unit. The two conditions written on 2026-10-04 were met on 2026-10-07, and the verdict went back to amber a day later because the read figure they rest on is not trusted yet (see the caveat and the path below):

1. **Real speed has a reference.** Linux `fio` on the same disk and window, 4 KiB, queue depth 1 (`bench/xenon-2026-10-04/linux-fio.log`, hand-transcribed from photographs). Writes: nife 458 MB/s against Linux's best, 425 MB/s (`io_uring`, polled), 1.08x. Reads: nife 272 MB/s against Linux's 126 MB/s, 2.16x. The condition was within about 0.8x of Linux for both.
2. **Confined is attacked.** The out-of-region DMA test, `a_confined_el0_server_cannot_dma_outside_its_region`, was done 2026-10-04 (milestone 261 (the NVMe driver leaves the kernel)) and its falsification replayed red 2026-10-05.

**The caveat, plainly.** A read 2.2x faster than Linux on the same disk is suspicious and unexplained. Polling does not explain it, since polled Linux reads were no faster. But nife's read would have to be overstated by more than 2.7x to fall below 0.8x. The rest: one pass per boot, figures hand-transcribed from photographs, read varied 16% across boots, and "real speed" is claimed at queue depth 1 only. The follow-up is milestone 833 (the same storage benchmark on nife and Linux, by porting real fio): port real fio and run one job file on both systems, queue depths 1, 4 and 32, with a per-I/O latency histogram on each.

**The path back to GREEN (proposed 2026-10-08; item 1's benchmark ruled 2026-10-18).**

1. The same unmodified storage benchmark, run on nife and on Linux on xenon, with real logs (not photographs), repeated passes and per-request latency, so the 2.16x read is either explained or corrected. The benchmark is `ioping`: asked "Should `ioping`, run unmodified on nife and on Linux on xenon with real logs and repeated passes, be the benchmark that explains or corrects risk 6's 2.16x read?", calef answered "Yes." (2026-10-18 UTC).
2. `fio` at queue depths 1, 4 and 32, one job file on both systems, per milestone 833 (the same storage benchmark on nife and Linux, by porting real fio). It waits on packaging, not threads: milestones 812 and 836 are built, and ports wait on 809 and 801 (calef, 2026-10-12 UTC).
3. The same `ioping` and `fio` runs on a second board of another architecture, so the result is not one device on one board (xenon). radon's M.2 slot is a candidate for that board and may need a drive.

GREEN needs items 1 and 2 to hold on that second board as well as on xenon (ruled 2026-10-27 UTC).

Caveats. Every figure is one command in flight, polled completion, and one pass per boot with no warm-up, so it is a lower bound on the device and not comparable to `fio` at queue depth 32. A Linux `fio` run on the same disk at queue depth 1 has been made (`bench/xenon-2026-10-04/linux-fio.log`, hand-transcribed from photographs): Fedora 44 live USB, fio-3.40, the Micron 2450 NVMe 256GB (`nvme0n1`), 4 KiB blocks, queue depth 1, `--direct=1`, offset 1 MiB, 64 MiB, one pass, with the same window nife used. The main run (psync) read 30.7k IOPS at 126 MB/s and wrote 77.6k IOPS at 318 MB/s; two further runs with polling requested (pvsync2 did not engage polling; io_uring did) read 126 MB/s and wrote 272 MB/s and 425 MB/s. Correction, 2026-10-08 (§216 (fatal-risk facts are correctable, and verdicts are the architect's)): the sentence that stood here, saying the comparison against the 0.8x condition was owed to calef and that no verdict changed, was stale. The comparison was made and ruled on 2026-10-07: writes 1.08x and reads 2.16x of Linux's best, both inside the condition, and the verdict went GREEN. On 2026-10-08 calef moved it back to AMBER, because the read gap is unexplained and the Linux figures are hand-transcribed. "Real speed" is claimed at queue depth 1 on this one disk, and stays provisional until the path above is walked. Read varied 16% across boots. [Appendix](the-confined-driver.md).

2026-10-08 (UTC): `fio` needs threads, which §263 (threads are built) schedules as milestone 812
(`std::thread::spawn` runs real threads in one address space).

2026-10-10 (UTC): milestone 812 is built, so Rust programs have threads. `fio` is C, and its
threads wait on a C library's pthreads (milestone 836 (a C library, stage 2: threads)), so the
queue-depth comparison above is no closer and the risk's status is unchanged.

2026-10-12 (UTC): milestone 836 is built (#2009), so a C program has pthreads, and hackbench's
thread mode runs unmodified on all three architectures. `fio` still does not build, and a
compile of fio 3.40 for aarch64 against nife's C library (#2038) shows the cause is not C's `__thread`.
fio's `configure` falls back to `pthread_getspecific` when `__thread` does not link, and 836 built
that. What fio lacks is in the C library: `sys/resource.h` with `getrusage`, `libgen.h`, `select`,
`O_SYNC`, two header bugs, and its own `os/os-nife.h`, listed in milestone 833 (the same storage
benchmark on nife and Linux, by porting real fio)'s block. The comparison is no closer, and the
status is unchanged.

## Benchmarks that inform this risk

As of 2026-10-08 (UTC), under §262 (nife is measured with the field's standard benchmarks).
Each is to run unmodified on nife and on Linux, and none has produced a number yet. A port does
not wait on a C library, since milestone 835 (#1896) built one from relibc under §265 (a C library
started from relibc). It waits on whatever that first stage lacks, such as threads or sockets.

- [Milestone 828 (iperf3 on nife and Linux)](../roadmap/0828-iperf3-on-nife-and-linux.md), the NIC half.
- [Milestone 829 (netperf TCP_RR on nife and Linux)](../roadmap/0829-netperf-tcp-rr-on-nife-and-linux.md), a driver's fixed delay.
- [Milestone 833 (the same storage benchmark on nife and Linux, by porting real fio)](../roadmap/0833-the-same-storage-benchmark-on-nife-and-linux.md), at queue depths 1, 4 and 32. Waits on packaging (809 and 801); threads are built.
- [Milestone 834 (ioping on nife and Linux)](../roadmap/0834-ioping-on-nife-and-linux.md), one request at a time. Runs on nife since 835; the Linux row is open, and the run that counts waits on its package, milestone 906 (ioping is packaged in basalt).
