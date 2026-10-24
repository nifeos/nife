//! Where a boot virtio device's DMA region records its own physical base.
//!
//! Name: provisional (milestone 872's lane, 2026-10-11 UTC; §281 (the kernel's user-mode file
//! splits into child modules of `user`)).

use super::*;

/// Where [`boot_virtio_mmio_device`](super::device_starters), [`display_service`] and
/// [`keyboard_service`] write a DMA region's own physical base, in its first page's last eight
/// bytes (`abi::virtio::DMA_PHYS_OFFSET`). No driver's layout reaches there: entropy's rings and
/// pool end at 0x500, `net_stack`'s rings and frame buffers at 0xF00.
pub(crate) const VIRTIO_DMA_PHYS_OFFSET: u64 = abi::virtio::DMA_PHYS_OFFSET;
const _: () = assert!(VIRTIO_DMA_PHYS_OFFSET + 8 <= FRAME_SIZE);

/// Write the DMA region `dma`'s own physical base into its first page, at
/// [`VIRTIO_DMA_PHYS_OFFSET`].
/// The one place every boot virtio device's region learns where it is; see that constant.
pub(crate) fn write_dma_phys(dma: u64) {
    // SAFETY: `dma` is a fresh frame the caller just allocated, direct-mapped and owned by nobody
    // else yet, and `VIRTIO_DMA_PHYS_OFFSET + 8` is inside `FRAME_SIZE` (the assertion above), so
    // the write stays in the frame.
    unsafe {
        core::ptr::write_unaligned(
            (mmu::phys_to_virt(dma) as *mut u8)
                .add(VIRTIO_DMA_PHYS_OFFSET as usize)
                .cast::<u64>(),
            dma,
        );
    }
}
