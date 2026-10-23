//! Wiring for `net_stack` over the JH7110's Ethernet port: `e1000e_service`'s slots, the DMA
//! region and the controller's DMA page (`0x1000` to `0x1fff`), without the MAC page or an
//! interrupt. See notes/designware-ethernet.md, "The confinement, stated plainly".
//!
//! # BUGS
//!
//! - The DMA page holds both ring bases and the controller's reset bit, and the JH7110 has no
//!   IOMMU here, so the server can aim the device's DMA anywhere and reset the MAC.
//! - One wiring at a time: a second bring-up resets the device under the first.
//! - Proved by one radon bench boot ([`PROVEN_ON_SILICON`]), at a rate its note's `BUGS` records.

use ::designware_ethernet::process::{DATA_PLANE_VA, DMA_PAGE_VA};

use super::*;
use crate::cap::{Rights, memory_region_cap, notification_cap, rendezvous_cap, timer_cap};
use crate::designware_ethernet::{Absent, Found};
use crate::sched::RendezvousId;
use crate::user::holding::Holding;
use crate::user::virtio_service::{NET_SERVER_BUDGET_PAGES, NET_SERVER_STACK_PAGES};

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

/// Whether the booted system may bring this port up on its own: `true` on one radon bench boot,
/// `bench/radon-2026-10-20/network-bench-3.log`, which ended `verdict LEASED-AND-MEASURED` with
/// `coherence : Coherent`. No emulator has this controller. Set it back to `false` if a radon boot
/// ever hangs in the bring-up, and the port is left as U-Boot left it.
pub(crate) const PROVEN_ON_SILICON: bool = true;

/// The last server's thread, so a second wiring can refuse while it lives.
static SERVER: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(u64::MAX);

/// Bring the port up and start `net_stack` over it, with `listen_grant` as the inbound ports.
pub(crate) fn start_net_server(
    image: &'static [u8],
    listen_grant: u64,
) -> Result<Wiring, NotStarted> {
    let previous = SERVER.load(core::sync::atomic::Ordering::Acquire);
    if previous != u64::MAX && crate::sched::is_thread_present(previous) {
        return Err(NotStarted::Busy);
    }
    let found = crate::designware_ethernet::bring_up().map_err(NotStarted::Absent)?;
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
    use core::sync::atomic::Ordering;
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

    const PLANE: usize = ::designware_ethernet::layout::PAGES as usize;
    const MAPS: usize = PLANE + 1 + NET_SERVER_STACK_PAGES as usize;
    let mut maps = [Mapping {
        va: 0,
        phys: 0,
        flags: Flags::user_data(),
    }; MAPS];
    super::fs_service::map_channel(
        &mut maps[..PLANE],
        DATA_PLANE_VA,
        found.handoff.data_plane_phys,
        PLANE,
    );
    // Device-typed, because a cached or reordered tail write is a frame the device never hears of.
    maps[PLANE] = Mapping {
        va: DMA_PAGE_VA,
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
                arg0, // `designware_ethernet::Handoff`: the role tag and the MAC address
                arg1, // the DMA region's physical base
                arg2: listen_grant,
                grants: &[rendezvous_cap(report, Rights::WRITE)], // slot 0
                maps: &maps,
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
