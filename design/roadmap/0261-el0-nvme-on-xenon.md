---
status: BUILT
raised: 2026-09-05
built: 2026-10-05
---
# 261. The NVMe driver leaves the kernel, on the machine that can finally confine it

Built 2026-09-17 under QEMU on all three architectures and closed on xenon 2026-10-04 to
2026-10-05 (status corrected 2026-10-07, when it still read PARTIAL): a confined EL0 process
serves the block contract off QEMU's NVMe and xenon's real one. Minted 2026-09-05 by the maintainer. [§86](../decisions/0086-el0-nvme-driver.md)
was DECIDED on 2026-09-03 and the work it authorizes has had no milestone since, which is
milestone 247's failure class (work identified by a finished piece goes nowhere) applied to a
decision rather than to a block. *(Number provisional until the merge queue lands it.)*

Of the second kind: the machine is on the desk and one thing on it is calef's,
below. The driver itself is buildable and testable under QEMU and does not wait for any of it.

## What §86 decided, so this block does not reopen it

Option 2a, in its own words: an EL0 NVMe server that adds no syscall surface at all, so *"the
only durable commitment is what an `nvme_server`'s spawn contract says, and this tree changes spawn
contracts routinely."* Designed so option 4 can be added later without reshaping the driver, and the
choice between them settled by measurement rather than argument.

Not reopened here: whether a new `Object` variant is minted, and whether a validator ships with
it. §86 (whether an NVMe driver can leave the kernel) says plainly those are an architect's, and
they are option 4's questions, not 2a's.

## One premise of §86 has changed, and it matters to this milestone

§86's reading, written 2026-09-03:

> fatal risk 6's decisive experiment is "one real, non-virtio device on real silicon, confined, at
> throughput", the silicon this project owns has no IOMMU, and option 4 is the only entry here
> that confines without one.

That is no longer true, and nobody had established it when §86 was written. xenon has VT-d and an
NVMe behind it. `notes/xenon-firmware.md` records the machine's own setup UI. A `Micron 2450 NVMe
256GB` sits on M.2 PCIe SSD-0 with SATA in AHCI rather than RAID, *"so the NVMe is a plain PCIe function
rather than hidden behind Intel RST"*, on a machine milestone 87 (bare) selected partly for VT-d. The tour
has already reported `vt-d: drhd 0xfed90000 up, translation enabled (gsts.tes confirmed)` under OVMF.

This does not overturn §86's choice; it strengthens it. Option 4 exists to confine a device on
silicon with no IOMMU. On xenon the IOMMU is present, so option 2a plus VT-d is a confined real
device at throughput, which is risk 6's decisive experiment without the validator option 4 was
reaching for. Option 4 remains what a board without an IOMMU would need, and that is a different
machine's problem.

## What calef has to do, and it is one thing

Wipe xenon's internal NVMe. The disk currently holds a Windows installation, and a disk this
project must not write to is not a disk it can drive. That is what has been standing
between this tree and risk 6's decisive experiment rather than any missing hardware.

- The machine's own firmware does it: Setup, Maintenance, Data Wipe, `Wipe on Next Boot`
  (`notes/xenon-firmware.md`, IMG_4091). It covers internal SATA HDD/SSD, M.2 SATA SSD, M.2 PCIe SSD
  and eMMC; on a 7050 Micro that is this NVMe and nothing else, and it does not touch removable
  media.
- It is not recoverable and cannot be terminated once started, which is the page's own warning.
- Nothing on it is wanted. calef, 2026-09-05: *"The Windows image is freshly wiped. Don't worry
  about it. The system listing told me it would be there and I wouldn't want somebody else's data
  anyways."* So this is a seller's fresh image rather than anyone's data.
- Cost: nothing, and one boot. No purchase, unlike milestone 87's requirements list.

Done 2026-09-17, per `design/fatal-risks/README.md` risk 6. Corrected 2026-09-24: this line still read
"not done" a week later.

## What a lane builds without any of that

An EL0 NVMe server under §86's option 2a, against QEMU's NVMe, which the runner already attaches.
`crates/non_volatile_memory_express` exists (716 lines, host-tested, 25 Kani sites) and `kernel/src/non_volatile_memory_express.rs` is the
in-kernel driver milestone 53 (the board's own peripherals) built. The work is moving the queue mechanics out to a confined process
that is handed a doorbell page and a DMA window and nothing else.

The name is an architect's and a lane should ship a provisional one and say so; `nvme_server` is
§86's passing use, not a ratification. Settled 2026-09-18 under DECISIONS §154 (the acronym test):
the program is `non_volatile_memory_express`, sharing its crate's name.

## What was built, 2026-09-17

The data plane left the kernel and nothing about the syscall surface moved, which is option 2a's
whole claim and is the thing to check first: no new `Object` variant, no new syscall number, no new
method constant. The split is expressible because NVMe 1.4 §3.1 already put a page boundary where the
authority boundary belongs. The controller registers (`CC`, `CSTS`, `AQA`, `ASQ`, `ACQ`) are at
offsets `0x000..0x1000` and the doorbells start at `0x1000`, so "the kernel keeps the admin plane and
EL0 gets the data path" is one page mapped and another not.

| piece | where it is now |
|---|---|
| reset, admin rings by register, enable, IDENTIFY, Create I/O Queue | `kernel/src/non_volatile_memory_express.rs`, EL1 |
| build a command, copy it to the ring, ring the doorbell, watch the phase tag | `components/src/non_volatile_memory_express.rs`, EL0 |
| every piece of arithmetic either half does | `crates/non_volatile_memory_express`, host-tested, Kani-reachable |
| what the process is handed and what it is refused | `kernel/src/user/non_volatile_memory_express_service.rs`'s `Spawn` literal |

What the server holds, and it is the complete list, because a capability system has no ambient
environment: the request endpoint (RECEIVE, `filesystem_protocol::blk`), a readiness endpoint (WRITE,
one message), one page of BAR0 device-typed (the doorbell page at `bar0 + 0x1000`). And the
**data plane's pages** of one confined DMA region (the two I/O rings and sixteen pages of transfer
buffer).

**What it is denied**, each line a decision rather than an omission:

- BAR0's controller register page. It cannot reset the controller, disable it, or repoint the
  admin rings, because it cannot name the page those registers are on.
- The admin plane's pages of the DMA region: the two admin rings and the IDENTIFY buffer. The
  kernel allocated the region and mints every mapping into it.
- Every admin command. Creating a queue is what names the physical address a ring lives at; this
  process never issues one, so where its rings live is the kernel's statement and not its own.
- An `Irq` capability. It polls. That is one authority fewer than `entropy.rs` holds. §86's
  own interrupt finding (this tree runs `intel-iommu` with no `intremap=on` and `gic-version=2` with
  no ITS, so nothing would confine an interrupt a userspace driver aimed) is the reason not to reach
  for MSI-X casually.
- A `Virtio` capability, because there is no kernel-mediated transport here; that absence is
  §86's reason for existing.
- A `DeviceFrame` or `PageFrame` capability. Both windows arrive as `Mapping`s installed before
  `_start` runs, so the server holds no *name* for either and can neither delegate nor revoke them.
  Milestone 159's TRNG driver made the same choice, and its header records why an earlier draft's
  capability slot was describing a thing nobody hands over.
- The physical address of anything but its own data plane. The handoff carries one base, and it
  is page 3 of the region rather than page 0.
- **The initrd, a budget, a filesystem, a network, a clock.**

**The test** is `kernel/src/user/non_volatile_memory_express_tests.rs`, one case, green on aarch64, riscv64 and x86_64. A
client holding one endpoint gets the disk's size, persists two neighboring blocks under different
patterns, reads each one back byte for byte. It is refused a block outside the namespace, gets a flush
count that moves, and is refused an opcode the server has no verb for. **The second pattern is what
proves a write landed where it said and not everywhere**: an earlier draft read the neighbor and
expected zeros. That is a claim about the disk's prior contents, and true only of a freshly made
image. Milestone 318 (the NVMe boot test on real geometry) replaced it with a claim the test itself establishes. Every assertion is
written against the geometry the boot was handed rather than against a constant, so the same case
proves the same things on QEMU's 8 MiB image and on xenon's 256 GB namespace. It asserts the IOMMU
was active,
which matters more here than it did for the kernel-resident driver: with the driver at EL0 the IOMMU
is the *whole* of what stops a compromised server reaching memory it was not given.

**The prover found a bug that only a real controller could have, and it is the best argument in
this milestone for keeping the logic in the crate.** A harness asserting that an accepted handoff's
doorbell offsets stay inside the one page of BAR0 the server is mapped failed in under a second, on
`CAP.DSTRD = 15`. Doorbell N sits at `0x1000 + N * (4 << DSTRD)`, so a wide stride scales the
doorbell *file*, and queue 1's submission tail lands a quarter of a megabyte past the window. QEMU
reports 0 and every hardware implementation §86 cites is expected to, so **no amount of testing on
any machine this project owns could have found it**. On a controller that did report a wide
stride it would have presented as a panic inside the disk driver rather than as a diagnosis. It is
now `non_volatile_memory_express::MAX_DSTRD`, refused in two places: `Handoff::unpack`, so the data plane cannot be built
around it. And the kernel's own bring-up, so such a controller fails loudly at EL1 naming itself
rather than starting a process that cannot address its own doorbells.

**What `crates/non_volatile_memory_express` gained**, because the rule is that logic stays where the prover reaches it:
`Handoff` (the three spawn scalars, packed and unpacked), `Handoff::holds_block` (the range check),
`Handoff::transfer_command` (the whole of what the data plane computes), and `Command::flush`. Three
new Kani harnesses cover the handoff's round trip, that an accepted handoff's doorbell offsets stay
inside the one mapped page, and that no caller-supplied `u64` can wrap into a command for a block the
namespace does not have.

## The bench boot, built 2026-09-24 without the machine

`cargo xtask disk-throughput --stage-only` stages the bench image; the boot prints two preflight
verdicts, measures, and ends in one verdict line. Procedure, expected output and outcomes:
[notes/risk-6-bench-evening.md](../../notes/risk-6-bench-evening.md). Building it found that the
kernel brought up the DMAR's first VT-d unit, very likely xenon's graphics unit, and called any
device confined while any unit translated; that page's first night-of condition has the evidence
and the fix.

## The first silicon runs, 2026-10-04

Three boots, transcribed in `bench/xenon-2026-10-04/` from calef's photographs. Main (milestone
594 in) tore the screen as the VT-d units came up. The image from before milestone 594 passed
preflight 1, the catch-all unit `0xfed91000` owning the NVMe at 01:00.0, then failed bring-up with
`CompletionTimeout`. A diagnostic image found why. The catch-all unit does not snoop the CPU caches
(`ECAP.C` = 0) and the kernel never wrote its VT-d tables back, so the controller's first admin
fetch faulted on a root entry the unit read as absent. The fix is in
`kernel/src/arch/x86_64/iommu.rs` (`Unit::publish`); the reading, and why QEMU could not show it,
is in notes/risk-6-bench-evening.md.

Main with that fix then passed all three bench boots: both preflights PASS, 16384 of 16384
blocks verified each time, `CONFINED-AT-RATE`. Medians: write 458142471 B/s, read 271854622 B/s
(16% spread across boots). Queue depth 1, polled, one pass, and no Linux comparison yet; the
bench note's Results have the spread and the caveats.

## The proof that this milestone worked

**A confined EL0 process drives xenon's real NVMe at a measured throughput, with VT-d translating
its DMA**, photographed, since xenon has no serial console this project can read.

Under QEMU the same driver working is the prerequisite and not the proof: milestone 16b already
proved IOMMU-backed isolation against emulated silicon, and risk 6's open clause is
about a real device at real speed.

## BUGS

*Reviewed 2026-09-17 against what the build learned, and since.*

- Answered 2026-10-04: xenon's DMAR gives the NVMe to the catch-all unit, and preflight 1
  passed on the real table. The unit then failed to read its own tables (above), so confinement
  on silicon is proven only in the negative so far: the unit did fault the device.
- Measured on xenon on 2026-10-04, but at queue depth 1, polled, one pass per boot. The figures
  are a lower bound on the device. Linux `fio` ran on the same disk that evening
  (`bench/xenon-2026-10-04/linux-fio.log`); fatal risk 6 owns the comparison.
- xenon halts at POST without a keyboard, so every boot here is attended until the two settings
  milestone 260 names are changed. That makes an iteration loop expensive in exactly the way the
  netboot work was meant to fix. Unchanged, and it is now the main cost of the remaining step,
  because the software side no longer needs iterating.
- `crates/non_volatile_memory_express`'s Kani harnesses cover queue mechanics, not confinement, and that is still true
  of the three this milestone added. They prove the handoff round-trips, that the doorbell offsets
  an accepted handoff produces stay inside the one page mapped, and that no block outside the
  namespace becomes a command. The first two are *about* the confinement's arithmetic rather than
  about the confinement, which is a hardware claim no prover reaches. **The confinement claim's own
  test is now the escape test**. A confined EL0 server aims a PRP outside its region and the IOMMU
  refuses it, with a replayable falsification that widens the domain and turns the test red
  (`a_confined_el0_server_cannot_dma_outside_its_region`, done 2026-10-04, in `## Follow-on`). What
  earlier stood here, the `confined_by_iommu` assertion alone, was the decorative case milestone 202
  (every confinement test is a ritual until somebody breaks the confinement and watches it fail) was
  minted against.
- A confined driver can still aim DMA inside its own confinement. The IOMMU bounds the controller
  to the whole allocation, admin rings included, because the controller fetches from both halves; so
  a server that computes a PRP backwards from its own base can make the controller overwrite the
  admin submission ring. Not an escape, and §86's option 2a states it in those terms; option 4's
  doorbell validator is what closes it. Recorded beside the feature in
  `kernel/src/user/non_volatile_memory_express_service.rs`'s and `components/src/non_volatile_memory_express.rs`'s `BUGS`.
- **With `CAP.DSTRD` = 0 the doorbell page carries the admin doorbells too**, so the server can ring
  the admin queue. It cannot *write* the admin submission ring, so the worst available is making the
  controller re-fetch slots the kernel wrote or never filled: the same class as the entry above.
  §86 predicted this exactly and called stride 0 "the expected doorbell stride value" for hardware,
  so xenon is unlikely to differ.

## Follow-on

- **Done.** The bench step that answers fatal risk 6, 2026-10-04: three boots of main with the
  table write-back, photographed, all `CONFINED-AT-RATE` with the NVMe owned by the catch-all
  unit (`bench/xenon-2026-10-04/`, read by
  [notes/risk-6-bench-evening.md](../../notes/risk-6-bench-evening.md)).
- **Milestone 594.** Milestone 594 (every VT-d unit translates its own devices): every unit up,
  each device routed to its owner, RMRRs identity-mapped (number provisional, promoted 2026-09-25).
- **Milestone 651.** Milestone 651 (the NVMe boot test on a machine whose IOMMU does not own the controller). `design/roadmap/0651-the-nvme-test-on-a-machine-whose-iommu-does-not-own-it.md`:
  the NVMe boot test on a machine whose IOMMU does not own the controller, as the replayable
  falsification of `confined_by_iommu`.
- **Done.** (2026-10-04) A replayable falsification for the confinement claim. A test-only role of
  the server (`run_dma_escape`, wired by `start_dma_escape`) aims a PRP outside its DMA region in
  both directions, the way `block_driver`'s attacker roles do for virtio. The test is
  `a_confined_el0_server_cannot_dma_outside_its_region`, green on all three ISAs: the IOMMU faults
  on the victim frame and its canary is untouched. Its patch widens the controller's domain to the
  victim (`ESCAPE_TARGET`) and both assertions go red, replayed 2026-10-05. The role is compiled
  only under `components/confinement_attackers`, which only `cargo xtask test` turns on, so a
  shipped server does not carry it.
- **Recorded.** A transfer is one NVMe command per filesystem block, even for a sixteen-block
  request, because `non_volatile_memory_express::physical_region_page_pair` refuses anything needing a PRP list. The virtio block server
  issues one request for the same range, so this server is slower on bulk by construction. In
  `components/src/non_volatile_memory_express.rs`'s `BUGS`.
- **Recorded.** The server cannot read `CSTS`, so a hung controller presents as a timeout rather
  than as `CSTS.CFS`. A worse diagnostic than the kernel-resident driver gave, and the direct price
  of not mapping the controller register page. Same `BUGS` section.
- **Recorded.** One command in flight, inherited from the driver this replaced, so any throughput
  measured against this server is a lower bound on the device rather than a measurement of it.
  `notes/non-volatile-memory-express.md` and the same `BUGS` section.
- **Milestone 421.** `block_roster` still cannot list the NVMe disk: §86 named its NVMe transport
  kind as blocked on who owns the controller, and that is now answered (a process does), so the wire
  shape can be decided. Not taken here because it is something two programs agree on, which is the
  expensive category.
- **Done.** The program's name. `nvme_server` was provisional in both halves' headers, carried
  from §86's passing use of it. calef settled it on 2026-09-18 under DECISIONS §154: the program is
  `non_volatile_memory_express`, sharing its crate's name because
  `non_volatile_memory_express_server` is 34 bytes against `nifefs`'s 32-byte `NAME_LEN`. Performed
  the same day; the argument is in the program's own header.

## Index row

§86 was DECIDED 2026-09-03 and its work had no milestone. Built 2026-09-17 under QEMU on all three
architectures: the data plane is an EL0 process holding one page of BAR0 and a confined DMA window,
at zero new syscall surface. What remains is the machine, and one thing on it is calef's: wiping the
disk
