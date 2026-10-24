//! **Synchronous IPC between kernel threads**: the round trip, a relay through a third thread,
//! `CALL`/`REPLY` and a broker.
//!
//! Name: provisional (lane `maintainer/last-three-over-2000`, 2026-10-12 UTC), for the module and
//! the file.

use super::*;

/// Synchronous IPC round trip between two kernel threads: two rendezvous, two copies, two wakes and
/// two switches per iteration.
///
/// It measures the receiver-first mode, which the window's opening tick selects; the run fails if
/// any send blocks.
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

/// The confined-server tax: a request routed through a relay that calls a backend, the shape the
/// FS server and `net_stack` use. Four rendezvous and four switches per iteration.
///
/// Its difference from [`ipc_rtt`] is what one confined intermediary costs, deterministic and on
/// the icount baseline. See notes/benchmarks/kernel-bench-harness.md.
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

    // The relay: a full round trip to the backend per request; on the sentinel, release it too.
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

/// What a queue broker costs when both ends are up, §41 (the endpoint is the broker).
///
/// The broker serves its front endpoint with `RECEIVE_CAP`, holds the client's one-shot Reply
/// capability while it CALLs the backend, and answers through it. Its difference from
/// [`call_reply`], the same client and backend with nothing between, is the opt-in rung's whole
/// tax. See notes/benchmarks/kernel-bench-harness.md.
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

    // The broker: both ends are up, so it buffers nothing.
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
