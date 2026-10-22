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
/// Two spaces map the *same* virtual address to different frames holding different bytes.
/// We install A and read through the VA (loading A's translation, tagged with A's ASID,
/// into the TLB), then install B, which since milestone 15 flushes nothing, and read
/// again. If user mappings were still global, or the ASID did not ride TTBR0, or two
/// spaces shared a tag, B's read would hit A's still-cached entry and see A's byte: one
/// process reading another's memory, the exact bug the sledgehammer flush used to prevent.
///
/// **This ran on aarch64 only until milestone 58, and the reason it could not be ported is worth
/// keeping.** `riscv64::mmu::write_satp` used to issue an unconditional `sfence.vma` on every root
/// switch, so a RISC-V twin would have read B's byte because everything had just been flushed, not
/// because the tagging works: a test that cannot fail for its stated reason, which is worse than no
/// test. Porting it was never a test-portability change; it needed the ASID shootdown underneath.
/// Now that RISC-V's context switch flushes nothing either, the assertion means the same thing on
/// both ISAs and this is one suite, which is what DECISIONS §19 asks for.
#[test_case]
fn asid_tagging_keeps_address_spaces_apart_without_flushes() {
    let mut a = AddressSpace::new(2).expect("no space A");
    let mut b = AddressSpace::new(2).expect("no space B");

    let (asid_a, asid_b) = (mmu::asid_of(a.ttbr0()), mmu::asid_of(b.ttbr0()));

    // **An architecture that does not tag at all cannot be tested for tagging**, and this is the
    // same refusal the doc comment above records about RISC-V before milestone 58: a test that
    // cannot fail for its stated reason is worse than no test.
    //
    // x86_64 is that architecture today. A PCID lives in `CR3[11:0]`, and with `CR4.PCIDE` clear
    // those bits are reserved-zero, so `arch::x86_64::mmu::ttbr0_value` drops the number and every
    // `mov cr3` flushes the whole TLB. B's read would then return B's byte because nothing was
    // cached, not because the tagging works. Turning `PCIDE` on is worth a measurement and is
    // recorded as an architect's call (item 3 of milestone 161 (the x86_64 kernel port)); the day
    // it is on, both
    // spaces get
    // real tags and this skip stops firing with nothing else edited.
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

/// **An ASID flush reaches the OTHER cores, which is the whole of milestone 58 on RISC-V.**
///
/// The test above proves tagging keeps two spaces apart on one core. This one proves the other half
/// of the contract, the half that is a distributed protocol on one ISA and a single instruction on
/// the other: when `crates/address_space_identifier` says "flush, then the number may tag someone else", the flush has
/// to have reached every core that could be holding an entry wearing it. aarch64's `tlbi aside1is`
/// broadcasts in hardware. RISC-V's `sfence.vma` affects only the hart that runs it, so `flush_asid`
/// has to IPI the others through SBI RFENCE and wait for them.
///
/// **This test fails without the shootdown**, which is the property worth having. The sequence:
///
///   1. a probe thread on **another** core installs space A and reads `VA`, which is what pulls the
///      translation into *that* core's TLB, tagged with A's ASID, and **leaves it installed**
///   2. this core moves `VA` onto a different frame **with no per-address invalidation at all**, so
///      the only thing that can announce the change anywhere is the ASID sweep
///   3. this core calls `flush_asid`
///   4. the probe reads `VA` again, on the same core, with the same space still installed
///
/// Step 4 must see the new frame. If the sweep stayed local it sees the old one, and that byte is
/// exactly the bug: a core still translating an address space that no longer means what it did.
///
/// **The space is never re-installed**, because a `satp` or `TTBR0` write between the reads may be
/// treated as a flush. And the mapping is **changed rather than recycled**, because a recycled ASID
/// would likely get the dead space's frames and read the right byte by accident.
///
/// # BUGS
///
/// - **The probe is placed, not pinned.** `spawn_on` is a hint and an idle core may steal, so the
///   probe records where it actually ran and the test asserts only that it was not *this* core. It
///   cannot land here: this thread spins without yielding, so its core never goes idle and never
///   steals. Both of the probe's reads happen inside one thread with interrupts masked, so they
///   cannot land on two different cores.
/// - **It needs two cores** and skips nothing if it has one: it fails, because the runner passes
///   `-smp 4` on both ISAs and a machine that came up single-core is a finding, not a reason to be
///   quiet.
/// - **A broken shootdown could hang this test instead of failing it**, if the reason it were broken
///   were that the remote core never services the request. The probe's spin is deadline-bounded and
///   reports `PROBE_GAVE_UP` so that case is named rather than silent, but the failure text would
///   then be about the handshake and the real cause would be one level down.
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
    // Any ONLINE core but this one, from the set rather than `0..count` (first-silicon sweep,
    // 2026-08-14): on a non-contiguous online set the range picks a parked core, the probe never
    // runs, and the shootdown test times out reading as a TLB bug.
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

/// **A space installed with `while_installed` is still installed when its body asks, however many
/// timer ticks the body spans.**
///
/// The witness for the x86_64 OVMF flake of 2026-10-13 (UTC), merge-group run 38345573542, where
/// `map_physical_maps_a_shared_frame_and_a_device_page` read "shared page not mapped" from a page it
/// had just mapped. `translate_user` walks the root the CPU holds now, and a kernel thread has no
/// space of its own, so a context switch back to it installs the kernel's reserved root. A
/// preemption between the install and the walk therefore took the space away under the body. It
/// needs a tick inside a window a few microseconds wide and another runnable thread on the same
/// core, and the one-core OVMF leg had that thread: `a_user_client_moves_data_through_shared_memory`,
/// the test just before, leaves its client spinning forever.
///
/// This test makes both conditions certain rather than lucky: another thread is queued on this
/// thread's core when the window opens, and the body spans five tick periods before it walks.
/// Without interrupts held off inside `while_installed` it failed on every run on all three
/// architectures (2026-10-13 UTC).
///
/// **The precondition is read inside the window, from this core's run queue**, not inferred from
/// where a competitor once ran. The first cut waited for a competitor to report this core's bit and
/// failed its own precondition on the riscv64 `rva23s64` CPU-matrix leg (merge-group run
/// 38351881399, 2026-10-13 UTC). `spawn_on` is a hint, and a steal (§28 (SMP placement), whose
/// message-shaped stealing lets an idle core pull a queued thread from a loaded one) breaks it:
/// while the competitors were spawned in core order, a core whose own competitor had not arrived
/// yet was idle, and it stole the one queued on this core. Every other core then held a spinner, so
/// no core was idle again, nothing could steal one back, and this thread sat alone until `wait_for`
/// gave up. Forcing that interleaving reproduced the panic with competitors on cores `[2, 1, 2, 3]`
/// and this thread on core 0. So the setup now repairs the placement (a competitor spawned onto
/// this core while its queue is empty) and the body reads the queue it is about to be preempted
/// against.
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

    // Warm up: the first user thread ever created pays for page tables in a region of
    // kernel VA that nothing has touched. Measure the STEADY state, which is the one that
    // has to hold forever.
    //
    // Snapshot the fault count BEFORE spawning (as the loop below always did). The old
    // order, spawn then snapshot, was a race: on SMP the `kernel_test_subject` can fault in that
    // gap, the snapshot swallows its fault, and the wait below times out on a count that will never
    // move again. Latent until milestone 14 phase A.2/A.3 made spawn-to-fault fast enough
    // to lose the race about once in seven runs.
    //
    // Pin the `kernel_test_subject` instances to THIS core (DECISIONS §28 made `spawn` scatter
    // them). PageFrame accounting must be exact to catch a leak (the milestone-6 bug this test
    // guards), but a thread's frames are freed by `finish_switch` on whatever core reaps it,
    // *after* it leaves the thread table and outside IPC_TABLES. Scattered across cores, that free
    // is asynchronous, so `used()` fluctuates and never reads exact. Kept on the test's own core,
    // each `kernel_test_subject` instance's fault, reap, and frame free happen synchronously under
    // the test's own yields, so `used()` is exact again. This tests the reaper, not placement, so
    // pinning costs nothing.
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

    // Sample the baseline only once `used()` has STOPPED MOVING, for the same reason the
    // assertion below waits rather than reading instantly, applied to the other end. The warm-up
    // `kernel_test_subject`'s address space is freed by `finish_switch` on whatever core actually
    // ran it, which under §28 placement need not be this one, and that free lands a beat *after*
    // its ThreadId stops resolving. Sampling `before` inside that window captures frames that are
    // about to come back, `used()` then settles BELOW `before`, and a wait for equality could never
    // succeed. The failure said so plainly when it happened: it reported "-18 frames did not come
    // back", a NEGATIVE leak, which no real leak can produce. Found when an unrelated change to
    // the std::fs test shifted this test's timing; the race was already here.
    // Two agreeing samples a yield apart mean nothing is in flight. Bounded by `wait_for`'s own
    // deadline, so a genuinely unstable allocator fails the test rather than spinning here.
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

    // Exact in the leak direction, but allow the asynchronous reap to settle. Pinning the
    // `kernel_test_subject` instances with `spawn_on` is a placement HINT, not a pin (DECISIONS
    // §28): an idle core can steal one before this core runs it, and then the frame free
    // (finish_switch dropping the address space, after the thread leaves the table and outside
    // IPC_TABLES) lands on that core a beat after the ThreadId resolves to nothing. So wait for
    // `used()` to come back to `before` rather than reading it the instant the last
    // `kernel_test_subject` is gone. Still a leak trap: a real leak (the milestone-6 bug) never
    // gives the frames back, so this wait times out and fails.
    //
    // `<=`, not `==`. Equality demanded that no OTHER test's teardown free a frame during this
    // window, which is not a property four `kernel_test_subject` address spaces are responsible
    // for, and it is the only way the old form could fail with a NEGATIVE count: it did, on CI and
    // on a quiet aarch64 dev machine, as "-19 frames did not come back", frames arriving from
    // outside the measured window. A leak still fails identically: every frame the
    // `kernel_test_subject` instances keep holds `used()` above `before` forever. See
    // notes/load-sensitive-assertions.md.
    //
    // The count in the message is the one the wait DECIDED on, not a fresh sample, and this site is
    // why the distinction is worth code. `used() as i64 - before as i64` re-reads the allocator
    // after the wait has already given up, so frames arriving in that gap make a genuine timeout
    // print zero or a NEGATIVE count: the exact "-19 frames did not come back" that cost this
    // family three separate diagnoses, now emitted by a form that can no longer fail for that
    // reason. A reader who trusts the sign would re-run the old investigation from the top.
    // `wait_for` re-evaluates the predicate once past its deadline, so a `false` return leaves
    // `seen > before` and the count below is positive by construction, no cast required.
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
