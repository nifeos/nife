//! **The base-update grant**: the approval file's `[base-update]` section and the host-callable
//! check that says whether a base update may be written (milestone 976 (an OS update through a
//! slot), part 7, under the approval file of milestone 911 (scripted installs grant only what the
//! owner's approval file allows); ruled 2026-11-01 UTC).
//!
//! The section holds two switches, `allow` and `unattended`, both false unless a person sets
//! them, and `accepts`, the keys the owner accepts. An update a person starts needs `allow`. An
//! update with nobody present needs `unattended` as well, and four bounds: it writes only the spare
//! slot, a key both the owner and the trust root accept signed its manifest, its version is
//! strictly greater than the running one, and no slot is on trial. A base version is exactly
//! MAJOR.MINOR.PATCH, three whole numbers compared left to right. The accepted keys live beside
//! the trust root; the file names which of them the owner accepts and never adds one.
//!
//! The check is pure. The caller has already verified the manifest's signature and passes the
//! identifier of the key that verified, so no cryptography lives here. No device updater calls it
//! yet (part 5); it is the rule that updater will call.
//!
//! # EXAMPLES
//!
//! ```
//! use approval_file::{File, base_update::{Request, Starter, Version}};
//! let text = "approval = \"fleet\"\n\n[base-update]\nallow = true\nunattended = true\naccepts = [\"release-1\"]\n";
//! let file = File::parse(text).unwrap();
//! let request = Request {
//!     starter: Starter::Unattended,
//!     writes_spare_slot_only: true,
//!     signed_by: Some("release-1"),
//!     trust_root_keys: &["release-1", "release-2"],
//!     candidate: Version::parse("1.2.4").unwrap(),
//!     running: Version::parse("1.2.3").unwrap(),
//!     slot_on_trial: false,
//! };
//! assert!(file.base_update().check(&request).is_ok());
//! ```
//!
//! Name: provisional, minted 2026-11-01 (UTC) by lane/976-part-7-the-base-update-grant.

use alloc::vec::Vec;
use core::fmt;

/// **A base version**: MAJOR.MINOR.PATCH, three whole numbers. Ordered left to right as integers,
/// so the field order below is the comparison order.
///
/// Name: provisional, minted 2026-11-01 (UTC) by lane/976-part-7-the-base-update-grant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    /// The first number.
    pub major: u64,
    /// The second number.
    pub minor: u64,
    /// The third number.
    pub patch: u64,
}

impl Version {
    /// The version `text` spells, or `None`. Exactly three decimal numbers joined by dots. A
    /// pre-release tag, build metadata, a sign, a space, an empty part, a fourth part and a
    /// number that does not fit 64 bits are all refused. A part with a leading zero is refused
    /// too (restrictive first, since a restricted form can admit more later and the reverse
    /// cannot; provisional, the ruling does not say).
    ///
    /// Name: provisional, minted 2026-11-01 (UTC) by lane/976-part-7-the-base-update-grant.
    pub fn parse(text: &str) -> Option<Version> {
        let mut parts = text.split('.');
        let major = number(parts.next()?)?;
        let minor = number(parts.next()?)?;
        let patch = number(parts.next()?)?;
        if parts.next().is_some() {
            return None;
        }
        Some(Version {
            major,
            minor,
            patch,
        })
    }
}

/// One version part: ASCII digits only, no leading zero unless it is `0` alone.
fn number(part: &str) -> Option<u64> {
    let bytes = part.as_bytes();
    if bytes.is_empty() || !bytes.iter().all(u8::is_ascii_digit) {
        return None;
    }
    if bytes.len() > 1 && bytes[0] == b'0' {
        return None;
    }
    part.parse().ok()
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Who started the update.
///
/// Name: provisional, minted 2026-11-01 (UTC) by lane/976-part-7-the-base-update-grant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Starter {
    /// A person asked for it.
    Person,
    /// Nobody is present: a timer or a script started it.
    Unattended,
}

/// **The `[base-update]` section**: both switches false and no key accepted unless the file
/// says otherwise, so a file without the section grants nothing.
///
/// Name: provisional, minted 2026-11-01 (UTC) by lane/976-part-7-the-base-update-grant.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Grant<'a> {
    pub(crate) allow: bool,
    pub(crate) unattended: bool,
    pub(crate) accepts: Vec<&'a str>,
}

/// **What the check is asked about**: the facts of one proposed base update, all established by
/// the caller before it asks.
///
/// Name: provisional, minted 2026-11-01 (UTC) by lane/976-part-7-the-base-update-grant.
#[derive(Debug, Clone, Copy)]
pub struct Request<'a> {
    /// Who started the update.
    pub starter: Starter,
    /// Whether the update writes the spare slot and nothing else.
    pub writes_spare_slot_only: bool,
    /// The identifier of the key that verified the manifest's signature, or `None` when the
    /// manifest is unsigned or no key verified it.
    pub signed_by: Option<&'a str>,
    /// The identifiers of the keys that live beside the trust root, the only keys that exist.
    pub trust_root_keys: &'a [&'a str],
    /// The version the manifest carries.
    pub candidate: Version,
    /// The version running now.
    pub running: Version,
    /// Whether a slot is on trial now.
    pub slot_on_trial: bool,
}

/// **Why a base update was refused**, naming the condition that failed. [`fmt::Display`] spells
/// it for a person.
///
/// Name: provisional, minted 2026-11-01 (UTC) by lane/976-part-7-the-base-update-grant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// The `[base-update]` section does not set `allow = true`.
    NotAllowed,
    /// An unattended update, and the section does not set `unattended = true`.
    NotUnattended,
    /// An unattended update that would write more than the spare slot.
    NotSpareSlotOnly,
    /// An unattended update whose manifest no key verified.
    Unsigned,
    /// The signing key is not in the owner's `accepts`, or not beside the trust root.
    KeyNotAccepted,
    /// The candidate version is not strictly greater than the running one.
    NotNewer,
    /// An unattended update while a slot is on trial.
    SlotOnTrial,
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Refusal::NotAllowed => "refused: the approval file does not set base-update allow",
            Refusal::NotUnattended => {
                "refused: the approval file does not set base-update unattended"
            }
            Refusal::NotSpareSlotOnly => "refused: an unattended update may write only the spare slot",
            Refusal::Unsigned => "refused: an unattended update needs a manifest a key signed",
            Refusal::KeyNotAccepted => {
                "refused: the signing key is not both accepted by the approval file and held beside the trust root"
            }
            Refusal::NotNewer => "refused: the version is not strictly greater than the running one",
            Refusal::SlotOnTrial => "refused: an unattended update may not start while a slot is on trial",
        })
    }
}

impl<'a> Grant<'a> {
    /// Whether the section sets `allow`.
    ///
    /// Name: provisional, minted 2026-11-01 (UTC) by lane/976-part-7-the-base-update-grant.
    pub fn allow(&self) -> bool {
        self.allow
    }

    /// Whether the section sets `unattended`.
    ///
    /// Name: provisional, minted 2026-11-01 (UTC) by lane/976-part-7-the-base-update-grant.
    pub fn unattended(&self) -> bool {
        self.unattended
    }

    /// The key identifiers the owner accepts, in the file's order.
    ///
    /// Name: provisional, minted 2026-11-01 (UTC) by lane/976-part-7-the-base-update-grant.
    pub fn accepts(&self) -> &[&'a str] {
        &self.accepts
    }

    /// **Whether `request` may be written**, or the first condition that failed. The order is the
    /// order a person would fix them in: the switches, the version, then the unattended bounds.
    /// The version rule binds every update, because the device accepts only a strictly greater
    /// one; the spare-slot, signature and trial rules bind an unattended one.
    ///
    /// Name: provisional, minted 2026-11-01 (UTC) by lane/976-part-7-the-base-update-grant.
    pub fn check(&self, request: &Request<'_>) -> Result<(), Refusal> {
        if !self.allow {
            return Err(Refusal::NotAllowed);
        }
        if request.candidate <= request.running {
            return Err(Refusal::NotNewer);
        }
        if request.starter == Starter::Person {
            return Ok(());
        }
        if !self.unattended {
            return Err(Refusal::NotUnattended);
        }
        if !request.writes_spare_slot_only {
            return Err(Refusal::NotSpareSlotOnly);
        }
        let Some(signer) = request.signed_by else {
            return Err(Refusal::Unsigned);
        };
        if !self.accepts.contains(&signer) || !request.trust_root_keys.contains(&signer) {
            return Err(Refusal::KeyNotAccepted);
        }
        if request.slot_on_trial {
            return Err(Refusal::SlotOnTrial);
        }
        Ok(())
    }
}
