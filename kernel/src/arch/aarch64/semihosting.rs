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
    // A coverage build writes its counters to the host first, on every exit path: the failing
    // runs are the ones whose coverage a reader most wants. See kernel/src/coverage.rs.
    #[cfg(feature = "coverage")]
    crate::coverage::dump();

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
        super::instructions::wfi();
    }
}

/// `SYS_OPEN`, `SYS_CLOSE` and `SYS_WRITE`, used to put a file on the host (milestone 781 (measure kernel and service coverage under QEMU)).
#[cfg(feature = "coverage")]
const SYS_OPEN: u64 = 0x01;
#[cfg(feature = "coverage")]
const SYS_CLOSE: u64 = 0x02;
#[cfg(feature = "coverage")]
const SYS_WRITE: u64 = 0x05;
/// `SYS_OPEN` mode 5: `wb`, truncate and write binary.
#[cfg(feature = "coverage")]
const OPEN_WRITE_BINARY: u64 = 5;

/// A file on the host opened for writing through semihosting. Needs QEMU's `-semihosting`.
#[cfg(feature = "coverage")]
pub(crate) struct HostFile(u64);

#[cfg(feature = "coverage")]
impl HostFile {
    /// Create `path` on the host, truncating it. `path` is NUL-terminated and the NUL is not
    /// counted in the length the host is given. `None` when the host refuses.
    pub(crate) fn create(path: &'static str) -> Option<Self> {
        assert!(path.ends_with('\0'), "a semihosting path ends in NUL");
        let block = [
            path.as_ptr() as u64,
            OPEN_WRITE_BINARY,
            (path.len() - 1) as u64,
        ];
        let handle = call(SYS_OPEN, block.as_ptr() as u64);
        // The host returns -1 for a file it could not open.
        (handle as i64 != -1).then_some(Self(handle))
    }

    /// Write all of `bytes`. False when the host reports any byte unwritten.
    pub(crate) fn write(&mut self, bytes: &[u8]) -> bool {
        let block = [self.0, bytes.as_ptr() as u64, bytes.len() as u64];
        // `SYS_WRITE` returns the number of bytes NOT written.
        call(SYS_WRITE, block.as_ptr() as u64) == 0
    }
}

#[cfg(feature = "coverage")]
impl Drop for HostFile {
    fn drop(&mut self) {
        let block = [self.0];
        call(SYS_CLOSE, block.as_ptr() as u64);
    }
}

/// One semihosting call: operation in `x0`, parameter block in `x1`, result back in `x0`.
#[cfg(feature = "coverage")]
fn call(operation: u64, block: u64) -> u64 {
    let mut result = operation;
    // SAFETY: `hlt #0xf000` is the aarch64 semihosting trap, answered by QEMU run with
    // `-semihosting`. `block` points at a live array of the words `operation` reads.
    unsafe {
        core::arch::asm!(
            "hlt #0xf000",
            inout("x0") result,
            in("x1") block,
        );
    }
    result
}
