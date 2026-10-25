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

/// **Die where the mistake was.** Raise a breakpoint (`brk #0`, `ebreak`, `ud2`) the kernel turns
/// into a fault, which reports `EVENT_FAULT` to a supervisor where [`exit`] reports `EVENT_EXIT`
/// (§26 (the fault endpoint)), so a panic must take this path. The trailing spin only satisfies
/// `-> !`; calling `exit` would make an impossible return look like a clean death.
///
/// A verb, like `send`, `receive`, `reap` and `exit`: the naming tenet's noun rule is about crates,
/// programs and modules, not the things they do.
///
/// # BUGS
/// - Every trap prints the same fault line (one `pc`); notes/user-mode-runtime.md has the fix.
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

/// Require that `ok` holds, or die on the line that does not. The one way a nife program says
/// "no": there is no error value to return, and limping past a failed check turns a detected
/// mistake into an undetected one, so this [`trap`]s at the call.
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
/// user_mode_runtime::program_entry!(start);
///
/// fn start(_arg0: u64, _arg1: u64, _arg2: u64) -> ! {
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

/// **The process entry point, entered with the stack the C ABI promises**: `_start`, calling
/// `$entry(arg0, arg1, arg2)`.
///
/// Every `no_std` program writes `user_mode_runtime::program_entry!(start);` once, where `start` is
/// a `fn(u64, u64, u64) -> !` taking the three argument registers its spawner set. The kernel
/// enters a new thread with the stack pointer at the top of its stack page, 16-byte aligned. On
/// `x86_64` the psABI promises `rsp % 16 == 8` at a function's first instruction, the return address
/// a `call` pushes, so a plain `extern "C" fn _start` would run every frame eight bytes off and
/// misplace every 16-byte-aligned local. On `x86_64` this expands to a one-`call` shim, as std's own
/// entry does (`patches/std-nife/overlay/std/src/sys/pal/nife/entry.rs`); aarch64 and riscv64 push
/// nothing on a call and get a plain `_start`. `loaded_image_check::verify` checks the alignment
/// from inside a program, and `script/lint` refuses a hand-written `_start` in a program. See
/// notes/user-mode-runtime.md, "Entering a process".
///
/// # Examples
///
/// ```ignore
/// user_mode_runtime::program_entry!(start);
///
/// fn start(arg0: u64, _arg1: u64, _arg2: u64) -> ! {
///     user_mode_runtime::exit()
/// }
/// ```
///
/// Name: provisional. Refused `entry!` alone (every crate has entries) and `start!` (reads as an
/// imperative at the call site).
#[macro_export]
macro_rules! program_entry {
    ($entry:path) => {
        /// The process entry point `program_entry!` writes, the symbol the link script's
        /// `ENTRY(_start)` reads.
        ///
        /// Name: recorded (crates/user_mode_runtime/link.ld, its `ENTRY` line). The ELF entry
        /// symbol every toolchain uses.
        #[cfg(any(not(target_arch = "x86_64"), kani))]
        #[unsafe(no_mangle)]
        pub extern "C" fn _start(arg0: u64, arg1: u64, arg2: u64) -> ! {
            $entry(arg0, arg1, arg2)
        }

        #[cfg(all(target_arch = "x86_64", not(kani)))]
        extern "C" fn __program_entry(arg0: u64, arg1: u64, arg2: u64) -> ! {
            $entry(arg0, arg1, arg2)
        }

        // The `call` pushes the return address the psABI expects at entry; the argument registers
        // pass through untouched. The `ud2` is unreachable, because the entry diverges. Kani
        // cannot compile `global_asm!`, so under it the plain `_start` above stands in.
        #[cfg(all(target_arch = "x86_64", not(kani)))]
        ::core::arch::global_asm!(
            ".pushsection .text._start, \"ax\", @progbits",
            ".globl _start",
            "_start:",
            "call {entry}",
            "ud2",
            ".popsection",
            entry = sym __program_entry,
        );
    };
}
