//! **Milestone 30 (the network stack as a confined component)'s gates, over the network card a PC
//! actually has** (milestone 494 (a driver for the network card a PC actually has); notes/e1000e.md).
//!
//! The same `net_stack`, the same smoltcp, the same socket contract and the same client role as
//! the virtio gates in `network_tests.rs`, with one thing changed: the NIC is QEMU's `e1000e` (an Intel
//! 82574L, the family xenon's I219 belongs to), reset and programmed by the kernel and driven at
//! EL0 through two pages of BAR0 and a DMA region confined by the IOMMU. **One module for all three
//! architectures**, because this is the first NIC the x86_64 leg has, and the parity claim is that
//! the same cases pass on each.
//!
//! What a pass here is not: a claim about the I219. QEMU's model is the 82574L; xenon's bench step
//! (notes/e1000e.md) is what proves the real part.

use super::*;

/// `socket_test_client`'s TCP echo selector and its success word; the same numbers `network_tests.rs` and
/// `riscv_virtio_tests` spell for the virtio gates (`components/src/socket_test_client.rs`).
const NET_TEST_TCP_ECHO: u64 = 2;
/// `socket_test_client`'s UDP selector against slirp's TFTP server.
const NET_TEST_UDP_TFTP: u64 = 4;
const NET_CLIENT_OK: u64 = 1;
/// `socket_test_client`'s HTTP package fetch selector, and its refusal word for a bad digest.
const NET_TEST_HTTP_PACKAGE: u64 = 6;
const NET_CLIENT_DIGEST_REFUSED: u64 = 3;

fn image() -> &'static [u8] {
    program("net_stack").expect("no net_stack program in the initial ramdisk")
}

/// **The lease, and the confinement it was won under.** smoltcp's DHCP round trip completes over
/// the `e1000e` data plane at EL0, and the address lands in slirp's 10.0.2.0/24, which only a real
/// exchange through the device can produce. The IOMMU that owns the NIC was translating it: on this
/// device the queue pages carry the ring base registers, so the IOMMU is the whole of the DMA
/// confinement and a pass without it would prove the driver and not the claim.
///
/// Falsification: attested 2026-10-04. On patagonia, x86_64 under the PVH runner, with
/// `kernel/src/e1000e.rs::region_for`'s `iommu::confine` call skipped so the NIC had no domain:
/// QEMU printed `vtd_iommu_translate: detected translation failure (dev=00:05:00, iova=0x920000)`,
/// the device's first fetch of its receive ring, and no lease ever came. **That failure is a hang,
/// not an assertion**: `net_stack`'s DHCP loop has no bound and this test blocks on its report, so
/// the run was stopped by hand after eleven minutes, which is why this is attested rather than a
/// replayable patch.
#[test_case]
fn net_stack_gets_a_dhcp_lease_over_the_e1000e_nic_behind_the_iommu() {
    let w = wired_or_skip!(e1000e_service::start_net_server(
        image(),
        socket_contract::NO_LISTEN_GRANT
    ));
    assert!(
        w.confined_by_iommu,
        "the e1000e NIC (device {:#06x}) was not behind a translating IOMMU unit that owns it",
        w.device
    );
    // Every runner gives this NIC `mac=52:54:00:e1:00:0e`, which is neither QEMU's default nor the
    // virtio server's constant: seeing it here proves the address came from the device's
    // receive-address register, through the kernel, into the handoff.
    assert_eq!(w.mac, [0x52, 0x54, 0x00, 0xe1, 0x00, 0x0e]);
    // QEMU's 82574L PHY identifies as 0x0141:0x0cb1 (`hw/net/e1000x_regs.h`,
    // `E1000_PHY_ID2_82574x`). Reading it proves the MDIO primitive the I219 bring-up stands on
    // (`crates/e1000e/src/pch/phy.rs`'s `read_mdic`) against a device rather than a simulation.
    assert_eq!(
        w.phy_id, 0x0141_0cb0,
        "the PHY identifier read over MDIO was {:#010x}",
        w.phy_id
    );
    let addr = crate::sched::ipc_receive(w.report)[0] as u32;
    assert_eq!(
        addr & 0xffff_ff00,
        0x0A00_0200,
        "net_stack's DHCP lease over the e1000e NIC, {addr:#010x}, is not in slirp's 10.0.2.0/24",
    );
    w.held.release_or_fail("net_stack over the e1000e NIC");
}

/// **TCP end to end**: a client of the socket contract connects to slirp's echo peer
/// (10.0.2.9:7777), sends, reads the echo back and closes, through `net_stack` and the `e1000e`
/// data plane. Milestone 30's TCP gate, byte for byte the virtio one's client.
///
/// Falsification: unfalsified. The confinement half of this path is the DHCP test's, attested
/// above; nobody has broken the exchange this test adds to watch it fail.
#[test_case]
fn a_client_echoes_over_tcp_through_the_e1000e_nic() {
    let (report, w) = wired_or_skip!(e1000e_service::start_net_stack(
        image(),
        NET_TEST_TCP_ECHO,
        socket_contract::NO_LISTEN_GRANT,
    ));
    let verdict = crate::sched::ipc_receive(report)[0];
    assert_eq!(
        verdict, NET_CLIENT_OK,
        "the TCP echo over the e1000e NIC failed (client code {verdict:#x})",
    );
    w.held.release_or_fail("net_stack over the e1000e NIC");
}

/// **UDP end to end**, against slirp's own TFTP server: the datagram path, and a second wiring of
/// the same device in one boot, which is what proves the kernel's reset-and-reprogram leaves the
/// rings in a state a fresh data plane can start from.
///
/// Falsification: unfalsified. The confinement half of this path is the DHCP test's, attested
/// above; nobody has broken the exchange this test adds to watch it fail.
#[test_case]
fn a_client_completes_a_udp_round_trip_through_the_e1000e_nic() {
    let (report, w) = wired_or_skip!(e1000e_service::start_net_stack(
        image(),
        NET_TEST_UDP_TFTP,
        socket_contract::NO_LISTEN_GRANT,
    ));
    let verdict = crate::sched::ipc_receive(report)[0];
    assert_eq!(
        verdict, NET_CLIENT_OK,
        "the UDP round trip over the e1000e NIC failed (client code {verdict:#x})",
    );
    w.held.release_or_fail("net_stack over the e1000e NIC");
}

/// **A real transfer: a package over HTTP, accepted only by the image's digest** (milestone 198 (a
/// package manager) rung 3a's gate, over the NIC rung 3b exists for). The client `GET`s a package
/// from `helpers/package-http-peer` through `net_stack` and this data plane, hashes it as it
/// arrives, and checks it against the image's catalogue; then it fetches the peer's copy with one
/// byte flipped, which must be refused. Tens of kilobytes over TCP, twice, in full-size frames, which the echo's
/// few bytes never exercise.
///
/// Falsification: unfalsified. The confinement half of this path is the DHCP test's, attested
/// above; nobody has broken the exchange this test adds to watch it fail.
#[test_case]
fn a_package_fetched_over_the_e1000e_nic_is_accepted_only_by_the_image_digest() {
    let catalogue = program(package_archive::CATALOG)
        .expect("no package catalogue in the initial ramdisk: the archive build packs one");
    let (report, w) = wired_or_skip!(e1000e_service::start_package_fetch(
        image(),
        NET_TEST_HTTP_PACKAGE,
        catalogue.len() as u64,
        catalogue,
    ));
    let [genuine, tampered, ..] = crate::sched::ipc_receive(report);
    assert_eq!(
        genuine, NET_CLIENT_OK,
        "the package fetched over the e1000e NIC was not accepted (client code {genuine:#x}; \
         {NET_CLIENT_DIGEST_REFUSED} means its digest disagreed with the image's catalogue)",
    );
    assert_eq!(
        tampered, NET_CLIENT_DIGEST_REFUSED,
        "a package with one byte flipped was not refused by digest over the e1000e NIC (client \
         code {tampered:#x})",
    );
    w.held.release_or_fail("net_stack over the e1000e NIC");
}
