//! **The system log service** (milestone 613 (a system log service: the in-memory half), for §242
//! (a system log)): one place the programs on a machine append to, and that readers read from. Its
//! decisions are `crates/system_log`, host-tested; this is the receive loop around one wait point.
//! `RECEIVE` on [`INTAKE`] returns a sender's three words and the badge the kernel read off its
//! capability (milestone 613's amendment to §230 (badged endpoint capabilities): a plain `SEND`
//! carries its badge too). [`Log::handle`] stamps a writer's bytes into the ring, takes a badge-0
//! registration from the spawner, or names a reader's window, which is filled and its notification
//! signalled. Signalling never blocks, so a reader that stops reading cannot hold this thread and a
//! writer never waits on a reader. In [`MODE_KERNEL`] the wait also ends on the kernel ring's
//! notification ([`drain`]). `system_tests/src/user/system_log_tests.rs` plays spawner and reader.

//! # BUGS
//!
//! - At boot it serves the kernel only: the progenitor keeps no unbadged intake, mints no writer
//!   badges and wires no reader windows, so no program's output reaches it and nothing reads it.
//! - Four readers at most (`system_log_contract::read::WINDOWS`), a page and a notification each.
//! - Its stack is one 4 KiB page, unoptimized, measured only on the QEMU test's paths (so
//!   `crates/system_log` lends 250-byte lines); a ninth writer mid-line runs on the host only.
//! - The flush deadline is the first forwarded line's, not the waiting line's ([`drain`]).
//!
//! Name: ratified 2026-10-06 (calef, "`system_log` ratified.", in conversation). §242 calls it "the
//! log service"; the crate and program share `system_log` so a reader finds both with one grep.
//! `starlog` was considered and deferred, not refused: an identity name fits the command people
//! type to read the log, which does not exist yet, rather than the service.

#![no_std]
// Program entry points, not the crates/ library surface milestone 68 (code-quality gates: one
// lint policy)'s ratchet tracks (§107 (`missing_docs` moves to `workspace.lints.rust`)): each
// `[[bin]]` is its own crate root with one `_start`, and documenting an OS-facing ABI entry point
// is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

use system_log::registry::Name;
use system_log::{Handled, Log};
use system_log_contract::kernel_ring::{self, Cursor, DETACHED, Read, Ring};
use system_log_contract::record::{self, flags};
use system_log_contract::{console, read};
use user_mode_runtime::{
    cntfrq, exit, monotonic_nanos, notification_signal, now, receive_badged, receive_badged_bound,
    send, timer_arm,
};

/// Slot 0, the intake endpoint, `READ`: writers, readers and the spawner all `SEND` here. The
/// spawner holds the only unbadged `WRITE` capability to it and mints a badged copy
/// (`abi::rendezvous::BADGE`) for each writer and reader. That makes attribution unforgeable: every
/// writer is somebody's badge, and only badge 0 may say whose.
const INTAKE: u64 = 0;

/// **`arg0` asking for the kernel's ring** (milestone 342 (the kernel and the `console` server
/// drive one UART from two address spaces), calef's ruling F). Then slot 1 is the kernel ring's
/// notification, bound to this thread by the spawner (`READ | WRITE`: the kernel signals it and so
/// does the timer), slot 2 a timer (`WRITE`), slot 3 the console server's request endpoint
/// (`WRITE`), and the readers' notifications (`WRITE`, each signalled when its window is filled)
/// follow from slot 4. Without it they follow from slot 1. `arg1` is how many readers the spawner
/// wired, at most `read::WINDOWS`.
const MODE_KERNEL: u64 = 1;
/// Slot 1 in [`MODE_KERNEL`]: the kernel ring's notification.
const KERNEL_NOTIFIED: u64 = 1;
/// Slot 2 in [`MODE_KERNEL`]: the timer that ends a kernel line's wait for a line end.
const TIMER: u64 = 2;
/// Slot 3 in [`MODE_KERNEL`]: the console server's request endpoint.
const CONSOLE: u64 = 3;
/// The bit the timer sets on the bound notification. The kernel's is `kernel_ring::NOTIFY_BIT`.
const FLUSH_BIT: u64 = 2;
/// How long a kernel line may wait for the terminal to reach a line end before the console is
/// told to put it on a line of its own: 250 ms, short of what a person waiting notices and far
/// longer than a typed line's echo takes.
const FLUSH_NANOS: u32 = 250_000_000;

/// How many reader windows the service maps, from the contract both sides link.
const WINDOWS: u64 = read::WINDOWS as u64;

/// The first reader window, checked against the address-space map at compile time. Window *n*
/// (0-based) is the page at `read::WINDOW_VA + n * 4096`, mapped read-write here and by the same
/// spawner into reader *n*. A window past the count is never written, and a missing notification
/// slot is a reader nobody can wake: its window still holds the answer.
const WINDOW_VA: u64 = address_space_map::pair_page(read::WINDOW_VA);
/// Where the spawner maps the kernel's ring (read-only, `kernel_ring::PAGES` pages) in
/// [`MODE_KERNEL`]. Must match `crates/system_initializer`'s `LOG_RING_VA`.
const RING_VA: u64 = address_space_map::pair_page(0x0070_0000);
/// Where the spawner maps the kernel ring's cursor page (read-write). Must match
/// `crates/system_initializer`'s `LOG_CURSOR_VA`.
const CURSOR_VA: u64 = address_space_map::pair_page(0x0080_0000);

/// The whole service's state, in `.bss` rather than on the one-page stack: the 64 KiB ring is
/// sixteen times the stack.
static mut LOG: Log = Log::new();

#[unsafe(no_mangle)]
pub extern "C" fn _start(mode: u64, readers: u64, _a2: u64) -> ! {
    // SAFETY: this program has one thread, and `_start` is the only code that names `LOG`, once,
    // here; every later use goes through this one reference.
    let log = unsafe { &mut *core::ptr::addr_of_mut!(LOG) };
    let kernel = mode == MODE_KERNEL;
    let first_reader = if kernel { 4 } else { 1 };
    let readers = readers.min(WINDOWS);
    let mut flush_armed = false;
    if kernel {
        drain(log, &mut flush_armed);
    }
    loop {
        let (w0, w1, w2, badge) = if kernel {
            match receive_badged_bound(INTAKE) {
                Ok(m) => m,
                Err(word) => {
                    if word & FLUSH_BIT != 0 {
                        flush_armed = false;
                        send(CONSOLE, console::OPERATION_FLUSH << 56, 0, 0);
                    }
                    if word & kernel_ring::NOTIFY_BIT != 0 {
                        drain(log, &mut flush_armed);
                    }
                    continue;
                }
            }
        } else {
            receive_badged(INTAKE)
        };
        // The intake endpoint was destroyed (its owner reclaimed the region it lived in): this
        // service has nothing left to serve. Only badge 0 can mean it, because the kernel writes
        // `Gone` with no badge, while a writer forging the word arrives with its own nonzero one.
        if badge == 0 && w0 as i64 == abi::Error::Gone as i64 {
            exit();
        }
        let Handled::Read { window, cursor } = log.handle(badge, w0, w1, w2, monotonic_nanos())
        else {
            continue;
        };
        if window as u64 >= readers {
            continue;
        }
        let va = WINDOW_VA + window as u64 * read::WINDOW_BYTES as u64;
        // SAFETY: windows 0 to `readers - 1` are pages the spawner mapped read-write at these
        // addresses before this thread started (the capability contract above). A reader shares
        // the page, so nothing here trusts what it reads from it; it only writes.
        let page = unsafe { core::slice::from_raw_parts_mut(va as *mut u8, read::WINDOW_BYTES) };
        log.fill(badge, cursor, page);
        // A refused signal (a slot the spawner left empty) is a reader nobody can wake; the
        // window holds the answer for whenever it next looks.
        let _ = notification_signal(first_reader + window as u64, 1);
    }
}

/// **Drain the kernel's ring**: store every record from the cursor on, forward the ones the kernel
/// did not print itself to the console whole, and move the cursor past them. The first call
/// attaches: the cursor goes from `DETACHED` to the oldest record still held, so the log starts
/// with what is left of the boot (`dmesg`'s shape) and the kernel stops printing for itself.
///
/// The console writes a forwarded line only at a line start. The flush timer is armed when a line
/// is forwarded and none is armed, even if the console did not queue it, so a later line queued
/// mid-line can flush far sooner than [`FLUSH_NANOS`] and its exact redraw lands inside what a
/// person types. Whether that widened the window `script/swish-check` misread (`without_redraws` in
/// `xtask/src/swish_check.rs`) is unmeasured; the fix moves the deadline to the console's queue.
fn drain(log: &mut Log, flush_armed: &mut bool) {
    // SAFETY: the spawner mapped `kernel_ring::PAGES` read-only frames at `RING_VA` and one
    // read-write frame at `CURSOR_VA` in this mode (the contract on `MODE_KERNEL`). Every access
    // below is an atomic load, or the cursor's one atomic store.
    let words = unsafe {
        core::slice::from_raw_parts(
            RING_VA as *const core::sync::atomic::AtomicU64,
            kernel_ring::WORDS,
        )
    };
    // SAFETY: as above.
    let cursor = Cursor(unsafe { &*(CURSOR_VA as *const core::sync::atomic::AtomicU64) });
    let Some(ring) = Ring::new(words) else {
        return;
    };
    if !ring.is_formatted() {
        return;
    }
    let mut seq = match cursor.get() {
        DETACHED => ring.oldest(),
        c => c.max(ring.oldest()),
    };
    let mut lost = false;
    let mut text = [0u8; record::TEXT_MAX];
    loop {
        match ring.read(seq, &mut text) {
            Read::Record(mut h, n) => {
                if lost {
                    h.flags |= flags::DROPPED_BEFORE;
                    lost = false;
                }
                log.ingest(&h, &text[..n], Some(Name::new(b"kernel")), None);
                if h.flags & flags::DIRECT == 0 {
                    forward(&text[..n]);
                    if !*flush_armed {
                        let deadline = now().saturating_add(abi::timer::counter_ticks_for(
                            0,
                            FLUSH_NANOS,
                            cntfrq(),
                        ));
                        *flush_armed = timer_arm(TIMER, deadline, KERNEL_NOTIFIED, FLUSH_BIT) >= 0;
                    }
                }
            }
            Read::Overwritten => lost = true,
            Read::NotYet => break,
        }
        seq += 1;
    }
    cursor.set(seq);
}

/// Send one kernel line to the console, sixteen bytes a message, newline included.
fn forward(text: &[u8]) {
    let mut line = [0u8; record::TEXT_MAX + 1];
    line[..text.len()].copy_from_slice(text);
    line[text.len()] = b'\n';
    for chunk in line[..text.len() + 1].chunks(16) {
        let mut w = [0u8; 16];
        w[..chunk.len()].copy_from_slice(chunk);
        let mut lo = [0u8; 8];
        let mut hi = [0u8; 8];
        lo.copy_from_slice(&w[..8]);
        hi.copy_from_slice(&w[8..]);
        send(
            CONSOLE,
            console::chunk(chunk.len()),
            u64::from_le_bytes(lo),
            u64::from_le_bytes(hi),
        );
    }
}

user_mode_runtime::panic_handler!();
