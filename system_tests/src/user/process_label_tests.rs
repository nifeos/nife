//! **A process's threads share one label in a survey, and the count is the rows** (milestone 942
//! (`ps` and `top` show which process each thread belongs to), §312 (`ENUMERATE` on a domain
//! covers which of its threads share a process)).
//!
//! The kernel arranges what no program can: five threads of one process, started one by one with
//! the supervision slot refilled before each `START` (the slot is consumed, so a sibling started
//! later is otherwise unsupervised), four of them under one rendezvous and the fifth under
//! another, and a second process of one thread under the first rendezvous. The survey is then read
//! the way `ps` reads it, by `ps::collect` and `Survey::join_process`, through the real
//! dispatcher. A `rg --threads 4` run is the model for the claim; it is not the harness, because a
//! `std` thread is not supervised unless its starter supervises it (the block's `BUGS`).

//! Name: provisional, minted 2026-10-27 (UTC) by lane milestone/942-ps-top-process-grouping.

use abi::fault::FAULT_EP_SLOT;
use abi::survey::record;

use super::process_tests::{SPINNER, STACK_VA, call, create_process, join, lay_out, wait_for};
use super::survey_record_tests::survey_record;
use super::survey_tests::{TEST_ROWS, hold_supervisor, hold_view, rendezvous};
use crate::capability_object::Rights;
use crate::sched;

/// Walk `slot`'s domain with `ps`'s own loop, then join the process labels, and hand back the
/// tid and label of every row in the order the kernel reported them.
fn labeled(slot: u64) -> ([(u64, Option<u64>); TEST_ROWS], usize, usize) {
    let mut rows = [ps::Row::default(); TEST_ROWS];
    let mut s = ps::collect(&mut rows, &mut |cursor| {
        survey_record(slot, cursor, record::STATE)
    });
    s.join_process(&mut |cursor| survey_record(slot, cursor, record::PROCESS));
    assert!(
        s.is_complete(),
        "the walk was refused or outgrew its buffer"
    );
    let mut out = [(0, None); TEST_ROWS];
    let mut with_count = 0;
    for (i, r) in s.rows().iter().enumerate() {
        out[i] = (r.tid, r.process);
        if let Some(l) = r.process {
            // The count `ps` prints for this row is the rows that carry its label.
            assert_eq!(
                s.threads_with_label(l),
                s.rows().iter().filter(|o| o.process == Some(l)).count()
            );
            with_count += 1;
        }
    }
    (out, s.rows().len(), with_count)
}

fn count(rows: &[(u64, Option<u64>)], label: Option<u64>) -> usize {
    rows.iter().filter(|r| r.1 == label).count()
}

/// **Four threads of a process share one label, another process has its own, and the viewer of a
/// partial process counts only what it can see.**
///
/// What nothing else catches: `process_threads_started` is read inside the kernel, and none of the
/// earlier tests goes through what a program can see. Here the first viewer holds a domain with
/// four threads of process A and the only thread of process B; the second holds a domain with
/// A's fifth thread alone, so A is a partial process for it.
/// Falsification: unfalsified. Written with the record it tests (milestone 942), and run green on
/// all three architectures; no replayable patch yet.
#[test_case]
fn threads_of_one_process_share_a_label_and_the_count_is_the_rows() {
    let region_a = crate::memory_region::create(32).expect("no region");
    let region_b = crate::memory_region::create(32).expect("no region");
    let endpoints = crate::memory_region::create(4).expect("no endpoint region");
    let domain_one = rendezvous(endpoints);
    let domain_two = rendezvous(endpoints);

    let (space_a, _) = lay_out(region_a, &[SPINNER]);
    let (process_a, pid_a) = create_process(region_a, space_a);
    let (space_b, _) = lay_out(region_b, &[SPINNER]);
    let (process_b, _) = create_process(region_b, space_b);

    let start = |region, process, ep| {
        let (tid, tcb) = join(region, process, 0, STACK_VA);
        sched::thread_control_block_insert_capability(
            tid,
            crate::capability_object::rendezvous_capability(ep, Rights::READ),
            Some(FAULT_EP_SLOT),
        )
        .expect("insert the supervision endpoint");
        sched::start_thread_control_block(tid, [0; 3]).expect("start a spinner");
        (tid, tcb)
    };
    let mut a = [(0, 0); 5];
    for (i, member) in a.iter_mut().enumerate() {
        *member = start(
            region_a,
            process_a,
            if i < 4 { domain_one } else { domain_two },
        );
    }
    let b = start(region_b, process_b, domain_one);
    assert_eq!(
        sched::process_state(pid_a).map(|(members, _)| members),
        Some(5),
        "process A does not have the five members this test counts"
    );

    // The first viewer: A's four threads and B's one.
    let view_one = hold_view(domain_one);
    let (rows, n, labeled_rows) = labeled(view_one);
    assert_eq!(
        n, 5,
        "the first domain holds four threads of A and one of B"
    );
    assert_eq!(labeled_rows, 5, "every row has a label");
    let rows = &rows[..n];
    let label_of = |tid| rows.iter().find(|r| r.0 == tid).and_then(|r| r.1);
    let a_label = label_of(a[0].0).expect("A's first thread has a label");
    for member in &a[..4] {
        assert_eq!(
            label_of(member.0),
            Some(a_label),
            "A's threads share a label"
        );
    }
    let b_label = label_of(b.0).expect("B's thread has a label");
    assert_ne!(a_label, b_label, "two processes must not share a label");
    assert_eq!(count(rows, Some(a_label)), 4, "A's count is its rows");
    assert_eq!(count(rows, Some(b_label)), 1, "B's count is its row");
    // An ordinal for the walk, so a kernel address or an identity cannot be hiding in it.
    for label in [a_label, b_label] {
        assert!(
            (1..=2).contains(&label),
            "label {label} is not a walk ordinal of two processes"
        );
    }
    // And the table `ps` prints says the same.
    let mut table = [ps::Row::default(); TEST_ROWS];
    let mut s = ps::collect(&mut table, &mut |c| {
        survey_record(view_one, c, record::STATE)
    });
    s.join_process(&mut |c| survey_record(view_one, c, record::PROCESS));
    let mut text = [0u8; 512];
    let mut len = 0;
    s.write_report(&mut |bytes| {
        text[len..len + bytes.len()].copy_from_slice(bytes);
        len += bytes.len();
    });
    let text = core::str::from_utf8(&text[..len]).expect("ASCII");
    assert!(
        text.starts_with("PROC  THR"),
        "the table lost its process columns: {text}"
    );
    assert_eq!(
        text.lines().count(),
        6,
        "a header and one line per row: {text}"
    );

    // The second viewer holds only A's fifth thread: a partial process, counted as one.
    let view_two = hold_view(domain_two);
    let (rows_two, n_two, _) = labeled(view_two);
    assert_eq!(n_two, 1, "the second domain holds one thread of A");
    assert_eq!(rows_two[0].0, a[4].0);
    assert_eq!(
        rows_two[0].1,
        Some(1),
        "the lone thread is the first process the walk meets"
    );
    assert_eq!(
        sched::process_state(pid_a).map(|(members, _)| members),
        Some(5),
        "the process still has five members: the second viewer counts its share, not the total"
    );

    // A thread of a process outside the viewer's domain does not appear. B's domain holds B only.
    assert!(
        !rows.iter().any(|r| r.0 == a[4].0),
        "the first viewer saw a thread outside its domain"
    );

    tear_down(
        domain_one,
        [(process_a, region_a), (process_b, region_b)],
        [a[0].0, b.0],
        &[view_one, view_two],
        endpoints,
        a.iter().map(|m| m.1).chain([b.1]),
    );
}

/// End both processes, collect one corpse of each through the first domain's supervisor, and give
/// every slot and region back.
fn tear_down(
    domain: sched::RendezvousId,
    processes: [(u64, u64); 2],
    reaped: [u64; 2],
    views: &[u64],
    endpoints: u64,
    tcbs: impl Iterator<Item = u64>,
) {
    let supervisor = hold_supervisor(domain);
    for (process, _) in processes {
        assert_eq!(
            call(process, abi::process::DESTROY, 0, 0, 0),
            Ok(0),
            "DESTROY refused"
        );
    }
    for tid in reaped {
        assert!(
            wait_for(5, || {
                let mut frame = crate::arch::exceptions::TrapFrame::for_user_entry(0, 0, [0, 0, 0]);
                crate::syscall::invoke(&mut frame, supervisor, abi::rendezvous::REAP, tid, 0, 0)
                    == Ok(0)
            }),
            "a member could not be collected through the rendezvous that showed it",
        );
    }
    for slot in views
        .iter()
        .copied()
        .chain([supervisor, processes[0].0, processes[1].0])
        .chain(tcbs)
    {
        let _ = sched::delete_current_capability(slot);
    }
    for (_, region) in processes {
        let _ = sched::reclaim_region(region);
    }
    sched::reclaim_region(endpoints).expect("the endpoint region did not come back");
}
