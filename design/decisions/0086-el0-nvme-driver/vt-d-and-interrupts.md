# §86 appendix: what the VT-d exercise needed, who may forge an interrupt, and reversibility

Three passages of [§86 (whether an NVMe driver can leave the kernel, and what capability would let
it)](../0086-el0-nvme-driver.md). The main page is complete without this one, which exists to verify
or challenge it. "Above" and "below" mean the main page. The first section continues that page's
"Recommendation" after its paragraph on the VT-d exercise. Three milestones are cited here by number
alone. They are milestone 87 (the x86_64 bare-metal machine), milestone 227 (a GICv3 driver, because
GICv2 boots and silently loses every interrupt) and milestone 261 (the NVMe driver leaves the
kernel, on the machine that can finally confine it).

## Recommendation

Getting there took two fixes, neither of which is a confinement-contract difference between VT-d
and the other two IOMMUs, and both recorded in full in `notes/non-volatile-memory-express.md`'s new "What x86_64 needed
that the other two did not" section:

- `kernel/src/pci.rs::place_bars` trusted any nonzero BAR as already placed and already mapped, true
  on the two device-tree architectures (nothing runs before this kernel to place one). And that is
  false on x86_64's PVH boot, where QEMU resets the NVMe device's BAR0 to a live address of its own
  choosing, unrelated to the kernel's own mapped BAR window. Fixed to check the existing address
  against the window this kernel actually mapped rather than against zero.
- The frame allocator (`memory::bring_up_page_frames`) and the direct map
  (`arch::x86_64::mmu::map_firmware_regions`) both sized themselves from the e820 map's `usable` RAM
  entries alone. Attaching VT-d and NVMe together grows the ACPI tables QEMU parks above the top of
  guest memory enough that the adjacent `reserved` entry swallows the last few hundred bytes of the
  initrd. (It is placed by the PVH loader at a fixed offset below the top of memory, sized for a
  smaller device set.) Fixed to size the bitmap from whatever `forbidden` reaches past RAM's own
  end, and to map the initrd's recorded bounds explicitly regardless of how the memmap classified
  them.

Both are the kind of gap only a real, non-virtio DMA device on a real PVH boot could have found
(virtio's BARs and a bare boot's initrd never got close to either edge). And both would in principle
also bite a real UEFI x86 machine (milestone 87), which is why neither fix is
architecture-conditional. They check against what is actually true (which window is mapped, which
bytes the kernel has claimed) rather than against which architecture is running. Neither changed the
driver's shape, the syscall surface, or the confinement contract itself.

With this data point in, the two data points §86 was waiting on are the JH7110 board and the
VT-d/NVMe exercise; the second is now done. This section still does not decide option 1 versus
option 2; it reports that the confinement claim itself has now been checked, not merely built, on
all three of this tree's targeted architectures.

If the answer is option 1 permanently, nothing is blocked, and notes/non-volatile-memory-express.md's BUGS entry becomes
the standing record. If it is never decided, the driver silently becomes load-bearing kernel code,
which is how a microkernel stops being one; that is the failure this entry exists to prevent.

## The axis nothing in this section has named: who may forge an interrupt

VFIO refuses to hand a device to an untrusted userspace driver on a machine without interrupt
remapping. And the escape hatch is a module parameter named `allow_unsafe_interrupts` (mechanism
confirmed by search paraphrase, not by a fetched kernel source file; the driver-api page says only
the general "Many modern systems now provide DMA and interrupt remapping facilities to help ensure
I/O devices behave within the boundaries they've been allotted"). The reason is that an MSI or MSI-X
message is a memory write to an architecturally special address, so plain DMA remapping does not
confine it. A driver that can write the MSI-X table can aim an interrupt wherever the platform will
deliver one.

**Every option here that maps any part of BAR0 to EL0 has to answer where the MSI-X table lives and
who may write it, and this tree has never asked.** Measured, not asserted:

- `helpers/qemu-runner-x86_64.sh` sets `IOMMU="-device intel-iommu"` with no `intremap=on`, and
  interrupt remapping is still on. Milestone 317 (the interrupt-remapping flags, and where MSI
  confinement actually lives) read `ECAP` from inside the guest, QEMU 11.1.1, `q35`/TCG: `-device intel-iommu` gives `0xf00f4a` with `IR` **set**;
  `intremap=on` gives the identical `0xf00f4a`, a no-op; only `intremap=off` clears it, at `0xf42`.
  QEMU's `intremap` defaults to `auto`, which resolves **on** with no in-kernel irqchip.
  **So the capability was offered in every boot this tree has ever run, and the kernel never writes
  `GCMD.IRE`.** `kernel-irqchip=split` is not required either; that is a KVM constraint, and all
  three irqchip settings start `intremap=on` with no diagnostic.
- `helpers/qemu-runner-aarch64.sh` uses `gic-version=2`, which has no ITS, so there is no MSI
  translation path on that machine either. **Confirmed 2026-09-18, and it cannot simply be
  switched.** `gic-version=3` moves `reg[1]` from the GICC to the GICR and adds `its@8080000`. But
  `memory::init` matches the interrupt controller by its `intc@` name prefix and never reads
  `compatible`, so a GICv2 driver takes the redistributor and the machine gets zero timer ticks (3
  of 5 self-tests against 5 of 5 at `gic-version=2`). That is milestone 227's territory, and its
  item 1, read `compatible` and report what was found, turns a silent wrong answer into a refusal
  and is far smaller than 227 itself.
- `riscv64` was missing from this list, and it is the one architecture where the mechanism is
  already reachable. Added 2026-09-18. The RISC-V IOMMU puts MSI translation inside its own
  device context rather than behind a separate feature or a separate device:
  `arch/riscv64/iommu.rs` reads `CAP_MSI_FLAT` and widens the device context from 32 to 64 bytes
  when the IOMMU reports it. Measured: `CAPS = 0x78c2cf4f10`, `MSI_FLAT` set, so the extended
  context is live and `attach` writes all four MSI words zero. **No flag is needed and none exists.**
  The inversion is worth stating: riscv64 is confined in the emulated case this project *can* test
  and untestable on the silicon it owns, since radon has no IOMMU at all. That is the exact reverse
  of x86_64's position now that xenon boots, and milestone 143's 2026-09-17 survey found a board
  with a ratified IOMMU buyable at about $299, which would close that gap and this one together.
- The NVMe driver never touches MSI-X. notes/non-volatile-memory-express.md's `BUGS` says so: "The controller is created
  with IEN=0 and no MSI-X table is touched." Still true after milestone 261 moved the driver to
  EL0 on 2026-09-17: it polls, holds no `Irq` capability, and the controller is still created
  `IEN=0` naming no vector.

So the gap is latent rather than live, and it becomes live the moment a driver leaves the kernel and
wants interrupts instead of polling. A driver has since left the kernel (milestone 261's EL0 NVMe
server, 2026-09-17) and it polls, so the gap is still latent by choice rather than by accident.
`notes/interrupts.md` records that this tree's polling is a confinement choice wearing a performance
choice's clothes.

And this section's own reasoning needs one correction, which is the point of the amendment above.
It treated the gap as unexercisable because the platforms did not offer the mechanism. Two of three
do. What is missing is a kernel that enables it, which is a smaller and far more answerable thing
than a platform that lacks it. calef declined to mint the MSI-X ownership decision on 2026-09-17 for
want of an experiment; milestone 317 built the experiment the next day, and the question can now be
prototyped rather than argued. riscv64 makes it cheapest: the mechanism is one mode field in a
structure `attach` already writes, so it needs neither a new capability nor a new driver.

The parity shape belongs in whatever this becomes (DECISIONS §19): x86_64 and riscv64 can offer
interrupt remapping, aarch64 cannot until milestone 227, so any claim built on it starts with a
recorded gap rather than acquiring one later. Whatever this section settles has to say who owns the page
holding the MSI-X table, because that is the one part of the confinement claim an IOMMU doing DMA
remapping does not cover.

It is work rather than argument, so it has a home. notes/confinement-claims.md now carries it as a
fifth claim that is stated nowhere, beside the timing and device-values entries milestone 202 put
there for the same reason, with the two runner flags that would make it exercisable at all. A
proposal file under `design/roadmap/proposals/` would be the better home and was written first.
`script/roadmap --check` rejects that directory on `main` today, because the convention lands with
milestone 247 (follow-on work named by a finished milestone goes nowhere) and has not merged. When
it does, this belongs there.

## Reversibility, and who has acted

- Option 1 commits nothing. Fully reversible.
- Option 2a adds no syscall surface at all, so the only durable commitment is what an
  `nvme_server`'s spawn contract says, and this tree changes spawn contracts routinely.
- Options 2b and 4 add an `Object` variant and its methods. The precedent says these grow
  compatibly: §23 added a queue argument to `SETUP_QUEUE` and `NOTIFY` without a new syscall and
  without changing the disk's ABI.
- The genuinely expensive thing is the one this section already named, and it is not the
  capability. It is the wire shape `block_roster` grows for an NVMe transport kind, because that is
  something two programs agree on and its content depends on who owns the controller.
- Who has acted: nobody outside this repository. The only named future consumers are
  `block_roster` and milestone 55's backend, both unbuilt.
