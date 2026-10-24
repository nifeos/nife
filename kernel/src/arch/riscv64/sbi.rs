//! One SBI call: the single `ecall` every Supervisor Binary Interface request in the kernel goes
//! through, so the calling convention is stated once and a proof can stub one function
//! (notes/kernel-proofs.md).
//!
//! *Name provisional (a lane minted it 2026-09-25; names are calef's). `sbi` is the specification's
//! own name for the interface.*

use core::arch::asm;

/// What the firmware hands back: `a0` is the error (zero is success), `a1` the value. SBI v0.2
/// onwards returns this pair from every call.
#[derive(Clone, Copy)]
pub(super) struct SbiRet {
    pub error: isize,
    pub value: usize,
}

/// One SBI call: extension `eid` in `a7`, function `fid` in `a6`, arguments in `a0` through `a5`.
///
/// Returns `a0` as the error and `a1` as the value. Unused arguments are passed as zero. Inlined so
/// a call's constant arguments fold into its caller.
///
/// # Safety
/// An `ecall` can do anything the firmware implements, including start a hart at an arbitrary
/// address or reset the machine. The caller must name a call whose effects it accounts for, and its
/// `// SAFETY:` comment is where that accounting is written.
#[inline(always)]
pub(super) unsafe fn call(eid: usize, fid: usize, args: [usize; 6]) -> SbiRet {
    let error: isize;
    let value: usize;
    // SAFETY: the caller's contract. The operands are the SBI calling convention: a7/a6 name the
    // call, a0..a5 carry its arguments, a0/a1 carry the result and nothing else is written.
    unsafe {
        asm!(
            "ecall",
            in("a7") eid,
            in("a6") fid,
            inout("a0") args[0] => error,
            inout("a1") args[1] => value,
            in("a2") args[2],
            in("a3") args[3],
            in("a4") args[4],
            in("a5") args[5],
            options(nostack),
        );
    }
    SbiRet { error, value }
}
