//! **What the spawner tells the name resolver**: the control words it sends through the unbadged
//! capability, the reply's refusal reasons, and the one spelling a zone may take. No client sends
//! these, so they stay out of the file the standard library copies (`wire`).
//!
//! Name: provisional, minted 2026-11-16 (UTC) by milestone/1086-resolver-contract-outside-review-items.
//! Private: the items are re-exported at the crate root, so the module's name is not public.

use super::wire::{MAXIMUM_NAME_TEXT_BYTES, OPERATION_ATTACH_PAGE_FRAME, OPERATION_RESOLVE};
#[cfg(doc)]
use super::wire::{Outcome, status};

/// How many clients one resolver can hold grants for (see the crate's BUGS).
///
/// Name: provisional, decided by the maintainer on 2026-11-14 (UTC) at the crate's outside review:
/// a count names the things, the limit word first.
pub const MAXIMUM_GRANTS: usize = 8;

/// **The root zone as the wire spells it**: the empty text, which grants every name. A
/// configuration may spell it `.` for a reader (the installed-programs table does), and converts
/// before it grants; [`is_canonical_zone`] accepts this spelling and refuses `.`.
///
/// Name: provisional, minted 2026-11-16 (UTC) by milestone/1086-resolver-contract-outside-review-items.
pub const ROOT_ZONE: &str = "";

/// **Whether `zone` is spelled the one way a zone may be asked for or granted** (milestone 892 (a
/// std program at the prompt holds the network and a resolver)): empty (the root zone, any name),
/// or labels of one to 63 bytes of `a`-`z`, `0`-`9` and `-`, joined by single dots, with no dot at
/// either end, at most [`MAXIMUM_NAME_TEXT_BYTES`] bytes in all. Lowercase only, so `Example.net` and
/// `example.net` cannot be two spellings of one ask. A manifest note's zones
/// (`capability_manifest_note::zones`) and the installed-programs table's `grant` lines both hold
/// to it.
///
/// Name: provisional, minted 2026-10-19 (UTC) by milestone/892-installed-programs-hold-the-network-and-zones.
pub const fn is_canonical_zone(zone: &[u8]) -> bool {
    if zone.len() > MAXIMUM_NAME_TEXT_BYTES {
        return false;
    }
    let mut label = 0;
    let mut i = 0;
    while i < zone.len() {
        match zone[i] {
            b'.' => {
                if label == 0 {
                    return false;
                }
                label = 0;
            }
            b'a'..=b'z' | b'0'..=b'9' | b'-' => {
                label += 1;
                if label > 63 {
                    return false;
                }
            }
            _ => return false,
        }
        i += 1;
    }
    zone.is_empty() || label > 0
}

/// **The spawner grants a badge a zone.** A `SEND` or a `CALL` through the unbadged capability,
/// whose first word is a [`control_word`] and whose second carries up to eight bytes of the zone's
/// dotted text, little-endian. Offset 0 starts the zone afresh, so a re-grant replaces rather than
/// appends. The spawner `CALL`s the last piece of each grant, and the reply's first word says
/// whether the grant is in force ([`Outcome`], as described at [`refusal`]). Starting at 0x10 so
/// that no client opcode is ever also a control opcode.
///
/// Name: provisional. Its origin is not recorded.
pub const OPERATION_GRANT: u64 = 0x10;
/// **The spawner withdraws a badge's grant.** A plain `SEND` or a `CALL` through the unbadged
/// capability, whose first word is a [`control_word`] naming the badge, with offset and total 0;
/// the second word is unused. A `CALL` is answered as a grant's last piece is ([`refusal`]). From
/// then on the badge is denied every name, the resolver gives up the page that badge attached, and
/// the window is free for the next grant. A spawner that outlives its clients sends one when each
/// client ends, which is what lets a resolver serve more than [`MAXIMUM_GRANTS`] clients in its
/// life (milestone 892 (a std program at the prompt holds the
/// network and a resolver); provisional).
///
/// Name: provisional, milestone 892's lane (#2011), 2026-10-12 (UTC).
/// `system_log_contract` has its own `OPERATION_FORGET`; the pull request records no other name
/// considered.
pub const OPERATION_FORGET: u64 = 0x11;

// A control opcode is never also a client's, so the resolver cannot read one as the other.
const _: () =
    assert!(OPERATION_ATTACH_PAGE_FRAME < OPERATION_GRANT && OPERATION_RESOLVE < OPERATION_GRANT);

/// A control word: `operation << 56 | offset << 48 | total << 40 | badge`, the badge in the low 32
/// bits. `offset` and `total` are bytes of the zone's text.
///
/// Name: provisional. Its origin is not recorded.
pub const fn control_word(operation: u64, offset: u8, total: u8, badge: u32) -> u64 {
    (operation << 56) | ((offset as u64) << 48) | ((total as u64) << 40) | badge as u64
}

/// A control word's `(operation, offset, total, badge)`.
///
/// Name: provisional. Its origin is not recorded.
pub const fn control_fields(first_word: u64) -> (u64, u8, u8, u32) {
    (
        first_word >> 56,
        (first_word >> 48) as u8,
        (first_word >> 40) as u8,
        (first_word & 0xffff_ffff) as u32,
    )
}

/// **Why a control message was refused**: the detail byte of the reply to a control `CALL` whose
/// status is [`status::BAD_REQUEST`]. The reply to a control `CALL` is an [`Outcome`] word, packed
/// exactly as a resolve reply's first word, with a count of 0 and a second word of 0. Its status is
/// [`status::OK`] when the message was accepted (a piece taken, a grant in force, a grant
/// withdrawn), or [`status::BAD_REQUEST`] with one of these in the detail byte when it was refused.
/// No other status answers a control message. The numbers are fixed: a new reason takes the next
/// one, and none is reused.
///
/// Name: provisional.
pub mod refusal {
    /// The first word names an operation the spawner does not have.
    ///
    /// Name: provisional.
    pub const UNKNOWN_OPERATION: u8 = 1;
    /// The message names badge 0, the spawner's own, which holds no grant.
    ///
    /// Name: provisional.
    pub const BADGE_ZERO: u8 = 2;
    /// A piece arrived out of order, or a continuation for a grant nobody started.
    ///
    /// Name: provisional.
    pub const OUT_OF_ORDER: u8 = 3;
    /// The zone is longer than a name can be.
    ///
    /// Name: provisional.
    pub const TOO_LONG: u8 = 4;
    /// The zone's text is not a host name.
    ///
    /// Name: provisional.
    pub const BAD_ZONE: u8 = 5;
    /// Every window already belongs to another badge.
    ///
    /// Name: provisional.
    pub const FULL: u8 = 6;
    /// A forget for a badge that holds no window.
    ///
    /// Name: provisional.
    pub const NO_SUCH_GRANT: u8 = 7;
}

#[cfg(test)]
mod tests {
    use super::super::wire::operation;
    use super::*;

    #[test]
    fn a_control_word_comes_back_field_for_field() {
        let w = control_word(OPERATION_GRANT, 16, 253, 0x8010);
        assert_eq!(control_fields(w), (OPERATION_GRANT, 16, 253, 0x8010));
        assert_eq!(operation(w), OPERATION_GRANT);
        assert_eq!(
            operation(control_word(OPERATION_FORGET, 0, 0, 1)),
            OPERATION_FORGET
        );
    }
}
