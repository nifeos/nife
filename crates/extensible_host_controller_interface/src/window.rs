//! **The register window**: which pages of the BAR the driver is mapped, and why a layout is refused.

use super::*;

/// **Why a register window could not be drawn.**
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
/// use extensible_host_controller_interface::{Capabilities, register_window};
///
/// // qemu-xhci's capability registers: 0x40 bytes of them, 64 slots, 4 ports, runtime at 0x1000,
/// // doorbells at 0x2000.
/// let caps = Capabilities::decode(0x0100_0040, 0x0400_0840, 0, 0x0000_7000, 0x2000, 0x1000);
/// let window = register_window(&caps, 0x4000, &[(0x3000, 0x3000 + 16 * 16), (0x3800, 0x3808)]);
/// assert_eq!(window, Ok(0b0111));
/// ```
pub fn register_window(
    caps: &Capabilities,
    bar_bytes: u64,
    withheld: &[(u64, u64)],
) -> Result<u64, WindowRefusal> {
    let pages = (bar_bytes / PAGE).min(MAX_REGISTER_PAGES);
    let mut mask = 0u64;
    for page in 0..pages {
        let (start, end) = (page * PAGE, (page + 1) * PAGE);
        if !withheld.iter().any(|&(a, b)| a < end && start < b) {
            mask |= 1 << page;
        }
    }
    // Every page each needed range touches, not just its two ends, so a range that spans a
    // withheld page is refused too. The offset named is the first needed byte on the bad page.
    for (first, last) in caps.needed_ranges() {
        for page in first / PAGE..=last / PAGE {
            let offset = first.max(page * PAGE);
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
