---
status: BUILT
raised: 2026-07-13
built: 2026-07-14
---
# 8. The console driver leaves the kernel

Built in `a5533cb` (2026-07-14). The revised milestone table (`491f23d`) called this "the milestone
that proves §10 (process model: capability-based, microkernel) was real": if the console could not
be taken out, "we did not build a microkernel; we built a monolithic kernel with an unusual syscall
table."

This is the milestone where plan and outcome diverge, and the divergence is the history worth
keeping. The original plan (`b7f10e7`) had milestone 8 as "virtio-blk driver + read-only
filesystem." When §10 committed the project to the microkernel shape, `491f23d` rewrote the
ladder: 8 became the console extraction, virtio-blk moved to 9, and the old 9 ("Processes:
spawn, exit, wait") folded into 10. A backfill that copied the original table would misdate the
driver work and record a plan that was overtaken two days in.

What the commit records, precisely so it does not lie: the console user programs use became an
EL0 process holding a mapping of the PL011's registers, running the same busy-wait loop at a
different exception level. A debug UART stays in the kernel for boot, panics, and the test
harness, the same split seL4 makes, and the honest claim is the narrow one: no code path a user
program can take reaches kernel UART code.

## Follow-on

- **Refused.** Taking the *debug* UART out of the kernel too. It stays for boot, panics and the test
  harness, which is the same split seL4 makes: a kernel that needs a userspace service to print
  cannot report the failure of that service, and a panic has to reach a human whatever else is
  broken. The claim was narrowed to match rather than the code changed, and the narrow claim is the
  true one: no code path a user program can take reaches kernel UART code.

## Index row

the microkernel thesis, executable: no user-reachable path touches kernel UART code
