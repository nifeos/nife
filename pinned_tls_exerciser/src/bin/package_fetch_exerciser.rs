//! **A package fetched through the distribution's index, under calef's five rulings of 2026-10-10
//! (UTC) on #1884**, for milestone 801 (packages over the internet), items 1, 3 and 4.
//!
//! `system_tests/src/user/package_index_tests.rs` starts it holding the network, a clock, entropy
//! and a resolver badge granted the root zone, as `jig`'s is (Q5). It then:
//!
//! | line | what it proves |
//! |---|---|
//! | `index unreachable at gone.basalt.test` | the image's first address is lost, so it tries the second (Q2) |
//! | `index ok ... from basalt.test` | the backup resolved by name, TLS 1.3 to the pinned root, the index read from `/<channel>/metadata/` (Q4) |
//! | `passed over https://packages.basalt.test...` | a listed location resolving to a private address is refused (Q1) |
//! | `fetched greeting-... from basalt.test's targets` | the repository's own copy is the fallback, admitted by the index's digest |
//! | `refused uptime-...: NotCataloged` | a repository copy with one byte flipped: only the digest can tell |
//! | `refused nosuch: not in the index` | nothing is fetched for a name the index does not list |
//! | `passed over https://rebind.basalt.test...` then `fetched rebound-... from basalt.test's targets` | the twin listed under the rebinding name: what its refusal says is the record of the address-check attack (milestone 868 (a sixth outsider pass attacks the confinement claim)) |
//!
//! It installs nothing: installing from an index is milestone 809 (the package client becomes a
//! program)'s `jig`. Under slirp every listed location is private, so taking bytes from one is
//! proved only by `package_index`'s host tests.
//!
//! Name: provisional 2026-10-09 (UTC), milestone 801's lane, `pinned_tls_exerciser`'s shape.

use std::net::{SocketAddr, TcpStream, ToSocketAddrs};

use entropy_backend as _;
use package_index::fixture::{ABSENT, BACKUP_HOST, GENUINE, INDEX_PORT, PRIMARY_HOST, REBOUND, TAMPERED};
use package_index::{
    Entry, Index, Location, Miss, PATH_MAX, PROVISIONAL_CHANNEL, Repository, STAND_IN_INDEX_FILE,
    Source, accept, public_address,
};
use pinned_tls_client::{PinnedPeer, Session};

/// The test authority `helpers/tls-peer` chains `basalt.test` to. Test-only. Listed locations are
/// pinned to it too: §196 (nife carries TLS) holds one root per source, and which root a listed
/// location's host must chain to is a question this lane put back on #1884.
const PINNED_ROOT: &[u8] = include_bytes!("../../../pinned_tls_client/fixtures/pinned-root.der");

/// One `GET` over TLS to `host:port`, pinned to the test root. The body, or why not.
fn get(host: &str, port: u16, path: &str) -> Result<Vec<u8>, String> {
    let peer = PinnedPeer::new(host, PINNED_ROOT).map_err(|e| format!("{e:?}"))?;
    let tcp = TcpStream::connect((host, port)).map_err(|e| format!("{:?}", e.kind()))?;
    let session = Session::handshake(&peer, tcp).map_err(|e| format!("{e:?}"))?;
    let mut body = Vec::new();
    match session.get(path, |part| body.extend_from_slice(part)) {
        Ok(200) => Ok(body),
        Ok(status) => Err(format!("status {status}")),
        Err(e) => Err(format!("{e:?}")),
    }
}

/// The index from the first of the image's addresses that answers (Q2).
fn fetch_index<'r>(addresses: &'r [Repository<'r>]) -> (String, &'r Repository<'r>) {
    for repo in addresses {
        let mut buf = [0u8; PATH_MAX];
        let path = repo.metadata_path(STAND_IN_INDEX_FILE, &mut buf).unwrap();
        match get(repo.host, repo.port, path) {
            Ok(body) => return (String::from_utf8(body).expect("the index is not text"), repo),
            Err(why) => println!("index unreachable at {}: {why}", repo.host),
        }
    }
    panic!("no index address answered");
}

/// A listed location, if every address its host resolves to is public (Q1's safeguard).
fn from_location(at: &Location<'_>) -> Result<Vec<u8>, String> {
    let addresses: Vec<SocketAddr> = (at.host, at.port)
        .to_socket_addrs()
        .map_err(|e| format!("{:?}", e.kind()))?
        .collect();
    for a in &addresses {
        if let SocketAddr::V4(v4) = a {
            if !public_address(v4.ip().octets()) {
                return Err(format!("a private address ({})", v4.ip()));
            }
        }
    }
    get(at.host, at.port, at.path)
}

/// Ask the index for `name`, try its sources in order, and say what happened.
fn install_check(index: &Index<'_>, from: &Repository<'_>, name: &str, architecture: &str) {
    let entry: Entry<'_> = match index.find(name, architecture) {
        Ok(entry) => entry,
        Err(Miss::NoSuchPackage) => return println!("refused {name}: not in the index"),
        Err(Miss::SeveralVersions) => return println!("refused {name}: several versions"),
    };
    for source in Index::sources(&entry, from, None) {
        let (bytes, whence) = match source {
            Source::Listed(at) => match from_location(&at) {
                Ok(bytes) => (bytes, format!("{}:{}", at.host, at.port)),
                Err(why) => {
                    println!("passed over https://{}:{}{}: {why}", at.host, at.port, at.path);
                    continue;
                }
            },
            Source::Repository(repo) => {
                let mut buf = [0u8; PATH_MAX];
                let path = repo.target_path(entry.stem, &mut buf).unwrap();
                match get(repo.host, repo.port, path) {
                    Ok(bytes) => (bytes, format!("{}'s targets", repo.host)),
                    Err(why) => return println!("refused {}: fetch failed: {why}", entry.stem),
                }
            }
        };
        return match accept(&entry, &bytes) {
            Ok(ok) => println!(
                "fetched {} from {whence}, {} bytes, program {}",
                entry.stem,
                bytes.len(),
                ok.program
            ),
            Err(why) => println!("refused {}: {why:?}", entry.stem),
        };
    }
}

fn main() {
    // The panic-to-exit hook `pinned_tls_exerciser` carries, for its reason.
    std::panic::set_hook(Box::new(|info| {
        println!("PANIC {info}");
        std::process::exit(101);
    }));
    println!("package_fetch_exerciser start");
    let addresses = [
        Repository::new(PRIMARY_HOST, INDEX_PORT, PROVISIONAL_CHANNEL).unwrap(),
        Repository::new(BACKUP_HOST, INDEX_PORT, PROVISIONAL_CHANNEL).unwrap(),
    ];
    let (text, from) = fetch_index(&addresses);
    let index = Index::parse(&text).unwrap_or_else(|bad| panic!("index line refused: {bad:?}"));
    println!(
        "index ok {} packages from {} over TLS",
        index.entries().count(),
        from.host
    );
    let architecture = std::env::consts::ARCH;
    for name in [GENUINE, TAMPERED, ABSENT, REBOUND] {
        install_check(&index, from, name, architecture);
    }
    println!("package_fetch_exerciser done");
}
