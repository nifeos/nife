//! `test_programs`: the program images, the bare spawn and reap, and the one progenitor-shaped
//! spawn, that more than one topic file of the userspace suite uses. Each is `pub(super)`, so it
//! reaches the other test modules under `user` and nothing outside `cfg(test)`.
//!
//! The in-kernel userspace suite runs on riscv64 as well as aarch64 (milestone 19 (run a real
//! workload)'s user-test port). What stays gated, and why, is written at each test, because a
//! blanket module comment outlives its reason: a property with no RISC-V analogue
//! (`el1_runs_on_sp_el1`), or one whose RISC-V twin in `riscv_virtio_tests` would be duplicated
//! rather than gained. See notes/riscv-parity-scope.md.
//!
//! Name: provisional, minted 2026-10-12 (UTC) by the lane for milestone 873 (the catch-all
//! userspace test file is split by topic).

use super::*;
use crate::cap::{Cap, Rights};
use crate::sched;

/// **The `image_self_checker` program's ELF bytes**: the loader's subject.
///
/// Four tests want a real user ELF to load, and they all want this one, so the structural half and
/// the behavioral half cannot drift apart. Three read its segment table (three segments, none
/// writable-and-executable, one with a `.bss`); the fourth runs it and asserts it did not fault,
/// which is the program checking those properties from the inside. The `.bss` is load-bearing:
/// without one, `the_initial_ramdisk_holds_a_native_executable` fails, because the zero-fill would
/// go untested.
pub(super) fn loader_subject_image() -> &'static [u8] {
    program("image_self_checker").expect("no image_self_checker program in the initial ramdisk")
}

/// **The `block_driver` binary's ELF bytes**: the virtio driver and its two DMA attackers, one
/// program on every architecture.
///
/// aarch64 and `x86_64`: the riscv64 leg drives these same properties through
/// `riscv_virtio_tests`, which has its own `blk_image` naming the same program, and `x86_64` runs
/// the two PCIe tests in this module (it has no `riscv_virtio_tests`).
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
pub(super) fn blk_image() -> &'static [u8] {
    program("block_driver").expect("no block_driver program in the initial ramdisk")
}

/// The `kernel_test_subject` program's ELF bytes: the two privilege-boundary behaviors. See
/// [`KERNEL_TEST_SUBJECT_ROUND_TRIP`].
pub(super) fn kernel_test_subject_image() -> &'static [u8] {
    program("kernel_test_subject").expect("no kernel_test_subject program in the initial ramdisk")
}

/// **An address in the kernel's own memory: mapped, readable by the kernel, forbidden to
/// userspace.** The address of a kernel static, rather than a per-ISA constant for the kernel's
/// text base, because taking the address of something the kernel is demonstrably using is true
/// on any ISA and any link layout, and needs no table of magic numbers to keep current.
pub(super) fn a_kernel_address() -> u64 {
    static SENTINEL: u64 = 0x00C0_FFEE_D00D;
    &raw const SENTINEL as u64
}

/// Run `image` at user mode with `arg0`/`arg1` and no authority at all: no capabilities, no
/// extra mappings. It can run its own code and touch its own memory and name nothing else.
pub(super) fn spawn_bare(
    image: &'static [u8],
    arg0: u64,
    arg1: u64,
) -> Option<crate::thread::ThreadId> {
    sched::spawn(move || {
        run(
            image,
            Spawn {
                arg0,
                arg1,
                arg2: 0,
                grants: &[],
                maps: &[],
            },
        )
    })
}

/// **Take back a thread from [`spawn_bare`] that will never exit on its own.**
///
/// A bare thread belongs to no reclaimable region, so nothing tears it down when its test returns,
/// and a spinner left running loads every later test: two on a four-hart machine starved
/// `reclaim_frees_a_started_then_exited_childs_regions` into its watchdog on CI. See
/// `notes/riscv-parity-scope.md`. The kill is armed rather than immediate (DECISIONS §16 (object
/// revocation)), so this waits for the name to stop resolving. Returns whether the thread went;
/// callers assert on it, because a silent failure here restores the leak this exists to remove.
pub(super) fn reap_bare(tid: crate::thread::ThreadId) -> bool {
    if !sched::kill_thread(tid) {
        // Already gone. Nothing to wait for, and not a failure.
        return true;
    }
    wait_for(|| !sched::is_thread_present(tid))
}

/// **What a progenitor-shaped spawn leaves the test that started it.**
///
/// The report rendezvous and its region are not here: the caller made them (it may have carved
/// more from the region than the report line, as `live_swap_tests` does) and keeps them.
pub(super) struct Started {
    /// The program's thread. Its space is bound to it and goes when it is reaped.
    pub tid: crate::thread::ThreadId,
    /// The budget region, slot 0's object, for `USAGE` reads and reclaim.
    pub budget: u64,
    /// The region the thread control block was retyped from.
    pub thread_control_block_region: u64,
}

/// The pages an ELF's loadable segments occupy: the first term of every hand-built space's budget
/// here, with the stack, the archive window and slack each site adds for itself.
pub(super) fn segment_pages(elf: &Elf) -> u64 {
    elf.segments()
        .map(|seg| {
            let (s, e) = seg.page_range(FRAME_SIZE);
            (e - s) / FRAME_SIZE
        })
        .sum()
}

/// Lay `elf` out in `space`, map `stack_pages` of stack ending at [`USER_STACK_VA`], and on x86_64
/// and riscv64 map the timebase page. The part of a progenitor-shaped spawn that
/// [`spawn_progenitor_shaped`] and the timetable's own spawn share; the stack size is each site's
/// stated choice, never inherited. `name` only words the panic messages.
pub(super) fn lay_out_program(space: &mut AddressSpace, elf: &Elf, stack_pages: u64, name: &str) {
    map_segments(space, elf).unwrap_or_else(|e| panic!("could not lay out {name}: {e}"));
    for k in 0..stack_pages {
        space
            .map_new(USER_STACK_VA - k * FRAME_SIZE, Flags::user_data())
            .unwrap_or_else(|e| panic!("could not map {name}'s stack: {e:?}"));
    }
    #[cfg(any(target_arch = "x86_64", target_arch = "riscv64"))]
    map_timebase_page(space)
        .unwrap_or_else(|e| panic!("could not map {name}'s timebase page: {e:?}"));
}

/// **One progenitor-shaped spawn for the system tests** (milestone 794 (one progenitor-shaped spawn
/// for the system tests)): start `name` as the kernel starts the progenitor, so a harness proves
/// the program's choices rather than a privileged shortcut. Lays out the ELF, a stack and, on
/// x86_64 and riscv64, the timebase page ([`lay_out_program`]), maps the initial ramdisk read-only
/// at [`INITIAL_RAMDISK_VA`], puts the budget in slot 0, `report` in slot 1 and `further_caps`
/// after in order (each named for its failure message), binds `bound_notification` between
/// configure and start (the only order without a race), and starts with the archive's length in
/// `x1`. Returns once the program is running. The design is milestone 794's roadmap block; the
/// bind's reason is in notes/system-tests-and-the-kernel-crate.md.
///
/// Name: provisional (milestone 794's lane, 2026-10-16 UTC).
#[allow(clippy::too_many_arguments)] // the endowment's rights and the two beyond-the-floor extensions are each a real part of the shape
pub(super) fn spawn_progenitor_shaped(
    name: &str,
    budget_pages: u64,
    budget_rights: Rights,
    report: sched::RendezvousId,
    report_rights: Rights,
    further_caps: &[(Cap, &str)],
    bound_notification: Option<sched::NotificationId>,
    x0: u64,
) -> Started {
    let (initial_ramdisk_start, initial_ramdisk_len) =
        memory::initial_ramdisk_region().expect("no initial ramdisk region");
    let initial_ramdisk_pages = initial_ramdisk_len.div_ceil(FRAME_SIZE);
    let Some(bytes) = program(name) else {
        panic!("no {name} program in the initial ramdisk");
    };
    let Ok(elf) = Elf::parse(bytes) else {
        panic!("{name} is not loadable");
    };

    // The budget every hand-built space here counts: each segment's pages, a table share for the
    // archive window, the window's log pages (`map_physical` records), the stack, and slack.
    let content: u64 = segment_pages(&elf)
        + 1
        + initial_ramdisk_pages / 512
        + crate::revoke::log_pages_for(initial_ramdisk_pages)
        + PROGENITOR_STACK_PAGES
        + 8;
    let mut space = AddressSpace::new(content).unwrap_or_else(|| panic!("no memory for {name}"));
    lay_out_program(&mut space, &elf, PROGENITOR_STACK_PAGES, name);
    // The ramdisk is reserved RAM the frame allocator does not own, so this maps rather than
    // allocates, one read-only page at a time, exactly as `spawn_elf_child_builder` maps it.
    for i in 0..initial_ramdisk_pages {
        space
            .map_physical(
                INITIAL_RAMDISK_VA + i * FRAME_SIZE,
                initial_ramdisk_start + i * FRAME_SIZE,
                Flags::user_rodata(),
                crate::revoke::PageMapSource::NoCapability,
            )
            .unwrap_or_else(|e| panic!("could not map the initial ramdisk into {name}: {e:?}"));
    }
    let aspace = readopt_user_address_space(space)
        .unwrap_or_else(|| panic!("could not register {name}'s space"));

    let budget = crate::memory_region::create(budget_pages)
        .unwrap_or_else(|| panic!("no budget for {name}"));
    let thread_control_block_region =
        crate::memory_region::create(2).expect("no region for a thread control block");
    let tid = sched::create_thread_control_block(thread_control_block_region)
        .unwrap_or_else(|| panic!("no thread control block for {name}"));
    let slot = sched::thread_control_block_insert_cap(
        tid,
        crate::cap::memory_region_cap_rights(budget, budget_rights),
        None,
    )
    .expect("insert the budget");
    assert_eq!(slot, 0, "{name}'s budget must land in slot 0");
    let slot = sched::thread_control_block_insert_cap(
        tid,
        crate::cap::rendezvous_cap(report, report_rights),
        None,
    )
    .expect("insert the report endpoint");
    assert_eq!(slot, 1, "{name}'s report endpoint must land in slot 1");
    for (i, (capability, what)) in further_caps.iter().enumerate() {
        let slot = sched::thread_control_block_insert_cap(tid, *capability, None)
            .unwrap_or_else(|e| panic!("could not insert {name}'s {what}: {e:?}"));
        assert_eq!(
            slot,
            (i + 2) as u64,
            "{name}'s {what} must land in slot {}",
            i + 2
        );
    }
    sched::configure_thread_control_block(tid, elf.entry(), USER_STACK_TOP, aspace)
        .unwrap_or_else(|e| panic!("could not configure {name}: {e:?}"));
    if let Some(notification) = bound_notification {
        sched::notification_bind(notification, tid)
            .unwrap_or_else(|e| panic!("{name}'s notification would not bind: {e:?}"));
    }
    sched::start_thread_control_block(tid, [x0, initial_ramdisk_len, 0])
        .unwrap_or_else(|e| panic!("could not start {name}: {e:?}"));
    Started {
        tid,
        budget,
        thread_control_block_region,
    }
}

/// The `std_exerciser` std program's ELF bytes. The same binary the offline std test spawns; given
/// the network here, its `UdpSocket::bind` probe succeeds and it runs the net transcript.
#[cfg(target_arch = "aarch64")]
pub(super) fn std_exerciser_image() -> &'static [u8] {
    program("std_exerciser").expect("no std_exerciser program in the initial ramdisk")
}
