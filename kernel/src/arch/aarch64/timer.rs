//! The ARM Generic Timer: the preemption source.
//!
//! # Why this lives in `arch/` and not `drivers/`
//!
//! It is not an MMIO device. It is **part of the CPU**, reached through system registers, and
//! every aarch64 core has one whether the board's designer wanted it or not. There is no base
//! address to discover and nothing in the device tree to look up except which interrupt it
//! raises.
//!
//! That is a real convenience and it is why aarch64 kernels can have a working clock before
//! they can enumerate a single peripheral.
//!
//! # The registers
//!
//! | | |
//! |---|---|
//! | `CNTFRQ_EL0` | how fast the counter ticks. **Set by firmware, read by us.** QEMU says 62.5 MHz. |
//! | `CNTVCT_EL0` | the counter itself: a 64-bit number that only ever goes up |
//! | `CNTV_CVAL_EL0` | an **absolute deadline**. Fire when `CNTVCT_EL0` reaches this. |
//! | `CNTV_TVAL_EL0` | a **relative countdown**, which is just `CVAL = CNTPCT + N`. A trap. |
//! | `CNTV_CTL_EL0` | enable, mask, and a read-only "did it fire" bit |
//!
//! `CNTVCT_EL0` is what `Instant` is made of. It never wraps in any timescale that matters (at
//! 62.5 MHz, 64 bits is about 9000 years) and it does not stop when interrupts are masked,
//! which makes it the only honest way to measure a critical section.
//!
//! # Re-arming is not optional
//!
//! The timer is **one-shot**. It counts down, fires, and then sits there with its "I fired" bit
//! set, raising the interrupt line forever. The handler must set a new deadline, and *that
//! write is what lowers the line*.
//!
//! Forget it and the timer fires exactly once and then the machine wedges in a permanent
//! interrupt storm, which looks nothing like "you forgot to write a register".
//!

//! # And re-arming with TVAL silently loses ticks
//!
//! We shipped this bug and then measured it. `TVAL` is a **relative** countdown: writing N
//! means "fire N ticks from *now*". So re-arming with `TVAL = interval` in the handler gives a
//! real period of
//!
//! ```text
//!     interval  +  however long it took to get into the handler and back
//! ```
//!
//! Every tick starts its countdown *late*, and the lateness is never recovered. **The clock
//! runs slow, permanently, and nothing tells you.** Measured under QEMU: 100 Hz configured,
//! ~70 Hz observed. Thirty percent of our preemptions, gone.
//!
//! `CVAL` is an **absolute** deadline: fire when the counter reaches this exact value. Set the
//! next one to `previous_deadline + interval` and the deadlines sit on a **fixed grid**. A slow
//! handler makes one tick *late*; it does not make the next one late as well.
//!
//! This is the difference between a clock that drifts and a clock that doesn't, and it is one
//! register.

// Four accessors below (`ticks`, `missed_ticks`, `uptime_ms`, `interval`) have no caller outside
// this file's own test module on aarch64: the tick handler re-arms from CVAL rather than from
// `interval()`, and the boot banner prints a tick count only on the RISC-V tour. They are each
// marked `cfg_attr(not(test), allow(dead_code))` rather than the whole module being suppressed,
// so a test that stops exercising one is a gate failure instead of silence. (The header comment
// this replaces predicted callers in "milestone 6 and 8", both long since shipped without them.)

use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use aarch64_cpu::registers::{CNTFRQ_EL0, CNTKCTL_EL1, CNTV_CTL_EL0, CNTV_CVAL_EL0, CNTVCT_EL0};
use tock_registers::interfaces::{Readable, Writeable};

use crate::cpu::{self, MAX_CPUS};

/// The EL1 **virtual** timer, as a GIC interrupt ID.
///
/// **A PPI, not an SPI**, and the device tree says so: `interrupts = <1 11 ...>` on the timer
/// node, where type 1 means PPI and 11 is the PPI number. PPIs start at INTID 16, so
/// `16 + 11 = 27`.
///
/// It *has* to be per-core. A timer that fired on only one core could not preempt threads
/// running on the others, so every core has its own, wearing the same number.
///
/// # Why the *virtual* timer, and not the physical one (INTID 30)
///
/// We used the physical timer (`CNTP_*`, INTID 30) through milestone 9, and it worked on QEMU's
/// software CPU and would work on bare metal. It **traps under a hypervisor**: the physical timer
/// belongs to EL2, and a guest at EL1 that writes `CNTP_CVAL_EL0` takes an "Unknown reason" trap
/// (ESR EC 0x00). We found this the first time we booted under Apple's Hypervisor.framework on an
/// M3, which is exactly the "which assumptions were secretly QEMU-shaped" moment
/// design/decisions/ and notes/portability.md anticipate for a new target, arriving early because
/// HVF runs the real
/// core.
///
/// The **virtual** timer (`CNTV_*`, INTID 27) is the one a guest is meant to use, and it is
/// available at EL1 both on bare metal and under any hypervisor. So this is strictly more
/// portable: it keeps working under QEMU/TCG, under HVF, and on a real board, with no
/// per-environment branching. See notes/virtualization.md.
pub(crate) const TIMER_INTID: u32 = 27;

/// 100 Hz. Ten milliseconds per tick.
///
/// The classic tradeoff, and it is a real one. Faster ticks mean finer-grained preemption (a
/// thread cannot hog the CPU for longer than one tick) but more time spent in the handler
/// doing nothing useful. Linux ships 250 Hz and can be built tickless; 100 Hz is the old Unix
/// default and it is plenty for a kernel with no threads yet.
///
/// **1 Hz in the bench build** (milestone 939 (the icount bench runs without a preemption tick in
/// its windows)), so a timed window opened just after a tick has a full second of virtual time
/// before the next one. `kernel/src/bench/timing.rs`'s `TickWindow` opens every row's window that
/// way and fails the run if a tick lands inside one.
pub const TICK_HZ: u64 = if cfg!(feature = "bench") { 1 } else { 100 };

/// Every tick, forever. The heartbeat.
/// Per-CPU, because each core has its own timer (a banked PPI). A single global counter would be
/// advanced by every core's tick, which breaks the "holding a lock masks *my* timer" invariant the
/// tests check: a lock masks only the holding core's interrupts, not the others'. See DECISIONS §11.
static TICKS: [AtomicU64; MAX_CPUS] = [const { AtomicU64::new(0) }; MAX_CPUS];

/// Counter ticks between interrupts. Computed from `CNTFRQ_EL0`, never hardcoded: the
/// frequency is a property of the board, and a hardcoded one would make our 10 ms into
/// something else entirely on a Pi.
static INTERVAL: AtomicU64 = AtomicU64::new(0);

/// **Does this core have the register the cycle-counter grant is written to** (milestone 229).
/// `PMUSERENR_EL0` exists only with `FEAT_PMUv3`, so `init` reads `ID_AA64DFR0_EL1.PMUVer` once per
/// core and records the answer here rather than re-reading an ID register on every context switch.
/// Per core because `PMUVer` is a per-PE ID register and this kernel does not assume a homogeneous
/// machine anywhere else either.
static PMU_PRESENT: [AtomicBool; MAX_CPUS] = [const { AtomicBool::new(false) }; MAX_CPUS];

/// **What this core last wrote to `PMUSERENR_EL0`**, as a bool: open (`CR` set) or closed (zero).
/// `init` writes zero, so `false` is the truth at boot on every core.
///
/// This is a cache rather than a read-back, which is the one place `set_cycle_counter_grant` below
/// departs from `mmu::switch_user_root`'s shape (plain backticks rather than a doc link because
/// that function is behind `cycle_counter_grant` and this static is not, milestone 237), and the
/// reason is the register above: on a part
/// without `FEAT_PMUv3` an `mrs` from `PMUSERENR_EL0` is as UNDEFINED as an `msr` to it, so "read
/// what the hardware holds" is not available on the architecture the way it is for `TTBR0_EL1`.
/// Nothing else in this kernel writes `PMUSERENR_EL0` after `init`, so the cache cannot go stale
/// behind our back. `arch::pmu` (milestone 74) writes the PMU's other registers and deliberately
/// not this one; `close_cycle_counter_to_el0`'s doc comment says why.
static COUNTER_OPEN: [AtomicBool; MAX_CPUS] = [const { AtomicBool::new(false) }; MAX_CPUS];

/// Start the heartbeat.
///
/// The GIC must already be up: we ask it to deliver INTID 30, and it has to exist to be asked.
pub fn init() {
    let freq = CNTFRQ_EL0.get();
    assert!(
        counter_frequency_contract::is_plausible(freq),
        "CNTFRQ_EL0 reports {freq} Hz, which no real part does: firmware never wrote it, or wrote \
         nonsense"
    );

    // Let EL0 read the virtual counter (`CNTVCT_EL0`) and `CNTFRQ_EL0`, so a userspace program can
    // time *itself*, the way Linux exposes the counter to its vDSO. Without this the read traps.
    //
    // This is a deliberate, eyes-open exception to §10's no-ambient-authority rule (notes/abi.md).
    // A monotonic counter grants no authority to *affect* anything, only to observe the passage of
    // time; the cost is that it is a timing side channel, which every OS that offers userspace
    // timing accepts. We accept it too, and we accept it *knowingly*, because the cross-OS primitive
    // suite needs userspace self-timing to be comparable to lmbench (which measures from userspace).
    // The physical counter and the timer registers stay trapped; only the virtual counter opens.
    // `aarch64_cpu` gives CNTKCTL_EL1 no named fields, so set the bit by hand: EL0VCTEN is bit 1.
    const EL0VCTEN: u64 = 1 << 1;
    CNTKCTL_EL1.set(CNTKCTL_EL1.get() | EL0VCTEN);

    close_cycle_counter_to_el0();
    // Start this core's cycle counter and check it moves (milestone 74's aarch64 half). Here
    // because this is the per-core init every core runs, beside the register that says whether EL0
    // may read the same counter. Init-time only; nothing on the switch path. See arch/aarch64/pmu.rs.
    super::pmu::init_this_cpu();

    let interval = freq / TICK_HZ;
    INTERVAL.store(interval, Ordering::Relaxed);

    super::irq::enable(TIMER_INTID); // PPI: enabled on this core, no target

    start(interval);
}

/// Write `PMUSERENR_EL0 = 0`, so EL0 cannot read the **cycle** counter.
///
/// # Why this is beside the `CNTKCTL_EL1` write and not in a PMU driver
///
/// The two writes are one decision seen twice. `CNTKCTL_EL1.EL0VCTEN` says EL0 may read the coarse
/// virtual counter; `PMUSERENR_EL0` says whether it may also read the fine one (`PMCCNTR_EL0`) and
/// the event counters. On riscv64 both answers live in a single CSR, `scounteren`, whose `TM` and
/// `CY` bits this project's per-hart timer init now writes together. This is that same pair, split
/// across two registers by the ISA rather than by us, so it belongs at the same site.
///
/// **`arch::pmu` now exists (milestone 74) and still does not own this line.** It starts the
/// counter; this decides who may read it from EL0, and that decision is the context switch's
/// (`set_cycle_counter_grant` below, which `sched` calls as `arch::timer::` on all three ISAs).
/// Moving the boot-time half into `pmu` would split one register's writers across two files, which
/// is what makes the `COUNTER_OPEN` cache above safe to keep: nothing but this file writes it.
///
/// # Why writing zero is not a policy change
///
/// Zero is what the rest of the tree already believes the value is. Nothing here grants or revokes
/// anything a program was using: `crates/user_mode_runtime`'s `now()` reads `CNTVCT_EL0`, which the line above
/// opens, and nothing in this tree reads `PMCCNTR_EL0` from EL0 at all. What changes is that the
/// belief stops being firmware's to confirm.
///
/// Arm says of every field in `PMUSERENR_EL0`, `EN`, `SW`, `CR` and `ER` alike, that on a warm reset
/// it "resets to an architecturally UNKNOWN value". Under QEMU it is almost certainly zero. On
/// the **Jetson TX1** nobody has booted this kernel on (argon), it is whatever TF-A and the boot
/// ROM left, and a claim that rests on that is not a claim. Linux writes the same zero for the same
/// reason, in the commit `arm64: kernel: enforce pmuserenr_el0 initialization and restore`, whose
/// message says the register "is architecturally UNKNOWN on reset".
///
/// Whether EL0 should ever be *granted* the cycle counter is a different question with a different
/// owner: milestone 75 (who may read the cycle counter, and by what authority), which is `Gate:
/// DECISION` and calef's.
///
/// # Why per core
///
/// `PMUSERENR_EL0` is banked per PE, so a secondary that skipped this would run on whatever that
/// core's reset value was, and only the threads that happened to be placed there would see it. That
/// is the identical argument the riscv64 timer records for `scounteren`, and it is why this sits in
/// the per-core `init` rather than in a boot-core-only path.
fn close_cycle_counter_to_el0() {
    // `PMUSERENR_EL0` exists only when FEAT_PMUv3 does; without it a direct access is UNDEFINED and
    // would take an undefined-instruction exception on the first line of every core's timer init.
    // `ID_AA64DFR0_EL1.PMUVer` reports it: 0 means no PMU, 0xf means an IMPLEMENTATION DEFINED PMU
    // that does not follow PMUv3 and so does not carry this register either. Both are boards where
    // there is no EL0 cycle-counter door to close.
    if !super::pmu::is_pmuv3_present() {
        return;
    }
    // Milestone 229 reads this back on the context-switch path, where re-reading an ID register
    // every switch would be paying for an answer that cannot change. Recorded here because this is
    // the one place that already had to work it out.
    PMU_PRESENT[cpu::id()].store(true, Ordering::Relaxed);

    // SAFETY: writing zero to `pmuserenr_el0` only *removes* EL0 permissions; every field of that register
    // is an EL0 enable, so zero cannot make any access newly legal, and it does not affect EL1's own
    // PMU access (which `MDCR_EL2.TPM` and `PMCR_EL0` govern, not this register). Writing it is a
    // legal EL1 operation whenever the register is present, which the PMUVer check above
    // establishes. It touches no memory and clobbers no flags, which the options state.
    unsafe { super::instructions::write_pmuserenr(0) };
    COUNTER_OPEN[cpu::id()].store(false, Ordering::Relaxed);
}

/// **Can a thread on this core be granted the cycle counter at all?** False on a part with no
/// `FEAT_PMUv3`, where `PMCCNTR_EL0` and the register that gates it both simply do not exist.
///
/// The grant is still *accepted* on such a part (a manifest is a declaration, and refusing it at
/// `START` would make a program un-runnable on a board rather than merely un-instrumented); this is
/// what lets a test say "there is nothing here to measure" instead of faulting.
///
/// **Built only under `test` or `--features cycle_counter_grant`** (milestone 237): the grant is
/// a measurement build the way `soak_test` is. `kernel/Cargo.toml`'s feature block carries the
/// reasoning and the measured cost. Milestone 228's closed default at `init` is NOT gated.
// Asked only by tests today (`sched`'s grant round trip and `user`'s EL0 one), which are the
// callers that have to skip rather than fault on a part with no counter to grant. Milestone 74's
// aarch64 half answers the neighbouring question (does the counter *run*) in `arch::pmu::outcome`,
// and deliberately did not fold the two: a counter can be grantable and stuck.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
#[cfg(any(test, feature = "system_tests", feature = "cycle_counter_grant"))]
pub fn is_cycle_counter_grantable() -> bool {
    PMU_PRESENT[cpu::id()].load(Ordering::Relaxed)
}

/// **Open or close EL0's view of `PMCCNTR_EL0` for the thread about to run** (milestone 229,
/// DECISIONS 139 option 4).
///
/// Called from the context switch, on every switch, right beside
/// [`mmu::switch_user_root`](crate::arch::mmu::switch_user_root), and deliberately the same shape:
/// compare first, write only when the value changes, so with no granted thread on this core the
/// cost is a load, a compare and a return.
///
/// # What it writes
///
/// `PMUSERENR_EL0.CR` (bit 2), and nothing else. `CR` is the field that permits an EL0 read of
/// `PMCCNTR_EL0` specifically; `EN` (bit 0) would open every PMU register at once and `ER` (bit 3)
/// the event counters, and neither is what DECISIONS 139 granted. So a granted thread runs with
/// exactly `CR` set and every other thread runs with the register at zero, which is what
/// [`close_cycle_counter_to_el0`] established at boot.
///
/// # What it is not
///
/// It is not timing confinement. An ungranted thread can still time itself against `CNTVCT_EL0`
/// (41 ns), and two threads sharing a word reconstruct a far finer clock than that; see
/// `notes/confinement-claims.md`. What closing the register buys is that the *cheap accurate*
/// instrument is granted rather than ambient.
///
/// # No barrier, on purpose
///
/// There is no `isb` here: the only way back to EL0 from this path is the `eret` that ends the
/// return-to-user sequence, a context-synchronizing event. Linux's arm64 per-task hook does the
/// same.
///
/// # BUGS
///
/// **Under QEMU with Hypervisor.framework, closing does not close.** On an Apple core,
/// `user::tests::a_granted_thread_reads_the_cycle_counter_and_an_ungranted_one_faults` fails its
/// negative half with this register at zero: the PMU access likely traps to the hypervisor and is
/// answered without consulting the guest's `PMUSERENR_EL0`. Under TCG it passes; silicon is
/// untested, and an HVF run is not evidence either way.
///
/// **Built only under `test` or `--features cycle_counter_grant`** (milestone 237): the grant is
/// a measurement build the way `soak_test` is. `kernel/Cargo.toml`'s feature block carries the
/// reasoning and the measured cost. Milestone 228's closed default at `init` is NOT gated.
#[cfg(any(test, feature = "system_tests", feature = "cycle_counter_grant"))]
pub(crate) fn set_cycle_counter_grant(granted: bool) {
    let cpu = cpu::id();
    if COUNTER_OPEN[cpu].load(Ordering::Relaxed) == granted {
        return;
    }
    if !PMU_PRESENT[cpu].load(Ordering::Relaxed) {
        // No `FEAT_PMUv3`: there is no register to write and nothing to open. Leaving the cache
        // `false` means a granted thread costs the same two loads every switch here rather than
        // one, on a board where the grant can never be honoured anyway.
        return;
    }

    // `CR`, bit 2 of `PMUSERENR_EL0`: "EL0 using AArch64: EL0 access to `PMCCNTR_EL0` is enabled."
    const CR: u64 = 1 << 2;
    let value = if granted { CR } else { 0 };

    // SAFETY: `PMUSERENR_EL0` is present, which the `PMU_PRESENT` check above establishes from this
    // core's own `ID_AA64DFR0_EL1.PMUVer`, so the `msr` is a legal EL1 operation and not UNDEFINED.
    // Every field of the register is an EL0 *enable*, so no value written here can affect what EL1
    // may do, nor make any EL1 access newly legal; EL1's own PMU access is governed by `MDCR_EL2`
    // and `PMCR_EL0`. It touches no memory and clobbers no flags, which the options state.
    unsafe { super::instructions::write_pmuserenr(value) };
    COUNTER_OPEN[cpu].store(granted, Ordering::Relaxed);
}

/// Set the first deadline and enable.
///
/// `IMASK` clear means "and actually raise the interrupt line". The timer will happily count
/// down and set its status bit with the interrupt masked; the mask only stops the line.
fn start(interval: u64) {
    CNTV_CVAL_EL0.set(CNTVCT_EL0.get() + interval);
    CNTV_CTL_EL0.write(CNTV_CTL_EL0::ENABLE::SET + CNTV_CTL_EL0::IMASK::CLEAR);
}

/// Move the deadline forward by exactly one interval.
///
/// **`previous + interval`, not `now + interval`.** That is the entire point: the deadlines sit
/// on a fixed grid anchored at boot, so however long the handler took, the next tick is still
/// where it always was going to be. A slow handler makes *one* tick late; it does not push the
/// next one out too.
///
/// The `if` is the safety valve. If we fell so far behind that the next deadline is *already in
/// the past*, we would fire again immediately, and again, and spin in the handler forever
/// trying to catch up on a debt we cannot pay. So we give up on the missed ticks and re-anchor
/// the grid to now. Linux calls this the same thing every kernel calls it: dropping ticks.
fn rearm(interval: u64) {
    let now = CNTVCT_EL0.get();
    // The deadline that just fired, named rather than folded into the sum below: milestone 78's
    // instruction-count instrument compares the arrival against it, and a value read after the
    // write is a different value.
    let fired = CNTV_CVAL_EL0.get();
    let mut next = fired + interval;

    if next <= now {
        MISSED_TICKS[cpu::id()].fetch_add(1, Ordering::Relaxed);
        // Test builds only: keep the numbers, so a failure says HOW LATE rather than only that it
        // was late. Two relaxed stores, no branch on the hot path, and nothing printed: this runs in
        // interrupt context and DECISIONS §9's rule (handlers record and defer) applies to
        // diagnostics too.
        #[cfg(any(test, feature = "system_tests"))]
        miss_detail::record(now, next);
        next = now + interval;
    }

    CNTV_CVAL_EL0.set(next);

    // The instruction-count instrument (milestone 78), in the boot mode that owns it and nowhere
    // else: three relaxed counters, after the deadline is armed so the measured span is the whole
    // handler rather than a prefix of it. Absent from the test and shipping builds, so what this
    // number describes is the handler that ships. See kernel/src/icount.rs.
    #[cfg(feature = "icount")]
    crate::icount::tick_trace::record(fired, now, CNTVCT_EL0.get());
}

/// **Instructions between a deadline and the handler observing it** (milestone 78's instrument),
/// the ceiling `icount::run` asserts.
///
/// Interrupt delivery, the vector, the register save, `exception_dispatch`, the GIC acknowledge and
/// `tick`'s counter bump, in the debug build this boot compiles. Measured rather than reasoned: the
/// run prints what it saw beside this number, so the margin is visible. The value is a ceiling with
/// room for ordinary codegen movement, not a baseline; a change that halves it is not a failure and
/// a change that doubles it is a fact worth stopping for.
#[cfg(feature = "icount")]
pub(crate) const ARRIVAL_BOUND: u64 = 2_000;

/// **Instructions from a deadline to the next one being armed** (milestone 78's instrument): the
/// whole handler, which is [`ARRIVAL_BOUND`]'s span plus the tick bookkeeping and the `CNTV_CVAL_EL0`
/// write.
///
/// This is the claim the missed-tick assertions could never make. A miss is this number exceeding
/// one tick period, which is **10,000,000 instructions** of virtual time at 100 Hz: ten
/// milliseconds, and one instruction is one nanosecond. So the bound here is about **4,000 times**
/// tighter than the thing it replaces, and unlike it, nothing the host does can move it.
///
/// *(Corrected 2026-08-18. This said "625,000 instructions of virtual time", which is the interval
/// in **counter ticks** wearing instructions' units; at 16 instructions per counter tick the two
/// differ by that factor, and `script/icount` prints both side by side as `tick_interval 625000
/// 10000000`. notes/instruction-clock.md carried the same slip and is fixed with it.)*
#[cfg(feature = "icount")]
pub(crate) const HANDLER_BOUND: u64 = 2_500;

/// Execute `2 * iters` instructions and return that count, so `icount::calibrate` can check the
/// virtual clock against a known instruction count and refuse to measure anything if this boot is
/// not on the instrument.
///
/// The loop is written in assembly on purpose. The whole value of the check is that the expected
/// instruction count is *known*, and a Rust loop's count is whatever the optimizer decided this
/// week. `subs`/`b.ne` is two instructions per iteration on every aarch64 that has ever existed.
///
/// **The return is the loop body only.** Materializing the operand costs the compiler a `mov` or
/// two either side, which is why the caller's tolerance is a percentage of a seven-figure window
/// rather than an equality: a handful of setup instructions must not be the thing that decides
/// whether the instrument is believed.
///
/// Here rather than in `icount.rs` because DECISIONS §3 puts every `asm!` under `arch/`.
#[cfg(feature = "icount")]
pub(crate) fn calibration_loop(iters: u64) -> u64 {
    // SAFETY: a self-contained counted loop in a caller-saved scratch register. It touches no
    // memory, makes no call, and leaves the register dead, which is what the operand spec and
    // `nomem`/`nostack` state.
    unsafe {
        core::arch::asm!(
            "2:",
            "subs {n}, {n}, #1",
            "b.ne 2b",
            n = inout(reg) iters => _,
            options(nomem, nostack),
        );
    }
    2 * iters
}

/// Deadlines that had already passed by the time we re-armed. **Should be zero.** A nonzero
/// count means the handler is taking longer than a whole tick period, which is a real problem
/// and not a rounding error.
static MISSED_TICKS: [AtomicU64; MAX_CPUS] = [const { AtomicU64::new(0) }; MAX_CPUS];

/// This core's missed ticks.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))] // this file's tests are the callers
pub fn missed_ticks() -> u64 {
    missed_ticks_on(cpu::id())
}

/// A named core's missed ticks. The [`ticks_on`] argument, applied to the miss count: a test that
/// reads it either side of a wait must name the core, or a migration compares two unrelated
/// counters.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub fn missed_ticks_on(cpu: usize) -> u64 {
    MISSED_TICKS[cpu].load(Ordering::Relaxed)
}

/// This core's next armed deadline: `CNTV_CVAL_EL0`, the grid cell [`rearm`] advances from.
///
/// Exposed so the drift test can assert the re-arm law directly (deadlines advance by exactly one
/// interval per delivered tick) instead of inferring it from a wall-clock tick rate, which a
/// descheduled emulator falsifies. The register is banked per core, like the counter.
///
/// **Two callers now**, which is milestone 62's shape: the suite's test, which measures the law on
/// a wall clock and may report `UNMEASURED` when a loaded host denies it a miss-free window, and
/// `icount::run`'s claim 4, which measures the same law in instructions and always answers.
#[cfg_attr(
    all(not(any(test, feature = "system_tests")), not(feature = "icount")),
    allow(dead_code)
)]
pub fn deadline() -> u64 {
    CNTV_CVAL_EL0.get()
}

/// Why a miss happened, kept only in test builds.
///
/// `missed_ticks()` says a deadline was already past when the handler re-armed. It does not say by
/// how much, and the difference matters: **a few hundred cycles late is a slow handler, and a whole
/// tick period late is the emulator having been descheduled.** Without the numbers those two look
/// identical in a panic message, which is the position milestone 78 records the suite being in.
///
/// The last miss only, plus a count. A burst records once and reports the final pair, which is
/// enough to tell the two cases apart and cheaper than a ring buffer in interrupt context.
#[cfg(any(test, feature = "system_tests"))]
pub mod miss_detail {
    use core::sync::atomic::{AtomicU64, Ordering};

    use crate::cpu::{self, MAX_CPUS};

    static NOW: [AtomicU64; MAX_CPUS] = [const { AtomicU64::new(0) }; MAX_CPUS];
    static NEXT: [AtomicU64; MAX_CPUS] = [const { AtomicU64::new(0) }; MAX_CPUS];
    static COUNT: [AtomicU64; MAX_CPUS] = [const { AtomicU64::new(0) }; MAX_CPUS];

    /// Record the counter and the deadline it had already passed. Called from `rearm`, in
    /// interrupt context, so this is three relaxed stores and nothing else.
    pub(crate) fn record(now: u64, next: u64) {
        let id = cpu::id();
        NOW[id].store(now, Ordering::Relaxed);
        NEXT[id].store(next, Ordering::Relaxed);
        COUNT[id].fetch_add(1, Ordering::Relaxed);
    }

    /// This core's last miss as `(now, next, count)`. `now - next` is how late the re-arm was, in
    /// counter ticks; compare it against `timer::interval()` to tell a slow handler from a
    /// descheduled emulator.
    pub fn last() -> (u64, u64, u64) {
        let id = cpu::id();
        (
            NOW[id].load(Ordering::Relaxed),
            NEXT[id].load(Ordering::Relaxed),
            COUNT[id].load(Ordering::Relaxed),
        )
    }
}

/// Called from the IRQ handler. **Must re-arm**, or the interrupt line stays high forever and
/// the machine drowns in its own timer.
///
/// This is the whole handler, and it is deliberately tiny: bump a counter, reload the
/// countdown, return. DECISIONS §9: **interrupt handlers record and defer; they do not do
/// work.** At milestone 6 this will also set a "reschedule wanted" flag, and the *scheduler*
/// will act on it in normal context.
pub(crate) fn tick() {
    TICKS[cpu::id()].fetch_add(1, Ordering::Relaxed);
    // Test builds only: watch for a hung test (a lost IPC wakeup) and fail fast with a diagnostic
    // instead of blocking the run forever. Costs a couple of atomic loads per tick.
    #[cfg(any(test, feature = "system_tests"))]
    crate::testing::watchdog_tick();
    rearm(INTERVAL.load(Ordering::Relaxed));
}

/// This core's ticks since it started.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))] // this file's tests are the callers
pub fn ticks() -> u64 {
    ticks_on(cpu::id())
}

/// Ticks since boot on a **named** core, which is what a test needs when the thread doing the
/// measuring can move between the two reads.
///
/// [`TICKS`] is per core, so `ticks()` before a wait and `ticks()` after it are the same counter
/// only if nothing migrated the caller in between; a kernel thread on a run queue can be stolen by
/// an idle core at any preemption point (DECISIONS §28.3). Reading by index makes the pair name one
/// core on purpose. See notes/load-sensitive-assertions.md.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub(crate) fn ticks_on(cpu: usize) -> u64 {
    TICKS[cpu].load(Ordering::Relaxed)
}

/// The raw counter. Monotonic, never wraps in any timescale that matters, and **keeps counting
/// while interrupts are masked**, which is precisely what makes it the only honest way to
/// measure how long a critical section held the CPU.
pub fn now() -> u64 {
    CNTVCT_EL0.get()
}

pub fn frequency() -> u64 {
    CNTFRQ_EL0.get()
}

/// **Cross-check `CNTFRQ_EL0` against what the device tree says the timer runs at**, and refuse the
/// boot if the two disagree.
///
/// This architecture is the one that needs no timebase page: `CNTFRQ_EL0` is architected, EL0 may
/// read the same register the kernel does (`init` opens `CNTKCTL_EL1.EL0VCTEN`), so there is one
/// source and nothing to keep in sync. That is a strong position and it has one weakness, which the
/// register table at the top of this file already states in three words: **set by firmware**. The
/// part does not know its own counter frequency; something running before us wrote it, and on real
/// ARM boards that something is wrong often enough that the device-tree binding carries an override
/// for it. `arm,armv8-timer`'s optional `clock-frequency` property exists for exactly that case, and
/// Linux honours it in preference to the register.
///
/// **This refuses rather than preferring either one.** Linux takes the override because Linux must
/// boot on the board in front of it; this kernel's ruling is calef's, 2026-09-21, that a wrong
/// number is worse than no number, and two sources that disagree mean we do not know which is
/// right. Preferring the device tree would be a guess with a rationale, and the failure it hides is
/// the one this whole change exists to stop: a plausible rate that silently scales every
/// measurement. Reversible: a board that turns up with a bad register and a good tree is an
/// argument for preferring the tree, and it is an argument somebody can make with that board's
/// numbers in hand.
///
/// **Unexercised on silicon, and that is stated rather than implied.** QEMU's `virt` machine states
/// no `clock-frequency` (measured 2026-09-21 with `-machine virt,dumpdtb=`: the `timer` node carries
/// `interrupts`, `always-on` and `compatible`, and nothing else), so on every machine this kernel is
/// tested on today this function finds no property and returns without comparing anything. The first
/// board that exercises it is argon, the Jetson TX1, which is the seL4 comparison bench and so the
/// one machine where a wrong rate would corrupt the numbers this project is most measured on. Until
/// something reports a real mismatch, the only thing proven here is that the absent-property path
/// does not stop a boot.
///
/// # Panics
///
/// If the device tree states a rate and the register disagrees with it, naming both values. Also if
/// the tree is unreadable, which is the same `expect` `isa::init` makes of the same pointer one
/// call earlier.
pub(crate) fn check_frequency_against_device_tree(dtb_ptr: usize) {
    // SAFETY: the pointer firmware handed us, whose magic `memory::init` has already checked, named
    // through the boot map's direct region. Same read `isa::init` makes of the same pointer.
    let dt = unsafe {
        device_tree_blob::DeviceTreeBlob::from_ptr(
            super::mmu::phys_to_virt(dtb_ptr as u64) as *const u8
        )
    }
    .expect("device tree is unreadable");

    let Ok(Some(bytes)) = dt.node_prop_compatible(b"arm,armv8-timer", b"clock-frequency") else {
        return;
    };
    // The binding spells this one cell, and a property that is not one cell is a tree we do not
    // understand rather than a mismatch to panic about.
    let Ok(cell) = <[u8; 4]>::try_from(bytes) else {
        return;
    };
    let stated = u64::from(u32::from_be_bytes(cell));

    let register = CNTFRQ_EL0.get();
    assert_eq!(
        stated, register,
        "the device tree says this timer runs at {stated} Hz and CNTFRQ_EL0 says {register} Hz; \
         one of them is wrong and nothing here can tell which"
    );
}

/// Milliseconds since boot, from the counter rather than from the tick count.
///
/// **Deliberately not `ticks() * 10`.** If an interrupt is ever missed: a long critical
/// section, a slow handler: the tick count undercounts and time appears to slow down. The
/// hardware counter cannot lie. This is `Instant`, and it is the thing `core` could never give
/// us because nothing in `core` knows what time it is.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))] // this file's tests are the callers
pub fn uptime_ms() -> u64 {
    let freq = CNTFRQ_EL0.get();
    if freq == 0 {
        return 0;
    }
    now() * 1000 / freq
}

/// **Is this core's tick raised and waiting** (`CNTV_CTL_EL0.ISTATUS`)? True once the deadline has
/// passed and the timer has signalled it, until the handler moves `CNTV_CVAL_EL0` forward, whether
/// or not interrupts are masked.
///
/// The twin of the riscv64 `is_tick_pending`, whose comment has the measurement: the emulator
/// raises the timer from its own main loop, milliseconds and occasionally tens of milliseconds
/// after the deadline, so a test that assumes a tick is pending after a fixed masked spin can be
/// measuring the host. The tests that hold a tick across a mask wait for this bit instead. See
/// notes/load-sensitive-assertions.md.
///
/// Name: ratified 2026-09-24 (calef, the Rust predicate-naming rule in design/naming.md). Refused
/// `tick_pending` (a bare participle reads as a getter, and Rust asks the question with `is_`).
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub fn is_tick_pending() -> bool {
    CNTV_CTL_EL0.is_set(CNTV_CTL_EL0::ISTATUS)
}

/// Busy-wait. Uses the counter, so it works with interrupts masked, which is exactly when a
/// tick-based delay would hang forever.
///
/// The callers are the milestone tour and the tests. A `bench` boot diverges before the tour and
/// runs no tests, so in that configuration this has no caller.
#[cfg_attr(feature = "bench", allow(dead_code))]
pub fn spin_for(counter_ticks: u64) {
    let start = now();
    while now().wrapping_sub(start) < counter_ticks {
        core::hint::spin_loop();
    }
}

/// Counter ticks in one timer period.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))] // this file's tests are the callers
pub fn interval() -> u64 {
    INTERVAL.load(Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    //! Tests for the timer, and for the thing the whole locking discipline was written to
    //! prevent.

    /// **The cycle-counter grant opens and closes `PMUSERENR_EL0.CR`, and opens nothing else**
    /// (milestone 229, DECISIONS 139 option 4).
    ///
    /// Reads the register back out of the core rather than out of the cache
    /// [`set_cycle_counter_grant`](super::set_cycle_counter_grant) keeps, which is the only way the
    /// cache can be caught lying. `EN` (bit 0) and `ER` (bit 3) stay clear throughout: DECISIONS
    /// 139 granted the cycle counter, not the event counters and not the whole PMU.
    ///
    /// Four calls where only two change anything, because the context switch makes this call on
    /// every switch and the cheap path has to be right as well as fast. Leaves the register at
    /// zero, which is where `init` left it and where every other test expects it.
    #[test_case]
    fn the_cycle_counter_grant_moves_cr_and_nothing_else() {
        if !super::is_cycle_counter_grantable() {
            crate::testing::skip!("this core has no PMUv3, so there is no register to grant");
        }
        super::set_cycle_counter_grant(true);
        super::set_cycle_counter_grant(true);
        assert_eq!(
            read_pmuserenr(),
            1 << 2,
            "the grant should set CR and only CR",
        );

        super::set_cycle_counter_grant(false);
        super::set_cycle_counter_grant(false);
        assert_eq!(
            read_pmuserenr(),
            0,
            "the cycle counter was left open to EL0"
        );
    }

    /// `PMUSERENR_EL0`, read back out of the core. Only call this where
    /// [`is_cycle_counter_grantable`](super::is_cycle_counter_grantable) is true: without
    /// `FEAT_PMUv3` the read is as UNDEFINED as the write.
    fn read_pmuserenr() -> u64 {
        let value: u64;
        // SAFETY: the caller has checked `is_cycle_counter_grantable`, so the register exists and
        // an `mrs` from it is a legal EL1 operation. It touches no memory and clobbers no flags,
        // which the options state.
        unsafe {
            core::arch::asm!("mrs {}, pmuserenr_el0", out(reg) value, options(nomem, nostack, preserves_flags));
        }
        value
    }
    /// The heartbeat is beating.
    #[test_case]
    fn the_timer_is_ticking() {
        use crate::arch::timer;

        // Name the core. `ticks()` reads a per-core counter and this thread can be migrated by a
        // steal at any preemption point, which would compare two unrelated counters (`ticks_on`).
        let cpu = crate::cpu::id();
        let before = timer::ticks_on(cpu);
        // **Waited for, not spun for.** A fixed three periods asked the emulator to have raised the
        // timer within 30 ms of wall clock, and it raises it from its own main loop: measured on
        // 2026-09-24 at up to 86 ms after the deadline (`is_tick_pending`'s comment has the
        // numbers). A timer that is genuinely dead still fails, a second later.
        let ticked = within_periods(RAISE_BOUND_PERIODS, || timer::ticks_on(cpu) > before);

        assert!(
            ticked,
            "no timer interrupt in three tick periods: the GIC or the timer is not delivering"
        );
    }

    /// Ticks arrive at the configured rate, proven by the grid rather than by the wall clock.
    ///
    /// This is the test that caught the drift. We re-armed with `CNTV_TVAL_EL0` (a *relative*
    /// countdown), so every period was `interval + handler latency`, the lateness compounded,
    /// and 100 Hz became about 70 Hz. Silently. `CNTV_CVAL_EL0` puts the deadlines on a fixed
    /// grid and the rate is right.
    ///
    /// The original assertion compared delivered ticks against elapsed counter time, one period
    /// of slack either way. That measures the emulator as much as the re-arm: the test runner
    /// passes no `-icount`, so `CNTVCT_EL0` follows host time, and a host that deschedules the
    /// vCPU for a few periods coalesces ticks into exactly the deficit the TVAL defect produced.
    /// It failed a quiet-window gate run on 2026-08-03 as "22 ticks in 25 periods" while the
    /// host compiled beside QEMU, the same shape its riscv64 twin kept failing CI with, and no
    /// margin separates "our re-arm is late" from "the emulator was not running"
    /// (notes/load-sensitive-assertions.md).
    ///
    /// So assert the re-arm LAW instead, the property this test exists for: over a window in
    /// which no miss was recorded, [`rearm`] moved `CNTV_CVAL_EL0` by **exactly one interval per
    /// delivered tick**. The TVAL defect fails this on the first tick (each re-arm lands late by
    /// the handler latency, so the sum overshoots the grid); a descheduled emulator cannot fail
    /// it, because a deschedule long enough to slip the grid is counted by `MISSED_TICKS` and
    /// the window is retried. Kept in lockstep with the riscv64 twin, whose grid is software
    /// (`DEADLINE`) because SBI's `set_timer` is write-only where CVAL reads back.
    #[test_case]
    fn ticks_arrive_at_the_configured_rate() {
        use crate::arch::timer;

        // A consistent (ticks, missed, deadline, counter) snapshot without masking: a tick
        // between the reads would skew them, so re-read until the tick count brackets the others
        // unchanged. The core id is in the bracket too: the statics and CVAL are per core, and a
        // snapshot pair taken on two cores compares unrelated grids.
        let snapshot = || loop {
            let cpu = crate::cpu::id();
            let t = timer::ticks();
            let m = timer::missed_ticks();
            let d = timer::deadline();
            let c = timer::now();
            if timer::ticks() == t && crate::cpu::id() == cpu {
                break (cpu, t, m, d, c);
            }
        };

        // A miss re-anchors the grid, which is correct behaviour (`rearm`'s safety valve), so a
        // window containing one proves nothing about the law either way: retry it.
        //
        // **Exhausting the budget is not a failure, and milestone 62 settled that on 2026-08-18.**
        // It used to be. The assertion that stood here read "either the host is too contended to
        // observe the grid, or the handler is slower than a whole tick period", and the word
        // deciding between those two is `or`: this is the family's signature confound, a claim
        // whose truth depends on the host, written from inside a guest that cannot see the host.
        // The acceptance run fired it four times in forty-five loaded runs, twice per ISA, and
        // widening the budget is the one fix DECISIONS §61 and this milestone both forbid by name,
        // because it changes how often you notice rather than what is measured.
        //
        // What is kept is everything below, and it is the whole reason this test exists: the
        // re-arm law is exact, and a host can only cost us the chance to measure it, never the
        // answer. What is given up is the second, implicit claim, which `script/icount` now
        // asserts with no host term in it (claim 2, the handler bounded at 2,500 instructions
        // against a measured 1,056, and claim 3, zero missed ticks) and with the law itself
        // beside them (claim 4, added by this milestone after an injection showed the instrument
        // was blind to the very defect this test catches). `script/ci-build` and CI both run it. See
        // notes/load-sensitive-assertions.md.
        const ATTEMPTS: u32 = 8;
        let mut attempts = 0;
        let measured = loop {
            let (k0, t0, m0, d0, c0) = snapshot();
            timer::spin_for(timer::frequency() / 4); // a quarter of a second, by the counter
            let (k1, t1, m1, d1, c1) = snapshot();

            if k0 == k1 && m0 == m1 && t1 - t0 >= 2 {
                break Some((t1 - t0, d1 - d0, (c1 - c0) / timer::interval()));
            }
            attempts += 1;
            if attempts == ATTEMPTS {
                break None;
            }
        };

        // Loud rather than silent, because a test that quietly measures nothing is worse than one
        // that flakes: the reader has to be able to tell "the law held" from "the law was not
        // looked at". The numbers come with it so a human can triage without re-running, which is
        // what `miss_detail` was added for in the first place.
        let Some((elapsed_ticks, deadline_delta, expected)) = measured else {
            let (now, next, misses) = super::miss_detail::last();
            crate::println!(
                "    (UNMEASURED: no miss-free window in {} tries, so the re-arm law was not \
                 tested this run. {} misses recorded on this core, the last re-armed {} counter \
                 ticks late against an interval of {}. A miss re-anchors the grid, so a window \
                 containing one proves nothing either way, and whether these misses are a \
                 contended host or a slow handler is the one question a wall clock cannot answer \
                 from inside the guest. `script/icount` answers it and asserts this same law with \
                 no host term in it. Milestone 62; notes/load-sensitive-assertions.md.)",
                ATTEMPTS,
                misses,
                now.saturating_sub(next),
                timer::interval(),
            );
            // The diagnostic above is the detail; this is the verdict, and it has to reach the
            // final line. A test that measures nothing is not a pass (milestone 214,
            // design/roadmap/0214-print-and-return-skips.md).
            crate::testing::skip!(
                "no miss-free window in which to measure the re-arm law this run"
            );
        };

        assert_eq!(
            deadline_delta,
            elapsed_ticks * timer::interval(),
            "timer drift: {elapsed_ticks} ticks moved CNTV_CVAL_EL0 off the grid. Re-arming with \
             a RELATIVE countdown (TVAL) instead of an absolute deadline (CVAL) does exactly this."
        );

        // The one wall-clock bound a contended host cannot falsify: descheduling only DROPS
        // ticks, so more ticks than elapsed periods (plus one for starting mid-period) means the
        // timer is firing faster than the grid, which is `rearm`'s spin-forever failure mode.
        assert!(
            elapsed_ticks <= expected + 1,
            "{elapsed_ticks} ticks in {expected} periods: the timer fires faster than configured"
        );
    }

    // **`the_handler_keeps_up_when_no_lock_is_held` was deleted here** (milestone 62, 2026-08-18),
    // and this comment is the argument, because a deleted test leaves no trace and the next person
    // to notice that the timer has no handler-latency test deserves to find out why rather than to
    // rebuild it.
    //
    // What it meant to prove: with interrupts live and no critical section in the way, a missed
    // deadline would mean the handler itself is too slow, which at milestone 6 means threads losing
    // time slices. What it actually measured: the missed-tick delta over five tick periods, with a
    // taxonomy deciding by how late the re-arm was. Less than one interval late was called a slow
    // handler and failed; a whole interval or more was called the emulator descheduled and passed.
    //
    // **Its true-positive band and its false-positive band are the same band**, which is what
    // makes this a deletion rather than a fix. Both were measured by injection on 2026-08-18, on
    // this ISA, against `script/icount` run on the same tree:
    //
    // | handler slow by | this assertion | `script/icount` |
    // |---|---|---|
    // | under 1 period | silent: no miss to classify | fails on the bound, 2,500 instructions |
    // | 1.5 periods | **fails**, correctly, "late by less than one interval" | fails |
    // | 2.5 periods | **passes**, printing "the emulator was descheduled; not this kernel's bug, not failed" | fails: handler 25,001,200 instructions, missed_ticks 32 |
    //
    // The third row is not a false negative so much as a false exoneration, printed, on the worst
    // timer defect this kernel could have. And the middle row is the same band in which milestone
    // 62's acceptance run caught a real host deschedule wearing the slow-handler message twice per
    // ISA (measured at 0.56 and 0.83 of an interval). Inside the band the assertion cannot tell the
    // two apart; outside it, it says nothing or says the wrong thing. No cut fixes that, because
    // from inside the guest a 30 ms handler and a 30 ms deschedule are the same observation.
    //
    // The claim now lives on `script/icount`, denominated in instructions, which the host cannot
    // move: the handler is bounded deadline-to-re-armed at `HANDLER_BOUND` against a measured 1,056
    // with zero variance across 64 consecutive ticks, and `missed_ticks == 0` is asserted with no
    // taxonomy at all. That is roughly 4,000x tighter than "did not exceed one tick period" and it
    // has no second explanation. `miss_detail` survives, now consumed by the drift test's
    // unmeasured-window report. See notes/load-sensitive-assertions.md and
    // notes/instruction-clock.md.

    /// **The cost of masking, made visible.**
    ///
    /// `IrqSafeMutex` prevents the deadlock by masking interrupts for as long as the lock is
    /// held. That is not free, and this is the bill: hold a lock across a tick deadline and the
    /// tick is *late*. Hold it across more than one and a tick is **lost outright**: the
    /// deadline passes, we re-arm to a deadline already in the past, and the only sane thing to
    /// do is give up on it and re-anchor.
    ///
    /// This is exactly why DECISIONS §9 says **keep critical sections short**, and it is the
    /// reason that rule has teeth rather than being good manners. At milestone 6, a lost tick is
    /// a thread that didn't get preempted.
    ///
    /// The test asserts the cost is *real*, which is a strange thing to assert until you notice
    /// that if it ever stopped being real, `IrqSafeMutex` would have stopped masking, and the
    /// deadlock would be back.
    #[test_case]
    fn a_long_critical_section_costs_a_tick() {
        use crate::arch::timer;
        use crate::sync::{IrqSafeMutex, rank};

        static M: IrqSafeMutex<u32> = IrqSafeMutex::new(rank::PAGE_FRAMES, 0);

        // `before` is read **inside** the critical section, and it names its core. Interrupts are
        // masked in there, so this thread can neither be preempted nor migrated and the measured
        // window is exactly the window under test. Read outside, it straddled a preemption point
        // and compared per-core counters across a possible steal (see `ticks_on`).
        let (cpu, before) = {
            let _guard = M.lock();
            let cpu = crate::cpu::id();
            let before = timer::missed_ticks_on(cpu);
            // Two whole tick periods with interrupts masked. At least one deadline passes while
            // we cannot service it.
            timer::spin_for(timer::interval() * 2 + timer::interval() / 2);
            (cpu, before)
        };

        // Let the pending interrupt land and the miss be counted. Bounded rather than a fixed
        // single period: the claim is that the miss *happens*, and a host that descheduled the
        // emulator only makes the delivery later.
        assert!(
            within_periods(20, || timer::missed_ticks_on(cpu) > before),
            "holding a lock across two tick periods did NOT lose a tick, which means \
             IrqSafeMutex is not masking interrupts and the deadlock is live"
        );
    }

    /// Uptime comes from the *counter*, not the tick count.
    ///
    /// If a tick were ever missed, `ticks * 10ms` would undercount and time would appear to
    /// slow down. The hardware counter cannot lie. This is what `Instant` is made of, and it is
    /// the thing `core` could never give us: nothing in `core` knows what time it is.
    #[test_case]
    fn uptime_advances_monotonically() {
        use crate::arch::timer;

        let a = timer::uptime_ms();
        timer::spin_for(timer::frequency() / 50); // 20 ms
        let b = timer::uptime_ms();

        assert!(b > a, "uptime went backwards or stalled: {a} -> {b}");
        assert!(
            b - a >= 15,
            "uptime advanced {} ms in 20 ms of counter time",
            b - a
        );
    }

    /// **THE TEST.**
    ///
    /// Everything in DECISIONS §9 and notes/locking.md exists to prevent one thing: a timer
    /// interrupt landing inside a critical section, taking the same lock, and spinning forever
    /// waiting for code that cannot run until it returns. On one core. Permanently.
    ///
    /// Until this milestone that was a hypothesis. There were no interrupts. Now there are, and
    /// this is the proof:
    ///
    ///   1. confirm ticks are flowing
    ///   2. take a lock and busy-wait across **three whole tick periods**
    ///   3. assert not one tick landed
    ///   4. release, and watch them resume
    ///
    /// Step 2 works because `spin_for` reads `CNTVCT_EL0`, the hardware counter, which **keeps
    /// counting while interrupts are masked**. A tick-based delay would simply hang here, which
    /// is its own kind of proof.
    #[test_case]
    fn holding_a_lock_masks_the_timer() {
        use crate::arch::{interrupts, timer};
        use crate::sync::{IrqSafeMutex, rank};

        static M: IrqSafeMutex<u32> = IrqSafeMutex::new(rank::PAGE_FRAMES, 0);

        assert!(
            interrupts::is_enabled(),
            "test setup: interrupts should be on"
        );

        // The timer is alive. Core-scoped, for `ticks_on`'s reason.
        let alive_on = crate::cpu::id();
        let t0 = timer::ticks_on(alive_on);
        // Waited for rather than spun for, for `the_timer_is_ticking`'s reason: the emulator can
        // raise a tick tens of milliseconds late, and this assertion (aarch64's copy) went red that
        // way before the suite's userspace half on 2026-08-27. See
        // notes/load-sensitive-assertions.md.
        assert!(
            within_periods(RAISE_BOUND_PERIODS, || timer::ticks_on(alive_on) > t0),
            "the timer is not ticking at all"
        );

        // **Both reads of the tick count happen inside the critical section**, and that is the
        // fix milestone 78 made here. `before` used to be read before `M.lock()`, which makes the
        // measured window wider than the property: a tick landing in the handful of instructions
        // between the read and the mask is charged to the lock. A contended host puts a tick there
        // almost by construction, because a descheduled vCPU resumes with its deadline already
        // past and takes the interrupt at the first instruction it executes. The riscv64 twin
        // failed CI that way on 2026-08-04 ("left: 41, right: 40", one surplus tick, on `rv64`,
        // the control model). The same window also straddled a preemption point, and TICKS is per
        // core, so a steal (§28.3) moving this thread compared two unrelated counters.
        let (cpu, before) = {
            let _guard = M.lock();
            // Interrupts are masked from here, so this core cannot switch threads: `cpu::id()` is
            // fixed for the whole block and both reads below are of one counter.
            let cpu = crate::cpu::id();
            let before = timer::ticks_on(cpu);

            // Thirty milliseconds. Three ticks' worth. Not one of them may land.
            timer::spin_for(timer::interval() * 3);

            assert_eq!(
                timer::ticks_on(cpu),
                before,
                "A TIMER INTERRUPT FIRED WHILE A LOCK WAS HELD. IrqSafeMutex is not masking, \
                 and the deadlock in notes/locking.md is live: a handler that touched this lock \
                 would spin forever waiting for code that cannot run."
            );

            // **And a tick is now raised and waiting**, which is what makes the release below a
            // test of `restore` rather than of the host. The spin above is thirty milliseconds of
            // wall clock, and the emulator raises the timer from its own main loop, as much as 86
            // ms after the deadline (measured 2026-09-24; `is_tick_pending`'s comment). This twin
            // failed CI once exactly that way, on `sifive-u54`, "interrupts did not resume" after
            // twenty periods of waiting for a tick nobody had raised. Still masked, so still this
            // core, and the assertion above is repeated because the wait is part of the window.
            assert!(
                within_raise_bound(timer::is_tick_pending),
                "the timer was never raised in a second with the lock held, so the release below \
                 would test nothing: the timer is not being armed, or the emulator stopped \
                 delivering it"
            );
            assert_eq!(
                timer::ticks_on(cpu),
                before,
                "A TIMER INTERRUPT FIRED WHILE A LOCK WAS HELD. IrqSafeMutex is not masking, \
                 and the deadlock in notes/locking.md is live: a handler that touched this lock \
                 would spin forever waiting for code that cannot run."
            );
            (cpu, before)
        };

        // And the moment we let go, the pending interrupt is delivered. Bounded rather than a
        // fixed two periods, and read by core index because dropping the guard is a preemption
        // point: this thread may be on another core by the next instruction, and the core we left
        // keeps ticking either way.
        assert!(
            within_periods(20, || timer::ticks_on(cpu) > before),
            "interrupts did not resume after the lock was released: `restore` is broken"
        );
    }

    /// Wait for `cond`, bounded in **tick periods of the free-running counter**, checking once a
    /// period.
    ///
    /// Every use is on the "has it happened yet" side of a property (a pending interrupt being
    /// delivered, a miss being counted), which is the direction where a busy host produces a late
    /// pass rather than a wrong answer. The fixed spins these replaced turned a late delivery into
    /// a failure. See notes/load-sensitive-assertions.md.
    /// How long a wait on the timer being raised may take, in tick periods: one second, against a
    /// worst case measured at under nine periods (86 ms, `is_tick_pending`'s comment). **A leak
    /// trap, not a timing claim**: nothing the kernel does is inside it once the deadline has
    /// passed.
    ///
    /// Name: provisional, minted 2026-09-24 (`cda66d656`, waiting for the tick to be raised).
    const RAISE_BOUND_PERIODS: u32 = 100;

    /// Spin until `cond`, bounded by [`RAISE_BOUND_PERIODS`] of the free-running counter, checking
    /// continuously rather than once a period. For waits made **with interrupts masked**, where
    /// nothing can change `cond` except the hardware and a period's granularity would only add
    /// latency.
    ///
    /// Name: provisional, minted 2026-09-24 (`cda66d656`, waiting for the tick to be raised).
    fn within_raise_bound(mut cond: impl FnMut() -> bool) -> bool {
        let bound = u64::from(RAISE_BOUND_PERIODS) * crate::arch::timer::interval();
        let start = crate::arch::timer::now();
        while !cond() {
            if crate::arch::timer::now().wrapping_sub(start) >= bound {
                return false;
            }
            core::hint::spin_loop();
        }
        true
    }

    fn within_periods(periods: u32, mut cond: impl FnMut() -> bool) -> bool {
        for _ in 0..periods {
            if cond() {
                return true;
            }
            crate::arch::timer::spin_for(crate::arch::timer::interval());
        }
        cond()
    }
}
