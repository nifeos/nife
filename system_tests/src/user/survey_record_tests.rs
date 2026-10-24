//! The `SURVEY` selector, and the records it answers: a thread's placement, and its CPU time.
//!
//! `survey_tests` proves the walk (what a domain contains, who may look); this proves the selector
//! (which per-thread fact a walk asks for), the axis calef's 2026-09-21 ruling added. They are
//! separate because a broken selector reports the wrong fact about the right threads, which reads
//! nothing like a broken walk and is the more dangerous, since every tid still looks correct.
//! Placement is `sched`'s (`pick_spawn_target`, `place_on`), with no line under `arch/`, so all
//! three ISAs run these assertions literally (DECISIONS §19 (architectural parity is a tenet)). See
//! notes/process-view/survey-design.md.

use abi::Error;
use abi::survey::record;

use super::supervision_tests::REPORT_STUB;
use super::survey_tests::{
    TEST_ROWS, arena, child_in, collect_all, drain, hold_supervisor, hold_view, hold_write,
    rendezvous, tidy,
};
use crate::arch::exceptions::TrapFrame;
use crate::sched;
use crate::syscall::invoke;

/// **A record number this kernel does not answer**, which is what the refusal tests ask for.
///
/// One past the last record there is, which is the way an unknown selector actually arrives: a
/// program built against a later kernel. Written as an offset from the last constant rather than
/// as a literal so that adding a record moves it, and
/// [`the_unknown_record_is_really_unknown`](self::the_unknown_record_is_really_unknown) fails
/// loudly if somebody adds one and this does not move.
const UNKNOWN_RECORD: u64 = record::CPU_TIME + 1;

/// **The refusal tests below are testing what they think they are.** If a later record takes
/// [`UNKNOWN_RECORD`]'s value, every one of them starts asserting that a *known* record is refused,
/// which would pass for the wrong reason until somebody read the file.
#[test_case]
fn the_unknown_record_is_really_unknown() {
    assert!(
        !record::is_known(UNKNOWN_RECORD),
        "UNKNOWN_RECORD names a record this kernel answers, so the refusal tests below are void",
    );
}

/// `invoke(cap, SURVEY, cursor, record, _)` through the real dispatcher, returning the three words
/// a userspace caller would read out of its registers.
///
/// The selector goes in the argument the method never used, which is what makes
/// [`record::STATE`]`== 0` the whole of the backward-compatibility story: a caller written before
/// the selector existed passed a zero here without meaning to, and gets the record it was already
/// reading.
fn survey_record(slot: u64, cursor: u64, record: u64) -> (i64, u64, u64) {
    let mut frame = TrapFrame::for_user_entry(0, 0, [0, 0, 0]);
    match invoke(&mut frame, slot, abi::rendezvous::SURVEY, cursor, record, 0) {
        Ok(next) => (next, frame.arg(1), frame.arg(2)),
        Err(e) => (e as i64, 0, 0),
    }
}

/// Walk a whole domain with one record, collecting `(tid, word)` pairs.
///
/// Written here rather than driven through `ps::collect` on purpose, and it is the one place in
/// this file that departs from `survey_tests`' discipline of using the real program's loop.
/// `ps::collect` is hard-wired to the state record, because that is the only record `ps` wants; a
/// test of the *selector* has to be able to ask for a record no program in the tree asks for yet,
/// or it could not have caught a record that returns the right answer only when `ps` asks.
fn walk(slot: u64, record: u64) -> ([(u64, u64); TEST_ROWS], usize) {
    let mut out = [(0u64, 0u64); TEST_ROWS];
    let mut n = 0;
    let mut cursor = 0;
    loop {
        let (next, tid, word) = survey_record(slot, cursor, record);
        assert!(
            next >= 0,
            "a walk that should have been permitted was refused with {next}",
        );
        let next = next as u64;
        if next == abi::survey::DONE {
            return (out, n);
        }
        assert!(
            n < TEST_ROWS,
            "the domain outgrew this test's buffer, so what it reported is not the domain",
        );
        out[n] = (tid, word);
        n += 1;
        cursor = next;
    }
}

/// Two live supervised children, parked in a send nobody receives, so they stay members of the
/// domain for the length of a test rather than exiting under it. Returns their tids and the world
/// to give back.
struct Domain {
    budget: u64,
    rendezvous_region: u64,
    supervision: sched::RendezvousId,
    parking: sched::RendezvousId,
    tids: [u64; 2],
}

impl Domain {
    fn build() -> Self {
        let (budget, rendezvous_region) = arena();
        let supervision = rendezvous(rendezvous_region);
        let parking = rendezvous(rendezvous_region);
        let a = child_in(budget, REPORT_STUB, Some(parking), supervision);
        let b = child_in(budget, REPORT_STUB, Some(parking), supervision);
        // Both children must have reached their sends, or "it was placed on a core" is a claim
        // about a thread that has not started and the placement field is still its initial value.
        assert!(
            super::wait_for(|| sched::rendezvous_waiting_senders(parking) == 2),
            "the two children never reached their sends",
        );
        Self {
            budget,
            rendezvous_region,
            supervision,
            parking,
            tids: [a, b],
        }
    }

    /// Release the children, collect the corpses, and give the arena back. Takes the viewer slots
    /// the test opened, because a survey capability left in the table outlives the region it names.
    fn tidy(self, slots: &[u64]) {
        drain(self.parking, 2);
        let supervisor = hold_supervisor(self.supervision);
        collect_all(supervisor, &self.tids);
        let mut all = [0u64; 8];
        let n = slots.len();
        assert!(n < all.len(), "more viewer slots than this helper can hold");
        all[..n].copy_from_slice(slots);
        all[n] = supervisor;
        tidy(self.budget, self.rendezvous_region, &all[..n + 1]);
    }
}

/// Every thread a survey reports was placed on a cpu that is actually online.
///
/// The assertion asks the online mask, not a count. The online set is not `0..n`: it is
/// `{1, 2, 3}` on the VisionFive 2, whose slot 0 is an M-mode monitor core with no MMU, so
/// `id < online_count()` would pass on QEMU `virt` while reporting a parked core. It also pins the
/// sentinel: a started thread never reports [`record::NO_CPU`], so a reader is not tallying "no
/// answer" as a core. See notes/process-view/survey-design.md, "The cpu id is a name, not an index".
#[test_case]
fn every_surveyed_thread_reports_a_cpu_that_is_online() {
    let domain = Domain::build();
    let viewer = hold_view(domain.supervision);

    let (rows, n) = walk(viewer, record::PLACEMENT);
    assert_eq!(n, 2, "the domain reported {n} members where two were built");

    let mask = crate::smp::online_harts_mask();
    for &(tid, cpu) in &rows[..n] {
        assert_ne!(
            cpu,
            record::NO_CPU,
            "thread {tid} is running and reported no placement at all",
        );
        assert!(
            cpu < usize::BITS as u64,
            "thread {tid} reported cpu {cpu}, which is not a position in the online mask",
        );
        assert!(
            mask & (1 << cpu) != 0,
            "thread {tid} was reported on cpu {cpu}, which is not in the online set {mask:#b}: \
             nothing drains a parked core's inbox, so either the placement is wrong or the \
             report is",
        );
    }

    domain.tidy(&[viewer]);
}

/// A placement census needs no online mask, which decides whether this record is safe to hand a
/// userspace supervisor on its own.
///
/// A reader keying a tally by the ids the record returns cannot have the VisionFive 2 bug, because
/// those ids are by construction a subset of the online set; this asserts that subset relation. A
/// census still cannot tell an online core with no threads from a core that is not online, and
/// nothing in this tree answers that for userspace today.
#[test_case]
fn the_cpus_a_census_observes_are_a_subset_of_the_online_set() {
    let domain = Domain::build();
    let viewer = hold_view(domain.supervision);

    let (rows, n) = walk(viewer, record::PLACEMENT);
    let mut observed = 0usize;
    for &(_, cpu) in &rows[..n] {
        observed |= 1 << cpu;
    }
    let online = crate::smp::online_harts_mask();
    assert_eq!(
        observed & !online,
        0,
        "a census observed cpus {observed:#b} and the machine is online on {online:#b}: a tally \
         keyed by the reported id would name a core that does not run anything",
    );

    domain.tidy(&[viewer]);
}

/// **The record a caller does not name is the record it used to get**, which is the whole
/// backward-compatibility claim and is a claim about a wire rather than a hope.
///
/// Every caller in the tree written before the selector existed passed 0 into the then-unused
/// argument. If [`record::STATE`] ever stops being 0, every one of them silently starts reading a
/// cpu id where it expects a run state, and nothing about the call fails: the walk still completes,
/// the tids are still right, and `ps` prints `RUNNING` for a thread on core 2. So the assertion is
/// that an explicit `record::STATE` walk and a walk that names no record at all are the same walk,
/// word for word.
#[test_case]
fn a_caller_that_names_no_record_gets_the_state_record() {
    let domain = Domain::build();
    let viewer = hold_view(domain.supervision);

    // Literally the pre-selector call: a zero in the argument, meant as padding.
    let (unnamed, n_unnamed) = walk(viewer, 0);
    let (named, n_named) = walk(viewer, record::STATE);
    assert_eq!(n_unnamed, n_named);
    assert_eq!(
        unnamed[..n_unnamed],
        named[..n_named],
        "a caller that passed a zero as padding no longer reads the record it used to read",
    );

    // And what it reads really is a state code rather than something that merely compares equal:
    // both children are blocked in a send, which is the state the parking rendezvous puts them in.
    for &(tid, state) in &named[..n_named] {
        assert_eq!(
            state,
            abi::survey::BLOCKED,
            "thread {tid} is parked in a send and the default record reported {state}",
        );
    }

    domain.tidy(&[viewer]);
}

/// **The frame does not depend on the record**: x0 and x1 are the cursor and the tid whichever
/// record is asked for, and only x2 moves.
///
/// This is what makes adding a record cost an existing reader nothing, and what lets a caller that
/// wants two facts walk the domain twice and join on the tid. It is asserted rather than assumed
/// because the cheap way to implement a selector is to let each record drive its own walk, and that
/// version passes every other test in this file while making the tids unjoinable.
#[test_case]
fn two_records_walk_the_same_domain_in_the_same_order() {
    let domain = Domain::build();
    let viewer = hold_view(domain.supervision);

    let (states, n_states) = walk(viewer, record::STATE);
    let (places, n_places) = walk(viewer, record::PLACEMENT);
    assert_eq!(
        n_states, n_places,
        "two records disagree on the domain size"
    );
    for i in 0..n_states {
        assert_eq!(
            states[i].0, places[i].0,
            "the two records reported different tids at position {i}, so a reader cannot join them",
        );
    }

    domain.tidy(&[viewer]);
}

/// An unknown record is refused, and refused loudly.
///
/// A selector this kernel does not answer gets `BadMethod`, as an unknown method word does, since
/// the selector is part of the method's name. A silent default would hand a program built against
/// a later kernel a well-formed number meaning something else. Two unknown values, the two ways one
/// arrives: one past the last known record (a later kernel's reader), and a wild value (a register
/// that held something else).
#[test_case]
fn an_unknown_record_is_refused_rather_than_defaulted() {
    let domain = Domain::build();
    let viewer = hold_view(domain.supervision);

    for bad in [UNKNOWN_RECORD, u64::MAX] {
        let (r0, tid, word) = survey_record(viewer, 0, bad);
        assert_eq!(
            r0,
            Error::BadMethod as i64,
            "record {bad} was answered instead of refused",
        );
        assert_eq!(
            (tid, word),
            (0, 0),
            "a refused survey still wrote something into the caller's registers",
        );
    }

    domain.tidy(&[viewer]);
}

/// **An unknown record against an empty domain is still a refusal, not `DONE`.**
///
/// The reason the selector is checked before the walk rather than at the point the record is
/// extracted. Checking it at extraction is the obvious implementation and it is wrong in exactly
/// one case, which is this one: with no member to extract from, the walk falls off the end and
/// returns `DONE`, and a caller prints "no threads" when what happened is that it asked a question
/// the kernel does not understand. That is the failure a monitor must never have, and it is the
/// same argument milestone 126 (who else is running, and who is allowed to ask) used to refuse
/// showing an unauthorized viewer an empty list.
#[test_case]
fn an_unknown_record_is_refused_even_when_the_domain_is_empty() {
    let (budget, rendezvous_region) = arena();
    let empty = rendezvous(rendezvous_region);
    let viewer = hold_view(empty);

    // The control: a known record against this domain really does answer, rather than refusing for
    // some reason of its own. Without this the assertion below proves nothing.
    let (r0, ..) = survey_record(viewer, 0, record::STATE);
    assert_eq!(
        r0,
        abi::survey::DONE as i64,
        "an empty domain did not answer with DONE, so this test is not measuring what it thinks",
    );

    let (r0, ..) = survey_record(viewer, 0, UNKNOWN_RECORD);
    assert_eq!(
        r0,
        Error::BadMethod as i64,
        "an unknown record against an empty domain answered DONE, which a caller prints as \
         'nothing here'",
    );

    tidy(budget, rendezvous_region, &[viewer]);
}

/// **The capability is still the whole of the authority**, and the selector cannot be used to get
/// around it or to probe with.
///
/// A send-only holder asking for a record that does not exist gets `NotPermitted`, not `BadMethod`:
/// the rights check runs first, so the refusal says "you may not look" and never leaks which
/// selectors this kernel answers to somebody who was not allowed to ask. The placement record adds
/// no authority of its own, which is the claim §150 (how does a thread's CPU time reach userspace?) already weighed for a continuous counter and
/// which a bounded value written once is strictly less than.
#[test_case]
fn a_viewer_without_enumerate_is_refused_before_the_record_is_read() {
    let domain = Domain::build();
    let peer = hold_write(domain.supervision);

    for record in [record::STATE, record::PLACEMENT, u64::MAX] {
        let (r0, ..) = survey_record(peer, 0, record);
        assert_eq!(
            r0,
            Error::NotPermitted as i64,
            "a send-only peer asking for record {record} got something other than a refusal",
        );
    }

    domain.tidy(&[peer]);
}
