//! `test_programs`: the program images, the bare spawn and reap, and the one progenitor-shaped
//! spawn, that more than one topic file of the userspace suite uses. Each is `pub(super)`, so it
//! reaches the other test modules under `user` and nothing outside `cfg(test)`.
//!
//! **The in-kernel userspace suite, on both instruction sets** (milestone 19 (run a real
//! workload)'s user-test port).
//!
//! It was aarch64-only for most of this project's life, and the module comment used to say the
//! reason was the tests: "every test drives a hand-written aarch64 program through `exec` and reads
//! aarch64 fault registers". That was true, and it was the wrong thing to fix. The tests were fine;
//! their *scaffolding* was aarch64. Three things moved and the tests came along unchanged:
//!
//! 1. The hand-assembled programs became real ELFs the toolchain builds for both targets (the
//!    `outlaw` binary and the `interrupt_ignorer` that already existed). See the note above
//!    `OUTLAW_ROUND_TRIP`.
//! 2. `ESR`/`FAR` became `arch::UserFault`, the same fact in words RISC-V can say, which is what
//!    keeps "a PERMISSION fault at exactly this address" assertable rather than softened to "a fault
//!    happened".
//! 3. `hello`, which carries the milestone 7-19 role catalogue (milestone 7 (user mode: EL0,
//!    capabilities, the ELF loader, and IPC) onward), was found to build for RISC-V once
//!    six syscalls it had hand-rolled in aarch64 `asm!` were routed through `user_mode_runtime`, which already
//!    had portable versions of all six.
//!
//! **What is still gated, and why, is written at each test rather than here**, because a blanket
//! module comment is how the old claim survived past the point of being true. Two kinds of gate
//! appear below: a property that has no RISC-V analogue at all (`el1_runs_on_sp_el1`), and a
//! property whose RISC-V twin lives in `riscv_virtio_tests` and would be duplicated rather than
//! gained. See notes/riscv-parity-scope.md.
//!
//! Name: provisional, minted 2026-10-12 (UTC) by the lane for milestone 873 (the catch-all
//! userspace test file is split by topic).

use super::*;
use crate::cap::{Cap, Rights};
use crate::sched;

/// **The `image_self_checker` program's ELF bytes**: the loader's subject.
///
/// Four tests want *a real user ELF to load* rather than a particular program, and they all want
/// the same one, which is the point of naming it once. Three read its segment table (three
/// segments, none writable-and-executable, one with a `.bss`) and the fourth runs it and asserts
/// it did not fault, which is that program checking those very properties from the inside. Naming
/// two different binaries there would let the structural half and the behavioural half drift apart.
///
/// **The `.bss` requirement is load-bearing and nearly went missing.** It was `hello`'s bytes until
/// milestone 291, and splitting hello's roles out took its last `static mut X = 0` with them, so
/// the binary stopped having a `.bss` at all and `the_initial_ramdisk_holds_a_native_executable`
/// said so: "no segment has a .bss, so the zero-fill is untested". The vacuity guard fired exactly
/// as written, which is why it is there.
///
/// **`x86_64` packs no initial ramdisk at all**, because no user program is built for
/// `x86_64-unknown-none` (`crates/user_mode_runtime` has no arms for this ISA; see notes/x86-port.md). Every
/// test that reaches for this skips instead.
pub(super) fn loader_subject_image() -> &'static [u8] {
    program("image_self_checker").expect("no image_self_checker program in the initial ramdisk")
}

/// **The `block_driver` binary's ELF bytes**: the virtio driver and its two DMA attackers, one
/// program on every architecture since milestone 291.
///
/// This read a role of `hello` on aarch64 until then: that binary carried seven virtio roles which
/// dispatched into the same `crates/virtio` the dedicated binary uses. `riscv_virtio_tests` has
/// always named `block_driver` here; the two now name the same thing.
///
/// aarch64 and `x86_64`: the riscv64 leg drives these same properties through
/// `riscv_virtio_tests`, which has its own `blk_image` naming the same program, and `x86_64` runs
/// the two PCIe tests in this module (it packs RISC-V's archive but has no `riscv_virtio_tests`).
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
pub(super) fn blk_image() -> &'static [u8] {
    program("block_driver").expect("no block_driver program in the initial ramdisk")
}

/// The `outlaw` program's ELF bytes: the two privilege-boundary behaviours that used to be
/// hand-assembled aarch64 machine code in this file. See [`OUTLAW_ROUND_TRIP`].
pub(super) fn outlaw_image() -> &'static [u8] {
    program("outlaw").expect("no outlaw program in the initial ramdisk")
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
/// A bare thread belongs to no reclaimable region, so nothing tears it down when the test that
/// started it returns. For a subject whose whole point is that it never exits (a spinner, or a
/// child holding a measurement still), "the test ended" and "the thread ended" are unrelated
/// events, and the thread just keeps running.
///
/// That is not merely untidy. Two such threads spinning on a four-hart machine is a load every
/// later test in the suite then runs under, and it starved
/// `reclaim_frees_a_started_then_exited_childs_regions` into its watchdog on CI, intermittently,
/// nowhere near the tests responsible. See `notes/riscv-parity-scope.md`.
///
/// The kill is armed rather than immediate (DECISIONS §16), so this waits for the name to stop
/// resolving instead of assuming the thread is gone on return. Returns whether it actually went;
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
/// for the system tests)).
///
/// Start `name` the way the kernel starts the progenitor: its ELF laid out in a hand-built space,
/// a stack, the timebase page on x86_64 and riscv64, the whole initial ramdisk mapped read-only
/// at [`INITIAL_RAMDISK_VA`], the budget's region capability in slot 0 and `report` in slot 1,
/// configure, and start with the archive's length in `x1`. Returns once the program is running.
/// A harness that spawns this way is proving the program's *choices* rather than a privileged
/// shortcut, which is why every copy this replaces said "deliberately the same endowment
/// `spawn_elf_child_builder` gives" over its own body.
///
/// The four copies it replaced (`authority_tests::spawn_tree`, `c_seam_tests::spawn_confiner`,
/// `live_swap_tests::spawn_swapper`, `process_composition_service::wire`) agreed line for line
/// apart from the names inside their `.expect` messages, and every one of them carried the space
/// budget's `log_pages_for(initial_ramdisk_pages)` term, so a change to that accounting was an edit
/// to each. The kernel's own `spawn_elf_child_builder` is still a sixth copy and is deliberately
/// not folded here: it also measures the program and routes the boot interrupts, which no test
/// harness wants. `timetable_tests::spawn_timetable_with` is the call site that stayed whole: its
/// slots are its own four-slot contract (output, budget, child report, deaths), its archive is a
/// narrowed copy it builds rather than the initial ramdisk, and its stack, extra mappings and start
/// arguments are its own. It shares [`segment_pages`] and [`lay_out_program`] with this function
/// and nothing else; a helper parameterized to cover the rest would be `spawn_timetable_with`
/// wearing this one's name.
///
/// [`crate::user::run`] cannot serve these callers: its extra mapped pages arrive as a slice,
/// and the archive is a few thousand pages the suite has no allocator to build a slice from.
/// That is why the space is hand-built here, the same as at every site this replaces.
///
/// The parameters past `report_rights` exist for the two callers that hold more than the
/// progenitor's floor. `further_caps` are inserted one apiece in the slots after 1, in order,
/// each paired with the name its failure message should carry. `bound_notification` is bound to
/// the thread between configure and start, because that is the only order with no race in it: a
/// program that reached a wait before its notification was bound would park outside the
/// bind-delivers-what-was-counted rule. The bind is the spawner's to make (a running thread holds
/// no capability to its own TCB), the same hand `net_stack`'s wiring performs for its retransmit
/// wait.
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
        + INIT_STACK_PAGES
        + 8;
    let mut space = AddressSpace::new(content).unwrap_or_else(|| panic!("no memory for {name}"));
    lay_out_program(&mut space, &elf, INIT_STACK_PAGES, name);
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
