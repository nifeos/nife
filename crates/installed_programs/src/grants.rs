//! **The owner's grants**: the `grant` lines of a generation, read strictly (milestone 892 (a std
//! program at the prompt holds the network and a resolver); calef's ruling "a" on #2405). The line
//! format is the crate's documentation's; this module is the type and its readers, split from the
//! crate root under §266 (a Rust source file stays under 2,000 lines).
//!
//! Name: provisional, minted 2026-10-19 (UTC) by milestone/892-installed-programs-hold-the-network-and-zones.

use super::generation::{Line, classify, entries};
use super::{Digest, Error};

/// **One `grant` line**: what the owner granted the bytes with this digest. Read with
/// [`grants`] and [`network_granted`].
///
/// Name: provisional, minted 2026-10-19 (UTC) by milestone/892-installed-programs-hold-the-network-and-zones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Grant<'a> {
    /// The executable's digest, as its row has it.
    pub digest: Digest,
    /// What was granted.
    pub what: Granted<'a>,
}

/// **What a `grant` line grants.**
///
/// Name: provisional, minted 2026-10-19 (UTC) by milestone/892-installed-programs-hold-the-network-and-zones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Granted<'a> {
    /// The network: a client view of the stack, connecting out.
    Network,
    /// Names in one resolver zone, as dotted text; `""` is the root zone, written `.` in the line.
    Zone(&'a str),
    /// **Runs as a service, now and at every boot** (milestone 910 (a package declares a
    /// service)'s grant, recorded here since milestone 911 (scripted installs grant only what the
    /// owner's approval file allows)). Nothing starts a recorded service until 910 builds the
    /// start; the record and the subset test are this milestone's.
    ///
    ///
    /// Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.
    Service,
    /// **A directory the package may hold, with its rights** (milestone 911). The name is one
    /// word a row can spell; the rights are [`DirectoryRights`]. No manifest asks for one yet (the ask
    /// rides milestone 910's declaration), so the host tests are the only readers.
    ///
    ///
    /// Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.
    Directory(&'a str, DirectoryRights),
    /// **The installer endpoint, within the approval that allows it** (milestone 911's delegation).
    /// A manifest asks for it by the approval's name; the record carries the ask without the name,
    /// because a package may hold the endpoint within one approval only and the approval that
    /// answered is the row generation's `answered` line.
    ///
    ///
    /// Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.
    Installer,
}

/// **The rights a [`Granted::Directory`] grant carries**, as the approval file and the `grant` line
/// spell them: `r` reads and walks, `rw` writes too. A `r` ask lies within a `rw` allowance, and
/// not the other way.
///
/// Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectoryRights {
    /// Read, list and walk (`filesystem_contract::dir`'s `ENUMERATE | READ | DESCEND`).
    R,
    /// And write.
    RW,
}

impl DirectoryRights {
    /// The rights the spelling names, or `None` for anything else.
    ///
    ///
    /// Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.
    pub fn parse(spelling: &str) -> Option<Self> {
        match spelling {
            "r" => Some(DirectoryRights::R),
            "rw" => Some(DirectoryRights::RW),
            _ => None,
        }
    }

    /// The spelling, as the approval file and the `grant` line write it.
    ///
    ///
    /// Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.
    pub fn spell(self) -> &'static str {
        match self {
            DirectoryRights::R => "r",
            DirectoryRights::RW => "rw",
        }
    }

    /// Whether this set's every right is in `allowance`'s.
    ///
    ///
    /// Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.
    pub fn is_within(self, allowance: DirectoryRights) -> bool {
        match (self, allowance) {
            (DirectoryRights::R, _) => true,
            (DirectoryRights::RW, DirectoryRights::RW) => true,
            (DirectoryRights::RW, DirectoryRights::R) => false,
        }
    }
}

/// The spelling of the root zone in a `grant` line, where an empty word cannot be written.
///
/// Name: provisional, minted 2026-10-19 (UTC) by milestone/892-installed-programs-hold-the-network-and-zones.
pub const ROOT_ZONE: &str = ".";

/// The rest of a `grant` line after its first word: a digest, then `network`, `service`,
/// `installer`, `zone` and one canonical zone (`.` for the root), or `dir`, one name and `r`/`rw`.
pub(super) fn classify_grant<'a>(mut words: impl Iterator<Item = &'a str>) -> Line<'a> {
    let Some(Ok(digest)) = words.next().map(measured_boot::parse_digest) else {
        return Line::Malformed;
    };
    let what = match (words.next(), words.next(), words.next()) {
        (Some("network"), None, _) => Granted::Network,
        (Some("service"), None, _) => Granted::Service,
        (Some("installer"), None, _) => Granted::Installer,
        (Some("zone"), Some(ROOT_ZONE), None) => Granted::Zone(""),
        (Some("zone"), Some(zone), None)
            if domain_name_resolution_contract::is_canonical_zone(zone.as_bytes()) =>
        {
            Granted::Zone(zone)
        }
        (Some("dir"), Some(name), Some(rights)) if good_dir_name(name) => {
            let Some(rights) = DirectoryRights::parse(rights) else {
                return Line::Malformed;
            };
            Granted::Directory(name, rights)
        }
        _ => return Line::Malformed,
    };
    // A fourth word is a line this reader does not know, and a half-read table would vouch for
    // what survived it.
    if words.next().is_some() {
        return Line::Malformed;
    }
    Line::Grant(Grant { digest, what })
}

/// Whether `name` can be a directory grant's name: the words a row's columns are held to
/// ([`good_name`]), since the line is space-separated text.
fn good_dir_name(name: &str) -> bool {
    super::generation::good_name(name)
}

/// **Every `grant` line in a generation**, in file order, or [`Error::Malformed`]: at the first
/// line that is malformed, at a grant whose digest names no row, and at a grant that does not sort
/// strictly after the one before (one grant record has one spelling).
///
/// Name: provisional, minted 2026-10-19 (UTC) by milestone/892-installed-programs-hold-the-network-and-zones.
pub fn grants(table: &str) -> impl Iterator<Item = Result<Grant<'_>, Error>> {
    let mut previous: Option<Grant<'_>> = None;
    table
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(move |line| match classify(line) {
            Line::Grant(g) => {
                let sorted = previous.is_none_or(|p| grant_before(&p, &g));
                let named = entries(table).flatten().any(|e| e.digest == g.digest);
                previous = Some(g);
                Some(if sorted && named {
                    Ok(g)
                } else {
                    Err(Error::Malformed)
                })
            }
            Line::Row { .. } | Line::Pointer { .. } | Line::Answered(_) => None,
            Line::Malformed => Some(Err(Error::Malformed)),
        })
}

/// Whether `a` sorts strictly before `b` as their lines' text does.
pub(super) fn grant_before(a: &Grant<'_>, b: &Grant<'_>) -> bool {
    fn key<'g>(
        g: &Grant<'g>,
    ) -> (
        [u8; measured_boot::DIGEST_TEXT_LEN],
        &'static str,
        &'g str,
        &'static str,
    ) {
        let (kind, first, second) = match g.what {
            Granted::Network => ("network", "", ""),
            Granted::Service => ("service", "", ""),
            Granted::Installer => ("installer", "", ""),
            Granted::Zone("") => ("zone", ROOT_ZONE, ""),
            Granted::Zone(z) => ("zone", z, ""),
            Granted::Directory(name, rights) => ("dir", name, rights.spell()),
        };
        (measured_boot::digest_text(&g.digest), kind, first, second)
    }
    key(a) < key(b)
}

/// **Whether one grant set covers another ask** (milestone 911 (scripted installs grant only
/// what the owner's approval file allows)): the ask is one of the allowed grants. A zone is
/// covered by the same zone and no other (the root zone is spelled, never implied); a directory
/// by the same name and rights that hold its own ([`DirectoryRights::within`]); the network, the
/// service and the installer each by themselves. The digest is not compared: an approval file's
/// entry bounds a package, and the asks of every program it carries are checked against the one
/// entry.
pub(super) fn covered(allowed: &[Granted<'_>], ask: &Granted<'_>) -> bool {
    allowed.iter().any(|a| covers(*a, *ask))
}

/// Whether one [`Granted`] covers another, digests apart: the comparison the subset test and the
/// approval file's check share.
///
/// Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.
pub fn covers(allowed: Granted<'_>, ask: Granted<'_>) -> bool {
    match (allowed, ask) {
        (Granted::Network, Granted::Network)
        | (Granted::Service, Granted::Service)
        | (Granted::Installer, Granted::Installer) => true,
        (Granted::Zone(a), Granted::Zone(b)) => a == b,
        (Granted::Directory(an, ar), Granted::Directory(bn, br)) => an == bn && br.is_within(ar),
        _ => false,
    }
}

/// **Every ask `allowed` does not cover**, in `asks`' order: what a subset test names, one extra
/// grant at a time. It never grants the intersection: the caller refuses whole when this yields
/// anything, and names what it yields.
///
/// ```
/// use installed_programs::{covers, Granted};
/// assert!(covers(Granted::Network, Granted::Network));
/// assert!(!covers(Granted::Zone("nife.test"), Granted::Zone("example.net")));
/// ```
///
/// Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.
pub fn extras<'g>(
    allowed: &[Granted<'g>],
    asks: &'g [Granted<'g>],
) -> impl Iterator<Item = &'g Granted<'g>> {
    asks.iter().filter(move |ask| !covered(allowed, ask))
}

/// **Whether the owner granted the bytes with this digest the network**: a `grant <digest>
/// network` line, read through [`grants`], so a malformed table grants nothing and says so.
///
/// Name: provisional, minted 2026-10-19 (UTC) by milestone/892-installed-programs-hold-the-network-and-zones.
pub fn network_granted(table: &str, digest: &Digest) -> Result<bool, Error> {
    let mut granted = false;
    for g in grants(table) {
        let g = g?;
        granted |= g.digest == *digest && g.what == Granted::Network;
    }
    Ok(granted)
}
