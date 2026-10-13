---
status: NOT-STARTED
raised: 2026-09-25
promoted_from: dma-on-a-non-coherent-risc-v-machine
milestone_dependencies: 89
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 655. DMA on a non-coherent RISC-V machine

*Title and slug are drafts.*
Raised by the lane that measured milestone 89 (Scaleway EM-RV1: a
second RISC-V implementation, rented)'s distance. Name provisional, this file's slug only.

Nothing can exercise it until nife boots on a TH1520, and no DMA driver is
needed for that machine's first light.

## What is true today

Every riscv64 machine nife has run on keeps DMA coherent with the caches. The DMA write barrier in
`kernel/src/arch/riscv64/mod.rs` is a plain `fence`, and so is `virtio_ring_barrier` in
`crates/user_mode_runtime/src/virtio.rs`. Nothing on riscv64 cleans or invalidates a data cache.

The TH1520 is the first machine where that is wrong. Its `/soc` node states `dma-noncoherent`
(Linux `th1520.dtsi` at `165768bb7`), and its C910 cores lack Zicbom. Linux uses T-Head's own
instructions instead (`arch/riscv/errata/thead/errata.c`). It works one 64-byte line at a time and
ends with `th.sync.s`:

| Operation | Instruction | Encoding with `a0` |
|---|---|---|
| clean, before a device reads | `th.dcache.cpa` | `0x0295000b` |
| invalidate, after a device writes | `th.dcache.ipa` | `0x02a5000b` |
| both | `th.dcache.cipa` | `0x02b5000b` |
| order them | `th.sync.s` | `0x0190000b` |

## What the work is

One seam for cache maintenance on DMA buffers, with two implementations behind it: Zicbom's
`cbo.clean` and `cbo.inval` where the tree states `zicbom`, and T-Head's where the vendor probe from
milestone 89's step 5 says C910. A coherent machine keeps the plain `fence` it has now. The drivers
that would call it are whichever reach the TH1520 first: its eMMC (`thead,th1520-dwcmshc`) or its
Ethernet (`snps,dwmac-3.70a`). Neither has a driver in this tree.

The instructions are privileged in their virtual-address forms and legal in U-mode only when
firmware sets `mxstatus.UCME`. So whether a capability-confined userspace driver can clean its own
buffers, or must ask the kernel, is a question for fatal risk 6 (a capability-confined userspace
driver cannot drive real hardware at real speed). The answer is measured on the machine, not assumed.

## Index row

Every riscv64 machine nife has run on keeps DMA coherent with the caches, and the TH1520 does not. Proposed: the cache maintenance that machine needs, which nothing can exercise until nife boots on a TH1520.
