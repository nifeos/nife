//! `address_space_tests`: address spaces: ASID tagging and the cross-CPU flush, a read-only
//! segment mapped read-only, a dead thread's space freed whole, and a space built by user code.
//!
//! Name: provisional, minted 2026-10-12 (UTC) by the lane for milestone 873 (the catch-all
//! userspace test file is split by topic).

use core::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, AtomicUsize, Ordering};

use super::test_programs::{a_kernel_address, kernel_test_subject_image, loader_subject_image};
use super::*;
use crate::arch::exceptions::USER_FAULTS;
use crate::arch::timer;
use crate::sched;

/// **The milestone 15 witness: address spaces stay apart with NO flush on switch.**
///
/// Two spaces map the *same* virtual address to different frames holding different bytes. The test
/// installs A and reads through the VA, loading A's ASID-tagged translation into the TLB, then
/// installs B, which flushes nothing, and reads again. If user mappings were global, or the ASID
/// did not ride the root register, or two spaces shared a tag, B's read would hit A's cached entry
/// and see A's byte: one process reading another's memory. The assertion means the same on aarch64
/// and RISC-V only because neither context switch flushes (DECISIONS §19 (architectural parity is a
/// tenet)); see notes/system-test-modules/address-space-tests.md.
#[test_case]
fn asid_tagging_keeps_address_spaces_apart_without_flushes() {
    let mut a = AddressSpace::new(2).expect("no space A");
    let mut b = AddressSpace::new(2).expect("no space B");

    let (asid_a, asid_b) = (mmu::asid_of(a.ttbr0()), mmu::asid_of(b.ttbr0()));

    // An architecture that does not tag cannot be tested for tagging: B's read would return B's
    // byte because nothing was cached. x86_64 runs with `CR4.PCIDE` clear, so `CR3[11:0]` is
    // reserved-zero, `arch::x86_64::mmu::ttbr0_value` drops the number and every `mov cr3` flushes
    // the whole TLB. Turning `PCIDE` on is an architect's call (item 3 of milestone 161 (the x86_64
    // kernel port)); once it is on, this skip stops firing with nothing else edited.
    if asid_a == 0 && asid_b == 0 {
        crate::testing::skip!(
            "this machine does not tag address spaces (x86 runs with CR4.PCIDE clear, so every \
             root switch flushes the whole TLB and this test could not fail for its stated reason)"
        );
    }

    assert_ne!(asid_a, asid_b, "two live spaces share an ASID");
    assert_ne!(asid_a, 0, "a user space got the kernel's ASID 0");
    assert_ne!(asid_b, 0, "a user space got the kernel's ASID 0");

    const VA: u64 = address_space_map::pair_page(0x40_0000);
    a.map_new(VA, Flags::user_data()).expect("map A")[0] = 0xAA;
    b.map_new(VA, Flags::user_data()).expect("map B")[0] = 0xBB;

    // Masked across the whole sequence: a preemption between an `activate_user` and its read would
    // put the reserved root back and turn `VA` into a kernel fault, which is a flaky test rather
    // than a finding. The window is a handful of instructions, so this is cheap insurance.
    let was_enabled = crate::arch::interrupts::disable();
    // And the ISA difference that kept this test on one architecture as surely as the flush did:
    // RISC-V's S-mode may not touch a `U` page unless `sstatus.SUM` says so, where EL1 may.
    let could_reach_user = mmu::permit_kernel_access_to_user_pages(true);

    // SAFETY: nothing is at EL0; we are a kernel thread mid-test, and each space outlives
    // its activation. The reads go through the live user translation, which is the point.
    let (read_a, read_b) = unsafe {
        mmu::activate_user(a.ttbr0());
        let ra = core::ptr::read_volatile(VA as *const u8);
        // Flushes NOTHING: milestone 15 on aarch64, milestone 58 on RISC-V.
        mmu::activate_user(b.ttbr0());
        let rb = core::ptr::read_volatile(VA as *const u8);
        mmu::deactivate_user();
        (ra, rb)
    };

    mmu::permit_kernel_access_to_user_pages(could_reach_user);
    crate::arch::interrupts::restore(was_enabled);

    assert_eq!(read_a, 0xAA);
    assert_eq!(
        read_b, 0xBB,
        "B read A's byte: a stale TLB entry crossed address spaces, so the tagging is broken \
         (aarch64: the nG bit and TTBR0's ASID; RISC-V: a non-global PTE and satp.ASID)",
    );
}

/// **An ASID flush reaches the other cores.** RISC-V's `sfence.vma` is local, so `flush_asid` must
/// IPI the other harts through SBI RFENCE, where aarch64's `tlbi aside1is` broadcasts. A probe on
/// another core caches `VA`'s translation; this core moves `VA` to a new frame with no per-address
/// invalidation and calls `flush_asid`; the probe re-reads, space still installed, and must see the
/// new frame. See notes/system-test-modules/address-space-tests.md.
///
/// # BUGS
///
/// - The probe is placed, not pinned, so the test asserts only that it ran on another core.
/// - A remote core that never services the shootdown fails as a handshake timeout (`gave_up`).
#[test_case]
fn an_asid_flush_reaches_the_other_cpus() {
    const VA: u64 = address_space_map::pair_page(0x40_0000);
    const OLD: u8 = 0xAA;
    const NEW: u8 = 0xBB;

    let handshake = &ASID_FLUSH_PROBE;
    handshake.stage.store(0, Ordering::SeqCst);
    handshake.probe_cpu.store(usize::MAX, Ordering::SeqCst);
    handshake.gave_up.store(false, Ordering::SeqCst);

    // **A skip rather than an assert** (milestone 161). Both `virt` boards run this leg at `-smp 4`,
    // so two cores was a fact rather than a hope for as long as there were two architectures; q35
    // runs at one, because x86 SMP is INIT-SIPI-SIPI and that is roadmap item 5. `smp.rs`'s
    // cross-core placement test already says this in these words, and one machine fact should not
    // read as a bug in one file and as a missing fixture in another.
    if crate::smp::online_count() < 2 {
        crate::testing::skip!("a cross-core TLB shootdown test needs at least two online cores");
    }
    let here = crate::cpu::id();
    // Any ONLINE core but this one, from the set rather than `0..count`: on a non-contiguous online
    // set the range picks a parked core, the probe never runs, and the test times out reading as a
    // TLB bug.
    let target = crate::smp::online_cpus()
        .find(|&c| c != here)
        .expect("cores >= 2");

    let mut space = AddressSpace::new(2).expect("no address space");
    space.map_new(VA, Flags::user_data()).expect("map")[0] = OLD;
    let installed = space.ttbr0();
    let asid = mmu::asid_of(installed);

    // The frame `VA` will be moved onto, written through the direct map because the kernel cannot
    // name a user VA. It comes from the general allocator rather than the space's own region, so the
    // space never owns it and its teardown cannot free it out from under us.
    let fresh = crate::memory::alloc().expect("out of memory");
    // SAFETY: the allocator just handed us this frame exclusively; the direct map reaches it.
    unsafe { core::ptr::write_volatile(mmu::phys_to_virt(fresh.addr()) as *mut u8, NEW) };

    let probe = sched::spawn_on(target, move || {
        probe_across_the_flush(handshake, installed, VA);
    })
    .expect("spawn_on failed");

    assert!(
        spin_until(|| handshake.stage.load(Ordering::SeqCst) >= 1),
        "the probe never cached the translation on core {target}",
    );

    remap_with_no_invalidation(&space, VA, fresh.addr());

    mmu::flush_asid(asid);
    handshake.stage.store(2, Ordering::SeqCst);

    assert!(
        spin_until(|| handshake.stage.load(Ordering::SeqCst) == 3),
        "the probe never finished its second read",
    );
    assert!(
        !handshake.gave_up.load(Ordering::SeqCst),
        "the probe timed out waiting for the remap: the handshake, not the TLB, is broken",
    );
    assert_ne!(
        handshake.probe_cpu.load(Ordering::SeqCst),
        here,
        "the probe ran on this core, so a local flush would have covered it and the test proves \
         nothing about the shootdown",
    );

    assert_eq!(
        handshake.seen_before.load(Ordering::SeqCst),
        OLD,
        "the probe did not read the original mapping, so it never cached the entry this test is \
         about",
    );
    assert_eq!(
        handshake.seen_after.load(Ordering::SeqCst),
        NEW,
        "STALE TLB ON ANOTHER CORE: core {} still translates {VA:#x} to the frame this space \
         stopped using, after a flush of its ASID. On RISC-V that means the SBI RFENCE never \
         reached it (sfence.vma is local); reuse the number and the next address space reads this \
         one's memory.",
        handshake.probe_cpu.load(Ordering::SeqCst),
    );

    assert!(
        wait_for(|| !sched::is_thread_present(probe)),
        "the probe thread was never reaped",
    );
    drop(space);
    crate::memory::free(fresh);
}

/// The handshake [`an_asid_flush_reaches_the_other_cpus`] runs between this core and its probe.
struct AsidFlushProbe {
    /// 0 = starting, 1 = the probe has cached the translation, 2 = the remap and sweep are done,
    /// 3 = the probe has re-read. `SeqCst` throughout: this is a handshake between two cores whose
    /// whole purpose is ordering, and shaving it would be optimizing the thing under test.
    stage: AtomicUsize,
    seen_before: AtomicU8,
    seen_after: AtomicU8,
    probe_cpu: AtomicUsize,
    gave_up: AtomicBool,
}

static ASID_FLUSH_PROBE: AsidFlushProbe = AsidFlushProbe {
    stage: AtomicUsize::new(0),
    seen_before: AtomicU8::new(0),
    seen_after: AtomicU8::new(0),
    probe_cpu: AtomicUsize::new(usize::MAX),
    gave_up: AtomicBool::new(false),
};

/// Spin (never yield) until `done`, so this core stays busy and cannot steal the probe.
fn spin_until(mut done: impl FnMut() -> bool) -> bool {
    let deadline = timer::now() + 5 * timer::frequency();
    while timer::now() < deadline {
        if done() {
            return true;
        }
        core::hint::spin_loop();
    }
    done()
}

/// The probe's whole life on the other core: install the space, read `va`, wait for the remap and
/// the sweep, and read it again, reporting both bytes and where it ran through `handshake`.
fn probe_across_the_flush(handshake: &AsidFlushProbe, installed: u64, va: u64) {
    // Masked for the whole body. Two reasons, and the first is the one that matters: a
    // preemption would migrate this thread and split the two reads across cores, and it would
    // also swap the address space out from under them. This does NOT block the shootdown:
    // RISC-V's SBI RFENCE arrives as an M-mode software interrupt, which S-mode masking does not
    // touch, and aarch64's `tlbi` needs no interrupt at all. If that were wrong, this test would
    // hang rather than lie, which is the right way round.
    let was_enabled = crate::arch::interrupts::disable();
    let could_reach_user = mmu::permit_kernel_access_to_user_pages(true);
    handshake
        .probe_cpu
        .store(crate::cpu::id(), Ordering::SeqCst);

    // **The space stays installed across both reads**, which is deliberate and is the realistic
    // shape: the hazard is a core actively running an address space whose mapping changes under
    // it. It also keeps the measurement clean, because a `satp` write between the reads is a
    // second event an emulator or a core is entitled to treat as a flush, and then the second
    // read would be correct for a reason that has nothing to do with the shootdown.
    //
    // SAFETY: nothing is at user mode on this core; we are a kernel thread with interrupts
    // masked, and the space outlives this thread (the test waits for the reap before dropping
    // it). The space's root carries the kernel high half, so this core's own code and stack stay
    // mapped throughout.
    unsafe { mmu::activate_user(installed) };

    // SAFETY: `VA` is mapped user-readable in the installed space, and this ISA has been told
    // that the kernel may read a user page.
    handshake.seen_before.store(
        unsafe { core::ptr::read_volatile(va as *const u8) },
        Ordering::SeqCst,
    );
    handshake.stage.store(1, Ordering::SeqCst);

    let deadline = timer::now() + 5 * timer::frequency();
    while handshake.stage.load(Ordering::SeqCst) < 2 {
        if timer::now() > deadline {
            handshake.gave_up.store(true, Ordering::SeqCst);
            break;
        }
        core::hint::spin_loop();
    }

    // SAFETY: as above. Same address, same installed space, no `satp` write in between.
    handshake.seen_after.store(
        unsafe { core::ptr::read_volatile(va as *const u8) },
        Ordering::SeqCst,
    );

    mmu::deactivate_user();
    mmu::permit_kernel_access_to_user_pages(could_reach_user);
    handshake.stage.store(3, Ordering::SeqCst);
    crate::arch::interrupts::restore(was_enabled);
}

/// Move `va` in `space` onto `frame` with NO per-address invalidation, so `flush_asid` is the only
/// announcement in the system and the test measures it alone.
fn remap_with_no_invalidation(space: &AddressSpace, va: u64, frame: u64) {
    // SAFETY: `space.root()` is this live space's low-half root, and the direct map makes
    // `phys_to_ptr` valid for its tables. `|| None`: the tables reaching `VA` already exist, so
    // the mapper never allocates.
    let mut mapper =
        unsafe { Mapper::<_, _, mmu::Format>::new(space.root(), Half::Low, || None, phys_to_ptr) };
    let (old, flush) = mapper.unmap(va).expect("VA was not mapped");
    assert_ne!(
        old, frame,
        "the allocator handed back the frame we just unmapped: nothing would change"
    );
    mapper
        .map(va, frame, Flags::user_data())
        .expect("remap onto the fresh frame");

    // Discharged with a no-op ON PURPOSE, and this is the only place in the tree that does it.
    // The real per-address flush (`arch::mmu::flush_tlb`) broadcasts too, so using it here would
    // make the test pass whatever `flush_asid` did. `assume_no_stale_entry` would be a lie: a
    // stale entry is precisely what we arranged.
    flush.flush(|_| {});
}

/// The loader honours the file's permissions, and does not widen them.
///
/// An ELF's `.rodata` segment is `PF_R` alone. The tempting shortcut is to map every
/// non-executable segment as `user_data()`, which is **writable**: quietly granting the
/// program authority its own file never asked for.
#[test_case]
fn a_read_only_segment_is_mapped_read_only() {
    let image = loader_subject_image();
    let (space, _) = load(image, 0).expect("the initial ramdisk did not load");

    let rodata = elf::Elf::parse(image)
        .unwrap()
        .segments()
        .find(|s| s.is_readable() && !s.is_writable() && !s.is_executable())
        .expect("the test binary has no read-only segment");

    // Install it so we can ask the CPU's own tables, rather than our record of them.
    space.while_installed(|| {
        let (_, flags) = mmu::translate_user(rodata.vaddr).expect(".rodata is not mapped at all");

        assert!(
            flags.is_user_accessible(),
            "EL0 cannot read its own .rodata"
        );
        assert!(!flags.is_writable(), "the loader made .rodata WRITABLE");
        assert!(!flags.is_user_executable(), ".rodata is executable at EL0");
        assert!(
            !flags.is_kernel_executable(),
            ".rodata is executable at EL1"
        );
    });
    drop(space);
}

/// **A space installed with `while_installed` stays installed however many ticks its body spans.**
///
/// A context switch back to a kernel thread installs the kernel's reserved root, so a preemption
/// between the install and `translate_user`'s walk would take the space away under the body;
/// `while_installed` holds interrupts off. The test makes that preemption certain: a competitor is
/// queued on this core inside the window, and the body spans five ticks. See
/// notes/system-test-modules/address-space-tests.md.
///
/// Name: provisional, lane maintainer/map-physical-flake, 2026-10-13 (UTC).
///
/// Falsification: replayable `system_tests/falsifications/user.address_space_tests.an_installed_space_stays_installed_across_a_timer_tick.patch`
#[test_case]
fn an_installed_space_stays_installed_across_a_timer_tick() {
    const VA: u64 = address_space_map::pair_page(0x0000_0000_0060_0000);
    // Windows tried before the test gives up on its precondition. One is enough in every run seen;
    // a window can open without a queued competitor only if a steal raced the repair below.
    const ATTEMPTS: usize = 3;
    static STOP: AtomicBool = AtomicBool::new(false);
    static DONE: AtomicU64 = AtomicU64::new(0);
    STOP.store(false, Ordering::SeqCst);
    DONE.store(0, Ordering::SeqCst);

    // A competitor never yields, so once it shares a core with this thread it stays queued there
    // whenever this thread runs, and a tick in the window switches to it.
    fn compete() {
        while !STOP.load(Ordering::Relaxed) {
            core::hint::spin_loop();
        }
        DONE.fetch_add(1, Ordering::SeqCst);
    }
    let cores = crate::smp::online_cpus().count() as u64;
    let mut spawned = 0u64;
    for target in crate::smp::online_cpus() {
        sched::spawn_on(target, compete).expect("spawn failed");
        spawned += 1;
    }

    let mut space = AddressSpace::new(2).expect("no address space");
    let frame = crate::memory::alloc().expect("no frame").addr();
    space
        .map_physical(
            VA,
            frame,
            Flags::user_data(),
            crate::revoke::PageMapSource::NoCapability,
        )
        .expect("map failed");

    let mut seen = None;
    let mut queued_at_open = [0usize; ATTEMPTS];
    for queued in queued_at_open.iter_mut() {
        // **Repair the placement.** A steal moves a queued thread only to an idle core, so once every
        // core holds a spinner the layout is fixed; until then this core's competitor can be taken.
        // An empty queue here is answered with a competitor spawned onto this core, which lands on
        // its own run queue. Each one a steal takes makes an idle core busy for good, so at most one
        // per core is ever needed: the cap is a bound, not a budget anyone expects to reach.
        let resident = wait_for(|| {
            if crate::cpu::current().runq_len() > 0 {
                return true;
            }
            if spawned < 2 * cores {
                sched::spawn_on(crate::cpu::id(), compete).expect("spawn failed");
                spawned += 1;
            }
            false
        });
        if !resident {
            continue;
        }
        // The first read inside the window is the precondition. With interrupts masked (the fix),
        // nothing can change this core's run queue until the uninstall: a remote placement goes to
        // the inbox, and a steal is served from the reschedule interrupt. Without the fix it is the
        // queue the first tick is preempted against.
        let (depth, pa) = space.while_installed(|| {
            let depth = crate::cpu::current().runq_len();
            timer::spin_for(5 * timer::interval());
            (depth, mmu::translate_user(VA).map(|(pa, _)| pa))
        });
        *queued = depth;
        if depth > 0 {
            seen = Some(pa);
            break;
        }
    }

    STOP.store(true, Ordering::SeqCst);
    assert!(
        wait_for(|| DONE.load(Ordering::SeqCst) == spawned),
        "the competitors never stopped",
    );
    drop(space);
    crate::memory::free(PageFrame::from_addr(frame));

    let Some(seen) = seen else {
        panic!(
            "no window opened with another thread queued on this core ({ATTEMPTS} tried, queue \
             depths at the window's start {queued_at_open:?}, {spawned} competitors spawned for \
             {cores} cores), so the test proved nothing"
        );
    };
    assert_eq!(
        seen,
        Some(frame),
        "the space was uninstalled under the body: a tick switched away and back, and the switch \
         back put the kernel's reserved root in its place",
    );
}

/// A dead user thread's address space is freed, all of it, including its page tables.
///
/// The milestone 6 reaper test found that stack VAs were bump-allocated and never reused,
/// because `unmap_page` leaves intermediate tables standing. An `AddressSpace` sidesteps
/// that entirely: it dies **all at once**, so it never unmaps anything. It records every
/// frame the mapper hands it, leaves and tables alike, and frees the lot.
///
/// The assertion is exact in the direction this test owns: a leaked frame keeps `used` above
/// the baseline forever and fails. Approximate there would have hidden the milestone 6 bug.
#[test_case]
fn a_dead_user_thread_frees_its_whole_address_space() {
    let used = || crate::memory::stats().expect("no allocator").used;

    // Warm up first: the first user thread pays for page tables in untouched kernel VA, and the
    // steady state is what must hold. Snapshot the fault count BEFORE each spawn, or a fast fault
    // lands in the gap and the wait times out. Subjects go to THIS core (DECISIONS §28 scatters
    // `spawn`): a thread's frames are freed by `finish_switch` on whichever core reaps it, after it
    // leaves the thread table and outside IPC_TABLES, so only on this core do fault, reap and free
    // happen under the test's own yields and keep `used()` exact. This tests the reaper, not
    // placement.
    let here = crate::cpu::id();
    let image = kernel_test_subject_image();
    let kernel_addr = a_kernel_address();
    let kernel_test_subject_here = move || {
        sched::spawn_on(here, move || {
            run(
                image,
                Spawn {
                    arg0: KERNEL_TEST_SUBJECT_READ_KERNEL,
                    arg1: kernel_addr,
                    arg2: 0,
                    grants: &[],
                    maps: &[],
                },
            )
        })
    };

    // Each `kernel_test_subject` instance's reap is proven by `is_thread_present` on ITS ThreadId,
    // not by `thread_count()` returning to a baseline sampled at the top of the test. The count is
    // the whole table, so a baseline taken while an earlier test's teardown is still in flight is a
    // number the system moves on its own; the per-ThreadId wait is immune to neighbours by
    // construction. Same fix as the reap wait in
    // `reclaim_frees_a_started_then_exited_childs_regions`; see notes/load-sensitive-assertions.md.
    let f0 = USER_FAULTS.load(Ordering::Relaxed);
    let warmup = kernel_test_subject_here().expect("spawn failed");
    assert!(wait_for(|| USER_FAULTS.load(Ordering::Relaxed) > f0));
    assert!(wait_for(|| !sched::is_thread_present(warmup)));

    // Sample the baseline only once `used()` has STOPPED MOVING. The warm-up's space is freed by
    // `finish_switch` on whatever core ran it (§28 (SMP placement)), a beat after its ThreadId
    // stops resolving; a baseline sampled in that window is too high: `used()` settles below it,
    // which reads as a negative leak. Two agreeing samples a yield apart mean nothing is in flight;
    // `wait_for`'s deadline bounds it.
    let mut last = used();
    let settled = wait_for(|| {
        sched::yield_now();
        let now = core::mem::replace(&mut last, used());
        now == last
    });
    assert!(
        settled,
        "frame accounting never settled before the baseline"
    );
    let before = last;

    for _ in 0..4 {
        let f = USER_FAULTS.load(Ordering::Relaxed);
        let kernel_test_subject = kernel_test_subject_here().expect("spawn failed");
        assert!(wait_for(|| USER_FAULTS.load(Ordering::Relaxed) > f));
        assert!(wait_for(|| !sched::is_thread_present(kernel_test_subject)));
    }

    // Wait for `used()` to come back rather than reading it once: `spawn_on` is a hint (DECISIONS
    // §28), so a stolen subject's frames are freed on another core a beat after its ThreadId goes.
    // A real leak never gives them back, so the wait times out. `<=`, not `==`: another test's
    // teardown may free frames inside this window. The message prints `seen`, the count the wait
    // decided on: `wait_for` re-evaluates once past its deadline, so on a timeout `seen > before`
    // by construction, where a fresh sample could print zero or a negative count. See
    // notes/load-sensitive-assertions.md and notes/system-test-modules/address-space-tests.md.
    let mut seen = before;
    let came_back = wait_for(|| {
        seen = used();
        seen <= before
    });
    assert!(
        came_back,
        "four user address spaces came and went and {} frames did not come back",
        seen - before,
    );
}

/// **Milestone 19b: a user-built address space is a first-class citizen of every memory
/// mechanism.** Retype a space out of a region, map a frame into it, and check the three
/// things that make it real: the CPU's walker sees the mapping with the exact flags asked
/// for; §13 revocation reaches into the user-built space (the record was paid and filed, so
/// `revoke_page_frame` unmaps it there like anywhere); and destroying the pinned backing region
/// frees nothing while the space lives in it.
#[test_case]
fn a_user_built_aspace_maps_translates_and_revokes() {
    let region = crate::memory_region::create(8).expect("no region");
    let name = user_address_space_create(region).expect("no address space");
    let root = user_address_space_root(name).expect("address space has no root");

    let frame_region = crate::memory_region::create(2).expect("no frame region");
    let phys = crate::memory_region::retype_page(frame_region).expect("no frame");
    let va = 0x40_0000u64;

    user_address_space_map(
        name,
        va,
        phys,
        Flags::user_rodata(),
        crate::revoke::PageMapSource::NoCapability,
    )
    .expect("map_into failed");

    let (mapped_pa, flags) = mmu::translate_at(root, va).expect("the walker sees no mapping");
    assert_eq!(mapped_pa, phys, "mapped the wrong frame");
    assert!(!flags.is_writable(), "asked read-only, got writable");
    assert!(
        !flags.is_global(),
        "a user mapping in a built space must be ASID-tagged"
    );

    // Same va twice: refused, the break-before-make contract holds for built spaces too.
    assert!(
        user_address_space_map(
            name,
            va,
            phys,
            Flags::user_rodata(),
            crate::revoke::PageMapSource::NoCapability
        )
        .is_err(),
        "double-map at one va was allowed"
    );

    // The reach of §13: revoking the frame unmaps it from the space nobody exec'd.
    crate::revoke::revoke_page_frame(phys);
    assert!(
        mmu::translate_at(root, va).is_none(),
        "revocation does not reach a user-built address space",
    );

    // The pin: the backing region hosts a live root, so destroy must free nothing.
    let free_before = crate::memory::stats().unwrap().free();
    crate::memory_region::destroy(region);
    assert_eq!(
        crate::memory::stats().unwrap().free(),
        free_before,
        "destroy reclaimed the region under a live user-built space",
    );

    // The frame region is unpinned (it only ever produced a plain frame), so destroy
    // reclaims it whole, the already-revoked frame included. No manual free: the region
    // owns its pages, and freeing one twice is the allocator's double-free panic.
    crate::memory_region::destroy(frame_region);
}
