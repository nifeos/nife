//! `capability_tests`: capabilities between processes: shared memory, `map_physical`, a new
//! thread that holds nothing, rendezvous, delegation, call and reply, and frame capabilities.
//!
//! Name: provisional, minted 2026-10-12 (UTC) by the lane for milestone 873 (the catch-all
//! userspace test file is split by topic).

use core::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};

use super::*;
use crate::arch::exceptions::USER_FAULTS;
use crate::sched;

/// **A program with the capability can print. The same program without it cannot.**
///
/// The binary is byte-identical. Nothing about it changed. What changed is what it was
/// *handed*, and that is the entire content of DECISIONS §10.
///
/// It reports by `brk`, which the kernel treats as a fault: the program expects `NoSuchSlot`
/// from an empty slot and expects `BadPointer` when it asks the kernel to read the kernel's
/// own memory, and it kills itself if either is wrong. So **no fault** means every one of
/// those held.
#[test_case]
fn a_user_client_moves_data_through_shared_memory() {
    // What the client prints first. Must match fixtures/src/console_test_client.rs.
    const FIRST_LINE: &[u8] = b"      hello from EL0, printed by a driver that also runs at EL0.\n";
    const SHARED_VA: u64 = address_space_map::pair_page(0x0000_0000_0060_0000);

    static CAPTURED: AtomicBool = AtomicBool::new(false);
    static LEN: AtomicU64 = AtomicU64::new(0);
    // Atomics rather than a `static mut`: the server thread writes and the test thread reads, and
    // `CAPTURED`'s SeqCst store is the handoff either way, so the bytes need no `unsafe` to cross.
    static BUF: [AtomicU8; 128] = [const { AtomicU8::new(0) }; 128];

    let image = program("console_test_client").expect("no console_test_client in the archive");
    // Deliberately kept for the boot, like the shared page below and for the same reason: the
    // client spins forever and the server below loops forever, so reclaiming these would turn both
    // into busy loops on a stale endpoint. Two pages from a region of their own, charged to this
    // test, rather than from the kernel's pool, whose chunk boundary charges a later test.
    let endpoints = crate::memory_region::create(2).expect("no endpoint region");
    let request = sched::create_rendezvous_from(endpoints).expect("no request rendezvous");
    let reply = sched::create_rendezvous_from(endpoints).expect("no reply rendezvous");

    // The shared page, owned by the test (not by either address space), so `map_physical`
    // will not free it. Deliberately leaked: the client spins forever, so there is no safe
    // moment to reclaim it, and one page is a fine price for the test.
    let shared = crate::memory::alloc().expect("no shared frame").addr();

    // The server: a kernel thread that reads the shared page and records the first message.
    sched::spawn(move || {
        loop {
            let m = sched::ipc_receive(request);
            let len = m[0].min(128);
            if !CAPTURED.load(Ordering::SeqCst) {
                let src = crate::arch::mmu::phys_to_virt(shared) as *const u8;
                for (i, byte) in BUF.iter().enumerate().take(len as usize) {
                    // SAFETY: the shared frame is ours via the direct map and `i < len <= 128`, inside
                    // it; the client wrote `len` bytes before sending.
                    byte.store(
                        unsafe { core::ptr::read_volatile(src.add(i)) },
                        Ordering::Relaxed,
                    );
                }
                LEN.store(len, Ordering::SeqCst);
                CAPTURED.store(true, Ordering::SeqCst);
            }
            sched::ipc_send(reply, [0, 0, 0]); // ack, so the client reuses the buffer
        }
    })
    .expect("spawn failed");

    // The client: the real binary, client role, wired to the endpoints and the shared page.
    let faults = USER_FAULTS.load(Ordering::Relaxed);
    sched::spawn(move || {
        run(
            image,
            Spawn {
                arg0: 0, // `console_test_client` has one job and reads no role selector
                arg1: 0,
                arg2: 0,
                grants: &[
                    crate::cap::rendezvous_cap(request, crate::cap::Rights::WRITE),
                    crate::cap::rendezvous_cap(reply, crate::cap::Rights::READ),
                ],
                maps: &[Mapping {
                    va: SHARED_VA,
                    phys: shared,
                    flags: Flags::user_data(),
                }],
            },
        )
    })
    .expect("spawn failed");

    assert!(
        wait_for(|| CAPTURED.load(Ordering::SeqCst)),
        "the server never received a message through shared memory",
    );
    assert_eq!(
        USER_FAULTS.load(Ordering::Relaxed),
        faults,
        "the client faulted instead of printing cleanly",
    );

    let len = LEN.load(Ordering::SeqCst) as usize;
    let mut got = [0u8; 128];
    for (g, b) in got.iter_mut().zip(&BUF) {
        *g = b.load(Ordering::Relaxed);
    }
    assert_eq!(
        &got[..len],
        FIRST_LINE,
        "the wrong bytes arrived through shared memory"
    );
}

/// `map_physical` puts one physical frame into an address space at a chosen VA, with exactly
/// the permissions asked for and no more. The mechanism a driver leaves the kernel on.
#[test_case]
fn map_physical_maps_a_shared_frame_and_a_device_page() {
    const DATA_VA: u64 = address_space_map::pair_page(0x0000_0000_0060_0000);
    const DEV_VA: u64 = address_space_map::pair_page(0x0000_0000_0070_0000);
    // A real device's MMIO on this machine, whichever machine it is: the virtio-mmio bus base.
    // It was the PL011's `0x0900_0000`, which is an aarch64 `virt` fact; the point of the test
    // is that a device-typed mapping lands where it was asked to, and either address serves it.
    let device_phys = mmu::VIRTIO_MMIO_BASE;

    let mut space = AddressSpace::new(2).expect("no address space");
    let frame = crate::memory::alloc().expect("no frame").addr();

    space
        .map_physical(
            DATA_VA,
            frame,
            Flags::user_data(),
            crate::revoke::PageMapSource::NoCapability,
        )
        .expect("shared map failed");
    space
        .map_physical(
            DEV_VA,
            device_phys,
            Flags::user_device(),
            crate::revoke::PageMapSource::NoCapability,
        )
        .expect("device map failed");

    space.while_installed(|| {
        let (data_pa, data_f) = mmu::translate_user(DATA_VA).expect("shared page not mapped");
        assert_eq!(data_pa, frame, "shared page maps the wrong frame");
        assert!(data_f.is_user_accessible() && data_f.is_writable());
        assert!(!data_f.is_user_executable());

        let (dev_pa, dev_f) = mmu::translate_user(DEV_VA).expect("device page not mapped");
        assert_eq!(
            dev_pa, device_phys,
            "device page maps the wrong physical address"
        );
        assert!(dev_f.is_user_accessible() && dev_f.is_writable());
    });
    crate::memory::free(PageFrame::from_addr(frame));
    drop(space);
}

/// A thread can name nothing until somebody hands it something.
#[test_case]
fn a_new_thread_holds_no_capabilities() {
    use crate::cap::Error;

    // The current thread is a kernel thread, spawned by the harness, and was handed nothing.
    for slot in 0..16 {
        assert_eq!(
            sched::current_cap(slot).err(),
            Some(Error::NoSuchSlot),
            "slot {slot} is not empty in a thread nobody granted anything",
        );
    }
}

/// **Milestone 19a, end to end: an rendezvous minted by a process out of its own memory
/// carries IPC between processes.** The maker retypes a page of its untyped into an rendezvous
/// (`RETYPE_OBJ`), delegates a READ view to a peer it has never met, and sends a word into
/// it; the peer listens on the received capability and reports what arrived. No kernel
/// wiring created the rendezvous: budget, mint, delegation, and rendezvous are all the
/// processes' own acts. The word arriving is the whole granular-construction story of 19a
/// working at EL0.
#[test_case]
fn a_process_can_mint_an_rendezvous_and_ipc_flows_over_it() {
    let endpoints = crate::memory_region::create(2).expect("no endpoint region");
    let report = retype_ep_service::wire(endpoints);
    let word = sched::ipc_receive(report)[0];
    assert_eq!(
        word, 0x77,
        "the word never crossed the process-minted rendezvous",
    );
    sched::reclaim_region(endpoints).expect("the endpoint region did not come back");
}

/// **Capability delegation, end to end.** A granter process passes a resource capability to a
/// receiver process over an IPC channel, narrowed to `WRITE`. Three things must hold, and this
/// checks all three: the receiver *gets* the capability, the receiver can *use* it (a
/// capability minted for it by another process works when it invokes it), and the receiver
/// *cannot pass it on* because it was handed the capability without `GRANT`. This is the
/// operation that makes the capability model composable by processes instead of brokered by the
/// kernel at spawn. See `fixtures/src/delegation_granter.rs` and `user::delegation_service`.
#[test_case]
fn a_capability_can_be_delegated_over_ipc_and_grant_gates_re_delegation() {
    let endpoints = crate::memory_region::create(4).expect("no endpoint region");
    let (resource, report) = delegation_service::wire(endpoints);

    // The receiver invoked the *delegated* capability to SEND this word. Collecting it here is
    // proof the capability the granter minted for the receiver actually carries authority.
    let used = sched::ipc_receive(resource)[0];
    assert_eq!(
        used,
        delegation_service::USED_WORD,
        "a delegated capability did not work when its recipient invoked it",
    );

    // The receiver's own two-bit verdict: bit 0 it received a capability, bit 1 the kernel
    // refused its attempt to re-delegate a capability it holds without GRANT.
    let verdict = sched::ipc_receive(report)[0];
    assert_eq!(
        verdict & 0b01,
        0b01,
        "the receiver never received the delegated capability",
    );
    assert_eq!(
        verdict & 0b10,
        0b10,
        "a capability held WITHOUT grant was allowed to be re-delegated: rights did not gate it",
    );
    sched::reclaim_region(endpoints).expect("the endpoint region did not come back");
}

/// **Milestone 12: a process calls a server it was never wired to, and the reply cap is
/// one-shot.** The client `CALL`s across the boundary; the server `RECEIVE_CAP`s the request plus a
/// kernel-minted reply capability naming the caller, answers through it (the round trip through
/// the real syscall path), then tries to answer a second time and reports that the kernel
/// refused. This is what a pre-wired reply rendezvous cannot guarantee.
#[test_case]
fn a_process_calls_a_server_and_the_reply_is_one_shot() {
    let endpoints = crate::memory_region::create(3).expect("no endpoint region");
    let (call_report, oneshot_report) = call_service::wire(endpoints);

    let reply = sched::ipc_receive(call_report)[0];
    assert_eq!(
        reply, 42,
        "the CALL did not return the server's reply (40 + 2)"
    );

    let one_shot = sched::ipc_receive(oneshot_report)[0];
    assert_eq!(
        one_shot, 1,
        "the server's second reply was NOT refused: the reply capability is not one-shot",
    );
    sched::reclaim_region(endpoints).expect("the endpoint region did not come back");
}

/// **A reply carries a capability** (`abi::reply::REPLY_CAPABILITY`, §255 (each socket is its own
/// capability), milestone 649 (every client of a network stack shares its socket numbers)). A
/// server answers a `CALL` with a copy of an endpoint capability; the client finds it in a slot of
/// its own and sends on it, and the word arrives at the server.
/// Around that, the method's two refusals, each of which must leave the Reply usable (a carried
/// capability without `GRANT`, and an empty slot), and the plain `REPLY` that must carry nothing
/// (`x2` is `NO_CAP`, never a slot 0 the caller would misread).
///
/// Falsification: replayable `system_tests/falsifications/user.capability_tests.a_reply_carries_a_capability_the_caller_can_use.patch`
#[test_case]
fn a_reply_carries_a_capability_the_caller_can_use() {
    let endpoints = crate::memory_region::create(4).expect("no endpoint region");
    let (server_report, client_report) = carried_capability_service::wire(endpoints);
    // The server first: a server that took a refusal for success reports early, and its client is
    // then left sending on a probe nobody receives, so reading the client first would hang rather
    // than name the failure.
    let server = sched::ipc_receive(server_report)[0];
    assert_eq!(
        server & 0b0001,
        0b0001,
        "a capability held without GRANT was carried by a reply"
    );
    assert_eq!(
        server & 0b0010,
        0b0010,
        "an empty slot was carried by a reply"
    );
    assert_eq!(
        server & 0b0100,
        0b0100,
        "the carrying reply was refused after two refusals: a refusal consumed the Reply"
    );
    let client = sched::ipc_receive(client_report)[0];
    assert_eq!(
        client & 0b0001,
        0b0001,
        "the reply arrived without the capability it carried"
    );
    assert_eq!(
        client & 0b1000,
        0b1000,
        "the carried capability landed on a slot the client already held"
    );
    assert_eq!(
        client & 0b0010,
        0b0010,
        "the client could not send on the carried capability"
    );
    assert_eq!(
        server & 0b1000,
        0b1000,
        "the word sent on the carried capability did not reach the endpoint it names"
    );
    assert_eq!(
        client & 0b0100,
        0b0100,
        "a plain REPLY delivered something in x2 a caller would read as a capability"
    );
    sched::reclaim_region(endpoints).expect("the endpoint region did not come back");
}

/// **Milestone 13: a process revokes a frame across the boundary.** It retypes a page, maps it,
/// then `REVOKE`s it; the kernel unmaps the page and deletes every capability to it, the
/// process's own included, so a second operation on that slot finds nothing there. This exercises
/// the REVOKE syscall path (rights, unmap, cap deletion). The multi-address-space unmapping and
/// the safe reclamation are proven directly in kernel/src/revoke.rs.
#[test_case]
fn a_process_revokes_a_frame_and_loses_the_capability() {
    let endpoints = crate::memory_region::create(1).expect("no endpoint region");
    let report = revoke_service::wire(endpoints);
    let verdict = sched::ipc_receive(report)[0];
    assert_eq!(
        verdict, 1,
        "REVOKE did not both succeed and leave the frame slot empty",
    );
    sched::reclaim_region(endpoints).expect("the endpoint region did not come back");
}

/// **`PageFrame` capabilities, end to end.** A producer retypes a page into a `PageFrame`, maps it, writes
/// a sentinel, and delegates a READ-only view to a consumer. Two things must hold: the consumer
/// reads the producer's sentinel through its *own* mapping of the same physical page (the memory
/// is genuinely shared, and the kernel copied nothing), and the consumer *cannot* map that page
/// writable, because it was handed the frame with `READ` alone. This is §10's "shared memory
/// carries data" done by the processes rather than wired by the kernel at spawn. See
/// `fixtures/src/page_frame_producer.rs` and `user::page_frame_service`.
#[test_case]
fn a_frame_capability_shares_a_page_and_a_read_only_view_cannot_write_it() {
    let endpoints = crate::memory_region::create(2).expect("no endpoint region");
    let report = page_frame_service::wire(endpoints);

    let verdict = sched::ipc_receive(report)[0];
    assert_eq!(
        verdict & 0b01,
        0b01,
        "the consumer did not read the producer's sentinel through the shared frame: the page was not shared",
    );
    assert_eq!(
        verdict & 0b10,
        0b10,
        "a frame delegated READ-only was mappable writable: rights did not confine the mapping",
    );
    sched::reclaim_region(endpoints).expect("the endpoint region did not come back");
}
