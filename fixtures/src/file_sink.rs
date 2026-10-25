//! A file behind a byte sink (notes/sink-protocol.md). It holds a directory capability on the FS
//! service, which can open, read, write anywhere, truncate and stat; its client holds an endpoint
//! over which the only expressible request is *here are up to sixteen bytes, append them*. So
//! `> report.txt` grants strictly less than a Unix fd 1, structurally: no message seeks,
//! truncates, re-reads or stats, and nothing in the client's table names the FS server.
//!
//! | slot 0 | slot 1 | slot 2 | [`PAGE_VA`] |
//! |---|---|---|---|
//! | the sink it serves, `READ` | the FS service, `WRITE` | the report, `WRITE` | shared with the FS server |

//! # Report
//!
//! [`fixture::READY`] once the file is open, then [`fixture::DONE`] and the byte total, or
//! [`fixture::BAD`] and the failing request; readiness is separate so a caller never hands the sink
//! to a writer before anything serves it. This is a fixture: the shell's `>` goes through
//! `components/src/fs_file_caretaker.rs`, and `kernel::user::fs_service::start_file_sink` is this
//! program's only caller. The two messages under EXAMPLES are the whole contract: there is no
//! third, and that absence is the confinement.

//! # EXAMPLES
//! ```
//! use byte_sink_contract::{eof, pack};
//!
//! let (w0, w1, w2, n) = pack(b"hello\n"); // SEND these three words: append n bytes
//! assert_eq!(n, 6);
//! let _end = eof();                       // SEND this: close the file
//! # let _ = (w0, w1, w2);
//! ```
//!

//! # BUGS
//!
//! - Each message is written straight through, an FS round trip per sixteen bytes; buffering would
//!   be a separate component speaking the sink contract on both sides.
//! - It appends to one name, [`fixture::SINK_NAME`]; two running on one image would interleave.
//! - [`PAGE_VA`] is copied in every program that maps the FS server's shared frame.
//!
//! Name: recorded (design/roadmap/0292-the-sink-contract-ends-are-three-programs.md), provisional:
//! the name the kernel side spells (`fs_service::FileSink`). Refused `file_sink_caretaker` (collides
//! with `fs_file_caretaker`) and `sink_file_writer` (this *is* the sink). What `caretaker` means for
//! a translating adapter is milestone 413 (what a `caretaker` is).

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

use byte_sink_contract::fixture;
use filesystem_contract::fs;
use user_mode_runtime::mapped_window::MappedWindow;
use user_mode_runtime::{call, exit, receive, send};

/// The byte sink this process serves, `READ`. Its clients hold `WRITE` on the same endpoint and
/// nothing else.
const SINK: u64 = 0;
/// The FS-service endpoint: a directory capability, and the authority this process holds that its
/// client deliberately does not.
const FS: u64 = 1;
/// Where readiness, the byte total, and any failure go.
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

/// One request to the FS server. Negative replies are the caller's to interpret.
fn fs_call(w0: u64, w1: u64) -> i64 {
    call(FS, w0, w1).0 as i64
}

/// **Open the sink's file, creating it if it is not there and emptying it if it is.**
///
/// `CREATE` is create, not create-or-open (DECISIONS §27), so the fallback is explicit: on any
/// refusal, open the existing name and truncate it to nothing. Stated rather than inferred, because
/// a sink that appended to whatever a previous run left behind would pass its own test on a stale
/// file. Both ISA legs write the same image name on a fresh image per run, so in practice the
/// create succeeds; the fallback is what keeps a repeat boot honest instead of confusing.
fn open_for_writing() -> u64 {
    let name = fixture::SINK_NAME.as_bytes();
    put(name);
    let created = fs_call(fs::req(fs::CREATE, fs::ROOT, name.len() as u64), 0);
    if created >= 0 {
        return created as u64;
    }
    put(name);
    let opened = fs_call(fs::req(fs::OPEN, fs::ROOT, name.len() as u64), 0);
    if opened < 0 {
        send(REPORT, fixture::BAD, opened as u64, 0);
        exit();
    }
    if fs_call(fs::req(fs::TRUNCATE, opened as u64, 0), 0) < 0 {
        send(REPORT, fixture::BAD, opened as u64, 1);
        exit();
    }
    opened as u64
}

user_mode_runtime::program_entry!(start);

/// Receive sink messages, append each one's bytes at the running offset, and close on end of
/// stream.
fn start(_a0: u64, _a1: u64, _a2: u64) -> ! {
    let handle = open_for_writing();
    send(REPORT, fixture::READY, 0, 0);

    let mut off = 0u64;
    loop {
        let (w0, w1, w2) = receive(SINK);
        let mut buf = [0u8; byte_sink_contract::INLINE_MAX];
        match byte_sink_contract::unpack(w0, w1, w2, &mut buf) {
            byte_sink_contract::Msg::Bytes(0) => {}
            byte_sink_contract::Msg::Bytes(n) => {
                put(&buf[..n]);
                let wrote = fs_call(fs::req(fs::WRITE, handle, n as u64), off);
                if wrote != n as i64 {
                    // A short write is a lost byte, and a sink that reported DONE after one would
                    // be certifying a file it did not write. Say which request failed and stop.
                    send(REPORT, fixture::BAD, wrote as u64, off);
                    exit();
                }
                off += n as u64;
            }
            byte_sink_contract::Msg::Eof => break,
            byte_sink_contract::Msg::Malformed => {
                send(REPORT, fixture::BAD, w0, off);
                exit();
            }
        }
    }

    let _ = fs_call(fs::req(fs::CLOSE, handle, 0), 0);
    send(REPORT, fixture::DONE, off, 0);
    exit();
}

user_mode_runtime::panic_handler!();
