//! RISC-V traps: the `stvec` vector, the saved [`TrapFrame`], and the dispatch into the portable
//! syscall and fault handlers.
//!
//! trap.s is the single trap entry. It fills a [`TrapFrame`] and calls [`riscv_trap_dispatch`],
//! which fans out on `scause`. The portable syscall dispatcher reads `ecall`'s `a7` and `a0`..`a5`
//! through `TrapFrame::{syscall_nr, arg, set_arg}`.
//!
//! See notes/riscv-port/traps.md.

use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use crate::arch::{UserFault, UserFaultAccess};

/// The registers saved on a trap: the general registers, then the trap CSRs. `#[repr(C)]` because
/// trap.s fills it field for field.
///
/// `x` is `pub` because the portable syscall dispatcher indexes it; the rest is arch-internal.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TrapFrame {
    /// The general registers `x0`..`x31`. `x0` is always zero; it is kept in the array so an index
    /// *is* a register number.
    pub x: [u64; 32],
    /// `sepc`: the PC the trap interrupted, where `sret` resumes.
    pub sepc: u64,
    /// `scause`: the trap cause (top bit = interrupt vs exception).
    pub scause: u64,
    /// `stval`: the trap value (faulting address, bad instruction, ...).
    pub stval: u64,
    /// `sstatus` at the trap, restored on the way out.
    pub sstatus: u64,
}

// trap.s spells 288 by hand: both `addi sp, sp, -288` in `trap_entry`, `addi sp, sp, 288` in
// `trap_return`, and the frame reservation in `user_entry_trampoline`.
const _: () = assert!(size_of::<TrapFrame>() == 288);

impl TrapFrame {
    /// The syscall number the caller passed, in `a7` (`x17`), so the portable dispatcher names no
    /// register (DECISIONS §17).
    pub(crate) fn syscall_nr(&self) -> u64 {
        self.x[17] // a7
    }

    /// Syscall argument register `i` (RISC-V: `a0`..`a5`, i.e. `x10`..`x15`).
    pub fn arg(&self, i: usize) -> u64 {
        self.x[10 + i]
    }

    /// Set syscall argument/return register `i`. The return value and IPC message words ride in
    /// `a0`..`a2` (`x10`..`x12`).
    pub fn set_arg(&mut self, i: usize, v: u64) {
        self.x[10 + i] = v;
    }

    /// Set the saved `tp` (`x4`) a return to U-mode restores (milestone 812 (`std::thread::spawn`
    /// runs real threads in one address space)). Only a U-mode frame's `tp` is ever restored:
    /// `trap_return` keeps the live per-hart pointer for an S-mode return. See `thread_pointer`.
    pub fn set_thread_pointer(&mut self, v: u64) {
        self.x[4] = v;
    }

    /// Build the frame that drops a new thread to U-mode at `entry` on `user_sp`, with `args` in
    /// `a0`..`a2` and its own thread pointer in `tp`.
    ///
    /// `sstatus` has SPP = 0 (return to U-mode), SPIE = 1 (interrupts on after `sret`, so the
    /// thread stays preemptible) and UXL = 64-bit. See notes/riscv-port/hal-leaks.md.
    pub fn for_user_entry(entry: u64, user_sp: u64, args: [u64; 3]) -> Self {
        // `trap_return` writes this whole word into sstatus, so UXL (bits 33:32) must be 2 here or
        // the first user instruction faults on an illegal XLEN.
        const SPIE: u64 = 1 << 5;
        const UXL_64: u64 = 2 << 32;

        // SIE must be clear in a fabricated frame. `sstatus` holds the live interrupt enable as
        // well as the staged fields, so SIE = 1 would unmask interrupts inside `trap_return` after
        // `sepc` is staged (notes/exceptions.md, the exception-return race).
        const SIE: u64 = 1 << 1;
        const _: () = assert!((SPIE | UXL_64) & SIE == 0);

        let mut x = [0u64; 32];
        x[10] = args[0]; // a0: _start's first argument
        x[11] = args[1]; // a1
        x[12] = args[2]; // a2
        x[2] = user_sp; // sp
        // tp is the thread's own pointer (§269 fork 4), zero if it has none, never the kernel's
        // per-hart one. Read now, while the frame is a value: once written at the stack top no call
        // may run (`enter_user`). See notes/riscv-port.md, the per-hart `tp` hazard.
        x[4] = crate::sched::current_thread_pointer();
        TrapFrame {
            x,
            sepc: entry,
            scause: 0,
            stval: 0,
            sstatus: SPIE | UXL_64,
        }
    }
}

unsafe extern "C" {
    /// Load `frame` as the trap frame and `sret` into it (the first entry to U-mode). Defined in
    /// trap.s; shares the restore path with the trap return. Its first instruction is `mv sp, a0`,
    /// so it does not touch the caller's stack.
    fn user_return(frame: *mut TrapFrame) -> !;
}

/// Drop to U-mode by loading `frame` and executing `sret`, the twin of aarch64's `enter_user`.
///
/// `#[inline(always)]` because the frame sits at the top of the caller's kernel stack, and a pushed
/// call frame would overwrite it before `user_return`'s `mv sp, a0`. Panics on `sepc == 0`.
///
/// # Safety
/// `frame` must be a correctly-built, writable `TrapFrame` at the top of the current thread's kernel
/// stack, with the user address space installed.
#[inline(always)]
pub unsafe fn enter_user(frame: *mut TrapFrame) -> ! {
    // A load and a branch, no call: nothing is pushed over the frame on the hot path. `sepc == 0`
    // would surface as a lost-wakeup hang far from its cause. See notes/riscv-port/traps.md.
    //
    // SAFETY: the caller's contract says `frame` is a valid, writable `TrapFrame`.
    let sepc = unsafe { (*frame).sepc };
    if sepc == 0 {
        // SAFETY: as above; read before the cold call below is allowed to use this stack.
        let user_sp = unsafe { (*frame).x[2] };
        entered_user_with_no_entry_point(user_sp);
    }

    // SAFETY: the caller's contract; `user_return` never returns.
    unsafe { user_return(frame) }
}

/// The `sepc == 0` case, out of line so [`enter_user`] stays a tail jump.
///
/// Its arguments are read from the frame **before** it is called, because the frame lives at the top
/// of this very stack and this call is entitled to overwrite it.
#[cold]
#[inline(never)]
fn entered_user_with_no_entry_point(user_sp: u64) -> ! {
    panic!(
        "thread {} on core {} was dispatched to U-mode with sepc = 0 (user sp {:#018x}). \
         Its context was never built, or was built and not seen by this core.",
        crate::sched::current(),
        crate::cpu::id(),
        user_sp,
    )
}

/// The U-mode PC of a thread, read from the `TrapFrame` at `stack_top - 288`, for the watchdog
/// dump. Meaningless for a pure kernel thread, which has no frame there.
pub(crate) fn user_pc(stack_top: u64) -> u64 {
    let frame = (stack_top - size_of::<TrapFrame>() as u64) as *const TrapFrame;
    // SAFETY: a diagnostic read of the frame `trap_entry` writes at the stack top. Volatile because
    // the owning thread is running on another core while we read, and this must not be hoisted.
    unsafe { core::ptr::read_volatile(&raw const (*frame).sepc) }
}

/// Write `v` to argument register `i` of the user frame at `stack_top`, which the return to
/// U-mode restores: a result word the syscall path does not write, like the death label (§148).
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

/// Interrupts routed to a userspace handler (delegated IRQs). Bumped by the trap dispatcher.
pub static ROUTED_IRQS: AtomicUsize = AtomicUsize::new(0);

/// Interrupts taken with no source enabled or routed to explain them.
pub(crate) static SPURIOUS_IRQS: AtomicUsize = AtomicUsize::new(0);

/// System calls served, in test builds only: every syscall on every core writes this one line
/// (notes/job-mix/null-syscall-under-load.md).
#[cfg(any(test, feature = "system_tests"))]
pub static SVC_COUNT: AtomicUsize = AtomicUsize::new(0);

/// User faults taken (a page fault or illegal instruction from U-mode). Read by the boot tour;
/// bumped by the trap dispatcher.
pub static USER_FAULTS: AtomicUsize = AtomicUsize::new(0);

/// Breakpoints (`ebreak`) caught from S-mode, so a test can prove the trap round trip ran.
pub(crate) static BRK_COUNT: AtomicUsize = AtomicUsize::new(0);

/// The `sstatus.SPP` bit: the privilege the trap came from (0 = U-mode, 1 = S-mode).
const SPP: u64 = 1 << 8;
/// The high bit of `scause`: set for an interrupt, clear for an exception.
const INTERRUPT: u64 = 1 << 63;
/// `scause` interrupt code for a supervisor external interrupt: a device raised a line into the
/// PLIC, which routed it here. The RISC-V analog of an aarch64 IRQ exception carrying a GIC intid.
const S_EXTERNAL: u32 = 9;
/// `scause` interrupt code for a supervisor software interrupt: another hart's reschedule IPI, set
/// through the SBI (which pends `sip.SSIP`). The RISC-V analog of aarch64's reschedule SGI.
const S_SOFTWARE: u32 = 1;

/// Clear this hart's pending software interrupt (`sip.SSIP`, bit 1). The IPI stays pending until
/// acknowledged, so the handler clears it or it would re-fire the instant interrupts reopen.
fn clear_software_interrupt() {
    const SSIP: u64 = 1 << 1;
    super::instructions::clear_sip(SSIP);
}

/// Unmask this hart's software interrupts (`sie.SSIE`, bit 1): the reschedule-IPI source. Armed per
/// hart when it becomes a scheduler participant, alongside the timer's `sie.STIE`. Without it a
/// reschedule IPI sets `sip.SSIP` but is never taken, so a migrated thread would sit in the inbox
/// until the target's next timer tick.
pub(crate) fn enable_software_interrupts() {
    const SSIE: u64 = 1 << 1;
    // Takes effect under sstatus.SIE.
    super::instructions::set_sie(SSIE);
}
/// `scause` exception code for `ecall` taken from U-mode: the syscall.
const CAUSE_ECALL_U: u64 = 8;
/// `scause` exception code for a breakpoint (`ebreak`).
const CAUSE_BREAKPOINT: u64 = 3;
/// `scause` exception code for an illegal instruction, which is **also** what an FP instruction
/// raises while `sstatus.FS` is Off. See the arm that reads it in [`riscv_trap_body`].
const CAUSE_ILLEGAL_INSTRUCTION: u64 = 2;
/// `scause` exception code for a page fault on an instruction fetch.
const CAUSE_INSTRUCTION_PAGE_FAULT: u64 = 12;
/// `scause` exception code for a page fault on a load.
const CAUSE_LOAD_PAGE_FAULT: u64 = 13;
/// `scause` exception code for a page fault on a store or an atomic.
const CAUSE_STORE_PAGE_FAULT: u64 = 15;

/// The most recent user fault, [`UserFault::encode`]d, and the address it named (`stval`). Read
/// back through [`last_user_fault`]; the aarch64 twin is `aarch64::exceptions::last_user_fault`.
static LAST_USER_FAULT: AtomicU64 = AtomicU64::new(0);
static LAST_USER_FAULT_ADDR: AtomicU64 = AtomicU64::new(0);

/// The last user fault's kind and the address it named, or `None` if no user thread has faulted
/// yet. [`classify`] says how the kind is derived.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
pub fn last_user_fault() -> Option<(UserFault, u64)> {
    // A caller that saw the counter rise reads the record the faulting hart wrote.
    //
    // PAIR: `USER_FAULTS.fetch_add(1, Ordering::Release)` in [`user_fault`], below in this file.
    core::sync::atomic::fence(Ordering::Acquire);
    let kind = UserFault::decode(LAST_USER_FAULT.load(Ordering::Relaxed))?;
    Some((kind, LAST_USER_FAULT_ADDR.load(Ordering::Relaxed)))
}

/// Read a U-mode trap with exception `code` as a [`UserFault`].
///
/// `scause` gives the access kind and not the reason, so a mapped `stval` in the current space
/// ([`mmu::is_mapped_in_current_space`]) is read as a permission fault, an unmapped one as a
/// translation fault. Returns [`UserFault::Other`] for a cause that is not a page fault.
///
/// # BUGS
///
/// The walk runs after the fault, so a concurrent map or unmap on another hart misclassifies it.
/// See notes/riscv-port/traps.md.
fn classify(frame: &TrapFrame, code: u64) -> UserFault {
    let access = match code {
        CAUSE_INSTRUCTION_PAGE_FAULT => UserFaultAccess::Fetch,
        CAUSE_LOAD_PAGE_FAULT => UserFaultAccess::Read,
        CAUSE_STORE_PAGE_FAULT => UserFaultAccess::Write,
        // Not a memory-permission question, so neither answer would be true.
        _ => return UserFault::Other,
    };

    if super::mmu::is_mapped_in_current_space(frame.stval) {
        UserFault::Permission(access)
    } else {
        UserFault::Translation(access)
    }
}

/// Unmask supervisor external interrupts (`sie.SEIE`, bit 9), the PLIC's deliveries. They are taken
/// only while `sstatus.SIE` is on.
pub(crate) fn enable_external() {
    const SEIE: u64 = 1 << 9;
    // Setting sie.SEIE only unmasks the external-interrupt source; it takes effect under SIE.
    super::instructions::set_sie(SEIE);
}

/// Install the trap vector: `stvec` = `trap_entry`, in direct mode. The low two bits of `stvec`
/// select the mode, and trap.s's `.balign 4` keeps them 0.
pub fn init() {
    unsafe extern "C" {
        fn trap_entry();
    }
    let vector = trap_entry as *const () as usize;
    // SAFETY: `vector` is our 4-byte-aligned trap entry (trap.s's `.balign 4`).
    unsafe { super::instructions::write_stvec(vector) };
}

/// Advance `sepc` past the instruction that trapped: 2 bytes if it is compressed (low two bits not
/// `0b11`), otherwise 4. Used to step over a handled breakpoint. `ecall` is always 4 bytes.
fn advance_past_trapping_insn(frame: &mut TrapFrame) {
    // SAFETY: `sepc` is the address of the instruction that trapped, which is mapped and readable.
    let low = unsafe { core::ptr::read_volatile(frame.sepc as *const u16) };
    // Wrapping, to keep the overflow check off the trap body (notes/overflow-checks.md): the one
    // caller is the S-mode breakpoint arm, so `sepc` is in kernel `.text`, far below `2^64 - 4`.
    frame.sepc = frame
        .sepc
        .wrapping_add(if low & 0b11 == 0b11 { 4 } else { 2 });
}

unsafe extern "C" {
    /// Switch to `top` (or stay put if it is 0), call [`riscv_trap_body`], come back.
    /// Defined in trap.s, because moving `sp` is assembly and policy is not.
    fn dispatch_on_interrupt_stack(frame: &mut TrapFrame, top: u64) -> bool;
}

/// The outer half of the trap path, which stays on the interrupted stack.
///
/// Picks the stack [`riscv_trap_body`] runs on, then runs any deferred `schedule()` on the
/// interrupted thread's own stack. See kernel/src/interrupt_stack.rs.
///
/// Context: interrupt context, with interrupts masked.
#[unsafe(no_mangle)]
// In the pinned hot section: milestone 796 (pin the hot trap path's placement).
#[unsafe(link_section = ".text.hot.riscv_trap_dispatch")]
extern "C" fn riscv_trap_dispatch(frame: &mut TrapFrame) {
    // A U-mode trap stays on its thread's kernel stack: the syscall may block, which an interrupt
    // stack may not.
    let from_user = frame.sstatus & SPP == 0;
    let top = crate::interrupt_stack::top_for_trap(from_user);
    let deferred_switch = if top == 0 {
        // The common case calls directly: the trampoline costs every `ecall` instructions.
        riscv_trap_body(frame)
    } else {
        // SAFETY: `top` is this hart's own interrupt-stack top, from the module that owns the
        // region; the trampoline calls `riscv_trap_body` with our own argument and restores `sp`
        // before returning. The frame outlives the call: it is on the stack we are standing on.
        unsafe { dispatch_on_interrupt_stack(frame, top) }
    };

    // Back on the interrupted thread's stack, whichever branch ran. Preemption happens HERE.
    if deferred_switch {
        crate::sched::preempt_if_needed();
    }
}

/// The trap handler proper, run on the interrupt stack when there is one. Fans out on `scause`:
/// syscall, first FP use, S-mode breakpoint, interrupt or fault.
///
/// Returns whether the caller owes a deferred `schedule()`: true for an interrupt, else false.
///
/// Context: interrupt context, with interrupts masked; it may not call `schedule()`.
#[unsafe(no_mangle)]
// In the pinned hot section: milestone 796 (pin the hot trap path's placement).
#[unsafe(link_section = ".text.hot.riscv_trap_body")]
extern "C" fn riscv_trap_body(frame: &mut TrapFrame) -> bool {
    let scause = frame.scause;

    if scause & INTERRUPT != 0 {
        let code = scause & 0xff;
        match code as u32 {
            // Record that a reschedule is due; the switch happens at the deferral (DECISIONS §9).
            super::timer::TIMER_INTID => {
                super::timer::tick();
                crate::sched::on_tick();
            }
            // The claim is the mask until the driver's completion. The source stays enabled: the
            // JH7110's PLIC drops a completion for a disabled source (drivers/plic.rs).
            S_EXTERNAL => {
                // Claim against this hart's own context; affinity may route a source to any hart.
                let ctx = super::irq::this_s_context();
                let source = crate::drivers::plic::claim(ctx);
                if source != 0 {
                    if let Some(ep) = crate::sched::irq_route(source) {
                        ROUTED_IRQS.fetch_add(1, Ordering::Relaxed);
                        super::irq::hold_claim(source, ctx);
                        crate::sched::irq_notify(ep);
                    } else {
                        // Nobody will ACK an unrouted source, so it is completed here.
                        SPURIOUS_IRQS.fetch_add(1, Ordering::Relaxed);
                        crate::drivers::plic::complete(source, ctx);
                    }
                }
            }
            // A reschedule IPI: clear `sip.SSIP` before draining, or it re-fires.
            S_SOFTWARE => {
                clear_software_interrupt();
                // One IPI carries both (DECISIONS §28): a handed thread and a steal request.
                crate::sched::drain_inbox();
                crate::sched::serve_steal_request();
            }
            _ => {
                SPURIOUS_IRQS.fetch_add(1, Ordering::Relaxed);
            }
        }

        // Preemption runs in `riscv_trap_dispatch`, on the interrupted thread's stack, never on
        // the interrupt stack this may be on (DECISIONS §5).
        return true;
    }

    let code = scause & 0xff;
    let from_user = frame.sstatus & SPP == 0;

    match code {
        CAUSE_ECALL_U => {
            // `sepc` points at the 4-byte `ecall`; step over it. Wrapping keeps the overflow check
            // off the syscall path: `sepc` is a user address, far below `2^64 - 4`.
            frame.sepc = frame.sepc.wrapping_add(4);
            #[cfg(any(test, feature = "system_tests"))]
            SVC_COUNT.fetch_add(1, Ordering::Relaxed);
            crate::syscall::dispatch(frame);
        }
        // An illegal instruction under `FS == Off` may be a first FP use. Open the unit and retry
        // without decoding: a genuinely illegal one traps again with `FS` on. Set the frame's FS,
        // which trap.s writes back, to Dirty. See notes/riscv-port/traps.md.
        CAUSE_ILLEGAL_INSTRUCTION
            if frame.sstatus & super::fp::SSTATUS_FS == 0 && crate::fp::enable_for_current() =>
        {
            frame.sstatus = (frame.sstatus & !super::fp::SSTATUS_FS) | super::fp::SSTATUS_FS_DIRTY;
        }

        // S-mode only: a U-mode `ebreak` ends a userspace panic and must kill the thread.
        CAUSE_BREAKPOINT if !from_user => {
            BRK_COUNT.fetch_add(1, Ordering::Relaxed);
            advance_past_trapping_insn(frame);
        }
        _ if from_user => user_fault(frame, scause, code),
        _ => {
            // A kernel exception is fatal. `x[2]` is the interrupted `sp`; the live one may be
            // this hart's interrupt stack.
            crate::stack::warn_if_guard_page(frame.stval, frame.x[2]);
            panic!(
                "unexpected RISC-V trap: scause={scause:#x} (code {code}) stval={:#x} sepc={:#x} \
                 from_user={from_user}",
                frame.stval, frame.sepc,
            );
        }
    }

    false
}

/// Print what the hardware walked for a user page fault at `stval`: the live `satp` and leaf, and
/// in `system_tests` builds whether the root is the thread's own.
///
/// Cold and out of line for `script/fastpath-footprint`. See notes/riscv-port/traps.md and
/// notes/riscv-tlb-shootdown.md, BUGS.
#[cold]
#[inline(never)]
fn report_translation(stval: u64) {
    let (satp, leaf) = super::mmu::installed_satp_and_leaf(stval);
    let live = super::mmu::current_user_root();
    #[cfg(feature = "system_tests")]
    let own = crate::sched::thread_space_root(crate::sched::current());
    #[cfg(not(feature = "system_tests"))]
    let own: Option<u64> = None;
    match own {
        Some(own) if own != live => crate::println!(
            "    satp {satp:#018x} (root {live:#x}, NOT the thread's own root {own:#x}), leaf at \
             stval {leaf:x?}"
        ),
        Some(_) => crate::println!(
            "    satp {satp:#018x} (root {live:#x}, the thread's own), leaf at stval {leaf:x?}"
        ),
        // `None` here is a thread with no space, run on the reserved root, not a shipping build.
        None if cfg!(feature = "system_tests") => crate::println!(
            "    satp {satp:#018x} (root {live:#x}, and the thread's record holds no space), leaf \
             at stval {leaf:x?}"
        ),
        None => crate::println!("    satp {satp:#018x} (root {live:#x}), leaf at stval {leaf:x?}"),
    }
}

/// Kill the user thread whose trap `frame` (cause `scause`, exception `code`) is a fault, and keep
/// the machine.
///
/// Records the fault for [`last_user_fault`], prints the kill report, and hands the thread to
/// `sched::fault`, which delivers it to a supervisor or retires it (DECISIONS §26). Never returns.
fn user_fault(frame: &TrapFrame, scause: u64, code: u64) -> ! {
    // Classify first, while this thread's space is still installed.
    //
    // The record, then the counter: the counter's Release publishes the record.
    //
    // PAIR: the `Acquire` fence in [`last_user_fault`].
    LAST_USER_FAULT_ADDR.store(frame.stval, Ordering::Relaxed);
    LAST_USER_FAULT.store(classify(frame, code).encode(), Ordering::Relaxed);
    USER_FAULTS.fetch_add(1, Ordering::Release);

    crate::println!();
    crate::println!(
        "  user thread {} killed: scause {:#x} (code {})",
        crate::sched::current(),
        scause,
        code,
    );
    crate::println!(
        "    pc {:#018x}   stval {:#018x}   user sp {:#018x}",
        frame.sepc,
        frame.stval,
        frame.x[2],
    );
    if matches!(
        code,
        CAUSE_INSTRUCTION_PAGE_FAULT | CAUSE_LOAD_PAGE_FAULT | CAUSE_STORE_PAGE_FAULT
    ) {
        report_translation(frame.stval);
    }
    crate::println!("  the kernel is fine.");

    crate::sched::fault(frame.sepc, frame.stval);
}

/// Prove the trap path end to end: execute an `ebreak` and return how many breakpoints the handler
/// counted, which is 1 when traps are wired. Without them this never returns.
// The boot self-test is the only caller, and it is not built under `test` or `bench`.
#[cfg_attr(
    any(test, feature = "system_tests", feature = "bench"),
    allow(dead_code)
)]
pub(crate) fn self_test() -> usize {
    let before = BRK_COUNT.load(Ordering::Relaxed);
    super::instructions::ebreak();
    BRK_COUNT.load(Ordering::Relaxed) - before
}

#[cfg(test)]
mod tests {
    //! Tests for trap handling.

    /// The trap vector is installed in direct mode, 4-byte aligned.
    ///
    /// The low two bits of `stvec` are its MODE field, so a 2-byte-aligned entry would select
    /// vectored mode and send every interrupt to a wrong address.
    #[test_case]
    fn stvec_points_at_our_trap_entry() {
        unsafe extern "C" {
            fn trap_entry();
        }
        let expected = trap_entry as *const () as u64;

        let stvec = crate::arch::riscv64::instructions::read_stvec();

        assert_eq!(
            stvec & !0b11,
            expected,
            "stvec does not point at trap_entry"
        );
        assert_eq!(stvec & 0b11, 0, "stvec is not in direct mode: {stvec:#x}");
        assert_eq!(expected % 4, 0, "trap entry misaligned: {expected:#x}");
    }

    /// Take a breakpoint and come back from it, which exercises the whole trap path.
    ///
    /// A count that rose also witnesses S-mode, which RISC-V gives no register to read, and the
    /// 2-byte `c.ebreak` step. See notes/riscv-port/traps.md.
    #[test_case]
    fn breakpoint_is_caught_and_execution_resumes() {
        use core::sync::atomic::Ordering;

        use crate::arch::exceptions::BRK_COUNT;

        let before = BRK_COUNT.load(Ordering::Relaxed);

        crate::arch::riscv64::instructions::ebreak();

        assert_eq!(
            BRK_COUNT.load(Ordering::Relaxed),
            before + 1,
            "the handler didn't run, but we resumed anyway?"
        );
    }

    /// A trap round-trips a register: a wrong offset in trap.s's save and restore would scramble
    /// one and still `sret` to the right address.
    ///
    /// `s2` (`x18`) is callee-saved, so the clobber is declared, and far enough into the frame that
    /// an off-by-one lands on it.
    #[test_case]
    fn registers_survive_a_trap() {
        let sent: u64 = 0xdead_beef_cafe_f00d;
        let got: u64;

        // SAFETY: deliberately traps; we handle it. s2 is callee-saved, so we declare the clobber.
        unsafe {
            core::arch::asm!(
                "mv s2, {sent}",
                "ebreak",
                "mv {got}, s2",
                sent = in(reg) sent,
                got = out(reg) got,
                out("s2") _,
            );
        }

        assert_eq!(got, sent, "the trap frame scrambled a register");
    }
}
