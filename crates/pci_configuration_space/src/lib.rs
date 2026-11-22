//! PCI configuration space: the decode logic, and none of the memory-mapped input and output. It
//! addresses a function in the enhanced configuration access mechanism (ECAM) window, walks a
//! machine's buses for functions, sizes a function's base address registers, and decodes its
//! capability lists, all over read and write closures on `(BusDeviceFunction, offset)`. A host operating system
//! passes volatile accessors into the mapped ECAM window and a test passes an array, which is why
//! the crate tests on a host in milliseconds.
//!
//! A function is addressed by bus, device and function number and has 4 KiB of configuration space
//! at `(bus << 20) | (device << 15) | (function << 12)` in the flat ECAM window (PCI Express Base
//! Specification, "Enhanced Configuration Access Mechanism"). Its first 64 bytes are the standard
//! header (PCI Local Bus Specification 3.0, section 6.1): the ids, the command and status
//! registers, the class code, the base address registers (BARs) that say where its register blocks
//! live, and the pointer to its capability list. Message-signaled interrupts (MSI and MSI-X) and
//! the virtio vendor capabilities hang off that list.
//!
//! What the crate refuses to decode: a header of any type but 0 (an endpoint) and 1 (a PCI-to-PCI
//! bridge) has no BARs it knows, I/O-space BARs are skipped, a 64-bit BAR in a header's last slot
//! is refused, and a BAR whose writable address bits are not contiguous is refused rather than
//! sized. Every function here is total: a device that answers every read with arbitrary bytes
//! leaves the caller with a bounded, well-formed answer or `None`.
//!
//! # Examples
//!
//! See [`enumerate`] for finding a function, and [`intx_interrupt_number`] for the legacy
//! interrupt swizzle.
//!
//! Name: ratified 2026-09-18, under the acronym test (the name is whether the phrase is spoken):
//! nobody says "peripheral component interconnect", so the short name stays `pci`. Refused
//! `peripheral_component_interconnect`. The package name is `pci_configuration_space`, ruled
//! 2026-11-14 (UTC) because `pci` is taken on crates.io; the repository will carry that name.

#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

/// A function's address: bus, device and function number.
///
/// The ECAM window and the PCI Express requester id both pack these three fields as
/// `bus:8 | device:5 | function:3`. A device number wider than five bits or a function number wider
/// than three is masked to its field by [`BusDeviceFunction::ecam_offset`] and [`BusDeviceFunction::requester_id`], so it can
/// never name another bus's or another device's page.
///
/// Name: ratified 2026-11-20. The fields were `dev` and `func`; `pci_types::PciAddress` and
/// `virtio-drivers`' `DeviceFunction` both spell them `device` and `function`. The specification
/// spellings ride as aliases.
#[doc(alias = "Bdf")]
#[doc(alias = "BDF")]
#[doc(alias = "bus device function")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BusDeviceFunction {
    /// The bus number, 0 to 255.
    pub bus: u8,
    /// The device number on the bus; only the low five bits address a device.
    pub device: u8,
    /// The function number in the device; only the low three bits address a function.
    pub function: u8,
}

impl BusDeviceFunction {
    /// The function's byte offset inside the ECAM window: 4 KiB per function, laid out
    /// `bus:8 | device:5 | function:3 | offset:12`. This shift-and-or is the ECAM layout.
    ///
    /// A `device` or `function` wider than its field is masked to the field here, so it can never
    /// land on the next bus's or the next device's page.
    #[doc(alias = "enhanced configuration access mechanism")]
    pub fn ecam_offset(self) -> u64 {
        ((self.bus as u64) << 20)
            | (((self.device & 0x1f) as u64) << 15)
            | (((self.function & 0x7) as u64) << 12)
    }

    /// The **PCIe requester id**: the 16-bit `bus:8 | device:5 | function:3` a function stamps on
    /// every memory transaction it issues (PCI Express Base Specification, "Requester ID"), and
    /// the key an input and output memory management unit looks a device up by: the Arm SMMUv3
    /// `StreamID` and the RISC-V IOMMU `device_id` are both this number when the machine's
    /// `iommu-map` is the identity.
    pub fn requester_id(self) -> u32 {
        ((self.bus as u32) << 8)
            | (((self.device & 0x1f) as u32) << 3)
            | ((self.function & 0x7) as u32)
    }
}

/// Byte offset of the Vendor ID and Device ID dword in the standard header (PCI Local Bus
/// Specification 3.0, section 6.2.1): the vendor in the low half, the device in the high half. A
/// vendor of `0xffff` means no function answers.
///
/// Name: ratified 2026-11-20. Was `VENDOR_ID`; a register offset ends in `_REGISTER`.
pub const VENDOR_ID_REGISTER: u64 = 0x00;
/// Byte offset of the Command and Status dword (section 6.2.2 and 6.2.3): the command register in
/// the low half and the status register in the high half. The capability walks read the status
/// half from this dword.
///
/// Name: ratified 2026-11-20. Was `COMMAND`; a register offset ends in `_REGISTER`.
pub const COMMAND_REGISTER: u64 = 0x04;
/// Byte offset of the Class Code and Revision ID dword (section 6.2.4): the 24-bit class code over
/// the 8-bit revision id. `read32(bus_device_function, CLASS_REVISION_REGISTER) >> 8` is the class code.
///
/// Name: ratified 2026-11-20. Was `CLASS_REVISION`; a register offset ends in `_REGISTER`.
pub const CLASS_REVISION_REGISTER: u64 = 0x08;
/// Byte offset of the Header Type byte (section 6.2.5): bits 6:0 give the layout, bit 7 says the
/// device is multifunction. The byte sits in the dword at `HEADER_TYPE_REGISTER & !3`.
///
/// Name: ratified 2026-11-20. Was `HEADER_TYPE`; a register offset ends in `_REGISTER`.
pub const HEADER_TYPE_REGISTER: u64 = 0x0e;
/// Byte offset of Base Address Register 0 (section 6.2.5.1); the six BARs of a type-0 header follow
/// at four-byte steps.
///
/// Name: ratified 2026-11-20. Was `BAR0`; a register offset ends in `_REGISTER`.
pub const BAR0_REGISTER: u64 = 0x10;
/// Byte offset of the Capabilities Pointer byte (section 6.7): where the capability list starts.
///
/// Name: ratified 2026-11-20. Spelled out from `CAP_PTR`; the register offsets end in `_REGISTER`.
pub const CAPABILITIES_POINTER_REGISTER: u64 = 0x34;
/// Byte offset of the Interrupt Pin byte (section 6.2.4): 0 for none, then 1 to 4 for INTA to INTD.
///
/// Name: ratified 2026-11-20. Was `INTERRUPT_PIN`; a register offset ends in `_REGISTER`.
pub const INTERRUPT_PIN_REGISTER: u64 = 0x3d;

/// **Header type 1: a PCI-to-PCI bridge**, which is what a PCI Express root port and every switch
/// port present themselves as (PCI-to-PCI Bridge Architecture Specification 1.2, section 3.2). The
/// low seven bits of [`HEADER_TYPE_REGISTER`] are the layout code (0 an endpoint, 1 a bridge, 2 the
/// long-dead `CardBus` shape); bit 7 is the multifunction flag and is masked off before comparing. A
/// bridge's header has two BAR slots where an endpoint has six, and where an endpoint's third BAR
/// sits it carries [`BUS_NUMBERS_REGISTER`] instead.
pub const HEADER_TYPE_BRIDGE: u8 = 1;

/// **Where a type-1 header states the buses behind it**: the primary, secondary and subordinate
/// bus numbers in the low three bytes of the dword at 0x18, then the secondary latency timer
/// (PCI-to-PCI Bridge Architecture Specification 1.2, section 3.2). Firmware writes these during
/// its own enumeration and they are the only record of the topology that exists; nothing derives
/// them, and software that does not read them cannot find a device behind a root port. See
/// [`bridge_buses`].
///
/// Name: ratified 2026-11-20. Was `BUS_NUMBERS`; a register offset ends in `_REGISTER`.
pub const BUS_NUMBERS_REGISTER: u64 = 0x18;

/// The non-volatile memory express (NVMe) class code: mass storage (0x01), non-volatile memory
/// (0x08), NVMe I/O (0x02). Matched by class rather than by vendor and device id on purpose:
/// QEMU's controller is Red Hat 1b36:0010, real drives are anything at all, and the class triple
/// is the one identity the NVM Express Base Specification 1.4, section 3.1 requires of every one
/// of them.
///
/// Name: ratified 2026-11-20. Spelled out from `CLASS_NVME`.
#[doc(alias = "NVMe")]
pub const CLASS_NON_VOLATILE_MEMORY_EXPRESS: u32 = 0x01_08_02;

/// The extensible host controller interface (xHCI) class code: serial bus (0x0c), USB (0x03), xHCI
/// (0x30). Matched by class for [`CLASS_NON_VOLATILE_MEMORY_EXPRESS`]'s reason: QEMU's `qemu-xhci`
/// is Red Hat 1b36:000d, an Intel chipset's is 8086:a2af or a hundred others, and the class triple
/// is what the eXtensible Host Controller Interface specification 1.2, section 5.2.1 requires of
/// all of them.
///
/// Name: ratified 2026-11-20. Spelled out from `CLASS_XHCI`.
#[doc(alias = "xHCI")]
pub const CLASS_EXTENSIBLE_HOST_CONTROLLER_INTERFACE: u32 = 0x0c_03_30;

/// Command register bit 1, Memory Space Enable (PCI Local Bus Specification 3.0, section 6.2.2):
/// the function responds to memory accesses that hit its BARs.
///
/// Name: ratified 2026-11-20. Spelled out from `CMD_MEMORY_SPACE`, with the specification's bit name.
pub const COMMAND_MEMORY_SPACE_ENABLE: u16 = 1 << 1;
/// Command register bit 2, Bus Master Enable (section 6.2.2). Bus mastering is DMA permission at
/// the PCI level: without it the function cannot issue a single memory transaction. A caller that
/// polices where the DMA goes sets this bit last, after its own checks are in place.
///
/// Name: ratified 2026-11-20. Spelled out from `CMD_BUS_MASTER`, with the specification's bit name.
pub const COMMAND_BUS_MASTER_ENABLE: u16 = 1 << 2;

/// Status register bit 4, Capabilities List (PCI Local Bus Specification 3.0, section 6.2.3): this
/// function has a capability list.
const STATUS_CAPABILITIES_LIST: u16 = 1 << 4;

/// virtio's PCI vendor id (Red Hat, 0x1af4), from virtio 1.2, section 4.1.2.
///
/// Name: ratified 2026-11-20. Spelled out from `VIRTIO_VENDOR`.
#[doc(alias = "virtio")]
pub const VIRTUAL_IO_VENDOR: u16 = 0x1af4;
/// The modern virtio block device id: 0x1040 plus device type 2 (virtio 1.2, section 4.1.2.1).
///
/// Name: ratified 2026-11-20. Spelled out from `VIRTIO_BLK_MODERN`, which also matches
/// [`VIRTUAL_IO_TYPE_BLOCK`].
#[doc(alias = "virtio")]
pub const VIRTUAL_IO_BLOCK_MODERN: u16 = 0x1042;
/// The transitional (legacy-capable) virtio block device id (virtio 1.2, section 4.1.2.1). The
/// crate does not decode legacy functions, but it recognizes the id so a caller can say "found a
/// disk, but it is transitional" instead of "no disk".
///
/// Name: ratified 2026-11-20. Spelled out from `VIRTIO_BLK_TRANSITIONAL`.
#[doc(alias = "virtio")]
pub const VIRTUAL_IO_BLOCK_TRANSITIONAL: u16 = 0x1001;

/// The modern virtio network device id: 0x1040 plus device type 1 (virtio 1.2, section 4.1.2.1).
///
/// Name: ratified 2026-11-20. Spelled out from `VIRTIO_NET_MODERN`.
#[doc(alias = "virtio")]
pub const VIRTUAL_IO_NETWORK_MODERN: u16 = 0x1041;
/// The transitional virtio network device id, the legacy twin of [`VIRTUAL_IO_NETWORK_MODERN`]. A
/// caller that drives modern functions only still reports a transitional one as such rather than
/// as absent.
///
/// Name: ratified 2026-11-20. Spelled out from `VIRTIO_NET_TRANSITIONAL`.
#[doc(alias = "virtio")]
pub const VIRTUAL_IO_NETWORK_TRANSITIONAL: u16 = 0x1000;

/// The modern virtio GPU device id: 0x1040 plus device type 16 (virtio 1.2, section 4.1.2.1). There
/// is **no transitional twin**: the legacy id space (0x1000 to 0x103f) was allocated before
/// virtio-gpu existed, so a GPU is modern or nothing and a caller passes `None` for the legacy id
/// rather than inventing one. QEMU's `virtio-gpu-pci` presents exactly this id.
///
/// Name: ratified 2026-11-20. Spelled out from `VIRTIO_GPU_MODERN`.
#[doc(alias = "virtio")]
pub const VIRTUAL_IO_GPU_MODERN: u16 = 0x1050;

/// The modern virtio input device id: 0x1040 plus device type 18 (virtio 1.2, section 4.1.2.1).
/// **No transitional twin**, for the reason [`VIRTUAL_IO_GPU_MODERN`] has none. QEMU's
/// `virtio-keyboard-pci` presents exactly this id, and so does `virtio-tablet-pci`: **the id names
/// the device class, not the keyboard**, so a machine with both must read the function's
/// configuration to tell them apart.
///
/// Name: ratified 2026-11-20. Spelled out from `VIRTIO_INPUT_MODERN`.
#[doc(alias = "virtio")]
pub const VIRTUAL_IO_INPUT_MODERN: u16 = 0x1052;

/// The modern virtio entropy device id: 0x1040 plus device type 4 (virtio 1.2, section 4.1.2.1).
/// Unlike the GPU and the input device this one **does** have a legacy id, [`VIRTUAL_IO_ENTROPY_TRANSITIONAL`]:
/// device type 4 is early enough to have been allocated a slot in the 0x1000 to 0x103f space.
///
/// Name: ratified 2026-11-20. Spelled out from `VIRTIO_RNG_MODERN`; the virtio specification calls device
/// type 4 the entropy device, as [`VIRTUAL_IO_TYPE_ENTROPY`] does.
#[doc(alias = "virtio")]
pub const VIRTUAL_IO_ENTROPY_MODERN: u16 = 0x1044;
/// The transitional virtio entropy device id, the legacy twin of [`VIRTUAL_IO_ENTROPY_MODERN`].
///
/// Name: ratified 2026-11-20. Spelled out from `VIRTIO_RNG_TRANSITIONAL`.
#[doc(alias = "virtio")]
pub const VIRTUAL_IO_ENTROPY_TRANSITIONAL: u16 = 0x1005;

/// The virtio **device type** numbers (virtio 1.2, section 5), as the virtio-mmio `DeviceID`
/// register reports them and as the PCI ids above encode them (`0x1040 + type`). A host that wraps
/// a PCI function in a virtio transport carries the type so a driver's `DeviceID` read answers
/// truthfully on either bus.
///
/// Network device.
///
/// Name: ratified 2026-11-20. Spelled out from `VIRTIO_TYPE_NET`.
#[doc(alias = "virtio")]
pub const VIRTUAL_IO_TYPE_NETWORK: u32 = 1;
/// The virtio block device type (virtio 1.2, section 5.2).
///
/// Name: ratified 2026-11-20. Spelled out from `VIRTIO_TYPE_BLOCK`.
#[doc(alias = "virtio")]
pub const VIRTUAL_IO_TYPE_BLOCK: u32 = 2;
/// The virtio entropy device type (virtio 1.2, section 5.4).
///
/// Name: ratified 2026-11-20. Spelled out from `VIRTIO_TYPE_ENTROPY`.
#[doc(alias = "virtio")]
pub const VIRTUAL_IO_TYPE_ENTROPY: u32 = 4;
/// The virtio GPU device type (virtio 1.2, section 5.7).
///
/// Name: ratified 2026-11-20. Spelled out from `VIRTIO_TYPE_GPU`.
#[doc(alias = "virtio")]
pub const VIRTUAL_IO_TYPE_GPU: u32 = 16;
/// The virtio input device type (virtio 1.2, section 5.8).
///
/// Name: ratified 2026-11-20. Spelled out from `VIRTIO_TYPE_INPUT`.
#[doc(alias = "virtio")]
pub const VIRTUAL_IO_TYPE_INPUT: u32 = 18;

/// **The buses a type-1 header says lie behind it.** Firmware's own enumeration wrote these; a
/// caller that does not read them enumerates bus 0 and calls the rest of the machine absent (PCI-to-PCI
/// Bridge Architecture Specification 1.2, section 3.2).
///
/// `subordinate` is the *highest* bus number reachable through this bridge, so `secondary ..=
/// subordinate` is the whole subtree. A bridge that firmware configured but left empty states
/// `secondary == subordinate`; one firmware never configured at all reads back zeros, which
/// [`walk`] treats as "nothing behind it" rather than as a second route to bus 0.
///
/// Name: ratified 2026-11-20, with every name in this group: this type, [`bridge_buses`], [`Function`],
/// [`walk`], [`BUS_NUMBERS_REGISTER`] and [`HEADER_TYPE_BRIDGE`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BridgeBuses {
    /// The bus the bridge itself sits on.
    pub primary: u8,
    /// The bus directly behind the bridge.
    pub secondary: u8,
    /// The highest bus number behind the bridge.
    pub subordinate: u8,
}

/// Decode the three bus numbers of a type-1 header. Total for any device response: it is three
/// byte fields of one dword and there is no arithmetic to overflow.
pub fn bridge_buses(
    bus_device_function: BusDeviceFunction,
    read32: &mut dyn FnMut(BusDeviceFunction, u64) -> u32,
) -> BridgeBuses {
    let word = read32(bus_device_function, BUS_NUMBERS_REGISTER);
    BridgeBuses {
        primary: (word & 0xff) as u8,
        secondary: ((word >> 8) & 0xff) as u8,
        subordinate: ((word >> 16) & 0xff) as u8,
    }
}

/// **One function the walk met**, with everything a census needs to print and everything the
/// walk itself needs to decide where to go next.
///
/// Name: ratified 2026-11-20.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Function {
    /// Where the function lives.
    pub bus_device_function: BusDeviceFunction,
    /// The Vendor ID half of the first header dword (PCI Local Bus Specification 3.0, section 6.2.1).
    pub vendor: u16,
    /// The Device ID half of the first header dword.
    pub device: u16,
    /// Class over subclass over programming interface, 24 bits: `CLASS_REVISION_REGISTER >> 8`.
    /// This is the identity [`CLASS_NON_VOLATILE_MEMORY_EXPRESS`] is matched against, and the one
    /// thing on the bus that says what a function *is* rather than who made it.
    pub class: u32,
    /// The header layout code, **with the multifunction bit already masked off**: 0 an endpoint,
    /// [`HEADER_TYPE_BRIDGE`] a bridge. Masked here rather than at every call site because an
    /// unmasked comparison against 1 silently misses every multifunction bridge, which is what a
    /// switch's upstream port is.
    pub header_type: u8,
    /// Whether this function reported itself multifunction, which is the bit that decides whether
    /// functions 1 to 7 were probed at all.
    pub multifunction: bool,
    /// The buses behind it, for a bridge; `None` for an endpoint.
    pub bridge: Option<BridgeBuses>,
}

/// **The buses [`walk`] still has to scan, and the ones it has already queued.** Private, because
/// it is the walk's bookkeeping rather than anything a caller needs; separate from the walk
/// because it is the one part of a bridge walk that can be wrong in a way a test would not see.
///
/// Every bus number a bridge states comes from firmware, so the queue is fed arbitrary bytes. What
/// keeps `queue[tail]` in bounds is that a bus is enqueued only when its bit in `seen` is clear and
/// the bit is set in the same breath, so at most 256 pushes can ever happen; the crate's Kani proofs
/// prove exactly that against any sequence of arbitrary bus numbers, which is the property no
/// fixture test can establish.
struct BusQueue {
    /// Bus numbers waiting to be scanned, `head..tail`.
    queue: [u8; 256],
    /// One bit per bus number: has it ever been enqueued? 256 bits is every bus that exists.
    seen: [u64; 4],
    head: usize,
    tail: usize,
}

impl BusQueue {
    /// A queue holding bus 0 alone. Bus 0 is the root complex and is where every PCI topology
    /// starts; it is marked seen so a bridge that names it (a firmware bug, or an unconfigured
    /// header) cannot put it in twice.
    fn from_the_root() -> Self {
        let mut q = BusQueue {
            queue: [0; 256],
            seen: [0; 4],
            head: 0,
            tail: 1,
        };
        q.seen[0] = 1;
        q
    }

    /// Add `bus` unless it has been queued before. Idempotent per bus number, which is what bounds
    /// the whole walk.
    fn enqueue(&mut self, bus: u8) {
        let (word, bit) = (bus as usize >> 6, 1u64 << (bus & 63));
        if self.seen[word] & bit != 0 {
            return;
        }
        self.seen[word] |= bit;
        self.queue[self.tail] = bus;
        self.tail += 1;
    }

    fn next_bus(&mut self) -> Option<u8> {
        (self.head < self.tail).then(|| {
            let bus = self.queue[self.head];
            self.head += 1;
            bus
        })
    }
}

/// **Walk the machine's PCI topology from bus 0, following bridges**, and call `f` once per
/// function found. `ceiling` is the exclusive bus-number bound the caller can actually *read*:
/// a bridge naming a secondary bus at or above it is reported (its [`Function`] carries the bus
/// numbers) but not descended into, because reading an unmapped bus is a fault rather than a
/// finding.
///
/// A bridge's bus numbers come from firmware and can name anything, a visited bus or its own
/// included, so the walk is a breadth-first queue over a 256-bit visited set: **each bus is scanned
/// at most once**, a cycle terminates, and nothing recurses or allocates (the queue is a 256-byte
/// array). Walking the bridges, rather than assuming bus 0 is the whole machine, is what finds a
/// device behind a PCI Express root port.
pub fn walk(
    ceiling: u16,
    read32: &mut dyn FnMut(BusDeviceFunction, u64) -> u32,
    f: &mut dyn FnMut(Function),
) {
    let ceiling = ceiling.min(256);
    if ceiling == 0 {
        return;
    }
    let mut buses = BusQueue::from_the_root();

    while let Some(bus) = buses.next_bus() {
        for device in 0..32 {
            let bus_device_function_zero = BusDeviceFunction {
                bus,
                device,
                function: 0,
            };
            if read32(bus_device_function_zero, VENDOR_ID_REGISTER) & 0xffff == 0xffff {
                continue; // empty slot: one config read, which is the whole point of this guard
            }
            let type0 = (read32(bus_device_function_zero, HEADER_TYPE_REGISTER & !3)
                >> ((HEADER_TYPE_REGISTER & 3) * 8))
                & 0xff;
            let multifunction = type0 & 0x80 != 0;
            let functions = if multifunction { 8 } else { 1 };
            for function in 0..functions {
                let bus_device_function = BusDeviceFunction {
                    bus,
                    device,
                    function,
                };
                let id = read32(bus_device_function, VENDOR_ID_REGISTER);
                if id & 0xffff == 0xffff {
                    continue;
                }
                let raw = (read32(bus_device_function, HEADER_TYPE_REGISTER & !3)
                    >> ((HEADER_TYPE_REGISTER & 3) * 8))
                    & 0xff;
                let header_type = (raw & 0x7f) as u8;
                let bridge = (header_type == HEADER_TYPE_BRIDGE).then(|| {
                    let behind = bridge_buses(bus_device_function, read32);
                    // Bus 0 is the root complex and is already the walk's starting point, so a
                    // bridge reporting secondary 0 is one firmware never configured rather than a
                    // second route to the top of the tree. Descending on it would be harmless
                    // (bus 0 is in `seen`) and reporting it as a subtree would not be.
                    if behind.secondary != 0 && (behind.secondary as u16) < ceiling {
                        buses.enqueue(behind.secondary);
                    }
                    behind
                });
                f(Function {
                    bus_device_function,
                    vendor: (id & 0xffff) as u16,
                    device: (id >> 16) as u16,
                    class: read32(bus_device_function, CLASS_REVISION_REGISTER) >> 8,
                    header_type,
                    multifunction,
                    bridge,
                });
            }
        }
    }
}

/// Every function the machine has, as (`BusDeviceFunction`, vendor, device): [`walk`] with the topology detail
/// dropped, which is all most callers want. `ceiling` is the exclusive bus bound the caller can
/// read; see [`walk`] for what happens at it.
///
/// # Examples
///
/// Enumerating a bus with no bus: a test's closure over an array stands in for the ECAM window.
///
/// ```
/// use pci_configuration_space::{BusDeviceFunction, VENDOR_ID_REGISTER, VIRTUAL_IO_VENDOR, enumerate};
///
/// // A config space with exactly one function: modern virtio-blk at 00:01.0. Everything else reads
/// // 0xffff, which is the bus's way of saying nobody is home.
/// let mut read32 = |bus_device_function: BusDeviceFunction, off: u64| -> u32 {
///     if bus_device_function == (BusDeviceFunction { bus: 0, device: 1, function: 0 }) && off == VENDOR_ID_REGISTER {
///         (0x1042 << 16) | VIRTUAL_IO_VENDOR as u32 // device 0x1042 in the high half
///     } else if off == VENDOR_ID_REGISTER {
///         0xffff_ffff
///     } else {
///         0 // header type 0: single function, so functions 1 to 7 are skipped
///     }
/// };
///
/// let mut found = Vec::new();
/// enumerate(1, &mut read32, &mut |bus_device_function, vendor, device| {
///     found.push((bus_device_function, vendor, device));
/// });
///
/// assert_eq!(found.len(), 1);
/// let (bus_device_function, vendor, device) = found[0];
/// assert_eq!((vendor, device), (VIRTUAL_IO_VENDOR, 0x1042));
///
/// // ECAM addressing: 4 KiB per function, `bus:8 | device:5 | function:3 | offset:12`.
/// assert_eq!(bus_device_function.ecam_offset(), 1 << 15);
///
/// // And the requester id, which is the key an IOMMU looks the function up by.
/// assert_eq!(bus_device_function.requester_id(), 0b0000_0000_0000_1000);
/// ```
pub fn enumerate(
    ceiling: u16,
    read32: &mut dyn FnMut(BusDeviceFunction, u64) -> u32,
    f: &mut dyn FnMut(BusDeviceFunction, u16, u16),
) {
    walk(ceiling, read32, &mut |function| {
        f(
            function.bus_device_function,
            function.vendor,
            function.device,
        );
    });
}

/// One decoded Base Address Register: where the register block is, how big, and how wide (PCI
/// Local Bus Specification 3.0, section 6.2.5.1).
///
/// Name: ratified 2026-11-20. `base` and `size` were unqualified and `is_64` did not say what is 64.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bar {
    /// The assigned base address (0 if firmware assigned nothing; the caller must then place it).
    /// It is the address a bus master puts on the bus; on a machine with no translation between the
    /// bus and the processor it is also the processor's physical address, which is what the name
    /// says.
    pub base_physical_address: u64,
    /// The region size in bytes, from the write-ones probe: a power of two of at least 16, and
    /// `base_physical_address + size_bytes` does not overflow.
    pub size_bytes: u64,
    /// A 64-bit BAR consumes two BAR slots; the caller's index bookkeeping needs to know.
    pub is_64_bit: bool,
}

/// **Why [`read_bars`] refused a memory BAR.** A refused BAR is left out of the result, as if the
/// slot were empty, and the reason goes to the caller's `refused` closure so it can say why a
/// function came up without one.
///
/// Name: ratified 2026-11-20, as are its variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BarRefusal {
    /// The register answered the probe, but no address bit is writable, so there is no size.
    NoWritableBits,
    /// The writable address bits are not every bit from the lowest one up to the top of the
    /// BAR's width. A 64-bit BAR whose upper half reads back zero is this shape too.
    MaskNotContiguous,
    /// The assigned base plus the size passes the top of the 64-bit address space.
    EndOverflows,
    /// A 64-bit BAR in the header's last slot: its upper half would be the register after the
    /// BARs (the `CardBus` CIS pointer on an endpoint), which is not a BAR.
    SixtyFourBitInLastSlot,
}

/// The address bits of a 32-bit memory BAR: everything above the four type and prefetch bits.
const BAR_DECODE_BITS_32: u64 = 0xffff_fff0;
/// The address bits of a 64-bit memory BAR, both halves.
const BAR_DECODE_BITS_64: u64 = 0xffff_ffff_ffff_fff0;

/// **A memory BAR's size from the address bits that stayed set after the all-ones probe.** The
/// size is the lowest set bit, as OpenBSD's `PCI_MAPREG_MEM_SIZE` (`mr & -mr`) and Linux's
/// `pci_size` compute it. `decode_bits` is the BAR's width, [`BAR_DECODE_BITS_32`] or
/// [`BAR_DECODE_BITS_64`], and `mask` must already be masked by it. The lowest set bit is taken as
/// `1 << trailing_zeros`, since `isolate_lowest_one` is not yet stable on the toolchain the Kani
/// proofs build with.
///
/// The probe can only describe a size if every address bit from that lowest one to the top of
/// the width is writable, so any other mask is refused rather than sized: `!mask + 1` gives a
/// size that is not a power of two for a mask with a hole, and one near `u64::MAX` for a 64-bit
/// BAR whose upper half reads zero.
fn size_from_mask(mask: u64, decode_bits: u64) -> Result<u64, BarRefusal> {
    if mask == 0 {
        return Err(BarRefusal::NoWritableBits);
    }
    let size = 1 << mask.trailing_zeros();
    if mask != decode_bits & !(size - 1) {
        return Err(BarRefusal::MaskNotContiguous);
    }
    Ok(size)
}

/// Read and size the memory BARs of a header: six slots for an endpoint (type 0), two for a bridge
/// (type 1), and none for any other layout, whose BAR slots this crate does not know. Returns one
/// entry per **slot** (six), `None` for the upper half of a 64-bit BAR, for I/O-space BARs (the
/// crate decodes memory BARs only), for an unimplemented BAR, for a slot past the header's last, and
/// for a BAR it refused. A refusal also calls `refused` with the slot and the reason.
///
/// Sizing is the dance PCI Local Bus Specification 3.0, section 6.2.5.1 prescribes: write all-ones,
/// read back the mask of writable bits (the low bits that stay zero encode the alignment and size),
/// restore the original. The function must be quiescent during this (memory decoding is off until
/// the command register enables it), which is why a caller sizes BARs before it enables the
/// function.
pub fn read_bars(
    bus_device_function: BusDeviceFunction,
    read32: &mut dyn FnMut(BusDeviceFunction, u64) -> u32,
    write32: &mut dyn FnMut(BusDeviceFunction, u64, u32),
    refused: &mut dyn FnMut(usize, BarRefusal),
) -> [Option<Bar>; 6] {
    let mut out = [None; 6];
    let header_type = (read32(bus_device_function, HEADER_TYPE_REGISTER & !3)
        >> ((HEADER_TYPE_REGISTER & 3) * 8)) as u8
        & 0x7f;
    let slots = match header_type {
        0 => 6,
        HEADER_TYPE_BRIDGE => 2,
        _ => 0,
    };
    // Set when the slot before was the lower half of a 64-bit BAR, whose upper half this slot is.
    // A flag rather than a two-slot step, so the loop is bounded by the six slots alone.
    let mut upper_half = false;
    for (i, slot) in out.iter_mut().enumerate().take(slots) {
        if core::mem::take(&mut upper_half) {
            continue;
        }
        let off = BAR0_REGISTER + i as u64 * 4;
        let original = read32(bus_device_function, off);
        if original & 1 != 0 {
            // An I/O-space BAR. x86 legacy; the crate decodes memory BARs only.
            continue;
        }
        let is_64_bit = original & 0b110 == 0b100;
        if is_64_bit && i + 1 == slots {
            // Refused before the probe, so the register after the last slot is never written.
            refused(i, BarRefusal::SixtyFourBitInLastSlot);
            continue;
        }

        write32(bus_device_function, off, u32::MAX);
        let mask_lo = read32(bus_device_function, off);
        write32(bus_device_function, off, original);

        let (base, mask, decode_bits) = if is_64_bit {
            let off_hi = off + 4;
            let original_high = read32(bus_device_function, off_hi);
            write32(bus_device_function, off_hi, u32::MAX);
            let mask_hi = read32(bus_device_function, off_hi);
            write32(bus_device_function, off_hi, original_high);
            (
                ((original_high as u64) << 32) | (original as u64 & BAR_DECODE_BITS_32),
                (((mask_hi as u64) << 32) | mask_lo as u64) & BAR_DECODE_BITS_64,
                BAR_DECODE_BITS_64,
            )
        } else {
            (
                original as u64 & BAR_DECODE_BITS_32,
                mask_lo as u64 & BAR_DECODE_BITS_32,
                BAR_DECODE_BITS_32,
            )
        };

        // An unimplemented BAR is hardwired to zero and reads back zero, type bits included.
        // That is an empty slot, not a malformed one.
        if !(original == 0 && mask_lo == 0) {
            match size_from_mask(mask, decode_bits).and_then(|size| {
                base.checked_add(size)
                    .map(|_| size)
                    .ok_or(BarRefusal::EndOverflows)
            }) {
                Ok(size) => {
                    *slot = Some(Bar {
                        base_physical_address: base,
                        size_bytes: size,
                        is_64_bit,
                    });
                }
                Err(reason) => refused(i, reason),
            }
        }
        upper_half = is_64_bit;
    }
    out
}

/// The most entries of a capability list any walk in this crate follows. A list is a linked list
/// the function supplies, so a cycle or a very long chain must end the walk rather than hang the
/// caller that is enumerating hardware nobody has vetted. A conforming list fits: capabilities are
/// at least four bytes and live in the 192 bytes above the standard header, so it has at most 48
/// entries.
const CAPABILITY_HOPS: usize = 64;

/// What [`capabilities`] hands each entry: its offset, its first dword and `read32` for the rest.
/// Returns `true` to stop the walk.
type CapabilityVisitor<'a> =
    dyn FnMut(u64, u32, &mut dyn FnMut(BusDeviceFunction, u64) -> u32) -> bool + 'a;

/// **The one walk of a function's capability list** (PCI Local Bus Specification 3.0, section 6.7),
/// which [`virtual_io_capabilities`], [`msi_capability`] and [`msix_capability`] share so a single bound
/// and a single proof cover all three.
///
/// Status bit 4 says a list exists, the pointer at [`CAPABILITIES_POINTER_REGISTER`] gives the first
/// entry, and each entry starts `{capability_id: u8, next: u8, ...}` with the pointer's low two bits
/// reserved. `visit` gets the entry's offset (nonzero, a multiple of four), its first dword and
/// `read32` for the rest of the entry; it returns `true` to stop. At most [`CAPABILITY_HOPS`]
/// entries are visited, so a cycle ends the walk.
fn capabilities(
    bus_device_function: BusDeviceFunction,
    read32: &mut dyn FnMut(BusDeviceFunction, u64) -> u32,
    visit: &mut CapabilityVisitor<'_>,
) {
    let status = (read32(bus_device_function, COMMAND_REGISTER) >> 16) as u16;
    if status & STATUS_CAPABILITIES_LIST == 0 {
        return;
    }
    let mut at = (read32(bus_device_function, CAPABILITIES_POINTER_REGISTER) & 0xfc) as u64;
    for _ in 0..CAPABILITY_HOPS {
        if at == 0 {
            return;
        }
        let head = read32(bus_device_function, at);
        if visit(at, head, read32) {
            return;
        }
        at = ((head >> 8) & 0xfc) as u64;
    }
}

/// The virtio vendor capability id in a function's capability list (virtio 1.2, section 4.1.4).
const CAPABILITY_ID_VENDOR: u8 = 0x09;

/// The virtio vendor capability's `cfg_type` for the common configuration structure (virtio 1.2,
/// section 4.1.4).
///
/// Name: ratified 2026-11-20. Spelled out from `VIRTIO_CAP_COMMON`.
#[doc(alias = "virtio")]
pub const VIRTUAL_IO_CAPABILITY_COMMON: u8 = 1;
/// The `cfg_type` for the notification structure: the doorbells (virtio 1.2, section 4.1.4.4).
///
/// Name: ratified 2026-11-20. Spelled out from `VIRTIO_CAP_NOTIFY`.
#[doc(alias = "virtio")]
pub const VIRTUAL_IO_CAPABILITY_NOTIFY: u8 = 2;
/// The `cfg_type` for the interrupt status structure (virtio 1.2, section 4.1.4.5): one byte whose
/// read acknowledges a legacy interrupt.
///
/// Name: ratified 2026-11-20. Spelled out from `VIRTIO_CAP_ISR`, with the specification's abbreviation as
/// the alias.
#[doc(alias = "ISR")]
#[doc(alias = "virtio")]
pub const VIRTUAL_IO_CAPABILITY_INTERRUPT_STATUS: u8 = 3;
/// The `cfg_type` for the device-specific configuration structure (virtio 1.2, section 4.1.4.6).
///
/// Name: ratified 2026-11-20. Spelled out from `VIRTIO_CAP_DEVICE`.
#[doc(alias = "virtio")]
pub const VIRTUAL_IO_CAPABILITY_DEVICE: u8 = 4;

/// A virtio vendor capability, decoded (virtio 1.2, section 4.1.4, `struct virtio_pci_cap`): which
/// register block it names, and where that block lives as (BAR index, offset into the BAR,
/// length). `notify_offset_multiplier` is meaningful only for the notify capability: the doorbell
/// for queue N is at `offset + queue_notify_off * notify_offset_multiplier`.
///
/// Name: ratified 2026-11-20. Spelled out from `VirtioCap`. `configuration_type` and
/// `notify_offset_multiplier` spell out the specification's `cfg_type` and `notify_off_multiplier`,
/// which stay as aliases because they are the specification's own field names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[doc(alias = "virtio")]
pub struct VirtualIoCapability {
    /// Which structure this capability describes: one of the `VIRTUAL_IO_CAPABILITY_` constants, or a
    /// value a later specification adds.
    #[doc(alias = "cfg_type")]
    pub configuration_type: u8,
    /// The BAR index (0 to 5) that holds the structure.
    pub bar: u8,
    /// The structure's byte offset into that BAR.
    pub offset: u32,
    /// The structure's length in bytes. Mirrors the specification's `length` field.
    #[doc(alias = "length")]
    pub length_bytes: u32,
    /// The notify capability's `notify_off_multiplier`; 0 for every other capability.
    #[doc(alias = "notify_off_multiplier")]
    pub notify_offset_multiplier: u32,
}

/// Walk the capability list and call `f` for each **virtio vendor capability** (capability id
/// 0x09, virtio 1.2, section 4.1.4). The list walk and its 64-entry bound are the crate's one
/// capability walk, so a cycle in a hostile or broken function terminates instead of hanging.
///
/// A vendor capability states its own length in byte 2 (`cap_len`): the specification fixes 16
/// bytes as the least a vendor capability holds (20 for the notify one, which carries a
/// multiplier). One that states less is bad data, and the fields past its end belong to the next
/// entry, so it is skipped.
///
/// Name: ratified 2026-11-20. Spelled out from `virtio_caps`.
#[doc(alias = "virtio")]
pub fn virtual_io_capabilities(
    bus_device_function: BusDeviceFunction,
    read32: &mut dyn FnMut(BusDeviceFunction, u64) -> u32,
    f: &mut dyn FnMut(VirtualIoCapability),
) {
    capabilities(bus_device_function, read32, &mut |at, head, read32| {
        let capability_id = (head & 0xff) as u8;
        let capability_length = (head >> 16) & 0xff;
        let configuration_type = ((head >> 24) & 0xff) as u8;
        let least_length = if configuration_type == VIRTUAL_IO_CAPABILITY_NOTIFY {
            20
        } else {
            16
        };
        if capability_id == CAPABILITY_ID_VENDOR && capability_length >= least_length {
            // struct virtio_pci_cap: u8 id, next, cap_len, cfg_type; u8 bar; 3 pad; u32 offset,
            // length; the notify capability carries one extra u32 after that.
            f(VirtualIoCapability {
                configuration_type,
                bar: (read32(bus_device_function, at + 4) & 0xff) as u8,
                offset: read32(bus_device_function, at + 8),
                length_bytes: read32(bus_device_function, at + 12),
                notify_offset_multiplier: if configuration_type == VIRTUAL_IO_CAPABILITY_NOTIFY {
                    read32(bus_device_function, at + 16)
                } else {
                    0
                },
            });
        }
        false
    });
}

/// A function's **MSI-X capability**, decoded (PCI Local Bus Specification 3.0, section 6.8.2).
/// Where its vector table lives, as a (BAR index, byte offset into that BAR) pair, how many
/// entries it has, and where the capability itself sits in config space so a caller can write its
/// Message Control word.
///
/// **Why this and not MSI.** MSI (capability 0x05) puts its address and data in config space and
/// gives a function one contiguous block of vectors; MSI-X puts them in a table in a BAR, one
/// independently maskable entry per vector. The deciding property for a host with no board-specific
/// interrupt routing table is not the count: neither of them consults one, because the *function*
/// is told where to deliver.
///
/// Name: ratified 2026-11-20. Spelled out from `MsixCap`; `pending_bit_array_*` spell out `pba_*`.
#[doc(alias = "message signaled interrupt extended")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MsixCapability {
    /// Where the capability structure starts in config space, so [`MESSAGE_CONTROL_REGISTER`]
    /// can be written to enable the function's MSI-X.
    ///
    /// Name: ratified 2026-11-20. Spelled out from `cap_offset`.
    pub capability_offset: u64,
    /// How many table entries the function has. At least 1, at most 2048, by the field's own
    /// encoding (it holds the count minus one in eleven bits).
    ///
    /// Name: ratified 2026-11-20. Was `table_size`; a count names the things counted.
    pub table_entries: u16,
    /// Which BAR the vector table lives in (the table BIR, 0 to 7 by the field's width).
    pub table_bar: u8,
    /// The table's byte offset into that BAR. Eight-byte aligned by the field's encoding.
    pub table_offset: u32,
    /// Which BAR the pending bit array lives in. Often the table's, sometimes not.
    pub pending_bit_array_bar: u8,
    /// The pending bit array's byte offset into that BAR. A driver that maps the page this sits on
    /// cannot aim an interrupt, but a caller may still deny the page, because the rule "never the
    /// pages the interrupt message lives on" is easier to check whole.
    pub pending_bit_array_offset: u32,
}

/// The MSI-X capability id in a config-space capability list (PCI Local Bus Specification 3.0,
/// section 6.8.2).
///
/// Name: ratified 2026-11-20. Spelled out from `CAP_ID_MSIX`.
pub const CAPABILITY_ID_MSIX: u8 = 0x11;

/// The MSI capability id in a config-space capability list (PCI Local Bus Specification 3.0,
/// section 6.8.1).
///
/// Name: ratified 2026-11-20. Spelled out from `CAP_ID_MSI`.
pub const CAPABILITY_ID_MSI: u8 = 0x05;

/// MSI Message Control bit 0: **MSI Enable** (PCI Local Bus Specification 3.0, section 6.8.1.3).
pub const MSI_ENABLE: u16 = 1 << 0;

/// MSI Message Control bit 7: the function takes a 64-bit message address, which moves the data
/// register from offset 8 to offset 12 (PCI Local Bus Specification 3.0, section 6.8.1.3).
pub const MSI_64_BIT: u16 = 1 << 7;

/// A function's **MSI capability** (PCI Local Bus Specification 3.0, section 6.8.1), decoded:
/// where it is, and where its message data register sits, which depends on whether the address is
/// 64-bit.
///
/// **Why this exists beside [`MsixCapability`]:** some functions have MSI and no MSI-X; one that
/// made it matter is the xHCI controller of an Intel chipset. MSI keeps its address and data in
/// config space rather than in a BAR, so a driver that maps every page of its BAR still cannot
/// reach them.
///
/// Name: ratified 2026-11-20. Spelled out from `MsiCap`; `is_64` was `is_64` and did not say what is 64.
#[doc(alias = "message signaled interrupt")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MsiCapability {
    /// Where the capability starts in config space.
    ///
    /// Name: ratified 2026-11-20. Spelled out from `cap_offset`.
    pub capability_offset: u64,
    /// Whether the message address is 64 bits.
    pub is_64_bit: bool,
}

impl MsiCapability {
    /// Config-space offset of the Message Address register's low dword (PCI Local Bus
    /// Specification 3.0, section 6.8.1.4).
    ///
    /// Name: ratified 2026-11-20. Was `address_low`; the specification's name is "Message Address".
    #[doc(alias = "Message Address")]
    pub const fn message_address_low(&self) -> u64 {
        self.capability_offset + 4
    }

    /// Config-space offset of the Message Data register: 8 past the capability for a 32-bit
    /// address, 12 for a 64-bit one (section 6.8.1.5).
    pub const fn data(&self) -> u64 {
        self.capability_offset + if self.is_64_bit { 12 } else { 8 }
    }
}

/// Find the **MSI capability** of `bus_device_function`, or `None`. The same bounded walk as [`msix_capability`]
/// and [`virtual_io_capabilities`]: at most 64 entries, so a cycle in the list ends the walk, and an
/// offset that is a multiple of four, since the list's pointers drop their low two bits.
///
/// Name: ratified 2026-11-20. Spelled out from `msi_cap`.
pub fn msi_capability(
    bus_device_function: BusDeviceFunction,
    read32: &mut dyn FnMut(BusDeviceFunction, u64) -> u32,
) -> Option<MsiCapability> {
    let mut found = None;
    capabilities(bus_device_function, read32, &mut |at, head, _| {
        if (head & 0xff) as u8 == CAPABILITY_ID_MSI {
            let control = (head >> 16) as u16;
            found = Some(MsiCapability {
                capability_offset: at,
                is_64_bit: control & MSI_64_BIT != 0,
            });
        }
        found.is_some()
    });
    found
}

/// Offset of the Message Control word from the start of an MSI-X capability (PCI Local Bus
/// Specification 3.0, section 6.8.2.3). It is the upper half of the capability's first dword, so
/// a driver writes it with a 32-bit config write that also rewrites the (read-only) id and
/// next-pointer bytes, which is what the hardware expects.
///
/// Name: ratified 2026-11-20. Was `MESSAGE_CONTROL`; a register offset ends in `_REGISTER`.
pub const MESSAGE_CONTROL_REGISTER: u64 = 0x02;

/// Message Control bit 15: **MSI-X Enable** (PCI Local Bus Specification 3.0, section 6.8.2.3).
/// Until it is set the function delivers INTx.
pub const MSIX_ENABLE: u16 = 1 << 15;

/// Message Control bit 14: **Function Mask** (section 6.8.2.3). Set, every vector is masked
/// whatever its own per-entry control bit says. It is set at reset on some parts, so a driver
/// clears it explicitly.
pub const MSIX_FUNCTION_MASK: u16 = 1 << 14;

/// How many bytes one MSI-X table entry occupies: message address (64 bits), message data (32),
/// vector control (32) (PCI Local Bus Specification 3.0, section 6.8.2.7). Fixed by the
/// specification, not by the function.
pub const MSIX_ENTRY_BYTES: u64 = 16;

/// Vector control bit 0: this entry is masked (PCI Local Bus Specification 3.0, section 6.8.2.9).
/// An entry powers on masked, so unmasking is a deliberate act.
pub const MSIX_ENTRY_MASKED: u32 = 1;

/// The virtio-pci "no vector" value for `config_msix_vector` and `queue_msix_vector` (virtio 1.2,
/// section 4.1.4.3). Written to a queue's vector field it means "raise no MSI-X for this queue",
/// which is also what a device reset leaves there.
///
/// Name: ratified 2026-11-20. Spelled out from `VIRTIO_MSIX_NO_VECTOR`.
#[doc(alias = "virtio")]
pub const VIRTUAL_IO_MSIX_NO_VECTOR: u16 = 0xffff;

/// **Where a function should write to raise an interrupt, and what to write there.** One MSI-X
/// table entry's payload (PCI Local Bus Specification 3.0, section 6.8.2.7), as the interrupt
/// controller of the machine defines it.
///
/// The pair is the PCI specification's, not any one architecture's: the x86 local APIC takes a
/// vector in `data` at an address encoding the destination processor, and an Arm GICv3 interrupt
/// translation service takes an event id at its doorbell register. A caller writes whatever pair
/// the machine hands it and understands neither.
///
/// Name: ratified 2026-11-20, with [`MsixCapability`] and [`msix_capability`]. `message_address` was
/// `address`; the specification's name is "Message Address".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MsiTarget {
    /// The address the function writes to (the specification's "Message Address"). 64-bit,
    /// written to the entry's low two dwords.
    #[doc(alias = "Message Address")]
    pub message_address: u64,
    /// The value it writes there (the specification's "Message Data").
    pub data: u32,
}

/// Find the **MSI-X capability** of `bus_device_function`, or `None` if the function has none.
///
/// Same walk as [`virtual_io_capabilities`], and bounded the same way (64 hops), for the same reason: the
/// list is a function-supplied linked list, so a broken or hostile function must terminate the walk
/// rather than hang the caller that is enumerating it.
///
/// Name: ratified 2026-11-20. Spelled out from `msix_cap`.
pub fn msix_capability(
    bus_device_function: BusDeviceFunction,
    read32: &mut dyn FnMut(BusDeviceFunction, u64) -> u32,
) -> Option<MsixCapability> {
    let mut found = None;
    capabilities(bus_device_function, read32, &mut |at, head, read32| {
        if (head & 0xff) as u8 == CAPABILITY_ID_MSIX {
            let control = (head >> 16) as u16;
            let table = read32(bus_device_function, at + 4);
            let pending = read32(bus_device_function, at + 8);
            found = Some(MsixCapability {
                capability_offset: at,
                // Bits 10:0 hold the count MINUS ONE, so a one-entry table reads as zero and the
                // maximum 2048-entry table reads as 0x7ff. Reading it as the count itself gives a
                // table one short of the truth on every function, and zero entries on a function that
                // has one, which is the shape of failure that looks like "no MSI-X here".
                table_entries: (control & 0x7ff) + 1,
                table_bar: (table & 0x7) as u8,
                table_offset: table & !0x7,
                pending_bit_array_bar: (pending & 0x7) as u8,
                pending_bit_array_offset: pending & !0x7,
            });
        }
        found.is_some()
    });
    found
}

/// The INTx swizzle on QEMU's `virt` boards: the function at device `d` using legacy pin `p`
/// (1 is INTA to 4 is INTD) lands on interrupt input `base + ((d + p - 1) % 4)`, the bridge
/// swizzle the PCI-to-PCI Bridge Architecture Specification 1.2, section 9 prescribes for a flat
/// bus, which QEMU's generic ECAM host bridge implements. A board that routes differently needs its
/// own `interrupt-map`, which is the authoritative routing table the device tree states (Devicetree
/// Specification, section 2.4.3).
///
/// Precondition: the function sits on bus 0, directly under the host bridge. A function behind a
/// bridge has its pin rotated by every bridge between it and the host first; this function does not
/// do that rotation.
///
/// # Examples
///
/// ```
/// use pci_configuration_space::intx_interrupt_number;
///
/// // Four devices, each on INTA, spread across four interrupt inputs rather than sharing one.
/// let base = 32;
/// let spread: Vec<u32> = (0..4).map(|device| intx_interrupt_number(base, device, 1)).collect();
/// assert_eq!(spread, vec![32, 33, 34, 35]);
///
/// // The swizzle wraps, which is what makes it a swizzle rather than an offset.
/// assert_eq!(intx_interrupt_number(base, 4, 1), 32);
/// // And one device's four pins also spread, so a multi-function card does not self-collide.
/// assert_eq!(intx_interrupt_number(base, 0, 4), 35);
/// ```
///
/// Name: ratified 2026-10-27.
#[doc(alias = "irq")]
#[doc(alias = "INTx")]
pub fn intx_interrupt_number(base: u32, device: u8, pin: u8) -> u32 {
    // Total for every input, proved in the verification module. Pins are 1-based (1=INTA); the
    // saturating_sub means a (contract-violating) pin of 0 behaves as INTA instead of
    // underflowing, and the saturating_add means a nonsense base cannot wrap. The callers all
    // check pin == 0 before calling; this is defence in depth, not an invitation.
    base.saturating_add((device as u32 + (pin as u32).saturating_sub(1)) % 4)
}

/// The 32-bit non-prefetchable memory window from a PCI host bridge's `ranges` property, as
/// `(cpu_base, size)`. `None` when no entry qualifies, or the property is not the standard shape.
///
/// `ranges` is big-endian cells, seven per entry (child `#address-cells = 3`, parent 2,
/// `#size-cells = 2`; Devicetree Specification, section 2.3.8, with the PCI binding's
/// `phys.hi` encoding from the "PCI Bus Binding to Open Firmware", section 2.2.1). `phys.hi`
/// carries the space code at bits 25:24 (0b01 I/O, 0b10 32-bit memory, 0b11 64-bit memory) and
/// prefetchability at bit 30; then come the 64-bit PCI address, CPU address and size. The first
/// 32-bit non-prefetchable entry is the window a caller assigns BARs from, and a prefetchable
/// 32-bit entry is not taken. An entry whose PCI and CPU addresses differ is skipped, not
/// translated: a caller that writes one number into both a BAR and its page tables needs an
/// identity window, and both QEMU `virt` boards state identity windows. A ragged length returns
/// `None` rather than a guess.
///
/// Name: ratified 2026-11-20. Was `mem32_window`; the function takes the first 32-bit window that is not
/// prefetchable, and `non_prefetchable_memory_window` was considered, since it names the entry
/// kind it refuses.
pub fn memory_window_32_bit(ranges: &[u8]) -> Option<(u64, u64)> {
    const ENTRY: usize = 7 * 4;
    if ranges.is_empty() || !ranges.len().is_multiple_of(ENTRY) {
        return None;
    }
    for entry in ranges.as_chunks::<ENTRY>().0 {
        let cell = |i: usize| -> u64 {
            u64::from(u32::from_be_bytes(
                entry[i * 4..i * 4 + 4].try_into().unwrap(),
            ))
        };
        let hi = cell(0);
        let space = (hi >> 24) & 0b11;
        let prefetchable = hi & (1 << 30) != 0;
        if space != 0b10 || prefetchable {
            continue;
        }
        let pci_address = (cell(1) << 32) | cell(2);
        let cpu_address = (cell(3) << 32) | cell(4);
        let size = (cell(5) << 32) | cell(6);
        if pci_address != cpu_address || size == 0 {
            continue;
        }
        return Some((cpu_address, size));
    }
    None
}

/// Machine-checked proofs, run with `cargo kani`.
///
/// A hostile or broken PCI function can return any bytes through the config-space closures, and
/// the decode runs in the kernel, so the properties proved are the hostile-input ones: the walks
/// are total and a cycle in a capability list terminates. [`BusQueue`], which [`walk`] indexes by
/// what a device said, is proved for every sequence firmware could write, which no fixture can
/// show. The scan loops are bounded by literals and not proved: a bounded model checker does not
/// finish unrolling 256 buses of 32 devices, and the queue's bound settles the only part a device
/// controls. A harness of [`walk`] itself on a two-bus machine was tried and stopped after more
/// than twelve CPU-minutes without a result, so it is not kept. `enumerate` is [`walk`] with a
/// field dropped, so it has no proof of its own.
#[cfg(kani)]
mod verification {
    use super::*;

    /// **ECAM addressing stays inside the window.** Any BDF's config page lies below the
    /// 256-bus window size (`0x1000_0000`), so `ecam_base + ecam_offset() + off` for off < 4096
    /// cannot escape a correctly-sized mapping. This is the arithmetic the kernel's volatile
    /// accessors trust.
    /// Falsification: replayable `crates/pci_configuration_space/falsifications/verification.ecam_offset_stays_inside_the_window.patch`
    #[kani::proof]
    fn ecam_offset_stays_inside_the_window() {
        let bus_device_function = BusDeviceFunction {
            bus: kani::any(),
            device: kani::any(),
            function: kani::any(),
        };
        // No assumption on `device` and `function`: the offset masks them to their fields.
        assert!(bus_device_function.ecam_offset() + 0xfff < 0x1000_0000);
    }

    /// **`intx_interrupt_number` is total and lands on one of the four lines.** For any base,
    /// device, and
    /// pin, no underflow (the pin-0 case that used to panic in debug builds) and no overflow,
    /// and the result is within `base..=base+3` whenever that range exists.
    /// Falsification: replayable `crates/pci_configuration_space/falsifications/verification.intx_interrupt_number_is_total_and_bounded.patch`
    #[kani::proof]
    #[doc(alias = "irq")]
    fn intx_interrupt_number_is_total_and_bounded() {
        let base: u32 = kani::any();
        let device: u8 = kani::any();
        let pin: u8 = kani::any();
        let interrupt_number = intx_interrupt_number(base, device, pin);
        assert!(interrupt_number >= base || interrupt_number == u32::MAX);
        assert!(interrupt_number.saturating_sub(base) <= 3 || interrupt_number == u32::MAX);
    }

    /// **`read_bars` is total for any device responses, and every BAR it returns can be used.**
    /// The closures return arbitrary values on every call, standing in for a device that answers
    /// the size probe with garbage; the decode must never panic, and each BAR it keeps has a
    /// power-of-two size and an end that does not overflow, which is what the kernel's placement
    /// and bounds arithmetic rely on.
    /// Falsification: replayable `crates/pci_configuration_space/falsifications/verification.read_bars_is_total_for_any_device.patch`
    #[kani::proof]
    #[kani::unwind(8)]
    fn read_bars_is_total_for_any_device() {
        let bus_device_function = BusDeviceFunction {
            bus: kani::any(),
            device: kani::any(),
            function: kani::any(),
        };
        let bars = read_bars(
            bus_device_function,
            &mut |_, _| kani::any(),
            &mut |_, _, _| {},
            &mut |_, _| {},
        );
        for bar in bars.iter().flatten() {
            assert!(bar.size_bytes.is_power_of_two() && bar.size_bytes >= 16);
            assert!(
                bar.base_physical_address
                    .checked_add(bar.size_bytes)
                    .is_some()
            );
        }
    }

    /// **A BAR size is the boundary between the bits a device ignores and the bits it decodes.**
    /// For any mask of either width: when a size comes back, every address bit at or above it is
    /// writable and every one below it is not, so the size is exactly the region the mask
    /// describes. And every mask that does describe a region, the width's address bits from some
    /// power of two up, gets that size back rather than a refusal.
    /// Falsification: replayable `crates/pci_configuration_space/falsifications/verification.a_bar_size_is_where_the_writable_bits_begin.patch`
    #[kani::proof]
    fn a_bar_size_is_where_the_writable_bits_begin() {
        let decode_bits = if kani::any() {
            BAR_DECODE_BITS_64
        } else {
            BAR_DECODE_BITS_32
        };
        let mask = kani::any::<u64>() & decode_bits;
        if let Ok(size) = size_from_mask(mask, decode_bits) {
            let bit: u32 = kani::any();
            kani::assume(bit < 64 && decode_bits & (1 << bit) != 0);
            assert_eq!(mask & (1 << bit) != 0, (1u64 << bit) >= size);
        }

        let shift: u32 = kani::any();
        kani::assume((4..64).contains(&shift) && decode_bits & (1 << shift) != 0);
        let region = 1u64 << shift;
        assert_eq!(
            size_from_mask(decode_bits & !(region - 1), decode_bits),
            Ok(region)
        );
    }

    /// **The capability walk terminates and never panics, even on a cyclic list.** The
    /// closures return arbitrary values, standing in for a device whose capability pointers
    /// form any graph at all; the walk visits at most 64 entries and the callback fires at
    /// most that often. This is the bounded-walk discipline (the virtqueue chain walk's twin)
    /// proved rather than argued.
    /// Falsification: replayable `crates/pci_configuration_space/falsifications/verification.the_capability_walk_terminates_on_any_device.patch`
    #[kani::proof]
    #[kani::unwind(66)]
    fn the_capability_walk_terminates_on_any_device() {
        let bus_device_function = BusDeviceFunction {
            bus: kani::any(),
            device: kani::any(),
            function: kani::any(),
        };
        let mut calls = 0u32;
        virtual_io_capabilities(bus_device_function, &mut |_, _| kani::any(), &mut |_| {
            calls += 1;
        });
        assert!(calls <= 64);
    }

    /// **The MSI-X capability walk terminates and its decoded table size is in range**, on any
    /// device response at all. The size field is eleven bits plus one, so the result is
    /// `1..=2048` and the `+ 1` cannot overflow a `u16`; a device that answers every config read
    /// with garbage must still leave the kernel with a bounded, non-panicking answer, because
    /// this walk runs during enumeration of hardware nobody has vetted.
    /// Falsification: replayable `crates/pci_configuration_space/falsifications/verification.the_msix_walk_terminates_and_bounds_its_table_size.patch`
    #[kani::proof]
    #[kani::unwind(66)]
    fn the_msix_walk_terminates_and_bounds_its_table_size() {
        let bus_device_function = BusDeviceFunction {
            bus: kani::any(),
            device: kani::any(),
            function: kani::any(),
        };
        if let Some(capability) = msix_capability(bus_device_function, &mut |_, _| kani::any()) {
            assert!(capability.table_entries >= 1 && capability.table_entries <= 2048);
            assert!(capability.table_bar < 8);
            assert!(capability.table_offset % 8 == 0);
        }
    }

    /// **The MSI capability walk terminates and never panics**, on any device response at all,
    /// like the virtio capability walk and the MSI-X walk. It reads the
    /// status and the list pointer once and one head per hop, at most 64 hops, so 66 reads; and a
    /// capability it finds sits at a nonzero offset that is a multiple of four.
    /// Falsification: replayable `crates/pci_configuration_space/falsifications/verification.the_message_signaled_interrupt_walk_terminates_on_any_device.patch`
    #[kani::proof]
    #[kani::unwind(66)]
    fn the_message_signaled_interrupt_walk_terminates_on_any_device() {
        let location = BusDeviceFunction {
            bus: kani::any(),
            device: kani::any(),
            function: kani::any(),
        };
        let mut reads = 0u32;
        let found = msi_capability(location, &mut |_, _| {
            reads += 1;
            kani::any()
        });
        assert!(reads <= 66);
        if let Some(capability) = found {
            assert!(capability.capability_offset != 0 && capability.capability_offset % 4 == 0);
        }
    }

    /// **The bus queue never runs off the end of its array, whatever firmware wrote in the
    /// bridges.** The bound is not local: `enqueue` writes `queue[tail]` and nothing there checks
    /// `tail` against 256.
    ///
    /// Proved as the invariant `tail == the number of set bits in seen`, which bounds `tail` at
    /// 256: [`BusQueue::from_the_root`] establishes it and this proves `enqueue` preserves it from
    /// any reachable state. A harness pushing 256 times needed an unwind past three hundred and
    /// did not finish in twenty minutes.
    /// Falsification: replayable `crates/pci_configuration_space/falsifications/verification.the_bus_queue_stays_inside_its_array_on_any_firmware.patch`
    #[kani::proof]
    fn the_bus_queue_stays_inside_its_array_on_any_firmware() {
        fn queued(seen: &[u64; 4]) -> usize {
            seen.iter().map(|w| w.count_ones() as usize).sum()
        }

        // Any state the invariant admits, not merely the ones a walk happens to reach.
        let mut q = BusQueue::from_the_root();
        q.seen = kani::any();
        q.head = kani::any();
        q.tail = queued(&q.seen);
        kani::assume(q.head <= q.tail);

        let before = q.tail;
        q.enqueue(kani::any());

        assert!(
            q.tail == queued(&q.seen),
            "the count and the bitmap disagree, so nothing bounds the index"
        );
        assert!(q.tail <= 256, "the queue pushed past the last bus number");
        assert!(q.tail <= before + 1, "one enqueue queued more than one bus");
    }

    /// **And the initial state satisfies the invariant the proof above preserves.** Written
    /// separately because an induction with no base case proves nothing, and the base case here is
    /// exactly the kind of one-line fact that gets changed by someone adjusting
    /// [`BusQueue::from_the_root`] without reading the harness next to it.
    /// Falsification: replayable `crates/pci_configuration_space/falsifications/verification.a_fresh_bus_queue_holds_the_root_and_nothing_else.patch`
    #[kani::proof]
    fn a_fresh_bus_queue_holds_the_root_and_nothing_else() {
        let q = BusQueue::from_the_root();
        let queued: usize = q.seen.iter().map(|w| w.count_ones() as usize).sum();
        assert!(q.tail == queued && q.tail == 1);
        assert!(q.head == 0 && q.queue[0] == 0);
    }

    /// **The ranges parser is total.** The property comes from firmware's device tree, which is
    /// the same hostile-input class as a device's config space: any byte string at all must
    /// come back as an answer or a `None`, never a panic. Three entries covers every branch
    /// (the loop is per-entry with no state across iterations).
    /// Falsification: replayable `crates/pci_configuration_space/falsifications/verification.the_ranges_parse_is_total.patch`
    #[kani::proof]
    #[kani::unwind(4)]
    fn the_ranges_parse_is_total() {
        let bytes: [u8; 3 * 28] = kani::any();
        let len: usize = kani::any();
        kani::assume(len <= bytes.len());
        let _ = memory_window_32_bit(&bytes[..len]);
    }
}

#[cfg(test)]
mod tests;
