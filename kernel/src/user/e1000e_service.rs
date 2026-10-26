//! Wiring for `net_stack` over the `e1000e` NIC. The process driving the rings the kernel's
//! `e1000e` module programs holds two queue pages of BAR0, the DMA region and `net_stack`'s usual
//! slots, the interrupt and virtio transport left empty. Tests start it through
//! [`start_net_server`], the booted system through [`start_for_boot`]. See notes/e1000e.md, "What
//! the server holds".
//!
//! # BUGS
//!
//! - The ring base registers are on the queue pages, so the IOMMU is the whole DMA confinement.
//! - One wiring at a time ([`Absent::Busy`]): a test that keeps its holding blocks every later one.

use ::driver_process_contract::e1000e::{
    DATA_PLANE_VIRTUAL_ADDRESS, RX_QUEUE_VIRTUAL_ADDRESS, TX_QUEUE_VIRTUAL_ADDRESS,
};

use super::*;
use crate::cap::{Rights, memory_region_cap, notification_cap, rendezvous_cap, timer_cap};
use crate::sched::RendezvousId;
use crate::user::holding::Holding;
use crate::user::virtio_service::{NET_SERVER_BUDGET_PAGES, NET_SERVER_STACK_PAGES};

/// Why no server was started.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Absent {
    /// No NIC this driver claims is on the bus.
    NoController,
    /// One is, and bring-up refused it. A failure, never a skip.
    Refused {
        /// The controller's requester id.
        rid: u32,
        /// What went wrong.
        why: crate::e1000e::Error,
    },
    /// The previous server is still alive, and a bring-up would reset the device under it.
    Busy,
    /// Bring-up finished and `STATUS.LU` never rose within `e1000e::LINK_WAIT_MS`, so no server was
    /// started: `net_stack`'s DHCP loop has no bound and every caller blocks on its lease. Whether
    /// this is a skip or a failure is the caller's, which knows whether a link was promised.
    NoLink {
        /// The PCI device id.
        device: u16,
    },
}

/// What a wiring started, from the outside.
pub struct Wiring {
    /// Where the server reports its DHCP lease, once.
    pub report: RendezvousId,
    /// The `Stack` endpoint a client is given WRITE on.
    pub stack: RendezvousId,
    /// Everything to give back when the test is done.
    pub held: Holding,
    /// True when the IOMMU unit that owns the NIC's requester id translates it. False means the
    /// driver is as confined as its arithmetic.
    pub confined_by_iommu: bool,
    /// The MAC address the kernel read and handed over.
    pub mac: [u8; 6],
    /// The PCI device id.
    pub device: u16,
    /// The PHY's identifier as the kernel read it over MDIO, revision masked off.
    pub phy_id: u32,
    /// The DHCP lease `net_stack` reported, once something has drained [`Wiring::report`]: zero
    /// from [`start_net_server`], the address from the functions that start a client.
    pub lease: u32,
}

/// The last server's thread, so a second wiring can refuse while it lives. On a booted system with
/// the stack, that is the boot's own server, so a later bring-up there refuses rather than reset it.
static SERVER: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(u64::MAX);

/// Bring the NIC up and start `net_stack` over it. `listen_grant` is the virtio server's: the
/// inbound ports this stack may bind. Refuses with [`Absent`].
pub fn start_net_server(image: &'static [u8], listen_grant: u64) -> Result<Wiring, Absent> {
    refuse_if_busy()?;
    let found = bring_up()?;
    if !found.link_up {
        return Err(Absent::NoLink {
            device: found.device,
        });
    }
    Ok(spawn_server(image, listen_grant, found))
}

/// Why the booted system has no stack over this NIC, when one is on the bus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NotAtBoot {
    /// What [`start_net_server`] would have said.
    Absent(Absent),
    /// A part no gate has driven, an I219 or another PCH part ([`::e1000e::pch::is_pch`]), left
    /// untouched: its bring-up runs FreeBSD's I219 passes (`crates/e1000e/src/pch/sequence.rs`),
    /// unproven on silicon, and a hang there would take xenon's prompt. See
    /// [`PCH_PROVEN_ON_SILICON`].
    Unproven {
        /// The PCI device id.
        device: u16,
    },
}

/// The booted system's stack over this NIC: [`start_net_server`] with no listen grant, plus one
/// refusal a boot wants and a test does not ([`NotAtBoot::Unproven`]).
///
/// `kernel::user::boot_progenitor` calls it when there is no virtio-net NIC. The progenitor waits
/// for the lease before the prompt, so [`Absent::NoLink`] is what lets a boot with no link reach
/// `$`.
pub(crate) fn start_for_boot(image: &'static [u8]) -> Result<Wiring, NotAtBoot> {
    let dev = crate::pci::find_e1000e_device().ok_or(NotAtBoot::Absent(Absent::NoController))?;
    if ::e1000e::pch::is_pch(dev.device) && !PCH_PROVEN_ON_SILICON {
        return Err(NotAtBoot::Unproven { device: dev.device });
    }
    start_net_server(image, socket_contract::NO_LISTEN_GRANT).map_err(NotAtBoot::Absent)
}

/// Whether the booted system may bring up an I219 on its own, gated on silicon, not on review.
///
/// Flip it to `true` only in a commit that cites a xenon bench boot of the same I219 bring-up
/// ending `verdict LEASED-AND-MEASURED` (notes/e1000e.md, "calef's bench step on xenon", with its
/// Results row filled in). Nothing a lane can run proves it: QEMU's 82574L never takes the PCH
/// path, and the host simulation is this tree's reading of FreeBSD, not the device.
pub(crate) const PCH_PROVEN_ON_SILICON: bool = false;

fn refuse_if_busy() -> Result<(), Absent> {
    let previous = SERVER.load(core::sync::atomic::Ordering::Acquire);
    if previous != u64::MAX && crate::sched::is_thread_present(previous) {
        return Err(Absent::Busy);
    }
    Ok(())
}

fn bring_up() -> Result<crate::e1000e::Found, Absent> {
    crate::e1000e::bring_up().map_err(|e| match e {
        crate::e1000e::Absent::NoController => Absent::NoController,
        crate::e1000e::Absent::Refused { rid, why } => Absent::Refused { rid, why },
    })
}

fn spawn_server(image: &'static [u8], listen_grant: u64, found: crate::e1000e::Found) -> Wiring {
    use core::sync::atomic::Ordering;
    let confined_by_iommu = crate::iommu::scope_of(found.rid).is_confining();
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

    const PLANE: usize = ::e1000e::layout::PAGES as usize;
    const MAPS: usize = PLANE + 2 + NET_SERVER_STACK_PAGES as usize;
    let mut maps = [Mapping {
        va: 0,
        phys: 0,
        flags: Flags::user_data(),
    }; MAPS];
    super::fs_service::map_channel(
        &mut maps[..PLANE],
        DATA_PLANE_VIRTUAL_ADDRESS,
        found.handoff.data_plane_phys,
        PLANE,
    );
    // Device-typed, because a cached or reordered tail write is a frame the device never hears of.
    maps[PLANE] = Mapping {
        va: RX_QUEUE_VIRTUAL_ADDRESS,
        phys: found.bar0 + ::e1000e::RX_QUEUE_PAGE,
        flags: Flags::user_device(),
    };
    maps[PLANE + 1] = Mapping {
        va: TX_QUEUE_VIRTUAL_ADDRESS,
        phys: found.bar0 + ::e1000e::TX_QUEUE_PAGE,
        flags: Flags::user_device(),
    };
    for k in 0..NET_SERVER_STACK_PAGES as usize {
        let phys =
            crate::memory_region::retype_page(stack_region).expect("no frame for the net stack");
        maps[PLANE + 2 + k] = Mapping {
            va: USER_STACK_VA - (k as u64 + 1) * FRAME_SIZE,
            phys,
            flags: Flags::user_data(),
        };
    }

    let tid = crate::sched::spawn(move || {
        // Slots 3 to 6 at their names before `run` grants in order, so the report lands in 0 and
        // slots 1 and 2 stay empty: no interrupt, no virtio transport.
        let at = |slot, cap| {
            crate::sched::grant_at(slot, cap).expect("a net_stack slot was already occupied");
        };
        at(3, memory_region_cap(budget));
        at(4, rendezvous_cap(stack, Rights::READ));
        at(5, notification_cap(wake, Rights::ALL));
        at(6, timer_cap(poll, Rights::WRITE));
        // What each socket is minted from (§255 (each socket is its own capability)); slot 8, the
        // server's own address space, `run_with_own_space` adds once the space exists.
        at(
            socket_contract::stack_slots::MINT,
            rendezvous_cap(stack, Rights::WRITE.union(Rights::GRANT)),
        );
        run_with_own_space(
            image,
            Spawn {
                arg0, // `e1000e::Handoff`: the role tag and the MAC address
                arg1, // the DMA region's physical base
                arg2: listen_grant,
                grants: &[rendezvous_cap(report, Rights::WRITE)], // slot 0
                maps: &maps,
            },
            socket_contract::stack_slots::OWN_SPACE,
        )
    })
    .expect("could not spawn net_stack over the e1000e NIC");
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
        confined_by_iommu,
        mac: found.handoff.mac,
        device: found.device,
        phy_id: found.phy_id,
        lease: 0,
    }
}

/// [`start_net_server`] plus one socket-contract client, `virtio_service::start_net_stack`'s shape:
/// the client's selector is `cli_arg`, and the server's lease is drained before this returns so the
/// client's first request is served. Returns the client's report endpoint with the wiring.
pub fn start_net_stack(
    image: &'static [u8],
    cli_arg: u64,
    listen_grant: u64,
) -> Result<(RendezvousId, Wiring), Absent> {
    start_with_client(image, cli_arg, 0, None, listen_grant)
}

/// `virtio_service::start_package_fetch`'s client, `catalogue` mapped into it read-only, over the
/// `e1000e` server instead of the virtio one.
pub fn start_package_fetch(
    image: &'static [u8],
    cli_arg: u64,
    arg1: u64,
    catalogue: &'static [u8],
) -> Result<(RendezvousId, Wiring), Absent> {
    start_with_client(
        image,
        cli_arg,
        arg1,
        Some(catalogue),
        socket_contract::NO_LISTEN_GRANT,
    )
}

/// The bench boot's client (`kernel/src/network_bench.rs`): `socket_test_client`'s
/// `TEST_TCP_DRAIN` against `peer`, packed `ipv4 << 16 | port`. The selector's number is the
/// client's, spelled again here as every kernel-side caller of that client spells it.
#[cfg_attr(not(feature = "network_bench"), allow(dead_code))]
pub(crate) fn start_drain(
    image: &'static [u8],
    peer: u64,
) -> Result<(RendezvousId, Wiring), Absent> {
    const TEST_TCP_DRAIN: u64 = 7;
    start_with_client(
        image,
        TEST_TCP_DRAIN,
        peer,
        None,
        socket_contract::NO_LISTEN_GRANT,
    )
}

fn start_with_client(
    image: &'static [u8],
    cli_arg: u64,
    cli_arg1: u64,
    blob: Option<&'static [u8]>,
    listen_grant: u64,
) -> Result<(RendezvousId, Wiring), Absent> {
    let mut w = start_net_server(image, listen_grant)?;
    let cli = super::virtio_service::spawn_stack_client(
        image,
        cli_arg,
        cli_arg1,
        blob,
        w.stack,
        &mut w.held,
    );
    w.lease = crate::sched::ipc_receive(w.report)[0] as u32;
    Ok((cli, w))
}
