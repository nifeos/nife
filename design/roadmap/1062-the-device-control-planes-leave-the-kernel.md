---
status: NOT-STARTED
raised: 2026-11-10
milestone_dependencies: 811
decision_dependencies: 4, 23, 86, 130, 251, 320
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
   window, which serves USB, PCIe, DMA, the security block and Ethernet. It answers the
   device-level verbs of §320 (the JH7110 clock and reset wire): the badge is the device, and a
   client brings up, reads and rates its own device's clocks and resets and never touches
   another's. No verb turns a device off. The verbs live in a contract crate both sides depend on
   (§311 (a contract is code, not prose)), at interface id 24. The names ship provisional.
   `kernel/src/drivers/jh7110_clock_and_reset.rs` shrinks to what the kernel's own
   JH7110 watchdog (milestone 593 (a wedged kernel resets itself)) needs to turn its clock on, and
   the entropy and Ethernet bring-ups ask the server instead.
3. The DesignWare GMAC control plane, which brings its clocks and resets up through step 2's
   server with §320 (the JH7110 clock and reset wire)'s bring-up verb. Its
   userspace control process holds the channel and MAC pages. `kernel/src/designware_ethernet.rs`
   goes, and the kernel stops linking `crates/designware_ethernet`. radon proves it, so this step
   needs a person.
4. The NVMe admin plane, as §86 states it: a userspace admin process holds BAR0 and the admin
   queues and grants the data-path driver its doorbell page and I/O queues. The admin half of
   `kernel/src/non_volatile_memory_express.rs` goes. The split keeps §86's option 4 addable.
5. The JH7110 PMIC route, as §251 (restarting the machine is a kernel object the progenitor
   hands out) states it. A userspace route holds the I2C5 controller's page as a `DeviceFrame`,
   brings I2C5's clocks and reset up through the step 2 server (§320 (the JH7110 clock and
   reset wire)), and writes the AXP15060. `reboot` calls it
   before `REBOOT`, with a timeout and a fallback to the plain `REBOOT` route.
   `kernel/src/reboot/designware_i2c.rs` and the PMIC half of `kernel/src/reboot/pmic.rs` go.
   radon proves it.
6. The x86 CMOS read, as §130 (how the kernel-resident CMOS RTC reaches the userspace clock
   service) states it: the clock service holds a `PortRange` over ports 0x70 and 0x71 and reads the
   CMOS itself. `kernel/src/arch/x86_64/rtc.rs` goes, and `clock_contract::rtc::CMOS` comes to name
   two ports the service reads. §130's title, and the glosses that cite it, change with the code.
7. A virtio driver starts with a mediated or a direct grant (§23 (multi-queue DMA confinement)),
   as the next section designs it. Ruled 2026-11-10 (UTC). That section lists its four pull
   requests.

Every new program, process, crate and verb ships with a provisional name.

## Step 7: a mediated or a direct virtio grant

Ruled 2026-11-10 (UTC). The starter that spawns a virtio driver grants its device in one of two
ways, and the grant decides how the driver reaches the device. The kernel has no mode flag, no
build feature and no boot parameter for it, and the choice can differ per device.

- Mediated. The driver holds the `Virtio` capability, as every virtio driver does today.
  `kernel/src/virtio.rs` keeps the device's registers, places the descriptor table and available
  ring in a kernel-private shadow page, validates each new descriptor on `NOTIFY` with
  `crates/direct_memory_access_validator`, and only then rings the device. Nothing in this path
  changes.
- Direct. The driver holds its device's register pages as plain device frames, listed on its
  startup page, and its DMA region. It resets the device, negotiates features, sets up each queue
  in the common configuration and writes its doorbells itself. The xHCI driver does the same with
  its register file (milestone 242), and the NVMe admin process with BAR0 (step 4). The kernel
  holds no `Device` entry and no shadow page for it, and validates nothing. The IOMMU domain is the
  confinement.

The direct grant applies to virtio-pci devices only while the runners keep virtio-mmio. On
aarch64's `virt` the virtio-mmio slots sit 0x200 apart (`VIRTIO_SLOT_STRIDE`), so one 4 KiB page
holds eight devices' registers, and no IOMMU fronts virtio-mmio on any machine nife runs on.

A new `Virtio` method that switches a device to direct, or a new object kind for a direct
transport, would each add to the syscall surface what a grant of frames already says, so the
design uses neither.

### What the starter carves out

`pci::bring_up` has already placed the function's BARs, enabled decoding and bus mastering, armed
its interrupt and read its virtio capability list. For a direct grant the starter then does four
things, in this order.

1. It confines the function with `iommu::confine` over the driver's DMA region alone, before any
   queue exists. It maps no shadow page, because none exists.
2. It maps, device-typed, the pages that hold the four virtio-pci structures the capability list
   names (common configuration, notify, ISR and device configuration, virtio 1.2 section 4.1.4),
   and nothing else of the BARs. A page that also holds the function's MSI-X table or pending-bit
   array is never mapped, as `PciXhciDevice::withheld` keeps them from the xHCI driver, and a
   function whose structures share a page with either gets the mediated grant. The structures lie
   in the function's own BARs, so no mapped page holds another device's registers.
3. It writes the startup page: one window for each run of register pages and one for the DMA
   region. The handoff carries what a window cannot: each structure's byte offset in its window,
   `notify_off_multiplier`, the MSI-X table entry the kernel armed (or none), and the region's bus
   address. That address is the physical one, because every domain is an identity map
   (`kernel/src/iommu.rs`). The window kinds and the packed handoff live in a new
   `driver_process_contract::virtio` module (provisional) that both sides use (§311 (a contract is
   code, not prose)).
4. It grants the `Interrupt` capability, as it does for a mediated driver.

The region and its domain live as long as the function does, as the xHCI driver's region does. The
starter allocates the region zeroed and never returns it to the allocator while the domain maps it,
so a driver that dies with a queue running leaves a device that can reach only that region. A
driver started on the same function resets the device first, by writing 0 to `device_status`
(virtio 1.2 section 2.1). A mediated device keeps its own reset in `virtio::unregister`.

### Why the IOMMU is enough

The direct driver writes every ring address the device reads. The domain maps only the driver's
region, so a descriptor, ring or buffer aimed anywhere else faults at the IOMMU and never reaches
memory (§20 (IOMMU-backed DMA isolation), §86 (whether an NVMe driver can leave the kernel)). The register
pages give the driver nothing beyond its own function. The common configuration selects that
function's features, queues and MSI-X table entries, and an entry the kernel did not arm stays
masked, which is its reset state. Configuration space stays with the kernel, so the driver cannot
move a BAR, change its requester id or reach the `VIRTIO_PCI_CAP_PCI_CFG` window.

One condition holds the argument up. A virtio device may treat a driver that declines
`VIRTIO_F_ACCESS_PLATFORM` as one whose addresses are physical and untranslated (virtio 1.2
section 6). The runners attach every virtio-pci function with `iommu_platform=on` (§20). There
QEMU sends the function's DMA through the IOMMU, and it refuses `FEATURES_OK` to a driver that
declines the bit (`reset_to_features_ok` in `kernel/src/virtio.rs` records it). So a direct grant
counts as confined only when the function offers `ACCESS_PLATFORM` and the active IOMMU owns its
requester id (`iommu::Scope::is_confining`). Without both, nothing confines a direct driver, and
nife trusts it as it trusts an NVMe driver on a machine without an IOMMU.

### Interrupts

A direct driver takes the same line or vector a mediated one does.

- On `x86_64` the kernel arms MSI-X table entry 0 at bring-up (`program_msix` in
  `kernel/src/pci.rs`). For a mediated device the kernel writes that entry into
  `queue_msix_vector` at each `SETUP_QUEUE`. A direct driver writes it itself, from the handoff,
  into `queue_msix_vector` and `config_msix_vector`.
- On aarch64 and riscv64 every PCI function takes INTx, since `alloc_msi_vector` answers `None`.
  The driver reads the ISR byte from its own page, which acknowledges the interrupt and deasserts
  the line, as `Transport::read_reg` does for a mediated device.

A line routes to one driver (`kernel/src/sched/interrupt_route.rs`), so a direct driver meets the
limit a mediated one meets, and the work that lets several drivers share an interrupt line lifts
it for both. Until that lands, a direct driver's test reads its used ring before it blocks, as the
entropy driver does.

### What stays in `kernel/src/virtio.rs`

All of it, for the mediated grant: both transports, `register`, `setup_queue`, `notify`, the
validator adapter, the shadow page, the reset in `unregister`, `release_owned_by` and the test
hooks. A direct function never reaches `virtio::register`, so the device table, its
`MAXIMUM_DEVICES` and the shadow frames count mediated devices only. The Kani harnesses and the
seven patches in `crates/direct_memory_access_validator/falsifications` prove the crate, not a
boot, so both run unchanged, and `notify` still calls the validator, so `kani-reach.yml` holds.

### Which grant, by default

Open, for the architect, and not ruled: which grant a starter gives a virtio-pci function that can
take either, on a machine whose IOMMU confines it and on one without. Until that ruling every
starter keeps the mediated grant, which is the kernel's behavior today, and the direct grant runs in
its own tests. Whatever the ruling, the rule lives in the starter and never in the kernel.

### Prior art

- nife's xHCI driver (milestone 242) holds its register file less the MSI-X pages, and a DMA region
  the kernel confined first. The kernel refuses to start it unconfined (`Absent::Unconfined` in
  `kernel/src/extensible_host_controller_interface.rs`). The NVMe data-path driver holds BAR0's
  doorbell page (`kernel/src/user/non_volatile_memory_express_service.rs`), and step 4 gives the
  admin process the rest of BAR0. A direct virtio driver has the same shape.
- Genode. The platform driver hands a driver its device's I/O memory, an interrupt service with
  the routing applied, and DMA buffers. On NOVA every DMA goes through the IOMMU, keyed by the
  function's bus, device and function, so the device reaches only its DMA buffers (read: Genode
  Foundations 23.05, "Device drivers").
- Linux VFIO. A userspace driver maps the device's regions at the offsets the device descriptor
  lists and receives interrupts through eventfds (`VFIO_DEVICE_SET_IRQS`). The IOMMU group is the
  unit of isolation, because DMA is the greatest risk a userspace driver poses (read:
  docs.kernel.org, "VFIO"). Userspace drives a virtio-pci function bound to `vfio-pci` exactly
  so.
- DPDK's virtio poll-mode driver binds the function to `vfio-pci`, in VFIO's no-IOMMU mode where a
  machine has no IOMMU, which trusts the driver (read: the DPDK guide, "Virtio"). That it maps the
  modern capabilities and writes the common configuration and the doorbells itself comes from
  memory, not from a reading.

nife differs in two ways. A starter chooses the grant per device when the driver starts, not
once per boot or per binding. And a device the starter will not hand over keeps a mediated path whose
validator has a Kani proof.

### An example

The `x86_64` runner attaches `virtio-blk-pci,iommu_platform=on` behind `intel-iommu`. A starter
that grants it direct confines its requester id to the driver's DMA region and maps the pages of
the four structures. QEMU places them at 0x0, 0x1000, 0x2000 and 0x3000 of BAR4, and the MSI-X
table in BAR1; that layout comes from memory, and the starter reads the capability list rather than relying on
it. It lists the pages on the startup page and passes `notify_off_multiplier` 4 and MSI-X entry 0.
The driver writes 0 to `device_status`, accepts `VERSION_1` and `ACCESS_PLATFORM`, selects queue
0, writes ring addresses inside its region, enables the queue and sets `DRIVER_OK`. To submit a
read it publishes the descriptor and stores the queue number at `queue_notify_off * 4` in the
notify page: one store, no syscall. A descriptor aimed at kernel memory faults at the IOMMU, where
the mediated grant refuses it at `NOTIFY` with `DmaEscape`.

### Tests, proofs and starters

- The kernel's own starters keep the mediated grant: nine `virtio::register` calls in six files
  under `kernel/src/user`, among them the system tests' disks, the entropy service and the
  keyboard. So the existing `DmaEscape` tests, and the system-test files that name virtio, run on
  the mediated path, and every aarch64 boot exercises it through the virtio-mmio boot disk.
- New: a direct disk reads and writes on `x86_64`, and on aarch64 and riscv64 through the PCI
  functions the test legs already attach behind the SMMUv3 and `riscv-iommu-pci`.
- New: a direct attacker aims a descriptor outside its region, and the IOMMU records a fault at
  that frame, the shape of `the_iommu_faults_a_dma_that_escapes_the_domain`. One that declines
  `ACCESS_PLATFORM` either never reaches `FEATURES_OK` or faults the same way.
- New: a function whose structures share a page with its MSI-X table or pending-bit array gets the
  mediated grant, over a fake capability list as `crates/pci/src/tests.rs` builds one.
- `xtask/src/network_bench.rs` drives e1000e and `xtask/src/disk_throughput.rs` drives NVMe, so
  neither changes.

### The surface

No syscall, object kind or method changes: `DeviceFrame`, `Interrupt` and the startup page already
exist. What is new is the `driver_process_contract::virtio` module (its window kinds and handoff,
provisional) and a virtio-pci transport in userspace that drives the mapped pages.

### Cost and effort

Not measured yet. A mediated `NOTIFY` costs a syscall, the validation and a copy of the new
descriptors. A direct one costs an uncached store from EL0. The last pull request measures both on
a PCI disk with `script/bench`. The work is medium, and the line counts are guesses: about 120 for
the starter's carve-out, 80 for the contract, 200 for the userspace transport and 200 of tests.

### Order of work

1. The contract: `driver_process_contract::virtio`, its window kinds and handoff, with tests.
2. The starter's carve-out: the confinement, the four structures' pages, the withheld MSI-X pages,
   the `ACCESS_PLATFORM` and requester id checks, and the startup page, with unit tests.
3. The driver's direct transport in userspace, the direct disk system test on all three
   architectures and the two escape tests.
4. The measurement.

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
5. A virtio-pci driver runs on either grant, the kernel's own starters keep the mediated grant
   until the architect rules the default, and CI runs the direct and the mediated path on all three
   architectures.

## Index row

The kernel still carries the control planes of e1000e, the GMAC, NVMe, the JH7110's clock and
reset block and its PMIC, and reads the x86 CMOS itself. None of that must run at EL1, and none of
it confines DMA, which is the IOMMU's job. This moves each one to a userspace process and takes about 1,500 kernel lines and
two driver crates out of the trusted base. The virtio validator stays for a device granted
mediated, and a starter may grant a virtio-pci device's registers directly behind the IOMMU.
