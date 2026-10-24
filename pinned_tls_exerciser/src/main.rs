//! **A TLS 1.3 handshake on nife, with a peer that is not ours, trusting one pinned root**, for
//! milestone 501 (a TLS client that speaks to one pinned peer).
//!
//! It connects through `std::net` to `helpers/tls-peer` (OpenSSL, run by slirp at 10.0.2.9:8443
//! per connection) and prints five lines: a handshake, an index body, 256 KiB of bulk, and two
//! refusals, a chain from another root and the pinned root's chain for another name. Only a client
//! that does what the pin says prints all five. The `cost` lines are milestone 501's measurement,
//! printed rather than asserted because they vary with the host. See notes/packages/pinned-tls.md.

//! # BUGS
//!
//! - The pinned root is the test authority, not ISRG Root X1: no gate reaches the internet, so X1
//!   meets a real chain only in `pinned_tls_client`'s ignored host tests.
//! - The timings are QEMU's under TCG on one host, comparable across architectures only there.

// `entropy_backend` defines `getrandom`'s custom-backend symbols and nothing references them from
// Rust, so an rlib nobody names is not linked. See that crate's docs.
use std::net::TcpStream;
use std::time::Instant;

use entropy_backend as _;
use pinned_tls_client::{Error, PinnedPeer, Session};

/// slirp's `guestfwd` to helpers/tls-peer, in each QEMU runner.
const PEER: &str = "10.0.2.9:8443";

/// The test authority a client pins to reach helpers/tls-peer as `basalt.test`. Test-only, as
/// pinned_tls_client/fixtures/regenerate.sh says.
const PINNED_ROOT: &[u8] = include_bytes!("../../pinned_tls_client/fixtures/pinned-root.der");

const INDEX: &[u8] = b"basalt test index: served over TLS 1.3 by helpers/tls-peer\n";
const BULK: usize = 256 * 1024;

fn milliseconds(since: Instant) -> u128 {
    since.elapsed().as_millis()
}

fn connect() -> TcpStream {
    TcpStream::connect(PEER)
        .unwrap_or_else(|e| panic!("could not reach helpers/tls-peer at {PEER}: {e}"))
}

/// The handshake for `name`, expecting a certificate refusal, and which one.
fn refused(name: &str) -> rustls::CertificateError {
    let peer = PinnedPeer::new(name, PINNED_ROOT).unwrap();
    match Session::handshake(&peer, connect()) {
        Err(Error::Tls(rustls::Error::InvalidCertificate(why))) => why,
        Err(other) => panic!("{name}: refused, but not for its certificate: {other:?}"),
        Ok(_) => panic!("{name}: the handshake was accepted and must not have been"),
    }
}

fn main() {
    // A panic under `panic = "abort"` traps before the sink's end-of-stream marker is sent, so the
    // reader never sees the message. Print it and leave through `exit`, which sends the marker.
    // `cryptography_exerciser` carries the story of the day this cost.
    std::panic::set_hook(Box::new(|info| {
        println!("PANIC {info}");
        std::process::exit(101);
    }));
    println!("pinned_tls_exerciser start");

    let peer = PinnedPeer::new("basalt.test", PINNED_ROOT).unwrap();

    // The handshake, timed apart from the TCP connect so the cost line says what TLS added.
    let started = Instant::now();
    let tcp = connect();
    let connected = milliseconds(started);
    let handshake_started = Instant::now();
    let session =
        Session::handshake(&peer, tcp).unwrap_or_else(|e| panic!("handshake refused: {e:?}"));
    let handshake = milliseconds(handshake_started);
    println!(
        "handshake ok {:?} {:?}",
        session
            .cipher_suite()
            .expect("a completed handshake has a suite"),
        session
            .key_exchange_group()
            .expect("a completed handshake has a group"),
    );
    let mut body = Vec::new();
    let status = session
        .get("/index", |part| body.extend_from_slice(part))
        .unwrap();
    assert_eq!(status, 200, "the index answered {status}");
    assert_eq!(body, INDEX, "the index body arrived altered");
    println!("index ok {} bytes", body.len());
    println!("cost connect {connected} ms, handshake {handshake} ms");

    // Bulk: the record layer under load, and the cost of a byte once the handshake is paid.
    let session = Session::handshake(&peer, connect()).unwrap();
    let transfer_started = Instant::now();
    let mut received = 0usize;
    let mut intact = true;
    let status = session
        .get(&format!("/bytes/{BULK}"), |part| {
            for &b in part {
                intact &= b == received as u8;
                received += 1;
            }
        })
        .unwrap();
    let transfer = milliseconds(transfer_started);
    assert_eq!(status, 200);
    assert_eq!(received, BULK, "the bulk body was {received} bytes");
    assert!(intact, "the bulk body's pattern was broken");
    println!("bulk ok {BULK} bytes");
    println!("cost bulk {BULK} bytes in {transfer} ms");

    let why = refused("stranger.test");
    assert_eq!(
        why,
        rustls::CertificateError::UnknownIssuer,
        "stranger.test was refused for {why:?}"
    );
    println!("refused stranger.test: unknown issuer");

    let why = refused("elsewhere.test");
    assert!(
        matches!(
            why,
            rustls::CertificateError::NotValidForName
                | rustls::CertificateError::NotValidForNameContext { .. }
        ),
        "elsewhere.test was refused for {why:?}",
    );
    println!("refused elsewhere.test: not valid for name");

    println!("pinned_tls_exerciser done");
}
