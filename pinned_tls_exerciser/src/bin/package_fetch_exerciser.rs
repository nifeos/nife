//! **A package fetched through the distribution's index, under calef's five rulings of 2026-10-10
//! (UTC) on #1884**, for milestone 801 (packages over the internet), items 1, 3 and 4.
//!
//! `system_tests/src/user/package_index_tests.rs` starts it holding the network, a clock, entropy
//! and a resolver badge granted the root zone, as `jig`'s is (Q5), and checks each line it prints:
//! the index's fallback address (Q2), a private listed location passed over (Q1), the digest's
//! refusals, the rebinding twin (milestone 871 (a sixth outsider pass attacks the confinement
//! claim)) and the redirect probes (calef's C2 on #2048), which go through `listed::fetch` as `jig`
//! does. It installs nothing. notes/packages/pinned-tls.md has the line-by-line table.
//!
//! Name: provisional 2026-10-09 (UTC), milestone 801's lane, `pinned_tls_exerciser`'s shape.

//! # BUGS
//!
//! - Only a test guards the rebinding fix outside the listed path: a caller could resolve again
//!   and hand [`get`] the fresh answer; `package_index_tests`' rebinding assertion catches it.
//! - Under slirp the client dials the checked public address, 192.0.2.1 (RFC 5737), for real, and
//!   waits out the net server's 15 s connect bound.

use std::net::{SocketAddr, TcpStream, ToSocketAddrs};

use entropy_backend as _;
use package_index::fixture::{
    ABSENT, BACKUP_HOST, GENUINE, INDEX_PORT, PRIMARY_HOST, REBOUND, TAMPERED,
};
use package_index::{
    AddressRefusal, Entry, Index, Location, Miss, PATH_MAX, PROVISIONAL_CHANNEL, Repository,
    STAND_IN_INDEX_FILE, Source, accept,
};
use pinned_tls_client::listed::{self, root_der};
use pinned_tls_client::{PinnedPeer, Session};

/// The second test authority, the stand-in for Sectigo E46 (calef's C2 on #2048): what
/// `releases.basalt.test`, `github.com`'s stand-in, chains to. Test-only.
const SECOND_ROOT: &[u8] = include_bytes!("../../../pinned_tls_client/fixtures/second-root.der");

/// The test authority `helpers/tls-peer` chains `basalt.test` to. Test-only. Listed locations are
/// pinned to it too: §196 (nife carries TLS) holds one root per source, and which root a listed
/// location's host must chain to is a question this lane put back on #1884.
const PINNED_ROOT: &[u8] = include_bytes!("../../../pinned_tls_client/fixtures/pinned-root.der");

/// One `GET` over TLS, connected to one of `to` and speaking to `server_name`, pinned to the test
/// root. The body, or why not.
///
/// It takes addresses, never a name to resolve: `server_name` is for TLS alone (the server name
/// sent and the name the certificate must carry). Connecting by name resolved a second time, and
/// that second resolution is the rebinding escape milestone 871 (a sixth outsider pass attacks the
/// confinement claim) booted; `TcpStream::connect` on a slice of `SocketAddr` asks no resolver.
fn get(server_name: &str, to: &[SocketAddr], path: &str) -> Result<Vec<u8>, String> {
    let peer = PinnedPeer::new(server_name, PINNED_ROOT).map_err(|e| format!("{e:?}"))?;
    let tcp = TcpStream::connect(to).map_err(|e| format!("{:?}", e.kind()))?;
    let session = Session::handshake(&peer, tcp).map_err(|e| format!("{e:?}"))?;
    let mut body = Vec::new();
    match session.get(path, |part| body.extend_from_slice(part)) {
        Ok(200) => Ok(body),
        Ok(status) => Err(format!("status {status}")),
        Err(e) => Err(format!("{e:?}")),
    }
}

/// Resolve `host` once. A repository's address is the owner's or the image's choice and is not
/// held to Q1, so nothing is checked here; it still resolves once, so [`get`] never resolves.
fn resolve(host: &str, port: u16) -> Result<Vec<SocketAddr>, String> {
    Ok((host, port)
        .to_socket_addrs()
        .map_err(|e| format!("{:?}", e.kind()))?
        .collect())
}

/// `GET` from a repository the owner or the image chose.
fn from_repository(repo: &Repository<'_>, path: &str) -> Result<Vec<u8>, String> {
    get(repo.host, &resolve(repo.host, repo.port)?, path)
}

/// The index from the first of the image's addresses that answers (Q2).
fn fetch_index<'r>(addresses: &'r [Repository<'r>]) -> (String, &'r Repository<'r>) {
    for repo in addresses {
        let mut buf = [0u8; PATH_MAX];
        let path = repo.metadata_path(STAND_IN_INDEX_FILE, &mut buf).unwrap();
        match from_repository(repo, path) {
            Ok(body) => {
                return (
                    String::from_utf8(body).expect("the index is not text"),
                    repo,
                );
            }
            Err(why) => println!("index unreachable at {}: {why}", repo.host),
        }
    }
    panic!("no index address answered");
}

/// A listed location, if every address its host resolves to is public (Q1's safeguard), fetched
/// from exactly those addresses. One resolution is checked and the same one is dialed:
/// `Location::check` hands back the slice it passed, the only addresses this connects to.
fn from_location(at: &Location<'_>) -> Result<Vec<u8>, String> {
    let resolved = resolve(at.host, at.port)?;
    let checked = at.check(&resolved).map_err(|why| match why {
        AddressRefusal::Private(ip) => format!("a private address ({ip})"),
        other => format!("{other:?}"),
    })?;
    get(checked.host(), checked.addresses(), checked.path())
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
                    println!(
                        "passed over https://{}:{}{}: {why}",
                        at.host, at.port, at.path
                    );
                    continue;
                }
            },
            Source::Repository(repo) => {
                let mut buf = [0u8; PATH_MAX];
                let path = repo.target_path(entry.stem, &mut buf).unwrap();
                match from_repository(repo, path) {
                    Ok(bytes) => (bytes, format!("{}'s targets", repo.host)),
                    Err(why) => return println!("refused {}: fetch failed: {why}", entry.stem),
                }
            }
        };
        return match accept(&entry, &bytes) {
            Ok(ok) => println!(
                "fetched {} from {whence}, {} bytes, programs {}",
                entry.stem,
                bytes.len(),
                ok.programs().map(|p| p.name).collect::<Vec<_>>().join(", ")
            ),
            Err(why) => println!("refused {}: {why:?}", entry.stem),
        };
    }
}

/// The redirect probes' judge of a public address: `package_index::public_address`, and the
/// runners' peer, which stands in for the internet because slirp offers no public address to
/// reach. Never a client's.
fn stand_in_public(ip: [u8; 4]) -> bool {
    ip == [10, 0, 2, 9] || package_index::public_address(ip)
}

/// The test image's sources as `jig` would read them, with the second authority carried under the
/// name the probes' locations give it.
fn test_sources() -> String {
    let hex = |der: &[u8]| der.iter().map(|b| format!("{b:02x}")).collect::<String>();
    format!(
        "repository {BACKUP_HOST} {INDEX_PORT} {PROVISIONAL_CHANNEL}\n\
         root {}\n\
         location-root test-second-root {}\n",
        hex(PINNED_ROOT),
        hex(SECOND_ROOT)
    )
}

/// **calef's C2 (2026-10-12 UTC, #2048), at the client `jig` runs**: `greeting`'s bytes through
/// each redirect probe, judged by the index's digest when they arrive.
fn redirect_checks(index: &Index<'_>, architecture: &str) {
    let text = test_sources();
    let sources = package_index::Sources::parse(&text).expect("the test sources are refused");
    let index_root = root_der(&sources.root).expect("the test root");
    let entry = index
        .find(GENUINE, architecture)
        .expect("greeting is in the index");
    for (probe, root_name) in [
        ("releases", "test-second-root="),
        ("releases", ""),
        ("releases", "no-such-root="),
        ("twice", "test-second-root="),
        ("private", "test-second-root="),
        ("offroot", "test-second-root="),
    ] {
        let label = match (probe, root_name) {
            ("releases", "") => "unnamed",
            ("releases", "no-such-root=") => "uncarried",
            _ => probe,
        };
        let listed = format!(
            "{root_name}https://releases.basalt.test:{INDEX_PORT}/{probe}/{}.nifepkg",
            entry.stem
        );
        let at = Location::parse(&listed).expect("a probe's location is refused");
        let mut bytes = Vec::new();
        let fetched = sources
            .root_for(&at)
            .ok_or_else(|| "it names a root this image does not carry".to_string())
            .and_then(|named| root_der(&named).map_err(str::to_string))
            .and_then(|first_root| {
                listed::fetch(
                    &at,
                    &first_root,
                    &index_root,
                    stand_in_public,
                    &mut |part| bytes.extend_from_slice(part),
                )
            });
        match fetched {
            Ok(from) => match accept(&entry, &bytes) {
                Ok(ok) => println!(
                    "redirect {label}: fetched {} from {} via {}, {} bytes, programs {}",
                    entry.stem,
                    from.host,
                    from.redirected_from.as_deref().unwrap_or("nothing"),
                    bytes.len(),
                    ok.programs().map(|p| p.name).collect::<Vec<_>>().join(", ")
                ),
                Err(why) => println!("redirect {label}: refused by the digest: {why:?}"),
            },
            Err(why) => println!("redirect {label}: refused: {why}"),
        }
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
    redirect_checks(&index, architecture);
    println!("package_fetch_exerciser done");
}
