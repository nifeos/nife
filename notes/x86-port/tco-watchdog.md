# The Intel TCO watchdog

*Name: provisional, like every stem in this directory.*

`kernel/src/arch/x86_64/tco.rs` drives the one piece of an Intel PC that resets the machine when the
kernel stops talking to it, milestone 593 (a wedged kernel resets itself). Only the watchdog soak
drives it, so only that build compiles it. This page holds its mechanism, its register map and
sources, and its BUGS.

## The mechanism

Every Intel chipset since the ICH carries a TCO ("total cost of ownership") timer beside its
power-management logic. Software loads a count of 0.6-second ticks, and the timer counts down. A
write to its reload register starts the count again, which is the pet. If the count reaches zero,
the chipset sets `TIMEOUT`, reloads, and counts again. If it reaches zero a second time with no pet
in between, it sets `SECOND_TO_STS` and asserts `PLTRST#`, a platform reset that needs nothing from
the CPU. That second timeout is the whole point. A kernel spinning with interrupts off cannot ask
for a reset, and this does not wait to be asked.

So the reset lands `2 x TMR x 0.6` seconds after the last pet, not `TMR x 0.6`. Linux's `iTCO_wdt`
halves the count only on TCO version 1, whose `TMR` is six bits wide. On versions 2 and 4 its
`heartbeat` is the first timeout, and the reset comes one heartbeat later. `Tco::arm` takes the time
to the reset as its argument and does the halving itself, so the number a caller writes is the
number that matters to a person at the board.

The driver is a value, not a global (rule 2). `tco::find` reads the chipset's configuration space
and returns a `Tco` holding the base it found, and the caller holds it. Nothing keeps a static, so
a kernel that never calls `find` never touches the chipset.

## Two generations, and where each keeps its registers

| | ICH9 (QEMU `q35`), TCO version 2 | 100/200-series PCH (xenon's Q270), TCO version 4 |
|---|---|---|
| found through | LPC bridge `00:1f.0`, device `0x2918` | SMBus controller `00:1f.4`, device `0xa2a3` (also `0xa123`, `0x9d23`) |
| TCO base | `PMBASE` (config `0x40`, bits 15:7) plus `0x60`; decode needs `ACPI_CNTL` (`0x44`) bit 7 | `TCOBASE` (config `0x50`, bits 15:5); decode needs `TCOCTL` (`0x54`) bit 8 |
| `NO_REBOOT` | `GCS`, memory at `RCBA` (config `0xf0`) plus `0x3410`, bit 5 | the SMBus `GC` register in private config space, `SBREG_BAR + 0xc6_000c`, bit 1 |

The registers inside the TCO block are the same on both. `TCO_RLD` is at `+0x00`, `TCO1_STS` at
`+0x04`, `TCO2_STS` at `+0x06`, `TCO1_CNT` at `+0x08` (bit 11 halts the timer), and `TCO_TMR` at
`+0x12` (bits 9:0, the tick count).

The sources were read on 2026-09-24. They are Intel's *200 Series Chipset Family PCH Datasheet,
Volume 2* (335193), sections 7.1.17, 7.1.18, 7.3.2, 4.1.6, 4.1.39 and 31.1. In Linux they are
`drivers/watchdog/iTCO_wdt.c`, `drivers/mfd/lpc_ich.c`, `drivers/i2c/busses/i2c-i801.c` and
`drivers/platform/x86/p2sb.c`. In QEMU they are `hw/acpi/ich9_tco.c`, `hw/isa/lpc_ich9.c` and
`include/hw/southbridge/ich9.h`. Intel's ICH9 datasheet (316972) could not be downloaded, so the
version 2 column rests on Linux and QEMU agreeing, not on Intel.

## The evidence that survives the reset

The datasheet says `SECOND_TO_STS` is cleared only by writing a 1 to it or by `RSMRST#`. That is the
resume well's reset, not the platform reset the watchdog asserts, so the next boot can read the bit
and know how the last one ended. `Tco::arm` reads it before clearing it and reports it. That turns
"the machine came back" into "the machine came back because the watchdog reset it". QEMU keeps the
register across its own reset (its `pm_reset` does not touch the TCO block), which is what lets
`script/soak-test --wedge` check the claim.

## BUGS

- The version 4 path has never run. q35 is an ICH9, so the SMBus lookup, the P2SB unhide and the
  sideband `NO_REBOOT` write execute for the first time on xenon. Each step prints what it found, so
  a first bench log says which step disagreed with the datasheet.
- Firmware can hold `NO_REBOOT` set, and then the watchdog counts and never resets. The datasheet
  lets the no-reboot strap override software on version 4, and a BIOS may use it. `Tco::arm` reads
  the bit back after clearing it and refuses to report armed if it stuck, so the failure is a line
  on the console. That read-back is not proof. Under q35 with `-global ICH9-LPC.noreboot=true` (the
  strap) the bit reads back clear, the driver reports armed, and the second timeout never resets.
  Only a wedge that comes back proves a machine resets.
- `TCO_EN` in `SMI_EN` is left as firmware set it. With it set, the first timeout raises an SMI,
  and firmware's SMI handler could in principle pet the timer on the kernel's behalf. Linux clears
  it only for version 1 by default, and this follows Linux. If xenon's first wedged soak never
  resets, this is the second thing to check, after `NO_REBOOT`.
- Nothing halts the timer at boot. The datasheet makes `TCO_TMR_HALT` default to 0 (counting) after
  a platform reset, and it is firmware that halts it. A firmware that does not would reset an
  unarmed kernel about 2.4 seconds after the reload value's default of four ticks runs out twice.
  Neither q35 nor, by all accounts, a Dell does this.
- The ACPI `WDAT` table, where firmware describes a watchdog for the OS, is not read. Linux prefers
  it when present. xenon's tables have not been dumped with that question in mind.
