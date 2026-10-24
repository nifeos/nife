use abi::Error;
use abi::fault::EVENT_EXIT;

use super::supervision_tests::{REPORT_STUB, REPORT_WORD, build_child_in, build_child_with};
use super::*;
use crate::arch::exceptions::TrapFrame;
use crate::cap::Rights;
use crate::sched;
use crate::syscall::invoke;
use crate::user::holding::Holding;

/// Pages per job region: what `build_child_in` lays down (an address space and its tables, a code
/// page, a stack page, a TCB). Sixteen is the number every region-reclaim test here uses.
const JOB_REGION_PAGES: u64 = 16;

/// How many live jobs the pool holds. Small on purpose: a pool nobody could exhaust would make the
/// renewal claim below unfalsifiable, which is the whole reason the first test exists.
const ROOM: u64 = 3;
const POOL_PAGES: u64 = JOB_REGION_PAGES * ROOM;

/// How many jobs the reaped run puts through that pool. Four times its capacity, so the pass cannot
/// be explained by anything except the regions coming back.
const JOBS: u64 = 12;

/// How long to let the collector get to a corpse before calling it a failure. It is an ordinary
/// unprivileged thread on the run queue, so this is a scheduling wait and not a timing assertion:
/// the *claim* is about which budget the pages land in, and that is asserted exactly.
const YIELDS: u32 = 4000;

/// `invoke(cap, REAP, tid, _, _)` through the real dispatcher, for the tidy-up in the control.
fn reap(slot: u64, tid: u64) -> Result<i64, Error> {
    let mut frame = TrapFrame::for_user_entry(0, 0, [0, 0, 0]);
    invoke(&mut frame, slot, abi::rendezvous::REAP, tid, 0, 0)
}

/// Receive one five-word death message through the ABI.
fn receive_death(slot: u64) -> [u64; 5] {
    let mut frame = TrapFrame::for_user_entry(0, 0, [0, 0, 0]);
    let w0 = invoke(&mut frame, slot, abi::rendezvous::RECEIVE, 0, 0, 0).expect("RECEIVE refused");
    [
        w0 as u64,
        frame.arg(1),
        frame.arg(2),
        frame.arg(3),
        frame.arg(4),
    ]
}

/// The pages the pool has spent and not got back.
fn spent(pool: u64) -> u64 {
    crate::memory_region::usage(pool)
        .expect("the pool exists")
        .0
}

/// Wait for the collector to bring the pool all the way back. `false` if it never does.
fn pool_came_back(pool: u64) -> bool {
    for _ in 0..YIELDS {
        if spent(pool) == 0 {
            return true;
        }
        sched::yield_now();
    }
    spent(pool) == 0
}

/// **Start the real `job_undertaker` binary** the way the interactive progenitor starts it: an ordinary user
/// process whose entire capability table is one endpoint with `READ`.
///
/// Deliberately the real program out of the initial ramdisk rather than a stub, because what is
/// under test is a *program's* behaviour, and a stub would be this test's opinion of it.
///
/// Its thread and its thread's region go into `held`, so the test hands them back when it ends.
/// The endpoints it waits on must be in a region `held` names first, or the kill cannot wake it
/// (`user::holding`'s `BUGS`).
fn spawn_job_undertaker(deaths: sched::RendezvousId, held: &mut Holding) {
    spawn_job_undertaker_with(deaths, None, held);
}

/// Pages for the endpoints a test that starts the undertaker creates: one region, reclaimed first,
/// so the undertaker parked on `deaths` is woken to spend its kill.
const ENDPOINT_PAGES: u64 = 4;

/// [`spawn_job_undertaker`], with the two endpoints the progenitor adds when there is one to tell
/// (milestone 685): `REPORT` at slot 1 and, at slot 2, the spawn endpoint badged
/// `spawnproto::UNDERTAKER_BADGE`, both `WRITE`. Without them the program's two sends are refused
/// for an empty slot and dropped, which is what the tests above run.
fn spawn_job_undertaker_with(
    deaths: sched::RendezvousId,
    tell: Option<(sched::RendezvousId, sched::RendezvousId)>,
    held: &mut Holding,
) {
    let bytes =
        program("job_undertaker").expect("no job_undertaker program in the initial ramdisk");
    let (space, entry) = load(bytes, 0).expect("job_undertaker is not loadable");
    let aspace = readopt_user_address_space(space).expect("register the job_undertaker aspace");

    let thread_control_block_region =
        crate::memory_region::create(2).expect("no tcb region for job_undertaker");
    let tid = sched::create_thread_control_block(thread_control_block_region)
        .expect("no tcb for job_undertaker");
    let slot = sched::thread_control_block_insert_cap(
        tid,
        crate::cap::rendezvous_cap(deaths, Rights::READ),
        None,
    )
    .expect("insert the supervision endpoint");
    assert_eq!(
        slot, 0,
        "job_undertaker reads its supervision endpoint from slot 0",
    );
    if let Some((report, spawn)) = tell {
        for (want, cap) in [
            (1, crate::cap::rendezvous_cap(report, Rights::WRITE)),
            (
                2,
                crate::cap::rendezvous_cap_badged(
                    spawn,
                    Rights::WRITE,
                    grant_plan::spawnproto::UNDERTAKER_BADGE,
                ),
            ),
        ] {
            let slot = sched::thread_control_block_insert_cap(tid, cap, None)
                .expect("insert job_undertaker's endpoints");
            assert_eq!(slot, want, "job_undertaker names its endpoints by slot");
        }
    }
    sched::configure_thread_control_block(tid, entry, USER_STACK_TOP, aspace)
        .expect("configure job_undertaker");
    sched::start_thread_control_block(tid, [0; 3]).expect("start job_undertaker");
    held.add_thread(tid);
    held.add_region(thread_control_block_region);
}

/// **The control: without a collector, a bounded job pool runs out and stays out.**
///
/// Three jobs fit; the fourth is refused, and it is refused *after every one of the three has
/// finished running*, which is the whole point. A finished job's region is not free memory: its
/// corpse persists (dead until reaped, DECISIONS §26) and the pool's watermark only moves forward.
/// This is what the interactive prompt did before this increment, one command at a time, until the
/// shell started answering "could not spawn (the progenitor is out of memory)".
///
/// It is here because the renewal test below is unfalsifiable without it: a pool that could not run
/// out would pass that test whether or not anything was ever collected.
#[test_case]
fn without_a_collector_a_bounded_job_pool_runs_out() {
    let pool = crate::memory_region::create(POOL_PAGES).expect("no job pool");
    let endpoints = crate::memory_region::create(2).expect("no endpoint region");
    let deaths = sched::create_rendezvous_from(endpoints).expect("deaths");
    let report = sched::create_rendezvous_from(endpoints).expect("report");

    let mut corpses = [0u64; ROOM as usize];
    for (i, slot) in corpses.iter_mut().enumerate() {
        let region = crate::memory_region::split(pool, JOB_REGION_PAGES)
            .unwrap_or_else(|| panic!("the pool had no room for job {i}, and it should have"));
        *slot = build_child_in(region, REPORT_STUB, Some(report), Some(deaths));
        assert_eq!(
            sched::ipc_receive(report)[0],
            REPORT_WORD,
            "job {i} never ran, so this test is not measuring what it thinks",
        );
    }

    assert_eq!(
        spent(pool),
        POOL_PAGES,
        "the pool should be fully committed after {ROOM} jobs",
    );
    assert!(
        crate::memory_region::split(pool, JOB_REGION_PAGES).is_none(),
        "a {ROOM}-job pool built a fourth job: this control proves nothing if the pool cannot run \
         out, and the renewal test below leans on it",
    );

    // Tidy: collect the three corpses the way the collector would, then give the pool back.
    let cap = sched::grant(crate::cap::rendezvous_cap(deaths, Rights::READ)).expect("hold deaths");
    for _ in 0..ROOM {
        let msg = receive_death(cap);
        assert_eq!(msg[0], EVENT_EXIT, "the report stub exits cleanly");
        assert_eq!(reap(cap, msg[1]), Ok(0), "the tidy-up reap failed");
    }
    let _ = sched::delete_current_cap(cap);
    sched::reclaim_region(pool).expect("the job pool did not come back");
    sched::reclaim_region(endpoints).expect("the endpoint region did not come back");
}

/// A bounded job pool is enough, because `job_undertaker` gives every region back.
///
/// Twelve jobs through a pool with room for three, each split off the pool, born supervised (§26's
/// spawn-slot convention), run and exited, the pool back to zero before the next. The collector is
/// an unprivileged process with one endpoint and no memory, as at the prompt. The assertion is
/// which budget the pages land in: a reap crediting the collector, or stranding the memory, fails
/// the first `pool_came_back`; the last two assert the pages are spendable again.
#[test_case]
fn job_undertaker_returns_every_finished_job_to_the_pool() {
    let pool = crate::memory_region::create(POOL_PAGES).expect("no job pool");
    let endpoints = crate::memory_region::create(ENDPOINT_PAGES).expect("no endpoint region");
    let deaths = sched::create_rendezvous_from(endpoints).expect("deaths");
    let report = sched::create_rendezvous_from(endpoints).expect("report");
    let mut held = Holding::new();
    held.add_region(endpoints);
    spawn_job_undertaker(deaths, &mut held);

    for i in 0..JOBS {
        assert!(
            pool_came_back(pool),
            "before job {i} the pool was still {} pages down: the collector did not return the \
             previous job's region, so a bounded budget is not enough after all",
            spent(pool),
        );
        let region = crate::memory_region::split(pool, JOB_REGION_PAGES).unwrap_or_else(|| {
            panic!(
                "job {i} could not be carved out of a pool that just \
                                       reported itself empty"
            )
        });
        build_child_in(region, REPORT_STUB, Some(report), Some(deaths));
        assert_eq!(
            sched::ipc_receive(report)[0],
            REPORT_WORD,
            "job {i} never ran",
        );
    }

    assert!(
        pool_came_back(pool),
        "the last job's region never came back",
    );
    assert!(
        !crate::memory_region::has_children(pool),
        "a job region outlived its corpse, so the pool is still committed to it",
    );
    // Spendable, not merely counted: the whole pool carves again in one piece.
    let again = crate::memory_region::split(pool, POOL_PAGES)
        .expect("the pool would not spend the pages the collector returned");
    sched::reclaim_region(again).expect("reclaim the re-split region");
    sched::reclaim_region(pool).expect("the job pool did not come back");
    held.release_or_fail("job_undertaker");
}

/// The reap protocol (milestone 685 (a job is finished when its memory is back), calef's ruling of
/// 2026-10-06 UTC, option A).
///
/// After collecting a job, `job_undertaker` sends `spawnproto::reaped(label, tid)` (label: the job's
/// supervision badge) on its `UNDERTAKER_BADGE` endpoint; the job's window frees on that message,
/// not before (`grant_plan::job_windows::Windows`). The booted half of confinement claim 24 (fatal
/// risk 7's criterion (a)); the host test is the rule. The replayable falsification breaks the
/// label; none reaches the region's return, which an early announcement still meets on one core.
///
/// Falsification: replayable `system_tests/falsifications/user.job_undertaker_tests.job_undertaker_says_which_job_it_reaped_and_only_then_is_its_window_free.patch`
#[test_case]
fn job_undertaker_says_which_job_it_reaped_and_only_then_is_its_window_free() {
    use grant_plan::job_windows::Windows;
    use grant_plan::spawnproto;

    /// The job's label, as the progenitor would have minted it. Any nonzero value.
    const LABEL: u64 = 0x685;

    let pool = crate::memory_region::create(JOB_REGION_PAGES).expect("no job pool");
    let endpoints = crate::memory_region::create(ENDPOINT_PAGES).expect("no endpoint region");
    let deaths = sched::create_rendezvous_from(endpoints).expect("deaths");
    let report = sched::create_rendezvous_from(endpoints).expect("report");
    let spawn = sched::create_rendezvous_from(endpoints).expect("spawn");
    let mut held = Holding::new();
    held.add_region(endpoints);
    spawn_job_undertaker_with(deaths, Some((report, spawn)), &mut held);

    // One window, so a second job's take can only be the first job's window or nothing.
    let mut windows = Windows::new(1, 2);
    let w = windows.take(LABEL).expect("an empty pool has a window");

    let region = crate::memory_region::split(pool, JOB_REGION_PAGES).expect("no job region");
    let tid = build_child_with(
        region,
        REPORT_STUB,
        &[crate::cap::rendezvous_cap(
            report,
            Rights::WRITE.union(Rights::GRANT),
        )],
        Some(crate::cap::rendezvous_cap_badged(
            deaths,
            Rights::READ,
            LABEL,
        )),
    );
    assert_eq!(
        sched::ipc_receive(report)[0],
        REPORT_WORD,
        "the job never ran"
    );
    assert_eq!(
        windows.take(LABEL + 1),
        None,
        "a second job got the window while its holder was unreaped"
    );

    let msg = sched::ipc_receive(spawn);
    assert_eq!(
        msg[3],
        spawnproto::UNDERTAKER_BADGE,
        "the reaped message did not carry the undertaker's badge, so the progenitor would read it \
         as a request"
    );
    assert_eq!(
        (msg[0], msg[1], msg[2]),
        spawnproto::reaped(LABEL, tid),
        "the reaped message named the wrong job (label, tid)"
    );
    assert_eq!(
        spent(pool),
        0,
        "the reaped message arrived before the job's region was back in the pool"
    );
    assert_eq!(
        windows.reaped(msg[0]),
        Some(w),
        "the reaped message did not free the window its job held"
    );
    assert_eq!(
        windows.take(LABEL + 1),
        Some(w),
        "the reaped job's window did not come back"
    );
    sched::reclaim_region(pool).expect("the job pool did not come back");
    held.release_or_fail("job_undertaker");
}
