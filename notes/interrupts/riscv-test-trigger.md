# Testing an interrupt as a message on RISC-V, which has no SGI

*Name: provisional, minted 2026-10-11 (UTC) by lane/third-ten-longest-notes.*

An appendix to [notes/interrupts.md](../interrupts.md), moved there on 2026-10-11 (UTC) under §212
(a prose budget). It records how the work of 2026-07-31 made the two interrupt-to-message tests
run on RISC-V.

The two tests above (`kernel::sched::tests::an_interrupt_becomes_a_message` and
`an_interrupt_that_arrives_before_the_wait_is_not_lost`) were aarch64-only for a year, gated because
they trigger with an SGI. The properties are not architectural, though: one is IRQ-to-IPC
delivery and the other is a lost-wakeup race, and RISC-V has interrupts and the same IPC. Only the
trigger was in the way. They are portable now, with the trigger behind three functions in the test
module (`arm_test_irq`, `raise_test_irq`, `quiet_test_irq`).

What RISC-V raises, and why that one. The console UART's own transmit-empty interrupt. A 16550
asserts its line the moment `IER.ETBEI` is set while `LSR.THRE` is set, and the transmitter of a
polling console is always empty, so one register write raises the line into the PLIC and one lowers
it. No transfer, no external stimulus, nothing to read back. `console::raise_uart_interrupt` /
`quiet_uart_interrupt`, test builds only.

Two other options were considered and are worse, for reasons worth keeping:

- The SBI's IPI (`sbi_send_ipi`, which `arch::irq::send_reschedule` already uses). It is the
  obvious "software-generated interrupt" on RISC-V and it is the wrong one. It arrives as a
  *supervisor software* interrupt, `scause` = 1, which is a different arm of
  `riscv_trap_dispatch` from a device's `scause` = 9: that arm drains the scheduler inbox and serves
  steal requests, and touches neither `irq_route` nor `irq_notify`. A test built on it would have
  looked like parity with aarch64 and proved nothing about IRQ-to-message delivery.
- Writing the PLIC's pending bits (base + 0x1000). Read-only by specification, and QEMU 11.0.2
  agrees: a probe that set source 20's pending bit and read the word back got `0x0` before and
  `0x0` after. Even if it had worked it would have been a QEMU behavior to lean a gate on, three
  weeks before the VisionFive 2 arrives.

**So the two legs are not twins, and the note says so rather than the doc comment claiming it.**
aarch64's SGI needs no device at all; RISC-V's needs the UART to exist. In the other direction
RISC-V covers *more* of the controller: an external interrupt goes through the PLIC's
claim / mask / notify / complete handshake, which an aarch64 SGI does not reach. The kernel path
under test, the part these tests exist for, is the same on both.

A claim that was in the tree and was not backed. The old doc comment on
`an_interrupt_becomes_a_message` said the same path was "proven on RISC-V by the boot tour's
userspace UART driver". The boot tour is `script/console`, interactive, and gates nothing, so as
written the claim cited a witness the suite does not run. The substance was true by then for a
different reason (the parity-C virtio tests), but the citation was to the wrong thing. A
parity claim resting on a demo nobody runs is the shape of gap §19 exists to catch.
