//! **Which grant a starter gives a virtio driver, and the direct grant's carve-out.** Mediated:
//! a `Virtio` capability, and the kernel validates every descriptor on `NOTIFY`. Direct: the
//! function's structure pages and a DMA region the IOMMU confines. See
//! design/roadmap/1062-the-device-control-planes-leave-the-kernel.md, "Step 7".
//!
//! The default, ruled 2026-11-11 (UTC), lives here: [`choose`] grants direct only when the active
//! IOMMU owns the function's requester id, the function offers `VIRTIO_F_ACCESS_PLATFORM`, and no
//! structure page shares a page with its MSI-X table or pending-bit array. Every other function,
//! and every virtio-mmio device, gets the mediated grant.
//!
//! # BUGS
//!
//! - A function granted direct again while its first driver still runs shares its region with
//!   that driver, and the reset stops the device under it. The starters grant a function again
//!   only after its driver has reported.
//! - No escape test drives a direct driver (a ring aimed outside its region, a declined
//!   `VIRTIO_F_ACCESS_PLATFORM`); `the_iommu_faults_a_dma_that_escapes_the_domain` provokes the
//!   same IOMMU fault through the mediated transport.
//!
//! Name: provisional, minted 2026-11-11 (UTC) by lane/1062-virtio-direct-starter.

use driver_process_contract::virtual_input_output::{
    self as contract, HandOff, RegisterWindow, WindowRefusal,
};

use super::*;
use crate::pci::PciVirtioDevice;
use crate::sync::{InterruptSafeMutex, rank};

/// Where a direct driver's register window starts. The driver reads it from the startup page.
const REGISTERS_VA: u64 = address_space_map::pair_page(0x0000_0000_0100_0000);

/// Where a direct driver's DMA region starts. The driver reads it from the startup page.
const DMA_VA: u64 = address_space_map::pair_page(0x0000_0000_0200_0000);

/// The most DMA pages a direct grant carries, so the mappings fit a fixed array.
///
/// Name: provisional, minted 2026-11-11 (UTC) by lane/1062-virtio-direct-starter.
pub(crate) const MAXIMUM_DATA_PLANE_PAGES: usize = 16;

/// `VIRTIO_F_ACCESS_PLATFORM`, feature bit 33: bit 1 of the device's second feature word.
const ACCESS_PLATFORM_HIGH: u32 = 1 << 1;
/// The common configuration's feature select and feature words (virtio 1.2 section 4.1.4.3).
const DEVICE_FEATURE_SELECT: u64 = 0x00;
const DEVICE_FEATURE: u64 = 0x04;
/// The common configuration's device status byte (virtio 1.2 section 4.1.4.3).
const DEVICE_STATUS: u64 = 0x14;
/// How many times [`reset`] reads the status byte waiting for the reset to land.
const RESET_POLLS: u32 = 100_000;
/// How many functions [`carve`] remembers a region for: more than any runner attaches.
const MAXIMUM_CARVED_FUNCTIONS: usize = 8;

/// **Each function's DMA region**, by requester id, with its page count. `iommu::confine` runs
/// once per function per boot (a second attach leaks the first domain's tables), so a function
/// granted direct again gets its first region back, reset and zeroed.
static CARVED: InterruptSafeMutex<[Option<(u32, u64, usize)>; MAXIMUM_CARVED_FUNCTIONS]> =
    InterruptSafeMutex::new(rank::VIRTIO, [None; MAXIMUM_CARVED_FUNCTIONS]);

/// **Why a function gets the mediated grant.**
///
/// Name: provisional, minted 2026-11-11 (UTC) by lane/1062-virtio-direct-starter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mediated {
    /// No IOMMU this kernel brought up owns the function's requester id, so nothing would confine
    /// a driver that held its registers.
    Unconfined,
    /// The function does not offer `VIRTIO_F_ACCESS_PLATFORM`, so it may treat a driver's
    /// addresses as untranslated (virtio 1.2 section 6).
    NoAccessPlatform,
    /// No register window could be drawn, most often because a structure shares a page with the
    /// MSI-X table.
    Window(WindowRefusal),
}

/// **The default, as a function of its three conditions.** Direct only when all three hold; the
/// first that fails names the reason. Kept apart from [`choose`] so its tests need no machine.
fn decide(
    confining: bool,
    offers_access_platform: bool,
    window: Result<RegisterWindow, WindowRefusal>,
) -> Result<RegisterWindow, Mediated> {
    if !confining {
        return Err(Mediated::Unconfined);
    }
    if !offers_access_platform {
        return Err(Mediated::NoAccessPlatform);
    }
    window.map_err(Mediated::Window)
}

/// **Which grant `d` gets**: `Ok` with the register window for a direct grant, `Err` with the
/// reason for a mediated one. Reads the function's second feature word, which selects nothing the
/// device acts on.
///
/// Name: provisional, minted 2026-11-11 (UTC) by lane/1062-virtio-direct-starter.
pub(crate) fn choose(d: &PciVirtioDevice) -> Result<RegisterWindow, Mediated> {
    let confining = crate::iommu::is_active() && crate::iommu::scope_of(d.rid).is_confining();
    let common = mmu::phys_to_virt(d.common);
    // SAFETY: `common` is the common configuration `pci::bring_up` resolved inside a placed BAR,
    // which `mmu::map_everything` maps as device memory; both registers lie in its first 0x38 bytes.
    let high = unsafe {
        core::ptr::write_volatile((common + DEVICE_FEATURE_SELECT) as *mut u32, 1);
        core::ptr::read_volatile((common + DEVICE_FEATURE) as *const u32)
    };
    decide(
        confining,
        high & ACCESS_PLATFORM_HIGH != 0,
        contract::register_window(
            &d.direct.structures,
            &d.direct.bar_bytes,
            &d.direct.withheld,
        ),
    )
}

/// **A direct grant, carved out**: the mappings, the two startup page windows and the three spawn
/// words, for a spawn closure to hand to `run`.
///
/// Name: provisional, minted 2026-11-11 (UTC) by lane/1062-virtio-direct-starter.
#[derive(Clone, Copy)]
pub(crate) struct Direct {
    maps: [Mapping; contract::MAXIMUM_REGISTER_PAGES + MAXIMUM_DATA_PLANE_PAGES],
    count: usize,
    /// The register window, then the DMA region.
    pub windows: [startup_page_contract::Window; 2],
    /// The spawn words, [`HandOff::pack`]'s, the DMA region's physical base among them.
    pub words: [u64; 3],
}

impl Direct {
    /// Every page the driver is mapped beyond its image: the register pages, device memory, then
    /// the DMA region, normal memory.
    ///
    /// Name: provisional, minted 2026-11-11 (UTC) by lane/1062-virtio-direct-starter.
    pub(crate) fn maps(&self) -> &[Mapping] {
        &self.maps[..self.count]
    }
}

/// **Carve out a direct grant of `d`** for a driver that runs `role`, with `pages` pages of DMA,
/// in the step 7 order: confine the function to a zeroed region before any queue exists, map the
/// window's pages and nothing else of its BARs, and pack the handoff. `None` when no contiguous run
/// of `pages` is free, or when the IOMMU refuses to attach the function. The region is never returned to the allocator, because the domain maps it
/// for as long as the function lives; a function granted again gets the same region, after
/// [`reset`] has stopped the device and the region is zeroed.
///
/// Name: provisional, minted 2026-11-11 (UTC) by lane/1062-virtio-direct-starter.
pub(crate) fn carve(
    d: &PciVirtioDevice,
    window: &RegisterWindow,
    role: u16,
    pages: usize,
) -> Option<Direct> {
    assert!(
        (1..=MAXIMUM_DATA_PLANE_PAGES).contains(&pages),
        "a direct grant of {pages} DMA pages"
    );
    let carved = CARVED
        .lock()
        .iter()
        .flatten()
        .find(|(rid, ..)| *rid == d.rid)
        .copied();
    let dma = match carved {
        Some((_, dma, carved_pages)) if pages <= carved_pages => {
            reset(d);
            // SAFETY: `dma` is this function's region of `carved_pages` frames, which no other
            // owner holds, and the reset has stopped the device writing it.
            unsafe {
                core::ptr::write_bytes(
                    mmu::phys_to_virt(dma) as *mut u8,
                    0,
                    carved_pages * FRAME_SIZE as usize,
                );
            }
            dma
        }
        _ => {
            let dma = crate::memory::alloc_contiguous_zeroed(pages)?.addr();
            let confined = crate::iommu::confine(
                d.rid,
                &[paging::domain::DmaRegion {
                    base: dma,
                    size: pages as u64 * FRAME_SIZE,
                    writable: true,
                }],
            );
            if confined.is_err() {
                // The device stays blocked, and the region goes back: no domain maps it.
                for k in 0..pages as u64 {
                    crate::memory::free(page_frames::PageFrame::from_addr(dma + k * FRAME_SIZE));
                }
                return None;
            }
            let mut table = CARVED.lock();
            let slot = table
                .iter()
                .position(|c| c.is_some_and(|(rid, ..)| rid == d.rid))
                .or_else(|| table.iter().position(Option::is_none));
            if let Some(slot) = slot {
                table[slot] = Some((d.rid, dma, pages));
            }
            dma
        }
    };
    let mut maps = [Mapping {
        va: 0,
        phys: 0,
        flags: Flags::user_data(),
    }; contract::MAXIMUM_REGISTER_PAGES + MAXIMUM_DATA_PLANE_PAGES];
    let registers = window.pages();
    for (n, page) in registers.iter().enumerate() {
        maps[n] = Mapping {
            va: REGISTERS_VA + n as u64 * FRAME_SIZE,
            phys: d.direct.bar_bases[usize::from(page.bar)] + page.offset,
            flags: Flags::user_device(),
        };
    }
    for k in 0..pages {
        maps[registers.len() + k] = Mapping {
            va: DMA_VA + k as u64 * FRAME_SIZE,
            phys: dma + k as u64 * FRAME_SIZE,
            flags: Flags::user_data(),
        };
    }
    let handoff = HandOff {
        common: window.common,
        notify: window.notify,
        interrupt_status: window.interrupt_status,
        device: window.device,
        notify_off_multiplier: d.notify_mult,
        interrupt_vector: (d.msix_vector != pci::VIRTIO_MSIX_NO_VECTOR).then_some(d.msix_vector),
        role,
        data_plane_physical_address: dma,
    };
    Some(Direct {
        maps,
        count: registers.len() + pages,
        windows: [
            startup_page_contract::Window {
                kind: contract::REGISTER_KIND,
                virtual_address: REGISTERS_VA,
                pages: registers.len() as u32,
            },
            startup_page_contract::Window {
                kind: contract::DATA_PLANE_KIND,
                virtual_address: DMA_VA,
                pages: pages as u32,
            },
        ],
        words: handoff.pack(),
    })
}

/// **Reset `d`** before its region goes to a new driver: write zero to the device status and wait
/// for it to read back zero, as `virtio::unregister` does for a mediated function, so the device
/// holds no queue the last driver programmed and stops writing the region.
fn reset(d: &PciVirtioDevice) {
    let status = (mmu::phys_to_virt(d.common) + DEVICE_STATUS) as *mut u8;
    // SAFETY: as in `choose`: the common configuration is mapped device memory, and the status
    // byte lies in its first 0x38 bytes.
    unsafe { core::ptr::write_volatile(status, 0) };
    // Bounded: a device that never reads back zero is as quiet as it will get.
    for _ in 0..RESET_POLLS {
        // SAFETY: as above.
        if unsafe { core::ptr::read_volatile(status) } == 0 {
            break;
        }
        core::hint::spin_loop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn some_window() -> Result<RegisterWindow, WindowRefusal> {
        let s = |offset| contract::Structure {
            bar: 4,
            offset,
            length: 0x1000,
        };
        contract::register_window(
            &contract::Structures {
                common: s(0),
                notify: s(0x3000),
                interrupt_status: s(0x1000),
                device: None,
            },
            &[0, 0, 0, 0, 0x4000, 0],
            &[],
        )
    }

    /// **The default's three conditions**: direct only when all three hold, and each one alone
    /// sends the function to the mediated grant.
    #[test_case]
    fn the_default_grants_direct_only_when_all_three_conditions_hold() {
        assert!(decide(true, true, some_window()).is_ok());
        assert_eq!(
            decide(false, true, some_window()).err(),
            Some(Mediated::Unconfined)
        );
        assert_eq!(
            decide(true, false, some_window()).err(),
            Some(Mediated::NoAccessPlatform)
        );
        let refusal = WindowRefusal::SharesTheInterruptPage {
            bar: 4,
            offset: 0x1000,
        };
        assert_eq!(
            decide(true, true, Err(refusal)).err(),
            Some(Mediated::Window(refusal))
        );
    }

    /// **The carve-out maps the window and the region and nothing else**, on the machine's PCI
    /// disk: every register mapping is a page of a structure the capability list names, the DMA
    /// mappings are the region, the windows list exactly them, and the handoff unpacks to the
    /// window's offsets and the region's base.
    #[test_case]
    fn a_direct_grant_maps_the_structure_pages_and_its_region_only() {
        let Some(d) = crate::pci::find_block_device() else {
            crate::testing::skip!("no PCIe disk on the bus, so there is nothing to grant");
        };
        let window = match choose(&d) {
            Ok(w) => w,
            Err(Mediated::Unconfined) if !crate::iommu::is_active() => {
                crate::testing::skip!("no IOMMU on this machine, so every grant is mediated");
            }
            Err(why) => {
                panic!("the runners' PCI disk is confined, yet the starter chose mediated: {why:?}")
            }
        };
        let direct = carve(&d, &window, 3, 2).expect("two pages for the DMA region");
        let maps = direct.maps();
        let registers = window.pages().len();
        assert_eq!(maps.len(), registers + 2);
        for (n, page) in window.pages().iter().enumerate() {
            assert_eq!(maps[n].va, REGISTERS_VA + n as u64 * FRAME_SIZE);
            assert_eq!(
                maps[n].phys,
                d.direct.bar_bases[usize::from(page.bar)] + page.offset
            );
        }
        assert_eq!(maps[registers + 1].va, DMA_VA + FRAME_SIZE);
        assert_eq!(direct.windows[0].pages as usize, registers);
        assert_eq!(direct.windows[1].pages, 2);

        let h = HandOff::unpack(direct.words).expect("the handoff a starter packs unpacks");
        assert_eq!(
            (h.common, h.notify, h.interrupt_status),
            (window.common, window.notify, window.interrupt_status)
        );
        assert_eq!(h.data_plane_physical_address, maps[registers].phys);
        assert_eq!(h.role, 3);
        assert!(h.fits(direct.windows[0].pages));
        // The common configuration the driver will find at its window offset is the one the
        // mediated transport drives.
        let page = window.pages()[usize::from(h.common) / FRAME_SIZE as usize];
        assert_eq!(
            d.direct.bar_bases[usize::from(page.bar)]
                + page.offset
                + u64::from(h.common) % FRAME_SIZE,
            d.common
        );
    }
}
