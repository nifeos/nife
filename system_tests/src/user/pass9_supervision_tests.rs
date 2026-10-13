//! **Pass 9: a foreign endpoint cannot reap or survey a child it does not supervise** (an outside
//! security reviewer attacks the confinement claim, 2026-10-13 UTC; fatal risk 7's confinement
//! claim, claims 6, 7, 8 and 9).
//!
//! The suite proves these properties with pure logic (`capability::reap_decision`,
//! `survey_includes`) and drives `REAP`/`SURVEY` refusals for a *WRITE-only* holder of the right
//! endpoint (`confinement_attack_tests`). What it did not have is a holder of the *wrong* endpoint:
//! a supervisor of its own children trying to act on a child that reports to someone else. That is
//! the adversary a `ps`-like tool becomes if the scope check is by rights rather than by
//! relationship, and it is what this module boots.
//!
//! A real ring-3 child (`supervision_tests::REPORT_STUB`, the same hand-assembled program the
//! supervision suite uses) is spawned supervised by one endpoint. A second, unrelated endpoint is
//! then held with `READ | ENUMERATE`, every right a supervisor needs, and used to attack:
//!
//! - `REAP` through the foreign endpoint answers `NotSupervised` (claim 6): a supervisor cannot
//!   collect a corpse it does not supervise, even holding `READ`.
//! - `REAP` of a made-up tid through the foreign endpoint answers the *same* `NotSupervised`
//!   (claim 7): a real-but-foreign child and a tid that never existed are indistinguishable from
//!   the foreign endpoint, so a refusal discloses nothing about the stranger.
//! - `SURVEY` through the foreign endpoint answers `DONE` with no entry (claim 8): the view shows
//!   exactly the endpoint's own children, and the foreign child is not among them.
//! - The positive controls, through the *real* supervision endpoint: `SURVEY` returns an entry
//!   (not `DONE`), and `REAP` collects the corpse (claim 9, the view and the reap are one domain).
//!
//! The reap and the survey are refused and permitted by the *same relationship*, which is claim 9:
//! the foreign endpoint is refused both, the owning endpoint is granted both.
//!
//! Name: provisional (pass 9's lane, 2026-10-13 UTC), for the module, the file and the function.
//! Cross-ISA: the supervision relationship, `reap_decision` and `survey_includes` are portable
//! kernel code; the child stub is per-architecture inside `supervision_tests` (DECISIONS §19
//! (architectural parity is a tenet)).

use abi::Error;

use super::supervision_tests::{REPORT_WORD, build_child_in};
use crate::arch::exceptions::TrapFrame;
// Aliased so this file spells `capability` out (calef, 2026-10-06); the kernel names keep their
// pre-sweep spelling until the capability worklist renames them.
use crate::cap::{Rights, rendezvous_cap as rendezvous_capability};
use crate::sched::{self, delete_current_cap as delete_current_capability};
use crate::syscall::invoke;

/// `invoke` through the real dispatcher. Copied in shape from `confinement_attack_tests::call`.
fn call(slot: u64, method: u64, a0: u64, a1: u64, a2: u64) -> Result<i64, Error> {
    let mut frame = TrapFrame::for_user_entry(0, 0, [0, 0, 0]);
    invoke(&mut frame, slot, method, a0, a1, a2)
}

/// **A holder of an unrelated supervision endpoint cannot reap or survey another endpoint's child,
/// and the two operations are refused and permitted by the same relationship.**
///
/// Nothing in the attack half blocks: `REAP` and `SURVEY` do not park. The positive `REAP` uses a
/// settle-retry (`supervision_tests`/`reap_tests` explain the one-context-switch window where a
/// just-dead corpse is still standing on its own kernel stack and the reclaim answers
/// `NotPermitted`).
#[test_case]
fn a_foreign_endpoint_cannot_reap_or_survey_a_child_it_does_not_supervise() {
    // One small region for the three endpoints; a bigger one for the child's whole world.
    let endpoints = crate::memory_region::create(4).expect("no endpoint region");
    let report = sched::create_rendezvous_from(endpoints).expect("no report rendezvous");
    let owner_ep = sched::create_rendezvous_from(endpoints).expect("no owner rendezvous");
    let foreign_ep = sched::create_rendezvous_from(endpoints).expect("no foreign rendezvous");

    let child_region = crate::memory_region::create(16).expect("no region for the child");
    // The child SENDs REPORT_WORD on slot 0 and exits; it is supervised by `owner_ep`.
    let child = build_child_in(
        child_region,
        super::supervision_tests::REPORT_STUB,
        Some(report),
        Some(owner_ep),
    );

    // Wait until the child has run (its report arrives), so it is a live-then-dying member of
    // `owner_ep`'s domain and nobody else's by the time the attacks run.
    assert_eq!(
        sched::ipc_receive(report)[0],
        REPORT_WORD,
        "the supervised child never reported, so the setup is not what the attack assumes",
    );

    // The supervisor's own capability, and a foreign one with the identical rights. READ is what
    // REAP takes; ENUMERATE is what SURVEY takes; a supervisor holds both. The foreign holder holds
    // both too, so what refuses it below is the relationship, never a missing right.
    let owner = sched::grant(rendezvous_capability(
        owner_ep,
        Rights::READ.union(Rights::ENUMERATE),
    ))
    .expect("grant the owner supervision capability");
    let foreign = sched::grant(rendezvous_capability(
        foreign_ep,
        Rights::READ.union(Rights::ENUMERATE),
    ))
    .expect("grant the foreign supervision capability");

    // --- Claim 6: reap through the wrong endpoint is refused. -----------------------------------
    assert_eq!(
        call(foreign, abi::rendezvous::REAP, child, 0, 0),
        Err(Error::NotSupervised),
        "a foreign endpoint reaped a child it does not supervise: a supervisor collected a corpse \
         that was never its own (claim 6)",
    );

    // --- Claim 7: a made-up tid through the wrong endpoint gives the SAME answer. ---------------
    // `child | 0xffff_0000_0000` perturbs the generation half of the generational name, so it
    // names nothing live. The answer must match the real-but-foreign child's exactly, or the
    // refusal would leak whether a given tid is a real thread elsewhere in the system.
    let bogus_tid = child | 0xffff_0000_0000;
    assert_eq!(
        call(foreign, abi::rendezvous::REAP, bogus_tid, 0, 0),
        Err(Error::NotSupervised),
        "reaping a nonexistent tid through a foreign endpoint answered differently from reaping a \
         real foreign child: the refusal discloses a stranger's liveness (claim 7)",
    );

    // --- Claim 8: survey through the wrong endpoint shows nothing. ------------------------------
    assert_eq!(
        call(
            foreign,
            abi::rendezvous::SURVEY,
            0,
            abi::survey::record::STATE,
            0
        ),
        Ok(abi::survey::DONE as i64),
        "a foreign endpoint's survey returned an entry: it saw a child outside its own domain \
         (claim 8)",
    );

    // --- Claim 9: the owning endpoint sees and reaps the same child. ----------------------------
    // SURVEY through the real supervisor returns a cursor past the first match (>= 1), not DONE.
    let surveyed = call(
        owner,
        abi::rendezvous::SURVEY,
        0,
        abi::survey::record::STATE,
        0,
    )
    .expect("the owning endpoint's survey was refused");
    assert_ne!(
        surveyed,
        abi::survey::DONE as i64,
        "the owning endpoint's survey showed no child: the view and the reap are not one domain \
         (claim 9)",
    );

    // And REAP through the real supervisor collects the corpse, once it has settled off its stack.
    let mut reaped = call(owner, abi::rendezvous::REAP, child, 0, 0);
    super::wait_for(|| {
        if reaped != Err(Error::NotPermitted) && reaped != Err(Error::StillAlive) {
            return true;
        }
        reaped = call(owner, abi::rendezvous::REAP, child, 0, 0);
        reaped != Err(Error::NotPermitted) && reaped != Err(Error::StillAlive)
    });
    assert_eq!(
        reaped,
        Ok(0),
        "the owning endpoint could not reap its own settled corpse: the view and the reap are not \
         one domain (claim 9)",
    );

    // Teardown: the child's region came back with the reap; give the endpoints' region back too.
    let _ = delete_current_capability(owner);
    let _ = delete_current_capability(foreign);
    sched::reclaim_region(endpoints).expect("the endpoint region did not come back");
}
