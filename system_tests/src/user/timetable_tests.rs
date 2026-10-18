use super::test_programs::{lay_out_program, segment_pages};
use super::*;
use crate::cap::{Rights, memory_region_root_cap, page_frame_cap, rendezvous_cap};
use crate::sched::RendezvousId;

/// The timetable's budget. Every scheduled instance is 48 pages of it (`INSTANCE_PAGES` in
/// `components/src/timetable.rs`) plus the loader's own scratch page tables, and the pages come home when
/// a corpse is reaped, so this covers a handful of live instances rather than one per fire.
const TIMETABLE_BUDGET_PAGES: u64 = 768;

/// How many fires to ask for before the timetable summarises and exits.
///
/// **Four is a count of fires, not a duration**, and that distinction is the whole of why this test
/// is safe on a loaded machine (notes/load-sensitive-assertions.md). Nothing here asserts that a
/// fire happened *within* any interval; the test waits for the fires to arrive and the harness's
/// per-test ceiling is the only backstop. A slow host makes this test slower and never makes it
/// red, which is the property milestone 62 spent a week putting back into this tree.
const FIRES: u64 = 4;

/// Stack pages for the timetable, tracking `timetable::contract::STACK_PAGES`.
///
/// A number a spawn site **states** rather than inherits, which is `supervision_protocol`'s own rule
/// (`CHILD_STACK_PAGES`: "a builder that silently inherits somebody else's stack size finds faults
/// that builder does not have"). The timetable needs it because its working set is the plan itself:
/// a `grant_plan::Endowment` is a kilobyte, mostly the name set a directory grant can carry, and a
/// `timetable::Registry` holds one per entry. Eight pages died here with a data abort whose faulting
/// address was the stack pointer, which is what a stack overflow looks like from the kernel side and
/// is worth recognising: it reads like a wild pointer and is not one. 32 became 48 on 2026-09-30,
/// with the measurement, in `timetable::contract`'s `STACK_PAGES`.
const TIMETABLE_STACK_PAGES: u64 = 48;

/// The line `components/src/timetable.rs` prints when the plan is complete and it is about to arm. The
/// test reads the plan up to it, which is what lets one endpoint carry the plan, the summary and
/// the end of the stream without the reader having to guess where each stops.
const ARMED: &str = "timetable: armed";

/// **The programs the timetable's document will ever build**, and the whole of what this spawn site
/// hands over.
///
/// The archive below holds exactly these and nothing else, which is milestone 129's second stratum:
/// `timetable::Registry::programs` is complete and computable at registration, so the endowment can
/// be narrowed to it instead of being the whole initial ramdisk.
///
/// **It is a written list rather than a computation here, and that is forced rather than chosen.**
/// Computing it means calling `timetable::Registry::register`, which means `grant_plan::plan`, and
/// those two carry 21632-byte and 12048-byte frames: `Registry` is a `[Row; MAX_ENTRIES]` and a
/// `Row` holds a kilobyte of `Endowment`. That is exactly the shape `script/stack-frame-check`
/// exists to refuse, because a frame larger than the 4096-byte guard page can step `sp` past the
/// guard without touching it and land in the neighbouring thread's stack. The sizes are fine in
/// `components/src/timetable.rs`, which is a process with a 32-page stack and says so; they are not fine
/// in this binary, and no threshold should be widened to make them fine.
///
/// **What keeps the list honest is a host test rather than a comment.**
/// `timetable::tests::the_archive_a_timetable_holds_is_measured_against_what_it_will_build` registers
/// the same `components/timetable.conf` against the same `timetable::SHIPPED_HELD` and asserts the plan is
/// exactly this set, by name. Editing the document without editing this list fails that test in
/// milliseconds, on the host, with no emulator. The program itself then audits what it was handed
/// and prints the answer, and the assertions below read it, so a wrong list fails twice.
///
/// `memory_grant_depleter` joined `least_authority_demo` on 2026-08-22, when milestone 129's `--mem` grant was backed:
/// `timetable.conf`'s `at-boot memory_grant_depleter --mem 4` is admitted because `SHIPPED_HELD.mem_pages` (4,
/// mirrored below as [`MEM_GRANT_PAGES`]) covers it.
const PLANNED_PROGRAMS: [&str; 2] = ["least_authority_demo", "memory_grant_depleter"];

/// **What `at-boot memory_grant_depleter --mem 4` grants, mirrored from `components/timetable.conf` and
/// `timetable::SHIPPED_HELD.mem_pages`.** A written constant rather than a computed one for the
/// same reason [`PLANNED_PROGRAMS`] is: this test does not depend on the `timetable` crate at all
/// (`script/stack-frame-check` is why, see that constant's own doc), so nothing here can read the
/// real value back. What keeps it honest is the same host test that keeps `PLANNED_PROGRAMS`
/// honest: it parses the same document against the same `SHIPPED_HELD` and would go red first if
/// the two drifted apart.
///
/// Used as a range rather than an exact literal below: how many of the 4 pages `memory_grant_depleter` actually
/// manages to map is page-table overhead, which is page-table-implementation detail this milestone
/// makes no claim about and which is free to differ between aarch64 and riscv64.
const MEM_GRANT_PAGES: u64 = 4;

/// **Room for the archive this spawn site builds**, which is not the initial ramdisk.
///
/// Sized for a handful of debug-build user programs (each is under a megabyte) plus nifefs's six
/// directory blocks. It is a fixed buffer because the kernel has no heap: `alloc` is not linked
/// here, deliberately, and a test that needed one would be the first. Overflowing it is a loud
/// `Error::OutOfBounds` from `nifefs::write_image` rather than a corrupt archive.
///
/// It is a `static`, not a local, for the reason the list above is a list: four megabytes on the
/// stack is four megabytes past the guard page.
const NARROWED_ARCHIVE_BYTES: usize = 4 << 20;

/// The narrowed archive itself. `#[cfg(test)]` reaches this module, so this costs nothing in a
/// shipping kernel; it is `.bss`, so it costs no image bytes either.
static mut NARROWED_ARCHIVE: [u8; NARROWED_ARCHIVE_BYTES] = [0; NARROWED_ARCHIVE_BYTES];

/// **Build an archive holding exactly [`PLANNED_PROGRAMS`], and nothing else.**
///
/// The timetable is handed this instead of the initial ramdisk, so a process whose document admits
/// one program cannot load the other fifty-six. It copies images out of the initial ramdisk into a
/// fresh archive; the initial ramdisk is untouched.
fn narrowed_archive(programs: &[&'static str]) -> &'static [u8] {
    let mut files: [(&str, &[u8]); PLANNED_PROGRAMS.len()] = [("", &[]); PLANNED_PROGRAMS.len()];
    for (i, name) in programs.iter().enumerate() {
        let bytes = program(name).expect("a planned program is not in the initial ramdisk");
        files[i] = (name, bytes);
    }
    let files = &files[..programs.len()];

    // SAFETY: single-threaded test setup; this is the only reference taken to the buffer, and the
    // slice returned below is read-only from here on.
    let buf = unsafe { &mut *core::ptr::addr_of_mut!(NARROWED_ARCHIVE) };
    let len = nifefs::write_image(files, buf).expect("the narrowed archive does not fit");
    &buf[..len]
}

/// **Spawn the timetable the way a boot would**, and hand back the three endpoints it is wired to.
///
/// Deliberately the same endowment shape `spawn_hello` and `c_seam_tests::spawn_confiner` use (the
/// archive read-only at `INITIAL_RAMDISK_VA`, capabilities in numbered slots, nothing privileged),
/// so what is under test is the scheduler rather than a shortcut. Its complete authority is the
/// four slots below, which is the same list `components/src/timetable.rs`'s header states and the
/// reason a scheduled `date` in the shipped document is refused.
fn spawn_timetable(fires: u64) -> Spawned {
    spawn_timetable_with(&PLANNED_PROGRAMS, fires, false, false, false)
}

/// **One spawned timetable**: the endpoints it is wired to, the registration page when it has one,
/// and the three regions the spawn site made for it, which [`Spawned::reclaim`] hands back.
struct Spawned {
    out: RendezvousId,
    reports: RendezvousId,
    page: Option<&'static mut [u8]>,
    /// The page's exit word, kept apart from `page` so [`report`] can look at it while the test
    /// holds the page itself.
    exit_word: Option<&'static core::sync::atomic::AtomicU64>,
    budget: u64,
    tcb_region: u64,
    /// The region every endpoint above was carved from, reclaimed last.
    endpoints: u64,
    /// The clock page's frame, when the spawn granted one: the kernel's handle for publishing to it.
    clock: Option<u64>,
}

impl Spawned {
    /// **Give back the three regions the spawn site made**, once the timetable has finished.
    ///
    /// Every test here used to leave its budget and its TCB region behind, and the kernel's region
    /// table is finite: milestone 152's lane found a later test's fire failing with "the budget
    /// cannot back one instance" at 252 of 256 live regions. `reclaim_region` refuses a region
    /// with a child still carved out of it, so the budget's success is also the statement that
    /// every scheduled instance was collected.
    fn reclaim(self) {
        // The timetable may still be on its way out of `exit` when the test has read its last word,
        // and a region with a running resident refuses (and arms the kill that ends it), so each
        // reclaim is retried until the thread has gone. Not a timing assertion: the deadline is
        // `wait_for`'s, far past the few scheduler passes an exit takes.
        let tcb = self.tcb_region;
        assert!(
            super::wait_for(|| crate::sched::reclaim_region(tcb).is_ok()),
            "the timetable's TCB region would not reclaim"
        );
        let budget = self.budget;
        assert!(
            super::wait_for(|| crate::sched::reclaim_region(budget).is_ok()),
            "the timetable's budget would not reclaim: an instance region is still carved out"
        );
        crate::sched::reclaim_region(self.endpoints)
            .expect("the timetable's endpoint region would not reclaim");
    }
}

/// The exit code a registrar-mode timetable left in its page, once it has stopped.
fn exited(t: &Spawned) -> Option<u64> {
    let word = t.exit_word?.load(core::sync::atomic::Ordering::Acquire);
    (word & timetable::registration::EXITED != 0).then_some(word & !timetable::registration::EXITED)
}

/// **The next scheduled child's report, or the timetable's own reason there will not be one.**
///
/// A plain `RECEIVE` on the report endpoint turned every fire failure into the 60-second hang
/// watchdog: the timetable says "the budget cannot back one instance" on its output endpoint and
/// stops, while the test waits on a report that is never coming. So this looks at both endpoints
/// and, if the timetable speaks first, fails with its sentence. No deadline: a slow host makes this
/// slower and never red (notes/load-sensitive-assertions.md).
fn report(t: &Spawned) -> u64 {
    report_until(t, u64::MAX).expect("no deadline, so a report or a panic")
}

/// **[`report`], or `None` once the counter passes `deadline`** (monotonic nanoseconds).
///
/// For a caller that has something better to do than wait when a report is late, never for a
/// verdict: `None` must lead to a retry, not to a red, or this is a timed assertion again.
fn report_until(t: &Spawned, deadline: u64) -> Option<u64> {
    loop {
        if crate::sched::rendezvous_waiting_senders(t.reports) > 0 {
            return Some(crate::sched::ipc_receive(t.reports)[0]);
        }
        if super::clock_service::monotonic_nanos() > deadline {
            return None;
        }
        if let Some(code) = exited(t) {
            panic!("waiting for a scheduled child's report, the timetable exited with {code:#x}");
        }
        if crate::sched::rendezvous_waiting_senders(t.out) > 0 {
            let mut buf = [0u8; 256];
            let said = match line(t.out, &mut buf) {
                Some(n) => core::str::from_utf8(&buf[..n]).unwrap_or("(not UTF-8)"),
                None => "(its output ended)",
            };
            panic!("waiting for a scheduled child's report, the timetable said instead: {said}");
        }
        crate::sched::yield_now();
    }
}

/// Where a registrar-mode timetable finds its registration page. The spawn site's choice, passed
/// in `a2`; clear of the loader's scratch window (`0x1000_0000` upward), the archive at
/// [`INITIAL_RAMDISK_VA`] and the stack below [`USER_STACK_VA`].
const REGISTRATION_VA: u64 = 0x0600_0000;

/// [`spawn_timetable`], with the archive's program list chosen by the caller and, when
/// `registration` is set, a registration page mapped at [`REGISTRATION_VA`] and handed back as the
/// kernel's own view of the same frame. That view is the registrar's half of the page.
fn spawn_timetable_with(
    programs: &[&'static str],
    fires: u64,
    registration: bool,
    run_unvouched: bool,
    clock: bool,
) -> Spawned {
    // **Not the initial ramdisk.** The archive this process is handed holds exactly the programs
    // its own document will ever build; see [`narrowed_archive`] for why the spawn site is the only
    // place that decision can be made.
    let archive = narrowed_archive(programs);
    let archive_len = archive.len() as u64;
    let archive_pages = archive_len.div_ceil(FRAME_SIZE);
    let bytes = program("timetable").expect("no timetable program in the initial ramdisk");
    let elf = Elf::parse(bytes).expect("timetable is not loadable");

    let content: u64 = segment_pages(&elf)
        + 1
        // The archive is **copied** into fresh pages rather than shared with the initial ramdisk's,
        // so it costs the address space a page each rather than only the tables reaching them. That
        // is the price of the narrowing: there is no capability in this system for "these blocks of
        // that archive", so a narrower endowment is a smaller archive, and a smaller archive is
        // bytes somebody has to own.
        + archive_pages
        + archive_pages / 512
        + TIMETABLE_STACK_PAGES
        + 9; // one more than before: the registration page, when there is one
    let mut space = AddressSpace::new(content).expect("no memory for the timetable");
    lay_out_program(&mut space, &elf, TIMETABLE_STACK_PAGES, "the timetable");
    map_the_archive(&mut space, archive, archive_pages);
    let page = registration.then(|| {
        space
            .map_new(REGISTRATION_VA, Flags::user_data())
            .expect("could not map the registration page")
    });
    // The clock page, when granted: a fresh frame the kernel initialises as UNKNOWN and publishes to
    // later, mapped read-only where `timetable::contract` says, exactly as `date`'s tests do.
    let clock_phys = clock.then(|| map_a_clock_page(&mut space));
    let aspace = readopt_user_address_space(space).expect("register the timetable aspace");

    // Four pages: the three endpoints below and the run-unvouched stand-in, when there is one.
    let endpoints = crate::memory_region::create(4).expect("no endpoint region");
    let out = crate::sched::create_rendezvous_from(endpoints).expect("no out rendezvous");
    let child_report =
        crate::sched::create_rendezvous_from(endpoints).expect("no child report rendezvous");
    let deaths = crate::sched::create_rendezvous_from(endpoints).expect("no deaths rendezvous");
    let budget = crate::memory_region::create(TIMETABLE_BUDGET_PAGES).expect("no budget");
    let thread_control_block_region = crate::memory_region::create(2).expect("no tcb region");
    let tid =
        crate::sched::create_thread_control_block(thread_control_block_region).expect("no tcb");

    install_the_fixed_slots(tid, out, budget, child_report, deaths);

    if let Some(phys) = clock_phys {
        // `READ` to read it and `GRANT` to hand it to a job whose manifest declares a clock.
        crate::sched::thread_control_block_insert_cap(
            tid,
            page_frame_cap(phys, Rights::READ.union(Rights::GRANT)),
            Some(timetable::contract::CLOCK_SLOT),
        )
        .expect("insert the clock page");
    }
    if run_unvouched {
        // Any endpoint will do: what the timetable checks is whether the slot is occupied, exactly
        // as `swish` probes it, because the capability's meaning is the slot it is placed in.
        let stand_in = crate::sched::create_rendezvous_from(endpoints).expect("no stand-in");
        crate::sched::thread_control_block_insert_cap(
            tid,
            rendezvous_cap(stand_in, Rights::WRITE),
            Some(grant_plan::spawnproto::RUN_UNVOUCHED_SLOT),
        )
        .expect("insert the run-unvouched stand-in");
    }
    crate::sched::configure_thread_control_block(tid, elf.entry(), USER_STACK_TOP, aspace)
        .expect("configure");
    let page_va = if page.is_some() { REGISTRATION_VA } else { 0 };
    crate::sched::start_thread_control_block(tid, [fires, archive_len, page_va]).expect("start");
    let exit_word = page.as_ref().map(|p| {
        #[allow(clippy::cast_ptr_alignment)] // page-aligned; EXIT is word 7
        // SAFETY: the page is page-aligned, so its exit word is aligned for an `AtomicU64`, and it
        // lives as long as the frame, which outlives the test. Both sides touch it only atomically.
        unsafe {
            &*p.as_ptr()
                .add(timetable::registration::EXIT)
                .cast::<core::sync::atomic::AtomicU64>()
        }
    });
    Spawned {
        out,
        reports: child_report,
        page,
        exit_word,
        budget,
        tcb_region: thread_control_block_region,
        endpoints,
        clock: clock_phys,
    }
}

/// Copy `archive` into `archive_pages` fresh read-only pages at [`INITIAL_RAMDISK_VA`].
fn map_the_archive(space: &mut AddressSpace, archive: &[u8], archive_pages: u64) {
    for i in 0..archive_pages {
        let page = space
            .map_new(INITIAL_RAMDISK_VA + i * FRAME_SIZE, Flags::user_rodata())
            .expect("could not map the narrowed archive");
        let from = (i * FRAME_SIZE) as usize;
        let to = (from + FRAME_SIZE as usize).min(archive.len());
        page[..to - from].copy_from_slice(&archive[from..to]);
    }
}

/// A fresh clock page, initialized as UNKNOWN, mapped read-only at
/// `timetable::contract::CLOCK_VA`. Returns its frame.
fn map_a_clock_page(space: &mut AddressSpace) -> u64 {
    let phys = crate::memory::alloc_zeroed()
        .expect("no frame for a clock page")
        .addr();
    // SAFETY: a frame this test just allocated, named through the direct map.
    unsafe { clock_contract::ClockPage::new(mmu::phys_to_virt(phys)) }.init();
    space
        .map_physical(
            timetable::contract::CLOCK_VA,
            phys,
            Flags::user_rodata(),
            crate::revoke::PageMapSource::Capability(phys),
        )
        .expect("could not map the clock page");
    phys
}

/// The timetable's four fixed slots, each asserted to land where its contract says.
fn install_the_fixed_slots(
    tid: crate::thread::ThreadId,
    out: RendezvousId,
    budget: u64,
    child_report: RendezvousId,
    deaths: RendezvousId,
) {
    // Slot 0: where its own text goes. WRITE, so it can say things and cannot listen.
    let s = crate::sched::thread_control_block_insert_cap(
        tid,
        rendezvous_cap(out, Rights::WRITE),
        None,
    )
    .expect("insert out");
    assert_eq!(s, 0, "the timetable's output endpoint must land in slot 0");
    // Slot 1: what every instance is made of.
    let s =
        crate::sched::thread_control_block_insert_cap(tid, memory_region_root_cap(budget), None)
            .expect("insert budget");
    assert_eq!(s, 1, "the timetable's budget must land in slot 1");
    // Slot 2: what each instance is handed as its own slot 0. `GRANT` because handing it on is the
    // entire purpose; `WRITE` because a scheduled child reports and never listens.
    let s = crate::sched::thread_control_block_insert_cap(
        tid,
        rendezvous_cap(child_report, Rights::WRITE.union(Rights::GRANT)),
        None,
    )
    .expect("insert child report");
    assert_eq!(s, 2, "the child report endpoint must land in slot 2");
    // Slot 3: the supervision endpoint. `READ` is what `RECEIVE` and `Rendezvous::REAP` take (§32 (a supervisor may collect a corpse without being able to build one));
    // `GRANT` is what lets it be placed in each child's reserved fault slot.
    let s = crate::sched::thread_control_block_insert_cap(
        tid,
        rendezvous_cap(deaths, Rights::READ.union(Rights::GRANT)),
        None,
    )
    .expect("insert deaths");
    assert_eq!(s, 3, "the supervision endpoint must land in slot 3");
}

/// One line of `byte_sink_contract` bytes off `ep`, without its newline. `None` at end of stream.
fn line(ep: RendezvousId, buf: &mut [u8; 256]) -> Option<usize> {
    let mut len = 0usize;
    loop {
        let m = crate::sched::ipc_receive(ep);
        let mut chunk = [0u8; byte_sink_contract::INLINE_MAX];
        match byte_sink_contract::unpack(m[0], m[1], m[2], &mut chunk) {
            byte_sink_contract::Msg::Eof => return None,
            byte_sink_contract::Msg::Malformed => {
                panic!("the timetable wrote a malformed sink message")
            }
            byte_sink_contract::Msg::Bytes(n) => {
                for &b in &chunk[..n] {
                    if b == b'\n' {
                        return Some(len);
                    }
                    assert!(len < buf.len(), "the timetable printed a very long line");
                    buf[len] = b;
                    len += 1;
                }
            }
        }
    }
}

/// **A cron whose every entry is a grant: the plan is printed before anything fires, the entries
/// that were admitted really run under supervision, and the entries that were refused never run at
/// all** (milestone 129).
///
/// One test, because the three parts are one claim and none of them means anything alone. A
/// scheduler that fires proves nothing about authority; a printed plan proves nothing if what fires
/// disagrees with it; and a refusal proves nothing unless something else in the same document did
/// fire, since a scheduler that fired *nothing* would satisfy it trivially.
///
/// What it walks is the real program on the real document. `components/timetable.conf` is `include_str!`d
/// by `components/src/timetable.rs` and read again by `crates/timetable`'s host tests, so there is one
/// source for what this machine schedules and editing it moves the program, the host tests and this
/// assertion together.
///
/// # The four answers, and why the refusals are the interesting ones
///
/// Upstream cron has one answer to a crontab line: it runs. The shipped document exercises four:
///
/// - `least_authority_demo 7`, `least_authority_demo 3`, and `memory_grant_depleter --mem 4` are **planned, backed and fired**, and their
///   answers come back on the endpoint the plan said they would hold. The last of the three is
///   milestone 129's `--mem` grant made real: this timetable holds `timetable::SHIPPED_HELD.mem_pages`
///   pages it may split off, and `memory_grant_depleter` maps some of them and reports how many.
/// - `memory_grant_depleter` with no `--mem` and `wc` are refused by **the prompt's own check**, unchanged: the
///   same lines typed at a shell get the same sentences, because it is literally the same function.
/// - `date` and `ps` are refused because **this timetable holds nothing to back them**. Those two
///   are the ones Unix cannot refuse: there a clock and a process listing are ambient, so the lines
///   run and the question of whether they should have is one nothing asks.
///
/// # What is asserted about time, and what deliberately is not
///
/// The fires are **counted, never timed**. Nothing here says an entry fired within its interval, and
/// nothing compares a wall clock to anything: the test asks the timetable for [`FIRES`] fires and
/// waits for them. A loaded host makes this slower and cannot make it red, which is
/// notes/load-sensitive-assertions.md's rule applied at the point where it is easiest to get wrong.
#[test_case]
fn a_scheduled_entry_holds_what_the_plan_said_and_a_refused_one_never_runs() {
    let t = spawn_timetable(FIRES);
    let out = t.out;
    let mut buf = [0u8; 256];

    the_plan_is_complete_before_the_first_tick(out);

    only_the_admitted_entries_fired(&t);

    // ---- the summary, after every corpse has been collected ----
    //
    // The clean-exit count is the supervision half of the claim, and it is not decoration: each of
    // those children was born with the timetable's supervision endpoint in its reserved fault slot,
    // died, reported its death to the timetable, and had its region reclaimed through
    // `Rendezvous::REAP`. A scheduler that leaked a region per fire would still print the fire count
    // and would run out of budget instead of finishing.
    let Some(n) = line(out, &mut buf) else {
        panic!("the timetable ended its stream without a summary");
    };
    let s = core::str::from_utf8(&buf[..n]).expect("non-UTF-8 summary");
    assert_eq!(
        s, "timetable: 4 fires, 4 clean exits, 0 faults",
        "the summary must account for every child it started",
    );

    // The stream ends rather than stopping, and the verdict says it finished cleanly.
    assert!(
        line(out, &mut buf).is_none(),
        "the timetable said something after its summary",
    );
    assert_eq!(
        crate::sched::ipc_receive(out)[0],
        0,
        "the timetable's verdict word must be a clean finish",
    );
    t.reclaim();
}

/// The shipped document's plan, read from `out` up to the arming line: every grant, every
/// refusal, and the narrowed archive, each said before anything fires.
fn the_plan_is_complete_before_the_first_tick(out: RendezvousId) {
    let mut buf = [0u8; 256];
    // ---- the plan, printed before the first tick ----
    //
    // Read it to the arming line, keeping the facts the assertions below need. Reading it at all is
    // half the claim: the timetable is blocked on these sends until this loop takes them, so
    // nothing can have fired before the plan was complete.
    let mut saw_demo_grant = false;
    let mut saw_depleter_grant = false;
    let mut saw_nothing_else = false;
    let mut saw_exact_archive = false;
    let mut saw_wide_archive = false;
    let mut saw_clock_refusal = false;
    let mut saw_domain_refusal = false;
    let mut saw_mem_refusal = false;
    let mut saw_input_refusal = false;
    let mut armed = false;
    for _ in 0..256 {
        let Some(n) = line(out, &mut buf) else {
            panic!("the timetable's output ended before it armed");
        };
        let s = core::str::from_utf8(&buf[..n]).expect("the timetable printed non-UTF-8");
        if s == ARMED {
            armed = true;
            break;
        }
        saw_demo_grant |= s.contains("grants least_authority_demo exactly:");
        // The backed `--mem` grant, printed the same way: the plan names the program and, on the
        // next line, the page count split from this timetable's own budget (`write_grant`).
        saw_depleter_grant |= s.contains("grants memory_grant_depleter exactly:");
        // **The endowment, audited by the process that holds it.** `least_authority_demo` and `memory_grant_depleter` are the
        // only programs the shipped document admits, so a correctly narrowed archive carries
        // exactly two.
        saw_exact_archive |=
            s == "timetable: the archive it holds carries exactly the 2 programs its plan names";
        saw_wide_archive |= s.starts_with("timetable: the archive it holds carries ")
            && s.ends_with("of them beyond its plan");
        saw_nothing_else |=
            s.contains("and nothing else: no clock, no disk, no console, no network");
        saw_clock_refusal |= s.contains("this timetable holds no clock, so it cannot grant one");
        saw_domain_refusal |= s.contains("this timetable holds no process view to grant");
        // The prompt's own refusals, arriving unchanged. `memory_grant_depleter` with no `--mem` and `wc` with
        // nothing feeding it are wrong wherever they are typed.
        saw_mem_refusal |= s.contains(grant_plan::Refusal::MemoryGrantRequired.message());
        saw_input_refusal |= s.contains(grant_plan::Refusal::InputRequired.message());
    }
    assert!(
        armed,
        "the timetable printed more than a plan's worth of lines"
    );
    assert!(
        saw_demo_grant && saw_nothing_else,
        "the plan did not say what a scheduled least_authority_demo would hold",
    );
    assert!(
        saw_depleter_grant,
        "the plan did not say what the backed `--mem` grant would hold",
    );
    assert!(
        saw_clock_refusal,
        "a scheduled `date` must be refused for want of a clock, and said so. On Unix this entry \
         runs, because there the clock is ambient; here it is a capability this process was not \
         granted, and the refusal has to be legible before anything fires.",
    );
    assert!(
        saw_domain_refusal,
        "a scheduled `ps` must be refused for want of a process view",
    );
    assert!(
        saw_mem_refusal && saw_input_refusal,
        "the prompt's own refusals must arrive here unchanged",
    );
    // **The negative control for the endowment**, which is milestone 129's second stratum. Before
    // it, this spawn site handed the timetable the whole initial ramdisk and the program said so:
    // an archive reaching every program in the tree behind a plan that names two. After it, the
    // spawn site builds a sub-archive from `Registry::programs` and the program can no longer reach
    // `date`, `ps`, `swish` or anything else it will never run. Asserting both directions is what
    // makes this a control rather than a spelling check: a spawn site that quietly went back to the
    // initial ramdisk would trip the second assertion, not merely fail to trip the first.
    assert!(
        saw_exact_archive,
        "the timetable must be handed an archive holding exactly the programs its plan builds, and \
         must say so; a wider one is authority no line in the document asked for",
    );
    assert!(
        !saw_wide_archive,
        "the timetable reported an archive wider than its plan, so the spawn site handed it \
         programs it will never run",
    );
}

/// The [`FIRES`] reports that follow the plan: each one from an entry the plan admitted.
fn only_the_admitted_entries_fired(t: &Spawned) {
    // ---- what actually fired ----
    //
    // `least_authority_demo` squares its argument, so the shipped document's interval and at-boot entries answer
    // 49 (`least_authority_demo 7`) and 9 (`least_authority_demo 3`); `memory_grant_depleter --mem 4` answers however many of its four pages
    // it actually managed to map, which is at least one and at most four (page-table overhead is
    // free to differ between aarch64 and riscv64, so the exact count is not this milestone's claim;
    // `MEM_GRANT_PAGES` bounds it rather than pinning it). **This is the negative control**: every
    // refused entry in the document is a program that would have written something else here (`date`
    // and `wc` write `byte_sink_contract` bytes), so a document whose refusals had leaked would fail on
    // the value rather than on a count.
    let mut nines = 0;
    let mut forty_nines = 0;
    let mut depleter_reports = 0;
    for _ in 0..FIRES {
        let answer = report(t);
        match answer {
            9 => nines += 1,
            49 => forty_nines += 1,
            n if (1..=MEM_GRANT_PAGES).contains(&n) => depleter_reports += 1,
            other => panic!(
                "a scheduled child reported {other}; only `least_authority_demo 3`, `least_authority_demo 7` and \
                 `memory_grant_depleter --mem 4` were admitted, so this is an entry that fired after being \
                 refused",
            ),
        }
    }
    assert_eq!(
        nines, 1,
        "`at-boot least_authority_demo 3` must fire exactly once"
    );
    assert_eq!(
        depleter_reports, 1,
        "`at-boot memory_grant_depleter --mem 4` must fire exactly once, backed by the grant this timetable \
         holds",
    );
    assert_eq!(
        forty_nines,
        FIRES - 2,
        "the rest must be the repeating `least_authority_demo 7` heartbeat",
    );
}

/// **Stage `doc` in the page and publish request `seq`**: the registrar's half of
/// `timetable::registration`, which a durable session will run once milestone 152 (durable
/// delegation) rebuilds one. The document is written first and the request word last, with
/// release ordering, so a timetable that sees the new sequence sees the whole document.
fn send_replace(page: &mut [u8], seq: u64, doc: &[u8]) {
    use core::sync::atomic::{AtomicU64, Ordering};

    use timetable::registration as r;
    r::stage(page, doc).expect("the document does not fit the registration page");
    #[allow(clippy::cast_ptr_alignment)] // the page is page-aligned and REQUEST is word 0
    // SAFETY: `page` is the kernel's view of the frame mapped at `REGISTRATION_VA`, page-aligned,
    // so its first word is aligned for an `AtomicU64`; both sides touch that word only atomically.
    let request = unsafe { &*page.as_ptr().add(r::REQUEST).cast::<AtomicU64>() };
    request.store(r::request(r::REPLACE, seq), Ordering::Release);
}

/// What the timetable answered in the page: `(status, detail, verdicts, plan)`, read only once the
/// reply word shows `seq`. The caller has already seen the timetable's own line down `OUT`, which
/// is a rendezvous and so already orders everything; the acquire load is the protocol's rule
/// rather than this test's need.
fn read_reply(page: &[u8], seq: u64) -> (u64, u64, u64, &[u8]) {
    use core::sync::atomic::{AtomicU64, Ordering};

    use timetable::registration as r;
    #[allow(clippy::cast_ptr_alignment)] // the page is page-aligned and REPLY is word 2
    // SAFETY: as in `send_replace`, for the reply word.
    let reply = unsafe { &*page.as_ptr().add(r::REPLY).cast::<AtomicU64>() };
    assert_eq!(
        reply.load(Ordering::Acquire),
        seq,
        "the timetable has not answered request {seq}"
    );
    let word = |off: usize| u64::from_le_bytes(page[off..off + 8].try_into().unwrap());
    let plan = word(r::PLAN_LEN) as usize;
    (
        word(r::STATUS),
        word(r::DETAIL),
        word(r::VERDICTS),
        &page[r::BODY..r::BODY + plan],
    )
}

/// **Wait for the reply to request `seq`**, by the page's reply word: a registrar-mode timetable says
/// nothing down its output endpoint (`timetable::contract`), because its real registrar is a session
/// that cannot drain one. Fails at once, with the timetable's code, if it exits instead. No deadline,
/// for [`report`]'s reason.
fn await_reply(t: &Spawned, page: &[u8], seq: u64) {
    use core::sync::atomic::{AtomicU64, Ordering};

    use timetable::registration as r;
    #[allow(clippy::cast_ptr_alignment)] // the page is page-aligned and REPLY is word 2
    // SAFETY: as in `send_replace`, for the reply word.
    let reply = unsafe { &*page.as_ptr().add(r::REPLY).cast::<AtomicU64>() };
    while reply.load(Ordering::Acquire) != seq {
        if let Some(code) = exited(t) {
            panic!("waiting for the reply to request {seq}, the timetable exited with {code:#x}");
        }
        crate::sched::yield_now();
    }
}

/// **The stored schedule's archive-mode control**: `schedule_store::fixture::DEMO_SCHEDULE_DOC`'s
/// shape with the program renamed, because this fixture's timetable resolves its document in a
/// narrowed **archive** and plans each line against `grant_plan`'s static manifest for a program it
/// knows, and only image programs are known there. The store's own document names the suite's
/// installed fixture `scheduled_demo`, which §229 (how a bare name at the prompt reaches an
/// installed program) keeps off an image program's name and no
/// `grant_plan::Prog` knows, so a store-mode timetable plans it against the bytes' own manifest
/// instead; that difference is Fork 8 D's, and it is why this control copies the shape rather than
/// the very bytes. Provisional name.
const STORED_SCHEDULE_CONTROL: &str =
    "at-boot least_authority_demo 3\nevery 30s least_authority_demo 7\n";

/// **A running timetable's document is replaced whole, by its registrar, and a replacement that
/// fails changes nothing** (milestone 129 (scheduled execution), §222 (who holds a user's schedule)).
///
/// The kernel test stands in for the registrar, which will be a user's durable session once
/// milestone 152 rebuilds one. Three replacements, each the control for the others:
///
/// 1. **The stored schedule's shape**, [`STORED_SCHEDULE_CONTROL`]: what milestone 152's start-up
///    test writes to disk, with the program renamed, for the reason that constant's own doc gives.
///    §222's fifth sub-ruling is that the session writes the store and then replaces, so what the
///    store holds must be exactly what a replacement accepts, unedited. Its `at-boot` line fires
///    (9).
/// 2. **A document that does not parse.** Refused whole, with its line number in the page, and
///    nothing fires: the schedule in force is untouched.
/// 3. **An edit.** The `at-boot` line is resent byte for byte and keeps its beat, so it does not
///    fire a second time; the `every 30s` line is removed; a new `at-boot` line fires once (16); a
///    scheduled `date` is unbacked; and a new hourly line arms and never comes round.
/// 4. **An empty document**, which ends the timetable. It was asked for no fire count, so nothing
///    else could.
///
/// Counted, never timed, as the test above. One tolerance, stated rather than hidden: the stored
/// schedule's `every 30s` line is in force from the first replacement until the third, and a host
/// stalled for thirty seconds in between would let it fire. A 49 before the 16 is therefore
/// accepted and counted, and nowhere else; everything the test claims is about 9, 16 and the page.
#[test_case]
fn a_registrar_replaces_the_document_whole_and_a_failed_replacement_changes_nothing() {
    use timetable::registration as r;
    let mut t = spawn_timetable_with(&["least_authority_demo"], 0, true, false, false);
    let page = t.page.take().expect("a registrar-mode spawn maps a page");

    the_stored_shape_is_accepted_as_it_is(&t, page);

    // ---- 2. a replacement that does not parse ----
    send_replace(
        page,
        2,
        b"at-boot least_authority_demo 3\nevery fortnight least_authority_demo 7\n",
    );
    await_reply(&t, page, 2);
    let (status, detail, _, _) = read_reply(page, 2);
    assert_eq!(
        status,
        r::STATUS_PARSE,
        "a document that does not parse must be refused"
    );
    assert_eq!(detail, 2, "the page names the line that did not parse");

    an_edit_keeps_a_resent_line_and_fires_a_new_one(&t, page);

    // ---- 4. an empty document ends the timetable ----
    //
    // A timetable holding nothing would still hold its session up (the live-children rule of §16
    // (object revocation)), so emptying it is how it goes away. This run was asked for no fire
    // count at all, so this is the only way it can end.
    send_replace(page, 4, b"# nothing scheduled\n");
    await_reply(&t, page, 4);
    let (status, _, verdicts, _) = read_reply(page, 4);
    assert_eq!(status, r::STATUS_EMPTIED);
    assert_eq!(verdicts, 0);
    while exited(&t).is_none() {
        crate::sched::yield_now();
    }
    assert_eq!(
        exited(&t),
        Some(0),
        "a clean finish, in the page's exit word"
    );
    // It reached `exit` at all, which a timetable that had tried to speak down its output endpoint
    // could not have: nobody here reads it, so that `SEND` would still be blocked.
    assert_eq!(
        crate::sched::rendezvous_waiting_senders(t.out),
        0,
        "a registrar-mode timetable is silent on its output endpoint",
    );
    t.reclaim();
}

/// Replacement 1 of [`a_registrar_replaces_the_document_whole_and_a_failed_replacement_changes_nothing`].
fn the_stored_shape_is_accepted_as_it_is(t: &Spawned, page: &mut [u8]) {
    use timetable::registration as r;

    // ---- 1. the stored schedule's shape, unedited apart from the name ----
    //
    // The timetable started with an empty document; this is how its first one arrives
    // (`timetable::contract`).
    send_replace(page, 1, STORED_SCHEDULE_CONTROL.as_bytes());
    await_reply(t, page, 1);
    let (status, _, verdicts, plan) = read_reply(page, 1);
    assert_eq!(
        status,
        r::STATUS_REPLACED,
        "the stored schedule must be accepted as it is"
    );
    assert_eq!(
        r::verdict_of(verdicts, 0),
        r::KIND_FIRES,
        "at-boot least_authority_demo 3"
    );
    assert_eq!(
        r::verdict_of(verdicts, 1),
        r::KIND_FIRES,
        "every 30s least_authority_demo 7"
    );
    assert_eq!(r::verdict_of(verdicts, 2), r::KIND_NONE, "and nothing else");
    assert!(
        plan.starts_with(b"timetable: the plan, before anything fires"),
        "the plan comes back in the page, which is the registrar's whole view",
    );
    assert_eq!(report(t), 9, "the stored at-boot line fires once");
}

/// Replacement 3 of [`a_registrar_replaces_the_document_whole_and_a_failed_replacement_changes_nothing`].
fn an_edit_keeps_a_resent_line_and_fires_a_new_one(t: &Spawned, page: &mut [u8]) {
    use timetable::registration as r;

    // ---- 3. an edit ----
    send_replace(
        page,
        3,
        b"at-boot least_authority_demo 3\n\
          at-boot least_authority_demo 4\n\
          every 1s date\n\
          every 60m least_authority_demo 2\n",
    );
    await_reply(t, page, 3);
    let (status, _, verdicts, plan) = read_reply(page, 3);
    assert_eq!(status, r::STATUS_REPLACED, "the edit must be accepted");
    assert_eq!(
        r::verdict_of(verdicts, 0),
        r::KIND_FIRES | r::KEPT_PHASE,
        "a line resent byte for byte keeps its beat, and an at-boot line's beat is 'already fired'",
    );
    assert_eq!(
        r::verdict_of(verdicts, 1),
        r::KIND_FIRES,
        "a new line arms fresh"
    );
    assert_eq!(
        r::verdict_of(verdicts, 2),
        r::KIND_UNBACKED | (r::unbacked_code(timetable::Unbacked::Clock) << 2),
        "a scheduled date is unbacked in a timetable holding no clock",
    );
    assert_eq!(
        r::verdict_of(verdicts, 3),
        r::KIND_FIRES,
        "an interval that will not come round here"
    );
    let plan = core::str::from_utf8(plan).expect("the plan in the page is text");
    assert!(
        plan.contains("this timetable holds no clock, so it cannot grant one"),
        "the refusal's sentence travels in the page, which is why the verdict byte omits it",
    );
    assert!(
        !plan.contains("least_authority_demo 7"),
        "removing an entry is a replacement without its line",
    );

    // The new at-boot line fires once. A 49 can only be the stored schedule's 30-second line firing
    // before the edit removed it (see the doc comment), and it queued first if it did.
    loop {
        match report(t) {
            16 => break,
            49 => {}
            9 => panic!("the resent at-boot line fired again: its beat was not kept"),
            other => panic!("a scheduled child reported {other}, which nothing in force answers"),
        }
    }
}

/// **A timetable handed the run-unvouched capability runs nothing** (§220 (signed builds, and trusting a key is scoped), gate D2 of §219 (how the shell names an installed program to the spawner)).
///
/// A scheduled job fires long after the session that registered it proved anything, so it is the
/// one program §220's key-trust drop could not reach if it could run an unvouched image. The
/// timetable refuses at `_start`, before it plans or builds anything, because a timetable that
/// holds the capability is the only way a job of its could. The document is the shipped one, which
/// would otherwise fire, so an absent refusal fails on the verdict rather than passing quietly.
#[test_case]
fn a_timetable_holding_the_run_unvouched_capability_schedules_nothing() {
    let t = spawn_timetable_with(&PLANNED_PROGRAMS, FIRES, false, true, false);
    let out = t.out;
    let mut buf = [0u8; 256];
    let n = line(out, &mut buf).expect("the timetable must say why it refuses");
    let s = core::str::from_utf8(&buf[..n]).expect("non-UTF-8 refusal");
    assert!(
        s.starts_with("timetable: it holds the run-unvouched capability"),
        "the refusal comes first, before any plan: {s}",
    );
    assert!(line(out, &mut buf).is_none(), "and nothing after it");
    assert_eq!(crate::sched::ipc_receive(out)[0], 0xE304, "E_UNVOUCHED");
    t.reclaim();
}

/// 2026-10-06, a Tuesday, as a day number.
const TUESDAY: u64 = 20_732;

/// **Publish a wall time to a spawned timetable's clock page**, as the clock service would: the
/// offset from the counter that makes the page read `hh:mm:ss` on [`TUESDAY`]. `set` publishes as an
/// operator's `SET`, otherwise as an accepted `SYNCED` proposal. Each call is a new generation.
fn publish(t: &Spawned, set: bool, hh: u64, mm: u64, ss: u64) {
    publish_at(
        t,
        set,
        ((TUESDAY * 86_400) + hh * 3600 + mm * 60 + ss) * clock_contract::NANOS_PER_SEC,
    );
}

/// 02:00:00 on [`TUESDAY`], in wall nanoseconds: the occurrence the calendar test waits on.
const TWO_AM: u64 = (TUESDAY * 86_400 + 2 * 3600) * clock_contract::NANOS_PER_SEC;

/// **How far before 02:00 the calendar test publishes**, and so how long it waits for real time.
///
/// It was five seconds (01:59:55), paid twice per boot on every architecture. The lead is not what
/// the test proves: it only has to outlast the gap between this publication and the timetable's
/// next pass, since a pass that first reads the page at 02:00 or later arms the next day's
/// occurrence instead. A pass is one yield of a loop that never blocks here, so a second covers it
/// with room, and when it does not, [`fires_at_two`] publishes again instead of going red.
const LEAD: u64 = clock_contract::NANOS_PER_SEC;

/// **How long after 02:00 to wait for the report before publishing again.** Four seconds, so a
/// lead plus this is the five-second window the test always had for the timetable to read a
/// publication; see [`fires_at_two`] for why step 4 depends on that window and step 2 does not.
const MISSED: u64 = 4 * clock_contract::NANOS_PER_SEC;

/// **Publish `wall` (nanoseconds) to the clock page, and return the offset it published**, so a
/// caller can turn any later wall time on that page into a reading of the same counter.
fn publish_at(t: &Spawned, set: bool, wall: u64) -> u64 {
    let phys = t.clock.expect("a clock-granted spawn");
    let state = if set {
        clock_contract::state::SET
    } else {
        clock_contract::state::SYNCED
    };
    // SAFETY: the frame `spawn_timetable_with` allocated for this test, through the direct map.
    let page = unsafe { clock_contract::ClockPage::new(mmu::phys_to_virt(phys)) };
    let offset = clock_contract::offset_for(wall, super::clock_service::monotonic_nanos());
    page.publish(state, offset);
    offset
}

/// **Publish [`LEAD`] before 02:00, as an operator's `SET` if `set`, and return the 02:00 line's
/// report**, which must arrive at or after 02:00 on the page's clock.
///
/// The bound is one-sided, so it is not a timed assertion: a slow host only makes a report later.
/// It is what makes the wait mean something. Without it a timetable that fired a calendar line the
/// moment it saw the clock, a minute early, met no assertion here (it ended, measured 2026-10-06,
/// in a 60-second watchdog hang); the minute is the grammar's resolution, so that is the earliest
/// an early fire can be.
///
/// **A missed publication is published again, never failed.** A timetable whose first pass after
/// the publication came at 02:00 or later armed tomorrow's occurrence, and only a later generation
/// re-arms it. The retry is always `SYNCED`, which is safe whatever the timetable had done:
///
/// - It never read the first publication, or read it late and armed tomorrow: the retry is a new
///   generation before 02:00, so 02:00 arms and fires once.
/// - It armed 02:00 and has not fired yet: the retry re-arms the same occurrence, once.
/// - It fired and the report is slow: the stamp it took at 02:00 keeps the retry from firing again.
///   A `SET` retry would clear that stamp and fire twice, which is why a retry is never one.
///
/// So a step-4 `SET` the timetable never read at all is not retried into existence: the `SYNCED`
/// retry fires only if the stamp was cleared, which is the property step 4 claims. That case needs
/// no pass for [`LEAD`] plus [`MISSED`], five seconds, the same window the test had when it waited
/// from 01:59:55. Step 2 has no stamp to protect and is covered by the retry whatever the delay.
fn fires_at_two(t: &Spawned, set: bool) -> u64 {
    const ATTEMPTS: u32 = 5;
    let due = TWO_AM - publish_at(t, set, TWO_AM - LEAD);
    let mut retry_at = due + MISSED;
    for _ in 0..ATTEMPTS {
        if let Some(r) = report_until(t, retry_at) {
            let now = super::clock_service::monotonic_nanos();
            assert!(
                now >= due,
                "the 02:00 line reported {} ms before 02:00 on the page's clock",
                (due - now) / 1_000_000
            );
            return r;
        }
        retry_at = TWO_AM - publish_at(t, false, TWO_AM - LEAD) + MISSED;
    }
    panic!("the 02:00 line never fired across {ATTEMPTS} publications a second before it")
}

/// **A calendar entry keeps a time of day by a clock it was granted, and a step moves it by S3**
/// (milestone 129, G5 as ruled with its step rule; notes/scheduled-execution/calendar-grammar-g5.md).
///
/// The timetable is granted the clock page at `timetable::contract::CLOCK_SLOT`, starting UNKNOWN,
/// and the test is both its registrar and its clock. Counted, never timed: every assertion is the
/// identity of the next report, so a line that fired while the clock was unknown, or fired twice
/// after a backward step, puts a report where the next assertion expects another one.
///
/// 1. Registered while the clock is unknown: two calendar lines, admitted, and dormant.
/// 2. The clock becomes known a second before 02:00 and runs on: the 02:00 line fires when it gets
///    there, and not before ([`fires_at_two`]).
/// 3. A `SYNCED` step back to 01:30 fires nothing, then forward to 03:05: only the 03:00 line fires,
///    once, for a step that covered it. It is `date`, which gets the clock page too and prints the
///    day the page says.
/// 4. An operator `SET` back to a second before 02:00 clears the stamps, so 02:00 fires again when
///    the clock reaches it.
/// 5. An empty document ends the timetable.
///
/// Each step publishes at most once before the test waits on a report, except step 3's pair, whose
/// answer is the same whether or not the timetable saw the first of the two: the stamp from step 2
/// is what keeps 02:00 from firing, and it holds across either generation. [`fires_at_two`]'s
/// retries are the other exception, and its comment shows the same of each of them. That is
/// deliberate: nothing tells the test when the timetable has read the page, so no assertion may
/// depend on it.
#[test_case]
fn a_calendar_entry_keeps_time_by_its_granted_clock_and_a_step_moves_it_by_s3() {
    use timetable::registration as r;
    let mut t = spawn_timetable_with(&["least_authority_demo", "date"], 0, true, false, true);
    let page = t.page.take().expect("a registrar-mode spawn maps a page");

    // ---- 1. registered with the clock unknown ----
    send_replace(
        page,
        1,
        b"every day at 02:00 least_authority_demo 3\nevery day at 03:00 date\n",
    );
    await_reply(&t, page, 1);
    let (status, _, verdicts, plan) = read_reply(page, 1);
    assert_eq!(status, r::STATUS_REPLACED);
    assert_eq!(
        r::verdict_of(verdicts, 0),
        r::KIND_FIRES,
        "a clock-granted timetable admits it"
    );
    assert_eq!(
        r::verdict_of(verdicts, 1),
        r::KIND_FIRES,
        "and endows `date` with the clock"
    );
    assert!(
        core::str::from_utf8(plan)
            .unwrap()
            .contains("FREQ=DAILY;BYHOUR=2;BYMINUTE=0 (UTC)"),
        "the plan names the line as the RRULE it is",
    );

    // ---- 2. known just before 02:00, and the clock runs on to 02:00 ----
    assert_eq!(
        fires_at_two(&t, false),
        9,
        "02:00 fires when the clock reaches it"
    );

    // ---- 3. back to 01:30, then forward to 03:05 ----
    publish(&t, false, 1, 30, 0);
    publish(&t, false, 3, 5, 0);
    let mut buf = [0u8; 256];
    let n = line(t.reports, &mut buf).expect("the scheduled `date` said nothing");
    let said = core::str::from_utf8(&buf[..n]).expect("date printed non-UTF-8");
    assert!(
        said.contains("2026-10-06") && said.contains("03:05"),
        "the scheduled `date` read the clock page it was endowed with: {said}",
    );
    assert!(line(t.reports, &mut buf).is_none(), "and ended its stream");

    // ---- 4. an operator SET back before 02:00 ----
    assert_eq!(
        fires_at_two(&t, true),
        9,
        "a SET cleared the stamp, so 02:00 fires again as the clock reaches it"
    );

    // ---- 5. empty ----
    send_replace(page, 2, b"");
    await_reply(&t, page, 2);
    assert_eq!(read_reply(page, 2).0, r::STATUS_EMPTIED);
    while exited(&t).is_none() {
        crate::sched::yield_now();
    }
    assert_eq!(exited(&t), Some(0));
    t.reclaim();
}
