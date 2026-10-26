---
status: BUILT
raised: 2026-09-19
built: 2026-10-03
promoted_from: an-x86-64-input-driver-that-never-lets-the-core-idle
milestone_dependencies: none
decision_dependencies: none
machine_requirements: none
specific_machine: none
needs_person: no
---
# 505. An x86_64 input driver that never lets the core idle

*(Number provisional until the merge queue lands it.)* Promoted from the
proposal `an-x86-64-input-driver-that-never-lets-the-core-idle`, filed 2026-09-19, on calef's
instruction of 2026-09-20 to give every proposal on `main` a number. The text below is the
proposal's own, unedited except for this paragraph: the argument is its author's and promotion is
not the moment to improve it. Found by milestone 182 (x86_64's own interactive-boot entry point)'s lane while building `script/swish-check`'s
x86_64 leg. Milestone 299 recorded x86_64's polling input driver as a latency and CPU limitation;
this is the measurement that says it is more than that.

As far as the lane could see, nothing gates it. The `Irq` capability, `irq_wait`/`irq_ack` and the
input driver's interrupt-driven arm all exist on aarch64 and riscv64. Milestone 299 (the x86
port-range capability)'s follow-on names what x86_64 lacks: delivering a device line (COM1's legacy IRQ 4, through the IO APIC) to a
userspace waiter, where today the kernel delivers only self-directed vectors to a driver. If that
turns out to need a new capability method or syscall, it becomes a design fork and stops there.

In brief. `components/src/input.rs`'s x86_64 `_start` is `loop { drain(); yield_now(); }`. A
thread that always yields is always runnable, so from the moment it starts the run queue is never
empty and `sched::run_idle` never runs again on that core. Two things live in the idle loop and both
stop:

- The halt. QEMU sat at 99 to 100% of a host core at an x86_64 prompt with nothing typed (76
  CPU-seconds in 78 wall-seconds, patagonia, 2026-09-19, one core under OVMF). AGENTS.md's `wfi`
  rule exists because a halted kernel spinning cost 99.7% of a core; this is the same cost at the
  prompt, and on a PC it is a fan and a battery.
- The capability-slot gauge (`kernel::cap::report_peak`, milestone 231 (nothing counts how many capability slots a boot actually uses, so the wall is always a surprise)). It prints the mark at
  the hand-over, 5 of 24, and never updates; the peak during `swish-check`'s script is 17, read by a
  temporary instrument. The gauge's `ABOVE` check therefore cannot fire on x86_64.

## What to build

Route IRQ 4 through the IO APIC to the `Irq` capability the progenitor already grants at slot 2 and
already has the slot layout for (`kernel::user::boot_progenitor`). Arm IER's receive bit in the x86
`uart` arm (the register layout is already written there). Give x86_64 the same `_start` the
other two run: drain, arm, then `irq_wait`/drain/`irq_ack`. Then delete the x86 `_start` twin.

## How to know it worked

- `script/swish-check --arch x86_64` prints a slot gauge equal to the peak (17 of 24 today, or
  whatever it measures), and the leg's "that gauge is stale" caveat in `xtask/src/main.rs` is deleted.
- QEMU's host CPU at an idle x86_64 prompt drops to near zero, measured the way the number above was.
- `script/swish-check`'s BUGS entry and milestone 182's two BUGS entries on this are closed.

## What was built

Built 2026-10-03 (UTC) by the 505 lane, on calef's approval of the merge-rate correction's action
items. Within the existing capability model: no new method, no new syscall, no ABI change.

1. The IO APIC's device vector becomes a driver's message. `kernel/src/arch/x86_64/exceptions.rs`
   asks `irq::intid_of_vector` which intid armed the line, then routes it through
   `sched::irq_route` as the MSI and self-IPI arms already did. `intid_of_vector` reads a table
   `irq::enable` fills, because a vector gives a GSI and a GSI is not a legacy IRQ.
2. `irq::disable` masks a level-triggered line and leaves an edge-triggered one live. An IO
   APIC drops an edge that arrives on a masked entry, and COM1's IRQ 4 is edge-triggered, so
   masking it would lose a byte that lands between the driver's drain and its ACK.
3. `boot_progenitor` arms IRQ 4 on x86_64, as aarch64 already did, and the progenitor grants
   the input driver its `Irq` capability in slot 1 with the port range moved to slot 2.
   `memory::record_uart_irq` now records the legacy number rather than the GSI, since `enable` is
   what resolves the override.
4. One `_start` on all three architectures. The x86 `uart` arm sets IER's receive bit and
   MCR's `OUT2`, the gate a real PC puts between the 16550 and the ISA line (QEMU does not model
   it). The polled twin is deleted.
5. The workarounds for the starved idle loop are deleted: `progenitor_stack::on_yield` and its
   yield-syscall hook, `swish-check`'s "that gauge is the mark at the hand-over" caveat and header
   paragraph. Milestone 182's two `BUGS` entries are marked answered.

## How it was proved

The x86_64 `swish-check` leg is the test that fails without the kernel half. With the polled twin
gone, no keystroke reaches the shell unless IRQ 4 reaches the driver. With `on_yield` gone, the
progenitor-stack gauge prints only if the idle loop runs at a prompt, and the leg fails when it
never prints. The capability-slot gauge's `ABOVE` check is live on x86_64 for the first time.

Measured on CI, the x86_64 leg under KVM (run 37146065011 against five main merge-group runs,
37141733965, 37141058488, 37139101598, 37137625820 and 37136859644):

| | main | this branch |
|---|---|---|
| capability-slot gauge | 5 of 32 (the hand-over) | 24 of 32 (the peak; aarch64 and riscv64 print 24) |
| first boot, 142 lines | 26.8 to 36.8 s, median 30.2 s | 36.5 s |
| second boot, 8 lines | 0.8 to 0.9 s | 0.8 s |

The gauge is the result: it is printed from the idle loop, so it moving to the peak is the idle loop
running at a prompt. The progenitor-stack gauge printed with its yield trigger gone, which says the
same thing. Line time is within main's own spread, so interrupt delivery costs nothing a person
would see. The 76-in-78 CPU-seconds figure at an idle prompt was not re-measured: that needs a local
x86_64 shell image, and the two gauges are the direct evidence that the core now idles.

## BUGS

- Revoking the input driver's `Irq` does not mask IRQ 4. `arch/x86_64/interrupt_controller.rs`'s
  BUGS has it.
- COM1's line goes to whichever core last ACKed, because `irq::enable` writes the calling
  core's local APIC id. Harmless (any core's handler routes it), and not distribution policy.

## Follow-on

- **Recorded.** Both BUGS above, in this block and in
  `kernel/src/arch/x86_64/interrupt_controller.rs`.

## Index row

`components/src/input.rs`'s x86_64 `_start` is `loop { drain(); yield_now(); }`.
