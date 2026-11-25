//! **The trap**: the one six-register round trip per architecture every system call shares, the
//! raw and capability-invoking wrappers over it, and [`trap_call`], which traps with the typed calls
//! `moho_abi::system_call` generates from the method table.
//!
//! The crate root re-exports every public item, so each keeps its crate-root path.
//!
//! Name: ratified 2026-11-20 (UTC); first minted, lane maintainer/files-over-1500 (#2017), 2026-10-12 (UTC), for the module and
//! the file. Named for the calls it holds, one module per seam of the old crate root; the pull
//! request records no other name considered.

/// **The one trap in this crate**, one per architecture, used by every system call: the number in
/// `x8`/`a7`/`rax` and six words in both directions. No register survives: the kernel writes the
/// first on every system call and some methods write up to the sixth, which goes in as zero so
/// [`receive_message`] reads `0` unless the kernel labelled a death. Declared as an input only, the
/// sixth register would let the compiler keep a live value in it across the trap. See
/// `docs/design.md`.
///
/// # Safety
/// `svc`/`ecall` traps to the kernel. The kernel validates the capability and the method before
/// acting; that is its whole job. The caller is trusting the kernel, not the other way around.
///
/// Name: ratified 2026-11-20 (UTC); first minted, `lane/hvf-spawn-destroy-gone`, 2026-10-05 (UTC): the trap every system call
/// shares. It began as milestone 105 (the two forks)'s `invoke6`.
#[cfg(target_arch = "aarch64")]
pub(crate) unsafe fn trap6(number: u64, a: [u64; 6]) -> (u64, u64, u64, u64, u64, u64) {
    let (mut w0, mut w1, mut w2, mut w3, mut w4, mut w5): (u64, u64, u64, u64, u64, u64);
    // SAFETY: see the function doc; `x8` selects the system call, `x0..x5` carry the six-word ABI
    // in both directions. `asm!`
    // is unsafe because the compiler cannot check that, not because a caller can get it wrong.
    unsafe {
        core::arch::asm!(
            "svc #0",
            in("x8") number,
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
/// Name: ratified 2026-11-20 (UTC); first minted, `lane/hvf-spawn-destroy-gone`, 2026-10-05 (UTC): the trap every system call
/// shares. It began as milestone 105 (the two forks)'s `invoke6`.
#[cfg(target_arch = "riscv64")]
pub(crate) unsafe fn trap6(number: u64, a: [u64; 6]) -> (u64, u64, u64, u64, u64, u64) {
    let (mut w0, mut w1, mut w2, mut w3, mut w4, mut w5): (u64, u64, u64, u64, u64, u64);
    // SAFETY: see the function doc; `a7` selects the system call, `a0..a5` carry the
    // six-word ABI in both directions.
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") number,
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
/// `rdi`, `rsi`, `rdx`, `r10`, `r8`, `r9`. `system_call` writes `rcx` and `r11`, so both are
/// declared clobbered, and `r10` replaces the C ABI's `rcx`. `options(nostack)` holds:
/// `system_call` does not push. See `docs/design.md`.
///
/// # Safety
/// `system_call` traps to the kernel, which validates the capability and method before acting. Same
/// contract as the aarch64 twin: the caller trusts the kernel, not the other way around.
///
/// Name: ratified 2026-11-20 (UTC); first minted, `lane/hvf-spawn-destroy-gone`, 2026-10-05 (UTC): the trap every system call
/// shares. It began as milestone 105 (the two forks)'s `invoke6`.
#[cfg(target_arch = "x86_64")]
pub(crate) unsafe fn trap6(number: u64, a: [u64; 6]) -> (u64, u64, u64, u64, u64, u64) {
    let (mut w0, mut w1, mut w2, mut w3, mut w4, mut w5): (u64, u64, u64, u64, u64, u64);
    // SAFETY: see the function doc; `rax` selects the system call, and the six argument registers
    // carry the six-word ABI in both directions.
    unsafe {
        core::arch::asm!(
            "syscall",
            in("rax") number,
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

/// **One system call with every register the caller's to choose**: `trap6`, public, for the one
/// program that must be able to say anything to the kernel: a fuzzer of the surface a confined
/// process can reach, which draws the number and all six words at random, unknown numbers included, which no typed wrapper here can
/// express. Every other caller wants a typed wrapper, and this is not one.
///
/// # Safety
/// `trap6`'s: the kernel validates the capability and the method before acting. A caller that
/// passes `SYSTEM_CALL_EXIT` does not come back.
///
/// Name: ratified 2026-11-20 (UTC); first minted, milestone 779's lane, 2026-10-06 (UTC).
pub unsafe fn raw_system_call(number: u64, words: [u64; 6]) -> [u64; 6] {
    // SAFETY: forwarded from this function's own contract.
    let (w0, w1, w2, w3, w4, w5) = unsafe { trap6(number, words) };
    [w0, w1, w2, w3, w4, w5]
}

/// **Trap with one of `moho_abi`'s typed calls**: [`trap6`] with the number and the six words the
/// method table placed, returning the six words the kernel answered. Every wrapper in this crate
/// that makes a method the table lists builds its call with `moho_abi::system_call` and traps here,
/// so the table, not the wrapper, decides which register carries which argument. A word the table
/// does not define goes in as zero, the sixth included, so [`receive_message`](crate::receive_message)
/// reads `0` for anything the kernel did not stamp.
///
/// # Safety
/// [`trap6`]'s: the kernel validates the capability and the method before acting.
///
/// Name: provisional: the trap, taking a call. Considered
/// `invoke_call` (refused: the table's calls include the four that are not `SYSTEM_CALL_INVOKE`).
#[inline(always)]
pub(crate) unsafe fn trap_call<A>(call: moho_abi::system_call::Call<A>) -> [u64; 6] {
    // SAFETY: forwarded from this function's own contract.
    let (w0, w1, w2, w3, w4, w5) = unsafe { trap6(call.number, call.words) };
    [w0, w1, w2, w3, w4, w5]
}

/// Invoke a capability: the one system call a userspace program makes. `capability` names a capability in the
/// process's capability table, `method` selects the operation, and `argument_zero` to `argument_two` are its arguments; the return is
/// the kernel's `i64` result. For a caller that names the method itself: this crate's own wrappers
/// build their calls from the method table and trap through [`trap_call`].
///
/// # Safety
/// `svc`/`ecall` traps to the kernel. The kernel validates the capability and the method before
/// acting; that is its whole job. The caller is trusting the kernel, not the other way around.
pub unsafe fn invoke(
    capability: u64,
    method: u64,
    argument_zero: u64,
    argument_one: u64,
    argument_two: u64,
) -> i64 {
    // SAFETY: forwarded from this function's own contract, which is `trap6`'s.
    unsafe {
        trap6(
            moho_abi::SYSTEM_CALL_INVOKE,
            [
                capability,
                method,
                argument_zero,
                argument_one,
                argument_two,
                0,
            ],
        )
        .0 as i64
    }
}
