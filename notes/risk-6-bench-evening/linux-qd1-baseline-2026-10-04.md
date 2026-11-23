# Linux's queue-depth-1 baseline on xenon, 2026-10-04

*Step 5 of [fatal risk 6's bench evening](../risk-6-bench-evening.md), run. This page's name is
recorded in the [directory's README](README.md).*

Run by calef on xenon on 2026-10-04, evening (bench clock 23:12 to 23:45, zone not recorded), from
a Fedora 44 Workstation Live USB with fio-3.40. The disk (Micron 2450, 256060514304 bytes) and the
window match nife's. Every run used `--offset=1M --size=64M --bs=4k --iodepth=1 --direct=1`, one
pass each. Photographs, scripts and logs are in `bench/xenon-2026-10-04/` (in nifeos/nife).

| Run | Engine | Order | Polling | Read | Write |
|---|---|---|---|---|---|
| A (23:12) | `psync`, interrupts | read, write | no | 126 MB/s, 30.7k IOPS | 318 MB/s, 77.6k IOPS |
| B (23:20) | `pvsync2 --hipri` | read, write | did not engage (one switch per I/O) | 126 MB/s, 30.5k IOPS | 272 MB/s, 66.3k IOPS |
| C (23:25) | `io_uring --hipri` | write, read | engaged (ctx=12 and 4; sys 84% and 95%) | 126 MB/s, 30.7k IOPS | 425 MB/s, 104k IOPS |

Percentiles and CPU split are in `linux-fio.log`. Runs B and C reloaded the nvme module with
`poll_queues=1`, and `io_poll` read 1.

nife's three-boot medians from the same night and window (`CONFINED-AT-RATE`): write 458142471
B/s, read 271854622 B/s, IPC floor 1197 ns. The logs are `bench/xenon-2026-10-04/boot-e-main-clflush-1.log`
through `boot-g-main-clflush-3.log`, landing with pull request 1636.

| | nife median | Best Linux qd1 | nife / Linux |
|---|---|---|---|
| Write | 458142471 B/s | 425 MB/s (run C, io_uring polled) | about 1.08x |
| Read | 271854622 B/s | 188 MB/s (aspm check, read A and B) | about 1.45x |

What the runs settle and what they leave open:

1. The write gap is mostly polling. Polled io_uring, write first (nife's order), reaches 425
   MB/s, within 8% of nife. Interrupt-driven psync reached 318 MB/s, which was 1.44x.
2. Run B is not a polled run, and why `--hipri` did not engage is unknown. Cite run C as "Linux polled".
3. The ASPM hypothesis is refuted (`aspm-233408.log`, `apst-233725.log`, run at 23:34 and 23:45 by
   `aspm.sh` and `apst.sh`; they never write the disk, but they did touch link and controller
   state). `LnkCtl` read "ASPM Disabled" on the NVMe (01:00.0) and its root port (00:1b.0),
   whose `LnkCap` says "ASPM not supported". The link runs 8GT/s x4, below the drive's 16GT/s,
   because the root port is Gen3.
4. APST is not the cause either. The module reload failed ("Module nvme_core is in use"), so
   aspm's "read B" ran with APST on, the same condition as read A. APST was then turned
   off at runtime (`nvme set-feature -f 0x0c -v 0`, confirmed). As found it was
   enabled, power state 4. Reads C (as found) and D (off) are identical at 178 MB/s.
5. Linux's read rate moves with drive state, and the spread exceeds the nife gap. 126 MB/s in
   runs A to C, 188 MB/s in aspm reads A and B, 178 MB/s in apst reads C and D, and 74 MB/s in read
   E (an immediate repeat, steady at 54 us per block). The cause is unknown; thermal throttling is
   a guess. The next check is `nvme smart-log` temperature.
6. Typical read latency matches. Median clat is 16 us in every polled run except E; nife's is
   about 15 us per block. Linux's lower throughput is a tail (about 10% of reads at 22 to 66 us)
   plus the state variation, neither explained.
7. Linux ran untranslated: `/sys/class/iommu` was empty, while nife ran under VT-d.
8. Counter scale is unchecked against wall time on xenon. Linux figures are single passes.

The supportable claim: confined nife at least matches Linux on this drive at queue depth 1 (write
1.08x the best Linux, equal read median latency). Do not claim nife reads twice as fast.
