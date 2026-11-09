//! The `e1000e` NIC's control plane: find the controller, confine its DMA to one region, reset
//! it, read its MAC address, program both rings' bases and enable receive and transmit.
//!
//! The process (`net_stack`) is handed BAR0's two queue pages and the region; this file keeps what
//! it must not do, on pages 0 and 5. A ring base shares a page with its tail, so the IOMMU domain
//! is the whole DMA bound. Every offset and layout is `nifeos/e1000e`'s. See notes/e1000e.md.

//! # BUGS
//! - Two bring-ups share this file (the 82574L's and the I219's), and QEMU proves only one.
//! - [`frame_counts`] reads counters that clear on read, so two callers would split one count
//!   between them; the bench boot and the DHCP gate are its only callers, one per boot.
//! - Waiting for link is a spin of up to [`LINK_WAIT_MS`], holding a core once per wiring.
//! - The DMA region is never returned: a wired boot keeps [`::e1000e::layout::DIRECT_MEMORY_ACCESS_REGION_PAGES`] pages.

use core::sync::atomic::{AtomicU64, Ordering};

use e1000e::{layout, registers};

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
    NoStationAddress,
    /// An I219 bring-up pass stopped (`nifeos/e1000e/src/platform_controller_hub/sequence.rs`).
    Pch(::e1000e::platform_controller_hub::sequence::Error),
    /// The PHY identifier could not be read over MDIO.
    PhysicalLayerDeviceId(::e1000e::platform_controller_hub::physical_layer_device::Error),
}

/// Why there is no NIC to hand out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Absent {
    /// No function on the bus is one `nifeos/e1000e` claims.
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
    /// BAR0's physical base. The process is mapped `bar0 + RECEIVE_QUEUE_PAGE` and
    /// `+ TRANSMIT_QUEUE_PAGE`.
    pub bar0: u64,
    /// The requester id the IOMMU confines.
    pub rid: u32,
    /// The PCI device id.
    pub device: u16,
    /// What the process is told: the MAC address and the region's physical base.
    pub handoff: ::driver_process_contract::e1000e::Handoff,
    /// Whether `STATUS.LU` was set before [`LINK_WAIT_MS`] ran out.
    pub link_up: bool,
    /// The PHY's identifier, revision masked off, read over MDIO. On the 82574L this is the read
    /// that proves [`::e1000e::platform_controller_hub::physical_layer_device::read_management_data_input_output`] under QEMU.
    pub physical_layer_device_id: u32,
}

/// The region's physical base, once allocated: zero until the first bring-up.
static REGION: AtomicU64 = AtomicU64::new(0);

/// BAR0's physical base of the last controller brought up: zero until the first bring-up.
static BRING_UP_BAR0: AtomicU64 = AtomicU64::new(0);

/// What the device's own statistics counters say crossed the wire since the last read, or since
/// [`bring_up`] for the first read after it. They count independently of the rings, so they are
/// the witness of whether frames passed in each direction.
///
/// Name: provisional, minted 2026-11-09 (UTC) by lane/i219-cabled-run.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FrameCounts {
    /// Good frames received (`GPRC`).
    pub received: u32,
    /// Good frames transmitted (`GPTC`).
    pub transmitted: u32,
    /// Received frames with a bad cyclic redundancy check (`CRCERRS`).
    pub cyclic_redundancy_check_errors: u32,
    /// Received frames the device missed (`MPC`).
    pub missed: u32,
}

/// Two reads summed, each field saturating: the counters clear on read, so a total is a sum.
impl core::ops::Add for FrameCounts {
    type Output = FrameCounts;
    fn add(self, later: FrameCounts) -> FrameCounts {
        FrameCounts {
            received: self.received.saturating_add(later.received),
            transmitted: self.transmitted.saturating_add(later.transmitted),
            cyclic_redundancy_check_errors: self
                .cyclic_redundancy_check_errors
                .saturating_add(later.cyclic_redundancy_check_errors),
            missed: self.missed.saturating_add(later.missed),
        }
    }
}

/// Read, and so clear, the four counters of the controller [`bring_up`] last brought up; `None`
/// before any bring-up. The counters sit on BAR0's page `0x4000`, which the process is not mapped,
/// so reading them here races nothing it does.
///
/// Name: provisional, minted 2026-11-09 (UTC) by lane/i219-cabled-run.
pub fn frame_counts() -> Option<FrameCounts> {
    let bar0 = BRING_UP_BAR0.load(Ordering::Acquire);
    (bar0 != 0).then(|| read_frame_counts(&Controller(mmu::phys_to_virt(bar0))))
}

fn read_frame_counts(c: &Controller) -> FrameCounts {
    FrameCounts {
        received: c.r(registers::GOOD_PACKETS_RECEIVED_COUNT_REGISTER),
        transmitted: c.r(registers::GOOD_PACKETS_TRANSMITTED_COUNT_REGISTER),
        cyclic_redundancy_check_errors: c
            .r(registers::CYCLIC_REDUNDANCY_CHECK_ERROR_COUNT_REGISTER),
        missed: c.r(registers::MISSED_PACKET_COUNT_REGISTER),
    }
}

/// Find, confine, reset and program the NIC, or return why not.
///
/// A second call resets the device under the process that held the first, so the service that
/// calls this refuses while that process lives (`e1000e_service`'s `BUGS`).
pub fn bring_up() -> Result<Found, Absent> {
    let dev = crate::pci::find_e1000e_device().ok_or(Absent::NoController)?;
    let region = region_for(dev.rid);
    let c = Controller(mmu::phys_to_virt(dev.bar0));
    let refused = |why| Absent::Refused { rid: dev.rid, why };
    let pch = ::e1000e::platform_controller_hub::is_platform_controller_hub_device(dev.device);
    let mut hw = Bar { c: &c, dev: &dev };
    let mut physical_layer_device_id = None;
    if pch {
        // An I219 behind a PCH: FreeBSD's passes, in FreeBSD's order; QEMU's 82574L never comes
        // here.
        use ::e1000e::platform_controller_hub::sequence;
        pass_begins(1, "initialize_physical_layer_device_workarounds");
        let id = sequence::initialize_physical_layer_device_workarounds(&mut hw)
            .map_err(|e| refused(Error::Pch(e)))?;
        physical_layer_device_id = Some(id);
        flush_descriptor_rings(&mut hw, &dev, region);
        pass_begins(2, "global_reset");
        sequence::global_reset(&mut hw, RESET_WAIT_MS).map_err(|e| refused(Error::Pch(e)))?;
    } else {
        reset_82574(&c).map_err(refused)?;
    }

    // Reset re-enabled nothing we want; mask again and clear anything pending.
    c.w(registers::INTERRUPT_CLEAR_MASK_REGISTER, u32::MAX);
    let _ = c.r(registers::INTERRUPT_CAUSE_READ_REGISTER);
    zero(region);

    // Tell management firmware a driver has the device, and bring the link up.
    c.w(
        registers::EXTENDED_DEVICE_CONTROL_REGISTER,
        c.r(registers::EXTENDED_DEVICE_CONTROL_REGISTER)
            | ::e1000e::EXTENDED_DEVICE_CONTROL_DRIVER_LOAD,
    );
    if pch {
        // FreeBSD's `e1000_init_hw_ich8lan`, whose copper-link setup sets `CTRL.SLU` itself.
        pass_begins(3, "initialize_hardware");
        ::e1000e::platform_controller_hub::sequence::initialize_hardware(&mut hw)
            .map_err(|e| refused(Error::Pch(e)))?;
        crate::println!("  e1000e: the three passes returned");
    } else {
        c.w(
            registers::DEVICE_CONTROL_REGISTER,
            c.r(registers::DEVICE_CONTROL_REGISTER)
                | ::e1000e::DEVICE_CONTROL_SET_LINK_UP
                | ::e1000e::DEVICE_CONTROL_AUTO_SPEED_DETECTION_ENABLE,
        );
    }

    let station_address = ::e1000e::station_mac_address_from_receive_words(
        c.r(registers::RECEIVE_MAC_ADDRESS_0_LOW_REGISTER),
        c.r(registers::RECEIVE_MAC_ADDRESS_0_HIGH_REGISTER),
    )
    .ok_or(refused(Error::NoStationAddress))?;

    // No multicast groups: the table is unspecified after reset.
    for k in 0..registers::MULTICAST_TABLE_ARRAY_ENTRIES {
        c.w(registers::MULTICAST_TABLE_ARRAY_REGISTER + 4 * k, 0);
    }
    // Legacy receive descriptors, the only format `nifeos/e1000e` reads.
    c.w(
        registers::RECEIVE_FILTER_CONTROL_REGISTER,
        c.r(registers::RECEIVE_FILTER_CONTROL_REGISTER)
            & !::e1000e::RECEIVE_FILTER_CONTROL_EXTENDED_DESCRIPTORS_ENABLE,
    );

    program_rings(&c, region);

    c.w(
        registers::TRANSMIT_INTER_PACKET_GAP_REGISTER,
        ::e1000e::transmit_inter_packet_gap_register_value(),
    );
    c.w(
        registers::TRANSMIT_CONTROL_REGISTER,
        ::e1000e::transmit_control_register_value(),
    );
    if pch {
        ::e1000e::platform_controller_hub::sequence::transmit_errata(&mut hw);
    }
    // Discard whatever the statistics counters hold from before, so the first read after this one
    // counts only what this bring-up's rings carry.
    let _ = read_frame_counts(&c);
    BRING_UP_BAR0.store(dev.bar0, Ordering::Release);
    c.w(
        registers::RECEIVE_CONTROL_REGISTER,
        ::e1000e::receive_control_register_value(),
    );

    // The 82574L's PHY answers at address 1 with no pages and no semaphore.
    let physical_layer_device_id = match physical_layer_device_id {
        Some(id) => id,
        None => {
            ::e1000e::platform_controller_hub::physical_layer_device::read_identifier_at_device_number(
                &mut hw, 1,
            )
            .map_err(|e| refused(Error::PhysicalLayerDeviceId(e)))?
        }
    };

    let link_up = wait_ms(LINK_WAIT_MS, || {
        c.r(registers::DEVICE_STATUS_REGISTER) & ::e1000e::DEVICE_STATUS_LINK_UP != 0
    });

    Ok(Found {
        bar0: dev.bar0,
        rid: dev.rid,
        device: dev.device,
        handoff: ::driver_process_contract::e1000e::Handoff {
            station_address,
            data_plane_physical_address: region,
        },
        link_up,
        physical_layer_device_id,
    })
}

/// The console line before each I219 pass, so a refusal or a hang on the bench names its pass: the
/// last `pass N` line above it.
fn pass_begins(n: u32, name: &str) {
    crate::println!("  e1000e: pass {n} of 3: {name}");
}

/// Point the device at both rings, with every head and tail at zero.
fn program_rings(c: &Controller, region: u64) {
    // Heads and tails at zero: the process's first act is to post receive buffers by the tail.
    let rx = region + layout::RECEIVE_RING_OFFSET_BYTES;
    let tx = region + layout::TRANSMIT_RING_OFFSET_BYTES;
    c.w(
        registers::RECEIVE_DESCRIPTOR_BASE_PHYSICAL_ADDRESS_LOW_REGISTER,
        rx as u32,
    );
    c.w(
        registers::RECEIVE_DESCRIPTOR_BASE_PHYSICAL_ADDRESS_HIGH_REGISTER,
        (rx >> 32) as u32,
    );
    c.w(
        registers::RECEIVE_DESCRIPTOR_LENGTH_BYTES_REGISTER,
        u32::from(::e1000e::RECEIVE_ENTRIES) * 16,
    );
    c.w(registers::RECEIVE_DESCRIPTOR_HEAD_REGISTER, 0);
    c.w(registers::RECEIVE_DESCRIPTOR_TAIL_REGISTER, 0);
    c.w(
        registers::TRANSMIT_DESCRIPTOR_BASE_PHYSICAL_ADDRESS_LOW_REGISTER,
        tx as u32,
    );
    c.w(
        registers::TRANSMIT_DESCRIPTOR_BASE_PHYSICAL_ADDRESS_HIGH_REGISTER,
        (tx >> 32) as u32,
    );
    c.w(
        registers::TRANSMIT_DESCRIPTOR_LENGTH_BYTES_REGISTER,
        u32::from(::e1000e::TRANSMIT_ENTRIES) * 16,
    );
    c.w(registers::TRANSMIT_DESCRIPTOR_HEAD_REGISTER, 0);
    c.w(registers::TRANSMIT_DESCRIPTOR_TAIL_REGISTER, 0);
}

/// **The 82574L's quiesce and global reset**, unchanged from before the I219 passes existed.
fn reset_82574(c: &Controller) -> Result<(), Error> {
    // Quiesce: no interrupts, receiver and transmitter off, then a posted-write flush.
    c.w(registers::INTERRUPT_CLEAR_MASK_REGISTER, u32::MAX);
    c.w(registers::RECEIVE_CONTROL_REGISTER, 0);
    c.w(registers::TRANSMIT_CONTROL_REGISTER, 0);
    let _ = c.r(registers::DEVICE_STATUS_REGISTER);
    spin_ms(10);
    // Global reset. Self-clearing; the registers read garbage while it runs, so wait first.
    c.w(
        registers::DEVICE_CONTROL_REGISTER,
        c.r(registers::DEVICE_CONTROL_REGISTER) | ::e1000e::DEVICE_CONTROL_GLOBAL_RESET,
    );
    spin_ms(20);
    if !wait_ms(RESET_WAIT_MS, || {
        c.r(registers::DEVICE_CONTROL_REGISTER) & ::e1000e::DEVICE_CONTROL_GLOBAL_RESET == 0
    }) {
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
    let base =
        crate::memory::alloc_contiguous_zeroed(layout::DIRECT_MEMORY_ACCESS_REGION_PAGES as usize)
            .expect("no DMA region for the e1000e NIC")
            .addr();
    if crate::iommu::is_active() {
        crate::iommu::confine(
            rid,
            &[paging::domain::DmaRegion {
                base,
                size: layout::DIRECT_MEMORY_ACCESS_REGION_PAGES * page_frames::FRAME_SIZE,
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
    // SAFETY: `region` is the run `region_for` allocated, `layout::DIRECT_MEMORY_ACCESS_REGION_BYTES` long, in the direct map.
    // The device was reset just before this, so it is not DMA-ing into it, and the service refuses
    // a new wiring while the previous process lives, so no process has it mapped.
    unsafe {
        core::ptr::write_bytes(
            mmu::phys_to_virt(region) as *mut u8,
            0,
            layout::DIRECT_MEMORY_ACCESS_REGION_BYTES as usize,
        );
    }
}

/// Empty the descriptor rings before the reset if the hardware says it must. The sequence, its
/// source and its notice are `nifeos/e1000e`'s (`platform_controller_hub::flush::empty_descriptor_rings`);
/// this supplies the configuration space and the ring memory it runs against.
fn flush_descriptor_rings(hw: &mut Bar<'_>, dev: &crate::pci::PciE1000eDevice, region: u64) {
    use ::e1000e::platform_controller_hub::flush;
    let mut rings = Rings { dev, region };
    let ring = region + layout::TRANSMIT_RING_OFFSET_BYTES;
    if let flush::EmptiedRings::ForeignTransmitRing { base } =
        flush::empty_descriptor_rings(hw, &mut rings, ring)
    {
        crate::println!(
            "  e1000e: the rings need flushing and the transmit ring at {base:#x} is not this \
             driver's; skipped, so the reset may hang the device"
        );
    }
}

/// What the descriptor-ring flush reaches beyond BAR0: the function's configuration space and this
/// driver's transmit ring in `region`.
struct Rings<'a> {
    dev: &'a crate::pci::PciE1000eDevice,
    region: u64,
}

impl ::e1000e::platform_controller_hub::flush::FlushSupport for Rings<'_> {
    fn descriptor_ring_status(&mut self) -> u16 {
        use ::e1000e::platform_controller_hub::flush::PCI_CONFIGURATION_DESCRIPTOR_RING_STATUS_REGISTER;
        self.dev
            .config_read32(PCI_CONFIGURATION_DESCRIPTOR_RING_STATUS_REGISTER) as u16
    }
    fn write_transmit_descriptor(&mut self, index: u16, words: [u64; 2]) {
        let d = mmu::phys_to_virt(self.region + layout::transmit_descriptor(index)) as *mut u64;
        // SAFETY: `d` is a 16-byte descriptor inside the transmit ring page of this driver's region
        // (`layout::transmit_descriptor` reduces its index), in the direct map. The device reads it
        // only after the tail write that follows, which the barrier orders after these stores.
        unsafe {
            core::ptr::write_volatile(d, words[0]);
            core::ptr::write_volatile(d.add(1), words[1]);
        }
        crate::arch::direct_memory_access_write_barrier();
    }
}

/// BAR0, through the direct map.
struct Controller(u64);

/// The largest offset any `nifeos/e1000e` constant reaches, plus the access: the flash data
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

/// What `nifeos/e1000e`'s I219 sequences and MDIO layer run against: BAR0, configuration space
/// (the vendor-id read the reset uses as a delay), the timer, and the console.
struct Bar<'a> {
    c: &'a Controller,
    dev: &'a crate::pci::PciE1000eDevice,
}

impl ::e1000e::platform_controller_hub::Hardware for Bar<'_> {
    fn read(&mut self, off: u64) -> u32 {
        self.c.r(off)
    }
    fn write(&mut self, off: u64, v: u32) {
        self.c.w(off, v);
    }
    fn read_flash_status(&mut self, off: u64) -> u16 {
        self.c.r16(off)
    }
    fn delay_microseconds(&mut self, microseconds: u64) {
        spin_microseconds(microseconds);
    }
    fn pci_vendor_id(&mut self) -> u16 {
        self.dev.config_read32(0) as u16
    }
    fn note(&mut self, n: ::e1000e::platform_controller_hub::sequence::BringUpObservation) {
        use ::e1000e::platform_controller_hub::sequence::BringUpObservation;
        match n {
            // The identifier in hex, as the bench card compares it.
            BringUpObservation::PhysicalLayerDeviceReached { at, id } => {
                crate::println!(
                    "  e1000e: PhysicalLayerDeviceReached {{ at: {at:?}, id: {id:#010x} }}"
                );
            }
            n => crate::println!("  e1000e: {n:?}"),
        }
    }
}

fn spin_microseconds(microseconds: u64) {
    crate::arch::timer::spin_for(microseconds * crate::arch::timer::frequency() / 1_000_000);
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
