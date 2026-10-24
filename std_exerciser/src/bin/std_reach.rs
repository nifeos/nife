//! **`std_reach`: an installed `std` program that holds the network and a resolver because the
//! owner granted them at install** (milestone 892 (a std program at the prompt holds the network
//! and a resolver)). `script/swish-check` installs it as `reach`, granting the network and the
//! zone `nife.test`; at the prompt it connects by address to the echo peer and looks up a name
//! outside its zone, which the resolver refuses without sending a packet. Without the grant the
//! prompt refuses it by name of the network; with the network and no resolver the lookup reads
//! `Unsupported`.

//! # EXAMPLES
//!
//! ```text
//! $ reach
//! reach: echo ok
//! reach: lookup example.com: PermissionDenied
//! ```
//!
//! Name: provisional, minted 2026-10-19 (UTC) by milestone/892-installed-programs-hold-the-network-and-zones.

use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream, ToSocketAddrs};

use socket_contract::fixture::{ECHO_PEER_IP, ECHO_PEER_PORT};

// What it asks the owner for: its words (every `std` image hears them), its output, the network,
// and names in `nife.test`.
capability_manifest_note::carry!(
    grant_plan::Manifest {
        program_argument: grant_plan::ProgramArgumentDeclaration::Words(
            grant_plan::WordGrant::ReadOnly
        ),
        network: true,
        startup: grant_plan::StartupContract::Std,
        ..grant_plan::NO_NOTE_MANIFEST
    },
    zones = &["nife.test"]
);

fn main() {
    const MSG: &[u8] = b"nife-reach";
    let peer = SocketAddr::from((Ipv4Addr::from(ECHO_PEER_IP), ECHO_PEER_PORT));
    let echoed = TcpStream::connect(peer).and_then(|mut s| {
        s.write_all(MSG)?;
        let mut back = [0u8; MSG.len()];
        s.read_exact(&mut back)?;
        Ok(back == *MSG)
    });
    match echoed {
        Ok(true) => println!("reach: echo ok"),
        Ok(false) => println!("reach: echo came back altered"),
        Err(e) => println!("reach: echo: {:?}", e.kind()),
    }
    match ("example.com", ECHO_PEER_PORT).to_socket_addrs() {
        Ok(_) => println!("reach: lookup example.com: resolved, outside the granted zone"),
        Err(e) => println!("reach: lookup example.com: {:?}", e.kind()),
    }
}
