//! Bringing the other cores online, and the roster of which cores exist (DECISIONS §11).
//!
//! The boot core reads the machine's core list ([`read_cpu_list`]), then starts each secondary
//! ([`bring_up_secondaries`]) at a physical entry point (`secondary_boot` in boot.s) on its own
//! stack. The secondary replays the MMU-enable and lands in [`secondary_main`], which makes it a
//! full scheduler participant. Cross-core placement goes through a core's inbox and the reschedule
//! IPI (`sched::spawn_on`); load balancing is `sched::spawn`'s (DECISIONS §28).
//!
//! See notes/smp.md.

use core::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

// The device-tree roster; on `x86_64` only the tests name it.
#[cfg(any(test, not(target_arch = "x86_64")))]
use machine_discovery::cpu_list::CpuList;

use crate::cpu::{self, MAX_CPUS};
use crate::{arch, print, println};

/// 64 KiB per secondary, matching core 0's boot stack (link-aarch64.ld).
const SECONDARY_STACK_SIZE: usize = 64 * 1024;

/// The unmapped page below each secondary stack.
///
/// One page of address space per core and **zero physical frames**: `map_everything` skips it, so
/// the MMU faults on the first byte written past the bottom of the stack. See notes/smp.md.
const SECONDARY_STACK_GUARD: usize = 4096;

/// A core's slot: its guard page, then the stack that grows down into it. A multiple of the page
/// size, so slot `n`'s guard is page-aligned given the region is.
const SECONDARY_STACK_SLOT: usize = SECONDARY_STACK_GUARD + SECONDARY_STACK_SIZE;

/// One boot stack per core, each over its own guard page. Slot 0 is unused (core 0 has its own
/// stack from link-aarch64.ld); slots `1..MAX_CPUS` belong to the secondaries. On RISC-V the boot
/// hart is whichever one OpenSBI picked, so the unused slot is not always 0.
///
/// The `UnsafeCell` keeps the static out of `.rodata`, which the fine kernel map makes read-only.
/// The `link_section` gives the stacks their own `(NOLOAD)` region, so the mapper can map each
/// stack and skip each guard; nothing zeroes it. The linker scripts only anchor
/// `__secondary_stacks_start`..`__secondary_stacks_end` around it. See notes/smp.md.
#[repr(C, align(4096))]
struct Stacks(core::cell::UnsafeCell<[[u8; SECONDARY_STACK_SLOT]; MAX_CPUS]>);

// SAFETY: each core writes only its OWN slot, as its stack via SP, so no two cores mutate the same
// bytes. The cell exists to place the stacks in writable memory, not to share them.
unsafe impl Sync for Stacks {}

#[cfg_attr(target_os = "none", unsafe(link_section = ".secondary_stacks"))]
static SECONDARY_STACKS: Stacks = Stacks(core::cell::UnsafeCell::new(
    [[0; SECONDARY_STACK_SLOT]; MAX_CPUS],
));

/// How many secondaries have reached [`secondary_main`] and are idling.
static ONLINE: AtomicUsize = AtomicUsize::new(0);

/// A bitmask of the online cores, by id. The boot core sets its bit before bring-up; each secondary
/// sets its own as it comes online. RISC-V uses it as the hart mask for SBI RFENCE TLB shootdown
/// (aarch64's `tlbi ..., is` broadcasts in hardware and needs no mask). Kept here, portable, because
/// which cores are online is a scheduler fact, not an arch one.
static ONLINE_MASK: AtomicUsize = AtomicUsize::new(0);

/// How many cores are online: the boot core plus the secondaries that came up. Used to spread work
/// (SMP step 3c) without baking in a core count.
pub fn online_count() -> usize {
    ONLINE.load(Ordering::Acquire) + 1
}

/// The bitmask of online cores (by id). RISC-V's TLB shootdown and icache push target every online
/// core but the one doing the flush (aarch64's `tlbi ..., is` broadcasts in hardware and needs no
/// mask there); [`online_cpus`] is this mask as an iterator on both ISAs.
pub fn online_harts_mask() -> usize {
    ONLINE_MASK.load(Ordering::Acquire)
}

/// The online cpus, by id, in ascending order: [`online_harts_mask`] as an iterator. The online
/// set is not contiguous from zero on real boards (the VisionFive 2's is {1,2,3,4}), so iterate
/// the set, never `0..online_count()`. The mask arithmetic is `crates/cpu_set`'s, host-tested.
/// See notes/smp.md.
pub fn online_cpus() -> cpu_set::Cpus {
    cpu_set::cpus_in(online_harts_mask())
}

/// The k-th online cpu, wrapping: the sampler's safe form of "a random online cpu". Falls back to
/// the calling cpu only if the mask is somehow empty (before the boot core registers itself).
pub(crate) fn nth_online(k: usize) -> usize {
    cpu_set::nth_in(online_harts_mask(), k).unwrap_or(crate::cpu::id())
}

/// Set by each secondary's probe thread, indexed by the core it actually ran on. The proof that a
/// secondary schedules real work from its own queue, not just idles. See `secondary_main` step 5.
#[cfg(any(test, feature = "system_tests"))]
static RAN_ON: [core::sync::atomic::AtomicBool; MAX_CPUS] =
    [const { core::sync::atomic::AtomicBool::new(false) }; MAX_CPUS];

/// Set by a probe placed on a specific core via `spawn_on`: indexed by the core it was **placed
/// on**, holding the core it actually **ran on**, plus one (so zero means "has not run yet"). The
/// two differ because an idle core may steal a placed thread before its target runs it (DECISIONS
/// §28.3). See `work_can_be_placed_on_every_cpu`.
#[cfg(test)]
static SPREAD: [core::sync::atomic::AtomicU32; MAX_CPUS] =
    [const { core::sync::atomic::AtomicU32::new(0) }; MAX_CPUS];

/// The low end of core `id`'s slot, which is its **guard page**: one page of address space the
/// kernel map deliberately leaves unmapped. `arch::mmu::map_everything` skips exactly this page per
/// core, and `verify` refuses to install a map in which it translates.
pub(crate) fn secondary_stack_guard(id: usize) -> u64 {
    SECONDARY_STACKS.0.get() as u64 + (id as u64) * SECONDARY_STACK_SLOT as u64
}

/// Core `id`'s usable stack as `(bottom, top)`: the slot above its guard page. Used by the mapper
/// (what to map) and by the high-water report (what to paint and scan, milestone 84). The boot
/// core's slot exists but is never painted and never used; the report skips it.
pub(crate) fn secondary_stack_span(id: usize) -> (u64, u64) {
    let bottom = secondary_stack_guard(id) + SECONDARY_STACK_GUARD as u64;
    (bottom, bottom + SECONDARY_STACK_SIZE as u64)
}

/// The whole region, `(start, end)`, for the check against what the linker reserved.
#[cfg(test)]
fn secondary_stack_region() -> (u64, u64) {
    let start = SECONDARY_STACKS.0.get() as u64;
    (start, start + (MAX_CPUS * SECONDARY_STACK_SLOT) as u64)
}

/// The high-VA top of core `id`'s boot stack. Stacks grow down, so the top is the high address, and
/// the 16-byte alignment `sp` requires comes from the region's page alignment and every size here
/// being a multiple of 16. See notes/stack.md.
fn stack_top(id: usize) -> u64 {
    secondary_stack_span(id).1
}

/// The hardware id of each core the machine described, indexed by logical id. `u64::MAX` in a slot
/// the tree did not fill. Written once by [`read_cpu_list`], before any secondary exists.
///
/// An "id" here is whatever the bring-up call names a core by: an `MPIDR_EL1[39:0]` affinity value
/// on aarch64, a hart id on RISC-V. Nothing portable interprets it; it is read out of `/cpus` and
/// handed back to `arch::cpu_start`.
///
/// `u64::MAX` is safe as the empty marker rather than merely convenient: an aarch64 affinity value
/// is 40 bits by architecture, so it cannot reach it, and a RISC-V hart id that could would fail the
/// hardware-id-equals-index check in [`bring_up_secondaries`] long before anything indexed by it.
static HWID: [AtomicU64; MAX_CPUS] = [const { AtomicU64::new(u64::MAX) }; MAX_CPUS];

/// Per slot: did the tree describe a core this kernel could actually start? False for a core
/// firmware marked `status = "disabled"`, for one whose own `riscv,isa` disclaims supervisor mode
/// (the VisionFive 2's S7, whose vendor node claims a schedulable core everywhere else), and
/// for one whose `enable-method` is a mechanism this kernel does not speak. See notes/smp.md.
static STARTABLE: [AtomicBool; MAX_CPUS] = [const { AtomicBool::new(false) }; MAX_CPUS];

/// How many `cpu@` nodes the tree had, which may be more than the slots that got seated. Kept so
/// the boot line can say "this machine has more cores than this kernel was built for" instead of
/// quietly using fewer.
static DESCRIBED: AtomicUsize = AtomicUsize::new(0);

/// **Read the machine's core list out of its device tree.**
///
/// Called once, on the boot core, before [`bring_up_secondaries`] and before `mmu::init` replaces
/// the coarse boot map the tree is reached through. Seats each described core at the slot its
/// hardware id names and prints the roster; it starts nothing. See notes/smp.md.
///
/// # Panics
///
/// If the blob is unreadable. `memory::init` has already parsed the same pointer, so that is a
/// kernel bug, not a machine to tolerate.
#[cfg(not(target_arch = "x86_64"))]
pub(crate) fn read_cpu_list() {
    let dt = crate::device_tree().expect("device tree is unreadable");
    let list = CpuList::from_device_tree(&dt).expect("cannot read /cpus");

    // Seat each core at the slot its hardware id names: logical id equals hardware id by
    // construction, and an excluded core costs no other core its seat. See notes/smp.md.
    let mut startable = 0;
    let mut unseated = 0; // startable cores whose hart id names no slot
    for cpu in list.cpus().iter() {
        let ok = cpu.is_startable();
        startable += usize::from(ok);
        let Some(slot) = usize::try_from(cpu.hwid).ok().filter(|&s| s < MAX_CPUS) else {
            // Its id is past the per-CPU statics, as every clustered aarch64 affinity value is.
            // Counted only when we could otherwise have run it.
            unseated += usize::from(ok);
            continue;
        };
        if HWID[slot].load(Ordering::Relaxed) != u64::MAX {
            // Two nodes claiming one id is a malformed tree (or `cpu_list`'s missing-`reg`
            // fallback of zero colliding with the real hart 0). First claim wins; say so.
            println!("  smp: two cpu@ nodes claim hart id {slot}; keeping the first");
            continue;
        }
        HWID[slot].store(cpu.hwid, Ordering::Relaxed);
        STARTABLE[slot].store(ok, Ordering::Relaxed);
    }
    DESCRIBED.store(list.described, Ordering::Release);

    print!("  smp: {} core(s) in the device tree", list.described);
    if startable != list.described {
        print!(", {startable} startable");
    }
    if unseated > 0 {
        print!(
            " ({unseated} of them with hart ids past cpu::MAX_CPUS = {MAX_CPUS}, which sizes the \
             per-CPU statics, so they stay parked)"
        );
    }
    println!();

    // The only place a hart excluded for missing supervisor mode is visible: the VisionFive 2's
    // vendor tree claims its S7 is schedulable in both status and mmu-type.
    for cpu in list.cpus().iter() {
        if !cpu.supervisor {
            println!(
                "  smp: cpu {}'s riscv,isa names user mode and not supervisor: it cannot run \
                 this kernel, whatever status and mmu-type claim; not started",
                cpu.hwid,
            );
        }
    }

    // The core we are executing on has to be in the roster at its own index, or the roster describes
    // a machine other than the one running this code. Nothing downstream can recover from that, but
    // saying it beats letting the bring-up report a plausible number.
    let boot = arch::boot_cpu_id();
    if hwid(boot) != Some(boot as u64) {
        println!(
            "  smp: the boot core is logical {boot}, and the device tree does not put a core with \
             that id there"
        );
    }
}

/// **Read the machine's core list out of the ACPI MADT**: `x86_64`'s [`read_cpu_list`], the same
/// step at the same point in `bring_up_machine`, reading the walk `memory::init` kept
/// (`arch::discovery`) rather than a device tree.
#[cfg(target_arch = "x86_64")]
pub(crate) fn read_cpu_list() {
    let (cpus, count) = arch::discovery::with_acpi(|a| (a.cpus, a.cpu_count));
    seat_cpus_from_acpi(&cpus[..count]);
}

/// **Seat the cores the ACPI MADT lists**: [`read_cpu_list`]'s seating, over the MADT's
/// `LocalApic` entries.
///
/// `cpus` is `(local_apic_id, enabled)` per entry, in table order
/// (`arch::machine::Acpi::cpus[..cpu_count]`). Each core is seated at the slot its local APIC id
/// names, and `enabled` is what [`STARTABLE`] takes. Called once, by `x86_64`'s [`read_cpu_list`],
/// before [`bring_up_secondaries`]. See notes/smp.md.
#[cfg(target_arch = "x86_64")]
fn seat_cpus_from_acpi(cpus: &[(u8, bool)]) {
    let mut startable = 0;
    let mut unseated = 0; // startable cores whose local apic id names no slot

    for &(apic_id, enabled) in cpus {
        let slot = apic_id as usize;
        if slot >= MAX_CPUS {
            // Past the per-CPU statics; counted only when we could otherwise have run it.
            unseated += usize::from(enabled);
            continue;
        }
        if HWID[slot].load(Ordering::Relaxed) != u64::MAX {
            // Two entries claiming one local apic id is a malformed table. First claim wins; say so.
            println!("  smp: two madt entries claim local apic id {slot}; keeping the first");
            continue;
        }
        HWID[slot].store(apic_id as u64, Ordering::Relaxed);
        STARTABLE[slot].store(enabled, Ordering::Relaxed);
        startable += usize::from(enabled);
    }
    DESCRIBED.store(cpus.len(), Ordering::Release);

    print!("  smp: {} core(s) in the madt", cpus.len());
    if startable != cpus.len() {
        print!(", {startable} startable");
    }
    if unseated > 0 {
        print!(
            " ({unseated} of them with local apic ids past cpu::MAX_CPUS = {MAX_CPUS}, which sizes \
             the per-CPU statics, so they stay parked)"
        );
    }
    println!();

    // As in read_cpu_list: the core we run on must be in the roster at its own index.
    let boot = arch::boot_cpu_id();
    if hwid(boot) != Some(boot as u64) {
        println!(
            "  smp: the boot core is logical {boot}, and the madt does not put a core with that \
             local apic id there"
        );
    }
}

/// The hardware id of logical core `id`, or `None` for a slot no described core's id names.
///
/// A filled slot holds its own index by construction, so a `Some` is always `Some(id)`; the `None`
/// is what distinguishes a seat the machine filled. Used outside tests only on `x86_64`, where
/// `arch::x86_64::irq::send_reschedule` looks up a target's local APIC id; aarch64 and RISC-V send
/// their reschedule IPIs by logical id.
#[cfg_attr(
    not(any(test, feature = "system_tests", target_arch = "x86_64")),
    allow(dead_code)
)]
pub(crate) fn hwid(id: usize) -> Option<u64> {
    match HWID.get(id)?.load(Ordering::Acquire) {
        u64::MAX => None,
        h => Some(h),
    }
}

/// How many cores the device tree described, which is not how many this kernel can use. Greater than
/// [`MAX_CPUS`] on a bigger machine than this build is sized for.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub fn described_count() -> usize {
    DESCRIBED.load(Ordering::Acquire)
}

unsafe extern "C" {
    /// The physical-mode entry every secondary starts at. Defined in boot.s.
    fn secondary_boot();
}

/// Start every secondary core the machine described, and wait until each has come online.
///
/// Called once, on the boot core, after the scheduler exists and [`read_cpu_list`] has recorded
/// the roster. Empty and unstartable seats are skipped. See notes/smp.md.
///
/// # BUGS
///
/// - A core whose hardware id is at or past `MAX_CPUS` stays parked rather than being remapped.
/// - The PSCI `smc` conduit is proved by parsing, never by a call. See `arch::cpu_start`.
#[cfg_attr(feature = "single_hart", allow(unreachable_code))]
pub(crate) fn bring_up_secondaries() {
    // Record the boot core as online before starting anyone, so a TLB shootdown from here on names
    // it. (Secondaries add their own bits as they come up.)
    ONLINE_MASK.fetch_or(1 << cpu::id(), Ordering::Release);

    // The single-hart measurement build (kernel/Cargo.toml's `single_hart`) starts nobody, but only
    // after the mask above: everything that broadcasts reads it. See notes/smp.md.
    #[cfg(feature = "single_hart")]
    {
        println!("  smp: 1 core(s) online (single_hart: secondaries left parked)");
        return;
    }

    // Paint every secondary's stack before any CPU_ON, while the slots are still untouched `.bss`,
    // so no live frame can be painted over (milestone 84). Whole slots: nothing has run on them.
    #[cfg(any(test, feature = "system_tests"))]
    for id in 0..MAX_CPUS {
        if id != cpu::id() {
            let (b, t) = secondary_stack_span(id);
            // SAFETY: a secondary's stack slot is `.bss`, mapped by the kernel's own image, and no
            // `CPU_ON` has been issued yet, so nothing has ever run on it. The `id != cpu::id()`
            // skips the boot core, which is on the linker-script stack and is live right now.
            unsafe { crate::stack::paint(b, t) };
        }
    }

    // A machine that states no start mechanism is told so once, here, rather than sent calls into
    // an exception level that may not exist.
    arch::print_bring_up_mechanism();
    if !arch::can_start_secondaries() {
        println!("  smp: 1 core(s) online");
        return;
    }

    // The entry is PHYSICAL: the core starts with its MMU off. Computed only once the machine can
    // be asked at all, and on x86_64 not converted, because `secondary_boot` is linked at its
    // physical trampoline address there and `virt_to_phys` would underflow. See notes/smp.md.
    #[cfg(target_arch = "x86_64")]
    let entry = arch::secondary_boot_entry(secondary_boot as *const () as u64);
    #[cfg(not(target_arch = "x86_64"))]
    let entry = arch::mmu::virt_to_phys(secondary_boot as *const () as u64);

    let mut started = 0;
    for id in 0..MAX_CPUS {
        // Start every seated core but the one we booted on. On aarch64 that is always core 0; on
        // RISC-V OpenSBI's boot hart is not guaranteed to be 0, so skip whichever this is.
        if id == cpu::id() {
            continue;
        }
        let hwid = HWID[id].load(Ordering::Relaxed);
        if hwid == u64::MAX {
            // No described core's hart id names this slot: an empty seat, not a refusal.
            // `read_cpu_list` already reported anything it could not seat.
            continue;
        }
        if !STARTABLE[id].load(Ordering::Relaxed) {
            println!("  smp: cpu {id} is not startable by this kernel (disabled, no supervisor");
            println!("       mode, or an enable-method we do not speak); leaving it alone.");
            continue;
        }
        // Seating placed this core at the slot its hardware id names (see read_cpu_list), so the
        // equation the rest of the kernel indexes by holds by construction; this cannot fire.
        debug_assert_eq!(hwid, id as u64, "a seated core is not at its own hart id");
        let ret = arch::cpu_start(hwid, entry, stack_top(id));
        if ret == 0 {
            started += 1;
        } else {
            // A core the firmware declines to start returns an error rather than hanging: note it
            // and carry on with the cores we have.
            println!("  smp: cpu {id} did not start (firmware returned {ret})");
        }
    }

    // Wait for every core we started to check in. Acquire pairs with the Release in
    // secondary_main: once the count is visible, so is everything that core did before it.
    while ONLINE.load(Ordering::Acquire) < started {
        core::hint::spin_loop();
    }

    println!("  smp: {} core(s) online", started + 1);
}

/// A secondary core's first Rust, called from `secondary_boot` once its MMU is on and it has a
/// stack. `cpu_id` came from `MPIDR_EL1`. It turns the core into a full scheduler participant and
/// then becomes that core's idle thread. See notes/smp.md.
#[unsafe(no_mangle)]
pub(crate) extern "C" fn secondary_main(cpu_id: usize) -> ! {
    // 1. Per-CPU pointer first, ahead of anything that could take a lock (the lock path reads this
    //    core's held-rank out of its PerCpu block). See cpu.rs.
    cpu::init_this_cpu(cpu_id);

    // 2. This core's exception vectors. VBAR_EL1 is per-core, so a secondary that never sets it
    //    jumps to a garbage vector on its first fault or timer tick and dies silently (and, if it
    //    died holding a lock, hangs the others). Set it before anything can trap. The vector table
    //    lives in kernel .text, mapped in both the coarse boot map and the fine map, so this is
    //    valid on either side of the switch below.
    arch::init();

    // 3. Adopt the kernel's fine map. The coarse boot map reaches only the first 2 GiB of the high
    //    half, not the thread-stack area 64 GiB up, so without this the core could not run a spawned
    //    thread. See arch::mmu::init_secondary.
    arch::mmu::init_secondary();

    // 4. Become a scheduler participant: adopt the context we are on as this core's idle thread,
    //    and reserve this core's run queue.
    crate::sched::adopt_secondary_idle();

    // 5. This core's interrupt hardware: its GIC CPU interface, then its timer (the PPI is banked,
    //    so this arms THIS core's tick, which is this core's source of preemption).
    crate::arch::irq::init_this_cpu();
    arch::timer::init();

    // 6. (tests only) Prove this core schedules from its OWN queue. `spawn_on`, not `spawn`:
    //    `spawn` places on the lighter of two sampled cores (DECISIONS §28), so the probes would
    //    not cover every core. See notes/smp.md.
    #[cfg(any(test, feature = "system_tests"))]
    crate::sched::spawn_on(cpu::id(), || {
        RAN_ON[cpu::id()].store(true, Ordering::Release);
    })
    .expect("a secondary could not spawn its probe");

    // 7. Online (Release, so a core seeing the count also sees everything set up above), and from
    //    the next line on, preemptible. Add this core to the online mask first, so a TLB shootdown
    //    that runs the instant we are counted also targets us.
    ONLINE_MASK.fetch_or(1 << cpu::id(), Ordering::Release);
    ONLINE.fetch_add(1, Ordering::Release);
    arch::interrupts::enable();

    // 8. This core's idle loop, which is now its idle thread's body: the shared idle body that also
    //    steals work from a loaded core (DECISIONS §28). Yield first so a thread already queued (the
    //    probe) runs at once, then hand off to `run_idle`, which steals, parks in wfi, and yields.
    crate::sched::yield_now();
    crate::sched::run_idle()
}

// The SMP tests need more than one core online (the runners default to `-smp 4`; NIFE_SMP moves
// it, up to MAX_CPUS). See notes/smp.md.
#[cfg(test)]
mod tests {
    use super::*;

    /// Wait for `cond`, yielding, bounded by the clock: up to 60 s, inside the suite's 90 s
    /// per-test watchdog (`DEFAULT_BUDGET_SECS`).
    ///
    /// Returns whether `cond` held by the deadline. The bound is wide for host contention starving
    /// the guest's vCPUs, and under the watchdog so that a failure names the subsystem. See
    /// notes/smp.md and notes/load-sensitive-assertions.md.
    fn wait_for(mut cond: impl FnMut() -> bool) -> bool {
        let deadline = crate::arch::timer::now() + 60 * crate::arch::timer::frequency();
        while crate::arch::timer::now() < deadline {
            if cond() {
                return true;
            }
            crate::sched::yield_now();
        }
        cond()
    }

    /// **Every secondary stack sits on an unmapped page**, in the live kernel page tables.
    ///
    /// `translate` reads the root out of the hardware register, so this is the map the CPU is using.
    /// The pages either side of each hole must translate, or the hole protects nothing. It says
    /// nothing about the coarse boot map, on which the guards are mapped. See notes/smp.md.
    #[test_case]
    fn every_secondary_stack_sits_on_a_guard_page() {
        for id in 0..MAX_CPUS {
            let guard = super::secondary_stack_guard(id);
            let (bottom, top) = secondary_stack_span(id);

            assert_eq!(
                guard % 4096,
                0,
                "core {id}'s guard page is not page-aligned"
            );
            assert_eq!(
                guard + 4096,
                bottom,
                "core {id}'s guard is not under its stack"
            );

            assert!(
                crate::arch::mmu::translate(guard).is_none(),
                "core {id}'s guard page at {guard:#x} IS mapped: a deep secondary would scribble \
                 instead of faulting",
            );
            assert!(
                crate::arch::mmu::translate(bottom).is_some(),
                "core {id}'s stack is not mapped at its bottom {bottom:#x}",
            );
            assert!(
                crate::arch::mmu::translate(top - 16).is_some(),
                "core {id}'s stack is not mapped at its top {top:#x}",
            );
        }
    }

    /// **The linker reserved exactly the region the Rust side emits.** If the static stopped
    /// landing in `.secondary_stacks` (a lost `link_section`, a section renamed in one script), the
    /// stacks would still work and the guard pages would silently be somewhere else.
    #[test_case]
    fn the_secondary_stack_region_is_the_size_the_linker_reserved() {
        unsafe extern "C" {
            static __secondary_stacks_start: core::ffi::c_void;
            static __secondary_stacks_end: core::ffi::c_void;
        }
        let lo = (&raw const __secondary_stacks_start) as u64;
        let hi = (&raw const __secondary_stacks_end) as u64;
        let (start, end) = super::secondary_stack_region();

        assert_eq!(lo, start, "the stacks are not at the start of their region");
        assert!(
            end <= hi,
            "the stacks ({start:#x}..{end:#x}) overflow the region the linker reserved \
             ({lo:#x}..{hi:#x})",
        );
        assert_eq!(lo % 4096, 0, "the region is not page-aligned");
    }

    /// **The cores we started are the cores the machine described**: the tree is re-read here,
    /// independently of the boot path, and each `cpu@` node must be seated at the slot its hardware
    /// id names, which is what the rest of the kernel indexes by.
    #[test_case]
    fn the_roster_is_the_machines_own_cpu_list() {
        // Zero means nobody has read a roster, not "this machine has no cores". x86_64 always
        // skips: `crate::DTB` holds PVH's `hvm_start_info` pointer there, and parsing it as a
        // device tree would panic. See notes/smp.md, BUGS.
        if super::described_count() == 0 || cfg!(target_arch = "x86_64") {
            crate::testing::skip!(
                "nothing has read this machine's core roster from a device tree (either \
                 described_count() is 0, or this is x86_64, whose roster is ACPI-based and has no \
                 independent device-tree re-read to check it against yet)"
            );
        }
        let dt = crate::device_tree().expect("device tree is unreadable");
        let list = CpuList::from_device_tree(&dt).expect("cannot read /cpus");

        assert_eq!(
            super::described_count(),
            list.described,
            "the roster and a fresh read of the tree disagree about how many cores exist",
        );
        // The tree's count is the runner's `-smp`. MAX_CPUS is a ceiling, not the machine; the
        // suite needs at least two cores and no more than the statics can seat.
        assert!(
            list.described >= 2,
            "the SMP suite needs -smp of at least 2; the tree describes {}",
            list.described,
        );
        assert!(
            list.described <= MAX_CPUS,
            "the runner passed -smp {} but this kernel seats only {MAX_CPUS}: cores would stay \
             parked, so run the suite at or below cpu::MAX_CPUS",
            list.described,
        );

        for cpu in list.cpus().iter() {
            let slot = usize::try_from(cpu.hwid).expect("a virt hart id fits in usize");
            assert!(
                slot < MAX_CPUS,
                "hart {slot} has no seat: the runner's -smp exceeds cpu::MAX_CPUS",
            );
            // Seating is by hart id (read_cpu_list), so the slot a core occupies IS its hardware id.
            assert_eq!(
                super::hwid(slot),
                Some(cpu.hwid),
                "hart {} is not seated at the slot its own id names",
                cpu.hwid,
            );
            // Every QEMU virt cpu is an S-mode hart, so the supervisor rule excludes none. A false
            // `supervisor` here means the parser read a modern ISA string's silence, or a
            // multi-letter extension, as a denial.
            assert!(
                cpu.is_startable(),
                "hart {} would be excluded from bring-up on the suite's own machine",
                cpu.hwid,
            );
        }
    }

    /// **Every core the machine described came online**: the pair of the test above, which says
    /// the roster is the machine's; this says we used all of it.
    #[test_case]
    fn every_cpu_the_tree_described_is_running() {
        // Zero means nobody has read a roster. Nothing below reads a device tree, so this runs on
        // x86_64 too.
        if super::described_count() == 0 {
            crate::testing::skip!(
                "nothing has read this machine's core roster yet (neither read_cpu_list nor \
                 seat_cpus_from_acpi has run)"
            );
        }
        assert_eq!(
            online_count(),
            super::described_count(),
            "the machine describes cores that never came online",
        );
    }

    /// **The online iterator is the live mask, exactly, and the sampler never leaves it.** Every
    /// broadcast-style loop iterates `online_cpus()` or masks with `online_harts_mask()`, so this
    /// proves those loops visit exactly the online set. The non-contiguous shape is host-proven in
    /// `crates/cpu_set`; this pins the delegation to the live mask.
    #[test_case]
    fn the_online_set_is_the_mask_and_the_sampler_stays_inside_it() {
        let mask = online_harts_mask();
        let mut seen = 0usize;
        let mut last = None;
        for c in online_cpus() {
            assert!(
                mask & (1 << c) != 0,
                "online_cpus yielded cpu {c}, which the mask {mask:#b} does not name",
            );
            assert!(last.is_none_or(|p| c > p), "online_cpus is not ascending");
            seen |= 1 << c;
            last = Some(c);
        }
        assert_eq!(seen, mask, "online_cpus missed a cpu the mask names");
        assert_eq!(
            online_cpus().count(),
            online_count(),
            "iterator and count disagree"
        );

        let n = online_count();
        for k in 0..2 * n {
            let c = nth_online(k);
            assert!(mask & (1 << c) != 0, "nth_online({k}) = {c} is not online");
            assert_eq!(
                c,
                nth_online(k + n),
                "nth_online does not wrap with period online_count",
            );
        }
    }

    /// Every secondary reached `secondary_main` and set up its per-CPU pointer.
    ///
    /// `bring_up_secondaries` already waited for the count before `test_main` ran, so the other
    /// cores are online by now. The expectation is the machine's own core count (whatever `-smp`
    /// the runner passed), not `MAX_CPUS`: the constant is capacity, and a leg run at `-smp 5`
    /// on an eight-slot kernel has four secondaries, honestly.
    #[test_case]
    fn all_secondaries_came_online() {
        // Zero means nobody has read a roster; no device tree is read, so this runs on x86_64 too.
        if super::described_count() == 0 {
            crate::testing::skip!(
                "nothing has read this machine's core roster yet (neither read_cpu_list nor \
                 seat_cpus_from_acpi has run)"
            );
        }
        assert_eq!(
            ONLINE.load(Ordering::Acquire),
            super::described_count() - 1,
            "not every core the tree describes came online (described {}, online {})",
            super::described_count(),
            online_count(),
        );
    }

    /// **Every secondary actually runs scheduled work on its own core.** Each spawned a probe onto
    /// its own run queue as it came online; the probe records the core it ran on. This is the proof
    /// that a secondary is a real scheduler participant, not just idling, and it also exercises the
    /// reaper on a secondary (the probe exits, and its successor reaps it).
    ///
    /// The probes run concurrently on their own cores; we wait for them, bounded, yielding so this
    /// core does other work meanwhile rather than pure-spinning.
    #[test_case]
    fn every_secondary_runs_scheduled_work() {
        // A secondary is any core that is not the boot core. On aarch64 the boot core is 0, so this
        // is cores 1..N; on RISC-V QEMU's boot hart is not fixed, so it is every core but that one.
        let boot = crate::arch::boot_cpu_id();
        let is_secondary = |c: usize| c != boot;
        // The online set, not `0..MAX_CPUS`: only a core that came online spawned a probe.
        let all_ran = || {
            online_cpus()
                .filter(|&c| is_secondary(c))
                .all(|c| RAN_ON[c].load(Ordering::Acquire))
        };

        assert!(
            wait_for(all_ran),
            "secondary cores did not run scheduled work in time"
        );

        for c in online_cpus().filter(|&c| is_secondary(c)) {
            assert!(
                RAN_ON[c].load(Ordering::Acquire),
                "secondary core {c} never ran scheduled work",
            );
        }
    }

    /// **Work can be placed on any core from another core**: `spawn_on(target, ..)` pushes the
    /// thread into `target`'s inbox and pokes it with a reschedule IPI, and `target` itself takes it
    /// out onto its own run queue.
    ///
    /// One placement is in flight at a time, each followed to a reap, so the adoption counted on the
    /// target (`PerCpu::adopted`) is this test's thread. It asserts arrival at the named core, not
    /// that the target then ran it: a steal can move a placed thread, and a wait for every core to
    /// run one can wedge forever. `every_secondary_runs_scheduled_work` and
    /// `a_batch_of_cpu_bound_work_reaches_every_cpu` cover execution. See notes/smp.md.
    #[test_case]
    fn work_can_be_placed_on_every_cpu() {
        for s in &SPREAD {
            s.store(0, Ordering::Relaxed);
        }

        let n = online_count();
        let here = cpu::id();
        // A machine with one core is not a machine that failed this test.
        if n < 2 {
            crate::testing::skip!("a cross-core placement test needs at least two online cores");
        }

        // Every ONLINE core but this one, placed from here: the remote path, inbox plus IPI.
        for target in online_cpus().filter(|&c| c != here) {
            place_one_probe_on(target);
        }

        // **And this core as a target**, which needs the placement to come from somewhere else, or
        // it is not the cross-core path at all (`place_on` puts a local target straight onto our own
        // queue, with no inbox and no SGI). The placer cannot itself end up here: a thread changes
        // core only by a steal, a steal is initiated only by an idle core, and this core is running
        // the test thread, which never goes idle.
        let other = online_cpus()
            .find(|&c| c != here)
            .expect("n >= 2 checked above");
        let placer = crate::sched::spawn_on(other, move || place_one_probe_on(here))
            .expect("spawn_on of the placer failed");
        assert!(
            wait_for(|| SPREAD[here].load(Ordering::Relaxed) != 0),
            "core {here} never received a placement made from core {other}",
        );
        assert!(
            wait_for(|| !crate::sched::is_thread_present(placer)),
            "the placer thread on core {other} was never reaped",
        );
    }

    /// Place one probe on `target` and follow it to a reap: the target must take it out of its own
    /// inbox (or, for a local target, off its own queue), the probe must run, and it must be reaped.
    ///
    /// Where it ran is recorded but not asserted, for the reason in the caller: a steal may move it
    /// first, and `spawn_on` is a placement hint rather than a pin (an explicit pin is a deferred
    /// §28 trigger).
    fn place_one_probe_on(target: usize) {
        let local = target == cpu::id();
        let adopted_before = cpu::of(target).adopted();

        let tid = crate::sched::spawn_on(target, move || {
            SPREAD[target].store(cpu::id() as u32 + 1, Ordering::Relaxed);
        })
        .expect("spawn_on failed");

        if !local {
            assert!(
                wait_for(|| cpu::of(target).adopted() > adopted_before),
                "core {target} never took the thread placed on it out of its inbox: the \
                 reschedule IPI is not reaching that core, or its drain is not running",
            );
        }

        assert!(
            wait_for(|| SPREAD[target].load(Ordering::Relaxed) != 0),
            "the probe placed on core {target} was delivered but never ran anywhere",
        );
        // Leave nothing behind for a later test's thread or frame accounting to find in flight.
        assert!(
            wait_for(|| !crate::sched::is_thread_present(tid)),
            "the probe placed on core {target} was never reaped",
        );
    }

    /// **A batch of cpu-bound work fills the whole machine** (SMP load balancing; DECISIONS §28).
    /// Offer the scheduler more runnable threads than cores and every core must end up running one.
    ///
    /// The measure is deliberately "which core is a thread running on *now*", marked every spin, not
    /// "which core did a thread first land on". Since §28 the two differ: `spawn` places work, but
    /// idle cores then *steal* it, so a thread migrates. A first-landing count would
    /// race (a core's placed thread can be stolen before that core runs it); the running-now mark
    /// captures placement and stealing together, which is the property that matters, the whole machine
    /// is busy. The threads spin until released so the work persists long enough for stealing to reach
    /// every idle core, then they exit and are reaped (the no-leak proxy would catch it otherwise).
    #[test_case]
    fn a_batch_of_cpu_bound_work_reaches_every_cpu() {
        static ON: [core::sync::atomic::AtomicU32; MAX_CPUS] =
            [const { core::sync::atomic::AtomicU32::new(0) }; MAX_CPUS];
        static RELEASE: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);
        for r in &ON {
            r.store(0, Ordering::Relaxed);
        }
        RELEASE.store(false, Ordering::Relaxed);

        let n = online_count();
        for _ in 0..(n * 4) {
            crate::sched::spawn(|| {
                // Mark whatever core we are running on right now, every pass, until released. A
                // thread stolen onto an idle core marks that core here, which is exactly what proves
                // the idle core got fed.
                while !RELEASE.load(Ordering::Relaxed) {
                    ON[cpu::id()].store(1, Ordering::Relaxed);
                    core::hint::spin_loop();
                }
            })
            .expect("spawn failed");
        }

        // Every ONLINE core must get fed, and a parked slot never can be.
        let done = || online_cpus().all(|c| ON[c].load(Ordering::Relaxed) != 0);
        assert!(
            wait_for(done),
            "cpu-bound work never reached every core: placement + stealing did not fill the machine",
        );
        // Let them all finish, so they are reaped rather than left spinning.
        RELEASE.store(true, Ordering::Relaxed);
    }

    /// **A kernel thread that migrates between harts keeps a per-CPU pointer that names the hart it
    /// is actually on** (DECISIONS §28).
    ///
    /// RISC-V keeps the per-CPU pointer in `tp`, which the trap frame saves and restores, so a
    /// stolen preempted thread must not resume with its origin hart's `tp`; on aarch64 it is
    /// `TPIDR_EL1`, which no frame carries, and this is a no-op. Waves of preempted cpu-bound
    /// workers, more than cores, migrate across harts, each re-checking `tp` on every spin.
    ///
    /// # BUGS
    /// Under host load this loses coverage quietly rather than going red. See notes/smp.md.
    #[test_case]
    fn a_migrated_kernel_thread_keeps_its_hart_pointer() {
        use core::sync::atomic::AtomicUsize;
        static BAD: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);
        static DONE: AtomicUsize = AtomicUsize::new(0);
        BAD.store(false, Ordering::Relaxed);
        DONE.store(0, Ordering::Relaxed);

        // ~40 ms per worker: at 100 Hz that is several preemption ticks, so a worker is certain to be
        // preempted (and thus stealable with a live frame) before it finishes.
        let life = crate::arch::timer::frequency() / 25;
        let n = online_count();
        let mut total = 0usize;

        for _wave in 0..6 {
            for _ in 0..(n * 3) {
                let spawned = crate::sched::spawn(move || {
                    let end = crate::arch::timer::now() + life;
                    while crate::arch::timer::now() < end {
                        if !crate::arch::percpu_matches_hart() {
                            BAD.store(true, Ordering::Relaxed);
                        }
                        core::hint::spin_loop();
                    }
                    DONE.fetch_add(1, Ordering::Relaxed);
                });
                if spawned.is_some() {
                    total += 1;
                } else {
                    // Thread table momentarily full (a prior wave still draining); stop adding and let
                    // it drain below. Not a failure: we still have plenty of migration in flight.
                    break;
                }
            }

            // Drain this wave, yielding so the workers sharing this core get their turns. The
            // budget is delivered ticks, not counter time or a spin count, so a descheduled
            // emulator stretches it; the workers' `life` stays counter time on purpose. See
            // `testing::TickBudget` and notes/smp.md.
            let mut budget = crate::testing::TickBudget::new(2 * crate::arch::timer::TICK_HZ);
            while DONE.load(Ordering::Relaxed) < total {
                assert!(
                    !BAD.load(Ordering::Relaxed),
                    "a migrated kernel thread read a per-CPU pointer for the WRONG hart: a stale tp \
                     rode a hart migration (DECISIONS §28; trap.s S-mode tp handling)",
                );
                assert!(
                    !budget.is_expired(),
                    "migration workers never drained ({}/{} done) within {} delivered ticks",
                    DONE.load(Ordering::Relaxed),
                    total,
                    2 * crate::arch::timer::TICK_HZ,
                );
                crate::sched::yield_now();
            }
        }

        assert!(
            !BAD.load(Ordering::Relaxed),
            "a migrated kernel thread read a per-CPU pointer for the WRONG hart (stale tp across a \
             hart migration)",
        );
    }

    /// **A preemptible kernel thread that asks for its own name gets its own name**, however it
    /// migrates (notes/mmu.md, BUGS: a user thread losing its text translation).
    ///
    /// The name lives in the running core's `PerCpu`, so a read split across a migration reads
    /// another core's `current`. Each worker takes its own name once with interrupts masked, then
    /// asks again with them open, in [`a_migrated_kernel_thread_keeps_its_hart_pointer`]'s waves;
    /// one answer naming another thread is the bug. The staggered lives, the clock read once per 256
    /// reads and the 24 waves are measured choices. See notes/smp.md.
    #[test_case]
    fn a_migrated_kernel_thread_reads_its_own_name() {
        use core::sync::atomic::{AtomicU64, AtomicUsize};
        static WRONG: AtomicU64 = AtomicU64::new(0);
        static DONE: AtomicUsize = AtomicUsize::new(0);
        WRONG.store(0, Ordering::Relaxed);
        DONE.store(0, Ordering::Relaxed);

        let life = crate::arch::timer::frequency() / 50;
        let n = online_count();
        let mut total = 0usize;

        for _wave in 0..24 {
            for k in 0..(n * 3) {
                let life = life * (1 + k as u64 % 4);
                let spawned = crate::sched::spawn(move || {
                    let was_enabled = crate::arch::interrupts::disable();
                    let me = crate::sched::current();
                    crate::arch::interrupts::restore(was_enabled);
                    let end = crate::arch::timer::now() + life;
                    while crate::arch::timer::now() < end {
                        for _ in 0..256 {
                            let seen = crate::sched::current();
                            if seen != me {
                                WRONG.store(seen, Ordering::Relaxed);
                            }
                        }
                    }
                    DONE.fetch_add(1, Ordering::Relaxed);
                });
                if spawned.is_some() {
                    total += 1;
                } else {
                    break; // the table is full of the last wave; let it drain
                }
            }
            let mut budget = crate::testing::TickBudget::new(2 * crate::arch::timer::TICK_HZ);
            while DONE.load(Ordering::Relaxed) < total {
                assert!(
                    !budget.is_expired(),
                    "name-reading workers never drained ({}/{} done) within {} delivered ticks",
                    DONE.load(Ordering::Relaxed),
                    total,
                    2 * crate::arch::timer::TICK_HZ,
                );
                crate::sched::yield_now();
            }
        }

        let wrong = WRONG.load(Ordering::Relaxed);
        assert!(
            wrong == 0,
            "a preemptible kernel thread asked for its own name and got thread {wrong:#x}'s: the \
             read of PerCpu::current was split across a migration",
        );
    }
}
