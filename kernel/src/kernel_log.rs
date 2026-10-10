//! **The kernel's ring, and where each of its lines goes** (milestone 342 (the kernel and the
//! `console` server drive one UART from two address spaces); §175 (where the kernel's own output
//! goes once userspace owns the console), ruled B with a panic escape and a fallback; the drain
//! shape is calef's ruling F of 2026-10-03 UTC, recorded in `notes/kernel-ring-drain.md`).
//!
//! Every line the kernel prints becomes one F3 record in a ring of frames (the layout is
//! `system_log_protocol::kernel_ring`, shared with the log service). What changes is whether the
//! kernel also writes the line to the UART itself:
//!
//! - **No drainer yet** (all of boot, and every boot that never starts one): yes, byte by byte as
//!   it always has, and the record is flagged `DIRECT` so the service will not print it again.
//! - **A drainer that keeps up**: no. The line waits in the ring, the kernel signals the service,
//!   and the service forwards it whole to the console, which puts it on a line of its own.
//! - **A drainer that has stopped keeping up** (more than half the ring unread, or the oldest
//!   unread line older than [`STALL_NANOS`]): yes, with every unread line before it, each counted
//!   in the ring's fallback count and flagged `DIRECT`. A torn line while the service is down is
//!   cheaper than a line nobody sees (calef's addition to §242 (a system log)).
//! - **A panic**: yes, always. [`enter_panic`] flushes the unread lines first and then stops
//!   holding anything back; it takes no lock beyond the console's, which the panic path has
//!   already broken open.
//!
//! **Signalling is deferred**, because the console lock is the leaf of the lock order (it takes
//! nothing) and `println!` runs under arbitrary locks, `IPC_TABLES` among them. A held line sets a
//! flag; [`signal_if_safe`] signals once this core holds no lock, which is the end of `_print` or,
//! failing that, the next timer tick (`sched::on_tick`). A drained line is therefore up to one
//! tick late, and never early.
//!
//! # BUGS
//!
//! - **A line held just before the drainer dies is shown only when the next kernel line, or a
//!   panic, finds the ring stalled.** Nothing polls for a dead drainer; the stall test runs when
//!   the kernel next prints.
//! - **A fallback can print a line the service is forwarding at the same moment**, so the line
//!   appears twice. The kernel flags it `DIRECT` before printing, but a service that read it just
//!   before has no reason to look again.
//! - **Between the console server starting and the log service attaching**, kernel lines still go
//!   straight to the UART and can splice with the console's. The progenitor starts the service
//!   right after the console to keep that window to the build of one program.
//!
//! Name: provisional (milestone 342's lane, 2026-10-03 UTC).

use core::sync::atomic::{AtomicBool, AtomicPtr, AtomicU64, Ordering};

use system_log_protocol::kernel_ring::{self, Cursor, DETACHED, Read, Ring};
use system_log_protocol::record::{TEXT_MAX, flags};
use system_log_protocol::severity;

/// How long the oldest unread line may wait before the kernel decides its drainer has stopped.
/// Fifty ticks: long enough that a busy service is not mistaken for a dead one, short enough that
/// a person waiting on a fault report does not wonder.
pub const STALL_NANOS: u64 = 500_000_000;

static RING: AtomicPtr<AtomicU64> = AtomicPtr::new(core::ptr::null_mut());
static CURSOR: AtomicPtr<AtomicU64> = AtomicPtr::new(core::ptr::null_mut());
static RING_PHYS: AtomicU64 = AtomicU64::new(0);
static CURSOR_PHYS: AtomicU64 = AtomicU64::new(0);
static NOTIFY: AtomicU64 = AtomicU64::new(u64::MAX);
static PANICKING: AtomicBool = AtomicBool::new(false);
static SIGNAL_PENDING: AtomicBool = AtomicBool::new(false);

/// The line being assembled, kept in the console's own struct so the console lock guards it.
pub struct Line {
    buf: [u8; TEXT_MAX],
    len: usize,
    /// Decided at the line's first byte: whether this line goes to the UART as it is printed.
    direct: bool,
    /// Decided with it: the line is direct because an attached drainer stopped keeping up, so it
    /// counts as a fallback.
    fallback: bool,
}

impl Line {
    /// Nothing assembled.
    pub const fn new() -> Self {
        Line {
            buf: [0; TEXT_MAX],
            len: 0,
            direct: true,
            fallback: false,
        }
    }
}

fn ring() -> Option<Ring<'static>> {
    let p = RING.load(Ordering::Acquire);
    if p.is_null() {
        return None;
    }
    // SAFETY: `publish` stored the direct-map address of `kernel_ring::PAGES` contiguous frames it
    // allocated and never frees, which is `kernel_ring::WORDS` aligned words.
    Ring::new(unsafe { core::slice::from_raw_parts(p, kernel_ring::WORDS) })
}

fn cursor() -> Option<Cursor<'static>> {
    let p = CURSOR.load(Ordering::Acquire);
    // SAFETY: `publish` stored the direct-map address of a frame it allocated and never frees.
    (!p.is_null()).then(|| Cursor(unsafe { &*p }))
}

/// Nanoseconds since boot, the records' clock.
fn now_nanos() -> u64 {
    let freq = crate::arch::timer::frequency();
    let t = crate::arch::timer::now();
    (t / freq) * 1_000_000_000 + (t % freq) * 1_000_000_000 / freq
}

/// **Allocate the ring, the cursor page and the notification**, once, before the progenitor is
/// built. A boot that cannot afford them keeps the kernel's old behaviour: every line direct.
pub fn publish() {
    if !RING.load(Ordering::Acquire).is_null() {
        return;
    }
    let Some(frames) = crate::memory::alloc_contiguous_zeroed(kernel_ring::PAGES) else {
        return;
    };
    let Some(page) = crate::memory::alloc_zeroed() else {
        return;
    };
    let Some(region) = crate::memory_region::create(1) else {
        return;
    };
    let Some(n) = crate::sched::create_notification_from(region) else {
        return;
    };
    let ring_va = crate::arch::mmu::phys_to_virt(frames.addr()) as *mut AtomicU64;
    let cursor_va = crate::arch::mmu::phys_to_virt(page.addr()) as *mut AtomicU64;
    // SAFETY: both are fresh frames this function owns, reached through the direct map.
    let words = unsafe { core::slice::from_raw_parts(ring_va, kernel_ring::WORDS) };
    if let Some(r) = Ring::new(words) {
        r.format();
    }
    // SAFETY: as above.
    unsafe { &*cursor_va }.store(DETACHED, Ordering::Release);
    RING_PHYS.store(frames.addr(), Ordering::Relaxed);
    CURSOR_PHYS.store(page.addr(), Ordering::Relaxed);
    NOTIFY.store(n, Ordering::Relaxed);
    CURSOR.store(cursor_va, Ordering::Release);
    RING.store(ring_va, Ordering::Release);
}

/// What `boot_progenitor` grants: the ring's first frame, the cursor page and the notification,
/// or `None` when [`publish`] could not allocate them.
pub fn grants() -> Option<(u64, u64, crate::sched::NotificationId)> {
    let ring = RING_PHYS.load(Ordering::Relaxed);
    (ring != 0).then(|| {
        (
            ring,
            CURSOR_PHYS.load(Ordering::Relaxed),
            NOTIFY.load(Ordering::Relaxed),
        )
    })
}

/// Whether a drainer has ever written its cursor. The flood probes start on it.
#[allow(dead_code)]
pub fn attached_once() -> bool {
    cursor().is_some_and(|c| c.get() != DETACHED)
}

/// **Whether the drainer is keeping up**: attached, less than half the ring unread, and the oldest
/// unread line younger than [`STALL_NANOS`].
fn draining(r: &Ring<'_>) -> bool {
    if cfg!(feature = "kernel_log_detached") || PANICKING.load(Ordering::Relaxed) {
        return false;
    }
    let Some(c) = cursor() else {
        return false;
    };
    let consumed = c.get();
    if consumed == DETACHED {
        return false;
    }
    let next = r.next_seq();
    let unread = next.saturating_sub(consumed);
    if unread == 0 {
        return true;
    }
    if unread >= kernel_ring::SLOTS / 2 {
        return false;
    }
    let mut scratch = [0u8; TEXT_MAX];
    match r.read(consumed, &mut scratch) {
        Read::Record(h, _) => now_nanos().saturating_sub(h.time) < STALL_NANOS,
        _ => false,
    }
}

/// Whether a drainer has attached and the kernel is printing for itself anyway: it stopped, and
/// this is not a panic (whose lines are not fallbacks) or the detached control.
fn stalled_attached() -> bool {
    !cfg!(feature = "kernel_log_detached")
        && !PANICKING.load(Ordering::Relaxed)
        && cursor().is_some_and(|c| c.get() != DETACHED)
}

/// Write `bytes` through `out`, which takes `&str`: valid UTF-8 as it is, anything else as the
/// replacement character, so a line cut mid-character still goes out.
fn emit(bytes: &[u8], out: &mut impl FnMut(&str)) {
    for chunk in bytes.utf8_chunks() {
        out(chunk.valid());
        if !chunk.invalid().is_empty() {
            out("\u{fffd}");
        }
    }
}

/// Print every unread line the kernel has not already printed itself, flagging each `DIRECT` and
/// counting it. The fallback's first half, and the panic's.
fn catch_up(r: &Ring<'_>, out: &mut impl FnMut(&str)) {
    let Some(c) = cursor() else {
        return;
    };
    let consumed = c.get();
    if consumed == DETACHED {
        return;
    }
    let mut scratch = [0u8; TEXT_MAX];
    for seq in consumed.max(r.oldest())..r.next_seq() {
        if let Read::Record(h, n) = r.read(seq, &mut scratch)
            && h.flags & flags::DIRECT == 0
            && r.add_flags(seq, flags::DIRECT)
        {
            emit(&scratch[..n], out);
            out("\n");
            r.count_fallback();
        }
    }
}

/// **Route one `print!` fragment.** `out` writes to the UART (and the kernel's screen); it is
/// called for direct lines as they arrive, and for held lines only when the drainer has stopped.
pub fn route(line: &mut Line, s: &str, mut out: impl FnMut(&str)) {
    let Some(r) = ring() else {
        out(s);
        return;
    };
    for piece in s.split_inclusive('\n') {
        if line.len == 0 {
            line.direct = !draining(&r);
            // A drainer is attached but has stopped: what it had not printed goes out first, so
            // the reader sees the lines in the order they were written.
            line.fallback = line.direct && stalled_attached();
            if line.fallback {
                catch_up(&r, &mut out);
            }
        }
        if line.direct {
            out(piece);
        }
        for &b in piece.as_bytes() {
            if b == b'\n' {
                commit(&r, line, false, &mut out);
                continue;
            }
            line.buf[line.len] = b;
            line.len += 1;
            if line.len == TEXT_MAX {
                commit(&r, line, true, &mut out);
            }
        }
    }
}

/// A line is finished (or full): record it, and print it now if the drainer has stopped.
fn commit(r: &Ring<'_>, line: &mut Line, cut: bool, out: &mut impl FnMut(&str)) {
    let mut f = flags::KERNEL | if cut { flags::CUT } else { 0 };
    if line.direct {
        f |= flags::DIRECT;
        if line.fallback {
            r.count_fallback();
        }
    } else if !draining(r) {
        catch_up(r, out);
        emit(&line.buf[..line.len], out);
        if !cut {
            out("\n");
        }
        r.count_fallback();
        f |= flags::DIRECT;
    } else {
        SIGNAL_PENDING.store(true, Ordering::Release);
    }
    r.append(now_nanos(), severity::INFO, f, &line.buf[..line.len]);
    line.len = 0;
    // A cut line's tail keeps going where its head went.
}

/// **The panic path's half.** From here on every line is direct, and whatever the drainer had not
/// printed yet (the partial line too) goes to the UART before the panic's own message. Called
/// with the console lock already broken open and held by the caller.
pub fn enter_panic(line: &mut Line, out: impl FnMut(&str)) {
    PANICKING.store(true, Ordering::Relaxed);
    flush(line, out);
}

/// **Catch the ring up to the wire, once, without the panic latch**, for milestone 592 (radon's
/// cold reboot dies in OpenSBI's PMIC write), 2026-10-10.
///
/// [`enter_panic`](Self)'s mechanics for a caller that is not panicking and is not staying: whatever
/// the drainer has not printed yet, plus the partial line, goes to the UART now, and the latch that
/// makes every later line direct is *not* set, so a caller whose reset the firmware refuses leaves
/// the console exactly as it found it. `console::drain` calls this before waiting on the
/// transmitter, so a reset's last lines survive both halves of the path: the ring and the wire.
///
/// Name: provisional (milestone 592, 2026-10-10): calef names public items.
pub fn flush(line: &mut Line, mut out: impl FnMut(&str)) {
    let Some(r) = ring() else {
        return;
    };
    catch_up(&r, &mut out);
    if line.len > 0 && !line.direct {
        emit(&line.buf[..line.len], &mut out);
        out("\n");
        line.len = 0;
    }
}

/// **Signal the drainer if a held line is waiting and this core holds no lock.** Called at the end
/// of `_print` and from the timer tick.
#[inline]
pub fn signal_if_safe() {
    if SIGNAL_PENDING.load(Ordering::Relaxed) {
        signal_slow();
    }
}

#[inline(never)]
#[cold]
fn signal_slow() {
    if crate::sync::current_rank() != crate::sync::rank::NONE {
        return;
    }
    if SIGNAL_PENDING.swap(false, Ordering::AcqRel) {
        crate::sched::signal_notification_from_interrupt(
            NOTIFY.load(Ordering::Relaxed),
            kernel_ring::NOTIFY_BIT,
        );
    }
}

/// The ring's fallback count, for the tests.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
pub fn fallback_count() -> u64 {
    ring().map_or(0, |r| r.fallback())
}

/// **Test support**: play the drainer. Set the cursor (or [`DETACHED`]) and say where the ring is.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
pub fn test_set_cursor(next: u64) {
    if let Some(c) = cursor() {
        c.set(next);
    }
}

/// **Test support**: the ring's next sequence number and a copy of record `seq`.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
pub fn test_read(seq: u64, out: &mut [u8; TEXT_MAX]) -> (u64, Option<(u8, usize)>) {
    let Some(r) = ring() else {
        return (0, None);
    };
    let rec = match r.read(seq, out) {
        Read::Record(h, n) => Some((h.flags, n)),
        _ => None,
    };
    (r.next_seq(), rec)
}

/// **Test support**: leave panic mode, which only a test that entered it on purpose may do.
#[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
pub fn test_leave_panic() {
    PANICKING.store(false, Ordering::Relaxed);
}

/// **The flood probe** (feature `console_flood`): one kernel line every few ticks on core 0 once a
/// drainer has attached, so `script/swish-check --flood` can type at a prompt while the kernel
/// talks over it. With `kernel_log_panic_probe` it panics on the thirtieth line instead, so the
/// gate can see a panic reach the UART with a drainer attached.
#[cfg(feature = "console_flood")]
pub fn flood_tick() {
    static TICKS: AtomicU64 = AtomicU64::new(0);
    static LINES: AtomicU64 = AtomicU64::new(0);
    if crate::cpu::id() != 0 || !attached_once() {
        return;
    }
    // Ten a second where the leg runs at 0.2 s a line, one a second on `x86_64`'s TCG leg, which
    // runs some thirty times slower (notes/benchmarks/swish-check-x86-leg.md). At ten a second
    // there the service fell behind, and the kernel's counted fallback spliced 29 of 6,990 lines,
    // which is the fallback working as ruled rather than the property this probe is for.
    const EVERY: u64 = if cfg!(target_arch = "x86_64") {
        100
    } else {
        10
    };
    if TICKS.fetch_add(1, Ordering::Relaxed) % EVERY != 0 {
        return;
    }
    let n = LINES.fetch_add(1, Ordering::Relaxed);
    #[cfg(feature = "kernel_log_panic_probe")]
    if n == 30 {
        panic!("kernel log panic probe");
    }
    crate::println!("  kernel flood: {n}");
}
