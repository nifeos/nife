//! Fuzz the package index reader with arbitrary text, and hold the address check to its word.
//!
//! `crates/package_index` reads the index a repository host or mirror operator serves, over pinned
//! TLS. `jig` (a confined `std` program) reads all of what was fetched; the progenitor then reads
//! the copy of up to `COPY_MAXIMUM` (4095) bytes. Nothing in the crate has a Kani harness, so this
//! is the only thing that runs `Index::parse`, `Location::parse` and `Location::check` on text
//! nobody here wrote. The sixth outsider pass (notes/confinement-outsider-pass-6.md) booted an
//! escape through `Location`, so the address check is the property that matters most here.
//!
//! # The input
//!
//! Text, then optionally one NUL and a tail. The text is read up to its first invalid UTF-8 byte.
//! The tail is a list of resolved addresses, five bytes each: a kind byte (odd means IPv6) and four
//! octets, which an IPv6 address repeats to fill its sixteen. A seed with no NUL has no addresses.
//!
//! # What is asserted, beyond "it returned"
//!
//! 1. **A fetched index cannot point `jig` at a non-public address.** For every location the
//!    text yields, `Location::check` on the tail's addresses admits them exactly when the list is
//!    not empty and every address is IPv4 and one `public_address` admits. The addresses it hands
//!    back are the slice it was given. A literal host that parses is one `public_address` admits.
//! 2. **`public_address` is the documented list**, checked against a second statement of the ranges
//!    in its own doc comment, for the tail's four octets and for the edges of every range.
//! 3. **What `Index::parse` accepts, `entries` and `Location::parse` agree on**: one entry per
//!    non-empty line, every listed location within the limits `Location` states, a stem within
//!    `STEM_LEN` whose version `version::check` admits, and `find` on an entry's own name and
//!    version never says the package does not exist. `sources` lists the locations then the
//!    repository, or the pinned mirror alone.
//! 4. **`version::compare` is an order**: the reverse of itself with the arguments swapped, equal
//!    to itself, transitive on triples, refusing exactly what `version::check` refuses, and
//!    `newest` agrees with it.
//! 5. `Sources::parse`, `Root::der` and `root_for` return on the same text, and a root they hand
//!    back decodes only from hex.

#![no_main]

use core::cmp::Ordering;
use core::net::{Ipv4Addr, Ipv6Addr, SocketAddr};

use libfuzzer_sys::fuzz_target;
use package_index::version::{BadVersion, check, compare, newest};
use package_index::{
    AddressRefusal, Index, Location, PATH_MAXIMUM, REDIRECT_PATH_MAXIMUM, Repository, Root,
    Source, Sources, accept, public_address,
};

/// The ranges `public_address` documents, written a second way: by the integer a range spans.
fn public_by_range(ip: [u8; 4]) -> bool {
    let n = u32::from_be_bytes(ip);
    let refused = [
        (0x0000_0000, 0x00ff_ffff),  // this host and the unspecified address
        (0x0a00_0000, 0x0aff_ffff),  // 10/8
        (0x6440_0000, 0x647f_ffff),  // 100.64/10, shared address space
        (0x7f00_0000, 0x7fff_ffff),  // loopback
        (0xa9fe_0000, 0xa9fe_ffff),  // 169.254/16, link-local
        (0xac10_0000, 0xac1f_ffff),  // 172.16/12
        (0xc0a8_0000, 0xc0a8_ffff),  // 192.168/16
        (0xe000_0000, 0xffff_ffff),  // multicast and above
    ];
    !refused.iter().any(|&(low, high)| (low..=high).contains(&n))
}

fn addresses(tail: &[u8]) -> Vec<SocketAddr> {
    tail.chunks_exact(5)
        .take(8)
        .map(|c| {
            let octets = [c[1], c[2], c[3], c[4]];
            if c[0] & 1 == 1 {
                let mut wide = [0u8; 16];
                for (i, b) in wide.iter_mut().enumerate() {
                    *b = octets[i % 4];
                }
                SocketAddr::new(Ipv6Addr::from(wide).into(), 443)
            } else {
                SocketAddr::new(Ipv4Addr::from(octets).into(), 443)
            }
        })
        .collect()
}

/// Property 1 and the shape `Location` states, for one location that parsed.
fn hold_location(location: &Location<'_>, redirect: bool, resolved: &[SocketAddr]) {
    assert!(location.port != 0, "port zero parsed");
    assert!(location.path.starts_with('/'), "a path without its slash");
    let maximum = if redirect {
        REDIRECT_PATH_MAXIMUM
    } else {
        PATH_MAXIMUM
    };
    assert!(location.path.len() <= maximum, "a path past its limit");
    assert!(
        location.path.bytes().all(|b| b.is_ascii_graphic()) && !location.path.contains('#'),
        "a path with a space, a control byte or a fragment"
    );
    assert!(!redirect || location.root.is_none(), "a redirect named a root");
    if let Ok(literal) = location.host.parse::<Ipv4Addr>() {
        assert!(
            public_address(literal.octets()),
            "a literal host parsed that public_address refuses: {literal}"
        );
    }

    let expected_ok = !resolved.is_empty()
        && resolved
            .iter()
            .all(|a| matches!(a, SocketAddr::V4(v4) if public_address(v4.ip().octets())));
    match location.check(resolved) {
        Ok(checked) => {
            assert!(expected_ok, "check admitted {resolved:?} for {}", location.host);
            assert_eq!(checked.addresses(), resolved);
            assert_eq!(checked.host(), location.host);
            assert_eq!(checked.path(), location.path);
        }
        Err(why) => {
            assert!(!expected_ok, "check refused {resolved:?}: {why:?}");
            match why {
                AddressRefusal::NoAddress => assert!(resolved.is_empty()),
                AddressRefusal::Private(ip) => {
                    assert!(!public_address(ip.octets()));
                    assert!(resolved.iter().any(|a| a.ip() == ip));
                }
                AddressRefusal::NotJudged(ip) => {
                    assert!(resolved.iter().any(|a| a.ip() == ip));
                }
            }
        }
    }
    // A judge that admits everything must still refuse an empty list and an IPv6 address.
    let lenient = location.check_by(resolved, |_| true);
    let any_v6 = resolved.iter().any(SocketAddr::is_ipv6);
    assert_eq!(lenient.is_ok(), !resolved.is_empty() && !any_v6);
}

/// Property 3, for text `Index::parse` accepted.
fn hold_index(text: &str, index: &Index<'_>, resolved: &[SocketAddr], spare: &[u8]) {
    let lines = text.split('\n').filter(|l| !l.is_empty()).count();
    assert_eq!(index.entries().count(), lines, "an accepted line is not an entry");

    let basalt = Repository::new("basalt.nifeos.org", 443, "rolling").unwrap();
    let mirror = Repository::new("mirror.example", 8443, "rolling").unwrap();
    for entry in index.entries() {
        assert!(entry.stem.len() <= package_archive::STEM_LEN);
        let mut parts = entry.stem.rsplitn(3, '-');
        let (architecture, version, name) = (
            parts.next().unwrap(),
            parts.next().unwrap(),
            parts.next().unwrap(),
        );
        assert_eq!(check(version), Ok(()), "a stem with a version check refuses");

        let listed: Vec<_> = entry.locations().collect();
        for location in &listed {
            hold_location(location, false, resolved);
            let written = format_location(location);
            let again = Location::parse(&written);
            assert_eq!(again, Ok(*location), "a listed location does not re-read");
        }

        // `matching_stem` reads `@` and an empty name as its own syntax; those names cannot ask.
        if !name.is_empty() && !name.contains('@') {
            let asked = format!("{name}@{version}");
            match index.find(&asked, architecture) {
                Ok(found) => assert_eq!(found.stem, entry.stem),
                Err(package_index::Miss::SeveralVersions) => {}
                Err(package_index::Miss::NoSuchPackage) => {
                    panic!("find does not find {asked} for {architecture}")
                }
            }
        }

        let order: Vec<_> = Index::sources(&entry, &basalt, None).collect();
        assert_eq!(order.len(), listed.len() + 1);
        assert!(matches!(order.last(), Some(Source::Repository(r)) if **r == basalt));
        let pinned: Vec<_> = Index::sources(&entry, &basalt, Some(&mirror)).collect();
        assert!(matches!(pinned.as_slice(), [Source::Repository(r)] if **r == mirror));

        // Arbitrary bytes against an entry's digest: refused or installable, never a panic.
        let _ = accept(&entry, spare);
    }
}

/// A location written the way an index writes it, for the re-read in `hold_index`.
fn format_location(l: &Location<'_>) -> String {
    let root = l.root.map(|r| format!("{r}=")).unwrap_or_default();
    format!("{root}https://{}:{}{}", l.host, l.port, l.path)
}

/// Property 4, on every ordered triple of the versions found.
fn hold_versions(versions: &[&str]) {
    for &a in versions {
        for &b in versions {
            let forward = compare(a, b);
            match (check(a), check(b)) {
                (Err(why), _) => assert_eq!(forward, Err(why)),
                (Ok(()), Err(why)) => assert_eq!(forward, Err(why)),
                (Ok(()), Ok(())) => {
                    let order = forward.expect("two checked versions compare");
                    assert_eq!(compare(b, a), Ok(order.reverse()), "{a} against {b}");
                    if a == b {
                        assert_eq!(order, Ordering::Equal);
                    }
                    assert_eq!(compare(a, a), Ok(Ordering::Equal));
                }
            }
        }
    }
    for &a in versions {
        for &b in versions {
            for &c in versions {
                if let (Ok(ab), Ok(bc), Ok(ac)) = (compare(a, b), compare(b, c), compare(a, c)) {
                    if ab != Ordering::Greater && bc != Ordering::Greater {
                        assert!(ac != Ordering::Greater, "{a} <= {b} <= {c} but {a} > {c}");
                        if ab == Ordering::Less || bc == Ordering::Less {
                            assert_eq!(ac, Ordering::Less, "{a} <= {b} <= {c} with one strict");
                        }
                    }
                    if ab == Ordering::Equal && bc == Ordering::Equal {
                        assert_eq!(ac, Ordering::Equal);
                    }
                }
            }
        }
    }
    match newest(versions.iter().copied()) {
        Ok(None) => assert!(versions.is_empty()),
        Ok(Some(best)) => {
            assert!(versions.contains(&best));
            let mut first_of_best = None;
            for &v in versions {
                assert_ne!(compare(v, best), Ok(Ordering::Greater), "{v} beats the newest");
                if compare(v, best) == Ok(Ordering::Equal) && first_of_best.is_none() {
                    first_of_best = Some(v);
                }
            }
            assert_eq!(first_of_best, Some(best), "the first of equals did not win");
        }
        Err((bad, why)) => {
            assert_eq!(check(bad), Err(why));
            let first_bad = versions.iter().find(|v| check(v).is_err()).unwrap();
            assert_eq!(*first_bad, bad, "newest named a later bad version");
            assert!(matches!(
                why,
                BadVersion::Empty | BadVersion::NoLeadingDigit | BadVersion::BadCharacter
            ));
        }
    }
}

fuzz_target!(|data: &[u8]| {
    let (text_bytes, tail) = match data.iter().position(|&b| b == 0) {
        Some(at) => (&data[..at], &data[at + 1..]),
        None => (data, &[][..]),
    };
    let text = match core::str::from_utf8(text_bytes) {
        Ok(text) => text,
        Err(e) => core::str::from_utf8(&text_bytes[..e.valid_up_to()]).unwrap(),
    };
    let resolved = addresses(tail);

    // 2. The address list, on the tail's octets and on both sides of every range's edge.
    for c in tail.chunks_exact(4).take(8) {
        let ip = [c[0], c[1], c[2], c[3]];
        assert_eq!(public_address(ip), public_by_range(ip), "{ip:?}");
    }
    for edge in [
        0x0000_0000u32, 0x00ff_ffff, 0x0100_0000, 0x09ff_ffff, 0x0a00_0000, 0x0aff_ffff,
        0x0b00_0000, 0x643f_ffff, 0x6440_0000, 0x647f_ffff, 0x6480_0000, 0x7eff_ffff,
        0x7f00_0000, 0x7fff_ffff, 0x8000_0000, 0xa9fd_ffff, 0xa9fe_0000, 0xa9fe_ffff,
        0xa9ff_0000, 0xac0f_ffff, 0xac10_0000, 0xac1f_ffff, 0xac20_0000, 0xc0a7_ffff,
        0xc0a8_0000, 0xc0a8_ffff, 0xc0a9_0000, 0xdfff_ffff, 0xe000_0000, 0xffff_ffff,
    ] {
        let ip = edge.to_be_bytes();
        assert_eq!(public_address(ip), public_by_range(ip), "{ip:?}");
    }

    // 3. The index, whole.
    if let Ok(index) = Index::parse(text) {
        hold_index(text, &index, &resolved, tail);
    } else if let Err(bad) = Index::parse(text) {
        assert!(bad.number >= 1);
        assert_eq!(text.split('\n').nth(bad.number - 1), Some(bad.line));
        assert!(!bad.line.is_empty());
    }

    // 1. Locations: the whole text, each line, each word, and as a redirect.
    let mut candidates: Vec<&str> = vec![text];
    candidates.extend(text.split(['\n', ' ']).filter(|w| !w.is_empty()).take(64));
    for candidate in &candidates {
        if let Ok(location) = Location::parse(candidate) {
            hold_location(&location, false, &resolved);
        }
        if let Ok(location) = Location::redirect(candidate) {
            hold_location(&location, true, &resolved);
        }
    }

    // 4. Versions: the words, their last-but-one dash parts, and what an entry's stem carries.
    let mut versions: Vec<&str> = Vec::new();
    for word in text.split(['\n', ' ']).filter(|w| !w.is_empty()).take(6) {
        versions.push(word);
        if let Some(version) = word.rsplitn(3, '-').nth(1) {
            versions.push(version);
        }
    }
    versions.truncate(6);
    hold_versions(&versions);

    // 5. The image's own sources text, on the same bytes.
    if let Ok(sources) = Sources::parse(text) {
        let count = sources.repositories().count();
        assert!((1..=package_index::MAXIMUM_REPOSITORIES).contains(&count));
        let mut buffer = [0u8; 64];
        if let Some(der) = sources.root.der(&mut buffer) {
            assert!(matches!(sources.root, Root::Der(hex) if hex.len() == 2 * der.len()));
        }
        for location in candidates.iter().filter_map(|c| Location::parse(c).ok()) {
            if let Some(Root::Der(hex)) = sources.root_for(&location) {
                assert!(hex.bytes().all(|b| b.is_ascii_hexdigit()));
            }
            if location.root.is_none() {
                assert_eq!(sources.root_for(&location), Some(sources.root));
            }
        }
    }
});
