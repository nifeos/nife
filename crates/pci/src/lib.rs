//! PCI configuration space: the decode logic, and none of the MMIO. ECAM addressing, walking a
//! config space for devices, sizing a BAR and finding the virtio vendor capabilities, all over
//! read/write closures on `(bdf, offset)`: the kernel passes volatile accessors into the mapped
//! ECAM window and the tests pass an array, which is why this is host-testable. See notes/pcie.md.
//!
//! A function is addressed by **BDF** (bus, device, function) and has 4 KB of configuration space
//! at `(bus << 20) | (dev << 15) | (fn << 12)` in the flat ECAM window. Its first 64 bytes are the
//! standard header: ids, command and status, class, the **BARs** (where its register blocks live)
//! and the capability list pointer, whose vendor capabilities carry what virtio-modern needs.
//!
//! Name: ratified 2026-09-18 (calef, §154 (the acronym test is whether the phrase is spoken)):
//! nobody says "peripheral component interconnect", so it stays `pci`. Refused
//! `peripheral_component_interconnect` (§154).

#![cfg_attr(not(test), no_std)]
// milestone 68's ratchet is workspace-wide (§107); this crate opts out until its 24-item
// worklist (notes/doc-coverage.md) is burned down.
#![allow(missing_docs)]

/// A function's address: (bus, device, function).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bdf {
    pub bus: u8,
    pub dev: u8,
    pub func: u8,
}

impl Bdf {
    /// The function's byte offset inside the ECAM window: 4 KB per function, laid out
    /// `bus:8 | dev:5 | fn:3 | offset:12`. This shift-and-or IS the ECAM spec.
    pub fn ecam_offset(self) -> u64 {
        ((self.bus as u64) << 20) | ((self.dev as u64) << 15) | ((self.func as u64) << 12)
    }

    /// The **PCIe requester id**: the 16-bit `bus:8 | dev:5 | fn:3` a function stamps on every
    /// memory transaction it issues, and the key an IOMMU (SMMUv3 `StreamID`, RISC-V IOMMU `device_id`)
    /// looks a device up by. Both `virt` boards publish an identity `iommu-map` in the device tree,
    /// so this number is exactly the id the IOMMU sees. See kernel/src/iommu.rs (milestone 16b).
    pub fn requester_id(self) -> u32 {
        ((self.bus as u32) << 8) | ((self.dev as u32) << 3) | (self.func as u32)
    }
}

// Standardized config-space header offsets (type 0).
pub const VENDOR_ID: u64 = 0x00;
pub const COMMAND: u64 = 0x04;
/// Class code (24 bits) over revision id (8): `cfg_read32(bdf, CLASS_REVISION) >> 8` is the class.
pub const CLASS_REVISION: u64 = 0x08;
pub const HEADER_TYPE: u64 = 0x0e;
pub const BAR0: u64 = 0x10;
pub const CAP_PTR: u64 = 0x34;
pub const INTERRUPT_PIN: u64 = 0x3d;

/// **Header type 1: a PCI-to-PCI bridge**, which is what a PCIe root port and every switch port
/// present themselves as. The low seven bits of [`HEADER_TYPE`] are the layout code (0 an endpoint,
/// 1 a bridge, 2 the long-dead `CardBus` shape); bit 7 is the multifunction flag and is masked off
/// before comparing. A bridge's header has three BAR slots where an endpoint has six, and where an
/// endpoint's BAR4 and BAR5 sit it carries [`BUS_NUMBERS`] instead.
pub const HEADER_TYPE_BRIDGE: u8 = 1;

/// **Where a type-1 header states the buses behind it**: primary, secondary and subordinate bus
/// numbers in the low three bytes of the dword at 0x18, then the secondary latency timer. Firmware
/// writes these during its own enumeration and they are the only record of the topology that
/// exists; nothing derives them, and a kernel that does not read them cannot find a device behind a
/// root port. See [`bridge_buses`].
pub const BUS_NUMBERS: u64 = 0x18;

/// The NVMe class code: mass storage (0x01) / non-volatile memory (0x08) / NVMe I/O (0x02).
/// Matched by class rather than by vendor/device id on purpose: QEMU's controller is Red Hat
/// 1b36:0010, real drives are anything at all, and the class triple is the one identity the spec
/// requires of every one of them (NVMe 1.4 §3.1's PCI header requirements).
pub const CLASS_NVME: u32 = 0x01_08_02;

/// The xHCI class code: serial bus (0x0c) / USB (0x03) / xHCI (0x30). Matched by class for
/// [`CLASS_NVME`]'s reason: QEMU's `qemu-xhci` is Red Hat 1b36:000d, an Intel PCH's is 8086:a2af
/// or a hundred others, and the class triple is what the xHCI specification requires of all of
/// them (xHCI 1.2 section 5.2.1). Milestone 242 (USB host and HID).
pub const CLASS_XHCI: u32 = 0x0c_03_30;

/// Command register bits we set to bring a device up.
pub const CMD_MEMORY_SPACE: u16 = 1 << 1;
/// Bus mastering is DMA permission at the PCI level: without it the device cannot issue a single
/// memory transaction. The kernel-side confinement (the shadow ring) polices *where* the DMA
/// goes; this bit is what allows DMA to exist at all, so it is set last, after the transport is
/// registered.
pub const CMD_BUS_MASTER: u16 = 1 << 2;

/// Status register bit 4: this function has a capability list.
const STATUS_CAP_LIST: u16 = 1 << 4;

/// virtio's PCI vendor id, and the modern virtio-blk device id (0x1040 + device type 2).
pub const VIRTIO_VENDOR: u16 = 0x1af4;
pub const VIRTIO_BLK_MODERN: u16 = 0x1042;
/// The transitional (legacy-capable) virtio-blk id. We do not drive legacy, but we recognize it
/// so enumeration can say "found a disk, but it is transitional" instead of "no disk".
pub const VIRTIO_BLK_TRANSITIONAL: u16 = 0x1001;

/// The modern virtio-net device id (0x1040 + device type 1), and its transitional twin. Same
/// recognition role as the blk pair: the net stack (milestone 30) drives modern only, and a
/// transitional NIC is reported as such rather than counted as absent.
pub const VIRTIO_NET_MODERN: u16 = 0x1041;
pub const VIRTIO_NET_TRANSITIONAL: u16 = 0x1000;

/// The modern virtio-gpu device id (0x1040 + device type 16), milestone 29. There is **no
/// transitional twin**: the legacy id space (0x1000..0x103f) was allocated before virtio-gpu
/// existed, so a GPU is modern-or-nothing and enumeration passes `None` for the legacy id rather
/// than inventing one. QEMU's `virtio-gpu-pci` presents exactly this id.
pub const VIRTIO_GPU_MODERN: u16 = 0x1050;

/// The modern virtio-input device id (0x1040 + device type 18), milestone 29's keyboard. **No
/// transitional twin**, for the same reason virtio-gpu has none: the device type was allocated long
/// after the legacy id space. QEMU's `virtio-keyboard-pci` presents exactly this id, and so does
/// `virtio-tablet-pci`, which is worth knowing rather than discovering: **the id names the device
/// class, not the keyboard**, so a machine with both would have to read the device's configuration
/// space to tell them apart. We attach only a keyboard.
pub const VIRTIO_INPUT_MODERN: u16 = 0x1052;

/// The modern virtio-rng device id (0x1040 + device type 4), milestone 56's entropy source, and its
/// transitional twin. Unlike the GPU and the keyboard this one **does** have a legacy id: virtio-rng
/// is device type 4, early enough to have been allocated a slot in the 0x1000..0x103f space. We
/// drive modern only, so the legacy id is here for the same reason blk's and net's are, to let
/// enumeration say "found an RNG, but it is transitional" rather than "no RNG".
pub const VIRTIO_RNG_MODERN: u16 = 0x1044;
pub const VIRTIO_RNG_TRANSITIONAL: u16 = 0x1005;

/// The virtio **device type** numbers, as the virtio-mmio `DeviceID` register reports them and as
/// the PCI ids above encode them (`0x1040 + type`). The kernel carries the type through the PCI
/// transport so a driver's `DeviceID` read answers truthfully on either bus; see
/// `kernel/src/virtio.rs`.
pub const VIRTIO_TYPE_NET: u32 = 1;
pub const VIRTIO_TYPE_BLOCK: u32 = 2;
pub const VIRTIO_TYPE_ENTROPY: u32 = 4;
pub const VIRTIO_TYPE_GPU: u32 = 16;
pub const VIRTIO_TYPE_INPUT: u32 = 18;

/// **The buses a type-1 header says lie behind it.** Firmware's own enumeration wrote these; a
/// kernel that does not read them enumerates bus 0 and calls the rest of the machine absent.
///
/// `subordinate` is the *highest* bus number reachable through this bridge, so `secondary ..=
/// subordinate` is the whole subtree. A bridge that firmware configured but left empty states
/// `secondary == subordinate`; one firmware never configured at all reads back zeros, which
/// [`walk`] treats as "nothing behind it" rather than as a second route to bus 0.
///
/// Name: provisional (milestone 320 (every PCI bus the machine has)), every name listed here: this
/// type, [`bridge_buses`], [`Function`], [`walk`], [`BUS_NUMBERS`] and [`HEADER_TYPE_BRIDGE`].
/// calef names public items.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BridgeBuses {
    pub primary: u8,
    pub secondary: u8,
    pub subordinate: u8,
}

/// Decode the three bus numbers of a type-1 header. Total for any device response: it is three
/// byte fields of one dword and there is no arithmetic to overflow.
pub fn bridge_buses(bdf: Bdf, read32: &mut dyn FnMut(Bdf, u64) -> u32) -> BridgeBuses {
    let w = read32(bdf, BUS_NUMBERS);
    BridgeBuses {
        primary: (w & 0xff) as u8,
        secondary: ((w >> 8) & 0xff) as u8,
        subordinate: ((w >> 16) & 0xff) as u8,
    }
}

/// **One function the walk met**, with everything a census needs to print and everything the
/// walk itself needs to decide where to go next.
///
/// Name: provisional (milestone 320): calef names public items.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Function {
    pub bdf: Bdf,
    pub vendor: u16,
    pub device: u16,
    /// Class over subclass over programming interface, 24 bits: `CLASS_REVISION >> 8`. This is the
    /// identity [`CLASS_NVME`] is matched against, and the one thing on the bus that says what a
    /// function *is* rather than who made it.
    pub class: u32,
    /// The header layout code, **with the multifunction bit already masked off**: 0 an endpoint,
    /// [`HEADER_TYPE_BRIDGE`] a bridge. Masked here rather than at every call site because an
    /// unmasked comparison against 1 silently misses every multifunction bridge, which is what a
    /// switch's upstream port is.
    pub header_type: u8,
    /// Whether this function reported itself multifunction, which is the bit that decides whether
    /// functions 1..8 were probed at all.
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
/// the bit is set in the same breath, so at most 256 pushes can ever happen; `verification` proves
/// exactly that against any sequence of arbitrary bus numbers, which is the property no fixture
/// test can establish.
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
/// array). See notes/pcie.md, "More than one bus (milestone 320)", for why it walks the bridges.
pub fn walk(ceiling: u16, read32: &mut dyn FnMut(Bdf, u64) -> u32, f: &mut dyn FnMut(Function)) {
    let ceiling = ceiling.min(256);
    if ceiling == 0 {
        return;
    }
    let mut buses = BusQueue::from_the_root();

    while let Some(bus) = buses.next_bus() {
        for dev in 0..32 {
            let bdf0 = Bdf { bus, dev, func: 0 };
            if read32(bdf0, VENDOR_ID) & 0xffff == 0xffff {
                continue; // empty slot: one config read, which is the whole point of this guard
            }
            let type0 = (read32(bdf0, HEADER_TYPE & !3) >> ((HEADER_TYPE & 3) * 8)) & 0xff;
            let multifunction = type0 & 0x80 != 0;
            let funcs = if multifunction { 8 } else { 1 };
            for func in 0..funcs {
                let bdf = Bdf { bus, dev, func };
                let id = read32(bdf, VENDOR_ID);
                if id & 0xffff == 0xffff {
                    continue;
                }
                let raw = (read32(bdf, HEADER_TYPE & !3) >> ((HEADER_TYPE & 3) * 8)) & 0xff;
                let header_type = (raw & 0x7f) as u8;
                let bridge = (header_type == HEADER_TYPE_BRIDGE).then(|| {
                    let behind = bridge_buses(bdf, read32);
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
                    bdf,
                    vendor: (id & 0xffff) as u16,
                    device: (id >> 16) as u16,
                    class: read32(bdf, CLASS_REVISION) >> 8,
                    header_type,
                    multifunction,
                    bridge,
                });
            }
        }
    }
}

/// Every function the machine has, as (bdf, vendor, device): [`walk`] with the topology detail
/// dropped, which is all most callers want. `ceiling` is the exclusive bus bound the caller can
/// read; see [`walk`] for what happens at it.
///
/// # Examples
///
/// Enumerating a bus with no bus: a test's closure over an array stands in for the ECAM window.
///
/// ```
/// use pci::{Bdf, VENDOR_ID, VIRTIO_VENDOR, enumerate};
///
/// // A config space with exactly one device: modern virtio-blk at 00:01.0. Everything else reads
/// // 0xffff, which is the bus's way of saying nobody is home.
/// let mut read32 = |bdf: Bdf, off: u64| -> u32 {
///     if bdf == (Bdf { bus: 0, dev: 1, func: 0 }) && off == VENDOR_ID {
///         (0x1042 << 16) | VIRTIO_VENDOR as u32 // device 0x1042 in the high half
///     } else if off == VENDOR_ID {
///         0xffff_ffff
///     } else {
///         0 // header type 0: single function, so functions 1..8 are skipped
///     }
/// };
///
/// let mut found = Vec::new();
/// enumerate(1, &mut read32, &mut |bdf, vendor, device| {
///     found.push((bdf, vendor, device));
/// });
///
/// assert_eq!(found.len(), 1);
/// let (bdf, vendor, device) = found[0];
/// assert_eq!((vendor, device), (VIRTIO_VENDOR, 0x1042));
///
/// // ECAM addressing: 4 KB per function, `bus:8 | dev:5 | fn:3 | offset:12`. This shift-and-or is
/// // the whole of the spec.
/// assert_eq!(bdf.ecam_offset(), 1 << 15);
///
/// // And the requester id, which is the key an IOMMU looks the device up by. Both `virt` boards
/// // publish an identity `iommu-map`, so this number is exactly what the IOMMU sees.
/// assert_eq!(bdf.requester_id(), 0b0000_0000_0000_1000);
/// ```
pub fn enumerate(
    ceiling: u16,
    read32: &mut dyn FnMut(Bdf, u64) -> u32,
    f: &mut dyn FnMut(Bdf, u16, u16),
) {
    walk(ceiling, read32, &mut |fun| {
        f(fun.bdf, fun.vendor, fun.device);
    });
}

/// One decoded Base Address Register: where the register block is, how big, and how wide.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bar {
    /// The assigned base address (0 if firmware assigned nothing; the caller must then place it).
    pub base: u64,
    /// The region size in bytes, from the write-ones probe.
    pub size: u64,
    /// A 64-bit BAR consumes two BAR slots; the caller's index bookkeeping needs to know.
    pub is_64: bool,
}

/// Read and size the six BAR slots of a type-0 header. Returns one entry per **slot** (six),
/// `None` for the upper half of a 64-bit BAR and for I/O-space BARs (we drive memory BARs only).
///
/// Sizing is the standard dance the spec prescribes: write all-ones, read back the mask of
/// writable bits (the low bits that stay zero encode the alignment/size), restore the original.
/// The device is quiescent during this (memory decoding is off until the command register is
/// set), which is why enumeration runs before enable.
pub fn read_bars(
    bdf: Bdf,
    read32: &mut dyn FnMut(Bdf, u64) -> u32,
    write32: &mut dyn FnMut(Bdf, u64, u32),
) -> [Option<Bar>; 6] {
    let mut out = [None; 6];
    let mut i = 0;
    while i < 6 {
        let off = BAR0 + i as u64 * 4;
        let orig = read32(bdf, off);
        if orig & 1 != 0 {
            // An I/O-space BAR. x86 legacy; nothing we drive uses one.
            i += 1;
            continue;
        }
        let is_64 = orig & 0b110 == 0b100;

        write32(bdf, off, u32::MAX);
        let mask_lo = read32(bdf, off);
        write32(bdf, off, orig);

        if is_64 {
            let off_hi = off + 4;
            let orig_hi = read32(bdf, off_hi);
            write32(bdf, off_hi, u32::MAX);
            let mask_hi = read32(bdf, off_hi);
            write32(bdf, off_hi, orig_hi);

            let mask = ((mask_hi as u64) << 32) | (mask_lo as u64 & 0xffff_fff0);
            if mask != 0 {
                out[i] = Some(Bar {
                    base: ((orig_hi as u64) << 32) | (orig as u64 & 0xffff_fff0),
                    size: !mask + 1,
                    is_64: true,
                });
            }
            i += 2; // the upper half consumed a slot
        } else {
            let mask = mask_lo as u64 & 0xffff_fff0;
            if mask != 0 {
                out[i] = Some(Bar {
                    base: orig as u64 & 0xffff_fff0,
                    // Bits above a 32-bit BAR's reach can never be set; extend the mask so the
                    // `!mask + 1` size math is done at u64 width.
                    size: !(mask | 0xffff_ffff_0000_0000) + 1,
                    is_64: false,
                });
            }
            i += 1;
        }
    }
    out
}

/// What each virtio vendor capability describes (virtio spec 4.1.4: `cfg_type`).
pub const VIRTIO_CAP_COMMON: u8 = 1;
pub const VIRTIO_CAP_NOTIFY: u8 = 2;
pub const VIRTIO_CAP_ISR: u8 = 3;
pub const VIRTIO_CAP_DEVICE: u8 = 4;

/// A virtio vendor capability, decoded: which register block it names, and where that block
/// lives as (BAR index, offset into the BAR, length). `notify_off_multiplier` is meaningful only
/// for the notify capability (the doorbell for queue N is at
/// `offset + queue_notify_off * multiplier`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VirtioCap {
    pub cfg_type: u8,
    pub bar: u8,
    pub offset: u32,
    pub length: u32,
    pub notify_off_multiplier: u32,
}

/// Walk the capability list and call `f` for each **virtio vendor capability** (cap id 0x09).
/// The list is a linked list in config space: status bit 4 says one exists, 0x34 points at the
/// first entry, each entry is `{cap_id: u8, next: u8, ...}`. Bounded at 64 hops so a cycle in a
/// hostile or broken device terminates instead of hanging the walk.
pub fn virtio_caps(
    bdf: Bdf,
    read32: &mut dyn FnMut(Bdf, u64) -> u32,
    f: &mut dyn FnMut(VirtioCap),
) {
    const CAP_ID_VENDOR: u8 = 0x09;

    let status = (read32(bdf, COMMAND) >> 16) as u16;
    if status & STATUS_CAP_LIST == 0 {
        return;
    }
    let mut at = (read32(bdf, CAP_PTR) & 0xfc) as u64;
    for _ in 0..64 {
        if at == 0 {
            return;
        }
        let head = read32(bdf, at);
        let cap_id = (head & 0xff) as u8;
        let next = ((head >> 8) & 0xfc) as u64;
        if cap_id == CAP_ID_VENDOR {
            // struct virtio_pci_cap: u8 id, next, cap_len, cfg_type; u8 bar; 3 pad; u32 offset,
            // length; the notify capability carries one extra u32 after that.
            let cfg_type = ((head >> 24) & 0xff) as u8;
            let bar = (read32(bdf, at + 4) & 0xff) as u8;
            f(VirtioCap {
                cfg_type,
                bar,
                offset: read32(bdf, at + 8),
                length: read32(bdf, at + 12),
                notify_off_multiplier: if cfg_type == VIRTIO_CAP_NOTIFY {
                    read32(bdf, at + 16)
                } else {
                    0
                },
            });
        }
        at = next;
    }
}

/// A function's **MSI-X capability**, decoded (PCI 3.0 §6.8.2). Where its vector table lives, as
/// a (BAR index, byte offset into that BAR) pair, how many entries it has, and where the
/// capability itself sits in config space so a caller can write its Message Control word.
///
/// **Why this and not MSI.** MSI (capability 0x05) puts its address and data in config space and
/// gives a function one contiguous block of vectors; MSI-X puts them in a table in a BAR, one
/// independently maskable entry per vector, and every device this tree attaches has it. The
/// deciding property is not the count: it is that neither of them consults a board-specific
/// interrupt routing table, because the *device* is told where to deliver. See
/// `kernel/src/pci.rs` and design/roadmap/0215-x86-64-pci-interrupt-routing.md.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MsixCap {
    /// Where the capability structure starts in config space, so [`MESSAGE_CONTROL`] can be
    /// written to enable the function's MSI-X.
    pub cap_offset: u64,
    /// How many table entries the function has. At least 1, at most 2048, by the field's own
    /// encoding (it holds the count minus one in eleven bits).
    pub table_size: u16,
    /// Which BAR the vector table lives in ("BIR", 0..=5).
    pub table_bar: u8,
    /// The table's byte offset into that BAR. Eight-byte aligned by the field's encoding.
    pub table_offset: u32,
    /// Which BAR the pending-bit array lives in. Often the table's, sometimes not.
    pub pba_bar: u8,
    /// The pending-bit array's byte offset into that BAR. A driver mapped the page this sits on
    /// could not aim an interrupt, but milestone 242's xHCI driver is denied it anyway, because
    /// the rule "never the pages the interrupt message lives on" is easier to check whole.
    pub pba_offset: u32,
}

/// The MSI-X capability id in a config-space capability list.
pub const CAP_ID_MSIX: u8 = 0x11;

/// The MSI capability id in a config-space capability list.
pub const CAP_ID_MSI: u8 = 0x05;

/// MSI Message Control bit 0: **MSI Enable**.
pub const MSI_ENABLE: u16 = 1 << 0;

/// MSI Message Control bit 7: the function takes a 64-bit message address, which moves the data
/// register from offset 8 to offset 12.
pub const MSI_64_BIT: u16 = 1 << 7;

/// A function's **MSI capability** (PCI 3.0 section 6.8.1), decoded: where it is, and where its
/// message data register sits, which depends on whether the address is 64-bit.
///
/// **Why this exists beside [`MsixCap`]** (milestone 242 (USB host and HID)): some functions have
/// MSI and no MSI-X, and the one that made it matter is the xHCI on an Intel PCH, which is the USB
/// controller on xenon. MSI keeps its address and data in config space rather than in a BAR, so a
/// driver mapped every page of its BAR still cannot reach them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MsiCap {
    /// Where the capability starts in config space.
    pub cap_offset: u64,
    /// Whether the message address is 64 bits.
    pub is_64: bool,
}

impl MsiCap {
    /// Config-space offset of the message address's low dword.
    pub const fn address_low(&self) -> u64 {
        self.cap_offset + 4
    }

    /// Config-space offset of the message data word: 8 past the capability for a 32-bit
    /// address, 12 for a 64-bit one.
    pub const fn data(&self) -> u64 {
        self.cap_offset + if self.is_64 { 12 } else { 8 }
    }
}

/// Find the **MSI capability** of `bdf`, or `None`. The same bounded walk as [`msix_cap`].
pub fn msi_cap(bdf: Bdf, read32: &mut dyn FnMut(Bdf, u64) -> u32) -> Option<MsiCap> {
    let status = (read32(bdf, COMMAND) >> 16) as u16;
    if status & STATUS_CAP_LIST == 0 {
        return None;
    }
    let mut at = (read32(bdf, CAP_PTR) & 0xfc) as u64;
    for _ in 0..64 {
        if at == 0 {
            return None;
        }
        let head = read32(bdf, at);
        if (head & 0xff) as u8 == CAP_ID_MSI {
            let control = (head >> 16) as u16;
            return Some(MsiCap {
                cap_offset: at,
                is_64: control & MSI_64_BIT != 0,
            });
        }
        at = ((head >> 8) & 0xfc) as u64;
    }
    None
}

/// Offset of the Message Control word from the start of an MSI-X capability. It is the upper half
/// of the capability's first dword, so a driver writes it with a 32-bit config write that also
/// rewrites the (read-only) id and next-pointer bytes, which is what the hardware expects.
pub const MESSAGE_CONTROL: u64 = 0x02;

/// Message Control bit 15: **MSI-X Enable**. Until it is set the function delivers INTx.
pub const MSIX_ENABLE: u16 = 1 << 15;

/// Message Control bit 14: **Function Mask**. Set, every vector is masked whatever its own
/// per-entry control bit says. It is set at reset on some parts, so it is cleared explicitly.
pub const MSIX_FUNCTION_MASK: u16 = 1 << 14;

/// How many bytes one MSI-X table entry occupies: message address (64 bits), message data (32),
/// vector control (32). Fixed by the specification, not by the device.
pub const MSIX_ENTRY_BYTES: u64 = 16;

/// Vector control bit 0: this entry is masked. An entry powers on masked, so unmasking is a
/// deliberate act, the same posture `arch::x86_64::irq::init_io_apic` takes with its own table.
pub const MSIX_ENTRY_MASKED: u32 = 1;

/// The virtio-pci "no vector" value for `config_msix_vector` and `queue_msix_vector` (virtio 1.1
/// §4.1.4.3). Written to a queue's vector field it means "raise no MSI-X for this queue", which is
/// also what a device reset leaves there.
pub const VIRTIO_MSIX_NO_VECTOR: u16 = 0xffff;

/// **Where a device should write to raise an interrupt, and what to write there.** One MSI-X
/// table entry's payload, as the interrupt controller of the machine defines it.
///
/// It is in this crate rather than in an arch module because the pair is the PCI specification's,
/// not any one architecture's: x86's local APIC takes a vector in `data` at an address encoding
/// the destination CPU, and an arm `GICv3` ITS takes an event id at a doorbell register in the
/// interrupt translation service.
/// The kernel's enumerator writes whatever pair the machine hands it and understands neither.
///
/// Name: provisional (milestone 215 (a PCI function's interrupt reaches nothing)), with
/// [`MsixCap`] and [`msix_cap`]: calef names public items.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MsiTarget {
    /// The address the device writes to. 64-bit, written to the entry's low two dwords.
    pub address: u64,
    /// The value it writes there.
    pub data: u32,
}

/// Find the **MSI-X capability** of `bdf`, or `None` if the function has none.
///
/// Same walk as [`virtio_caps`], and bounded the same way (64 hops), for the same reason: the
/// list is a device-supplied linked list, so a broken or hostile function must terminate the walk
/// rather than hang the kernel that is enumerating it.
pub fn msix_cap(bdf: Bdf, read32: &mut dyn FnMut(Bdf, u64) -> u32) -> Option<MsixCap> {
    let status = (read32(bdf, COMMAND) >> 16) as u16;
    if status & STATUS_CAP_LIST == 0 {
        return None;
    }
    let mut at = (read32(bdf, CAP_PTR) & 0xfc) as u64;
    for _ in 0..64 {
        if at == 0 {
            return None;
        }
        let head = read32(bdf, at);
        if (head & 0xff) as u8 == CAP_ID_MSIX {
            let control = (head >> 16) as u16;
            let table = read32(bdf, at + 4);
            let pba = read32(bdf, at + 8);
            return Some(MsixCap {
                cap_offset: at,
                // Bits 10:0 hold the count MINUS ONE, so a one-entry table reads as zero and the
                // maximum 2048-entry table reads as 0x7ff. Reading it as the count itself gives a
                // table one short of the truth on every device, and zero entries on a device that
                // has one, which is the shape of failure that looks like "no MSI-X here".
                table_size: (control & 0x7ff) + 1,
                table_bar: (table & 0x7) as u8,
                table_offset: table & !0x7,
                pba_bar: (pba & 0x7) as u8,
                pba_offset: pba & !0x7,
            });
        }
        at = ((head >> 8) & 0xfc) as u64;
    }
    None
}

/// The INTx swizzle on QEMU's `virt` boards: the function at device `d` using legacy pin `p`
/// (1=INTA..4=INTD) lands on PLIC/GIC input `base + ((d + p - 1) % 4)`, the bridge swizzle the PCI
/// spec prescribes for a flat bus, which QEMU's generic ECAM bridge implements.
/// `tests/qemu_virt_dtb.rs` holds it against the machine's own `interrupt-map`, so a board that
/// routes differently fails a host test. See notes/pcie.md, "INTx".
///
/// # Examples
///
/// ```
/// use pci::intx_irq;
///
/// // Four devices, each on INTA, spread across four interrupt inputs rather than sharing one.
/// let base = 32;
/// let spread: Vec<u32> = (0..4).map(|dev| intx_irq(base, dev, 1)).collect();
/// assert_eq!(spread, vec![32, 33, 34, 35]);
///
/// // The swizzle wraps, which is what makes it a swizzle rather than an offset.
/// assert_eq!(intx_irq(base, 4, 1), 32);
/// // And one device's four pins also spread, so a multi-function card does not self-collide.
/// assert_eq!(intx_irq(base, 0, 4), 35);
/// ```
pub fn intx_irq(base: u32, dev: u8, pin: u8) -> u32 {
    // Total for every input, proved in the verification module. Pins are 1-based (1=INTA); the
    // saturating_sub means a (contract-violating) pin of 0 behaves as INTA instead of
    // underflowing, and the saturating_add means a nonsense base cannot wrap. The callers all
    // check pin == 0 before calling; this is defence in depth, not an invitation.
    base.saturating_add((dev as u32 + (pin as u32).saturating_sub(1)) % 4)
}

/// The 32-bit non-prefetchable memory window from a PCI host bridge's `ranges` property, as
/// `(cpu_base, size)`. `None` when no entry qualifies, or the property is not the standard shape.
///
/// `ranges` is big-endian cells, seven per entry (child `#address-cells = 3`, parent 2,
/// `#size-cells = 2`). `phys.hi` carries the space code at bits 25:24 (0b01 IO, 0b10 32-bit
/// memory, 0b11 64-bit memory) and prefetchability at bit 30; then come the 64-bit PCI address, CPU
/// address and size. The first 32-bit non-prefetchable entry is the window kernel/src/pci.rs
/// assigns BARs from. An entry whose PCI and CPU addresses differ is skipped, not translated: the
/// BAR assigner writes one number into both the BAR and its page tables (both QEMU `virt` boards
/// state identity windows). A ragged length returns `None` rather than a guess.
pub fn mem32_window(ranges: &[u8]) -> Option<(u64, u64)> {
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
        let pci_addr = (cell(1) << 32) | cell(2);
        let cpu_addr = (cell(3) << 32) | cell(4);
        let size = (cell(5) << 32) | cell(6);
        if pci_addr != cpu_addr || size == 0 {
            continue;
        }
        return Some((cpu_addr, size));
    }
    None
}

/// Machine-checked proofs (`script/verify`; notes/verification.md).
///
/// A hostile or broken PCI function can return any bytes through the config-space closures, and
/// the decode runs in the kernel, so the properties proved are the hostile-input ones: the walks
/// are total and a cycle in a capability list terminates. [`BusQueue`], which [`walk`] indexes by
/// what a device said, is proved for every sequence firmware could write, which no fixture can
/// show. The scan loops are bounded by literals and not proved: a bounded model checker does not
/// finish unrolling 256 buses of 32 devices, and the queue's bound settles the only part a device
/// controls. `enumerate` is [`walk`] with a field dropped, so it has no proof of its own.
#[cfg(kani)]
mod verification {
    use super::*;

    /// **ECAM addressing stays inside the window.** Any BDF's config page lies below the
    /// 256-bus window size (`0x1000_0000`), so `ecam_base + ecam_offset() + off` for off < 4096
    /// cannot escape a correctly-sized mapping. This is the arithmetic the kernel's volatile
    /// accessors trust.
    /// Falsification: replayable `crates/pci/falsifications/verification.ecam_offset_stays_inside_the_window.patch`
    #[kani::proof]
    fn ecam_offset_stays_inside_the_window() {
        let bdf = Bdf {
            bus: kani::any(),
            dev: kani::any(),
            func: kani::any(),
        };
        // dev and func are 5- and 3-bit fields by construction of every caller (enumerate
        // produces them from bounded loops); the offset must hold for all such values.
        kani::assume(bdf.dev < 32 && bdf.func < 8);
        assert!(bdf.ecam_offset() + 0xfff < 0x1000_0000);
    }

    /// **`intx_irq` is total and lands on one of the four lines.** For any base, device, and
    /// pin, no underflow (the pin-0 case that used to panic in debug builds) and no overflow,
    /// and the result is within `base..=base+3` whenever that range exists.
    /// Falsification: replayable `crates/pci/falsifications/verification.intx_irq_is_total_and_bounded.patch`
    #[kani::proof]
    fn intx_irq_is_total_and_bounded() {
        let base: u32 = kani::any();
        let dev: u8 = kani::any();
        let pin: u8 = kani::any();
        let irq = intx_irq(base, dev, pin);
        assert!(irq >= base || irq == u32::MAX);
        assert!(irq.saturating_sub(base) <= 3 || irq == u32::MAX);
    }

    /// **`read_bars` is total for any device responses.** The closures return arbitrary values
    /// on every call, standing in for a device that answers the size probe with garbage; the
    /// decode must never panic (the size arithmetic `!mask + 1` cannot overflow because the
    /// type bits are masked out of `mask` first, so it is never all-ones).
    /// Falsification: replayable `crates/pci/falsifications/verification.read_bars_is_total_for_any_device.patch`
    #[kani::proof]
    #[kani::unwind(8)]
    fn read_bars_is_total_for_any_device() {
        let bdf = Bdf {
            bus: kani::any(),
            dev: kani::any(),
            func: kani::any(),
        };
        let _ = read_bars(bdf, &mut |_, _| kani::any(), &mut |_, _, _| {});
    }

    /// **The capability walk terminates and never panics, even on a cyclic list.** The
    /// closures return arbitrary values, standing in for a device whose capability pointers
    /// form any graph at all; the walk visits at most 64 entries and the callback fires at
    /// most that often. This is the bounded-walk discipline (the virtqueue chain walk's twin)
    /// proved rather than argued.
    /// Falsification: replayable `crates/pci/falsifications/verification.the_capability_walk_terminates_on_any_device.patch`
    #[kani::proof]
    #[kani::unwind(66)]
    fn the_capability_walk_terminates_on_any_device() {
        let bdf = Bdf {
            bus: kani::any(),
            dev: kani::any(),
            func: kani::any(),
        };
        let mut calls = 0u32;
        virtio_caps(bdf, &mut |_, _| kani::any(), &mut |_| calls += 1);
        assert!(calls <= 64);
    }

    /// **The MSI-X capability walk terminates and its decoded table size is in range**, on any
    /// device response at all. The size field is eleven bits plus one, so the result is
    /// `1..=2048` and the `+ 1` cannot overflow a `u16`; a device that answers every config read
    /// with garbage must still leave the kernel with a bounded, non-panicking answer, because
    /// this walk runs during enumeration of hardware nobody has vetted.
    /// Falsification: replayable `crates/pci/falsifications/verification.the_msix_walk_terminates_and_bounds_its_table_size.patch`
    #[kani::proof]
    #[kani::unwind(66)]
    fn the_msix_walk_terminates_and_bounds_its_table_size() {
        let bdf = Bdf {
            bus: kani::any(),
            dev: kani::any(),
            func: kani::any(),
        };
        if let Some(cap) = msix_cap(bdf, &mut |_, _| kani::any()) {
            assert!(cap.table_size >= 1 && cap.table_size <= 2048);
            assert!(cap.table_bar < 8);
            assert!(cap.table_offset % 8 == 0);
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
    /// Falsification: replayable `crates/pci/falsifications/verification.the_bus_queue_stays_inside_its_array_on_any_firmware.patch`
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
    /// Falsification: replayable `crates/pci/falsifications/verification.a_fresh_bus_queue_holds_the_root_and_nothing_else.patch`
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
    /// Falsification: replayable `crates/pci/falsifications/verification.the_ranges_parse_is_total.patch`
    #[kani::proof]
    #[kani::unwind(4)]
    fn the_ranges_parse_is_total() {
        let bytes: [u8; 3 * 28] = kani::any();
        let len: usize = kani::any();
        kani::assume(len <= bytes.len());
        let _ = mem32_window(&bytes[..len]);
    }
}

#[cfg(test)]
mod tests;
