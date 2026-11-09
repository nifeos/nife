# The I219 cabled run: a runbook for one bench session on xenon

*Name: provisional, minted by lane `lane/i219-cabled-run`, for the stem `the-i219-cabled-run`.
The session that `nifeos/e1000e` issue 10 asks for, written so that a person at xenon, or an agent
session on the build machine beside that person, can run it from this page alone.*

**Nothing on this page has run on silicon with a cable.** The bench boot it uses has run under
QEMU's 82574L only, which never takes the I219 path, and the I219 bring-up has run on xenon once,
with no cable ([the note's BUGS](../e1000e.md#bugs)). The crate's README keeps its one-run
sentence until this session has a result.

## The prompt to start the session

Paste this into a Claude session started in a checkout of nife on `main`, on the machine that has
xenon's serial adapter and shares a local network with xenon:

> You are running the I219 cabled run on xenon. Read `notes/e1000e/the-i219-cabled-run.md` in this
> checkout and follow it step by step, from "Before anything else" to "Record the result". Read the
> off-tree bench README it names before you touch anything, and never switch an outlet or unplug a
> USB hub: ask me for every power cycle and for anything physical. Ask before you open the serial
> port or write the stick. Do not claim a pass that did not run. Stop and tell me at every stop
> condition the page lists, and finish by showing me the filled results table.

## What this decides, and what it does not

One boot of the `network_bench` kernel on xenon's I219-LM (`8086:15e3`, Sunrise Point), cabled to
a switch, records the three things the issue asks for:

1. the bring-up observations that `Hardware::note` receives, as `  e1000e:` lines;
2. the link state, `DEVICE_STATUS_LINK_UP`, as the `network-bench: nic` line's `link up` or
   `link NOT UP`;
3. whether frames passed in both directions, with counts, as the `network-bench: frames` lines.
   They are the device's own statistics counters (good frames received and transmitted, CRC
   errors, missed frames), totaled since bring-up, so they count what crossed the wire whatever
   the rings did.

The boot also takes a DHCP lease and times one TCP transfer, which is milestone 494 (a driver for
the network card a PC actually has)'s exit criterion 2 and the condition for setting
`e1000e_service::PCH_PROVEN_ON_SILICON`. One boot does both; about twenty minutes.

It does not measure the I219's speed: the transfer rate is this stack's, as
[notes/bench-boots.md](../bench-boots.md) says. It does not test the link-up half, which the crate
does not port.

## The NIC's flash is only read

No step here writes the I219's NVM or any firmware. Read from the code:

- The crate's `platform_controller_hub::non_volatile_memory` is the only code that reaches the
  flash registers at BAR0 offset `0xe000`. Its module documentation says it is read only, and its
  code agrees: `read_u32_at_byte` clears the cycle-type field (`flash_control::CYCLE_MASK`) to zero,
  which is a read (`ICH_CYCLE_READ`), before every cycle. Its other writes clear the status
  register's error bits, set the linear address and start that read cycle. It never writes the
  flash data register and never selects a write or erase cycle.
- The kernel (`kernel/src/e1000e.rs`) writes only the control, interrupt, ring, filter and
  multicast registers by name, and passes the crate's own accesses through `Bar`.
- What the passes do write is volatile: MAC registers, PHY registers over MDIO (the workarounds and
  the local area network connected device configuration the NVM describes), and the host-to-ME
  request to leave ultra-low-power mode. A power cycle forgets all of it.

If a reading of the commit under test finds a write to the flash data register, or a cycle type
other than zero, stop and report it before building anything.

## Before anything else

1. **Read the off-tree bench instructions.** The maintainers keep the bench's unit-specific details
   off-tree: which serial device is xenon's, which outlet powers it, and which outlets and hubs
   must never lose power. Ask the person where that checkout is, and read its `README.md` before
   touching any hardware. Follow its warnings over anything here. Never quote it or name anything
   from it in public text: a commit, a pull request, an issue or a note.
2. **The rules for the whole session.**
   - Never switch any outlet, and never unplug or power-cycle a USB hub. Every power cycle and
     every key at xenon's firmware is the person's: ask in one sentence and wait.
   - Ask before opening the serial port and before writing the stick. Building on the host and
     reading a log need no ask.
   - Anything physical (the cable, the switch, the stick, the boot menu key) is the person's. Say
     what you need and wait; do not infer the state of the desk.
   - Do not end your turn while xenon runs. Poll the log in calls of up to ten minutes.
   - A pass that did not run is UNRUN in your report, never PASS.

## Preconditions

| what | how to confirm | if not |
|---|---|---|
| An Ethernet cable from xenon's port to a switch whose network has a DHCP server, and this machine on the same network | the person confirms; the switch port's link light comes on once xenon is powered | stop and ask |
| A monitor and a USB keyboard on xenon: it halts at POST without a keyboard | the person confirms | stop and ask |
| xenon's serial console reaches this machine (the off-tree README names the adapter) | the adapter's device is listed (`ls /dev/cu.usb*` on macOS, `/dev/ttyUSB*` on Linux) | the screen is the record; photograph it |
| No other process holds that port | `lsof <device>` prints nothing | ask the person which session holds it; do not kill it unasked |
| A FAT32 stick xenon boots from ([notes/x86-uefi-boot.md](../x86-uefi-boot.md), "What you need" and "Firmware settings") | it mounts on this machine | ask the person; formatting a stick is a media write and needs their go |
| TCP port 9494 free here, and inbound connections to it allowed | `lsof -i TCP:9494` prints nothing | pick another port and use it in steps 3 and 4 |
| The toolchain from `script/setup`, and QEMU with OVMF for step 2 | `cargo xtask network-bench` runs | run `script/setup`; without QEMU, skip step 2 and say so |

## The steps

### Step 1: choose the commit under test

The commit under test is `main` of nife, with the `e1000e` pin it carries. Record both:

```sh
git log -1 --format=%H
grep 'nifeos/e1000e' kernel/Cargo.toml
```

The first is row R1, the crate revision in the second is row R2. Work in a worktree of that commit,
never in a checkout whose `target/` another session uses.

### Step 2: rehearse under QEMU

```sh
helpers/qemu-bounded.sh 900 cargo xtask network-bench
```

It must end `network-bench: as expected`. The transcript shows the 82574L's `nic` line, a lease in
`10.0.2.0/24`, a `frames` line after the transfer with good frames counted both ways, and
`verdict LEASED-AND-MEASURED`. It proves the boot and its counters work on QEMU's device and says
nothing about the I219. Record its last line in your report.

### Step 3: build and stage for xenon

`P` is this machine's address on the network the switch is on (on macOS
`P=$(ipconfig getifaddr en0)`, or `en1` on Wi-Fi).

```sh
cargo xtask network-bench --stage-only --peer "$P:9494"
shasum -a 256 target/esp-network-bench/EFI/BOOT/BOOTX64.EFI
```

The hash is row R3. The peer is compiled into the image, so a different network or port is a
rebuild. With the person's go, copy the image to the stick at `/EFI/BOOT/BOOTX64.EFI` and eject
it.

### Step 4: start the peer and the console

In one terminal, the peer, which serves 100 MiB per connection (allow `nc` if macOS asks):

```sh
while true; do head -c 104857600 /dev/zero | nc -l 9494; done
```

With the person's go, start the console capture, under `target/` so that the log stays local:

```sh
nohup script/board-console --board xenon --port <device> --for 15m --until none \
    --log target/xenon-i219-cabled-run.log > target/xenon-i219-console.log 2>&1 &
```

### Step 5: ask for the boot

Ask: "Please put the stick in xenon and power it on; press F12 for the one-time boot menu if it
does not take the stick." Record in row R4 how it came up (for example "cold power-on by the
person, F12, stick").

### Step 6: watch the bring-up

`LC_ALL=C grep -a -E 'e1000e:|network-bench:' target/xenon-i219-cabled-run.log` shows the run.
The boot prints its banner line (`network-bench: milestone 494 bench boot. release build`), then
one `  e1000e: pass N of 3` line before each I219 pass, the observations each pass notes, and
`  e1000e: the three passes returned`, then the `network-bench:` lines. The run is over at
`network-bench: done, halting.`; the transfer takes well under a minute on a working gigabit link.
Then stop the console (`kill` the process you started) and the peer loop. xenon sits halted and
needs a power cycle before anything else boots, which is the person's.

## Reading the result, pass by pass

A refusal or a hang belongs to the last `pass N of 3` line above it.

| pass | what runs | PASS | FAIL |
|---|---|---|---|
| 1, `initialize_physical_layer_device_workarounds` | the Management Engine is asked to leave ultra-low-power mode, the PHY is reached, the PHY is reset and its configuration replayed | a `ManagementEngineUltraLowPowerExit` (or `SoftwareUltraLowPowerExit`) line, `PhysicalLayerDeviceReached { at: ..., id: ... }`, and then `pass 2 of 3` | `refused at bring-up` before `pass 2 of 3`, or no line after `pass 1 of 3` |
| 2, `global_reset` | the descriptor-ring flush, then the global reset and the PHY's post-reset configuration | `pass 3 of 3` follows | `refused at bring-up` before `pass 3 of 3` (`ResetTimeout` belongs here), or no line after `pass 2 of 3` |
| 3, `initialize_hardware` | the hardware bits, the copper link setup | `the three passes returned` | `refused at bring-up` before it, or no line after `pass 3 of 3` |

The observations come from `platform_controller_hub::sequence::BringUpObservation`, and each line
is `e1000e: ` and the variant as Rust prints it. On xenon the one earlier run printed, during pass
1, `cleared: true`, the PHY reached `Directly` with identifier `0x015400a0` (`I217_E_PHY_ID` in
FreeBSD), and a configuration of 22 pairs
([bench/xenon-2026-10-21/README-494.md](../../bench/xenon-2026-10-21/README-494.md)). A different
identifier, access path or pair count is a value to record, not a failure. A line such as
`LocalAreaNetworkInitializationTimeout` or `MasterRequestsPending` is an observation FreeBSD also
carries on from; record it.

After the passes:

| line | what it records | PASS | FAIL |
|---|---|---|---|
| `network-bench: nic : 8086:15e3 I219-LM5 (SPT), link up, ...` | the link state: `DEVICE_STATUS_LINK_UP` rose within 3 s (`LINK_WAIT_MS`) | `link up` | `link NOT UP` then `FAILED: no link` |
| `network-bench: phy : id 0x...` | the identifier pass 1 read | any value | none |
| `network-bench: preflight dmar scope : PASS` | the IOMMU owns the NIC | `PASS` | `FAIL`: record it; the frame counts still stand |
| `network-bench: dhcp : leased a.b.c.d` | frames both ways at the protocol level | a lease | `waiting for a lease` is the last line |
| `network-bench: frames : at the lease, received R good, transmitted T good, C CRC errors, M missed` | the counters when the lease was reported; the transfer may already have begun | R and T at least 2 | R or T zero |
| `network-bench: transfer : N bytes in S s = X Mbit/s` | one TCP stream from the peer | N is 104857600 | `client stopped with code ...` |
| `network-bench: frames : after the transfer, received R good, transmitted T good, ...` | the totals since bring-up | R in the tens of thousands, T above zero | R or T zero |
| `network-bench: verdict LEASED-AND-MEASURED` | milestone 494's criterion 2 | this verdict | any other verdict |

CRC errors or missed frames above zero on a run that otherwise passes are values to record. Many
of either, with a slow or stalled transfer, point first at the link-up half that the crate does not
port (its `platform_controller_hub` BUGS).

## Stop conditions

Stop on any of these, keep the log, and report the last 50 lines. None of them asks for a retry
with anything changed on xenon's firmware or the NIC.

| you see | it means | next |
|---|---|---|
| no `network-bench:` line within two minutes of power | the stick was not booted, or the kernel did not start | the person checks the boot menu; [notes/x86-uefi-boot.md](../x86-uefi-boot.md), "If it does not", has the table |
| `FAILED: no NIC this driver claims is on the bus` | the card's id is not one `nifeos/e1000e` claims | record the PCI survey's class `020000` line |
| `refused at bring-up: Pch(...)` | a pass stopped; the last `pass N` line names it | record the error and every `e1000e:` line; a lane reads them |
| a hang after a `pass N of 3` line | that pass did not return | photograph the screen; the person power-cycles; a lane |
| `link NOT UP` | no autonegotiation in 3 s | the cable, the switch port's light and the switch first; one more boot with another port is the person's call |
| `waiting for a lease` last, after `link up` | the link came up and no lease came, so no `frames` line prints | the network's DHCP server; record the switch port's light; a lane |
| `client stopped with code ...` | the stream did not complete | the peer loop and this machine's firewall; the `frames` line after it still records the counts |
| any line that says the flash was written, or a reading of the code that finds a write | not expected from this code | stop at once and report; do not boot again |

## Record the result

- Copy the log to `bench/xenon-<date>/i219-cabled-run.log` (UTC date) in a lane's pull request,
  after removing any line that carries a machine-local value (a MAC or IP address). A lane fills
  the table below in this page in the same pull request.
- The results then go to the crate: they replace the one-run sentence in its README's BUGS and in
  `platform_controller_hub`'s BUGS, through the crate repository's own pull request, whichever way
  the run went.
- A `LEASED-AND-MEASURED` verdict is what a lane cites to set `PCH_PROVEN_ON_SILICON`
  (`kernel/src/user/e1000e_service.rs`), in its own pull request.

### Results

Rows R1 to R4 are the run; the rest are what it printed. A row that did not run says UNRUN.

| row | what | value | verdict |
|---|---|---|---|
| R1 | nife commit | | |
| R2 | `nifeos/e1000e` revision | | |
| R3 | image SHA-256 | | |
| R4 | how xenon came up | | |
| 1 | pass 1 observations (`e1000e:` lines) | | PASS / FAIL / UNRUN |
| 2 | pass 2 observations | | PASS / FAIL / UNRUN |
| 3 | pass 3 | | PASS / FAIL / UNRUN |
| 4 | link (`DEVICE_STATUS_LINK_UP`) | | PASS / FAIL / UNRUN |
| 5 | PHY identifier | | RECORDED |
| 6 | DMAR preflight | | PASS / FAIL / UNRUN |
| 7 | lease | | PASS / FAIL / UNRUN |
| 8 | frames at the lease (received, transmitted, CRC errors, missed) | | PASS / FAIL / UNRUN |
| 9 | transfer (bytes, seconds, Mbit/s) | | PASS / FAIL / UNRUN |
| 10 | frames after the transfer (received, transmitted, CRC errors, missed) | | PASS / FAIL / UNRUN |
| 11 | verdict | | |
