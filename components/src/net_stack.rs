//! **The net server: smoltcp as a userspace TCP/IP stack over the confined NIC** (milestone 30,
//! piece 3).
//!
//! The networking form of the userspace-reuse thesis: the kernel confines the NIC's DMA (pieces 1
//! and 2), and a real, reused TCP/IP stack (smoltcp, not hand-built) runs it entirely at EL0. The
//! kernel knows nothing about TCP, UDP, or DHCP; it owns only the DMA confinement.
//!
//! `net_stack` brings the NIC up, runs DHCP to completion (reporting the lease), then serves a
//! capability-shaped socket contract on a `Stack` endpoint (DECISIONS §25, notes/net.md,
//! `crates/socket_contract/src/lib.rs`): a socket is a capability this server mints and names by its
//! badge (§255 (each socket is its own capability)), per-connection bytes cross in a shared frame
//! the client delegates, and every operation is one message. Phase one is single-threaded and
//! synchronous, one exchange per request; the server blocks on the `Stack` endpoint between
//! requests and drives the network inside handling one.
//!
//! # Capability contract
//! - slot 0: the report endpoint (WRITE) for the DHCP lease
//! - slot 1: the NIC interrupt (WAIT / ACK)
//! - slot 2: the confined `Virtio` transport
//! - slot 3: an untyped budget, for the heap and for mapping clients' shared frames
//! - slot 4: the `Stack` endpoint (READ), where clients' requests arrive
//! - slot 5: the retransmit notification, bound to this thread (absent from the progenitor's spawn)
//! - slot 6: the retransmit timer (likewise)
//! - slot 7: the `Stack` endpoint again (WRITE | GRANT), which each socket is minted from
//! - slot 8: this process's own address space (WRITE), to unmap a closed socket's page
//! - arg1: the DMA page's physical address
//! - arg2: the **grant word**: the TCP listen range (milestone 107) in its low half and the
//!   fixed-UDP bind range (milestone 55's stack half) in its high half, both packed by
//!   `socket_contract`. Zero, the default, means no port anywhere in either protocol: a stack serves
//!   inbound connections, or claims a fixed UDP port, only when whoever spawned it said which.
//!
//! Name: ratified 2026-07-30 (calef, DECISIONS §39, landed by milestone 46), replacing `netd`, and
//! respelled from `netstack` on 2026-08-01 (milestone 63) because `net` is already this tree's word
//! and the two halves are separate concepts. Refused `netd`, the name DECISIONS §39 was written
//! about: it holds five explicit capabilities, cannot name its own callers, is supervised, and can
//! be reaped by something that lacks the authority to build it, which is about as far from the
//! model that suffix claims as a long-running process gets.
//!

//! # BUGS
//!
//! **No host fuzz target reaches this dispatch** (proposal #1592 part a, rank 2). The request match
//! and every helper under it live in this EL0 binary and take `virtio_net_transport::VirtioNet`,
//! the clock and two blocking waits directly, so nothing builds for the host. What a host part
//! would cost, measured by reading on 2026-10-04 (UTC): about 630 lines moved into a sans-IO crate
//! over smoltcp's `phy::Device`, with the clock and the waits behind an edge trait. See
//! `notes/fuzzing-the-services.md`.
//!
//! **One client can take every socket.** The table is [`SOCKETS`] entries shared by every client
//! of the stack, and nothing bounds how many one client holds, so a client that opens sixteen and
//! keeps them starves the rest of the network. Before §255 (each socket is its own capability) the
//! same was true of the six shared socket numbers. A per-client quota wants the client named, which
//! a badged front door per client would do; nothing builds that yet. Recorded by milestone 649
//! (every client of a network stack shares its socket numbers).
//!
//! **DHCP has no bound.** The lease loop runs until a server answers, and every spawner blocks
//! on the report, so a link that is up with no DHCP server behind it is a wait with no end. The
//! e1000e and DesignWare wirings refuse a link that never came up (`Absent::NoLink`,
//! `NotStarted::NoLink`) and every test runner provides slirp's server, so only a live link to a
//! LAN with no DHCP server reaches it.
//!
//! **A spawn without slot 8 cannot reuse a socket's page.** With no address space to `UNMAP` from,
//! a closed socket's page stays mapped, and its table entry refuses every later frame rather than
//! show the next socket the last one's page, so after sixteen sockets with pages the stack carries
//! no more bytes. Every spawner in the tree grants slot 8.

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

extern crate alloc;

use alloc::vec;

use smoltcp::iface::{Config, Interface, SocketHandle, SocketSet};
use smoltcp::socket::{dhcpv4, tcp, udp};
use smoltcp::time::Instant;
use smoltcp::wire::{EthernetAddress, HardwareAddress, IpAddress, IpCidr, IpEndpoint, Ipv4Address};
use user_mode_runtime::mapped_window::{MappedWindow, PAGE};
use user_mode_runtime::{
    Delivered, Reply, cap_delete, cntfrq, interrupt_wait, map_page_frame, notification_poll, now,
    receive_request, reply, reply_capability, send, sleep_until, timer_arm, timer_cancel,
};

#[path = "designware_ethernet_transport.rs"]
mod designware_ethernet_transport;
#[path = "e1000e_transport.rs"]
mod e1000e_transport;
#[path = "virtio_net_transport.rs"]
mod virtio_net_transport;
// The server's start-up: the heap, the interface, and the DHCP lease it reports.
#[path = "interface_bring_up.rs"]
mod interface_bring_up;
use interface_bring_up::{dhcp_lease, heap_and_interface};
// Ending a TCP socket or a listener on `CLOSE`, and why a listener resets what landed in it.
#[path = "tcp_close.rs"]
mod tcp_close;
use tcp_close::close_tcp;
// Waiting for the NIC between polls, bounded by the caller's deadline as well as smoltcp's timers.
#[path = "nic_wait.rs"]
mod nic_wait;
use nic_wait::wait_for_nic;
// The socket-contract client rides in this same binary (dispatched by the entry role), because the
// initial ramdisk directory holds at most 15 files; see components/src/socket_test_client.rs.
#[path = "socket_test_client.rs"]
mod socket_test_client;
// The hostile second client milestone 649's confinement test runs (role 8), here for the same
// 15-file reason; its header records why that is an exception.
#[path = "socket_squatter.rs"]
mod socket_squatter;
use socket_contract::*;

#[path = "nic_device.rs"]
mod nic_device;
#[path = "socket_operations.rs"]
mod socket_operations;
#[path = "socket_server.rs"]
mod socket_server;
#[path = "socket_table.rs"]
mod socket_table;

use nic_device::*;
use socket_operations::*;
use socket_server::*;
use socket_table::*;

user_mode_runtime::panic_handler!();
