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
7. The virtio shadow-ring validation becomes a switch (§23 (multi-queue DMA confinement)).
   `crates/direct_memory_access_validator` and its harnesses stay, switched on if a cloud target
   with virtio and no IOMMU becomes real. Off by default until then is provisional: the architect
   has not ruled on the default, and this step waits for that ruling.

Every new program, process, crate and verb ships with a provisional name.

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

## Index row

The kernel still carries the control planes of e1000e, the GMAC, NVMe, the JH7110's clock and
reset block and its PMIC, and reads the x86 CMOS itself. None of that must run at EL1, and none of
it confines DMA, which is the IOMMU's job. This moves each one to a userspace process, keeps the
virtio validator as a switch for a target without an IOMMU, and takes about 1,500 kernel lines and
two driver crates out of the trusted base.
