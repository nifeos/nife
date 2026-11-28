//! **`net_stack`'s entry and its serve loop**: the capability slots it is spawned with, the heap
//! smoltcp allocates against, the role dispatch in `_start`, and the server that brings the NIC
//! up and answers the socket contract.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

pub(crate) const REPORT: u64 = 0;

#[doc(alias = "irq")]
pub(crate) const INTERRUPT: u64 = 1;

pub(crate) const MEMORY_REGION: u64 = 3;

pub(crate) const STACK: u64 = 4;

/// The lease word an `e1000e` server sends instead of an address when its spawn arguments were
/// not a handoff the kernel could have made. Outside every private /24 a test accepts.
pub(crate) const BAD_HANDOFF: u64 = 0xDEAD_0001;

/// The notification bound to this thread, which the retransmit timer signals (milestone 106 (a wait
/// that ends on either the interrupt or the deadline)). The spawner makes and binds it.
pub(crate) const WAKE: u64 = 5;

/// The retransmit timer: armed for smoltcp's next deadline while this server waits for a frame.
pub(crate) const RETRANSMIT: u64 = 6;

/// Our own endpoint again, `WRITE | GRANT`, and our own address space (§255 (each socket is its own
/// capability)); `socket_contract::stack_slots` says what each is for.
pub(crate) const MINT: u64 = socket_contract::stack_slots::MINTING_ENDPOINT_SLOT;

pub(crate) const OWN_SPACE: u64 = socket_contract::stack_slots::OWN_ADDRESS_SPACE_SLOT;

/// The heap smoltcp allocates against, capped under the granted budget.
///
/// Paired with the spawn service's `NET_SERVER_BUDGET_PAGES = 128`: the 32 pages between them pay
/// for the heap's page tables and for mapping clients' shared frames, and the budget is small
/// because no `net_stack` region is ever reclaimed (`kernel/src/user/virtual_io_service.rs` has the
/// measurement). If either number moves, both must.
pub(crate) const HEAP_MAXIMUM: u64 = 96 * 4096;

#[global_allocator]
pub(crate) static HEAP: memory_region_heap::MemoryRegionHeap =
    memory_region_heap::MemoryRegionHeap::new();

user_mode_runtime::program_entry!(start);

/// Entry role 0 is the net server; the NIC handoff roles are servers on other cards, and any other
/// role runs a socket-contract client in the same binary (nifeos/record:notes/net.md, "One binary, one archive
/// entry").
///
/// `a2` is the server's **grant word**, both port authorities in one spawn argument: the TCP
/// listen range in the low half (milestone 107 (the socket contract learns to accept), `socket_contract::listen_grant`) and the fixed-UDP
/// bind range in the high half (milestone 55 (time Machine: SMB3 with Apple's extensions, and mDNS)'s stack half, `socket_contract::udp_bind_grant`).
/// Whoever spawns the server decides both; a server spawned with zero (`NO_PORT_GRANT`, which
/// every outbound-only test uses) refuses every `LISTEN` and every `BIND_UDP`. The client half
/// ignores it.
fn start(role: u64, direct_memory_access_phys: u64, a2: u64) -> ! {
    if role == 0 {
        let dev = Nic::VirtualIo(virtual_io_net_transport::VirtualIoNet::bring_up(
            direct_memory_access_phys,
        ));
        server(dev, a2)
    } else if driver_process_contract::e1000e::Handoff::is_role(role) {
        // The `e1000e` server (milestone 494 (a driver for the network card a PC actually has)):
        // the role word carries the tag and the MAC address, the second word the DMA region.
        let Some(handoff) =
            driver_process_contract::e1000e::Handoff::unpack(role, direct_memory_access_phys)
        else {
            send(REPORT, BAD_HANDOFF, 0, 0);
            user_mode_runtime::exit();
        };
        server(
            Nic::Gigabit(e1000e_transport::GigabitNic::bring_up(handoff)),
            a2,
        )
    } else if driver_process_contract::designware_ethernet::Handoff::is_role(role) {
        // radon's Ethernet port (milestone 53 (the board's own peripherals: network and storage on
        // real silicon)), the same two words in the `e1000e` server's shape.
        let Some(handoff) = driver_process_contract::designware_ethernet::Handoff::unpack(
            role,
            direct_memory_access_phys,
        ) else {
            send(REPORT, BAD_HANDOFF, 0, 0);
            user_mode_runtime::exit();
        };
        server(
            Nic::DesignWare(designware_ethernet_transport::DesignWareNic::bring_up(
                handoff,
            )),
            a2,
        )
    } else if role == socket_squatter::ROLE {
        socket_squatter::run()
    } else {
        // The client's second word is its own (only the package exchange reads it); the DMA page is
        // the server's.
        socket_test_client::run(role, direct_memory_access_phys)
    }
}

/// The net server: bring the NIC up, run DHCP, then serve the socket contract.
pub(crate) fn server(mut dev: Nic, grant_word: u64) -> ! {
    let mut iface = heap_and_interface(&mut dev);
    let mut sockets = SocketSet::new(vec![]);

    // --- DHCP: bring the interface up, then report the lease (the phase-A test asserts it). ---
    dhcp_lease(&mut iface, &mut dev, &mut sockets);

    // No multicast group is joined. A group is interface state rather than socket state, so a
    // multicast client puts its join here, after DHCP so the IGMP report carries a real source
    // address, and smoltcp's `multicast` feature back in components/Cargo.toml (nifeos/record:notes/mdns.md).

    // --- Serve the socket contract. One synchronous exchange per request. ---
    let mut table = Table::new();
    // Local ports follow the boot clock, so a stack started after another starts past its ports
    // (network_stack::ephemeral_port says why that matters to a peer in TIME_WAIT).
    let mut ports = ephemeral_port::Cursor::new();
    loop {
        let req = receive_request(STACK);
        let (operation, arg, badge) = (req.w0, req.w1, req.badge);

        // `ATTACH` is the one operation that takes a delegation, the socket's frame sent on that
        // socket's own capability; every other is a CALL, and a delegation sent with one is deleted
        // here rather than answered into (§245 (a `CALL` server tells a Reply from a delegation)).
        let to = match req.delivered {
            Delivered::Delegation(frame) if operation == OPERATION_ATTACH_SHARED_PAGE => {
                table.attach(badge, frame);
                continue;
            }
            other => other.into_reply(),
        };
        // Nobody to answer: a plain SEND, an ATTACH that carried no frame, or a delegation (now
        // deleted) on an operation that wants a CALL.
        let Some(to) = to else {
            continue;
        };

        // **The front door makes sockets and does nothing else** (§255 (each socket is its own
        // capability)). A badge outside the socket range is a front door, whoever badged it.
        if !names_a_socket(badge) {
            let opened = match operation {
                OPERATION_OPEN_UDP => open_udp(&mut sockets, &mut table, &mut ports),
                OPERATION_OPEN_TCP => open_tcp(&mut sockets, &mut table, &mut ports),
                OPERATION_LISTEN => tcp_listen(&mut sockets, &mut table, arg, grant_word),
                OPERATION_BIND_UDP => udp_bind(&mut sockets, &mut table, arg, grant_word),
                _ => Err(REPLY_ERROR),
            };
            match opened {
                Ok((entry, word)) => table.hand_over(&mut sockets, to, entry, word, 0),
                Err(word) => {
                    reply(to, word, 0);
                }
            }
            continue;
        }

        // A socket's capability. A badge the table does not hold is a closed socket's, or one
        // nobody minted: it reaches nothing.
        let Some(entry) = table.find(badge) else {
            reply(to, REPLY_ERROR, 0);
            continue;
        };
        // The trace covers one TCP connection, from its connect to its close.
        let tcp = table.socks[entry].is_some_and(|s| s.kind == Kind::Tcp);
        if tcp && operation == OPERATION_CONNECT {
            receive_trace::connecting();
        }
        let closed_tcp = tcp && operation == OPERATION_CLOSE;
        let rep = match operation {
            OPERATION_SEND_TO => udp_sendto(
                &mut iface,
                &mut dev,
                &mut sockets,
                &table,
                entry,
                arg as usize,
            ),
            OPERATION_RECEIVE => sock_receive(&mut iface, &mut dev, &mut sockets, &table, entry),
            OPERATION_CONNECT => tcp_connect(&mut iface, &mut dev, &mut sockets, &table, entry),
            OPERATION_SEND => tcp_send(
                &mut iface,
                &mut dev,
                &mut sockets,
                &table,
                entry,
                arg as usize,
            ),
            OPERATION_CLOSE => {
                table.close(&mut iface, &mut dev, &mut sockets, entry);
                REPLY_OK
            }
            OPERATION_ACCEPT => {
                match tcp_accept(&mut iface, &mut dev, &mut sockets, &mut table, entry) {
                    Ok((connection, peer)) => {
                        table.hand_over(&mut sockets, to, connection, REPLY_OK, peer);
                        continue;
                    }
                    Err(word) => word,
                }
            }
            // A CALL naming ATTACH carries no frame; anything else is not an operation.
            _ => REPLY_ERROR,
        };
        reply(to, rep, 0);
        if closed_tcp {
            receive_trace::closed();
        }
    }
}
