//! **Starting the secondary harts and interrupting them**: SBI HSM, the line every boot prints
//! about it, and the reschedule IPI.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

/// Start hart `target_hart` at physical `entry` in S-mode, paging off, with its hart id in `a0` and
/// `context` in `a1`, through SBI HSM `hart_start`. See boot.s `secondary_boot`.
///
/// Returns the SBI error: 0 on success, nonzero for a hart the machine does not have.
pub fn cpu_start(target_hart: u64, entry: u64, context: u64) -> i64 {
    const SBI_HSM_EID: usize = 0x0048_534D; // "HSM"
    const SBI_HART_START_FID: usize = 0;
    let args = [
        target_hart as usize,
        entry as usize,
        context as usize,
        0,
        0,
        0,
    ];
    // SAFETY: HSM hart_start with (hartid, start_addr, opaque). It starts another hart at `entry`,
    // which this function's callers (the SMP bring-up) point at boot.s `secondary_boot`; it touches
    // none of this hart's memory.
    unsafe { sbi::call(SBI_HSM_EID, SBI_HART_START_FID, args) }.error as i64
}

/// Whether this machine can start a secondary hart, asked once by `smp::bring_up_secondaries`.
///
/// Always true: SBI HSM is the one mechanism, and `isa::init` refuses firmware without it
/// (`SBI_REQUIRED`) before this is reached.
pub fn can_start_secondaries() -> bool {
    true
}

/// Print how this machine starts a hart, one line on every boot beside the SMP count.
pub fn print_bring_up_mechanism() {
    crate::println!("  smp: sbi hsm hart_start (the only mechanism RISC-V defines)");
}

/// Send a reschedule IPI to `target_hart` through SBI IPI, which pends its `sip.SSIP` so it takes a
/// supervisor software interrupt and drains its inbox. The analog of aarch64's reschedule SGI.
pub fn sbi_send_ipi(target_hart: usize) {
    const SBI_IPI_EID: usize = 0x0073_5049; // "sPI"
    const SBI_SEND_IPI_FID: usize = 0;
    let mask = 1usize << target_hart; // hart_mask, relative to base 0
    // SAFETY: send_ipi with (hart bitmap, mask base 0). It sets another hart's `sip.SSIP` and
    // touches no memory of ours. The result is ignored, as it always was.
    unsafe { sbi::call(SBI_IPI_EID, SBI_SEND_IPI_FID, [mask, 0, 0, 0, 0, 0]) };
}
