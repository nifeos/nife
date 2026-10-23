# Interrupts: the appendices to notes/interrupts.md

[`notes/interrupts.md`](../interrupts.md) carries the GIC, the timer, an interrupt as a message,
the two GIC versions with their BUGS, and the conclusions below. These files carry the evidence its
prose budget has no room for.

- [`riscv-test-trigger.md`](riscv-test-trigger.md): how the interrupt-to-message tests run on
  RISC-V, which has no SGI, and the two triggers refused.
- [`affinity.md`](affinity.md): spreading device lines off core 0 on the GIC and the PLIC, the
  PLIC's lost update step by step, and the `std_net` hang that was not an interrupt bug.
- [`gicv3-history.md`](gicv3-history.md): the 2026-09-02 measurement of GICv3 losing every
  interrupt, the driver priced in advance, and why aarch64 has no CPU-model matrix.
- [`polling.md`](polling.md): interrupts or polling, and why an MSI decides it.
- [`controller-drivers.md`](controller-drivers.md): the GICv2, GICv3 and PLIC drivers, register
  by register: banking, the redistributor walk, `RWP`, the PLIC's one lock and its write order.

*Name: provisional, minted 2026-10-11 (UTC) by lane/third-ten-longest-notes, which split the parent
under §212 (a prose budget). The directory follows §212's siting default, `notes/X/` beside
`notes/X.md`. calef names directories.*

The stems below were minted the same day by the same lane. Each is named for its content, and calef
names files.

- `riscv-test-trigger` Name: provisional, minted 2026-10-11 (UTC) by lane/third-ten-longest-notes.
- `affinity` Name: provisional, minted 2026-10-11 (UTC) by lane/third-ten-longest-notes.
- `gicv3-history` Name: provisional, minted 2026-10-11 (UTC) by lane/third-ten-longest-notes.
- `polling` Name: provisional, minted 2026-10-11 (UTC) by lane/third-ten-longest-notes.
- `controller-drivers` Name: provisional, minted 2026-10-23 (UTC) by lane comments/929-kernel-devices.
