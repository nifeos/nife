# Fatal risk 6's bench evening on xenon, and how to read its verdict

*(Milestone 261 (the NVMe driver leaves the kernel). Names here are provisional per the naming tenet; calef names them:
`disk_throughput`, `cargo xtask disk-throughput`, `DmarUnits`, `Scope` and this page's stem among
them.)*

Written on 2026-09-24, before any boot of the instrument it describes. Everything was code in this
tree, built, host-tested and rehearsed under QEMU with OVMF and `-device intel-iommu`, or a bench
question marked as one. The outcomes predate the numbers, so the numbers could not choose their own
interpretation.

## What the evening is for, in one paragraph

`design/fatal-risks/README.md` risk 6 asks whether a capability-confined userspace driver can drive real
hardware at real speed. Its decisive experiment is one real, non-virtio device on real silicon,
confined, at throughput. Milestone 261 built the driver: an EL0 process holding one page of the
NVMe's BAR0 and a confined DMA window serves the block verbs. xenon has the device (a Micron 2450
behind a PCIe root port) and a VT-d IOMMU, and calef wiped its disk on 2026-09-17. Missing was a
measuring boot that shows on the night whether its measurement means what risk 6 needs: the
`disk_throughput` kernel feature.

## The two night-of conditions, and what changed so neither can pass silently

The risk entry names two night-of conditions no code can make true.

**1. The DMAR's device scope must cover the NVMe function.** A VT-d unit translates only the
requesters the firmware's DMAR assigns it. The kernel brought up the DMAR's *first* unit and
reported the device confined whenever any unit was translating (`confined_by_iommu` was
`iommu::is_active()`). That is the wrong question on a client Intel machine. On the Skylake OptiPlex
7040, the same family as xenon's 7050 with the same register addresses, Linux prints:

```text
DMAR: DRHD base: 0x000000fed90000 flags: 0x0
DMAR: DRHD base: 0x000000fed91000 flags: 0x1
```

Read on 2026-09-24 from <http://linux-hardware.org/index.php?probe=a94acdc2f3&log=dmesg>. Flags
`0x0` with a scope naming only the integrated GPU, then the catch-all (`INCLUDE_PCI_ALL`). xenon's
DMAR is 204 bytes and its first unit is `0xfed90000`
(`bench/xenon-2026-09-17/tour-display-225100.log`), the size of two units plus RMRRs.
If xenon matches its sibling, every earlier boot translated the graphics unit, and the NVMe
test asserted "confined" about a device nothing translated. That is inference from a sibling; the
preflight below is the reading.

What changed:

- `machine_discovery::acpi::DmarUnits` decodes every DRHD and PCI device scope, and answers which
  unit owns a function by VT-d 3.x section 8.3's rule (an explicit scope, then a bridge's
  sub-hierarchy, then the catch-all). A table it could not fully record answers "unknown", never
  "the catch-all".
- Since milestone 594 (every VT-d unit translates its own devices), the kernel brings up every unit
  and routes each device to its owner. The RMRRs are identity-mapped first. On QEMU's `q35` nothing
  changes.
- `iommu::scope_of(rid)` asks the DMAR whether a unit that is up owns the requester.
  `confined_by_iommu` is that answer, so the ordinary NVMe boot test now fails there instead of
  passing.

**2. The LBA size must give `blocks_per` in `1..=8`.** The driver moves 4096-byte blocks, so the
namespace needs 512, 1024, 2048 or 4096-byte LBAs. A refused namespace and an
absent controller were both `None`, and the boot test printed both as `skipped: no NVMe controller
came up`. xenon's second attempt on 2026-09-17 printed exactly that about a disk that was
there. Now the refusal carries the LBA size, the test fails on a present-but-refused
controller, and the bench boot prints the size either way.

## What the boot prints

After the ordinary tour (unchanged, so its `vt-d` lines are still evidence), each line is prefixed
`disk-throughput:` so a photograph and a capture read alike:

```text
disk-throughput: fatal risk 6 bench boot (milestone 261). WRITES to the NVMe disk.
disk-throughput: preflight first; a skip is not a pass. release build, 2713000000 Hz counter.
disk-throughput: measured    non_volatile_memory_express sha256 f62871988f413c27.. matches the table this kernel vouches for
disk-throughput: preflight 1/2 dmar scope : PASS  nvme 01:00.0 (rid 0x0100): drhd 0xfed91000 is the catch-all and no other unit names it
disk-throughput: preflight 2/2 lba size   : PASS  512-byte lbas, blocks_per 8 (needs 1..=8); namespace N bytes
disk-throughput: ipc floor   2000 SIZE round trips in N us (N ns each, no device)
disk-throughput: write       16384 x 4096 B in N us = N B/s (N ns per block)
disk-throughput: flush       one FLUSH in N us
disk-throughput: read        16384 x 4096 B in N us = N B/s (N ns per block)
disk-throughput: verified    16384 of 16384 blocks came back stamped with their own number (window 256..16640 in 4096-byte blocks)
disk-throughput: verdict CONFINED-AT-RATE: read N B/s, write N B/s, EL0 driver; drhd 0xfed91000 is the catch-all and no other unit names it
disk-throughput: done, halting.
```

The unit address and requester id above are predictions, not readings: `0xfed91000` is the
7040's catch-all, and `01:00.0` assumes the root port's secondary bus is 1. The line shapes are exact; each has printed under QEMU.

What a figure counts: one command in flight of 4096 bytes, polled completion, and two context
switches per block, since the kernel's boot thread `CALL`s the EL0 server once per block.
That is a lower bound on the device and an honest measure of this driver, not comparable to `fio` at
queue depth 32. The `ipc floor` line lets whoever quotes a figure subtract the IPC share. The
server's measurement is checked against the archive's table before it runs, since this boot
replaces the hand-over that would.

## Rehearsing it, which needs no machine

```sh
cargo xtask disk-throughput                  # all four cases, about a minute on an Apple M3 once built
cargo xtask disk-throughput --case bypass    # one of them
```

Each case boots the release image under OVMF with `-device intel-iommu`. The command fails if any
case prints a verdict other than its own, or a verdict line without a preflight above it.

| case | machine | must print |
|---|---|---|
| `root-port` | NVMe behind a PCIe root port, 512-byte LBAs: xenon's shape | both PASS, owner by bridge scope, `CONFINED-AT-RATE` |
| `lba-4096` | 4096-byte LBAs | both PASS, `blocks_per 1`, `CONFINED-AT-RATE` |
| `lba-8192` | 8192-byte LBAs | lba FAIL, `SKIPPED` |
| `bypass` | `default_bus_bypass_iommu=on`: the DMAR names no unit for the NVMe | scope FAIL, `UNCONFINED` |

Rehearsed 2026-09-24 on patagonia, all four as expected. QEMU's DMAR has one unit with flags `0x0`
and explicit scopes, so `root-port` exercises the bridge path and `lba-4096` the named-endpoint
path. The catch-all path is exercised by host tests only (`cargo test -p machine_discovery`), since
QEMU cannot present two units. QEMU's rates (2 to 17 MB/s under TCG across five runs on 2026-09-24,
moving with host load) describe TCG only, as `notes/job-mix.md` warns.

## The evening, start to finish

calef needs about thirty minutes at xenon for three boots, more if a preflight fails. A monitor and a USB keyboard (it halts at POST without one, per `notes/x86-uefi-boot.md`),
the FAT32 stick, and a phone for photographs. Nothing new to buy or plug in. The disk was wiped on 2026-09-17; **this boot writes 64 MiB from 1 MiB in**, so if anything
went on that disk since, stop here.

### 0. Before power

- `pgrep -l qemu` on the build machine: nothing should be running while the stick is built.
- xenon's firmware needs no change: `VT for Direct I/O` is enabled (`notes/xenon-firmware.md`).

### 1. Build the image once, and note the commit

```sh
git log -1 --format=%h                        # goes in the Results row
cargo xtask disk-throughput --stage-only
cp target/esp-disk-throughput/EFI/BOOT/BOOTX64.EFI /Volumes/NIFE/EFI/BOOT/BOOTX64.EFI
diskutil eject /Volumes/NIFE
```

`target/esp-disk-throughput`, not `target/esp`: this kernel writes to the disk, and the ordinary
stick must never be it. A netboot (`script/board-netboot --root
target/esp-disk-throughput`) serves the same file once milestone 260 (boot xenon over the network)'s two firmware settings are in.

### 2. Boot, and photograph the last screen

Power on with the stick in, F12 if the firmware does not pick it. The `disk-throughput:` block
follows the tour and ends in `done, halting.` seconds later. Photograph the whole screen: the
`vt-d` lines above the block are the other half of preflight 1.

Watch the screen as the `vt-d        : drhd ... up` lines print: the graphics unit starts
translating, and the display survives only if the graphics RMRR covers its scanout memory. Three
outcomes:

- The tour carries on: the RMRR covers the scanout, and the graphics unit is proven.
- The screen goes black, freezes or tears at that line: the graphics unit is faulting the display.
  Photograph the last readable screen (it should show the `vt-d rmrr` lines), then rebuild from
  before milestone 594, which translates only the catch-all, and run the evening on that: `git checkout "$(git log --merges --grep=milestone/594 --format=%h -1)^1"`.
- The machine resets or hangs with the screen intact: a unit's register writes did not take, and
  the kernel panicked on a screen it cannot redraw. Photograph it; it names the unit.

### 3. Read the two preflight lines before the numbers

Both must read `PASS` for `CONFINED-AT-RATE` (the kernel enforces it). A FAIL is the evening's
most important result; do not skim past it to a number.

### 4. Boots: three, power-cycling between

Record all three; quote the median and the spread, never one boot.

### 5. Optional: the denominator, if a Linux live stick is to hand

The honest comparison is the same disk, window and shape under Linux:

```sh
sudo fio --name=r --filename=/dev/nvme0n1 --direct=1 --rw=read --bs=4k --iodepth=1 \
    --offset=1M --size=64M --ioengine=psync
sudo fio --name=w --filename=/dev/nvme0n1 --direct=1 --rw=write --bs=4k --iodepth=1 \
    --offset=1M --size=64M --ioengine=psync
```

Queue depth 1, 4 KiB, O_DIRECT, one pass: nife's driver's shape. Without it, the verdict answers
"confined, at a measured rate" and leaves "at *real* speed" to judgment; with it, the ratio is the
number. **Never quote nife's figure against a queue-depth-32 Linux figure.**

### 6. Record it

Transcribe each photograph's `disk-throughput:` block into
`bench/xenon-<date>/disk-throughput-boot<N>.log`, commit the photographs beside them, and fill a
Results row. The verdict goes into risk 6's appendix
(`design/fatal-risks/the-confined-driver.md`) by whoever holds `design/`, with this page's caveats
attached.

## What each outcome means

| What the photograph shows | What it means | Where it routes |
|---|---|---|
| `verdict CONFINED-AT-RATE` | a confined EL0 driver moved verified blocks on real silicon at the stated rate | risk 6's decisive experiment ran. Record it. Whether the rate is "real speed" is step 5's Linux ratio's question |
| preflight 1 `FAIL ... drhd X owns it, but it did not come up` | the unit that owns the NVMe was refused | numbers below are labeled `UNCONFINED`, not risk 6's answer; `vt-d ... NOT up` gives the reason. The fix is a kernel lane, not a bench step |
| preflight 1 `FAIL ... no drhd owns it` | no unit's scope names the NVMe and there is no catch-all | same routing. It also means firmware leaves the NVMe untranslated under any OS, worth writing down |
| preflight 1 `FAIL ... the dmar did not fit` | the DMAR held more DRHDs or scopes than `DmarUnits` records | raise `MAX_DRHDS`/`MAX_SCOPES` in `crates/machine_discovery`; the tour's `vt-d` lines say by how much |
| preflight 2 `FAIL  N-byte lbas` and `verdict SKIPPED` | the Micron is formatted with an LBA size this driver cannot serve | a skip, not a pass. Reformatting the namespace (`nvme format` from a Linux live stick, LBA format 0) or teaching the driver PRP lists for larger LBAs is calef's call |
| `verdict SKIPPED: no NVMe controller on the bus` | the bus walk found no NVMe class code | the 2026-09-17 root-port fix regressed, or the firmware hid the device (SATA mode `RAID On` would) |
| `verdict FAILED: ...` with a controller or server step | bring-up or the server failed on real hardware | the line names the phase. A `ControllerTimeout` or `CompletionTimeout` only on silicon is the finding; photograph and stop |
| `verified N of M` with N < M | blocks came back from the wrong place or not at all | a correctness failure, worse than any rate; the verdict is `FAILED`; stop |
| `MEASURED BOOT REFUSED` | the stick carries a kernel and an archive from different builds | rebuild with `cargo xtask disk-throughput --stage-only`, which packs them in order |
| the tour, then nothing after `vt-d` or the scheduler | a release-only defect, or the catch-all unit's default-deny broke something the firmware left running | rebuild with `--debug`; if that boots, the release build is the finding, else see `BUGS` on RMRRs |

## Results

| Date (UTC) | Commit | Boots | Preflight 1 | Preflight 2 | Read B/s | Write B/s | IPC floor | Linux qd1 read/write | Notes |
|---|---|---|---|---|---|---|---|---|---|
| 2026-10-04 | a08efc8dc | 1 | not reached | not reached | | | | | screen tore as the `vt-d` lines printed (`bench/xenon-2026-10-04/boot-a-main.log`) |
| 2026-10-04 | 7ae6d4e15 | 1 | PASS (catch-all `0xfed91000` owns 01:00.0) | not reached | | | | | bring-up `CompletionTimeout` (`boot-b-pre594.log`) |
| 2026-10-04 | 3dfd2e813 | 1 | PASS | not reached | | | | | diagnostic image: VT-d fault reason 0x01 on the admin queue, `ECAP.C` = 0 (`boot-c-diag.log`) |
| 2026-10-04 | `d3dbe8cf253d33f648808393ce89063983583c12` | 1 | PASS | not reached | | | | | with `wbinvd`: fault moves to reason 0x0b, context entry reserved field (`boot-d-wbinvd.log`) |
| 2026-10-04 | `fef2e3206` (built at 414eda9de) | 1 of 3 | PASS (catch-all owns 01:00.0) | PASS (512-byte lbas, 256060514304 bytes) | 281608311 | 458142471 | 1198 ns | not run | `CONFINED-AT-RATE`, 16384 of 16384 verified; screen held (`boot-e-main-clflush-1.log`) |
| 2026-10-04 | `fef2e3206` (built at 414eda9de) | 2 of 3 | PASS | PASS | 271854622 | 474990381 | 1197 ns | not run | `CONFINED-AT-RATE`, 16384 of 16384 verified; screen held (`boot-f-main-clflush-2.log`) |
| 2026-10-04 | `fef2e3206` (built at 414eda9de) | 3 of 3 | PASS | PASS | 237098519 | 457744197 | 1197 ns | not run | `CONFINED-AT-RATE`, 16384 of 16384 verified; screen held (`boot-g-main-clflush-3.log`) |

**Quotable figures, medians of the three boots:** write 458142471 B/s (range 457744197
to 474990381, 3.8% spread); read 271854622 B/s (237098519 to 281608311, 16% spread); one FLUSH
549 us (186 to 1305); IPC floor 1197 ns per round trip. The IPC floor is 13% of a median write
block (8940 ns) and 8% of a median read block (15066 ns). Each is one polled command in flight,
one pass per boot, no warm-up: a lower bound on the Micron, not its speed.

### What the first evening found, 2026-10-04

Neither night-of condition failed (preflight 1 passed on the real DMAR); what failed was unlisted.
**xenon's VT-d units do not snoop the CPU caches when they walk their tables** (`ECAP.C` = 0,
`ecap 0xf050da`), and the kernel never wrote a table back. The controller enabled and fetched its
first admin command from ASQ `0x9e1000`; the unit faulted it with reason 0x01 (root entry not
present) while the CPU read `root[1]` as present. Bus-Master was on at the endpoint and the root
port. QEMU could not show this: its unit reports `C=0` too but reads guest memory directly.

A `wbinvd` image got one fault further, confirming the cause. The unit then faulted the
context entry, reason 0x0b (a reserved field; meaning from memory of the specification). Its upper
half was `0x10002`: domain id 0x100 on a unit whose `CAP.ND` = 2 allows 8 bits. The pre-594 kernel
used the requester id as the domain id; milestone 594 had already replaced that. Nothing else
in `CAP` or `ECAP` constrains the entries written.

The fix is `Unit::publish` in `kernel/src/arch/x86_64/iommu.rs`: a `clflush` per table line
written, then `mfence`. Its BUGS entry has the costs. Main with it passed every bench boot since, screen
held and both units translating, so Boot A's tear was the same defect and the graphics RMRR covers
the scanout. Diagnostic images came from `lane/xenon-nvme-diag-pre594`; `diag`
lines print only under the `disk_throughput` feature.

### Linux qd1 baseline, xenon, 2026-10-04 (step 5, run)

Linux's best qd1 write is 425 MB/s (polled io_uring), so nife's median is about 1.08x; read
latency matches. Claim parity, not a 2x read.
[The runs and their caveats](risk-6-bench-evening/linux-qd1-baseline-2026-10-04.md).

## What this cannot settle, said plainly

It does not measure the device at its rated speed: one command in flight and polled completion
cap it well below the Micron at depth (`components/src/non_volatile_memory_express.rs` `BUGS` says
why). A bad number may be the driver's shape, not confinement's cost; the Linux qd1 ratio tells.

It confines DMA, not interrupts: the server polls and holds no interrupt capability, so interrupt
remapping (off in this kernel, offered by xenon's unit) is off this path.

## BUGS

- **FIXED (2026-10-04): the VT-d tables never reached memory on a unit that does not snoop.**
  See "What the first evening found" above. The fix (`fef2e3206`, #1636) held on silicon: three
  xenon boots on 2026-10-04 read `CONFINED-AT-RATE`, 16384 of 16384
  verified, in the Results table.
- The two-unit route and the RMRR maps first met a real DMAR on 2026-10-04: preflight 1 passed
  (the catch-all owns 01:00.0), both units translated and the screen held on all three boots, so
  the graphics RMRR covers the scanout. That is one machine; elsewhere only host tests over the
  7040's table cover them, since QEMU presents one unit and no RMRR.
- The domain-id width, the `GCMD` read-modify-write and the protected-memory switch-off are code
  paths QEMU does not reach. Milestone 594's block says why each matters on xenon.
- One pass per boot, no warm-up. A second boot is the repeat, and three is this page's floor.
- The client is in the kernel. A client process would add context switches; the server, which
  risk 6 is about, is the same either way.
- Linux's read rate (74 to 188 MB/s on one drive) and its tail are unexplained, and nife's counter
  frequency is unchecked against wall time; the
  [baseline page](risk-6-bench-evening/linux-qd1-baseline-2026-10-04.md) lists the next checks.
