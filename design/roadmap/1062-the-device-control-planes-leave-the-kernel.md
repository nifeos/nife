---
status: NOT-STARTED
raised: 2026-11-10
milestone_dependencies: 811
decision_dependencies: 4, 23, 86, 130, 251
machine_requirements: none
specific_machine: none
needs_person: yes
---
# 1062. The device control planes leave the kernel

*(The number, title and slug are provisional.)*

Reuse: the shape is the tree's own. Milestone 242 (xHCI) already splits a device this way, and
milestone 811 (the boot services leave the kernel) supplies the grants. No outside code fits a
capability split of nife's own kernel, so this lane searched only the tree.

The kernel holds only what must run at EL1 plus what a confinement argument needs (§4 (kernel
shape)), and confinement means an IOMMU (§86 (whether an NVMe driver can leave the kernel)). A
device's control plane is neither. On a machine with an IOMMU the IOMMU confines the device; on a
machine without one nife trusts the driver wherever its control plane runs. So each control plane
the kernel carries today moves to a userspace process, and the kernel keeps only the confinement:
it binds the device's requester id to its DMA region and grants the device's pages.

Ruled 2026-11-10 (UTC). The order follows the dependencies, and each step is one pull request.

## The work, in order

1. e1000e to the xHCI shape, after milestone 811 (the boot services leave the kernel)'s step 6.
   The kernel confines the controller and grants all of BAR0 and the DMA run to a userspace control
   process. That process keeps BAR0's pages 0 and 5 (reset, MAC address, PHY, filters) and grants
   the queue pages to `net_stack`, the shape milestone 242 (xHCI) has for USB. `kernel/src/e1000e.rs` goes,
   and the kernel stops linking `crates/e1000e`. Waiting for 811's step 6 keeps the starter and the
   control plane from moving twice.
2. The JH7110 clock and reset server. A userspace server holds the shared clock and reset
   window, which serves USB, PCIe, DMA, the security block and Ethernet. It answers per-device
   verbs: a client asks for its own device's clocks and resets, by name, and never touches
   another's. The verbs live in a contract crate both sides depend on (§311 (a contract is
   code, not prose)). The wire details (verb numbers, the request layout, how a client's device is
   named) return to the architect as a separate decision before they freeze. Until then they ship
   provisional. `kernel/src/drivers/jh7110_clock_and_reset.rs` shrinks to what the kernel's own
   JH7110 watchdog (milestone 593 (a wedged kernel resets itself)) needs to turn its clock on, and
   the entropy and Ethernet bring-ups ask the server instead.
3. The DesignWare GMAC control plane, which needs step 2 for its clocks and resets. Its
   userspace control process holds the channel and MAC pages. `kernel/src/designware_ethernet.rs`
   goes, and the kernel stops linking `crates/designware_ethernet`. radon proves it, so this step
   needs a person.
4. The NVMe admin plane, as §86 states it: a userspace admin process holds BAR0 and the admin
   queues and grants the data-path driver its doorbell page and I/O queues. The admin half of
   `kernel/src/non_volatile_memory_express.rs` goes. The split keeps §86's option 4 addable.
5. The JH7110 PMIC route, as §251 (restarting the machine is a kernel object the progenitor
   hands out) states it. A userspace route holds the I2C5 controller's page as a `DeviceFrame`,
   asks the step 2 server for I2C5's clock and reset, and writes the AXP15060. `reboot` calls it
   before `REBOOT`, with a timeout and a fallback to the plain `REBOOT` route.
   `kernel/src/reboot/designware_i2c.rs` and the PMIC half of `kernel/src/reboot/pmic.rs` go.
   radon proves it.
6. The x86 CMOS read, as §130 (how the kernel-resident CMOS RTC reaches the userspace clock
   service) states it: the clock service holds a `PortRange` over ports 0x70 and 0x71 and reads the
   CMOS itself. `kernel/src/arch/x86_64/rtc.rs` goes, and `clock_contract::rtc::CMOS` comes to name
   two ports the service reads. §130's title, and the glosses that cite it, change with the code.
7. The virtio validator becomes a grant the progenitor makes when it starts a driver (§23
   (multi-queue DMA confinement)), as the next section designs it. Ruled 2026-11-10 (UTC), option 3
   of three. Its four pull requests are listed there.

Every new program, process, crate and verb ships with a provisional name.

## Step 7: a mediated or a direct virtio grant

The progenitor, a program, hands each virtio driver one of two grants. The kernel has no mode
flag, no build feature and no boot parameter for it, and the choice can differ per device.

- Mediated. The driver holds the `Virtio` capability, as every virtio driver does today. It rings
  the doorbell with `NOTIFY`. The kernel validates each new descriptor with
  `crates/direct_memory_access_validator`, copies the checked ones into the kernel-private shadow
  page the device reads, and only then rings the real doorbell (`notify` in
  `kernel/src/virtio.rs`).
- Direct. The driver also holds its device's doorbell page as a `DeviceFrame`, and writes the
  doorbell itself, as the NVMe data-path driver does with BAR0's second page. The device reads the
  driver's own rings. Nothing validates a `NOTIFY`, and the driver is trusted, as an NVMe driver
  is on a machine without an IOMMU.

The kernel keeps the validator, the shadow page and the proofs for both grants.

### The objects

The mediated grant is the existing `Virtio` capability, unchanged. The direct grant needs no new
object kind either: the progenitor invokes a new `Virtio` method, `DOORBELL_PAGE` (provisional),
with `GRANT`. The kernel then records the device as direct in its `Device` entry and mints a
`DeviceFrame` for the doorbell page into the caller's table. From then on `SETUP_QUEUE` points the
device at the driver's descriptor table and available ring rather than the shadow, and `NOTIFY`
answers `WrongObject`. The entry's field is how the kernel tells the grants apart at queue setup.
A device stays direct until `unregister`, which also revokes the doorbell page from every holder.

`DOORBELL_PAGE` refuses unless every queue is still unset and the doorbell sits alone on its page.
On virtio-pci the kernel checks that the notify region's page holds no common configuration, ISR
byte or device configuration, and that every queue's doorbell falls on that one page. On
virtio-mmio the doorbell shares a slot with the queue-address registers. On aarch64's `virt` the
slots sit 0x200 apart (`VIRTIO_SLOT_STRIDE`), so one page holds eight devices. The method therefore
refuses every virtio-mmio slot, and every virtio-mmio device gets the mediated grant. Granting
riscv64's whole 0x1000 slot later loosens this compatibly, so the stricter rule ships first.

The new method is a syscall-surface change, so its semantics go into §23 as an amendment the
architect approves.

### With an IOMMU

`register` confines a PCI function before any grant exists, over its DMA region plus the shadow
page, read-only to the device (`iommu::virtio_regions`). A direct grant keeps that domain. The
device can then reach only the driver's region and one zeroed page that the kernel never writes
once `NOTIFY` refuses. So with an IOMMU a direct grant stays confined in hardware, and without one
the driver is trusted. `DOORBELL_PAGE` needs nothing more of the IOMMU case.

### Which grant, by default

Provisional, not yet ruled: the progenitor on the QEMU runners grants direct wherever the device
allows it, which means the PCI functions. Provisional, not yet ruled: a cloud image without an
IOMMU grants mediated. The rule lives in the image's progenitor, never in the kernel. A virtio-mmio
driver is trusted without an IOMMU, as an NVMe driver is. The validator stays as the proved form of
§86's option 4, used only if a cloud target with virtio and no IOMMU becomes real.

### Tests, proofs and starters

- The Kani harnesses and the seven patches in `crates/direct_memory_access_validator/falsifications`
  prove the crate, not a boot, so both run unchanged. `notify` still calls the validator, so the
  kernel's reachability claim (`kani-reach.yml`) holds.
- The kernel's own starters keep the `Virtio` capability alone, which is the mediated grant. That
  covers nine `virtio::register` calls in six files under `kernel/src/user`, among them the system
  tests' disks, the entropy service and the keyboard. So the existing `DmaEscape` tests, and the 25
  system-test files that name virtio, run on the mediated path. aarch64's boot disk is
  virtio-mmio, so every aarch64 boot exercises it as well.
- New tests: a direct PCI disk reads and writes on all three architectures; `DOORBELL_PAGE` refuses
  a virtio-mmio slot, a set-up queue and a second call; `NOTIFY` refuses on a direct device.
- `xtask/src/network_bench.rs` drives e1000e and `xtask/src/disk_throughput.rs` drives NVMe, so
  neither changes. `disk_throughput`'s `bypass` case is the shape of a direct grant with no IOMMU.

### ABI changes

Three, all on the `Virtio` capability in `crates/abi`: the new `DOORBELL_PAGE` method; `SETUP_QUEUE`
returns the queue's doorbell offset within the page on a direct device, and 0 otherwise; `NOTIFY`
on a direct device answers `WrongObject`. No wire format between programs changes, and the
progenitor's `GRANTS` table keeps its slots.

### Prior art, recalled, not re-read

- Xen grant tables. A guest grants a page to the backend domain by a grant reference. With grant
  copy, netback copies each packet through the hypervisor, which checks every reference: the
  mediated shape. A PCI device passed through behind the IOMMU is the direct one.
- seL4 runs drivers at user level. On a platform without an SMMU a driver that programs DMA is
  trusted. In the seL4 Device Driver Framework a virtualiser between client and driver checks each
  buffer address in the queues, so it mediates in userspace and not in the kernel.
- Linux virtio without `VIRTIO_F_ACCESS_PLATFORM` hands the device guest physical addresses and
  trusts it. With the flag and an IOMMU, the DMA API maps each buffer. In a confidential guest,
  swiotlb bounces every buffer through a shared pool: a TDX guest's virtio-blk read lands in the
  pool and the guest copies it into the page cache.

nife differs in two ways. It copies and checks descriptors, not data, with a Kani proof. And the
choice is a grant per device at start, where Linux sets `swiotlb=force` or `iommu=pt` once per boot.

### Cost and effort

Not measured yet. A mediated `NOTIFY` costs a syscall, the validation and a copy of the new
descriptors. A direct one costs an uncached store from EL0. The last pull request measures both on
a PCI disk. The work is medium: about 150 kernel lines, 60 in `crates/user_mode_runtime/src/virtio.rs`,
80 in the progenitor, and 200 of tests.

### Order of work

1. The §23 amendment and the ABI: `DOORBELL_PAGE`'s number and semantics, approved by the
   architect before any code.
2. The kernel half: the `Device` field, `DOORBELL_PAGE` and its refusals, the `SETUP_QUEUE` and
   `NOTIFY` branches, and the revoke at `unregister`, with the kernel tests.
3. The driver half in `crates/user_mode_runtime`: write the doorbell page when the driver holds
   one, and a direct PCI disk system test on all three architectures.
4. The progenitor's per-device grant with the provisional defaults above, and the measurement.

## Done when

1. None of the files that steps 1, 3, 5 and 6 remove exists, the kernel's clock and reset driver
   serves only its watchdog, and `kernel/Cargo.toml` names neither `e1000e` nor
   `designware_ethernet`.
2. The existing gates for each device pass on every architecture it runs on, with no test weakened.
   That means the e1000e, NVMe and virtio gates under QEMU, the clock tests on `x86_64` with a wall
   clock, and radon's Ethernet and reboot bench boots.
3. `reboot` on radon resets the board through the userspace route, and resets it through `REBOOT`
   when the route is absent.
4. `script/fastpath-footprint` reports no change. None of these paths is hot, so a change there
   is a finding.
5. A virtio driver runs on either grant, the kernel's own starters still grant mediated, and CI
   runs the direct and the mediated path on all three architectures.

## Index row

The kernel still carries the control planes of e1000e, the GMAC, NVMe, the JH7110's clock and
reset block and its PMIC, and reads the x86 CMOS itself. None of that must run at EL1, and none of
it confines DMA, which is the IOMMU's job. This moves each one to a userspace process, keeps the
virtio validator as a grant the progenitor chooses per device, and takes about 1,500 kernel lines and
two driver crates out of the trusted base.
