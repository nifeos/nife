//! **The `x86_64` architecture layer.** The third implementation of the `arch/` contract (milestone
//! 161, notes/x86-port.md), and the one milestone 20 said would be the real test of it: the first
//! two architectures are both RISC machines with a device tree, weak memory and a similar MMU, and
//! this one is none of those things.
//!
//! **This is a partial port, and it says so in every remaining stub.** What is real: the boot path
//! (a 32-bit multiboot-style trampoline into long mode and the high half), the GDT and TSS, the IDT
//! and the trap frame, the console UART over port I/O, the page-table format, the fine-grained W^X
//! kernel map, the local APIC and a calibrated timer, the IO APIC and a routed device line, user
//! address spaces, the `syscall` pair and ring 3, the address arithmetic, the interrupt-masking
//! primitives, the context switch, and the test exit. What is not: VT-d and SMP bring-up, each an
//! `unimplemented!()` that names itself and the reason, so that nobody mistakes a stub for a working
//! port.
//!
//! **And one thing that is real only at this layer**, which is worth saying here because this module
//! is where a reader looks first: a program runs at ring 3, but there is no *process* behind it. The
//! scheduler, the kernel heap and the untyped budget have never been brought up on this
//! architecture, so `user::run` and `KernelStack::new` have nothing to stand on. See
//! design/roadmap/0161-x86-64-kernel-port.md, item 4.
//!
//! # What this port has already shown about the seam
//!
//! Two things, both worth more than the code. The `paging` crate's split into a generic level walk
//! plus a per-architecture entry codec **held**: `paging::x86_64::Ia32e` is 60 lines and nothing
//! above it changed. And the 16550 driver spans a *different address space* (x86 port I/O rather
//! than MMIO) as a type parameter rather than a second driver, which is the strongest evidence so
//! far that "a new ISA is a new directory" is true rather than aspirational.
//!
//! # And two places it did not hold, until calef ratified the rename
//!
//! **`arch::psci_cpu_on` was an aarch64 name that leaked**, and RISC-V already had to implement it
//! as an SBI call under an ARM firmware interface's name. x86 has no third mechanism to hide behind
//! it: SMP bring-up here is INIT-SIPI-SIPI through the local APIC, sent by the interrupt controller,
//! at a physical page below 1 MiB, in 16-bit real mode. Ratified as `arch::cpu_start`.
//!
//! **The single pointer `kernel_main` took was called `dtb`.** x86 has no device tree; what arrives
//! there is PVH's `hvm_start_info`, which carries the memory map and the ACPI RSDP address, so the
//! *shape* was right and only the name was wrong. Ratified as `boot_info_pointer`.

use core::arch::{asm, global_asm};

// The AP real-mode trampoline's copy-and-prepare step (milestone 161's SMP item). See its own
// header, and `boot.s`'s `secondary_boot` for what it prepares.
// AMD's IOMMU (lane `amd-vi`), beside VT-d's `iommu`, which forwards to it on an AMD machine.
pub mod amd_vi;
pub mod ap_boot;
pub mod context;
pub mod exceptions;
pub mod fp;
mod instructions;
pub mod interrupts;
pub mod iommu;
pub mod irq;
pub mod isa;
// What the loader said (milestone 161): the kernel side of `machine_discovery::x86_64`.
pub mod machine;
pub mod mmu;
// Unhalted core cycles (milestone 309), the x86_64 half of milestone 74's measurement side. It is
// here rather than folded into `timer` because it is a different counter answering a different
// question: `timer::now()` is `rdtsc`, a constant-rate clock, and this is what the core actually
// ran. Its own header has the table of all three architectures' two counters.
pub mod pmu;
pub mod port;
pub mod reset;
pub mod rtc;
// The Intel TCO watchdog (milestone 593 (a wedged kernel resets itself), provisional number). Only
// the watchdog soak drives it, so only that build compiles it; see its own header.
pub mod segments;
pub mod semihosting;
#[cfg(feature = "watchdog_soak_test")]
pub mod tco;
pub mod thread_pointer;
pub mod timer;
/// The TSC calibration's estimator and its two boot assertions. Test-only; see the file header for
/// why they run under QEMU rather than on the host.
#[cfg(test)]
mod timer_calibration_tests;
/// The TSC-vs-RTC measurement instrument. Off by default; see notes/tsc-under-tcg.md.
#[cfg(feature = "tsc_probe")]
pub mod tsc_probe;

// The saved thread context and how a new one is faked (the Rust half of context.s). Re-exported
// flat so `crate::arch::{Context, switch_to}` names them regardless of architecture.
pub use context::{Context, switch_to};
// How the console reaches its UART's registers on this architecture. Named flat through `arch`
// because `console.rs` picks it by `target_arch` and must not reach into `arch::x86_64::` directly.
pub use port::PortIo;
/// The arch contract for a kernel-initiated cold reboot (milestone 249 (the boot lottery is sampled by a person walking to the board)). See [`reset::reboot`].
pub use reset::reboot;

// The 32-bit entry (_start), the long-mode transition, the .bss zeroing, and the stack handoff to
// `kernel_main`.
global_asm!(include_str!("boot.s"));

// The context switch and the two first-run trampolines (the asm half of context.rs).
global_asm!(include_str!("context.s"));

// Saving and restoring the `FXSAVE` area (milestone 447 (a thread's vector registers are its own)).
// Separate from context.s because it moves
// a register file rather than a calling convention's callee-saved set; see fp.rs.
global_asm!(include_str!("fp.s"));

// The 256 trap stubs, the shared restore path, the `syscall` entry, and the door into ring 3
// (the asm half of exceptions.rs). The constants are substituted rather than duplicated: a 64-bit
// assembler cannot read a Rust `const`, and the alternative is two files that can drift. The three
// `*_OFF` ones are `gs`-relative byte offsets into `cpu::PerCpu::x86_trap` (milestone 161's SMP
// item), computed by `core::mem::offset_of!` rather than hand-counted, so a field added or reordered
// in `cpu.rs` cannot silently desynchronize the assembly that reaches through them.
global_asm!(
    include_str!("trap.s"),
    USER_CODE = const segments::USER_CODE as u64,
    USER_DATA = const segments::USER_DATA as u64,
    SYSCALL_VECTOR = const exceptions::SYSCALL_VECTOR,
    TSS_RSP0_PTR_OFF = const core::mem::offset_of!(crate::cpu::PerCpu, x86_trap.tss_rsp0_ptr),
    SYSCALL_KERNEL_RSP_OFF =
        const core::mem::offset_of!(crate::cpu::PerCpu, x86_trap.syscall_kernel_rsp),
    SYSCALL_USER_RSP_OFF =
        const core::mem::offset_of!(crate::cpu::PerCpu, x86_trap.syscall_user_rsp),
);

/// `IA32_GS_BASE`: the MSR holding the base of the `gs` segment, and this architecture's answer to
/// aarch64's `TPIDR_EL1` and RISC-V's `tp`. A per-CPU register the kernel owns.
///
/// **There is a second one, and it is now load-bearing.** `IA32_KERNEL_GS_BASE` (0xC0000102) holds
/// the *other* value, and `swapgs` exchanges the two. That pair is how a trap from ring 3 recovers
/// the kernel's per-CPU pointer without trusting anything the user program could have set, which is
/// exactly the problem RISC-V solves with `sscratch`. The convention this kernel keeps, stated once
/// here because it is the sort of thing that is otherwise only true by accident: **while executing
/// in ring 0, `IA32_GS_BASE` names the per-CPU block and `IA32_KERNEL_GS_BASE` holds the user's
/// value; in ring 3 they are the other way round.** trap.s does the swapping, guarded on the
/// interrupted CPL, and its header says why the guard rather than a bare instruction.
const IA32_GS_BASE: u32 = 0xC000_0101;

/// Read a model-specific register. `rdmsr` returns the value split across `edx:eax`, with the
/// register number in `ecx`.
///
/// Name: provisional (milestone 161 (the `x86_64` kernel port)): calef names public functions
/// (AGENTS.md, milestone 160 (review the public function names across the kernel's dependency
/// crates)), and this one was minted by a lane.
///
/// # Safety
/// `msr` must be a register this CPU implements. Reading one it does not is a general protection
/// fault, and `CPUID` is the only way to know for the optional ones.
pub unsafe fn read_msr(msr: u32) -> u64 {
    let low: u32;
    let high: u32;
    // SAFETY: the caller's contract. A read has no architectural side effects.
    unsafe {
        asm!(
            "rdmsr",
            in("ecx") msr,
            out("eax") low,
            out("edx") high,
            options(nomem, nostack, preserves_flags),
        );
    }
    ((high as u64) << 32) | low as u64
}

/// Write a model-specific register. The counterpart of [`read_msr`], with the same split.
///
/// Name: provisional (milestone 161).
///
/// # Safety
/// `msr` must be one this CPU implements, and `value` must be legal for it. An MSR write is one of
/// the few instructions that can change what mode the machine is in (`IA32_EFER` alone controls
/// long mode, `NXE` and `syscall`), so this is exactly as dangerous as what the caller names.
pub unsafe fn write_msr(msr: u32, value: u64) {
    // SAFETY: the caller's contract.
    unsafe {
        asm!(
            "wrmsr",
            in("ecx") msr,
            in("eax") value as u32,
            in("edx") (value >> 32) as u32,
            options(nomem, nostack, preserves_flags),
        );
    }
}

/// **The local APIC id of the CPU the kernel booted on**, recorded once by `boot.s`'s `_start_high`
/// (just after it zeroes `.bss`, and before it calls `kernel_main`) from `CPUID` leaf 1's
/// `EBX[31:24]`, "Initial APIC ID".
///
/// **Recorded rather than recomputed, and that distinction is the whole point** (milestone 316,
/// `ap_boot.rs`'s BUGS #3). `CPUID` answers *"which core am I"*. Every caller of [`boot_cpu_id`]
/// wants *"which core booted"*, and the two are the same number only while there is one core. Once
/// a secondary is online, a caller that §28's placement has migrated onto it read its own id and
/// called it the boot core's, which made `smp::tests::every_secondary_runs_scheduled_work` wait
/// forever for a mark the real boot core never sets, and made `stack::report_high_water` scan a
/// never-painted slot and report it at 65536/65536. Reading a value stamped by code that runs
/// exactly once, on exactly the boot processor, cannot say that whoever asks later.
///
/// This is riscv64's `BOOT_HARTID` in x86 clothing. The difference is only in where the number
/// comes from: OpenSBI hands RISC-V the hart id in `a0` and it would be lost if boot.s did not
/// catch it, whereas `CPUID` remains readable forever, which is exactly what made the recomputing
/// version look correct. aarch64 needs no static at all because its boot core is architecturally 0.
///
/// The name is **provisional** (an architect names things): it mirrors `BOOT_HARTID` on the
/// architecture that already had this shape.
#[unsafe(no_mangle)]
static BOOT_CPU_ID: core::sync::atomic::AtomicUsize = core::sync::atomic::AtomicUsize::new(0);

/// The logical id of the CPU the kernel boots on.
///
/// **Read from a boot-time record, not recomputed** (milestone 316; it was a live `CPUID` read from
/// milestone 161 until then, and `BOOT_CPU_ID`'s own doc has why that was wrong and what it broke).
/// Before 161 it was the constant 0, on the reasoning that the boot processor is selected by
/// hardware rather than by firmware choice the way RISC-V's boot hart is. That is true and beside
/// the point: the number this returns has to agree with the *roster's* seating
/// (`smp::seat_cpus_from_acpi`, which seats every core, boot core included, at the slot its own
/// local APIC id names, the same logical-id-equals-hardware-id invariant `read_cpu_list` gives the
/// other two architectures), and nothing guarantees the boot CPU's local APIC id is 0 in general,
/// only that it usually is on QEMU.
///
/// Still callable from the kernel's very first Rust statement, which is what
/// `cpu::init_this_cpu(arch::boot_cpu_id())` needs: `boot.s` stamps the record before it calls
/// `kernel_main`, so there is no window in which this reads the `AtomicUsize`'s zero default.
pub fn boot_cpu_id() -> usize {
    BOOT_CPU_ID.load(core::sync::atomic::Ordering::Relaxed)
}

/// The `gs`-relative offset of `PerCpu::x86_trap.self_ptr`, which [`percpu`] loads.
const PERCPU_SELF_OFF: usize = core::mem::offset_of!(crate::cpu::PerCpu, x86_trap.self_ptr);

/// Set this CPU's per-CPU pointer, by writing the `gs` segment base, and then the block's own
/// address into the block, through that base, for [`percpu`] to read back.
pub fn set_percpu(ptr: usize) {
    // SAFETY: `IA32_GS_BASE` exists on every long-mode CPU, and this value is the per-CPU block
    // this kernel reserves the register for.
    unsafe { write_msr(IA32_GS_BASE, ptr as u64) };
    // SAFETY: `gs` now bases at `ptr`, a `PerCpu` this core owns, and the offset is that struct's
    // own `self_ptr` field, computed by `offset_of!`. Only this core reaches its block through `gs`.
    unsafe {
        asm!(
            "mov gs:[{off}], {p}",
            off = const PERCPU_SELF_OFF,
            p = in(reg) ptr,
            options(nostack, preserves_flags),
        );
    }
}

/// Read this CPU's per-CPU pointer (the value last handed to [`set_percpu`]).
///
/// **One `gs`-relative load, not `rdmsr IA32_GS_BASE`** (milestone 758 (the IPC fast paths shrink
/// back inside their band), provisional). The two answer the same question, since `gs`'s base IS
/// that MSR, but `rdmsr` is a microcoded, serialising instruction that clobbers `rax`, `rdx` and
/// `rcx`, and this runs several times per syscall: every lock, unlock and current-thread read.
/// Inlined, it was the largest single line in x86_64's IPC fast-path closures on 2026-10-04 UTC.
///
/// It reads through `gs` where the old one read the MSR, so the window where `IA32_GS_BASE` holds
/// the user's value (between the exit `swapgs` and `iretq`, `trap.s`) is now a load from a user
/// address rather than a wrong number. Nothing may call this in that window either way, and the
/// two code paths that can land there (the shootdown NMI and `segments::set_port_range_grant_on`)
/// already name their core from the local APIC instead; `mmu::serve_shootdown_nmi` says why.
pub fn percpu() -> usize {
    let p: usize;
    // SAFETY: after `set_percpu` on this core, `gs:[PERCPU_SELF_OFF]` is this core's own
    // `PerCpu::x86_trap.self_ptr`, which `set_percpu` wrote. `readonly`: it reads memory and writes
    // none. Not `pure`, so LLVM never merges two reads across a migration point.
    unsafe {
        asm!(
            "mov {p}, gs:[{off}]",
            p = out(reg) p,
            off = const PERCPU_SELF_OFF,
            options(nostack, readonly, preserves_flags),
        );
    }
    p
}

/// **Test-only: does the per-CPU pointer name the CPU we are physically running on?**
///
/// A constant `true` here, and for aarch64's reason rather than RISC-V's. RISC-V keeps the pointer
/// in `tp`, an ordinary register a trap frame carries, so it can go stale when a preempted thread
/// resumes on a different hart. `IA32_GS_BASE` is an MSR: it is not saved or restored by any context
/// switch and does not travel with a thread, so there is nothing that could make it stale. The
/// independent ground truth (the local APIC id) is not readable yet either way.
#[cfg(any(test, feature = "system_tests"))]
pub fn percpu_matches_hart() -> bool {
    true
}

/// Start a secondary CPU via INIT-SIPI-SIPI (milestone 161's SMP item). `target_cpu` is a local
/// APIC id (the arch contract's "hardware id", `smp::bring_up_secondaries` reads it out of the
/// roster `smp::seat_cpus_from_acpi` built), `entry` is the trampoline's own physical page
/// (`ap_boot::trampoline_phys()`, handed back through `secondary_boot_entry`, see below), and
/// `context` is the stack top the trampoline hands to `secondary_main`.
///
/// **Blocks until the core it started is fully online, or has been given up on**, which is not
/// `smp::bring_up_secondaries`'s usual contract (aarch64 and RISC-V return as soon as the firmware
/// call is accepted) but is required here: there is exactly one trampoline scratch page, shared by
/// every `STARTUP` IPI this kernel ever sends, and `bring_up_secondaries` starts one core per call
/// with nothing else serializing them. Waiting for `online_count()` to move is what makes the next
/// call's `ap_boot::prepare` safe to overwrite that page.
///
/// # BUGS
/// **`entry` must equal `ap_boot::trampoline_phys()`.** The arch contract passes whatever
/// `smp::bring_up_secondaries` computed for `secondary_boot`'s address, which on this architecture
/// *is* that trampoline page (see `secondary_boot_entry`'s own doc), so this holds by construction
/// today; it is not re-derived here because there is nowhere else it could sensibly come from.
pub fn cpu_start(target_cpu: u64, entry: u64, context: u64) -> i64 {
    if !irq::is_local_apic_ready() {
        return -1;
    }
    debug_assert_eq!(
        entry,
        ap_boot::trampoline_phys(),
        "cpu_start's entry is not the AP trampoline's own page"
    );
    // SAFETY: this function does not return until the core it is about to start has either come
    // fully online or been given up on (see the wait loop below), which is what keeps two calls
    // from ever overwriting the shared trampoline page while an earlier one is still in use.
    unsafe { ap_boot::prepare(context) };

    let dest = target_cpu as u8;
    // **Read once, here, before the INIT, and compared against by every wait below.** This is the
    // whole fix for `ap_boot`'s BUGS #1. The wait loop used to re-read the count after the
    // STARTUP IPIs and wait for it to move from *that* value, so a core fast enough to check in
    // during the 200 us settle delays had already moved it: the loop then waited ten seconds for a
    // second increment nobody would make, returned -1, and the core was online but uncounted.
    let before = crate::smp::online_count();

    // The universal INIT-SIPI-SIPI startup algorithm (Intel MP spec, appendix B.4): INIT, a settle
    // delay, then a STARTUP IPI.
    irq::send_init(dest);
    busy_wait_us(10_000);
    let vector = (entry >> 12) as u8;
    irq::send_startup(dest, vector);
    busy_wait_us(200);

    // **The second STARTUP IPI is conditional, not unconditional.** The MP spec calls for two, "the
    // second is a no-op on a core that already started", meaning a core that has already left the
    // wait-for-SIPI state is defined to ignore a further one (QEMU's `apic_sipi` does exactly
    // that). Sending the second only when the first evidently has not worked yet costs nothing when
    // the first succeeds. It was added as a hypothesis for `ap_boot`'s BUGS #1 and was not the fix;
    // it is kept because it is no less correct than the unconditional form.
    if crate::smp::online_count() == before {
        irq::send_startup(dest, vector);
        busy_wait_us(200);
    }

    // Bounded: a core the firmware silently declines to start must not hang bring-up.
    //
    // **Ten seconds, not one.** The delays above are the real thing's timing, sub-millisecond on
    // real hardware; the budget below is not that, it is how long CPU 0 waits for the SIPI'd core to
    // run its own trampoline, adopt the fine map, and reach `secondary_main`'s online mark, all of
    // it QEMU TCG instructions on whatever host thread the emulator's own scheduler gets around to
    // next. Measured too short once already, at one second: the target core's own vCPU thread had
    // simply not been scheduled by the host yet, and `cpu_start` gave up and reported "did not
    // start" for a core that came up perfectly well a moment later, arriving too late to be counted
    // and permanently invisible to the roster. (That diagnosis predates `ap_boot`'s BUGS #1 being
    // root-caused, and "a core that came up perfectly well" but was not counted is exactly #1's
    // signature, so it may have been #1 rather than the budget; the ten seconds are kept because a
    // loaded host really can starve a vCPU thread that long.) This is exactly the host-contention shape
    // `smp::tests::wait_for`'s own sixty-second budget exists for, one level earlier: bring-up
    // itself can be starved the same way a test's own wait can.
    //
    // **`hlt` between checks, not a tight spin.** TCG runs each vCPU as one host thread, and
    // `spin_loop`'s `pause` hint does nothing for *host* scheduling; a CPU 0 that never stops
    // consuming its host thread's time slice can only make it harder for the vCPU thread whose
    // progress this loop is waiting on to get scheduled, on a busy host (this project's own
    // recorded condition; AGENTS.md, "What bounds lane count, and the three ceilings").
    // `wait_for_interrupt` parks CPU 0 on `hlt` until its own local APIC timer (armed at `TICK_HZ`,
    // already ticking: interrupts are enabled before `bring_up_secondaries` runs) wakes it, which
    // costs at most one tick of latency per check and, unlike the spin, actually yields host CPU
    // time. Measured to turn an occasional full hang (waiting past even a sixty-second budget) into
    // a reliable, clean give-up within the stated budget when a core does not come up. The "deeper
    // reason a core sometimes does not come up" that this paragraph used to defer to was the
    // re-read `before` fixes above: the cores were coming up, and this loop was not counting them
    // (`ap_boot`'s BUGS #1).
    let budget = 10 * crate::arch::timer::frequency();
    let start = crate::arch::timer::now();
    while crate::smp::online_count() == before {
        if crate::arch::timer::now().wrapping_sub(start) >= budget {
            return -1;
        }
        wait_for_interrupt();
    }
    0
}

/// Busy-wait roughly `us` microseconds, against the calibrated TSC. No `wfi`/`hlt` equivalent
/// here: INIT-SIPI-SIPI's delays are a handful of instructions on real hardware, and parking this
/// core for them would need an interrupt to wake it that nothing is going to send.
fn busy_wait_us(us: u64) {
    let ticks = crate::arch::timer::frequency() / 1_000_000 * us;
    let start = crate::arch::timer::now();
    while crate::arch::timer::now().wrapping_sub(start) < ticks {
        core::hint::spin_loop();
    }
}

/// Can this machine start a secondary CPU at all? Yes, once the local APIC is up: INIT-SIPI-SIPI is
/// sent *through* it, unlike PSCI or SBI, which need no device at all before the first call.
pub fn can_start_secondaries() -> bool {
    irq::is_local_apic_ready()
}

/// Print how this machine starts a CPU. One line, on every boot, beside the SMP count.
pub fn print_bring_up_mechanism() {
    if irq::is_local_apic_ready() {
        crate::println!(
            "  smp: init-sipi-sipi via the local apic, trampoline at {:#x}",
            ap_boot::trampoline_phys()
        );
    } else {
        crate::println!(
            "  smp: the local apic is not up, so no core can be started here (init-sipi-sipi needs it)"
        );
    }
}

/// **The physical address `smp::bring_up_secondaries` hands `cpu_start` as `entry`.**
///
/// On aarch64 and RISC-V this is `virt_to_phys(secondary_boot)`: their `secondary_boot` is an
/// ordinary high-linked label, and firmware wants its physical address. Here `secondary_boot` is
/// **already** physical: link-x86_64.ld gives it the fixed low virtual address
/// (`AP_TRAMPOLINE_PHYS`) it has to execute from, because a `STARTUP` IPI can only name a page
/// below 1 MiB, so its own address *is* that page and converting it again would be wrong (this was
/// milestone 161's first named bug: `smp::bring_up_secondaries` used to call `virt_to_phys` on it
/// unconditionally, computing `secondary_boot's address - DIRECT_MAP_BASE`, an underflow, since
/// `secondary_boot`'s address is nowhere near the direct map).
pub fn secondary_boot_entry(secondary_boot_addr: u64) -> u64 {
    secondary_boot_addr
}

/// Bring this CPU's architecture state up: the GDT and TSS, then the IDT. The order is forced, and
/// not obviously: an IDT entry names a code **selector**, so the GDT that selector indexes has to be
/// the one installed before any trap can be delivered through it.
pub fn init() {
    // First, before this core prints anything or adopts the fine map: the fine map's framebuffer
    // pages select PAT entry 1, and on a core that has not run this, entry 1 is write-through.
    give_write_combining_a_page_attribute_entry();
    // SAFETY: called once per CPU during boot, with a valid stack, before interrupts are unmasked.
    unsafe { segments::init() };
    exceptions::init();
    // And the second door into the kernel, which shares none of the first one's machinery: four
    // MSRs, no gate and no descriptor. It goes here rather than with ring 3 because it is per-CPU
    // state like the GDT and the IDT, and because a `syscall` before it is programmed is a jump to
    // whatever `IA32_LSTAR` holds, which on a cold machine is zero.
    //
    // SAFETY: `segments::init` above installed the GDT these selectors index, and nothing has
    // entered ring 3 (nothing can: this is the boot CPU's own bring-up).
    unsafe { exceptions::init_syscall() };

    close_performance_counters_to_ring3();
    close_ring3_pages_to_ring0_execution();
}

/// **Program `IA32_PAT` so that `PWT` alone means write-combining** (the console-scroll lane,
/// 2026-10-04), which is what `paging::Flags::write_combining()` encodes and what the kernel maps
/// the framebuffer with (`mmu::direct_map_claims`).
///
/// # Why
///
/// Until this, the framebuffer was mapped like a register: `PCD | PWT`, strong uncacheable, one
/// bus transaction per four-byte store and nothing the MTRRs could relax. xenon's 1920x1080
/// aperture is 8 MB, so every full redraw was two million of them, and the boot console's
/// scrolling swept visibly down the screen. Write-combining lets the core gather a run of stores
/// into one burst. A PAT type of WC also wins over whatever the firmware's MTRRs say about the
/// range, so this does not depend on the firmware having done anything.
///
/// # Why entry 1 can simply be rewritten
///
/// The Intel SDM asks for a cache flush around a PAT change because a mapping that already uses
/// the changed entry would otherwise hold lines of the old type. **No mapping uses entry 1 when
/// this runs**: `boot.s`'s coarse map sets neither `PWT` nor `PCD`, every fine-map device page is
/// `PCD | PWT` (entry 3), and on the boot core this runs before `mmu::init` builds the fine map, on
/// a secondary before `mmu::init_secondary` adopts it. What is left is consistency between cores,
/// which is why it runs on every one: `smp::secondary_main` calls [`init`] too.
///
/// # Gated on CPUID
///
/// `CPUID.1:EDX[16]` advertises the PAT. Every `x86_64` processor this tree knows of has one
/// (from memory, not a specification citation). Without it `PWT` alone is write-through, which
/// for a screen nothing reads back is merely slow, and the boot line says so.
///
/// Name: provisional (the console-scroll lane).
fn give_write_combining_a_page_attribute_entry() {
    /// `CPUID.1:EDX` bit 16: "PAT".
    const CPUID_1_EDX_PAT: u32 = 1 << 16;
    if isa::cpuid(1).edx & CPUID_1_EDX_PAT == 0 {
        crate::println!(
            "  pat         : not offered by cpuid on core {}; the screen is write-through here",
            crate::cpu::id()
        );
        return;
    }
    // SAFETY: CPUID advertised the MSR, so neither access can `#GP`, and every entry in the value
    // is a defined memory type. The one entry it changes is selected by no mapping on this core
    // (see above), so no cached line or TLB entry has the old type to disagree with.
    let found = unsafe { read_msr(paging::x86_64::pat::MSR) };
    if found != paging::x86_64::pat::VALUE {
        // SAFETY: as the read's.
        unsafe { write_msr(paging::x86_64::pat::MSR, paging::x86_64::pat::VALUE) };
    }
    crate::println!(
        "  pat         : {:#018x} on core {} (was {found:#018x}); entry 1 is write-combining",
        paging::x86_64::pat::VALUE,
        crate::cpu::id()
    );
}

/// Establish `CR4.SMEP` set, so ring 0 cannot execute an instruction fetched from a page whose
/// `U/S` bit says it belongs to ring 3.
///
/// # Why this is a confinement matter and not a hardening nicety
///
/// `crates/paging`'s x86 decoder reports a user page as *not* kernel-executable, and every
/// confinement test that reads a mapping through `Flags::is_kernel_executable` believes it. The
/// hardware disagrees unless this bit is set: x86 has one execute permission, `XD`, and it applies
/// at every ring, so a user code page with `XD` clear is executable at ring 0 too. RISC-V refuses a
/// supervisor fetch from a `U` page unconditionally and aarch64 has `PXN`; x86 gates the same
/// refusal behind a control-register bit that nothing here set. Milestone 313's audit found the
/// decoder's claim, and `notes/confinement-claims.md`'s sentence that "the hardware really does make
/// a user page non-executable in supervisor mode", both true only with this bit on.
///
/// What it costs: nothing on any path. SMEP is checked by the fetch unit against the leaf's `U/S`
/// bit; there is no per-access software toggle, which is what separates it from `SMAP` (`stac`/`clac`
/// around every kernel access to user memory, and the reason SMAP stays off, per
/// `mmu::permit_kernel_access_to_user_pages`'s `BUGS`). What it forbids: nothing this kernel does.
/// Every kernel page is mapped `U/S` clear, and a user program's code is only ever *entered* through
/// `iretq`/`sysret` at ring 3, never called from ring 0.
///
/// # Gated on CPUID, and honest on the console when absent
///
/// `CPUID.(EAX=7,ECX=0):EBX[7]` is the feature bit (Ivy Bridge and every Intel since; AMD from
/// Excavator; QEMU's `-cpu max`, HVF and the `q35` defaults all offer it). Setting `CR4.SMEP` on a
/// CPU that does not advertise it is `#GP`, so this reads first. A machine that does not offer it
/// gets a boot line saying that ring 0 can execute ring-3 pages there, because the decoder's answer
/// is then wrong on that machine and a reader of the transcript should know.
///
/// Per core, like `close_performance_counters_to_ring3` above it: `CR4` is per-CPU state.
fn close_ring3_pages_to_ring0_execution() {
    /// `CR4.SMEP`: "Supervisor Mode Execution Prevention".
    const SMEP: u64 = 1 << 20;
    /// `CPUID.(7,0):EBX` bit 7 advertises SMEP.
    const CPUID_7_EBX_SMEP: u32 = 1 << 7;

    // Leaf 7 exists only when leaf 0 says the maximum basic leaf reaches it; `isa::init` makes
    // the same check before reading RDSEED out of the same word.
    let offered = isa::cpuid(0).eax >= 7 && isa::cpuid_count(7, 0).ebx & CPUID_7_EBX_SMEP != 0;
    if !offered {
        crate::println!(
            "  cr4.smep    : not offered by cpuid on core {}; ring 0 can execute ring-3 pages here",
            crate::cpu::id()
        );
        return;
    }

    let cr4 = instructions::read_cr4();
    if cr4 & SMEP != 0 {
        return;
    }
    // SAFETY: setting `CR4.SMEP` only *removes* a ring-0 permission the kernel never uses (no
    // kernel code lives in a `U/S` page). CPUID advertised the bit, so the write cannot `#GP`. Paging
    // bits are preserved; no TLB entry is invalidated, and the bit takes effect on the next fetch
    // without one, because SMEP is evaluated against the leaf at fetch time.
    unsafe { instructions::write_cr4(cr4 | SMEP) };
    crate::println!(
        "  cr4.smep    : set on core {}; ring 0 faults on a fetch from a ring-3 page",
        crate::cpu::id()
    );
}

/// Establish `CR4.PCE` clear, so ring 3 cannot execute `RDPMC`.
///
/// # The second door, and it is not the one milestone 228's block talks about
///
/// That block names `CR4.TSD` (bit 2), which gates `RDTSC`, and deliberately leaves it alone: this
/// architecture's `user_mode_runtime::now()` **is** `rdtsc` and there is no coarse counter to fall back to, so
/// closing it would take `Instant`, `thread::sleep`, the random seed, smoltcp's timestamps and the
/// benchmark harness away on one instruction. That trade is recorded in `notes/x86-port/user-mode-runtime.md` and in a
/// `BUGS` section beside `now()`, and nothing here changes it.
///
/// **`CR4.PCE` is bit 8 and gates a different instruction.** `RDPMC` reads a performance counter by
/// index, and fixed counter 2 (`CPU_CLK_UNHALTED.REF_TSC`) runs at the TSC rate, so an open `PCE` is
/// a second path to a cycle-rate instrument that has nothing to do with `TSD`. Nobody had looked
/// when 228 was minted, which is why its block named only two architectures as fixable.
///
/// This is the same defect the other two architectures had: a counter whose openness depended on
/// what firmware left rather than on anything this kernel decided. `boot.s` writes `CR4` exactly
/// twice, at the BSP and AP long-mode transitions, and both are `or eax, 1 << 5`, which is `PAE`.
/// Nothing has ever written bit 8 or programmed a perf-counter MSR.
///
/// # Why clearing it costs nothing, and what it would mean if it did
///
/// Nothing in this tree reads a performance counter from ring 3, or from ring 0 for that matter:
/// there is no perf-MSR programming anywhere under `arch/x86_64/`, so with the counters unprogrammed
/// an open `PCE` would let a process read zeros or firmware's leftovers rather than anything useful.
/// If clearing this ever breaks something, that is a finding rather than a reason to skip it, because
/// it would mean this tree already depends on a counter it never granted itself.
///
/// # It is read back, not assumed
///
/// The reset value of `CR4` is zero and this could have been a blind `and`. Assuming a reset value
/// is the exact habit milestone 228 exists to stop, so this reads `CR4` first and says so out loud
/// when the bit was set, which is the only case worth a line of boot output.
///
/// **On QEMU it is clear and this is silent, and that was read rather than assumed.** A temporary
/// probe printed `CR4` here on 2026-09-02 and got **0x20** on the PVH boot (`PAE` alone, which is
/// what `boot.s` sets) and **0x668** under OVMF (`DE`, `PAE`, `MCE`, `OSFXSR`, `OSXMMEXCPT`). Bit 8
/// was clear in both, so nothing was actually closed. But five bits this kernel never wrote were
/// already set by firmware before any of our code ran, on the one "firmware" this port has ever
/// booted under, and that is the argument for the read in one number. On the **Dell `OptiPlex`**
/// (xenon), real firmware runs first and the value is unknown.
///
/// Per core, like the GDT and the IDT above it: `CR4` is per-CPU state, so a secondary that skipped
/// this would run on whatever its own path left behind.
fn close_performance_counters_to_ring3() {
    /// `CR4.PCE`: "Performance-Monitoring Counter Enable". Set means `RDPMC` is legal at any
    /// privilege level; clear means ring 0 only.
    const PCE: u64 = 1 << 8;

    let cr4 = instructions::read_cr4();
    if cr4 & PCE == 0 {
        return;
    }

    crate::println!("  cr4.pce     : firmware left RDPMC open to ring 3; closing it");
    // SAFETY: clearing `CR4.PCE` only *removes* a ring-3 permission. It does not touch paging
    // (`PAE`, bit 5, is preserved by the mask), does not invalidate any TLB entry, and cannot make
    // a kernel access illegal, since `RDPMC` at CPL 0 is legal whatever this bit says. Every other
    // bit is written back as it was read.
    unsafe { instructions::write_cr4(cr4 & !PCE) };
}

/// Stop this CPU forever, cheaply. `hlt` parks it until an interrupt; with interrupts masked and
/// nothing left to wake it, that is the rest of time at zero host CPU. The same discipline as the
/// other two architectures' `wfi`. See CLAUDE.md, "Never leave QEMU running".
/// Takes a [`super::HaltReason`], whose constructors are the list of who may stop a core for good;
/// a thread whose work is done leaves with `sched::exit` instead (milestone 720 (provisional)).
pub fn halt(_: super::HaltReason) -> ! {
    loop {
        instructions::hlt();
    }
}

/// Park until the next interrupt (the scheduler's idle primitive).
pub fn wait_for_interrupt() {
    instructions::hlt();
}

/// This CPU's current stack pointer, for the stack-overflow canary check (stack.rs).
pub fn current_sp() -> u64 {
    instructions::read_rsp()
}

/// A DMA write memory barrier: order all prior stores before any device sees a later one.
///
/// **`sfence`, and the reason it is nearly free is the whole reason x86 is worth porting to.** This
/// machine is TSO: stores are already globally ordered with respect to each other, so the ordinary
/// case needs no instruction at all. `sfence` is here because non-temporal stores and
/// write-combining memory are the exceptions TSO does not cover, and a DMA buffer can legitimately
/// be either.
///
/// This is where rule #4's bet pays out in the direction it was made. Code proven correct under
/// ARM's weak model and RISC-V's RVWMO is correct here by construction; nothing about developing on
/// x86 first could have said the reverse, and the tree would have accumulated invisible
/// strong-ordering assumptions that only a real port would have found.
pub fn direct_memory_access_write_barrier() {
    instructions::sfence();
}

/// Make the instruction fetcher aware of code just written as data.
///
/// **Nothing to do, and this is the one place x86's complexity buys something.** The instruction
/// cache on x86 is architecturally coherent with the data caches: a store to an address, followed
/// by a fetch of that address, sees the store, on this core and on every other, with no
/// software action at all. aarch64 needs a clean/invalidate loop over cache lines and a broadcast;
/// RISC-V needs `fence.i` locally and an SBI RFENCE remotely (and getting that wrong is what hung
/// init on first silicon). Here the guarantee is the hardware's.
///
/// A serialising instruction is still required if the *modifying* store and the fetch are separated
/// by a jump the CPU may have already speculated past, which the callers here are not doing.
pub fn sync_icache(va: u64, len: usize) {
    let _ = (va, len);
}
