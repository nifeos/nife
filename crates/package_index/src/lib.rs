//! **A distribution's package index: which packages exist, the digest each must hash to, and where
//! its bytes may be fetched**, for milestone 801 (packages over the internet).
//!
//! §250 (an image names its distribution's package index, and the bytes may live anywhere) splits
//! what the image's catalog holds together: the index comes from the distribution's repository, and
//! a package's bytes come from wherever the index says, believed only if they hash to the index's
//! digest (§195 (a reviewed recipe vouches for a package)). calef's rulings of 2026-10-10 (UTC) on
//! #1884 shape the rest, and `notes/packages/the-index-format.md` has them whole:
//!
//! - A [`Repository`] is a host and a channel: the index under `/<channel>/metadata/`, every
//!   package under `/<channel>/targets/`, all three architectures in one index (Q4).
//! - An [`Entry`] may list [`Location`]s, HTTPS only, tried in order; the repository's own
//!   `targets/` is always the last source, and an owner may pin one mirror instead (Q1,
//!   [`Index::sources`]). A listed location may never reach a private or link-local address
//!   ([`public_address`]); a repository the owner chose may.
//!
//! **The encoding is a stand-in, not TUF.** [`Index::parse`] reads the catalog's line followed by
//! zero or more locations, and the TUF client of milestone 858 (lab machines update themselves
//! through packages) replaces that function and nothing
//! above it. [`accept`] judges fetched bytes with the progenitor installer's own check,
//! `package_archive::installable_as`.
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
    /// `None` for a host or channel that is not that shape, or port 0.
    pub fn new(host: &'a str, port: u16, channel: &'a str) -> Option<Repository<'a>> {
        let channel_ok = !channel.is_empty()
            && channel.len() <= CHANNEL_MAX
            && channel
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
        (is_host(host) && port != 0 && channel_ok).then_some(Repository {
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

/// **One place the index says a package's bytes may be fetched**: `https://`, a host, an optional
/// port, a path. No user, no fragment; a query is part of the path. HTTPS only (Q1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Location<'a> {
    /// A DNS name, or a dotted IPv4 address [`public_address`] admits.
    pub host: &'a str,
    /// The port written, or 443.
    pub port: u16,
    /// Starts with `/`, at most [`PATH_MAX`] bytes, printable ASCII.
    pub path: &'a str,
}

impl<'a> Location<'a> {
    /// Read one location. `None` for anything that is not exactly the shape above, plain HTTP
    /// included, and for an address literal outside [`public_address`].
    pub fn parse(text: &'a str) -> Option<Location<'a>> {
        let rest = text.strip_prefix("https://")?;
        let slash = rest.find('/')?;
        let (authority, path) = rest.split_at(slash);
        let (host, port) = match authority.split_once(':') {
            Some((host, port)) => (host, parse_port(port)?),
            None => (authority, 443),
        };
        if !is_host(host) || path.len() > PATH_MAX || !path.bytes().all(|b| b.is_ascii_graphic()) {
            return None;
        }
        // A fragment is the browser's and never reaches a server.
        if path.contains('#') {
            return None;
        }
        if ipv4_literal(host).is_some_and(|ip| !public_address(ip)) {
            return None;
        }
        Some(Location { host, port, path })
    }
}

fn ipv4_literal(host: &str) -> Option<[u8; 4]> {
    let mut out = [0u8; 4];
    let mut parts = host.split('.');
    for b in out.iter_mut() {
        let part = parts.next()?;
        if part.is_empty() || part.len() > 3 || !part.bytes().all(|c| c.is_ascii_digit()) {
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
            .filter_map(Location::parse)
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
            if entry(line).is_none() {
                return Err(BadLine { number, line });
            }
        }
        Ok(index)
    }

    /// Every entry, in the order the index lists them.
    pub fn entries(&self) -> impl Iterator<Item = Entry<'a>> + 'a {
        let text = self.text;
        Index { text }.lines().filter_map(|(_, line)| entry(line))
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

fn entry(line: &str) -> Option<Entry<'_>> {
    let (stem, rest) = line.split_once(' ')?;
    let (digest, listed) = rest.split_once(' ').unwrap_or((rest, ""));
    if stem.is_empty() || stem.len() > STEM_LEN {
        return None;
    }
    if !listed.is_empty() && !listed.split(' ').all(|l| Location::parse(l).is_some()) {
        return None;
    }
    Some(Entry {
        stem,
        digest: parse_digest(digest).ok()?,
        listed,
    })
}

/// **May these fetched bytes be installed as `entry`?** The package must be the one the entry names
/// and its whole file must hash to the entry's digest; then its program member must match its own
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

/// **The boot test's names, in one place for the programs that must agree on them**:
/// `package_fetch_exerciser` asks, `system_tests/src/user/package_index_tests.rs` grants and judges.
/// The hosts that answer, `helpers/name-server-peer` and `helpers/tls-peer`, spell these again in
/// Python on purpose, as `name_resolution_protocol::fixture` explains.
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
    /// `rebind.basalt.test`, for the address-check attack of milestone 868 (a sixth
    /// outsider pass attacks the confinement claim).
    pub const REBOUND: &str = "rebound@0.1.0";
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
            assert_eq!(Location::parse(bad), None, "{bad}");
        }
        let long = format!("https://h/{}", "a".repeat(PATH_MAX));
        assert_eq!(Location::parse(&long), None);
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
            assert_eq!(Location::parse(&text), None, "{text}");
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
        assert_eq!(accept(&entry, &greeting).unwrap().program, "greeting");
        let mut tampered = greeting.clone();
        let mid = tampered.len() / 2;
        tampered[mid] ^= 1;
        assert!(matches!(
            accept(&entry, &tampered),
            Err(Refusal::NotCataloged | Refusal::Unreadable)
        ));
        // A package the index also vouches for, served in place of the one asked for.
        assert_eq!(
            accept(&entry, &uptime).map(|i| i.program),
            Err(Refusal::NotRequested)
        );
        assert_eq!(
            accept(&entry, b"not a package").map(|i| i.program),
            Err(Refusal::Unreadable)
        );
    }
}
