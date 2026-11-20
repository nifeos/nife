# A cross-check on the Apple cores, attempted 2026-09-19, and why it did not run

An appendix to [notes/job-mix.md](../job-mix.md). It was a section there until 2026-10-11.

This was never going to be a result, and in the event it produced no numbers at all. The idea
(calef's, 2026-09-19) was to run the seven-job sweep under QEMU with Apple's Hypervisor.framework on
patagonia, where four vCPUs run on four real cores at once. It would cross-check the curve's *shape*,
and whether the median holds `tasks=4` still. HVF timing cannot decide anything for §96 (process kernel or event kernel, and how to decide it) or risk
4, because macOS schedules the vCPUs underneath the guest. It could only say whether the shape
appears on real cores other than radon's.

`script/job-mix` gained `--hvf` and `--release` for it (the spellings `cargo xtask run` and
`cargo xtask bench` already use). Five attempts, each started only when no other QEMU was running
on the host, were all refused before the kernel ran:

```text
qemu-system-aarch64: HVF does not support GICv2 emulation
```

QEMU 11.1.1 on this host refuses HVF with a GICv2, and `kernel/src/drivers/gic.rs` speaks only
GICv2 (`notes/hvf-leg.md`, `notes/interrupts.md`'s `BUGS`). Milestone 227 (GICv3) fixed that;
the spawn job then fails (2026-10-05, [BUGS](null-system-call-under-load.md#bugs)). The transcript is
`bench/patagonia-hvf-2026-09-19/jobmix-hvf-refused.log`. The flags stay, because the command that
will take the cross-check once 227 lands is then already written:

```sh
script/job-mix --hvf --release --smp 4    # alone on the host: check `pgrep -l qemu` first
```
