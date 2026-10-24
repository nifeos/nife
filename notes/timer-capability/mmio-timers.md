# MMIO timers: the route the kernel already knows how to delegate

*Name: provisional. For the parent's "The other route: an MMIO timer the kernel already knows how to
delegate". calef names files.*

An appendix of [can a userspace process hold a timer?](../timer-capability.md). The parent keeps why
this route does not rescue a userspace timer service. This file keeps the per-machine table and the
sources, read on 2026-09-05.

## The route

The parent's specification verdicts are about the *architected* timer on each ISA. There is a second
route that needs no new mechanism at all, because this tree already has it. An MMIO timer block is a
`DeviceFrame` plus an `Object::Irq`, which is exactly how every userspace driver in this system
already owns a device. If a machine has a spare MMIO timer, a userspace timer service can hold it
today.

The machines do have them, and QEMU does not. Read 2026-09-05 from the vendor manuals, mainline
Linux bindings and QEMU's own source:

| machine | spare MMIO timer | channels | per-channel interrupt |
|---|---|---|---|
| argon, Jetson TX1 (Tegra X1 / T210) | yes, `timer@60005000` | 14 29-bit counters plus a 32-bit timestamp | yes, 14 distinct GIC SPIs |
| radon, VisionFive 2 (JH7110) | yes, `timer@13050000` (`si5_timer`) | 4, 24 MHz | yes, 4 distinct PLIC lines |
| xenon, OptiPlex 7050 | HPET, presumed present, not confirmed on this machine | up to 32; the spec's recommended minimum is 3 | yes, per-timer routing when the legacy route is off |
| QEMU `virt`, aarch64 | none | | |
| QEMU `virt`, riscv64 | the goldfish RTC's alarm only | 1 comparator | yes, IRQ 11 |

Sources and the exact text:

- Tegra X1. `Documentation/devicetree/bindings/timer/nvidia,tegra-timer.yaml` in mainline Linux:
  *"The Tegra210 timer provides fourteen 29-bit timer counters and one 32-bit timestamp counter...
  Each TMR can be programmed to generate one-shot, periodic, or watchdog interrupts."* and *"A list
  of 14 interrupts; one per each timer channels 0 through 13"*, with
  `arch/arm64/boot/dts/nvidia/tegra210.dtsi` listing all fourteen SPIs. The Tegra X1 TRM itself is
  behind an NVIDIA developer login and was not read, so this row is mainline Linux written by
  NVIDIA's own maintainers rather than the vendor manual.
- JH7110. StarFive JH-7110 TRM, Preliminary V2 (2023-04-24, JH7110-TRMEN-001), *Timer → Overview*:
  *"Si5_timer consists of 4 decrement bd_timer that cause interrupts in single-run or continuous-run
  mode and have their own clock input."* Its *Interrupt Connections* table gives `TIMER_INTR[0..3]`
  four separate PLIC sources. There is no mainline Linux driver: the binding was posted ten times
  from December 2022 and never merged, and mainline's `jh7110.dtsi` has only the CLINT. The driver
  exists in StarFive's vendor tree (`drivers/clocksource/timer-starfive.c`,
  `compatible = "starfive,jh7110-timers"`). So this hardware would be a driver this project writes.
- QEMU aarch64 `virt`. `hw/arm/virt.c`'s `base_memmap[]` has no timer device. The only
  timer-adjacent entries are `VIRT_RTC` (a PL031) and `VIRT_GWDT_*`, the SBSA watchdog, which is not
  created by default. And the PL031's alarm is one-second resolution: `pl031_set_alarm` computes
  `ticks = s->mr - pl031_get_count(s)` and arms `now + ticks * NANOSECONDS_PER_SECOND`. Useless as a
  general-purpose timer.
- QEMU riscv64 `virt`. `hw/riscv/virt.c`'s `virt_memmap[]` has the CLINT and a goldfish RTC and no
  general-purpose timer. The goldfish RTC's alarm *is* usable: `hw/rtc/goldfish_rtc.c` has
  `RTC_ALARM_LOW/HIGH`, a nanosecond counter, and a real one-shot `timer_mod`. One comparator, and
  this tree already drives that device for the wall clock (`crates/clock_contract`, §43 (reading the clock is a page, setting it is a page you may write, proposing is an endpoint)).
- **HPET.** IA-PC HPET Specification 1.0a (Intel, October 2004). Section 2.3.4, `NUM_TIM_CAP` (bits
  12:8): *"This indicates the number of timers in this block. The number in this field indicates the
  last timer"*, so the count is `NUM_TIM_CAP + 1`. Section 2.2's recommended minimum is **3
  comparators**, all three one-shot capable and one periodic capable. Section 2.3.5's `LEG_RT_CNF`,
  when set, spends timer 0 on IRQ0 and timer 1 on IRQ8 and leaves *"Timer 2-n... routed as per the
  routing in the timer n config registers"*. On a minimum implementation with the legacy route on,
  that is exactly one free general-purpose timer.
