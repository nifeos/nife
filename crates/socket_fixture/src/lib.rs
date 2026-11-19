#![no_std]
//! **The socket test fixture**: the fixed addresses, ports and messages of the QEMU runners'
//! network tests, in one place so the programs that touch them agree: the in-guest clients, the
//! standard library exerciser, the package-source client and the kernel tests. They are a test
//! harness's facts, not part of the socket contract. The host's copy of the inbound gate's facts is
//! deliberately not here: the host side spells them again, so the gate proves two independently
//! written sides agree. Its one development dependency is `socket_contract`, whose grants it is checked against.
//!
//! Not published: nothing outside this tree has a reason to read it.
//!
//! Name: provisional, minted 2026-11-19 (UTC) by lane 1108-renames-scope; the crate holds what
//! `socket_contract` held as its `fixture` module. Considered `socket_test_fixture`, which repeats
//! what `fixture` already says, and followed `filesystem_fixture`. Calef names crates.

/// The port the inbound gate listens on, and the guest side of the runners' `hostfwd`. Both
/// architecture legs use the same number because they run one after the other and never hold it at once.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane 1108-renames-scope.
pub const LISTEN_PORT: u16 = 7778;
/// A port deliberately outside every listen grant the tests hand out, so asking for it
/// proves the grant *refuses* rather than that nothing happened to bind. It is the whole of the
/// authority claim: a client that could bind this bound a port nothing granted.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane 1108-renames-scope.
pub const DENIED_PORT: u16 = 8080;

/// What the host sends in, and what the guest answers with. Different strings on purpose: an
/// echo would pass even if the guest were only reflecting the host's own bytes, and the point
/// of the gate is that the guest *composed* an answer to a connection it did not make.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane 1108-renames-scope.
pub const INBOUND_MESSAGE: &[u8] = b"nife-in!";
/// What the guest answers with, composed rather than echoed (see the field-group docs).
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane 1108-renames-scope.
pub const OUTBOUND_MESSAGE: &[u8] = b"nife-out!";

/// How many connections one listener serves before the client stops. Two, and the second is
/// the load-bearing one: a listener that accepts exactly one connection would pass a
/// one-round gate and is precisely what a file server cannot use.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane 1108-renames-scope.
pub const ROUNDS: usize = 2;

/// The runners' Transmission Control Protocol (TCP) echo peer: a `guestfwd` inside QEMU's user-mode network that pipes every
/// connection to `10.0.2.9:7777` into a fresh `/bin/cat`. Two binaries dial it, so it is here; the runners spell it a third
/// time for [`LISTEN_PORT`]'s reason.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane 1108-renames-scope.
pub const ECHO_PEER_ADDRESS: [u8; 4] = [10, 0, 2, 9];
/// The echo peer's port; see [`ECHO_PEER_ADDRESS`].
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane 1108-renames-scope.
pub const ECHO_PEER_PORT: u16 = 7777;

/// The runners' package source: a `guestfwd` that runs a package server once per
/// connection to `10.0.2.9:8080`, serving packages over HTTP/1.0. Two programs dial it. Only
/// reachable inside a QEMU runner.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane 1108-renames-scope.
pub const PACKAGE_PEER_ADDRESS: [u8; 4] = [10, 0, 2, 9];
/// The package source's port; see [`PACKAGE_PEER_ADDRESS`]. The same number as [`DENIED_PORT`], and
/// unrelated to it: that is a port a *guest* may not listen on, this is one on a *host* the
/// guest dials.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane 1108-renames-scope.
pub const PACKAGE_PEER_PORT: u16 = 8080;
/// The `Host` header a request to the package source carries: [`PACKAGE_PEER_ADDRESS`], spelled.
///
/// Name: provisional, minted 2026-11-19 (UTC) by lane 1108-renames-scope.
pub const PACKAGE_PEER_HOST: &str = "10.0.2.9";

#[cfg(test)]
mod tests {
    use socket_contract::{NO_PORT_GRANT, listen_grant, listen_grant_allows};

    use super::*;

    /// The inbound fixture's own claim, checked: the port the gate is granted is inside the
    /// grant the tests hand out and the port it probes is outside it. Both device legs spend
    /// minutes proving the refusal on real hardware-shaped paths, and neither of them would notice
    /// if somebody edited [`DENIED_PORT`] to a number inside the range: the client would
    /// bind it, print that it bound it, and every assertion would still pass. This is the check
    /// that a "denied" port is denied.
    #[test]
    fn the_inbound_fixture_probes_a_port_no_test_grants() {
        let g = listen_grant(LISTEN_PORT, LISTEN_PORT);
        assert!(listen_grant_allows(g, LISTEN_PORT));
        assert!(
            !listen_grant_allows(g, DENIED_PORT),
            "the port the gate calls denied is inside the grant it is handed",
        );
        // And the wider range the combined inbound boot hands out (the SMB port rides beside the
        // listen port) must not reach it either.
        let wide = listen_grant(LISTEN_PORT, LISTEN_PORT + 1);
        assert!(!listen_grant_allows(wide, DENIED_PORT));
        // The default admits neither, which is what "granted, never assumed" means for the fixture.
        assert!(!listen_grant_allows(NO_PORT_GRANT, LISTEN_PORT));
    }
}
