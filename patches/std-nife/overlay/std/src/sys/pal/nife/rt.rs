//! The std runtime contract, and the syscall glue that meets it.
//!
//! This is the PAL's twin of `crates/user_mode_runtime`: the same `svc #0` / `ecall` / `syscall`
//! instructions and register convention, re-stated here because std cannot depend on an
//! out-of-tree crate. The ABI *constants* are not re-stated: `abi.rs` next door is generated
//! verbatim from `crates/abi/src/lib.rs` by `cargo xtask std-src`. Only the one trap, `trap6`, is
//! hand-copied; if `user_mode_runtime`'s changes, change this.
//!
//! # The std slot convention
//!
//! A std program's loader owes it, per the out-of-band-contract rule of notes/abi.md §4, the slots
//! `crates/std_runtime_contract` lists, each with what it grants. Two are always there: an untyped
//! budget the global allocator draws heap pages from lazily (`memory_region::MAP`, at
//! [`HEAP_BASE`], capped at [`HEAP_MAX`]), and the endpoint `stdout` and `stderr` SEND to, 16
//! bytes per message (w0 = byte count, w1|w2 = the bytes, little-endian), so out and err
//! interleave.
//!
//! The rest (the network, a directory, the clock, entropy, configuration, arguments, a resolver,
//! threads) are granted only to a program given that authority, and each is bound by the `sys/`
//! module that owns it. A slot left empty makes the matching std call answer that it has nothing:
//! `Unsupported`, `Err`, or a loud panic where the signature has no error channel (as
//! `SystemTime::now()` and `SystemRng` do, rather than report 1970 or predictable bytes). Programs
//! that never use a slot never touch it.

// **The numbers themselves live in `crates/std_runtime_contract`** (milestone 595 (provisional)),
// generated into this PAL as `runtimeproto` by `cargo xtask std-src`. They were written here, and
// again twice in the kernel test harness, until the progenitor became a fourth place that had to
// agree; now all of them read one file. Every address in it is a row of the user address-space map
// (`crates/address_space_map`, milestone 206): the three pages are runtime windows, above the net
// PAL's per-socket frames (0x1000_0000 upward, one page per socket id) and below the initial
// ramdisk window (0x2000_0000), and the heap is the map's heap band. That crate's tests pin each
// number to its band.
pub use super::runtimeproto::{
    ARGS_PAGE, ARGS_SLOT, CLOCK_PAGE, CLOCK_SLOT, CONFIG_PAGE, CONFIG_SLOT, ENTROPY_SLOT, FS_DIR_SLOT, FS_PAGE, HEAP_BASE,
    HEAP_MAX, MEMORY_REGION_SLOT, NET_MEMORY_REGION_SLOT, RESOLVER_PAGE, RESOLVER_SLOT, RESOLVER_SLOTS,
    STACK_SLOT, STDIN_READ_REQUEST, STDIN_SLOT, STDIN_STREAM_SLOT,
    STDOUT_SLOT,
};

use super::abi;

/// **The one trap** in this PAL, a twin of `crates/user_mode_runtime`'s `trap6`, whose doc has
/// the contract: the number in `x8`/`a7`/`rax`, six words in and six back in `x0..x5`,
/// `a0..a5`, or `rdi`, `rsi`, `rdx`, `r10`, `r8`, `r9`.
///
/// **Every syscall comes through here**, and the reason is a defect found on 2026-10-05
/// (notes/job-mix/spawn-job-miscompiled-yield.md). The kernel writes its result into the first register on
/// the way out of every syscall, and some methods write the next five. A wrapper that declares
/// fewer outputs than that tells the compiler a register survives the trap when it does not, and
/// the compiler is entitled to keep a live value there. `user_mode_runtime::yield_now` did, and an
/// optimised loop sent `DESTROY`'s arguments to the wrong capability after a yield. This file had
/// the same declarations (`yield_now` with no output, `invoke` with `x1..x4` input-only); no std
/// program is known to have been miscompiled by them, which is luck rather than a property.
///
/// No `nomem`: a `CALL` is how a shared page changes hands (`sys/fs`'s `Page` relies on that),
/// and a yield is when other threads write memory this one shares with them.
#[inline(always)]
unsafe fn trap6(nr: u64, a: [u64; 6]) -> [u64; 6] {
    let mut w = a;
    #[cfg(target_arch = "aarch64")]
    unsafe {
        core::arch::asm!(
            "svc #0",
            in("x8") nr,
            inlateout("x0") w[0],
            inlateout("x1") w[1],
            inlateout("x2") w[2],
            inlateout("x3") w[3],
            inlateout("x4") w[4],
            inlateout("x5") w[5],
            options(nostack),
        );
    }
    #[cfg(target_arch = "riscv64")]
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") nr,
            inlateout("a0") w[0],
            inlateout("a1") w[1],
            inlateout("a2") w[2],
            inlateout("a3") w[3],
            inlateout("a4") w[4],
            inlateout("a5") w[5],
            options(nostack),
        );
    }
    // `rcx` and `r11` are the `syscall` instruction's own: it writes the return address and
    // `RFLAGS` there unconditionally.
    #[cfg(target_arch = "x86_64")]
    unsafe {
        core::arch::asm!(
            "syscall",
            in("rax") nr,
            inlateout("rdi") w[0],
            inlateout("rsi") w[1],
            inlateout("rdx") w[2],
            inlateout("r10") w[3],
            inlateout("r8") w[4],
            inlateout("r9") w[5],
            lateout("rcx") _,
            lateout("r11") _,
            options(nostack),
        );
    }
    w
}

/// Invoke a capability. See `crates/user_mode_runtime::invoke`, of which this is a twin.
pub unsafe fn invoke(cap: u64, method: u64, a0: u64, a1: u64, a2: u64) -> i64 {
    unsafe { trap6(abi::SYS_INVOKE, [cap, method, a0, a1, a2, 0])[0] as i64 }
}

/// SEND three words on the endpoint in `slot`. Blocks until a receiver takes them.
pub fn send(slot: u64, w0: u64, w1: u64, w2: u64) -> i64 {
    unsafe { invoke(slot, abi::rendezvous::SEND, w0, w1, w2) }
}

/// `RECEIVE` on the endpoint in `slot`: block until a sender's three words arrive. A twin of
/// `user_mode_runtime::receive`; standard input's byte stream is read with it.
///
/// On a syscall-level failure (an empty slot, no `READ`) the kernel returns a negative value in the
/// first word, which a byte-stream reader tells apart from a frame because every frame's first word
/// is a small opcode and length.
pub fn receive(slot: u64) -> (u64, u64, u64) {
    // SAFETY: RECEIVE returns the three message words in the first three registers.
    let w = unsafe { trap6(abi::SYS_INVOKE, [slot, abi::rendezvous::RECEIVE, 0, 0, 0, 0]) };
    (w[0], w[1], w[2])
}

/// `CALL` the endpoint in `slot`: send two words and block until the server replies through the
/// one-shot Reply capability the kernel mints. Returns the two reply words. A twin of
/// `user_mode_runtime::call`; the net PAL (`sys/net`) drives the socket contract with it.
///
/// On a syscall-level failure (an empty slot, wrong rights) the kernel returns a negative value
/// in the first result register, which a caller distinguishes from a server reply by reading it
/// as `i64` (the net server never replies a negative word).
pub fn call(slot: u64, w0: u64, w1: u64) -> (u64, u64) {
    // SAFETY: CALL returns the two reply words in the first two registers.
    let w = unsafe { trap6(abi::SYS_INVOKE, [slot, abi::rendezvous::CALL, w0, w1, 0, 0]) };
    (w[0], w[1])
}

/// [`call`], also returning the capability the server's reply carried, if it carried one (§255
/// (each socket is its own capability)): the net PAL's socket opens. A twin of
/// `user_mode_runtime::call_receiving`. `NO_CAP` goes in as the unused third argument, so a refused
/// `CALL`, which writes no `x2`, reads as no capability rather than as slot 0.
pub fn call_receiving(slot: u64, w0: u64, w1: u64) -> (u64, u64, Option<u64>) {
    let none = abi::rendezvous::NO_CAP;
    // SAFETY: CALL returns the two reply words and the delivered slot in the first three registers.
    let w = unsafe { trap6(abi::SYS_INVOKE, [slot, abi::rendezvous::CALL, w0, w1, none, 0]) };
    (w[0], w[1], (w[2] != none).then_some(w[2]))
}

/// Drop the capability in `slot` (`SYS_CAP_DELETE`). A twin of `user_mode_runtime::cap_delete`.
/// **`ThreadControlBlock::CONFIGURE` joining a process** (milestone 812 (`std::thread::spawn` runs
/// real threads in one address space)): the process capability in `process` with `BIND`, and the new
/// thread's thread pointer in the sixth register. The answer is the thread's current-CPU page, or a
/// negative `abi::Error`.
pub fn configure_joining(tcb: u64, entry: u64, user_sp: u64, process: u64, thread_pointer: u64) -> i64 {
    let w = unsafe {
        trap6(
            abi::SYS_INVOKE,
            [tcb, abi::thread_control_block::CONFIGURE, entry, user_sp, process, thread_pointer],
        )
    };
    w[0] as i64
}

/// **`SYS_EXIT_THREAD`** (milestone 812): end the calling thread, and its process only if it was
/// the last to run. `word`, if not zero, is cleared and its futex waiters woken once this thread
/// has left user mode for good.
pub fn exit_thread(word: u64) -> ! {
    let _ = unsafe { trap6(abi::SYS_EXIT_THREAD, [word, 0, 0, 0, 0, 0]) };
    loop {
        core::hint::spin_loop();
    }
}

pub fn cap_delete(slot: u64) {
    let _ = unsafe { trap6(abi::SYS_CAP_DELETE, [slot, 0, 0, 0, 0, 0]) };
}

/// Give up the CPU (`SYS_YIELD`); the timed sleep loop is built on this.
pub fn yield_now() {
    let _ = unsafe { trap6(abi::SYS_YIELD, [0; 6]) };
}

/// Terminate this process (`SYS_EXIT`). The kernel reaps the thread and frees the address space.
pub fn exit(code: i64) -> ! {
    let _ = unsafe { trap6(abi::SYS_EXIT, [code as u64, 0, 0, 0, 0, 0]) };
    loop {
        core::hint::spin_loop();
    }
}

/// Fault on purpose (`brk` / `ebreak` / `ud2`): the kernel kills the process and reports where. This is
/// `abort()` on an OS whose failure story is "a fault the kernel attributes", and it is what
/// `panic!` reaches after printing, since the target is panic=abort.
pub fn abort() -> ! {
    #[cfg(target_arch = "aarch64")]
    unsafe {
        core::arch::asm!("brk #0", options(nostack, nomem));
    }
    #[cfg(target_arch = "riscv64")]
    unsafe {
        core::arch::asm!("ebreak", options(nostack, nomem));
    }
    // `ud2`, not `int3`, for the reason `user_mode_runtime::trap` measured: this kernel's IDT gates
    // are all DPL 0, so `int3` from ring 3 is refused as a #GP that names neither the instruction
    // nor the reason, where `ud2` is a fault the CPU raises with no gate involved.
    #[cfg(target_arch = "x86_64")]
    unsafe {
        core::arch::asm!("ud2", options(nostack, nomem));
    }
    loop {
        core::hint::spin_loop();
    }
}

/// The monotonic tick count: the one ambient readable this ABI grants (notes/abi.md, "the one
/// ambient thing"). aarch64 `CNTVCT_EL0`; RISC-V `rdtime`; `x86_64` `rdtsc`, which ring 3 may execute
/// because the kernel leaves `CR4.TSD` clear (see `user_mode_runtime::now`'s `x86_64` arm for what that
/// costs).
pub fn now() -> u64 {
    #[cfg(target_arch = "aarch64")]
    {
        let t: u64;
        unsafe {
            core::arch::asm!("mrs {}, cntvct_el0", out(reg) t, options(nomem, nostack));
        }
        t
    }
    #[cfg(target_arch = "riscv64")]
    {
        let t: u64;
        unsafe {
            core::arch::asm!("rdtime {}", out(reg) t, options(nomem, nostack));
        }
        t
    }
    #[cfg(target_arch = "x86_64")]
    {
        let (lo, hi): (u32, u32);
        unsafe {
            core::arch::asm!("rdtsc", out("eax") lo, out("edx") hi, options(nomem, nostack));
        }
        ((hi as u64) << 32) | (lo as u64)
    }
}

/// Ticks per second. aarch64 reports it in `CNTFRQ_EL0`, the one architecture where the machine
/// states its own rate. Neither other one has such a register: RISC-V states the timebase in the
/// device tree and `x86_64`'s TSC rate is measured or read from `CPUID`, and in both cases only the
/// kernel can ever know it. So the kernel learns it once at boot and maps the answer read-only at
/// `counter_frequency_contract::PAGE_VA` into every process it builds; this reads that page, the
/// same way `user_mode_runtime::cntfrq` does, which is what keeps a `std` program and a `no_std`
/// program on one machine agreeing about what a second is.
///
/// **RISC-V returned a hardcoded 10 MHz here until 2026-09-21**, QEMU `virt`'s rate, copied from the
/// same gap `user_mode_runtime::cntfrq` carried. radon (the VisionFive 2) runs at 4 MHz, so every
/// `Instant` duration and every `thread::sleep` in a std program on that board was out by 2.5x and
/// said nothing. calef's ruling: only accurate numbers, never hardcoded ones.
///
/// # Panics
///
/// If the rate is unknown, which is a zeroed or unrecognized page. **It used to fall back to 1 GHz**,
/// and that fallback is gone for the reason above: a std program whose `Instant` is silently scaled
/// wrong reports durations nobody can tell from real ones. Dying is the honest outcome, and it
/// matches `user_mode_runtime::cntfrq` exactly, which is the property this function exists to keep.
pub fn cntfrq() -> u64 {
    #[cfg(target_arch = "aarch64")]
    {
        let f: u64;
        unsafe {
            core::arch::asm!("mrs {}, cntfrq_el0", out(reg) f, options(nomem, nostack));
        }
        f
    }
    #[cfg(any(target_arch = "riscv64", target_arch = "x86_64"))]
    {
        // SAFETY: every path that builds a process on these architectures maps a page (real, or
        // zeroed when the rate was never learned) read-only at `PAGE_VA` before it runs.
        let page = unsafe {
            super::counterfreqproto::TimebasePage::new(super::counterfreqproto::PAGE_VA)
        };
        page.hz().expect(
            "the counter frequency is unknown: this process's timebase page is zeroed or \
             unrecognized",
        )
    }
}
