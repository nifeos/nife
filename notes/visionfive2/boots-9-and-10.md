# Boots 9 and 10: the fifth bench stop

*Name: ratified 2026-10-18 (calef, "Approve all").*

An appendix to [notes/visionfive2.md](../visionfive2.md). On 2026-08-15 boots 9 and 10 showed that
the state [boots-7-and-8.md](boots-7-and-8.md) read as corruption was the terminal state of a
finished tour.

## The fourth stop's conviction falls

The dumps were showing a finished tour, and every "fabricated" value is the fingerprint of health.

Boot 10 (`booti ${kernel_addr_r} - <dtb>`, no initrd) ran the whole tour on silicon. It went
through preemption on three harts to the final banner. The base kernel is good, and the failure is
initrd-path-coupled.

Boot 9 had the initrd and the undelivered-wake gate live. It reproduced the "hang" with the gate
silent. There were zero `refuse:` events. The boot thread's wake carried `ipc_served`, and no
message print followed. The rows were the same as boots 7 and 8. One user thread was `Blocked` as a
Receiver on ep `0x1` at pc `0x00400188`. A slot-6 gen-2 kernel thread was `Blocked` as a Receiver
on ep `0x2` at a stack-top-looking pc. svc was frozen at 20.

## The census, re-audited

The re-audit of the fourth stop's endpoint census confirmed its two positive claims and overturned
its conclusion. At the park point on this path the report endpoint is the only endpoint (`0x0`).
The tour's release build creates no other, since `boot_via_progenitor` and the service modules are
aarch64- or test-gated. And `components/src/builder.rs` issues no receive. Its verbs are `invoke`,
`send`, `cap_delete` and `exit`, and its retypes are ASPACE, FRAME and TCB, never ENDPOINT.

What the census never asked is what the machine looks like *after* the receive returns. The answer
is: exactly like those dumps. There are five independent identifications, each checkable from the
tree.

1. The endpoint names. The next two endpoints ever created on this path are
   `riscv_uart_driver_demo`'s `irq_ep` then `report` (kernel/src/user.rs). The registry names them
   `0x1` and `0x2`, in that order, because names are minted lowest-slot-first (crates/slots).
2. The roles and the kinds of thread on them. The driver program's first act is `WAIT` on its Irq
   capability. That parks it as a *Receiver* on `0x1`, a user thread with a nonzero aspace. The
   tour then spawns a kernel thread whose whole body is `ipc_receive(report)` (kernel/src/main.rs,
   the byte receiver). It is a *Receiver* on `0x2` with no aspace. Both wait forever by design:
   nobody types on a bench boot.
3. The pc columns. Every user program links at 0x40_0000. So the driver's post-`ecall` pc
   (`0x00400188`) resolves "plausibly against several binaries at once", which is the dump's own
   recorded warning. The fourth stop resolved it against the builder and got "memset". A kernel
   thread's pc column reads a trap frame that was never written, because kernel threads take no
   user traps. So its bytes are stack-top garbage. "A receiver parked at a stack-top pc" is what a
   *healthy* parked kernel receiver looks like in this dump.
4. The generations. The board has three online harts, so slots 0..3 are the boot thread and three
   idles. Slot 4's occupants in order are a scheduler-step probe thread (gen 0), the outlaw wrapper
   (gen 1), init itself (gen 2), a preemption spinner (gen 3), then the driver at gen 4. That is
   the observed `0x400000004`. Slot 6 held the worker child (gen 0), the second spinner (gen 1),
   then the byte receiver at gen 2, the observed `0x200000006`. The "init" row was the driver
   wearing init's reaped slot.
5. The syscall count. The worker ELF has one loadable page (118 bytes, one `PT_LOAD`). So the whole
   choreography is exactly 20 ecalls. Outlaw makes 3 (yield, yield, exit). The builder makes 14: 1
   aspace retype, 4 for its one page, 3 for the stack, 5 for the TCB and 1 exit. The worker makes
   2 (send, exit). The driver makes 1, the WAIT it parks in. A count *frozen at 20* is not a build
   stalled mid-memset. It is every user program finished or parked.

## QEMU settles it

A healthy tour run with the same initrd prints every line: "the child sent 81 (expected 81)",
preemption, driver started, the banner. Its post-completion dumps show the identical state, shifted
one slot because QEMU's fourth idle thread occupies slot 4. The one user thread is `0x400000005`,
`Blocked`, `wait=0x1/Receiver`, at pc `0x00400188`. The byte receiver sits on `0x2`. Eps `0x1` and
`0x2` hold one receiver each, and svc is 20. The boot thread stands `Running` as its core's current
forever, with climbing ticks and a silent ring. That is what `arch::halt()`'s wfi loop looks like
from this dump.

The boot-8 "wake with no sender in existence" re-reads. The sender was the worker. Its `SEND` of
81 staged the mailbox and set `ipc_served` in the same SCHED section (sched.rs `ipc_send`). That is
why boot 9's gate passed it. The delivered word goes into the "init/build" line the receive's
caller prints. The new `serve:` ring event shows it directly (`serve:0x0/1` on the QEMU run).

## What actually remains broken, and it is not the scheduler

The machine state says the tour's printing steps ran on boots 7 through 9; the state they left is
the proof. The tick counts say the boot hart kept executing. Yet the bench record has none of the
tour's lines after "init : measured, built, started".

No in-kernel loss mechanism was found. `write_byte`'s THRE poll is unbounded, so a wedged
transmitter hangs the printer and never drops. The console lock was demonstrably free, because the
diag dumps kept printing through it. So there are two possibilities. The lines are in the raw
captures and were misread under the hang assumption; the boot 7, 8 and 9 logs want re-examining for
"init/build", "device IRQ" and the banner. Or bytes were lost downstream of the kernel.

Boot 11 answers this without needing the lines themselves. Every dump header now carries the tour
stage last reached. The diag line carries `tx=`, the bytes handed to the transmitter. The ring
carries `serve:` events naming who completed each rendezvous. The corruption canary is armed across
the demo window. It prints every byte that changes in the thread table and endpoint registry, with
address, tick and before/after.

A boot 11 dump showing stage 10 and a grown `tx`, while the wire shows no banner, proves
emitted-then-lost. A stalled stage number names the real wedge point. And the canary either shows
legal deltas matching the choreography, or the stray write the corruption theory needs. As of that
night the stray write had no observed instance. Boots 12 and 13 closed the story; see
[bench-boots-2026-08.md](bench-boots-2026-08.md).
