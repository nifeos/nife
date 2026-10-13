---
risk: 6
color: amber
updated: 2026-10-08
---
# Appendix to risk 6: A capability-confined userspace driver cannot drive real hardware at real speed

*An appendix to [`6-a-confined-driver-is-too-slow.md`](6-a-confined-driver-is-too-slow.md) (risk 6). That entry is the claim of
record, and it is written so that a reader can decide what to work on next without opening this
file. This one exists to be verified or challenged: it holds the evidence, the dates, the numbers,
the corrections and the refusals behind the verdict, at the length they need rather than the length
the six-pager has. Where a study has its own home in `notes/` this page links it rather than copying
it. Name provisional (`design/fatal-risks/` and this file's stem); naming is an architect's.*

### The claim

The thing that makes the thesis interesting, drivers outside the kernel behind an IOMMU, does not
survive contact with a real device.

### Evidence today

Milestone 16 (real hardware and IOMMU-backed driver isolation)'s 16b proved IOMMU-backed DMA
isolation against QEMU's emulation of the ratified RISC-V IOMMU, over the PCIe transport of §18 (one
driver, two buses, the seam in the kernel). And milestone 35 (prove the DMA-confinement boundary)
built the DMA validator. All of it is virtio or emulated. The VisionFive 2 boots and its
ratified-IOMMU silicon does not exist (milestone 143).

### The verdict of record

RUN, and as of 2026-09-16 all three of its parts are measured on silicon. The two 2026-09-04 halves
are below; the third was taken on 2026-09-16. This
does not retire the risk, and the reason is in the third bullet and repeated at the foot of this
entry: a TRNG is the smallest real device on the board, and throughput is what a TRNG cannot test.

The risk names three things and they were never one claim. Measured on radon, transcripts at
`target/board/radon-2026-09-04-trng-success.log` and the committed boots named under the speed bullet:

- Confined: yes, 2026-09-03. Milestone 159 (a real hardware entropy source: the JH7110's TRNG)'s
  driver is an EL0 process started from the archive, reaching the JH7110's TRNG through a capability
  that names no device. This is the tree's only confined driver for a real, non-virtio device.
- Drives real hardware: yes, 2026-09-04, reproducibly. `served 32+32 bytes`, two boots, first draws
  `3faa07e1` and `731191ba`, each boot's two draws differing from each other. Reseeded per boot
  rather than a constant in silicon or a stale register file.
- At real speed: MEASURED on silicon, 2026-09-16. 955,223 bytes/s, 64 bytes in 67 us over eight
  `entropy_protocol` round trips, which is about 8.4 us per round trip; bring-up 562 us. Corrected
  2026-10-03 (§216 (fatal-risk facts are correctable, and verdicts are the architect's), from #1495): that figure has no committed transcript. It was cited to
  `bench/radon-2026-09-16/tour-083200.log`, which has no `hw entropy` line, and no committed log
  holds 955,223 (commit `4af8ff980` carries only the number). Four committed boots of radon
  measure the same step: 988,416, 992,248 and 992,248 bytes/s on 2026-09-16 (`jobmix-boot3.log:721`,
  `jobmix-boot5.log:736`, `jobmix-boot4.log:814` under `bench/radon-2026-09-16/`) and 973,384 on
  2026-09-25 (`bench/radon-2026-09-25/soak-8h.log:208`), a spread of about 4%. The instrument is the one built on
  2026-09-10 (the tour reads the timebase around the step and the `hw entropy` line carries three
  figures), and this is the boot that proposal was waiting for.

The QEMU denominator did not survive contact with the board, and that is the finding. It was built
to say that the path itself costs about 250 us per 8-byte exchange with an emulated device that
costs nothing, so that whatever radon spent beyond that would be the JH7110's. radon spends 8.4 us,
which is thirty times *less* than the floor it was supposed to be read against, and the bring-up is
562 us against QEMU's 8069 to 13057 us. So TCG was slower than silicon in both halves and the
subtraction the denominator was for cannot be done. The proposal had already said to distrust the
QEMU bring-up figure; the rate figure turns out to want the same warning.

What the number counts is what the tour's own line says: the round trips, the context switches each
one costs, the driver's poll loop, and the device. It does not count the spawn or the bring-up, and
it is not comparable to a Linux `hwrng` throughput figure, which is a read from an already-running
in-kernel driver with no IPC in it. The honest comparison, against `jh7110-trng.c` on the same
silicon, is interrupt-driven where this driver polls and remains unmeasured.

What it took is worth recording, because none of it was the driver. Milestone 239 (radon's device
tree does not describe the TRNG) found the device tree spells the node with the vendor U-Boot's
`starfive,trng` rather than mainline's `starfive,jh7110-trng`. Milestone 220 (this kernel drives no
clock or reset controller) found the block's clocks gated and its reset asserted, and this kernel
had never programmed either. And milestone 159's own boot tour asked for 32 bytes down a channel
that carries 8. So its success line had been unreachable on any device, working or dead, since the
day it was written and survived because QEMU has no TRNG node to take the other arm.

The remaining part is the one the risk is named for. A driver that serves entropy slowly still
refutes nothing; the claim is about cost, and cost is what has not been measured.

### The decisive experiment

One real, non-virtio device on real silicon, confined, at throughput. The JH7110's GMAC from
milestone 53 (the board's own peripherals: network and storage on real silicon), or NVMe behind
milestone 163 (the JH7110's PCIe root complex), are the candidates.

Journey 3 settles most of this as a side effect, because a framebuffer and a keyboard on real
hardware are real devices.

Two corrections, 2026-09-03, from the §86 research lane. The evidence line above says "All of it is
virtio or emulated", and the first half went stale on 2026-08-15: milestone 53 built a real,
non-virtio NVMe driver, confined by the IOMMU on all three architectures. Still emulated, so the
sentence's conclusion holds; its reason does not.

The second correction is the one that matters. No IOMMU data point can settle this risk while the
NVMe driver is kernel-resident, because the driver whose confinement the risk is about would be the
kernel. That is what DECISIONS §86 (whether an NVMe driver can leave the kernel, and what capability
would let it) decides, which puts §86 on this risk's critical path rather than beside it. And the
board this risk's experiment names has no IOMMU: milestone 143 (silicon IOMMU) exists because no
board shipping the ratified RISC-V IOMMU spec exists today. So a real-silicon NVMe experiment on
radon confines nothing unless something in software does, which §86's research pass found is
possible and had been ruled out on a false premise.

The third correction, 2026-09-04, and it is a result rather than a correction. On radon, a userspace
program brought up the JH7110's TRNG and served a client bytes that were not zero and that changed
between draws. It held two rendezvous capabilities and one page of device memory, with no IRQ
capability, no DMA page and no `Virtio` capability, through a capability that names no device.
Transcript: `target/board/radon-2026-09-04-clock-and-first-entropy.log`, milestone 159.

That splits this risk into three, and two of them are now answered:

- Confined: yes, demonstrated on silicon.
- Drive real hardware: yes, demonstrated on silicon. A TRNG is a small device, and that is worth
  saying plainly rather than glossing: it has no DMA, no interrupt in this driver's path, and one
  register window. It is the *smallest* real device on the board, so it settles "a confined
  userspace process can reach non-virtio silicon at all" and it settles nothing about a device with
  a ring buffer.
- At real speed: measured on silicon 2026-09-16, and the small-device question is closed. 973,384 to
  992,248 bytes/s over four committed boots (the first figure, 955,223, has no committed transcript;
  see the correction at the head of this entry), 8.4 us per round trip, bring-up 562 us. The full figures and what they do and do not
  count are at the head of this entry. Corrected 2026-09-11: this used to read "nothing in the boot
  tour timestamps the step, so the only available clock is a person watching a serial console". And
  that stopped being true on 2026-09-10 when the step began timing itself. The boot that read it
  came five days after that.

The decisive experiment above is unchanged, because throughput is what a TRNG cannot test.

And the machine for it exists, which nobody had established until 2026-09-05. The decisive
experiment is one real non-virtio device on real silicon, confined, at throughput.
[§86](../decisions/0086-el0-nvme-driver.md) was decided on 2026-09-03 and its own research recorded
that no board this project owns has an IOMMU in front of a real NVMe controller. xenon has both. And
its firmware transcription says so precisely. A `Micron 2450 NVMe 256GB` on M.2 PCIe SSD-0, with
SATA in AHCI rather than RAID, *"so the NVMe is a plain PCIe function rather than hidden behind
Intel RST"*. Milestone 87 (the x86_64 bare-metal machine) selected that machine partly for VT-d.

And that machine now boots nife, as of 2026-09-17 (milestone 87, `BUILT`), which removes the last
thing standing between this risk and its decisive experiment that was not code. Its first complete
boot reported `iommu : VT-d drhd at 0x00000000fed90000, root table default-deny, translating` and
`pci : 15 function(s) on the bus`, so the IOMMU this experiment needs came up on its own hardware
rather than under emulation.

Corrected 2026-09-25 (UTC), from pull request #1275 (milestone 261's bench rehearsal). The sentence
above claims more than that boot showed. The kernel brought up the DMAR's first unit and reported a
device confined whenever any unit was translating. On the OptiPlex 7040, which shares xenon's
register addresses, the first unit (`0xfed90000`, flags `0x0`) covers only the integrated graphics.
The catch-all unit there is `0xfed91000`. So the line shows that a VT-d unit came up on xenon, not
that the unit in front of the NVMe did. That is an inference from a sibling machine, not a reading
of xenon's own table, and it stays unverified until milestone 261's bench evening reads xenon's DMAR
(`notes/risk-6-bench-evening.md`, added by #1275).

What it does not touch. No result recorded on xenon asserted that a PCI device was confined. The
2026-09-17 NVMe attempts crashed or skipped, and xenon's entropy source is `rdseed`, which has no
requester id to confine. Under QEMU `-device intel-iommu` presents one unit that owns the whole bus,
so every x86_64 confinement result from QEMU stands. The verdict above rests on radon's TRNG, which
has no IOMMU in its path at all.

What stood in the way was not hardware, it was that the disk held somebody else's Windows, and a
disk this project must not write to is not a disk it can drive. calef confirmed on 2026-09-05 that
the installation is a freshly wiped image from the seller, not anyone's data. He also confirmed that
the machine's own firmware can clear it (Maintenance, Data Wipe, `Wipe on Next Boot`, which covers
M.2 PCIe SSD; `notes/xenon-firmware.md`, IMG_4091).

calef ran that wipe on 2026-09-17, so the disk is this project's to write to.

The driver exists too, as of 2026-09-17: milestone 261 (the NVMe driver leaves the kernel, on the
machine that can finally confine it), §86's option 2a. An EL0 process holding two endpoints, one
page of BAR0 and a run of DMA pages brings a controller from reset through identify to an I/O queue
pair, and serves the block verbs. The IOMMU is the whole of what stops it reaching memory it was not
given. And `kernel/src/user/non_volatile_memory_express_tests.rs` asserts that confinement on every
leg the runner attaches a controller to. Milestone 318 (the NVMe boot test on real geometry) then
rewrote its assertions against the geometry each boot is handed, which is what lets the same case
run on xenon's Micron rather than only on `mknvmedisk`'s 8 MiB image.

So the remaining distance to this risk's decisive experiment is a bench evening, and nothing else.
Every piece is on `main` and the stick is written. Two things still have to be true on the night.
And neither is code: the DMAR's device scope must cover the NVMe function, because a throughput
number from an unconfined device answers a different question. And the controller's LBA size must
give `blocks_per` in `1..=8`, or the server never starts and the line reads `skipped` rather than
`ok`. A skip is not a pass.
