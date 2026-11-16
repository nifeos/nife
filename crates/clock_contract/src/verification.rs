//! **Machine-checked proofs (Kani) of the proposal policy and the wall-clock arithmetic**, run with
//! `cargo kani`. Every input is a symbolic `u64`, so each harness covers every value inside its
//! stated bound, not a sample.
//!
//! Name: provisional, after the `verification` module every other Kani crate here keeps out of
//! line.

use super::*;

/// `policy::decide` accepts a proposal only when the proposed time is plausible, and when the
/// clock already knows the time, only when the step lies inside the bound on its side. It also
/// accepts every proposal that meets both, so a refusal is never a surprise. This is the function
/// a compromised proposer can aim at.
#[kani::proof]
fn a_proposal_is_accepted_only_inside_the_policys_bounds() {
    let current_state: u64 = kani::any();
    let current: u64 = kani::any();
    let proposed: u64 = kani::any();

    let verdict = policy::decide(current_state, current, proposed);

    if verdict == status::ACCEPTED {
        assert!(policy::is_plausible(proposed));
        if state::is_known(current_state) {
            if proposed > current {
                assert!(proposed - current <= policy::MAXIMUM_STEP_FORWARD_NANOSECONDS);
            } else {
                assert!(current - proposed <= policy::MAXIMUM_STEP_BACKWARD_NANOSECONDS);
            }
        }
    } else {
        assert!(
            verdict == status::REFUSED_IMPLAUSIBLE
                || verdict == status::REFUSED_TOO_FAR_FORWARD
                || verdict == status::REFUSED_TOO_FAR_BACKWARD
        );
    }

    // The converse for a clock that does not know the time: a plausible proposal is accepted.
    if !state::is_known(current_state) && policy::is_plausible(proposed) {
        assert!(verdict == status::ACCEPTED);
    }
}

/// The wire format and the arithmetic the page's readers and writers agree on: the opcode that
/// `propose::req` puts in the top byte comes back out of `propose::operation`, and the offset
/// `offset_for` computes reads back as the wall time through `wall_nanoseconds`.
#[kani::proof]
fn the_request_word_and_the_offset_round_trip() {
    let opcode: u64 = kani::any();
    kani::assume(opcode < 256);
    assert!(propose::operation(propose::req(opcode)) == opcode);

    let wall: u64 = kani::any();
    let monotonic: u64 = kani::any();
    kani::assume(wall >= monotonic);
    assert!(wall_nanoseconds(offset_for(wall, monotonic), monotonic) == wall);
}
