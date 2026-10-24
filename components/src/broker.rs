//! The queue broker, the latency ladder's opt-in middle rung (milestone 23, DECISIONS §41 (the
//! endpoint is the broker)).
//!
//! It stands between a producer and a backend so the producer's CALL never blocks on a swap:
//! pass-through when the backend is up (two round trips, the tax `broker_rtt` prices against
//! `call_reply`), and an immediate `ACCEPTED` with the item held when it is away. The backlog is
//! bounded and lives in this process's own `.bss`, so the kernel does not grow; a full queue
//! answers [`QUEUE_FULL`](hot_swap_contract::QUEUE_FULL). It writes nothing to storage and does
//! not survive its own crash. See notes/live-replacement.md, "The latency ladder".
//!
//! Name: ratified 2026-07-30 (calef, DECISIONS §39, landed by milestone 46), replacing `brokerd`.
//! Refused `brokerd` (the `-d` suffix claims the Unix process model DECISIONS §39 rejects, before a
//! reader has seen a line of code; that model is defined by the ambient authority this OS
//! deliberately lacks).

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

use user_mode_runtime::{call, reply, send};

// This is its own binary. What it shares with the other hot-swap programs comes from the
// `hot_swap_contract` crate.
// Shared code was a `#[path]` module compiled into each binary until rule 7 (2026-08-01) made it
// a crate, so this file needs no dead-code allow under §38 (a suppression is scoped to an item
// and carries a reason).

/// What `swapper` endowed us with, in order. No budget, no device, no way to build anything: a
/// compromised broker can reorder or drop the one channel it was placed on, and nothing else.
// Derived from this program's own declaration (`hot_swap_contract::BROKER`, milestone 23 (a capability-routed component OS with live replacement)'s manifest), so
// the slot a name lands in and the slot this code reads are one number rather than two that agree.
// `requests` is the only `Serve` here: a broker answers producers and asks its backend, and those
// two directions on one hop are the whole of what a queue rung is.
const FRONT: u64 = capability_plan::slot_of(&hot_swap_contract::BROKER, "requests");
const BACK: u64 = capability_plan::slot_of(&hot_swap_contract::BROKER, "backend");
const RPT: u64 = capability_plan::slot_of(&hot_swap_contract::BROKER, "report");
const NOTE: u64 = capability_plan::slot_of(&hot_swap_contract::BROKER, "operator");

/// How deep the backlog goes. Fixed storage in our own `.bss`, which lives in the region this
/// instance was built in, so the bound is a bound on *this process's* memory and the kernel's
/// footprint is unchanged. Sized to hold a whole run's worth of requests with room to spare, so a
/// full queue in the test means a bug rather than an undersized constant.
const CAPACITY: usize = 128;

struct Queue {
    items: [u64; CAPACITY],
    head: usize,
    count: usize,
}

impl Queue {
    fn push(&mut self, item: u64) -> bool {
        if self.count == CAPACITY {
            return false;
        }
        self.items[(self.head + self.count) % CAPACITY] = item;
        self.count += 1;
        true
    }

    fn pop(&mut self) -> Option<u64> {
        if self.count == 0 {
            return None;
        }
        let item = self.items[self.head];
        self.head = (self.head + 1) % CAPACITY;
        self.count -= 1;
        Some(item)
    }
}

user_mode_runtime::program_entry!(start);

fn start(_a0: u64, _a1: u64, _a2: u64) -> ! {
    let mut q = Queue {
        items: [0; CAPACITY],
        head: 0,
        count: 0,
    };
    // Pass-through until told otherwise. The broker does not probe the backend and cannot: liveness
    // is exactly the thing a synchronous rendezvous cannot report on, and guessing at it with a
    // timeout is the poor death detector DECISIONS §26 argued against. The operator, which is the
    // one that takes the backend away, is the one that says so.
    let mut up = true;
    let mut buffered = 0u64;
    send(RPT, hot_swap_contract::RPT_UP, 0, 0);

    loop {
        // **The warning is read on every wakeup** (DECISIONS §231 (a swap's warning to a dependent
        // is advisory, and the supervisor never waits for it)). A signal only wakes us when idle; a
        // request that arrives first finds the page already written, and a signal we never see
        // costs latency and nothing else.
        let got = user_mode_runtime::receive_request_bound(FRONT);
        let wants_down = hot_swap_contract::broker_wants_down();
        if wants_down && up {
            up = false;
        } else if !wants_down && !up {
            // Drain in arrival order before serving anything newer, so the backlog reaches the new
            // backend ahead of whatever the producer sends next.
            let mut drained = 0u64;
            while let Some(item) = q.pop() {
                let _ = call(BACK, hot_swap_contract::OPERATION_PUT, item);
                drained += 1;
            }
            up = true;
            send(RPT, hot_swap_contract::RPT_DRAINED, drained, buffered);
        }
        let Ok(req) = got else {
            continue; // the signal itself carries nothing; the page is the message
        };
        let (operation, arg) = (req.w0, req.w1);
        // The contract says CALL. Anything but the kernel's Reply has nobody to answer, and a
        // delegation is deleted rather than kept (milestone 706 (a `CALL` server can tell a Reply
        // from a delegation)).
        let Some(slot) = req.delivered.into_reply() else {
            continue;
        };
        match operation {
            hot_swap_contract::OPERATION_PUT if up => {
                // **Pass-through.** No copy, no queue, no scheduling policy: forward the two words
                // and hand the backend's own answer straight back. This is the steady state, and it
                // is the whole of the tax `broker_rtt` measures.
                let (r0, r1) = call(BACK, hot_swap_contract::OPERATION_PUT, arg);
                reply(slot, r0, r1);
            }
            hot_swap_contract::OPERATION_PUT => {
                // The backend is away. Take custody and answer immediately, so the producer keeps
                // running rather than parking on an endpoint nobody is receiving on.
                if q.push(arg) {
                    buffered += 1;
                    reply(slot, hot_swap_contract::ACCEPTED, q.count as u64);
                } else {
                    reply(slot, hot_swap_contract::QUEUE_FULL, q.count as u64);
                }
            }
            hot_swap_contract::OPERATION_QUIESCE => {
                send(RPT, hot_swap_contract::RPT_QUIESCED, 0, buffered);
                reply(slot, hot_swap_contract::QUIESCED, buffered);
                send(NOTE, hot_swap_contract::NOTE_BROKER_DONE, buffered, 0);
                user_mode_runtime::exit()
            }
            _ => {
                reply(slot, hot_swap_contract::BAD_REQUEST, 0);
            }
        }
    }
}

user_mode_runtime::panic_handler!();
