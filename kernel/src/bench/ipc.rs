//! **Synchronous IPC between kernel threads**: the round trip, a relay through a third thread,
//! `CALL`/`REPLY` and a broker.
//!
//! Cut out of `bench.rs` unchanged by lane `maintainer/last-three-over-2000`, which left the
//! parent an index (§282 (when a file is split, the parent becomes an index)) and under §266 (a
//! Rust source file stays under 2,000 lines).
//!
//! Name: provisional (lane `maintainer/last-three-over-2000`, 2026-10-12 UTC), for the module and
//! the file.

use super::*;

/// **Synchronous IPC round trip, the classic microkernel number.** A server loops
/// receive-then-send; the client times send-then-receive. One iteration is two rendezvous, two
/// mailbox copies, two wakes, two switches.
///
/// It is the receiver-first mode: every send finds its receiver already waiting, the path a server
/// loop lives on. The window's opening tick selects it, and the run fails if any send blocks.
pub(super) fn ipc_rtt() {
    let request = sched::create_rendezvous();
    let reply = sched::create_rendezvous();

    sched::spawn(move || {
        loop {
            let m = sched::ipc_receive(request);
            if m[0] == u64::MAX {
                break; // the client is done with us
            }
            sched::ipc_send(reply, [m[0], 0, 0]);
        }
    })
    .expect("bench: no server");

    for _ in 0..WARMUP {
        sched::ipc_send(request, [1, 0, 0]);
        sched::ipc_receive(reply);
    }
    let blocked = sched::sends_blocked();
    timed("ipc_rtt", IPC_ITERS, || {
        for _ in 0..IPC_ITERS {
            sched::ipc_send(request, [1, 0, 0]);
            sched::ipc_receive(reply);
        }
    });
    let blocked = sched::sends_blocked() - blocked;
    assert!(
        blocked == 0,
        "{blocked} sends blocked in ipc_rtt's window, so it ran sender-first; the row is the \
         receiver-first mode (calef, 2026-10-22 UTC; see sched::sends_blocked)",
    );
    sched::ipc_send(request, [u64::MAX, 0, 0]); // release the server
}

/// **The confined-server tax: a request routed through a server that fans out to a backend.** This
/// is the microkernel architecture's per-request cost that a monolith does not pay, and the topology
/// both real userspace servers use: the FS server CALLs the block server (`client -> fs -> blk -> fs
/// -> client`), `net_stack` CALLs the NIC driver (`client -> net_stack -> driver -> net_stack -> client`). Each
/// iteration here is that two-hop shape: the client sends to a relay, the relay forwards to a backend
/// and waits, the backend replies, the relay replies to the client. Two rendezvous become four, two
/// context switches become four.
///
/// Read it against `ipc_rtt` above (the one-hop client<->server round trip): the **difference** is
/// what one confined intermediary that delegates to a backend costs, the "server tax" a skeptic asks
/// about, isolated and deterministic. It is on the icount baseline for exactly that reason. The real
/// servers' end-to-end numbers are elsewhere and cannot be gated: `fs_read` (below) is device-latency
/// dominated (~200 us/block under HVF swamps this few-hundred-tick tax), and `net_stack`'s path is DHCP- and
/// timer-driven, neither deterministic under `-icount`. So this kernel-side topology bench is how the
/// server tax gets a gated regression number; see notes/benchmarks.md.
pub(super) fn relay_rtt() {
    let cl_req = sched::create_rendezvous(); // client -> relay
    let cl_reply = sched::create_rendezvous(); // relay -> client
    let bk_req = sched::create_rendezvous(); // relay -> backend
    let bk_reply = sched::create_rendezvous(); // backend -> relay

    // The backend: the leaf service. Receive a request, send a reply, until the sentinel.
    sched::spawn(move || {
        loop {
            let m = sched::ipc_receive(bk_req);
            if m[0] == u64::MAX {
                break;
            }
            sched::ipc_send(bk_reply, [m[0], 0, 0]);
        }
    })
    .expect("bench: no relay backend");

    // The relay: the confined intermediary. For each client request it does a full round trip to the
    // backend, then answers the client. On the sentinel it releases the backend and exits too.
    sched::spawn(move || {
        loop {
            let m = sched::ipc_receive(cl_req);
            if m[0] == u64::MAX {
                sched::ipc_send(bk_req, [u64::MAX, 0, 0]);
                break;
            }
            sched::ipc_send(bk_req, [m[0], 0, 0]);
            let r = sched::ipc_receive(bk_reply);
            sched::ipc_send(cl_reply, [r[0], 0, 0]);
        }
    })
    .expect("bench: no relay");

    for _ in 0..WARMUP {
        sched::ipc_send(cl_req, [1, 0, 0]);
        sched::ipc_receive(cl_reply);
    }
    timed("relay_rtt", RELAY_ITERS, || {
        for _ in 0..RELAY_ITERS {
            sched::ipc_send(cl_req, [1, 0, 0]);
            sched::ipc_receive(cl_reply);
        }
    });
    sched::ipc_send(cl_req, [u64::MAX, 0, 0]); // release the relay, which releases the backend
}

/// **Call/Reply round trip** (milestone 12): the one-endpoint shape real services use. One
/// iteration mints a one-shot Reply capability, rendezvouses, replies through it, consumes it.
pub(super) fn call_reply() {
    let ep = sched::create_rendezvous();

    sched::spawn(move || {
        loop {
            let m = sched::ipc_receive_cap(ep); // [word, reply_slot, word2]
            if m[0] == u64::MAX {
                break;
            }
            let slot = m[1];
            let crate::cap::Object::Reply(caller) = sched::current_cap(slot)
                .expect("bench: no reply cap")
                .object
            else {
                panic!("bench: RECEIVE_CAP of a CALL did not deliver a Reply capability");
            };
            sched::ipc_reply(caller, [m[0], 0]);
            let _ = sched::delete_current_cap(slot);
        }
    })
    .expect("bench: no call server");

    for _ in 0..WARMUP {
        sched::ipc_call(ep, [1, 0]);
    }
    timed("call_reply", CALL_ITERS, || {
        for _ in 0..CALL_ITERS {
            sched::ipc_call(ep, [1, 0]);
        }
    });
    // Release the server: it is parked in RECEIVE_CAP, and a plain SEND rendezvouses with it all
    // the same (the cap and plain paths share the wait queues), delivering the sentinel.
    sched::ipc_send(ep, [u64::MAX, 0, 0]);
}

/// **What a queue broker costs when both ends are up** (milestone 23, DECISIONS §41).
///
/// Milestone 23's latency ladder has two rungs built, and the rule that governs where each is used
/// is *opt-in per channel, never the default*. This benchmark is why that rule is a rule.
///
/// - **The default rung has no benchmark of its own, because it has no cost of its own.** A client
///   holds a capability to a stable endpoint and whoever is parked in `RECEIVE_CAP` on it answers; a
///   swap changes who that is. No process stands in the data path, so the steady state *is*
///   [`call_reply`] above, instruction for instruction, and the swap adds nothing to it. That is
///   the number to quote for milestone 23's flagship.
/// - **This is the opt-in rung**: `broker` interposed, so a producer never blocks on an absent
///   consumer. Read it against `call_reply`, which is deliberately the same client and the same
///   backend with nothing in between: the **difference** is the whole tax, and it is paid on every
///   request in the steady state, not only during a swap.
///
/// The topology is the CALL/reply idiom rather than `relay_rtt`'s SEND/RECEIVE pairs, because that is
/// what the broker actually speaks: the broker serves its front endpoint with `RECEIVE_CAP`, holds the
/// client's one-shot Reply capability while it CALLs the backend, and answers through it.
pub(super) fn broker_rtt() {
    let front = sched::create_rendezvous(); // client -> broker
    let back = sched::create_rendezvous(); // broker -> backend

    // The backend: the leaf service, answering through the Reply capability exactly as the direct
    // server in `call_reply` does.
    sched::spawn(move || {
        loop {
            let m = sched::ipc_receive_cap(back);
            if m[0] == u64::MAX {
                break;
            }
            reply_to(m[1], m[0]);
        }
    })
    .expect("bench: no broker backend");

    // The broker: pass-through. Both ends are up, so it buffers nothing; it forwards the request and
    // hands the backend's answer straight back.
    sched::spawn(move || {
        loop {
            let m = sched::ipc_receive_cap(front);
            if m[0] == u64::MAX {
                sched::ipc_send(back, [u64::MAX, 0, 0]);
                break;
            }
            let r = sched::ipc_call(back, [m[0], 0]);
            reply_to(m[1], r[0]);
        }
    })
    .expect("bench: no broker");

    for _ in 0..WARMUP {
        sched::ipc_call(front, [1, 0]);
    }
    timed("broker_rtt", CALL_ITERS, || {
        for _ in 0..CALL_ITERS {
            sched::ipc_call(front, [1, 0]);
        }
    });
    sched::ipc_send(front, [u64::MAX, 0, 0]); // releases the broker, which releases the backend
}

/// Answer through the one-shot Reply capability `RECEIVE_CAP` delivered, and consume it.
fn reply_to(slot: u64, word: u64) {
    let crate::cap::Object::Reply(caller) = sched::current_cap(slot)
        .expect("bench: no reply cap")
        .object
    else {
        panic!("bench: RECEIVE_CAP of a CALL did not deliver a Reply capability");
    };
    sched::ipc_reply(caller, [word, 0]);
    let _ = sched::delete_current_cap(slot);
}
