//! **A process's own lifetime**: yielding, exiting, trapping on a failed check, and the panic
//! handler every program declares with one macro.
//!
//! The crate root re-exports every public item, so each keeps its crate-root path.
//!
//! Name: provisional, lane maintainer/files-over-1500 (#2017), 2026-10-12 (UTC), for the module and
//! the file. Named for the calls it holds, one module per seam of the old crate root; the pull
//! request records no other name considered.

use crate::syscall::trap6;

/// Give up the CPU (`SYS_YIELD`). Returns when the scheduler runs this thread again; if another
/// thread is ready, control goes there and back, which is one context-switch round trip.
///
/// Not `nomem`: while this thread is away others write memory it shares with them, which a caller
/// spinning on a shared word with `yield_now` relies on. Through [`trap6`], because the kernel
/// writes its result into `x0`/`a0`/`rdi` on the way out of a yield too.
pub fn yield_now() {
    // SAFETY: SYS_YIELD gives up the CPU and comes back; it reads no argument and touches no
    // memory of this thread's own.
    let _ = unsafe { trap6(abi::SYS_YIELD, [0; 6]) };
}

/// Terminate this process. The kernel reaps the thread and frees its whole address space. Never
/// returns; the trailing spin is only there to satisfy the `-> !` type if the trap ever came back.
pub fn exit() -> ! {
    // SAFETY: the syscall never returns; the trailing spin only satisfies the `-> !` type.
    let _ = unsafe { trap6(abi::SYS_EXIT, [0; 6]) };
    loop {
        core::hint::spin_loop();
    }
}

/// **Die where the mistake was.** Raise a breakpoint the kernel turns into a fault, so the process
/// is killed rather than allowed to limp on. Unlike [`exit`] (`EVENT_EXIT`), this reports
/// `EVENT_FAULT` to a supervisor (§26 (the fault endpoint)), so a panic must take this path:
/// `brk #0`, `ebreak` or `ud2`. The trailing spin only satisfies `-> !`; calling `exit` would make
/// an impossible return look like a clean death.
///
/// # BUGS
/// - Every trap in a program prints the same fault line, since its `pc` is this one instruction;
///   notes/user-mode-runtime.md has the fix and the workaround.
///
/// Name: a verb, which is right for a function here: `send`, `receive`, `reap` and `exit` are all verbs, and
/// the naming tenet's noun rule is about crates, programs and modules rather than about the things
/// they do.
pub fn trap() -> ! {
    #[cfg(target_arch = "aarch64")]
    // SAFETY: `brk` traps; the kernel turns a trap from userspace into a kill. The options promise
    // it touches neither memory nor the stack.
    unsafe {
        core::arch::asm!("brk #0", options(nostack, nomem));
    };
    #[cfg(target_arch = "riscv64")]
    // SAFETY: `ebreak` traps; the kernel turns a trap from userspace into a kill. The options
    // promise it touches neither memory nor the stack.
    unsafe {
        core::arch::asm!("ebreak", options(nostack, nomem));
    };
    #[cfg(target_arch = "x86_64")]
    // SAFETY: `ud2` faults; the kernel turns a fault from ring 3 into a kill. The options promise
    // it touches neither memory nor the stack.
    //
    // **`ud2` rather than `int3`, and this was measured rather than chosen** (milestone 161).
    // `int3` is the obvious transliteration of `brk`/`ebreak` and it does not work from ring 3
    // here: a *software* interrupt is refused unless the IDT gate's DPL admits the caller's
    // privilege level, and this kernel's gates are all DPL 0, so `int3` from a process raises
    // **#GP with error code 0x1a** (`(3 << 3) | 2`, the vector it was refused, tagged as an IDT
    // selector) instead of #BP. The process does die, so the first version of this looked like it
    // worked; what it reported was a general protection fault at address zero, which names neither
    // the instruction nor the reason and would have sent the next reader hunting a segmentation
    // bug.
    //
    // Opening vector 3 to ring 3 is the other fix and Linux takes it, because Linux has ptrace and
    // wants a debugger to be able to plant breakpoints. There is no debugger here, so that would
    // widen what a process may do to buy nothing. `ud2` is a **fault the CPU raises** on an opcode
    // permanently reserved to be invalid, so no gate DPL is involved, and "this must never execute"
    // is exactly what the instruction means.
    unsafe {
        core::arch::asm!("ud2", options(nostack, nomem));
    };
    loop {
        core::hint::spin_loop();
    }
}

/// Require that `ok` holds, or die on the line that does not.
///
/// The one way a nife program says "no": there is no error value to return, and limping past a
/// failed check turns a detected mistake into an undetected one, so this [`trap`]s at the call.
///
/// # Examples
/// ```ignore
/// require(reply == 0);
/// require(frame >= 0);
/// ```
///
/// Name: provisional, minted by milestone 408 (one home for `fn check(ok: bool)`) on
/// 2026-10-15 (UTC). The kernel spends `require` on a mismatch that must not be recovered from
/// (`trust::require`, which halts), so the same idea one privilege level down wears the same
/// word. Refused `check`, what every copy was called, because
/// it is one of the generic words design/naming.md names a failure mode: half the tree checks
/// something, and the crate every program links is the worst place to spend a vague word. Refused
/// `insist` (unusual enough to need explaining) and `must` (reads as a modal verb at the call
/// site, so the line parses as a fragment rather than a call).
pub fn require(ok: bool) {
    if !ok {
        trap()
    }
}

/// **The panic handler every nife program wants**, as a macro so it stays per-final-binary.
///
/// Write `user_mode_runtime::panic_handler!();` once at the top level of a binary and it expands to a
/// `#[panic_handler]` that calls [`trap`]. A library that defined the handler would force it on
/// every binary that links the library; a macro defines nothing until a binary asks.
///
/// # Examples
///
/// ```ignore
/// #![no_std]
/// #![no_main]
///
/// user_mode_runtime::panic_handler!();
///
/// #[unsafe(no_mangle)]
/// pub extern "C" fn _start() -> ! {
///     user_mode_runtime::exit()
/// }
/// ```
///
/// # BUGS
/// It takes no arguments and ignores the `PanicInfo`, because a program with no console cannot
/// print one. A program that *can* print (it holds a terminal endpoint) and wants the message on
/// the way down still writes its own handler; this macro is the default, not a mandate.
///
/// Named for the item it expands to, which is the one thing a reader needs it to say.
#[macro_export]
macro_rules! panic_handler {
    () => {
        #[panic_handler]
        fn panic(_: &::core::panic::PanicInfo) -> ! {
            $crate::trap()
        }
    };
}
