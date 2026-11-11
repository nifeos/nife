//! **The direct grant's transport**: the driver drives its virtio-pci function's registers itself,
//! from the pages its starter mapped, and the IOMMU, not the kernel, confines what the device
//! reaches. A driver granted mediated never enters this module.
//!
//! The rest of the crate speaks virtio-mmio's register vocabulary, the one the kernel's `Virtio`
//! capability answers. On a direct grant [`read`], [`write`], [`setup_queue`] and [`notify`]
//! answer the same vocabulary from the common configuration, the ISR (Interrupt Status Register)
//! byte and the doorbells, as the
//! kernel's own `Transport::Pci` does for a mediated function (`kernel/src/virtio.rs`), so every
//! driver role runs unchanged on either grant. The rings sit at the offsets the mediated grant
//! uses, in the driver's own region, and the device reads them there. See
//! design/roadmap/1062-the-device-control-planes-leave-the-kernel.md, "Step 7".
//!
//! Name: provisional, minted 2026-11-11 (UTC) by lane/1062-virtio-direct-transport, after the
//! grant it serves.

use core::sync::atomic::{AtomicU64, Ordering};

use driver_process_contract::virtual_input_output::{DATA_PLANE_KIND, HandOff, REGISTER_KIND};

// The mmio offsets the rest of the crate reads and writes (virtio 1.2 section 4.2.2).
const MAGIC: u64 = 0x000;
const DEVICE_FEATURES: u64 = 0x010;
const DEVICE_FEATURES_SELECT: u64 = 0x014;
const DRIVER_FEATURES: u64 = 0x020;
const DRIVER_FEATURES_SELECT: u64 = 0x024;
const INTERRUPT_STATUS: u64 = 0x060;
const STATUS: u64 = 0x070;
const CONFIGURATION: u64 = 0x100;

// The common configuration's registers (virtio 1.2 section 4.1.4.3).
const COMMON_DEVICE_FEATURE_SELECT: u64 = 0x00;
const COMMON_DEVICE_FEATURE: u64 = 0x04;
const COMMON_DRIVER_FEATURE_SELECT: u64 = 0x08;
const COMMON_DRIVER_FEATURE: u64 = 0x0c;
const COMMON_CONFIGURATION_INTERRUPT_VECTOR: u64 = 0x10;
const COMMON_DEVICE_STATUS: u64 = 0x14;
const COMMON_QUEUE_SELECT: u64 = 0x16;
const COMMON_QUEUE_ENTRIES_REGISTER: u64 = 0x18;
const COMMON_QUEUE_INTERRUPT_VECTOR: u64 = 0x1a;
const COMMON_QUEUE_ENABLE: u64 = 0x1c;
const COMMON_QUEUE_NOTIFY_OFF: u64 = 0x1e;
const COMMON_QUEUE_DESCRIPTOR: u64 = 0x20;
const COMMON_QUEUE_DRIVER: u64 = 0x28;
const COMMON_QUEUE_DEVICE: u64 = 0x30;

/// The stride between queues' ring blocks in the DMA region, and each ring's offset in its block:
/// the layout the mediated grant's kernel uses (`direct_memory_access_validator`), so a driver
/// role writes its descriptors at one place on either grant.
const RING_BLOCK: u64 = 0x200;
const DESCRIPTOR_TABLE_OFFSET_BYTES: u64 = 0x000;
const AVAILABLE_RING_OFFSET_BYTES: u64 = 0x080;
const USED_RING_OFFSET_BYTES: u64 = 0x100;

/// The queues a driver role uses: a disk one, a NIC two.
const QUEUES: usize = 2;

/// The register window's base, zero while the grant is mediated.
static REGISTERS: AtomicU64 = AtomicU64::new(0);
/// The DMA region's base, zero while the grant is mediated.
static REGION: AtomicU64 = AtomicU64::new(0);
/// The three handoff words, [`HandOff::pack`]'s.
static WORDS: [AtomicU64; 3] = [const { AtomicU64::new(0) }; 3];
/// Each queue's doorbell, as a virtual address, zero until [`setup_queue`] resolves it.
static DOORBELLS: [AtomicU64; QUEUES] = [const { AtomicU64::new(0) }; QUEUES];

/// **What a direct grant gives the driver to run**: its role, and its DMA region's physical and
/// virtual bases.
///
/// Name: provisional, minted 2026-11-11 (UTC) by lane/1062-virtio-direct-transport.
#[derive(Debug, Clone, Copy)]
pub struct Granted {
    /// The role to run, in the driver's own numbering.
    pub role: u64,
    /// The DMA region's physical base, which every descriptor names.
    pub physical_address: u64,
    /// Where the region is mapped.
    pub virtual_address: u64,
}

/// **Take a direct grant, if this process holds one**: the startup page lists a register window
/// and a DMA region, and `words` are a handoff whose structures fit the window. `None` for a
/// process granted mediated, whose startup page lists no register window; a direct grant whose
/// words do not unpack, or do not fit, panics, because no kernel sends them.
///
/// Called once, first thing in `_start`, before any register access.
///
/// Name: provisional, minted 2026-11-11 (UTC) by lane/1062-virtio-direct-transport.
pub fn take(words: [u64; 3]) -> Option<Granted> {
    // SAFETY: every spawner maps a startup page at its address, read-only, for the process's life;
    // a process the kernel builds by hand is never this driver.
    let page = unsafe { startup_page_contract::StartupPage::mapped() }?;
    let registers = page.window(REGISTER_KIND)?;
    let region = page
        .window(DATA_PLANE_KIND)
        .expect("a direct grant lists its DMA region");
    let hand_off = HandOff::unpack(words).expect("a direct grant's words unpack");
    assert!(
        hand_off.fits(registers.pages),
        "the handoff's structures fit the register window"
    );
    REGISTERS.store(registers.virtual_address, Ordering::Relaxed);
    REGION.store(region.virtual_address, Ordering::Relaxed);
    for (slot, word) in WORDS.iter().zip(words) {
        slot.store(word, Ordering::Relaxed);
    }
    Some(Granted {
        role: u64::from(hand_off.role),
        physical_address: hand_off.data_plane_physical_address,
        virtual_address: region.virtual_address,
    })
}

/// **Whether this process holds a direct grant.**
///
/// Name: provisional, minted 2026-11-11 (UTC) by lane/1062-virtio-direct-transport.
pub fn is_direct() -> bool {
    REGISTERS.load(Ordering::Relaxed) != 0
}

/// The DMA region's virtual base on a direct grant, or `None` on a mediated one.
pub(crate) fn region() -> Option<u64> {
    match REGION.load(Ordering::Relaxed) {
        0 => None,
        va => Some(va),
    }
}

fn hand_off() -> HandOff {
    let words = core::array::from_fn(|n| WORDS[n].load(Ordering::Relaxed));
    HandOff::unpack(words).expect("take() stored a handoff that unpacks")
}

/// One register of the window, by its byte offset there.
fn at(offset: u64) -> u64 {
    REGISTERS.load(Ordering::Relaxed) + offset
}

fn load<T: Copy>(offset: u64) -> T {
    // SAFETY: `take` checked the structures fit the window the starter mapped, device-typed and
    // read-write, for the process's life; every offset here is a structure base plus a register
    // inside that structure's specified layout.
    unsafe { core::ptr::read_volatile(at(offset) as *const T) }
}

fn store<T: Copy>(offset: u64, value: T) {
    // SAFETY: as `load`.
    unsafe { core::ptr::write_volatile(at(offset) as *mut T, value) }
}

/// **Answer a read in the mmio vocabulary**, from the common configuration, the ISR byte or the
/// device configuration. Reading the ISR byte acknowledges the interrupt and deasserts the legacy pin
/// interrupt.
pub(crate) fn read(offset: u64) -> u32 {
    let h = hand_off();
    let common = u64::from(h.common);
    match offset {
        // virtio-pci has no magic register; answered so a driver's check means the same here.
        MAGIC => 0x7472_6976,
        DEVICE_FEATURES => load::<u32>(common + COMMON_DEVICE_FEATURE),
        INTERRUPT_STATUS => u32::from(load::<u8>(u64::from(h.interrupt_status))),
        STATUS => u32::from(load::<u8>(common + COMMON_DEVICE_STATUS)),
        o if o == CONFIGURATION || o == CONFIGURATION + 4 => match h.device {
            Some(device) => load::<u32>(u64::from(device) + (o - CONFIGURATION)),
            None => 0,
        },
        _ => 0,
    }
}

/// **Answer a write in the mmio vocabulary.** The interrupt acknowledgement has no virtio-pci
/// register: the ISR read already acknowledged.
pub(crate) fn write(offset: u64, value: u32) {
    let common = u64::from(hand_off().common);
    match offset {
        DEVICE_FEATURES_SELECT => store::<u32>(common + COMMON_DEVICE_FEATURE_SELECT, value),
        DRIVER_FEATURES_SELECT => store::<u32>(common + COMMON_DRIVER_FEATURE_SELECT, value),
        DRIVER_FEATURES => store::<u32>(common + COMMON_DRIVER_FEATURE, value),
        STATUS => store::<u8>(common + COMMON_DEVICE_STATUS, value as u8),
        _ => {}
    }
}

/// **Set up queue `queue` with `size` entries**, its rings at the mediated layout's offsets in the
/// driver's own region, aimed at `physical_address`'s pages. Writes the armed interrupt table entry into the
/// queue's and the configuration's vector, resolves the queue's doorbell and enables it. Returns 0,
/// or -1 for a queue past what this transport tracks or a device that refuses the vector.
pub(crate) fn setup_queue(size: u64, queue: u64) -> i64 {
    if queue as usize >= QUEUES {
        return -1;
    }
    let h = hand_off();
    let common = u64::from(h.common);
    store::<u16>(common + COMMON_QUEUE_SELECT, queue as u16);
    store::<u16>(common + COMMON_QUEUE_ENTRIES_REGISTER, size as u16);
    if let Some(vector) = h.interrupt_vector {
        store::<u16>(common + COMMON_QUEUE_INTERRUPT_VECTOR, vector);
        store::<u16>(common + COMMON_CONFIGURATION_INTERRUPT_VECTOR, vector);
        if load::<u16>(common + COMMON_QUEUE_INTERRUPT_VECTOR) != vector {
            return -1;
        }
    }
    let block = h.data_plane_physical_address + queue * RING_BLOCK;
    store::<u64>(
        common + COMMON_QUEUE_DESCRIPTOR,
        block + DESCRIPTOR_TABLE_OFFSET_BYTES,
    );
    store::<u64>(
        common + COMMON_QUEUE_DRIVER,
        block + AVAILABLE_RING_OFFSET_BYTES,
    );
    store::<u64>(common + COMMON_QUEUE_DEVICE, block + USED_RING_OFFSET_BYTES);
    let notify_off = load::<u16>(common + COMMON_QUEUE_NOTIFY_OFF);
    DOORBELLS[queue as usize].store(at(h.doorbell(notify_off)), Ordering::Relaxed);
    store::<u16>(common + COMMON_QUEUE_ENABLE, 1);
    0
}

/// **Ring queue `queue`'s doorbell**: one store, no system call. Returns 0, or -1 for a queue
/// [`setup_queue`] has not resolved.
pub(crate) fn notify(queue: u64) -> i64 {
    let Some(doorbell) = DOORBELLS
        .get(queue as usize)
        .map(|d| d.load(Ordering::Relaxed))
    else {
        return -1;
    };
    if doorbell == 0 {
        return -1;
    }
    // The ring writes before the doorbell, to the device and not only to other CPUs.
    user_mode_runtime::virtio::doorbell_barrier();
    // SAFETY: `setup_queue` resolved the doorbell inside the notify structure, which `take` checked
    // lies in the device-typed window.
    unsafe { core::ptr::write_volatile(doorbell as *mut u16, queue as u16) };
    0
}
