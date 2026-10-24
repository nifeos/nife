use super::*;

/// The `current_cpu_reader` program's ELF bytes: reads its own core twice across a yield, with no
/// syscall, and reports both plus the id bound.
fn current_cpu_reader_image() -> &'static [u8] {
    program("current_cpu_reader").expect("no current_cpu_reader program in the initial ramdisk")
}

/// **A userspace thread learns which core it is on, by loading from memory, on all three
/// architectures.**
///
/// calef's 2026-09-21 ruling that a thread observing itself reads a page, proved end to end:
/// `crates/current_cpu_contract` lays the page out, `AddressSpace::new` maps it read-only,
/// `schedule()` writes the core at switch-in, and `user_mode_runtime::current_cpu` reads it. Each
/// read must name a core in the online set rather than a particular number, since the scheduler may
/// place the thread anywhere and a consumer indexing by the id needs a real one (the VisionFive 2's
/// set is `{1, 2, 3}`). Neither read may be `None`: a running thread has been switched in, so its
/// page has been written. See notes/scheduler/the-current-cpu-page.md.
#[test_case]
fn a_userspace_thread_reads_a_cpu_that_is_really_online() {
    let endpoints = crate::memory_region::create(1).expect("no endpoint region");
    let result = crate::sched::create_rendezvous_from(endpoints).expect("no result rendezvous");
    crate::sched::spawn(move || {
        run(
            current_cpu_reader_image(),
            Spawn {
                arg0: 0,
                arg1: 0,
                arg2: 0,
                grants: &[crate::cap::rendezvous_cap(
                    result,
                    crate::cap::Rights::WRITE,
                )],
                maps: &[],
            },
        )
    })
    .expect("spawn failed");

    let [first, second, bound, _, _] = crate::sched::ipc_receive(result);
    crate::sched::reclaim_region(endpoints).expect("the endpoint region did not come back");
    let online = crate::smp::online_harts_mask();

    for (which, cpu) in [("first", first), ("second", second)] {
        assert_ne!(
            cpu,
            current_cpu_contract::UNSCHEDULED,
            "the {which} read came back unknown: a running thread's page was never mapped or \
             never written",
        );
        assert!(
            cpu < current_cpu_contract::CPU_ID_BOUND as u64,
            "the {which} read is cpu {cpu}, past the bound userspace sizes its arrays by",
        );
        assert!(
            online & (1 << cpu) != 0,
            "the {which} read is cpu {cpu}, which is not in the online set {online:#b}: a \
             consumer indexing by this would reach a core that is not there",
        );
    }

    // The one number two binaries have to agree on, carried back across the boundary. Cheap here
    // and impossible anywhere else: a host test can only compare the crate against itself.
    assert_eq!(
        bound,
        crate::cpu::MAX_CPUS as u64,
        "userspace sizes its per-cpu arrays by {bound} and the kernel hands out ids under {}; one \
         of them is a stale copy of the other",
        crate::cpu::MAX_CPUS,
    );
}

/// **A space that has never run a thread reads as unknown, not as CPU 0**, and the page is a real
/// mapping rather than a promise.
///
/// The half the end-to-end test above structurally cannot reach: by the time a program can ask,
/// it has been switched in, so the unset state is unobservable from inside the thread. Read here
/// from the kernel's own side of the same frame, before anything has run in the space.
///
/// Zero would be a plausible answer and a wrong one, which is why the page carries a magic and a
/// sentinel instead of relying on a zeroed frame. calef ruled the same day, about the counter
/// frequency, that a wrong number is worse than no number.
#[test_case]
fn a_space_that_never_ran_has_no_answer_and_then_has_the_right_one() {
    let (space, _entry) = load(current_cpu_reader_image(), 0).expect("load failed");
    let va = space
        .current_cpu_page_kernel_va()
        .expect("a freshly loaded space has no current-cpu page");

    // SAFETY: `va` is this space's own frame through the direct map, `PAGE_BYTES` wide, and this
    // is the only thread that can reach it: no thread has been bound to the space.
    let page = unsafe { current_cpu_contract::CurrentCpuPage::new(va) };
    assert_eq!(
        page.cpu(),
        None,
        "a space whose thread has never been switched in reported a core",
    );

    // The write the context switch makes, made by hand, so the reader and the writer are proved
    // against each other on the machine rather than only on the host.
    space.publish_current_cpu(1);
    assert_eq!(page.cpu(), Some(1), "the published core did not read back");
    space.publish_current_cpu(0);
    assert_eq!(
        page.cpu(),
        Some(0),
        "cpu 0 read back as unknown: the sentinel and a real core got confused",
    );
}

/// **The page comes back when the space dies.** It is the one frame an address space owns that its
/// region does not pay for, so `memory_region::destroy` does not cover it and `Drop` frees it by
/// hand, which is the shape a leak hides in.
///
/// It asks the allocator about that one frame's bit rather than bracketing the machine's free total,
/// which a neighbouring test's late teardown can move; a leak still fails it, because a frame `Drop`
/// never returned stays marked used. `memory_region::usage` cannot scope it, since the frame belongs
/// to no region. See notes/load-sensitive-assertions.md and notes/scheduler/the-current-cpu-page.md.
#[test_case]
fn the_page_is_returned_when_the_space_is_dropped() {
    let frame = {
        let space = load(current_cpu_reader_image(), 0).expect("load failed").0;
        let va = space
            .current_cpu_page_kernel_va()
            .expect("a loaded space has no current-cpu page to return");

        // The kernel VA is this frame through the direct map, so inverting the map names the
        // physical frame the allocator handed out. Taken while the space is alive, because the
        // accessor goes away with it.
        let frame = PageFrame::containing(mmu::virt_to_phys(va));
        assert_eq!(
            crate::memory::is_page_frame_used(frame),
            Some(true),
            "the current-cpu page at {va:#x} is not a frame this allocator has handed out: the \
             direct-map inversion below names the wrong frame and the check after the drop would \
             prove nothing",
        );
        frame
    };

    assert_eq!(
        crate::memory::is_page_frame_used(frame),
        Some(false),
        "dropping an address space did not return its current-cpu frame at {:#x}",
        frame.addr(),
    );
}
