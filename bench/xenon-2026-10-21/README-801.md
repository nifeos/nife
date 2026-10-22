# xenon, 2026-10-21 (UTC): the installed system no longer boots

A finding for milestone 801 (packages over the internet), whose exit criterion runs from xenon's
installed system. It came from milestone 593 (a wedged kernel resets itself)'s bench step: after the
watchdog reset xenon at about 23:36 UTC, the firmware booted the system installed on xenon's NVMe on
2026-10-04 (`bench/xenon-2026-10-04/`), not the stick.

| file | what it shows |
|---|---|
| `593-after-reset-nvme.log` | that boot, byte for byte the file pull request #2535 adds |

The installed kernel's loader handed over an 11-region memory map ending at `0xb759c000` (a
current stick boot on the same machine hands over 38 regions), and the kernel panicked before its
tour finished:

```
[PANIC] panicked at kernel/src/lib.rs:519:33:
no PCI BAR window on this machine: the firmware map stops describing memory at 0xb759c000 but the host bridge's TOLUD says 0xd0000000
```

then reported its stack canary dead. Current `main` boots the same machine (the README of milestone 890 (riscv64 and x86_64
join the shared boot path) in this directory), so this is the 2026-10-04 build, not today's code. Before 801's run, xenon
needs a fresh install from a current image.
