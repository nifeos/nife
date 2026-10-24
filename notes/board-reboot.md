# A kernel-initiated reboot on every board

Milestone 249 (the boot lottery is sampled by a person walking to the board) built a soak that
reboots itself, on riscv64 only. On 2026-09-24 aarch64 and `x86_64` joined it. This note records how
each architecture resets, what QEMU proves, what each board must still show on the bench, and the
correction to radon's 2026-09-04 result.

Name provisional: calef names notes.

## One contract, three routes

`soak::draw_again` calls `arch::reboot(marker)`. Each architecture prints one line per attempt,
before making it. A reset stops the UART draining, so the line before the attempt is the only record.
The call returns only when every route was refused. The soak then prints `FAILED` and keeps running.

| arch | route, strongest first | where the route comes from |
|---|---|---|
| riscv64 | SBI SRST `system_reset`, reset type 1 | fixed; the SBI is always there |
| aarch64 | PSCI `SYSTEM_RESET` (`0x8400_0009`), after `PSCI_VERSION` | `/psci`'s `method`, the conduit `CPU_ON` already uses |
| `x86_64` | FADT `RESET_REG`, then `0xCF9 <- 0x02, 0x0E`, then 8042 `0xFE` | the FADT, read during the ACPI walk |

On `x86_64` the FADT register may be in I/O space or PCI config space. A memory-space register is
logged and skipped. The `0xCF9` write sets full reset (bit 3), which drops the rails, so it is a
cold reset. Every `x86_64` boot now prints a `fadt reset register:` line. On q35 it reads
`Io 0xcf9 (8 bits) <- 0x0f`.

The escape is the same on every console. The kernel polls a sticky data-ready bit: `LSR.DR` on the
NS16550 (riscv64 and `x86_64`), and `FR.RXFE` on the PL011 (aarch64).

The feature `reboot_soak_test` no longer implies `board`. The old reason was that a reset under QEMU
means nothing. It does mean something: QEMU resets the machine and loads `-kernel` again.

The code is in `kernel/src/arch/aarch64/mod.rs`, `kernel/src/arch/x86_64/reset.rs` and
`kernel/src/arch/riscv64/semihosting.rs`. The FADT parser is
`machine_discovery::acpi::parse_fadt_reset`, with a host test.

### Why x86_64 tries its routes in that order

A PC has no one firmware call that owns its power, the way PSCI does on aarch64 or SBI SRST on
RISC-V. It has three reset mechanisms of different vintages, so `arch::x86_64::reset::reboot` tries
them strongest first. The FADT's reset register comes first because firmware vouches for it. On
every Intel chipset this project has seen it names port `0xCF9` anyway, and a machine that puts it
elsewhere is exactly the machine on which the `0xCF9` guess is wrong. `0xCF9` is the chipset's Reset
Control register, written `0x02` (system reset) then `0x0E`. Bit 3, full reset, makes the platform
drop power to the rails briefly rather than only pulling the CPU's reset line. That matters on a
board whose firmware does not survive a warm reset, which is radon's lesson. The 8042 keyboard
controller's pulse, `0xFE` to port `0x64`, is the oldest route and the one a platform with nothing
else is most likely to emulate. Each route gets a tenth of a second before the next is tried, and
nothing halts, because the caller still has a running machine to report on.

Since milestone 805 (`reboot` at the prompt) the route also runs on a machine holding a filesystem
open, through the reboot object (`kernel/src/reboot.rs`). Nothing in `reset.rs` quiesces devices
first: a reset through `0xCF9` cuts off a DMA engine mid-transfer with everything else, which is
the point of a cold reset. What makes it safe is that `reboot` syncs the writable filesystem before
it invokes the object, so the block servers are idle, and nothing else writes persistent storage.

## What QEMU proves

`script/soak-test --reboot --arch <arch>` passes only on a second boot. The soak's start line must
appear again after `rebooting now`. A QEMU that exits on the reset fails it. So does a guest that
prints the line and stops, and a reset that hangs in firmware.

All three passed on patagonia on 2026-09-24, and again after rebasing:

```console
aarch64  PASS, 128s in: attempt 1 of 1: PSCI SYSTEM_RESET over hvc (PSCI_VERSION answered 1.1)
x86_64   PASS, 129s in: attempt 1 of 3: FADT reset register, I/O port 0xcf9 <- 0x0f
riscv64  PASS, 128s in: attempt 1 of 1: SBI SRST system_reset, reset type 1 (cold reboot)
```

## BUGS

- The proof is in no CI gate. Each leg takes over two minutes, because the window is the board's
  own 120 seconds. A regression is found when somebody runs it.
- On `x86_64` only attempt 1 has run. q35 offers the FADT route (`0xCF9 <- 0x0F`), so the `0xCF9`
  and 8042 fallbacks have compiled and never executed. A QEMU proof of either needs a machine whose
  FADT does not offer that route, and no runner here starts one. Whether xenon's `0xCF9` full reset
  brings it all the way back through firmware to a netboot is the bench's first question.
- The aarch64 `smc` conduit has never executed. QEMU's PSCI answers on `hvc`. `smc` is argon's.
- A memory-space FADT reset register is not attempted. No machine here uses one.
- Nothing has run on any board.

## Each board's first bench step

A reset the firmware does not survive is not a reboot. So each first step passes only when the board
returns all the way to a netboot and `soak-test: started`. A dark board fails it, and so does a
banner with nothing after it. The escape check in notes/soak.md's procedure comes after that.

| board | first step | what QEMU cannot show |
|---|---|---|
| radon | one reset with milestone 592 (radon's cold reboot dies in OpenSBI's PMIC write)'s kernel, watched to `soak-test: started` | OpenSBI's PMIC write failing again |
| argon | boot nife at all, which has never happened; then one PSCI reset over `smc` | NVIDIA's boot chain; upstream TF-A has dropped Tegra210 |
| xenon | read the boot log's `fadt reset register:` line, then one reset watched to a netboot | Dell firmware stopping at a POST prompt ("Prompt on Warnings and Errors") |

Unattended today: none of the three. radon's reset hangs in OpenSBI. argon does not boot nife.
xenon has a proven kernel side and an untested firmware.

## Correction: radon never reset on 2026-09-04

The 2026-09-04 record said radon's SoC reset and U-Boot SPL then failed on the PMIC. **That is
wrong.** No second boot ever began.

In `target/board/radon-2026-09-04-srst-reset-pmic.log`, with CRs stripped, line 200 is `rebooting
now`. Lines 201 to 211 are ten `i2c read: write daddr 36 to` lines and `cannot read pmic power
register`. The file ends there, with no `U-Boot SPL` banner after the reset line.

The message is OpenSBI's. radon's banner says `OpenSBI v1.2` and `Platform Reboot Device :
pm-reset`. On this platform a cold reboot is an I2C write to the AXP15060 PMIC: register `0x32`, bit
6. If the read before the write fails, OpenSBI prints that line and hangs the hart. The source is
`starfive-tech/opensbi`, branch `JH7110_VisionFive2_devel`, `platform/generic/starfive/jh7110.c`,
lines 126 to 190, read 2026-09-24. The binary on radon's flash is not published as source, so this
is the matching source, not a proven one.

The likely cause is in the same log. At `Starting kernel`, U-Boot prints `clk u5_dw_i2c_clk_core
already disabled` and `clk u5_dw_i2c_clk_apb already disabled`. U-Boot gated I2C5, the PMIC's bus.
OpenSBI turns the APB gate back on, but only when that register reads zero, and never the core
clock. That inference is half right, by milestone 592 (radon's cold reboot dies in OpenSBI's PMIC
write). radon
runs an older OpenSBI than the branch above, pinned by SDK tag `VF2_v2.10.4`. U-Boot's handover
also asserts I2C5's reset, which that OpenSBI never releases. It computes the clock word from the
node name, and U-Boot's `i2c@12050000` gives UART4's core clock instead of I2C5's gate. The "core
clock" is a divide-by-one child of the APB gate with no register of its own. The kernel now brings
I2C5 back up before the reset; the milestone's block has the sources and the bench outcome table.

SRST shutdown fails the same way on this board (notes/visionfive2.md, boot 15+), which fits. The
route is not closed. It is blocked on one I2C transaction that the kernel may be able to set up.

## Proposals this work filed

Filed in `design/roadmap/proposals/`. Two have since been promoted to milestones.

- `radons-reboot-dies-in-opensbis-pmic-write.md`, now milestone 592: bring I2C5 back up before
  SRST (built). Then a nife PMIC write, about 250 lines. Last, a firmware update, which is calef's
  call.
- Milestone 593 (a wedged kernel resets itself), promoted from `a-wedged-kernel-resets-itself.md`:
  hardware watchdogs. xenon's TCO is built and proven under q35; radon and argon follow.
- `0653-xenon-may-carry-amt.md`: remote power and serial over LAN, if xenon's factory option has AMT.
  It needs a firmware change and calef's network.
