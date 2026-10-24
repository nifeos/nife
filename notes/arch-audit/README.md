# The arch audit: the appendices to notes/arch-audit.md

[`notes/arch-audit.md`](../arch-audit.md) carries the bug class, what was read, the three findings
and their dispositions, the x86_64 gap and the audit's limit. These files carry the evidence its
prose budget has no room for.

- [`candidates-cleared.md`](candidates-cleared.md): every candidate the audit cleared on aarch64 and
  riscv64, and the reasoning that makes each one safe.
- [`plic-lock.md`](plic-lock.md): how finding 3 was closed, why the lock is IRQ-safe, at that rank
  and scoped to that register, and what the tests can and cannot prove.

*Name: provisional. The directory follows the siting default of §212 (a prose budget), `notes/X/`
beside `notes/X.md`. calef names directories.*

Each stem here is named for its content, and calef names files.
