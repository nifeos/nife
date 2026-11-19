# The JH7110's M.2 root port at the bench: a runbook for a person

*Name: provisional, minted 2026-11-19 (UTC) by lane lane/163-jh7110-pcie-kernel-wiring.*

This page takes a VisionFive 2 with an NVMe drive in its M.2 slot from a fresh build to a judged
log. It holds the bench steps of milestone 163 (the JH7110's PCIe root complex), "Steps that need
the board". [jh7110-pci-express.md](jh7110-pci-express.md) explains what the bring-up does, and
[visionfive2.md](visionfive2.md) covers booting the board at all. One boot answers most of the
questions here, and every step that needs a second boot says so.

## Before anything else

- Read the off-tree bench instructions first. The maintainers keep the bench's unit-specific
  details in a private checkout, not in this repository: which serial device is the board's, which
  outlet powers it, and which outlets and hubs must never lose power. Follow its warnings over
  anything here. Never put anything from it in public text.
- Power off before you touch the M.2 slot. Seat the drive and fit its screw with the board
  unpowered. The slot takes an M-key 2230, 2242, 2260 or 2280 drive.
- Nothing on this page writes the drive. The kernel's disk tree mounts the drive read-only at
  `/disk`, so a drive with data on it is safe. Still, use a drive whose contents you can lose,
  because a fault in a new driver is exactly what this run looks for.

## What the run needs

| what | how to confirm |
|---|---|
| A board that already boots nife over the network | [visionfive2.md](visionfive2.md), "The bench runbook": a boot prints `nife: payload came from net` and `nife on RISC-V (rv64, S-mode, Sv39)` |
| The serial console attached before power | `script/board-console` logs every byte to a file ([board-console.md](board-console.md)) |
| A build of the commit under test | `git log -1` in your checkout names it; record the hash in the results table |
| An NVMe drive in the M.2 slot | seen, with the board off |

## Step 1: check that the tree names the port

The kernel brings up port 1 only from its node in the tree U-Boot hands over. It has no constant
fallback, because a guessed window is how a write lands on the wrong device. So the first question
is what that tree says. Stop at the U-Boot prompt (any key during the countdown), then:

```
fdt addr ${fdtcontroladdr}
fdt list /soc
```

Find the node whose `compatible` is `"starfive,jh7110-pcie"` and whose `linux,pci-domain` is 1 (in
mainline's tree, `pcie@9c0000000`), and print it:

```
fdt print /soc/pcie@9c0000000
```

Record each of these in the results table:

- `reg-names`: the kernel needs `"apb", "cfg"`. Mainline's tree uses those names.
- `reg`: the `apb` window at `0x2c000000` and the `cfg` window at `0x9c0000000`.
- `ranges`: a 32-bit range at `0x38000000` and a prefetchable one at `0x980000000`.
- `perst-gpios`: its second cell should be 28.
- `status`: `"okay"`.

If the board boots with the tree in `${fdt_addr_r}` instead (the extlinux path), use that address
in `fdt addr`. Then `boot` (or power-cycle) to continue.

If the tree names the controller under other `reg-names` (StarFive's own kernel tree has used
`"reg", "config"`), the kernel prints `port One stays off` in step 2 and goes no further. Record
the names exactly. That answers the first board question in milestone 163, and the next change
is in `machine_discovery::jh7110::pci_express`, not at the bench.

## Step 2: boot and read the lines

Boot the build. Every line the bring-up prints starts with `  jh7110 pcie:`. They appear after the
memory map and before the IOMMU lines. A good boot looks like this (addresses from mainline's
tree; counts vary):

```
  jh7110 pcie: port One: bridge 0x2c000000, cfg 0x9c0000000, 32-bit memory 0x38000000+0x8000000, PERST# pin Some(28)
  jh7110 pcie: SYS clocks: 1 turned on, 1 of 1 reset(s) released, 0 step(s) rejected
  jh7110 pcie: STG clocks: 3 turned on, 6 of 6 reset(s) released, 0 step(s) rejected
  jh7110 pcie: link up after 2 read(s), status 0x00000020, 3 window(s) translated
  jh7110 pcie: root port forwards: bus numbers 0x00010100, memory base and limit 0x3ff03800
  jh7110 pcie: the walk over 2 bus(es):
                00:00.0 1556:xxxx class 060400 bridge to buses 01..=01
                01:00.0 vvvv:dddd class 010802
                2 function(s) over 2 bus(es) of the 2 described; mapping 2048 KiB of config space
```

The link status word needs bit 5 (`DATA_LINK_ACTIVE`) set; other bits may differ. Then the NVMe
path runs as it does on every machine. The admin process identifies the controller, and the disk
tree starts only once the drive has answered with its size
([block-devices.md](block-devices.md)). At the shell prompt:

```
ls /disk/nvme0
```

It lists `n1`, the whole namespace as one read-only file. If the image carries `ioping`, read the
drive with it, which makes every request a device read:

```
cd /disk/nvme0
ioping -c 10 -s 4k n1
```

Record both outputs. Without `ioping`, check 8 is UNRUN.

## Step 3: judge the log

Fill one row per check. A check that did not run is UNRUN, never PASS.

| # | check | pass when | result |
|---|---|---|---|
| 0 | commit under test | recorded | |
| 1 | tree node (step 1) | `reg-names` `apb`, `cfg`; `status` `okay`; `perst-gpios` pin 28 | |
| 2 | clocks | both clock lines show `0 step(s) rejected` and every reset released | |
| 3 | no refused access | no `fell outside the mapped pages` line | |
| 4 | link | `link up` | |
| 5 | root port | `00:00.0` class `060400`, `bridge to buses 01..=01` | |
| 6 | drive | `01:00.0` class `010802`, and no other `01:` line | |
| 7 | NVMe | no `controller present but failed bring-up` or `disk tree : not started` line; `ls /disk/nvme0` lists `n1` | |
| 8 | reads | ten `ioping` reads complete with no error | |
| 9 | second boot | checks 2 to 8 again after a power cycle | |

## What a failure looks like

| line | most likely cause, in order |
|---|---|
| no `jh7110 pcie:` line at all | the build predates the bring-up; or the kernel did not take the machine for a JH7110 (no clock-and-reset node and no TRNG) |
| `port One stays off: ...` | the reason is the tree's: step 1 records what it says instead |
| `bring-up stopped: ClocksAndResets` | a clock gate or reset did not take. The two clock lines above it say which domain. A reset that never releases usually means a clock it needs is still off. |
| `bring-up stopped: Window(n)` | a window in the tree is not a power of two of at least 4 KiB. Record the `reg` and `ranges` |
| `link down after 10 reads` | no drive, a drive not seated, or a drive that did not train. Reseat it with the board off and try a second drive before suspecting the sequence. Record the status word. |
| a trap or a hang right after `link up` | a configuration read faulted. See "Unsupported requests" below |
| `01:01.0`, `01:02.0` and so on, all the same device | the root port does not filter device numbers on bus 1. See "Aliased devices" below |
| `controller present but failed bring-up: ...` | the root complex worked and the NVMe driver refused the drive. That belongs to the NVMe path, not this milestone. Record the reason. |
| `disk tree : not started` | the drive did not answer `SIZE`. The same holds as for the line above |

## What only the board can answer

These are the bring-up's open questions, each one recorded in the crate's or the note's `BUGS`.
The run above answers each one, so record what it shows:

- **Unsupported requests.** The walk reads every device number on bus 1. On a PC an absent device
  reads as all ones. Whether this bridge does the same, or raises a bus error the kernel takes as
  a fault, is unknown. A hang or trap after `link up` that names an address inside
  `0x9c0100000..0x9c0200000` is this.
- **Aliased devices.** A downstream port should answer configuration requests on its secondary bus
  for device 0 only. Linux enforces this in software (`only_one_child`), and `pci.rs` does not.
  Duplicates of `01:00.0` mean this bridge answers every device number, and the next change is to
  limit the walk.
- **DMA coherence.** The Ethernet controller's DMA is measured coherent on this board
  ([designware-ethernet.md](designware-ethernet.md)). The PCI Express master sits on another port
  of the interconnect, and nothing has measured it. The NVMe admin queue is itself a test: its
  submission and completion rings are DMA, so an identify that times out or returns a zero size
  with the link up points here first. A byte-for-byte comparison of the drive's data against
  another machine's reading needs a dump tool the board image does not carry yet.
- **The `PCIE_SLV_MAIN` clock.** The STG generator has one, and neither plan turns it on; it stays
  as firmware left it. A link that trains with an endpoint that never answers points here.
- **Who else writes the GPIO page.** The bring-up writes PERST# in the SYS GPIO controller's
  output registers. Nothing else in the kernel writes that page today. If a later server does,
  the two need one owner.

## Record the result

Commit the console log under `bench/radon-<date>/` (UTC) beside the filled table, in a pull request
that ticks the matching items under "Steps that need the board" in
`design/roadmap/0163-jh7110-pcie-root-complex.md`. A FAIL is a result: record it the same way,
with the line that showed it.
