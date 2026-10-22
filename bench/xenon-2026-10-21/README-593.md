# xenon, 2026-10-21 (UTC): milestone 593's Intel TCO watchdog resets a wedged kernel

The xenon bench step of milestone 593 (a wedged kernel resets itself, through a watchdog that does
not need asking). `main` at `5ee9b4afb`, built with `cargo xtask uefi-image --features
wedge_soak_test` (which arms the watchdog as `watchdog_soak_test` does, then wedges at 30 s), and
booted from a stick at about 23:35 UTC with F12 at POST. POST settings unchanged.

The block's runbook asks for a `watchdog_soak_test` boot first and a wedge build second. This ran
the wedge build alone, because it prints the same three lines before it wedges; had they read
`NOT ARMED`, the wedge would have cost the power cycle a separate boot costs anyway.

| file | what it shows |
|---|---|
| `593-wedge.log` | the boot: the TCO found, armed, petted, the soak's first six beats, the deliberate wedge |
| `593-after-reset-nvme.log` | the next boot, which nobody started |

The console was read by a raw reader (115200 8N1), which sends nothing, and cleaned with
`briefs/bench-session.md` step 6's `tr`. Nothing was typed on the console or on xenon's keyboard
after the boot began.

## What the boot printed

```
soak-test-watchdog: found the Intel TCO watchdog: Pch layout, TCO registers at I/O port 0x400
soak-test-watchdog: the previous boot was not ended by this watchdog (SECOND_TO_STS clear)
soak-test-watchdog: ARMED. THIS BUILD RESETS THE MACHINE IF THE SOAK STOPS BEATING: 50 ticks of 0.6s counted twice, so a reset 60.0s after the last pet, which comes every 5s from the beat.
soak-test-watchdog: petted at t=5s beat=1: the count had 41 of 50 ticks left
soak-test-watchdog: WEDGING NOW at t=30s, deliberately (--features wedge_soak_test): ...
```

The PCH path (SMBus lookup, P2SB, the sideband `NO_REBOOT` write) found the TCO and armed it, and
the count moves.

## The reset

A new boot followed the wedge with no person touching xenon. calef, asked afterwards whether he had
touched xenon's power or keyboard between about 23:36 and 23:38 UTC: *"No I did not touch
xenon."* POST did not stop at a prompt.

The firmware's boot order chose the NVMe, not the stick, because the stick is reached only through
F12. So the boot after the reset is the kernel installed on xenon's NVMe on 2026-10-04 (a different
entry address and an 11-region memory map, against the stick kernel's 38), which panics early:
`no PCI BAR window on this machine: the firmware map stops describing memory at 0xb759c000 but the
host bridge's TOLUD says 0xd0000000`. That kernel does not read `SECOND_TO_STS`, so the chipset's
own record of the reset went unread. The reset itself is the result; the second boot's failure
belongs to milestone 801 (packages over the internet)'s installed system, not to this one.
