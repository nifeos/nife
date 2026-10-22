//! **The benchmarks measured from EL0**, through the `os_primitives_benchmarker` program: a null
//! syscall, a switch, IPC, pipe throughput, mapping and spawning.
//!
//! Cut out of `bench.rs` unchanged by lane `maintainer/last-three-over-2000`, which left the
//! parent an index (§282 (when a file is split, the parent becomes an index)) and under §266 (a
//! Rust source file stays under 2,000 lines).
//!
//! Name: provisional (lane `maintainer/last-three-over-2000`, 2026-10-12 UTC), for the module and
//! the file.

use super::*;

// Roles for the `os_primitives_benchmarker` EL0 program (must match fixtures/src/os_primitives_benchmarker.rs). One binary, one micro-
// measurement per role, chosen through `START`'s `arg0`.
const EL_NULL_SYSCALL: u64 = 0;
const EL_YIELDER: u64 = 1;
const EL_CTX_SWITCH: u64 = 2;
const EL_IPC_SERVER: u64 = 3;
const EL_IPC_CLIENT: u64 = 4;
const EL_MAP: u64 = 5;
const EL_SPAWN: u64 = 6;
const EL_SINK_PRODUCER: u64 = 7;
const EL_SINK_CONSUMER: u64 = 8;

/// The spawner's untyped budget, in pages. **Small on purpose:** each child is split, run, and
/// destroyed strictly LIFO, so its pages return to this budget (DECISIONS §16), and only one child
/// lives at a time. The budget need only fund one child (`CHILD_PAGES`) plus the shared code frame
/// and the scratch page tables, not one per iteration. That 64 funds a 100-iteration loop (which
/// would need >1000 pages without return-to-parent) is itself the proof LIFO reclaim works.
const SPAWN_EL0_BUDGET: u64 = 64;

/// Warmup + timed map counts the bench boot must provision the target region for. `MAP_EL0_ITERS`
/// **must equal** `os_primitives_benchmarker`'s `MAP_ITERS` and `MAP_EL0_WARMUP` its `MAP_WARMUP`: the region is sized for
/// their sum plus page-table and record overhead, and if `os_primitives_benchmarker` asks for more maps than the region
/// funds, the surplus fail (a cheap error return, not a real map) and skew the number. Kept here
/// rather than shared because the two crates have no common header, the way EL_* mirror ROLE_*.
const MAP_EL0_ITERS: u64 = 500;
const MAP_EL0_WARMUP: u64 = 8;
const MAP_EL0_OVERHEAD: u64 = 32;

/// Spawn the `os_primitives_benchmarker` EL0 program in a given role, granting it `report` (slot 0) to answer on.
/// `false` if there is no `os_primitives_benchmarker` in the initial ramdisk (the bench boot then
/// skips that line).
fn spawn_os_primitives_benchmarker(role: u64, report: sched::RendezvousId) -> bool {
    let Some(image) = crate::trust::require_program("os_primitives_benchmarker") else {
        return false;
    };
    sched::spawn(move || {
        crate::user::run(
            image,
            crate::user::Spawn {
                arg0: role,
                arg1: 0,
                arg2: 0,
                grants: &[rendezvous_cap(report, Rights::WRITE)],
                maps: &[],
            },
        )
    })
    .expect("bench: could not spawn os_primitives_benchmarker");
    true
}

/// **Null syscall latency, measured from EL0 (the primitive suite).** The `bench:` lines above are
/// kernel-internal, no trap. This one is what lmbench measures: the bench boot spawns the `os_primitives_benchmarker`
/// EL0 program, which self-times a loop of the cheapest `svc` and reports `[ticks, iters]`; we print
/// it in the same format. The gap between this and a hypothetical kernel-side null syscall is roughly
/// the EL0<->EL1 boundary cost, which is the whole point of measuring here. See `fixtures/src/os_primitives_benchmarker.rs`.
pub(super) fn null_syscall_el0() {
    let report = sched::create_rendezvous();
    let window = TickWindow::open("null_syscall");
    if !spawn_os_primitives_benchmarker(EL_NULL_SYSCALL, report) {
        window.close();
        println!(
            "bench: null_syscall skipped (no os_primitives_benchmarker in the initial ramdisk)"
        );
        return;
    }
    let [ticks, iters, ..] = sched::ipc_receive(report);
    window.close();
    println!("bench: null_syscall {ticks} {iters}");
}

/// **Context switch latency, measured from EL0 (the primitive suite).** lmbench's `lat_ctx`. The
/// bench boot spawns a *yielder* peer and a *timer*, two separate EL0 processes; the timer self-times
/// a loop of `SYS_YIELD`, each handing the CPU to the peer and back, two switches per iteration, each
/// an address-space change. With the boot thread blocked here on the report and only those two ready,
/// the alternation is clean. See `fixtures/src/os_primitives_benchmarker.rs`.
pub(super) fn ctx_switch_el0() {
    let report = sched::create_rendezvous();
    // The peer first, so the timer always has something to switch to. It shares the report endpoint
    // (it never sends on it); the spawn shape stays uniform.
    let window = TickWindow::open("ctx_switch");
    if !spawn_os_primitives_benchmarker(EL_YIELDER, report) {
        window.close();
        println!("bench: ctx_switch skipped (no os_primitives_benchmarker in the initial ramdisk)");
        return;
    }
    if !spawn_os_primitives_benchmarker(EL_CTX_SWITCH, report) {
        window.close();
        return;
    }
    let [ticks, iters, ..] = sched::ipc_receive(report);
    window.close();
    println!("bench: ctx_switch {ticks} {iters}");
}

/// **IPC round-trip latency, measured from EL0 (the primitive suite).** lmbench's `lat_pipe`. Two
/// EL0 processes and two endpoints: a server (RECEIVE request, SEND reply) and a client that self-times
/// a loop of SEND-then-RECEIVE and reports. The server is spawned first so a request always meets a
/// waiting receiver. Grants differ per role, so the spawns are inline rather than via `spawn_os_primitives_benchmarker`.
pub(super) fn ipc_rtt_el0() {
    let Some(image) = crate::trust::require_program("os_primitives_benchmarker") else {
        println!("bench: ipc_rtt skipped (no os_primitives_benchmarker in the initial ramdisk)");
        return;
    };
    let request = sched::create_rendezvous();
    let reply = sched::create_rendezvous();
    let report = sched::create_rendezvous();

    let window = TickWindow::open("ipc_rtt_el0");
    sched::spawn(move || {
        crate::user::run(
            image,
            crate::user::Spawn {
                arg0: EL_IPC_SERVER,
                arg1: 0,
                arg2: 0,
                grants: &[
                    rendezvous_cap(request, Rights::READ), // slot 0: RECEIVE requests
                    rendezvous_cap(reply, Rights::WRITE),  // slot 1: SEND replies
                ],
                maps: &[],
            },
        )
    })
    .expect("bench: could not spawn the ipc server");

    sched::spawn(move || {
        crate::user::run(
            image,
            crate::user::Spawn {
                arg0: EL_IPC_CLIENT,
                arg1: 0,
                arg2: 0,
                grants: &[
                    rendezvous_cap(report, Rights::WRITE), // slot 0: report the result
                    rendezvous_cap(request, Rights::WRITE), // slot 1: SEND requests
                    rendezvous_cap(reply, Rights::READ),   // slot 2: RECEIVE replies
                ],
                maps: &[],
            },
        )
    })
    .expect("bench: could not spawn the ipc client");

    let [ticks, iters, ..] = sched::ipc_receive(report);
    window.close();
    // Distinct from the kernel-side `ipc_rtt` above: this one crosses the EL0<->EL1 boundary on every
    // send and receive, which is the whole point (comparable to lmbench). The gap between them is roughly
    // the trap cost of the four svcs per round trip.
    println!("bench: ipc_rtt_el0 {ticks} {iters}");
}

/// **`a | b` throughput, measured from EL0** (milestone 50, notes/pipes.md).
///
/// Two EL0 processes and one endpoint, which is literally what a pipeline is here: the shell mints an
/// endpoint, gives the left stage `WRITE` and the right stage `READ`, and there is no object in
/// between. The producer packs sixteen bytes into a sink message and `SEND`s; the consumer `RECEIVE`s
/// and self-times. The reported pair is `[ticks, bytes]` rather than `[ticks, iters]`, because bytes
/// is the number a Unix pipe can be compared against, which is the whole reason this exists: the
/// design note said measure the lockstep before deciding anything about buffering.
///
/// The comparison is `bench/host/pipe_throughput.rs`, and it deliberately measures a Unix pipe
/// **twice**: once with the same sixteen-byte writes, which isolates the cost of having no buffer,
/// and once with the 64 KiB writes a real Unix program would use, which is what Unix actually gets.
/// Only the first pair is apples to apples.
pub(super) fn sink_throughput() {
    let Some(image) = crate::trust::require_program("os_primitives_benchmarker") else {
        println!(
            "bench: sink_throughput skipped (no os_primitives_benchmarker in the initial ramdisk)"
        );
        return;
    };
    let pipe = sched::create_rendezvous();
    let report = sched::create_rendezvous();

    // The producer first, so the consumer's first `RECEIVE` meets a waiting sender rather than the
    // other way round. Either order works (a rendezvous blocks whichever side arrives first) and
    // this one keeps the warmup honest: the consumer's timed loop starts with the pipe already hot.
    let window = TickWindow::open("sink_throughput");
    sched::spawn(move || {
        crate::user::run(
            image,
            crate::user::Spawn {
                arg0: EL_SINK_PRODUCER,
                arg1: 0,
                arg2: 0,
                // **`WRITE` and nothing else**, which is exactly what the shell delegates to the
                // left of a `|`. It cannot read back up its own output.
                grants: &[rendezvous_cap(pipe, Rights::WRITE)],
                maps: &[],
            },
        )
    })
    .expect("bench: could not spawn the sink producer");

    sched::spawn(move || {
        crate::user::run(
            image,
            crate::user::Spawn {
                arg0: EL_SINK_CONSUMER,
                arg1: 0,
                arg2: 0,
                grants: &[
                    rendezvous_cap(report, Rights::WRITE), // slot 0: report the result
                    rendezvous_cap(pipe, Rights::READ),    // slot 1: the pipe's read end
                ],
                maps: &[],
            },
        )
    })
    .expect("bench: could not spawn the sink consumer");

    let [ticks, bytes, ..] = sched::ipc_receive(report);
    window.close();
    println!("bench: sink_throughput {ticks} {bytes}");
}

/// **Map latency, measured from EL0 (the primitive suite).** lmbench's `lat_mmap`. Unlike the three
/// above, the map path *consumes* resources per call, so the bench boot builds the target here rather
/// than granting an endpoint: a fresh registry address space (its own untyped region as budget) and a
/// single frame. `os_primitives_benchmarker` (`EL_MAP`) is granted a WRITE cap on that space and a READ cap on the frame,
/// then times a loop of `invoke(address space, MAP_INTO, va_i, frame, MAP_RO)`, aliasing the one frame at a
/// fresh VA each iteration. The target is a separate space, not `os_primitives_benchmarker`'s own (a run()-adopted space
/// is not in the registry `MAP_INTO` resolves), which is immaterial: the map path's cost is the same
/// whoever owns the space. See `fixtures/src/os_primitives_benchmarker.rs`.
pub(super) fn map_el0() {
    let Some(image) = crate::trust::require_program("os_primitives_benchmarker") else {
        println!("bench: map_el0 skipped (no os_primitives_benchmarker in the initial ramdisk)");
        return;
    };
    // The target space, backed by its own region. The region pays for the root, the intermediate
    // tables, and the mapping-record log pages; the leaves are aliases of one frame, so they cost it
    // nothing. Sized for the warmup plus timed maps plus that overhead.
    let Some(region) =
        crate::memory_region::create(MAP_EL0_WARMUP + MAP_EL0_ITERS + MAP_EL0_OVERHEAD)
    else {
        println!("bench: map_el0 skipped (no region)");
        return;
    };
    let Some(name) = crate::user::user_address_space_create(region) else {
        println!("bench: map_el0 skipped (no address space)");
        return;
    };
    // One frame to alias-map, from its own one-page region so the address space region stays pure overhead.
    let Some(frame_region) = crate::memory_region::create(1) else {
        println!("bench: map_el0 skipped (no frame region)");
        return;
    };
    let Some(phys) = crate::memory_region::retype_page(frame_region) else {
        println!("bench: map_el0 skipped (no frame)");
        return;
    };

    let report = sched::create_rendezvous();
    let window = TickWindow::open("map_el0");
    sched::spawn(move || {
        crate::user::run(
            image,
            crate::user::Spawn {
                arg0: EL_MAP,
                arg1: 0,
                arg2: 0,
                grants: &[
                    rendezvous_cap(report, Rights::WRITE), // slot 0: report the result
                    address_space_cap(name, Rights::WRITE), // slot 1: the space we map into
                    page_frame_cap(phys, Rights::READ),    // slot 2: the frame we alias-map
                ],
                maps: &[],
            },
        )
    })
    .expect("bench: could not spawn the map bencher");

    let [ticks, iters, ..] = sched::ipc_receive(report);
    window.close();
    println!("bench: map_el0 {ticks} {iters}");
}

/// **Spawn latency, measured from EL0 (the primitive suite).** lmbench's `lat_proc`, and the payoff
/// of object revocation: a userspace spawner builds a whole child from EL0 through the granular verbs
/// and, crucially, `DESTROY`s the child's region afterward, so the loop repeats. The bench boot hands
/// the spawner three things: a big untyped budget (slot 1), a report endpoint to answer on (slot 0),
/// and a child-done endpoint (slot 2, READ|WRITE|GRANT) it delegates a WRITE view of to each child.
/// See `fixtures/src/os_primitives_benchmarker.rs`.
pub(super) fn spawn_el0() {
    let Some(image) = crate::trust::require_program("os_primitives_benchmarker") else {
        println!("bench: spawn_el0 skipped (no os_primitives_benchmarker in the initial ramdisk)");
        return;
    };
    let Some(region) = crate::memory_region::create(SPAWN_EL0_BUDGET) else {
        println!("bench: spawn_el0 skipped (no budget)");
        return;
    };
    let report = sched::create_rendezvous();
    let child_done = sched::create_rendezvous();
    let window = TickWindow::open("spawn_el0");
    sched::spawn(move || {
        crate::user::run(
            image,
            crate::user::Spawn {
                arg0: EL_SPAWN,
                arg1: 0,
                arg2: 0,
                grants: &[
                    rendezvous_cap(report, Rights::WRITE), // slot 0: report the result home
                    crate::cap::memory_region_lending_capability(region), // slot 1: the spawner's whole budget
                    rendezvous_cap(
                        child_done,
                        Rights::READ.union(Rights::WRITE).union(Rights::GRANT),
                    ), // slot 2: children signal done; spawner receives and delegates WRITE
                ],
                maps: &[],
            },
        )
    })
    .expect("bench: could not spawn the spawner");
    let [ticks, iters, ..] = sched::ipc_receive(report);
    window.close();
    println!("bench: spawn_el0 {ticks} {iters}");
}
