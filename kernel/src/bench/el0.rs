//! **The benchmarks measured from EL0**, through the `os_primitives_benchmarker` program: a null
//! syscall, a switch, IPC, pipe throughput, mapping and spawning.
//!
//! Name: provisional (lane `maintainer/last-three-over-2000`, 2026-10-12 UTC), for the module and
//! the file.

use super::*;

// Roles for the `os_primitives_benchmarker` EL0 program, chosen through `arg0`; they must match
// fixtures/src/os_primitives_benchmarker.rs.
const EL_YIELDER: u64 = 1;
const EL_CTX_SWITCH: u64 = 2;
const EL_IPC_SERVER: u64 = 3;
const EL_IPC_CLIENT: u64 = 4;
const EL_SPAWN: u64 = 6;

/// `arg1` for the IPC client when it holds the cycle-counter grant: it reads the counter across its
/// timed loop and sends a second report. It must equal `IPC_CLIENT_READS_CYCLES` in
/// `fixtures/src/os_primitives_benchmarker.rs`; the two crates share no header.
const IPC_CLIENT_READS_CYCLES: u64 = 1;

/// The spawner's untyped budget, in pages: one child plus the shared code frame and scratch tables,
/// since children are destroyed LIFO and return their pages (§16 (object revocation: reclaim the
/// objects a process built)). See notes/benchmarks/kernel-bench-harness.md.
const SPAWN_EL0_BUDGET: u64 = 64;

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
                grants: &[rendezvous_capability(report, Rights::WRITE)],
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
/// What a shell-runnable bench program holds in slot 1, after its output in slot 0.
#[derive(Clone, Copy)]
enum SlotOne {
    /// Nothing.
    Empty,
    /// The read end of a pipe, as a stage of a pipeline holds its input.
    Input(sched::RendezvousId),
    /// A memory region the program builds from, as the shell's `--mem` hands one over.
    Budget(u64),
}

/// Spawn one of the programs a production shell also runs (`null_syscall`, `sink_producer`,
/// `sink_consumer`, `map_el0`), with the quiet-boot startup word so its row reads `bench: ...`.
/// Slot 0 is its output (`WRITE`), the layout the shell builds. `None` when the initial ramdisk
/// lacks the program.
fn spawn_shell_runnable_program(
    name: &'static str,
    output: sched::RendezvousId,
    slot_one: SlotOne,
) -> Option<()> {
    let image = crate::trust::require_program(name)?;
    sched::spawn(move || {
        let output = rendezvous_capability(output, Rights::WRITE);
        let both = match slot_one {
            SlotOne::Empty => None,
            SlotOne::Input(input) => Some([output, rendezvous_capability(input, Rights::READ)]),
            SlotOne::Budget(region) => Some([
                output,
                crate::capability_object::memory_region_lending_capability(region),
            ]),
        };
        let only_output = [output];
        crate::user::run(
            image,
            crate::user::Spawn {
                arg0: primitive_benchmark_contract::QUIET_BOOT_STARTUP_WORD,
                arg1: 0,
                arg2: 0,
                grants: match &both {
                    Some(both) => both,
                    None => &only_output,
                },
                maps: &[],
            },
        )
    })
    .expect("bench: could not spawn a shell-runnable bench program");
    Some(())
}

/// Print the sink-contract text a bench program writes on `report` until its end of stream.
///
/// The program formats its own row (`bench: null_syscall <ticks> <iters>`); the kernel only
/// relays the bytes, so the row and the line a person gets at the shell cannot differ.
fn relay_row(report: sched::RendezvousId) {
    loop {
        let [w0, w1, w2, ..] = sched::ipc_receive(report);
        let mut bytes = [0u8; byte_sink_contract::INLINE_MAXIMUM];
        match byte_sink_contract::unpack(w0, w1, w2, &mut bytes) {
            byte_sink_contract::Msg::Bytes(n) => {
                for &b in &bytes[..n] {
                    crate::print!("{}", b as char);
                }
            }
            byte_sink_contract::Msg::Eof | byte_sink_contract::Msg::Malformed => return,
        }
    }
}

pub(super) fn null_syscall_el0() {
    let report = sched::create_rendezvous();
    let window = TickWindow::open("null_syscall");
    if spawn_shell_runnable_program("null_syscall", report, SlotOne::Empty).is_none() {
        window.close();
        println!("bench: null_syscall skipped (no null_syscall in the initial ramdisk)");
        return;
    }
    relay_row(report);
    window.close();
}

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
/// counter's own units, read rather than converted (milestone 101 (the L4 calibration)). The row itself does not change.
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
                    rendezvous_capability(request, Rights::READ), // slot 0: RECEIVE requests
                    rendezvous_capability(reply, Rights::WRITE),  // slot 1: SEND replies
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
                    rendezvous_capability(report, Rights::WRITE), // slot 0: report the result
                    rendezvous_capability(request, Rights::WRITE), // slot 1: SEND requests
                    rendezvous_capability(reply, Rights::READ),   // slot 2: RECEIVE replies
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
    let pipe = sched::create_rendezvous();
    let report = sched::create_rendezvous();

    let window = TickWindow::open("sink_throughput");
    if spawn_shell_runnable_program("sink_producer", pipe, SlotOne::Empty).is_none() {
        window.close();
        println!("bench: sink_throughput skipped (no sink_producer in the initial ramdisk)");
        return;
    }
    if spawn_shell_runnable_program("sink_consumer", report, SlotOne::Input(pipe)).is_none() {
        window.close();
        println!("bench: sink_throughput skipped (no sink_consumer in the initial ramdisk)");
        return;
    }
    relay_row(report);
    window.close();
}

pub(super) fn map_el0() {
    let Some(region) = crate::memory_region::create(primitive_benchmark_contract::MAP_BUDGET_PAGES)
    else {
        println!("bench: map_el0 skipped (no budget)");
        return;
    };
    let report = sched::create_rendezvous();
    let window = TickWindow::open("map_el0");
    if spawn_shell_runnable_program("map_el0", report, SlotOne::Budget(region)).is_none() {
        window.close();
        println!("bench: map_el0 skipped (no map_el0 in the initial ramdisk)");
        return;
    }
    relay_row(report);
    window.close();
}

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
                    rendezvous_capability(report, Rights::WRITE), // slot 0: report the result home
                    crate::capability_object::memory_region_lending_capability(region), // slot 1: the spawner's whole budget
                    rendezvous_capability(
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
