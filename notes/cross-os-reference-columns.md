# The reference operating-system columns for the cross-OS benchmark suite

*Name: provisional, minted 2026-10-30 (UTC) by bench/cross-os-reference-columns. calef names notes.*

## What this page is

The cross-OS suite sets nife's numbers beside the same program's numbers from other operating
systems, on the same machine, with the same versions and flags. When nife's ports land (milestones
826, 827, 828, 829, 831, 832, 833 and 834), each row holds its program's method. This page
holds what no row holds: the reference operating systems, the record list every row carries, the
machine assignment, and the controls-first rule. It links the rows rather than restating their
method sections, so the copies cannot drift. nife's column lives in [benchmarks.md](benchmarks.md); [bench-runbook.md](bench-runbook.md) holds
how an evening runs.

## The suite

| program | what it isolates | nife row | runs on |
|---|---|---|---|
| `coremark` | pure compute; the control that must tie | the EL0 row, [benchmarks.md](benchmarks.md) | every unix, both boards |
| STREAM, Copy, Scale, Add, Triad | memory bandwidth, zero syscalls; the second control | [832](../design/roadmap/0832-stream-on-nife-and-linux.md) | every unix, both boards |
| lmbench: `lat_syscall null`, `lat_ctx`, `lat_pipe`, `lat_proc exec`, `bw_mem`, `lat_mem_rd` | [risk 4](../design/fatal-risks/4-the-per-crossing-cost.md)'s per-crossing cost: trap, switch, pipe, fault, cache latency | [826](../design/roadmap/0826-an-lmbench-subset-on-nife-and-linux.md) | every unix |
| lmbench: `lat_proc fork` | what process creation costs a Unix | recorded-not-run, [decision 264](../design/decisions/0264-fork-is-declined-and-spawn-is-supported.md) | every unix; the recorded absence beside the spawn caveat ([benchmarks.md](benchmarks.md)) is the comparison |
| `hackbench` | scheduler and socket pressure under load | [827](../design/roadmap/0827-hackbench-on-nife-and-linux.md) | every unix; source where not packaged |
| sqlite `speedtest1` | a real workload, stranger-credible | [831](../design/roadmap/0831-sqlite-speedtest1-on-nife-and-linux.md) | source or package everywhere |
| `fio`: one job file, `randread`/`randwrite`/`read`/`write`, 4 KiB and 128 KiB, queue depths 1, 4, 32, `direct`, JSON, per-I/O histograms | [risk 6](../design/fatal-risks/6-a-confined-driver-is-too-slow.md), the confined-driver verdict | [833](../design/roadmap/0833-the-same-storage-benchmark-on-nife-and-linux.md) | xenon only; Linux also runs SPDK's `spdk_nvme_perf` |
| `ioping` | queue-depth-1 latency | [834](../design/roadmap/0834-ioping-on-nife-and-linux.md) | everywhere |
| `iperf3` | NIC and stack goodput | [828](../design/roadmap/0828-iperf3-on-nife-and-linux.md) | needs a wire |
| `netperf`, TCP_RR and UDP_RR | fixed cost per round trip: driver, stack, client | [829](../design/roadmap/0829-netperf-tcp-rr-on-nife-and-linux.md) | needs a wire |
| `sel4bench` | the seL4 column | [25](../design/roadmap/0025-cross-os-comparison.md) | nife-side; waits on a silicon PMU, not a unix row |

## Controls first

`coremark` and STREAM make no syscalls, so on the same silicon they must tie across operating
systems. Take them first on every fresh install. A disagreement indicts the rig, not the operating
system: the governor, the turbo state, the memory population or the mitigations moved between
installs. A tie on both controls makes the later rows evidence about the operating system.

## What every row records

- The kernel version, plus config deltas from the distribution default.
- The governor, `performance`, and the clock the OS itself reports; the turbo state; the memory
  population.
- The tools' exact versions.
- On x86_64, the mitigations state: defaults by default, plus an optional labeled `mitigations=off`
  row for attribution; mitigations move `null_syscall` and `lat_ctx` most.
- Network rows: link speed first, so a 100 Mbit fallback does not read as slowness; then offloads
  and coalescing, parity or both modes reported
  ([milestone 829 (netperf TCP_RR)](../design/roadmap/0829-netperf-tcp-rr-on-nife-and-linux.md)).
- fio rows: JSON output and per-I/O histograms on every side.
- lmbench rows: `-P 1 -W 1 -N 5`, one core pinned.
- netperf rows: which `netserver` connection mode ran, so nife's no-fork row (decision 264) later
  has a like-mode unix row.

A reference row reports every pass it ran, never its best. fio, iperf3 and netperf take three
passes on the unix side; the row records all three and the spread. nife's rows keep their own
milestones' protocols, and every row names its protocol, so the two never mix silently.
[Risk 6](../design/fatal-risks/6-a-confined-driver-is-too-slow.md) holds the reason: its Linux read
figure varied 16% across boots on one pass per boot, transcribed by hand. The unix rows measure
that variance on purpose.

## The reference operating systems

As ruled 2026-10-30 (calef); statuses come from the named external pages, read 2026-10-30, and no
install on this bench has proven any yet.

- Debian 13 (trixie), amd64 and riscv64: the standard. This release supports riscv64 officially
  for the first time, debian-installer ships riscv64 netboot images, and the kernel is 6.12 LTS
  ([the riscv64 installation guide](https://www.debian.org/releases/stable/riscv64/)).
- Ubuntu 24.04 LTS riscv64: the fallback for a VisionFive 2 image when Debian's is rough; the row
  records the kernel version regardless.
- Fedora 44 on xenon: the fio baseline already taken
  ([linux-fio.log](../bench/xenon-2026-10-04/linux-fio.log), transcribed by hand from photographs);
  one Debian run of the same job file settles whether the distribution moves the number and
  upgrades the provenance to serial-captured logs.
- FreeBSD 15.1-RELEASE, amd64 first-class. The [RuyiSDK support
  matrix](https://matrix.ruyisdk.org/reports/) records BASIC status for FreeBSD on the VisionFive 2
  (entry dated 2026-05-02, a 15.1-BETA1 GENERICSD image, serial login); the first install takes
  15.1-RELEASE.
- OpenBSD 8.0, amd64 first-class. On the VisionFive 2,
  [INSTALL.riscv64](https://cdn.openbsd.org/pub/OpenBSD/8.0/riscv64/INSTALL.riscv64) names only the
  earlier VisionFive among the partially supported, and the [platform
  page](https://www.openbsd.org/riscv64.html) lists the VisionFive 2 with a firmware-tweaks caveat.
  The community installs on record are 7.5-era: a device tree on the EFI partition, manual U-Boot
  `bootefi` commands; plan the first install as exploratory.
- NetBSD, declined for now: it runs the Star64 (the same JH7110 SoC), and the one VisionFive 2
  report on record stops at a serial login, no working ethernet.

## The driver lineage

nife's GMAC driver adapts OpenBSD's `dwqe` ([0053](../design/roadmap/0053-board-peripherals.md),
[0940](../design/roadmap/0940-second-jh7110-ethernet-port.md)), and the e1000e crate ports
FreeBSD's `em` ([0815](../design/roadmap/0815-the-e1000e-crate-released-on-its-own.md)). OpenBSD on
radon and FreeBSD on xenon therefore run those reference implementations, on the same silicon: the
strongest form of the confined-driver comparison.

## Machine assignment

The bench reimages both machines per experiment, so no install coexists with another. calef,
2026-10-30 (UTC): "We spin up and tear down xenon constantly. It is a lab machine."

xenon evenings need no wire: Debian 13 amd64 and one or both BSDs, taking every non-network row
of the table above; the Debian side adds SPDK's `spdk_nvme_perf` beside fio. Network rows wait on a
wired port.

radon evenings take Debian 13 riscv64 and one BSD card: the same CPU and memory rows, plus iperf3
and netperf against a peer on the same wire, cable in `gmac0`, the port nife's GMAC driver drives.
radon cannot reboot itself; every OS switch is a manual power cycle, so one Linux plus one BSD
bounds the cost.

fio on radon would measure an SD card, not risk 6's instrument: [milestone 833 (the same storage
benchmark)](../design/roadmap/0833-the-same-storage-benchmark-on-nife-and-linux.md)'s
machine_requirements name x86_64 silicon with VT-d and an NVMe, xenon the specific machine.

## Scope notes

[Milestone 830 (wakeup latency)](../design/roadmap/0830-wakeup-latency-on-nife-and-linux.md) reads
each kernel's own scheduler trace; a BSD-side trace reader is a design question, so 830 stays
nife-plus-Linux for now. This page records that scope because a lane edits only its own roadmap
block.

## EXAMPLES

```sh
# iperf3, milestone 828
iperf3 -c <peer> -t 30 -P 1
iperf3 -c <peer> -t 30 -P 1 -R
iperf3 -c <peer> -t 30 -P 4
iperf3 -c <peer> -t 30 -u -b     # UDP; -R reverses

# netperf, milestone 829
netperf -H <peer> -t TCP_RR -l 30 -- -r 1,1
netperf -H <peer> -t UDP_RR -l 30 -- -r 1,1

# lmbench, milestone 826
lat_syscall -P 1 -W 1 -N 5 null
```

Every lmbench program takes the same three flags. The fio job file is
[833](../design/roadmap/0833-the-same-storage-benchmark-on-nife-and-linux.md)'s deliverable, which
nothing here pins.

## BUGS

- The statuses above come from external pages, read 2026-10-30; no install on this bench has
  proven any.
- The OpenBSD VisionFive 2 column may need a device tree on the EFI partition and U-Boot work
  before it boots.
- Debian's own riscv64 installation guide warns that it is not fully fact-checked for riscv64.
- This page is a plan until the first install happens; the first bench evening rewrites the
  machine assignment if the installs say otherwise.
