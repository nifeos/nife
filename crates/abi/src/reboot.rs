/// `invoke(cap, REBOOT, _, _, _)`. **On success it does not return**: the kernel asks the firmware
/// for a cold reset (PSCI `SYSTEM_RESET` on aarch64, SBI SRST type 1 on riscv64, and on `x86_64`
/// the FADT reset register, then port `0xCF9`, then the 8042). When every route was refused it
/// returns the reason, one of the four `Reset…` values of [`crate::Error`], and the firmware's raw
/// code is on the kernel console just before (calef's ruling of 2026-10-06 UTC on the amendment to
/// §251 (restarting the machine is a kernel object), item 3).
///
/// **It syncs nothing**: a caller with a writable filesystem sends `filesystem_contract::fs::SYNC`
/// and waits for the reply first. A firmware that accepts the call and hangs looks the same as a
/// slow reset from inside the machine.
pub const REBOOT: u64 = 0;

/// **Why the machine did not restart, from PSCI** (milestone 805 (`reboot` at the prompt)):
/// `SYSTEM_RESET`'s return (ARM DEN 0022) as the error `REBOOT` answers. `NOT_SUPPORTED` (-1) is
/// [`super::Error::ResetNotSupported`], `DENIED` (-3) is [`super::Error::ResetDenied`], and any
/// other code means the reset was asked for and the machine is still running. The raw code is
/// on the kernel console. *(Name provisional.)*
pub const fn from_psci(code: i64) -> super::Error {
    match code {
        -1 => super::Error::ResetNotSupported,
        -3 => super::Error::ResetDenied,
        _ => super::Error::ResetDidNotHappen,
    }
}

/// **The same, from SBI SRST** `system_reset`'s `sbiret.error`: `SBI_ERR_NOT_SUPPORTED` (-2) and
/// `SBI_ERR_DENIED` (-4). *(Name provisional.)*
pub const fn from_sbi(code: i64) -> super::Error {
    match code {
        -2 => super::Error::ResetNotSupported,
        -4 => super::Error::ResetDenied,
        _ => super::Error::ResetDidNotHappen,
    }
}
