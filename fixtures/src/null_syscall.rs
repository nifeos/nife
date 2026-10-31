//! **`null_syscall`: the cost of crossing into the kernel and back**, measured from EL0 the lmbench
//! way (`lat_syscall null`), as a command (milestone 961 step 1).
//!
//! It makes [`primitive_benchmark_contract::NULL_SYSCALL_ITERATIONS`] traps with an unrecognized
//! syscall number, which the kernel rejects at once (`Err(BadSyscall)`: no scheduler, no object
//! lookup), so the loop isolates trap, dispatch and return. It times the loop with
//! `user_mode_runtime::now` and writes one row, `bench-production: null_syscall <ticks> <iters>`,
//! to its output. The bench boot spawns the same program with startup word
//! [`primitive_benchmark_contract::QUIET_BOOT_STARTUP_WORD`] and reads the row as `bench: ...`, so
//! the gated instrument and the typed one are one binary. The row reports ticks, not converted
//! time; the boot log's counter-frequency line converts them.
//!
//! # Capability contract
//!
//! | slot | what | why |
//! |---|---|---|
//! | 0 | the output sink, `WRITE` | where the row goes |
//!
//! No argument, no memory grant, no file, no input.
//!
//! # EXAMPLES
//!
//! ```text
//! $ null_syscall
//!   bench-production: null_syscall 1048576 20000
//! ```
//!
//! # BUGS
//!
//! - A production-shell row competes with the console, `net_stack` and the shell for the core. It
//!   is a different measure from the bench boot's quiet row and is recorded beside it, never gated
//!   (milestone 961, decision D6).
//!
//! Name: provisional (milestone 961 step 1, 2026-10-31 UTC). The row's own name, which is what a
//! lmbench reader looks for; the order of a ratified calef name is not claimed.

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

use primitive_benchmark_contract::{NULL_SYSCALL_ITERATIONS, Prefix, write_row};
use user_mode_runtime::{exit, now, send};

/// Slot 0: where the row goes. An endpoint with `WRITE`, the sink contract's framing.
const OUTPUT: u64 = 0;

user_mode_runtime::program_entry!(start);

fn start(startup_word: u64, _a1: u64, _a2: u64) -> ! {
    let start = now();
    for _ in 0..NULL_SYSCALL_ITERATIONS {
        null_syscall();
    }
    let ticks = now().wrapping_sub(start);
    write_row(
        |w0, w1, w2| {
            matches!(
                byte_sink_contract::classify(send(OUTPUT, w0, w1, w2)),
                byte_sink_contract::Sent::Ok
            )
        },
        Prefix::from_startup_word(startup_word),
        "null_syscall",
        ticks,
        NULL_SYSCALL_ITERATIONS,
    );
    exit();
}

/// One null syscall: a trap with an unrecognized number. The kernel returns an error, which we
/// discard; we are timing the crossing, not the result. aarch64 `svc` + `x8`, RISC-V `ecall` + `a7`,
/// `x86_64` `syscall` + `rax`.
#[inline(never)]
fn null_syscall() {
    // SAFETY: the trap is rejected (unknown number) with an error and no side effect.
    #[cfg(target_arch = "aarch64")]
    unsafe {
        core::arch::asm!(
            "svc #0",
            in("x8") 0xFFFFu64, // no defined syscall has this number
            lateout("x0") _,    // the kernel writes an error here; discard it
            // It writes nothing else for an unknown number today, but the ABI lets it answer in
            // six registers, and helpers/syscall_asm.py holds every trap to that. This function
            // is `#[inline(never)]` and these are caller-saved, so the measured code is unchanged.
            lateout("x1") _,
            lateout("x2") _,
            lateout("x3") _,
            lateout("x4") _,
            lateout("x5") _,
            options(nostack, nomem),
        );
    }
    #[cfg(target_arch = "riscv64")]
    // SAFETY: `ecall` with the benchmark's null-syscall number: it traps to the kernel, which returns immediately. This is the measurement, and the options promise it touches neither memory nor the stack.
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") 0xFFFFu64,
            lateout("a0") _,
            lateout("a1") _, // the other five ABI words, for the aarch64 arm's reason
            lateout("a2") _,
            lateout("a3") _,
            lateout("a4") _,
            lateout("a5") _,
            options(nostack, nomem),
        );
    }
    #[cfg(target_arch = "x86_64")]
    // SAFETY: `syscall` with the benchmark's null-syscall number: it traps to the kernel, which
    // returns immediately. This is the measurement, and the options promise it touches neither
    // memory nor the stack. `rcx` and `r11` are the instruction's own clobbers, declared here for
    // the same reason every other `syscall` site declares them (crates/user_mode_runtime).
    unsafe {
        core::arch::asm!(
            "syscall",
            in("rax") 0xFFFFu64,
            lateout("rdi") _,
            lateout("rsi") _, // the other five ABI words, for the aarch64 arm's reason
            lateout("rdx") _,
            lateout("r10") _,
            lateout("r8") _,
            lateout("r9") _,
            lateout("rcx") _,
            lateout("r11") _,
            options(nostack, nomem),
        );
    }
}

user_mode_runtime::panic_handler!();
