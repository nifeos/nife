//! What the kernel and the USB keyboard driver (`components/src/usb_keyboard_driver.rs`) agree on
//! about one xHCI controller: which pages of its register file the driver is mapped
//! ([`register_window`]), the three spawn words ([`Handoff`]), and how many pages of the DMA region
//! are the driver's ([`WORK_PAGES`], [`region_pages`]). The register and ring arithmetic is
//! `extensible_host_controller_interface`'s; this is nife's process plan on top of it, which the
//! 2026-10-25 names ruling moved out of that crate so it carries only the xHCI specification.
//!
//! Name: provisional, minted 2026-10-26 (UTC) by lane xhci-names-ratified, after the crate it
//! serves, as this crate's other modules are named.

use ::extensible_host_controller_interface::{Capabilities, PAGE_BYTES};

/// **How many pages of the register file a driver can be mapped**: the bits of [`Handoff`]'s
/// register mask. 64 KiB, which is the BAR of every Intel xHCI this project has read about; a
/// larger BAR is mapped this far and no further, and [`register_window`] refuses a controller
/// whose needed registers lie beyond it.
///
/// Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).
pub const MAX_REGISTER_PAGES: u64 = 16;

/// How many pages of the DMA region the driver is mapped: every page of
/// `extensible_host_controller_interface::dma`'s layout, through its scratchpad array. Scratchpad
/// page `n` is at page `WORK_PAGES + n`, and is not mapped.
///
/// Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).
pub const WORK_PAGES: u64 = 11;

/// The region's length in pages for a controller asking for `scratchpads` pages.
///
/// Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).
pub const fn region_pages(scratchpads: u16) -> u64 {
    WORK_PAGES + scratchpads as u64
}

/// The bytes of one interrupter register set (xHCI 1.2 section 5.5.2).
const INTERRUPTER_BYTES: u64 = 0x20;

/// **The byte ranges a driver must reach, inclusive**: the capability and operational registers
/// through the last port's status set, interrupter 0, and every slot's doorbell. What
/// [`register_window`] checks the mapped pages against.
const fn needed_ranges(capabilities: &Capabilities) -> [(u64, u64); 3] {
    [
        (0, capabilities.portsc(capabilities.max_ports) + 0x0f),
        (
            capabilities.interrupter(0),
            capabilities.interrupter(0) + INTERRUPTER_BYTES - 1,
        ),
        (
            capabilities.doorbell(0),
            capabilities.doorbell(capabilities.max_slots) + 3,
        ),
    ]
}

/// **Everything the driver is told at spawn**, and the whole of what it could not discover for
/// itself: where its DMA region is physically (a process knows only virtual addresses, and every
/// pointer the controller reads is physical), how many pages it is, and which register pages it
/// was mapped.
///
/// Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Handoff {
    /// The DMA region's physical base. Page aligned.
    pub dma_phys: u64,
    /// The region's length in pages: [`WORK_PAGES`] plus the controller's scratchpads.
    pub dma_pages: u32,
    /// The register pages mapped, [`register_window`]'s mask.
    pub registers: u16,
}

impl Handoff {
    /// The three spawn words.
    ///
    /// Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).
    pub const fn pack(&self) -> [u64; 3] {
        [
            self.dma_phys,
            self.dma_pages as u64 | (self.registers as u64) << 32,
            0,
        ]
    }

    /// The three spawn words back, or `None` for words no kernel would have sent: an unaligned or
    /// zero base, a region too small for the layout, or a register mask without page 0 (which
    /// holds the capability registers the driver reads first).
    ///
    /// Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).
    pub fn unpack(w: [u64; 3]) -> Option<Handoff> {
        let dma_pages = u32::try_from(w[1] & 0xffff_ffff).ok()?;
        let registers = u16::try_from(w[1] >> 32).ok()?;
        let ok = w[0] != 0
            && w[0].is_multiple_of(PAGE_BYTES)
            && u64::from(dma_pages) >= WORK_PAGES
            && registers & 1 != 0
            && w[2] == 0;
        ok.then_some(Handoff {
            dma_phys: w[0],
            dma_pages,
            registers,
        })
    }
}

/// **Why a register window could not be drawn.**
///
/// Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowRefusal {
    /// A register the driver must reach (named by its byte offset) is past
    /// [`MAX_REGISTER_PAGES`] or past the end of the BAR.
    BeyondWindow {
        /// The register's byte offset into the BAR.
        offset: u64,
    },
    /// A register the driver must reach shares a page with the MSI-X table or pending-bit array,
    /// which the driver must never be mapped: whoever can write the table can aim the device's
    /// interrupt message anywhere.
    SharesTheInterruptPage {
        /// The register's byte offset into the BAR.
        offset: u64,
    },
}

/// **Which pages of the register file a confined driver is mapped**, as a mask (bit `n` is the
/// page at byte `n * 4096`), or why none can be.
///
/// Every page of the BAR up to [`MAX_REGISTER_PAGES`] is mapped except any page that overlaps a
/// `withheld` byte range, where the kernel passes the MSI-X table and pending-bit array. A page the
/// driver needs ([`Capabilities`]' registers through the last `PORTSC`, interrupter 0, every
/// doorbell) that is withheld or out of reach is refused.
///
/// # Examples
///
/// QEMU's `qemu-xhci` puts its MSI-X table in the fourth page of its 16 KiB register file, so a
/// driver is mapped the first three and never the fourth:
///
/// ```
/// use extensible_host_controller_interface::Capabilities;
/// use driver_process_contract::extensible_host_controller_interface::register_window;
///
/// // qemu-xhci's capability registers: 0x40 bytes of them, 64 slots, 4 ports, runtime at 0x1000,
/// // doorbells at 0x2000.
/// let capabilities = Capabilities::decode(0x0100_0040, 0x0400_0840, 0, 0x0000_7000, 0x2000, 0x1000);
/// let window = register_window(&capabilities, 0x4000, &[(0x3000, 0x3000 + 16 * 16), (0x3800, 0x3808)]);
/// assert_eq!(window, Ok(0b0111));
/// ```
///
/// Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).
pub fn register_window(
    capabilities: &Capabilities,
    bar_bytes: u64,
    withheld: &[(u64, u64)],
) -> Result<u64, WindowRefusal> {
    let pages = (bar_bytes / PAGE_BYTES).min(MAX_REGISTER_PAGES);
    let mut mask = 0u64;
    for page in 0..pages {
        let (start, end) = (page * PAGE_BYTES, (page + 1) * PAGE_BYTES);
        if !withheld.iter().any(|&(a, b)| a < end && start < b) {
            mask |= 1 << page;
        }
    }
    // Every page each needed range touches, not just its two ends, so a range that spans a
    // withheld page is refused too. The offset named is the first needed byte on the bad page.
    for (first, last) in needed_ranges(capabilities) {
        for page in first / PAGE_BYTES..=last / PAGE_BYTES {
            let offset = first.max(page * PAGE_BYTES);
            if page >= pages {
                return Err(WindowRefusal::BeyondWindow { offset });
            }
            if mask & (1 << page) == 0 {
                return Err(WindowRefusal::SharesTheInterruptPage { offset });
            }
        }
    }
    Ok(mask)
}

#[cfg(kani)]
mod proofs;

#[cfg(test)]
mod tests {
    extern crate std;
    use ::extensible_host_controller_interface::dma::*;

    use super::*;

    fn qemu() -> Capabilities {
        Capabilities::decode(0x0100_0040, 0x0400_0840, 0, 0x0000_7000, 0x2000, 0x1000)
    }

    #[test]
    fn qemus_window_withholds_the_msix_page_and_nothing_else() {
        assert_eq!(
            register_window(&qemu(), 0x4000, &[(0x3000, 0x3100), (0x3800, 0x3808)]),
            Ok(0b0111)
        );
        // No MSI-X at all (a function delivering INTx): every page.
        assert_eq!(register_window(&qemu(), 0x4000, &[]), Ok(0b1111));
        // A withheld range is half open: one that ends where page 4 starts leaves page 4 mapped.
        assert_eq!(
            register_window(&qemu(), 0x5000, &[(0x3000, 0x4000)]),
            Ok(0b1_0111)
        );
    }

    /// **The refusal that makes the rule checkable**: a controller whose doorbells share a page
    /// with its MSI-X table cannot be handed to a driver at all, because mapping the doorbells
    /// would map the table.
    #[test]
    fn a_needed_register_on_the_interrupt_page_is_refused() {
        assert_eq!(
            register_window(&qemu(), 0x4000, &[(0x2000, 0x2100)]),
            Err(WindowRefusal::SharesTheInterruptPage { offset: 0x2000 })
        );
    }

    #[test]
    fn a_needed_register_beyond_the_bar_is_refused() {
        assert_eq!(
            register_window(&qemu(), 0x2000, &[]),
            Err(WindowRefusal::BeyondWindow { offset: 0x2000 })
        );
    }

    /// An Intel-shaped controller: a 64 KiB BAR, runtime at 0x2000, doorbells at 0x3000, the
    /// MSI-X table well above them. Values from the xHCI capability layout Intel's datasheets
    /// describe, not read from xenon (a bench step on xenon reads its own).
    #[test]
    fn a_sixty_four_kibibyte_bar_maps_sixteen_pages_less_the_table() {
        let capabilities =
            Capabilities::decode(0x0100_0080, 0x1a00_0840, 0, 0x0000_7000, 0x3000, 0x2000);
        let mask = register_window(&capabilities, 0x1_0000, &[(0xa000, 0xa000 + 16 * 8)]).unwrap();
        assert_eq!(mask, 0xffff & !(1 << 10));
    }

    #[test]
    fn the_handoff_round_trips_and_refuses_what_no_kernel_sends() {
        let h = Handoff {
            dma_phys: 0x4000_0000,
            dma_pages: 12,
            registers: 0b0111,
        };
        assert_eq!(Handoff::unpack(h.pack()), Some(h));
        assert_eq!(Handoff::unpack([0x4000_0001, 12 | 7 << 32, 0]), None);
        assert_eq!(Handoff::unpack([0, 12 | 7 << 32, 0]), None);
        assert_eq!(Handoff::unpack([0x4000_0000, 3 | 7 << 32, 0]), None);
        assert_eq!(Handoff::unpack([0x4000_0000, 12 | 6 << 32, 0]), None);
    }

    /// A doorbell range that runs onto a withheld page is refused at the first needed byte on it,
    /// and so is a needed range that starts on one.
    #[test]
    fn a_needed_range_crossing_onto_a_withheld_page_is_refused() {
        // 64 slots of doorbells from 0x2f00 run to 0x3003, onto page 3.
        let capabilities = Capabilities::decode(0x0100_0040, 0x0400_0840, 0, 0, 0x2f00, 0x1000);
        assert_eq!(
            register_window(&capabilities, 0x4000, &[(0x3000, 0x3100)]),
            Err(WindowRefusal::SharesTheInterruptPage { offset: 0x3000 })
        );
    }

    /// QEMU's needed ranges, inclusive at both ends: through port 4's status set, the 32 bytes of
    /// interrupter 0, and the doorbells through slot 64's.
    #[test]
    fn the_needed_ranges_end_at_their_last_byte() {
        let capabilities = Capabilities::decode(0x0100_0040, 0x0400_0840, 0, 0, 0x2000, 0x1000);
        assert_eq!(
            needed_ranges(&capabilities),
            [(0, 0x47f), (0x1020, 0x103f), (0x2000, 0x2103)]
        );
    }

    /// **The layout both ends rely on**: every work page is used once and lies inside the work
    /// window the driver is mapped, and the scratchpad pages begin exactly where it ends, so the
    /// controller's private memory never overlaps a page the driver writes. A page constant moved
    /// onto another, or past `WORK_PAGES`, would hand the controller a ring the driver also uses
    /// as a buffer, or a scratchpad the driver is mapped.
    #[test]
    fn the_work_pages_are_distinct_and_the_scratchpads_follow_them() {
        let pages = [
            DEVICE_CONTEXT_ARRAY,
            COMMAND_RING,
            EVENT_RING,
            EVENT_SEGMENT_TABLE,
            INPUT_CONTEXT,
            DEVICE_CONTEXT,
            CONTROL_RING,
            INTERRUPT_RING,
            CONTROL_BUFFER,
            REPORT_BUFFERS,
            SCRATCHPAD_ARRAY,
        ];
        let mut seen = [false; WORK_PAGES as usize];
        for p in pages {
            assert!(p < WORK_PAGES, "page {p} is outside the mapped work pages");
            assert!(!seen[p as usize], "page {p} is used twice");
            seen[p as usize] = true;
        }
        assert!(seen.iter().all(|&s| s), "a work page is mapped and unused");
        assert_eq!(region_pages(0), WORK_PAGES);
        // The last scratchpad of the largest accepted set is the region's last page.
        assert_eq!(
            region_pages(MAX_SCRATCHPADS) - 1,
            WORK_PAGES + u64::from(MAX_SCRATCHPADS) - 1
        );
    }
}
