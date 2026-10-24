# Can a userspace process hold a timer: the appendices to notes/timer-capability.md

[`notes/timer-capability.md`](../timer-capability.md) carries the answer and the verdict per
architecture. It also carries why the MMIO route does not rescue a userspace timer service, what the
fourth shape and the second scaffold cost in headline figures, the dependency on milestone 151
(notification objects) and the `BUGS`. This directory carries the working its prose budget has no
room for.

- [`specifications.md`](specifications.md): the register fields and specification text behind each
  architecture's verdict, including the stale sentence in §139 (who may read the cycle counter).
- [`mmio-timers.md`](mmio-timers.md): the spare MMIO timer on each machine, and the sources.
- [`first-scaffold.md`](first-scaffold.md): the 2026-09-05 scaffold's method, its byte tables and
  what it did not price.
- [`second-scaffold.md`](second-scaffold.md): the 2026-09-13 scaffold's reclamation and
  kernel-thread pricing, the four builds and their zero error bar, and the riscv64 `.rodata` cliff.

*Name: provisional. The directory follows §212 (a prose budget)'s siting default, `notes/X/` beside
`notes/X.md`. calef names directories.*
