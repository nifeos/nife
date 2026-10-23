//! The JH7110 Ethernet controller's control plane, `kernel/src/e1000e.rs`'s shape on radon's first
//! port: clocks and resets, RGMII, the YT8531 PHY, the controller reset, a DMA coherence probe
//! before any descriptor is trusted, both rings, the link and the start. Then a process
//! (`net_stack`) fills descriptors. Every offset, bit and sequence comes from
//! `crates/designware_ethernet`; every window from `memory::jh7110_ethernet` (rule 2). See
//! notes/designware-ethernet.md.
//!

//! # Scope note (rule 5, architectural parity)
//!
//! riscv64-only on purpose: no aarch64 or x86_64 machine nife runs on has this controller. The
//! network stack above it is shared, through `e1000e`'s `Handoff` shape. A second board with
//! Synopsys's Ethernet `QoS` controller (the RK3568 and RK3588 are aarch64) would reuse the crate
//! whole and add its own glue beside `designware_ethernet::jh7110`.
//!

//! # BUGS
//!
//! - One board: radon is the only silicon it has run on (notes/designware-ethernet.md, Results);
//!   everything else runs against the crate's simulation.
//! - Waiting for a link is a bounded spin at bring-up, up to [`LINK_WAIT_MS`], holding a core.
//! - The DMA region is allocated once per boot and never returned.
//! - A non-coherent verdict refuses the port, since this tree has no cache maintenance for this
//!   machine (milestone 655 (DMA on a non-coherent RISC-V machine)). An inconclusive verdict does
//!   not refuse, and the bring-up says so.

use core::sync::atomic::{AtomicU64, Ordering};

use ::designware_ethernet::controller::{self, Identity};
use ::designware_ethernet::motorcomm::{self, Link, Speed};
use ::designware_ethernet::{
    DataPlane, Handoff, Hw, Region, Tails, coherence, jh7110, layout, mdio, regs,
};
use jh7110_clock_and_reset as crg;

use crate::arch::mmu;

/// The longest bring-up waits for the PHY to resolve a link after autonegotiation restarts. A
/// gigabit negotiation takes about three seconds; five is slack, not a guess at a slow switch.
pub(crate) const LINK_WAIT_MS: u64 = 5000;

/// Why a port the tree describes was not brought up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// `phy-mode` names no RGMII variant, so there is no interface select to make.
    NotRgmii,
    /// A clock's enable bit did not read back: the window is not the controller it was named as.
    ClocksNotRunning,
    /// A reset did not read as released before the clock driver's poll gave up.
    ResetsHeld,
    /// The controller's version register read as nothing (zero or all ones): its clocks or resets
    /// are not what the plans say, or the window is wrong.
    Silent {
        /// What the version register read.
        version: u32,
    },
    /// The station address is in neither the tree nor the controller's own registers.
    NoStationAddress,
    /// An MDIO command did not complete.
    Mdio(mdio::Error),
    /// The PHY is not the YT8531 this driver's board settings are for.
    UnknownPhy {
        /// What it answered.
        id: u32,
    },
    /// A controller sequence refused (a reset that did not complete, a region out of reach).
    Controller(controller::Error),
    /// The DMA region could not be allocated.
    NoRegion,
    /// The coherence probe found device DMA not coherent with the caches.
    NotCoherent(coherence::Verdict),
}

/// Where the station address came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AddressSource {
    /// The tree's `local-mac-address`, which U-Boot writes from the board's EEPROM.
    Tree,
    /// The controller's address registers, as firmware left them.
    Registers,
}

/// **Everything the bring-up saw**, for the bench boot's lines. Filled as far as the bring-up got,
/// so a refusal still says where it stopped.
#[derive(Debug, Clone, Copy)]
#[cfg_attr(not(feature = "network_bench"), allow(dead_code))]
// read by the bench boot's lines; the booted system prints only `Debug`
pub(crate) struct Report {
    /// The controller's physical base.
    pub base: u64,
    /// Which `compatible` the tree used.
    pub compatible: &'static [u8],
    /// Whether the clock-and-reset windows came from the tree or from the constants.
    pub windows_from_tree: bool,
    /// The SYS-domain clocks' before-and-after words.
    pub sys: crg::Report,
    /// The AON-domain clocks', mux's and resets' words.
    pub aon: crg::Report,
    /// The syscon word before and after the interface select.
    pub syscon: (u32, u32),
    /// The controller's version register.
    pub version: u32,
    /// The DMA's address width.
    pub dma_bits: u8,
    /// The PHY's identifier.
    pub phy_id: u32,
    /// The PHY's I/O voltage and which tree spelling its settings came from.
    pub phy: Option<(u32, motorcomm::Source)>,
    /// The station address and where it came from.
    pub mac: Option<([u8; 6], AddressSource)>,
    /// How long each software reset took, in microseconds (the probe's and the real one's).
    pub resets_us: [u64; 2],
    /// The coherence probe's verdict and what it saw.
    pub coherence: Option<(coherence::Verdict, coherence::Observation)>,
    /// The negotiated link, if one resolved.
    pub link: Option<Link>,
    /// How long the link took to resolve, or how long was waited for it.
    pub link_wait_ms: u64,
}

impl Report {
    fn new(e: &crate::memory::Jh7110Ethernet) -> Report {
        Report {
            base: e.port.base,
            compatible: e.port.compatible,
            windows_from_tree: e.aon.from_tree && e.sys.from_tree && e.syscon.from_tree,
            sys: crg::Report::default(),
            aon: crg::Report::default(),
            syscon: (0, 0),
            version: 0,
            dma_bits: 0,
            phy_id: 0,
            phy: None,
            mac: None,
            resets_us: [0; 2],
            coherence: None,
            link: None,
            link_wait_ms: 0,
        }
    }
}

/// Why there is no port to hand out. Small on purpose: how far a refused bring-up got is in
/// [`last_report`], not carried through every `Result` on the way out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Absent {
    /// The tree names no JH7110 Ethernet port: every machine but a JH7110.
    NoController,
    /// It does, and bring-up refused it. A failure, never a skip.
    Refused(Error),
}

/// A port brought up, programmed, and ready for a data plane.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Found {
    /// What the process is told: the station address and the region's physical base.
    pub handoff: Handoff,
    /// The controller's DMA page, physical: the one page of the window the process is mapped.
    pub dma_page: u64,
    /// The link, if one resolved. The controller is started only when one did.
    pub link: Option<Link>,
}

/// The region's physical base, once allocated: zero until the first bring-up.
static REGION: AtomicU64 = AtomicU64::new(0);

/// The last bring-up's account, refused or not: one boot artifact, read by the bench boot's lines.
static LAST_REPORT: crate::sync::IrqSafeMutex<Option<Report>> =
    crate::sync::IrqSafeMutex::new(crate::sync::rank::RAM, None);

/// **What the last bring-up saw**, as far as it got. `None` before any.
#[cfg_attr(not(feature = "network_bench"), allow(dead_code))] // the bench boot is its reader
pub(crate) fn last_report() -> Option<Report> {
    *LAST_REPORT.lock()
}

/// **Bring the first port up.** A second call resets the controller under whatever process held
/// the first, so the service refuses while that process lives (`designware_ethernet_service`).
pub fn bring_up() -> Result<Found, Absent> {
    let e = crate::memory::jh7110_ethernet().ok_or(Absent::NoController)?;
    let mut report = Report::new(&e);
    let outcome = run(&e, &mut report);
    *LAST_REPORT.lock() = Some(report);
    let (handoff, link) = outcome.map_err(Absent::Refused)?;
    Ok(Found {
        handoff,
        dma_page: e.port.base + u64::from(regs::DMA_PAGE),
        link,
    })
}

/// The bring-up, in order; see this module's header.
fn run(
    e: &crate::memory::Jh7110Ethernet,
    r: &mut Report,
) -> Result<(Handoff, Option<Link>), Error> {
    if !e.port.rgmii {
        return Err(Error::NotRgmii);
    }

    // Clocks, then resets: the SYS plan is all clocks, and the AON plan ends in both resets, so
    // running SYS first means every clock is up before either reset is released.
    // SAFETY: `memory::jh7110_ethernet` recorded these windows only on a tree that names a JH7110
    // Ethernet port, and RISC-V's `mmu::init` mapped each device-typed (steps 6c and 6d), which is
    // `bring_up`'s contract.
    unsafe {
        r.sys = crate::drivers::jh7110_clock_and_reset::bring_up(
            mmu::phys_to_virt(e.sys.base) as usize,
            &crg::SYS,
            crg::GMAC0_SYS_BRING_UP,
        );
        r.aon = crate::drivers::jh7110_clock_and_reset::bring_up(
            mmu::phys_to_virt(e.aon.base) as usize,
            &crg::AON,
            crg::GMAC0_AON_BRING_UP,
        );
    }
    if !r.sys.has_clocks_running() || !r.aon.has_clocks_running() {
        return Err(Error::ClocksNotRunning);
    }
    if !r.aon.every_reset_released() {
        return Err(Error::ResetsHeld);
    }

    let syscon = mmu::phys_to_virt(e.syscon.base + jh7110::GMAC0_SYSCON_OFFSET) as *mut u32;
    // SAFETY: the AON syscon page is mapped device-typed (step 6d) and the offset is inside its
    // 4 KiB. A read-modify-write of `gmac0`'s three bits; nothing else in this kernel writes the
    // word, and this runs on the one thread wiring the port.
    unsafe {
        let before = core::ptr::read_volatile(syscon);
        core::ptr::write_volatile(syscon, jh7110::select_rgmii(before));
        r.syscon = (before, core::ptr::read_volatile(syscon));
    }

    let mut w = Window(mmu::phys_to_virt(e.port.base));
    let id = Identity::read(&mut w);
    r.version = id.version;
    r.dma_bits = id.dma_bits();
    if id.version == 0 || id.version == u32::MAX {
        return Err(Error::Silent {
            version: id.version,
        });
    }

    // The address before the reset, which returns the registers to all ones.
    let mac = match e.port.mac {
        Some(m) => (m, AddressSource::Tree),
        None => controller::station_address(w.read(regs::MAC_ADDR0_HI), w.read(regs::MAC_ADDR0_LO))
            .map(|m| (m, AddressSource::Registers))
            .ok_or(Error::NoStationAddress)?,
    };
    r.mac = Some(mac);
    let mac = mac.0;

    let phy = e.port.phy_address;
    r.phy_id = mdio::phy_id(&mut mdio::Bus(&mut w), phy).map_err(Error::Mdio)?;
    if r.phy_id != motorcomm::YT8531_ID {
        return Err(Error::UnknownPhy { id: r.phy_id });
    }
    let applied =
        motorcomm::configure(&mut mdio::Bus(&mut w), phy, &e.port.phy).map_err(Error::Mdio)?;
    r.phy = Some((applied.volt_mv, e.port.phy.source));

    // The PHY keeps its receive clock running from here on, which the reset needs.
    r.resets_us[0] = controller::soft_reset(&mut w).map_err(Error::Controller)?;
    let region = region()?;

    let verdict = probe(&mut w, &id, region, mac, r)?;
    if !matches!(
        verdict,
        coherence::Verdict::Coherent | coherence::Verdict::Inconclusive
    ) {
        return Err(Error::NotCoherent(verdict));
    }

    // The probe left the controller in loopback with a used ring: reset it and start clean.
    let _ = controller::stop(&mut w);
    r.resets_us[1] = controller::soft_reset(&mut w).map_err(Error::Controller)?;
    zero(region);
    controller::configure(&mut w, &id, &jh7110::BUS, region, mac).map_err(Error::Controller)?;

    let mut bus = mdio::Bus(&mut w);
    mdio::restart_autonegotiation(&mut bus, phy).map_err(Error::Mdio)?;
    let started = crate::arch::timer::now();
    let deadline = started + LINK_WAIT_MS * crate::arch::timer::frequency() / 1000;
    let link = loop {
        if let Some(l) = motorcomm::link(&mut bus, phy).map_err(Error::Mdio)? {
            break Some(l);
        }
        if crate::arch::timer::now() >= deadline {
            break None;
        }
        spin_ms(10);
    };
    r.link_wait_ms =
        (crate::arch::timer::now() - started) * 1000 / crate::arch::timer::frequency().max(1);
    r.link = link;
    if let Some(l) = link {
        motorcomm::apply_speed(&mut bus, phy, &e.port.phy, l.speed).map_err(Error::Mdio)?;
        controller::start(&mut w, l, false);
    }
    Ok((
        Handoff {
            mac,
            data_plane_phys: region,
        },
        link,
    ))
}

/// **The coherence probe** (`designware_ethernet::coherence`'s header): MAC loopback, one frame,
/// the CPU's account against the device's.
fn probe(
    w: &mut Window,
    id: &Identity,
    region: u64,
    mac: [u8; 6],
    r: &mut Report,
) -> Result<coherence::Verdict, Error> {
    zero(region);
    controller::configure(w, id, &jh7110::BUS, region, mac).map_err(Error::Controller)?;
    let loopback = Link {
        speed: Speed::Thousand,
        full_duplex: true,
    };
    controller::start(w, loopback, true);
    let mut plane = Plane(mmu::phys_to_virt(region));
    let mut tails = TailRegs(w.0);
    let mut dp = DataPlane::new(region);
    // A probe frame is a fixed 128 bytes, so `transmit` cannot refuse it for its length, and the
    // ring was just emptied, so it cannot be full; an error here would leave nothing to observe,
    // which `classify` reports as inconclusive.
    let _ = coherence::prepare(&mut dp, &mut plane, &mut tails, mac);
    let deadline = crate::arch::timer::now()
        + coherence::WAIT_US * crate::arch::timer::frequency() / 1_000_000;
    while !coherence::delivered(&plane) && crate::arch::timer::now() < deadline {
        core::hint::spin_loop();
    }
    let status = w.read(regs::CHAN_STATUS);
    let o = coherence::observe(&plane, mac, status);
    let v = coherence::classify(&o);
    r.coherence = Some((v, o));
    Ok(v)
}

/// The DMA region: allocated on the first call, the same region on every later one.
fn region() -> Result<u64, Error> {
    let base = REGION.load(Ordering::Acquire);
    if base != 0 {
        return Ok(base);
    }
    let base = crate::memory::alloc_contiguous_zeroed(layout::PAGES as usize)
        .ok_or(Error::NoRegion)?
        .addr();
    REGION.store(base, Ordering::Release);
    Ok(base)
}

/// Zero the region. A stale descriptor from the probe, or a previous wiring, would otherwise read
/// as a received frame.
fn zero(region: u64) {
    // SAFETY: `region` is the run `region` allocated, `layout::BYTES` long, in the direct map. The
    // controller was reset (or is about to be configured with nothing available) so it is not
    // writing it, and the service refuses a new wiring while a previous process lives.
    unsafe {
        core::ptr::write_bytes(
            mmu::phys_to_virt(region) as *mut u8,
            0,
            layout::BYTES as usize,
        );
    }
}

/// The controller's register window, through the direct map.
struct Window(u64);

impl Hw for Window {
    fn read(&mut self, off: u32) -> u32 {
        assert!(off + 4 <= regs::WINDOW);
        // SAFETY: the window is device-mapped (step 6d), and the assertion keeps `off` inside its
        // 64 KiB.
        unsafe { core::ptr::read_volatile((self.0 + u64::from(off)) as *const u32) }
    }
    fn write(&mut self, off: u32, v: u32) {
        assert!(off + 4 <= regs::WINDOW);
        // SAFETY: as `read`.
        unsafe { core::ptr::write_volatile((self.0 + u64::from(off)) as *mut u32, v) }
    }
    fn delay_us(&mut self, us: u64) {
        crate::arch::timer::spin_for(us * crate::arch::timer::frequency() / 1_000_000);
    }
}

/// The two tail pointers, for the probe's own data plane.
struct TailRegs(u64);

impl Tails for TailRegs {
    fn set_rx_tail(&mut self, addr: u32) {
        Window(self.0).write(regs::CHAN_RX_TAIL, addr);
    }
    fn set_tx_tail(&mut self, addr: u32) {
        Window(self.0).write(regs::CHAN_TX_TAIL, addr);
    }
    fn barrier(&mut self) {
        crate::arch::direct_memory_access_write_barrier();
    }
}

/// The DMA region, through the direct map: ordinary cached memory, which is the point of the probe.
struct Plane(u64);

impl Plane {
    fn at(&self, off: u64, len: usize) -> u64 {
        assert!(off + len as u64 <= layout::BYTES);
        self.0 + off
    }
}

impl Region for Plane {
    fn read_u64(&self, off: u64) -> u64 {
        // SAFETY: `at` bounds the access to the region, which is mapped normal memory in the
        // direct map; descriptor offsets are 8-aligned.
        unsafe { core::ptr::read_volatile(self.at(off, 8) as *const u64) }
    }
    fn write_u64(&mut self, off: u64, v: u64) {
        // SAFETY: as `read_u64`.
        unsafe { core::ptr::write_volatile(self.at(off, 8) as *mut u64, v) }
    }
    fn read_bytes(&self, off: u64, out: &mut [u8]) {
        let base = self.at(off, out.len());
        for (i, b) in out.iter_mut().enumerate() {
            // SAFETY: as `read_u64`, one byte at a time inside the bounded span.
            *b = unsafe { core::ptr::read_volatile((base + i as u64) as *const u8) };
        }
    }
    fn write_bytes(&mut self, off: u64, src: &[u8]) {
        let base = self.at(off, src.len());
        for (i, &b) in src.iter().enumerate() {
            // SAFETY: as `read_bytes`.
            unsafe { core::ptr::write_volatile((base + i as u64) as *mut u8, b) };
        }
    }
}

fn spin_ms(ms: u64) {
    crate::arch::timer::spin_for(ms * crate::arch::timer::frequency() / 1000);
}
