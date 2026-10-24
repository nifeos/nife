use super::*;
use crate::sched;

/// Where the hand-built child's code and stack go: the map's image base and top stack page, as
/// `supervision_tests` gives for its own.
const CODE_VA: u64 = address_space_map::IMAGE_BASE;
const STACK_VA: u64 = address_space_map::STACK_TOP_PAGE;

/// A one-instruction runaway: branch (aarch64) or jump (riscv) to self, forever. It never
/// yields, never syscalls, never touches an rendezvous, so nothing cooperative can end it and the
/// forcible tier is the only thing that can.
#[cfg(target_arch = "aarch64")]
const SPIN_STUB: &[u32] = &[0x1400_0000]; // b .
#[cfg(target_arch = "riscv64")]
const SPIN_STUB: &[u32] = &[0x0000_006F]; // j .  (jal x0, 0)
/// `x86_64`'s is in `user::x86_programs`, shared with the boot tour; see that module's header.
#[cfg(target_arch = "x86_64")]
const SPIN_STUB: &[u32] = super::x86_programs::SPIN;

/// Build a runaway from parts (address space, code, stack, TCB all in one region), start it, then
/// reclaim its region while it still spins, and assert the region comes back whole.
#[test_case]
fn destroy_force_kills_a_runaway_and_reclaims_its_region() {
    let threads_before = sched::thread_count();

    // The runaway's whole world in one region: the address space's root and tables, its code
    // page, its stack, and its TCB, so a single `DESTROY` reclaims all of it.
    let region = crate::memory_region::create(16).expect("no region for the runaway");
    // These sixteen frames, by name: the window below (a remote core's tick and up to a second of
    // yielding) is room for a neighbour's teardown to land in a machine-wide free-frame delta. See
    // §28 (SMP placement: two random choices at spawn) and notes/load-sensitive-assertions.md.
    let run = crate::testing::RegionRun::of(region);
    let aspace = user_address_space_create(region).expect("no aspace");

    let code_phys = code_page(region, SPIN_STUB);
    user_address_space_map(
        aspace,
        CODE_VA,
        code_phys,
        Flags::user_code(),
        crate::revoke::PageMapSource::NoCapability,
    )
    .expect("map code");

    let stack_phys = crate::memory_region::retype_page(region).expect("no stack frame");
    user_address_space_map(
        aspace,
        STACK_VA,
        stack_phys,
        Flags::user_data(),
        crate::revoke::PageMapSource::NoCapability,
    )
    .expect("map stack");

    let tid = sched::create_thread_control_block(region).expect("no tcb");
    sched::configure_thread_control_block(tid, CODE_VA, STACK_VA + page_frames::FRAME_SIZE, aspace)
        .expect("configure");
    sched::start_thread_control_block(tid, [0; 3]).expect("start");

    // Let the runaway reach EL0, so the teardown is of a running thread, not an embryo.
    for _ in 0..8 {
        sched::yield_now();
    }

    // The first pass arms the kill and refuses; the runaway becomes a corpse at its next
    // preemption; the retry reclaims. Time-bounded, because the runaway may be on another core that
    // only its own tick converts (§28.3 / §16 (object revocation)): a yield loop here would end
    // inside one 10 ms tick, and one second spans ~100 ticks while still failing a real bug.
    let deadline = crate::arch::timer::now() + crate::arch::timer::frequency();
    let mut reclaimed = false;
    while crate::arch::timer::now() < deadline {
        if sched::reclaim_region(region).is_ok() {
            reclaimed = true;
            break;
        }
        sched::yield_now();
    }
    assert!(
        reclaimed,
        "DESTROY never tore down a runaway: the killed flag did not convert it to a corpse",
    );

    assert!(
        sched::thread_count() <= threads_before,
        "the force-killed runaway was reclaimed but never actually reaped",
    );
    run.assert_returned("reclaiming a force-killed runaway did not return its frames");
}

/// A child that blocks in `RECEIVE` on the rendezvous in slot 0 and never comes back on its own. Nine
/// instructions, the [`super::supervision_tests::REPORT_STUB`] shape with `RECEIVE` where the `SEND`
/// was: the tail `EXIT` is reached only if the receive returns, which happens exactly when the
/// rendezvous is revoked out from under it.
#[cfg(target_arch = "aarch64")]
const RECEIVE_STUB: &[u32] = &[
    0xD280_0000, // movz x0, #0            (slot 0)
    0xD280_0000 | ((abi::rendezvous::RECEIVE as u32) << 5) | 1, // movz x1, #RECEIVE
    0xD280_0002, // movz x2, #0
    0xD280_0003, // movz x3, #0
    0xD280_0004, // movz x4, #0
    0xD280_0000 | ((abi::SYS_INVOKE as u32) << 5) | 8, // movz x8, #SYS_INVOKE
    0xD400_0001, // svc #0                 (RECEIVE: blocks)
    0xD280_0008, // movz x8, #SYS_EXIT
    0xD400_0001, // svc #0                 (exit)
];
#[cfg(target_arch = "riscv64")]
const RECEIVE_STUB: &[u32] = &[
    0x0000_0513,                                             // li a0, 0            (slot 0)
    0x0000_0593 | ((abi::rendezvous::RECEIVE as u32) << 20), // li a1, RECEIVE
    0x0000_0613,                                             // li a2, 0
    0x0000_0693,                                             // li a3, 0
    0x0000_0713,                                             // li a4, 0
    0x0000_0893 | ((abi::SYS_INVOKE as u32) << 20),          // li a7, SYS_INVOKE
    0x0000_0073,                                  // ecall               (RECEIVE: blocks)
    0x0000_0893 | ((abi::SYS_EXIT as u32) << 20), // li a7, SYS_EXIT
    0x0000_0073,                                  // ecall               (exit)
];
/// `x86_64`'s is in `user::x86_programs`; see [`SPIN_STUB`].
#[cfg(target_arch = "x86_64")]
const RECEIVE_STUB: &[u32] = &super::x86_programs::receive();

/// A child that blocks in `CALL` on the rendezvous in slot 0 and is never replied to: [`RECEIVE_STUB`]
/// with `CALL` for `RECEIVE`. Once a server collects the request the caller is on no queue, so no
/// rendezvous sweep reaches it and only the server's one-shot `Reply` capability could wake it.
#[cfg(target_arch = "aarch64")]
const CALL_STUB: &[u32] = &[
    0xD280_0000, // movz x0, #0            (slot 0)
    0xD280_0000 | ((abi::rendezvous::CALL as u32) << 5) | 1, // movz x1, #CALL
    0xD280_0002, // movz x2, #0
    0xD280_0003, // movz x3, #0
    0xD280_0004, // movz x4, #0
    0xD280_0000 | ((abi::SYS_INVOKE as u32) << 5) | 8, // movz x8, #SYS_INVOKE
    0xD400_0001, // svc #0                 (CALL: blocks until replied)
    0xD280_0008, // movz x8, #SYS_EXIT
    0xD400_0001, // svc #0                 (exit)
];
#[cfg(target_arch = "riscv64")]
const CALL_STUB: &[u32] = &[
    0x0000_0513,                                          // li a0, 0            (slot 0)
    0x0000_0593 | ((abi::rendezvous::CALL as u32) << 20), // li a1, CALL
    0x0000_0613,                                          // li a2, 0
    0x0000_0693,                                          // li a3, 0
    0x0000_0713,                                          // li a4, 0
    0x0000_0893 | ((abi::SYS_INVOKE as u32) << 20),       // li a7, SYS_INVOKE
    0x0000_0073,                                          // ecall               (CALL: blocks)
    0x0000_0893 | ((abi::SYS_EXIT as u32) << 20),         // li a7, SYS_EXIT
    0x0000_0073,                                          // ecall               (exit)
];
/// `x86_64`'s is in `user::x86_programs`; see [`SPIN_STUB`].
#[cfg(target_arch = "x86_64")]
const CALL_STUB: &[u32] = &super::x86_programs::call();

/// Build a child from parts in one region, give it `rights` on `ep` in slot 0, run `code`, and
/// hand back its region and its tid. The milestone 133 (ending a permanently blocked thread, and
/// deciding who may) tests below differ only in stub and rights.
///
/// `ep` is not created from `region`, unlike in
/// [`destroy_reclaims_a_region_whose_resident_is_blocked_in_receive`], where the rendezvous sweep
/// wakes the resident: here it belongs to somebody else, so nothing ever wakes the resident.
fn child_blocked_on(
    ep: crate::sched::RendezvousId,
    rights: crate::cap::Rights,
    code: &[u32],
) -> (u64, crate::thread::ThreadId) {
    let region = crate::memory_region::create(16).expect("no region for the blocked child");
    let aspace = user_address_space_create(region).expect("no aspace");

    let code_phys = code_page(region, code);
    user_address_space_map(
        aspace,
        CODE_VA,
        code_phys,
        Flags::user_code(),
        crate::revoke::PageMapSource::NoCapability,
    )
    .expect("map code");

    let stack_phys = crate::memory_region::retype_page(region).expect("no stack frame");
    user_address_space_map(
        aspace,
        STACK_VA,
        stack_phys,
        Flags::user_data(),
        crate::revoke::PageMapSource::NoCapability,
    )
    .expect("map stack");

    let tid = sched::create_thread_control_block(region).expect("no tcb");
    let slot =
        sched::thread_control_block_insert_cap(tid, crate::cap::rendezvous_cap(ep, rights), None)
            .expect("insert the rendezvous");
    assert_eq!(
        slot, 0,
        "the rendezvous must land in slot 0 (every stub here assumes it)"
    );
    sched::configure_thread_control_block(tid, CODE_VA, STACK_VA + page_frames::FRAME_SIZE, aspace)
        .expect("configure");
    sched::start_thread_control_block(tid, [0; 3]).expect("start");
    (region, tid)
}

/// Run the owner's retry loop against `region` for up to two seconds, exactly as
/// `user::holding::Holding` runs it, and say whether it ever reclaimed. Time-bounded rather than
/// spin-count-bounded for DECISIONS §28's reason: the resident may be on another core, and only
/// that core's own tick moves it.
fn reclaim_within_two_seconds(region: u64) -> bool {
    let deadline = crate::arch::timer::now() + 2 * crate::arch::timer::frequency();
    while crate::arch::timer::now() < deadline {
        if sched::reclaim_region(region).is_ok() {
            return true;
        }
        sched::yield_now();
    }
    false
}

/// `DESTROY` reclaims a region whose resident is `Blocked` in `RECEIVE`, not just one that spins.
///
/// §16's kill is armed by the refusal and spent by `schedule()`, which a parked thread never
/// reaches. The child's rendezvous is in the region, so `reap_region_objects`' sweep, which runs
/// before the refusal for this reason, aborts the IPC and wakes it, and the armed kill is spent.
/// See notes/object-revocation.md and notes/frames.md.
///
/// Verified it can fail: moving the rendezvous sweep below the refusal in `reap_region_objects`
/// spends the two-second deadline and trips the reclaim assertion (checked 2026-08-16).
#[test_case]
fn destroy_reclaims_a_region_whose_resident_is_blocked_in_receive() {
    // The child's whole world in one region, the rendezvous it parks on included.
    let region = crate::memory_region::create(16).expect("no region for the blocked child");
    let run = crate::testing::RegionRun::of(region); // this region's frames, not the machine's
    let ep = sched::create_rendezvous_from(region).expect("no rendezvous in the child's region");

    let aspace = user_address_space_create(region).expect("no aspace");
    let code_phys = code_page(region, RECEIVE_STUB);
    user_address_space_map(
        aspace,
        CODE_VA,
        code_phys,
        Flags::user_code(),
        crate::revoke::PageMapSource::NoCapability,
    )
    .expect("map code");
    let stack_phys = crate::memory_region::retype_page(region).expect("no stack frame");
    user_address_space_map(
        aspace,
        STACK_VA,
        stack_phys,
        Flags::user_data(),
        crate::revoke::PageMapSource::NoCapability,
    )
    .expect("map stack");

    let tid = sched::create_thread_control_block(region).expect("no tcb");
    // READ, because the child receives; `supervision_tests::build_child_in` inserts WRITE.
    let slot = sched::thread_control_block_insert_cap(
        tid,
        crate::cap::rendezvous_cap(ep, crate::cap::Rights::READ),
        None,
    )
    .expect("insert the rendezvous");
    assert_eq!(
        slot, 0,
        "the rendezvous must land in slot 0 (the stub assumes it)"
    );
    sched::configure_thread_control_block(tid, CODE_VA, STACK_VA + page_frames::FRAME_SIZE, aspace)
        .expect("configure");
    sched::start_thread_control_block(tid, [0; 3]).expect("start");

    // Wait until it is queued: a child still `Ready` would take the ordinary force-kill path.
    assert!(
        super::wait_for(|| sched::rendezvous_waiting_receivers(ep) == 1),
        "the child never blocked on its rendezvous, so this test would prove the wrong thing",
    );

    // As `user::holding::Holding` runs it: the first pass sweeps the rendezvous (waking the child)
    // and arms the kill; a later pass finds the corpse and reclaims.
    let deadline = crate::arch::timer::now() + 2 * crate::arch::timer::frequency();
    let mut reclaimed = false;
    while crate::arch::timer::now() < deadline {
        if sched::reclaim_region(region).is_ok() {
            reclaimed = true;
            break;
        }
        sched::yield_now();
    }
    assert!(
        reclaimed,
        "a region holding a resident blocked in RECEIVE never reclaimed: the rendezvous sweep no longer \
         runs before the refusal, so the armed kill can never be spent and the memory is gone for \
         the life of the boot",
    );
    assert!(
        super::wait_for(|| !sched::is_thread_present(tid)),
        "the region reclaimed but its blocked resident was never reaped",
    );
    run.assert_returned("reclaiming a blocked resident's region did not return its frames");
}

/// `DESTROY` reclaims a region whose resident is blocked on a rendezvous it does not own
/// (milestone 133, proposal A).
///
/// This is what a hung component is: a resident waiting on somebody else's object, which the
/// region sweep never touches, so nothing wakes it to spend the armed kill. Asserted in order: the
/// region comes back; the child is off the receiver queue (else a freed TCB is still linked, a
/// dangling pointer rather than a leak); every frame returns. See notes/blocked-thread-teardown.md.
///
/// Verified it can fail: reverting `region_reap_verdict`'s `Blocked` arm to
/// `RegionReap::RefuseAndArm` spends the two-second deadline and trips the first assertion.
#[test_case]
fn destroy_reclaims_a_region_whose_resident_blocks_on_a_rendezvous_it_does_not_own() {
    // Somebody else's rendezvous, from a region of the test's own that the child's reclaim cannot
    // reach and the test reclaims last. The accounting names the child's region, so these pages
    // are outside it by construction.
    let endpoints = crate::memory_region::create(1).expect("no endpoint region");
    let ep = sched::create_rendezvous_from(endpoints).expect("no rendezvous");

    let (region, tid) = child_blocked_on(ep, crate::cap::Rights::READ, RECEIVE_STUB);
    let run = crate::testing::RegionRun::of(region);

    // Wait until it is queued: a child still `Ready` proves only the force-kill path.
    assert!(
        super::wait_for(|| sched::rendezvous_waiting_receivers(ep) == 1),
        "the child never blocked on the rendezvous, so this test would prove the wrong thing",
    );

    assert!(
        reclaim_within_two_seconds(region),
        "a region whose resident is blocked on a rendezvous it does not own never reclaimed: the \
         arm can never be spent on a thread that never runs again, so that memory is gone for the \
         life of the boot",
    );
    assert_eq!(
        sched::rendezvous_waiting_receivers(ep),
        0,
        "the resident was reaped while still linked on the rendezvous's receiver queue: the next \
         send follows that link into a freed page",
    );
    assert!(
        super::wait_for(|| !sched::is_thread_present(tid)),
        "the region reclaimed but its blocked resident was never reaped",
    );
    run.assert_returned(
        "reclaiming the region of a resident blocked elsewhere did not return its frames",
    );
    sched::reclaim_region(endpoints).expect("the endpoint region did not come back");
}

/// A `Reply` capability naming a torn-down caller does not survive the teardown (milestone 133's
/// security half).
///
/// `Object::Reply(tid)` carries no call identity, so a stale one could forge an answer to a later
/// `CALL`. Proposal A never wakes the caller and also sweeps the capability (seL4's two fixes);
/// this pins the sweep, which a change could drop while every capacity assertion passed. The
/// caller is collected, on no queue at all. See notes/blocked-thread-teardown.md, "A hazard".
///
/// Verified it can fail: deleting the `delete_matching` sweep from `sched::finish_blocked_resident`
/// trips the middle assertion while the reclaim still succeeds.
#[test_case]
fn tearing_down_a_reply_parked_caller_sweeps_the_reply_capability() {
    // From a region of the test's own, reclaimed last, outside what the test accounts for.
    let endpoints = crate::memory_region::create(1).expect("no endpoint region");
    let ep = sched::create_rendezvous_from(endpoints).expect("no rendezvous");

    // WRITE, because the child calls on it. This test is the server.
    let (region, tid) = child_blocked_on(ep, crate::cap::Rights::WRITE, CALL_STUB);
    let run = crate::testing::RegionRun::of(region); // this region's frames, not the machine's

    // Collecting the request leaves the caller reply-parked on nothing. `ipc_receive_cap` does not
    // wake a caller: only the reply may, and this server never sends one.
    let [_word, slot, _w1, _badge, ..] = sched::ipc_receive_cap(ep);
    assert_ne!(
        slot,
        abi::rendezvous::NO_CAP,
        "the CALL carried no reply capability"
    );
    assert!(
        matches!(
            sched::current_cap(slot).expect("the reply capability was not in its slot").object,
            crate::cap::Object::Reply(named) if named == tid,
        ),
        "the collected capability was not a Reply naming this caller",
    );
    assert_eq!(
        sched::rendezvous_waiting_senders(ep),
        0,
        "a collected caller must be off the sender queue, or this test stages the easy case",
    );

    assert!(
        reclaim_within_two_seconds(region),
        "a region whose resident is reply-parked never reclaimed",
    );

    // The sweep: this thread's `Reply` named a thread that no longer exists, and is gone.
    assert!(
        sched::current_cap(slot).is_err(),
        "a stale Reply capability outlived the caller it names: a hung or compromised server can \
         invoke it against whatever thread later inherits that name",
    );

    assert!(
        super::wait_for(|| !sched::is_thread_present(tid)),
        "the region reclaimed but its reply-parked resident was never reaped",
    );
    run.assert_returned("reclaiming a reply-parked resident's region did not return its frames");
    sched::reclaim_region(endpoints).expect("the endpoint region did not come back");
}

/// A region lent to an address space is freed by its owner, and by nobody else.
///
/// The deterministic half of the intermittent riscv64 double free in notes/object-revocation.md
/// ("Two names for one region"), staged by hand. `sched::reclaim_region` reaps under `IPC_TABLES`, unpins, destroys; the
/// reaper drops a dead space after releasing `IPC_TABLES` (its `Drop` takes it), so unpin and drop
/// are unordered. Asserted: dropping a space built from a lent region returns nothing. Against the
/// old code that fails an assertion rather than needing two `destroy` calls to overlap.
#[test_case]
fn an_address_space_never_frees_a_region_it_was_lent() {
    // Four pages is enough for a root and one table; nothing is mapped here, so the space is only
    // ever asked who owns its memory.
    let region = crate::memory_region::create(4).expect("no region for the lent-backing test");
    // These four frames, by name: another core's late reap cannot touch them, as it can a
    // machine-wide free count. `RegionRun::of` also asserts the region cost exactly its 4 pages.
    let run = crate::testing::RegionRun::of(region);
    let name = user_address_space_create(region).expect("no address space from the region");

    // Out of the registry, as a dead thread's reaper takes it (§249 (a running address space stays
    // nameable)): an owned value whose `Drop` is under test.
    let space = take_user_address_space(name).expect("the space was not in the registry");

    // `reclaim_region`'s unpin, arriving before the drop: the whole race.
    crate::memory_region::unpin(region);
    drop(space);

    run.assert_held(
        "dropping an address space returned a region it was only lent: the region's real owner \
         still has a name for those pages, and its `DESTROY` frees every one of them a second time",
    );

    // And the owner's reclaim still works, returning the run exactly once.
    crate::memory_region::destroy(region);
    run.assert_returned("the region's owner could not return a run its borrower had let go of");
}
