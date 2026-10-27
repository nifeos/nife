//! A distribution's package index: which packages exist, the digest each must hash to, and where
//! its bytes may be fetched, for milestone 801 (packages over the internet) and §250 (an image
//! names its distribution's package index, and the bytes may live anywhere). A [`Repository`] is a
//! host and a channel; an [`Entry`]'s [`Location`]s are HTTPS only, tried before the repository's
//! own `targets/` ([`Index::sources`]), and never reach a private address ([`public_address`]).
//! The encoding is a stand-in that a TUF client replaces in [`Index::parse`] alone, and [`accept`]
//! is the installer's own check. See notes/packages/the-index-format.md for calef's rulings and
//! notes/packages/the-index-crate.md.
//!
//! # EXAMPLES
//!
//! ```
//! use package_index::{Index, Repository, Source};
//!
//! let text = "greeting-0.1.0-aarch64 \
//!     sha256:6d1a1e0cafa7f2bbcf4a5c4c3ff6b6a8b6bd1e0c50f8c64fa2ce1c4bf0c2d8a1 \
//!     https://mirror.example/g.nifepkg\n";
//! let basalt = Repository::new("basalt.nifeos.org", 443, "rolling").unwrap();
//! let entry = Index::parse(text).unwrap().find("greeting", "aarch64").unwrap();
//! let order: Vec<_> = Index::sources(&entry, &basalt, None).collect();
//! assert!(matches!(order[0], Source::Listed(l) if l.host == "mirror.example"));
//! assert!(matches!(order[1], Source::Repository(r) if r.host == "basalt.nifeos.org"));
//! ```
//!
//! Name: provisional 2026-10-09 (UTC), milestone 801's lane, with every public item here.

#![cfg_attr(not(test), no_std)]

use core::net::{Ipv4Addr, Ipv6Addr, SocketAddr};

use measured_boot::{DIGEST_TEXT_LEN, Digest, digest_text, parse_digest};
use package_archive::{CatalogMiss, Installable, Refusal, STEM_LEN, installable_as, matching_stem};

/// **The one channel this tree names, provisionally** (Q4: "The lane ships one provisional channel
/// name, and naming the channel is calef's"). basalt is a rolling distribution, so `rolling`.
pub const PROVISIONAL_CHANNEL: &str = "rolling";

/// The stand-in index's file name under `/<channel>/metadata/`. TUF's own files (`root.json`,
/// `timestamp.json` and the rest) replace it with the TUF client.
pub const STAND_IN_INDEX_FILE: &str = "stand-in-index";

/// What a package's target name ends in: `<stem>.nifepkg`, the file name the producer writes.
pub const TARGET_SUFFIX: &str = ".nifepkg";

/// The longest path a request may carry. `http_response::get_request` writes the request line
/// into a 512-byte buffer beside the host header, so a path near that would not fit a request.
pub const PATH_MAX: usize = 256;

/// The longest channel name.
pub const CHANNEL_MAX: usize = 32;

/// **A TUF repository on a host: one channel of one distribution** (Q4). The image carries two,
/// the index's address and a backup tried when the first stops answering (Q2), and an owner may
/// pin a mirror as a third (Q1). Each is a source the owner or the image chose, so its address is
/// not held to [`public_address`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Repository<'a> {
    /// A DNS name or a dotted IPv4 address.
    pub host: &'a str,
    /// Spoken HTTPS on.
    pub port: u16,
    /// Lowercase letters, digits and hyphens.
    pub channel: &'a str,
}

impl<'a> Repository<'a> {
    /// `None` for a host or channel that is not that shape, or port 0. A host that ends in a
    /// number must be a canonical IPv4 address, as for a [`Location`].
    pub fn new(host: &'a str, port: u16, channel: &'a str) -> Option<Repository<'a>> {
        let channel_ok = !channel.is_empty()
            && channel.len() <= CHANNEL_MAX
            && channel
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
        let host_ok = is_host(host) && ipv4_literal(host).is_ok();
        (host_ok && port != 0 && channel_ok).then_some(Repository {
            host,
            port,
            channel,
        })
    }

    /// `/<channel>/metadata/<file>`, written into `out`.
    pub fn metadata_path<'b>(&self, file: &str, out: &'b mut [u8; PATH_MAX]) -> Option<&'b str> {
        join(out, &["/", self.channel, "/metadata/", file])
    }

    /// `/<channel>/targets/<stem>.nifepkg`, written into `out`.
    pub fn target_path<'b>(&self, stem: &str, out: &'b mut [u8; PATH_MAX]) -> Option<&'b str> {
        join(out, &["/", self.channel, "/targets/", stem, TARGET_SUFFIX])
    }
}

fn join<'b>(out: &'b mut [u8; PATH_MAX], parts: &[&str]) -> Option<&'b str> {
    let mut at = 0;
    for part in parts {
        let end = at + part.len();
        out.get_mut(at..end)?.copy_from_slice(part.as_bytes());
        at = end;
    }
    core::str::from_utf8(&out[..at]).ok()
}

/// **May a location the index listed reach this address?** Q1's safeguard: never a private or
/// link-local range, so a signed index cannot point a client at the owner's own network. Also
/// refused, for the same reason: this host, the unspecified and shared-address ranges, and
/// multicast and above. A client applies it to every address a listed host resolves to.
pub fn public_address(ip: [u8; 4]) -> bool {
    !matches!(
        ip,
        [0, ..] | [10, ..] | [127, ..] | [169, 254, ..] | [192, 168, ..] | [224..=255, ..]
    ) && !(ip[0] == 172 && (16..32).contains(&ip[1]))
        && !(ip[0] == 100 && (64..128).contains(&ip[1]))
}

/// **The longest path a redirect may send a client to**: a GitHub Release asset's signed URL has
/// a 931-byte path (measured 2026-10-13 UTC), far past [`PATH_MAX`], which bounds only what an
/// index may list. Name: provisional, minted 2026-10-13 (UTC) by lane
/// `lane/basalt-on-pages-and-releases`.
pub const REDIRECT_PATH_MAX: usize = 2048;

/// **How many redirects a fetch from a listed location follows: one** (calef, 2026-10-12 UTC, on
/// #2048, option C2). A Release download is one hop from `github.com` to the host that serves the
/// bytes; a second is refused rather than followed, so a listed location cannot walk a client
/// anywhere further than one step from where the signed index pointed. Name: provisional, as
/// above.
pub const MAX_REDIRECTS: usize = 1;

/// **One place the index says a package's bytes may be fetched**: optionally the name of the root
/// its certificate must chain to, then `https://`, a host, an optional port, a path. No user, no
/// fragment; a query is part of the path. HTTPS only (Q1).
///
/// The root, written `<root>=https://...`, is calef's C2 of 2026-10-12 (UTC) on #2048: the signed
/// index chooses, per location, among the roots the image carries ([`Sources::root_for`]), and a
/// location that names none chains to the index's own root, ISRG Root X1 in production. A
/// redirect's target ([`Location::redirect`]) never names one: it is held to the index's root.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Location<'a> {
    /// The root the index named for this location, or `None` for the index's own. Name:
    /// provisional, minted 2026-10-13 (UTC) by lane `lane/basalt-on-pages-and-releases`.
    pub root: Option<&'a str>,
    /// A DNS name, or a canonical dotted-decimal IPv4 address [`public_address`] admits.
    pub host: &'a str,
    /// The port written, or 443.
    pub port: u16,
    /// Starts with `/`, at most [`PATH_MAX`] bytes, printable ASCII.
    pub path: &'a str,
}

impl<'a> Location<'a> {
    /// Read one location. Refused: anything that is not exactly the shape above, plain HTTP
    /// included; a host that ends in a number but is not a canonical IPv4 address; and an address
    /// literal outside [`public_address`].
    pub fn parse(text: &'a str) -> Result<Location<'a>, LocationRefusal<'a>> {
        let (root, url) = if text.starts_with("https://") {
            (None, text)
        } else {
            match text.split_once('=') {
                Some((root, url)) if is_root_name(root) => (Some(root), url),
                _ => return Err(LocationRefusal::Malformed),
            }
        };
        let at = Location::url(url, PATH_MAX)?;
        Ok(Location { root, ..at })
    }

    /// **A redirect's `Location`, read as a place to fetch from**: an absolute `https://` URL with
    /// no root of its own and a path of up to [`REDIRECT_PATH_MAX`] bytes, refused on the same
    /// terms as [`Location::parse`] otherwise. A relative `Location` is refused: the one redirect
    /// this follows (GitHub's) is absolute, and refusing is cheaper than resolving one right.
    ///
    /// Name: provisional, minted 2026-10-13 (UTC) by lane `lane/basalt-on-pages-and-releases`.
    pub fn redirect(text: &'a str) -> Result<Location<'a>, LocationRefusal<'a>> {
        Location::url(text, REDIRECT_PATH_MAX)
    }

    fn url(text: &'a str, path_max: usize) -> Result<Location<'a>, LocationRefusal<'a>> {
        use LocationRefusal::Malformed;
        let rest = text.strip_prefix("https://").ok_or(Malformed)?;
        let slash = rest.find('/').ok_or(Malformed)?;
        let (authority, path) = rest.split_at(slash);
        let (host, port) = match authority.split_once(':') {
            Some((host, port)) => (host, parse_port(port).ok_or(Malformed)?),
            None => (authority, 443),
        };
        if !is_host(host) || path.len() > path_max || !path.bytes().all(|b| b.is_ascii_graphic()) {
            return Err(Malformed);
        }
        // A fragment is the browser's and never reaches a server.
        if path.contains('#') {
            return Err(Malformed);
        }
        match ipv4_literal(host)? {
            Some(ip) if !public_address(ip) => Err(LocationRefusal::Private(Ipv4Addr::from(ip))),
            _ => Ok(Location {
                root: None,
                host,
                port,
                path,
            }),
        }
    }

    /// Q1's safeguard on one resolution, and the addresses a client may then connect to.
    /// `resolved` is what this location's host resolved to, asked once. Every address must be an
    /// IPv4 address [`public_address`] admits, and the slice must not be empty, or the location is
    /// refused. The returned [`CheckedLocation`] holds that very slice: its addresses are the only
    /// ones a client dials, and its host name is for TLS alone, never resolved again, which closes
    /// DNS rebinding (calef, #1901, 2026-10-10 UTC). See notes/packages/the-index-crate.md.
    pub fn check<'r>(
        &self,
        resolved: &'r [SocketAddr],
    ) -> Result<CheckedLocation<'a, 'r>, AddressRefusal> {
        self.check_by(resolved, public_address)
    }

    /// [`Location::check`] with the judge of a public address passed in. A client passes
    /// [`public_address`] and nothing else; `package_fetch_exerciser` passes one that also admits
    /// the runners' peer, because under QEMU's slirp no public address can be reached at all (a
    /// `guestfwd` outside 10.0.2.0/24 is refused, measured 2026-10-13 UTC), and its redirect test
    /// needs one host that passes. Name: provisional, minted 2026-10-13 (UTC) by lane
    /// `lane/basalt-on-pages-and-releases`.
    pub fn check_by<'r>(
        &self,
        resolved: &'r [SocketAddr],
        public: impl Fn([u8; 4]) -> bool,
    ) -> Result<CheckedLocation<'a, 'r>, AddressRefusal> {
        if resolved.is_empty() {
            return Err(AddressRefusal::NoAddress);
        }
        for address in resolved {
            match address {
                SocketAddr::V4(v4) if public(v4.ip().octets()) => {}
                SocketAddr::V4(v4) => return Err(AddressRefusal::Private(*v4.ip())),
                SocketAddr::V6(v6) => return Err(AddressRefusal::NotJudged(*v6.ip())),
            }
        }
        Ok(CheckedLocation {
            location: *self,
            addresses: resolved,
        })
    }
}

/// **A listed location whose one resolution passed Q1's check**, made only by
/// [`Location::check`]. A client connects to [`CheckedLocation::addresses`] and to nothing else,
/// and uses [`CheckedLocation::host`] only as the TLS server name. The fields are private so that
/// no other path can pair a location with addresses nobody checked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CheckedLocation<'a, 'r> {
    location: Location<'a>,
    addresses: &'r [SocketAddr],
}

impl<'a, 'r> CheckedLocation<'a, 'r> {
    /// The addresses that were checked, the only ones to dial, in the resolver's order.
    pub fn addresses(&self) -> &'r [SocketAddr] {
        self.addresses
    }

    /// The host name, for TLS only: what the client sends as the server name, and what the
    /// certificate must be valid for. Resolving it again would reopen the escape.
    pub fn host(&self) -> &'a str {
        self.location.host
    }

    /// The request path.
    pub fn path(&self) -> &'a str {
        self.location.path
    }
}

/// Why [`Location::check`] refused a resolution.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddressRefusal {
    /// The host resolved to nothing.
    NoAddress,
    /// An address [`public_address`] refuses: private, link-local, or another range Q1 keeps a
    /// listed location away from.
    Private(Ipv4Addr),
    /// An IPv6 address, which [`public_address`] does not judge.
    NotJudged(Ipv6Addr),
}

/// Why [`Location::parse`] refused a location. Its `Display` says what was wrong, naming the host
/// where the host is the problem.
///
/// Name: provisional, lane maintainer/canonical-ipv4-hosts (#2020), 2026-10-12 (UTC).
/// Named after the crate's `AddressRefusal` and `package_archive::Refusal`; the pull request
/// records no other name considered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocationRefusal<'a> {
    /// Not `https://`, a host, an optional port and a path within the limits [`Location`] states.
    ///
    /// Name: provisional, lane maintainer/canonical-ipv4-hosts (#2020), 2026-10-12 (UTC).
    /// No other name recorded.
    Malformed,
    /// A host whose last label is a number, so that some reader takes it for an IPv4 address, but
    /// which is not one written the canonical way: four decimal parts, each 0 to 255, no leading
    /// zeros. Refused rather than read, because readers disagree on which address it names
    /// (`0010` is 8 to WHATWG and `inet_aton`, 10 to others).
    ///
    /// Name: provisional, lane maintainer/canonical-ipv4-hosts (#2020), 2026-10-12 (UTC).
    /// No other name recorded.
    NonCanonicalAddress(&'a str),
    /// A canonical address literal [`public_address`] refuses.
    ///
    /// Name: provisional, lane maintainer/canonical-ipv4-hosts (#2020), 2026-10-12 (UTC).
    /// Matches `AddressRefusal::Private`; no other name recorded.
    Private(Ipv4Addr),
}

impl core::fmt::Display for LocationRefusal<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            LocationRefusal::Malformed => {
                f.write_str("not https://, a host, an optional port and a path")
            }
            LocationRefusal::NonCanonicalAddress(host) => write!(
                f,
                "host {host} ends in a number but is not a canonical IPv4 address \
                 (four decimal parts, each 0 to 255, no leading zeros)"
            ),
            LocationRefusal::Private(ip) => write!(f, "address {ip} is not a public address"),
        }
    }
}

/// Is `host` an IPv4 address, written the one way every reader agrees on? `Ok(None)` for a DNS
/// name, `Ok(Some(_))` for canonical dotted decimal, and a refusal for a host whose last label is a
/// number (WHATWG's test: decimal digits, or `0x` and hex) but that is not canonical: `0010.0.0.1`,
/// `0x0a.0.0.1`, `10.1`, `167772161`, `10.0.0.0x1`. Readers disagree on those (CVE-2021-29921,
/// CVE-2021-29923), so they are refused outright (calef, #2020, 2026-10-12 UTC). [`is_host`] has
/// already admitted `host`. See notes/packages/the-index-format.md.
fn ipv4_literal(host: &str) -> Result<Option<[u8; 4]>, LocationRefusal<'_>> {
    let last = host.rsplit('.').next().unwrap_or(host);
    let hex = last.strip_prefix("0x").or_else(|| last.strip_prefix("0X"));
    let ends_in_a_number = last.bytes().all(|b| b.is_ascii_digit())
        || hex.is_some_and(|h| h.bytes().all(|b| b.is_ascii_hexdigit()));
    if !ends_in_a_number {
        return Ok(None);
    }
    canonical_ipv4(host)
        .map(Some)
        .ok_or(LocationRefusal::NonCanonicalAddress(host))
}

fn canonical_ipv4(host: &str) -> Option<[u8; 4]> {
    let mut out = [0u8; 4];
    let mut parts = host.split('.');
    for b in out.iter_mut() {
        let part = parts.next()?;
        if part != "0" && part.starts_with('0') {
            return None;
        }
        *b = part.parse().ok()?;
    }
    parts.next().is_none().then_some(out)
}

fn parse_port(text: &str) -> Option<u16> {
    if text.is_empty() || text.len() > 5 || !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    text.parse().ok().filter(|&p| p != 0)
}

/// Letters, digits, hyphens and dots, with no empty label: a DNS name or a dotted IPv4 address.
fn is_host(host: &str) -> bool {
    !host.is_empty()
        && host.len() <= 253
        && host.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && label
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-')
                && !label.starts_with('-')
                && !label.ends_with('-')
        })
}

/// **One package the index lists**: the stem the image catalog would key it by, the digest its
/// whole file must hash to, and the locations it may also be fetched from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry<'a> {
    /// `name-version-architecture`, as in a catalog line and a package's own header.
    pub stem: &'a str,
    /// What the whole file must hash to.
    pub digest: Digest,
    /// The listed locations as they arrived, space-separated, each already read once by
    /// [`Index::parse`]. [`Entry::locations`] reads them.
    listed: &'a str,
}

impl<'a> Entry<'a> {
    /// The listed locations, in the order the index gives them.
    pub fn locations(&self) -> impl Iterator<Item = Location<'a>> + use<'a> {
        let listed: &'a str = self.listed;
        listed
            .split(' ')
            .filter(|l| !l.is_empty())
            .filter_map(|l| Location::parse(l).ok())
    }
}

/// Where to try next for an entry's bytes. See [`Index::sources`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source<'a, 'r> {
    /// A location the index listed. The client must refuse it if its host resolves to an address
    /// [`public_address`] refuses.
    Listed(Location<'a>),
    /// A repository's own `targets/`: the index's, or the mirror the owner pinned.
    Repository(&'r Repository<'r>),
}

/// A line [`Index::parse`] refused, and its number from 1, so a client can say which.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BadLine<'a> {
    /// Counted from 1.
    pub number: usize,
    /// The line as it arrived.
    pub line: &'a str,
    /// Why, when a listed location is what was refused; `None` for the stem or the digest.
    ///
    /// Name: provisional, lane maintainer/canonical-ipv4-hosts (#2020), 2026-10-12 (UTC).
    /// No other name recorded.
    pub location: Option<LocationRefusal<'a>>,
}

/// Why [`Index::find`] found no one entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Miss {
    /// The index lists no package of that name (at that version) for this architecture.
    NoSuchPackage,
    /// A bare name, and the index lists several versions of it: the rule of milestone 614 (two
    /// installed versions of one program, each runnable).
    SeveralVersions,
}

/// **An index whose every line was read.** One unreadable line refuses the whole index: a client
/// that skipped it would be installing from an index it only partly understood, which is the
/// failure a newer format meeting an older client looks like. A plain-HTTP or private location is
/// unreadable in this sense: the producer wrote what Q1 forbids.
#[derive(Clone, Copy, Debug)]
pub struct Index<'a> {
    text: &'a str,
}

impl<'a> Index<'a> {
    /// Read `text`, refusing it whole at the first line that is not `<stem> <digest>` followed by
    /// zero or more locations.
    pub fn parse(text: &'a str) -> Result<Index<'a>, BadLine<'a>> {
        let index = Index { text };
        for (number, line) in index.lines() {
            if let Err(location) = entry(line) {
                return Err(BadLine {
                    number,
                    line,
                    location,
                });
            }
        }
        Ok(index)
    }

    /// Every entry, in the order the index lists them.
    pub fn entries(&self) -> impl Iterator<Item = Entry<'a>> + 'a {
        let text = self.text;
        Index { text }
            .lines()
            .filter_map(|(_, line)| entry(line).ok())
    }

    /// **The one entry `name` (or `name@version`) asks for on `architecture`**, by the same rule the
    /// image's catalog lookup uses (`package_archive::matching_stem`).
    pub fn find(&self, name: &str, architecture: &str) -> Result<Entry<'a>, Miss> {
        let stem =
            matching_stem(self.entries().map(|e| e.stem), name, architecture).map_err(|miss| {
                match miss {
                    CatalogMiss::NoSuchPackage => Miss::NoSuchPackage,
                    CatalogMiss::SeveralVersions => Miss::SeveralVersions,
                }
            })?;
        // The same stem twice is one package to `matching_stem`; here it must also be one digest
        // and one list of locations, so a second line that disagrees is refused.
        let mut found = None;
        for e in self.entries().filter(|e| e.stem == stem) {
            match found {
                None => found = Some(e),
                Some(first) if first == e => {}
                Some(_) => return Err(Miss::SeveralVersions),
            }
        }
        found.ok_or(Miss::NoSuchPackage)
    }

    /// **Where to fetch `entry`'s bytes, in order** (Q1). With a mirror the owner `pinned`, that
    /// mirror alone. Otherwise every listed location, then `from`'s own `targets/`, which basalt
    /// always keeps a copy in. Whatever is fetched is judged by [`accept`], wherever it came from.
    pub fn sources<'r>(
        entry: &Entry<'a>,
        from: &'r Repository<'r>,
        pinned: Option<&'r Repository<'r>>,
    ) -> impl Iterator<Item = Source<'a, 'r>> + use<'a, 'r> {
        let listed = entry.locations().filter(move |_| pinned.is_none());
        listed
            .map(Source::Listed)
            .chain(core::iter::once(Source::Repository(pinned.unwrap_or(from))))
    }

    fn lines(&self) -> impl Iterator<Item = (usize, &'a str)> + 'a {
        self.text
            .split('\n')
            .enumerate()
            .map(|(i, line)| (i + 1, line))
            .filter(|(_, line)| !line.is_empty())
    }
}

/// One line read, or refused with the location refusal that refused it (`None` when the stem or
/// the digest did).
fn entry(line: &str) -> Result<Entry<'_>, Option<LocationRefusal<'_>>> {
    let (stem, rest) = line.split_once(' ').ok_or(None)?;
    let (digest, listed) = rest.split_once(' ').unwrap_or((rest, ""));
    if stem.is_empty() || stem.len() > STEM_LEN {
        return Err(None);
    }
    // **A version dpkg could not order refuses the line** (calef, 2026-10-18 UTC: "Debian dpkg
    // order"), so `jig update` keeps no copy that `jig list -u` would have to guess about.
    let version = stem
        .rsplit_once('-')
        .and_then(|(rest, _architecture)| rest.rsplit_once('-'))
        .map(|(_name, version)| version)
        .ok_or(None)?;
    version::check(version).map_err(|_| None)?;
    if !listed.is_empty() {
        for location in listed.split(' ') {
            Location::parse(location).map_err(Some)?;
        }
    }
    Ok(Entry {
        stem,
        digest: parse_digest(digest).map_err(|_| None)?,
        listed,
    })
}

/// **May these fetched bytes be installed as `entry`?** The package must be the one the entry names
/// and its whole file must hash to the entry's digest; then every program member must match its own
/// table of contents. That is `package_archive::installable_as`, handed the entry as a one-line
/// catalog, so the decision is the installer's own and not a second copy of it.
pub fn accept<'b>(entry: &Entry<'_>, bytes: &'b [u8]) -> Result<Installable<'b>, Refusal> {
    let mut line = [0u8; STEM_LEN + 1 + DIGEST_TEXT_LEN];
    let stem = entry.stem.as_bytes();
    line[..stem.len()].copy_from_slice(stem);
    line[stem.len()] = b' ';
    let end = stem.len() + 1 + DIGEST_TEXT_LEN;
    line[stem.len() + 1..end].copy_from_slice(&digest_text(&entry.digest));
    // ASCII by construction: a stem `Index::parse` admitted and a digest's text form.
    let catalog = core::str::from_utf8(&line[..end]).map_err(|_| Refusal::NotCataloged)?;
    installable_as(catalog, entry.stem, bytes)
}

/// Which of two package versions is newer: Debian's order, as dpkg computes it (calef, 2026-10-18
/// UTC: *"Debian dpkg order"*), for `jig list -u` (milestone 809 (the package client becomes a
/// program)). A version is dpkg's upstream version, with no epoch or revision.
/// [`check`](version::check) refuses whatever dpkg refuses or warns about, where the index is read
/// ([`crate::Index::parse`]); [`compare`](version::compare) is dpkg's `verrevcmp`. See
/// notes/packages/the-index-crate.md.
///
/// # EXAMPLES
///
/// ```
/// use core::cmp::Ordering;
/// use package_index::version::{BadVersion, compare};
///
/// assert_eq!(compare("1.10", "1.9"), Ok(Ordering::Greater));
/// assert_eq!(compare("1.0~rc1", "1.0"), Ok(Ordering::Less));
/// assert_eq!(compare("2.11", "2.11"), Ok(Ordering::Equal));
/// assert_eq!(compare("v1.3", "1.3"), Err(BadVersion::NoLeadingDigit));
/// ```
///
/// Name: provisional, minted 2026-10-18 (UTC) by lane `milestone/809-jig-list-u`.
pub mod version {
    use core::cmp::Ordering;

    /// Why [`check`] refused a version string.
    ///
    /// Name: provisional, minted 2026-10-18 (UTC) by lane `milestone/809-jig-list-u`.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum BadVersion {
        /// Nothing between the name and the architecture. dpkg: "version string is empty".
        Empty,
        /// The first character is not a digit. dpkg: "version number does not start with digit".
        NoLeadingDigit,
        /// A character outside `0-9 A-Z a-z . + ~`. dpkg: "invalid character in version number".
        BadCharacter,
    }

    /// **Whether `version` is one dpkg reads without complaint**, as an upstream version with no epoch
    /// and no revision. See the module's documentation for the rule.
    ///
    /// Name: provisional, minted 2026-10-18 (UTC) by lane `milestone/809-jig-list-u`.
    pub fn check(version: &str) -> Result<(), BadVersion> {
        let bytes = version.as_bytes();
        match bytes.first() {
            None => Err(BadVersion::Empty),
            Some(b) if !b.is_ascii_digit() => Err(BadVersion::NoLeadingDigit),
            Some(_)
                if bytes
                    .iter()
                    .all(|&b| b.is_ascii_alphanumeric() || b"+.~".contains(&b)) =>
            {
                Ok(())
            }
            Some(_) => Err(BadVersion::BadCharacter),
        }
    }

    /// **`a` against `b`, in Debian's order**, or why one of them has no place in it. `Greater` means
    /// `a` is the newer.
    ///
    /// Name: provisional, minted 2026-10-18 (UTC) by lane `milestone/809-jig-list-u`.
    pub fn compare(a: &str, b: &str) -> Result<Ordering, BadVersion> {
        check(a)?;
        check(b)?;
        Ok(verrevcmp(a.as_bytes(), b.as_bytes()))
    }

    /// **The newest of `versions`**, the first of any that are equal, or `None` for none. Every one is
    /// checked, so the first that [`check`] refuses is handed back with why, even when it would not
    /// have been the newest.
    ///
    /// Name: provisional, minted 2026-10-18 (UTC) by lane `milestone/809-jig-list-u`.
    pub fn newest<'a, I>(versions: I) -> Result<Option<&'a str>, (&'a str, BadVersion)>
    where
        I: IntoIterator<Item = &'a str>,
    {
        let mut best: Option<&'a str> = None;
        for v in versions {
            check(v).map_err(|why| (v, why))?;
            if best.is_none_or(|b| verrevcmp(v.as_bytes(), b.as_bytes()) == Ordering::Greater) {
                best = Some(v);
            }
        }
        Ok(best)
    }

    /// dpkg's weight for one character of a non-digit run, with `None` for the end of the string: the
    /// end and a digit weigh nothing, letters weigh their own byte, `~` less than nothing, and anything
    /// else after every letter.
    fn weight(c: Option<u8>) -> i32 {
        match c {
            None => 0,
            Some(c) if c.is_ascii_digit() => 0,
            Some(c) if c.is_ascii_alphabetic() => i32::from(c),
            Some(b'~') => -1,
            Some(c) => i32::from(c) + 256,
        }
    }

    /// dpkg's `verrevcmp`, over two strings [`check`] admitted.
    fn verrevcmp(mut a: &[u8], mut b: &[u8]) -> Ordering {
        while !a.is_empty() || !b.is_empty() {
            // A run of non-digits, character by character. One side may end first: its end weighs
            // nothing, so `~` still sorts before it and a letter after it.
            while a.first().is_some_and(|c| !c.is_ascii_digit())
                || b.first().is_some_and(|c| !c.is_ascii_digit())
            {
                let (wa, wb) = (weight(a.first().copied()), weight(b.first().copied()));
                if wa != wb {
                    return wa.cmp(&wb);
                }
                a = a.get(1..).unwrap_or_default();
                b = b.get(1..).unwrap_or_default();
            }
            let (digits_a, rest_a) = digit_run(a);
            let (digits_b, rest_b) = digit_run(b);
            let by_number = digits_a
                .len()
                .cmp(&digits_b.len())
                .then_with(|| digits_a.cmp(digits_b));
            if by_number != Ordering::Equal {
                return by_number;
            }
            (a, b) = (rest_a, rest_b);
        }
        Ordering::Equal
    }

    /// The run of digits `s` starts with, its leading zeros dropped, and what follows it. Two runs
    /// without leading zeros compare as numbers by length first, then bytewise.
    fn digit_run(s: &[u8]) -> (&[u8], &[u8]) {
        let end = s
            .iter()
            .position(|c| !c.is_ascii_digit())
            .unwrap_or(s.len());
        let (run, rest) = s.split_at(end);
        let zeros = run.iter().take_while(|&&c| c == b'0').count();
        (&run[zeros..], rest)
    }

    #[cfg(test)]
    mod tests {
        use Ordering::{Equal, Greater, Less};

        use super::*;

        /// **Pairs whose order dpkg states**, each checked both ways round. Read from
        /// `dpkg --compare-versions`' documented behavior and its own test suite's cases
        /// (`lib/dpkg/t/t-version.c`, the `verrevcmp` block), with the epoch and revision cases left out
        /// because a stem carries neither.
        #[test]
        fn dpkg_orders_these_pairs() {
            let pairs = [
                ("1.10", "1.9", Greater),
                ("1.0", "1.00", Equal),
                ("1.0", "1.0.0", Less),
                ("1.0~rc1", "1.0", Less),
                ("1.0~~", "1.0~", Less),
                ("1.0~", "1.0", Less),
                ("1.0a", "1.0", Greater),
                ("1.0a", "1.0+", Less),
                ("1.0+", "1.0.", Less),
                ("1.0.", "1.0", Greater),
                ("2.11", "2.9", Greater),
                ("14.1.1", "14.1.0", Greater),
                ("3.50.4", "3.50.10", Less),
                ("0.1.0", "0.3.0", Less),
                ("1.3", "1.3", Equal),
                ("007", "7", Equal),
                ("1a", "1b", Less),
                ("1Z", "1a", Less),
                // A digit weighs nothing inside a run of non-digits, so a letter facing one wins.
                ("1.a", "1.0", Greater),
            ];
            for (a, b, order) in pairs {
                assert_eq!(compare(a, b), Ok(order), "{a} against {b}");
                assert_eq!(compare(b, a), Ok(order.reverse()), "{b} against {a}");
            }
        }

        /// **The newest is found by number, not by text, the first of equals wins, and one version the
        /// order cannot read refuses the set** even when it is not the newest.
        #[test]
        fn newest_orders_by_number_and_refuses_what_it_cannot_read() {
            assert_eq!(newest(["1.9", "1.10", "1.2"]), Ok(Some("1.10")));
            assert_eq!(newest(["1.0", "1.00"]), Ok(Some("1.0")));
            assert_eq!(newest(["1.00", "1.0"]), Ok(Some("1.00")));
            assert_eq!(newest([]), Ok(None));
            assert_eq!(
                newest(["2.0", "v1.0"]),
                Err(("v1.0", BadVersion::NoLeadingDigit))
            );
        }

        /// **What dpkg would not read is refused, not ordered.** A version that names an epoch or a
        /// revision, which a stem cannot carry, is a bad character here.
        #[test]
        fn what_dpkg_rejects_is_refused() {
            assert_eq!(check(""), Err(BadVersion::Empty));
            assert_eq!(check("v1.3"), Err(BadVersion::NoLeadingDigit));
            assert_eq!(check("~1"), Err(BadVersion::NoLeadingDigit));
            for bad in ["1.3 beta", "1:2.0", "1.0-1", "1_0", "1.0/2", "1.0é"] {
                assert_eq!(check(bad), Err(BadVersion::BadCharacter), "{bad}");
            }
            assert_eq!(compare("1.0", "v1"), Err(BadVersion::NoLeadingDigit));
            assert_eq!(compare("x", "1.0"), Err(BadVersion::NoLeadingDigit));
            for good in ["0", "1.3", "2.11", "1.0~rc1+dfsg", "9.0.0.0A"] {
                assert_eq!(check(good), Ok(()), "{good}");
            }
        }
    }
}

/// **The boot test's names, in one place for the programs that must agree on them**:
/// `package_fetch_exerciser` asks, `system_tests/src/user/package_index_tests.rs` grants and judges.
/// The hosts that answer, `helpers/name-server-peer` and `helpers/tls-peer`, spell these again in
/// Python on purpose, as `domain_name_resolution_contract::fixture` explains.
pub mod fixture {
    /// The image's first index address, standing in for `basalt.nifeos.org`. The name server
    /// refuses it, so the client must move to [`BACKUP_HOST`] (Q2's backup address).
    pub const PRIMARY_HOST: &str = "gone.basalt.test";
    /// The image's second index address. `helpers/tls-peer` presents the pinned test authority's
    /// certificate for this name.
    pub const BACKUP_HOST: &str = "basalt.test";
    /// `helpers/tls-peer`'s port in every runner. Production would be 443.
    pub const INDEX_PORT: u16 = 8443;
    /// The zone the test grants the client's resolver badge: the root, every name, as `jig`'s is
    /// (Q5). Q1's address safeguard is what bounds where a listed location may reach.
    pub const ZONE: &str = "";
    /// Where the stand-in index lists every package as also living. It resolves to the runners'
    /// peer at 10.0.2.9, a private address, so the client must refuse it and fall back.
    pub const LISTED_HOST: &str = "packages.basalt.test";
    /// A package the index lists and the repository serves intact.
    pub const GENUINE: &str = "greeting@0.1.0";
    /// A package the index lists whose repository copy has one byte flipped.
    pub const TAMPERED: &str = "uptime@0.1.0";
    /// A package the index does not list, so nothing is fetched.
    pub const ABSENT: &str = "nosuch";
    /// The `greeting` twin the index lists under the rebinding name
    /// `rebind.basalt.test`, for the address-check attack of milestone 871 (a sixth
    /// outsider pass attacks the confinement claim).
    pub const REBOUND: &str = "rebound@0.1.0";
}

// An image's index sources (milestone 801 (packages over the internet)): where an image says its
// distribution's index is, and the root its TLS must chain to, read from the image's
// `package_index_sources` archive entry ([`SOURCES_ENTRY`]). The format is on [`Sources`] and in
// notes/packages/the-index-crate.md. Every name in this section is provisional.

/// The archive entry an image's index sources travel in, packed before the measurement table
/// so the table vouches for it, as `package_archive::CATALOG` is. Name: provisional.
pub const SOURCES_ENTRY: &str = "package_index_sources";

/// basalt's production sources: the text an image built for real machines carries (xtask's
/// `image_index_sources` chooses it or the test image's). `basalt.nifeos.org` (GitHub Pages, calef,
/// 2026-10-12 UTC) on 443, pinned to ISRG Root X1 as §250 (an image names its distribution's
/// package index) rules, then `basalt.crickertech.com` on another registrable domain (clause 6).
/// See notes/packages/the-index-crate.md.
///
/// Name: provisional, minted 2026-10-13 (UTC) by lane `lane/basalt-on-pages-and-releases`. The
/// host label `basalt.crickertech.com` is provisional too: calef chose the domain, and `basalt.`
/// follows `basalt.nifeos.org` without his ruling on it.
pub const BASALT_SOURCES: &str = "\
# basalt's index (§250): GitHub Pages under basalt.nifeos.org, pinned to ISRG Root X1.
repository basalt.nifeos.org 443 rolling
# The second address, on another registrable domain (§250 clause 6), tried when the first fails.
repository basalt.crickertech.com 443 rolling
root isrg-root-x1
# GitHub Release assets: github.com chains to Sectigo E46 (calef's C2 on #2048, 2026-10-12 UTC).
location-root sectigo-root-e46
";

/// **The index store's directory**, inside the activation directory: `activation/indexes`, which
/// the progenitor opens for a package manager holding `grant_plan::Manifest::indexes` (milestone
/// 801 (packages over the internet), ruling I2). Name: provisional 2026-10-12 (UTC).
pub const STORE_DIRECTORY: &str = "indexes";

/// **The copy of the image's own index** in that directory, which `jig update` writes and the
/// progenitor's installer reads: this architecture's lines of the index the image's sources named,
/// as they arrived. At most a page, so the installer reads it in one call. Name: provisional
/// 2026-10-12 (UTC).
pub const IMAGE_INDEX_COPY: &str = "image";

/// The largest index copy, in bytes: less than a page, so the end of the text is in it.
pub const COPY_MAX: usize = 4095;

/// The most repositories an image may name: the index's address and a backup (Q2), and room for
/// one more. A fixed bound so the reader allocates nothing.
pub const MAX_REPOSITORIES: usize = 4;

/// One index address the image names, with where to connect for it when the image says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Configured<'a> {
    /// The host is also the TLS server name.
    pub repository: Repository<'a>,
    /// Where to connect, if the image says; otherwise the host is resolved.
    pub address: Option<Ipv4Addr>,
}

/// The root every repository must chain to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Root<'a> {
    /// A root the client carries, by name: `isrg-root-x1` is the only one today.
    Named(&'a str),
    /// A certificate in DER, as hex; [`Root::der`] decodes it.
    Der(&'a str),
}

impl Root<'_> {
    /// Decode a [`Root::Der`] into `out`, or `None` for a named root or one that does not fit.
    pub fn der<'o>(&self, out: &'o mut [u8]) -> Option<&'o [u8]> {
        let Root::Der(hex) = self else {
            return None;
        };
        let n = hex.len() / 2;
        let out = out.get_mut(..n)?;
        for (i, b) in out.iter_mut().enumerate() {
            *b = u8::from_str_radix(hex.get(2 * i..2 * i + 2)?, 16).ok()?;
        }
        Some(out)
    }
}

/// An image's index sources, every line read: at least one repository, exactly one root.
///
/// ```text
/// repository <host> <port> <channel> [<address>]   one per index address, tried in order
/// root <name> | root <hex>                          the one root every repository chains to
/// location-root <name> [<root>]                     a further root for listed locations
/// ```
///
/// Blank lines and `#` comments are skipped. See notes/packages/the-index-crate.md.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sources<'a> {
    repositories: [Option<Configured<'a>>; MAX_REPOSITORIES],
    /// The index's root: every repository here chains to it, and so does a listed location that
    /// names no root, and so does a redirect's target.
    pub root: Root<'a>,
    location_roots: [Option<(&'a str, Root<'a>)>; MAX_LOCATION_ROOTS],
}

/// **The most further roots an image may carry for listed locations**: Sectigo E46 for GitHub
/// Releases (calef's C2 on #2048, 2026-10-12 UTC), and room for one more. Name: provisional,
/// minted 2026-10-13 (UTC) by lane `lane/basalt-on-pages-and-releases`.
pub const MAX_LOCATION_ROOTS: usize = 2;

/// Why [`Sources::parse`] refused the text, with the line's number from 1 where one line is why.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourcesRefusal {
    /// That line is not one of the two shapes.
    BadLine(usize),
    /// More than [`MAX_REPOSITORIES`] repositories.
    TooMany,
    /// No repository at all.
    NoRepository,
    /// No root, or a second one.
    OneRoot,
    /// More than [`MAX_LOCATION_ROOTS`] `location-root` lines, or one name twice.
    TooManyLocationRoots,
}

impl<'a> Sources<'a> {
    /// Read the text, refusing it whole at the first line that is neither shape.
    pub fn parse(text: &'a str) -> Result<Sources<'a>, SourcesRefusal> {
        let mut repositories = [None; MAX_REPOSITORIES];
        let mut count = 0;
        let mut root = None;
        let mut location_roots = [None; MAX_LOCATION_ROOTS];
        let mut further = 0;
        for (i, line) in text.split('\n').enumerate() {
            let number = i + 1;
            let line = line.trim_end_matches('\r');
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let mut words = line.split(' ');
            match words.next() {
                Some("repository") => {
                    let configured =
                        repository(&mut words).ok_or(SourcesRefusal::BadLine(number))?;
                    let slot = repositories.get_mut(count).ok_or(SourcesRefusal::TooMany)?;
                    *slot = Some(configured);
                    count += 1;
                }
                Some("root") => {
                    let named = words.next().ok_or(SourcesRefusal::BadLine(number))?;
                    if words.next().is_some() {
                        return Err(SourcesRefusal::BadLine(number));
                    }
                    if root.is_some() {
                        return Err(SourcesRefusal::OneRoot);
                    }
                    root = Some(root_word(named).ok_or(SourcesRefusal::BadLine(number))?);
                }
                // `location-root <name> [<root>]`: a root the signed index may name for a listed
                // location. Without a second word the name is one the client carries itself.
                Some("location-root") => {
                    let name = words
                        .next()
                        .filter(|n| is_root_name(n))
                        .ok_or(SourcesRefusal::BadLine(number))?;
                    let found = match words.next() {
                        None => Root::Named(name),
                        Some(word) => root_word(word).ok_or(SourcesRefusal::BadLine(number))?,
                    };
                    if words.next().is_some() {
                        return Err(SourcesRefusal::BadLine(number));
                    }
                    if location_roots.iter().flatten().any(|(n, _)| *n == name) {
                        return Err(SourcesRefusal::TooManyLocationRoots);
                    }
                    let slot = location_roots
                        .get_mut(further)
                        .ok_or(SourcesRefusal::TooManyLocationRoots)?;
                    *slot = Some((name, found));
                    further += 1;
                }
                _ => return Err(SourcesRefusal::BadLine(number)),
            }
        }
        if count == 0 {
            return Err(SourcesRefusal::NoRepository);
        }
        Ok(Sources {
            repositories,
            root: root.ok_or(SourcesRefusal::OneRoot)?,
            location_roots,
        })
    }

    /// The repositories, in the order they are tried.
    pub fn repositories(&self) -> impl Iterator<Item = Configured<'a>> + '_ {
        self.repositories.iter().flatten().copied()
    }

    /// **The root a listed location's certificate must chain to** (calef's C2, 2026-10-12 UTC, on
    /// #2048): the index's own when the location names none; otherwise the `location-root` of
    /// that name, or the index's root when it is a named root of that name. `None` when the index
    /// names a root this image does not carry, and the client passes over that location.
    ///
    /// The signed index can only choose among roots the image already trusts, never add one: the
    /// roots are the image's (measured with it), and the index carries names.
    ///
    /// Name: provisional, minted 2026-10-13 (UTC) by lane `lane/basalt-on-pages-and-releases`.
    pub fn root_for(&self, location: &Location<'_>) -> Option<Root<'a>> {
        let Some(name) = location.root else {
            return Some(self.root);
        };
        if self.root == Root::Named(name) {
            return Some(self.root);
        }
        self.location_roots
            .iter()
            .flatten()
            .find(|(n, _)| *n == name)
            .map(|(_, root)| *root)
    }
}

fn repository<'a>(words: &mut core::str::Split<'a, char>) -> Option<Configured<'a>> {
    let host = words.next()?;
    let port = words.next()?;
    let channel = words.next()?;
    let address = match words.next() {
        None => None,
        Some(a) => Some(Ipv4Addr::from(canonical_ipv4(a)?)),
    };
    if words.next().is_some() || port.len() > 5 || !port.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let repository = Repository::new(host, port.parse().ok()?, channel)?;
    Some(Configured {
        repository,
        address,
    })
}

/// A root's name: lowercase letters, digits and hyphens, with at least one hyphen, so it is never
/// all hex and never a host.
fn is_root_name(word: &str) -> bool {
    !word.is_empty()
        && word.len() <= 64
        && word.contains('-')
        && word
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// A root's word: a name of lowercase letters, digits and hyphens, or an even run of hex digits
/// long enough to be a certificate (a name is never all hex, since every name the client carries
/// has a hyphen).
fn root_word(word: &str) -> Option<Root<'_>> {
    let hex = word.len() >= 64
        && word.len().is_multiple_of(2)
        && word.bytes().all(|b| b.is_ascii_hexdigit());
    if hex {
        return Some(Root::Der(word));
    }
    is_root_name(word).then_some(Root::Named(word))
}

#[cfg(test)]
mod sources_tests {
    use super::*;

    const TEST: &str = "# the test image\n\
        repository gone.basalt.test 8443 rolling\n\
        repository basalt.test 8443 rolling 10.0.2.9\n\
        root 3082aabb3082aabb3082aabb3082aabb3082aabb3082aabb3082aabb3082aabb\n";

    #[test]
    fn the_test_images_sources_read_in_order_with_their_root() {
        let s = Sources::parse(TEST).unwrap();
        let r: Vec<_> = s.repositories().collect();
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].repository.host, "gone.basalt.test");
        assert_eq!(r[0].address, None);
        assert_eq!(r[1].repository.port, 8443);
        assert_eq!(r[1].address, Some(Ipv4Addr::new(10, 0, 2, 9)));
        let mut out = [0u8; 64];
        assert_eq!(
            &s.root.der(&mut out).unwrap()[..4],
            &[0x30, 0x82, 0xaa, 0xbb]
        );
    }

    #[test]
    fn the_production_pin_is_named() {
        let s = Sources::parse(BASALT_SOURCES).unwrap();
        assert_eq!(s.root, Root::Named("isrg-root-x1"));
        assert_eq!(s.root.der(&mut [0u8; 8]), None);
        let r: Vec<_> = s.repositories().collect();
        let hosts = ["basalt.nifeos.org", "basalt.crickertech.com"];
        assert_eq!(r.len(), hosts.len());
        for (configured, host) in r.iter().zip(hosts) {
            assert_eq!(
                configured.repository,
                Repository::new(host, 443, PROVISIONAL_CHANNEL).unwrap()
            );
            // Resolved by name, never by an address the image fixes.
            assert_eq!(configured.address, None);
        }
    }

    #[test]
    fn a_listed_location_chains_to_the_root_the_index_names_and_the_image_carries() {
        let s = Sources::parse(BASALT_SOURCES).unwrap();
        let at = |t| Location::parse(t).unwrap();
        let x1 = Root::Named("isrg-root-x1");
        assert_eq!(s.root_for(&at("https://basalt.nifeos.org/p")), Some(x1));
        assert_eq!(
            s.root_for(&at("isrg-root-x1=https://a.example/p")),
            Some(x1)
        );
        assert_eq!(
            s.root_for(&at(
                "sectigo-root-e46=https://github.com/o/r/releases/download/t/x"
            )),
            Some(Root::Named("sectigo-root-e46"))
        );
        // A name the image does not carry: the index cannot add a root.
        assert_eq!(s.root_for(&at("some-other-root=https://a.example/p")), None);
        // The test image names its second authority by hex.
        let test = Sources::parse(
            "repository basalt.test 8443 rolling 10.0.2.9\n\
             root 3082aabb3082aabb3082aabb3082aabb3082aabb3082aabb3082aabb3082aabb\n\
             location-root test-second-root 3082ccdd3082ccdd3082ccdd3082ccdd3082ccdd3082ccdd3082ccdd3082ccdd\n",
        )
        .unwrap();
        assert!(matches!(
            test.root_for(&at("test-second-root=https://releases.basalt.test:8443/x")),
            Some(Root::Der(h)) if h.starts_with("3082ccdd")
        ));
        assert!(
            matches!(test.root_for(&at("https://basalt.test/x")), Some(Root::Der(h)) if h.starts_with("3082aabb"))
        );
    }

    #[test]
    fn location_roots_are_bounded_and_named_once() {
        let base = "repository h.example 443 rolling\nroot isrg-root-x1\n";
        let refused = [
            (
                "location-root a-b\nlocation-root c-d\nlocation-root e-f\n",
                SourcesRefusal::TooManyLocationRoots,
            ),
            (
                "location-root a-b\nlocation-root a-b\n",
                SourcesRefusal::TooManyLocationRoots,
            ),
            ("location-root nohyphen\n", SourcesRefusal::BadLine(3)),
            ("location-root a-b c-d e-f\n", SourcesRefusal::BadLine(3)),
        ];
        for (extra, why) in refused {
            let text = std::format!("{base}{extra}");
            assert_eq!(Sources::parse(&text), Err(why), "{extra:?}");
        }
    }

    /// Five digits is the longest port the check lets through, and the highest port is one.
    #[test]
    fn a_five_digit_port_is_read() {
        let s = Sources::parse("repository h 65535 rolling\nroot a-b\n").unwrap();
        let r: Vec<_> = s.repositories().collect();
        assert_eq!(r[0].repository.port, 65535);
    }

    #[test]
    fn one_unreadable_line_refuses_the_whole() {
        let refused = [
            (
                "repository h 443\nroot isrg-root-x1\n",
                SourcesRefusal::BadLine(1),
            ),
            (
                "repository h 0 rolling\nroot isrg-root-x1\n",
                SourcesRefusal::BadLine(1),
            ),
            (
                "repository h 443 Rolling\nroot isrg-root-x1\n",
                SourcesRefusal::BadLine(1),
            ),
            (
                "repository h 443 rolling 010.0.2.9\nroot a-b\n",
                SourcesRefusal::BadLine(1),
            ),
            (
                "repository h 443 rolling 10.0.2.9 x\nroot a-b\n",
                SourcesRefusal::BadLine(1),
            ),
            (
                "repository h 443 rolling\nroot ISRG\n",
                SourcesRefusal::BadLine(2),
            ),
            (
                "repository h 443 rolling\nroot abc\n",
                SourcesRefusal::BadLine(2),
            ),
            // Even and all hex, but far too short to be a certificate, and no hyphen to be a name.
            (
                "repository h 443 rolling\nroot abcd\n",
                SourcesRefusal::BadLine(2),
            ),
            // A port is at most five decimal digits. `u16`'s parser alone would take a sign and
            // any number of leading zeros.
            (
                "repository h +443 rolling\nroot a-b\n",
                SourcesRefusal::BadLine(1),
            ),
            (
                "repository h 000443 rolling\nroot a-b\n",
                SourcesRefusal::BadLine(1),
            ),
            (
                "repository h 443 rolling\nmirror x\nroot a-b\n",
                SourcesRefusal::BadLine(2),
            ),
            ("root a-b\n", SourcesRefusal::NoRepository),
            ("repository h 443 rolling\n", SourcesRefusal::OneRoot),
            (
                "repository h 443 rolling\nroot a-b\nroot c-d\n",
                SourcesRefusal::OneRoot,
            ),
            (
                "repository a 1 c\nrepository b 1 c\nrepository c 1 c\nrepository d 1 c\n\
                 repository e 1 c\nroot a-b\n",
                SourcesRefusal::TooMany,
            ),
        ];
        for (text, why) in refused {
            assert_eq!(Sources::parse(text), Err(why), "{text:?}");
        }
    }

    #[test]
    fn hex_that_does_not_fit_is_not_decoded() {
        let s = Sources::parse(TEST).unwrap();
        assert_eq!(s.root.der(&mut [0u8; 8]), None);
    }
}

#[cfg(test)]
mod tests {
    use measured_boot::sha256;
    use package_archive::{package_size, write_package};

    use super::*;

    fn package(name: &str, version: &str, program: &[u8]) -> Vec<u8> {
        let members: [(&str, &[u8]); 1] = [(name, program)];
        let mut out = vec![0u8; package_size(&members)];
        let attributes = package_archive::Attributes {
            name,
            version,
            architecture: "aarch64",
        };
        write_package(&attributes, &members, &mut out).unwrap();
        out
    }

    fn line(stem: &str, bytes: &[u8], locations: &str) -> String {
        let digest = digest_text(&sha256(bytes));
        let digest = core::str::from_utf8(&digest).unwrap();
        if locations.is_empty() {
            format!("{stem} {digest}\n")
        } else {
            format!("{stem} {digest} {locations}\n")
        }
    }

    #[test]
    fn a_location_is_https_a_host_a_port_and_a_path() {
        let l = Location::parse("https://packages.example:8443/a/b.nifepkg?x=1").unwrap();
        assert_eq!(
            (l.host, l.port, l.path),
            ("packages.example", 8443, "/a/b.nifepkg?x=1")
        );
        assert_eq!(Location::parse("https://h/p").unwrap().port, 443);
        assert_eq!(
            Location::parse("https://8.8.8.8/p").unwrap().host,
            "8.8.8.8"
        );
    }

    #[test]
    fn a_location_may_name_its_root_and_a_redirect_may_not() {
        let l = Location::parse("sectigo-root-e46=https://github.com/o/r/releases/download/t/x")
            .unwrap();
        assert_eq!((l.root, l.host), (Some("sectigo-root-e46"), "github.com"));
        assert_eq!(Location::parse("https://h/p").unwrap().root, None);
        // A query that carries a URL is a path, not a root.
        let q = Location::parse("https://h/p?u=https://x/").unwrap();
        assert_eq!((q.root, q.path), (None, "/p?u=https://x/"));
        for bad in [
            "Root=https://h/p",
            "nohyphen=https://h/p",
            "a-b=http://h/p",
            "=https://h/p",
        ] {
            assert_eq!(
                Location::parse(bad),
                Err(LocationRefusal::Malformed),
                "{bad}"
            );
        }
        // GitHub's redirect: a 931-byte path, read as a redirect and refused as a listing.
        let long = std::format!("https://release-assets.example/{}", "a".repeat(930));
        assert_eq!(Location::redirect(&long).unwrap().path.len(), 931);
        assert_eq!(Location::parse(&long), Err(LocationRefusal::Malformed));
        let too_long = std::format!("https://h/{}", "a".repeat(REDIRECT_PATH_MAX));
        assert_eq!(
            Location::redirect(&too_long),
            Err(LocationRefusal::Malformed)
        );
        for bad in [
            "a-b=https://h/p",
            "http://h/p",
            "/relative",
            "https://10.0.0.1/p",
        ] {
            assert!(Location::redirect(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn the_judge_of_a_public_address_is_the_callers() {
        use core::net::SocketAddrV4;
        let peer = [SocketAddr::V4(SocketAddrV4::new(
            Ipv4Addr::new(10, 0, 2, 9),
            8443,
        ))];
        let at = Location::parse("https://releases.basalt.test:8443/x").unwrap();
        assert_eq!(
            at.check(&peer),
            Err(AddressRefusal::Private(Ipv4Addr::new(10, 0, 2, 9)))
        );
        let stand_in = |ip: [u8; 4]| ip == [10, 0, 2, 9] || public_address(ip);
        assert_eq!(at.check_by(&peer, stand_in).unwrap().addresses(), &peer[..]);
    }

    #[test]
    fn a_location_that_is_not_that_shape_is_refused() {
        for bad in [
            "http://h/p",
            "ftp://h/p",
            "https://h",
            "https:///p",
            "https://h:0/p",
            "https://h:65536/p",
            "https://h:/p",
            "https://user@h/p",
            "https://-h/p",
            "https://h..x/p",
            "https://h/p q",
            "https://h/p#frag",
            "HTTPS://h/p",
        ] {
            assert_eq!(
                Location::parse(bad),
                Err(LocationRefusal::Malformed),
                "{bad}"
            );
        }
        let long = format!("https://h/{}", "a".repeat(PATH_MAX));
        assert_eq!(Location::parse(&long), Err(LocationRefusal::Malformed));
    }

    /// **Each limit is inclusive.** A port is at most five digits and nothing but digits, so a
    /// sign or zero padding past five is refused, and a path of exactly [`PATH_MAX`] fits.
    #[test]
    fn a_location_at_each_limit_is_read_and_one_past_it_is_not() {
        assert_eq!(Location::parse("https://h:65535/p").unwrap().port, 65535);
        assert_eq!(
            Location::parse("https://h:+80/p"),
            Err(LocationRefusal::Malformed)
        );
        assert_eq!(
            Location::parse("https://h:000080/p"),
            Err(LocationRefusal::Malformed)
        );
        let path = format!("/{}", "a".repeat(PATH_MAX - 1));
        let at_limit = format!("https://h{path}");
        assert_eq!(Location::parse(&at_limit).unwrap().path, path);
    }

    /// A stem may be [`STEM_LEN`] bytes and no more, and not empty. The longest one still fits
    /// the one-line catalog `accept` writes for it, so a package is judged rather than a buffer
    /// overrun.
    #[test]
    fn a_stem_of_the_longest_length_is_read_and_judged() {
        let n = package_archive::NAME_LEN;
        let stem = format!("{}-{}-{}", "a".repeat(n), "1".repeat(n), "c".repeat(n));
        assert_eq!(stem.len(), STEM_LEN);
        let bytes = package("greeting", "0.1.0", b"\x7fELF g");
        let text = line(&stem, &bytes, "");
        let index = Index::parse(&text).unwrap();
        let entry = index.entries().next().unwrap();
        assert_eq!(entry.stem, stem);
        assert_eq!(
            accept(&entry, &bytes).map(|i| i.package().name()),
            Err(Refusal::NotRequested)
        );

        let long = format!("a{stem}");
        assert!(Index::parse(&line(&long, &bytes, "")).is_err());
        let digest = digest_text(&sha256(b"a"));
        let empty = format!(" {}\n", core::str::from_utf8(&digest).unwrap());
        assert!(Index::parse(&empty).is_err());
    }

    /// The same line twice is one package, not two versions of it.
    #[test]
    fn the_same_line_twice_is_one_package() {
        let one = line("greeting-0.1.0-aarch64", b"a", "https://h/1");
        let twice = format!("{one}{one}");
        let index = Index::parse(&twice).unwrap();
        assert!(index.find("greeting", "aarch64").is_ok());
    }

    /// Q1's safeguard, on address literals and as the check a client runs on resolved addresses.
    #[test]
    fn a_listed_location_never_reaches_a_private_or_link_local_address() {
        for private in [
            [10, 0, 2, 9],
            [172, 16, 0, 1],
            [172, 31, 255, 255],
            [192, 168, 1, 1],
            [169, 254, 0, 1],
            [127, 0, 0, 1],
            [0, 0, 0, 0],
            [100, 64, 0, 1],
            [224, 0, 0, 1],
            [255, 255, 255, 255],
        ] {
            assert!(!public_address(private), "{private:?}");
            let text = format!(
                "https://{}.{}.{}.{}/p",
                private[0], private[1], private[2], private[3]
            );
            assert_eq!(
                Location::parse(&text),
                Err(LocationRefusal::Private(Ipv4Addr::from(private))),
                "{text}"
            );
        }
        for public in [
            [8, 8, 8, 8],
            [172, 15, 0, 1],
            [172, 32, 0, 1],
            [100, 63, 0, 1],
            [1, 1, 1, 1],
        ] {
            assert!(public_address(public), "{public:?}");
        }
    }

    /// calef's ruling of 2026-10-12 (UTC): a host that ends in a number is an address written the
    /// canonical way or is refused, by name, before anything resolves it. `0010.0.0.1` was read as
    /// a DNS name before, and `inet_aton` reads it as 8.0.0.1.
    #[test]
    fn a_host_that_ends_in_a_number_must_be_a_canonical_address() {
        let at = Location::parse("https://8.0.10.8/p").unwrap();
        assert_eq!(at.host, "8.0.10.8");
        assert!(Location::parse("https://1password.example/p").is_ok());
        assert!(Location::parse("https://a.0x1g/p").is_ok());
        for host in [
            "0010.0.0.1",
            "8.8.8.08",
            "00.8.8.8",
            "0x0a.0.0.1",
            "8.8.8.0x8",
            "8.8.8.0X8",
            "8.8.8.0x",
            "10.1",
            "167772161",
            "8.8.8.8.8",
            "8.8.8.256",
            "packages.example.8",
        ] {
            let text = format!("https://{host}/p");
            let refused = Location::parse(&text).unwrap_err();
            assert_eq!(refused, LocationRefusal::NonCanonicalAddress(host));
            let said = refused.to_string();
            assert!(said.contains(host) && said.contains("canonical"), "{said}");
            assert_eq!(Repository::new(host, 443, "rolling"), None, "{host}");
        }
        let text = line(
            "greeting-0.1.0-aarch64",
            b"a",
            "https://h/1 https://0010.0.0.1/g",
        );
        let refused = Index::parse(&text).unwrap_err();
        assert_eq!(
            refused.location,
            Some(LocationRefusal::NonCanonicalAddress("0010.0.0.1"))
        );
        assert!(Repository::new("10.0.0.5", 443, "rolling").is_some());
    }

    #[test]
    fn a_repository_is_a_channel_with_metadata_and_targets_under_it() {
        let r = Repository::new("basalt.nifeos.org", 443, PROVISIONAL_CHANNEL).unwrap();
        let mut buf = [0u8; PATH_MAX];
        assert_eq!(
            r.metadata_path("timestamp.json", &mut buf),
            Some("/rolling/metadata/timestamp.json")
        );
        let mut buf = [0u8; PATH_MAX];
        assert_eq!(
            r.target_path("greeting-0.1.0-aarch64", &mut buf),
            Some("/rolling/targets/greeting-0.1.0-aarch64.nifepkg")
        );
        for (host, port, channel) in [
            ("h", 0, "c"),
            ("h", 1, ""),
            ("h", 1, "Rolling"),
            ("h", 1, "a/b"),
            ("", 1, "c"),
        ] {
            assert_eq!(
                Repository::new(host, port, channel),
                None,
                "{host} {port} {channel}"
            );
        }
        let mut buf = [0u8; PATH_MAX];
        assert_eq!(r.target_path(&"x".repeat(PATH_MAX), &mut buf), None);
    }

    #[test]
    fn find_reads_name_and_version_as_the_catalog_does() {
        let text = format!(
            "{}{}{}",
            line("greeting-0.1.0-aarch64", b"a", "https://h/1"),
            line("greeting-0.2.0-aarch64", b"b", ""),
            line("greeting-0.1.0-riscv64", b"c", "https://h/3 https://g/3"),
        );
        let index = Index::parse(&text).unwrap();
        assert_eq!(
            index.find("greeting", "aarch64"),
            Err(Miss::SeveralVersions)
        );
        let e = index.find("greeting@0.2.0", "aarch64").unwrap();
        assert_eq!(
            (e.stem, e.locations().count()),
            ("greeting-0.2.0-aarch64", 0)
        );
        let e = index.find("greeting", "riscv64").unwrap();
        let hosts: Vec<_> = e.locations().map(|l| l.host).collect();
        assert_eq!(hosts, ["h", "g"]);
        assert_eq!(index.find("greeting", "x86_64"), Err(Miss::NoSuchPackage));
        assert_eq!(index.find("nosuch", "aarch64"), Err(Miss::NoSuchPackage));
    }

    /// Q1's order: the listed locations, then the repository's own copy; a pinned mirror alone.
    #[test]
    fn sources_are_the_listed_locations_then_the_repository_or_the_pinned_mirror_alone() {
        let text = line(
            "greeting-0.1.0-aarch64",
            b"a",
            "https://one.example/a https://two.example/a",
        );
        let index = Index::parse(&text).unwrap();
        let entry = index.find("greeting", "aarch64").unwrap();
        let basalt = Repository::new("basalt.nifeos.org", 443, "rolling").unwrap();
        let mirror = Repository::new("10.0.0.5", 8443, "rolling").unwrap();
        let order: Vec<_> = Index::sources(&entry, &basalt, None).collect();
        assert_eq!(order.len(), 3);
        assert!(matches!(order[0], Source::Listed(l) if l.host == "one.example"));
        assert!(matches!(order[1], Source::Listed(l) if l.host == "two.example"));
        assert_eq!(order[2], Source::Repository(&basalt));
        let pinned: Vec<_> = Index::sources(&entry, &basalt, Some(&mirror)).collect();
        assert_eq!(pinned, [Source::Repository(&mirror)]);
        let bare = line("greeting-0.1.0-aarch64", b"a", "");
        let index = Index::parse(&bare).unwrap();
        let entry = index.find("greeting", "aarch64").unwrap();
        let order: Vec<_> = Index::sources(&entry, &basalt, None).collect();
        assert_eq!(order, [Source::Repository(&basalt)]);
    }

    #[test]
    fn one_unreadable_line_refuses_the_whole_index() {
        let good = line("greeting-0.1.0-aarch64", b"a", "https://h/1");
        let plain_http = line("uptime-0.1.0-aarch64", b"b", "http://h/1");
        let private = line(
            "uptime-0.1.0-aarch64",
            b"b",
            "https://h/1 https://192.168.1.1/u",
        );
        for bad in [
            "greeting-0.1.0-aarch64 sha256:00 https://h/1",
            "greeting-0.1.0-aarch64 https://h/1",
            "greeting-0.1.0-aarch64",
            "moved-to https://elsewhere/index",
            plain_http.trim_end(),
            private.trim_end(),
        ] {
            let text = format!("{good}{bad}\n");
            let refused = Index::parse(&text).unwrap_err();
            assert_eq!((refused.number, refused.line), (2, bad));
        }
    }

    /// **A version dpkg would not read refuses the index**, wherever its line is and whatever
    /// architecture it is for, because basalt and `jig` must order the same strings the same way.
    #[test]
    fn a_version_dpkg_would_not_read_refuses_the_index() {
        let good = line("greeting-1.3-aarch64", b"a", "");
        for stem in [
            "greeting-v1.3-riscv64",
            "greeting-1.3_1-aarch64",
            "greeting-aarch64",
        ] {
            let bad = line(stem, b"b", "");
            let text = format!("{good}{bad}");
            let refused = Index::parse(&text).unwrap_err();
            assert_eq!((refused.number, refused.location), (2, None), "{stem}");
        }
        assert!(Index::parse(&good).is_ok());
    }

    #[test]
    fn two_lines_for_one_stem_that_disagree_are_refused() {
        let text = format!(
            "{}{}",
            line("greeting-0.1.0-aarch64", b"a", "https://h/1"),
            line("greeting-0.1.0-aarch64", b"a", "https://elsewhere/1"),
        );
        let index = Index::parse(&text).unwrap();
        assert_eq!(
            index.find("greeting", "aarch64"),
            Err(Miss::SeveralVersions)
        );
    }

    /// The property item 1 exists for: the bytes are believed by the index's digest and by nothing
    /// about where they came from.
    #[test]
    fn fetched_bytes_are_accepted_only_if_they_are_the_entry_named() {
        let greeting = package("greeting", "0.1.0", b"\x7fELF greeting");
        let uptime = package("uptime", "0.1.0", b"\x7fELF uptime");
        let text = format!(
            "{}{}",
            line("greeting-0.1.0-aarch64", &greeting, "https://anywhere/g"),
            line("uptime-0.1.0-aarch64", &uptime, ""),
        );
        let index = Index::parse(&text).unwrap();
        let entry = index.find("greeting", "aarch64").unwrap();
        assert_eq!(
            accept(&entry, &greeting)
                .unwrap()
                .programs()
                .next()
                .unwrap()
                .name,
            "greeting"
        );
        let mut tampered = greeting.clone();
        let mid = tampered.len() / 2;
        tampered[mid] ^= 1;
        assert!(matches!(
            accept(&entry, &tampered),
            Err(Refusal::NotCataloged | Refusal::Unreadable)
        ));
        // A package the index also vouches for, served in place of the one asked for.
        assert_eq!(
            accept(&entry, &uptime).map(|i| i.package().name()),
            Err(Refusal::NotRequested)
        );
        assert_eq!(
            accept(&entry, b"not a package").map(|i| i.package().name()),
            Err(Refusal::Unreadable)
        );
    }

    /// The check judges the exact addresses it is handed, and a passed check hands back that same
    /// slice: one resolution, checked and dialed, never a second (milestone 871's escape).
    #[test]
    fn a_listed_location_is_dialed_only_at_the_addresses_its_check_passed() {
        let at = Location::parse("https://mirror.example:8443/g.nifepkg").unwrap();
        let public: [SocketAddr; 2] = [
            "192.0.2.1:8443".parse().unwrap(),
            "8.8.8.8:8443".parse().unwrap(),
        ];
        let checked = at.check(&public).unwrap();
        assert!(core::ptr::eq(checked.addresses(), &public[..]));
        assert_eq!(
            (checked.host(), checked.path()),
            ("mirror.example", "/g.nifepkg")
        );

        // One private answer among public ones refuses the whole location.
        let mixed: [SocketAddr; 2] = [
            "192.0.2.1:8443".parse().unwrap(),
            "10.0.2.9:8443".parse().unwrap(),
        ];
        assert_eq!(
            at.check(&mixed),
            Err(AddressRefusal::Private(Ipv4Addr::new(10, 0, 2, 9)))
        );
        // IPv6 is refused rather than passed unjudged; nothing resolved is refused too.
        let v6: [SocketAddr; 1] = ["[fe80::1]:8443".parse().unwrap()];
        assert!(matches!(at.check(&v6), Err(AddressRefusal::NotJudged(_))));
        assert_eq!(at.check(&[]), Err(AddressRefusal::NoAddress));
    }
}
