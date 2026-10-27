//! **The messages a holder of the resolver sends**: granting a zone, forgetting a badge, and the
//! small number each rejection travels as.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

/// The [`OPERATION_GRANT`] messages that grant `badge` the zone `zone` (dotted text, a trailing dot
/// allowed; empty for the root, which is every name). `None` when the badge is 0, which is the
/// spawner's own, or the text is longer than [`NAME_TEXT_MAX`] plus a trailing dot.
pub fn grant_messages(badge: u32, zone: &[u8]) -> Option<impl Iterator<Item = (u64, u64)> + '_> {
    if badge == 0 || zone.len() > NAME_TEXT_MAX + 1 {
        return None;
    }
    let total = zone.len() as u8;
    // `max(1)` so the root (no bytes) is still one message: a zero-length grant is a grant.
    let messages = zone.len().div_ceil(8).max(1);
    Some((0..messages).map(move |i| {
        let start = i * 8;
        let part = zone.get(start..).unwrap_or(&[]);
        let n = part.len().min(8);
        let mut bytes = [0u8; 8];
        bytes[..n].copy_from_slice(&part[..n]);
        (
            control_word(OPERATION_GRANT, start as u8, total, badge),
            u64::from_le_bytes(bytes),
        )
    }))
}

/// The [`OPERATION_FORGET`] message that withdraws `badge`'s grant: the control word, and an unused
/// second word. `None` for badge 0, the spawner's own, which holds no grant to withdraw.
///
/// Name: provisional, milestone 892's lane (#2011), 2026-10-12 (UTC).
/// The pull request records no other name considered.
pub const fn forget_message(badge: u32) -> Option<(u64, u64)> {
    if badge == 0 {
        return None;
    }
    Some((control_word(OPERATION_FORGET, 0, 0, badge), 0))
}

/// A stable small number for each `domain_name_system::Reject`, the detail of a
/// [`status::REFUSED`]: which check refused the reply, so that a forged answer and a broken server
/// are not one word.
pub const fn reject_code(reject: domain_name_system::Reject) -> u8 {
    use domain_name_system::Reject;
    match reject {
        Reject::Malformed(_) => 1,
        Reject::NotAResponse => 2,
        Reject::IdMismatch => 3,
        Reject::Truncated => 4,
        Reject::QuestionMismatch => 5,
        Reject::NoSuchName => 6,
        Reject::ServerError(_) => 7,
        Reject::ChainTooLong => 8,
        Reject::NoAddress => 9,
    }
}
