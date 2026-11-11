//! **Which grant a starter gives a virtio driver, and the direct grant's carve-out.** A driver
//! granted mediated holds a `Virtio` capability and the kernel validates every descriptor on
//! `NOTIFY` (`kernel/src/virtio.rs`). A driver granted direct holds its function's structure
//! pages and a DMA region the IOMMU confines, and the kernel validates nothing. See
//! design/roadmap/1062-the-device-control-planes-leave-the-kernel.md, "Step 7".
//!
//! The default, ruled 2026-11-11 (UTC), lives here and nowhere in the kernel's device code:
//! [`choose`] grants direct to a virtio-pci function the IOMMU confines, which takes all three of
//! the active IOMMU owning its requester id, the function offering `VIRTIO_F_ACCESS_PLATFORM`,
//! and no structure page sharing a page with its MSI-X table or pending-bit array. Every other
//! function, and every virtio-mmio device, gets the mediated grant.
//!
//! # BUGS
//!
//! - A direct grant's DMA region and domain live as long as the function, so a function granted
//!   direct twice in one boot holds two regions; the second confinement drops the first from the
//!   domain, and the first is never freed.
//! - The direct grant has no escape test yet. Step 7 lists two: a direct driver that aims a ring
//!   outside its region, and one that declines `VIRTIO_F_ACCESS_PLATFORM`, each answered by an
//!   IOMMU fault or a refused `FEATURES_OK`. Until they land, the suite shows the direct disk
//!   working and the default choosing it, and the IOMMU's confinement of a direct driver rests on
//!   `the_iommu_faults_a_dma_that_escapes_the_domain`, which provokes the same fault through the
//!   mediated transport.
//!
//! Name: provisional, minted 2026-11-11 (UTC) by lane/1062-virtio-direct-starter, after what it
//! decides.

use driver_process_contract::virtual_input_output::{
    self as contract, HandOff, RegisterWindow, WindowRefusal,
};

use super::*;
use crate::pci::PciVirtioDevice;

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
    /// The spawn words, [`HandOff::pack`]'s.
    pub words: [u64; 3],
    /// The DMA region's physical base.
    pub data_plane_physical_address: u64,
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
/// in the step 7 order: confine the function to a fresh zeroed region before any queue exists,
/// map the window's pages and nothing else of its BARs, and pack the handoff. `None` when no
/// contiguous run of `pages` is free. The region is never returned to the allocator, because the
/// domain maps it for as long as the function lives.
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
    let dma = crate::memory::alloc_contiguous_zeroed(pages)?.addr();
    crate::iommu::confine(
        d.rid,
        &[paging::domain::DmaRegion {
            base: dma,
            size: pages as u64 * FRAME_SIZE,
            writable: true,
        }],
    );
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
        data_plane_physical_address: dma,
    })
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
        assert_eq!(maps[registers].phys, direct.data_plane_physical_address);
        assert_eq!(maps[registers + 1].va, DMA_VA + FRAME_SIZE);
        assert_eq!(direct.windows[0].pages as usize, registers);
        assert_eq!(direct.windows[1].pages, 2);

        let h = HandOff::unpack(direct.words).expect("the handoff a starter packs unpacks");
        assert_eq!(
            (h.common, h.notify, h.interrupt_status),
            (window.common, window.notify, window.interrupt_status)
        );
        assert_eq!(
            h.data_plane_physical_address,
            direct.data_plane_physical_address
        );
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
