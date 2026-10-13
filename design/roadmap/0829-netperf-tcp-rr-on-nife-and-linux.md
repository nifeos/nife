---
status: NOT-STARTED
raised: 2026-10-08
promoted_from: netperf-tcp-rr-on-nife-and-linux
milestone_dependencies: 494, 53, 835, 809, 801
decision_dependencies: 262, 265
machine_requirements: silicon with a NIC nife drives and a peer on the same wire, xenon or radon
specific_machine: none
needs_person: yes
---
# 829. netperf TCP_RR on nife and Linux, request and response latency

*(The title and slug are provisional.)*

Model: milestone 833 (the same storage benchmark on nife and Linux, by porting real fio). Pairs with milestone
828 (iperf3 on nife and Linux); throughput hides per-packet cost and this exposes it.

**In brief.** Port `netperf` and `netserver` and run TCP_RR (and UDP_RR): a one-byte request, a
one-byte response, in a loop. The result is transactions per second, so its inverse is round-trip
latency.

## What it measures

One round trip through the whole path: client, stack, driver, wire, peer and back. With
a one-byte payload there is nothing to amortize, so the number is dominated by fixed cost per
crossing.

## Which fatal risks it informs

Risk 4 (the per-crossing cost): a request and response cross driver, stack and client boundaries
each way; the count is known from the design, so the measured time divides into a cost per
crossing that can be set beside `ipc_rtt_el0`. Risk 6: a confined driver that adds a fixed delay
shows up here at once.

## What nife must supply, as questions

- The same socket surface questions as iperf3. Does netserver's fork-per-connection model need
  `fork`, or can it run with `-D` and one process?
- `TCP_NODELAY`, `SO_RCVBUF` and `getsockopt`: which does the stack implement? Nagle behavior
  changes the answer by an order of magnitude, so it must match on both sides.
- Interrupt or poll in the driver? Record it; it moves the number more than any other setting.

## The Linux comparison

The same netperf version and parameters (`-t TCP_RR -l 30 -- -r 1,1`), same peer and cable,
Linux and nife in turn on the same board, with `netserver` on the peer and CPU affinity recorded.
Linux's interrupt coalescing is set to the same mode as nife's driver, or both modes are reported.

Reuse: netperf is taken and ported.

## What it waits on

The C library is §265 (a C library started from relibc), and its first stage, milestone 835 (a C
library, stage 1: files, clock and memory), is built. This program's other needs are below.

A NIC on silicon, milestones 494 (a driver for the network card a PC actually has) and 53 (the
board's own peripherals).

Fork. `netserver` forks a child per test by default. nife has no `fork` by design (§10 (process
model: capability-based, microkernel)), so a path that needs it never unblocks. The port runs the
program's no-fork mode where one exists, and a row that cannot run is recorded with its reason, as
`notes/benchmarks.md` already does for spawn. From memory, `netserver` has a flag that runs tests
serially in one process; the port checks the source before relying on it.

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

netperf must run on nife installed by `jig install` from basalt's package index. basalt builds the
package against the nife commit it pins. The install waits on milestone 809 (the package client
becomes a program, `jig`, with the verbs an index needs). It also waits on milestone 801 (packages
over the internet: a host name, HTTPS and the distribution's index). No work toward that starts
until those milestones are built (calef, 2026-10-12 UTC: "For our other programs let's block start
on packaging.").

## Index row

netperf TCP_RR round trips through driver, stack and client, a fixed cost per crossing with nothing
to amortize. Waits on a C library and a NIC on silicon; netserver's default fork never runs here.
