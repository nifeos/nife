//! **The benchmarks measured from EL0**, through the `os_primitives_benchmarker` program: a null
//! syscall, a switch, IPC, pipe throughput, mapping and spawning.
//!
//! Name: provisional (lane `maintainer/last-three-over-2000`, 2026-10-12 UTC), for the module and
//! the file.

use super::*;

// Roles for the `os_primitives_benchmarker` EL0 program, chosen through `arg0`; they must match
// fixtures/src/os_primitives_benchmarker.rs.
const EL_NULL_SYSCALL: u64 = 0;
const EL_YIELDER: u64 = 1;
const EL_CTX_SWITCH: u64 = 2;
const EL_IPC_SERVER: u64 = 3;
const EL_IPC_CLIENT: u64 = 4;
const EL_MAP: u64 = 5;
const EL_SPAWN: u64 = 6;
const EL_SINK_PRODUCER: u64 = 7;
const EL_SINK_CONSUMER: u64 = 8;

/// `arg1` for the IPC client when it holds the cycle-counter grant: it reads the counter across its
/// timed loop and sends a second report. It must equal `IPC_CLIENT_READS_CYCLES` in
/// `fixtures/src/os_primitives_benchmarker.rs`; the two crates share no header.
const IPC_CLIENT_READS_CYCLES: u64 = 1;

/// The spawner's untyped budget, in pages: one child plus the shared code frame and scratch tables,
/// since children are destroyed LIFO and return their pages (§16 (object revocation: reclaim the
/// objects a process built)). See notes/benchmarks/kernel-bench-harness.md.
const SPAWN_EL0_BUDGET: u64 = 64;

/// Warmup and timed map counts the target region is provisioned for. They must equal
/// `os_primitives_benchmarker`'s `MAP_ITERS` and `MAP_WARMUP`, or the surplus maps fail cheaply
/// and skew the row; the two crates share no header.
const MAP_EL0_ITERS: u64 = 500;
const MAP_EL0_WARMUP: u64 = 8;
const MAP_EL0_OVERHEAD: u64 = 32;

/// Spawn `os_primitives_benchmarker` in `role`, granting it `report` (slot 0) to answer on.
/// Returns `false` if the initial ramdisk has no such program, and the row is skipped.
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

/// Null syscall latency, measured from EL0: the program self-times the cheapest `svc` and reports
/// `[ticks, iters]`. This is what lmbench measures. See
/// `fixtures/src/os_primitives_benchmarker.rs`.
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

/// Context switch latency, measured from EL0 (lmbench's `lat_ctx`): a timer process yields to a
/// yielder peer and back, two address-space switches per iteration, while the boot thread blocks.
pub(super) fn ctx_switch_el0() {
    let report = sched::create_rendezvous();
    // The peer first, so the timer always has something to switch to.
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

/// IPC round-trip latency, measured from EL0 (lmbench's `lat_pipe`): a server and a client that
/// self-times SEND-then-RECEIVE. The server is spawned first so a request meets a waiting receiver.
/// Its gap to `ipc_rtt` is roughly the four `svc` traps per round trip.
///
/// Under `cycle_counter_grant` the client is also granted the cycle counter and reads it across the
/// same loop, and a `bench-probe: ipc_rtt_el0_cycles` line follows the row: the round trip in the
/// counter's own units, read rather than converted (milestone 101). The row itself does not change.
pub(super) fn ipc_rtt_el0() {
    let Some(image) = crate::trust::require_program("os_primitives_benchmarker") else {
        println!("bench: ipc_rtt skipped (no os_primitives_benchmarker in the initial ramdisk)");
        return;
    };
    let request = sched::create_rendezvous();
    let reply = sched::create_rendezvous();
    let report = sched::create_rendezvous();

    // Decided before anything is spawned: a granted read that traps kills the client, and the
    // report this function waits for would never come.
    #[cfg(feature = "cycle_counter_grant")]
    let cycle_refusal = el0_cycle_counter_refusal();
    #[cfg(feature = "cycle_counter_grant")]
    let reads_cycles = cycle_refusal.is_none();
    #[cfg(not(feature = "cycle_counter_grant"))]
    let reads_cycles = false;

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
        #[cfg(feature = "cycle_counter_grant")]
        if reads_cycles {
            sched::grant_cycle_counter_to_current();
        }
        crate::user::run(
            image,
            crate::user::Spawn {
                arg0: EL_IPC_CLIENT,
                arg1: if reads_cycles {
                    IPC_CLIENT_READS_CYCLES
                } else {
                    0
                },
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
    println!("bench: ipc_rtt_el0 {ticks} {iters}");

    #[cfg(feature = "cycle_counter_grant")]
    match cycle_refusal {
        Some(why) => println!("bench-probe: ipc_rtt_el0_cycles unavailable ({why})"),
        None => {
            let [cycles, meaning, ..] = sched::ipc_receive(report);
            print_ipc_rtt_el0_cycles(cycles, meaning, ticks, iters);
        }
    }
}

/// Why the `ipc_rtt_el0` client cannot be given a cycle counter whose difference means anything,
/// or `None` when it can.
///
/// One core only, because aarch64 and riscv64 counters are per core and the client may move
/// between reads on a machine with more; `--real` and `board-image --bench` boot one. On aarch64
/// the core must have a running `PMCCNTR_EL0`. On riscv64 the kernel's counter must be the `cycle`
/// CSR itself, which proves firmware lets a lower mode read it. `x86_64`'s TSC is ambient.
#[cfg(feature = "cycle_counter_grant")]
fn el0_cycle_counter_refusal() -> Option<&'static str> {
    if crate::smp::online_cpus().count() != 1 {
        return Some("more than one core online, and the cycle counters are per core");
    }
    #[cfg(target_arch = "aarch64")]
    if !crate::arch::timer::is_cycle_counter_grantable()
        || crate::arch::pmu::outcome() != crate::arch::pmu::CycleCounter::Running
    {
        return Some("this core has no running PMCCNTR_EL0 to grant");
    }
    #[cfg(target_arch = "riscv64")]
    if !crate::arch::pmu::counts_through_the_cycle_csr() {
        return Some(
            "the kernel's cycle counter is not the `cycle` CSR a granted U-mode read uses",
        );
    }
    None
}

/// Print the `ipc_rtt_el0` client's own cycle count: per round trip in hundredths, the raw inputs,
/// and cycles per tick over the same window, which should agree with the `cycles_per_tick` probe
/// earlier in the boot. A disagreement means the core's clock moved between the two (frequency
/// scaling), and this line, read across the loop itself, is the one to believe. A second line says
/// what the client's counter counts, as the client reported it. See
/// notes/benchmarks/kernel-bench-harness.md.
#[cfg(feature = "cycle_counter_grant")]
fn print_ipc_rtt_el0_cycles(cycles: u64, meaning: u64, ticks: u64, iters: u64) {
    if cycles == 0 || cycles > u64::MAX / 200 {
        println!(
            "bench-probe: ipc_rtt_el0_cycles unavailable (the counter read {cycles} across the loop)"
        );
        return;
    }
    let per_iter = (cycles * 100).checked_div(iters).unwrap_or(0);
    let per_tick = (cycles * 100).checked_div(ticks).unwrap_or(0);
    println!(
        "bench-probe: ipc_rtt_el0_cycles {}.{:02} per round trip ({cycles} cycles over {iters} \
         iters; {}.{:02} per tick across the same {ticks} ticks)",
        per_iter / 100,
        per_iter % 100,
        per_tick / 100,
        per_tick % 100,
    );
    let means = match abi::cycle_counter::CycleMeaning::from_word(meaning) {
        Some(abi::cycle_counter::CycleMeaning::CoreCyclesUserAndKernel) => {
            "core cycles at EL0 and EL1 (PMCCNTR_EL0, PMCCFILTR_EL0 0)"
        }
        Some(abi::cycle_counter::CycleMeaning::CoreCyclesEveryMode) => {
            "core cycles in every mode (the `cycle` CSR)"
        }
        Some(abi::cycle_counter::CycleMeaning::ConstantRateReferenceCycles) => {
            "constant-rate reference cycles (the TSC), not core cycles"
        }
        None => "nothing: the client reported no meaning",
    };
    println!("bench-probe: ipc_rtt_el0_cycles_means {means}");
}

/// `a | b` throughput, measured from EL0: a producer with `WRITE` and a consumer with `READ` on one
/// endpoint, sixteen bytes a message, reported as `[ticks, bytes]`.
///
/// See notes/pipes.md, and notes/benchmarks/kernel-bench-harness.md for the Unix comparison.
pub(super) fn sink_throughput() {
    let Some(image) = crate::trust::require_program("os_primitives_benchmarker") else {
        println!(
            "bench: sink_throughput skipped (no os_primitives_benchmarker in the initial ramdisk)"
        );
        return;
    };
    let pipe = sched::create_rendezvous();
    let report = sched::create_rendezvous();

    // The producer first, so the consumer's timed loop starts with the pipe already hot.
    let window = TickWindow::open("sink_throughput");
    sched::spawn(move || {
        crate::user::run(
            image,
            crate::user::Spawn {
                arg0: EL_SINK_PRODUCER,
                arg1: 0,
                arg2: 0,
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

/// Map latency, measured from EL0 (lmbench's `lat_mmap`): the program times `MAP_INTO` of one frame
/// at a fresh address each iteration, into a separate registry address space built here.
///
/// The program gets a WRITE capability on the space and a READ capability on the frame. The space
/// is not its own, because a `run()`-adopted space is not in the registry `MAP_INTO` resolves; the
/// map path costs the same whoever owns it. See `fixtures/src/os_primitives_benchmarker.rs`.
pub(super) fn map_el0() {
    let Some(image) = crate::trust::require_program("os_primitives_benchmarker") else {
        println!("bench: map_el0 skipped (no os_primitives_benchmarker in the initial ramdisk)");
        return;
    };
    // The region pays for the tables and record pages; the leaves alias one frame.
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
    // From its own region, so the address space's region is pure overhead.
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

/// Spawn latency, measured from EL0 (lmbench's `lat_proc`): a userspace spawner builds each child
/// through the granular verbs and `DESTROY`s its region, so the loop repeats.
///
/// The spawner gets a report endpoint (slot 0), an untyped budget (slot 1) and a child-done
/// endpoint (slot 2, READ|WRITE|GRANT) it delegates a WRITE view of. See
/// `fixtures/src/os_primitives_benchmarker.rs`.
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
