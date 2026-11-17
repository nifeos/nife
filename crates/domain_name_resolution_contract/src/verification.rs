//! **Machine-checked proofs (Kani) of the grant table**: from an empty table, any short run of
//! control messages never panics, a badge's grant is in force only once a message said `Granted`,
//! and its window never moves while it holds one. Behind `#[cfg(kani)]`, so only `cargo kani`
//! compiles them.
//!
//! Name: provisional, after the module every other crate's harnesses live in.

use domain_name_system::Name;

use super::*;

/// How many control messages a run holds. Two reach a fresh grant, a re-grant over it, a piece out
/// of order after a first piece, and a forget after a grant, and each zone piece is eight bytes, so
/// a zone of up to 32 bytes can complete. A third message made the proof outrun its twenty-minute
/// shard, so the run stays at two.
const STEPS: usize = 2;

/// **Any run of up to two control messages, from an empty table, never panics, and the table
/// grants only what it said it granted.** Each message is two free words, so its operation, offset,
/// total and badge are all free. After each one:
///
/// - `Granted(at)` leaves the message's badge in force in window `at`.
/// - `Partial` and `Forgotten` leave it in force nowhere: a re-grant withdraws the old zone at its
///   first piece, not its last.
/// - `Refused` leaves it as it was, or withdrawn.
/// - A badge that was in force and still is keeps its window.
/// - Every other badge the run has named is as it was.
///
/// Together, from a table where nothing is in force, these mean `check` answers `Ok` only for a
/// badge whose most recent change was a `Granted`. The arithmetic the grant path leans on (the
/// `u8` subtraction of the bytes received from the total, and the slice of the text buffer) is
/// inside the run, so a wrap or an index past the buffer fails the proof.
/// Falsification: replayable `crates/domain_name_resolution_contract/falsifications/verification.the_grant_table_grants_only_what_it_said.patch`
#[kani::proof]
#[kani::unwind(34)]
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
        if let Ok(at) = grants.check(badge, &Name::root()) {
            assert_eq!(Some(at), after);
        }
        for earlier in 0..step {
            if badges[earlier] != badge {
                assert_eq!(grants.granted(badges[earlier]), others_before[earlier]);
            }
        }
        badges[step] = badge;
    }
}
