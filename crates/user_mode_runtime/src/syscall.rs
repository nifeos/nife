//! **The trap**: the one six-register round trip per architecture every syscall shares, and the raw
//! and capability-invoking wrappers over it.
//!
//! The crate root re-exports every public item, so each keeps its crate-root path.
//!
//! Name: provisional, lane maintainer/files-over-1500 (#2017), 2026-10-12 (UTC), for the module and
//! the file. Named for the calls it holds, one module per seam of the old crate root; the pull
//! request records no other name considered.

/// **The one trap in this crate**, one per architecture, used by every syscall: the number in
/// `x8`/`a7`/`rax` and six words in both directions. No caller may declare a register as
/// surviving the trap: the kernel writes its result into the first register on the way out of
/// every syscall, and some methods write up to the sixth. The sixth goes in as zero, so
/// [`receive_fault`] reads `0` for a death message the kernel did not label (§148 (resolves by
/// asking the kernel)). `helpers/syscall_asm.py` gates every userspace trap. See
/// notes/user-mode-runtime.md.

/// # Safety
/// `svc`/`ecall` traps to the kernel. The kernel validates the capability and the method before
/// acting; that is its whole job. The caller is trusting the kernel, not the other way around.
///
/// Name: provisional, `lane/hvf-spawn-destroy-gone`, 2026-10-05 (UTC): the trap every syscall
/// shares. It began as milestone 105 (the two forks)'s `invoke6`, which is now a wrapper over it.
#[cfg(target_arch = "aarch64")]
pub(crate) unsafe fn trap6(nr: u64, a: [u64; 6]) -> (u64, u64, u64, u64, u64, u64) {
    let (mut w0, mut w1, mut w2, mut w3, mut w4, mut w5): (u64, u64, u64, u64, u64, u64);
    // SAFETY: see the function doc; `x8` selects the syscall (DECISIONS §10 (process model:
    // capability-based, microkernel)), `x0..x5` carry the six-word ABI in both directions. `asm!`
    // is unsafe because the compiler cannot check that, not because a caller can get it wrong.
    unsafe {
        core::arch::asm!(
            "svc #0",
            in("x8") nr,
            inlateout("x0") a[0] => w0,
            inlateout("x1") a[1] => w1,
            inlateout("x2") a[2] => w2,
            inlateout("x3") a[3] => w3,
            inlateout("x4") a[4] => w4,
            inlateout("x5") a[5] => w5,
            options(nostack),
        );
    }
    (w0, w1, w2, w3, w4, w5)
}

/// The raw six-register round trip (RISC-V). See the aarch64 twin's doc for the contract this
/// collapses; only the trap instruction and register file differ: `ecall`, number in `a7`, the six
/// words in `a0..a5`.
///
/// # Safety
/// `ecall` traps to the kernel, which validates the capability and method before acting. Same
/// contract as the aarch64 twin: the caller trusts the kernel, not the other way around.
///
/// Name: provisional, `lane/hvf-spawn-destroy-gone`, 2026-10-05 (UTC): the trap every syscall
/// shares. It began as milestone 105 (the two forks)'s `invoke6`, which is now a wrapper over it.
#[cfg(target_arch = "riscv64")]
pub(crate) unsafe fn trap6(nr: u64, a: [u64; 6]) -> (u64, u64, u64, u64, u64, u64) {
    let (mut w0, mut w1, mut w2, mut w3, mut w4, mut w5): (u64, u64, u64, u64, u64, u64);
    // SAFETY: see the function doc; `a7` selects the syscall (DECISIONS §10), `a0..a5` carry the
    // six-word ABI in both directions.
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") nr,
            inlateout("a0") a[0] => w0,
            inlateout("a1") a[1] => w1,
            inlateout("a2") a[2] => w2,
            inlateout("a3") a[3] => w3,
            inlateout("a4") a[4] => w4,
            inlateout("a5") a[5] => w5,
            options(nostack),
        );
    }
    (w0, w1, w2, w3, w4, w5)
}

/// The raw six-register round trip (`x86_64`). See the aarch64 twin's doc for the contract; only
/// the trap instruction and register file differ: `syscall`, the number in `rax`, the words in
/// `rdi`, `rsi`, `rdx`, `r10`, `r8`, `r9` (§124 (ratify the x86_64 syscall ABI)).
///
/// `syscall` writes `rcx` and `r11` unconditionally, so both are declared clobbered; without them
/// this compiles and corrupts whatever local sat there. `r10` replaces the C ABI's `rcx` for the
/// same reason. `options(nostack)` holds because `syscall` does not push. See
/// notes/user-mode-runtime.md.

/// # Safety
/// `syscall` traps to the kernel, which validates the capability and method before acting. Same
/// contract as the aarch64 twin: the caller trusts the kernel, not the other way around.
///
/// Name: provisional, `lane/hvf-spawn-destroy-gone`, 2026-10-05 (UTC): the trap every syscall
/// shares. It began as milestone 105 (the two forks)'s `invoke6`, which is now a wrapper over it.
#[cfg(target_arch = "x86_64")]
pub(crate) unsafe fn trap6(nr: u64, a: [u64; 6]) -> (u64, u64, u64, u64, u64, u64) {
    let (mut w0, mut w1, mut w2, mut w3, mut w4, mut w5): (u64, u64, u64, u64, u64, u64);
    // SAFETY: see the function doc; `rax` selects the syscall (DECISIONS §10, §124 (the `x86_64`
    // syscall ABI)), and the six argument registers carry the six-word ABI in both directions.
    unsafe {
        core::arch::asm!(
            "syscall",
            in("rax") nr,
            inlateout("rdi") a[0] => w0,
            inlateout("rsi") a[1] => w1,
            inlateout("rdx") a[2] => w2,
            inlateout("r10") a[3] => w3,
            inlateout("r8") a[4] => w4,
            inlateout("r9") a[5] => w5,
            lateout("rcx") _,
            lateout("r11") _,
            options(nostack),
        );
    }
    (w0, w1, w2, w3, w4, w5)
}

/// **One syscall with every register the caller's to choose**: [`trap6`], public, for the one
/// program that must be able to say anything to the kernel (milestone 779 (fuzz the surface a
/// confined process can reach), provisional). `fixtures/src/confined_syscall_fuzzer.rs` draws the
/// number and all six words at random, unknown numbers included, which no typed wrapper here can
/// express. Every other caller wants a typed wrapper, and this is not one.
///
/// # Safety
/// [`trap6`]'s: the kernel validates the capability and the method before acting. A caller that
/// passes `SYS_EXIT` does not come back.
///
/// Name: provisional, milestone 779's lane, 2026-10-06 (UTC).
pub unsafe fn raw_syscall(number: u64, words: [u64; 6]) -> [u64; 6] {
    // SAFETY: forwarded from this function's own contract.
    let (w0, w1, w2, w3, w4, w5) = unsafe { trap6(number, words) };
    [w0, w1, w2, w3, w4, w5]
}

/// The six-register round trip through `SYS_INVOKE`: [`trap6`] with the number fixed and the sixth
/// word in as zero, so [`receive_fault`](crate::receive_fault) reads `0` for anything the kernel
/// did not stamp.
///
/// # Safety
/// [`trap6`]'s: the kernel validates the capability and the method before acting.
///
/// Name: provisional, milestone 105 (the two forks)'s lane, 2026-10-05 (UTC).
#[inline(always)]
pub(crate) unsafe fn invoke6(
    cap: u64,
    method: u64,
    a0: u64,
    a1: u64,
    a2: u64,
) -> (u64, u64, u64, u64, u64, u64) {
    // SAFETY: forwarded from this function's own contract.
    unsafe { trap6(abi::SYS_INVOKE, [cap, method, a0, a1, a2, 0]) }
}

/// [`invoke6`] read to the five words every method but a death delivery uses.
///
/// # Safety
/// Exactly [`invoke6`]'s contract.
#[inline(always)]
pub(crate) unsafe fn invoke5(
    cap: u64,
    method: u64,
    a0: u64,
    a1: u64,
    a2: u64,
) -> (u64, u64, u64, u64, u64) {
    // SAFETY: forwarded from this function's own contract.
    let (w0, w1, w2, w3, w4, _) = unsafe { invoke6(cap, method, a0, a1, a2) };
    (w0, w1, w2, w3, w4)
}

/// Invoke a capability: the one syscall a userspace program makes. `cap` names a capability in the
/// process's capability table, `method` selects the operation, and `a0..a2` are its arguments; the return is
/// the kernel's `i64` result. Everything else in this crate is built on this.
///
/// # Safety
/// `svc`/`ecall` traps to the kernel. The kernel validates the capability and the method before
/// acting; that is its whole job. The caller is trusting the kernel, not the other way around.
pub unsafe fn invoke(cap: u64, method: u64, a0: u64, a1: u64, a2: u64) -> i64 {
    // SAFETY: forwarded from this function's own contract, which is `invoke5`'s contract exactly.
    unsafe { invoke5(cap, method, a0, a1, a2).0 as i64 }
}
