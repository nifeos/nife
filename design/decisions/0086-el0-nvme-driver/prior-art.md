# §86 appendix: the research pass's findings, and what the lane would decide

Five sections of the 2026-09-03 research pass in [§86 (whether an NVMe driver can leave the kernel,
and what capability would let it)](../0086-el0-nvme-driver.md). The main page is complete without
this one, which exists to verify or challenge it. "Above" and "below" mean the main page.

## The hold is spent, and it was measuring the wrong variable

The recommendation above is "option 2, but not yet", held "pending either the JH7110 board bring-up
or VT-d, whichever lands first". VT-d landed on 2026-08-25 and the block above records it. By its
own words the hold is over.

**Do not re-arm it on the JH7110.** The stated reason for waiting was that a second data point tells
you which parts of the contract are QEMU artifacts, and that reason does not survive being looked at:

- **The VT-d exercise confined a kernel-resident driver, so it is not a userspace-driver data point
  and could never become one.** The thing whose confinement it checked *is* the kernel. Every future
  IOMMU data point has the same shape while option 1 stands, so no number of them moves this
  decision. That is the sharp version of the QEMU worry and it is not about QEMU at all.
- Three IOMMUs under QEMU is three emulations. The x86_64 result is genuinely stronger evidence
  for *discovery*, and the block above gives the right reason (ACPI names an ECAM window on real
  hardware exactly as it does on `q35`). It is not stronger for *confinement*: QEMU's `intel-iommu`
  is a model, and its invalidation, IOTLB and fault behavior are the model's.
- **And the board the hold names has no IOMMU at all.** Milestone 143 (silicon IOMMU) exists to
  carry the RISC-V IOMMU driver to hardware, and milestone 16's block says why it is separate: it
  "waits on a board that ships the ratified RISC-V IOMMU spec and no such board exists today"
  (`design/roadmap/0016-real-hardware-iommu.md`). The JH7110 is not that board. So on radon, option 2's
  entire confinement story is absent, and a real-silicon NVMe experiment there confines nothing
  unless something in software does. Waiting for milestone 163 (the JH7110's PCIe root complex) to
  decide §86 would deliver a data point in which option 2 cannot be evaluated.

What actually bears on this decision is readable today, because it is a question about the NVMe
interface and about this tree's existing capability vocabulary. Both were read for this pass, and
both changed the answer.

## The premise is false, and that is the largest single finding

The section above says NVMe "has no such point to stand on", that "the controller fetches 64-byte
commands directly out of driver-written memory", and that "nothing kernel-side sees a command on its
way past". The last clause is wrong, and it is the clause the whole argument rests on.

The NVM Express Base Specification, revision 1.4c (fetched 2026-09-03 from
`https://nvmexpress.org/wp-content/uploads/NVM-Express-1_4c-2021.06.28-Ratified.pdf`; QEMU's `nvme`
device documents itself as implementing version 1.4), section 7.2.2:

> Host software writes the corresponding Submission Queue doorbell register (SQxTDBL)
> to submit one or more commands for processing.
>
> The write to the Submission Queue doorbell register triggers the controller to consume one or more
> new commands contained in the Submission Queue entry.

And section 7.2.1, steps 1 through 3, in order: the host places commands in the queue slots; the host
updates the Submission Queue Tail Doorbell register, which "indicates to the controller that a new
command(s) is submitted for processing"; **then** "the controller transfers the command(s) from in
the Submission Queue slot(s) into the controller for future execution".

That is a doorbell, and it is the structural twin of the one this tree already stands on.
`abi::virtio::NOTIFY`'s own documentation describes the identical mechanism from the other side:
"or `DeviceRefused` if a newly-published descriptor on that queue points outside the driver's DMA
region. On refusal the device is NOT told to go." A command written into an SQ slot is inert until
the doorbell is rung, exactly as a descriptor is inert until the queue is notified.

Three things are true that the premise garbled, and they are worth separating, because two of
them are real constraints and only one of them was stated:

1. Ring placement is already kernel-ownable, through the admin plane alone. An I/O queue's base
   address is named in a PRP field of an admin Create I/O Submission/Completion Queue command
   (`crates/nvme/src/lib.rs::Command::create_io_sq` / `create_io_cq`), and the admin queues' own
   bases live in the ASQ and ACQ registers. A driver that never issues an admin command and never
   touches those registers cannot choose where any ring lives. The section above has this right and
   it is what option 2 is built on.
2. **A mediating kernel must copy, not inspect in place.** Section 7.2.4 says a submission queue slot
   is free for reuse only once a completion reports the head advanced past it, so between the
   doorbell and the fetch the slot is the controller's and the driver's at once. A validator that
   read the driver's slot and left it there would be checking a value the driver can change before
   the controller reads it. This is the same time-of-check hazard that made
   `dma_validator::validate_and_shadow` copy into a kernel-private shadow ring rather than validate
   in place, and it has the same answer.
3. PRP2 can be a pointer to a PRP List, so a validator recurses one level for a transfer that
   spans more than two pages, and SGL-mode commands (CDW0.PSDT) would have to be refused or walked.
   Today neither occurs: `non_volatile_memory_express::prp_pair` answers `None` rather than build a list, and notes/non-volatile-memory-express.md
   records "One namespace, PRP-only, no SGLs, no PRP lists". Milestone 55's bulk path is what would
   change that, and it is the honest cost of the mediating option.

The tree half-knew this and lost it in a parenthesis. notes/non-volatile-memory-express.md's `BUGS` already says a
confined EL0 driver needs a capability "in the `Virtio` capability's mold (or command parsing at the
doorbell, which is the same decision wearing worse clothes)". The doorbell was named. Calling it the
same decision is what buried it, and it is not the same decision: parsing at the doorbell confines
without an IOMMU, and the mold it is being compared to does not.

So the option set above is incomplete. There is a fourth option, and it is the one that makes
NVMe match what §23 (multi-queue DMA confinement) did for virtio.

## What this tree already does, item by item

Question 2 turned out to decide most of the cost. Every material an EL0 `nvme_server` needs already
exists, and the closest analogue is `components/src/gpu_driver.rs`, which is a confined EL0 driver holding
a multi-page DMA region.

| What an EL0 NVMe driver needs | What supplies it today | New surface |
|---|---|---|
| BAR0's doorbell page mapped | `Object::DeviceFrame` plus `abi::page_frame::MAP` | none |
| The DMA region's frames | one `PageFrame` naming the whole run, per §102 (a Frame names a run of pages); `gpu_driver` slot 5 | none |
| The region's **physical** base, because PRP fields carry physical addresses | a spawn argument in `x1`. `components/src/entropy.rs` says it plainly: "Descriptors speak physical addresses; a process knows virtual ones, so the spawner passes this in" | none |
| The controller confined to that run | `crate::iommu::confine`, already called from `kernel/src/non_volatile_memory_express.rs::bring_up` before the controller is enabled | none |
| Completions | `Object::Irq`'s `WAIT` and `ACK` | none |
| The admin plane | stays in `Nvme::new` and `bring_up` | none |

Nothing in that table is speculative. The one place option 2 could still want new surface is the
doorbell, and only under 2b.

## Prior art, fetched rather than recalled

Read on 2026-09-03. Where a claim came back as a search paraphrase rather than a page this lane
pulled, it is marked as such, per the tree's own fabricated-quote scar.

- seL4 hands out `seL4_X86_IOSpace`, `seL4_X86_IOPageTable` and `seL4_X86_Page_MapIO`, which
  map memory into an IO address space assigned to a PCI device
  (`https://docs.sel4.systems/projects/sel4/api-doc.html`). It mediates no command stream, and it is
  explicit that the IOMMU is the whole mechanism: on the seL4 devel list, Gerwin Klein wrote
  "Without an IOMMU, you will need to trust the drivers and the hardware of DMA-capable devices to
  either not use DMA or to use it safely only"
  (`https://lists.sel4.systems/hyperkitty/list/devel@sel4.systems/message/XN7ZN344AKBLF5BIIKP7YU5TUICKPVIV/`).
  The official manual PDF could not be parsed and is not cited here.
- Genode's platform driver keeps enumeration, config space, interrupt routing and, load-bearing
  for this decision, **DMA buffer allocation**: "a device driver must allocate DMA buffers at the
  ACPI/PCI server (while specifying the PCI device the buffer is intended for) instead of using
  core's RAM service to allocate buffers anonymously"
  (`https://genode.org/documentation/release-notes/13.02`). No command inspection.
- Redox is the outlier and the cautionary one. `nvmed` calls `pcid_handle.map_bar(0)`, taking the
  whole BAR including the doorbells, and reads `.physical()` off its own DMA wrappers to program the
  controller (`https://github.com/redox-os/drivers/blob/master/storage/nvmed/src/main.rs` and
  `.../src/nvme/mod.rs`). No IOMMU appears anywhere in it. That is option 3 with nothing underneath
  it, and Redox's own team names the missing IOMMU as a known gap (search paraphrase; the news page
  answered 403 and is not quoted).
- Linux VFIO and SPDK are the mainstream shape. The kernel keeps the IOMMU group as the unit of
  ownership, the DMA-map ioctl, and interrupt registration by eventfd
  (`https://docs.kernel.org/driver-api/vfio.html`). Userspace gets the BAR mapped: SPDK says "User
  space drivers utilize features in uio or vfio to map the PCI BAR for the device into the current
  process, which allows the driver to perform MMIO directly" (`https://spdk.io/doc/userspace.html`).
  Nothing inspects submission queue entries.
- Software mediation of NVMe queues exists, and it stops at the admin queue. "High-performance and
  Scalable Software-based NVMe Virtualization Mechanism with I/O Queues Passthrough" (arXiv
  2304.05148, read at `https://ar5iv.labs.arxiv.org/html/2304.05148`) traps and emulates the admin
  queue. It then bounds-checks PRP addresses and LBA ranges per request and lets the I/O queues run
  through with no host software in the path. MDev-NVMe (USENIX ATC 2018) is described the same way
  in secondary sources; its PDF answered 403 and this lane did not read it, so it is named and not
  relied on.

Two things follow, and they cut in opposite directions. Option 2 is the field's answer: everyone
who has solved this keeps the admin plane and hands over the doorbells. And **option 4 is genuinely
novel**, in the specific sense that this lane found nobody doing inline per-entry mediation of an
I/O submission queue, with the cited paper explicitly avoiding it on cost grounds. Novel is not the
same as wrong, and this tree's own §23 is the same trick at a smaller scale, but a decision for
option 4 should be taken knowing that nobody is going to have measured it for us.

## What this lane would decide, and what is left to calef

Per AGENTS.md, a fork that touches the syscall surface gets options rather than a recommendation, so
this is offered as a reading and not as an answer.

**Two things this pass does settle, and they are not calef's to rule on**: the hold condition above
is satisfied and should not be re-armed. And the premise the argument rested on is false, which puts
a fourth option on the table that was never considered.

The reading. Option 2a is the cheapest honest thing and costs no surface, which makes it hard to
argue against on this tree's own tenets. Option 4 is the one that answers what §86 is ultimately
for: fatal risk 6's decisive experiment is "one real, non-virtio device on real silicon, confined, at
throughput", the silicon this project owns has no IOMMU, and option 4 is the only entry here that
confines without one. Those two are not exclusive; 4 is 2b plus a validator, and 2a is what you build
if you never intend to write the validator.

What is genuinely calef's: whether a new `Object` variant is minted at all (2b and 4) or the
doorbell page is simply mapped (2a). And, if a variant is minted, whether it ships with the
validator (4) or without it (2b), because that choice decides whether this system's confinement
claim for non-virtio devices depends on hardware nobody has yet put on a board.

If the answer is option 1 permanently, that is still a decision worth having, and it wants one extra
line the section above did not have. It means fatal risk 6 has no route left that this project can
run, because the driver whose confinement the risk is about would be the kernel by design rather
than by deferral.
