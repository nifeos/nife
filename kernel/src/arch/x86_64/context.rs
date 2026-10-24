//! **The saved register context of a thread, `x86_64`.** The Rust half of `context.s`, and the third
//! implementation of the seam `thread.rs` talks to: it asks for a context "for a kernel thread" or
//! "for a user thread" and never names a register, so which register carries the closure, the entry
//! or the return stays inside `arch/`.
//!
//! Where aarch64 saves `x19`..`x30` and RISC-V saves `ra` plus `s0`..`s11`, the System V AMD64 ABI's
//! callee-saved set is six registers wide: `rbx`, `rbp`, `r12`..`r15`. The return address is not a
//! register at all on x86, it is a stack slot, which is why it is the last field here rather than
//! the first as RISC-V's `ra` is.

/// The callee-saved registers, as `switch_to` pushes them.
///
/// **This layout is a contract with `context.s`**: seven `u64` slots, 56 bytes, in this order. The
/// first six are read back by the `pop` sequence and the seventh is what `ret` jumps to.
///
/// The size must stay 56. A context sits at `stack_top - size_of::<Context>()` below a 16-byte
/// aligned `stack_top`, so 56 leaves `rsp` 16-byte aligned when `ret` lands in a trampoline, as the
/// trampoline's own `call` requires. Padded to 64, every thread's first call would be misaligned,
/// which surfaces much later as a fault in code that uses SSE.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Context {
    /// `r15`. **Also the third payload for a brand-new user thread.**
    r15: u64,
    /// `r14`. **Also the second payload for a brand-new user thread.**
    r14: u64,
    /// `r13`. **Also the first payload for a brand-new user thread.**
    r13: u64,
    /// `r12`.
    r12: u64,
    /// `rbx`. **Doubles as the first payload for a brand-new thread**: the closure pointer for a
    /// kernel thread, the entry address for a user one.
    rbx: u64,
    /// `rbp`, the frame pointer. **Doubles as the second payload**: the call shim, or the user
    /// stack pointer. Zeroed by the trampolines once running, as the bottom of the backtrace.
    rbp: u64,
    /// The return address. **Where `switch_to`'s `ret` jumps.** For a new thread this is a
    /// trampoline, so a thread that has never run starts by the same instruction that resumes one
    /// that has.
    rip: u64,
}

const _: () = assert!(size_of::<Context>() == 56);

impl Context {
    /// The initial context for a brand-new **kernel** thread. `switch_to`'s first `ret` lands in
    /// `thread_trampoline`, which reads the closure pointer out of `rbx` and the monomorphized
    /// caller out of `rbp`, then calls the portable `thread_entry`. See context.s.
    pub(crate) fn for_kernel_thread(closure_at: u64, call_shim: u64) -> Self {
        Context {
            r15: 0,
            r14: 0,
            r13: 0,
            r12: 0,
            rbx: closure_at,
            rbp: call_shim,
            rip: thread_trampoline as *const () as u64,
        }
    }

    /// The initial context for a brand-new **user** thread. `switch_to`'s first `ret` lands in
    /// `user_entry_trampoline`, which moves `rbx` to the entry, `rbp` to the user stack pointer and
    /// `r13`..`r15` to the child's first three arguments, then drops to ring 3 via the portable
    /// `user_thread_entry`.
    pub(crate) fn for_user_thread(entry: u64, user_sp: u64, args: [u64; 3]) -> Self {
        Context {
            r15: args[2],
            r14: args[1],
            r13: args[0],
            r12: 0,
            rbx: entry,
            rbp: user_sp,
            rip: user_entry_trampoline as *const () as u64,
        }
    }
}

unsafe extern "C" {
    /// Save our callee-saved registers, swap `rsp`, restore theirs, and `ret` into **their** saved
    /// return address. See context.s: the last instruction returns to a different thread.
    pub fn switch_to(prev_context: *mut *mut Context, next_context: *mut Context);

    /// The first-run landing pad for a kernel thread. Referenced only by
    /// [`Context::for_kernel_thread`].
    fn thread_trampoline();

    /// The first-run landing pad for a user thread. Referenced only by
    /// [`Context::for_user_thread`].
    fn user_entry_trampoline();
}
