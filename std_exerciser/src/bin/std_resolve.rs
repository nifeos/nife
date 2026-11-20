//! **`std_resolve`: `ToSocketAddrs` on nife asks the resolver this program was granted, and
//! nothing else** (milestone 801 (packages over the internet), item 3).
//!
//! The kernel test (`a_std_program_resolves_its_granted_zone_and_nothing_without_a_grant` in
//! `system_tests/src/user/name_resolver_tests.rs`) starts it holding a resolver badge granted
//! `nife.test` at `RESOLVER_SLOT`. Each name it asks is a case of `domain_name_resolution_contract::fixture`,
//! so the zone, the peer and the answers are the ones milestone 384 (in a capability system the
//! resolver is a grant)'s native client is judged by, and this program adds only the std half:
//! `lookup_host` in `patches/std-nife`. Run with the slot empty, every lookup is `Unsupported`.
//!
//! Name: provisional 2026-10-09 (UTC), milestone 801's lane, beside `std_echo` and `std_grep`.

use std::io::{ErrorKind, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};

use domain_name_resolution_contract::fixture::CASES;
use domain_name_resolution_contract::status;
use socket_fixture::ECHO_PEER_PORT;

/// The first case whose expected reply has this status.
fn case(want: u8) -> &'static str {
    CASES
        .iter()
        .find(|c| c.status == want)
        .map(|c| c.name)
        .expect("the fixture has a case for every status this program asks")
}

fn lookup(name: &str) {
    match (name, ECHO_PEER_PORT).to_socket_addrs() {
        Ok(mut addresses) => match addresses.next() {
            Some(a) => println!("lookup {name}: {a}"),
            None => println!("lookup {name}: no address"),
        },
        // The kind is what the test matches. A refused network step also says which step, since
        // the one CI failure of 2026-10-12 (UTC) printed only the kind.
        Err(e) if e.kind() == ErrorKind::NetworkUnreachable => {
            println!("lookup {name}: {:?} ({e})", e.kind())
        }
        Err(e) => println!("lookup {name}: {:?}", e.kind()),
    }
}

fn main() {
    println!("std_resolve start");
    let inside = case(status::OK);
    for name in [inside, case(status::NO_SUCH_NAME), case(status::DENIED)] {
        lookup(name);
    }
    // By name, the way a program written for any OS connects: the address never appears here.
    const MSG: &[u8] = b"nife-std-by-name";
    let echoed = TcpStream::connect((inside, ECHO_PEER_PORT)).and_then(|mut s| {
        s.write_all(MSG)?;
        let mut back = [0u8; MSG.len()];
        s.read_exact(&mut back)?;
        Ok(back == *MSG)
    });
    match echoed {
        Ok(true) => println!("echo by name ok"),
        Ok(false) => println!("echo by name: the bytes came back altered"),
        Err(e) => println!("echo by name: {:?}", e.kind()),
    }
    println!("std_resolve done");
}
