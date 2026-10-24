//! **What `free`, `vmstat` and `slabtop` read, proved in the kernel** (milestone 126 (the `procps` package), DECISIONS
//! §225 (`free` sees the machine and your share)).
//!
//! Each mechanism is tested where it could be wrong. `MemoryRegion::USAGE` has a rights gate, so it
//! is driven through the real dispatcher with capabilities holding one right each. The machine
//! statistics page is scheduler-written counters, read through its protocol crate for what only a
//! running kernel makes true: the page exists, is recognized, and its counters move. Nothing here
//! is under `arch/`, so all three architectures run it (DECISIONS §19 (architectural parity is a
//! tenet)).

use abi::{Error, usage};

use crate::arch::exceptions::TrapFrame;
use crate::cap::Rights;
use crate::sched;
use crate::syscall::invoke;

/// `invoke(cap, method, a0, _, _)` through the real dispatcher.
fn call(slot: u64, method: u64, a0: u64) -> i64 {
    let mut frame = TrapFrame::for_user_entry(0, 0, [0, 0, 0]);
    match invoke(&mut frame, slot, method, a0, 0, 0) {
        Ok(v) => v,
        Err(e) => e as i64,
    }
}

fn hold(region: u64, rights: Rights) -> u64 {
    sched::grant(crate::cap::memory_region_cap_rights(region, rights)).expect("grant the region")
}

/// **`ENUMERATE` answers, and answers only.** A view of a region learns its size and what it spent,
/// and is refused every method that would spend, split or destroy it.
#[test_case]
fn a_view_of_a_region_learns_what_it_spent_and_can_spend_nothing() {
    let region = crate::memory_region::create(8).expect("an 8-page region");
    let view = hold(region, Rights::ENUMERATE);

    assert_eq!(call(view, abi::memory_region::USAGE, usage::SIZE), 8);
    assert_eq!(call(view, abi::memory_region::USAGE, usage::COMMITTED), 0);

    crate::memory_region::retype_page(region).expect("one plain page");
    crate::memory_region::retype_object_page(region, crate::memory_region::ObjectKind::Rendezvous)
        .expect("one object page");
    assert_eq!(call(view, abi::memory_region::USAGE, usage::COMMITTED), 2);
    assert_eq!(call(view, abi::memory_region::USAGE, usage::FRAMES), 1);
    assert_eq!(call(view, abi::memory_region::USAGE, usage::RENDEZVOUS), 1);
    assert_eq!(call(view, abi::memory_region::USAGE, usage::THREADS), 0);

    // Named through a `use` so `script/lint`'s RETYPE check, which reads call sites that pass a
    // count, does not read this list of refused methods as one: every one is called with 0.
    use abi::memory_region::{DESTROY, RETYPE, SPLIT};
    for method in [RETYPE, SPLIT, DESTROY] {
        assert_eq!(
            call(view, method, 0),
            Error::NotPermitted as i64,
            "an ENUMERATE view was allowed method {method}",
        );
    }
    let _ = sched::delete_current_cap(view);
    crate::memory_region::unpin(region);
    crate::memory_region::destroy(region);
}

/// **The spender cannot ask**, which is the other half of the separation: `WRITE` is the right to
/// spend, and it is not the right to learn what was spent. And an unknown record is refused before
/// the region is consulted, `SURVEY`'s order.
#[test_case]
fn a_budget_without_enumerate_is_refused_and_an_unknown_record_is_refused_first() {
    let region = crate::memory_region::create(4).expect("a 4-page region");
    let spend = hold(region, Rights::WRITE);
    assert_eq!(
        call(spend, abi::memory_region::USAGE, usage::SIZE),
        Error::NotPermitted as i64
    );
    let view = hold(region, Rights::ENUMERATE);
    assert!(!usage::is_known(usage::CHILDREN + 1));
    assert_eq!(
        call(view, abi::memory_region::USAGE, usage::CHILDREN + 1),
        Error::BadMethod as i64
    );

    // A reclaimed region's view is stale, and says `Gone` rather than answering zero.
    crate::memory_region::destroy(region);
    assert_eq!(
        call(view, abi::memory_region::USAGE, usage::SIZE),
        Error::Gone as i64
    );
    let _ = sched::delete_current_cap(spend);
    let _ = sched::delete_current_cap(view);
}

/// **The page exists, is recognized, and moves.** The frame count is checked against the allocator,
/// the context-switch count against the fact that the suite has switched, and the tick by waiting
/// for one to land, so each counter is observed rather than assumed.
#[test_case]
fn the_machine_statistics_page_is_published_and_its_counters_move() {
    let phys = crate::machine_statistics::page_phys();
    assert_ne!(phys, 0, "sched::init did not publish the page");
    let va = crate::arch::mmu::phys_to_virt(phys);
    // SAFETY: the frame `publish` allocated and never frees, reached through the direct map.
    let before = unsafe { machine_statistics_contract::Snapshot::read(va) }
        .expect("the page carries its magic");
    assert!(before.total_frames > 0 && before.free_frames <= before.total_frames);
    assert_eq!(before.frame_bytes, page_frames::FRAME_SIZE);
    assert!(before.online_cpus() >= 1, "no core has ticked since boot");

    // Free frames track the allocator: after an allocation the page agrees with the allocator's own
    // count, because the allocator stores it under the same lock it allocated under.
    let frame = crate::memory::alloc().expect("a frame");
    let allocator_says = crate::memory::free_page_frames() as u64;
    // SAFETY: as above.
    let taken = unsafe { machine_statistics_contract::Snapshot::read(va) }.unwrap();
    crate::memory::free(frame);
    assert_eq!(taken.free_frames, allocator_says);
    assert!(
        before.context_switches() > 0,
        "the suite has switched threads many times and the page counted none"
    );

    // Wait for both the tick and its interrupt: a tick's busy-or-idle add and its interrupt add are
    // two relaxed adds on another core, and this read can land between them. Bounded by 50 ticks
    // of progress, so only a counter that never moves fails.
    let ticks = |s: &machine_statistics_contract::Snapshot| s.busy_ticks() + s.idle_ticks();
    let start = ticks(&before);
    let mut now = before;
    for _ in 0..100_000_000u64 {
        sched::yield_now();
        // SAFETY: as above.
        now = unsafe { machine_statistics_contract::Snapshot::read(va) }.unwrap();
        let moved = ticks(&now) > start && now.interrupts() > before.interrupts();
        if moved || ticks(&now) > start + 50 {
            break;
        }
    }
    assert!(ticks(&now) > start, "no tick reached the page");
    assert!(
        now.interrupts() > before.interrupts(),
        "the tick was not counted as an interrupt"
    );
    assert!(
        now.context_switches() >= before.context_switches(),
        "a context-switch count went backwards"
    );
}

/// **The page's switch count is each core's exact count, as of that core's last tick** (milestone
/// 629 (the context-switch statistic stops costing the switch path)). The switch counts into
/// `PerCpu::switches` and the tick copies it, so two things must hold for every online core: the
/// page never runs ahead of the core's own count, and once the core has ticked, the page has caught
/// up to at least what the count was before that tick. A tick-sampled or lossy copy fails the
/// second; a page incremented by anything other than the copy can fail the first.
///
/// Every core is read, not this one, so a migration of the test thread between reads changes
/// nothing: each comparison is between one core's page line and that same core's block.
#[test_case]
fn the_pages_switch_count_is_each_cpus_exact_count_as_of_its_last_tick() {
    use core::sync::atomic::Ordering;

    use machine_statistics_contract::CPU_ID_BOUND;

    let va = crate::arch::mmu::phys_to_virt(crate::machine_statistics::page_phys());
    // SAFETY: the frame `publish` allocated and never frees, reached through the direct map.
    let read = || unsafe { machine_statistics_contract::Snapshot::read(va) }.unwrap();
    let count = |c: usize| crate::cpu::of(c).switches.load(Ordering::Relaxed);

    let before = read();
    let counted: [u64; CPU_ID_BOUND] = core::array::from_fn(count);
    let ticks = |s: &machine_statistics_contract::Snapshot, c: usize| {
        s.cpus[c].busy_ticks + s.cpus[c].idle_ticks
    };

    // Wait on the claim, not the tick word: the tick's two stores are relaxed, and another core may
    // see the tick word move before the copy (rule 4, weak memory ordering). A parked core keeps its
    // last copy, as the protocol's BUGS allow, so the wait is bounded by 50 ticks per core and the
    // lower bound is asserted only for cores that ticked.
    let ticked =
        |now: &machine_statistics_contract::Snapshot, c: usize| ticks(now, c) > ticks(&before, c);
    let online = || (0..CPU_ID_BOUND).filter(|&c| before.cpus[c].online);
    let total =
        |s: &machine_statistics_contract::Snapshot| online().map(|c| ticks(s, c)).sum::<u64>();
    let budget = total(&before) + 50 * online().count() as u64;
    let mut now = before;
    for _ in 0..100_000_000u64 {
        sched::yield_now();
        now = read();
        let caught_up =
            online().all(|c| ticked(&now, c) && now.cpus[c].context_switches >= counted[c]);
        if caught_up || total(&now) > budget {
            break;
        }
    }
    let after: [u64; CPU_ID_BOUND] = core::array::from_fn(count);
    let ticked = |c: usize| ticked(&now, c);
    assert!(
        (0..CPU_ID_BOUND).any(|c| before.cpus[c].online && ticked(c)),
        "no core ticked"
    );
    for c in (0..CPU_ID_BOUND).filter(|&c| before.cpus[c].online) {
        let published = now.cpus[c].context_switches;
        if ticked(c) {
            assert!(
                published >= counted[c],
                "core {c}: the page says {published} switches after a tick, but the core had counted {} before it",
                counted[c],
            );
        }
        assert!(
            published <= after[c],
            "core {c}: the page says {published} switches, ahead of the core's own count {}",
            after[c],
        );
    }
}
