//! **The attacker against a per-file grant** (milestone 31 (a capability shell: designation is
//! authorization), phase 2).
//!
//! Cut out of `fs_test_client.rs` unchanged on 2026-10-12 (UTC) by lane
//! maintainer/files-over-1500, for §282 (when a file is split, the parent becomes an index). What
//! another role uses is `pub(crate)` here.
//!
//! Name: provisional (lane maintainer/files-over-1500, 2026-10-12 UTC), for the module and the
//! file.

use filesystem_contract::{fs, grant, xattr};
use filesystem_fixture as fixture;
use fixture::escape;
use user_mode_runtime::{call, exit, send};

use crate::channel::{FILE, REPORT, get_page, put_page, put_page_at};

/// The attacker against a per-file grant (milestone 31 phase 2): holds the file caretaker's
/// endpoint, which designates one file, and reports which escapes got through (`fixture::escape`).
///
/// It is its own negative control: with `writable` false every bit must be clear, and with it true
/// `WROTE` and `TRUNCATED` must be set and the rest clear. `writable` also picks the file: the
/// writable run gets `scratch`, which the post-run host check pins and which it restores as its
/// last write, never `motd`, which two other tests compare byte for byte. Each attempt targets
/// something real that the caretaker could open. See notes/fs-server/the-test-client.md.
pub(crate) fn attacker(writable: bool) -> ! {
    let mut verdict = 0u64;
    let mut buf = [0u8; 128];

    let (granted, neighbour) = if writable {
        (fixture::SCRATCH_NAME, fixture::MOTD_NAME)
    } else {
        (fixture::MOTD_NAME, fixture::SCRATCH_NAME)
    };

    verdict |= read_the_granted_file(granted, writable);

    // 1. A second file, by name. It exists, it sits in the same directory, and the caretaker could
    //    open it. This capability names one file, so this must find nothing.
    put_page(neighbour.as_bytes());
    let (r, _) = call(FILE, fs::req(fs::OPEN, 0, neighbour.len() as u64), 0);
    if (r as i64) >= 0 {
        verdict |= escape::SECOND_FILE;
    }

    // 2. A write, against the file it IS allowed to read. Refusing a write to a file it cannot even
    //    name would prove nothing; this is the sharp case. When it is *supposed* to succeed, the
    //    bytes are read straight back: "the server accepted my write" and "my write landed" are
    //    different claims, and only the second one makes this a control for the refusal above.
    let probe = probe_payload();
    put_page(&probe[..PROBE_LEN]);
    let (w, _) = call(FILE, fs::req(fs::WRITE, grant::HANDLE, PROBE_LEN as u64), 0);
    if (w as i64) >= 0 {
        verdict |= escape::WROTE;
        let (rb, _) = call(FILE, fs::req(fs::READ, grant::HANDLE, PROBE_LEN as u64), 0);
        get_page(PROBE_LEN, &mut buf);
        if rb as usize != PROBE_LEN || buf[..PROBE_LEN] != probe[..PROBE_LEN] {
            verdict |= escape::GRANTED_READ_FAILED;
        }
    }

    // 3. Truncation is a write that carries no bytes, so a guard that only covered WRITE would miss
    //    it, and truncating to zero destroys a file just as thoroughly.
    let (t, _) = call(FILE, fs::req(fs::TRUNCATE, grant::HANDLE, 0), 0);
    if (t as i64) >= 0 {
        verdict |= escape::TRUNCATED;
    }

    // 4. Creating a new name: the way to get a second file without opening one that exists. Refused
    //    in both directions, because a file capability is not a directory.
    put_page(b"made-by-attacker");
    let (c, _) = call(FILE, fs::req(fs::CREATE, 0, 16), 0);
    if (c as i64) >= 0 {
        verdict |= escape::CREATED;
    }

    // 5. Handle guessing. The caretaker minted one handle and the FS server's own handle for the
    //    file is a different number this process never saw, so spraying numbers is probing a table
    //    it is not addressing. Every miss must be refused by the same check.
    let mut guess = 1u64;
    while guess < 8 {
        let (g, _) = call(FILE, fs::req(fs::READ, guess, 8), 0);
        if (g as i64) >= 0 {
            verdict |= escape::FORGED_HANDLE;
        }
        guess += 1;
    }

    verdict |= probe_attributes(writable);

    // Leave the fixture behind, as the LAST write, on the run that was allowed to damage it. The gate
    // reopens the image with the host tool afterwards and compares `scratch` byte for byte, so an
    // attacker that walked away from a truncated file would fail a check three steps downstream and
    // look like a filesystem bug. That exact coupling is what DECISIONS §27 was corrected four times
    // over, so it is paid off here rather than left to be rediscovered.
    if writable {
        let pat = fixture::WRITE_PATTERN;
        put_page(pat);
        let (rw, _) = call(FILE, fs::req(fs::WRITE, grant::HANDLE, pat.len() as u64), 0);
        let (rr, _) = call(FILE, fs::req(fs::READ, grant::HANDLE, pat.len() as u64), 0);
        get_page(pat.len(), &mut buf);
        if (rw as i64) < 0 || rr as usize != pat.len() || &buf[..pat.len()] != pat {
            verdict |= escape::GRANTED_READ_FAILED;
        }
    }

    send(REPORT, fixture::VERDICT, verdict, 0);
    exit();
}

/// The control: open the one file this capability designates, `granted`, and read it back.
/// Returns the verdict bits it earned.
fn read_the_granted_file(granted: &str, writable: bool) -> u64 {
    let mut verdict = 0u64;
    let mut buf = [0u8; 128];
    // The control: the one file this capability designates must open, and read back what is on it.
    //
    // The read-only run compares against `MOTD`, which nothing ever writes. The writable run cannot
    // and must not: the suite runs its tests in **alphabetical order**, so this program runs before
    // the two clients that put a known pattern in `scratch`, and asserting on those bytes here would
    // be asserting on a history this test did not write. That is the exact failure DECISIONS §27 was
    // corrected four times over, met again from the other side. So the writable run's control is that
    // the file reads back the number of bytes `FSTAT` says it has, and, further down, that the bytes
    // it writes are the bytes it reads back.
    put_page(granted.as_bytes());
    let (h, _) = call(FILE, fs::req(fs::OPEN, 0, granted.len() as u64), 0);
    if (h as i64) < 0 {
        verdict |= escape::GRANTED_READ_FAILED;
    } else {
        let (size, _) = call(FILE, fs::req(fs::FSTAT, h, 0), 0);
        let want = if writable {
            size as usize
        } else {
            fixture::MOTD.len()
        };
        let (n, _) = call(FILE, fs::req(fs::READ, h, want as u64), 0);
        if (size as i64) < 0 || (n as i64) < 0 || n as usize != want || want > buf.len() {
            verdict |= escape::GRANTED_READ_FAILED;
        } else {
            get_page(n as usize, &mut buf);
            if !writable && &buf[..n as usize] != fixture::MOTD {
                verdict |= escape::GRANTED_READ_FAILED;
            }
        }
    }
    verdict
}

/// Step 6: list, get and set an extended attribute through the grant. Returns the verdict bits
/// it earned.
fn probe_attributes(writable: bool) -> u64 {
    let mut verdict = 0u64;
    let mut buf = [0u8; 128];
    // 6. **Extended attributes** (milestone 61). An attribute is part of what a file *is*, so a
    //    capability that may read the file may read what is attached to it, and one that may not
    //    write it may not change them. Three probes, and the middle one is the sharp one.
    //
    //    A listing works through **either** direction, because it is a read. Zero attributes is a
    //    successful listing (`r0` = 0), so this distinguishes "forwarded and there were none" from
    //    "refused".
    let (l, _) = call(FILE, fs::req(fs::LISTXATTR, grant::HANDLE, 0), 0);
    if (l as i64) < 0 {
        verdict |= escape::GRANTED_ATTRS_FAILED;
    }

    //    A get of an attribute that is not set must answer `ENODATA`, **not** `EOPNOTSUPP`. That is
    //    the difference between a request that reached the store and one the caretaker refused, and
    //    it is the only probe here that a read-only grant can make about a verb that reads.
    put_page(fixture::attrs::NAME);
    let (g0, _) = call(
        FILE,
        fs::req(
            fs::GETXATTR,
            grant::HANDLE,
            fixture::attrs::NAME.len() as u64,
        ),
        0,
    );
    if filesystem_contract::reply_errno(g0 as i64) == Some(xattr::ENOTSUP) {
        verdict |= escape::GRANTED_ATTRS_FAILED;
    }

    //    A set: accepted only with the write direction, which is `WROTE`'s and `TRUNCATED`'s rule
    //    applied to the third way of changing a file. When it is accepted it is read straight back
    //    with its **type code**, because "the server took my attribute" and "my attribute is on the
    //    file" are different claims, and then removed, so the run leaves the image as it found it.
    put_page(fixture::attrs::NAME);
    put_page_at(fixture::attrs::NAME.len(), fixture::attrs::VALUE);
    let (s, _) = call(
        FILE,
        fs::req(
            fs::SETXATTR,
            grant::HANDLE,
            fixture::attrs::NAME.len() as u64,
        ),
        xattr::spec(fixture::attrs::KIND, fixture::attrs::VALUE.len() as u64),
    );
    if (s as i64) >= 0 {
        verdict |= escape::WROTE_ATTR;
        put_page(fixture::attrs::NAME);
        let (g, _) = call(
            FILE,
            fs::req(
                fs::GETXATTR,
                grant::HANDLE,
                fixture::attrs::NAME.len() as u64,
            ),
            0,
        );
        get_page(xattr::reply_value_len(g as i64), &mut buf);
        if (g as i64) < 0
            || xattr::reply_kind(g as i64) != fixture::attrs::KIND
            || xattr::reply_value_len(g as i64) != fixture::attrs::VALUE.len()
            || &buf[..fixture::attrs::VALUE.len()] != fixture::attrs::VALUE
        {
            verdict |= escape::GRANTED_ATTRS_FAILED;
        }
        put_page(fixture::attrs::NAME);
        let (rm, _) = call(
            FILE,
            fs::req(
                fs::REMOVEXATTR,
                grant::HANDLE,
                fixture::attrs::NAME.len() as u64,
            ),
            0,
        );
        if (rm as i64) < 0 {
            verdict |= escape::GRANTED_ATTRS_FAILED;
        }
    } else if writable {
        // A writable grant that cannot set one is the control for the refusal: without it, a
        // caretaker that refused every attribute request would pass the read-only run.
        verdict |= escape::GRANTED_ATTRS_FAILED;
    }
    verdict
}

/// How many bytes the attacker's write probe carries. A fixed length rather than the file's, because
/// the file's length is history this program did not write (see the control's note).
const PROBE_LEN: usize = 32;

/// A recognisable payload for the write probe: if a read-only grant ever let one through, the bytes
/// left on the disk say who put them there.
fn probe_payload() -> [u8; PROBE_LEN] {
    let mut p = [b'!'; PROBE_LEN];
    p[..16].copy_from_slice(b"CLOBBERED-BY-ATK");
    p
}
