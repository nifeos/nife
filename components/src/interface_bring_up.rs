//! **The network server's start-up** (`net_stack`'s `server`): this process's heap, the smoltcp
//! interface over the NIC, and the DHCP exchange whose lease the server reports before it serves a
//! single socket.
//!
//! Name: provisional, the long-function paydown lane, 2026-10-12 (UTC). Moved out of `net_stack.rs`
//! when `server` was split for milestone 888 (a ratchet on function length), because §282 (when a
//! file is split, the parent becomes an index) gives a split parent's new code nowhere to land but
//! a child named for it.

use super::*;

/// Set up this process's heap, then the smoltcp interface over `dev`, seeded from the clock.
pub(super) fn heap_and_interface(dev: &mut Nic) -> Interface {
    HEAP.init(
        MEMORY_REGION,
        memory_region_heap::DEFAULT_BASE,
        HEAP_MAX,
    );

    let mut config = Config::new(HardwareAddress::Ethernet(EthernetAddress(dev.mac())));
    config.random_seed = now();
    Interface::new(config, dev, instant())
}

/// Run DHCP until the interface is configured, then report the lease on [`REPORT`].
pub(super) fn dhcp_lease(iface: &mut Interface, dev: &mut Nic, sockets: &mut SocketSet) {
    let dhcp = sockets.add(dhcpv4::Socket::new());
    loop {
        iface.poll(instant(), dev, sockets);
        if let Some(dhcpv4::Event::Configured(cfg)) = sockets.get_mut::<dhcpv4::Socket>(dhcp).poll()
        {
            iface.update_ip_addrs(|addrs| {
                let _ = addrs.push(IpCidr::Ipv4(cfg.address));
            });
            if let Some(router) = cfg.router {
                let _ = iface.routes_mut().add_default_ipv4_route(router);
            }
            // The lease report's layout is `socket_contract::lease`: the address, then the first
            // DNS server the lease named, which a resolver's spawner hands on (§248 (the name
            // resolver is its own confined program)).
            let address = socket_contract::lease::ipv4_word(cfg.address.address().octets());
            let nameserver = cfg
                .dns_servers
                .first()
                .map_or(socket_contract::lease::NO_NAMESERVER, |a| {
                    socket_contract::lease::ipv4_word(a.octets())
                });
            send(REPORT, address, nameserver, 0);
            break;
        }
        // Same discipline as service_until: block on the interrupt only when smoltcp has no timer
        // pending, so a dropped DHCP OFFER is retried by smoltcp's own DISCOVER retransmit rather
        // than deadlocking on an interrupt that will not come.
        wait_for_nic(iface, dev, sockets, None);
    }
}
