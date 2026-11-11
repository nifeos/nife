//! The JH7110 Ethernet port's two processes. The kernel finds the port in the tree, has the clock
//! and reset server bring its clocks and resets up, and allocates the DMA region. The control
//! process (`designware_ethernet_control`) then does the rest of the bring-up and reports; on a
//! link, `net_stack` starts over the DMA region and the controller's DMA page (`0x1000` to
//! `0x1fff`), without the MAC page or an interrupt. See notes/designware-ethernet.md, "The
//! confinement, stated plainly".

//! # Scope note (rule 5, architectural parity)
//!
//! riscv64-only on purpose: no aarch64 or x86_64 machine nife runs on has this controller. The
//! network stack above it is shared, through a `Handoff` of the same shape as `e1000e`'s (both in
//! `driver_process_contract`). A second board with Synopsys's Ethernet `QoS` controller (the RK3568
//! and RK3588 are aarch64) would reuse the control process and the crate, and add its own glue
//! beside `designware_ethernet::jh7110`.

//! # BUGS
//!
//! - The JH7110 has no IOMMU here, so both processes can aim the device's DMA anywhere, and the
//!   DMA page holds the controller's reset bit: both run trusted (§86 (whether an NVMe driver can
//!   leave the kernel)).
//! - One wiring at a time: a second bring-up resets the device under the first.
//! - The kernel maps `net_stack`'s pages at its spawn, as `e1000e_service` does, rather than the
//!   control process granting them as the NVMe admin process does.
//! - A control process that dies before its last record leaves the starter waiting for it.
//! - The DMA region and the control process's stack are allocated once per boot and kept.
//! - The moved bring-up has not run on silicon ([`PROVEN_ON_SILICON`]).

use core::sync::atomic::{AtomicU64, Ordering};

use ::driver_process_contract::designware_ethernet::{
    self as contract, ACCOUNT_RECORDS, ALWAYS_ON_SYSTEM_CONTROLLER_VIRTUAL_ADDRESS,
    CONTROL_REGISTER_PAGES, CONTROL_REGISTER_VIRTUAL_ADDRESS, ControlRecord, ControlReport,
    DATA_PLANE_VIRTUAL_ADDRESS, DIRECT_MEMORY_ACCESS_REGISTER_PAGE_VIRTUAL_ADDRESS, Handoff,
};
use contract::device::bring_up::{self as port_bring_up, Inputs};
use contract::device::motorcomm_physical_layer_device::Link;
use contract::device::{layout, registers};

use super::*;
use crate::capability_object::{
    Rights, memory_region_capability, notification_capability, rendezvous_capability,
    timer_capability,
};
use crate::sched::RendezvousId;
use crate::user::holding::Holding;
use crate::user::jh7110_clock_and_reset_service::BroughtUp;
use crate::user::virtio_service::{NET_SERVER_BUDGET_PAGES, NET_SERVER_STACK_PAGES};

/// Why a port the tree describes was not brought up.
///
/// Name: provisional. The kernel's own module's `Error`, which moved here with the starter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// `phy-mode` names no RGMII variant, so there is no interface select to make.
    NotRgmii,
    /// No clock and reset server runs to bring the port's clocks and resets up.
    NoClockServer,
    /// A clock's enable bit did not read back: the window is not the controller it was named as.
    ClocksNotRunning,
    /// A reset did not read as released before the clock driver's poll gave up.
    ResetsHeld,
    /// The DMA region could not be allocated.
    NoRegion,
    /// The tree's PHY settings do not fit the control process's spawn word.
    SettingsUnrepresentable,
    /// The image carries no `designware_ethernet_control`.
    NoControlProgram,
    /// The control process sent a record no control process sends, or sent one out of order.
    ControlReportUnreadable,
    /// The control process refused the port.
    Port(port_bring_up::Error),
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
    /// What the clock and reset server answered for the port's SYS and AON clocks and resets.
    pub clocks: Option<BroughtUp>,
    /// What the control process reported, once it has.
    pub control: Option<ControlReport>,
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
    /// What `net_stack` is told: the station address and the region's physical base.
    pub handoff: Handoff,
    /// The controller's DMA page, physical: the one page of the window `net_stack` is mapped.
    pub dma_page: u64,
    /// The link, if one resolved. The controller is started only when one did.
    pub link: Option<Link>,
}

/// The region's physical base, once allocated: zero until the first bring-up.
static REGION: AtomicU64 = AtomicU64::new(0);

/// Stack pages the control process gets beyond the one every loaded program gets, as the NVMe
/// admin process has: its bring-up carries the probe frame and the account by value.
const CONTROL_EXTRA_STACK_PAGES: usize = 3;

/// The control process's extra stack frames, once allocated: zero until the first bring-up.
static CONTROL_STACK: [AtomicU64; CONTROL_EXTRA_STACK_PAGES] =
    [const { AtomicU64::new(0) }; CONTROL_EXTRA_STACK_PAGES];

/// The last control process's thread, so a bring-up refuses while it lives.
static CONTROL: AtomicU64 = AtomicU64::new(u64::MAX);

/// The last bring-up's account, refused or not: one boot artifact, read by the bench boot's lines.
static LAST_REPORT: crate::sync::InterruptSafeMutex<Option<Report>> =
    crate::sync::InterruptSafeMutex::new(crate::sync::rank::RAM, None);

/// **What the last bring-up saw**, as far as it got. `None` before any.
#[cfg_attr(not(feature = "network_bench"), allow(dead_code))] // the bench boot is its reader
pub(crate) fn last_report() -> Option<Report> {
    *LAST_REPORT.lock()
}

/// **Bring the first port up**: the clocks and resets through their server, then the control
/// process. A second call resets the controller under whatever process held the first, so
/// [`start_net_server`] refuses while that process lives.
fn bring_up() -> Result<Found, Absent> {
    let e = crate::memory::jh7110_ethernet().ok_or(Absent::NoController)?;
    let mut report = Report {
        base: e.port.registers_physical_address,
        compatible: e.port.compatible,
        windows_from_tree: e.aon.from_tree && e.sys.from_tree && e.syscon.from_tree,
        clocks: None,
        control: None,
    };
    let outcome = bring_up_port(&e, &mut report);
    *LAST_REPORT.lock() = Some(report);
    let (handoff, link) = outcome.map_err(Absent::Refused)?;
    Ok(Found {
        handoff,
        dma_page: e.port.registers_physical_address
            + u64::from(registers::DIRECT_MEMORY_ACCESS_REGISTER_PAGE),
        link,
    })
}

/// The kernel's part of the bring-up, then the control process's; see this module's header.
fn bring_up_port(
    e: &crate::memory::Jh7110Ethernet,
    r: &mut Report,
) -> Result<(Handoff, Option<Link>), Error> {
    if !e.port.reduced_gigabit_media_independent_interface {
        return Err(Error::NotRgmii);
    }
    // Clocks, then resets, across both domains: the clock and reset server takes the order from
    // the port's plan, for this port's badge alone (§320 (the JH7110 clock and reset wire)).
    let brought =
        crate::user::jh7110_clock_and_reset_service::bring_up(e.port.registers_physical_address)
            .ok_or(Error::NoClockServer)?;
    r.clocks = Some(brought);
    match brought.status {
        jh7110_clock_and_reset_contract::Status::Done => {}
        jh7110_clock_and_reset_contract::Status::ResetHeld => return Err(Error::ResetsHeld),
        _ => return Err(Error::ClocksNotRunning),
    }
    let region = region()?;
    let inputs = Inputs {
        region_physical_address: region,
        station_address: e.port.station_address,
        physical_layer_device_address: e.port.physical_layer_device_address,
        physical_layer_device: e.port.physical_layer_device,
    };
    let control = start_control(e, inputs)?;
    r.control = Some(control.0);
    let linked = control.1.map_err(Error::Port)?;
    let account = control.0.account;
    let station_address = account
        .station_address
        .ok_or(Error::ControlReportUnreadable)?
        .0;
    // A report that says it linked and carries no link, or the reverse, is not this program's.
    if linked != account.link.is_some() {
        return Err(Error::ControlReportUnreadable);
    }
    Ok((
        Handoff {
            station_address,
            data_plane_physical_address: region,
        },
        account.link,
    ))
}

/// The DMA region: allocated on the first call, the same region on every later one.
fn region() -> Result<u64, Error> {
    let base = REGION.load(Ordering::Acquire);
    if base != 0 {
        return Ok(base);
    }
    let base =
        crate::memory::alloc_contiguous_zeroed(layout::DIRECT_MEMORY_ACCESS_REGION_PAGES as usize)
            .ok_or(Error::NoRegion)?
            .addr();
    REGION.store(base, Ordering::Release);
    Ok(base)
}

/// The control process's extra stack: allocated on the first call, the same frames on every
/// later one, which is sound because [`start_net_server`] refuses while the last one lives.
fn control_stack() -> Result<[u64; CONTROL_EXTRA_STACK_PAGES], Error> {
    let mut frames = [0u64; CONTROL_EXTRA_STACK_PAGES];
    for (frame, kept) in frames.iter_mut().zip(&CONTROL_STACK) {
        *frame = match kept.load(Ordering::Acquire) {
            0 => {
                let fresh = crate::memory::alloc_zeroed().ok_or(Error::NoRegion)?.addr();
                kept.store(fresh, Ordering::Release);
                fresh
            }
            known => known,
        };
    }
    Ok(frames)
}

const PLANE: usize = layout::DIRECT_MEMORY_ACCESS_REGION_PAGES as usize;
const CONTROL_MAPS: usize = CONTROL_REGISTER_PAGES as usize + 1 + PLANE + CONTROL_EXTRA_STACK_PAGES;

/// **Start the control process and read its report**: the controller's first two pages and the
/// always-on syscon page device-typed, the region as normal memory, and one endpoint to report
/// on. It holds no capability to any of the pages, so it cannot pass them on.
fn start_control(
    e: &crate::memory::Jh7110Ethernet,
    inputs: Inputs,
) -> Result<(ControlReport, Result<bool, port_bring_up::Error>), Error> {
    let [arg0, arg1, arg2] =
        contract::control_spawn_words(&inputs).ok_or(Error::SettingsUnrepresentable)?;
    let image =
        crate::user::program("designware_ethernet_control").ok_or(Error::NoControlProgram)?;
    let stack = control_stack()?;
    let base = e.port.registers_physical_address;
    let syscon_page = e.syscon.base_physical_address & !(FRAME_SIZE - 1);
    let region = inputs.region_physical_address;
    let report = crate::sched::create_rendezvous();

    let mut maps = [Mapping {
        va: 0,
        phys: 0,
        flags: Flags::user_data(),
    }; CONTROL_MAPS];
    // Device-typed, because a cached or reordered register write is one the controller never sees.
    for (page, m) in maps
        .iter_mut()
        .take(CONTROL_REGISTER_PAGES as usize)
        .enumerate()
    {
        *m = Mapping {
            va: CONTROL_REGISTER_VIRTUAL_ADDRESS + page as u64 * FRAME_SIZE,
            phys: base + page as u64 * FRAME_SIZE,
            flags: Flags::user_device(),
        };
    }
    let syscon_at = CONTROL_REGISTER_PAGES as usize;
    maps[syscon_at] = Mapping {
        va: ALWAYS_ON_SYSTEM_CONTROLLER_VIRTUAL_ADDRESS,
        phys: syscon_page,
        flags: Flags::user_device(),
    };
    super::fs_service::map_channel(
        &mut maps[syscon_at + 1..syscon_at + 1 + PLANE],
        DATA_PLANE_VIRTUAL_ADDRESS,
        region,
        PLANE,
    );
    for (k, &frame) in stack.iter().enumerate() {
        maps[syscon_at + 1 + PLANE + k] = Mapping {
            va: USER_STACK_VA - (k as u64 + 1) * FRAME_SIZE,
            phys: frame,
            flags: Flags::user_data(),
        };
    }

    const _: () = assert!(contract::CONTROL_REPORT_SLOT == 0);
    let tid = crate::sched::spawn(move || {
        run(
            image,
            Spawn {
                arg0, // the PHY's settings word
                arg1, // the region's physical base and the PHY's MDIO address
                arg2, // the tree's station address, if any
                grants: &[rendezvous_capability(report, Rights::WRITE)],
                maps: &maps,
                windows: &[
                    startup_page_contract::Window {
                        kind: contract::CONTROL_REGISTER_KIND,
                        virtual_address: CONTROL_REGISTER_VIRTUAL_ADDRESS,
                        pages: CONTROL_REGISTER_PAGES as u32,
                    },
                    startup_page_contract::Window {
                        kind: contract::ALWAYS_ON_SYSTEM_CONTROLLER_KIND,
                        virtual_address: ALWAYS_ON_SYSTEM_CONTROLLER_VIRTUAL_ADDRESS,
                        pages: 1,
                    },
                    startup_page_contract::Window {
                        kind: contract::DATA_PLANE_KIND,
                        virtual_address: DATA_PLANE_VIRTUAL_ADDRESS,
                        pages: PLANE as u32,
                    },
                ],
            },
        )
    })
    .expect("could not spawn the JH7110 Ethernet control process");
    CONTROL.store(tid, Ordering::Release);

    let mut words = [0u64; 2 * ACCOUNT_RECORDS];
    for expected in 0..ACCOUNT_RECORDS {
        let [w0, w1, w2, ..] = crate::sched::ipc_receive(report);
        match ControlRecord::unpack([w0, w1, w2]) {
            Some(ControlRecord::Part { index, words: pair }) if usize::from(index) == expected => {
                words[2 * expected..2 * expected + 2].copy_from_slice(&pair);
            }
            _ => return Err(Error::ControlReportUnreadable),
        }
    }
    let control = ControlReport::from_words(&words).ok_or(Error::ControlReportUnreadable)?;
    let [w0, w1, w2, ..] = crate::sched::ipc_receive(report);
    match ControlRecord::unpack([w0, w1, w2]) {
        Some(ControlRecord::Done(outcome)) => Ok((control, outcome)),
        _ => Err(Error::ControlReportUnreadable),
    }
}

/// Why no server was started.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NotStarted {
    /// What the bring-up said.
    Absent(Absent),
    /// The previous server is still alive, and a bring-up would reset the controller under it.
    Busy,
    /// The port came up and no link resolved, so the controller was not started.
    NoLink,
}

/// What a wiring started, from the outside.
pub(crate) struct Wiring {
    /// Where the server reports its DHCP lease, once.
    pub report: RendezvousId,
    /// The `Stack` endpoint a client is given WRITE on.
    pub stack: RendezvousId,
    /// Everything to give back when the caller is done.
    pub held: Holding,
    /// The DHCP lease, once something has drained [`Wiring::report`].
    pub lease: u32,
}

/// Whether the booted system may bring this port up on its own. `false`: the bring-up that
/// `bench/radon-2026-10-20/network-bench-3.log` proved ran in the kernel, and the control process
/// that now runs it has not run on silicon. No emulator has this controller. Set it to `true` on
/// a radon bench boot of this path that ends `verdict LEASED-AND-MEASURED` with
/// `coherence : Coherent`; until then the booted system leaves the port as U-Boot left it.
pub(crate) const PROVEN_ON_SILICON: bool = false;

/// The last `net_stack`'s thread, so a second wiring can refuse while it lives.
static SERVER: AtomicU64 = AtomicU64::new(u64::MAX);

/// Bring the port up and start `net_stack` over it, with `listen_grant` as the inbound ports.
pub(crate) fn start_net_server(
    image: &'static [u8],
    listen_grant: u64,
) -> Result<Wiring, NotStarted> {
    for holder in [&SERVER, &CONTROL] {
        let previous = holder.load(Ordering::Acquire);
        if previous != u64::MAX && crate::sched::is_thread_present(previous) {
            return Err(NotStarted::Busy);
        }
    }
    let found = bring_up().map_err(NotStarted::Absent)?;
    if found.link.is_none() {
        return Err(NotStarted::NoLink);
    }
    Ok(spawn_server(image, listen_grant, found))
}

/// The bench boot's client (`kernel/src/network_bench.rs`): `socket_test_client`'s
/// `TEST_TCP_DRAIN` against `peer`, packed `ipv4 << 16 | port`, as `e1000e_service::start_drain`.
#[cfg_attr(not(feature = "network_bench"), allow(dead_code))]
pub(crate) fn start_drain(
    image: &'static [u8],
    peer: u64,
) -> Result<(RendezvousId, Wiring), NotStarted> {
    const TEST_TCP_DRAIN: u64 = 7;
    let mut w = start_net_server(image, socket_contract::NO_LISTEN_GRANT)?;
    let cli = super::virtio_service::spawn_stack_client(
        image,
        TEST_TCP_DRAIN,
        peer,
        None,
        w.stack,
        &mut w.held,
    );
    w.lease = crate::sched::ipc_receive(w.report)[0] as u32;
    Ok((cli, w))
}

fn spawn_server(image: &'static [u8], listen_grant: u64, found: Found) -> Wiring {
    let [arg0, arg1] = found.handoff.pack();

    // The endpoints, the notification and the timer out of one region, so reclaiming it wakes a
    // server blocked on any of them, as in `virtio_service::wire_net_server`.
    let ep_region = crate::memory_region::create(5).expect("no endpoint region for net_stack");
    let report = crate::sched::create_rendezvous_from(ep_region).expect("no report endpoint");
    let stack = crate::sched::create_rendezvous_from(ep_region).expect("no stack endpoint");
    let wake = crate::sched::create_notification_from(ep_region).expect("no notification");
    let poll = crate::sched::create_timer_from(ep_region).expect("no poll timer");
    let budget =
        crate::memory_region::create(NET_SERVER_BUDGET_PAGES).expect("no untyped for net_stack");
    let stack_region = crate::memory_region::create(NET_SERVER_STACK_PAGES)
        .expect("no stack region for net_stack");

    const MAPS: usize = PLANE + 1 + NET_SERVER_STACK_PAGES as usize;
    let mut maps = [Mapping {
        va: 0,
        phys: 0,
        flags: Flags::user_data(),
    }; MAPS];
    super::fs_service::map_channel(
        &mut maps[..PLANE],
        DATA_PLANE_VIRTUAL_ADDRESS,
        found.handoff.data_plane_physical_address,
        PLANE,
    );
    // Device-typed, because a cached or reordered tail write is a frame the device never hears of.
    maps[PLANE] = Mapping {
        va: DIRECT_MEMORY_ACCESS_REGISTER_PAGE_VIRTUAL_ADDRESS,
        phys: found.dma_page,
        flags: Flags::user_device(),
    };
    for k in 0..NET_SERVER_STACK_PAGES as usize {
        let phys =
            crate::memory_region::retype_page(stack_region).expect("no frame for the net stack");
        maps[PLANE + 1 + k] = Mapping {
            va: USER_STACK_VA - (k as u64 + 1) * FRAME_SIZE,
            phys,
            flags: Flags::user_data(),
        };
    }

    let tid = crate::sched::spawn(move || {
        // Slots 3 to 6 at their names before `run` grants in order, so the report lands in 0 and
        // slots 1 and 2 stay empty, `e1000e_service`'s arrangement.
        let at = |slot, capability| {
            crate::sched::grant_at(slot, capability)
                .expect("a net_stack slot was already occupied");
        };
        at(3, memory_region_capability(budget));
        at(4, rendezvous_capability(stack, Rights::READ));
        at(5, notification_capability(wake, Rights::ALL));
        at(6, timer_capability(poll, Rights::WRITE));
        // What each socket is minted from (§255 (each socket is its own capability)); slot 8, the
        // server's own address space, `run_with_own_space` adds once the space exists.
        at(
            socket_contract::stack_slots::MINT,
            rendezvous_capability(stack, Rights::WRITE.union(Rights::GRANT)),
        );
        run_with_own_space(
            image,
            Spawn {
                arg0, // `driver_process_contract::designware_ethernet::Handoff`: the role tag and the MAC address
                arg1, // the DMA region's physical base
                arg2: listen_grant,
                grants: &[rendezvous_capability(report, Rights::WRITE)], // slot 0
                maps: &maps,
                windows: &[
                    startup_page_contract::Window {
                        kind: contract::DATA_PLANE_KIND,
                        virtual_address: DATA_PLANE_VIRTUAL_ADDRESS,
                        pages: PLANE as u32,
                    },
                    startup_page_contract::Window {
                        kind: contract::DIRECT_MEMORY_ACCESS_REGISTER_PAGE_KIND,
                        virtual_address: DIRECT_MEMORY_ACCESS_REGISTER_PAGE_VIRTUAL_ADDRESS,
                        pages: 1,
                    },
                ],
            },
            socket_contract::stack_slots::OWN_SPACE,
        )
    })
    .expect("could not spawn net_stack over the JH7110 Ethernet port");
    crate::sched::notification_bind(wake, tid).expect("bind the poll notification");
    SERVER.store(tid, Ordering::Release);

    let mut held = Holding::new();
    held.add_thread(tid);
    held.add_region(ep_region);
    held.add_region_after_death(budget);
    held.add_region_after_death(stack_region);
    Wiring {
        report,
        stack,
        held,
        lease: 0,
    }
}
