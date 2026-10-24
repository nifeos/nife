//! **`virtio-drivers` over the kernel's confined transport**: [`ConfinedTransport`], the crate's
//! `Transport` over a `Virtio` capability, and [`WindowHal`], its `Hal` over the one DMA window the
//! spawner mapped. A driver built on the crate's `VirtQueue` takes both and drives a device it never
//! maps, with the kernel still placing every ring and validating every descriptor.
//!
//! The kernel decides where a queue's rings go (`SETUP_QUEUE` programs the fixed offsets
//! `direct_memory_access_validator` owns), and the crate's `VirtQueue` allocates its rings through
//! `Hal::dma_alloc` and hands their addresses to `Transport::queue_set`. The two meet because
//! [`WindowHal`] answers each queue's two allocations with exactly the kernel's offsets, in queue
//! order, and [`ConfinedTransport::queue_set`] refuses any other address before it asks the kernel.
//! Behind the `virtio-drivers` feature, so a program that does not drive a device links nothing
//! of the crate. See notes/user-mode-runtime.md, "`virtio-drivers` over the confined transport".
//!
//! # BUGS
//!
//! - `WindowHal` hands out pieces of one page, not whole pages: a queue's descriptor table and
//!   available ring at its ring block, its used ring 0x100 bytes on. The crate's `Hal` contract
//!   promises page-sized, zeroed, unshared allocations; these are zeroed only over the ring bytes
//!   and the two of a queue overlap as pages. `VirtQueue` touches nothing past its rings, so this
//!   holds today. It is an exception and a foot gun: a crate change that wrote a whole allocation
//!   would write over the next ring or the window's buffers.
//! - `dma_alloc` serves only those ring allocations. A device driver that asks the crate for any
//!   other DMA memory (the GPU's framebuffer, an indirect table) gets a `DmaError`.
//! - Device configuration reads go through `READ_REGISTER`, one aligned word per byte. On PCI the
//!   kernel answers only the first eight bytes of the device's configuration and reads zero past
//!   them; writes are refused.
//! - The window lives in globals, because the crate's `Hal` methods take no `self`. One window per
//!   process: a program driving two devices cannot use this pair for both.
//!
//! Name: provisional (milestone 947, 2026-10-24 UTC), for the module, both types, [`Refusal`],
//! [`install_window`] and [`QUEUE_SIZE`].

use core::ptr::NonNull;
use core::sync::atomic::{AtomicU64, Ordering};

use direct_memory_access_validator::{
    AVAIL_OFF, DESC_OFF, LAYOUT_QSIZE, MAX_QUEUES, RING_END, USED_OFF, queue_block,
};
pub use virtio_drivers;
use virtio_drivers::transport::{DeviceStatus, DeviceType, InterruptStatus, Transport};
use virtio_drivers::{BufferDirection, Error, Hal, PhysAddr};

/// The most entries a queue may have under the confinement: a driver's `VirtQueue` size.
pub const QUEUE_SIZE: usize = LAYOUT_QSIZE as usize;

// The virtio-mmio register vocabulary the kernel answers on both buses (virtio spec 4.2.2).
const REG_MAGIC: u64 = 0x000;
const REG_DEVICE_ID: u64 = 0x008;
const REG_DEVICE_FEATURES: u64 = 0x010;
const REG_DEVICE_FEATURES_SEL: u64 = 0x014;
const REG_DRIVER_FEATURES: u64 = 0x020;
const REG_DRIVER_FEATURES_SEL: u64 = 0x024;
const REG_QUEUE_NUM_MAX: u64 = 0x034;
const REG_INTERRUPT_STATUS: u64 = 0x060;
const REG_INTERRUPT_ACK: u64 = 0x064;
const REG_STATUS: u64 = 0x070;
const REG_CONFIG_GENERATION: u64 = 0x0fc;
const REG_DEVICE_CONFIG: u64 = 0x100;

/// "virt", little-endian: what `REG_MAGIC` reads on a virtio device, synthesized on PCI.
const MAGIC: u32 = 0x7472_6976;

/// Why a [`ConfinedTransport`] could not be built or refused an operation. The crate's `Transport`
/// methods return nothing, so a refusal is kept and read back with [`ConfinedTransport::refusal`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// The capability's device does not read the virtio magic.
    NotVirtio,
    /// The device's type is not one the crate knows; the value is what `DeviceID` read.
    UnknownDevice(u32),
    /// The crate placed a queue's rings somewhere other than the kernel's fixed offsets in the
    /// window, so `SETUP_QUEUE` was not asked.
    Layout {
        /// The queue.
        queue: u16,
    },
    /// The kernel refused `SETUP_QUEUE`; `error` is its negative `abi::Error`.
    SetupQueue {
        /// The queue.
        queue: u16,
        /// The kernel's answer.
        error: i64,
    },
    /// The kernel refused `NOTIFY`; `error` is its negative `abi::Error` (`DeviceRefused` for a
    /// descriptor outside the window).
    Notify {
        /// The queue.
        queue: u16,
        /// The kernel's answer.
        error: i64,
    },
}

/// **The crate's `Transport`, over a `Virtio` capability.** Every register access is a
/// capability invocation the kernel checks; queue setup and notification go through the two
/// validated methods.
pub struct ConfinedTransport {
    slot: u64,
    device_type: DeviceType,
    /// Bit `q` set once queue `q` is set up, for `queue_used`.
    queues_set: u32,
    refusal: Option<Refusal>,
}

impl ConfinedTransport {
    /// The transport over the `Virtio` capability in `slot`. Refuses a device that does not read
    /// the virtio magic, or whose type the crate does not know.
    pub fn new(slot: u64) -> Result<Self, Refusal> {
        let transport = Self {
            slot,
            device_type: DeviceType::Block, // replaced below, once the register is read
            queues_set: 0,
            refusal: None,
        };
        if transport.read(REG_MAGIC) != MAGIC {
            return Err(Refusal::NotVirtio);
        }
        let id = transport.read(REG_DEVICE_ID);
        let device_type = DeviceType::try_from(id).map_err(|_| Refusal::UnknownDevice(id))?;
        Ok(Self {
            device_type,
            ..transport
        })
    }

    /// The first operation the kernel refused, if any: a driver checks this after setting up its
    /// queues and after notifying.
    pub fn refusal(&self) -> Option<Refusal> {
        self.refusal
    }

    fn refuse(&mut self, refusal: Refusal) {
        self.refusal.get_or_insert(refusal);
    }

    fn read(&self, off: u64) -> u32 {
        crate::virtio::read_register(self.slot, off) as u32
    }

    fn write(&mut self, off: u64, val: u32) {
        crate::virtio::write_register(self.slot, off, val as u64);
    }
}

impl Transport for ConfinedTransport {
    fn device_type(&self) -> DeviceType {
        self.device_type
    }

    fn read_device_features(&mut self) -> u64 {
        self.write(REG_DEVICE_FEATURES_SEL, 0);
        let low = self.read(REG_DEVICE_FEATURES) as u64;
        self.write(REG_DEVICE_FEATURES_SEL, 1);
        let high = self.read(REG_DEVICE_FEATURES) as u64;
        high << 32 | low
    }

    fn write_driver_features(&mut self, driver_features: u64) {
        self.write(REG_DRIVER_FEATURES_SEL, 0);
        self.write(REG_DRIVER_FEATURES, driver_features as u32);
        self.write(REG_DRIVER_FEATURES_SEL, 1);
        self.write(REG_DRIVER_FEATURES, (driver_features >> 32) as u32);
    }

    /// The device's queue size, capped at [`QUEUE_SIZE`]. A driver cannot select a queue, so this
    /// reads whichever the device has selected (queue 0 on PCI); `SETUP_QUEUE` checks the real
    /// queue's maximum and refuses a size it cannot take.
    fn max_queue_size(&mut self, _queue: u16) -> u32 {
        self.read(REG_QUEUE_NUM_MAX).min(QUEUE_SIZE as u32)
    }

    fn notify(&mut self, queue: u16) {
        let error = crate::virtio::notify(self.slot, queue as u64);
        if error < 0 {
            self.refuse(Refusal::Notify { queue, error });
        }
    }

    fn get_status(&self) -> DeviceStatus {
        DeviceStatus::from_bits_retain(self.read(REG_STATUS))
    }

    fn set_status(&mut self, status: DeviceStatus) {
        self.write(REG_STATUS, status.bits());
    }

    /// Modern virtio has no guest page size; nothing to write.
    fn set_guest_page_size(&mut self, _guest_page_size: u32) {}

    fn requires_legacy_layout(&self) -> bool {
        false
    }

    /// Ask the kernel to set up `queue`, after checking that the crate's three ring addresses are
    /// the ones `SETUP_QUEUE` programs. Either failure is kept as a [`Refusal`].
    fn queue_set(
        &mut self,
        queue: u16,
        size: u32,
        descriptors: PhysAddr,
        driver_area: PhysAddr,
        device_area: PhysAddr,
    ) {
        let block = window_pa() + queue_block(queue);
        if descriptors != block + DESC_OFF
            || driver_area != block + AVAIL_OFF
            || device_area != block + USED_OFF
        {
            self.refuse(Refusal::Layout { queue });
            return;
        }
        let error = crate::virtio::setup_queue(self.slot, size as u64, queue as u64);
        if error != 0 {
            self.refuse(Refusal::SetupQueue { queue, error });
            return;
        }
        self.queues_set |= 1 << queue;
    }

    /// The confinement has no method that tears one queue down; a device reset (status 0) is the
    /// only way, and a driver that wants it writes the status itself.
    fn queue_unset(&mut self, queue: u16) {
        self.queues_set &= !(1 << queue);
    }

    fn queue_used(&mut self, queue: u16) -> bool {
        queue < 32 && self.queues_set & 1 << queue != 0
    }

    fn ack_interrupt(&mut self) -> InterruptStatus {
        let status = self.read(REG_INTERRUPT_STATUS);
        self.write(REG_INTERRUPT_ACK, status);
        InterruptStatus::from_bits_retain(status)
    }

    fn read_config_generation(&self) -> u32 {
        self.read(REG_CONFIG_GENERATION)
    }

    /// Read a `T` at `offset` in the device's configuration, one aligned register word per byte,
    /// because the kernel answers only aligned words.
    fn read_config_space<T: zerocopy::FromBytes + zerocopy::IntoBytes>(
        &self,
        offset: usize,
    ) -> virtio_drivers::Result<T> {
        let mut value = T::new_zeroed();
        for (i, byte) in value.as_mut_bytes().iter_mut().enumerate() {
            let at = REG_DEVICE_CONFIG + (offset + i) as u64;
            let word = crate::virtio::read_register(self.slot, at & !3);
            if word < 0 {
                return Err(Error::IoError);
            }
            *byte = (word as u32 >> (8 * (at & 3))) as u8;
        }
        Ok(value)
    }

    /// The kernel refuses every configuration write (`WRITE_REGISTER`'s safe set holds none).
    fn write_config_space<T: zerocopy::IntoBytes + zerocopy::Immutable>(
        &mut self,
        _offset: usize,
        _value: T,
    ) -> virtio_drivers::Result<()> {
        Err(Error::Unsupported)
    }
}

// The window: where the spawner mapped this process's DMA region, and how many queues' rings
// `dma_alloc` has handed out. Globals because `Hal`'s methods take no `self`; see the BUGS above.
static WINDOW_VA: AtomicU64 = AtomicU64::new(0);
static WINDOW_PA: AtomicU64 = AtomicU64::new(0);
static WINDOW_LEN: AtomicU64 = AtomicU64::new(0);
/// Twice the number of queues whose rings are handed out, plus one while a queue's used ring is
/// still owed: even asks for a descriptor table, odd for a used ring.
static RING_ALLOCATIONS: AtomicU64 = AtomicU64::new(0);

/// Tell [`WindowHal`] where the DMA window is: mapped at `va`, at physical (and, behind the
/// IOMMU's identity domain, device) address `pa`, `len` bytes. Called once, before the first
/// `VirtQueue`; a program is one thread, so the relaxed stores are seen by every later load.
pub fn install_window(va: u64, pa: u64, len: u64) {
    WINDOW_VA.store(va, Ordering::Relaxed);
    WINDOW_PA.store(pa, Ordering::Relaxed);
    WINDOW_LEN.store(len, Ordering::Relaxed);
}

fn window_pa() -> u64 {
    WINDOW_PA.load(Ordering::Relaxed)
}

/// **The crate's `Hal`, over the one DMA window.** Ring allocations come out of the kernel's fixed
/// ring blocks; a shared buffer must already be in the window and is translated, never copied.
pub struct WindowHal;

// SAFETY: `dma_alloc` returns addresses inside the window the spawner mapped read/write for this
// process alone, zeroed over every byte a `VirtQueue` touches, and never the same ring twice;
// `share` returns the physical address of the very bytes it is handed. The page-size part of the
// contract is not met, and the module's BUGS say why that holds for `VirtQueue`.
unsafe impl Hal for WindowHal {
    /// Hand out the next ring: a descriptor table and available ring (`DriverToDevice`) at the
    /// next queue's ring block, then that queue's used ring (`DeviceToDriver`). Anything else, or
    /// a ring that would not fit the window, is `(0, dangling)`, which the crate reads as
    /// `DmaError`.
    fn dma_alloc(
        pages: usize,
        direction: BufferDirection,
        _access_platform: bool,
    ) -> (PhysAddr, NonNull<u8>) {
        let refused = (0, NonNull::dangling());
        let n = RING_ALLOCATIONS.load(Ordering::Relaxed);
        let (queue, used) = ((n / 2) as u16, n % 2 == 1);
        let wanted = if used {
            BufferDirection::DeviceToDriver
        } else {
            BufferDirection::DriverToDevice
        };
        let block = queue_block(queue);
        if pages != 1
            || direction != wanted
            || queue as u64 >= MAX_QUEUES
            || block + RING_END > WINDOW_LEN.load(Ordering::Relaxed)
        {
            return refused;
        }
        let (start, end) = if used {
            (block + USED_OFF, block + RING_END)
        } else {
            (block + DESC_OFF, block + USED_OFF)
        };
        let va = WINDOW_VA.load(Ordering::Relaxed) + start;
        for off in 0..end - start {
            // SAFETY: inside the window, which is mapped read/write, and inside this queue's ring
            // block, which nothing has been handed yet.
            unsafe { core::ptr::write_volatile((va + off) as *mut u8, 0) };
        }
        RING_ALLOCATIONS.store(n + 1, Ordering::Relaxed);
        match NonNull::new(va as *mut u8) {
            Some(ptr) => (window_pa() + start, ptr),
            None => refused,
        }
    }

    /// Rings are never given back: a driver keeps its queues for its life.
    unsafe fn dma_dealloc(
        _paddr: PhysAddr,
        _vaddr: NonNull<u8>,
        _pages: usize,
        _access_platform: bool,
    ) -> i32 {
        0
    }

    /// Never called: [`ConfinedTransport`] maps no registers.
    unsafe fn mmio_phys_to_virt(_paddr: PhysAddr, _size: usize) -> NonNull<u8> {
        panic!("confined_virtio: no device registers are mapped; the kernel answers them")
    }

    /// The physical address of `buffer`, which must lie inside the window.
    ///
    /// # Panics
    /// If any of `buffer` is outside the window. The kernel's validator would refuse such a
    /// descriptor at `NOTIFY` anyway; panicking names the driver bug where it is.
    unsafe fn share(
        buffer: NonNull<[u8]>,
        _direction: BufferDirection,
        _access_platform: bool,
    ) -> PhysAddr {
        let va = buffer.as_ptr().cast::<u8>() as u64;
        let (base, len) = (
            WINDOW_VA.load(Ordering::Relaxed),
            WINDOW_LEN.load(Ordering::Relaxed),
        );
        let inside = va >= base && va - base + buffer.len() as u64 <= len;
        assert!(
            inside,
            "confined_virtio: a shared buffer is outside the DMA window"
        );
        window_pa() + (va - base)
    }

    /// Nothing to copy back: the device wrote the buffer itself.
    unsafe fn unshare(
        _paddr: PhysAddr,
        _buffer: NonNull<[u8]>,
        _direction: BufferDirection,
        _access_platform: bool,
    ) {
    }
}
