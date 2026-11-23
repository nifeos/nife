//! Kani proofs of the supervision decisions in `region_reaping`: a reap is authorized by the
//! supervision relationship alone, a refusal about a stranger discloses nothing, and the survey and
//! the reap are scoped by one predicate. Each holds for every pair of rendezvous names and every
//! liveness. nifeos/record:notes/kernel-proofs.md says what a proof over the kernel crate can and cannot say.

use super::*;

/// **A reap is authorized by the supervision relationship, and by nothing else**. For
/// every pair of rendezvous names and every liveness, `Permitted` implies the thread's recorded
/// fault rendezvous *is* the rendezvous being invoked and the thread is dead, so no
/// unsupervised thread, other supervisor's child or stale name is reapable. The proof pins the
/// gate; `reap_decision` taking and returning no `Capability` pins the absence of a grant.
/// Falsification: replayable `kernel/falsifications/sched.region_reaping.verification.reap_is_permitted_only_to_the_supervising_rendezvous.patch`
#[kani::proof]
fn reap_is_permitted_only_to_the_supervising_rendezvous() {
    let invoked: RendezvousId = kani::any();
    let supervising_rendezvous: Option<RendezvousId> =
        if kani::any() { Some(kani::any()) } else { None };
    let dead: bool = kani::any();

    match reap_decision(supervising_rendezvous, invoked, dead) {
        Reap::Permitted => {
            assert_eq!(
                supervising_rendezvous,
                Some(invoked),
                "reaped through the wrong rendezvous"
            );
            assert!(dead, "reaped a live thread");
        }
        // A live child is refused as such only when it really is ours; otherwise the answer
        // must disclose nothing, which is the next harness.
        Reap::StillAlive => {
            assert_eq!(supervising_rendezvous, Some(invoked));
            assert!(!dead);
        }
        Reap::NotSupervised => assert_ne!(supervising_rendezvous, Some(invoked)),
    }
}

/// **The refusal for a thread you do not supervise does not depend on the thread.** For any two
/// liveness values, a tid whose fault rendezvous is not the invoked one answers identically, so a
/// supervisor cannot use `REAP` to learn whether some other supervisor's child (or a recycled
/// tid) is alive. The two facts are distinguishable only *inside* the relationship.
/// Falsification: replayable `kernel/falsifications/sched.region_reaping.verification.a_stranger_reveals_nothing_about_its_liveness.patch`
#[kani::proof]
fn a_stranger_reveals_nothing_about_its_liveness() {
    let invoked: RendezvousId = kani::any();
    let supervising_rendezvous: Option<RendezvousId> =
        if kani::any() { Some(kani::any()) } else { None };
    kani::assume(supervising_rendezvous != Some(invoked));

    assert_eq!(
        reap_decision(supervising_rendezvous, invoked, true),
        reap_decision(supervising_rendezvous, invoked, false),
    );
    assert_eq!(
        reap_decision(supervising_rendezvous, invoked, true),
        Reap::NotSupervised
    );
}

/// **A survey shows a thread only to the rendezvous that supervises it.** For
/// every pair of rendezvous names, inclusion holds exactly when the recorded fault rendezvous
/// is the invoked one. Both directions matter: a stranger is never shown, and every member is,
/// so a monitor missing an entry can conclude the thread is gone rather than hidden.
/// Falsification: replayable `kernel/falsifications/sched.region_reaping.verification.a_survey_shows_exactly_the_endpoints_own_children.patch`
#[kani::proof]
fn a_survey_shows_exactly_the_endpoints_own_children() {
    let invoked: RendezvousId = kani::any();
    let supervising_rendezvous: Option<RendezvousId> =
        if kani::any() { Some(kani::any()) } else { None };

    assert_eq!(
        survey_includes(supervising_rendezvous, invoked),
        supervising_rendezvous == Some(invoked)
    );
}

/// **Seeing and collecting are scoped by the same relationship.** For every
/// input, a thread is in the survey exactly when a reap of it would get past the supervision
/// check, whatever its liveness. So the domain `ps` reports and the domain a supervisor may
/// reap from cannot diverge, which is the property that lets one predicate answer both.
/// Falsification: replayable `kernel/falsifications/sched.region_reaping.verification.the_view_and_the_reap_have_the_same_scope.patch`
#[kani::proof]
fn the_view_and_the_reap_have_the_same_scope() {
    let invoked: RendezvousId = kani::any();
    let supervising_rendezvous: Option<RendezvousId> =
        if kani::any() { Some(kani::any()) } else { None };
    let dead: bool = kani::any();

    assert_eq!(
        survey_includes(supervising_rendezvous, invoked),
        reap_decision(supervising_rendezvous, invoked, dead) != Reap::NotSupervised,
    );
}
