//! **Time**: the counter and its frequency on each architecture, the cycle counter and what it
//! counts, the current CPU, and monotonic nanoseconds.
//!
//! Cut out of `lib.rs` unchanged on 2026-10-12 (UTC) by lane maintainer/files-over-1500, for §282
//! (when a file is split, the parent becomes an index); the crate root re-exports every public
//! item, so each keeps its path.
//!
//! Name: provisional, lane maintainer/files-over-1500 (#2017), 2026-10-12 (UTC), for the module and
//! the file. Named for the calls it holds, one module per seam of the old crate root; the pull
//! request records no other name considered.

use abi::cycle_counter::CycleMeaning;

/// The virtual counter, `CNTVCT_EL0`: a monotonic tick count for self-timing. Readable at EL0 only
/// because the kernel opened `CNTKCTL_EL1.EL0VCTEN` (see kernel `timer::init` and notes/abi.md); the
/// read is a plain register move, no syscall. Pair with [`cntfrq`] to turn tick deltas into seconds.
#[cfg(target_arch = "aarch64")]
pub fn now() -> u64 {
    let t: u64;
    // SAFETY: reading a system register the kernel made EL0-readable. No side effects.
    unsafe {
        core::arch::asm!("mrs {}, cntvct_el0", out(reg) t, options(nomem, nostack));
    }
    t
}

/// The monotonic tick count (RISC-V): `rdtime`, which reads the `time` CSR. Readable from U-mode
/// only because the kernel sets `scounteren.TM` in its per-hart timer init, the same shape as
/// aarch64 needing `CNTKCTL_EL1.EL0VCTEN`. That claim was aspirational until 2026-07-30: the bit was
/// never set and this worked only because QEMU's OpenSBI leaves it permitted. Pair with [`cntfrq`]
/// to get seconds.
#[cfg(target_arch = "riscv64")]
pub fn now() -> u64 {
    let t: u64;
    // SAFETY: reading the time CSR the kernel made U-mode-readable. No side effects.
    unsafe {
        core::arch::asm!("rdtime {}", out(reg) t, options(nomem, nostack));
    }
    t
}

/// The counter frequency in Hz, `CNTFRQ_EL0`: how many [`now`] ticks make a second. Constant for the
/// life of the machine (QEMU reports 62.5 MHz under TCG, the host's counter frequency under HVF).
///
/// This is the one architecture where the rate is always knowable, so [`cntfrq_checked`] here is
/// always `Some` and this function cannot fail. The other two read a page the kernel fills, and both
/// can report "unknown"; see [`cntfrq_checked`].
#[cfg(target_arch = "aarch64")]
pub fn cntfrq() -> u64 {
    let f: u64;
    // SAFETY: reading a system register; EL0-readable once EL0VCTEN is set.
    unsafe {
        core::arch::asm!("mrs {}, cntfrq_el0", out(reg) f, options(nomem, nostack));
    }
    f
}

/// The counter frequency in Hz, or `None` if this process cannot know it (aarch64: never, the
/// machine states it in `CNTFRQ_EL0`).
#[cfg(target_arch = "aarch64")]
pub fn cntfrq_checked() -> Option<u64> {
    Some(cntfrq())
}

/// The counter frequency in Hz (RISC-V), read from the **timebase page** the kernel maps read-only
/// into every process at [`counter_frequency_contract::PAGE_VA`].
///
/// RISC-V has **no** register that reports the timebase. The machine states it in the device tree,
/// at `/cpus/timebase-frequency`, and a process holds neither a pointer to the blob nor a capability
/// naming the memory it lives in, so the kernel is the only party that can ever know this number.
/// `kernel::arch::riscv64::timer::init_frequency` reads it on the boot hart and
/// `kernel::user::timebase_page_phys` publishes it; this function is the reader, a plain load, no
/// syscall, the same ambient shape [`now`] already has.
///
/// **It returned a hardcoded `10_000_000` until 2026-09-21**, QEMU `virt`'s rate, with a comment
/// proposing an aux-vector as the eventual fix. radon (the VisionFive 2) runs its `time` CSR at
/// **4 MHz**: every userspace duration measured on that board was 2.5x too large and nothing said
/// so. The aux-vector was never built and is not needed; `x86_64` had already solved the same
/// problem with a page, one directory away, and this now uses it. calef's ruling that forced it:
/// *"We should ensure that the program only returns accurate numbers versus leveraging hard coded
/// ones."*
///
/// # Panics
///
/// If the rate is unknown: see [`cntfrq_checked`], which is this without the panic.
#[cfg(target_arch = "riscv64")]
pub fn cntfrq() -> u64 {
    cntfrq_checked().expect(
        "the counter frequency is unknown: this process's timebase page is zeroed or unrecognized",
    )
}

/// The counter frequency in Hz (RISC-V), or `None` if this process cannot know it. See [`cntfrq`]
/// for where the number comes from and [`cntfrq_checked`]'s `x86_64` twin for why "unknown" is a
/// state worth representing rather than papering over.
#[cfg(target_arch = "riscv64")]
pub fn cntfrq_checked() -> Option<u64> {
    // SAFETY: every path that builds a riscv64 process maps a page (real, or zeroed when the rate
    // was never learned) read-only at `counter_frequency_contract::PAGE_VA` before it runs:
    // `kernel::user::load`, `kernel::user::map_timebase_page`, and
    // `supervision_protocol::build_child_space`. See that crate's `BUGS` section for the one shape
    // that does not, which faults here on purpose.
    let page = unsafe {
        counter_frequency_contract::TimebasePage::new(counter_frequency_contract::PAGE_VA)
    };
    page.hz()
}

/// The monotonic tick count (`x86_64`, milestone 161): `rdtsc`, which reads the time-stamp counter.
/// Readable from ring 3 because `CR4.TSD` is clear, which is the reset state and which this kernel
/// does not change; that is the same shape as aarch64 needing `CNTKCTL_EL1.EL0VCTEN` and RISC-V
/// needing `scounteren.TM`, with the difference that here the permissive state is the default and
/// the kernel would have to act to *close* it.
///
/// **`rdtsc` returns the count split across `edx:eax`**, so this is a shift and an or. A single
/// `out(reg)` compiles and reads only the low half, which wraps every four seconds at 1 GHz.
///
/// **No serialization, deliberately.** `rdtsc` is not ordered against surrounding instructions; the
/// reordering window is tens of cycles, the trade the kernel's own calibration accepts.
///
/// # BUGS
///
/// **On `x86_64` the cycle counter is ambient, and nobody chose that.** aarch64 and riscv64 give
/// userspace a *coarse* counter and keep the fine one shut (milestone 228 (the cycle counters are
/// closed by assumption, and on two architectures the assumption is a comment)). Here the TSC is
/// both, `CR4.TSD` is clear at reset and never written, so every ring-3 program holds a
/// sub-nanosecond timing instrument it was never granted. (`CR4.PCE`, the `rdpmc` door, is closed
/// per core in `arch::init`.) Setting `CR4.TSD` would take away `Instant`, `thread::sleep`, the
/// random seed and smoltcp's timestamps at once; it needs a coarse monotonic page first,
/// DECISIONS §43 (reading the clock is a page)'s move one axis over. Until then `x86_64` answers
/// milestone 75 (who may read the cycle counter, and by what authority) with "everyone, always"
/// (§19 (architectural parity is a tenet)).
#[cfg(target_arch = "x86_64")]
pub fn now() -> u64 {
    let (lo, hi): (u32, u32);
    // SAFETY: reading a counter ring 3 is permitted to read. No side effects, no memory touched.
    unsafe {
        core::arch::asm!("rdtsc", out("eax") lo, out("edx") hi, options(nomem, nostack));
    }
    ((hi as u64) << 32) | (lo as u64)
}

/// The counter frequency in Hz (`x86_64`), read from the **timebase page** the kernel maps
/// read-only into every process at [`counter_frequency_contract::PAGE_VA`] (milestone 161 (the x86_64 kernel port: bring up the HAL's third architecture)'s `cntfrq`
/// follow-up).
///
/// x86 has **no architected rate a ring-3 program can ask for**: `CPUID` leaf `0x15` is absent on
/// many parts, and calibrating against the 8254 PIT needs port I/O, which DECISIONS §121 (what a
/// device capability is when the device has no page: x86 port I/O) closed to userspace
/// permanently. So the kernel publishes
/// the rate: `arch::x86_64::timer::init_frequency` reads the leaf or calibrates, and every kernel
/// path that builds a top-level address space maps the page
/// (`kernel::user::map_x86_timebase_page`). `supervision_protocol::build_child_space` fills a
/// child's page from its own rate, so a subtree inherits the measured number. This is a plain load,
/// no syscall, the same "ambient, no capability" shape [`now`] has.
///
/// # Panics
///
/// If the rate is unknown (a zeroed or unrecognized page): calef, 2026-09-21, *"We should ensure
/// that the program only returns accurate numbers versus leveraging hard coded ones."* A plausible
/// wrong rate produces a benchmark figure somebody quotes. [`cntfrq_checked`] is this without the
/// panic, the same pair as the kernel's `arch::timer::frequency`/`frequency_checked`, so
/// [`monotonic_nanos`] keeps its `u64`. The panic traps (see [`trap`]) and the kernel kills the
/// process with no message, the floor every userspace `expect` has. A parent that does not know
/// the rate writes a zeroed page, and its whole subtree refuses rather than guessing.
#[cfg(target_arch = "x86_64")]
pub fn cntfrq() -> u64 {
    cntfrq_checked().expect(
        "the counter frequency is unknown: this process's timebase page is zeroed or unrecognized",
    )
}

/// The counter frequency in Hz (`x86_64`), or `None` if this process cannot know it: a zeroed page
/// (the rate was never measured, or the parent that built this process did not know it) or one
/// whose magic does not match. See [`cntfrq`] for where the number comes from and why the
/// unchecked twin refuses rather than substituting a constant.
#[cfg(target_arch = "x86_64")]
pub fn cntfrq_checked() -> Option<u64> {
    // SAFETY: every kernel-side space-building function this crate's own docs list maps a page
    // (real, or zeroed when the rate was never learned) read-only at
    // `counter_frequency_contract::PAGE_VA` into every x86_64 process before it ever runs.
    let page = unsafe {
        counter_frequency_contract::TimebasePage::new(counter_frequency_contract::PAGE_VA)
    };
    page.hz()
}

/// **A cycle-counter read and what it counts**, returned together by [`cycle_reading`].
///
/// The count alone is not a quantity a reader can compare across machines: aarch64 and riscv64
/// count core cycles (over different privilege levels), and `x86_64` counts reference cycles at a
/// constant rate. So the count never travels without its [`CycleMeaning`].
///
/// Name: provisional, minted by the lane for milestone 353 (the aarch64 half of 74) on 2026-10-07
/// (UTC), the noun for what
/// [`cycle_reading`] returns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CycleReading {
    /// The raw counter value. **Only a difference of two reads means anything**, and only when
    /// both were taken on the same core (aarch64 and riscv64 counters are per core and start at
    /// different times).
    pub count: u64,
    /// What [`Self::count`] counts on this architecture.
    pub meaning: CycleMeaning,
}

/// **Read the CPU's cycle counter from user mode, paired with what it counts** (milestone 353,
/// calef's ruling B4 step 1, 2026-10-07 UTC). One instruction per architecture, no syscall, the
/// property DECISIONS §139 (who may read the cycle counter, and by what authority) option 4 chose
/// the grant to keep:
///
/// | architecture | instruction | [`CycleMeaning`] |
/// |---|---|---|
/// | aarch64 | `mrs PMCCNTR_EL0` | [`CycleMeaning::CoreCyclesUserAndKernel`]: EL0 and EL1, not EL2 |
/// | riscv64 | `csrr cycle` | [`CycleMeaning::CoreCyclesEveryMode`]: every privilege mode |
/// | `x86_64` | `rdtsc` | [`CycleMeaning::ConstantRateReferenceCycles`]: not core cycles |
///
/// **Only a program granted the cycle counter may call it on aarch64 and riscv64**, where the
/// kernel opens `PMUSERENR_EL0` or `scounteren.CY` per thread (milestone 229 (the cycle-counter
/// grant)); an ungranted call traps and the thread is killed. On `x86_64` the TSC is ambient.
///
/// # Examples
///
/// ```no_run
/// let before = user_mode_runtime::cycle_reading();
/// // ... the work being measured ...
/// let after = user_mode_runtime::cycle_reading();
/// let spent = after.count.wrapping_sub(before.count);
/// if !after.meaning.is_core_cycles() {
///     // x86_64: `spent` is reference cycles at the TSC's constant rate, not core cycles.
/// }
/// # let _ = spent;
/// ```
///
/// # BUGS
///
/// - **No manifest field grants the cycle counter yet**, so outside the kernel's own test no
///   aarch64 or riscv64 program can call this without being killed (milestone 75's work).
/// - **riscv64 cannot say whether its counter is the kernel's**: `cycle` is `mcycle`, and the
///   kernel's probe may hold `hpmcounter3` (milestone 856 (a program asks whether it may read the
///   cycle counter)).
/// - **Under QEMU every count is emulator time**, never a measurement of a core.
///
/// Name: provisional, minted by milestone 353's lane on 2026-10-07 (UTC), Rust's getter shape.
/// Refused `cycles` (the kernel's `arch::pmu::cycles` means core cycles, and the TSC is not).
/// Refused `read_cycle_counter` (a verb, and the fixture's private name).
pub fn cycle_reading() -> CycleReading {
    CycleReading {
        count: read_cycle_counter(),
        meaning: CYCLE_MEANING,
    }
}

/// What [`cycle_reading`] counts here: core cycles at EL0 and EL1, under the `PMCCFILTR_EL0` of
/// zero the kernel writes on every core.
#[cfg(target_arch = "aarch64")]
const CYCLE_MEANING: CycleMeaning = CycleMeaning::CoreCyclesUserAndKernel;
/// What [`cycle_reading`] counts here: `mcycle`, every privilege mode.
#[cfg(target_arch = "riscv64")]
const CYCLE_MEANING: CycleMeaning = CycleMeaning::CoreCyclesEveryMode;
/// What [`cycle_reading`] counts here: the TSC, constant-rate reference cycles.
#[cfg(target_arch = "x86_64")]
const CYCLE_MEANING: CycleMeaning = CycleMeaning::ConstantRateReferenceCycles;

#[cfg(target_arch = "aarch64")]
fn read_cycle_counter() -> u64 {
    let value: u64;
    // SAFETY: `mrs` from `PMCCNTR_EL0` reads a counter and touches no memory. It is UNDEFINED at
    // EL0 unless `PMUSERENR_EL0` permits it, which is the grant `cycle_reading`'s caller must hold;
    // an ungranted read traps and the kernel kills the thread, which is not memory unsafety.
    unsafe {
        core::arch::asm!("mrs {}, pmccntr_el0", out(reg) value, options(nomem, nostack, preserves_flags));
    }
    value
}

#[cfg(target_arch = "riscv64")]
fn read_cycle_counter() -> u64 {
    let value: u64;
    // SAFETY: `csrr` from the `cycle` CSR reads a counter and touches no memory. It is an illegal
    // instruction in U-mode unless `scounteren.CY` permits it; see the aarch64 twin above.
    unsafe {
        core::arch::asm!("csrr {}, cycle", out(reg) value, options(nomem, nostack, preserves_flags));
    }
    value
}

/// The TSC. The same instruction [`now`] executes on this architecture, kept separate because
/// the two answer different questions: `now` is the clock, this is the counter with its meaning.
#[cfg(target_arch = "x86_64")]
fn read_cycle_counter() -> u64 {
    now()
}

/// **Which CPU this thread is running on**, or `None` if this process has no page to read it from
/// or has somehow not been switched in yet.
///
/// A plain load, no syscall, and that is the point rather than an optimization. calef ruled on
/// 2026-09-21 that a thread observing *itself* gets a page and a thread observing *another* gets a
/// selector, because the consumer here is a memory allocator keeping a per-CPU cache: it asks once
/// per allocation, and a crossing at that rate is not a cost you can amortize away. (The ruling's
/// `design/decisions/` section is on another branch and not on `main` yet, so it is named here
/// rather than cited.) `crates/current_cpu_contract` carries the layout, the ordering argument and
/// the reason this is not a register on any of the three targets.
///
/// **The answer can be stale before you use it**, because the kernel may migrate this thread the
/// instruction after the load, and nothing in this function or that crate prevents it. A per-CPU
/// cache built on this must be *correct* when the answer turns out to be the previous core's and
/// merely faster when it is not. Linux solves the same problem with `rseq`'s restartable
/// sequences; this tree does not have those and is not pretending to.
///
/// **The value is a CPU id, not an index into anything you have counted.** Size a per-CPU array by
/// [`current_cpu_contract::CPU_ID_BOUND`] and nothing else: the VisionFive 2's online set is
/// `{1, 2, 3}`, so counting cores and indexing by the count reaches a core that is not there and
/// misses one that is.
///
/// # EXAMPLES
///
/// ```no_run
/// # use user_mode_runtime::current_cpu;
/// let mut per_cpu = [0u64; current_cpu_contract::CPU_ID_BOUND];
/// if let Some(cpu) = current_cpu() {
///     per_cpu[cpu] += 1;
/// }
/// ```
///
/// # BUGS
///
/// - **A process whose address space was never bound to a TCB has no page here**, and this faults
///   on an unmapped read rather than answering `None`. Every space that runs a thread gets one
///   (`kernel::user::AddressSpace::new` for the kernel-built ones,
///   `kernel::sched::configure_thread_control_block` for the userspace-built ones), so the shape
///   that faults is a space with no thread in it, which has nothing to ask. Stated because the
///   failure is a fault rather than a value, which is the one thing a caller cannot handle.
pub fn current_cpu() -> Option<usize> {
    // SAFETY: the kernel maps a page it owns, read-only, at `current_cpu_contract::PAGE_VA` into
    // every address space that a TCB binds, before that thread's first instruction runs. See this
    // function's own BUGS section for the one shape that has no page.
    let page = unsafe { current_cpu_contract::CurrentCpuPage::new(current_cpu_contract::PAGE_VA) };
    page.cpu()
}

/// **The scheduler tick's rate in Hz**, the resolution of every timed wait, from this thread's
/// current-CPU page, or `None` if the page carries none. A [`timer_arm`](crate::timer_arm)
/// deadline fires at the first tick at or after it, so a program that must not lose a tick to a
/// sleep reads this rather than assuming the kernel's rate
/// (`current_cpu_contract::CurrentCpuPage::tick_hz`).
///
/// Name: provisional, minted 2026-10-21 (UTC) by milestone 53 (the board's own peripherals)'s
/// receive-rate lane.
pub fn tick_hz() -> Option<core::num::NonZeroU64> {
    // SAFETY: as `current_cpu`: the kernel maps the page read-only at `PAGE_VA` into every address
    // space a TCB binds, before that thread's first instruction runs.
    let page = unsafe { current_cpu_contract::CurrentCpuPage::new(current_cpu_contract::PAGE_VA) };
    page.tick_hz()
}

/// **Monotonic nanoseconds since boot**, from [`now`] and [`cntfrq`].
///
/// Here rather than in each program because two of them need it and the naive form is wrong:
/// `ticks * 1_000_000_000` overflows a `u64` about five minutes into a boot at 62.5 MHz, so the
/// conversion splits into whole seconds and a remainder. `components/src/clock.rs` found that the hard
/// way; `date` and the shell's `time` then wanted the same five lines, which is CLAUDE.md rule 7 at
/// the smallest size it comes in.
///
/// **This needs no capability**, and that is worth knowing when reading anything that calls it: the
/// counter is ambient (the kernel opened it to EL0), so a *duration* is measurable by any process.
/// What needs a capability is the **wall clock**, which is this plus an offset only the clock page
/// carries. See notes/clock.md.
pub fn monotonic_nanos() -> u64 {
    /// Nanoseconds in a second. Spelled here rather than taken from `clock_contract`, because this
    /// crate is the syscall runtime and depends on `abi` alone; a runtime that pulled in a wire
    /// contract to name a unit would be the wrong direction for the dependency.
    const NANOS_PER_SEC: u64 = 1_000_000_000;
    let freq = cntfrq();
    let ticks = now();
    let secs = ticks / freq;
    let rem = ticks % freq;
    secs * NANOS_PER_SEC + rem * NANOS_PER_SEC / freq
}
