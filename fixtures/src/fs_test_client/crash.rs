//! **The crash witness** (milestone 37 (prove RedoxFS's crash consistency)): the writes the FS
//! server is killed in the middle of, and the read back through the server that mounted the crashed
//! disk.
//!
//! Cut out of `fs_test_client.rs` unchanged on 2026-10-12 (UTC) by lane
//! maintainer/files-over-1500, for §282 (when a file is split, the parent becomes an index). What
//! another role uses is `pub(crate)` here.
//!
//! Name: provisional (lane maintainer/files-over-1500, 2026-10-12 UTC), for the module and the
//! file.

use filesystem_fixture as fixture;
use user_mode_runtime::{exit, require, send};

use crate::channel::{REPORT, get_page, open, read, write};

/// The crash driver (milestone 37): writes one payload and gets it acknowledged, then writes the
/// next into a server told to die partway through it.
///
/// It reports after the acknowledged write, because "payload A was acknowledged" is what makes
/// losing it a failure rather than a permitted outcome. It never returns from the second write: a
/// client of a dead server blocks forever, the open item DECISIONS §27 and notes/fs-server.md
/// record. The kernel test waits on nothing after the first report. See
/// notes/fs-server/the-test-client.md.
pub(crate) fn crash_driver() -> ! {
    use fixture::crash;
    let h = open(crash::NAME);

    // Write A and read it straight back. "The server accepted my write" and "my write landed" are
    // different claims, and the second is the one that has to be true before the kill means anything.
    write(h, 0, crash::A);
    let mut buf = [0u8; 128];
    let n = read(h, 0, crash::A.len());
    require(n == crash::A.len());
    get_page(n, &mut buf);
    require(&buf[..n] == crash::A);
    send(REPORT, fixture::SUCCESS, crash::SAW_A, 0);

    // And now the one the server dies inside. This call never returns.
    write(h, 0, crash::B);
    // Unreachable in the test's configuration; if the injector failed to fire, say so rather than
    // exiting quietly and leaving the kernel test to time out with no explanation.
    send(REPORT, fixture::SUCCESS, crash::SAW_B, 0);
    exit();
}

/// **The crash verifier** (milestone 37): a fresh client of a fresh FS server that mounted the disk
/// the killed one left behind. It reports which of the two payloads the file holds.
///
/// It classifies rather than asserts, for the reason milestone 31's attacker reports a bitmap: the
/// interesting failure is not "it did not read back", it is "it read back something that was never
/// written", and a client that panicked on a mismatch would tell the kernel test nothing about
/// which.
pub(crate) fn crash_verify() -> ! {
    use fixture::crash;
    let h = open(crash::NAME);
    let mut buf = [0u8; 256];
    let n = read(h, 0, crash::A.len().max(crash::B.len()) + 8);
    get_page(n, &mut buf);
    let saw = if n == crash::A.len() && &buf[..n] == crash::A {
        crash::SAW_A
    } else if n == crash::B.len() && &buf[..n] == crash::B {
        crash::SAW_B
    } else {
        crash::SAW_NEITHER
    };
    // The length rides home in the first word: a partial write shows up as a number rather than as
    // a bare verdict, which is the difference between "this failed" and "this is what it did".
    send(REPORT, n as u64, saw, 0);
    exit();
}
