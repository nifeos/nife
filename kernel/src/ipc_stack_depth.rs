//! How deep one IPC reaches into a thread's kernel stack.
//!
//! Immediately before one IPC operation the thread paints its own kernel stack up to [`MARGIN`]
//! below its live `sp`; immediately after, it scans for the deepest word no longer paint. Kernel
//! threads wrap each call in [`measured`] ([`kernel_thread_shapes`]); for EL0 threads
//! [`after_syscall`] scans and repaints at the end of each syscall ([`el0_shapes`]). Each series
//! reports its median, which is the per-IPC figure, beside the interrupt-contaminated maximum.
//! Built only under `any(test, feature = "ipc_stack_depth")`. See
//! notes/stack-high-water/per-ipc-depth.md.
//!
//! Name: provisional (milestone 134 (the register of measures)'s lane, 2026-09-19): module, feature
//! and the `ipc-stack-depth:` line prefix alike. Names are an architect's.

use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};

use crate::{println, sched};

/// Bytes between the arming frame's `sp` and the top of the painted region.
///
/// It covers the frames that run below [`arm`] while it paints and the scan's frame afterwards. A
/// series whose median reads at or below its floor never went below the margin, and [`report`]
/// refuses it. Larger in debug, where the paint loop keeps real calls; 512 put every release series
/// at its floor. The `null` series checks the value on every run.
#[cfg(debug_assertions)]
const MARGIN: u64 = 512;
#[cfg(not(debug_assertions))]
const MARGIN: u64 = 64;

/// Samples kept per series. Depth is nearly deterministic, so more samples only characterize the
/// interrupt tail.
const SAMPLES: usize = 256;

/// One series: one operation, by one role, in one shape.
///
/// Single writer (the role's thread), read only after it has finished, so relaxed atomics suffice;
/// atomics rather than a `static mut` so the storage needs no `unsafe`.
struct Series {
    n: AtomicUsize,
    /// `top - ceiling`: the shallowest value a sample can read, recorded from the last arming.
    floor: AtomicU64,
    /// `top - sp` at the call site, so a reader can subtract the caller's own frames.
    caller: AtomicU64,
    depth: [AtomicU32; SAMPLES],
}

impl Series {
    const fn new() -> Self {
        Series {
            n: AtomicUsize::new(0),
            floor: AtomicU64::new(0),
            caller: AtomicU64::new(0),
            depth: [const { AtomicU32::new(0) }; SAMPLES],
        }
    }

    fn reset(&self) {
        self.n.store(0, Ordering::Relaxed);
    }

    fn is_full(&self) -> bool {
        self.n.load(Ordering::Relaxed) >= SAMPLES
    }

    fn record(&self, used: u64, floor: u64, caller: u64) {
        let i = self.n.fetch_add(1, Ordering::Relaxed);
        if i < SAMPLES {
            self.depth[i].store(used as u32, Ordering::Relaxed);
            self.floor.store(floor, Ordering::Relaxed);
            self.caller.store(caller, Ordering::Relaxed);
        } else {
            self.n.store(SAMPLES, Ordering::Relaxed);
        }
    }
}

/// The current thread's kernel stack as `(bottom, top)`.
///
/// Computes the bottom as `KernelStack::new` does, `STACK_PAGES` below the top, rather than taking
/// the scheduler lock for a second lookup.
fn own_span() -> (u64, u64) {
    let top = sched::current_kernel_stack_top()
        .expect("ipc_stack_depth: this thread has no kernel stack");
    (top - (crate::thread::STACK_PAGES as u64) * 4096, top)
}

/// Paint `[bottom, sp - MARGIN)` and return the ceiling. `inline(never)` so that its frame, and the
/// paint loop's below it, are the same size every time and sit inside the margin.
#[inline(never)]
fn arm(bottom: u64) -> u64 {
    let ceiling = (crate::arch::current_sp() - MARGIN) & !7;
    // SAFETY: `[bottom, ceiling)` is inside the current thread's own kernel stack (from `own_span`),
    // mapped and 8-byte aligned, and everything below `sp - MARGIN` is dead: no frame of this thread
    // lives below its own `sp`, and no other thread ever runs on this stack. The margin covers this
    // function's callees while the loop runs.
    unsafe { crate::stack::paint(bottom, ceiling) };
    ceiling
}

/// The deepest byte reached since the last [`arm`], as a distance from `top`.
#[inline(never)]
fn scan(bottom: u64, top: u64) -> u64 {
    // SAFETY: the span `arm` painted up to its ceiling, on this thread's own mapped stack; above the
    // ceiling is live data, which the scan reads as used, which is correct.
    unsafe { crate::stack::high_water(bottom, top) }
}

/// Run `operation` with the stack painted beneath it and record how deep it went.
#[inline(never)]
fn measured<R>(series: &Series, span: (u64, u64), operation: impl FnOnce() -> R) -> R {
    let (bottom, top) = span;
    let caller = top - crate::arch::current_sp();
    let ceiling = arm(bottom);
    let r = operation();
    let used = scan(bottom, top);
    series.record(used, top - ceiling, caller);
    r
}

// --- The kernel-thread shapes: E1's own ---

static K_NULL: Series = Series::new();
static K_SR_CLIENT_SEND: Series = Series::new();
static K_SR_CLIENT_RECEIVE: Series = Series::new();
static K_SR_SERVER_RECEIVE: Series = Series::new();
static K_SR_SERVER_SEND: Series = Series::new();
static K_CR_CLIENT_CALL: Series = Series::new();
static K_CR_SERVER_RECEIVE_CAP: Series = Series::new();
static K_CR_SERVER_REPLY: Series = Series::new();

/// Unmeasured round trips first, so a lazily built path (the first rendezvous, a first reply
/// capability) is not in the samples. The same idea as `bench.rs`'s `WARMUP`.
const WARMUP: usize = 16;

/// SEND/RECEIVE between two kernel threads, the shape `bench.rs`'s `ipc_rtt` and E1's
/// `ipc_thread_scaling` time.
fn send_receive_kernel() {
    for s in [
        &K_SR_CLIENT_SEND,
        &K_SR_CLIENT_RECEIVE,
        &K_SR_SERVER_RECEIVE,
        &K_SR_SERVER_SEND,
    ] {
        s.reset();
    }
    let request = sched::create_rendezvous();
    let reply = sched::create_rendezvous();
    let done = sched::create_rendezvous();
    let cpu = crate::cpu::id();

    sched::spawn_on(cpu, move || {
        let span = own_span();
        loop {
            let m = measured(&K_SR_SERVER_RECEIVE, span, || sched::ipc_receive(request));
            if m[0] == u64::MAX {
                break;
            }
            measured(&K_SR_SERVER_SEND, span, || {
                sched::ipc_send(reply, [m[0], 0, 0]);
            });
        }
    })
    .expect("ipc_stack_depth: no send/receive server");

    sched::spawn_on(cpu, move || {
        let span = own_span();
        for _ in 0..WARMUP {
            sched::ipc_send(request, [1, 0, 0]);
            sched::ipc_receive(reply);
        }
        for _ in 0..SAMPLES {
            measured(&K_SR_CLIENT_SEND, span, || {
                sched::ipc_send(request, [1, 0, 0]);
            });
            measured(&K_SR_CLIENT_RECEIVE, span, || sched::ipc_receive(reply));
        }
        sched::ipc_send(request, [u64::MAX, 0, 0]);
        sched::ipc_send(done, [0, 0, 0]);
    })
    .expect("ipc_stack_depth: no send/receive client");

    sched::ipc_receive(done);
}

/// CALL against `RECEIVE_CAP` and REPLY between two kernel threads, the shape `bench.rs`'s
/// `call_reply` times and the one real services issue.
fn call_reply_kernel() {
    for s in [
        &K_CR_CLIENT_CALL,
        &K_CR_SERVER_RECEIVE_CAP,
        &K_CR_SERVER_REPLY,
    ] {
        s.reset();
    }
    let ep = sched::create_rendezvous();
    let done = sched::create_rendezvous();
    let cpu = crate::cpu::id();

    sched::spawn_on(cpu, move || {
        let span = own_span();
        loop {
            let m = measured(&K_CR_SERVER_RECEIVE_CAP, span, || {
                sched::ipc_receive_cap(ep)
            });
            if m[0] == u64::MAX {
                break;
            }
            // The whole of `bench.rs`'s `call_reply` server body, so the sample is what a server
            // does to answer: find the reply capability, reply through it, drop the slot.
            measured(&K_CR_SERVER_REPLY, span, || {
                let slot = m[1];
                let crate::cap::Object::Reply(caller) = sched::current_cap(slot)
                    .expect("ipc_stack_depth: no reply capability")
                    .object
                else {
                    panic!(
                        "ipc_stack_depth: RECEIVE_CAP of a CALL did not deliver a Reply capability"
                    );
                };
                sched::ipc_reply(caller, [m[0], 0]);
                let _ = sched::delete_current_cap(slot);
            });
        }
    })
    .expect("ipc_stack_depth: no call/reply server");

    sched::spawn_on(cpu, move || {
        let span = own_span();
        for _ in 0..WARMUP {
            sched::ipc_call(ep, [1, 0]);
        }
        for _ in 0..SAMPLES {
            measured(&K_CR_CLIENT_CALL, span, || sched::ipc_call(ep, [1, 0]));
        }
        // A plain SEND meets a server parked in RECEIVE_CAP all the same (`bench.rs`'s `call_reply`).
        sched::ipc_send(ep, [u64::MAX, 0, 0]);
        sched::ipc_send(done, [0, 0, 0]);
    })
    .expect("ipc_stack_depth: no call/reply client");

    sched::ipc_receive(done);
}

/// Both kernel-thread shapes, then their report. Returns how many series fell short of a usable
/// measurement (see [`report`]); zero is a clean run.
pub(crate) fn kernel_thread_shapes() -> usize {
    // The calibration: `measured` around nothing reads the instrument's own reach. On a spawned
    // thread, because the caller may be the boot thread, which owns no kernel stack.
    K_NULL.reset();
    let done = sched::create_rendezvous();
    sched::spawn_on(crate::cpu::id(), move || {
        let span = own_span();
        for _ in 0..SAMPLES {
            measured(&K_NULL, span, || core::hint::black_box(()));
        }
        sched::ipc_send(done, [0, 0, 0]);
    })
    .expect("ipc_stack_depth: no calibration thread");
    sched::ipc_receive(done);
    // Judged on the median: the maximum carries a timer tick's trap frame, which no margin moves.
    let null_floor = K_NULL.floor.load(Ordering::Relaxed);
    let mut null = [0u32; SAMPLES];
    for (dst, src) in null.iter_mut().zip(K_NULL.depth.iter()) {
        *dst = src.load(Ordering::Relaxed);
    }
    null.sort_unstable();
    let (null_median, null_max) = (u64::from(null[SAMPLES / 2]), u64::from(null[SAMPLES - 1]));
    // `<=`, not `==`: whether the median reads exactly the floor depends on when the first trap
    // frame overwrites the ceiling word (notes/stack-high-water/per-ipc-depth.md). Too small a
    // margin reads deeper than the floor, which this still catches.
    let null_clean = null_median <= null_floor;
    println!(
        "ipc-stack-depth: kernel null (the instrument alone) floor {null_floor} median \
         {null_median} max {null_max} margin {MARGIN} {}",
        if null_clean {
            "clean"
        } else {
            "REACHED BELOW ITS MARGIN"
        },
    );

    send_receive_kernel();
    call_reply_kernel();
    let mut bad = usize::from(!null_clean);
    bad += report(
        "kernel",
        "send_receive",
        "client",
        "SEND",
        &K_SR_CLIENT_SEND,
    );
    bad += report(
        "kernel",
        "send_receive",
        "client",
        "RECEIVE",
        &K_SR_CLIENT_RECEIVE,
    );
    bad += report(
        "kernel",
        "send_receive",
        "server",
        "RECEIVE",
        &K_SR_SERVER_RECEIVE,
    );
    bad += report(
        "kernel",
        "send_receive",
        "server",
        "SEND",
        &K_SR_SERVER_SEND,
    );
    bad += report("kernel", "call_reply", "client", "CALL", &K_CR_CLIENT_CALL);
    bad += report(
        "kernel",
        "call_reply",
        "server",
        "RECEIVE_CAP",
        &K_CR_SERVER_RECEIVE_CAP,
    );
    bad += report(
        "kernel",
        "call_reply",
        "server",
        "REPLY",
        &K_CR_SERVER_REPLY,
    );
    bad
}

// --- The EL0 shapes: what a service runs ---

/// One registered EL0 thread. `tid` is `u64::MAX` when the slot is empty.
struct El0Slot {
    tid: AtomicU64,
    bottom: AtomicU64,
    top: AtomicU64,
    /// The ceiling of the last paint, or 0 before the thread's first syscall, whose window is the
    /// spawn path into EL0 and so records nothing.
    ceiling: AtomicU64,
    /// One series per rendezvous method number (`abi::rendezvous::*` are 0 to 4, and a Reply
    /// capability's `REPLY` is 0). Within one role the methods it issues are distinct, which is
    /// what makes a per-slot index enough to tell SEND from REPLY.
    series: [Series; 8],
}

impl El0Slot {
    const fn new() -> Self {
        El0Slot {
            tid: AtomicU64::new(u64::MAX),
            bottom: AtomicU64::new(0),
            top: AtomicU64::new(0),
            ceiling: AtomicU64::new(0),
            series: [const { Series::new() }; 8],
        }
    }

    /// This role has started at least one series and filled every series it started, so painting
    /// can stop. Before the first sample every series is empty, and `all` over nothing is true.
    fn is_done(&self) -> bool {
        self.series.iter().any(|s| s.n.load(Ordering::Relaxed) > 0)
            && self
                .series
                .iter()
                .all(|s| s.n.load(Ordering::Relaxed) == 0 || s.is_full())
    }
}

/// Two roles at a time: a client and a server.
static EL0: [El0Slot; 2] = [const { El0Slot::new() }; 2];

/// The only thing an unregistered syscall pays in a test build.
static EL0_ARMED: AtomicBool = AtomicBool::new(false);

/// Register the calling kernel thread as EL0 role `slot`, just before it enters user mode.
fn register(slot: usize) {
    let (bottom, top) = own_span();
    let s = &EL0[slot];
    s.bottom.store(bottom, Ordering::Relaxed);
    s.top.store(top, Ordering::Relaxed);
    s.ceiling.store(0, Ordering::Relaxed);
    s.tid.store(sched::current(), Ordering::Release);
}

fn reset_el0() {
    EL0_ARMED.store(false, Ordering::Release);
    for s in &EL0 {
        s.tid.store(u64::MAX, Ordering::Relaxed);
        for series in &s.series {
            series.reset();
        }
    }
}

/// Record how deep the syscall now ending went, then repaint for the next.
///
/// `nr` is the syscall number and `method` is `frame.arg(1)` as the syscall found it. Called by
/// `syscall::dispatch` last. An unregistered thread pays one load. The window also holds any
/// interrupt taken from EL0 since the last call, which is the tail the median discards.
#[inline(never)]
pub(crate) fn after_syscall(nr: u64, method: u64) {
    if !EL0_ARMED.load(Ordering::Relaxed) {
        return;
    }
    let me = sched::current();
    let Some(s) = EL0.iter().find(|s| s.tid.load(Ordering::Acquire) == me) else {
        return;
    };
    if s.is_done() {
        return;
    }
    let (bottom, top) = (
        s.bottom.load(Ordering::Relaxed),
        s.top.load(Ordering::Relaxed),
    );
    let ceiling = s.ceiling.load(Ordering::Relaxed);
    if ceiling != 0 && nr == abi::SYS_INVOKE {
        let used = scan(bottom, top);
        // The trap frame is at the top, so the caller's own depth is zero by construction.
        s.series[(method & 7) as usize].record(used, top - ceiling, 0);
    }
    s.ceiling.store(arm(bottom), Ordering::Relaxed);
}

/// `os_primitives_benchmarker`'s roles, as `bench.rs` spells them and the fixture's `_start`
/// matches them.
///
/// # BUGS
///
/// A third copy of a number two binaries agree on, which AGENTS.md rule 7 says should be a crate
/// (notes/stack-high-water.md's BUGS).
#[cfg(initial_ramdisk)]
const EL_IPC_SERVER: u64 = 3;
#[cfg(initial_ramdisk)]
const EL_IPC_CLIENT: u64 = 4;

/// `soaker`'s roles, likewise already copied once into `soak.rs`.
#[cfg(initial_ramdisk)]
const SOAK_RESPONDER: u64 = 0;
#[cfg(initial_ramdisk)]
const SOAK_CALLER: u64 = 1;

/// SEND/RECEIVE between two EL0 processes, `bench.rs`'s `ipc_rtt_el0`.
///
/// Returns `None` when the program is not in the initial ramdisk. The server parks in RECEIVE
/// forever, so the endpoints come from a region a `Holding` reclaims, which wakes it to die.
#[cfg(initial_ramdisk)]
fn send_receive_el0() -> Option<()> {
    use crate::cap::{Rights, rendezvous_cap};
    use crate::user::Spawn;
    use crate::user::holding::Holding;

    let Some(image) = crate::user::program("os_primitives_benchmarker") else {
        println!(
            "ipc-stack-depth: el0 send_receive skipped (no os_primitives_benchmarker in the initial ramdisk)"
        );
        return None;
    };
    let region = crate::memory_region::create(4).expect("ipc_stack_depth: no endpoint region");
    let request = sched::create_rendezvous_from(region).expect("no request endpoint");
    let reply = sched::create_rendezvous_from(region).expect("no reply endpoint");
    let report = sched::create_rendezvous_from(region).expect("no report endpoint");
    let cpu = crate::cpu::id();

    reset_el0();
    EL0_ARMED.store(true, Ordering::Release);

    let server = sched::spawn_on(cpu, move || {
        register(0);
        crate::user::run(
            image,
            Spawn {
                arg0: EL_IPC_SERVER,
                arg1: 0,
                arg2: 0,
                grants: &[
                    rendezvous_cap(request, Rights::READ),
                    rendezvous_cap(reply, Rights::WRITE),
                ],
                maps: &[],
            },
        )
    })
    .expect("ipc_stack_depth: could not spawn the EL0 server");
    let client = sched::spawn_on(cpu, move || {
        register(1);
        crate::user::run(
            image,
            Spawn {
                arg0: EL_IPC_CLIENT,
                arg1: 0,
                arg2: 0,
                grants: &[
                    rendezvous_cap(report, Rights::WRITE),
                    rendezvous_cap(request, Rights::WRITE),
                    rendezvous_cap(reply, Rights::READ),
                ],
                maps: &[],
            },
        )
    })
    .expect("ipc_stack_depth: could not spawn the EL0 client");

    sched::ipc_receive(report);
    EL0_ARMED.store(false, Ordering::Release);

    let mut held = Holding::new();
    held.add_thread(server);
    held.add_thread(client);
    held.add_region(region);
    held.release_or_fail("ipc_stack_depth's EL0 send/receive pair");
    Some(())
}

/// CALL against `RECEIVE_CAP` and REPLY between two EL0 processes, the soak workload's pair.
///
/// Returns `None` when `soaker` is not in the initial ramdisk. Both loop forever, so this waits up
/// to 30 seconds for their series to fill and then reclaims them.
#[cfg(initial_ramdisk)]
fn call_reply_el0() -> Option<()> {
    use crate::cap::{Rights, rendezvous_cap};
    use crate::user::holding::Holding;
    use crate::user::{Mapping, Spawn};

    let Some(image) = crate::user::program("soaker") else {
        println!("ipc-stack-depth: el0 call_reply skipped (no soaker in the initial ramdisk)");
        return None;
    };
    let region = crate::memory_region::create(2).expect("ipc_stack_depth: no endpoint region");
    let ep = sched::create_rendezvous_from(region).expect("no call endpoint");
    // The soaker writes its progress counters to this page; nothing here reads them.
    let page = crate::memory::alloc_zeroed().expect("ipc_stack_depth: no page for the soaker");
    let phys = page.addr();
    let cpu = crate::cpu::id();

    reset_el0();
    EL0_ARMED.store(true, Ordering::Release);

    let spawn = |slot: usize, role: u64, rights: Rights| {
        sched::spawn_on(cpu, move || {
            register(slot);
            crate::user::run(
                image,
                Spawn {
                    arg0: role,
                    arg1: slot as u64,
                    arg2: 2 * slot as u64 + 1,
                    grants: &[rendezvous_cap(ep, rights)],
                    maps: &[Mapping {
                        va: soak_page::VA,
                        phys,
                        flags: paging::Flags::user_data(),
                    }],
                },
            )
        })
        .expect("ipc_stack_depth: could not spawn a soaker")
    };
    let responder = spawn(0, SOAK_RESPONDER, Rights::READ);
    let caller = spawn(1, SOAK_CALLER, Rights::WRITE);

    // Wall clock, not a yield count, for `Holding`'s own reason (threads may run on other cores).
    let deadline = crate::arch::timer::now() + 30 * crate::arch::timer::frequency();
    let filled = || {
        EL0[0].series[abi::rendezvous::RECEIVE_CAP as usize].is_full()
            && EL0[0].series[abi::reply::REPLY as usize].is_full()
            && EL0[1].series[abi::rendezvous::CALL as usize].is_full()
    };
    while !filled() && crate::arch::timer::now() < deadline {
        sched::yield_now();
    }
    EL0_ARMED.store(false, Ordering::Release);

    let mut held = Holding::new();
    held.add_thread(responder);
    held.add_thread(caller);
    held.add_region(region);
    held.release_or_fail("ipc_stack_depth's EL0 soaker pair");
    // Only now: a live soaker was writing to it until `release` returned.
    crate::memory::free(page);
    Some(())
}

/// Both EL0 shapes, then their report, in the same form as [`kernel_thread_shapes`].
#[cfg(initial_ramdisk)]
pub(crate) fn el0_shapes() -> usize {
    use abi::rendezvous::{CALL, RECEIVE, RECEIVE_CAP, SEND};
    let mut bad = 0;
    if send_receive_el0().is_some() {
        bad += report(
            "el0",
            "send_receive",
            "client",
            "SEND",
            &EL0[1].series[SEND as usize],
        );
        bad += report(
            "el0",
            "send_receive",
            "client",
            "RECEIVE",
            &EL0[1].series[RECEIVE as usize],
        );
        bad += report(
            "el0",
            "send_receive",
            "server",
            "RECEIVE",
            &EL0[0].series[RECEIVE as usize],
        );
        bad += report(
            "el0",
            "send_receive",
            "server",
            "SEND",
            &EL0[0].series[SEND as usize],
        );
    }
    if call_reply_el0().is_some() {
        let reply = abi::reply::REPLY as usize;
        bad += report(
            "el0",
            "call_reply",
            "client",
            "CALL",
            &EL0[1].series[CALL as usize],
        );
        bad += report(
            "el0",
            "call_reply",
            "server",
            "RECEIVE_CAP",
            &EL0[0].series[RECEIVE_CAP as usize],
        );
        bad += report(
            "el0",
            "call_reply",
            "server",
            "REPLY",
            &EL0[0].series[reply],
        );
    }
    reset_el0();
    bad
}

/// Print one series as a line and say whether it is a measurement.
///
/// Returns 1 for too few samples or a median at the floor, where the number is the instrument's own
/// frames rather than the kernel's depth; 0 otherwise. A minimum at the floor under a higher median
/// is still a measurement: the shorter of an operation's two paths, such as a RECEIVE that does not
/// block.
fn report(plane: &str, shape: &str, role: &str, operation: &str, s: &Series) -> usize {
    let n = s.n.load(Ordering::Relaxed).min(SAMPLES);
    let mut v = [0u32; SAMPLES];
    for (dst, src) in v.iter_mut().zip(s.depth.iter()).take(n) {
        *dst = src.load(Ordering::Relaxed);
    }
    let v = &mut v[..n];
    v.sort_unstable();
    let floor = s.floor.load(Ordering::Relaxed);
    let caller = s.caller.load(Ordering::Relaxed);
    if n == 0 {
        println!("ipc-stack-depth: {plane} {shape} {role} {operation} NO SAMPLES");
        return 1;
    }
    let (min, median, max) = (v[0] as u64, v[n / 2] as u64, v[n - 1] as u64);
    if median <= floor {
        // Not a depth: the operation stayed inside the instrument's own frames, so print the bound.
        println!(
            "ipc-stack-depth: {plane} {shape} {role} {operation} AT FLOOR: at most {floor}, shallower than \
             the instrument can see (max {max}, samples {n})"
        );
        return 1;
    }
    // Below the measuring frame: the operation's own cost. Zero caller offset for EL0.
    println!(
        "ipc-stack-depth: {plane} {shape} {role} {operation} median {median} min {min} max {max} \
         below-caller {} floor {floor} samples {n}",
        median - caller,
    );
    usize::from(n < SAMPLES / 2)
}

#[cfg(test)]
mod tests {
    /// Every IPC series is a measurement: one `ipc-stack-depth:` line per role and operation.
    ///
    /// Asserts enough samples and a median below the paint ceiling, not the numbers themselves.
    /// notes/stack-high-water/per-ipc-depth.md carries the readings.
    #[test_case]
    fn one_ipc_reaches_a_measured_depth_into_its_kernel_stack() {
        let bad = super::kernel_thread_shapes();
        #[cfg(initial_ramdisk)]
        let bad = bad + super::el0_shapes();
        assert_eq!(
            bad, 0,
            "{bad} ipc-stack-depth series were not measurements (see the lines above): too few \
             samples, a median at the paint floor, or an instrument that reached below its margin"
        );
    }
}
