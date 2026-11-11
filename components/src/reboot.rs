//! **`reboot`**: restart the machine (milestone 805 (`reboot` at the prompt), DECISIONS §251
//! (restarting the machine is a kernel object the progenitor hands out)).
//!
//! Sync the writable filesystem, say what the sync answered, then ask the board's PMIC route where
//! the board has one, and the reboot object when there is none or the board is still running after
//! it (§251, "A board's PMIC route runs in userspace"). On success the last step never returns and
//! the next thing on the console is the firmware. The sync
//! is this program's own act, through a capability the file server binds sync-only, and the
//! end-of-stream after the report is the barrier that keeps the reset from beating the report to
//! the terminal (notes/board-reboot.md, "The `reboot` program", has both arguments).

//! # Capability contract
//!
//! | slot | what | why it is that and not wider |
//! |---|---|---|
//! | 0 | the output sink, `WRITE` | where the sync report goes |
//! | 8 | the diagnostics sink, `WRITE` | where a refusal goes, so `>` cannot swallow it |
//! | 13 | the reboot object, `WRITE` | the right to invoke it, and no `GRANT` to hand it on |
//! | 14 | the file server, sync-only, `WRITE` | `SYNC` and nothing else; empty with no filesystem |
//! | 23 | the board's PMIC route, `WRITE` | `RESET` and nothing else; empty on a board with none |

//! # BUGS
//!
//! - A firmware that accepts the reset and hangs cannot be told from a slow one, from inside.
//! - The call to the PMIC route has no timeout of its own: the route bounds its wait for the board
//!   to go, and a route that stops answering leaves this program blocked.
//! - A refusal does not exit non-zero: no program in this system reports an exit status.
//! - The sync is not fenced against background jobs; milestone 853 (orderly shutdown closes the
//!   sync window) owns closing it.
//! - Devices are not quiesced: a DMA transfer in flight is cut off.
//!
//! Name: ratified 2026-10-17 (calef, a program-naming session). Kept: it is the Unix word, a verb
//! that design/naming.md passes as a term of art. His answer to keeping it was "Yes".

#![no_std]
#![allow(missing_docs)]
#![no_main]

use user_mode_runtime::{call, exit, is_granted, send};

const REPORT: u64 = 0;
const DIAG_SLOT: u64 = grant_plan::DIAGNOSTICS_SLOT;
const REBOOT_SLOT: u64 = grant_plan::REBOOT_SLOT;
const SYNC_SLOT: u64 = grant_plan::SYNC_SLOT;
const ROUTE_SLOT: u64 = grant_plan::POWER_MANAGEMENT_CHIP_ROUTE_SLOT;

/// `EOPNOTSUPP`: the device offers no flush, which `fs::SYNC` passes through on purpose.
const EOPNOTSUPP: i64 = 95;

user_mode_runtime::program_entry!(start);

fn start(_a0: u64, _a1: u64, _a2: u64) -> ! {
    let has_diag = is_granted(DIAG_SLOT);
    let complain = |text: &[u8]| write_on(if has_diag { DIAG_SLOT } else { REPORT }, text);

    if !is_granted(REBOOT_SLOT) {
        complain(b"reboot: no reboot capability was granted; the machine keeps running\n");
        finish(has_diag);
    }

    // **The sync, and its answer.** An empty slot means this boot attached no writable
    // filesystem (the progenitor refuses the spawn when there is one it could not bind), so there
    // is nothing to sync. Otherwise the reply is the device's flush count, or a negative errno.
    let synced = if is_granted(SYNC_SLOT) {
        use filesystem_contract::fs;
        call(SYNC_SLOT, fs::req(fs::SYNC, fs::ROOT, 0), 0).0 as i64
    } else {
        0
    };
    let has_route = is_granted(ROUTE_SLOT);
    let restarting: &[u8] = if has_route {
        b"; restarting through the board's power controller\n"
    } else {
        b"; restarting\n"
    };
    let mut line = Line::new();
    if synced > 0 {
        line.push(b"reboot: filesystem synced (device flushes completed since boot: ");
        line.number(synced as u64);
        line.push(b")");
        line.push(restarting);
    } else if synced == 0 {
        line.push(b"reboot: no writable filesystem is attached, so nothing to sync");
        line.push(restarting);
    } else if synced == -EOPNOTSUPP {
        line.push(b"reboot: this device cannot flush on demand (EOPNOTSUPP), so every write it acknowledged is as durable as it makes it");
        line.push(restarting);
    } else {
        complain(b"reboot: the filesystem sync failed, so the machine was not restarted (errno ");
        let mut n = Line::new();
        n.number(synced.unsigned_abs());
        n.push(b")\n");
        complain(n.bytes());
        finish(has_diag);
    }
    write_on(REPORT, line.bytes());
    // The barrier: the shell takes end-of-stream only after it has passed the line on.
    send(REPORT, byte_sink_contract::eof(), 0, 0);

    // **The board's own route first**: a reset it honors never answers. Any answer means the
    // board is still running, and the reboot object is next.
    if has_route {
        use driver_process_contract::power_management_chip_route::{self as route, Status};
        let (status, word) = call(ROUTE_SLOT, route::request(route::RESET), 0);
        let why: &[u8] = match Status::from_word(status) {
            Some(Status::ReadFailed) => b"its read of the reset register failed",
            Some(Status::WriteFailed) => b"its write of the reset bit failed",
            Some(Status::StillRunning) => b"the board kept running after the write",
            Some(Status::UnknownOperation) => b"it does not know the reset request",
            None => b"the call failed",
        };
        let mut n = Line::new();
        n.push(b"reboot: the board's power controller did not restart it (");
        n.push(why);
        let (kind, failure) = route::unpack_failure(word);
        if kind != 0 {
            n.push(b"; I2C failure ");
            n.number(u64::from(kind));
            n.push(b", word ");
            n.number(u64::from(failure));
        }
        n.push(b"); asking the kernel\n");
        write_on(if has_diag { DIAG_SLOT } else { REPORT }, n.bytes());
    }

    // **Why not, in words** (calef's ruling on §251's amendment, item 3): the kernel answers one
    // of four portable reasons, and the firmware's raw code is on the kernel console above.
    let answer = user_mode_runtime::reboot(REBOOT_SLOT);
    let reason: &[u8] = match abi::Error::from_ret(answer) {
        Some(abi::Error::NoResetMechanism) => {
            b"reboot: not restarted: this machine has no reset route the kernel can ask\n"
        }
        Some(abi::Error::ResetNotSupported) => {
            b"reboot: not restarted: the firmware does not offer a reset\n"
        }
        Some(abi::Error::ResetDenied) => b"reboot: not restarted: the firmware refused the reset\n",
        Some(abi::Error::ResetDidNotHappen) => {
            b"reboot: not restarted: the reset was asked for and the machine is still running\n"
        }
        _ => {
            b"reboot: not restarted: the kernel answered with an error this program does not know\n"
        }
    };
    write_on(if has_diag { DIAG_SLOT } else { REPORT }, reason);
    if has_diag {
        send(DIAG_SLOT, byte_sink_contract::eof(), 0, 0);
    }
    exit();
}

/// End both streams and exit, for the two paths that never reached the reset.
fn finish(has_diag: bool) -> ! {
    if has_diag {
        send(DIAG_SLOT, byte_sink_contract::eof(), 0, 0);
    }
    send(REPORT, byte_sink_contract::eof(), 0, 0);
    exit();
}

/// A line built without a heap: the program has none.
struct Line {
    buf: [u8; 192],
    len: usize,
}

impl Line {
    fn new() -> Self {
        Line {
            buf: [0; 192],
            len: 0,
        }
    }

    fn push(&mut self, bytes: &[u8]) {
        let n = bytes.len().min(self.buf.len() - self.len);
        self.buf[self.len..self.len + n].copy_from_slice(&bytes[..n]);
        self.len += n;
    }

    fn number(&mut self, mut v: u64) {
        let mut digits = [0u8; 20];
        let mut i = digits.len();
        loop {
            i -= 1;
            digits[i] = b'0' + (v % 10) as u8;
            v /= 10;
            if v == 0 {
                break;
            }
        }
        self.push(&digits[i..]);
    }

    fn bytes(&self) -> &[u8] {
        &self.buf[..self.len]
    }
}

fn write_on(slot: u64, bytes: &[u8]) {
    let mut rest = bytes;
    while !rest.is_empty() {
        let (w0, w1, w2, n) = byte_sink_contract::pack(rest);
        send(slot, w0, w1, w2);
        rest = &rest[n..];
    }
}

user_mode_runtime::panic_handler!();
