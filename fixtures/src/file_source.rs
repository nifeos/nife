//! A file in front of a byte source (milestone 50, split out of the `sink` binary by milestone
//! 292; notes/sink-protocol.md): it opens the name `file_sink` wrote and streams the contents out
//! over the sink contract, the same sixteen-byte framing that carries a program's `println!`.
//!
//! That makes `<` the same shape as `|`: the reader (`wc`, in
//! `system_tests/src/user/sink_tests.rs::one_reader_two_sources_and_the_same_answer`) holds one
//! endpoint, cannot tell a pipe from a filesystem behind it, and gives the same answer for both. It
//! is a separate process with its own FS session, so the reader's side of the claim is not
//! co-located with the writer's.

//! # Capability contract
//!
//! | slot 0 | slot 1 | slot 2 | [`PAGE_VA`] |
//! |---|---|---|---|
//! | the byte sink, `WRITE` | the FS service, `WRITE` | the report, `WRITE` | shared with the FS server |
//!
//! Slot 0 is `WRITE` here and `READ` in `file_sink`: the same slot and contract, and the direction
//! is a right. It is spawned only after `file_sink` has reported that it closed the file, because
//! the two share the FS server's one file page.

//! # BUGS
//!
//! - It reports [`fixture::DONE`] and the size it *found*, not the size it managed to send; a
//!   caller that wants delivery proven compares what it received, as every caller does.
//! - It opens one name, [`fixture::SINK_NAME`], in the FS service's root: the read half of a
//!   two-program fixture, not a general `cat`.
//! - [`PAGE_VA`] is hard-coded here and in every other program that maps the FS server's shared
//!   frame, each with its own copy of the address and its own byte accessors.
//!
//! Name: recorded (design/roadmap/0292-the-sink-contract-ends-are-three-programs.md and
//! notes/sink-protocol.md). **Provisional: calef has not ratified it.** `source` is this tree's own
//! word for the reading end of the sink contract (`grant_plan::spawnproto::Wiring` carries `sink`
//! and `source`), so `file_sink` and `file_source` are one pair. Refused `sink_verifier` (names the
//! test it serves; it verifies nothing). Refused `file_reader` (generic). Refused `cat` (a reader
//! would expect a name argument and a general program).

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

use byte_sink_contract::fixture;
use filesystem_contract::fs;
use user_mode_runtime::mapped_window::MappedWindow;
use user_mode_runtime::{call, exit, send};

/// The byte sink this process writes the file into, `WRITE`.
const SINK: u64 = 0;
/// The FS-service endpoint: a directory capability, and the authority the program on the far side
/// of [`SINK`] deliberately does not hold.
const FS: u64 = 1;
/// Where the size it found, or any failure, goes.
const REPORT: u64 = 2;

/// The page shared with the FS server. Matches `fs_service`'s `FILE_VA_CLIENT`.
const PAGE_VA: u64 = address_space_map::pair_page(0x0000_0000_0060_0000);
/// Its size, the FS contract's transfer unit.
const PAGE: usize = filesystem_contract::PAGE;

// SAFETY: the wiring maps one page read/write at PAGE_VA before this program runs (milestone 139
// round 2; see `user_mode_runtime::mapped_window`, which is what collapsed the hand-rolled read_volatile/
// write_volatile this used to carry).
const WINDOW: MappedWindow = unsafe { MappedWindow::new(PAGE_VA, PAGE as u64) };

/// Copy `bytes` into the shared page.
fn put(bytes: &[u8]) {
    for (i, &b) in bytes.iter().take(PAGE).enumerate() {
        WINDOW.w8(i as u64, b);
    }
}

/// Read one byte out of the shared page.
fn get(i: usize) -> u8 {
    WINDOW.r8(i as u64)
}

/// One request to the FS server. Negative replies are the caller's to interpret.
fn fs_call(w0: u64, w1: u64) -> i64 {
    call(FS, w0, w1).0 as i64
}

/// Open what `file_sink` wrote and stream it out over the sink contract, so the bytes that reach the
/// test have been through a real filesystem and back.
#[unsafe(no_mangle)]
pub extern "C" fn _start(_a0: u64, _a1: u64, _a2: u64) -> ! {
    let name = fixture::SINK_NAME.as_bytes();
    put(name);
    let handle = fs_call(fs::req(fs::OPEN, fs::ROOT, name.len() as u64), 0);
    if handle < 0 {
        send(REPORT, fixture::BAD, handle as u64, 0);
        exit();
    }
    let handle = handle as u64;

    let size = fs_call(fs::req(fs::FSTAT, handle, 0), 0);
    if size < 0 {
        send(REPORT, fixture::BAD, size as u64, 1);
        exit();
    }
    let size = size as u64;

    let mut off = 0u64;
    while off < size {
        let want = (size - off).min(PAGE as u64);
        let got = fs_call(fs::req(fs::READ, handle, want), off);
        if got <= 0 {
            send(REPORT, fixture::BAD, got as u64, off);
            exit();
        }
        // Out of the page and onto the sink, sixteen bytes at a time. The page is the FS contract's
        // transfer unit and the message is the sink contract's; this loop is the seam between them,
        // and it is the whole of what a sink adapter is.
        let mut i = 0usize;
        while i < got as usize {
            let mut chunk = [0u8; byte_sink_contract::INLINE_MAX];
            let n = (got as usize - i).min(byte_sink_contract::INLINE_MAX);
            for (k, b) in chunk[..n].iter_mut().enumerate() {
                *b = get(i + k);
            }
            let (w0, w1, w2, _) = byte_sink_contract::pack(&chunk[..n]);
            send(SINK, w0, w1, w2);
            i += n;
        }
        off += got as u64;
    }

    send(SINK, byte_sink_contract::eof(), 0, 0);
    let _ = fs_call(fs::req(fs::CLOSE, handle, 0), 0);
    send(REPORT, fixture::DONE, size, 0);
    exit();
}

user_mode_runtime::panic_handler!();
