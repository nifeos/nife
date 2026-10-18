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
    // These sixteen frames, by name. The window below spans a runaway spinning on whichever core
    // §28 (SMP placement: two random choices at spawn) placed it on, a kill converted by that
    // core's own tick, and up to a second of yielding: ample room for a neighbouring test's
    // teardown to land inside a machine-wide free-frame delta. See `testing::RegionRun` and
    // notes/load-sensitive-assertions.md.
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

    // Let the runaway actually reach EL0 and start spinning, so we tear down a running thread,
    // not an embryo. A few yields is plenty; it is preemptible the instant it lands.
    for _ in 0..8 {
        sched::yield_now();
    }

    // The forcible tier: reclaim the region while the runaway is still live. The first pass arms
    // the kill and refuses; the runaway is converted to a corpse at its next preemption; the
    // retry reclaims. The wait is time-based, not a fixed spin count, because since DECISIONS §28
    // the runaway may be placed on another core, where only that core's own timer tick converts
    // it (the kill is bounded by the tick, §28.3 / §16). A tight yield loop on this core would
    // finish inside one 10 ms tick and never give the remote core a chance; a one-second deadline
    // spans ~100 ticks, ample, while still failing a real bug rather than hanging the emulator.
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

/// A child that blocks in `CALL` on the rendezvous in slot 0 and is never replied to. Byte for
/// byte [`RECEIVE_STUB`] with `CALL` where the `RECEIVE` was, and the difference in the machine is the
/// whole of milestone 133's harder case: once a server has collected the request, the caller is on
/// **no queue at all**, so no rendezvous sweep can reach it and the only thing that could wake it
/// is the one-shot `Reply` capability the server is now holding.
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
/// hand back its region and its tid. The three milestone 133 tests below differ only in the stub
/// and the rights, and the twenty lines of address-space assembly they share had already been
/// copied twice in this file before it was worth lifting.
///
/// The rendezvous is deliberately **not** created from `region`: that is the difference between
/// the case [`destroy_reclaims_a_region_whose_resident_is_blocked_in_receive`] already covered (the
/// rendezvous sweep wakes the resident, so the armed kill becomes spendable) and the case that
/// hung forever until milestone 133, ending a permanently blocked thread: the rendezvous belongs to
/// somebody else, so nothing wakes the resident and nothing ever will.
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

/// **`DESTROY` reclaims a region whose resident is `Blocked`, not just one that spins.**
///
/// The companion to the test above and the property the aarch64 test boot actually needed. §16's
/// kill is *armed* by the refusal and *spent* by `schedule()`, and a thread parked in `RECEIVE` never
/// reaches `schedule()`: before 2026-08-16 a region holding a blocked server was refused on every
/// pass, forever, and its memory was gone until the machine stopped. That is not an exotic case. A
/// server is a thing that blocks, and the `spawn_elf_child_builder` tests each built one out of a
/// 2048-frame budget, which is how the boot came to end with 216 free frames (notes/frames.md).
///
/// What makes the reclaim land is that the child's rendezvous is **in the region being reclaimed**, so
/// the sweep that removes it aborts the child's IPC and wakes it, and the kill the same pass arms is
/// then spendable. `reap_region_objects` does that sweep before the refusal for exactly this reason.
///
/// **Verified it can fail**, and it was: moving the rendezvous sweep back below the refusal in
/// `reap_region_objects` makes this test spend its whole two-second deadline and trip its own
/// assertion, "a region holding a resident blocked in RECEIVE never reclaimed" (checked 2026-08-16).
#[test_case]
fn destroy_reclaims_a_region_whose_resident_is_blocked_in_receive() {
    // The child's whole world in one region, its rendezvous included: address space, code, stack, TCB, and
    // the rendezvous it will park on.
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
    // READ, because the child receives on it. The rights are the point of not reusing
    // `supervision_tests::build_child_in`, which inserts a WRITE report cap.
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

    // **Wait for it to be queued on the rendezvous, not for "probably scheduled by now."** A test that
    // reclaims before the child has parked proves nothing at all: the child would still be Ready, and
    // the ordinary force-kill path above would carry it. See the same lesson in
    // `a_blocked_waiter_wakes_with_an_error_when_its_rendezvous_is_revoked`.
    assert!(
        super::wait_for(|| sched::rendezvous_waiting_receivers(ep) == 1),
        "the child never blocked on its rendezvous, so this test would prove the wrong thing",
    );

    // The retry loop, as `user::holding::Holding` runs it: the first pass sweeps the rendezvous (which
    // wakes the child) and arms the kill; a later pass finds the corpse and reclaims.
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

/// **`DESTROY` reclaims a region whose resident is blocked on a rendezvous it does not own**
/// (milestone 133, proposal A). **This is the test that hung before this milestone**, and the
/// difference from its sibling above is one line: the rendezvous is created from the kernel's own
/// pinned region rather than from the child's, so the sweep that opens `reap_region_objects` never
/// touches it and the child is never woken by anything.
///
/// That is not an exotic arrangement, it is what a hung component *is*. A server parks in `RECEIVE`
/// on a rendezvous its clients own; a client parks in `CALL` on a server's. Either way the
/// resident waits on somebody else's object, §16's kill is armed by the refusal and spent by
/// `schedule()` only for a thread that is `Running`, and a thread waiting for a rendezvous nobody
/// will ever complete does not become `Running` again. So the arm was armed and never landed, the
/// refusal was permanent, and **the region was gone for the life of the machine.** No privilege
/// fixed it, because it was a scheduler property rather than an authorization one.
///
/// The three assertions are the milestone, in order: the region comes back, the child is off the
/// rendezvous's receiver queue (so the reclaim did not free a TCB that a queue still pointed at,
/// which is the failure mode that would be a dangling pointer rather than a leak), and every frame
/// returns to baseline.
///
/// **Verified it can fail**: reverting `region_reap_verdict`'s `Blocked` arm to
/// `RegionReap::RefuseAndArm` makes this spend its whole two-second deadline and trip its own
/// first assertion.
#[test_case]
fn destroy_reclaims_a_region_whose_resident_blocks_on_a_rendezvous_it_does_not_own() {
    // Somebody else's rendezvous is the whole point: it is not created from `region`, so the sweep
    // that opens `reap_region_objects` never touches it and nothing ever wakes the child. It comes
    // out of a region of the test's own, which the child's reclaim cannot reach, and which the
    // test reclaims last.
    //
    // Creating it before a machine-wide free-frame baseline used to be load-bearing, and sampling
    // in the other order cost this test two runs at a deterministic 32 frames. The accounting
    // below names the child's own region instead, so the rendezvous's pages are outside the
    // measurement by construction rather than by ordering.
    let endpoints = crate::memory_region::create(1).expect("no endpoint region");
    let ep = sched::create_rendezvous_from(endpoints).expect("no rendezvous");

    let (region, tid) = child_blocked_on(ep, crate::cap::Rights::READ, RECEIVE_STUB);
    let run = crate::testing::RegionRun::of(region);

    // **Wait for it to be queued, not for "probably scheduled by now."** Reclaiming a child that
    // is still `Ready` proves the old force-kill path and nothing about this one.
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

/// **A `Reply` capability naming a torn-down caller does not survive the teardown**, which is the
/// half of milestone 133 that is a security property rather than a capacity one.
///
/// `cap::reply_cap` mints `Object::Reply(tid)` whose payload is a generational thread name with
/// **no call identity**, and `sched::ipc_reply`'s guard checks the caller's `WaitRole` and discards
/// the rendezvous. That is sound only while nothing can leave a reply park and enter a second
/// `CALL` with an unconsumed `Reply` still naming it. A design that freed a stranded caller by
/// *waking* it would create exactly that path, and a hung server's stale `Reply` would then pass
/// the role check and forge an answer to a later, unrelated conversation. `L4Re` documents the
/// identical hazard as a consequence of its own finite receive timeouts and Zircon documents it
/// for `zx_channel_call`'s timeout; see notes/blocked-thread-teardown.md.
///
/// **Proposal A takes both of seL4's fixes rather than either.** The caller is never woken, which
/// is `ThreadState_Inactive` and means no second `CALL` can exist to be forged against; and the
/// outstanding capability is swept anyway, which is `cteDeleteOne(callerCap)` from `cancelIPC`.
/// This test pins the sweep, because the sweep is the half that a later change could quietly drop
/// while every capacity assertion above went on passing. **A change that traded a permanent block
/// for a forgeable reply would be a worse defect than the one it fixes**, and a test that asserted
/// only the reclaim would not notice.
///
/// The caller staged here is the hardest shape there is: its request was collected, so it sits on
/// **no queue at all** and no rendezvous sweep could reach it.
///
/// **Verified it can fail**: deleting the `delete_matching` sweep from
/// `sched::finish_blocked_resident` leaves the slot occupied and trips the middle assertion, while
/// the reclaim itself still succeeds.
#[test_case]
fn tearing_down_a_reply_parked_caller_sweeps_the_reply_capability() {
    // Out of a region of the test's own, reclaimed last, so it is outside what this test accounts
    // for. See the test above.
    let endpoints = crate::memory_region::create(1).expect("no endpoint region");
    let ep = sched::create_rendezvous_from(endpoints).expect("no rendezvous");

    // WRITE, because the child calls on it. This test is the server.
    let (region, tid) = child_blocked_on(ep, crate::cap::Rights::WRITE, CALL_STUB);
    let run = crate::testing::RegionRun::of(region); // this region's frames, not the machine's

    // Collect the request, which is what moves the caller off the sender queue and leaves it
    // reply-parked on nothing. `ipc_receive_cap` deliberately does not wake a caller: the reply is
    // the only thing that may, and this server never sends one.
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

    // **The sweep.** The server (this thread) held a live `Reply` naming a thread that no longer
    // exists; after the teardown it holds nothing, so there is no capability left to forge with.
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

/// **A region lent to an address space is freed by its owner, and by nobody else.**
///
/// The deterministic half of the intermittent `double free of frame 0x82a3e000` that
/// [`destroy_reclaims_a_region_whose_resident_is_blocked_in_receive`] hit once in 45 runs on riscv64
/// (notes/object-revocation.md BUGS). That test needs two cores to disagree; this one needs
/// nobody, because it stages the window by hand instead of racing for it.
///
/// The window, in the order the machine takes it. `sched::reclaim_region` reaps the region's
/// threads under `IPC_TABLES`, then unpins, then destroys. The reaper (`sched::finish_switch`) takes a
/// dead thread's address space out of the table under `IPC_TABLES`, **releases the lock**, and only then
/// drops it, because `AddressSpace::drop` runs a revocation sweep that takes `IPC_TABLES` itself. So the
/// unpin and the drop are unordered, and the old code's safety argument (the pin refuses the
/// borrower's `destroy`) is exactly the thing the unpin has already withdrawn.
///
/// What this asserts is the property that makes the ordering irrelevant: **dropping a space built
/// from a lent region returns nothing.** Called on the old code it fails on its own assertion
/// rather than panicking in the allocator, which is the difference between a regression gate and a
/// coin flip: the double free needs the two `memory_region::destroy` calls to also overlap, and no test
/// can schedule that.
#[test_case]
fn an_address_space_never_frees_a_region_it_was_lent() {
    // Four pages is enough for a root and one table; nothing is mapped here, so the space is only
    // ever asked who owns its memory.
    let region = crate::memory_region::create(4).expect("no region for the lent-backing test");
    // **These four frames, by name, and that is what retires the settling loop that stood here.**
    // This test used to sample the machine's free-frame count until two reads a yield apart
    // agreed, because an earlier test's thread can be reaped from another core a beat after it
    // leaves the table and its space's region freed inside this window. The wait was real work
    // done to make a global counter usable; asking about this region's own run removes the need
    // for it, since nothing any other core does can touch these four bits. `RegionRun::of` also
    // asserts all four are used, which is the "the region should have cost exactly its 4 pages"
    // claim the subtraction used to make.
    let run = crate::testing::RegionRun::of(region);
    let name = user_address_space_create(region).expect("no address space from the region");

    // Out of the registry, exactly as a dead thread's reaper takes it (§249 (a running address
    // space stays nameable)): from here the space is an owned value whose `Drop` is the thing under
    // test, which is the shape the reaper holds it in.
    let space = take_user_address_space(name).expect("the space was not in the registry");

    // `reclaim_region`'s unpin, arriving BEFORE the drop. This one line is the whole race.
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
