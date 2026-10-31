//! EL0 microbenchmarks, measured the lmbench way (the cross-OS primitive suite, milestone 21+).
//!
//! The kernel threads of `kernel/src/bench.rs` never pay the EL0->EL1 trap; lmbench measures from
//! userspace, trap included, so this program self-times loops of real `svc` syscalls at EL0.
//! Spawned by the bench boot, it times each primitive with `user_mode_runtime::now` (the virtual
//! counter, EL0-readable since milestone 19e (run a real workload)) and SENDs `[ticks, iters]` on
//! slot 0, so the icount baseline gates it and `--real` gives its magnitude. Four primitives: context
//! switch, IPC round trip, page map, and spawn, which reclaims each child's region
//! (`MemoryRegion::DESTROY`). The null syscall and the pipe throughput are programs of their own
//! (`null_syscall`, `sink_producer`, `sink_consumer`), because a person can type them at the
//! production shell; they need no peer this program would have to build. See notes/benchmarks/cross-os-primitives.md.
//!

//! Name: ratified 2026-08-01 (calef, milestone 63), replacing `elbench`, and it is the name that
//! forced `nifefs`'s `NAME_LEN` from 24 to 32. The raise was taken first and on its own merits,
//! deliberately, because choosing a worse name to fit a limit and raising a limit because a name
//! demanded it are both the wrong way round. Refused `elbench` (`el` is aarch64-only vocabulary for
//! a program that runs on both ISAs, since RISC-V has U, S and M modes and no exception levels) and
//! `os_primitives_benchmark` (this tree already uses "benchmark" for the output:
//! `bench/baseline.txt` holds the numbers, so the agent noun names the producer).

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

use user_mode_runtime::{
    address_space_map_into, capability_delete, cycle_reading, destroy_memory_region, exit,
    map_page_frame, now, receive, retype_into_kernel_object, retype_into_page_frame, send,
    split_memory_region, thread_control_block_capability_insert, thread_control_block_configure,
    thread_control_block_start, yield_now,
};

/// The endpoint the bench boot grants us (slot 0): we SEND `[ticks, iters, 0]` here per primitive.
const REPORT: u64 = 0;

// Which primitive to measure, chosen by `x0` at `START` (the bench boot picks). A benchmark program
// is exactly the place a role selector still earns its keep: one binary, one micro-measurement each.
const ROLE_YIELDER: u64 = 1;
const ROLE_CTX_SWITCH: u64 = 2;
const ROLE_IPC_SERVER: u64 = 3;
const ROLE_IPC_CLIENT: u64 = 4;
const ROLE_MAP: u64 = 5;
const ROLE_SPAWN: u64 = 6;

// Spawn-benchmark slots. slot 0 REPORT (the final result home to the bench boot), slot 1 MEMORY_REGION
// (the spawner's whole budget), slot 2 CHILD_DONE (children SEND here, the spawner receives; it also
// delegates a WRITE view to each child, so it needs READ|WRITE|GRANT). The shared code frame is
// retyped at setup into whatever slot `grant` hands back.
const SP_REPORT: u64 = 0;
const SP_UNTYPED: u64 = 1;
const SP_CHILD_DONE: u64 = 2;

// Map-benchmark slots (slot 0 is always REPORT). The bench boot grants a WRITE cap on a target
// address space and a READ cap on one frame; we map that frame at a fresh VA each iteration.
const MAP_ADDRESS_SPACE: u64 = 1;
const MAP_PAGE_FRAME: u64 = 2;

// IPC round-trip slots. The server holds two endpoints; the client holds three (report first, so
// slot 0 stays "the endpoint I report on" across every reporting role).
const SRV_REQUEST: u64 = 0; // server receives a request here
const SRV_REPLY: u64 = 1; // server SENDs the reply here
const CLI_REQUEST: u64 = 1; // client SENDs the request here (slot 0 is REPORT)
const CLI_REPLY: u64 = 2; // client receives the reply here

/// `x1` for [`ROLE_IPC_CLIENT`] when the bench boot has granted the client the cycle counter: read
/// it across the timed loop too, and send a second report. Any other value leaves the client as it
/// always was, so the `ipc_rtt_el0` row is the same code with or without it. It must match
/// `IPC_CLIENT_READS_CYCLES` in `kernel/src/bench/el0.rs`.
const IPC_CLIENT_READS_CYCLES: u64 = 1;

/// Iterations for the IPC round-trip loop. One iteration is a `SEND` to the server and a `RECEIVE` of
/// its reply: two rendezvous, four `svc`s (two on each side), two context switches. lmbench's
/// `lat_pipe` shape, over our endpoints.
const IPC_ITERS: u64 = 5_000;

/// Iterations for the context-switch loop. Each is one `SYS_YIELD` that hands the CPU to the peer
/// and gets it back: a round trip of two switches (each an address-space change, since the peer is a
/// separate process). Fewer than the null loop: a switch is much heavier under TCG.
const CTX_ITERS: u64 = 5_000;

/// Iterations for the map loop. Unlike the loops above, each `MAP_INTO` **consumes** budget: a fresh
/// leaf VA needs a page-table entry (and, cold, an intermediate table) plus a revocation record, all
/// paid from the target space's region. So the count is bounded by that region, not free to grow, and
/// the bench boot sizes the region for `MAP_WARMUP + MAP_ITERS` (it must be kept in step with the
/// `MAP_EL0_ITERS` mirror there). 500 fits inside a single L3 table (512 entries from the timed base),
/// so the loop stays one table's worth of walks, and it is enough samples for the HVF median to
/// settle (64 was too few: the per-map counter delta was in the noise). Under icount it is exact.
const MAP_ITERS: u64 = 500;
/// Untimed warmup maps, so the first cold page-table allocation lands outside the window. They use a
/// disjoint VA range from the timed maps; both are paid from the same region (which is sized for the
/// sum). The page size is fixed (aarch64 4 KiB), and the VA bases are arbitrary aligned user pages.
const MAP_WARMUP: u64 = 8;
const PAGE: u64 = 4096;
const MAP_WARM_BASE: u64 = address_space_map::pair_page(0x20_0000);
const MAP_TIMED_BASE: u64 = address_space_map::pair_page(0x40_0000);

user_mode_runtime::program_entry!(start);

fn start(role: u64, x1: u64, _x2: u64) -> ! {
    match role {
        ROLE_YIELDER => yielder(),
        ROLE_CTX_SWITCH => ctx_switch_bench(),
        ROLE_IPC_SERVER => ipc_server(),
        ROLE_IPC_CLIENT => ipc_client(x1 == IPC_CLIENT_READS_CYCLES),
        ROLE_MAP => map_bench(),
        ROLE_SPAWN => spawn_bench(),
        _ => exit(),
    }
}

/// **The peer for the context-switch benchmark.** Yields so the timer process always has exactly
/// one other ready thread to switch to. Capped, not infinite: it must outlast the timer's warmup +
/// timed loop (so the timer always has a peer to switch to), then exit rather than spin, so it does
/// not burn a core after the measurement (CLAUDE.md's no-busy-halt rule). The cap is comfortably
/// larger than the timer's yield count.
fn yielder() -> ! {
    for _ in 0..(2 * CTX_ITERS + 4096) {
        yield_now();
    }
    exit();
}

/// **Context switch latency, measured from EL0.** With the yielder as the only other ready thread,
/// each `SYS_YIELD` here switches to it and back: two context switches, each including an address-
/// space change because the peer is a separate process (this is lmbench's `lat_ctx`, process flavor).
fn ctx_switch_bench() -> ! {
    // Warm up: let both processes reach steady state before the timed loop.
    for _ in 0..64 {
        yield_now();
    }
    let start = now();
    for _ in 0..CTX_ITERS {
        yield_now();
    }
    let ticks = now().wrapping_sub(start);
    send(REPORT, ticks, CTX_ITERS, 0);
    exit();
}

/// **The server half of the IPC round-trip benchmark.** Loops forever: RECEIVE a request, SEND a reply.
/// It BLOCKS on the RECEIVE when idle (0% CPU, unlike the busy yielder), so it can loop unbounded and
/// still park cleanly once the client is done; the bench boot halts and reaps it.
fn ipc_server() -> ! {
    loop {
        let (n, _, _) = receive(SRV_REQUEST);
        send(SRV_REPLY, n, 0, 0);
    }
}

/// **IPC round-trip latency, measured from EL0.** lmbench's `lat_pipe`, over our endpoints. Each
/// iteration is a SEND to the server and a RECEIVE of its reply: two rendezvous, four `svc`s, two
/// context switches. Self-timed; the bench boot spawns the server first so a request always meets a
/// waiting receiver.
///
/// With `reads_cycles` (the caller holds the cycle-counter grant) the cycle counter is read just
/// inside the tick reads, and a second report follows the first: `[cycles, meaning, 0]`, the
/// meaning as [`user_mode_runtime::CycleMeaning::to_word`]. Without the grant those reads would
/// trap, so the flag is the bench boot's statement that it granted. See milestone 101 (the L4
/// calibration).
fn ipc_client(reads_cycles: bool) -> ! {
    // Warm up: pay the first rendezvous and any cold paths outside the timed loop.
    for _ in 0..64 {
        send(CLI_REQUEST, 1, 0, 0);
        receive(CLI_REPLY);
    }
    let start = now();
    let first = reads_cycles.then(cycle_reading);
    for _ in 0..IPC_ITERS {
        send(CLI_REQUEST, 1, 0, 0);
        receive(CLI_REPLY);
    }
    let last = reads_cycles.then(cycle_reading);
    let ticks = now().wrapping_sub(start);
    send(REPORT, ticks, IPC_ITERS, 0);
    if let (Some(first), Some(last)) = (first, last) {
        send(
            REPORT,
            last.count.wrapping_sub(first.count),
            last.meaning.to_word(),
            0,
        );
    }
    exit();
}

/// **Map latency, measured from EL0 (the primitive suite).** lmbench's `lat_mmap`, in the shape our
/// surface allows: `invoke(address space, MAP_INTO, va, frame_slot, MAP_RO)` maps the one granted frame at a
/// fresh VA. We alias a single frame across every VA, so no data memory is consumed per iteration,
/// only the page-table entry and the revocation record the map path must write anyway, which is
/// exactly what we are timing. There is no unmap in the surface yet, so the loop is bounded (each VA
/// is used once); the bench boot provisions the target region for the warmup plus timed counts. The
/// gap between this and the kernel-side `map_new` is roughly the EL0<->EL1 trap on each `svc`.
fn map_bench() -> ! {
    // Warm the cold page-table allocations at a disjoint VA range, untimed.
    for i in 0..MAP_WARMUP {
        map_one(MAP_WARM_BASE + i * PAGE);
    }
    let start = now();
    for i in 0..MAP_ITERS {
        map_one(MAP_TIMED_BASE + i * PAGE);
    }
    let ticks = now().wrapping_sub(start);
    send(REPORT, ticks, MAP_ITERS, 0);
    exit();
}

/// One `MAP_INTO`: map the granted frame (slot `MAP_PAGE_FRAME`) read-only into the granted space (slot
/// `MAP_ADDRESS_SPACE`) at `va`. Read-only needs only READ on the frame cap, which is all we were granted.
/// Returns the syscall result: 0 on success, a negative error otherwise.
#[inline(never)]
fn map_one(va: u64) -> i64 {
    address_space_map_into(
        MAP_ADDRESS_SPACE,
        va,
        MAP_PAGE_FRAME,
        abi::address_space::MAP_RO,
    )
}

/// Spawn benchmark tuning. Each iteration builds a whole child from EL0 and reclaims it, so the
/// count is modest (spawn is the heaviest primitive). The child is split, run, and destroyed
/// strictly LIFO, so `DESTROY` returns its pages to the budget (DECISIONS §16): only one child lives
/// at a time, and the bench boot's untyped need only fund one child, not one per iteration.
const SPAWN_ITERS: u64 = 100;
const SPAWN_WARMUP: u64 = 8;
const CHILD_PAGES: u64 = 10; // the child's address space root + tables + one stack page + revoke records
// The child's code and stack sit where a loaded program's would, the address-space map's image base
// and top stack page (milestone 206 (a program image has under 896 KiB)). That keeps the page tables this benchmark pays for the shape
// every real spawn pays for: one table for the code, one shared by the stack and the current-CPU
// page, the same count as when both lived at `0x40_0000` and `0x50_0000`.
const CHILD_CODE_VA: u64 = address_space_map::IMAGE_BASE;
const CHILD_STACK_VA: u64 = address_space_map::STACK_TOP_PAGE;
const SPAWN_SCRATCH_VA: u64 = address_space_map::pair_page(0x0100_0000); // where we map the shared code frame to write the stub
const SPAWN_PAGE: u64 = 4096;

/// **Spawn latency, measured from EL0 (the primitive suite).** lmbench's `lat_proc`: the cost to
/// build, start, run to exit, reap, and reclaim a whole child process, driven entirely from EL0
/// through the granular verbs (`SPLIT` a region, retype an address space and a TCB, map code and a
/// stack, configure, start), then `DESTROY` the child's region. This is the payoff of object
/// revocation: without reclamation the loop could not repeat.
fn spawn_bench() -> ! {
    // One shared code frame, retyped from our OWN untyped so destroying a child never frees it. Map
    // it writable in our own space, write the child stub once, and keep the cap to alias into every
    // child. The kernel makes the icache coherent when we later map it as CODE into a child.
    let code_frame = retype_into_page_frame(SP_UNTYPED) as u64;
    map_page_frame(code_frame, SPAWN_SCRATCH_VA, true, SP_UNTYPED);
    // Copied as **bytes**, not as elements, because the stub's element type differs by
    // architecture: two fixed-width ISAs spell it as `[u32; 9]` and x86_64 has to spell it as
    // `[u8; 43]`. One `copy_nonoverlapping` over `size_of_val` serves all three and reads the same
    // on each; a loop over `.iter()` would need the element type to agree, which is exactly the
    // thing that cannot.
    //
    // SAFETY: SPAWN_SCRATCH_VA is a page we just mapped read/write into our own address space, and
    // the stub is far smaller than the 4 KiB frame. Source and destination cannot overlap: one is
    // this program's `.rodata` and the other is a frame we retyped a moment ago.
    unsafe {
        core::ptr::copy_nonoverlapping(
            user_mode_runtime::child_stub::SEND_THEN_EXIT
                .as_ptr()
                .cast::<u8>(),
            SPAWN_SCRATCH_VA as *mut u8,
            core::mem::size_of_val(&user_mode_runtime::child_stub::SEND_THEN_EXIT),
        );
    }

    for _ in 0..SPAWN_WARMUP {
        spawn_one(code_frame);
    }
    let start = now();
    for _ in 0..SPAWN_ITERS {
        spawn_one(code_frame);
    }
    let ticks = now().wrapping_sub(start);
    send(SP_REPORT, ticks, SPAWN_ITERS, 0);
    exit();
}

/// Build one child from EL0, run it to exit, and reclaim its region. `code_frame` is the shared,
/// already-filled code page aliased into the child; everything else (address space, stack, TCB)
/// comes from the child's own region so `DESTROY` reclaims it whole.
#[inline(never)]
fn spawn_one(code_frame: u64) {
    // The child's own region, so it is independently reclaimable.
    let child_ut = split_memory_region(SP_UNTYPED, CHILD_PAGES) as u64;
    let aspace = retype_into_kernel_object(child_ut, abi::objtype::ADDRESS_SPACE) as u64;
    // Alias the shared code frame as the child's code; a stack from the child's own region.
    address_space_map_into(
        aspace,
        CHILD_CODE_VA,
        code_frame,
        abi::address_space::MAP_CODE,
    );
    let stack = retype_into_page_frame(child_ut) as u64;
    address_space_map_into(aspace, CHILD_STACK_VA, stack, abi::address_space::MAP_RW);
    capability_delete(stack);

    // The thread: give it CHILD_DONE (narrowed to WRITE) in its slot 0, configure (which consumes
    // the address space cap), and start. The child drops to EL0, SENDs its done word, and exits.
    let tcb = retype_into_kernel_object(child_ut, abi::objtype::THREAD_CONTROL_BLOCK) as u64;
    thread_control_block_capability_insert(tcb, SP_CHILD_DONE, abi::rights::WRITE, 0);
    thread_control_block_configure(tcb, CHILD_CODE_VA, CHILD_STACK_VA + SPAWN_PAGE, aspace);
    thread_control_block_start(tcb, 0, 0, 0);

    // Wait for the child to run and signal, then reclaim its region. The retry covers the sliver
    // between the child's SEND and its SYS_EXIT: DESTROY refuses a region with a still-live thread,
    // so yield until the child has finished exiting.
    let _ = receive(SP_CHILD_DONE);
    while destroy_memory_region(child_ut) != 0 {
        yield_now();
    }
    // Free the slots this child used (the address space cap was consumed by CONFIGURE) so the fixed capability table
    // does not fill over the loop.
    capability_delete(tcb);
    capability_delete(child_ut);
}

user_mode_runtime::panic_handler!();
