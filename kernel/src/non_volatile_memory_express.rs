//! What the kernel does for an NVMe controller under §86 (whether an NVMe driver can leave the
//! kernel): find it, allocate its DMA run, and confine its requester id to that run before any
//! process can reach BAR0. Everything else about NVMe (reset, the admin rings, IDENTIFY, the I/O
//! queues, every command) runs in userspace: the admin process and the data plane, both
//! `components/src/non_volatile_memory_express.rs`. See notes/non-volatile-memory-express.md.
//!
//! Name: ratified 2026-09-17 (calef, DECISIONS §154), performed 2026-09-18. Introduced 2026-08-15
//! with milestone 53's NVMe block driver as `nvme.rs`, and it takes its crate's name because that
//! is what the pairing means: `pci` and `virtio` already sit this way. The argument and the
//! refusals are recorded once, beside the crate, in `crates/non_volatile_memory_express`.

use driver_process_contract::non_volatile_memory_express::{
    BringUpError, DIRECT_MEMORY_ACCESS_PAGES,
};

/// One transfer unit: a filesystem block, the same unit the blk-IPC protocol moves, so the EL0
/// block server serves the FS server without a translation layer.
pub const BLOCK_SIZE: usize = filesystem_contract::blk::BLOCK_SIZE;

/// The EL0 server's client windows (milestone 834 (ioping on nife and Linux)), named here for the
/// NVMe system tests.
#[cfg(feature = "system_tests")]
pub use ::driver_process_contract::non_volatile_memory_express::{
    CLIENT_WINDOWS, DISK_TREE_WINDOW, FILESYSTEM_WINDOW, INSTALLER_WINDOW,
};

/// A frame outside the DMA run that [`find_and_confine`] adds to the controller's IOMMU domain;
/// zero in every shipped boot and passing test.
///
/// Only the replayable falsification
/// (`system_tests/falsifications/...a_confined_el0_server_cannot_dma_outside_its_region.patch`)
/// stores one, so an EL0 server's PRP at it lands instead of faulting, per milestone 202 (every
/// confinement test is a ritual until somebody breaks the confinement and watches it fail).
#[cfg(feature = "system_tests")]
pub(crate) static ESCAPE_TARGET: core::sync::atomic::AtomicU64 =
    core::sync::atomic::AtomicU64::new(0);

/// **A controller found and confined, not yet touched**: where its registers are, who it is to
/// the IOMMU, and the DMA run it may reach. Nothing here has reset or enabled it.
pub struct Found {
    /// BAR0's physical base: the register page, then from `0x1000` the doorbell page.
    pub bar0: u64,
    /// The controller's PCIe requester id, which is what an IOMMU unit owns or does not.
    pub rid: u32,
    /// The physical base of the zeroed [`DIRECT_MEMORY_ACCESS_PAGES`]-page run the controller is
    /// confined to.
    pub dma: u64,
}

/// Why there is no NVMe server: none on the bus, or one the admin process refused, two facts a
/// test must not report alike.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Absent {
    /// No function on the bus carries the NVMe class code.
    NoController,
    /// A controller is there and failed bring-up at the named phase.
    Refused {
        /// The controller's PCIe requester id.
        rid: u32,
        /// The phase, as the admin process reported it.
        why: BringUpError,
    },
}

/// Find the machine's NVMe controller, allocate its DMA run, and confine it.
///
/// Returns [`Absent::NoController`] with none on the bus. The controller is confined to its run
/// before this returns, and so before any process holds BAR0; with no IOMMU nothing bounds it, and
/// nife trusts the driver (§86's ruling).
///
/// Name: provisional, minted 2026-11-10 (UTC) by lane/1062-nvme-admin-plane-in-userspace.
pub fn find_and_confine() -> Result<Found, Absent> {
    let dev = crate::pci::find_nvme_device().ok_or(Absent::NoController)?;
    // Zeroing is load-bearing for the completion rings: the phase discipline starts from
    // all-zero entries.
    let dma = crate::memory::alloc_contiguous_zeroed(DIRECT_MEMORY_ACCESS_PAGES as usize)
        .expect("no DMA region for the NVMe driver")
        .addr();
    if crate::iommu::is_active() {
        // The device's own DMA run, and in a shipped boot nothing else: the controller faults on
        // any address outside it.
        let own = paging::domain::DmaRegion {
            base: dma,
            size: DIRECT_MEMORY_ACCESS_PAGES * page_frames::FRAME_SIZE,
            writable: true,
        };
        // The falsification's widening point; `None` unless the patch set [`ESCAPE_TARGET`].
        #[cfg(feature = "system_tests")]
        let extra = {
            let escape = ESCAPE_TARGET.load(core::sync::atomic::Ordering::Relaxed);
            (escape != 0).then_some(paging::domain::DmaRegion {
                base: escape,
                size: page_frames::FRAME_SIZE,
                writable: true,
            })
        };
        #[cfg(not(feature = "system_tests"))]
        let extra: Option<paging::domain::DmaRegion> = None;
        match extra {
            Some(x) => crate::iommu::confine(dev.rid, &[own, x]),
            None => crate::iommu::confine(dev.rid, &[own]),
        }
    }
    Ok(Found {
        bar0: dev.bar0,
        rid: dev.rid,
        dma,
    })
}

/// Hand a retired controller's DMA run back.
///
/// The caller has had the admin process take the controller down
/// ([`driver_process_contract::non_volatile_memory_express::RETIRE`]) and has outlived every
/// process that mapped the run, because freeing under an enabled controller is use-after-free by
/// hardware (notes/dma.md). The IOMMU domain still names the run until the next
/// [`find_and_confine`], reachable only by a controller that is off.
///
/// Name: provisional, minted 2026-11-10 (UTC) by lane/1062-nvme-admin-plane-in-userspace.
#[cfg(feature = "system_tests")]
pub fn free_retired_run(dma: u64) {
    for page in 0..DIRECT_MEMORY_ACCESS_PAGES {
        crate::memory::free(page_frames::PageFrame::from_addr(
            dma + page * page_frames::FRAME_SIZE,
        ));
    }
}
