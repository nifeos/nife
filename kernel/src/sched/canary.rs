use core::sync::atomic::{AtomicU64, Ordering};

use memory_corruption_canary_gate::Gate;

/// One watched range and where its shadow copy lives.
#[derive(Clone, Copy)]
struct Watch {
    base: usize,
    len: usize,
    shadow_off: usize,
}

const MAX_RANGES: usize = 4;
/// Both registries today total ~13 KiB (128 `Option<ThreadControlBlockPointer>` at 16 bytes, 512 `Option<u64>`
/// at 16 bytes, plus generations); 24 KiB leaves room for growth and the test's scratch.
const SHADOW_BYTES: usize = 24 * 1024;
/// Print at most this many diverging bytes over an armed window, so a large legal rewrite
/// cannot flood the serial log that the dump itself needs.
const PRINT_CAP: u64 = 48;

/// Interior-mutable statics whose one owner at a time is a live `memory_corruption_canary_gate` guard:
/// `arm` writes them holding an `ArmGuard`, `check` reads and writes them holding a
/// `CheckGuard`, and the gate admits at most one guard of either kind (the exclusion is
/// loom-checked in `crates/memory_corruption_canary_gate`, where the previous hand-written spelling of this
/// serialization is also falsified).
struct Racy<T>(core::cell::UnsafeCell<T>);
// SAFETY: access is serialized by the gate's guards; see the struct comment.
unsafe impl<T> Sync for Racy<T> {}

static GATE: Gate = Gate::new();
static DIVERGED: AtomicU64 = AtomicU64::new(0);
static PRINTED: AtomicU64 = AtomicU64::new(0);
static WATCHES: Racy<([Watch; MAX_RANGES], usize)> = Racy(core::cell::UnsafeCell::new((
    [Watch {
        base: 0,
        len: 0,
        shadow_off: 0,
    }; MAX_RANGES],
    0,
)));
static SHADOW: Racy<[u8; SHADOW_BYTES]> = Racy(core::cell::UnsafeCell::new([0; SHADOW_BYTES]));

/// Arm over `ranges`, snapshotting their current bytes. Caller guarantees the ranges stay
/// readable while armed (ours are `'static` kernel tables). Serializes itself: any in-flight
/// check finishes before the plan is touched, and the new plan is published whole. Spins, so
/// call from thread context (both callers do); the tick side never spins, so a tick landing
/// mid-arm skips rather than deadlocks.
pub(crate) fn arm(ranges: &[(usize, usize)]) {
    let guard = GATE.arm();
    DIVERGED.store(0, Ordering::Relaxed);
    PRINTED.store(0, Ordering::Relaxed);
    // SAFETY: the ArmGuard is exclusive ownership of these statics (the gate's contract,
    // loom-checked in crates/memory_corruption_canary_gate); no check pass can start until it drops.
    let (watches, count) = unsafe { &mut *WATCHES.0.get() };
    // SAFETY: as above.
    let shadow = unsafe { &mut *SHADOW.0.get() };
    let mut off = 0usize;
    let mut n = 0usize;
    for &(base, len) in ranges.iter().take(MAX_RANGES) {
        assert!(off + len <= SHADOW_BYTES, "canary shadow too small");
        for i in 0..len {
            // SAFETY: caller promises base..base+len readable; volatile because another
            // core may be mid-write under IPC_TABLES (a torn snapshot only costs a printed delta).
            shadow[off + i] = unsafe { core::ptr::read_volatile((base + i) as *const u8) };
        }
        watches[n] = Watch {
            base,
            len,
            shadow_off: off,
        };
        off += len;
        n += 1;
    }
    *count = n;
    drop(guard); // the release store that publishes the plan
}

/// Disarm, and QUIESCE: when this returns, no check pass is mid-flight and none can start,
/// so the caller may repurpose the watched memory. (Today's watched ranges are `'static`, so
/// the quiescence buys certainty rather than papers over a lifetime; it costs a bounded spin
/// while at most one in-flight pass finishes.)
pub(crate) fn disarm() {
    GATE.disarm();
}

/// How many bytes have diverged since arming. The test hook, and a bench-note number.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))] // release builds read it off the serial print
pub(crate) fn divergences() -> u64 {
    DIVERGED.load(Ordering::Relaxed)
}

/// Re-read every watched byte against the shadow; print and absorb what changed. Single-flight, so
/// passes on two cores cannot interleave shadow updates.
///
/// Split in two so a disarmed tick costs no stack: a debug-build prologue reserves the whole frame
/// before an early return, and the one-piece spelling put a 592-byte frame on every thread's stack
/// every tick. Returns whether a full pass ran; `false` means disarmed, mid-arm, or another core's
/// pass holds the slot, so a caller that must see a completed pass loops until `true`.
///
/// Context: the timer tick (interrupt context, interrupts masked) and the test.
pub(crate) fn check() -> bool {
    if !GATE.is_armed_hint() {
        return false; // one relaxed load: every unarmed tick's whole cost
    }
    check_armed()
}

/// The armed pass, outlined. `#[inline(never)]` is what keeps [`check`]'s frame from
/// swallowing this one's; without it the split is cosmetic.
#[inline(never)]
fn check_armed() -> bool {
    let Some(guard) = GATE.try_check() else {
        return false;
    };
    // SAFETY: the CheckGuard is exclusive ownership of these statics (the gate's contract,
    // loom-checked in crates/memory_corruption_canary_gate), and taking it saw the arm guard's release, so the
    // plan is whole, never torn.
    let (watches, count) = unsafe { &*WATCHES.0.get() };
    // SAFETY: as above, and mutation is confined to the guard's lifetime.
    let shadow = unsafe { &mut *SHADOW.0.get() };
    let tick = crate::arch::timer::ticks();
    for w in watches.iter().take(*count) {
        for i in 0..w.len {
            // SAFETY: the armed range is 'static kernel memory (arm's contract); volatile
            // because IPC_TABLES-holding writers mutate it concurrently and honestly.
            let now = unsafe { core::ptr::read_volatile((w.base + i) as *const u8) };
            let was = shadow[w.shadow_off + i];
            if now != was {
                DIVERGED.fetch_add(1, Ordering::Relaxed);
                if PRINTED.fetch_add(1, Ordering::Relaxed) < PRINT_CAP {
                    crate::println!(
                        "    canary: tick={tick} addr={:#x} (range {:#x}+{:#x} off {:#x}) {was:#04x} -> {now:#04x}",
                        w.base + i,
                        w.base,
                        w.len,
                        i,
                    );
                }
                shadow[w.shadow_off + i] = now;
            }
        }
    }
    drop(guard); // the release store the next pass's acquire pairs with
    true
}
