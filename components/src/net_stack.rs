//! **The net server: smoltcp as a userspace TCP/IP stack over the confined NIC** (milestone 30,
//! piece 3; notes/net.md).
//!
//! The kernel confines the NIC's DMA and knows nothing about TCP, UDP or DHCP. `net_stack` brings
//! the NIC up, runs DHCP to completion and reports the lease, then serves the socket contract
//! (DECISIONS §25 (socket identity), `crates/socket_contract`) on its `Stack` endpoint: a socket is
//! a capability it mints and names by its badge (§255 (each socket is its own capability)), a
//! connection's bytes cross in a frame the client delegates, and every operation is one message.
//! One thread: it blocks on `Stack` between requests, drives the network inside one, and accepts
//! (milestone 107 (the socket contract learns to accept)) only on ports its spawner granted.

//! # Capability contract
//!
//! Slot 0 the lease's report endpoint (`WRITE`); 1 the NIC interrupt (`WAIT`/`ACK`); 2 the confined
//! `Virtio` transport; 3 an untyped budget, for the heap and clients' frames; 4 `Stack` (`READ`),
//! where requests arrive; 5 and 6 a notification bound to this thread and its retransmit timer (not
//! in the progenitor's spawn); 7 `Stack` (`WRITE | GRANT`) to mint sockets from; 8 this address
//! space (`WRITE`), to unmap a closed socket's page. `arg1` is the DMA page's physical address and
//! `arg2` the TCP listen (milestone 107) and UDP bind ranges, of which zero opens neither.
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
//! Each in full in notes/net/memory-and-reclamation.md, "The server's own limits".
//!
//! - No host fuzz target reaches this dispatch; `notes/fuzzing-the-services.md` prices one.
//! - One client can take every socket: nothing bounds how many of [`SOCKETS`] one client holds.
//! - DHCP has no bound: a live link to a LAN with no DHCP server is a wait with no end.
//! - A spawn without slot 8 cannot reuse a socket's page, so after sixteen it carries no bytes.

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
