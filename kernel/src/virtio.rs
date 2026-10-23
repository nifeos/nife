//! virtio-mmio enumeration, and the confined transport a userspace virtio driver drives. Not a
//! driver.
//!
//! The kernel reads each slot's identity registers and hands the device to the driver that claims
//! its type (`components/src/block_driver.rs` for a disk). It keeps the two DMA-critical powers,
//! the ring addresses and the "go", and validates every descriptor before the device sees it. See
//! notes/virtio.md, "The kernel's half", and notes/dma.md.

use crate::arch::mmu::{self, VIRTIO_IRQ_BASE, VIRTIO_MMIO_BASE, VIRTIO_SLOT_STRIDE};

/// The virtio-mmio slot stride, from the arch: aarch64's `virt` has 32 slots 0x200 apart, RISC-V's
/// has 8 slots 0x1000 apart. The probe walks `mmu::virtio_slots()` of them, which is zero on a
/// machine whose tree names no `virtio,mmio` node (on the TH1520 that address is DRAM).
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

/// How many virtio block devices are on the mmio bus, for the roster and for
/// [`find_block_device_n`], where it marks the end of the mmio half. The same walk as
/// [`mmio_block_device_n`], so the ordinals agree by construction; reads two ID registers per slot.
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

/// Scan the bus for the first virtio entropy source. `None` if there is no RNG attached. The PCIe
/// twin is [`crate::pci::find_rng_device`], and the tests run the entropy service over each (§18
/// (the PCIe transport: one driver, two buses, the seam in the kernel)).
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))] // entropy_service is the caller, and the m56 tests drive it
pub(crate) fn find_entropy_device() -> Option<VirtioMmioDevice> {
    find_by_device_id(DEVICE_ID_ENTROPY)
}

/// One block device, resolved on whichever bus it is on (provisional name): what
/// [`find_block_device_n`] hands a wiring. Holds the transport, the interrupt to bind, and the
/// IOMMU requester id, `None` on virtio-mmio (no IOMMU fronts that bus) and `Some` on PCI, where
/// `virtio::register` keys the domain on it.
pub(crate) struct BlockDevice {
    pub transport: Transport,
    pub intid: u32,
    pub rid: Option<u32>,
}

/// The `n`-th (0-based) block device this machine offers, on any bus. `None` if there is no such
/// disk.
///
/// The order is a contract, the roster's row order: every virtio-mmio slot in slot order, then
/// every modern virtio-pci function in bus:device.function order. Resolving a PCI ordinal brings
/// the function up; the mmio half only reads identity registers. See notes/virtio.md, "The
/// kernel's half".
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
// DMA confinement: the kernel owns the device's transport, keeps the ring addresses and the "go",
// and validates every descriptor against the driver's DMA region first. See notes/dma.md.
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
/// The device-specific config area, in the mmio vocabulary (virtio spec 4.2.2: offset 0x100). A blk
/// device's capacity is at its offset 0, read by `blk::SIZE` in two 32-bit halves. On PCI it is
/// [`Transport::Pci::device_config`].
const REG_DEVICE_CONFIG: u64 = 0x100;

/// The transport seam: everything above it speaks virtio-mmio's register vocabulary, the one
/// `abi::virtio` exposes, and a `Transport` answers it on whichever bus the device sits. The mmio
/// variant passes through; the pci variant translates to the common-config layout, the ISR byte
/// and the notify doorbell. See notes/pcie.md, "The transport seam".
pub(crate) enum Transport {
    /// A virtio-mmio slot: the vocabulary IS this device's register block.
    Mmio { mmio_phys: u64 },
    /// A modern virtio-pci function, resolved by kernel/src/pci.rs: the common-config block, the
    /// ISR byte, and the notify doorbell parameters, all physical addresses in a mapped BAR.
    Pci {
        common: u64,
        notify_base: u64,
        notify_mult: u32,
        /// The virtio device type (2 = block, 1 = net, 16 = gpu), from the PCI device id
        /// (`0x1040 + type`); a driver's `DeviceID` read is answered from it.
        device_type: u32,
        /// Each queue's resolved doorbell, computed at `setup_queue` (it needs `queue_notify_off`,
        /// which is only readable with that queue selected, and it differs per queue). Zero until
        /// the queue is set up. Indexed by queue number; see `MAX_QUEUES`.
        notify_addr: [u64; MAX_QUEUES],
        isr: u64,
        /// Which MSI-X table entry this function's queues should raise, or
        /// `pci::VIRTIO_MSIX_NO_VECTOR` when it delivers INTx. Written at
        /// [`Transport::setup_queue`], since the field is behind the queue selector and a device
        /// reset clears it.
        msix_vector: u16,
        /// The device-specific config region's physical address, `None` for net, rng and console.
        /// The mmio arm reads the same area at [`REG_DEVICE_CONFIG`].
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
/// Safe fns over a module invariant: they are private, and every call site passes a field of
/// `Transport::Pci` (resolved from a mapped BAR, copied only by `Transport::pci`) plus a bounded
/// offset. `notify_addr[q]` is zero until set up, so [`notify`] checks
/// [`Transport::is_doorbell_ready`] first. See notes/virtio.md, "The kernel's half".
fn pread<T: Copy>(phys: u64) -> T {
    // SAFETY: `phys` is a field of a `Transport::Pci`, plus at most a common-config offset or a
    // device-config offset. Those fields come from `pci.rs`'s BAR resolution and reach this module
    // only through `Transport::pci`; this fn is private, so that closed set of call sites is the
    // whole population. See the module invariant above.
    unsafe { core::ptr::read_volatile(mmu::phys_to_virt(phys) as *const T) }
}
fn pwrite<T: Copy>(phys: u64, v: T) {
    // SAFETY: as above, and the same closed call set: `pwrite` is private to this module too.
    unsafe { core::ptr::write_volatile(mmu::phys_to_virt(phys) as *mut T, v) }
}

impl Transport {
    /// The PCI transport for an enumerated function (kernel/src/pci.rs). The one place its fields
    /// are copied, which [`pread`]'s invariant rests on.
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
                // pci has no magic/version/device-id registers; synthesized so the driver's
                // sanity checks mean the same on both buses.
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
                // `blk::SIZE`'s two 32-bit halves. No region answers as an mmio hole would.
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

    /// Queue `q`'s maximum size, as the device reports it; each queue is selected and read on its
    /// own, since a device may size its queues differently.
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
                // Selected, and after the driver's reset cleared it. A device that refuses the
                // vector reads back `NO_VECTOR`, and would otherwise hang its first completion.
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
                // `queue_notify_off` is readable only with this queue selected.
                let off = pread::<u16>(*common + PCI_QUEUE_NOTIFY_OFF) as u64;
                notify_addr[q as usize] = *notify_base + off * (*notify_mult as u64);
                pwrite::<u16>(*common + PCI_QUEUE_ENABLE, 1);
            }
        }
    }

    /// Is queue `q`'s doorbell a real address yet? On PCI it is zero until
    /// [`Transport::setup_queue`], and ringing it would write through `phys_to_virt(0)`. Always
    /// true on mmio, whose one notify register takes the queue number. [`notify`] checks it.
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
/// Aliases, not copies, of `crates/direct_memory_access_validator`'s layout, so the layout
/// `distinct_queues_occupy_disjoint_blocks` proves is the layout that runs.
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

/// The stride between successive queues' ring areas, in both the driver's DMA region and the
/// kernel-private shadow. 0x200 holds a whole ring area (`RING_END`) and keeps queue 0 identical to
/// the disk's single-queue layout, whose data buffers begin at 0x200 (a disk has no queue 1).
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

// Descriptor flags, for the attacker tests in `virtio/tests.rs`, which build descriptor words by
// hand; the validator's own are `direct_memory_access_validator::F_NEXT` and `F_INDIRECT`.
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
    /// Physical base of the kernel-private shadow page: the descriptor table and available ring
    /// the device reads, copied here by `notify` after validation, so the bytes the device reads
    /// are the bytes the kernel validated. See notes/dma.md, "The shadow descriptor ring".
    shadow_base: u64,
}

/// The most virtio transports we will drive. Fixed-size, so probing never allocates.
///
/// Each `register` takes a slot for the life of the boot, so this counts devices a boot has ever
/// wired, not devices that exist.
///
/// This is an exception and a foot gun: see notes/virtio.md, "The kernel's half", and
/// `design/roadmap/0647-a-virtio-slot-comes-back-when-its-driver-dies.md`, which the next lane that
/// needs a device should take rather than a bump.
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
/// `rid` is the PCIe requester id, `None` on virtio-mmio. With an IOMMU active and a `rid`, the
/// device is confined in hardware to its DMA region plus the shadow page before it is entered in
/// the table; the shadow ring stays as defence in depth (notes/dma.md).
pub(crate) fn register(
    transport: Transport,
    direct_memory_access_base: u64,
    direct_memory_access_size: u64,
    rid: Option<u32>,
) -> usize {
    // Zeroed so a stale word can never look like a valid descriptor before the first copy.
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

/// The security-critical step: validate the driver's descriptors and copy them into the shadow
/// ring the device reads. Returns false if any descriptor escapes the DMA region.
///
/// A thin adapter over `crates/direct_memory_access_validator::validate_and_shadow`, which Kani
/// proves for every input: ring physical addresses, the direct-map closures and `QSIZE` pass
/// straight through. See notes/dma.md and notes/verification.md.
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
/// `INDIRECT_DESC` (bit 28) and `RING_PACKED` (bit 34) both change what the device reads
/// descriptors from, which the validator assumes it controls, so both are forced off. See
/// notes/dma.md, "The shadow descriptor ring".
fn sanitize_driver_features(sel: u32, val: u32) -> u32 {
    const F_INDIRECT_DESC_LO: u32 = 1 << 28; // feature bit 28
    const F_RING_PACKED_HI: u32 = 1 << (34 - 32); // feature bit 34
    match sel {
        0 => val & !F_INDIRECT_DESC_LO,
        1 => val & !F_RING_PACKED_HI,
        _ => val,
    }
}

/// Set up queue `queue` with `num` entries, the kernel choosing the ring addresses: descriptor
/// table and available ring in the kernel-private shadow page, used ring in the driver's region
/// (the device writes only indices and lengths there). Each queue's rings sit at
/// `queue * RING_BLOCK` in both. Refuses a queue that is out of range or does not fit.
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

/// The validated "go" for queue `queue`. Validate the descriptor chains the driver has newly
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

    // An in-range queue may still be unconfigured, and then has no doorbell.
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

/// Reset device `dev` and take it through the modern handshake up to `FEATURES_OK`, accepting
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
    // Drain after the reset, not before: an earlier test's queue keeps faulting until the reset.
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

/// Test hook: make device `id` write into its shadow page, which its domain maps read-only, and
/// return the used ring's `flags` word afterwards: `0xffff` if the write was refused, `0xfffe` if
/// it landed.
///
/// Points queue 0's used ring into the shadow page, fills `flags` with `0xffff`, and kicks with
/// nothing available, so the device's one write is clearing `VRING_USED_F_NO_NOTIFY`. No fault
/// record is promised. See notes/virtio.md, "The kernel's half".
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

/// Test hook: make device `id` emit an address its IOMMU domain does not map, so the IOMMU must
/// fault.
///
/// Goes around the shadow ring, which would refuse first: queue 0's available ring is pointed at
/// `avail_out_of_domain`, filled from the CPU (which the IOMMU does not police), and kicked. The
/// device's first read is the out-of-domain IOVA. See notes/iommu.md.
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

    // Under TCG QEMU processes the kick synchronously in this vCPU thread, so the fault is
    // already in the IOMMU's queue when this returns.
    dev.transport.notify_queue(0);

    // Reset so a later test does not inherit this queue. The fault event is in the IOMMU's own
    // queue, which a device reset does not touch.
    dev.transport.write_reg(REG_STATUS, 0);
}

#[cfg(test)]
mod tests;
