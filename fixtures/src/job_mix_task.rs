//! One task of the multi-tasking workload (milestone 168), run at EL0, where a real workload runs.
//!
//! `crates/job_mix` defines the workload. This task runs the mix when the supervisor
//! (`kernel/src/job_mix.rs`, which owns the wall clock) releases it, self-times, and reports. It is
//! an EL0 process because DECISIONS §96 (process kernel or event kernel) asks what a process kernel
//! costs a workload: every job crosses the trap boundary as a real program does, and the two that
//! do not ([`job_mix::COMPUTE`], [`job_mix::TOUCH`]) are the application time that displaces the
//! cache a returning thread wanted. See notes/job-mix.md.

//! # The two roles
//!
//! - [`job_mix::ROLE_MIXER`]: block on the go endpoint, run [`job_mix::ROUNDS_PER_TASK`] rounds of
//!   the mix in this task's own order, report `[jobs, ticks, index]`, block again. A go word of
//!   [`job_mix::GO_BREAKDOWN`] instead asks for the last subrun's per-kind ticks, untimed. It never
//!   exits; the supervisor halts the machine when the sweep is done.
//! - [`job_mix::ROLE_ECHO`]: `RECEIVE_CAP` and `REPLY`, forever: the [`job_mix::ROUND_TRIP`] job's
//!   other half.
//!

//! # BUGS
//!
//! - A mixer's self-timed ticks are per-task latency, not the benchmark's number (the supervisor's
//!   wall clock); they are printed so one slow task can be told from many.
//! - A future compiler could recognize `grind`; nothing checks that its defense still holds.
//! - The touch job walks this task's own `.bss`: cache displacement, not page tables.
//! - Only the map and spawn jobs report a refusal ([`job_mix::REPORT_FAILED`]); the rest cannot.
//! - A refused `CALL` still counts as done (`call` returns no status), so a wedged echo server
//!   shows as a subrun that never completes.
//!
//! Name: ratified 2026-09-05 (calef, milestone 168 (a multi-tasking workload benchmark)). One task
//! *inside* the mix; `job_mixer` claimed
//! an agent role it does not have. `job_mix_task` keeps the family greppable as one string, and
//! `task` is `crates/job_mix`'s own word. Refused `job` (the shell's job control), `aim` and `aim7`
//! (naming somebody else's benchmark, whose comparability `crates/job_mix`'s `BUGS` denies), and
//! `worker` (generic).

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`.
#![allow(missing_docs)]
#![no_main]

use user_mode_runtime::{call, now, receive, receive_request, reply, send, yield_now};

/// The working set the [`job_mix::TOUCH`] job walks: this task's own memory, sized in
/// `crates/job_mix` against the smallest L1d this project targets.
///
/// A `static mut` rather than a stack array because the user stack is one page and this is 32 KiB,
/// and because every task having its own copy is the point: two tasks sharing one buffer would be
/// measuring a cache line's worth of sharing instead of displacement.
static mut WORKING_SET: [u64; job_mix::TOUCH_WORDS] = [0; job_mix::TOUCH_WORDS];

/// A capability slot this task was given nothing in, for the [`job_mix::NULL_SYSCALL`] job to be
/// refused on. Past every slot `kernel/src/job_mix.rs` grants, and it must stay that way: a slot
/// that ever held an object would make the job invoke it rather than bounce off the kernel's
/// slot check.
const EMPTY_SLOT: u64 = 63;

/// A non-elidable integer grind, the same shape as `kernel/src/bench/smp_throughput.rs`'s `busy`:
/// an LCG mixed with an xorshift, folded into a returned value the caller accumulates so the
/// optimizer cannot delete the loop.
#[inline(never)]
fn grind(iters: u64, seed: u64) -> u64 {
    let mut x = seed | 1;
    let mut i = 0;
    while i < iters {
        x = x
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407 ^ i);
        x ^= x >> 29;
        i += 1;
    }
    x
}

/// Read and write every word of [`WORKING_SET`] once, returning a value the caller accumulates so
/// the walk cannot be deleted.
#[inline(never)]
fn touch(seed: u64) -> u64 {
    let mut acc = seed;
    let mut i = 0;
    while i < job_mix::TOUCH_WORDS {
        // SAFETY: `WORKING_SET` is this process's own `.bss` and this program is single-threaded
        // within its address space, so there is no other reference to it anywhere. The index is
        // bounded by the array's own length.
        unsafe {
            let p = &raw mut WORKING_SET[i];
            acc = acc.wrapping_add(p.read()).rotate_left(7);
            p.write(acc);
        }
        i += 1;
    }
    acc
}

/// Where the [`job_mix::MAP`] job maps its frame in the space it builds for itself. Any page-aligned
/// user address works, since nothing else is ever mapped in that space; this one is the address
/// `os_primitives_benchmarker`'s timed maps use.
const MAP_TARGET_VA: u64 = address_space_map::pair_page(0x40_0000);
/// Every architecture this tree targets maps 4 KiB pages at the leaf.
const PAGE: u64 = 4096;
/// Where a child's code and stack go in the address space the [`job_mix::SPAWN`] job builds for it:
/// the address-space map's image base and top stack page, where a loaded program's would be.
const CHILD_CODE_VA: u64 = address_space_map::IMAGE_BASE;
const CHILD_STACK_VA: u64 = address_space_map::STACK_TOP_PAGE;
/// Where this task maps, writable, the one frame it fills with the child's code. Far from anything
/// the loader maps, which is the choice `os_primitives_benchmarker` made for the same frame.
const STUB_SCRATCH_VA: u64 = address_space_map::pair_page(0x0100_0000);

/// A job's syscall was refused: the refusal, as the negative `abi::Error` the kernel returned.
type Refused = i64;

/// A syscall's result as a slot or a refusal: the retype and split verbs return the new slot, or a
/// negative error.
fn slot(r: i64) -> Result<u64, Refused> {
    if r < 0 { Err(r) } else { Ok(r as u64) }
}

/// A syscall that answers zero on success.
fn zero(r: i64) -> Result<(), Refused> {
    if r == 0 { Ok(()) } else { Err(r) }
}

/// **User page mapping**: split a region, build an address space and a frame in it, map the frame
/// at [`job_mix::MAP_CALLS`] fresh addresses, and give the whole region back.
///
/// Returns the ticks spent in `SPLIT` and `DESTROY`, the two calls that take the kernel's one
/// memory-region lock, so the supervisor can say how much of the job is that lock rather than the
/// map path (milestone 168's hazard: at 32 tasks this could be measuring the allocator).
#[inline(never)]
fn map_job() -> Result<u64, Refused> {
    let t0 = now();
    let region = slot(user_mode_runtime::split_memory_region(
        job_mix::SLOT_BUDGET,
        job_mix::MAP_REGION_PAGES,
    ))?;
    let mut region_ticks = now() - t0;
    let aspace = slot(user_mode_runtime::retype_into_kernel_object(
        region,
        abi::objtype::ADDRESS_SPACE,
    ))?;
    let frame = slot(user_mode_runtime::retype_into_page_frame(region))?;
    let mut i = 0;
    while i < job_mix::MAP_CALLS {
        zero(user_mode_runtime::address_space_map_into(
            aspace,
            MAP_TARGET_VA + i * PAGE,
            frame,
            abi::address_space::MAP_RO,
        ))?;
        i += 1;
    }
    let t1 = now();
    zero(user_mode_runtime::destroy_memory_region(region))?;
    region_ticks += now() - t1;
    // The objects are gone with their region; the slots that named them are not, and a fixed
    // table would fill over a subrun if they stayed.
    user_mode_runtime::cap_delete(frame);
    user_mode_runtime::cap_delete(aspace);
    user_mode_runtime::cap_delete(region);
    Ok(region_ticks)
}

/// **Process creation**: [`job_mix::SPAWN_CALLS`] children, each built from EL0, run to its exit,
/// reaped and reclaimed, one at a time. `os_primitives_benchmarker`'s `spawn_one`, per task.
///
/// Returns the ticks spent inside `SPLIT` and `DESTROY` calls, for [`map_job`]'s reason. The yields
/// between a refused `DESTROY` and the next attempt are not counted there: that wait is the child
/// finishing its exit, which is the scheduler's time and not the allocator's.
#[inline(never)]
fn spawn_job(code_frame: u64) -> Result<u64, Refused> {
    let mut region_ticks = 0;
    let mut n = 0;
    while n < job_mix::SPAWN_CALLS {
        let t0 = now();
        let child = slot(user_mode_runtime::split_memory_region(
            job_mix::SLOT_BUDGET,
            job_mix::CHILD_PAGES,
        ))?;
        region_ticks += now() - t0;
        let aspace = slot(user_mode_runtime::retype_into_kernel_object(
            child,
            abi::objtype::ADDRESS_SPACE,
        ))?;
        zero(user_mode_runtime::address_space_map_into(
            aspace,
            CHILD_CODE_VA,
            code_frame,
            abi::address_space::MAP_CODE,
        ))?;
        let stack = slot(user_mode_runtime::retype_into_page_frame(child))?;
        zero(user_mode_runtime::address_space_map_into(
            aspace,
            CHILD_STACK_VA,
            stack,
            abi::address_space::MAP_RW,
        ))?;
        user_mode_runtime::cap_delete(stack);
        let tcb = slot(user_mode_runtime::retype_into_kernel_object(
            child,
            abi::objtype::THREAD_CONTROL_BLOCK,
        ))?;
        slot(user_mode_runtime::thread_control_block_capability_insert(
            tcb,
            job_mix::SLOT_CHILD_DONE,
            abi::rights::WRITE,
            0,
        ))?;
        // CONFIGURE consumes the address-space capability, so there is no slot to delete for it.
        // Its result is the child's current-CPU page address since milestone 812
        // (`std::thread::spawn` runs real threads in one address space), so success is
        // any non-negative answer.
        let configured = user_mode_runtime::thread_control_block_configure(
            tcb,
            CHILD_CODE_VA,
            CHILD_STACK_VA + PAGE,
            aspace,
        );
        zero(configured.min(0))?;
        zero(user_mode_runtime::thread_control_block_start(tcb, 0, 0, 0))?;
        // The child's done word. Then reclaim: `DESTROY` refuses a region a live thread occupies,
        // and the child is between its `SEND` and its exit for a moment after this returns.
        let _ = receive(job_mix::SLOT_CHILD_DONE);
        loop {
            let t1 = now();
            let r = user_mode_runtime::destroy_memory_region(child);
            region_ticks += now() - t1;
            if r == 0 {
                break;
            }
            if r != abi::Error::NotPermitted as i64 {
                return Err(r);
            }
            yield_now();
        }
        user_mode_runtime::cap_delete(tcb);
        user_mode_runtime::cap_delete(child);
        n += 1;
    }
    Ok(region_ticks)
}

/// The frame every child's code is aliased from, retyped from this task's budget **before** any job
/// splits it, so it outlives every child and every `DESTROY` (a region reclaims in reverse order of
/// what was taken from it). Filled once with [`user_mode_runtime::child_stub::SEND_THEN_EXIT`].
fn prepare_child_code() -> Result<u64, Refused> {
    let frame = slot(user_mode_runtime::retype_into_page_frame(
        job_mix::SLOT_BUDGET,
    ))?;
    if !user_mode_runtime::map_page_frame(frame, STUB_SCRATCH_VA, true, job_mix::SLOT_BUDGET) {
        return Err(abi::Error::OutOfMemory as i64);
    }
    // SAFETY: `STUB_SCRATCH_VA` is the page just mapped read/write into this address space, and
    // the stub is far smaller than it. The source is this program's `.rodata` and the destination a
    // frame retyped a moment ago, so they cannot overlap. Copied as bytes because the stub's element
    // type differs by architecture (`os_primitives_benchmarker` has the longer argument).
    unsafe {
        core::ptr::copy_nonoverlapping(
            user_mode_runtime::child_stub::SEND_THEN_EXIT
                .as_ptr()
                .cast::<u8>(),
            STUB_SCRATCH_VA as *mut u8,
            core::mem::size_of_val(&user_mode_runtime::child_stub::SEND_THEN_EXIT),
        );
    }
    Ok(frame)
}

/// Run one job. `Ok((value, region_ticks))`: a value the caller accumulates so no job can be
/// optimised away, and the ticks the job spent in region calls (zero for the five that make none).
fn run_job(job: u8, seed: u64, code_frame: u64) -> Result<(u64, u64), Refused> {
    let value = match job {
        job_mix::COMPUTE => grind(job_mix::COMPUTE_ITERS, seed),
        job_mix::TOUCH => touch(seed),
        job_mix::NULL_SYSCALL => {
            let mut i = 0;
            while i < job_mix::NULL_SYSCALL_CALLS {
                // **A real trap, and the cheapest one this ABI has.** `is_granted` invokes a method
                // number no object type defines on a slot this task was given nothing in, so the
                // kernel validates the slot, refuses, and returns: an entry and an exit with no
                // object work between them. `now()` was refused for this job because it is *not* a
                // syscall on any of the three architectures (an unprivileged counter read), so a
                // job named for the trap would have measured a loop.
                core::hint::black_box(user_mode_runtime::is_granted(EMPTY_SLOT));
                i += 1;
            }
            seed
        }
        job_mix::YIELD => {
            let mut i = 0;
            while i < job_mix::YIELD_CALLS {
                yield_now();
                i += 1;
            }
            seed
        }
        job_mix::ROUND_TRIP => {
            let mut acc = seed;
            let mut i = 0;
            while i < job_mix::ROUND_TRIP_CALLS {
                let (r0, _r1) = call(job_mix::SLOT_ECHO, acc, i);
                acc = acc.wrapping_add(r0).rotate_left(11);
                i += 1;
            }
            acc
        }
        job_mix::MAP => return map_job().map(|r| (seed.rotate_left(3), r)),
        job_mix::SPAWN => return spawn_job(code_frame).map(|r| (seed.rotate_left(5), r)),
        // Unreachable while `crates/job_mix`'s own test holds (the mix contains no undefined kind),
        // and a no-op rather than a panic if it ever does not: a task that cannot say anything is
        // better reported by the supervisor's stall than by a fault report interleaved with the
        // console of a board nobody is watching.
        _ => seed,
    };
    Ok((value, 0))
}

/// The task's entry.
///
/// `x0` is the role, `x1` this task's index, and `x2` its seed. The supervisor gives every task a
/// distinct index and a distinct seed, so no two tasks walk the mix in the same order.
#[unsafe(no_mangle)]
pub extern "C" fn _start(role: u64, index: u64, seed: u64) -> ! {
    if role == job_mix::ROLE_ECHO {
        loop {
            let req = receive_request(job_mix::SLOT_SERVE);
            if let Some(to) = req.delivered.into_reply() {
                reply(to, req.w1, 0);
            }
        }
    }

    let order = job_mix::order(seed);
    let mut acc = seed | 1;
    // Built before the first go-ahead, so no subrun times it. A refusal here is kept and reported
    // as the first subrun's failure, because nothing can be said before the supervisor asks.
    let code_frame = prepare_child_code();
    // Per kind: ticks spent in the kind's jobs, and within them in region calls, for the last subrun.
    let mut kind_ticks = [0u64; job_mix::JOB_KINDS];
    let mut region_ticks = [0u64; job_mix::JOB_KINDS];

    loop {
        // The go-ahead. The supervisor hands these out one rendezvous at a time, so a task's own
        // clock starts when it is released rather than when the subrun does; the difference between
        // the two is the release skew the supervisor's own doc comment prices.
        let (go, _, _) = receive(job_mix::SLOT_GO);
        if go == job_mix::GO_BREAKDOWN {
            // Untimed: the supervisor stopped its clock before asking.
            for kind in 0..job_mix::JOB_KINDS {
                send(
                    job_mix::SLOT_REPORT,
                    kind as u64,
                    kind_ticks[kind],
                    region_ticks[kind],
                );
            }
            continue;
        }

        let code = match code_frame {
            Ok(frame) => frame,
            Err(e) => {
                send(
                    job_mix::SLOT_REPORT,
                    job_mix::REPORT_FAILED,
                    e as u64,
                    index | (u64::from(job_mix::SPAWN) << 32),
                );
                continue;
            }
        };
        kind_ticks = [0; job_mix::JOB_KINDS];
        region_ticks = [0; job_mix::JOB_KINDS];

        let t0 = now();
        let mut failed = None;
        let mut round = 0;
        'subrun: while round < job_mix::ROUNDS_PER_TASK {
            for &job in &order {
                // Two counter reads per job, and neither is a syscall on any architecture this
                // tree targets, so the breakdown costs the subrun a few instructions a job.
                let j0 = now();
                match run_job(job, acc, code) {
                    Ok((value, region)) => {
                        acc = value;
                        region_ticks[job as usize] += region;
                    }
                    Err(e) => {
                        failed = Some((job, e));
                        break 'subrun;
                    }
                }
                kind_ticks[job as usize] += now() - j0;
            }
            round += 1;
        }
        let ticks = now() - t0;

        // `acc` is kept alive across the report rather than folded into it: the supervisor reads
        // `index` and would print a corrupted one, and a benchmark that mangles its own labels to
        // defeat the optimizer has traded a readable result for a defence `black_box` already
        // gives. It carries into the next subrun, so nothing above it is dead.
        core::hint::black_box(acc);
        match failed {
            None => {
                send(job_mix::SLOT_REPORT, job_mix::JOBS_PER_TASK, ticks, index);
            }
            Some((job, e)) => {
                send(
                    job_mix::SLOT_REPORT,
                    job_mix::REPORT_FAILED,
                    e as u64,
                    index | (u64::from(job) << 32),
                );
            }
        }
    }
}

user_mode_runtime::panic_handler!();
