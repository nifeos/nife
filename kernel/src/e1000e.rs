//! **The `e1000e` NIC's control plane** (milestone 494 (a driver for the network card a PC actually
//! has); notes/e1000e.md).
//!
//! The kernel half of milestone 261 (the NVMe driver leaves the kernel)'s shape applied to a second
//! device. This file finds the controller, confines its DMA to one region, resets it, reads its MAC
//! address, programs both rings' base registers and turns the receiver and transmitter on; then
//! `kernel/src/user/e1000e_service.rs` hands a process the two queue pages of BAR0 and the region,
//! and that process (`net_stack`, through `components/src/e1000e_transport.rs`) fills descriptors
//! and moves tails. Every offset, bit and layout here comes from `crates/e1000e`.
//!
//! What stays here is what the process must not be able to do: reset the device, change the MAC
//! address or the receive filter, turn on promiscuous receive, or reach the PHY. All of it is on
//! BAR0's pages 0 and 5, which the process is never mapped. **What does not stay here is ring
//! placement**: the ring base registers share a page with the tails (`crates/e1000e`'s header), so
//! a process that drives the data plane can repoint a ring, and the IOMMU domain built below is the
//! whole of what bounds the device's DMA.
//!
//! # BUGS
//!
//! - **Two bring-ups share this file, and QEMU proves one of them.** The 82574L's reset is this
//!   file's own; an I219 instead runs FreeBSD's three passes from `crates/e1000e/src/pch/`
//!   (`sequence`: the PHY workarounds, the global reset with the PHY, the hardware and copper-link
//!   setup), with the SPT ring flush here between the first two. The QEMU gates run the 82574L and
//!   the MDIO primitive both paths share (the PHY identifier read); the I219 passes first run on
//!   xenon (notes/e1000e.md's bench step). FreeBSD's link-up reconfiguration is not ported
//!   (`pch`'s `BUGS`).
//! - **Waiting for link is a bounded spin at bring-up**, up to [`LINK_WAIT_MS`]. QEMU's link is
//!   up at once; a real copper link negotiates in seconds. A bring-up that times out reports
//!   [`Found::link_up`] false and `user::e1000e_service` starts no server over it, because a
//!   `net_stack` with no link waits for a lease that never comes. The spin holds a core, which is
//!   acceptable once per wiring and not more often.
//! - **The DMA region is allocated once per boot and never returned**, like the NVMe region, so a
//!   boot that wires the NIC carries [`::e1000e::layout::PAGES`] pages for its whole life.

use core::sync::atomic::{AtomicU64, Ordering};

use ::e1000e::{layout, regs};

use crate::arch::mmu;

/// The longest bring-up waits for `CTRL.RST` to clear.
const RESET_WAIT_MS: u64 = 1000;
/// The longest bring-up waits for `STATUS.LU`.
pub(crate) const LINK_WAIT_MS: u64 = 3000;

/// Why a controller that is on the bus was not brought up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// `CTRL.RST` did not self-clear.
    ResetTimeout,
    /// Receive-address entry 0 was not marked valid after reset: the NVM load did not happen.
    NoMacAddress,
    /// An I219 bring-up pass stopped (`crates/e1000e/src/pch/sequence.rs`).
    Pch(::e1000e::pch::sequence::Error),
    /// The PHY identifier could not be read over MDIO.
    PhyId(::e1000e::pch::phy::Error),
}

/// Why there is no NIC to hand out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Absent {
    /// No function on the bus is one `crates/e1000e` claims.
    NoController,
    /// One is, and bring-up refused it. Never a skip.
    Refused {
        /// The controller's requester id.
        rid: u32,
        /// What went wrong.
        why: Error,
    },
}

/// A controller reset, programmed, and ready for a data plane.
#[derive(Debug, Clone, Copy)]
pub struct Found {
    /// BAR0's physical base. The process is mapped `bar0 + RX_QUEUE_PAGE` and `+ TX_QUEUE_PAGE`.
    pub bar0: u64,
    /// The requester id the IOMMU confines.
    pub rid: u32,
    /// The PCI device id.
    pub device: u16,
    /// What the process is told: the MAC address and the region's physical base.
    pub handoff: ::e1000e::Handoff,
    /// Whether `STATUS.LU` was set before [`LINK_WAIT_MS`] ran out.
    pub link_up: bool,
    /// The PHY's identifier, revision masked off, read over MDIO. On the 82574L this is the read
    /// that proves [`::e1000e::pch::phy::read_mdic`] under QEMU.
    pub phy_id: u32,
}

/// The region's physical base, once allocated: zero until the first bring-up.
static REGION: AtomicU64 = AtomicU64::new(0);

/// **Find, confine, reset and program the NIC.** Called once per data plane; a second call resets
/// the device under whatever process held the first, so the service that calls this refuses while
/// that process lives (`e1000e_service`'s `BUGS`).
pub fn bring_up() -> Result<Found, Absent> {
    let dev = crate::pci::find_e1000e_device().ok_or(Absent::NoController)?;
    let region = region_for(dev.rid);
    let c = Controller(mmu::phys_to_virt(dev.bar0));
    let refused = |why| Absent::Refused { rid: dev.rid, why };
    let pch = ::e1000e::pch::is_pch(dev.device);
    let mut hw = Bar { c: &c, dev: &dev };
    let mut phy_id = None;
    if pch {
        // An I219 behind a PCH: FreeBSD's passes, in FreeBSD's order (`crates/e1000e/src/pch/`,
        // which has the sources and Intel's licence). QEMU's 82574L does not take this branch.
        use ::e1000e::pch::sequence;
        let id = sequence::init_phy_workarounds(&mut hw).map_err(|e| refused(Error::Pch(e)))?;
        phy_id = Some(id);
        flush_descriptor_rings(&c, &dev, region);
        sequence::global_reset(&mut hw, RESET_WAIT_MS).map_err(|e| refused(Error::Pch(e)))?;
    } else {
        reset_82574(&c).map_err(refused)?;
    }

    // Reset re-enabled nothing we want; mask again and clear anything pending.
    c.w(regs::IMC, u32::MAX);
    let _ = c.r(regs::ICR);
    zero(region);

    // Tell management firmware a driver has the device, and bring the link up.
    c.w(
        regs::CTRL_EXT,
        c.r(regs::CTRL_EXT) | ::e1000e::CTRL_EXT_DRV_LOAD,
    );
    if pch {
        // FreeBSD's `e1000_init_hw_ich8lan`, whose copper-link setup sets `CTRL.SLU` itself.
        ::e1000e::pch::sequence::init_hw(&mut hw).map_err(|e| refused(Error::Pch(e)))?;
    } else {
        c.w(
            regs::CTRL,
            c.r(regs::CTRL) | ::e1000e::CTRL_SLU | ::e1000e::CTRL_ASDE,
        );
    }

    let mac = ::e1000e::mac_from_receive_address(c.r(regs::RAL0), c.r(regs::RAH0))
        .ok_or(refused(Error::NoMacAddress))?;

    // No multicast groups: the table is unspecified after reset.
    for k in 0..regs::MTA_DWORDS {
        c.w(regs::MTA + 4 * k, 0);
    }
    // Legacy receive descriptors, the only format `crates/e1000e` reads.
    c.w(regs::RFCTL, c.r(regs::RFCTL) & !::e1000e::RFCTL_EXTEN);

    // The rings. Heads and tails at zero: `DataPlane::start` posts the receive buffers by moving
    // the receive tail, which is the process's first act.
    let rx = region + layout::RX_RING;
    let tx = region + layout::TX_RING;
    c.w(regs::RDBAL, rx as u32);
    c.w(regs::RDBAH, (rx >> 32) as u32);
    c.w(regs::RDLEN, u32::from(::e1000e::RX_ENTRIES) * 16);
    c.w(regs::RDH, 0);
    c.w(regs::RDT, 0);
    c.w(regs::TDBAL, tx as u32);
    c.w(regs::TDBAH, (tx >> 32) as u32);
    c.w(regs::TDLEN, u32::from(::e1000e::TX_ENTRIES) * 16);
    c.w(regs::TDH, 0);
    c.w(regs::TDT, 0);

    c.w(regs::TIPG, ::e1000e::tipg());
    c.w(regs::TCTL, ::e1000e::tctl());
    if pch {
        ::e1000e::pch::sequence::transmit_errata(&mut hw);
    }
    c.w(regs::RCTL, ::e1000e::rctl());

    // The 82574L's PHY answers at address 1 with no pages and no semaphore. QEMU emulates it, so
    // this is the one MDIO read the gates see.
    let phy_id = match phy_id {
        Some(id) => id,
        None => ::e1000e::pch::phy::id_at(&mut hw, 1).map_err(|e| refused(Error::PhyId(e)))?,
    };

    let link_up = wait_ms(LINK_WAIT_MS, || {
        c.r(regs::STATUS) & ::e1000e::STATUS_LU != 0
    });

    Ok(Found {
        bar0: dev.bar0,
        rid: dev.rid,
        device: dev.device,
        handoff: ::e1000e::Handoff {
            mac,
            data_plane_phys: region,
        },
        link_up,
        phy_id,
    })
}

/// **The 82574L's quiesce and global reset**, unchanged from before the I219 passes existed.
fn reset_82574(c: &Controller) -> Result<(), Error> {
    // Quiesce: no interrupts, receiver and transmitter off, then a posted-write flush.
    c.w(regs::IMC, u32::MAX);
    c.w(regs::RCTL, 0);
    c.w(regs::TCTL, 0);
    let _ = c.r(regs::STATUS);
    spin_ms(10);
    // Global reset. Self-clearing; the registers read garbage while it runs, so wait first.
    c.w(regs::CTRL, c.r(regs::CTRL) | ::e1000e::CTRL_RST);
    spin_ms(20);
    if !wait_ms(RESET_WAIT_MS, || c.r(regs::CTRL) & ::e1000e::CTRL_RST == 0) {
        return Err(Error::ResetTimeout);
    }
    Ok(())
}

/// The DMA region: allocated and confined on the first call, the same region on every later one.
fn region_for(rid: u32) -> u64 {
    let base = REGION.load(Ordering::Acquire);
    if base != 0 {
        return base;
    }
    let base = crate::memory::alloc_contiguous_zeroed(layout::PAGES as usize)
        .expect("no DMA region for the e1000e NIC")
        .addr();
    if crate::iommu::is_active() {
        crate::iommu::confine(
            rid,
            &[paging::domain::DmaRegion {
                base,
                size: layout::PAGES * page_frames::FRAME_SIZE,
                writable: true,
            }],
        );
    }
    REGION.store(base, Ordering::Release);
    base
}

/// Zero the region, after the reset and before the rings are programmed. A stale descriptor status
/// from a previous wiring would otherwise read as a received frame.
fn zero(region: u64) {
    // SAFETY: `region` is the run `region_for` allocated, `layout::BYTES` long, in the direct map.
    // The device was reset just before this, so it is not DMA-ing into it, and the service refuses
    // a new wiring while the previous process lives, so no process has it mapped.
    unsafe {
        core::ptr::write_bytes(
            mmu::phys_to_virt(region) as *mut u8,
            0,
            layout::BYTES as usize,
        );
    }
}

/// **Empty the descriptor rings before the reset, if the hardware says it must**, FreeBSD's
/// `em_flush_desc_rings`. Skipped when the transmit ring the device points at is not this
/// driver's region: the dummy descriptor has to go somewhere both the device may read and this
/// kernel owns, and a ring firmware left behind is neither.
fn flush_descriptor_rings(c: &Controller, dev: &crate::pci::PciE1000eDevice, region: u64) {
    use ::e1000e::pch::{flush, regs as pch};
    c.w(
        pch::FEXTNVM11,
        c.r(pch::FEXTNVM11) | flush::FEXTNVM11_DISABLE_MULR_FIX,
    );
    let status = || dev.config_read32(flush::DESC_RING_STATUS) as u16;
    if !flush::required(status(), c.r(regs::TDLEN)) {
        return;
    }
    let ring = region + layout::TX_RING;
    let tdba = u64::from(c.r(regs::TDBAL)) | u64::from(c.r(regs::TDBAH)) << 32;
    if tdba != ring {
        crate::println!(
            "  e1000e: the rings need flushing and the transmit ring at {tdba:#x} is not this \
             driver's; skipped, so the reset may hang the device"
        );
        return;
    }
    c.w(regs::TCTL, c.r(regs::TCTL) | 0x2);
    let tdt = c.r(regs::TDT) % u32::from(::e1000e::TX_ENTRIES);
    let d = mmu::phys_to_virt(region + layout::tx_descriptor(tdt as u16)) as *mut u64;
    // SAFETY: `d` is a 16-byte descriptor inside the transmit ring page of this driver's region
    // (`layout::tx_descriptor` reduces its index), in the direct map. The device reads it only
    // after the tail write below, which the barrier orders after these stores.
    unsafe {
        core::ptr::write_volatile(d, ring);
        core::ptr::write_volatile(d.add(1), flush::tx_word());
    }
    crate::arch::direct_memory_access_write_barrier();
    c.w(regs::TDT, flush::tail_after(tdt, ::e1000e::TX_ENTRIES));
    spin_us(250);
    if status() & flush::REQUIRED == 0 {
        return;
    }
    let rctl = c.r(regs::RCTL);
    c.w(regs::RCTL, rctl & !0x2);
    let _ = c.r(regs::STATUS);
    spin_us(150);
    c.w(pch::RXDCTL, flush::rxdctl(c.r(pch::RXDCTL)));
    c.w(regs::RCTL, rctl | 0x2);
    let _ = c.r(regs::STATUS);
    spin_us(150);
    c.w(regs::RCTL, rctl & !0x2);
}

/// BAR0, through the direct map.
struct Controller(u64);

/// The largest offset any `crates/e1000e` constant reaches, plus the access: the flash data
/// register at `0xe010` on SPT is the highest. Both parts' BAR0 is 128 KiB.
const BAR0_BYTES: u64 = 128 * 1024;

impl Controller {
    fn r(&self, off: u64) -> u32 {
        assert!(off + 4 <= BAR0_BYTES);
        // SAFETY: BAR0 is device-mapped inside the PCI BAR window `mmu::map_everything` maps, and
        // the assertion keeps `off` inside the 128 KiB BAR both claimed families have.
        unsafe { core::ptr::read_volatile((self.0 + off) as *const u32) }
    }
    fn w(&self, off: u64, v: u32) {
        assert!(off + 4 <= BAR0_BYTES);
        // SAFETY: as `r`.
        unsafe { core::ptr::write_volatile((self.0 + off) as *mut u32, v) }
    }
    fn r16(&self, off: u64) -> u16 {
        assert!(off + 2 <= BAR0_BYTES);
        // SAFETY: as `r`. Only the SPT flash status word is read this way.
        unsafe { core::ptr::read_volatile((self.0 + off) as *const u16) }
    }
}

/// **What `crates/e1000e`'s I219 sequences and its MDIO layer run against**: BAR0, the function's
/// configuration space for the vendor-id read the reset needs as a delay, the timer, and the
/// console for what the bench boot should see.
struct Bar<'a> {
    c: &'a Controller,
    dev: &'a crate::pci::PciE1000eDevice,
}

impl ::e1000e::pch::Hw for Bar<'_> {
    fn read(&mut self, off: u64) -> u32 {
        self.c.r(off)
    }
    fn write(&mut self, off: u64, v: u32) {
        self.c.w(off, v);
    }
    fn read16(&mut self, off: u64) -> u16 {
        self.c.r16(off)
    }
    fn delay_us(&mut self, us: u64) {
        spin_us(us);
    }
    fn pci_vendor_id(&mut self) -> u16 {
        self.dev.config_read32(0) as u16
    }
    fn note(&mut self, n: ::e1000e::pch::sequence::Note) {
        use ::e1000e::pch::sequence::Note;
        match n {
            // The identifier in hex, as the bench card compares it.
            Note::PhyReached { at, id } => {
                crate::println!("  e1000e: PhyReached {{ at: {at:?}, id: {id:#010x} }}");
            }
            n => crate::println!("  e1000e: {n:?}"),
        }
    }
}

fn spin_us(us: u64) {
    crate::arch::timer::spin_for(us * crate::arch::timer::frequency() / 1_000_000);
}

fn spin_ms(ms: u64) {
    crate::arch::timer::spin_for(ms * crate::arch::timer::frequency() / 1000);
}

/// Poll `done` until it holds or `ms` pass. True when it held.
fn wait_ms(ms: u64, mut done: impl FnMut() -> bool) -> bool {
    let deadline = crate::arch::timer::now() + ms * crate::arch::timer::frequency() / 1000;
    loop {
        if done() {
            return true;
        }
        if crate::arch::timer::now() >= deadline {
            return done();
        }
        core::hint::spin_loop();
    }
}
