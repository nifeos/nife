//! virtio-mmio **enumeration**. Not a driver.
//!
//! This module reads the standardized identity registers of each virtio-mmio slot to find the
//! block device and route it to a userspace driver. It does not set up a queue, negotiate a
//! feature, or move a byte of data. That is all the driver's job, at EL0 (see
//! `components/src/block_driver.rs`, which is that driver on every architecture since milestone
//! 291; it was a role of `fixtures/src/hello.rs` on aarch64 until then).
//!
//! **Why this much lives in the kernel.** Discovering which device is in which slot is bus
//! enumeration, the way firmware walks PCI: you read a device-independent ID register and hand
//! the device to whatever driver claims that ID. It is a bootstrap role, not device operation,
//! and it is the smallest amount of virtio knowledge that lets the kernel say "the block device
//! is in slot 3, its interrupt is INTID 51" without knowing the first thing about how a block
//! device works.

use crate::arch::mmu::{self, VIRTIO_IRQ_BASE, VIRTIO_MMIO_BASE, VIRTIO_SLOT_STRIDE};

/// The virtio-mmio slot layout, from the arch: aarch64's `virt` has 32 slots 0x200 apart, RISC-V's
/// has 8 slots 0x1000 apart. The probe walks `mmu::virtio_slots()` of them at `SLOT_STRIDE`.
///
/// **A function, not a constant, since milestone 89 (Scaleway EM-RV1).** On riscv64 the window is
/// mapped only when the device tree names a `virtio,mmio` node, because on the T-Head TH1520 the
/// QEMU window's address is DRAM, and a probe there would read RAM and could mistake it for a
/// device. A machine without the bus reports zero slots, and the probe reads nothing.
const SLOT_STRIDE: u64 = VIRTIO_SLOT_STRIDE;

/// "virt", little-endian, at offset 0x000 of every slot.
const MAGIC: u32 = 0x7472_6976;
/// `DeviceID` at offset 0x008. 0 means "empty slot"; the virtio device-type numbers we route: 1 is a
/// network card, 2 is a block device, 4 is an entropy source.
const DEVICE_ID_NET: u32 = 1;
const DEVICE_ID_BLOCK: u32 = 2;
const DEVICE_ID_ENTROPY: u32 = 4;

// Register offsets we read here. The driver knows many more; the kernel knows exactly three.
const REG_MAGIC: u64 = 0x000;
const REG_VERSION: u64 = 0x004;
const REG_DEVICE_ID: u64 = 0x008;

/// A virtio device found on the mmio bus: where its registers are, and which interrupt it raises.
/// Device-neutral on purpose, because the fields the kernel hands a driver (a register base and an
/// interrupt) are the same whether the device is a disk or a NIC; only the driver differs.
#[derive(Debug, Clone, Copy)]
pub(crate) struct VirtioMmioDevice {
    /// Physical address of this slot's registers. Handed to the driver as a device mapping.
    pub mmio_phys: u64,
    /// The interrupt this device raises. Handed to the driver as an `Irq` capability.
    pub intid: u32,
}

fn read_reg(slot: u64, offset: u64) -> u32 {
    let va = mmu::phys_to_virt(VIRTIO_MMIO_BASE + slot * SLOT_STRIDE + offset);
    // SAFETY: the virtio-mmio window is mapped device memory (mmu::map_everything), and these
    // offsets are within a slot. Reading an ID register has no side effect.
    unsafe { core::ptr::read_volatile(va as *const u32) }
}

/// Scan the bus for the first virtio device of type `device_id`. The kernel's whole knowledge of a
/// device at this layer is its type number and its slot; which driver claims the type is a userspace
/// decision (see `virtio_service`).
fn find_by_device_id(device_id: u32) -> Option<VirtioMmioDevice> {
    for slot in 0..mmu::virtio_slots() {
        if read_reg(slot, REG_MAGIC) != MAGIC {
            continue; // not a virtio-mmio slot at all
        }
        if read_reg(slot, REG_DEVICE_ID) != device_id {
            continue; // empty, or some other kind of device
        }

        // We require modern virtio (version 2); the register is read for the debug assertion.
        debug_assert_eq!(
            read_reg(slot, REG_VERSION),
            2,
            "expected modern virtio-mmio"
        );

        return Some(VirtioMmioDevice {
            mmio_phys: VIRTIO_MMIO_BASE + slot * SLOT_STRIDE,
            intid: VIRTIO_IRQ_BASE + slot as u32,
        });
    }
    None
}

/// Scan the bus for the first virtio block device. `None` if there is no disk attached.
pub(crate) fn find_block_device() -> Option<VirtioMmioDevice> {
    find_by_device_id(DEVICE_ID_BLOCK)
}

/// How many virtio block devices are on the mmio bus (milestone 57's roster).
///
/// The same walk [`mmio_block_device_n`] does, counted rather than stopped, so the roster's mmio
/// ordinals and the ordinal a wiring passes to [`find_block_device_n`] are the same numbers by
/// construction. Reads two ID registers per slot and nothing else: enumeration is not bring-up.
///
/// [`find_block_device_n`] also calls it, to know where the mmio half of the ordering ends and the
/// PCI half begins.
pub(crate) fn count_block_devices() -> usize {
    let mut n = 0;
    for slot in 0..mmu::virtio_slots() {
        if read_reg(slot, REG_MAGIC) == MAGIC && read_reg(slot, REG_DEVICE_ID) == DEVICE_ID_BLOCK {
            n += 1;
        }
    }
    n
}

/// Scan the bus for the first virtio network device. `None` if there is no NIC attached.
pub(crate) fn find_net_device() -> Option<VirtioMmioDevice> {
    find_by_device_id(DEVICE_ID_NET)
}

/// Scan the bus for the first virtio entropy source (milestone 56). `None` if there is no RNG
/// attached. The PCIe twin is [`crate::pci::find_rng_device`]; the entropy service is wired over
/// whichever one its caller names, and the milestone-56 tests run it over each in turn, because a
/// driver that works on one transport and silently not the other is the bug DECISIONS §18 exists
/// to prevent.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))] // entropy_service is the caller, and the m56 tests drive it
pub(crate) fn find_entropy_device() -> Option<VirtioMmioDevice> {
    find_by_device_id(DEVICE_ID_ENTROPY)
}

/// **One block device, resolved on whichever bus it turned out to be on** (provisional name,
/// milestone 303). What [`find_block_device_n`] hands a wiring, and exactly the three things
/// `virtio_service::wire` and `fs_service::spawn_block_server` need: the transport seam's answer
/// for this device, the interrupt to bind, and the IOMMU requester id when there is one.
///
/// `rid` is `None` for a virtio-mmio device because neither `virt` board puts an IOMMU in front of
/// that bus, and `Some` for a PCI function because the domain `virtio::register` builds is keyed on
/// it. A caller that had to ask which bus it got would be a caller the seam failed.
pub(crate) struct BlockDevice {
    pub transport: Transport,
    pub intid: u32,
    pub rid: Option<u32>,
}

/// **The `n`-th (0-based) block device this machine offers, on any bus.** `None` if there is no
/// such disk.
///
/// The FS server (milestone 32 phase 2) drives the SECOND one (`n = 1`), a RedoxFS image, leaving
/// the first (the nifefs disk) to the phase-1 driver tests. Milestone 57's roster wirings take the
/// fourth and fifth, and milestone 37's crash test the third.
///
/// **The ordering is a contract, and it is the roster's row order**: every virtio-mmio slot first,
/// in slot order, then every modern virtio-pci function, in bus:device.function order. That is the
/// same order `disk_service::devices` lists the roster in, so a listing and a wiring cannot
/// disagree about which disk is which, and it is what keeps these ordinals stable across the two
/// `virt` boards while widening them to a machine with no mmio bus at all. QEMU numbers the mmio
/// slots in device order, so the aarch64 and riscv64 runners attach the nifefs disk first and the
/// RedoxFS disk second and nothing about those two architectures moved when this stopped being
/// mmio-only (milestone 303): both have five mmio block devices, so every ordinal any wiring asks
/// for is still resolved before the PCI half is reached.
///
/// **`q35` has no virtio-mmio bus at all** (`arch::x86_64::mmu::VIRTIO_SLOTS` is 0), so on `x86_64`
/// the mmio half is empty and ordinal 0 is the first `virtio-blk-pci` function, ordinal 1 the
/// second. `helpers/qemu-runner-x86_64.sh` attaches the nifefs image and then the RedoxFS image in
/// that order for exactly that reason.
///
/// **Resolving a PCI ordinal brings the function up**, which sizes and assigns its BARs and enables
/// memory decoding and bus mastering; the mmio half reads two identity registers and nothing else.
/// That asymmetry is `pci::bring_up`'s, not this function's, and it is why a bare *presence* probe
/// (`fs_service::is_crash_disk_present`) should be read as "this ordinal exists", with the side
/// effects of having asked.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))] // fs_service is the caller, and the phase-2 test drives it
pub(crate) fn find_block_device_n(n: usize) -> Option<BlockDevice> {
    if let Some(d) = mmio_block_device_n(n) {
        return Some(BlockDevice {
            transport: Transport::Mmio {
                mmio_phys: d.mmio_phys,
            },
            intid: d.intid,
            rid: None, // virtio-mmio has no IOMMU in front of it on either board
        });
    }
    let d = crate::pci::find_block_device_n(n.checked_sub(count_block_devices())?)?;
    Some(BlockDevice {
        transport: Transport::pci(&d),
        intid: d.intid,
        rid: Some(d.rid), // the PCIe requester id, the IOMMU keys its tables on it
    })
}

/// The mmio half of [`find_block_device_n`]: the `n`-th virtio-mmio block slot, in slot order.
fn mmio_block_device_n(n: usize) -> Option<VirtioMmioDevice> {
    let mut seen = 0;
    for slot in 0..mmu::virtio_slots() {
        if read_reg(slot, REG_MAGIC) != MAGIC || read_reg(slot, REG_DEVICE_ID) != DEVICE_ID_BLOCK {
            continue;
        }
        if seen == n {
            return Some(VirtioMmioDevice {
                mmio_phys: VIRTIO_MMIO_BASE + slot * SLOT_STRIDE,
                intid: VIRTIO_IRQ_BASE + slot as u32,
            });
        }
        seen += 1;
    }
    None
}

// ---------------------------------------------------------------------------------------------
// Milestone: DMA confinement. The kernel owns the block device's transport.
//
// The device does DMA against raw physical addresses with no IOMMU in front of it, so page-table
// permissions do not apply to it. If a userspace driver could program the queue and ring the
// device itself, it could point the device at any physical address (the kernel, another process)
// and the device would read or write it. So the kernel keeps the two DMA-critical powers: the
// queue's ring addresses and the "go" signal, and **validates that every address the device
// will touch lies within the driver's own DMA region** before letting it proceed. The driver
// still builds its own requests in its own region and reads its own results; it simply cannot
// aim the device anywhere else.
//
// This is the software stand-in for an IOMMU. It is not generic (it understands the virtqueue
// *transport*: the descriptor table and the available ring), but it knows nothing about block
// devices: the request format, sectors, and results stay in the userspace driver.
// ---------------------------------------------------------------------------------------------

use crate::sync::{IrqSafeMutex, rank};

// virtio-mmio v2 registers the kernel drives.
const REG_DEVICE_FEATURES_SEL: u64 = 0x014;
const REG_DRIVER_FEATURES: u64 = 0x020;
const REG_DRIVER_FEATURES_SEL: u64 = 0x024;
const REG_QUEUE_SEL: u64 = 0x030;
const REG_QUEUE_NUM_MAX: u64 = 0x034;
const REG_QUEUE_NUM: u64 = 0x038;
const REG_QUEUE_READY: u64 = 0x044;
const REG_QUEUE_NOTIFY: u64 = 0x050;
const REG_INTERRUPT_ACK: u64 = 0x064;
const REG_STATUS: u64 = 0x070;
const REG_QUEUE_DESC_LOW: u64 = 0x080;
const REG_QUEUE_DESC_HIGH: u64 = 0x084;
const REG_QUEUE_DRIVER_LOW: u64 = 0x090;
const REG_QUEUE_DRIVER_HIGH: u64 = 0x094;
const REG_QUEUE_DEVICE_LOW: u64 = 0x0a0;
const REG_QUEUE_DEVICE_HIGH: u64 = 0x0a4;
// Driver-visible reads the pci transport synthesizes (see `Transport::read_reg`).
const REG_MAGIC_RO: u64 = 0x000;
const REG_VERSION_RO: u64 = 0x004;
const REG_DEVICE_ID_RO: u64 = 0x008;
const REG_DEVICE_FEATURES: u64 = 0x010;
const REG_INTERRUPT_STATUS: u64 = 0x060;
/// The device-specific config area, in the mmio vocabulary (virtio spec 4.2.2: `config` at
/// offset 0x100 of the register block). A blk device's capacity lives at its offset 0, which is
/// what `blk::SIZE` reads in two 32-bit halves. On PCI the region is its own piece of a BAR,
/// described by a `DEVICE_CFG` vendor capability, resolved at bring-up and carried in
/// [`Transport::Pci::device_config`].
const REG_DEVICE_CONFIG: u64 = 0x100;

/// **The transport seam** (the PCIe workstream's P4; notes/pcie-transport-scope.md). Everything
/// above this enum, the shadow ring, the validator, the queue layout contract, the userspace
/// driver, speaks one register vocabulary: virtio-mmio's. The seam keeps it that way. A
/// `Transport` answers that vocabulary against whichever bus the device actually sits on: the
/// mmio variant is a passthrough to the slot's registers; the pci variant translates each name
/// to the virtio-pci common-config layout (a different arrangement of the same registers), the
/// ISR byte, and the notify doorbell. The userspace driver is byte-identical over both, and the
/// DMA confinement neither knows nor cares which bus rang the device.
///
/// The mmio vocabulary is the seam's canonical language on purpose: it is the vocabulary the ABI
/// already exposes to drivers (`abi::virtio`), and it is flat (offset -> register), which makes
/// the translation table below legible. Nothing about the choice is load-bearing beyond that.
pub(crate) enum Transport {
    /// A virtio-mmio slot: the vocabulary IS this device's register block.
    Mmio { mmio_phys: u64 },
    /// A modern virtio-pci function, resolved by kernel/src/pci.rs: the common-config block, the
    /// ISR byte, and the notify doorbell parameters, all physical addresses in a mapped BAR.
    Pci {
        common: u64,
        notify_base: u64,
        notify_mult: u32,
        /// The virtio **device type** (2 = block, 1 = net, 16 = gpu), from the PCI device id
        /// (`0x1040 + type`). PCI config space has no `DeviceID` register in the mmio sense, so a
        /// driver's `DeviceID` read is answered from this. It used to be answered with a hardcoded
        /// 2, which was a manufactured fact for every device that is not a disk; milestone 29's GPU
        /// driver is the first to check it, and finding the lie is what got it fixed.
        device_type: u32,
        /// Each queue's resolved doorbell, computed at `setup_queue` (it needs `queue_notify_off`,
        /// which is only readable with that queue selected, and it differs per queue). Zero until
        /// the queue is set up. Indexed by queue number; see `MAX_QUEUES`.
        notify_addr: [u64; MAX_QUEUES],
        isr: u64,
        /// **Which MSI-X table entry this function's queues should raise**, or
        /// `pci::VIRTIO_MSIX_NO_VECTOR` when the function delivers INTx (both `virt` boards; see
        /// `crate::pci::enable_msix`).
        ///
        /// It is carried here rather than programmed once at bring-up because virtio puts the
        /// field **inside the common config, behind the queue selector**, and clears it on device
        /// reset. Every driver resets the device on the way in, so the only correct moment to
        /// write it is [`Transport::setup_queue`], which is also the only place the queue is
        /// already selected.
        msix_vector: u16,
        /// The device-specific config region (a `DEVICE_CFG` vendor capability resolved to a
        /// physical address by `pci::PciVirtioDevice`), when this function has one; `None` for
        /// the device types that have none (net, rng, console). The mmio arm reads the same area
        /// at [`REG_DEVICE_CONFIG`] inside its own register block, which is the translation this
        /// field exists for: one offset in the shared vocabulary, two different places to read.
        device_config: Option<u64>,
    },
}

// virtio-pci common-config offsets (virtio spec 4.1.4.3), the pci side of the translation.
const PCI_DEVICE_FEATURE_SEL: u64 = 0x00;
const PCI_DEVICE_FEATURE: u64 = 0x04;
const PCI_DRIVER_FEATURE_SEL: u64 = 0x08;
const PCI_DRIVER_FEATURE: u64 = 0x0c;
const PCI_DEVICE_STATUS: u64 = 0x14;
const PCI_QUEUE_SEL: u64 = 0x16;
const PCI_QUEUE_SIZE: u64 = 0x18;
const PCI_QUEUE_ENABLE: u64 = 0x1c;
const PCI_QUEUE_NOTIFY_OFF: u64 = 0x1e;
/// The MSI-X vector the **selected queue** raises, `0xffff` for none. Behind the queue selector,
/// and reset to `0xffff` by a device reset, which is why the kernel rewrites it at every
/// `setup_queue` rather than once at bring-up.
const PCI_QUEUE_MSIX_VECTOR: u64 = 0x1a;
const PCI_QUEUE_DESC: u64 = 0x20;
const PCI_QUEUE_DRIVER: u64 = 0x28;
const PCI_QUEUE_DEVICE: u64 = 0x30;

/// Volatile accessors at a physical address through the direct map, in the width the pci
/// common-config block prescribes per register (it is packed; mmio is uniformly u32).
///
/// # Why these two stay safe fns (milestone 112)
///
/// They were on milestone 82's list of four safe functions whose SAFETY comment discharged onto
/// "the caller" while the signature imposed the obligation on nobody. The other three converted to
/// `unsafe fn`. These did not, and the difference is not that the obligation is weaker: it is that
/// **the set of callers is closed and the compiler is what closes it.**
///
/// `pread` and `pwrite` are private to this module. Every one of their call sites is in the
/// `impl Transport` block above, and each passes `common + <a constant offset under 0x40>`, `isr`,
/// `notify_addr[q]`, or `device_config + <an offset the DEVICE_CFG cap's length bounds>`: four
/// fields of `Transport::Pci`, all resolved from a mapped BAR by `pci::PciVirtioDevice` and copied
/// in one place, by `Transport::pci`. That is a module invariant,
/// and a module invariant is a real way to be sound. Converting would have put twenty `unsafe`
/// blocks in one file, each restating the same sentence, which is the "discharged by ritual rather
/// than by thought" failure the milestone was written to avoid; and it would not have made anything
/// checkable, because an `unsafe fn` whose contract nothing verifies is still a contract nothing
/// verifies.
///
/// # BUGS
///
/// **Taking the old comment seriously found a way to make it false**, which is the argument for
/// this milestone in one bug. `notify_addr[q]` is **zero until [`Transport::setup_queue`] resolves
/// it**, and the `NOTIFY` syscall used to check only that the queue number was in range. A
/// userspace driver could therefore ring a queue it had never set up, and this module would
/// `write_volatile` a `u16` through `phys_to_virt(0)`: not inside any BAR, which is exactly what
/// the comment claimed could not happen. [`notify`] now refuses that queue
/// ([`Transport::is_doorbell_ready`]). The mmio transport was never exposed, because it has one
/// fixed notify register rather than a per-queue address to resolve.
fn pread<T: Copy>(phys: u64) -> T {
    // SAFETY: `phys` is a field of a `Transport::Pci`, plus at most a common-config offset or a
    // device-config offset. Those
    // fields come from `pci.rs`'s BAR resolution and reach this module only through
    // `Transport::pci`; this fn is private, so that closed set of call sites is the whole
    // population. See the module invariant above, and the doorbell bug it did not used to cover.
    unsafe { core::ptr::read_volatile(mmu::phys_to_virt(phys) as *const T) }
}
fn pwrite<T: Copy>(phys: u64, v: T) {
    // SAFETY: as above, and the same closed call set: `pwrite` is private to this module too.
    unsafe { core::ptr::write_volatile(mmu::phys_to_virt(phys) as *mut T, v) }
}

impl Transport {
    /// The PCI transport for an enumerated function (kernel/src/pci.rs). Every field copied out of
    /// one struct, in one place, so a new field (milestone 29's `device_type` was the second) is not
    /// a many-call-site edit and cannot be filled in differently at one of them.
    pub(crate) fn pci(d: &crate::pci::PciVirtioDevice) -> Transport {
        Transport::Pci {
            common: d.common,
            notify_base: d.notify_base,
            notify_mult: d.notify_mult,
            device_type: d.device_type,
            notify_addr: [0; MAX_QUEUES],
            isr: d.isr,
            msix_vector: d.msix_vector,
            device_config: d.device_config,
        }
    }

    /// Answer a read in the mmio vocabulary.
    fn read_reg(&self, off: u64) -> u32 {
        match self {
            Transport::Mmio { mmio_phys } => reg_read(*mmio_phys, off),
            Transport::Pci {
                common,
                isr,
                device_type,
                device_config,
                ..
            } => match off {
                // pci has no magic/version/device-id registers: identity was proved from config
                // space (vendor/device id) before this transport existed. Synthesized so the
                // driver's sanity checks mean the same thing on both buses. The device id is the
                // virtio device TYPE recovered from the PCI id, not a constant: see `device_type`.
                REG_MAGIC_RO => 0x7472_6976,
                REG_VERSION_RO => 2,
                REG_DEVICE_ID_RO => *device_type,
                REG_DEVICE_FEATURES => pread::<u32>(common + PCI_DEVICE_FEATURE),
                // The ISR byte is read-to-ack at the device: the read itself deasserts INTx.
                // Same bit 0 (queue interrupt) the mmio register reports.
                REG_INTERRUPT_STATUS => pread::<u8>(*isr) as u32,
                REG_STATUS => pread::<u8>(common + PCI_DEVICE_STATUS) as u32,
                REG_QUEUE_NUM_MAX => {
                    pwrite::<u16>(common + PCI_QUEUE_SEL, 0);
                    pread::<u16>(common + PCI_QUEUE_SIZE) as u32
                }
                // The device-specific config, in the 32-bit halves the vocabulary reads it in
                // (today exactly two, `blk::SIZE`'s capacity at REG_DEVICE_CONFIG and +4). A
                // function with no such region answers as the mmio arm would read a hole.
                off if off == REG_DEVICE_CONFIG || off == REG_DEVICE_CONFIG + 4 => {
                    match device_config {
                        Some(config) => pread::<u32>(config + (off - REG_DEVICE_CONFIG)),
                        None => 0,
                    }
                }
                _ => 0,
            },
        }
    }

    /// Answer a write in the mmio vocabulary. Only the offsets the kernel itself writes and the
    /// driver-safe set (`write_register`'s SAFE list) reach here.
    fn write_reg(&mut self, off: u64, v: u32) {
        match self {
            Transport::Mmio { mmio_phys } => reg_write(*mmio_phys, off, v),
            Transport::Pci { common, .. } => match off {
                REG_DEVICE_FEATURES_SEL => pwrite::<u32>(*common + PCI_DEVICE_FEATURE_SEL, v),
                REG_DRIVER_FEATURES_SEL => pwrite::<u32>(*common + PCI_DRIVER_FEATURE_SEL, v),
                REG_DRIVER_FEATURES => pwrite::<u32>(*common + PCI_DRIVER_FEATURE, v),
                REG_STATUS => pwrite::<u8>(*common + PCI_DEVICE_STATUS, v as u8),
                // The ISR read already acknowledged; the mmio-style ack has no pci counterpart.
                REG_INTERRUPT_ACK => {}
                _ => {}
            },
        }
    }

    /// Queue `q`'s maximum size, as the device reports it. Each queue is selected and read on its
    /// own: a device may size its queues differently (virtio-net rarely does, but the kernel does
    /// not assume it).
    fn queue_num_max(&self, q: u16) -> u16 {
        match self {
            Transport::Mmio { mmio_phys } => {
                reg_write(*mmio_phys, REG_QUEUE_SEL, q as u32);
                reg_read(*mmio_phys, REG_QUEUE_NUM_MAX) as u16
            }
            Transport::Pci { common, .. } => {
                pwrite::<u16>(*common + PCI_QUEUE_SEL, q);
                pread::<u16>(*common + PCI_QUEUE_SIZE)
            }
        }
    }

    /// Program queue `q`: its size and its three ring addresses, then mark it live. The addresses
    /// are the kernel's choice (the shadow page and the driver's used ring); that this method is
    /// the only way they reach the device is the confinement. On PCI the queue's doorbell is
    /// resolved here (it is only readable with the queue selected) and remembered per queue.
    fn setup_queue(&mut self, q: u16, num: u16, desc: u64, avail: u64, used: u64) {
        match self {
            Transport::Mmio { mmio_phys } => {
                let m = *mmio_phys;
                reg_write(m, REG_QUEUE_SEL, q as u32);
                reg_write(m, REG_QUEUE_NUM, num as u32);
                reg_write(m, REG_QUEUE_DESC_LOW, desc as u32);
                reg_write(m, REG_QUEUE_DESC_HIGH, (desc >> 32) as u32);
                reg_write(m, REG_QUEUE_DRIVER_LOW, avail as u32);
                reg_write(m, REG_QUEUE_DRIVER_HIGH, (avail >> 32) as u32);
                reg_write(m, REG_QUEUE_DEVICE_LOW, used as u32);
                reg_write(m, REG_QUEUE_DEVICE_HIGH, (used >> 32) as u32);
                reg_write(m, REG_QUEUE_READY, 1);
            }
            Transport::Pci {
                common,
                notify_base,
                notify_mult,
                notify_addr,
                msix_vector,
                ..
            } => {
                pwrite::<u16>(*common + PCI_QUEUE_SEL, q);
                pwrite::<u16>(*common + PCI_QUEUE_SIZE, num);
                // **Tell the queue which MSI-X vector to raise** (milestone 215), while it is
                // selected and after the driver's reset has cleared the field to `NO_VECTOR`. On a
                // function delivering INTx this writes `NO_VECTOR`, which is exactly what is
                // already there, so both boards see no change at all. The read-back is the
                // specification's own way of asking whether the device accepted the vector (it
                // answers `NO_VECTOR` when it could not), and a device that refused would
                // otherwise present as a driver that hangs on its first completion.
                if *msix_vector != pci::VIRTIO_MSIX_NO_VECTOR {
                    pwrite::<u16>(*common + PCI_QUEUE_MSIX_VECTOR, *msix_vector);
                    let accepted = pread::<u16>(*common + PCI_QUEUE_MSIX_VECTOR);
                    if accepted != *msix_vector {
                        crate::println!(
                            "  virtio: queue {q} refused msi-x vector {msix_vector} (answered {accepted})",
                        );
                    }
                }
                pwrite::<u64>(*common + PCI_QUEUE_DESC, desc);
                pwrite::<u64>(*common + PCI_QUEUE_DRIVER, avail);
                pwrite::<u64>(*common + PCI_QUEUE_DEVICE, used);
                // The doorbell: notify_base + queue_notify_off * multiplier, readable only with
                // this queue selected, so it is resolved here and remembered for this queue.
                let off = pread::<u16>(*common + PCI_QUEUE_NOTIFY_OFF) as u64;
                notify_addr[q as usize] = *notify_base + off * (*notify_mult as u64);
                pwrite::<u16>(*common + PCI_QUEUE_ENABLE, 1);
            }
        }
    }

    /// **Is queue `q`'s doorbell a real address yet?** (milestone 112.)
    ///
    /// On PCI it is resolved at [`Transport::setup_queue`] and the field is zero until then, so
    /// ringing an un-set-up queue would write through `phys_to_virt(0)`, outside every BAR. On mmio
    /// there is one fixed notify register that carries the queue number as its value, so there is
    /// nothing per-queue to resolve and nothing to be unresolved.
    ///
    /// [`notify`] is the gate that uses this. It exists because the SAFETY comment on [`pread`]
    /// claimed every address reaching it was inside a device-mapped BAR, and this was the path where
    /// that was false.
    fn is_doorbell_ready(&self, q: u16) -> bool {
        match self {
            Transport::Mmio { .. } => true,
            Transport::Pci { notify_addr, .. } => notify_addr[q as usize] != 0,
        }
    }

    /// Ring the doorbell for queue `q`. Only [`notify`] calls this, after validation and after
    /// [`Transport::is_doorbell_ready`]. On mmio the notify register carries the queue number as
    /// its value; on PCI each queue has its own doorbell address, resolved at [`setup_queue`].
    fn notify_queue(&self, q: u16) {
        match self {
            Transport::Mmio { mmio_phys } => reg_write(*mmio_phys, REG_QUEUE_NOTIFY, q as u32),
            Transport::Pci { notify_addr, .. } => pwrite::<u16>(notify_addr[q as usize], 0),
        }
    }
}

/// The fixed queue layout, a contract shared with the userspace driver (user/src/virtio.rs). The
/// kernel places the rings at these offsets in the DMA region, so it always knows where they are.
/// These offsets are **relative to a queue's ring block**; a device's queue `q` puts its rings at
/// `q * RING_BLOCK + {DESC,AVAIL,USED}_OFF` (see `RING_BLOCK`).
///
/// **These are aliases, not copies.** The layout is defined in `crates/direct_memory_access_validator`, where
/// `distinct_queues_occupy_disjoint_blocks` proves the isolation that follows from it (a queue's ring
/// area fits inside its own block, blocks do not overlap, and the descriptor table a validation walk
/// writes ends before the available ring begins). Milestone 35 first duplicated them here, which left
/// the proof quantifying over constants this file could drift away from silently; aliasing makes the
/// proved layout the layout that runs, the same discipline that has the kernel *call*
/// `direct_memory_access_validator::validate_and_shadow` rather than keep a parallel copy of it.
pub(crate) const QSIZE: u16 = direct_memory_access_validator::LAYOUT_QSIZE;
const DESC_OFF: u64 = direct_memory_access_validator::DESC_OFF; // 16 * QSIZE
const AVAIL_OFF: u64 = direct_memory_access_validator::AVAIL_OFF; // 6 + 2*QSIZE
const USED_OFF: u64 = direct_memory_access_validator::USED_OFF; // 6 + 8*QSIZE
/// The whole ring area of one queue must fit under this; a queue's data buffers live above it.
const RING_END: u64 = direct_memory_access_validator::RING_END;

/// The most virtqueues the confinement drives per device. A virtio-net device needs two (receive =
/// queue 0, transmit = queue 1); the disk uses only queue 0. Fixed and small on purpose: one
/// kernel-private shadow frame per device holds every queue's shadow rings, so the ceiling is what
/// fits there (`MAX_QUEUES * RING_BLOCK <= FRAME_SIZE`), asserted below, not a policy dial.
pub(crate) const MAX_QUEUES: usize = direct_memory_access_validator::MAX_QUEUES as usize;

/// The stride between successive queues' ring areas, in **both** the driver's DMA region and the
/// kernel-private shadow. Queue `q`'s descriptor table, available ring, and used ring sit at
/// `q * RING_BLOCK + {DESC,AVAIL,USED}_OFF`. 0x200 leaves room for a whole queue's ring area
/// (`RING_END`) and keeps queue 0 byte-identical to the single-queue layout the disk driver already
/// uses: the disk's data buffers begin at 0x200 (= queue 1's block), which is free because a disk
/// has no queue 1, so the disk needs no change at all.
const RING_BLOCK: u64 = direct_memory_access_validator::RING_BLOCK;

// The shadow frame must hold every queue's ring block, and a queue's ring area must fit in its
// block. Both are compile-time facts, so break the build if a future edit violates either.
const _: () = assert!(
    RING_END <= RING_BLOCK,
    "a queue's rings overflow its ring block"
);
const _: () = assert!(
    MAX_QUEUES as u64 * RING_BLOCK <= page_frames::FRAME_SIZE,
    "the shadow frame cannot hold every queue's ring block",
);

/// Queue `q`'s ring block base, relative to a region (driver DMA or shadow) base. The proved
/// function, for the same reason the constants above are aliases.
const fn queue_block(q: u16) -> u64 {
    direct_memory_access_validator::queue_block(q)
}

// The descriptor flags the validator acts on. The validation logic that reads them now lives in
// `crates/direct_memory_access_validator` (as `direct_memory_access_validator::F_NEXT` /
// `F_INDIRECT`); these copies remain for the attacker tests in `virtio/tests.rs`, which build
// descriptor words by hand. Bit 2 (`INDIRECT`) points a descriptor at a table of further
// descriptors the validator never copies into the shadow, so it is refused (and the feature that
// enables it is negotiated off in `sanitize_driver_features`), which is the confinement failing
// closed if that negotiation ever regresses.
#[cfg(test)]
const VIRTQ_DESC_F_NEXT: u16 = 1;
#[cfg(test)]
const VIRTQ_DESC_F_INDIRECT: u16 = 4;

/// One block device the kernel operates the transport for.
struct Device {
    transport: Transport,
    direct_memory_access_base: u64,
    direct_memory_access_size: u64,
    /// The last available-ring index we have already validated and forwarded, **per queue**.
    /// Descriptors are only ever *added* by the driver, so we validate the new ones each notify;
    /// RX and TX advance independently, so each queue keeps its own high-water mark.
    last_avail: [u16; MAX_QUEUES],
    /// Which 32-bit word of the feature bits the driver's next `DRIVER_FEATURES` write targets
    /// (`DRIVER_FEATURES_SEL`: 0 = features 0..31, 1 = 32..63). Tracked so a feature write can have
    /// the ring-layout features the validator cannot police stripped from whichever word carries
    /// them. See `sanitize_driver_features`.
    driver_features_sel: u32,
    /// Physical base of the **kernel-private shadow page**: the descriptor table and available ring
    /// the *device* actually reads. The driver builds its own copies in its DMA region; on `notify`
    /// the kernel validates those and copies them here, so the device only ever reads descriptors
    /// the driver cannot touch. This is what closes the time-of-check/time-of-use race: the bytes
    /// the device reads are the bytes the kernel validated. See notes/dma.md.
    shadow_base: u64,
}

/// The most virtio transports we will drive. Fixed-size, so probing never allocates.
///
/// **Each `register` takes a slot for the life of the boot**: nothing unregisters, so this counts
/// how many devices a boot has ever wired, not how many exist (the aarch64 machine has five). The
/// test suite spawns a driver per gate, and every milestone that wires one more confined device
/// has bumped it; tests share a transport where they can to avoid that (the genuine and tampered
/// package fetches share one `net_stack`). Memory is no longer the binding constraint (a net
/// service hands its frames back, notes/frames.md); only this counter is.
///
/// **This is an exception and a foot gun.** The fix is reuse on driver death, which needs a
/// generational name so a stale `Object::Virtio` capability cannot resolve to a different device:
/// a change to what a capability means, a design fork under §16 (object revocation), and
/// interacting with `entropy_service::ensure`'s one-service-per-device rule. It is proposed as
/// `design/roadmap/0647-a-virtio-slot-comes-back-when-its-driver-dies.md`; the next lane that needs
/// a device should take that rather than a bump.
const MAX_DEVICES: usize = 34;

/// The device table, fixed. `get`/`get_mut` mirror the slice API the call sites already used.
struct Devices {
    entries: [Option<Device>; MAX_DEVICES],
    count: usize,
}

impl Devices {
    fn get(&self, i: usize) -> Option<&Device> {
        self.entries.get(i)?.as_ref()
    }

    fn get_mut(&mut self, i: usize) -> Option<&mut Device> {
        self.entries.get_mut(i)?.as_mut()
    }
}

static DEVICES: IrqSafeMutex<Devices> = IrqSafeMutex::new(
    rank::VIRTIO,
    Devices {
        entries: [const { None }; MAX_DEVICES],
        count: 0,
    },
);

/// Register the block device and its DMA region with the transport layer. Returns its id, which
/// is what goes inside an `Object::Virtio` capability. The driver never sees the device's
/// registers on either bus; it drives the device through that capability.
///
/// `rid` is the PCIe requester id when the device sits behind an IOMMU (the PCI transport), `None`
/// for a virtio-mmio device (no IOMMU fronts the mmio bus on either board). When an IOMMU is active
/// and a requester id is given, the device is confined in hardware to exactly the frames it may
/// reach (its DMA region plus the shadow page) before it is entered in the table: from that moment
/// any other address it emits faults at the IOMMU. The software shadow ring stays either way, now
/// as defence in depth (notes/dma.md).
pub(crate) fn register(
    transport: Transport,
    direct_memory_access_base: u64,
    direct_memory_access_size: u64,
    rid: Option<u32>,
) -> usize {
    // The shadow page the device reads its rings from. One frame per device, kernel-owned and never
    // mapped into the driver, so the driver cannot touch what the device sees.
    // Zeroed so a stale word can never look like a valid descriptor before the first copy fills
    // it.
    let shadow_base = crate::memory::alloc_zeroed()
        .expect("no frame for the virtio shadow ring")
        .addr();

    // Confine the device in hardware before it is entered in the table. Done outside the DEVICES
    // lock: building the domain allocates page-table frames, and the IOMMU attach takes its own
    // lock, so keeping both off the VIRTIO rank keeps the lock order a plain leaf (sync::rank).
    if let Some(rid) = rid
        && crate::iommu::is_active()
    {
        let regions = crate::iommu::virtio_regions(
            direct_memory_access_base,
            direct_memory_access_size,
            shadow_base,
        );
        crate::iommu::confine(rid, &regions);
    }

    let mut devs = DEVICES.lock();
    assert!(
        devs.count < MAX_DEVICES,
        "more virtio devices than MAX_DEVICES"
    );
    let id = devs.count;
    devs.entries[id] = Some(Device {
        transport,
        direct_memory_access_base,
        direct_memory_access_size,
        last_avail: [0; MAX_QUEUES],
        driver_features_sel: 0,
        shadow_base,
    });
    devs.count += 1;
    id
}

fn reg_read(mmio_phys: u64, off: u64) -> u32 {
    // SAFETY: the virtio-mmio window is mapped device memory (mmu::map_everything); off is a v2
    // register within a slot.
    unsafe { core::ptr::read_volatile(mmu::phys_to_virt(mmio_phys + off) as *const u32) }
}
fn reg_write(mmio_phys: u64, off: u64, v: u32) {
    // SAFETY: as above.
    unsafe { core::ptr::write_volatile(mmu::phys_to_virt(mmio_phys + off) as *mut u32, v) }
}

/// Read `n` bytes from a physical address in the DMA region, through the direct map. Used by the
/// validator to walk the driver's descriptor table and available ring.
fn direct_memory_access_read16(phys: u64) -> u16 {
    // SAFETY: `phys` lies in the DMA region, which `mmu::map_everything` covers, so `phys_to_virt` names mapped memory. Volatile because this is the DRIVER's memory: the validator must read what is there now, not a value the compiler cached.
    unsafe { core::ptr::read_volatile(mmu::phys_to_virt(phys) as *const u16) }
}
fn direct_memory_access_read64(phys: u64) -> u64 {
    // SAFETY: `phys` lies in the DMA region, which `mmu::map_everything` covers, so `phys_to_virt` names mapped memory. Volatile because this is the DRIVER's memory: the validator must read what is there now, not a value the compiler cached.
    unsafe { core::ptr::read_volatile(mmu::phys_to_virt(phys) as *const u64) }
}

/// Write into the kernel-private shadow ring, through the direct map. The shadow page is a
/// kernel-owned frame, so this is a plain store to memory only the kernel names.
fn direct_memory_access_write16(phys: u64, v: u16) {
    // SAFETY: the shadow ring is a kernel-owned frame reached through the direct map, so this is an ordinary store to memory only the kernel names.
    unsafe { core::ptr::write_volatile(mmu::phys_to_virt(phys) as *mut u16, v) }
}
fn direct_memory_access_write64(phys: u64, v: u64) {
    // SAFETY: the shadow ring is a kernel-owned frame reached through the direct map, so this is an ordinary store to memory only the kernel names.
    unsafe { core::ptr::write_volatile(mmu::phys_to_virt(phys) as *mut u64, v) }
}

/// **The security-critical step: validate the driver's descriptors AND copy them into the shadow
/// ring the device reads.**
///
/// The logic lives in `crates/direct_memory_access_validator` (milestone 35 (prove the
/// DMA-confinement boundary)), lifted out so Kani can prove it for every input: no descriptor the
/// device reads out of the shadow references memory outside
/// `[direct_memory_access_base, direct_memory_access_base + direct_memory_access_size)`, in either
/// direction, with indirect descriptors refused, over any batch. This is the thin kernel adapter:
/// it passes the driver and shadow ring *physical addresses* and the direct-map read/write
/// closures, plus `QSIZE`, straight through to the proved walk. The device reads the shadow (see
/// [`setup_queue`]), which the driver cannot write, so the bytes the device acts on are exactly the
/// bytes validated here; mutating a descriptor after this returns changes only the driver's own
/// copy, which nothing reads. See notes/dma.md and notes/verification.md.
#[allow(clippy::too_many_arguments)]
fn validate_and_shadow(
    direct_memory_access_base: u64,
    direct_memory_access_size: u64,
    driver_desc: u64,
    driver_avail: u64,
    shadow_desc: u64,
    shadow_avail: u64,
    from_idx: u16,
    to_idx: u16,
    read16: &dyn Fn(u64) -> u16,
    read64: &dyn Fn(u64) -> u64,
    write16: &dyn Fn(u64, u16),
    write64: &dyn Fn(u64, u64),
) -> bool {
    direct_memory_access_validator::validate_and_shadow(
        direct_memory_access_base,
        direct_memory_access_size,
        driver_desc,
        driver_avail,
        shadow_desc,
        shadow_avail,
        from_idx,
        to_idx,
        QSIZE,
        read16,
        read64,
        write16,
        write64,
    )
}

/// Errors the transport can return to the driver.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TransportError {
    /// No such device id.
    NoDevice,
    /// The queue does not fit in the DMA region, `QUEUE_NUM_MAX` is too small, or the queue is in
    /// range but was never set up, so it has no doorbell to ring (milestone 112).
    BadQueue,
    /// **A descriptor pointed outside the driver's DMA region.** The device was NOT told to go.
    DmaEscape,
}

/// Read a device register the driver is allowed to see (status, features, interrupt status).
///
/// `off` is the caller's, and the mmio arm adds it to the slot's physical base and reads through
/// the direct map, so an offset past the slot would read whatever RAM or device lies there. Only
/// an aligned word inside the device's own slot is answered.
pub(crate) fn read_register(id: usize, off: u64) -> Option<u32> {
    if !register_in_slot(off) {
        return None;
    }
    let devs = DEVICES.lock();
    let dev = devs.get(id)?;
    Some(dev.transport.read_reg(off))
}

/// Whether `off` names an aligned 32-bit register inside one virtio-mmio slot.
fn register_in_slot(off: u64) -> bool {
    off.is_multiple_of(4) && off < SLOT_STRIDE
}

/// Write one of the DMA-*safe* registers (status, features selection, interrupt ack). Refuses the
/// DMA-critical ones (queue addresses, notify), which have their own validated paths.
pub(crate) fn write_register(id: usize, off: u64, val: u32) -> Result<(), TransportError> {
    // Only these offsets are safe to pass straight through. Everything to do with queue setup or
    // notification goes through setup_queue / notify, which validate.
    const SAFE: &[u64] = &[
        REG_STATUS,
        REG_DEVICE_FEATURES_SEL,
        REG_DRIVER_FEATURES_SEL,
        REG_DRIVER_FEATURES,
        REG_INTERRUPT_ACK,
    ];
    if !SAFE.contains(&off) {
        return Err(TransportError::BadQueue);
    }
    let mut devs = DEVICES.lock();
    let dev = devs.get_mut(id).ok_or(TransportError::NoDevice)?;

    // Feature negotiation is a two-step dance: the driver selects a 32-bit word with
    // `DRIVER_FEATURES_SEL`, then writes that word with `DRIVER_FEATURES`. We remember the selector
    // so the write can have the ring-layout features the validator cannot police stripped from
    // whichever word carries them, before the device ever sees the value.
    let val = match off {
        REG_DRIVER_FEATURES_SEL => {
            dev.driver_features_sel = val;
            val
        }
        REG_DRIVER_FEATURES => sanitize_driver_features(dev.driver_features_sel, val),
        _ => val,
    };

    dev.transport.write_reg(off, val);
    Ok(())
}

/// Strip the ring-layout features the descriptor validator cannot police from a `DRIVER_FEATURES`
/// word. `sel` is the word the driver selected: 0 = features 0..31, 1 = features 32..63.
///
/// Two features change **what the device reads descriptors from**, which is exactly the thing
/// `validate_and_shadow` assumes it controls:
///
/// - **`INDIRECT_DESC`** (bit 28, low word): a descriptor may point at a table of further
///   descriptors. The validator walks the flat chain and never follows that table, so the inner
///   descriptors reach the device unchecked.
/// - **`RING_PACKED`** (bit 34, high word): the entire ring format changes. The validator
///   understands only the split ring, so a packed ring would be read by the device and validated by
///   nobody.
///
/// Forcing both off keeps every descriptor the device ever sees on the split-ring path the
/// validator actually covers. The honest driver negotiates neither, so nothing legitimate breaks.
/// The shadow descriptor ring ([`validate_and_shadow`]) is the structural fix that removes the
/// underlying race; this stripping stays as defence in depth, so the transport refuses a format it
/// cannot police even before a descriptor is built. See notes/dma.md.
fn sanitize_driver_features(sel: u32, val: u32) -> u32 {
    const F_INDIRECT_DESC_LO: u32 = 1 << 28; // feature bit 28
    const F_RING_PACKED_HI: u32 = 1 << (34 - 32); // feature bit 34
    match sel {
        0 => val & !F_INDIRECT_DESC_LO,
        1 => val & !F_RING_PACKED_HI,
        _ => val,
    }
}

/// Set up queue `queue` with `num` entries. The kernel programs the ring addresses, so the driver
/// never gets to choose them:
///
/// - **Descriptor table and available ring** point at this queue's block in the kernel-private
///   **shadow** page. The device reads its descriptors from memory the driver cannot write; the
///   driver builds its own copies in its region and the kernel validates and copies them across on
///   `notify`.
/// - **Used ring** stays in this queue's block in the driver's region, so the driver reads
///   completions directly. The device only ever *writes* indices and lengths there, never
///   addresses, so nothing to confine.
///
/// A device with several queues (virtio-net: receive on 0, transmit on 1) calls this once per
/// queue; each queue's rings sit at `queue * RING_BLOCK` in both regions, so they never overlap.
pub(crate) fn setup_queue(id: usize, num: u16, queue: u16) -> Result<(), TransportError> {
    if queue as usize >= MAX_QUEUES {
        return Err(TransportError::BadQueue);
    }
    let mut devs = DEVICES.lock();
    let dev = devs.get_mut(id).ok_or(TransportError::NoDevice)?;

    // Every queue's ring block must lie inside the driver's DMA region. The last block ends at
    // `MAX_QUEUES * RING_BLOCK`, but this queue only needs its own block plus a ring area.
    let block = queue_block(queue);
    if num == 0 || num > QSIZE || dev.direct_memory_access_size < block + RING_END {
        return Err(TransportError::BadQueue);
    }
    if dev.transport.queue_num_max(queue) < num {
        return Err(TransportError::BadQueue);
    }

    let desc = dev.shadow_base + block + DESC_OFF; // the SHADOW descriptor table (device-read)
    let avail = dev.shadow_base + block + AVAIL_OFF; // the SHADOW available ring
    let used = dev.direct_memory_access_base + block + USED_OFF; // the used ring stays in the driver's region
    dev.transport.setup_queue(queue, num, desc, avail, used);
    Ok(())
}

/// **The validated "go" for queue `queue`.** Validate the descriptor chains the driver has newly
/// published on that queue, copy the validated ones into the queue's shadow ring the device reads,
/// and only then ring the device. If any descriptor escapes the driver's DMA region, the shadow is
/// not published, the device is NOT notified, and the driver gets `DmaEscape`. Each queue keeps its
/// own last-validated index, so a receive submit and a transmit submit never interfere.
pub(crate) fn notify(id: usize, queue: u16) -> Result<(), TransportError> {
    if queue as usize >= MAX_QUEUES {
        return Err(TransportError::BadQueue);
    }
    let mut devs = DEVICES.lock();
    let dev = devs.get_mut(id).ok_or(TransportError::NoDevice)?;

    // A queue nobody set up has no doorbell (milestone 112). Before this check, a PCI device's
    // `notify_addr[queue]` was still zero here and the ring below wrote a u16 through
    // `phys_to_virt(0)`, which is a kernel store at an address the driver chose the timing of. The
    // range check above was not enough: an in-range queue can still be an unconfigured one.
    if !dev.transport.is_doorbell_ready(queue) {
        return Err(TransportError::BadQueue);
    }

    let block = queue_block(queue);
    let driver_desc = dev.direct_memory_access_base + block + DESC_OFF;
    let driver_avail = dev.direct_memory_access_base + block + AVAIL_OFF;
    let shadow_desc = dev.shadow_base + block + DESC_OFF;
    let shadow_avail = dev.shadow_base + block + AVAIL_OFF;
    let to_idx = direct_memory_access_read16(driver_avail + 2); // the driver's avail.idx for this queue

    let ok = validate_and_shadow(
        dev.direct_memory_access_base,
        dev.direct_memory_access_size,
        driver_desc,
        driver_avail,
        shadow_desc,
        shadow_avail,
        dev.last_avail[queue as usize],
        to_idx,
        &|p| direct_memory_access_read16(p),
        &|p| direct_memory_access_read64(p),
        &|p, v| direct_memory_access_write16(p, v),
        &|p, v| direct_memory_access_write64(p, v),
    );
    if !ok {
        return Err(TransportError::DmaEscape);
    }
    dev.last_avail[queue as usize] = to_idx;

    // The shadow writes above must be globally visible before the device is rung: the device is a
    // separate observer that will read the shadow by DMA. See
    // arch::direct_memory_access_write_barrier.
    crate::arch::direct_memory_access_write_barrier();
    dev.transport.notify_queue(queue);
    Ok(())
}

/// **Reset device `dev` and take it through the modern handshake up to `FEATURES_OK`**, accepting
/// `VERSION_1` and, when the device offers it, `ACCESS_PLATFORM`, and nothing else. The shared first
/// half of the two IOMMU test hooks below; the caller sets up a queue and sets `DRIVER_OK`.
#[cfg(test)]
fn reset_to_features_ok(dev: &mut Device) {
    // Status bits and the two feature words, in the mmio vocabulary the transport speaks.
    const S_ACK: u32 = 1;
    const S_DRIVER: u32 = 2;
    const S_FEATURES_OK: u32 = 8;
    const F_VERSION_1_HI: u32 = 1; // feature bit 32
    const F_ACCESS_PLATFORM_HI: u32 = 1 << 1; // feature bit 33 (set when behind an IOMMU)

    // Reset, then the modern handshake up to FEATURES_OK.
    dev.transport.write_reg(REG_STATUS, 0);
    // Drain after the reset, not before it. An earlier test may have left this device running a
    // queue that the new domain does not map, and it faults until the reset stops it; a drain before
    // the reset left those stale records to be read as this test's (milestone 609 (the system tests
    // leave the kernel crate) found it on x86_64, once the kernel's tests booted without the suite's).
    while crate::iommu::take_fault().is_some() {}
    dev.transport.write_reg(REG_STATUS, S_ACK);
    dev.transport.write_reg(REG_STATUS, S_ACK | S_DRIVER);
    dev.transport.write_reg(REG_DRIVER_FEATURES_SEL, 0);
    dev.transport.write_reg(REG_DRIVER_FEATURES, 0);
    dev.transport.write_reg(REG_DEVICE_FEATURES_SEL, 1);
    let dev_hi = dev.transport.read_reg(REG_DEVICE_FEATURES);
    let mut ack_hi = F_VERSION_1_HI;
    if dev_hi & F_ACCESS_PLATFORM_HI != 0 {
        ack_hi |= F_ACCESS_PLATFORM_HI; // required when iommu_platform=on, else FEATURES_OK sticks off
    }
    dev.transport.write_reg(REG_DRIVER_FEATURES_SEL, 1);
    dev.transport.write_reg(REG_DRIVER_FEATURES, ack_hi);
    dev.transport
        .write_reg(REG_STATUS, S_ACK | S_DRIVER | S_FEATURES_OK);
}

/// The status a test hook sets once its queue is up: `ACKNOWLEDGE | DRIVER | FEATURES_OK |
/// DRIVER_OK`.
#[cfg(test)]
const TEST_HOOK_DRIVER_OK: u32 = 1 | 2 | 8 | 4;

/// **Test hook: make device `id` write into its shadow page, which its domain maps read-only, and
/// return what the page holds afterwards** (provisional milestone 767 (AMD-Vi hardening before the
/// first AMD boot)).
///
/// The used ring is the one structure a split virtqueue's device writes, so this points queue 0's
/// used ring **into the shadow page** and leaves the descriptor table and available ring in the
/// driver's (writable) region, so that the device's first touch of the shadow page is through the
/// used ring. The used ring's `flags` word is filled with `0xffff` from the CPU first. A virtio-blk
/// device answers a kick by setting and then clearing `VRING_USED_F_NO_NOTIFY` in that word (QEMU
/// 11.1.1, `virtio_blk_handle_vq` through `virtio_queue_set_notification`, read for this hook; it
/// does so whenever `VIRTIO_RING_F_EVENT_IDX` is not negotiated, which this handshake never
/// accepts), so a device that can write the page leaves `0xfffe` there and one that cannot leaves
/// `0xffff`. The available ring's index is zero, so there is no request to process: the word is
/// the only thing the device writes.
///
/// **What QEMU decides here: no fault record is promised.** The device reads the word before it
/// writes it, and QEMU's AMD-Vi model caches the read's translation and then refuses the write
/// from that cache without logging an event (`amdvi_do_translate`'s IOTLB hit returns the cached
/// permission, read at 11.1.1). The other three units' models were not read for this. So the test
/// asks whether the write landed, which every unit answers the same way, rather than whether a
/// fault was recorded.
#[cfg(test)]
pub(crate) fn provoke_write_to_shadow(id: usize) -> u16 {
    let mut devs = DEVICES.lock();
    let dev = devs
        .get_mut(id)
        .expect("provoke_write_to_shadow: no such device");
    reset_to_features_ok(dev);

    let num = dev.transport.queue_num_max(0).min(QSIZE);
    let desc = dev.direct_memory_access_base + DESC_OFF;
    let avail = dev.direct_memory_access_base + AVAIL_OFF;
    let used = dev.shadow_base + USED_OFF;
    // avail = { flags, idx = 0 }: nothing to process. used.flags = the canary.
    direct_memory_access_write16(avail, 0);
    direct_memory_access_write16(avail + 2, 0);
    direct_memory_access_write16(used, 0xffff);
    direct_memory_access_write16(used + 2, 0);
    crate::arch::direct_memory_access_write_barrier();
    dev.transport.setup_queue(0, num, desc, avail, used);
    dev.transport.write_reg(REG_STATUS, TEST_HOOK_DRIVER_OK);
    // Under TCG QEMU handles the kick synchronously in this vCPU thread, so the device's writes
    // (or its refused attempts) are done when this returns.
    dev.transport.notify_queue(0);
    let flags = direct_memory_access_read16(used);
    // Reset, as `provoke_iommu_escape` does, so a later test does not inherit this queue.
    dev.transport.write_reg(REG_STATUS, 0);
    flags
}

/// **Test hook: make device `id` emit an address its IOMMU domain does not map, so the IOMMU must
/// fault** (milestone 16b confinement proof, kernel/src/iommu.rs).
///
/// The software shadow ring refuses an out-of-region descriptor *before* the device is ever rung,
/// so a normal path can never reach the hardware. To prove the IOMMU itself confines the device we
/// deliberately go around that: bring the device up at the transport level and point its **available
/// ring** at `avail_out_of_domain`, a frame the domain does not include. On the kick, the device's
/// first act is to read the available ring; that read is an out-of-domain IOVA, so the IOMMU faults
/// it and records the event. The CPU can still fill the frame through the direct map (CPU accesses
/// do not go through the IOMMU); only the *device's* view of it is unmapped, which is the whole
/// point. No block-device knowledge is needed: the fault happens before any descriptor is read.
#[cfg(test)]
pub(crate) fn provoke_iommu_escape(id: usize, avail_out_of_domain: u64) {
    let mut devs = DEVICES.lock();
    let dev = devs
        .get_mut(id)
        .expect("provoke_iommu_escape: no such device");
    reset_to_features_ok(dev);

    // Queue 0: descriptor table on the (in-domain) shadow, used ring in the (in-domain) driver
    // region, available ring pointed OUT of the domain. Then DRIVER_OK to make the queue live.
    let num = dev.transport.queue_num_max(0).min(QSIZE);
    let desc = dev.shadow_base + DESC_OFF;
    let used = dev.direct_memory_access_base + USED_OFF;
    dev.transport
        .setup_queue(0, num, desc, avail_out_of_domain, used);
    dev.transport.write_reg(REG_STATUS, TEST_HOOK_DRIVER_OK);

    // Publish an available head from the CPU side (bypassing the IOMMU) so the device has a reason to
    // read the available ring. avail = { u16 flags; u16 idx; u16 ring[] }.
    direct_memory_access_write16(avail_out_of_domain, 0); // flags
    direct_memory_access_write16(avail_out_of_domain + 2, 1); // idx = 1: one entry available
    direct_memory_access_write16(avail_out_of_domain + 4, 0); // ring[0] = head 0
    crate::arch::direct_memory_access_write_barrier();

    // Kick. The device now reads the available ring at an unmapped IOVA and the IOMMU faults. Under
    // TCG QEMU processes the kick synchronously in this vCPU thread, so the fault is already in the
    // IOMMU's queue by the time this returns.
    dev.transport.notify_queue(0);

    // Reset the device so a later test that re-registers it does not inherit this deliberately
    // broken queue. The fault event lives in the IOMMU's own queue, which a device reset does not
    // touch, so this does not erase what the caller is about to drain.
    dev.transport.write_reg(REG_STATUS, 0);
}

#[cfg(test)]
mod tests;
