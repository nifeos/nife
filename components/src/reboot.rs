//! **`reboot`**: restart the machine (milestone 805 (`reboot` at the prompt), DECISIONS §251
//! (restarting the machine is a kernel object the progenitor hands out)).
//!
//! The whole program is: sync the writable filesystem, say what the sync answered, then invoke
//! the reboot object. On success the last step never returns, and the next thing on the console is
//! the firmware.
//!
//! # The capability table
//!
//! | slot | what | why it is that and not wider |
//! |---|---|---|
//! | 0 | the output sink, `WRITE` | where the sync report goes |
//! | 8 | the diagnostics sink, `WRITE` | where a refusal goes, so `>` cannot swallow it |
//! | 13 | the reboot object, `WRITE` | the right to invoke it, and no `GRANT` to hand it on |
//! | 14 | the file server, sync-only, `WRITE` | `SYNC` and nothing else; empty with no filesystem |
//!
//! # Why a sync-only capability
//!
//! §251's clause 3 says the sync comes before the reset, and the kernel does none of it. `SYNC`
//! needs `dir::WRITE` on a handle for an ordinary client, which is also the right to open and
//! truncate files by name. So the file server binds this program's badge sync-only
//! (`filesystem_contract::fs::BIND_SYNC`): it answers `SYNC` and refuses every other verb, and
//! nothing here can reach a file. calef ruled against the progenitor syncing on this program's
//! behalf (2026-10-06 UTC, #1783), so the sync is this program's own act.
//!
//! # Why the report is sent twice
//!
//! The report goes to the shell, which forwards it to the terminal, and the reset that follows
//! stops everything. A `SEND` on a rendezvous returns when the receiver has taken the message, not
//! when it has printed it, so the end-of-stream after the report is the barrier: the shell takes it
//! only once it has finished with the report. Without that the report raced the reset and lost.
//!

//! # BUGS
//!
//! - **A firmware that accepts the reset and hangs cannot be told from a slow one**, from inside
//!   the machine. radon's OpenSBI hangs that way when U-Boot has left the PMIC's I2C bus held in
//!   reset. The kernel releases the bus before the reset (milestone 592 (radon's cold reboot dies
//!   in OpenSBI's PMIC write)), and `reboot` typed at radon's prompt resets the board through the
//!   AXP15060 (`bench/radon-2026-10-21/README-890.md`, #2516).
//! - **A refusal does not exit non-zero**, because no program in this system reports an exit
//!   status (`crates/swish`'s `Status` says so). The refusal is a sentence on the second stream,
//!   naming which of the kernel's four reasons it was.
//! - **The sync is not fenced against background jobs.** A write another job makes after this
//!   program's `SYNC` is answered and before the reset lands is not covered: the window is the
//!   report's two sends and the invoke. The shell runs `reboot` in the foreground, so this is a job
//!   somebody started earlier and left writing. Milestone 853 (orderly shutdown closes the sync
//!   window) owns closing it.
//! - **Devices are not quiesced.** A DMA transfer in flight is cut off; after `SYNC` the block
//!   servers are idle and nothing else writes to persistent storage.
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

/// `EOPNOTSUPP`: the device offers no flush, which `fs::SYNC` passes through on purpose.
const EOPNOTSUPP: i64 = 95;

#[unsafe(no_mangle)]
pub extern "C" fn _start(_a0: u64, _a1: u64, _a2: u64) -> ! {
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
    let mut line = Line::new();
    if synced > 0 {
        line.push(b"reboot: filesystem synced (device flushes completed since boot: ");
        line.number(synced as u64);
        line.push(b"); restarting\n");
    } else if synced == 0 {
        line.push(b"reboot: no writable filesystem is attached, so nothing to sync; restarting\n");
    } else if synced == -EOPNOTSUPP {
        line.push(b"reboot: this device cannot flush on demand (EOPNOTSUPP), so every write it acknowledged is as durable as it makes it; restarting\n");
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
