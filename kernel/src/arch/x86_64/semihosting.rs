//! **Asking the host to terminate, `x86_64`.** The test harness's exit path, through QEMU's
//! `isa-debug-exit` device: a write to its I/O port ends the guest with status `(value << 1) | 1`.
//!
//! Every status that device produces is odd, so success is reported as host status **3** (a write
//! of 1), and `helpers/qemu-runner-x86_64.sh` maps it back. Both halves name the number, because
//! getting it backwards makes a failing suite pass. See notes/semihosting.md.
//!
//! # BUGS
//!
//! - A real machine has no `isa-debug-exit`, so a test build there falls through to the halt loop.

use core::arch::asm;

// Everything below is reachable only from the test harness and the test-mode panic arm, both
// `cfg(test)`, exactly as on the other two architectures.

/// The harness's success code. **Three, not zero**, because `isa-debug-exit` can only produce odd
/// statuses; see the module header.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub(crate) const EXIT_SUCCESS: u32 = 3;
/// The harness's failure code (any status that is not [`EXIT_SUCCESS`]).
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub(crate) const EXIT_FAILURE: u32 = 1;

/// The `isa-debug-exit` device's I/O port, as `helpers/qemu-runner-x86_64.sh` places it
/// (`iobase=0xf4`). Not a fixed address in the machine: it is where we put the device, and the two
/// files have to agree.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
const DEBUG_EXIT_PORT: u16 = 0xf4;

/// Terminate the QEMU guest with `code`. [`EXIT_SUCCESS`] is a clean exit; anything else fails the
/// run.
///
/// The value written is `code >> 1` so that the status QEMU reports is `code` itself: the device
/// computes `(written << 1) | 1`, so writing 1 yields 3 and writing 0 yields 1. That arithmetic
/// lives here rather than at the call sites, which name only the two constants above.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub fn exit(code: u32) -> ! {
    // SAFETY: a write to the port `helpers/qemu-runner-x86_64.sh` attached `isa-debug-exit` to. The
    // write terminates the guest, so nothing after it runs.
    unsafe { super::port::out8(DEBUG_EXIT_PORT, (code >> 1) as u8) };

    // The device terminates the guest; if it somehow does not (a real machine, or a runner that did
    // not attach it), stop rather than run on.
    loop {
        // SAFETY: halt until the next interrupt is always safe.
        unsafe { asm!("hlt", options(nomem, nostack)) };
    }
}
