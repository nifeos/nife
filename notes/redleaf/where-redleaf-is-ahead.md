# Where RedLeaf is ahead, without hedging

*Name: provisional. After the parent's own heading.*

An appendix to [`notes/redleaf.md`](../redleaf.md). It holds the four results RedLeaf has that this
tree does not, with the figures quoted from the OSDI '20 paper, and one design observation about its
IDL.

Four places, and the first two are not close.

1. Per-crossing cost. 124 cycles against seL4's 834 on the same machine. Nothing in a hardware-
   isolated design reaches that, and the Atmosphere numbers in [`crossing-costs.md`](crossing-costs.md) are that statement made by the
   people with the most incentive to find otherwise.
2. Drivers measured against the fastest thing available, on real hardware. Their ixgbe driver
   against DPDK: *"On a batch of one, DPDK achieves 6.7 Mpps and is 7% faster than RedLeaf (6.5
   Mpps)"*, and at batch 32 *"both drivers achieve the line-rate performance of a 10GbE interface
   (14.2 Mpps)."* Their NVMe driver against SPDK: *"the RedLeaf driver is 1% faster (457K IOPS
   per-core) than SPDK (452K IOPS per-core)."* This tree has no driver benchmarked against a
   user-space framework at all, on any architecture.
3. A recovery number taken under a crash loop. *"we trigger a crash of the block device driver
   every second ... For reads, the throughput with and without restarts averages at 2062 MB/s and
   2164 MB/s respectively (a 5% drop in performance). For writes, the total throughput averages at
   356 MB/s with restarts and 423 MB/s without restarts (a 16% drop)."* nife has the mechanism:
   DECISIONS §26 (the fault endpoint: thread death becomes a message) and DECISIONS §41 (the
   endpoint is the broker). `notes/live-replacement.md` records that live replacement costs zero in
   steady state. But nobody here has published what a component dying repeatedly costs a workload.
4. Application-level numbers against commodity baselines. Maglev, a network key-value store, and
   an httpd measured against Linux sockets, DPDK and nginx: *"On Linux, Nginx can serve 70.9 K
   requests per second, whereas our implementation of httpd achieves 212 K requests per second."*
   Whatever the caveats, that is a workload a stranger recognizes, and milestone 168 (a multi-tasking
   workload benchmark) was still PARTIAL on 2026-09-20. *(Corrected 2026-10-11 UTC: it was built
   on 2026-10-04.)*

A fifth is a design observation rather than a result. RedLeaf's IDL generates the proxy, the entry
point, the create trait and the microkernel-side create function from one interface definition. It
enforces the exchangeability invariant as a static analysis pass over the resulting type graph.
That is rung one of AGENTS.md's ladder (make the wrong state unrepresentable) applied to an IPC
surface, and it is further up that ladder than anything in this tree's component contracts. Milestone
23 (a capability-routed component OS with live replacement) is where it would be relevant.
