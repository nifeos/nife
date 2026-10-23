//! Exception handling.
//!
//! One mechanism serves faults, interrupts and syscalls: each switches to EL1 and jumps to the
//! vector table the kernel installed. See notes/exceptions.md, "The dispatcher's arms".

use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use aarch64_cpu::asm::barrier;
use aarch64_cpu::registers::{ESR_EL1, FAR_EL1, VBAR_EL1};
use tock_registers::interfaces::{Readable, Writeable};

use super::{irq, timer};
use crate::arch::{UserFault, UserFaultAccess};
use crate::println;

/// The interrupted CPU state, as saved by `SAVE_CONTEXT` in `vectors.s`.
///
/// This layout is a **contract with assembly**. The compiler cannot check it for us,
/// so there is a size assertion below, which catches about half of the ways to get it
/// wrong. The other half (reordering two fields of the same type) it cannot catch, so
/// be careful.
#[repr(C)]
pub struct TrapFrame {
    /// `x0` through `x30`. `x30` is the link register.
    pub x: [u64; 31],

    /// Where the interrupted code will resume.
    ///
    /// Writable: `eret` resumes here, which is how a `brk` is stepped over.
    pub elr: u64,

    /// The processor state (condition flags, exception level, interrupt masks) the
    /// interrupted code was in. `eret` restores it.
    pub spsr: u64,

    /// **The user's stack pointer.** `SP_EL0`, which is a different register from the
    /// `sp` the kernel is using.
    ///
    /// An exception from EL0 leaves `SP_EL0` alone, but a switch to another user thread would
    /// spend it, so it travels in the frame. It fills what would otherwise be the frame's padding.
    pub sp_el0: u64,
}

// If this fails, `SAVE_CONTEXT` and `TrapFrame` have drifted apart, and the Rust side
// is about to read the wrong bytes.
const _: () = assert!(size_of::<TrapFrame>() == 272);

impl TrapFrame {
    /// The syscall number the caller passed. aarch64 ABI: register `x8` (DECISIONS §10). The
    /// portable syscall dispatcher reads it through here so it never names a register directly.
    pub(crate) fn syscall_nr(&self) -> u64 {
        self.x[8]
    }

    /// Syscall argument register `i` (aarch64: `x0`..`x5`).
    pub fn arg(&self, i: usize) -> u64 {
        self.x[i]
    }

    /// Set syscall argument/return register `i`. The syscall return value and the IPC message words
    /// ride in these same registers, which is why the setter is general.
    pub fn set_arg(&mut self, i: usize, v: u64) {
        self.x[i] = v;
    }

    /// Build the frame that drops a brand-new thread to EL0 at `entry` on `user_sp`, with `args` in
    /// `x0`..`x2`, so portable `user.rs` never names `elr`, `spsr` or `sp_el0` (notes/riscv-port.md,
    /// leak #3).
    ///
    /// `spsr = 0` returns to `EL0t`, AArch64, with IRQs unmasked, so a tight-loop user thread is
    /// still preemptible (DECISIONS §5 (preemptive threads with real stacks)). Zero looks like a
    /// bug and is not.
    pub fn for_user_entry(entry: u64, user_sp: u64, args: [u64; 3]) -> Self {
        let mut x = [0u64; 31];
        x[0] = args[0]; // _start's first argument (AAPCS64 puts it in x0)
        x[1] = args[1];
        x[2] = args[2];
        TrapFrame {
            x,
            elr: entry, // where `eret` jumps
            spsr: 0,    // EL0t, interrupts on (see above)
            sp_el0: user_sp,
        }
    }
}

unsafe extern "C" {
    /// `mov sp, x0` then fall into `exception_restore`: `eret` into the EL0 state the frame
    /// describes. Defined in vectors.s.
    fn enter_userspace(frame: *mut TrapFrame) -> !;
}

/// Drop to EL0 by returning from the exception into `frame`, built by
/// [`TrapFrame::for_user_entry`]. `#[inline(always)]` is load-bearing, and a frame with `elr == 0`
/// is refused. See notes/exceptions.md, "Entering EL0 for the first time".
///
/// # Safety
/// `frame` must be a correctly-built, writable `TrapFrame` at the top of the current thread's kernel
/// stack, with EL0's code and stack mapped and `TTBR0` installed.
#[inline(always)]
pub unsafe fn enter_user(frame: *mut TrapFrame) -> ! {
    // SAFETY: the caller owns the frame's validity, as documented.
    let elr = unsafe { (*frame).elr };
    if elr == 0 {
        // SAFETY: as above; read before the cold call is allowed to use this stack.
        let sp_el0 = unsafe { (*frame).sp_el0 };
        entered_user_with_no_entry_point(sp_el0);
    }

    // SAFETY: the caller owns the frame's validity, as documented.
    unsafe { enter_userspace(frame) }
}

/// The `elr == 0` case, out of line so [`enter_user`] stays a tail call.
///
/// Its argument is read from the frame **before** it is called, because the frame overlaps this very
/// stack and this call is entitled to overwrite it, which is the corruption the check exists to name.
#[cold]
#[inline(never)]
fn entered_user_with_no_entry_point(sp_el0: u64) -> ! {
    panic!(
        "thread {} on core {} was dispatched to EL0 with elr = 0 (sp_el0 {:#018x}). \
         Its context was never built, or was built and not seen by this core.",
        crate::sched::current(),
        crate::cpu::id(),
        sp_el0,
    )
}

/// **Diagnostic: the EL0 PC of a thread from the `TrapFrame` at the top of its kernel stack.** A
/// thread that trapped from EL0 (a syscall, or a timer preemption while spinning) left its frame at
/// `stack_top - size_of::<TrapFrame>()`, where `elr` is its EL0 PC. Used by the watchdog dump to say
/// *where* each thread is, not just its scheduler state. Meaningless for a pure kernel thread.
pub(crate) fn user_pc(stack_top: u64) -> u64 {
    let frame = (stack_top - size_of::<TrapFrame>() as u64) as *const TrapFrame;
    // SAFETY: diagnostic read of the frame the vector's SAVE_CONTEXT wrote at the stack top.
    unsafe { core::ptr::read_volatile(core::ptr::addr_of!((*frame).elr)) }
}

/// **Write argument register `i` of a thread's saved user frame**, the one at the top of its kernel
/// stack that [`user_pc`] reads, so a blocked or trapped thread gets a result word beyond the five
/// its syscall path writes. The death message's label uses register 5 (DECISIONS §148 (a
/// supervisor restarts by asking)).
///
/// # Safety
/// `stack_top` must be the top of the kernel stack of a **user** thread that is inside a trap
/// from user mode (blocked in a syscall, or the caller's own), and nothing else may be writing that
/// frame. A pure kernel thread has no frame there: those bytes are its own stack.
///
/// Name: provisional, milestone 105 (the two forks)'s lane, 2026-10-05 (UTC). Named for `TrapFrame::set_arg`, which it applies to a
/// frame found by stack top the way `user_pc` reads one.
pub unsafe fn set_user_arg(stack_top: u64, i: usize, v: u64) {
    let frame = (stack_top - size_of::<TrapFrame>() as u64) as *mut TrapFrame;
    // SAFETY: the caller's contract: a live user frame at the stack top that nobody else writes.
    unsafe { (*frame).set_arg(i, v) }
}

/// How many `brk` instructions we have caught and stepped over.
///
/// Exists so the tests can prove the handler actually ran, rather than proving only
/// that we didn't crash.
pub(crate) static BRK_COUNT: AtomicUsize = AtomicUsize::new(0);

/// **Prove the trap path works end to end: execute a breakpoint and return**, the twin of the
/// riscv64 and `x86_64` `self_test`.
///
/// Returns how many breakpoints were caught during the call. If the vector is not wired, this
/// never returns.
// The machine description and the boot self-test are the only callers, and both are
// `#[cfg(not(any(test, feature = "bench")))]`: a test boot exits through semihosting and a bench
// boot diverges into `bench::run`, so neither reads a bring-up transcript. Same treatment
// `memory::print_summary` already carries, and for the same reason.
#[cfg_attr(
    any(test, feature = "system_tests", feature = "bench"),
    allow(dead_code)
)]
pub(crate) fn self_test() -> usize {
    let before = BRK_COUNT.load(Ordering::Relaxed);
    // SAFETY: `brk #0` raises a synchronous exception the dispatcher handles, and the guard in that
    // arm (`!is_from_lower_el`) admits this one because it comes from EL1. It has no other effect.
    unsafe { core::arch::asm!("brk #0") };
    BRK_COUNT.load(Ordering::Relaxed) - before
}

/// Exception Class: `ESR_EL1` bits 31:26.
///
/// What kind of exception happened; the rest of `ESR_EL1` is detail.
mod ec {
    pub(crate) const UNKNOWN: u64 = 0x00;
    pub(crate) const TRAPPED_WFI_WFE: u64 = 0x01;
    /// Access to SVE, Advanced SIMD or floating-point functionality, trapped by
    /// `CPACR_EL1.FPEN`: the first-use trap. See `arch::aarch64::fp`.
    pub(crate) const FP_SIMD_ACCESS: u64 = 0x07;
    pub(crate) const ILLEGAL_EXECUTION_STATE: u64 = 0x0e;
    pub(crate) const SVC64: u64 = 0x15;
    pub(crate) const TRAPPED_MSR_MRS: u64 = 0x18;
    pub(crate) const INSTRUCTION_ABORT_LOWER_EL: u64 = 0x20;
    pub(crate) const INSTRUCTION_ABORT_SAME_EL: u64 = 0x21;
    pub(crate) const PC_ALIGNMENT_FAULT: u64 = 0x22;
    pub(crate) const DATA_ABORT_LOWER_EL: u64 = 0x24;
    pub(crate) const DATA_ABORT_SAME_EL: u64 = 0x25;
    pub(crate) const SP_ALIGNMENT_FAULT: u64 = 0x26;
    pub(crate) const SERROR: u64 = 0x2f;
    pub(crate) const BREAKPOINT_LOWER_EL: u64 = 0x30;
    pub(crate) const BREAKPOINT_SAME_EL: u64 = 0x31;
    pub(crate) const BRK64: u64 = 0x3c;
}

fn ec_name(class: u64) -> &'static str {
    match class {
        ec::UNKNOWN => "Unknown reason",
        ec::TRAPPED_WFI_WFE => "Trapped WFI/WFE",
        ec::ILLEGAL_EXECUTION_STATE => "Illegal execution state",
        ec::SVC64 => "SVC (syscall) from AArch64",
        ec::TRAPPED_MSR_MRS => "Trapped system register access",
        ec::INSTRUCTION_ABORT_LOWER_EL => "Instruction abort from a lower EL",
        ec::INSTRUCTION_ABORT_SAME_EL => "Instruction abort from the same EL",
        ec::PC_ALIGNMENT_FAULT => "PC alignment fault",
        ec::DATA_ABORT_LOWER_EL => "Data abort from a lower EL",
        ec::DATA_ABORT_SAME_EL => "Data abort from the same EL",
        ec::SP_ALIGNMENT_FAULT => "SP alignment fault",
        ec::SERROR => "SError",
        ec::BREAKPOINT_LOWER_EL => "Breakpoint from a lower EL",
        ec::BREAKPOINT_SAME_EL => "Breakpoint from the same EL",
        ec::BRK64 => "BRK instruction",
        _ => "unrecognized exception class",
    }
}

/// The sixteen slots in the vector table, in hardware order. See `vectors.s`.
const VECTOR_NAMES: [&str; 16] = [
    "Current EL, SP_EL0, Synchronous",
    "Current EL, SP_EL0, IRQ",
    "Current EL, SP_EL0, FIQ",
    "Current EL, SP_EL0, SError",
    "Current EL, SP_ELx, Synchronous",
    "Current EL, SP_ELx, IRQ",
    "Current EL, SP_ELx, FIQ",
    "Current EL, SP_ELx, SError",
    "Lower EL, AArch64, Synchronous",
    "Lower EL, AArch64, IRQ",
    "Lower EL, AArch64, FIQ",
    "Lower EL, AArch64, SError",
    "Lower EL, AArch32, Synchronous",
    "Lower EL, AArch32, IRQ",
    "Lower EL, AArch32, FIQ",
    "Lower EL, AArch32, SError",
];

/// Install the vector table.
///
/// After this returns, a fault produces a legible report instead of a silent death.
/// Until it returns, it doesn't.
pub fn init() {
    unsafe extern "C" {
        static exception_vectors: core::ffi::c_void;
    }

    let base = (&raw const exception_vectors) as u64;
    VBAR_EL1.set(base);

    // Without the `isb` the VBAR_EL1 write need not be in effect for the next instruction, which is
    // when a fault might arrive. See notes/exceptions.md, "The ISB that is easy to forget".
    barrier::isb(barrier::SY);
}

/// Vector slot 5: Current EL, `SP_ELx`, IRQ. **The kernel being interrupted.**
const VECTOR_IRQ_CURRENT: u64 = 5;
/// Vector slot 9: Lower EL, AArch64, IRQ. Userspace being interrupted. Milestone 7.
const VECTOR_IRQ_LOWER: u64 = 9;

unsafe extern "C" {
    /// Switch to `top` (or stay put if it is 0), call [`exception_body`], come back.
    /// Defined in vectors.s, because moving `sp` is assembly and policy is not.
    fn dispatch_on_interrupt_stack(frame: &mut TrapFrame, index: u64, top: u64) -> bool;
}

/// **The outer half of the trap path, on the interrupted stack**, called from every vector entry
/// with the saved state and which slot fired (`x0`, `x1`, per AAPCS64; notes/registers.md).
///
/// It picks the stack the handler runs on, and runs any deferred `schedule()` afterwards, which may
/// run only on a stack the thread owns. See `kernel/src/interrupt_stack.rs` and notes/exceptions.md,
/// "The split around the interrupt stack".
#[unsafe(no_mangle)]
// In the pinned hot section: milestone 796 (pin the hot trap path's placement).
#[unsafe(link_section = ".text.hot.exception_dispatch")]
extern "C" fn exception_dispatch(frame: &mut TrapFrame, index: u64) {
    let top = crate::interrupt_stack::top_for_trap(is_from_lower_el(index));
    let deferred_switch = if top == 0 {
        // Every syscall: branched in Rust, because the debug build the icount gate measures pays
        // for a trampoline call that moves no stack.
        exception_body(frame, index)
    } else {
        // SAFETY: `top` is this core's own interrupt-stack top, from the module that owns the
        // region; the trampoline calls `exception_body` with our own two arguments and restores
        // `sp` before returning. The frame outlives the call: it is on the stack we are standing on.
        unsafe { dispatch_on_interrupt_stack(frame, index, top) }
    };

    // Back on the interrupted thread's stack, whichever branch ran. Preemption happens HERE.
    if deferred_switch {
        crate::sched::preempt_if_needed();
    }
}

/// The trap handler proper: everything that runs on the interrupt stack when there is one.
///
/// Returns whether the caller owes a deferred `schedule()`: true for an IRQ, false otherwise.
#[unsafe(no_mangle)]
extern "C" fn exception_body(frame: &mut TrapFrame, index: u64) -> bool {
    // By slot, not by ESR: ESR_EL1 describes the last synchronous exception, not an IRQ.
    if index == VECTOR_IRQ_CURRENT || index == VECTOR_IRQ_LOWER {
        handle_irq(frame);
        return true;
    }

    let esr = ESR_EL1.get();
    let class = (esr >> 26) & 0x3f;

    match class {
        // From EL1 only: a `brk` from EL0 is a fault, or a program could loop on one forever.
        ec::BRK64 if !is_from_lower_el(index) => {
            // ELR_EL1 points at the `brk`, not past it; every instruction is 4 bytes.
            BRK_COUNT.fetch_add(1, Ordering::Relaxed);
            frame.elr += 4;
        }

        // FP first use, from EL0 or EL1: `elr` is not advanced, so the instruction re-executes
        // with the unit open. False (no thread to record against) falls through rather than
        // retaking the trap forever.
        ec::FP_SIMD_ACCESS if crate::fp::enable_for_current() => {}

        // The syscall. ELR already points past the `svc`, and `dispatch` writes its result into
        // the frame, which RESTORE_CONTEXT returns to the user's registers.
        ec::SVC64 if is_from_lower_el(index) => {
            #[cfg(any(test, feature = "system_tests"))]
            SVC_COUNT.fetch_add(1, Ordering::Relaxed);
            crate::syscall::dispatch(frame);
        }

        // Anything else from EL0 kills the thread, not the kernel.
        _ if is_from_lower_el(index) => user_fault(frame, esr),

        // Everything else is a kernel bug.
        _ => fatal(frame, index, esr),
    }

    false
}

/// Did this exception come from a lower exception level, i.e. from EL0?
///
/// Slots 8-11 are "Lower EL, AArch64" (see `VECTOR_NAMES`). `#[inline(always)]` and two written-out
/// comparisons because the icount gate measures a debug build; see notes/exceptions.md, "Costs the
/// debug build pays".
#[inline(always)]
// A measured exception to clippy's `(8..=11).contains(&index)`, for the reason above.
#[allow(clippy::manual_range_contains)]
fn is_from_lower_el(index: u64) -> bool {
    index >= 8 && index <= 11
}

/// **System calls served, counted for the system tests only** (`system_tests/src/user/el0_boundary_tests.rs`
/// watches it rise). Compiled out of every other build: it is a shared write on the cheapest
/// syscall's path (notes/job-mix/null-syscall-under-load.md).
#[cfg(any(test, feature = "system_tests"))]
pub static SVC_COUNT: AtomicUsize = AtomicUsize::new(0);

/// How many user threads have been killed for faulting.
pub static USER_FAULTS: AtomicUsize = AtomicUsize::new(0);

/// How many device/SGI interrupts were routed to a userspace endpoint. Bring-up diagnostic.
pub static ROUTED_IRQS: AtomicUsize = AtomicUsize::new(0);

/// The most recent user fault, [`UserFault::encode`]d, and the address it named. Two relaxed
/// stores on a path that is already killing a thread; read back through [`last_user_fault`].
///
/// Private, so a test reads the fault in words both ISAs can say ([`UserFault`]), not as `ESR_EL1`.
static LAST_USER_FAULT: AtomicU64 = AtomicU64::new(0);
static LAST_USER_FAULT_ADDR: AtomicU64 = AtomicU64::new(0);

/// The last user fault's kind and the address it named, or `None` if no user thread has faulted
/// yet. The RISC-V twin is `arch::riscv64::exceptions::last_user_fault`.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
pub fn last_user_fault() -> Option<(UserFault, u64)> {
    // PAIR: `USER_FAULTS.fetch_add(1, Ordering::Release)` in [`user_fault`]. A caller that saw the
    // counter rise reads the record that core wrote, not an older one.
    core::sync::atomic::fence(Ordering::Acquire);
    let kind = UserFault::decode(LAST_USER_FAULT.load(Ordering::Relaxed))?;
    Some((kind, LAST_USER_FAULT_ADDR.load(Ordering::Relaxed)))
}

/// Read `ESR_EL1` as a [`UserFault`].
///
/// The fault status code in `ESR_EL1[5:0]` says permission (`0b0011LL`) or translation
/// (`0b0001LL`) outright, with the walk level in the low two bits.
fn classify(esr: u64) -> UserFault {
    /// EC for an instruction abort taken from a lower EL: a failed *fetch*.
    const EC_INSTRUCTION_ABORT_LOWER: u64 = 0x20;
    /// EC for a data abort taken from a lower EL: a failed load or store.
    const EC_DATA_ABORT_LOWER: u64 = 0x24;
    /// ISS bit 6 of a data abort: set for a write, clear for a read.
    const WNR: u64 = 1 << 6;

    let access = match (esr >> 26) & 0x3f {
        EC_INSTRUCTION_ABORT_LOWER => UserFaultAccess::Fetch,
        EC_DATA_ABORT_LOWER if esr & WNR != 0 => UserFaultAccess::Write,
        EC_DATA_ABORT_LOWER => UserFaultAccess::Read,
        // An illegal instruction, a `brk`, a stack-alignment fault: not a memory access, so
        // neither "permission" nor "translation" is a true thing to say about it.
        _ => return UserFault::Other,
    };

    // (I|D)FSC[5:2] is the fault type; [1:0] is the walk level, which nothing above this cares
    // about. 0b0001 = translation, 0b0011 = permission.
    match (esr >> 2) & 0xf {
        0b0001 => UserFault::Translation(access),
        0b0011 => UserFault::Permission(access),
        _ => UserFault::Other,
    }
}

/// A user thread did something it is not allowed to do. Kill it.
///
/// Records the fault, then hands the thread to `sched::fault`, which does not return. See
/// notes/exceptions.md, "A user fault is published, then counted".
fn user_fault(frame: &TrapFrame, esr: u64) -> ! {
    let class = (esr >> 26) & 0x3f;
    let far = FAR_EL1.get();

    // The record first, the counter last, as the release: PAIR with the fence in
    // [`last_user_fault`].
    LAST_USER_FAULT_ADDR.store(far, Ordering::Relaxed);
    LAST_USER_FAULT.store(classify(esr).encode(), Ordering::Relaxed);
    USER_FAULTS.fetch_add(1, Ordering::Release);

    crate::println!();
    crate::println!(
        "  user thread {} killed: {}",
        crate::sched::current(),
        ec_name(class),
    );
    crate::println!(
        "    pc {:#018x}   far {:#018x}   user sp {:#018x}   esr {:#010x}",
        frame.elr,
        far,
        frame.sp_el0,
        esr,
    );
    crate::println!("  the kernel is fine.");

    // To a supervisor if there is one, else `exit`'s path (§26 (the fault endpoint)).
    crate::sched::fault(frame.elr, far);
}

/// Service one hardware interrupt.
///
/// Context: interrupt context, IRQs masked by the vector. Records and defers (§9 (locking:
/// IrqSafeMutex, plus a discipline)): allocates nothing and takes no lock above rank GIC.
fn handle_irq(_frame: &mut TrapFrame) {
    // The acknowledge has a side effect: exactly once.
    let intid = irq::acknowledge();

    // Spurious: no end-of-interrupt, which would corrupt the GIC's priority stack.
    if intid == irq::SPURIOUS {
        SPURIOUS_IRQS.fetch_add(1, Ordering::Relaxed);
        return;
    }

    match intid {
        timer::TIMER_INTID => {
            timer::tick();
            // Record only: the GIC has not had its end-of-interrupt yet.
            crate::sched::on_tick();
        }
        crate::sched::RESCHED_SGI => {
            // One SGI carries both an inbox hand-off and a steal request (§28 (SMP placement)).
            crate::sched::drain_inbox();
            crate::sched::serve_steal_request();
        }
        other => {
            // Masked before the notification, or a level-triggered line re-fires on the EOI; the
            // driver's ACK re-enables it. See notes/interrupts.md.
            if let Some(ep) = crate::sched::irq_route(other) {
                ROUTED_IRQS.fetch_add(1, Ordering::Relaxed);
                irq::disable(other);
                crate::sched::irq_notify(ep);
            } else {
                UNEXPECTED_IRQS.fetch_add(1, Ordering::Relaxed);
                println!("[IRQ] unexpected interrupt {other}, ignoring");
            }
        }
    }

    // Before the deferred switch in `exception_dispatch`: until it is written the GIC delivers
    // nothing of equal or lower priority, to this thread or the one switched to.
    irq::end_of_interrupt(intid);
}

/// Interrupts the GIC raised and then withdrew. Not an error; worth counting.
pub(crate) static SPURIOUS_IRQS: AtomicUsize = AtomicUsize::new(0);

/// Interrupts we enabled but have no handler for. Definitely worth counting.
pub(crate) static UNEXPECTED_IRQS: AtomicUsize = AtomicUsize::new(0);

/// Print everything we know and stop.
///
/// Does not return.
fn fatal(frame: &TrapFrame, index: u64, esr: u64) -> ! {
    // SAFETY: same reasoning as the panic handler. A fault taken mid-`println!` would
    // otherwise deadlock on the console lock and we would print nothing at all.
    // SAFETY: same reasoning as the panic handler. A fault taken while holding a lock would
    // otherwise deadlock, or trip the lock-ranking assertion, and we would print nothing at all.
    unsafe {
        crate::sync::force_reset_ranks();
        crate::console::force_unlock();
    };

    let class = (esr >> 26) & 0x3f;
    let name = VECTOR_NAMES
        .get(index as usize)
        .copied()
        .unwrap_or("<vector index out of range>");

    println!();
    println!("[EXCEPTION]  {name}");
    println!("             {} (EC {:#04x})", ec_name(class), class);
    println!();
    println!("  ESR_EL1   {esr:#018x}   what happened");

    // FAR_EL1 holds an address only for aborts and PC alignment faults; otherwise it is stale.
    let far_is_meaningful = matches!(
        class,
        ec::INSTRUCTION_ABORT_LOWER_EL
            | ec::INSTRUCTION_ABORT_SAME_EL
            | ec::DATA_ABORT_LOWER_EL
            | ec::DATA_ABORT_SAME_EL
            | ec::PC_ALIGNMENT_FAULT
    );
    if far_is_meaningful {
        println!(
            "  FAR_EL1   {:#018x}   the address that faulted",
            FAR_EL1.get()
        );
        // The interrupted `SP_EL1` is the frame's address plus its size; the live `sp` may be the
        // interrupt stack.
        let trapped_sp = frame as *const TrapFrame as u64 + size_of::<TrapFrame>() as u64;
        crate::stack::warn_if_guard_page(FAR_EL1.get(), trapped_sp);
    } else {
        println!("  FAR_EL1   (not meaningful for this exception class)");
    }

    println!(
        "  ELR_EL1   {:#018x}   the instruction that did it",
        frame.elr
    );
    println!("  SPSR_EL1  {:#018x}   the state it was in", frame.spsr);
    println!();

    for row in 0..8 {
        let a = row * 4;
        print_reg_row(frame, a);
    }
    println!();
    crate::stack::warn_if_smashed();

    panic!("unhandled exception: {}", ec_name(class));
}

/// Four registers per line, x28..x30 on the short last row.
fn print_reg_row(frame: &TrapFrame, first: usize) {
    use crate::print;
    print!("  ");
    for (i, x) in frame
        .x
        .iter()
        .enumerate()
        .take((first + 4).min(31))
        .skip(first)
    {
        print!("x{i:<2} {x:#018x}  ");
    }
    println!();
}

#[cfg(test)]
mod tests {
    //! Tests for exception handling.
    //!
    //! `registers_survive_an_exception` is the load-bearing one: a wrong `TrapFrame` offset
    //! scrambles a register and still returns to the right address.

    /// Proves the vector table is installed, and that the hardware's alignment rule
    /// is satisfied.
    ///
    /// The CPU computes `VBAR_EL1 + offset` assuming the low 11 bits are zero.
    #[test_case]
    fn vbar_el1_points_at_our_vector_table() {
        use aarch64_cpu::registers::VBAR_EL1;
        use tock_registers::interfaces::Readable;

        unsafe extern "C" {
            static exception_vectors: core::ffi::c_void;
        }
        let expected = (&raw const exception_vectors) as u64;

        assert_eq!(VBAR_EL1.get(), expected, "VBAR_EL1 not installed");
        assert_eq!(expected % 2048, 0, "vector table misaligned: {expected:#x}");
    }

    /// The real one: take an exception and come back from it.
    ///
    /// Reaching the line after `brk #0` needs the table, slot 4, `SAVE_CONTEXT`, the decode, the
    /// ELR step and `RESTORE_CONTEXT` all right; a failure is a hang or a crash, not an assertion.
    #[test_case]
    fn breakpoint_is_caught_and_execution_resumes() {
        use core::sync::atomic::Ordering;

        use crate::arch::exceptions::BRK_COUNT;

        let before = BRK_COUNT.load(Ordering::Relaxed);

        // SAFETY: this deliberately faults. We handle it.
        unsafe { core::arch::asm!("brk #0") };

        assert_eq!(
            BRK_COUNT.load(Ordering::Relaxed),
            before + 1,
            "the handler didn't run, but we resumed anyway?"
        );
    }

    /// Proves the trap frame actually round-trips a register.
    ///
    /// A wrong offset or a swapped pair in `SAVE_CONTEXT`/`RESTORE_CONTEXT` would still return to
    /// the right address, so put a known value in a register, trap, and read it back.
    #[test_case]
    fn registers_survive_an_exception() {
        let sent: u64 = 0xdead_beef_cafe_f00d;
        let got: u64;

        // SAFETY: deliberately faults; we handle it. x20 is callee-saved, so we tell
        // the compiler we're clobbering it.
        unsafe {
            core::arch::asm!(
                "mov x20, {sent}",
                "brk #0",
                "mov {got}, x20",
                sent = in(reg) sent,
                got = out(reg) got,
                out("x20") _,
            );
        }

        assert_eq!(got, sent, "the trap frame scrambled a register");
    }
}
