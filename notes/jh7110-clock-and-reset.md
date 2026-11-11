# Programming a clock and a reset line, for the first time

Milestone 220 (this kernel drives no clock or reset controller, and the first real device will
need one). `crates/jh7110_clock_and_reset` (the offsets, the plans, each tree's spellings),
`crates/machine_discovery/src/jh7110/clock_and_reset.rs` (the device-tree walk),
`crates/jh7110_clock_and_reset_contract` (the wire, §320 (the JH7110 clock and reset wire)),
`components/src/jh7110_clock_and_reset_server.rs` (the stores),
`kernel/src/user/jh7110_clock_and_reset_service.rs` (the spawn and the starters' calls), and
`kernel/src/drivers/jh7110_clock_and_reset.rs` (the kernel's own walk, for its watchdog).

**This has never run against silicon.** QEMU's riscv64 `virt` machine has no clock or reset
controller of any kind, so an emulator cannot validate the sequence end to end: what CI exercises
is the absence path and the arithmetic. The bench procedure below is the part that would settle it,
and it is written to be followed rather than interpreted, the way `notes/x86-uefi-boot.md`'s is,
because the lane that wrote it could not reach the machine either.

## Why nife needed this at all

Every device this kernel has driven came up already running. QEMU's virtio devices are software.
The PL011 and NS16550 consoles are already clocked when firmware hands over. The PLIC is part of
the core complex. So nothing in this tree had ever programmed a clock, and nothing knew it needed
to.

Real `SoC` peripherals are not like that. Most of the JH7110's blocks boot gated, and the firmware
ungates only what the firmware itself uses. A gated block's register window is not absent, which is
the part that makes this hard to diagnose: it accepts every store and reads back zero.

**Measured on 2026-09-04.** Two boots of radon, byte-identical:

```text
hw entropy  : FAILED: JH7110 TRNG at 0x1600c000 (tree says starfive,trng, status disabled):
              report 0x524e475550, bring-up diagnostic 0x0000000000000000,
              draws 0/0 bytes, first-all-zero true, draws-differ false
```

Transcript: `target/board/radon-2026-09-04-trng-bringup.log`. The diagnostic is the raw
`(STAT << 32) | ISTAT`, so the whole register file read back as zeros, and the vendor device tree
independently marks the node `status disabled`. Milestone 239 deliberately *reports* that `status`
without acting on it. The same tree calls the S7 core `okay` and gives it an MMU it does not have,
so the agreement between the tree and the hardware is corroboration rather than circularity.

## What the TRNG needs, from two trees that agree

Linux's `jh7110-trng.c` takes two clocks and a reset before it touches a register, in this order:

```c
trng->hclk = devm_clk_get(&pdev->dev, "hclk");
trng->ahb  = devm_clk_get(&pdev->dev, "ahb");
trng->rst  = devm_reset_control_get_shared(&pdev->dev, NULL);
clk_prepare_enable(trng->hclk);
clk_prepare_enable(trng->ahb);
reset_control_deassert(trng->rst);
```

The order is load-bearing, and Linux's own reset driver says why in a comment: *"if the
associated clock is gated, deasserting might otherwise hang forever"*. `ENTROPY_BRING_UP` reproduces
it and a host test pins it, because a plan that deasserted first would wedge a boot on hardware and
pass every test an emulator can run.

The identifiers are the interesting part, because radon does not run mainline's device tree. It
runs the vendor U-Boot's, which spells everything differently and numbers its clocks and resets
flat across all five domains rather than per domain. Both were fetched on 2026-09-04 and they
converge:

| | mainline Linux | vendor U-Boot (what radon serves) | resolved |
|---|---|---|---|
| node | `rng@1600c000`, `starfive,jh7110-trng` | `trng@1600C000`, `starfive,trng`, `status = "disabled"` | (not this table's subject) |
| controller | `stgcrg`, `starfive,jh7110-stgcrg` | `clkgen`, `starfive,jh7110-clkgen`, `reg-names = "sys", "stg", "aon"` | `0x1023_0000`, size `0x1_0000` |
| clock 1 | `JH7110_STGCLK_SEC_AHB` = 15 | `JH7110_SEC_HCLK` = 205, stg group starts at 190 | **index 15**, word at `0x3c` |
| clock 2 | `JH7110_STGCLK_SEC_MISC_AHB` = 16 | `JH7110_SEC_MISCAHB_CLK` = 206 | **index 16**, word at `0x40` |
| reset | `JH7110_STGRST_SEC_AHB` = 3 | `RSTN_U0_SEC_TOP_HRESETN` = 131, stg group starts at 128 | **id 3**, bit 3 of `0x74`, watched at `0x78` |

The rebase arithmetic is a host test (`the_two_trees_agree_on_the_identifiers`) rather than a
paragraph, so a renumbering upstream fails the build instead of surprising somebody at the bench.
Every URL and fetch date is in `crates/jh7110_clock_and_reset/src/lib.rs`'s header.

All three identifiers are STG-domain, so `ENTROPY_BRING_UP` means nothing without `STG`, and that
coupling is why `Step` has no domain field. A second device in a second domain carries its own plan
and its own domain beside it; that is the time to decide whether the pair wants a type.

Two mechanics worth knowing, both transcribed rather than inferred:

- A clock is one 32-bit word per index, at `base + 4 * index`, and bit 31 turns it on
  (`JH71X0_CLK_ENABLE`).
- A reset's status bit is inverted from what the name suggests: a *set* bit means the line is
  out of reset. Linux's `jh71x0_reset_update` computes `done = mask` for a deassert (the JH7110
  passes `asserted = NULL`) and polls until `(value & mask) == done`. Getting this backwards
  produces a driver that waits forever on a device that came up correctly, which is why
  `jh7110_clock_and_reset::is_deasserted` exists as one function with one test rather than as an
  expression at two call sites.

## Where it lives, and who may drive it

One userspace process, the clock and reset server, writes the shared windows. The kernel spawns it
on a JH7110 with the first page of each window the tree named: STG, SYS and AON device-typed, and
the SYS syscon read-only, since the server only reads PLL2's words there. Every clock word and
every reset word of the three domains sits in that first page
(`jh7110_clock_and_reset::device::USED_WINDOW_BYTES`, checked at compile time).

A client never names a device. Its capability on the server's endpoint carries a badge, the
device's register base physical address as the tree's `reg` gives it, and the server looks the
badge up in `jh7110_clock_and_reset::device::DEVICES`: the TRNG at `0x1600_c000`, the first GMAC
port at `0x1603_0000`, the two SD/MMC controllers at `0x1601_0000` and `0x1602_0000`, and I2C5 at
`0x1205_0000`. A badge outside the table, 0 included, gets status 1, and so does a device whose
window this machine's tree named nowhere. The window is shared by USB, both PCIe root ports, the
DMA engine and the security block, so a driver that held it would hold the widest authority in the
tree. Holding a badge instead, it can bring up, read and rate its own device and nothing else
(§256 (a server that keeps windows for many clients scopes each by the caller's badge)).

The wire is §320's, in `jh7110_clock_and_reset_contract`: interface id 24, version 1, five
operations (version query reserved, bring up, read clock, read reset, set rate), eight statuses,
and a packed bring-up summary. Bring up enables every clock the device's plan names, in every
domain, before it releases any reset, because Linux's reset driver warns that a deassert against
a gated clock can hang. It is idempotent. No operation turns a device off (§304 (who may turn a
device off)).

The kernel's own starters are the server's clients today: the entropy starter, the GMAC
bring-up and both SD/MMC starters each call it with their device's badge through
`sched::ipc_call_badged` before they spawn the driver. The kernel keeps its own walk
(`drivers::jh7110_clock_and_reset::bring_up`) for two callers: the soak's watchdog (milestone 593
(a wedged kernel resets itself)), whose supervisor pets it from the kernel, and the reboot path's
I2C5 bring-up, which milestone 1062 (the device control planes leave the kernel)'s step 5 moves to
a userspace PMIC route.

### The one genuinely dangerous thing here, and what stops it

`machine_discovery::jh7110::clock_and_reset::discover` **never fails to produce an address**. A tree
that names no controller gets `SYSTEM_TOP_GROUP_BASE` (`0x1023_0000`) with `from_tree: false`. That
is deliberate: radon's firmware tree is already known to omit and misdescribe things, and a bench
session that comes back with "no controller node, nothing attempted" has spent a trip to the machine
and learned nothing. Two independently published trees agree on the number, which is a stronger
warrant than most device-tree reads get.

But a caller that took that answer unconditionally would store to `0x1023_0000` on every board
this kernel boots, and on QEMU's `virt` that address is unmapped. So the guard is one rung up
from a comment:

- `memory::initialize` records the window **only when the tree names a JH7110** (a clock controller,
  or the TRNG whose clocks this exists to ungate). QEMU's `virt` names neither.
- `mmu::map_everything` maps only what `memory::jh7110_clock_and_reset()` returned, and the server
  starter maps the server only the windows `memory` recorded, so on any other machine there is no
  mapping to store through and no server.
- `no_clock_window_is_mapped_where_there_is_no_jh7110` (`system_tests/src/user/entropy_tests.rs`) pins
  it, because the failure is silent in the direction that matters: a load or store fault during
  boot on a machine nobody would think to blame a clock driver for.
- The boot tour prints which source the address came from, in words, so a transcript can never
  be read as confirmation of an address nobody on that machine confirmed.

### How a window is found

`discover`, `discover_aon` and `discover_sys` (in `machine_discovery::jh7110::clock_and_reset`)
try the three spellings of a `jh7110_clock_and_reset::RegisterWindow`, mainline's first, as the TRNG's
discovery does. A tree that carries the standardized binding is describing itself
in that binding's language, so it is the one to believe. The two vendor nodes carry several windows
and are indexed by `reg-names` rather than by position, because a position that happens to be right
today is a fact nobody wrote down. `pmic_bus` reads the same way, and one id bound serves both
trees there because the vendor providers number every domain in one flat space with SYS first.

## The bench: bringing the TRNG's clocks up on radon

**This has not been done.** radon was powered off and there was no bench session when this was
written.

### What you need

Everything `notes/visionfive2.md`'s bench runbook already lists: the card, the DIP switches on
QSPI, the serial chain, and USB-C power. Nothing new. Power-cycle only radon's own outlet (milestone
224); **never switch any other outlet.** Which is which is kept off-tree by the maintainers.

### Build and copy

```console
$ cd /path/to/nife
$ script/board-image --card /Volumes/NIFE
$ diskutil eject /Volumes/NIFE
```

The archive must be the matched set: the kernel measures it, and a mismatched pair halts with
`MEASURED BOOT REFUSED` rather than booting something ambiguous.

### Watch it

```console
$ ls /dev/cu.usbmodem*
$ script/board-console --for 5m
```

### What you should see, in order

The boot proceeds exactly as `notes/visionfive2.md` records, up to the tour. Two lines are the
whole of this milestone, and the clock line now comes first:

```text
  hw clock    : JH7110 STG CRG at 0x10230000 (from the tree), through the clock and reset
                server: Done, clocks 2/2 running, resets 1/1 released, 0 refused
  hw entropy  : JH7110 TRNG at 0x1600c000 served 32+32 bytes to a client through a capability
                that names no device; first draw 8f3a1c04.., second differs
```

(wrapped here; each is one line on the wire).

### What each outcome means

Read the clock line first: it decides how the entropy line should be read.

| The `hw clock` line says | What it means | What to do |
|---|---|---|
| `no clock and reset server runs` | No window was recorded, or the archive has no `jh7110_clock_and_reset_server`. On radon the first is a regression in discovery, not a fact about the board | Dump the tree at the U-Boot prompt (`fdt addr $fdtcontroladdr; fdt list /soc`) and check what the controller node is actually called. Milestone 239 is the precedent: the node name was `trng@1600C000` with an upper-case C |
| `the published constant` | The TRNG node was found but no clock controller was, so the address came from the constant | The bring-up still ran and the rest of the line is real. Capture `fdt list /soc` and add the node's real compatible to `jh7110_clock_and_reset`'s discovery list, so the next boot reads it rather than assuming it |
| `Done, clocks 2/2 running, resets 1/1 released` | **The sequence worked.** Bit 31 read back on both clocks and the status bit came up | Read the `hw entropy` line, below |
| `ClockNotRunning, clocks 0/2 running` | The enable bit did not stick. The window is device-mapped and accepting stores, and nothing is behind it | The base address is wrong, or the STG domain itself is gated by a parent this milestone does not program. See BUGS: the SYSCRG `stg_axiahb` parent is the first suspect |
| `ResetHeld, resets 0/1 released` | The clocks came up and the reset did not release. Linux's own comment says a gated clock is the usual cause of exactly this hang, so a `running` verdict beside it is contradictory and interesting | Capture the whole line. This is the outcome that most wants a register dump before anyone changes code |
| `; the firmware had already done all of it` | **The premise was wrong.** The clocks were on and the reset released before nife touched anything | Then a gated clock is *not* why the TRNG read zeros, and milestone 220 does not explain the 2026-09-04 transcript. The next suspect is the base address or the `reg` decode, which is milestone 159's territory again |

Then the entropy line:

| The `hw entropy` line says | What it means |
|---|---|
| `served 32+32 bytes ... second differs` | **Fatal risk 6's "drives real hardware" half is closed**, and it was closed by a confined EL0 driver holding one device page |
| `FAILED ... bring-up diagnostic 0x0000000000000000` **with** a `Done` clock line | The window still reads as nothing, with its clocks demonstrably on. That is a new fact, and it is not this milestone's | 
| `FAILED` with a non-zero diagnostic | The device answered and the driver's sequence is wrong. That is milestone 159's, and it is the outcome 159's block was written to route |
| `FAILED` with the clock line reporting `ClockNotRunning` | One cause, two symptoms. Fix the clock line first; the entropy line carries no independent information until it is |

### If it works

Record the transcript under `target/board/` with the date in the filename, the way
`radon-2026-09-04-trng-bringup.log` is, and say in the roadmap block which boot it was. The line
this milestone exists to produce is a *measurement*, and a measurement nobody can find is a claim.

## BUGS

- None of this has run on hardware. Every register offset, bit position and identifier is
  transcribed from published source; the only thing proven is that the two published trees agree
  with each other and that the arithmetic is right. An emulator cannot do better here, because
  QEMU's `virt` machine has no clock or reset controller to model.
- **Only five devices, in three domains.** The JH7110 has five domains (`syscrg`, `stgcrg`,
  `aoncrg`, `ispcrg`, `voutcrg`) and hundreds of clocks; the server serves the devices in
  `device::DEVICES` and answers status 1 for every other badge. A device is a table entry, not a
  rewrite.
- Each device's plan is the crate's constant, not its tree's `clocks` and `resets`. A board whose
  tree wires a device differently gets the constant plan.
- The server answers the version query with status 2 until milestone 1000 (message contracts
  carry bind-time version checks) defines its reply. A server that dies is not restarted.
- Parent clocks are not programmed, and this is the first thing to suspect if the enable bits do
  not read back. The STG domain's own bus clock (`stg_axiahb`) comes from the SYSCRG at
  `0x1302_0000`, and nothing here touches it. Linux gets away with the same narrow sequence because
  its clock framework walks parents automatically. This does not, and relies on the firmware
  having left the bus clocks running, which is plausible (U-Boot uses the STG domain for USB and
  PCIe) and unverified.
- The deassert poll is an iteration count, not a duration. Linux uses a 1000 µs timeout;
  the server's and the kernel walk's `POLL_LIMIT` count a million reads instead, as the kernel walk
  did before any calibrated delay existed. A million MMIO reads is far longer than the microsecond the hardware needs and far
  shorter than a boot anyone would call hung, but it is not a time bound and a slower board would
  scale it silently.
- Nothing here can turn a clock off or assert a reset, deliberately. Gating a clock or
  asserting a shared reset would stop or interrupt a neighbor mid-transaction, and no caller in
  this tree has a reason to. It also means there is no teardown: a service that dies leaves its
  device clocked. That is the right trade today (nothing reclaims device power) and it is the piece
  a power-management milestone would have to add.
- **`BringUpReport`, the kernel walk's report, records only the first four clock steps.** A longer plan still performs every step;
  the recording stops and `truncated` says so. Nothing in the tree has a plan that long.
- **The vendor `rstgen` node is tried and has never been exercised by a fixture.** Its
  `reg-names` spelling (`"stgcrg"`, not `"stg"`) is transcribed from the vendor tree and is only
  reached by a tree carrying a reset controller and no clock controller, which no tree here has.
