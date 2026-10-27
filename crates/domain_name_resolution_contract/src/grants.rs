//! **The grant table**: which badge may resolve which zone, and in which window its page lives.
//! Moved out of `lib.rs` unchanged by milestone 892 (a std program at the prompt holds the network
//! and a resolver), whose forget verb grew the crate root, so the root stays an index (§282 (when a
//! file is split, the parent becomes an index)).
//!
//! Name: provisional, milestone 892's lane (#2011), 2026-10-12 (UTC), for the module and the file.
//! The pull request records no other name considered.

use domain_name_system::Name;

use crate::{GRANTS_MAX, NAME_TEXT_MAX, OPERATION_FORGET, OPERATION_GRANT, control_fields};

/// Why [`Grants::check`] refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Denied {
    /// No grant was ever completed for this badge.
    NoGrant,
    /// The badge has a grant, and the name is not within its zone.
    OutsideZone,
}

/// What a control message did, for the resolver to ignore and a test to read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Control {
    /// A piece of a grant was taken; more are due.
    Partial,
    /// The last piece arrived and the zone parsed: the badge's grant is in force in this window.
    Granted(usize),
    /// Refused, and the badge's grant (if this was a re-grant) is gone rather than half-written.
    Refused(Refusal),
    /// The badge's grant is withdrawn and this window is free: the resolver gives up whatever page
    /// is mapped there before another badge can be granted it.
    Forgotten(usize),
}

/// Why a control message was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// Not an operation the spawner has.
    UnknownOperation,
    /// Badge 0 is the spawner's own capability, so it cannot be granted anything.
    BadgeZero,
    /// A piece arrived out of order, or a continuation for a grant nobody started.
    OutOfOrder,
    /// The zone is longer than a name can be.
    TooLong,
    /// The zone's text is not a name `Name::from_dotted` accepts.
    BadZone,
    /// Every window already belongs to another badge.
    Full,
    /// A forget for a badge that holds no window.
    NoSuchGrant,
}

#[derive(Clone, Copy)]
struct Entry {
    /// 0 for a free entry: no client's badge is 0.
    badge: u32,
    /// In force only when `complete`.
    zone: Name,
    complete: bool,
    /// The zone's text so far, while the spawner's messages arrive.
    text: [u8; NAME_TEXT_MAX + 1],
    text_len: u8,
    text_total: u8,
}

impl Entry {
    const FREE: Entry = Entry {
        badge: 0,
        zone: Name::root(),
        complete: false,
        text: [0; NAME_TEXT_MAX + 1],
        text_len: 0,
        text_total: 0,
    };
}

/// **Which badge may resolve which zone**, and which window its page lives in.
///
/// An entry's index is its client's window, fixed for the resolver's life (see the crate's BUGS),
/// so the resolver maps a client's page at the same place every time and a re-grant does not move
/// it.
///
/// # Examples
///
/// A spawner grants badge 7 the zone `nife.test`:
///
/// ```
/// use domain_name_system::Name;
/// use domain_name_resolution_contract::{Denied, Grants, grant_messages};
///
/// let mut grants = Grants::new();
/// for (w0, w1) in grant_messages(7, b"nife.test").unwrap() {
///     grants.control(w0, w1);
/// }
/// let asked = Name::from_dotted("packages.nife.test").unwrap();
/// assert!(grants.check(7, &asked).is_ok());
/// let elsewhere = Name::from_dotted("evilnife.test").unwrap();
/// assert_eq!(grants.check(7, &elsewhere), Err(Denied::OutsideZone));
/// assert_eq!(grants.check(8, &asked), Err(Denied::NoGrant));
/// ```
pub struct Grants {
    entries: [Entry; GRANTS_MAX],
}

impl Default for Grants {
    fn default() -> Self {
        Self::new()
    }
}

impl Grants {
    /// No grants: every badge is denied.
    pub const fn new() -> Grants {
        Grants {
            entries: [Entry::FREE; GRANTS_MAX],
        }
    }

    /// **Take one control message from the spawner.** The caller has already established that it
    /// arrived with badge 0; this reads only the words.
    pub fn control(&mut self, w0: u64, w1: u64) -> Control {
        let (op, offset, total, badge) = control_fields(w0);
        if op == OPERATION_FORGET {
            return self.forget(badge);
        }
        if op != OPERATION_GRANT {
            return Control::Refused(Refusal::UnknownOperation);
        }
        if badge == 0 {
            return Control::Refused(Refusal::BadgeZero);
        }
        if total as usize > NAME_TEXT_MAX + 1 {
            return self.refuse(badge, Refusal::TooLong);
        }
        let at = match self.position(badge) {
            Some(at) => at,
            None if offset == 0 => match self.entries.iter().position(|e| e.badge == 0) {
                Some(free) => free,
                None => return Control::Refused(Refusal::Full),
            },
            None => return Control::Refused(Refusal::OutOfOrder),
        };
        let e = &mut self.entries[at];
        if offset == 0 {
            // A fresh grant, or a re-grant: the old zone stops applying now, not when the new one
            // completes, so there is never a moment where a half-sent zone is in force.
            e.badge = badge;
            e.complete = false;
            e.text_len = 0;
            e.text_total = total;
        } else if e.complete || offset != e.text_len || total != e.text_total {
            return self.refuse(badge, Refusal::OutOfOrder);
        }
        let bytes = w1.to_le_bytes();
        let n = (e.text_total - e.text_len).min(8) as usize;
        let start = e.text_len as usize;
        e.text[start..start + n].copy_from_slice(&bytes[..n]);
        e.text_len += n as u8;
        if e.text_len < e.text_total {
            return Control::Partial;
        }
        let zone = core::str::from_utf8(&e.text[..e.text_len as usize])
            .ok()
            .and_then(|text| Name::from_dotted(text).ok());
        match zone {
            Some(zone) => {
                e.zone = zone;
                e.complete = true;
                Control::Granted(at)
            }
            None => self.refuse(badge, Refusal::BadZone),
        }
    }

    /// **May `badge` ask about `name`?** The window its page is in, or why not.
    pub fn check(&self, badge: u32, name: &Name) -> Result<usize, Denied> {
        let at = self.granted(badge).ok_or(Denied::NoGrant)?;
        if name.is_within(&self.entries[at].zone) {
            Ok(at)
        } else {
            Err(Denied::OutsideZone)
        }
    }

    /// The window of `badge`'s grant, if one is in force. Where its page goes when it attaches one.
    pub fn granted(&self, badge: u32) -> Option<usize> {
        self.position(badge).filter(|&at| self.entries[at].complete)
    }

    fn position(&self, badge: u32) -> Option<usize> {
        if badge == 0 {
            return None;
        }
        self.entries.iter().position(|e| e.badge == badge)
    }

    /// Withdraw `badge`'s grant, complete or half-sent, and free its window.
    fn forget(&mut self, badge: u32) -> Control {
        if badge == 0 {
            return Control::Refused(Refusal::BadgeZero);
        }
        match self.position(badge) {
            Some(at) => {
                self.entries[at] = Entry::FREE;
                Control::Forgotten(at)
            }
            None => Control::Refused(Refusal::NoSuchGrant),
        }
    }

    /// A refused grant leaves nothing behind: the badge keeps its window (it is still the badge's,
    /// see the BUGS), and has no zone.
    fn refuse(&mut self, badge: u32, why: Refusal) -> Control {
        if let Some(at) = self.position(badge) {
            let e = &mut self.entries[at];
            e.complete = false;
            e.text_len = 0;
            e.text_total = 0;
        }
        Control::Refused(why)
    }
}
