//! **Machine-checked proofs (Kani) of the grant table**: from an empty table, any short run of
//! control messages never panics, a badge's grant is in force only once a message said `Granted`,
//! and its window never moves while it holds one. Behind `#[cfg(kani)]`, so only `cargo kani`
//! compiles them.
//!
//! Name: provisional, after the module every other crate's harnesses live in.

use domain_name_system::Name;

use super::*;

/// How many control messages the bookkeeping run holds. Two reach a fresh grant, a re-grant over
/// it, a piece out of order after a first piece, and a forget after a grant. A third message, and
/// the parse of a zone inside the run, each made the proof outrun its twenty-minute shard (the
/// parse of up to 32 free bytes, over a table of 256-byte names, passed the memory fence), so the
/// run stays at two and the parse is stubbed here and proved in its own harness below.
const STEPS: usize = 2;

/// The stand-in for `core::str::from_utf8` in the bookkeeping proof: the text is accepted whole.
/// The grant table's bookkeeping (which badge holds which window, the byte counts) does not depend
/// on the zone's bytes, only on whether the parse said yes or no, which the stand-in for
/// `Name::from_dotted` leaves free.
#[allow(dead_code)] // named only by `#[kani::stub]`, which the lint pass's shim ignores
fn text_without_validation(_bytes: &[u8]) -> Result<&str, core::str::Utf8Error> {
    Ok("")
}

/// The stand-in for `Name::from_dotted` in the bookkeeping proof: either a zone or a refusal, free.
/// The real parse is `the_one_message_zone_parses_or_is_refused` (and the name crate's own tests).
#[allow(dead_code)] // as above
fn zone_without_parsing(_text: &str) -> Result<Name, domain_name_system::Error> {
    if kani::any() {
        Ok(Name::root())
    } else {
        Err(domain_name_system::Error::BadCharacter)
    }
}

/// **Any run of up to two control messages, from an empty table, never panics, and the table
/// grants only what it said it granted, whatever the zone text.** Each message is two free words, so its operation, offset,
/// total and badge are all free. After each one:
///
/// - `Granted(at)` leaves the message's badge in force in window `at`.
/// - `Partial` and `Forgotten` leave it in force nowhere: a re-grant withdraws the old zone at its
///   first piece, not its last.
/// - `Refused` leaves it as it was, or withdrawn.
/// - A badge that was in force and still is keeps its window.
/// - Every other badge the run has named is as it was.
///
/// Together, from a table where nothing is in force, a badge is in force only when its most recent
/// change was a `Granted`. The arithmetic the grant path leans on (the
/// `u8` subtraction of the bytes received from the total, and the slice of the text buffer) is
/// inside the run, so a wrap or an index past the buffer fails the proof.
/// Falsification: replayable `crates/domain_name_resolution_contract/falsifications/verification.the_grant_table_grants_only_what_it_said.patch`
#[kani::proof]
#[kani::unwind(34)]
#[kani::stub(core::str::from_utf8, text_without_validation)]
#[kani::stub(domain_name_system::Name::from_dotted, zone_without_parsing)]
fn the_grant_table_grants_only_what_it_said() {
    let mut grants = Grants::new();
    let mut badges = [0u32; STEPS];
    for step in 0..STEPS {
        let first_word: u64 = kani::any();
        let second_word: u64 = kani::any();
        let (_, _, _, badge) = control_fields(first_word);
        let before = grants.granted(badge);
        let mut others_before = [None; STEPS];
        for earlier in 0..step {
            others_before[earlier] = grants.granted(badges[earlier]);
        }

        let result = grants.control(first_word, second_word);

        let after = grants.granted(badge);
        match result {
            Control::Granted(at) => {
                assert!(at < MAXIMUM_GRANTS);
                assert_eq!(after, Some(at));
            }
            Control::Partial | Control::Forgotten(_) => assert_eq!(after, None),
            Control::Refused(_) => assert!(after.is_none() || after == before),
        }
        if let (Some(was), Some(is)) = (before, after) {
            assert_eq!(was, is, "a badge's window moved while it held a grant");
        }
        for earlier in 0..step {
            if badges[earlier] != badge {
                assert_eq!(grants.granted(badges[earlier]), others_before[earlier]);
            }
        }
        badges[step] = badge;
    }
}

/// **One control message, whose zone text is free, is a grant or a refusal, and `check` answers `Ok`
/// only for a badge that was granted.** This is the real parse (`core::str::from_utf8` and
/// `Name::from_dotted`, not stubbed) of up to eight free bytes, the most one message carries; the
/// bookkeeping proof above covers runs of messages with the parse left free. After the message:
///
/// - `Granted(at)` leaves the badge in force in window `at`, and `check` against the root answers
///   `Ok(at)` or `OutsideZone`, never `NoGrant`.
/// - Every other answer leaves the badge in force nowhere, and `check` answers `NoGrant`.
/// Falsification: replayable `crates/domain_name_resolution_contract/falsifications/verification.the_one_message_zone_parses_or_is_refused.patch`
#[kani::proof]
#[kani::unwind(10)]
fn the_one_message_zone_parses_or_is_refused() {
    let first_word: u64 = kani::any();
    let second_word: u64 = kani::any();
    let (_, _, total, badge) = control_fields(first_word);
    kani::assume(total <= 8);
    let mut grants = Grants::new();

    let result = grants.control(first_word, second_word);

    let after = grants.granted(badge);
    match result {
        Control::Granted(at) => {
            assert!(at < MAXIMUM_GRANTS);
            assert_eq!(after, Some(at));
            assert!(grants.check(badge, &Name::root()) != Err(Denied::NoGrant));
        }
        _ => {
            assert_eq!(after, None);
            assert_eq!(grants.check(badge, &Name::root()), Err(Denied::NoGrant));
        }
    }
}
