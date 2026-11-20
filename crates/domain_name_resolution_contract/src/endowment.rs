/// Ask over UDP, and again over TCP when the reply comes back truncated (`RFC 1035` and `RFC 7766`'s
/// ordinary behavior).
pub const TRANSPORT_UDP: u64 = 0;
/// Ask over TCP only (`RFC 7766` lets a stub resolver do this). For a name server that does not
/// answer UDP at all, which is the QEMU runners' peer: slirp forwards only TCP to a host process.
pub const TRANSPORT_TCP: u64 = 1;

/// The first start word: the name server's IPv4 address, big-endian in the low 32 bits, the
/// format `socket_contract::lease::ipv4_word` reports a lease's name server in.
pub const fn server(octets: [u8; 4]) -> u64 {
    u32::from_be_bytes(octets) as u64
}

/// The first start word back to an address.
pub const fn server_octets(word: u64) -> [u8; 4] {
    (word as u32).to_be_bytes()
}

/// The second start word: the name server's port and the transport.
pub const fn port_and_transport(port: u16, transport: u64) -> u64 {
    port as u64 | transport << 16
}

/// Whether a transport word is one this contract defines, `TRANSPORT_UDP` or `TRANSPORT_TCP`. The
/// resolver refuses any other word at start.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane milestone/1086-rulings.
pub const fn is_known_transport(transport: u64) -> bool {
    transport == TRANSPORT_UDP || transport == TRANSPORT_TCP
}

/// The second start word's `(port, transport)`.
pub const fn port_and_transport_fields(word: u64) -> (u16, u64) {
    (word as u16, word >> 16)
}

/// **Slot 4: the resolver's own address space, `WRITE`**, so a forget can `UNMAP` the page the
/// forgotten badge attached, as a network stack gives up a closed socket's page through
/// `socket_contract::stack_slots::OWN_SPACE`. Slots 0 to 3 are the resolver's own business, set by
/// the resolver program; this one is here because the program that spawns the resolver and the
/// test harness that spawns it both place it. Provisional.
///
/// Name: provisional, milestone 892's lane (#2011), 2026-10-12 (UTC).
/// After `socket_contract::stack_slots::OWN_SPACE`; the pull request records no other name
/// considered.
pub const OWN_SPACE_SLOT: u64 = 4;
