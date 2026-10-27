//! The crate's host tests, moved out of `lib.rs` on 2026-10-12 (UTC) by lane
//! maintainer/mutation-triage-2026w42, so the crate root stays an index under §282 (when a file is
//! split, the parent becomes an index) as its tests grow.

extern crate std;
use std::vec::Vec;

use domain_name_system::Name;

use super::*;

fn grant(g: &mut Grants, badge: u32, zone: &str) -> Control {
    // The first refusal, or the last message's outcome: once a piece is refused, the ones after
    // it are continuations of nothing, and their own refusal would hide the reason.
    let mut last = Control::Partial;
    for (w0, w1) in grant_messages(badge, zone.as_bytes()).unwrap() {
        last = g.control(w0, w1);
        if matches!(last, Control::Refused(_)) {
            break;
        }
    }
    last
}

fn name(text: &str) -> Name {
    Name::from_dotted(text).unwrap()
}

#[test]
fn a_zone_grant_answers_for_the_zone_and_everything_under_it_and_nothing_beside_it() {
    let mut g = Grants::new();
    assert_eq!(grant(&mut g, 5, "nife.test"), Control::Granted(0));
    assert_eq!(g.check(5, &name("nife.test")), Ok(0));
    assert_eq!(g.check(5, &name("packages.nife.test")), Ok(0));
    assert_eq!(g.check(5, &name("a.b.c.NIFE.Test")), Ok(0));
    assert_eq!(g.check(5, &name("evilnife.test")), Err(Denied::OutsideZone));
    assert_eq!(
        g.check(5, &name("nife.test.evil")),
        Err(Denied::OutsideZone)
    );
    assert_eq!(g.check(5, &name("test")), Err(Denied::OutsideZone));
    assert_eq!(g.check(5, &name("example.com")), Err(Denied::OutsideZone));
}

#[test]
fn a_badge_with_no_grant_and_the_spawners_own_badge_are_denied() {
    let mut g = Grants::new();
    grant(&mut g, 5, "");
    assert_eq!(g.check(6, &name("anything.test")), Err(Denied::NoGrant));
    assert_eq!(g.check(0, &name("anything.test")), Err(Denied::NoGrant));
    assert!(grant_messages(0, b"nife.test").is_none());
    assert_eq!(
        g.control(control_word(OPERATION_GRANT, 0, 0, 0), 0),
        Control::Refused(Refusal::BadgeZero)
    );
}

#[test]
fn the_root_zone_is_every_name_and_one_host_name_is_exactly_itself() {
    let mut g = Grants::new();
    assert_eq!(grant(&mut g, 1, ""), Control::Granted(0));
    assert_eq!(grant(&mut g, 2, "packages.nife.test."), Control::Granted(1));
    assert_eq!(g.check(1, &name("example.com")), Ok(0));
    assert_eq!(g.check(2, &name("packages.nife.test")), Ok(1));
    assert_eq!(
        g.check(2, &name("mirror.nife.test")),
        Err(Denied::OutsideZone)
    );
}

#[test]
fn a_longest_zone_crosses_in_thirty_two_messages_and_still_parses() {
    // Four 63-byte labels and their dots: 255 characters, past what a name can be.
    let label = "a".repeat(63);
    let too_long = [label.as_str(); 4].join(".");
    assert!(grant_messages(3, too_long.as_bytes()).is_none());
    // 61 + 63 * 3 + 3 dots = 253, the longest dotted name there is.
    let longest = std::format!("{}.{label}.{label}.{label}", "b".repeat(61));
    assert_eq!(longest.len(), NAME_TEXT_MAX);
    assert_eq!(
        grant_messages(3, longest.as_bytes()).unwrap().count(),
        NAME_TEXT_MAX.div_ceil(8)
    );
    let mut g = Grants::new();
    assert_eq!(grant(&mut g, 3, &longest), Control::Granted(0));
    assert_eq!(g.check(3, &name(&longest)), Ok(0));
}

#[test]
fn a_regrant_replaces_the_zone_in_the_same_window_and_is_not_in_force_until_whole() {
    let mut g = Grants::new();
    grant(&mut g, 9, "nife.test");
    let messages: Vec<_> = grant_messages(9, b"example.org").unwrap().collect();
    assert_eq!(messages.len(), 2);
    assert_eq!(g.control(messages[0].0, messages[0].1), Control::Partial);
    // Half way through, neither the old zone nor a prefix of the new one is in force.
    assert_eq!(g.check(9, &name("nife.test")), Err(Denied::NoGrant));
    assert_eq!(g.granted(9), None);
    assert_eq!(g.control(messages[1].0, messages[1].1), Control::Granted(0));
    assert_eq!(g.check(9, &name("www.example.org")), Ok(0));
    assert_eq!(g.check(9, &name("nife.test")), Err(Denied::OutsideZone));
}

#[test]
fn pieces_out_of_order_or_unstarted_refuse_and_leave_no_grant() {
    let mut g = Grants::new();
    let messages: Vec<_> = grant_messages(4, b"packages.nife.test").unwrap().collect();
    // A continuation for a grant nobody started.
    assert_eq!(
        g.control(messages[1].0, messages[1].1),
        Control::Refused(Refusal::OutOfOrder)
    );
    g.control(messages[0].0, messages[0].1);
    // The third piece before the second.
    assert_eq!(
        g.control(messages[2].0, messages[2].1),
        Control::Refused(Refusal::OutOfOrder)
    );
    assert_eq!(
        g.check(4, &name("packages.nife.test")),
        Err(Denied::NoGrant)
    );
}

#[test]
fn a_zone_that_is_not_a_name_is_refused_and_so_is_an_unknown_operation() {
    let mut g = Grants::new();
    assert_eq!(
        grant(&mut g, 2, "nife..test"),
        Control::Refused(Refusal::BadZone)
    );
    assert_eq!(
        grant(&mut g, 2, "nife test"),
        Control::Refused(Refusal::BadZone)
    );
    assert_eq!(g.granted(2), None);
    assert_eq!(
        g.control(control_word(0x7f, 0, 0, 2), 0),
        Control::Refused(Refusal::UnknownOperation)
    );
}

#[test]
fn the_ninth_badge_finds_every_window_taken() {
    let mut g = Grants::new();
    for badge in 1..=GRANTS_MAX as u32 {
        assert_eq!(
            grant(&mut g, badge, "nife.test"),
            Control::Granted(badge as usize - 1)
        );
    }
    assert_eq!(
        grant(&mut g, 99, "nife.test"),
        Control::Refused(Refusal::Full)
    );
    assert_eq!(g.check(99, &name("nife.test")), Err(Denied::NoGrant));
}

/// A continuation from a badge with no grant is out of order even when every window is
/// taken: only a first piece may claim a window, so only a first piece can find none free.
#[test]
fn a_continuation_into_a_full_table_is_out_of_order_not_full() {
    let mut g = Grants::new();
    for badge in 1..=GRANTS_MAX as u32 {
        grant(&mut g, badge, "nife.test");
    }
    let (w0, w1) = grant_messages(99, b"packages.nife.test")
        .unwrap()
        .nth(1)
        .unwrap();
    assert_eq!(g.control(w0, w1), Control::Refused(Refusal::OutOfOrder));
}

/// **The longest name with its trailing dot is 254 bytes, and is granted**; a total past that
/// is refused before any of it is read.
#[test]
fn the_longest_zone_with_a_trailing_dot_is_granted_and_one_byte_more_is_not() {
    let label = "a".repeat(63);
    let dotted = std::format!("{}.{label}.{label}.{label}.", "b".repeat(61));
    assert_eq!(dotted.len(), NAME_TEXT_MAX + 1);
    let mut g = Grants::new();
    assert_eq!(grant(&mut g, 3, &dotted), Control::Granted(0));
    let total = (NAME_TEXT_MAX + 2) as u8;
    assert_eq!(
        g.control(control_word(OPERATION_GRANT, 0, total, 4), 0),
        Control::Refused(Refusal::TooLong)
    );
}

/// Milestone 892 (a std program at the prompt holds the network and a resolver): a spawner that
/// outlives its clients forgets each one, and the window comes back, so the ninth client of a
/// resolver whose first eight have ended is not refused.
#[test]
fn a_forgotten_badge_is_denied_and_its_window_serves_the_next_grant() {
    let mut g = Grants::new();
    for badge in 1..=GRANTS_MAX as u32 {
        grant(&mut g, badge, "nife.test");
    }
    let (w0, w1) = forget_message(3).unwrap();
    assert_eq!(g.control(w0, w1), Control::Forgotten(2));
    assert_eq!(g.check(3, &name("nife.test")), Err(Denied::NoGrant));
    assert_eq!(g.granted(3), None);
    // The others are untouched.
    assert_eq!(g.check(4, &name("packages.nife.test")), Ok(3));
    // A fresh badge takes the freed window, not a new one.
    assert_eq!(grant(&mut g, 99, ""), Control::Granted(2));
    assert_eq!(g.check(99, &name("example.com")), Ok(2));
    // The forgotten badge stays forgotten: forgetting is not a re-grant of anything.
    assert_eq!(g.check(3, &name("example.com")), Err(Denied::NoGrant));
}

#[test]
fn forgetting_a_badge_with_no_window_or_badge_zero_is_refused() {
    let mut g = Grants::new();
    let (w0, w1) = forget_message(5).unwrap();
    assert_eq!(g.control(w0, w1), Control::Refused(Refusal::NoSuchGrant));
    assert!(forget_message(0).is_none());
    assert_eq!(
        g.control(control_word(OPERATION_FORGET, 0, 0, 0), 0),
        Control::Refused(Refusal::BadgeZero)
    );
    // A half-sent grant holds a window, and a forget frees it too.
    let first = grant_messages(6, b"packages.nife.test")
        .unwrap()
        .next()
        .unwrap();
    assert_eq!(g.control(first.0, first.1), Control::Partial);
    assert_eq!(
        g.control(forget_message(6).unwrap().0, 0),
        Control::Forgotten(0)
    );
    assert_eq!(grant(&mut g, 7, "nife.test"), Control::Granted(0));
}

#[test]
fn an_outcome_round_trips_and_a_kernel_error_is_not_one() {
    let o = Outcome {
        status: status::REFUSED,
        detail: reject_code(domain_name_system::Reject::IdMismatch),
        count: 0,
    };
    assert_eq!(Outcome::from_word(o.word()), Some(o));
    let ok = Outcome {
        status: status::OK,
        detail: 0,
        count: 8,
    };
    assert_eq!(Outcome::from_word(ok.word()), Some(ok));
    // A `CALL` on an empty slot returns a small negative error, read as a huge u64.
    assert_eq!(Outcome::from_word(-3i64 as u64), None);
}

#[test]
fn the_endowment_words_round_trip() {
    let w = endowment::server([10, 0, 2, 9]);
    assert_eq!(endowment::server_octets(w), [10, 0, 2, 9]);
    let w = endowment::port_and_transport(53, endowment::TRANSPORT_TCP);
    assert_eq!(
        endowment::port_and_transport_fields(w),
        (53, endowment::TRANSPORT_TCP)
    );
}

#[test]
fn every_reject_has_its_own_code() {
    use domain_name_system::{Error, Reject};
    let all = [
        Reject::Malformed(Error::Truncated),
        Reject::NotAResponse,
        Reject::IdMismatch,
        Reject::Truncated,
        Reject::QuestionMismatch,
        Reject::NoSuchName,
        Reject::ServerError(2),
        Reject::ChainTooLong,
        Reject::NoAddress,
    ];
    let mut codes: Vec<u8> = all.iter().map(|&r| reject_code(r)).collect();
    codes.sort_unstable();
    codes.dedup();
    assert_eq!(codes.len(), all.len());
    assert!(!codes.contains(&0));
}

/// **A zone has one spelling** (milestone 892 (a std program at the prompt holds the network and a
/// resolver)): each boundary of `is_canonical_zone` on both sides. The root zone is empty; a label
/// is 1 to 63 bytes of lowercase letters, digits and hyphens; dots join labels and never end or
/// double; the whole is at most 253 bytes.
#[test]
fn a_zone_is_spelled_one_way() {
    let label = |n: usize| "a".repeat(n);
    let accepted: Vec<std::string::String> = std::vec![
        std::string::String::new(),
        "a".into(),
        "nife.test".into(),
        "x-1.example-2.net".into(),
        label(63),
        std::format!("{}.{}", label(63), label(63)),
        // 253 bytes exactly: three labels of 63, one of 61, and three dots.
        std::format!("{}.{}.{}.{}", label(63), label(63), label(63), label(61)),
    ];
    for zone in &accepted {
        assert!(is_canonical_zone(zone.as_bytes()), "{zone:?} is canonical");
    }
    assert_eq!(accepted[6].len(), NAME_TEXT_MAX);
    let refused: Vec<std::string::String> = std::vec![
        label(64),
        std::format!("{}.a", label(64)),
        std::format!("{}a", accepted[6]),
        ".".into(),
        ".nife.test".into(),
        "nife.test.".into(),
        "nife..test".into(),
        "Nife.test".into(),
        "nife_test".into(),
        "nife test".into(),
        "nife.tést".into(),
    ];
    for zone in &refused {
        assert!(
            !is_canonical_zone(zone.as_bytes()),
            "{zone:?} is not canonical"
        );
    }
}
