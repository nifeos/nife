---
status: PARTIAL
raised: 2026-09-25
promoted_from: a-wedged-kernel-resets-itself
milestone_dependencies: none
decision_dependencies: none
machine_requirements: riscv64 and aarch64 silicon with a hardware watchdog
specific_machine: xenon (its Intel TCO watchdog step is the first bench step)
needs_person: yes
---
# 593. A wedged kernel resets itself, through a watchdog that does not need asking

Promoted from the proposal `a-wedged-kernel-resets-itself` on 2026-09-25, and
its first step built the same day on `milestone/593-a-wedged-kernel-resets-itself`. *(Number
provisional until the merge queue lands it.)* The proposal was raised by the lane that brought
milestone 249 (the boot lottery is sampled by a person walking to the board)'s reboot to all three
architectures. That milestone resets a kernel that is working and chooses to. A kernel that has
wedged cannot call `arch::reboot`, and a wedge is what the soak (milestones 219 and 225) looks for.

xenon's step and radon's have each reset a deliberately wedged board with nobody at it. argon's
waits on its first nife boot.

## The shape

The soak arms the board's watchdog when its beat begins and pets it on every beat, every five
seconds. A kernel that stops beating stops petting, and the board resets itself. The reset follows
the stall dump the soak already prints and never replaces it: a failing beat panics before the pet,
so the dump is on the console before the timer runs out. A build that arms the watchdog says so in
its banner, the way `reboot_soak_test` does.

## Step 1, xenon's Intel TCO: built 2026-09-25

`kernel/src/arch/x86_64/tco.rs` is the driver. `find()` reads the chipset's configuration space and
returns the TCO it found, holding its I/O base. It knows two layouts. The ICH9 that QEMU's `q35`
emulates keeps the TCO at `PMBASE + 0x60` and `NO_REBOOT` in `RCBA + 0x3410`. The 100/200-series
PCH that xenon's Q270 belongs to keeps it at the SMBus function's `TCOBASE` and `NO_REBOOT` behind
the P2SB sideband. `arm(60)` loads 50 ticks of 0.6 s. The chipset counts them twice before it
resets, so the reset lands 60 s after the last pet. The driver's header cites what was read: Intel's
200-series datasheet volume 2, Linux's `iTCO_wdt`, `lpc_ich`, `i2c-i801` and `p2sb`, and QEMU's
`ich9_tco`.

Every x86_64 `--features soak_test` build arms it from `kernel/src/soak.rs`, xenon's
`cargo xtask uefi-image --features soak_test` stick included. `kernel/build.rs` sets the
`soak_watchdog` cfg for it, because cargo features cannot differ by target, and a `compile_error!`
in `kernel/src/soak.rs` refuses an x86_64 soak build without it. `--features unwatched_soak_test`
(`script/soak-test --no-watchdog`) is the opt-out, for a run where a wedge must stay a wedge on the
console. `--features watchdog_soak_test` insists on the watchdog, and naming it on aarch64 is a
`compile_error!`, so no build can quietly fail to arm. On riscv64 it arms step 2's. `--features wedge_soak_test` is test only: thirty seconds in, the supervisor masks
interrupts on its core and spins.

Two checks in `script/soak-test` prove it under `q35`, both measured on patagonia on 2026-09-25:

| run | passes when | result |
|---|---|---|
| `--wedge --arch x86_64` | the start line appears again after the wedge, and the second boot reads `SECOND_TO_STS` set | PASS, back 63 s after the wedge |
| `--watchdog --arch x86_64` | nothing resets the machine for 180 s after arming, and a pet saw the count below its reload value | PASS, every pet saw 41 of 50 ticks left |

The first uses milestone 249's criterion, a second start line, and adds one fact. `SECOND_TO_STS`
is set only by the chipset's second timeout and cleared only by a write or `RSMRST#`. So the second
boot can say the watchdog reset it, not a triple fault or a stray `0xcf9` write. The second check's
counting condition matters as much. A halted TCO reads its reload value forever, and surviving a
window proves nothing unless the timer ran.

The wedge check was made to fail once, on purpose. With QEMU's no-reboot strap set
(`-global ICH9-LPC.noreboot=true`, a one-off runner edit), it failed at its 200 s deadline with the
wedge on the console. That run also found a limit: the driver still printed `ARMED`, because
`NO_REBOOT` read back clear while the strap held it. So an `ARMED` line does not prove a machine
resets. Only a wedge that comes back does.

CI runs both in the `watchdog` job, only when a path they depend on changes, and always on `main`.

## What needs calef

- Decided by calef, 2026-09-26 (UTC): *"A soak should reset itself when it freezes."* This
  answered whether a soak should reset itself over its own wedge at all, and went further than the
  recommendation it answered ("allow it, as built"): resetting is wanted, not merely allowed. The
  follow-on it sets is recorded under Follow-on below.
- xenon's POST settings. `notes/xenon-firmware.md` records `Prompt on Warnings and Errors` and
  keyboard error detection on. A watchdog reset that stops at a POST prompt has recovered nothing.
  Changing them is a firmware change and stays off until calef says otherwise.

## xenon's first bench step

This lane found that a soak kernel could not reach xenon's stick, because the UEFI loader's seal
check refused it. Milestone 563 (a seal check that reads bytes cannot see a check that was dropped)
closed that gap. After rebasing onto it, `cargo xtask uefi-image --features watchdog_soak_test`
builds on patagonia (2026-09-26), and `uefi_loader/build.rs` panics `NOT SEALED` on any unsealed
pair, so the image is sealed.

The step is one boot with the POST settings unchanged. Build a `watchdog_soak_test`
image, boot it with the serial chain on the desk, and read three lines. The `found the Intel TCO`
line says whether step 1's discovery agrees with the datasheet. A `NOT ARMED` reason says firmware
holds `NO_REBOOT` visibly; an `ARMED` line cannot rule out a strap. The first `petted` line says whether the
count moves. Only then run a wedge build, and watch whether the machine comes back or stops at POST.

xenon passed it on 2026-10-21 (UTC), with one wedge build, which prints the same three lines
before it wedges (`bench/xenon-2026-10-21/README-593.md`). `found the Intel TCO watchdog: Pch
layout, TCO registers at I/O port 0x400`, then `ARMED`, then `petted at t=5s beat=1: the count had
41 of 50 ticks left`. After `WEDGING NOW` the machine reset with nobody at it and POST did not stop.
The firmware then booted the NVMe's older kernel rather than the stick, so `SECOND_TO_STS` on the
boot after went unread.

## Step 2, radon's JH7110 watchdog: built 2026-10-20

`crates/jh7110_watchdog` is the logic: the register offsets, the arm, pet and disarm sequences,
the double-timeout arithmetic and the tree query, transcribed from Linux's `starfive-wdt.c` and
host-tested. The block is SP805-like. It interrupts on the first expiry and resets on the second, and
every write needs `0x1ACCE551` in its lock word first. `kernel/src/drivers/jh7110_watchdog.rs`
walks the sequences.

Nothing about where it is comes from a constant. `memory::init` finds the node by mainline's
`starfive,jh7110-wdt` or by `starfive,dskit-wdt`, the binding radon's U-Boot tree uses, and takes
the address from its `reg`. The clocks and resets come from its own `clocks` and `resets`, read by
`jh7110_clock_and_reset::device_plan`. In the vendor tree those are clocks 123 and 122 and resets
109 and 110. `wdt_core` gates the 24 MHz oscillator, and the tree does not say so, so the soak
measures the counter against its own timer for 100 ms and refuses to arm outside 5 %. A gated
clock reads a frozen count, and that refusal is what stops `ARMED` being printed over it. A tree
naming no watchdog gets `NOT ARMED`. Under QEMU `virt` that line prints and the soak beats on.

The load is 720,000,000 ticks, so the reset comes 60 s after the last pet, the same as xenon's.
Every riscv64 `soak_test` build arms it, as every x86_64 one arms the TCO, and
`--features unwatched_soak_test` is the opt-out. Under QEMU `virt` the soak prints `NOT ARMED` and
beats on.

Name: provisional. `jh7110_watchdog`, `Jh7110Watchdog`, `device_plan` and `DevicePlan`.

## radon's bench step

Its reset does not go through OpenSBI's `pm-reset`, so it may come back where SRST does not. This
boot is the test. It needs a person in reach of radon's power, because nothing can power-cycle radon
(milestone 224 (nothing can power-cycle radon, so a hung soak needs a person)) and a reset that never comes leaves a wedged board. It assumes the procedure in
`notes/soak/rebooting-soak-procedure.md` for the card, the boot script and the console.

1. A healthy soak, armed. Build and write the card, start the watcher, and power the board:

   ```
   script/board-image --soak --extra-features watchdog_soak_test --card /Volumes/NIFE
   script/board-console --for 10m --until none --log target/radon-watchdog-$(date +%s).log
   ```

   Read, in order: `found the JH7110 watchdog: starfive,dskit-wdt at 0x13070000`; a line per
   clock and `2 of 2 reset(s) released, 0 step(s) rejected`; `the counter runs at N Hz` with N
   near 24,000,000; `ARMED`. Every `petted` line should read about 600,000,000 of 720,000,000
   ticks left, one beat's 120,000,000 gone. It passes when nothing resets the board for the ten
   minutes. Then press a key and expect `DISARMED` within five seconds. That proves the escape.
2. A wedge. Build the second card the same way with `--extra-features wedge_soak_test`, and
   watch for twenty minutes. `WEDGING NOW at t=30s` comes six beats in. It passes when U-Boot SPL
   prints about 60 s later and a second `soak-test: started` follows. Record the time from
   `WEDGING NOW` to the SPL banner. The JH7110 keeps no record this kernel knows of how a boot
   ended, so this timing is the evidence that the watchdog reset it.

| seen | means |
|---|---|
| `NOT ARMED` with a reason | Discovery, a clock, a reset or the rate check refused; the line says which. Nothing was armed. |
| a reset during step 1 | The pet does not reload. Compare the `petted` counts. |
| a reset well before 60 s in step 2 | The double-timeout or the rate is wrong. Record the time. |
| silence for three minutes after `WEDGING NOW` | The watchdog's reset does not reach the chip's. Power-cycle radon. |
| SPL, then a stop before nife | The reset works and the boot after it does not. Record where it stops. |

radon passed it on 2026-10-22 (UTC), netbooted rather than from a card
(`bench/radon-2026-10-22/README-593.md`). The armed soak read `starfive,dskit-wdt at 0x13070000`,
both clocks, `2 of 2 reset(s) released, 0 step(s) rejected`, a counter at 23,999,850 Hz and
`ARMED`. It ran 608 s with no reset, every `petted` line between 594,358,858 and 599,367,957 of
720,000,000 ticks left, and one byte gave `DISARMED at t=618s` with the soak still beating. The
wedge build then reset itself three times in a row with nobody at the board, each time booting
nife again and wedging again, until the served kernel was swapped for the healthy one. From
`WEDGING NOW` to the next `U-Boot SPL` took 61.8 s, 61.7 s and 61.8 s, timed on the host to about
0.2 s.

## Later steps

| board | part | how it resets | testable in QEMU |
|---|---|---|---|
| argon | Tegra X1 WDT0 at `0x6000_5100`, paired with timer 5, 1 MHz. Linux `tegra_wdt.c`, which binds only `tegra30-timer`, so the layout is an inference to check against the TRM | chip reset through the PMC on the fourth expiry | no |

## BUGS

- The PCH path has run on one board, xenon's Q270. Another PCH generation may place `TCOBASE` or
  `NO_REBOOT` differently.
- `TCO_EN` in `SMI_EN` is left as firmware set it, following Linux. xenon's firmware does not pet
  the timer from SMI (it reset), but another board's might.
- No silicon boot has read `SECOND_TO_STS` after a real reset: xenon's came back to a kernel that
  does not read it.
- radon's watchdog has run on one board, from netboot. The reset it gives has not yet been seen
  to bring back a boot from radon's card.
- The JH7110 keeps no record of a watchdog reset that this kernel knows of, so a second boot on
  radon cannot say how the first ended. Only the timing can.
- Every riscv64 rebooting soak is also a watchdog soak. On radon the two ran together for 52 draws
  (milestone 249's series of 2026-10-22), each draw arming afresh after the AXP15060 reset, and none
  was reset early. On x86_64 nothing guards the pair: under QEMU the TCO keeps counting across a
  `0xcf9` reset, so the second boot could be reset before it arms.
- In a rebooting soak only `arm_reboot` drains the receive buffer, so the escape that
  `script/board-console --stop` sends on its banner reaches both. That is proven under QEMU by
  hand and not by any check in `script/soak-test`, whose `--reboot` run sends nothing.

## Follow-on

- **Done.** x86_64 soak builds include the watchdog by default, per calef's 2026-09-26 ruling, and
  `unwatched_soak_test` (`script/soak-test --no-watchdog`) is the opt-out. See step 1.
- **Done.** xenon's first bench step, under that heading above (2026-10-21).
- **Done.** radon's bench step, under that heading above (2026-10-22).
- **Done.** riscv64 soaks arm the watchdog unless they name `unwatched_soak_test`, as x86_64's do:
  the `kernel/build.rs` condition and the `compile_error!` beside it in `kernel/src/soak.rs`.
- **Outstanding.** argon's Tegra X1 watchdog, the row of Later steps. It waits on argon's
  first nife boot.
- **Milestone 563.** A soak kernel could not reach xenon's stick. That gap was folded into milestone
  563 and built there.

## Index row

A soak arms the Intel TCO watchdog on x86_64 and pets it every beat, so a wedged kernel is reset by
the chipset 60 s after its last pet. Proven under QEMU q35 both ways: a deliberate wedge resets and
the second boot reads the chipset's own record of it, and a healthy soak survives 180 s. On in every
x86_64 soak build, with `unwatched_soak_test` as the opt-out. radon's JH7110 watchdog is armed by
every riscv64 soak, found and clocked from the tree. A wedged xenon or radon resets itself; argon is
a bench step.
