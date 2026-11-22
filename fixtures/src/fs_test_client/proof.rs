//! **The end-to-end proof** (role 0): the `motd` read, the repeat writes and the attribute witness,
//! reported home.
//!
//! Cut out of `fs_test_client.rs` unchanged on 2026-10-12 (UTC) by lane
//! maintainer/files-over-1500, for §282 (when a file is split, the parent becomes an index). What
//! another role uses is `pub(crate)` here.
//!
//! Name: provisional (lane maintainer/files-over-1500, 2026-10-12 UTC), for the module and the
//! file.

use file_system_fixture as fixture;
use user_mode_runtime::{exit, require, send};

use crate::channel::{REPORT, get_page, open, read, write};
use crate::extended_attributes::attr_witness;

/// How long each repeat-write payload is: exactly the fixture pattern's length, so every write in
/// this test (including the final one, which restores the fixture) replaces the file's contents
/// completely. There is no truncate verb, so a shorter write would leave the previous tail behind and
/// the gate's post-run check would see a mixture.
const REPEAT_LEN: usize = fixture::WRITE_PATTERN.len();

/// The payload for repeat-write pass `n`: tagged with the pass, position-dependent after that, and
/// [`REPEAT_LEN`] bytes like every other write here, so a stale or shifted read cannot match. The
/// twin of `redoxfs_server`'s host-side `repeat_write_payload`.
fn repeat_payload(pass: u8) -> [u8; REPEAT_LEN] {
    let mut p = [0u8; REPEAT_LEN];
    p[..8].copy_from_slice(b"CRKRPT__");
    p[7] = b'0' + pass;
    let mut i = 8;
    while i < REPEAT_LEN {
        p[i] = (i as u8).wrapping_mul(31) ^ pass;
        i += 1;
    }
    p
}

pub(crate) fn proof() -> ! {
    // Read the motd the image ships with, through a handle the server minted for us. This proves
    // the whole read path end to end: a real RedoxFS image we did not write, mounted by a confined
    // FS server over blk IPC, its files opened by name under a granted directory capability and read
    // by a client that names nothing else in the system.
    //
    let motd = open(fixture::MOTD_NAME);
    let n = read(motd, 0, fixture::MOTD.len());
    require(n == fixture::MOTD.len());
    let mut buf = [0u8; 128];
    get_page(n, &mut buf);
    require(&buf[..n] == fixture::MOTD);
    let mut head = [0u8; 8];
    head.copy_from_slice(&buf[..8]);

    // **Repeat writes, in one run.** A first write to a pristine block always worked; a write to a
    // block the image already carries is the case that loops, and the gate could not see it because
    // `mkredoxfs` rewrites the target to a placeholder before every run, making every gated write a
    // first write. Writing the same block three times here depends on nothing left over from a
    // previous invocation, so the bug cannot hide behind the harness again.
    let scratch = open(fixture::SCRATCH_NAME);
    let mut pass = 1u8;
    while pass <= 3 {
        let payload = repeat_payload(pass);
        write(scratch, 0, &payload);
        let m = read(scratch, 0, payload.len());
        require(m == payload.len());
        get_page(m, &mut buf);
        require(buf[..m] == payload[..]);
        pass += 1;
    }

    // Leave the fixture pattern behind as the LAST write, so the gate's post-run host-tool check
    // (`redoxfs_check_after_run`, which reopens the image with the pinned engine and compares
    // `scratch` against `WRITE_PATTERN`) still validates a documented value. Every write above is the
    // same length as this one, so this replaces them completely.
    write(scratch, 0, fixture::WRITE_PATTERN);
    let f = read(scratch, 0, fixture::WRITE_PATTERN.len());
    require(f == fixture::WRITE_PATTERN.len());
    get_page(f, &mut buf);
    require(&buf[..f] == fixture::WRITE_PATTERN);

    // **Extended attributes** (milestone 57), on the same server over the same endpoint, so the
    // whole three-process stack does not have to be booted a second time to prove a fourth verb
    // family. It runs after the fixture restore above because attributes do not touch a file's
    // contents, so the gate's post-run byte comparison is unaffected either way and this ordering
    // keeps the restore the last thing that writes bytes.
    let attrs = attr_witness(scratch);

    // Report the motd head, the success sentinel, and the attribute witness's bitmap; the kernel
    // asserts all three, and the third against an exact expected set.
    send(REPORT, u64::from_le_bytes(head), fixture::SUCCESS, attrs);
    exit();
}
