//! The soaker: the workload that does not stop. The fifth entry of `design/fatal-risks/README.md`
//! names sustained multi-core stress with the load-sensitive assertions live as its experiment;
//! `kernel/src/soak.rs` spawns and watches this. See notes/soak.md.
//!
//! **It stresses cross-core IPC rendezvous at the highest rate the machine will do them**, through
//! the real syscall boundary, because that is where the one defect this risk has produced lived (a
//! receiver made `Ready` with nothing delivered, in `sched::wake_load_aware`). A round trip
//! (`CALL`, `RECEIVE_CAP`, `REPLY`, the caller waking) is two block/wake handshakes, each of which
//! may place its peer on another core, and with two runnable threads per core the run queues empty
//! often enough to keep `serve_steal_request` under load too.

//! # How it crosses cores
//!
//! A saturated workload never crosses cores (§138 (how a saturated workload is made to hand threads
//! across cores)), so a waiter role blocks through `Irq::WAIT` on a rendezvous the kernel signals
//! from `sched::on_tick`. The waiter is deliberately trivial; the mechanism is the kernel's wake
//! path, and what crosses is the waiters, not the pairs. Each worker spins a pseudo-random number
//! of iterations between round trips (xorshift, seeded from `arg2`, distinct per worker), so pairs
//! drift rather than lock into one interleaving.

//! # What it holds
//!
//! Slot 0 is the request endpoint, `WRITE` for a caller and `READ` for a responder, or an `Irq`
//! naming the tick route for a waiter; nothing else, not the console, clock or peer. The shared
//! page at [`soak_page::VA`] is where it publishes its counters for the kernel to read. It never
//! exits.

//! # BUGS
//!
//! - A green soak proves nothing about the concurrency being correct; its number is for comparison.
//! - The kernel samples the counters while workers write them, so a total may be a few short.
//! - A waiter's `wakes` are counted apart from round trips, so totals mean one thing across builds.
//! - A waiter whose `WAIT` is refused, and a responder whose caller stops, just stop counting; the
//!   kernel reports "stalled", and only a thread dump says which half wedged.
//!
//! Name: ratified 2026-09-13 (calef): "soak" is a term of art used correctly, already this tree's
//! vocabulary, and true of all four roles. Refused `ipc_soaker` (the grinder and waiter do no IPC),
//! `stressor` ("stress" already means `script/repeat-under-load`'s host load) and `churner` (the
//! motion, not the purpose).

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`.
#![allow(missing_docs)]
#![no_main]

use user_mode_runtime::mapped_window::{self, MappedWindow};
use user_mode_runtime::{call, interrupt_wait, receive_request, reply};

/// The one capability a soaker holds: the request endpoint, in slot 0.
const ENDPOINT: u64 = 0;

/// The one capability a **waiter** holds: the tick route's `Irq`, also in slot 0 (milestone 221).
///
/// The same slot, a different object, and that is the point rather than a collision: a waiter can
/// no more `CALL` its group's endpoint than a caller can `WAIT` on the tick route, because neither
/// holds the other's capability. One slot each is the least authority the role needs.
const TICK_IRQ: u64 = 0;

/// `arg0`: this soaker answers calls.
const ROLE_RESPONDER: u64 = 0;
/// `arg0`: this soaker calls.
const ROLE_CALLER: u64 = 1;
/// `arg0`: this soaker computes and never blocks.
const ROLE_GRINDER: u64 = 2;
/// `arg0`: this soaker blocks on the tick route and counts its wakes (milestone 221).
const ROLE_WAITER: u64 = 3;

// SAFETY: the kernel's `soak::run` maps one page read/write at `soak_page::VA` in every worker's
// address space before starting it, and `soak_page`'s host tests prove every offset this program
// uses lands inside that page.
const PAGE: MappedWindow = unsafe { MappedWindow::new(soak_page::VA, mapped_window::PAGE) };

/// The soaker's entry.
///
/// `x0` is the role, `x1` the worker index (which slot of the shared page is this one's), and `x2`
/// the seed for the jitter. The kernel gives every worker a distinct index and a distinct seed.
#[unsafe(no_mangle)]
pub extern "C" fn _start(role: u64, index: u64, seed: u64) -> ! {
    let index = index as usize;
    let rounds_at = soak_page::rounds(index);
    let mismatches_at = soak_page::mismatches(index);
    let wakes_at = soak_page::wakes(index);

    // Never zero: an xorshift seeded with zero produces zeros forever, and a worker that never
    // jitters would silently drop out of the widening this program exists to do.
    let mut state = if seed == 0 {
        0x243F_6A88_85A3_08D3
    } else {
        seed
    };
    let mut rounds: u64 = 0;
    let mut wakes: u64 = 0;
    let mut mismatches: u64 = 0;
    // Start from the seed rather than from zero, so two callers sharing an index-derived sequence
    // are not sending each other's words at the same moment by construction.
    let mut sequence: u64 = seed;

    loop {
        if role == ROLE_WAITER {
            // The waiter role (milestone 221): `interrupt_wait` is the `Irq::WAIT` a driver uses,
            // and the kernel's soak build signals the route from `sched::on_tick` on every core. No
            // `interrupt_acknowledge`: there is no line and nothing was masked, so an ack would be
            // a real controller write for an interrupt that does not exist. A refusal
            // (`abi::Error`) stops the counting rather than retrying, and the kernel's stall check
            // reports it at the next beat, as every other failure here surfaces.
            if interrupt_wait(TICK_IRQ) < 0 {
                loop {
                    core::hint::spin_loop();
                }
            }
            wakes = wakes.wrapping_add(1);
            PAGE.write(wakes_at, wakes);
            continue;
        }
        if role == ROLE_GRINDER {
            for _ in 0..2048 {
                core::hint::spin_loop();
            }
        } else if role == ROLE_RESPONDER {
            // `receive_request` (a `RECEIVE_CAP`), not `receive`: a CALL arrives with a reply capability,
            // and answering it is what completes the caller's parked rendezvous. A responder that
            // used `receive` would leave every caller blocked forever, which is a hang this workload
            // would then report as a finding about the kernel.
            let req = receive_request(ENDPOINT);
            if let Some(to) = req.delivered.into_reply() {
                reply(to, soak_page::answer(req.w0), 0);
            } else {
                // Nobody to answer. Counted as a mismatch rather than ignored: a CALL that arrived
                // without its reply capability is exactly the shape of defect this workload is
                // looking for, and swallowing it would be the soak lying about itself.
                mismatches = mismatches.wrapping_add(1);
                PAGE.write(mismatches_at, mismatches);
            }
        } else {
            debug_assert!(role == ROLE_CALLER);
            sequence = sequence.wrapping_add(1);
            let (got, _) = call(ENDPOINT, sequence, 0);
            if got != soak_page::answer(sequence) {
                mismatches = mismatches.wrapping_add(1);
                PAGE.write(mismatches_at, mismatches);
            }
        }

        rounds = rounds.wrapping_add(1);
        PAGE.write(rounds_at, rounds);

        // The jitter. xorshift64, four instructions, no memory traffic; the low bits decide how
        // many spins to burn before the next round trip. 0..63 is enough to slide a pair's phase
        // against its neighbours without meaningfully lowering the round-trip rate.
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        for _ in 0..(state & 0x3f) {
            core::hint::spin_loop();
        }
    }
}

user_mode_runtime::panic_handler!();
