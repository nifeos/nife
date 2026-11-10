//! **How the progenitor endows `boot_self_test`, and what the program says back** (milestone 1063
//! (the boot self-test's spawning checks run from userspace)): the capability slots, the pair-page
//! addresses, the image table and the two messages the progenitor and the program agree on. A user
//! of an address wraps it in `address_space_map::pair_page`, so an address off the map stops that
//! build. See notes/self-test-checks.md, "The two halves".
//!
//! Name: provisional, minted 2026-11-10 (UTC) by lane
//! `milestone/1063-the-boot-self-test-spawns-from-userspace`, for the program it describes.

/// **The program's capability slots.** The budget and the report endpoint are always there; the
/// rest are empty when the machine has nothing for them, and the program probes them before it
/// retypes anything, since a retype lands in the first free slot.
///
/// Name: provisional (milestone 1063's lane, 2026-11-10 UTC).
pub mod slot {
    /// The budget the program builds every child from: `SPLIT`, `DESTROY` and `GRANT`.
    ///
    /// Name: provisional.
    pub const BUDGET: u64 = 0;
    /// `WRITE` on the progenitor's endpoint, for [`super::message`]. The same endpoint is the
    /// program's supervision endpoint, so its death reaches the progenitor there too.
    ///
    /// Name: provisional.
    pub const REPORT: u64 = 1;
    /// `WRITE` on the boot's entropy service, when it has one.
    ///
    /// Name: provisional.
    pub const ENTROPY: u64 = 2;
    /// `WRITE` on the JH7110 TRNG service the kernel wired, on a JH7110 whose service answered.
    ///
    /// Name: provisional.
    pub const TRNG: u64 = 3;
    /// Disk 0's `Virtio` transport, when this machine has a virtio disk.
    ///
    /// Name: provisional.
    pub const DISK_TRANSPORT: u64 = 4;
    /// Disk 0's interrupt.
    ///
    /// Name: provisional.
    pub const DISK_INTERRUPT: u64 = 5;
    /// Disk 0's DMA page, which records its own physical base in its last word.
    ///
    /// Name: provisional.
    pub const DISK_DMA: u64 = 6;
    /// `WRITE` on the program's own thread, which a bound notification needs, so a receive here
    /// can end at a timer's deadline.
    ///
    /// Name: provisional.
    pub const OWN_THREAD: u64 = 7;
}

/// Where the kernel-half page (`crate::kernel_half`) is mapped, read-only.
///
/// Name: provisional.
pub const KERNEL_HALF_VIRTUAL_ADDRESS: u64 = 0x0060_0000;

/// **Where the transcript page is mapped, read/write**: the program appends each line's bytes here
/// and never rewrites them, and the progenitor, which maps the same frame read-only, prints from
/// its last offset up to the one a [`message::LINE`] names.
///
/// Name: provisional.
pub const TRANSCRIPT_VIRTUAL_ADDRESS: u64 = 0x0061_0000;

/// The transcript's size: one page.
///
/// Name: provisional.
pub const TRANSCRIPT_BYTES: usize = 4096;

/// **Where the image table starts**: one page of [`header`] words, then each image in
/// [`image`] order, each starting on a fresh page.
///
/// Name: provisional.
pub const IMAGES_VIRTUAL_ADDRESS: u64 = 0x0100_0000;

/// The most bytes the images may take together, so they stay inside the pair pages.
///
/// Name: provisional.
pub const IMAGES_MAXIMUM_BYTES: u64 = 0x0700_0000;

/// **The programs the progenitor hands over as bytes**, by index into the image table. The program
/// cannot read the archive; the progenitor measured each before copying it.
///
/// Name: provisional (milestone 1063's lane, 2026-11-10 UTC).
pub mod image {
    /// `kernel_test_subject`: reads an address it is given and dies.
    ///
    /// Name: provisional.
    pub const KERNEL_TEST_SUBJECT: usize = 0;
    /// `memory_region_depleter`: maps from its budget until the budget is spent.
    ///
    /// Name: provisional.
    pub const MEMORY_REGION_DEPLETER: usize = 1;
    /// `block_driver`: reads `motd` off a virtio disk.
    ///
    /// Name: provisional.
    pub const BLOCK_DRIVER: usize = 2;
    /// How many there are.
    ///
    /// Name: provisional.
    pub const COUNT: usize = 3;
    /// Their archive names, in index order.
    ///
    /// Name: provisional.
    pub const NAMES: [&str; COUNT] = [
        "kernel_test_subject",
        "memory_region_depleter",
        "block_driver",
    ];
}

/// **The header page's words.**
///
/// Name: provisional (milestone 1063's lane, 2026-11-10 UTC).
pub mod header {
    /// Which [`super::entropy_source`] [`super::slot::ENTROPY`] serves.
    ///
    /// Name: provisional.
    pub const ENTROPY_SOURCE: usize = 0;
    /// Each image's length in bytes, in [`super::image`] order; 0 for one the archive lacked or
    /// the measurement refused.
    ///
    /// Name: provisional.
    pub const IMAGE_LENGTHS: usize = 1;
    /// The number of words in use.
    ///
    /// Name: provisional.
    pub const COUNT: usize = IMAGE_LENGTHS + super::image::COUNT;
}

/// **What the boot's entropy service draws from**, so the `entropy` check can say.
///
/// Name: provisional (milestone 1063's lane, 2026-11-10 UTC).
pub mod entropy_source {
    /// No entropy service.
    ///
    /// Name: provisional.
    pub const NONE: u64 = 0;
    /// The CPU's seed instruction, a service the kernel built.
    ///
    /// Name: provisional.
    pub const INSTRUCTION: u64 = 1;
    /// A virtio-rng device, a service the progenitor built.
    ///
    /// Name: provisional.
    pub const VIRTIO: u64 = 2;
}

/// The page size the table rounds to.
const PAGE: u64 = 4096;

/// **Where image `index` starts**, given every image's length: after the header page and every
/// earlier image, each rounded up to a page.
///
/// Name: provisional.
pub const fn image_address(lengths: &[u64; image::COUNT], index: usize) -> u64 {
    let mut va = IMAGES_VIRTUAL_ADDRESS + PAGE;
    let mut i = 0;
    while i < index {
        va += lengths[i].div_ceil(PAGE) * PAGE;
        i += 1;
    }
    va
}

/// Whether the images with these lengths fit [`IMAGES_MAXIMUM_BYTES`], header included.
///
/// Name: provisional.
pub const fn images_fit(lengths: &[u64; image::COUNT]) -> bool {
    image_address(lengths, image::COUNT) - IMAGES_VIRTUAL_ADDRESS <= IMAGES_MAXIMUM_BYTES
}

/// **The program's two messages on [`slot::REPORT`]**: three words, the code first and the
/// transcript's new end second. Neither is `abi::fault::EVENT_FAULT` or `EVENT_EXIT`, which arrive
/// on the same endpoint when the program dies.
///
/// Name: provisional (milestone 1063's lane, 2026-11-10 UTC).
pub mod message {
    /// The transcript grew; print it up to the second word.
    ///
    /// Name: provisional.
    pub const LINE: u64 = 0x5e1f_7e57_0000_0001;
    /// The verdict is in the transcript, up to the second word; the program exits next.
    ///
    /// Name: provisional.
    pub const DONE: u64 = 0x5e1f_7e57_0000_0002;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each image starts on its own page after the header, and the table's end is checked.
    #[test]
    fn images_follow_the_header_page_by_page() {
        let lengths = [1, 4096, 4097];
        assert_eq!(image_address(&lengths, 0), IMAGES_VIRTUAL_ADDRESS + PAGE);
        assert_eq!(
            image_address(&lengths, 1),
            IMAGES_VIRTUAL_ADDRESS + 2 * PAGE
        );
        assert_eq!(
            image_address(&lengths, 2),
            IMAGES_VIRTUAL_ADDRESS + 3 * PAGE
        );
        assert_eq!(
            image_address(&lengths, 3),
            IMAGES_VIRTUAL_ADDRESS + 5 * PAGE
        );
        assert!(images_fit(&lengths));
        assert!(!images_fit(&[IMAGES_MAXIMUM_BYTES, 0, 0]));
    }
}
