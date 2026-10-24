//! `network_tests`: the network path: the virtio-net drivers, DHCP, the socket contract over
//! UDP and TCP, name resolution, a package fetched over HTTP, and `std::net`.
//!
//! Name: provisional, minted 2026-10-12 (UTC) by the lane for milestone 873 (the catch-all
//! userspace test file is split by topic).

#[cfg(target_arch = "aarch64")]
use super::std_tests::assert_std_transcript_from;
#[cfg(target_arch = "aarch64")]
use super::test_programs::{blk_image, std_exerciser_image};
#[cfg(target_arch = "aarch64")]
use super::*;
#[cfg(target_arch = "aarch64")]
use crate::sched;

/// The `net_stack` program's ELF bytes (milestone 30, piece 3): the smoltcp net server, a distinct
/// binary loaded by name. Used only by the net tests, whose RISC-V twins live in
/// `riscv_virtio_tests`.
#[cfg(target_arch = "aarch64")]
fn net_stack_image() -> &'static [u8] {
    program("net_stack").expect("no net_stack program in the initial ramdisk")
}

/// The net client's test selectors and its success word, matching
/// `components/src/socket_test_client.rs`. The client is a nonzero entry role of the `net_stack`
/// binary, so it needs no image of its own.
#[cfg(target_arch = "aarch64")]
const NET_TEST_NAME_RESOLUTION: u64 = 1;
#[cfg(target_arch = "aarch64")]
const NET_TEST_TCP_ECHO: u64 = 2;
#[cfg(target_arch = "aarch64")]
const NET_TEST_TCP_REOPEN: u64 = 3;
#[cfg(target_arch = "aarch64")]
const NET_TEST_UDP_TFTP: u64 = 4;
#[cfg(target_arch = "aarch64")]
const NET_TEST_TCP_ACCEPT: u64 = 5;
#[cfg(target_arch = "aarch64")]
const NET_TEST_HTTP_PACKAGE: u64 = 6;
#[cfg(target_arch = "aarch64")]
const NET_TEST_TCP_ACCEPT_UNHEARD: u64 = 12;
/// `TEST_HTTP_PACKAGE`'s refusal word, reported for a package whose digest disagrees.
#[cfg(target_arch = "aarch64")]
const NET_CLIENT_DIGEST_REFUSED: u64 = 3;
/// The one port the inbound gate is granted (milestone 107). The runners forward a host port to
/// exactly this one, and the client asks for `fixture::DENIED_PORT` as well to prove the grant
/// refuses. Named here because the *spawn service* is what grants it, which is the point.
///
/// Taken from `socket_contract::fixture` rather than spelled again (milestone 64 (enough `std` to
/// run somebody else's crate)): three binaries and this test have to agree on the number, and a
/// literal repeated per call site is the shape rule 7 exists to retire. The runners' `hostfwd`
/// spells it a second time because a shell script cannot read a Rust crate, and that drift is loud
/// (the prober reports the guest served none).
#[cfg(target_arch = "aarch64")]
const NET_LISTEN_PORT: u16 = socket_contract::fixture::LISTEN_PORT;
/// The fixed UDP port range the accept test's stack is granted (milestone 55's UDP bind grant).
/// Named here for the same reason as the listen port: the spawn service grants it, and a program
/// cannot ask for what it was not given. `socket_test_client` binds 5354 inside it to prove a
/// granted port binds and is exclusive, and asks for 4444 outside it to prove the refusal.
///
/// The numbers carry no meaning of their own (5353 is multicast DNS's port, which nothing here uses
/// any more; notes/mdns.md); they are kept so the client and this grant agree.
#[cfg(target_arch = "aarch64")]
const NET_UDP_GRANT_BOTTOM: u16 = 5353;
#[cfg(target_arch = "aarch64")]
const NET_UDP_GRANT_TOP: u16 = 5354;
#[cfg(target_arch = "aarch64")]
const NET_CLIENT_OK: u64 = 1;

/// The client could not complete for an ENVIRONMENTAL reason (the host resolver never answered),
/// not because of a defect here. Only the non-gating real-DNS half of the name test can report it.
#[cfg(target_arch = "aarch64")]
const NET_CLIENT_NO_ANSWER: u64 = 2;

/// **A userspace driver completes a DHCP round trip over virtio-net.** Milestone 30, end to
/// end, and the proof the multi-queue confinement carries a real NIC.
///
/// The kernel enumerates the NIC and hands a driver at EL0 a confined `Virtio` capability, a DMA
/// page, and an interrupt. The driver brings up BOTH virtqueues (receive = 0, transmit = 1),
/// posts a receive buffer, transmits a hand-built DHCP DISCOVER, and waits for QEMU user-mode
/// networking's OFFER. It reports the offered address, which must land in slirp's 10.0.2.0/24.
/// Because a valid OFFER for our transaction is the only path to that report, a match proves the
/// DISCOVER left (TX) and the OFFER returned (RX), across both queues and both directions of the
/// confinement, with no TCP/IP stack in the loop.
// RISC-V twin:
// `riscv_virtio_tests::a_userspace_driver_completes_a_dhcp_round_trip_over_virtio_net`. Gated here
// rather than run twice: that module drives the same property on the other instruction set through
// the same `block_driver` and `net_stack` binaries, and a second copy would double the suite's
// slowest tests to prove nothing.
#[cfg(target_arch = "aarch64")]
#[test_case]
fn a_userspace_driver_completes_a_dhcp_round_trip_over_virtio_net() {
    let Some(report) = virtio_service::start_net(blk_image()) else {
        // No NIC on this run (a bare boot). The test runners always attach one, so this
        // branch is not the parity gate. See helpers/qemu-runner-*.sh (NIFE_NET).
        crate::testing::skip!("no virtio-net device attached");
    };

    let yiaddr = sched::ipc_receive(report)[0] as u32;
    assert_eq!(
        yiaddr & 0xffff_ff00,
        0x0A00_0200,
        "the DHCP OFFER's yiaddr {yiaddr:#010x} is not in QEMU slirp's 10.0.2.0/24: the round \
         trip did not complete correctly",
    );
    // No fresh routed interrupt is asserted, unlike the disk read test: the net driver's completion
    // is the used ring advancing, not one interrupt per operation (notes/dma.md), and the suite
    // shares one NIC across many drivers, so a prior operator's leftover completion can be consumed
    // as a stale wakeup. The OFFER round trip is the proof the interrupt path carried it.
}

/// The same DHCP round trip over the PCIe transport, behind the IOMMU (milestone 30, §20): the
/// NIC is confined in hardware to its DMA region, and the driver binary is byte-identical to the
/// mmio one. Proves the multi-queue confinement and the net driver work over the bus real
/// hardware uses.
// RISC-V twin:
// `riscv_virtio_tests::a_userspace_driver_completes_a_dhcp_round_trip_over_virtio_net_pci`. Gated
// here rather than run twice: that module drives the same property on the other instruction set
// through the same `block_driver` and `net_stack` binaries, and a second copy would double the
// suite's slowest tests to prove nothing.
#[cfg(target_arch = "aarch64")]
#[test_case]
fn a_userspace_driver_completes_a_dhcp_round_trip_over_virtio_net_pci() {
    let Some(report) = virtio_service::start_net_pci(blk_image()) else {
        crate::testing::skip!("no virtio-net-pci device attached");
    };

    let yiaddr = sched::ipc_receive(report)[0] as u32;
    assert_eq!(
        yiaddr & 0xffff_ff00,
        0x0A00_0200,
        "the DHCP OFFER's yiaddr {yiaddr:#010x} over PCIe is not in QEMU slirp's 10.0.2.0/24",
    );
}

/// **The net server: smoltcp running DHCP over the confined NIC** (milestone 30, piece 3). The
/// integration proof and the thesis headline for networking: a real, reused TCP/IP stack
/// (smoltcp, not hand-built) runs entirely at EL0, brings the NIC up through the `Virtio`
/// capability, and completes a DHCP handshake against QEMU user-mode networking. The kernel
/// knows nothing about DHCP; it owns only the DMA confinement. The server reports the acquired
/// address, which must land in slirp's 10.0.2.0/24, so only a real DHCP round trip driven by
/// smoltcp over the confined NIC can produce it.
// RISC-V twin: `riscv_virtio_tests::the_net_server_acquires_a_dhcp_lease_over_smoltcp`. Gated here
// rather than run twice: that module drives the same property on the other instruction set through
// the same `block_driver` and `net_stack` binaries, and a second copy would double the suite's
// slowest tests to prove nothing.
#[cfg(target_arch = "aarch64")]
#[test_case]
fn the_net_server_acquires_a_dhcp_lease_over_smoltcp() {
    let Some((report, net)) = virtio_service::start_net_server(net_stack_image()) else {
        crate::testing::skip!("no virtio-net device attached");
    };
    let [addr, nameserver, ..] = sched::ipc_receive(report);
    let addr = addr as u32;
    assert_eq!(
        addr & 0xffff_ff00,
        0x0A00_0200,
        "smoltcp's DHCP lease {addr:#010x} is not in QEMU slirp's 10.0.2.0/24",
    );
    // slirp's DHCP names 10.0.2.3 as the DNS server, and the lease report carries it in its second
    // word (`socket_contract::lease`, provisional) for a resolver's spawner (§248 (the name
    // resolver is its own confined program)).
    assert_eq!(
        socket_contract::lease::word_ipv4(nameserver),
        Some([10, 0, 2, 3]),
        "the lease report did not carry slirp's DNS server (word {nameserver:#x})",
    );
    net.release_or_fail("a net test's net_stack");
}

/// The net server over the PCIe transport, behind the IOMMU (milestone 30, §20): smoltcp drives
/// a NIC confined in hardware and still gets its lease.
// RISC-V twin: `riscv_virtio_tests::the_net_server_acquires_a_dhcp_lease_over_smoltcp_pci`. Gated
// here rather than run twice: that module drives the same property on the other instruction set
// through the same `block_driver` and `net_stack` binaries, and a second copy would double the
// suite's slowest tests to prove nothing.
#[cfg(target_arch = "aarch64")]
#[test_case]
fn the_net_server_acquires_a_dhcp_lease_over_smoltcp_pci() {
    let Some((report, net)) = virtio_service::start_net_server_pci(net_stack_image()) else {
        crate::testing::skip!("no virtio-net-pci device attached");
    };
    let addr = sched::ipc_receive(report)[0] as u32;
    assert_eq!(
        addr & 0xffff_ff00,
        0x0A00_0200,
        "smoltcp's DHCP lease {addr:#010x} over PCIe is not in QEMU slirp's 10.0.2.0/24",
    );
    net.release_or_fail("a net test's net_stack");
}

/// **The socket contract, UDP end to end** (milestone 30 (the network stack as a confined
/// component), piece 3 phase B; DECISIONS §25 (socket identity)). A client process holds a `Stack`
/// rendezvous and its own untyped, mints a shared frame, delegates it, opens a UDP socket by id,
/// sends a datagram, and reads the reply back through the same frame. No ambient network: the
/// client acts only through the capability it was granted, and the bytes cross in the shared frame,
/// never in a message. Proves the whole path, client to `net_stack` to smoltcp to the confined NIC,
/// over the mmio transport.
///
/// The peer is slirp's own TFTP server (10.0.2.2:69), served inside libslirp, so the exchange is
/// deterministic and never leaves the emulator. 10.0.2.3:53 is not a local resolver: libslirp NATs
/// it to the host's nameserver, which fails about 2.5% of queries (measured), so it cannot gate.
// RISC-V twin:
// `riscv_virtio_tests::a_client_completes_a_udp_round_trip_through_the_socket_contract`. Gated here
// rather than run twice: that module drives the same property on the other instruction set through
// the same `block_driver` and `net_stack` binaries, and a second copy would double the suite's
// slowest tests to prove nothing.
#[cfg(target_arch = "aarch64")]
#[test_case]
fn a_client_completes_a_udp_round_trip_through_the_socket_contract() {
    let Some((report, net)) = virtio_service::start_net_stack(
        net_stack_image(),
        NET_TEST_UDP_TFTP,
        false,
        socket_contract::NO_LISTEN_GRANT,
    ) else {
        crate::testing::skip!("no virtio-net device attached");
    };
    let verdict = sched::ipc_receive(report)[0];
    assert_eq!(
        verdict, NET_CLIENT_OK,
        "the UDP round trip against slirp's TFTP server failed (client code {verdict:#x})",
    );
    net.release_or_fail("a net test's net_stack");
}

/// The same UDP round trip over the PCIe transport, behind the IOMMU.
// RISC-V twin:
// `riscv_virtio_tests::a_client_completes_a_udp_round_trip_through_the_socket_contract_pci`. Gated
// here rather than run twice: that module drives the same property on the other instruction set
// through the same `block_driver` and `net_stack` binaries, and a second copy would double the
// suite's slowest tests to prove nothing.
#[cfg(target_arch = "aarch64")]
#[test_case]
fn a_client_completes_a_udp_round_trip_through_the_socket_contract_pci() {
    let Some((report, net)) = virtio_service::start_net_stack(
        net_stack_image(),
        NET_TEST_UDP_TFTP,
        true,
        socket_contract::NO_LISTEN_GRANT,
    ) else {
        crate::testing::skip!("no virtio-net-pci device attached");
    };
    let verdict = sched::ipc_receive(report)[0];
    assert_eq!(
        verdict, NET_CLIENT_OK,
        "the UDP round trip over PCIe failed (client code {verdict:#x})",
    );
    net.release_or_fail("a net test's net_stack");
}

/// **An accept nobody answers ends at `net_stack`'s bound** (the riscv-cpu-net-flake lane,
/// 2026-10-13 UTC). One `ACCEPT` on the PCI NIC, which no `hostfwd` reaches, so no SYN arrives and
/// smoltcp holds no timer. The stack's wait must end by its own 15 s bound and refuse, because
/// `nic_wait` takes the caller's deadline; a wait that blocked for a frame would make this a 60 s
/// watchdog, and the listener retries built on the bound would never run. x86_64 has no twin: its
/// NIC is the polled `e1000e`, whose wait is a 1 ms sleep, so the bound holds there.
///
/// Falsification: replayable `system_tests/falsifications/user.network_tests.an_accept_nobody_answers_ends_at_the_stacks_bound.patch`
#[cfg(target_arch = "aarch64")]
#[test_case]
fn an_accept_nobody_answers_ends_at_the_stacks_bound() {
    let started = crate::arch::timer::now();
    let Some((report, net)) = virtio_service::start_net_stack(
        net_stack_image(),
        NET_TEST_TCP_ACCEPT_UNHEARD,
        true,
        socket_contract::listen_grant(NET_LISTEN_PORT, NET_LISTEN_PORT),
    ) else {
        crate::testing::skip!("no virtio-net-pci device attached");
    };
    let verdict = sched::ipc_receive(report)[0];
    let waited_ms = (crate::arch::timer::now() - started) * 1000 / crate::arch::timer::frequency();
    assert_eq!(
        verdict, NET_CLIENT_OK,
        "an accept nobody could reach did not come back refused (client code {verdict:#x})",
    );
    assert!(
        waited_ms >= 15_000,
        "the accept was refused after {waited_ms} ms, before net_stack's 15 s bound",
    );
    net.release_or_fail("a net test's net_stack");
}

/// **A host name resolves through the socket contract, and the lies do not** (milestone 384 (in a
/// capability system the resolver is a grant)). Two halves in one client, because a second
/// `net_stack` would need a virtio slot past `MAX_DEVICES`.
///
/// The first half gates: the client asks `helpers/name-server-peer` (a `guestfwd` at 10.0.2.9:53,
/// DNS over TCP) about six names through `domain_name_system`. Two resolve, one via a compressed
/// CNAME; four fail their own checks (no such name, a wrong transaction id, an unasked-for address,
/// a self-pointing compression pointer); then it connects to the resolved address for an echo. The
/// second half, real DNS via 10.0.2.3 (NATed to the host's nameserver), does not gate: no answer
/// skips (milestone 214's partial shape), and only a reply that is not a valid answer fails.
// RISC-V twin:
// `riscv_virtio_tests::a_name_resolves_through_the_stack_and_a_real_one_when_the_host_answers`.
// Gated here rather than run twice: that module drives the same property on the other instruction
// set through the same `block_driver` and `net_stack` binaries, and a second copy would double the
// suite's slowest tests to prove nothing.
/// Falsification: replayable `system_tests/falsifications/user.network_tests.a_name_resolves_through_the_stack_and_a_real_one_when_the_host_answers.patch`
#[cfg(target_arch = "aarch64")]
#[test_case]
fn a_name_resolves_through_the_stack_and_a_real_one_when_the_host_answers() {
    let Some((report, net)) = virtio_service::start_net_stack(
        net_stack_image(),
        NET_TEST_NAME_RESOLUTION,
        false,
        socket_contract::NO_LISTEN_GRANT,
    ) else {
        crate::testing::skip!("no virtio-net device attached");
    };
    let [gating, real, ..] = sched::ipc_receive(report);
    assert_eq!(
        gating, NET_CLIENT_OK,
        "name resolution against the runners' name server failed (client code {gating:#x}: \
         0xE1 then the case in NAME_SERVER_CASES then the stage, where F is a wrong verdict)",
    );
    net.release_or_fail("a net test's net_stack");
    if real == NET_CLIENT_NO_ANSWER {
        // The partial shape milestone 214 (a test that prints "skipping" and returns is counted as
        // passed) settled on: the gating half ran and was asserted above, and the conditioned half
        // was never put to the test, so the reason says which is which.
        crate::testing::skip!(
            "the name server half passed; the host's resolver did not answer, so no real DNS name \
             was resolved this run"
        );
    }
    assert_eq!(
        real, NET_CLIENT_OK,
        "a DNS response came back from the host's resolver but was not a valid answer to our \
         query (client code {real:#x}): a defect here, not a network problem",
    );
}

/// **The socket contract, TCP end to end** (milestone 30, piece 3 phase B). A client opens a TCP
/// socket by id, connects to slirp's guestfwd echo peer (10.0.2.9:7777, piped to `/bin/cat`),
/// sends a payload, receives the echo, and closes. The full round trip, handshake through
/// bidirectional data to teardown, deterministic and zero-host-setup (nothing outlives QEMU),
/// through the client, `net_stack`, smoltcp, and the confined NIC.
// RISC-V twin: `riscv_virtio_tests::a_client_echoes_over_tcp_through_the_socket_contract`. Gated
// here rather than run twice: that module drives the same property on the other instruction set
// through the same `block_driver` and `net_stack` binaries, and a second copy would double the
// suite's slowest tests to prove nothing.
#[cfg(target_arch = "aarch64")]
#[test_case]
fn a_client_echoes_over_tcp_through_the_socket_contract() {
    let Some((report, net)) = virtio_service::start_net_stack(
        net_stack_image(),
        NET_TEST_TCP_ECHO,
        false,
        socket_contract::NO_LISTEN_GRANT,
    ) else {
        crate::testing::skip!("no virtio-net device attached");
    };
    let verdict = sched::ipc_receive(report)[0];
    assert_eq!(
        verdict, NET_CLIENT_OK,
        "the TCP echo round trip through the socket contract failed (client code {verdict:#x})",
    );
    net.release_or_fail("a net test's net_stack");
}

/// **A package fetched over plain HTTP is accepted only by the digest the image vouches for**
/// (milestone 198 (a package manager) rung 3a). The client `GET`s `uptime 0.1.0` from a host
/// process on the same network (`helpers/package-http-peer`, a `guestfwd` peer), hashes it as it
/// arrives through `http_response` and the net stack, and compares against the image's package
/// catalogue, which the archive build packed above the measurement table. **Then it fetches the
/// peer's copy with one byte flipped, which must be refused**: that half is what gives the first
/// its meaning, because the tampered response is a complete, correct HTTP exchange and only the
/// digest can tell. Both fetches share one `net_stack`, because every stack a test starts holds a
/// virtio slot for the rest of the boot (`MAX_DEVICES`). Nothing else in the tree fetches a file
/// over TCP or checks a package's digest on a target.
// RISC-V twin:
// `riscv_virtio_tests::a_package_fetched_over_http_is_accepted_only_by_the_image_digest`.
#[cfg(target_arch = "aarch64")]
#[test_case]
fn a_package_fetched_over_http_is_accepted_only_by_the_image_digest() {
    let catalogue = program(package_archive::CATALOG)
        .expect("no package catalogue in the initial ramdisk: the archive build packs one");
    let Some((report, net)) = virtio_service::start_package_fetch(
        net_stack_image(),
        NET_TEST_HTTP_PACKAGE,
        catalogue.len() as u64,
        catalogue,
    ) else {
        crate::testing::skip!("no virtio-net device attached");
    };
    let [genuine, tampered, ..] = sched::ipc_receive(report);
    assert_eq!(
        genuine, NET_CLIENT_OK,
        "the package fetched over HTTP was not accepted (client code {genuine:#x}; \
         {NET_CLIENT_DIGEST_REFUSED} means its digest disagreed with the image's catalogue)",
    );
    assert_eq!(
        tampered, NET_CLIENT_DIGEST_REFUSED,
        "a package with one byte flipped was not refused by digest (client code {tampered:#x}; \
         {NET_CLIENT_OK} would mean it was accepted)",
    );
    net.release_or_fail("the package fetch's net_stack");
}

/// The same TCP echo round trip over the PCIe transport, behind the IOMMU.
// RISC-V twin: `riscv_virtio_tests::a_client_echoes_over_tcp_through_the_socket_contract_pci`.
// Gated here rather than run twice: that module drives the same property on the other instruction
// set through the same `block_driver` and `net_stack` binaries, and a second copy would double the
// suite's slowest tests to prove nothing.
#[cfg(target_arch = "aarch64")]
#[test_case]
fn a_client_echoes_over_tcp_through_the_socket_contract_pci() {
    let Some((report, net)) = virtio_service::start_net_stack(
        net_stack_image(),
        NET_TEST_TCP_ECHO,
        true,
        socket_contract::NO_LISTEN_GRANT,
    ) else {
        crate::testing::skip!("no virtio-net-pci device attached");
    };
    let verdict = sched::ipc_receive(report)[0];
    assert_eq!(
        verdict, NET_CLIENT_OK,
        "the TCP echo round trip over PCIe failed (client code {verdict:#x})",
    );
    net.release_or_fail("a net test's net_stack");
}

/// **Regression: reusing a socket id is safe** (the ephemeral-port fix). A client opens a TCP
/// socket on id 0, connects to the echo peer, closes it, then reopens the same id and connects
/// again. `net_stack` derived the local port from the socket id, so the reopen reused the exact
/// port and the second connect stalled on a slirp flow that had not cleared; the rotating allocator
/// hands the reopen a fresh port, so both connects complete. The client reports OK only if they do.
// RISC-V twin: `riscv_virtio_tests::a_reopened_socket_id_connects_again_over_tcp`. Gated here
// rather than run twice: that module drives the same property on the other instruction set through
// the same `block_driver` and `net_stack` binaries, and a second copy would double the suite's
// slowest tests to prove nothing.
#[cfg(target_arch = "aarch64")]
#[test_case]
fn a_reopened_socket_id_connects_again_over_tcp() {
    let Some((report, net)) = virtio_service::start_net_stack(
        net_stack_image(),
        NET_TEST_TCP_REOPEN,
        false,
        socket_contract::NO_LISTEN_GRANT,
    ) else {
        crate::testing::skip!("no virtio-net device attached");
    };
    let verdict = sched::ipc_receive(report)[0];
    assert_eq!(
        verdict, NET_CLIENT_OK,
        "reopening a socket id and connecting again failed (client code {verdict:#x}): the \
         ephemeral local port is not independent of the socket id",
    );
    net.release_or_fail("a net test's net_stack");
}

/// **The guest is connected TO, on a port it was granted** (milestone 107 (the socket contract
/// learns to accept)). The TCP gate, the UDP gate and the DHCP bring-up are all outbound; this is
/// the one where nife is reached.
///
/// A host process (xtask's inbound prober) connects to a port QEMU forwards into the guest
/// (`hostfwd`), sends a payload and reads back an answer the guest composed. The guest holds only a
/// `Stack` rendezvous and a shared frame, listens on the one port its spawn granted, and serves two
/// connections on one listener, which proves it re-arms. The listen and UDP bind grants are
/// refused, bound and collided in the same spawn; notes/net/the-inbound-half.md has both halves.
// RISC-V twin: `riscv_virtio_tests::a_host_process_connects_to_the_guest_and_is_answered`.
#[cfg(target_arch = "aarch64")]
#[test_case]
fn a_host_process_connects_to_the_guest_and_is_answered() {
    // E2's baseline (milestone 134 (the register of measures)): `thread_count()` counts every live
    // thread in the whole boot, and the suite runs in one boot, so only a delta against this
    // isolates this topology.
    let e2_baseline_threads = sched::thread_count();
    let Some((report, net)) = virtio_service::start_net_stack(
        net_stack_image(),
        NET_TEST_TCP_ACCEPT,
        false,
        socket_contract::listen_grant(NET_LISTEN_PORT, NET_LISTEN_PORT)
            | socket_contract::udp_bind_grant(NET_UDP_GRANT_BOTTOM, NET_UDP_GRANT_TOP),
    ) else {
        crate::testing::skip!("no virtio-net device attached");
    };
    // E2 (milestone 134, design/roadmap/0134-the-measurements-that-decide.md): the thread census on
    // the customer path. Every process this topology needs is spawned by now, and each is a
    // single-threaded event loop that spawns nothing per connection, so this is the peak. Its
    // topology is narrower than the one milestone 134 recorded, so the numbers are not comparable.
    // See notes/benchmarks.md.
    crate::println!(
        "    (E2 thread census: {} threads created by wiring this customer-path topology \
         ({} live now, {e2_baseline_threads} live before this test wired anything): main + \
         net_stack + echo client)",
        sched::thread_count().saturating_sub(e2_baseline_threads),
        sched::thread_count(),
    );
    let verdict = sched::ipc_receive(report)[0];
    assert_eq!(
        verdict, NET_CLIENT_OK,
        "the guest did not serve the inbound exchange (client code {verdict:#x}). 0xE050 or \
         0xE080 mean a port outside a grant was bound anyway, which is the capability failure; \
         0xE060 or 0xE070 means nobody ever connected, which is the host side: is the runner \
         adding a hostfwd (NIFE_HOSTFWD_PORT) and is xtask's inbound prober running beside this \
         suite? 0xE082 or 0xE084 mean the UDP bind grant admitted or refused the wrong port",
    );
    net.release_or_fail("a net test's net_stack");
}

/// The exact transcript `std_exerciser` prints when it is granted the network **and refused every
/// listening port**. Pinned so a drift in the net PAL, the contract, or the demo is a loud diff
/// rather than a mystery.
///
/// `listen refused` is milestone 64's negative control and it costs this boot nothing: the stack
/// this program is spawned with carries `socket_contract::NO_LISTEN_GRANT`, so
/// `std::net::TcpListener::bind` answers `PermissionDenied`, and the program says so on its way
/// past rather than quietly running a smaller demo. A lane that broke the grant check open would
/// turn this line into `listen ok` and fail here, in a transcript comparison, rather than passing
/// with more authority than it was given.
#[cfg(target_arch = "aarch64")]
const STD_NET_EXPECTED: &[u8] = b"std net on nife\nlisten refused\nudp ok\ntcp echo ok\n";

/// The exact transcript the same binary prints when its stack **is** granted the listening port
/// (milestone 64's inbound half). Four lines, and each is a separate claim: the granted port binds,
/// a port outside the grant is refused as a matter of authority, the granted port is exclusive, and
/// the listener served `socket_contract::fixture::ROUNDS` connections one after another.
#[cfg(target_arch = "aarch64")]
const STD_LISTEN_EXPECTED: &[u8] =
    b"std net on nife\nlisten ok\ndenied refused\nin use refused\nserved 2\n";

/// **`std::net` end to end over the socket contract** (milestone 27 phase two): the `std_exerciser`
/// std binary, given the network, does a real UDP DNS query and a TCP echo round trip through
/// `std::net::{UdpSocket, TcpStream}`, whose PAL binds to `net_stack`'s contract. The program never
/// sees a capability or a socket id; it writes to a socket and reads from it. This closes the
/// `net honestly unsupported` gap from phase one: std's networking runs on the native ABI,
/// reaching the same path the hand-written client does through std's blocking API. Its stdout
/// is reassembled off the rendezvous and compared byte for byte, the `std_exerciser` discipline.
// RISC-V twin: `riscv_virtio_tests::std_net_runs_over_the_socket_contract`. Gated here rather than
// run twice: that module drives the same property on the other instruction set through the same
// `block_driver` and `net_stack` binaries, and a second copy would double the suite's slowest tests
// to prove nothing.
#[cfg(target_arch = "aarch64")]
#[test_case]
fn std_net_runs_over_the_socket_contract() {
    if std_service::std_exerciser_image().is_none() {
        crate::testing::skip!(std_service::NO_STD_EXERCISER);
    }
    let Some((report, writer, net)) = virtio_service::start_net_std(
        net_stack_image(),
        std_exerciser_image(),
        socket_contract::NO_LISTEN_GRANT,
    ) else {
        crate::testing::skip!("no virtio-net device attached");
    };

    assert_std_transcript_from(report, writer, STD_NET_EXPECTED, "std net", false);
    net.release_or_fail("a net test's net_stack");
}

/// **A `std::net::TcpListener` serves a port it was granted, and refuses one it was not**
/// (milestone 64's inbound half, on milestone 107's contract).
///
/// The ordinary `std_exerciser`, over a stack granted exactly `NET_LISTEN_PORT`, binds it through
/// `std::net` and serves two connections a host opens through `hostfwd`; the second proves the
/// listener re-arms. `denied refused` (`fixture::DENIED_PORT`, `PermissionDenied`) and `in use
/// refused` (`AddrInUse`) ride the same spawn, because a boot is the expensive unit; the sibling
/// test above is the stack with no grant. xtask's inbound prober checks the answer was the guest's
/// own, which the guest cannot know. See notes/net/std-tcp-listener.md.
// RISC-V twin: `riscv_virtio_tests::a_std_program_serves_a_granted_listening_port`.
#[cfg(target_arch = "aarch64")]
#[test_case]
fn a_std_program_serves_a_granted_listening_port() {
    if std_service::std_exerciser_image().is_none() {
        crate::testing::skip!(std_service::NO_STD_EXERCISER);
    }
    let Some((report, writer, net)) = virtio_service::start_net_std(
        net_stack_image(),
        std_exerciser_image(),
        socket_contract::listen_grant(NET_LISTEN_PORT, NET_LISTEN_PORT),
    ) else {
        crate::testing::skip!("no virtio-net device attached");
    };

    assert_std_transcript_from(report, writer, STD_LISTEN_EXPECTED, "std listen", true);
    net.release_or_fail("a net test's net_stack");
}
