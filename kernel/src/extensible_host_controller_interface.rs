//! **The xHCI controller's bring-up policy** (milestone 242 (USB host and HID, because on
//! commodity hardware the keyboard is not a UART); notes/usb.md).
//!
//! What the kernel does for a USB host controller, which is less than it does for NVMe and on
//! purpose. NVMe kept its admin plane at EL1 (DECISIONS §86 (whether an NVMe driver can leave the
//! kernel), option 2a) because creating a queue is where a ring's physical address is named. An
//! xHCI names physical addresses everywhere (the slot array, the command ring, the event ring,
//! every transfer ring, every input context), and enumeration means **reading what a device chose
//! to send**, which is the last thing to do at EL1. So this module does only the things that are
//! not the driver's to do, and the whole controller goes to a confined EL0 process:
//!
//! 1. **Find it** (`pci::find_xhci_device`): place the BAR, enable decoding and bus mastering,
//!    resolve the interrupt (MSI-X, MSI, or the INTx swizzle, as this machine delivers them).
//! 2. **Take it from the firmware** ([`take_from_firmware`]): the USB Legacy Support handshake,
//!    so a PC's BIOS stops answering SMIs for the keyboard. A firmware relationship, which no
//!    process should hold.
//! 3. **Decide which register pages the driver is mapped**
//!    (`extensible_host_controller_interface::register_window`): every page of BAR0 except the
//!    ones the MSI-X table and pending-bit array sit on, refused outright if a register the driver
//!    needs shares one of those pages.
//! 4. **Allocate its DMA region and confine the controller to it**, and refuse to go further when
//!    the IOMMU does not confine this requester id. A process that holds a whole xHCI register file
//!    can point the controller at any physical address it names; the IOMMU is the only thing that
//!    bounds that, so without it there is no driver to start. NVMe runs unconfined on a machine
//!    with no IOMMU; this one does not, and the difference is that this process names every ring.
//!
//! The kernel never resets the controller, never writes a ring and never reads a descriptor. Rule
//! 2 holds the usual way: the policy function reaches for the bus and the allocator, and nothing
//! below it does.
//!
//! # BUGS
//!
//! - **The first xHCI on the bus is the only one.** A PC with a second controller (a USB 3.1 add-in
//!   card, or a laptop's Thunderbolt one) has its keyboard found only if it is on the first.
//! - **No confinement, no keyboard.** A machine whose IOMMU does not own the controller's requester
//!   id (radon has no IOMMU; a VT-d table whose catch-all unit is not the one this kernel brought
//!   up) gets a refusal sentence and keystrokes over the UART only. That is the design, and it is a
//!   cost.
//!

//! Name: provisional (milestone 242 (USB host and HID)). The crate's name, as
//! `kernel/src/non_volatile_memory_express.rs` takes its crate's: the volatile half of one pair.

use extensible_host_controller_interface::{Capabilities, Handoff, WindowRefusal, dma, regs};

use crate::arch::mmu;

/// **Why there is no xHCI driver this boot.** [`Absent::NoController`] is a fact about the machine
/// and prints nothing; every other variant is a fact about this driver and is a sentence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Absent {
    /// No function on the bus carries the xHCI class code.
    NoController,
    /// The register window could not be drawn without mapping the interrupt message's page.
    Window(WindowRefusal),
    /// The controller wants more scratchpad pages than one page of pointers can name.
    TooManyScratchpads(u16),
    /// No contiguous run of memory this size.
    NoMemory(u64),
    /// The IOMMU does not confine this controller, so a process holding its register file could
    /// aim it at any physical address.
    Unconfined,
}

/// **A controller ready to be handed to a driver**: where its registers are, which pages of them
/// the driver gets, its DMA region, its interrupt.
pub(crate) struct Found {
    /// BAR0's physical base.
    pub bar0: u64,
    /// The spawn words.
    pub handoff: Handoff,
    /// The interrupt the driver waits on.
    pub intid: u32,
}

/// How many polls of the legacy semaphore before the firmware is declared to have kept the
/// controller. Roughly a second on hardware, where each MMIO read is around a microsecond; the
/// xHCI specification gives the BIOS no deadline, and Linux waits one second.
const FIRMWARE_SPIN_BOUND: u32 = 1_000_000;

/// **Find, take, window and confine the machine's xHCI controller.**
pub fn bring_up() -> Result<Found, Absent> {
    let dev = crate::pci::find_xhci_device().ok_or(Absent::NoController)?;
    let regs_va = mmu::phys_to_virt(dev.bar0);
    let read = |off: u64| -> u32 {
        // SAFETY: BAR0 is inside the PCI window `mmu::map_everything` maps as device memory, and
        // every offset read here is a capability register or an extended capability the walk
        // bounded by `bar_bytes`.
        unsafe { core::ptr::read_volatile((regs_va + off) as *const u32) }
    };
    let caps = Capabilities::decode(
        read(regs::CAPLENGTH),
        read(regs::HCSPARAMS1),
        read(regs::HCSPARAMS2),
        read(regs::HCCPARAMS1),
        read(regs::DBOFF),
        read(regs::RTSOFF),
    );
    #[cfg(feature = "xhci_port_probe")]
    crate::xhci_port_probe::before_handoff(regs_va, dev.bar_bytes);
    take_from_firmware(regs_va, &caps, dev.bar_bytes);

    let registers =
        extensible_host_controller_interface::register_window(&caps, dev.bar_bytes, &dev.withheld)
            .map_err(Absent::Window)?;
    if caps.scratchpads > dma::MAX_SCRATCHPADS {
        return Err(Absent::TooManyScratchpads(caps.scratchpads));
    }
    let pages = dma::region_pages(caps.scratchpads);

    // Confinement is checked before anything is allocated for it, because the answer does not
    // depend on the region: whether the IOMMU unit this kernel runs owns this requester id.
    let scope = crate::iommu::scope_of(dev.rid);
    if !crate::iommu::is_active() || !scope.is_confining() {
        return Err(Absent::Unconfined);
    }
    // Zeroed, and that is load-bearing: every ring's cycle discipline starts from zeroed TRBs,
    // and the scratchpads are the controller's own memory, which must hold nothing of anyone's.
    let dma = crate::memory::alloc_contiguous_zeroed(pages as usize)
        .ok_or(Absent::NoMemory(pages))?
        .addr();
    crate::iommu::confine(
        dev.rid,
        &[paging::domain::DmaRegion {
            base: dma,
            size: pages * page_frames::FRAME_SIZE,
            writable: true,
        }],
    );
    Ok(Found {
        bar0: dev.bar0,
        handoff: Handoff {
            dma_phys: dma,
            dma_pages: pages as u32,
            // `register_window` never sets a bit at or above `MAX_REGISTER_PAGES` (16).
            registers: registers as u16,
        },
        intid: dev.intid,
    })
}

/// **The USB Legacy Support handshake** (xHCI 1.2 section 4.22.1): set the OS-owned semaphore,
/// wait for the BIOS to drop its own, then turn off every SMI the firmware asked the controller to
/// raise. A PC's firmware drives the keyboard itself until this happens, through SMIs that stop the
/// whole machine; QEMU's controller has no such capability and this does nothing there.
///
/// A firmware that never lets go is reported and the boot goes on, which is Linux's choice too: the
/// controller is reset by the driver next, and the firmware's SMI handler then fights a controller
/// it no longer understands, which is a degraded machine rather than a dead one.
fn take_from_firmware(regs_va: u64, caps: &Capabilities, bar_bytes: u64) {
    let mut read = |off: u64| -> u32 {
        // SAFETY: `find_extended` bounds `off + 4` by `bar_bytes`, inside the mapped BAR.
        unsafe { core::ptr::read_volatile((regs_va + off) as *const u32) }
    };
    let Some(at) = regs::find_extended(
        caps.extended,
        bar_bytes,
        regs::EXTENDED_LEGACY_SUPPORT,
        &mut read,
    ) else {
        return;
    };
    if read(at) & regs::LEGACY_BIOS_OWNED != 0 {
        // The OS-owned semaphore is the capability's fourth byte, written alone so the BIOS's own
        // byte is never written back by this side.
        // SAFETY: `at + 3` is inside the capability `find_extended` found inside the BAR.
        unsafe { core::ptr::write_volatile((regs_va + at + 3) as *mut u8, 1) };
        let mut released = false;
        for _ in 0..FIRMWARE_SPIN_BOUND {
            if read(at) & regs::LEGACY_BIOS_OWNED == 0 {
                released = true;
                break;
            }
            core::hint::spin_loop();
        }
        if !released {
            crate::println!(
                "  usb       : the firmware did not release the USB controller; taking it anyway"
            );
        }
    }
    let ctl = read(at + 4);
    // SAFETY: `USBLEGCTLSTS` is the dword after `USBLEGSUP`, inside the same capability.
    unsafe {
        core::ptr::write_volatile(
            (regs_va + at + 4) as *mut u32,
            (ctl & !regs::LEGACY_SMI_ENABLES) | regs::LEGACY_SMI_EVENTS,
        );
    }
}
