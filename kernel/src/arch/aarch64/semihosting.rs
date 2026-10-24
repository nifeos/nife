//! ARM semihosting.
//!
//! A debug channel that lets the guest ask the *host* to do things it has no
//! hardware for. We use exactly one call, `SYS_EXIT`, which asks QEMU to terminate
//! with a given process exit status. That is how `cargo test` learns whether the
//! kernel's tests passed (DECISIONS §7).
//!
//! Requires QEMU's `-semihosting` flag. See notes/semihosting.md.

// Every item here is reachable only from the test harness and the test-mode panic handler, so the
// allows are `not(test)`: the test build still holds this file to the dead-code gate.

#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub(crate) const EXIT_SUCCESS: u32 = 0;
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub(crate) const EXIT_FAILURE: u32 = 1;

/// Semihosting operation number for "terminate".
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
const SYS_EXIT: u64 = 0x18;

/// Reason code: the application exited normally (as opposed to hitting a
/// breakpoint, running out of memory, etc).
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
const ADP_STOPPED_APPLICATION_EXIT: u64 = 0x2_0026;

/// Ask the host to terminate the emulator with `code` as its exit status.
///
/// `cargo test` interprets a nonzero exit status as a test failure, so this is the
/// whole reporting mechanism.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub fn exit(code: u32) -> ! {
    // On aarch64, SYS_EXIT wants x1 to point at a two-word block:
    //   [0] = reason code
    //   [1] = exit status
    let block = [ADP_STOPPED_APPLICATION_EXIT, code as u64];

    // SAFETY: `hlt #0xf000` is the aarch64 semihosting trap. With a host attached it
    // never returns.
    //
    // Without one it raises a real exception, and since VBAR_EL1 is not yet set up we
    // would jump to garbage and die silently. The loop below would NOT be reached.
    // That is a genuine hole; milestone 2 (exception vectors) closes it. It doesn't
    // bite today only because we always run under QEMU with -semihosting.
    unsafe {
        core::arch::asm!(
            "hlt #0xf000",
            in("x0") SYS_EXIT,
            in("x1") block.as_ptr(),
        );
    }

    // The host did not end the run: stop. Not `halt`, which takes a `HaltReason` this path is
    // beneath every holder of.
    loop {
        aarch64_cpu::asm::wfi();
    }
}
