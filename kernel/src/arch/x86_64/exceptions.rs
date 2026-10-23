//! The IDT and the trap path, `x86_64`: the arch contract aarch64 states with `VBAR_EL1` and
//! RISC-V with `stvec`.
//!
//! x86 has 256 entry points and tells the handler nothing, so trap.s manufactures the vector with
//! one stub per vector, and ten of the 32 exceptions push an error code the other 246 vectors do
//! not. The kernel sees a [`TrapFrame`], counters it can assert on, the last user fault, and an
//! `init` that makes faults reportable. See notes/x86-port/traps.md.

use core::cell::SyncUnsafeCell;
use core::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

use super::segments::KERNEL_CODE;
use crate::arch::UserFault;

/// **The registers a trap saves, in exactly the order trap.s pushes them.**
///
/// The first fifteen fields are the general registers, lowest address first, which is the *reverse*
/// of the push order because the stack grows down. Then the vector number and error code the stub
/// manufactured, then the five words the CPU itself pushed.
///
/// `rsp` and `ss` are in the frame unconditionally, which is an `x86_64` change from 32-bit and is
/// load-bearing here: a long-mode interrupt pushes SS:RSP even when the ring does not change, so
/// there is exactly one frame layout rather than two, and `iretq` always pops five words.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TrapFrame {
    pub rax: u64,
    pub rbx: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub rbp: u64,
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    /// Which of the 256 entries was taken. Pushed by the per-vector stub, because the CPU does not
    /// say.
    pub vector: u64,
    /// The hardware error code, or the zero the stub substituted for a vector that has none.
    pub error_code: u64,
    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
    pub rsp: u64,
    pub ss: u64,
}

// trap.s pushes 22 quadwords and `add rsp, 16` at the end assumes the vector and error code are the
// two words directly above the register block. Both facts are checked here rather than trusted.
const _: () = {
    assert!(size_of::<TrapFrame>() == 22 * 8);
    assert!(core::mem::offset_of!(TrapFrame, vector) == 15 * 8);
    assert!(core::mem::offset_of!(TrapFrame, error_code) == 16 * 8);
    assert!(core::mem::offset_of!(TrapFrame, rip) == 17 * 8);
};

impl TrapFrame {
    /// The syscall number the caller passed: `rax`, following Linux.
    ///
    /// The arguments are `rdi`, `rsi`, `rdx`, `r10`, `r8`, `r9`, argument 0 doubling as the
    /// return: the tree's one number register plus six (aarch64 `x8` + `x0`..`x5`, RISC-V `a7` +
    /// `a0`..`a5`). `r10` stands in for `rcx` because `syscall` overwrites `rcx` with the return
    /// address and `r11` with RFLAGS. The ABI is §124 (the x86-64 syscall ABI); see
    /// notes/x86-port/traps.md.
    pub(crate) fn syscall_nr(&self) -> u64 {
        self.rax
    }

    /// Syscall argument register `i`. See [`syscall_nr`](Self::syscall_nr) for the register list
    /// and why `r10` is in it.
    pub fn arg(&self, i: usize) -> u64 {
        match i {
            0 => self.rdi,
            1 => self.rsi,
            2 => self.rdx,
            3 => self.r10,
            4 => self.r8,
            5 => self.r9,
            _ => panic!("x86_64 syscall argument {i} does not exist (there are six)"),
        }
    }

    /// Set syscall argument/return register `i`. The return value and IPC message words ride back
    /// in the first three, exactly as they do on the other two architectures.
    pub fn set_arg(&mut self, i: usize, v: u64) {
        match i {
            0 => self.rdi = v,
            1 => self.rsi = v,
            2 => self.rdx = v,
            3 => self.r10 = v,
            4 => self.r8 = v,
            5 => self.r9 = v,
            _ => panic!("x86_64 syscall argument {i} does not exist (there are six)"),
        }
    }

    /// Build the frame that drops a brand-new thread to ring 3 at `entry` on `user_sp`, with `args`
    /// in the first three argument registers.
    ///
    /// The selectors carry the privilege change: `iretq` returns to the ring in the low two bits
    /// of the CS it pops (the GDT layout in `segments.rs`). `rflags` is `IF | 0x2`: bit 1 always
    /// reads as one, and `IF` keeps a looping user thread preemptible. `IOPL` stays 0, so ring 3
    /// reaches a port only through the TSS's I/O permission bitmap, which a port capability fills
    /// (§121 (x86 port I/O); `segments.rs`).
    #[allow(dead_code)]
    pub fn for_user_entry(entry: u64, user_sp: u64, args: [u64; 3]) -> Self {
        const RFLAGS_IF: u64 = 1 << 9;
        const RFLAGS_ALWAYS_ONE: u64 = 1 << 1;
        TrapFrame {
            rax: 0,
            rbx: 0,
            rcx: 0,
            rdx: args[2],
            rsi: args[1],
            rdi: args[0],
            rbp: 0,
            r8: 0,
            r9: 0,
            r10: 0,
            r11: 0,
            r12: 0,
            r13: 0,
            r14: 0,
            r15: 0,
            vector: 0,
            error_code: 0,
            rip: entry,
            cs: super::segments::USER_CODE as u64,
            rflags: RFLAGS_IF | RFLAGS_ALWAYS_ONE,
            rsp: user_sp,
            ss: super::segments::USER_DATA as u64,
        }
    }
}

/// One IDT entry: a 64-bit interrupt/trap gate.
///
/// The handler address is split across three fields for the same reason the TSS descriptor's is:
/// the layout grew from the 16-bit 286 outward and the new bits were appended rather than the old
/// ones widened.
#[repr(C)]
#[derive(Clone, Copy)]
struct IdtEntry {
    offset_low: u16,
    selector: u16,
    /// Bits 2:0 select an IST slot (0 = do not switch stacks); the rest is reserved.
    ist: u8,
    /// Present, DPL, and gate type. 0x8E is "present, DPL 0, 64-bit interrupt gate".
    type_attr: u8,
    offset_mid: u16,
    offset_high: u32,
    _reserved: u32,
}

impl IdtEntry {
    const fn empty() -> Self {
        Self {
            offset_low: 0,
            selector: 0,
            ist: 0,
            type_attr: 0,
            offset_mid: 0,
            offset_high: 0,
            _reserved: 0,
        }
    }

    /// An interrupt gate at `handler`. **Interrupt gate, not trap gate**, and the difference is one
    /// bit that decides whether the CPU clears `RFLAGS.IF` on entry. An interrupt gate does, so a
    /// handler starts with interrupts masked and cannot be re-entered by the same source before it
    /// has established anything. A trap gate does not, which is what you want for a deliberate
    /// software interrupt and not for anything else.
    fn interrupt_gate(handler: u64, ist: u8) -> Self {
        Self {
            offset_low: handler as u16,
            selector: KERNEL_CODE,
            ist: ist & 0b111,
            type_attr: 0x8E,
            offset_mid: (handler >> 16) as u16,
            offset_high: (handler >> 32) as u32,
            _reserved: 0,
        }
    }
}

const _: () = assert!(size_of::<IdtEntry>() == 16);

/// The IDT. 256 entries, 4 KiB, filled by [`init`] from the stub table trap.s exports.
///
/// A `SyncUnsafeCell`, by calef's ruling of 2026-10-09 on milestone 891 (a mutable static becomes
/// an atomic, a lock or a recorded cell): `lidt` takes its address and the CPU reads gates from it
/// on every trap, so it stays a fixed, statically placed table written in place. Plain access is
/// sound because [`IDT_BUILT`] admits exactly one writer, once, before any core's `lidt` points at
/// it; after that every access is the CPU's read.
static IDT: SyncUnsafeCell<[IdtEntry; 256]> = SyncUnsafeCell::new([IdtEntry::empty(); 256]);

/// The IST entry vector `vector`'s gate selects (0 for none), as this core's CPU reads it. For the
/// tests in `super::ist`.
#[cfg(test)]
pub(super) fn gate_ist(vector: usize) -> u8 {
    // SAFETY: a read of one gate, which `init` wrote once before any core loaded the table.
    (unsafe { (*IDT.get())[vector].ist }) & 0b111
}

unsafe extern "C" {
    /// 256 stub addresses, one per vector, in vector order. Generated by trap.s.
    static ISR_STUBS: [u64; 256];
}

/// How many interrupts became a message to a userspace endpoint: the trap handler took one,
/// `sched::irq_route` found an endpoint bound to it, and `sched::irq_notify` was called. The
/// portable counter every architecture's interrupt tests read.
///
/// The timer, the reschedule IPI and unclaimed device vectors are not counted, so a tick cannot
/// satisfy a test waiting for a device's completion (notes/load-sensitive-assertions.md). Ticks are
/// `arch::timer::ticks()`; a device vector is [`DEVICE_IRQS`], and one with no route
/// [`SPURIOUS_IRQS`].
pub static ROUTED_IRQS: AtomicUsize = AtomicUsize::new(0);

/// **How many interrupts arrived with nothing to route them to.** A spurious interrupt is normal on
/// x86 in a way it is not elsewhere: the 8259 PIC manufactures vector 7 (or 15) when a line drops
/// between assertion and acknowledgement, and the local APIC has a dedicated spurious vector.
pub(crate) static SPURIOUS_IRQS: AtomicUsize = AtomicUsize::new(0);

/// **How many NMIs arrived that the TLB shootdown did not claim.** Vector 2 is this kernel's
/// shootdown message (`mmu::shoot_down_others`) and nothing else on this machine sends one, so this
/// should stay at zero; it is counted rather than assumed away because an NMI from a source we do
/// not know about is exactly the kind of fact that is invisible until someone looks.
pub(crate) static NMIS_UNCLAIMED: AtomicUsize = AtomicUsize::new(0);

/// **How many *device* interrupts arrived through the IO APIC**, as distinct from the local APIC's
/// own timer. Counted separately because the two prove different things: the timer proves the local
/// APIC delivers and the trap path returns, and this proves a line outside the CPU reached it.
pub(crate) static DEVICE_IRQS: AtomicUsize = AtomicUsize::new(0);

/// **System calls served, counted for the system tests only** (`system_tests/src/user/el0_boundary_tests.rs`
/// proves a program reached user mode and came back by watching it rise). Every syscall on every
/// core bumps this one line, so it is compiled out of every other build: until 2026-10-04 it was
/// in all of them, and it was one of two shared writes on the cheapest syscall's path
/// (notes/job-mix/null-syscall-under-load.md).
#[cfg(any(test, feature = "system_tests"))]
pub static SVC_COUNT: AtomicUsize = AtomicUsize::new(0);

/// **How many user-mode faults were taken.**
pub static USER_FAULTS: AtomicUsize = AtomicUsize::new(0);

/// **How many breakpoints were taken.** `int3`, vector 3, which is what [`self_test`] uses to prove
/// the trap path round-trips.
pub(crate) static BRK_COUNT: AtomicUsize = AtomicUsize::new(0);

/// The last user fault, packed by [`UserFault::encode`]. 0 means no user thread has faulted.
static LAST_USER_FAULT: AtomicU64 = AtomicU64::new(0);
/// The faulting address of the fault [`LAST_USER_FAULT`] describes.
static LAST_USER_FAULT_ADDRESS: AtomicU64 = AtomicU64::new(0);

/// What the last user-mode fault was and where. `None` until one has happened.
///
/// # BUGS
///
/// One slot for the whole machine, as on the other two ISAs. A fault in an unrelated thread
/// overwrites it before a test reads it, and a user exception that is not a page fault records
/// address 0. A test that asserts on it must retry or tie the record to its own thread. Found by
/// the eighth outsider pass (`notes/confinement-outsider-pass-8.md`), whose row-21 test retries.
pub fn last_user_fault() -> Option<(UserFault, u64)> {
    // Pairs with the `Release` on `USER_FAULTS` in [`user_fault`]: a caller that has already seen
    // the counter rise must see the record that rise was announcing, and not the previous one.
    let code = LAST_USER_FAULT.load(Ordering::Acquire);
    UserFault::decode(code).map(|f| (f, LAST_USER_FAULT_ADDRESS.load(Ordering::Relaxed)))
}

/// **Diagnostic: the ring-3 PC of a thread, from the [`TrapFrame`] at the top of its kernel
/// stack.** The twin of the other two architectures' `user_pc`, and the watchdog dump's
/// user-PC column.
///
/// The frame lives at `stack_top - size_of::<TrapFrame>()` on all three architectures, which is
/// what makes this readable at all; see `user::enter_frame` for why that placement is a rule rather
/// than a convenience. Meaningless for a pure kernel thread, which never builds one.
pub(crate) fn user_pc(stack_top: u64) -> u64 {
    let frame = (stack_top - size_of::<TrapFrame>() as u64) as *const TrapFrame;
    // SAFETY: a diagnostic read of the frame the entry path writes at the stack top. Volatile
    // because the owning thread may be running on another CPU while we read, and this must not be
    // hoisted.
    unsafe { core::ptr::read_volatile(&raw const (*frame).rip) }
}

/// Write argument register `i` of a thread's saved user frame, the one [`user_pc`] reads. The
/// return to user mode restores it, so this hands a trapped user thread a word its syscall path
/// does not write; the death message's label rides in register 5 (§148 (resolves by asking the
/// kernel), as amended 2026-10-04).
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

unsafe extern "C" {
    /// Load `frame` as the register state and `iretq` into it: the first entry to ring 3. Defined
    /// in trap.s, sharing the restore path with every trap return. Its first instructions are `cli`
    /// and `mov rsp, rdi`, so it does not touch the caller's stack.
    fn user_return(frame: *mut TrapFrame) -> !;
}

/// Drop to ring 3 by loading `frame` and `iretq`ing into it; the counterpart of aarch64's and
/// RISC-V's `enter_user`. The frame's CS names the ring (see [`TrapFrame::for_user_entry`]).
///
/// `#[inline(always)]` is load-bearing: the frame sits at the top of the caller's kernel stack, and
/// a call frame pushed here could land on it. Inlined, the caller tail-jumps to `user_return`.
///
/// # Safety
/// `frame` must be a correctly-built, writable [`TrapFrame`] at the top of the current thread's
/// kernel stack, with the user address space installed and `TSS.RSP0` naming a kernel stack the
/// next trap can be pushed onto (`segments::set_kernel_stack`).
#[inline(always)]
pub unsafe fn enter_user(frame: *mut TrapFrame) -> ! {
    // A thread entered with `rip == 0` faults at address 0 and dies, a hang far from its cause, so
    // panic instead. A load and a branch push nothing over the frame at the top of this stack.
    // SAFETY: the caller's contract says `frame` is a valid, writable `TrapFrame`.
    let rip = unsafe { (*frame).rip };
    if rip == 0 {
        // SAFETY: as above; read before the cold call below is allowed to use this stack.
        let user_sp = unsafe { (*frame).rsp };
        entered_user_with_no_entry_point(user_sp);
    }

    // SAFETY: the caller's contract; `user_return` never returns.
    unsafe { user_return(frame) }
}

/// The `rip == 0` case, out of line so [`enter_user`] stays a tail jump.
///
/// Its argument is read from the frame **before** it is called, because the frame lives at the top
/// of this very stack and this call is entitled to overwrite it.
#[cold]
#[inline(never)]
fn entered_user_with_no_entry_point(user_sp: u64) -> ! {
    panic!(
        "a thread on cpu {} was dispatched to ring 3 with rip = 0 (user rsp {user_sp:#018x}). \
         Its context was never built, or was built and not seen by this cpu.",
        crate::cpu::id(),
    )
}

/// Unmask external interrupts at the controller, which on x86 is nothing: its one gate is
/// `RFLAGS.IF` (`arch::interrupts::enable`), where RISC-V also has `sie.SEIE`. The per-source masks
/// are the IO APIC's (`irq::enable`) and the TPR, which `irq::init_local_apic` zeroes.
///
/// # Panics
/// If the local APIC is not up, so no caller believes interrupts unmasked when nothing can deliver.
pub(crate) fn enable_external() {
    assert!(
        super::irq::is_local_apic_ready(),
        "external interrupts are the local APIC's to deliver, and it is not up yet",
    );
}

/// Whether [`IDT`] has been built. The first core to swap it to true builds the table, once.
static IDT_BUILT: AtomicBool = AtomicBool::new(false);

/// Install the IDT on this CPU.
///
/// Every vector gets a present gate, so an unexpected vector reports itself instead of raising a
/// general protection fault. The NMI, the double fault and the machine check switch to per-core
/// IST stacks (`super::ist`); every other vector pushes on the stack it found.
///
/// The table is built once, by the first core here (the boot core), and every core `lidt`s its
/// own IDTR. A secondary rebuilding it would race the boot core's live interrupt delivery, which
/// stays enabled across `bring_up_secondaries`. See notes/x86-port/traps.md.
pub fn init() {
    if !IDT_BUILT.swap(true, Ordering::AcqRel) {
        // SAFETY: guarded by the swap above, so at most one core ever runs this loop, and it runs
        // to completion before any OTHER core's `lidt` (below) can possibly point at this table:
        // that core would have to have gotten here first, which the swap already ruled out. The
        // stub table is 256 entries and so is the IDT, so the two zip one to one. The `&mut` to
        // the table lives only for this loop, and nothing else can reach the table until it ends.
        unsafe {
            let idt = &mut *IDT.get();
            for (vector, (gate, &stub)) in idt.iter_mut().zip(ISR_STUBS.iter()).enumerate() {
                *gate = IdtEntry::interrupt_gate(stub, super::ist::slot_for(vector));
            }
        }
    }

    let idtr = super::segments::DescriptorTablePointer {
        limit: (size_of::<[IdtEntry; 256]>() - 1) as u16,
        base: IDT.get() as u64,
    };
    // SAFETY: `idtr` describes the table, which the branch above guarantees is fully built by the
    // time any core reaches this `lidt`, whose every entry is a well-formed gate pointing at a stub
    // in this image. `lidt` writes only this core's own IDTR.
    unsafe {
        core::arch::asm!("lidt [{}]", in(reg) &idtr, options(readonly, nostack, preserves_flags));
    }
}

/// Prove the trap path round-trips by taking a breakpoint and returning from it.
///
/// Returns how many breakpoints were caught during this call, not in total, as the RISC-V and
/// aarch64 twins do; the tour and `self_test::run` each call it. An IDT whose return path is wrong
/// runs until the first real fault and then vanishes, and this is the cheap moment to find out.
pub(crate) fn self_test() -> usize {
    let before = BRK_COUNT.load(Ordering::Relaxed);
    // SAFETY: `int3` raises vector 3, which the IDT above routes to the common handler. The handler
    // treats a breakpoint as an event to count and step over, so this returns to the next
    // instruction.
    unsafe { core::arch::asm!("int3", options(nomem, nostack)) };
    BRK_COUNT.load(Ordering::Relaxed) - before
}

// ---------------------------------------------------------------------------------------------
// The `syscall` instruction pair (milestone 161, roadmap item 3): the second way into the kernel,
// sharing nothing with the IDT path above (no gate, no descriptor, no stack switch, no pushes).
// Four MSRs are its whole configuration; the constraints on each are on the items below.
// ---------------------------------------------------------------------------------------------

/// `IA32_EFER`. Bit 0, `SCE`, is what makes `syscall` a legal instruction rather than `#UD`. The
/// same register carries `LME` and `NXE`, which `boot.s` set before any table was built.
const IA32_EFER: u32 = 0xC000_0080;
/// `IA32_STAR`: the selectors. `[47:32]` is what `syscall` loads into CS (and, plus 8, into SS);
/// `[63:48]` is the base `sysret` derives the user pair from.
const IA32_STAR: u32 = 0xC000_0081;
/// `IA32_LSTAR`: where a 64-bit `syscall` jumps. There is no default and no descriptor: this is a
/// raw address, so a zero here is a jump to zero.
const IA32_LSTAR: u32 = 0xC000_0082;
/// `IA32_FMASK`: the `RFLAGS` bits a `syscall` clears on the way in.
const IA32_FMASK: u32 = 0xC000_0084;

/// `IA32_EFER.SCE`.
const EFER_SCE: u64 = 1 << 0;

/// The selector base `sysret` derives the user pair from, and `syscall`'s kernel pair sits at
/// [`KERNEL_CODE`].
///
/// **The GDT's order is this arithmetic, not a style choice** (segments.rs says so at its top). The
/// three assertions below are the mechanism that keeps the two files agreeing: change a selector in
/// segments.rs without changing this and the build stops, rather than `sysret` landing a user
/// program on the kernel's data segment.
const SYSRET_SELECTOR_BASE: u16 = 0x10;

const _: () = {
    assert!(
        SYSRET_SELECTOR_BASE + 8 == super::segments::USER_DATA & !3,
        "sysret computes the user SS as IA32_STAR[63:48] + 8"
    );
    assert!(
        SYSRET_SELECTOR_BASE + 16 == super::segments::USER_CODE & !3,
        "sysret computes the user CS as IA32_STAR[63:48] + 16"
    );
    assert!(
        KERNEL_CODE + 8 == super::segments::KERNEL_DATA,
        "syscall computes the kernel SS as IA32_STAR[47:32] + 8"
    );
};

/// The `RFLAGS` bits cleared on every `syscall`, each a hazard.
///
/// `IF` (9) masks interrupts on entry, as an interrupt gate does, so the handler cannot be
/// re-entered before it has a stack. `TF` (8) would single-step the kernel for a user debugger.
/// `DF` (10) must be clear on entry to a System V function. `IOPL` (13:12) inherited from ring 3 is
/// not the kernel's choice. `NT` (14) changes what `iret` means. `RF` (16) suppresses instruction
/// breakpoints. `AC` (18) faults unaligned kernel accesses, a known privilege-escalation lever.
const SYSCALL_FLAG_MASK: u64 = (1 << 8)      // TF
    | (1 << 9)                               // IF
    | (1 << 10)                              // DF
    | (0b11 << 12)                           // IOPL
    | (1 << 14)                              // NT
    | (1 << 16)                              // RF
    | (1 << 18); // AC

/// The pseudo-vector a `syscall` frame carries, so a fault report or a dump can say where the frame
/// came from. **256 is out of the IDT's range by construction**, which is what keeps it from
/// colliding with a real vector; the assertion says so rather than leaving it to be noticed.
pub(crate) const SYSCALL_VECTOR: u64 = 0x100;
const _: () = assert!(
    SYSCALL_VECTOR > 255,
    "a real IDT vector would be ambiguous here"
);

/// Program the four MSRs that make `syscall` work, once per CPU, at boot. The `syscall` path's
/// kernel stack is not among them: `isr_restore` in trap.s sets it with `TSS.RSP0`.
///
/// `SCE` is enabled last, so the instruction is legal only once its target and flag mask are in
/// place; otherwise a `syscall` could jump to whatever `IA32_LSTAR` held, zero on a cold machine.
///
/// # Safety
/// Must be called with the GDT installed (the selectors this writes index it) and before anything
/// enters ring 3.
pub(crate) unsafe fn init_syscall() {
    let star = ((SYSRET_SELECTOR_BASE as u64) << 48) | ((KERNEL_CODE as u64) << 32);
    // SAFETY: these are the architectural `syscall` MSRs, present on every long-mode CPU (they are
    // part of the mode itself, not an optional feature). The values are checked against the GDT by
    // the const assertions above.
    unsafe {
        super::write_msr(IA32_STAR, star);
        super::write_msr(IA32_LSTAR, x86_syscall_entry as *const () as u64);
        super::write_msr(IA32_FMASK, SYSCALL_FLAG_MASK);
        let efer = super::read_msr(IA32_EFER);
        super::write_msr(IA32_EFER, efer | EFER_SCE);
    }
}

unsafe extern "C" {
    /// The `IA32_LSTAR` target: where a ring-3 `syscall` lands. Defined in trap.s.
    fn x86_syscall_entry();
}

/// The Rust function every `syscall` reaches, with the [`TrapFrame`] `x86_syscall_entry` built;
/// the twin of [`x86_trap_dispatch`]. The dispatcher reads it only through `TrapFrame`'s accessors.
///
/// # BUGS
/// The return is `iretq`, not `sysretq`, which faults in ring 0 on the user's stack for a
/// non-canonical `rcx` (CVE-2012-0217). See notes/x86-port/traps.md.
///
/// # Safety
/// Called only from `x86_syscall_entry`, which has just constructed a complete [`TrapFrame`] at the
/// address it passes. Nothing else may call it.
#[unsafe(no_mangle)]
// In the pinned hot section: milestone 796 (pin the hot trap path's placement).
#[unsafe(link_section = ".text.hot.x86_syscall_handler")]
pub(crate) unsafe extern "C" fn x86_syscall_handler(frame: *mut TrapFrame) {
    // SAFETY: `x86_syscall_entry` built the frame directly below the pointer it passed, and no
    // other caller exists (the symbol is only referenced from trap.s).
    let frame = unsafe { &mut *frame };
    #[cfg(any(test, feature = "system_tests"))]
    SVC_COUNT.fetch_add(1, Ordering::Relaxed);
    crate::syscall::dispatch(frame);
}

/// Read `CR2`, which holds the faulting address after a page fault (vector 14) and nothing
/// meaningful otherwise. The x86 analog of `FAR_EL1` and of RISC-V's `stval`.
fn faulting_address() -> u64 {
    super::instructions::read_cr2()
}

/// The names of the 32 architecturally defined exceptions, so a fault report says what happened
/// rather than only which number it was.
const EXCEPTION_NAMES: [&str; 32] = [
    "divide error",
    "debug",
    "non-maskable interrupt",
    "breakpoint",
    "overflow",
    "bound range exceeded",
    "invalid opcode",
    "device not available",
    "double fault",
    "coprocessor segment overrun",
    "invalid TSS",
    "segment not present",
    "stack-segment fault",
    "general protection fault",
    "page fault",
    "reserved (15)",
    "x87 floating-point exception",
    "alignment check",
    "machine check",
    "SIMD floating-point exception",
    "virtualization exception",
    "control protection exception",
    "reserved (22)",
    "reserved (23)",
    "reserved (24)",
    "reserved (25)",
    "reserved (26)",
    "reserved (27)",
    "hypervisor injection exception",
    "VMM communication exception",
    "security exception",
    "reserved (31)",
];

unsafe extern "C" {
    /// Switch to `top` (or stay put if it is 0), call [`x86_trap_body`], come back. Defined in
    /// trap.s, because moving `rsp` is assembly and policy is not.
    fn dispatch_on_interrupt_stack(frame: *mut TrapFrame, top: u64) -> bool;
}

/// The outer half of the trap path, which stays on the interrupted stack: it picks the stack the
/// handler runs on, then runs the deferred `schedule()`, as on the other two architectures.
///
/// `schedule()` parks this thread's `rsp` in its `Context`, so it runs only on a stack the thread
/// owns, never the per-CPU interrupt stack. See `kernel/src/interrupt_stack.rs`.
///
/// # Safety
/// Called only from `isr_common`, which has just constructed a complete [`TrapFrame`] at the
/// address it passes. Nothing else may call it.
#[unsafe(no_mangle)]
pub(crate) unsafe extern "C" fn x86_trap_dispatch(frame: *mut TrapFrame) {
    // Vectors 2, 8 and 18 never arrive here: their stubs enter through `isr_paranoid` and
    // [`x86_paranoid_dispatch`], on their own IST stacks.
    // SAFETY: `isr_common` built the frame directly below the pointer it passed.
    let from_user = unsafe { (*frame).cs } & 3 == 3;
    let top = crate::interrupt_stack::top_for_trap(from_user);
    let deferred_switch = if top == 0 {
        // No trampoline here: the other ports measured 8.6 instructions per syscall for it.
        // SAFETY: the caller's contract; forwarded unchanged.
        unsafe { x86_trap_body(frame) }
    } else {
        // SAFETY: `top` is this CPU's own interrupt-stack top, from the module that owns the
        // region; the trampoline calls `x86_trap_body` with our own argument and restores `rsp`
        // before returning. The frame outlives the call: it is on the stack we are standing on.
        unsafe { dispatch_on_interrupt_stack(frame, top) }
    };

    // Back on the interrupted thread's stack, whichever branch ran. Preemption happens HERE.
    if deferred_switch {
        crate::sched::preempt_if_needed();
    }
}

/// **The trap handler proper**: everything that runs on the interrupt stack when there is one.
///
/// Returns whether the caller owes a deferred `schedule()`, which is true for an interrupt and
/// false for everything else.
///
/// # Safety
/// Called only from [`x86_trap_dispatch`] or from `dispatch_on_interrupt_stack`, both of which pass
/// a complete [`TrapFrame`]. Nothing else may call it.
#[unsafe(no_mangle)]
pub(crate) unsafe extern "C" fn x86_trap_body(frame: *mut TrapFrame) -> bool {
    // SAFETY: `isr_common` built the frame directly below the pointer it passed, and no other
    // caller exists (the symbol is only referenced from trap.s and from the function above).
    let frame = unsafe { &mut *frame };

    match frame.vector {
        // A breakpoint is an event to count and step over: `int3` is one byte and the CPU already
        // advanced rip past it, so returning is enough. This is what `self_test` proves.
        3 => {
            BRK_COUNT.fetch_add(1, Ordering::Relaxed);
            false
        }
        // `#NM`: `CR0.TS` traps a thread's first FP or SSE instruction (milestone 447 (a thread's
        // vector registers are its own)). It is a fault, so the `iret` re-executes the instruction
        // with the unit open; `crate::fp` has why this is not LazyFP. Ring 0 takes it too. With no
        // thread to record against, fall through rather than retake the fault forever.
        7 if crate::fp::enable_for_current() => false,

        // The local APIC timer. Record and defer (§9 (locking)): `on_tick` marks a reschedule due,
        // and the switch happens in `x86_trap_dispatch` on the thread's own stack, after the EOI.
        v if v == super::irq::TIMER_VECTOR as u64 => {
            super::timer::tick();
            crate::sched::on_tick();
            super::irq::end_of_interrupt();
            true
        }
        // A reschedule IPI (`irq::send_reschedule`): the sender may have put a thread in our inbox,
        // or an idle CPU may want work. Both are served, and the deferred `schedule()` runs
        // whatever arrived. Not a routed interrupt, and not spurious.
        v if v == super::irq::RESCHEDULE_VECTOR as u64 => {
            crate::sched::drain_inbox();
            crate::sched::serve_steal_request();
            super::irq::end_of_interrupt();
            true
        }
        // A self-directed interrupt, which becomes a message; the local APIC delivers it through
        // the ICR, IRR, ISR and EOI. Its intid is its vector, where a device line's is a legacy IRQ
        // (0..15), so the two cannot collide. See notes/x86-port/traps.md.
        v if v == super::irq::SELF_TEST_VECTOR as u64
            || v == super::irq::SELF_TEST_VECTOR_B as u64 =>
        {
            match crate::sched::irq_route(v as u32) {
                Some(ep) => {
                    ROUTED_IRQS.fetch_add(1, Ordering::Relaxed);
                    // Nothing to mask: a self-IPI is one-shot, with no asserted line to hold off.
                    crate::sched::irq_notify(ep);
                }
                None => {
                    SPURIOUS_IRQS.fetch_add(1, Ordering::Relaxed);
                }
            }
            super::irq::end_of_interrupt();
            true
        }
        // A PCI function's MSI-X message, which becomes a driver's wakeup (milestone 215 (a PCI
        // function's interrupt reaches nothing on x86_64)). An MSI vector is its intid
        // (`irq::MSI_VECTOR_BASE`). Nothing is masked: the message is a posted write that already
        // happened, so the driver's `Irq::ACK` is a no-op.
        v if super::irq::is_msi_vector(v) => {
            match crate::sched::irq_route(v as u32) {
                Some(ep) => {
                    DEVICE_IRQS.fetch_add(1, Ordering::Relaxed);
                    ROUTED_IRQS.fetch_add(1, Ordering::Relaxed);
                    crate::sched::irq_notify(ep);
                }
                None => {
                    SPURIOUS_IRQS.fetch_add(1, Ordering::Relaxed);
                }
            }
            super::irq::end_of_interrupt();
            true
        }
        // A device line on `irq::GSI_VECTOR_BASE` plus its redirection index, which becomes a
        // driver's message (milestone 505 (an x86_64 input driver that never lets the core idle)).
        // `irq::intid_of_vector` recovers the intid a driver bound. `irq::disable` holds off only a
        // level-triggered line: an IO APIC drops an edge that arrives while its entry is masked. A
        // line nothing bound is counted and acknowledged.
        v if super::irq::is_device_vector(v) => {
            DEVICE_IRQS.fetch_add(1, Ordering::Relaxed);
            let routed = super::irq::intid_of_vector(v)
                .and_then(|intid| crate::sched::irq_route(intid).map(|ep| (intid, ep)));
            if let Some((intid, ep)) = routed {
                ROUTED_IRQS.fetch_add(1, Ordering::Relaxed);
                super::irq::disable(intid);
                crate::sched::irq_notify(ep);
            }
            super::irq::end_of_interrupt();
            true
        }
        // A spurious interrupt: the local APIC had to deliver something and had nothing real. It
        // is **not** acknowledged, and that is architecture rather than an oversight: the APIC does
        // not set an in-service bit for a spurious interrupt, so an EOI here would acknowledge
        // whatever genuinely is in service and lose it.
        v if v == super::irq::SPURIOUS_VECTOR as u64 => {
            SPURIOUS_IRQS.fetch_add(1, Ordering::Relaxed);
            false
        }
        // Any other vector at or above 32 is a device interrupt nothing has claimed. Counted and
        // acknowledged rather than fatal: an unowned interrupt that is not acknowledged wedges the
        // local APIC, which turns a stray line into a dead machine.
        v if v >= 32 => {
            SPURIOUS_IRQS.fetch_add(1, Ordering::Relaxed);
            super::irq::end_of_interrupt();
            true
        }
        // An exception taken in ring 3 is the program's fault, recorded rather than fatal. Only
        // vectors below 32 reach here: an interrupt during user code was handled above.
        vector if frame.cs & 3 == 3 => user_fault(frame, vector),
        // Everything below 32 is an architectural exception, and every one of them is fatal here.
        vector => fatal_exception(frame, vector),
    }
}

/// Print what a fatal architectural exception left in its frame, and panic.
fn fatal_exception(frame: &TrapFrame, vector: u64) -> ! {
    let name = EXCEPTION_NAMES
        .get(vector as usize)
        .copied()
        .unwrap_or("reserved");
    crate::println!();
    crate::println!("=== x86_64 trap: vector {vector} ({name}) ===");
    crate::println!("  error code : {:#018x}", frame.error_code);
    crate::println!(
        "  rip        : {:#018x}   cs : {:#06x}",
        frame.rip,
        frame.cs
    );
    crate::println!(
        "  rsp        : {:#018x}   ss : {:#06x}",
        frame.rsp,
        frame.ss
    );
    crate::println!("  rflags     : {:#018x}", frame.rflags);
    if vector == 14 {
        // Only meaningful for a page fault, so only printed for one: CR2 is stale
        // otherwise, and a stale address printed as if it were current is worse than no
        // address at all.
        crate::println!("  cr2        : {:#018x}", faulting_address());
    }
    panic!("unhandled x86_64 exception: vector {vector} ({name})");
}

/// The NMI, double fault and machine check land here on their own IST stacks, from `isr_paranoid`
/// with the kernel's GS base loaded (trap.s has how, `super::ist` why these three).
///
/// None may reach the deferred `schedule()`: an NMI often lands in a critical section, the other
/// two never return, and a switch from an IST stack parks a per-core address in a thread. A machine
/// check is fatal from either ring, being the hardware's failure; recovery is not built.
///
/// # Safety
/// Called only from `isr_paranoid`, which has just built a complete [`TrapFrame`] at the address it
/// passes, on an IST stack. Nothing else may call it.
#[unsafe(no_mangle)]
pub(crate) unsafe extern "C" fn x86_paranoid_dispatch(frame: *mut TrapFrame) {
    // SAFETY: `isr_paranoid` built the frame directly below the pointer it passed, and no other
    // caller exists (the symbol is only referenced from trap.s).
    let frame = unsafe { &mut *frame };
    if frame.vector != 2 {
        fatal_exception(frame, frame.vector);
    }

    // Only this handler's own `iretq` may unblock NMIs (`super::ist`, "Nesting"). A nested NMI's
    // frame overwrites the first's and records an `rsp` inside this stack, and returning would
    // resume garbage. The check reads the stack address, not `cpu::id()`, to hold with a bad GS.
    if super::ist::is_on_an_nmi_stack(frame.rsp) {
        panic!(
            "nested NMI: interrupted rsp {:#x} is on an NMI stack, so an exception inside the NMI \
             handler returned and its iretq unblocked a second NMI onto the same IST stack",
            frame.rsp
        );
    }

    let claimed = super::mmu::serve_shootdown_nmi();
    #[cfg(test)]
    let claimed = claimed | super::ist::serve_test_nmi();
    if !claimed {
        NMIS_UNCLAIMED.fetch_add(1, Ordering::Relaxed);
    }
}

/// A ring-3 thread faulted: record what happened, say so, and kill it, so a driver bug is a crashed
/// process and not a dead machine (§10 (process model)).
///
/// Never returns: `sched::fault` delivers the death to a supervisor or takes the unsupervised path,
/// and switches away for good, so `isr_restore` never runs. The next thread (the reaper) frees the
/// kernel stack this runs on.
fn user_fault(frame: &TrapFrame, vector: u64) -> ! {
    let (fault, address) = classify_user_fault(vector, frame.error_code);

    // The record first and the counter last, its store the release: a test watches `USER_FAULTS`
    // rise and then calls [`last_user_fault`], so the counter publishes the record.
    LAST_USER_FAULT_ADDRESS.store(address, Ordering::Relaxed);
    LAST_USER_FAULT.store(fault.encode(), Ordering::Relaxed);
    USER_FAULTS.fetch_add(1, Ordering::Release);

    let name = EXCEPTION_NAMES
        .get(vector as usize)
        .copied()
        .unwrap_or("reserved");
    crate::println!();
    crate::println!(
        "  user thread {} killed: vector {vector} ({name})",
        crate::sched::current(),
    );
    crate::println!(
        "    rip {:#018x}   addr {:#018x}   user rsp {:#018x}   err {:#010x}",
        frame.rip,
        address,
        frame.rsp,
        frame.error_code,
    );
    if vector == 14 {
        report_walk(address);
    }
    crate::println!("  the kernel is fine.");

    // Deliver the fault to a supervisor if this thread had one, and become a corpse; otherwise this
    // is `exit`'s unsupervised path (Finished, reaped by the next thread). See DECISIONS §26 and
    // sched::depart. `frame.rip` is the faulting pc, `address` the faulting address.
    crate::sched::fault(frame.rip, address);
}

/// Print what the hardware walked for a user page fault, level by level, beside the thread's own
/// root: whether the live `CR3` was another space's, or where the thread's tables lost the entry.
/// Each table says whether its frame is allocated now, so one freed while linked shows. The own
/// root is a `system_tests` build's only (`sched::thread_space_root`). Cold, so the trap path pays
/// nothing until a thread dies. See "A user thread can lose the translation of its own text" in
/// `notes/riscv-tlb-shootdown.md`.
#[cold]
#[inline(never)]
fn report_walk(address: u64) {
    let live = super::mmu::current_user_root();
    #[cfg(feature = "system_tests")]
    let own = crate::sched::thread_space_root(crate::sched::current());
    #[cfg(not(feature = "system_tests"))]
    let own: Option<u64> = None;
    match own {
        Some(own) if own != live => {
            crate::println!("    cr3 root {live:#x}, NOT the thread's own root {own:#x}");
        }
        Some(_) => crate::println!("    cr3 root {live:#x}, the thread's own"),
        // `thread_space_root` answers `None` for a thread whose record holds no space, which the
        // switch-in then runs on the reserved root; say so, or it reads like a shipping build.
        None if cfg!(feature = "system_tests") => {
            crate::println!("    cr3 root {live:#x}, and the thread's record holds no space");
        }
        None => crate::println!("    cr3 root {live:#x}"),
    }
    let walk = super::mmu::walk_user(live, address);
    for (level, step) in walk.steps().iter().enumerate() {
        let frame = match crate::memory::frame_is_allocated(step.table) {
            Some(true) => "allocated",
            Some(false) => "FREE",
            None => "unmanaged",
        };
        crate::println!(
            "      level {level}: table {:#x} ({frame}) [{}] = {:#018x}",
            step.table,
            step.index,
            step.entry,
        );
    }
    crate::println!("      walk ends: {:?}", walk.end());
}

/// Turn an x86 exception into the portable [`UserFault`].
///
/// A page fault's error code states the class (notes/x86-port/traps.md has its bits). Bit 0 (`P`)
/// set means the page was present and the hardware refused it, [`UserFault::Permission`] rather
/// than [`UserFault::Translation`]; bit 1 is a write and bit 4 an instruction fetch. Any other
/// exception is [`UserFault::Other`], as the other two report an illegal instruction.
fn classify_user_fault(vector: u64, error_code: u64) -> (UserFault, u64) {
    use crate::arch::UserFaultAccess;

    if vector != 14 {
        return (UserFault::Other, 0);
    }
    let access = if error_code & (1 << 4) != 0 {
        UserFaultAccess::Fetch
    } else if error_code & (1 << 1) != 0 {
        UserFaultAccess::Write
    } else {
        UserFaultAccess::Read
    };
    let fault = if error_code & 1 != 0 {
        UserFault::Permission(access)
    } else {
        UserFault::Translation(access)
    };
    (fault, faulting_address())
}
