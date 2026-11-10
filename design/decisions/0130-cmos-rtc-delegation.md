---
status: AMENDED
raised: 2026-08-26
decided: 2026-08-26
ratified_by: calef
---

# 130. How the kernel-resident CMOS RTC reaches the userspace clock service

The rule in force, ruled 2026-11-10 (UTC): the userspace clock service reads the x86 CMOS itself,
through a `PortRange` capability over ports 0x70 and 0x71. On the other two architectures it reads
a PL031 or a Goldfish RTC through a mapped page in the same way. The kernel reads no CMOS. That is
option 2 below. The lane of [milestone 176 (the x86_64 discovery seam's wide half: COM1's IRQ and a
CMOS RTC)](../roadmap/0176-x86-64-discovery-seam-wide-half.md) raised the fork.

## What §121 provides

[DECISIONS §121 (x86 port I/O)](0121-port-io-capability.md) gives x86's legacy devices a
`PortRange` capability, a `(base, count)` range enforced by the TSS I/O permission bitmap. The
bitmap is written lazily: a thread that holds no port capability switches with no bitmap at all, so
only a switch to or from a port holder pays. A `PortRange` opens its ports only with `WRITE`,
because the bitmap cannot permit `in` without `out`. So the clock service holds `WRITE` over the two
CMOS ports, which also lets it set the clock, as its mapping already does on aarch64 and riscv64.

## What was considered

1. Repurpose `RTC_REGION`'s `(u64, u64, u64)` to carry `(0x70, 0x71, CMOS-kind)` instead of an
   address and size. Rejected outright, not merely disfavored: `clock_service.rs` maps `rtc.0` as
   a physical page address. Pouring port numbers into that field would have the mapper build a
   device mapping of physical page `0x70`, which is real memory near the start of RAM. A
   correctness bug waiting to happen, not an implementation detail to smooth over.
2. A port-range capability, so the userspace service reads CMOS the way it maps a PL031. This is
   §121's option 1, and it makes x86 match the other two architectures: a capability the driver
   holds, enforced by hardware, with no kernel mediation per access. **Decided.** §121 built the
   capability with a lazy bitmap write, so its cost lands only on switches involving the clock
   service, and moving the read out takes the CMOS code out of the kernel.
3. The kernel reads CMOS once and passes the wall-clock seed to the clock service as a `Spawn`
   argument. The read is a dozen `in8`/`out8` pairs and costs under a microsecond. Linux reads the
   RTC once at boot in the same way. Refused: it keeps a device driver in the kernel, and it makes
   the kernel a writer of the clock's initial value.
4. A kernel-mediated IPC broker, queried on demand (§121's own option 3 for the console, about
   337 ns per round trip). Refused: a second new mechanism for a value that changes once a boot.

## Decision

Option 2. The clock service holds a `PortRange` over 0x70 and 0x71 and reads the CMOS itself.
`clock_contract::rtc::CMOS` then names two ports the service reads, as `PL031` and `GOLDFISH` name
register layouts. A userspace process can run this driver with a capability nife already has, so
the kernel does not. Milestone 1062 (the device control planes leave the kernel) carries the change.
Until it lands, `kernel/src/arch/x86_64/rtc.rs` still reads the CMOS and passes the seed.
