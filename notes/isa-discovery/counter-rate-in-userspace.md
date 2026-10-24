# The counter rate in userspace

*Name: provisional. For how `cntfrq` learns the rate. calef names files.*

An appendix of [ISA discovery](../isa-discovery.md): how userspace learns the counter rate, and the
lesson.

## How userspace learns the rate

RISC-V has no register that reports the timebase, and userspace cannot *read* the device tree, but
the kernel can *hand it a page*. `user_mode_runtime::cntfrq` on RISC-V reads the timebase page the
kernel maps read-only into every process at `counter_frequency_contract::PAGE_VA`, the mechanism
milestone 161 (the x86_64 kernel port) shipped on `x86_64`. Not an ABI addition such as an
aux-vector entry at process start (the way Linux passes `AT_HWCAP`), because the mechanism already
existed.

## The lesson

A limitation recorded with a proposed fix attached stops being re-examined, so before filing one as
a design fork, check whether the tree already solves it somewhere else. A hard-coded rate is not an
honest gap: radon's rate is 4 MHz, so a 10 MHz default is a silent 2.5x on the one board that
matters.
