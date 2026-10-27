//! **The channel to the FS server**: the two endpoint slots, the shared window, and the requests
//! every role makes over them (open, read, write and the directory verbs), with the failure report
//! that keeps the server's reason.
//!
//! Cut out of `fs_test_client.rs` unchanged on 2026-10-12 (UTC) by lane
//! maintainer/files-over-1500, for §282 (when a file is split, the parent becomes an index). What
//! another role uses is `pub(crate)` here.
//!
//! Name: provisional (lane maintainer/files-over-1500, 2026-10-12 UTC), for the module and the
//! file.

use filesystem_contract::fs;
use user_mode_runtime::mapped_window::MappedWindow;
use user_mode_runtime::{call, exit, require, send};

/// The file-service endpoint: the client's whole authority to the filesystem. Naming a file over it
/// is a request the server resolves under the one directory this endpoint is bound to.
pub(crate) const FILE: u64 = 0;
/// Where the client reports success (WRITE).
pub(crate) const REPORT: u64 = 1;
/// The base of the client's mapping of the channel it shares with the FS server.
///
/// **It is `filesystem_contract::fs::TRANSFER_MAX` bytes wide, not one page** (milestone 138 (close the read gap: a 4 KiB request must stop moving 128 KiB) step 3), and the
/// kernel's wiring maps every page of it here (`kernel/src/user/fs_service/channel.rs`, `map_channel`). A
/// client may not ask for more than it mapped, and this one maps all of it, which is what lets the
/// throughput role measure the contract's own ceiling rather than a page.
const FILE_VA: u64 = address_space_map::pair_page(0x0000_0000_0060_0000);

// SAFETY: the kernel's wiring maps every page of the fs::TRANSFER_MAX-byte channel at FILE_VA
// before this program runs (milestone 139 round 2; see `user_mode_runtime::mapped_window`, which is what
// collapsed the five hand-rolled read_volatile/write_volatile loops below into bounds-checked
// calls).
pub(crate) const WINDOW: MappedWindow =
    unsafe { MappedWindow::new(FILE_VA, fs::TRANSFER_MAX as u64) };

/// Copy `bytes` into the shared page (a name to open, or data to write).
pub(crate) fn put_page(bytes: &[u8]) {
    for (i, &b) in bytes.iter().enumerate() {
        WINDOW.w8(i as u64, b);
    }
}

/// Copy `bytes` into the shared page at `off`, for the one request that carries two payloads: a
/// rename's source and destination names sit back to back.
pub(crate) fn put_page_at(off: usize, bytes: &[u8]) {
    for (i, &b) in bytes.iter().enumerate() {
        WINDOW.w8((off + i) as u64, b);
    }
}

/// Read `n` bytes out of the shared page (a completed read landed there).
pub(crate) fn get_page(n: usize, out: &mut [u8]) {
    for (i, b) in out.iter_mut().take(n).enumerate() {
        *b = WINDOW.r8(i as u64);
    }
}

/// Open a name under the bound directory; returns the handle. A negative reply (no such file, or a
/// forged request) is REPORTED, not trapped, so the server's reason survives the failure.
pub(crate) fn open(name: &str) -> u64 {
    put_page(name.as_bytes());
    let (r0, _) = call(FILE, fs::req(fs::OPEN, 0, name.len() as u64), 0);
    if (r0 as i64) < 0 {
        fail(STAGE_OPEN, r0);
    }
    r0
}

/// Stage tags for [`fail`], so a reported failure says *which* request the server refused.
const STAGE_OPEN: u64 = 1;
const STAGE_READ: u64 = 2;
pub(crate) const STAGE_WRITE: u64 = 3;

/// **Report a refused request instead of faulting.** A [`require`] failure traps, and a trapped client
/// tells the waiting test only that something went wrong; the server's *reason* dies with it. This
/// sends it instead, so the reason reaches the kernel test's assertion, which prints the value it
/// compared against `SUCCESS`.
///
/// `w0` is the raw reply word, `w1` is `0xBADD_0000 | stage << 12 | errno`. The errno is recovered
/// with `filesystem_contract::reply_errno`'s rule (a negative reply is a negated errno). Note the known
/// reply-space overlap (notes/std/fs.md): the kernel's own `invoke` errors are -1..-8, so a small value
/// here is ambiguous between "the server returned this errno" and "the IPC itself failed". The raw
/// word travels in `w0` precisely so that ambiguity is visible rather than hidden.
pub(crate) fn fail(stage: u64, r0: u64) -> ! {
    let errno = if (r0 as i64) < 0 {
        (-(r0 as i64)) as u64
    } else {
        0
    };
    send(REPORT, r0, 0xBADD_0000 | (stage << 12) | errno, 0);
    exit();
}

/// Write `data` to `handle` at `offset`; checks the server accepted the whole slice.
pub(crate) fn write(handle: u64, offset: u64, data: &[u8]) {
    put_page(data);
    let (r0, _) = call(FILE, fs::req(fs::WRITE, handle, data.len() as u64), offset);
    if (r0 as i64) < 0 {
        fail(STAGE_WRITE, r0);
    }
    require(r0 as usize == data.len());
}

/// Read up to `n` bytes from `handle` at `offset` into the shared page; returns the count.
pub(crate) fn read(handle: u64, offset: u64, n: usize) -> usize {
    let (r0, _) = call(FILE, fs::req(fs::READ, handle, n as u64), offset);
    if (r0 as i64) < 0 {
        fail(STAGE_READ, r0);
    }
    r0 as usize
}

/// `OPEN` a name under a directory handle; the raw reply, so a refusal is a value rather than a trap.
pub(crate) fn dir_open(parent: u64, name: &str) -> i64 {
    put_page(name.as_bytes());
    call(FILE, fs::req(fs::OPEN, parent, name.len() as u64), 0).0 as i64
}

/// `CREATE` a name under a directory handle.
pub(crate) fn dir_create(parent: u64, name: &str) -> i64 {
    put_page(name.as_bytes());
    call(FILE, fs::req(fs::CREATE, parent, name.len() as u64), 0).0 as i64
}

/// `OPENDIR`: descend, asking for `rights`. The requested rights ride in the second word.
pub(crate) fn dir_descend(parent: u64, name: &str, rights: u64) -> i64 {
    put_page(name.as_bytes());
    call(
        FILE,
        fs::req(fs::OPENDIR, parent, name.len() as u64),
        rights,
    )
    .0 as i64
}

/// `MKDIR`: the same shape as [`dir_descend`], with creation.
pub(crate) fn dir_mkdir(parent: u64, name: &str, rights: u64) -> i64 {
    put_page(name.as_bytes());
    call(FILE, fs::req(fs::MKDIR, parent, name.len() as u64), rights).0 as i64
}

/// `UNLINK` a name under a directory handle; the raw reply, so a name that was not there is a value
/// rather than a trap. The witness uses it to clean up after itself and to provoke a node's death.
pub(crate) fn dir_unlink(parent: u64, name: &str) -> i64 {
    put_page(name.as_bytes());
    call(FILE, fs::req(fs::UNLINK, parent, name.len() as u64), 0).0 as i64
}

/// `RENAME`: two directories and two names, so both names go into the page back to back (source
/// first) and the destination's handle and length ride in the second word.
pub(crate) fn dir_rename(src_dir: u64, src: &str, dst_dir: u64, dst: &str) -> i64 {
    put_page(src.as_bytes());
    put_page_at(src.len(), dst.as_bytes());
    call(
        FILE,
        fs::req(fs::RENAME, src_dir, src.len() as u64),
        fs::rename_dst(dst_dir, dst.len() as u64),
    )
    .0 as i64
}
