//! **What the kernel and a virtio driver granted direct agree on**: which pages of the function's
//! BARs the driver is mapped ([`register_window`]), where each virtio-pci structure sits in that
//! window, and the three spawn words ([`HandOff`]). A driver granted mediated holds a `Virtio`
//! capability instead and uses none of this. The virtio-pci layout itself is the specification's
//! (virtio 1.2 section 4.1.4); this module holds only nife's process plan around it. See
//! design/roadmap/1062-the-device-control-planes-leave-the-kernel.md, "Step 7".
//!
//! # BUGS
//!
//! - One register window, at most [`MAXIMUM_REGISTER_PAGES`] pages: a function whose four
//!   structures touch more distinct pages than that is refused, and its starter grants mediated.
//!
//! Name: provisional, minted 2026-11-11 (UTC) by lane/1062-virtio-direct-contract, after the
//! device family it serves, as this crate's other modules are named after their device crates.

use pci::{
    Bdf, VIRTIO_CAPABILITY_COMMON, VIRTIO_CAPABILITY_DEVICE, VIRTIO_CAPABILITY_ISR,
    VIRTIO_CAPABILITY_NOTIFY,
};
use startup_page_contract::WindowKind;

/// **One page, in bytes**: the granule of every mapping a starter makes.
const PAGE_BYTES: u64 = address_space_map::PAGE;

/// **How many pages the register window can hold**: the most distinct BAR pages the four
/// structures may touch together. QEMU's modern virtio-pci functions put each structure on a page
/// of its own, four in all; twice that leaves room for a structure that spans two pages.
///
/// Name: provisional, minted 2026-11-11 (UTC) by lane/1062-virtio-direct-contract.
pub const MAXIMUM_REGISTER_PAGES: usize = 8;

/// The register window's size in bytes. Every structure offset in a [`HandOff`] lies below it.
const WINDOW_BYTES: u64 = MAXIMUM_REGISTER_PAGES as u64 * PAGE_BYTES;

/// The register window's entry on the startup page: the pages of the four virtio-pci structures,
/// device memory, one after another in the order [`register_window`] lists them.
///
/// Name: provisional, minted 2026-11-11 (UTC) by lane/1062-virtio-direct-contract.
pub const REGISTER_KIND: WindowKind = WindowKind(12);
/// The DMA region's entry on the startup page: every page of it, normal memory. The device reaches
/// this region and nothing else.
///
/// Name: provisional, minted 2026-11-11 (UTC) by lane/1062-virtio-direct-contract.
pub const DATA_PLANE_KIND: WindowKind = WindowKind(13);

/// **The bytes of the common configuration structure** a virtio 1.0 device must present, through
/// `queue_device` (virtio 1.2 section 4.1.4.3). A shorter structure is refused.
///
/// Name: provisional, minted 2026-11-11 (UTC) by lane/1062-virtio-direct-contract.
pub const COMMON_CONFIGURATION_BYTES: u64 = 0x38;

/// The MSI-X table entry value that names no vector (virtio 1.2 section 4.1.5.1.2,
/// `VIRTIO_MSI_NO_VECTOR`), which [`HandOff`] packs for a function that
/// delivers its legacy pin interrupt.
const NO_VECTOR: u16 = 0xffff;

/// The device-configuration offset value that names no structure, above every window offset.
const NO_DEVICE_CONFIGURATION: u16 = 0xffff;

// Both sentinels lie past every offset the window can hold, so neither is ever a real offset.
const _: () = assert!(WINDOW_BYTES <= NO_DEVICE_CONFIGURATION as u64);

/// **One virtio-pci structure**, as its capability names it (virtio 1.2 section 4.1.4): which BAR,
/// and where in it. The starter has already checked that it lies inside that BAR.
///
/// Name: provisional, minted 2026-11-11 (UTC) by lane/1062-virtio-direct-contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Structure {
    /// The BAR index, 0 to 5.
    pub bar: u8,
    /// The structure's byte offset into that BAR.
    pub offset: u64,
    /// **Its length in bytes** (length, virtio 1.2 section 4.1.4, `struct virtio_pci_cap`).
    #[doc(alias = "length in bytes")]
    pub length: u64,
}

/// **The four structures a direct driver reaches**: the common configuration, the notify
/// doorbells, the ISR (Interrupt Status Register) byte and, where the device has one, its device-specific configuration.
///
/// Name: provisional, minted 2026-11-11 (UTC) by lane/1062-virtio-direct-contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Structures {
    /// `VIRTIO_PCI_CAP_COMMON_CFG`.
    pub common: Structure,
    /// `VIRTIO_PCI_CAP_NOTIFY_CFG`.
    pub notify: Structure,
    /// `VIRTIO_PCI_CAP_ISR_CFG`.
    pub interrupt_status: Structure,
    /// `VIRTIO_PCI_CAP_DEVICE_CFG`, absent for a device type without one.
    pub device: Option<Structure>,
}

/// **The four structures a function's capability list names, and its notify multiplier**, or
/// `None` for a function that lacks a common configuration, a notify structure or an ISR byte. The
/// first capability of each type wins, as virtio 1.2 section 4.1.4 tells a driver to prefer it.
///
/// `read32` reads the function's configuration space, as [`pci::virtio_capabilities`] takes it.
///
/// Name: provisional, minted 2026-11-11 (UTC) by lane/1062-virtio-direct-contract.
pub fn structures(
    function: Bdf,
    read32: &mut dyn FnMut(Bdf, u64) -> u32,
) -> Option<(Structures, u32)> {
    let (mut common, mut notify, mut interrupt_status, mut device) = (None, None, None, None);
    let mut multiplier = 0;
    pci::virtio_capabilities(function, read32, &mut |c| {
        let s = Structure {
            bar: c.bar,
            offset: u64::from(c.offset),
            length: u64::from(c.length),
        };
        match c.cfg_type {
            VIRTIO_CAPABILITY_COMMON if common.is_none() => common = Some(s),
            VIRTIO_CAPABILITY_NOTIFY if notify.is_none() => {
                notify = Some(s);
                multiplier = c.notify_off_multiplier;
            }
            VIRTIO_CAPABILITY_ISR if interrupt_status.is_none() => interrupt_status = Some(s),
            VIRTIO_CAPABILITY_DEVICE if device.is_none() => device = Some(s),
            _ => {}
        }
    });
    Some((
        Structures {
            common: common?,
            notify: notify?,
            interrupt_status: interrupt_status?,
            device,
        },
        multiplier,
    ))
}

/// **The MSI-X table and pending-bit array of a function, as byte ranges of their BARs**, whether
/// or not this machine delivers the function's interrupt by MSI-X: a driver that could write the
/// table could enable it and aim it. A function without MSI-X withholds nothing, two empty ranges.
///
/// Name: provisional, minted 2026-11-11 (UTC) by lane/1062-virtio-direct-contract.
pub fn withheld(function: Bdf, read32: &mut dyn FnMut(Bdf, u64) -> u32) -> [Withheld; 2] {
    let none = Withheld {
        bar: 0,
        start: 0,
        end: 0,
    };
    let Some(c) = pci::msix_capability(function, read32) else {
        return [none; 2];
    };
    let (table, pending) = (u64::from(c.table_offset), u64::from(c.pba_offset));
    [
        Withheld {
            bar: c.table_bar,
            start: table,
            end: table + u64::from(c.table_size) * pci::MSIX_ENTRY_BYTES,
        },
        // One bit per entry, in whole quadwords.
        Withheld {
            bar: c.pba_bar,
            start: pending,
            end: pending + u64::from(c.table_size).div_ceil(64) * 8,
        },
    ]
}

/// **A byte range of one BAR the driver must never be mapped**: the MSI-X table or the pending-bit
/// array. Half open, `start..end`.
///
/// Name: provisional, minted 2026-11-11 (UTC) by lane/1062-virtio-direct-contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Withheld {
    /// The BAR index the range lies in.
    pub bar: u8,
    /// Its first byte.
    pub start: u64,
    /// One past its last byte.
    pub end: u64,
}

/// **One page of the register window**: which BAR, and the page's byte offset into it.
///
/// Name: provisional, minted 2026-11-11 (UTC) by lane/1062-virtio-direct-contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct RegisterPage {
    /// The BAR index.
    pub bar: u8,
    /// The page's byte offset into that BAR, page aligned.
    pub offset: u64,
}

/// **The register window a starter maps**: its pages, in window order, and where each structure
/// begins in it. Page `n` of [`pages`](Self::pages) is mapped at the window's base plus
/// `n * 4096`.
///
/// Name: provisional, minted 2026-11-11 (UTC) by lane/1062-virtio-direct-contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegisterWindow {
    pages: [RegisterPage; MAXIMUM_REGISTER_PAGES],
    count: usize,
    /// The common configuration's byte offset in the window.
    pub common: u16,
    /// The notify structure's byte offset in the window.
    pub notify: u16,
    /// The ISR byte's offset in the window.
    pub interrupt_status: u16,
    /// The device-specific configuration's byte offset in the window, if the device has one.
    pub device: Option<u16>,
}

impl RegisterWindow {
    /// The window's pages, in the order the starter maps them.
    ///
    /// Name: provisional, minted 2026-11-11 (UTC) by lane/1062-virtio-direct-contract.
    pub fn pages(&self) -> &[RegisterPage] {
        &self.pages[..self.count]
    }
}

/// **Why a register window could not be drawn.** Each one sends the function to the mediated
/// grant.
///
/// Name: provisional, minted 2026-11-11 (UTC) by lane/1062-virtio-direct-contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowRefusal {
    /// A structure is shorter than the specification allows: no byte, or a common configuration
    /// shorter than [`COMMON_CONFIGURATION_BYTES`].
    TooShort {
        /// The BAR the structure lies in.
        bar: u8,
        /// Its byte offset into that BAR.
        offset: u64,
    },
    /// A structure shares a page with the MSI-X table or the pending-bit array. Whoever can write
    /// the table can aim the function's interrupt message at any address.
    SharesTheInterruptPage {
        /// The BAR the page lies in.
        bar: u8,
        /// The first structure byte on that page.
        offset: u64,
    },
    /// A page a structure touches runs past the end of its BAR, or the structure names a BAR the
    /// function does not have. The rest of that page could hold another device's registers.
    BeyondBar {
        /// The BAR the structure names.
        bar: u8,
        /// The first structure byte on the page.
        offset: u64,
    },
    /// The structures touch more than [`MAXIMUM_REGISTER_PAGES`] distinct pages.
    TooManyPages,
}

/// **Which pages a driver granted direct is mapped**, and where each structure lands, or why no
/// window can be drawn.
///
/// Every page any of the four structures touches is mapped once, and nothing else of the BARs. A
/// page that also holds a `withheld` byte is never mapped, and the function is refused instead, as
/// it is when a page runs past its BAR, whose size `bar_bytes` gives by index (zero for a BAR the
/// function does not have).
/// The pages are sorted by BAR and offset, so a structure that spans two pages finds them next to
/// each other in the window.
///
/// # Examples
///
/// QEMU's `virtio-blk-pci` puts its four structures on four pages of BAR 4 and its MSI-X table in
/// BAR 1, so the window is those four pages in order:
///
/// ```
/// use driver_process_contract::virtual_input_output::{Structure, Structures, Withheld, register_window};
///
/// let s = |offset, length| Structure { bar: 4, offset, length };
/// let structures = Structures {
///     common: s(0x0000, 0x38),
///     interrupt_status: s(0x1000, 0x1),
///     device: Some(s(0x2000, 0x3c)),
///     notify: s(0x3000, 0x4),
/// };
/// let msix = [Withheld { bar: 1, start: 0, end: 0x20 }, Withheld { bar: 1, start: 0x800, end: 0x808 }];
/// let bar_bytes = [0, 0x1000, 0, 0, 0x4000, 0];
/// let window = register_window(&structures, &bar_bytes, &msix).unwrap();
/// assert_eq!(window.pages().len(), 4);
/// assert_eq!((window.common, window.interrupt_status, window.device, window.notify), (0, 0x1000, Some(0x2000), 0x3000));
/// ```
///
/// Name: provisional, minted 2026-11-11 (UTC) by lane/1062-virtio-direct-contract.
pub fn register_window(
    structures: &Structures,
    bar_bytes: &[u64; 6],
    withheld: &[Withheld],
) -> Result<RegisterWindow, WindowRefusal> {
    let all = [
        Some(structures.common),
        Some(structures.notify),
        Some(structures.interrupt_status),
        structures.device,
    ];
    let mut pages = [RegisterPage { bar: 0, offset: 0 }; MAXIMUM_REGISTER_PAGES];
    let mut count = 0;
    for (n, s) in all.iter().enumerate() {
        let Some(s) = s else { continue };
        let shortest = if n == 0 {
            COMMON_CONFIGURATION_BYTES
        } else {
            1
        };
        let Some(last) = (s.length >= shortest)
            .then(|| s.offset.checked_add(s.length - 1))
            .flatten()
        else {
            return Err(WindowRefusal::TooShort {
                bar: s.bar,
                offset: s.offset,
            });
        };
        // Every page the structure touches, not just its two ends, so a structure that runs across
        // a withheld page is refused too.
        for page in s.offset / PAGE_BYTES..=last / PAGE_BYTES {
            let (start, end) = (page * PAGE_BYTES, (page + 1) * PAGE_BYTES);
            if bar_bytes
                .get(usize::from(s.bar))
                .is_none_or(|&size| end > size)
            {
                return Err(WindowRefusal::BeyondBar {
                    bar: s.bar,
                    offset: s.offset.max(start),
                });
            }
            if withheld
                .iter()
                .any(|w| w.bar == s.bar && w.start < end && start < w.end)
            {
                return Err(WindowRefusal::SharesTheInterruptPage {
                    bar: s.bar,
                    offset: s.offset.max(start),
                });
            }
            let p = RegisterPage {
                bar: s.bar,
                offset: start,
            };
            // Kept sorted as it grows, so a structure that spans two pages finds them side by side.
            let at = pages[..count].iter().take_while(|q| **q < p).count();
            if pages.get(at).is_none_or(|q| at == count || *q != p) {
                if count == MAXIMUM_REGISTER_PAGES {
                    return Err(WindowRefusal::TooManyPages);
                }
                // Shift the tail up by one, last first, so the page lands at its sorted place.
                let mut k = count;
                while k > at {
                    pages[k] = pages[k - 1];
                    k -= 1;
                }
                pages[at] = p;
                count += 1;
            }
        }
    }
    // Where a structure begins in the window: the index of its first page, plus its offset in it.
    // Its first page is in the list, so the search always finds it, and the index is below
    // `MAXIMUM_REGISTER_PAGES`, so the offset fits a `u16` below `WINDOW_BYTES`.
    let at = |s: &Structure| -> u16 {
        let first = RegisterPage {
            bar: s.bar,
            offset: s.offset / PAGE_BYTES * PAGE_BYTES,
        };
        let index = pages[..count].iter().position(|p| *p == first).unwrap_or(0) as u64;
        (index * PAGE_BYTES + s.offset % PAGE_BYTES) as u16
    };
    Ok(RegisterWindow {
        pages,
        count,
        common: at(&structures.common),
        notify: at(&structures.notify),
        interrupt_status: at(&structures.interrupt_status),
        device: structures.device.as_ref().map(at),
    })
}

/// **Everything a direct driver is told at spawn that a window cannot say**: which of its roles to
/// run, where each structure begins in its register window, the notify multiplier, the MSI-X table
/// entry the kernel armed, and where its DMA region is physically. The region's length is its
/// window's, on the startup page. Every ring address the device reads is physical, and a
/// process knows only virtual ones; every IOMMU domain is an identity map
/// (`kernel/src/iommu.rs`), so the physical address is the bus address too.
///
/// Name: provisional, minted 2026-11-11 (UTC) by lane/1062-virtio-direct-contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HandOff {
    /// The common configuration's byte offset in the register window. Four-byte aligned.
    pub common: u16,
    /// The notify structure's byte offset in the register window. Two-byte aligned.
    pub notify: u16,
    /// The ISR byte's offset in the register window.
    pub interrupt_status: u16,
    /// The device-specific configuration's byte offset in the register window, four-byte aligned,
    /// if the device has one.
    pub device: Option<u16>,
    /// `notify_off_multiplier`: queue `q`'s doorbell is at `notify + queue_notify_off(q) *` this.
    /// Zero, or a power of two of at least 2 (virtio 1.2 section 4.1.4.4).
    pub notify_off_multiplier: u32,
    /// The MSI-X table entry the kernel armed, for `queue_msix_vector` and `config_msix_vector`,
    /// or `None` where the function delivers its legacy pin interrupt.
    pub interrupt_vector: Option<u16>,
    /// Which of the program's roles to run, in the program's own numbering: the number a mediated
    /// spawn of the same program passes as its first word.
    pub role: u16,
    /// The DMA region's physical base, page aligned and not zero.
    pub data_plane_physical_address: u64,
}

/// The largest MSI-X table entry index a function can have: a table holds at most 2048.
const LAST_INTERRUPT_TABLE_ENTRY: u16 = 2047;

/// The two packed words' fields share no bit (four `u16`s in word 0; a `u32` and two `u16`s in word
/// 1), which the compiler checks here. The constant-assertion lint is off on purpose: the assertion
/// is constant by construction, and the compiler checking it is its job.
///
/// Name: provisional, minted 2026-11-11 (UTC) by lane/1062-virtio-direct-contract.
#[allow(clippy::assertions_on_constants, clippy::eq_op)]
pub const THE_HAND_OFF_WORD_FIELDS_SHARE_NO_BIT: () =
    assert!(u32::MAX as u64 + ((u16::MAX as u64) << 32) + ((u16::MAX as u64) << 48) == u64::MAX);

impl HandOff {
    /// The three spawn words: the four offsets, then the multiplier, vector and role, then the
    /// physical base.
    ///
    /// Name: provisional, minted 2026-11-11 (UTC) by lane/1062-virtio-direct-contract.
    pub const fn pack(&self) -> [u64; 3] {
        let device = match self.device {
            Some(d) => d,
            None => NO_DEVICE_CONFIGURATION,
        };
        let vector = match self.interrupt_vector {
            Some(v) => v,
            None => NO_VECTOR,
        };
        [
            self.common as u64
                | (self.notify as u64) << 16
                | (self.interrupt_status as u64) << 32
                | (device as u64) << 48,
            self.notify_off_multiplier as u64 | (vector as u64) << 32 | (self.role as u64) << 48,
            self.data_plane_physical_address,
        ]
    }

    /// The three spawn words back, or `None` for words no kernel would have sent: an offset outside
    /// the register window or misaligned for its structure, a multiplier the specification does
    /// not allow, a vector past any MSI-X table, or a base that is zero or not page aligned.
    ///
    /// Name: provisional, minted 2026-11-11 (UTC) by lane/1062-virtio-direct-contract.
    pub fn unpack(w: [u64; 3]) -> Option<HandOff> {
        let field = |word: u64, at: u32| (word >> at) as u16;
        let (common, notify, interrupt_status, device) = (
            field(w[0], 0),
            field(w[0], 16),
            field(w[0], 32),
            field(w[0], 48),
        );
        let notify_off_multiplier = w[1] as u32;
        let (vector, role) = (field(w[1], 32), field(w[1], 48));
        let inside = |offset: u16| u64::from(offset) < WINDOW_BYTES;
        let device = match device {
            NO_DEVICE_CONFIGURATION => None,
            d if inside(d) && d.is_multiple_of(4) => Some(d),
            _ => return None,
        };
        let interrupt_vector = match vector {
            NO_VECTOR => None,
            v if v <= LAST_INTERRUPT_TABLE_ENTRY => Some(v),
            _ => return None,
        };
        let ok = inside(common)
            && common.is_multiple_of(4)
            && inside(notify)
            && notify.is_multiple_of(2)
            && inside(interrupt_status)
            && (notify_off_multiplier == 0
                || (notify_off_multiplier.is_power_of_two() && notify_off_multiplier >= 2))
            && w[2] != 0
            && w[2].is_multiple_of(PAGE_BYTES);
        ok.then_some(HandOff {
            common,
            notify,
            interrupt_status,
            device,
            notify_off_multiplier,
            interrupt_vector,
            role,
            data_plane_physical_address: w[2],
        })
    }

    /// **Whether every structure's first bytes lie inside a window of `pages` pages**: the whole
    /// common configuration, the ISR byte, and queue 0's two-byte doorbell. A driver checks it
    /// against the startup page before its first register access.
    ///
    /// Name: provisional, minted 2026-11-11 (UTC) by lane/1062-virtio-direct-contract.
    pub fn fits(&self, pages: u32) -> bool {
        let end = u64::from(pages) * PAGE_BYTES;
        u64::from(self.common) + COMMON_CONFIGURATION_BYTES <= end
            && u64::from(self.interrupt_status) < end
            && u64::from(self.notify) + 2 <= end
            && self.device.is_none_or(|d| u64::from(d) < end)
    }

    /// **Queue `q`'s doorbell, as a byte offset in the register window**, from the
    /// `queue_notify_off` the device reports for it (virtio 1.2 section 4.1.4.4).
    ///
    /// Name: provisional, minted 2026-11-11 (UTC) by lane/1062-virtio-direct-contract.
    pub const fn doorbell(&self, queue_notify_off: u16) -> u64 {
        self.notify as u64 + queue_notify_off as u64 * self.notify_off_multiplier as u64
    }
}

#[cfg(kani)]
mod proofs;

#[cfg(test)]
mod tests {
    use super::*;

    fn s(bar: u8, offset: u64, length: u64) -> Structure {
        Structure {
            bar,
            offset,
            length,
        }
    }

    /// QEMU's modern layout: four structures on four pages of BAR 4, the MSI-X table in BAR 1.
    fn qemu() -> Structures {
        Structures {
            common: s(4, 0x0000, 0x38),
            interrupt_status: s(4, 0x1000, 0x1),
            device: Some(s(4, 0x2000, 0x3c)),
            notify: s(4, 0x3000, 0x4),
        }
    }

    /// QEMU's BAR sizes for `virtio-blk-pci`: the MSI-X table's BAR 1, the structures' BAR 4.
    const QEMU_BARS: [u64; 6] = [0, 0x1000, 0, 0, 0x4000, 0];
    /// Every BAR large, so a test of another refusal never meets this one.
    const WIDE_BARS: [u64; 6] = [1 << 20; 6];

    fn qemu_interrupt_table_and_pending_bits() -> [Withheld; 2] {
        [
            Withheld {
                bar: 1,
                start: 0,
                end: 0x20,
            },
            Withheld {
                bar: 1,
                start: 0x800,
                end: 0x808,
            },
        ]
    }

    #[test]
    fn qemus_window_maps_the_four_structure_pages_and_nothing_else() {
        let window = register_window(
            &qemu(),
            &QEMU_BARS,
            &qemu_interrupt_table_and_pending_bits(),
        )
        .unwrap();
        let offsets: [u64; 4] = core::array::from_fn(|n| window.pages()[n].offset);
        assert_eq!(offsets, [0, 0x1000, 0x2000, 0x3000]);
        assert!(window.pages().iter().all(|p| p.bar == 4));
        assert_eq!(window.common, 0);
        assert_eq!(window.interrupt_status, 0x1000);
        assert_eq!(window.device, Some(0x2000));
        assert_eq!(window.notify, 0x3000);
    }

    /// **The refusal the default rests on**: a function whose structures share a page with its
    /// MSI-X table or pending-bit array cannot be granted direct, because mapping the structure
    /// would map the table.
    #[test]
    fn a_structure_on_the_interrupt_page_is_refused() {
        let table_on_the_interrupt_status_page = [Withheld {
            bar: 4,
            start: 0x1800,
            end: 0x1820,
        }];
        assert_eq!(
            register_window(&qemu(), &QEMU_BARS, &table_on_the_interrupt_status_page),
            Err(WindowRefusal::SharesTheInterruptPage {
                bar: 4,
                offset: 0x1000
            })
        );
        let pending_bits_on_the_notify_page = [Withheld {
            bar: 4,
            start: 0x3ff8,
            end: 0x4000,
        }];
        assert_eq!(
            register_window(&qemu(), &QEMU_BARS, &pending_bits_on_the_notify_page),
            Err(WindowRefusal::SharesTheInterruptPage {
                bar: 4,
                offset: 0x3000
            })
        );
        // The same range in another BAR withholds nothing here, and a range that ends where a
        // structure page starts is half open and leaves it mapped.
        let elsewhere = [
            Withheld {
                bar: 2,
                start: 0x1000,
                end: 0x2000,
            },
            Withheld {
                bar: 4,
                start: 0x4000,
                end: 0x4100,
            },
        ];
        assert!(register_window(&qemu(), &QEMU_BARS, &elsewhere).is_ok());
    }

    /// A structure that runs onto a withheld page is refused at its first byte on that page.
    #[test]
    fn a_structure_crossing_onto_a_withheld_page_is_refused() {
        let mut structures = qemu();
        structures.notify = s(4, 0x3f00, 0x200);
        let table = [Withheld {
            bar: 4,
            start: 0x4000,
            end: 0x4040,
        }];
        assert_eq!(
            register_window(&structures, &WIDE_BARS, &table),
            Err(WindowRefusal::SharesTheInterruptPage {
                bar: 4,
                offset: 0x4000
            })
        );
    }

    /// Two structures on one page map it once, a structure spanning two pages gets them side by
    /// side, and structures in two BARs sort by BAR.
    #[test]
    fn shared_and_spanning_pages_are_mapped_once_and_in_order() {
        let structures = Structures {
            common: s(2, 0x0000, 0x38),
            interrupt_status: s(2, 0x0040, 0x1),
            device: None,
            notify: s(0, 0x0f00, 0x400),
        };
        let window = register_window(&structures, &WIDE_BARS, &[]).unwrap();
        assert_eq!(
            window.pages(),
            &[
                RegisterPage { bar: 0, offset: 0 },
                RegisterPage {
                    bar: 0,
                    offset: 0x1000
                },
                RegisterPage { bar: 2, offset: 0 },
            ]
        );
        assert_eq!(window.notify, 0x0f00);
        assert_eq!(window.common, 0x2000);
        assert_eq!(window.interrupt_status, 0x2040);
        assert_eq!(window.device, None);
    }

    /// A page that runs past its BAR, or a BAR the function lacks, is never mapped: the rest of
    /// such a page could be another function's registers.
    #[test]
    fn a_page_past_the_end_of_its_bar_is_refused() {
        let small_bar = [0, 0x1000, 0, 0, 0x3800, 0];
        assert_eq!(
            register_window(&qemu(), &small_bar, &[]),
            Err(WindowRefusal::BeyondBar {
                bar: 4,
                offset: 0x3000
            })
        );
        let mut structures = qemu();
        structures.interrupt_status = s(6, 0, 1);
        assert_eq!(
            register_window(&structures, &WIDE_BARS, &[]),
            Err(WindowRefusal::BeyondBar { bar: 6, offset: 0 })
        );
        assert!(register_window(&qemu(), &QEMU_BARS, &[]).is_ok());
    }

    /// QEMU's capability list for a modern `virtio-blk-pci`, in the fake configuration space
    /// `crates/pci/src/tests.rs` builds: the four vendor capabilities, then MSI-X with its table at
    /// `table_offset` in `table_bar` and its pending bits at 0x800 of BAR 1.
    fn configuration_space(table_bar: u32, table_offset: u32) -> impl FnMut(Bdf, u64) -> u32 {
        let vendor =
            |next: u32, kind: u32, length: u32| 0x09 | next << 8 | length << 16 | kind << 24;
        let space: std::collections::HashMap<u64, u32> = [
            (0x04, 0x0010 << 16),
            (0x34, 0x40),
            (0x40, vendor(0x50, 1, 16)),
            (0x44, 4),
            (0x48, 0x0000),
            (0x4c, 0x1000),
            (0x50, vendor(0x60, 3, 16)),
            (0x54, 4),
            (0x58, 0x1000),
            (0x5c, 0x1000),
            (0x60, vendor(0x70, 4, 16)),
            (0x64, 4),
            (0x68, 0x2000),
            (0x6c, 0x1000),
            (0x70, vendor(0x84, 2, 20)),
            (0x74, 4),
            (0x78, 0x3000),
            (0x7c, 0x1000),
            (0x80, 4),
            // MSI-X, two entries (the field holds one less), end of list.
            (0x84, 0x11 | 1 << 16),
            (0x88, table_offset | table_bar),
            (0x8c, 0x800 | 1),
        ]
        .into_iter()
        .collect();
        move |_, offset| *space.get(&(offset & !3)).unwrap_or(&0)
    }

    const FUNCTION: Bdf = Bdf {
        bus: 0,
        dev: 3,
        func: 0,
    };

    /// A modern function's capability list reads as QEMU's layout, and its window maps the four
    /// structure pages.
    #[test]
    fn the_capability_list_reads_as_the_four_structures_and_the_interrupt_table_ranges() {
        let mut read = configuration_space(1, 0);
        let (structures, multiplier) = structures(FUNCTION, &mut read).unwrap();
        assert_eq!(structures, qemu_four_pages());
        assert_eq!(multiplier, 4);
        let withheld = withheld(FUNCTION, &mut read);
        assert_eq!(withheld, qemu_interrupt_table_of_two_entries());
        assert!(register_window(&structures, &QEMU_BARS, &withheld).is_ok());
    }

    /// **A function whose structures share a page with its MSI-X table gets the mediated grant**,
    /// read from the capability list as a starter reads it: the same function with its table moved
    /// into BAR 4, onto the ISR page, is refused.
    #[test]
    fn a_table_on_a_structure_page_refuses_the_function_from_its_capability_list() {
        let mut read = configuration_space(4, 0x1800);
        let (structures, _) = structures(FUNCTION, &mut read).unwrap();
        assert_eq!(
            register_window(&structures, &QEMU_BARS, &withheld(FUNCTION, &mut read)),
            Err(WindowRefusal::SharesTheInterruptPage {
                bar: 4,
                offset: 0x1000
            })
        );
    }

    /// A function without MSI-X withholds nothing, and one missing a structure has no window.
    #[test]
    fn a_list_without_an_interrupt_table_withholds_nothing_and_one_without_interrupt_status_names_no_structures()
     {
        let mut read = |_: Bdf, offset: u64| match offset {
            0x04 => 0x0010 << 16,
            0x34 => 0x40,
            0x40 => 0x09 | 16 << 16 | 1 << 24,
            0x44 => 4,
            0x4c => 0x1000,
            _ => 0,
        };
        assert!(
            withheld(FUNCTION, &mut read)
                .iter()
                .all(|w| w.start == w.end)
        );
        assert_eq!(structures(FUNCTION, &mut read), None);
    }

    fn qemu_four_pages() -> Structures {
        Structures {
            common: s(4, 0x0000, 0x1000),
            interrupt_status: s(4, 0x1000, 0x1000),
            device: Some(s(4, 0x2000, 0x1000)),
            notify: s(4, 0x3000, 0x1000),
        }
    }

    fn qemu_interrupt_table_of_two_entries() -> [Withheld; 2] {
        [
            Withheld {
                bar: 1,
                start: 0,
                end: 0x20,
            },
            Withheld {
                bar: 1,
                start: 0x800,
                end: 0x808,
            },
        ]
    }

    #[test]
    fn short_structures_and_too_many_pages_are_refused() {
        let mut structures = qemu();
        structures.common = s(4, 0, 0x37);
        assert_eq!(
            register_window(&structures, &WIDE_BARS, &[]),
            Err(WindowRefusal::TooShort { bar: 4, offset: 0 })
        );
        let mut structures = qemu();
        structures.interrupt_status = s(4, 0x1000, 0);
        assert_eq!(
            register_window(&structures, &WIDE_BARS, &[]),
            Err(WindowRefusal::TooShort {
                bar: 4,
                offset: 0x1000
            })
        );
        // A structure whose last byte would lie past the end of the address space.
        let mut structures = qemu();
        structures.notify = s(4, u64::MAX, 2);
        assert!(matches!(
            register_window(&structures, &WIDE_BARS, &[]),
            Err(WindowRefusal::TooShort { .. })
        ));
        // Nine pages of notify doorbells.
        let mut structures = qemu();
        structures.notify = s(4, 0x3000, 9 * 0x1000);
        assert_eq!(
            register_window(&structures, &WIDE_BARS, &[]),
            Err(WindowRefusal::TooManyPages)
        );
        // Exactly eight distinct pages fit.
        let mut structures = qemu();
        structures.notify = s(4, 0x3000, 5 * 0x1000);
        assert_eq!(
            register_window(&structures, &WIDE_BARS, &[])
                .unwrap()
                .pages()
                .len(),
            8
        );
    }

    fn hand_off() -> HandOff {
        HandOff {
            common: 0,
            notify: 0x3000,
            interrupt_status: 0x1000,
            device: Some(0x2000),
            notify_off_multiplier: 4,
            interrupt_vector: Some(0),
            role: 3,
            data_plane_physical_address: 0x4010_0000,
        }
    }

    #[test]
    fn the_hand_off_round_trips_and_refuses_what_no_kernel_sends() {
        let h = hand_off();
        assert_eq!(HandOff::unpack(h.pack()), Some(h));
        let pin_interrupt = HandOff {
            device: None,
            interrupt_vector: None,
            notify_off_multiplier: 0,
            ..h
        };
        assert_eq!(HandOff::unpack(pin_interrupt.pack()), Some(pin_interrupt));

        let refused = |edit: fn(&mut [u64; 3])| {
            let mut w = h.pack();
            edit(&mut w);
            HandOff::unpack(w)
        };
        // The common configuration past the window, and misaligned.
        assert_eq!(refused(|w| w[0] = w[0] & !0xffff | 0x8000), None);
        assert_eq!(refused(|w| w[0] |= 2), None);
        // The notify structure misaligned; the ISR byte past the window.
        assert_eq!(refused(|w| w[0] |= 1 << 16), None);
        assert_eq!(
            refused(|w| w[0] = w[0] & !(0xffff << 32) | 0x8000 << 32),
            None
        );
        // The device configuration misaligned.
        assert_eq!(refused(|w| w[0] |= 1 << 48), None);
        // A multiplier of one, or of three.
        assert_eq!(refused(|w| w[1] = w[1] & !0xffff_ffff | 1), None);
        assert_eq!(refused(|w| w[1] = w[1] & !0xffff_ffff | 3), None);
        // A vector past the largest table.
        assert_eq!(
            refused(|w| w[1] = w[1] & !(0xffff << 32) | 2048 << 32),
            None
        );
        // A base of zero or off a page.
        assert_eq!(refused(|w| w[2] = 0), None);
        assert_eq!(refused(|w| w[2] |= 0x800), None);
        // The edges that are admitted: the last table entry, the widest multiplier, the last
        // aligned offset in the window.
        assert!(refused(|w| w[1] = w[1] & !(0xffff << 32) | 2047 << 32).is_some());
        assert!(refused(|w| w[1] = w[1] & !0xffff_ffff | 1 << 31).is_some());
        assert!(refused(|w| w[0] = w[0] & !0xffff | 0x7ffc).is_some());
    }

    #[test]
    fn the_fields_land_in_their_own_bits() {
        let w = hand_off().pack();
        assert_eq!(w[0], 0x2000_u64 << 48 | 0x1000 << 32 | 0x3000 << 16);
        assert_eq!(w[1], 3_u64 << 48 | 4);
        assert_eq!(w[2], 0x4010_0000);
    }

    #[test]
    fn the_hand_off_fits_a_window_only_when_every_structure_does() {
        let h = hand_off();
        assert!(h.fits(4));
        assert!(!h.fits(3), "the notify structure is on page 3");
        let one_page = HandOff {
            common: 0,
            notify: 0xffe,
            interrupt_status: 0xfff,
            device: Some(0xffc),
            ..h
        };
        assert!(one_page.fits(1));
        assert!(
            !HandOff {
                notify: 0xfff,
                ..one_page
            }
            .fits(1)
        );
        assert!(
            !HandOff {
                common: 0xfcc,
                ..one_page
            }
            .fits(1)
        );
        assert!(
            HandOff {
                common: 0xfc8,
                ..one_page
            }
            .fits(1)
        );
        assert!(
            !HandOff {
                device: Some(0x1000),
                ..one_page
            }
            .fits(1)
        );
    }

    #[test]
    fn the_doorbell_is_the_notify_base_plus_the_scaled_queue_offset() {
        let h = hand_off();
        assert_eq!(h.doorbell(0), 0x3000);
        assert_eq!(h.doorbell(3), 0x300c);
        let shared = HandOff {
            notify_off_multiplier: 0,
            ..h
        };
        assert_eq!(shared.doorbell(3), 0x3000);
    }
}
