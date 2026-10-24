//! The floating-point and vector registers belong to one thread at a time.
//!
//! The register file holds the running thread's data or [`FpState::INITIAL`], never a thread's
//! data while another runs. [`hand_over`] keeps that true at each context switch, eagerly: the
//! first-use trap answers only whether a thread wants FP, never whose data is in the registers,
//! because a trap is not a boundary (`LazyFP`, CVE-2018-3665). The policy half of the
//! `arch::fp` modules. See design/roadmap/0447-vector-registers-are-a-thread-s-own.md.

//! # BUGS
//!
//! - `live` never clears, so a thread that used FP once moves its file on every later switch.
//! - A migrating thread carries its register file through memory. Nothing measures it.

use core::sync::atomic::{AtomicUsize, Ordering};

use crate::arch::fp::FpState;

/// How many threads have ever asked for the floating-point unit. Zero on every shipping boot, which
/// is the evidence the soft-float targets are weighed against (milestone 447 (a thread's vector
/// registers are its own)).
pub(crate) static ENABLES: AtomicUsize = AtomicUsize::new(0);

/// Move the register file between threads; live to non-live scrubs to [`FpState::INITIAL`].
///
/// Context: just before `arch::switch_to`; interrupts masked, `IPC_TABLES` released.
///
/// # Safety
/// `prev` and `next` must name live, distinct `FpState`s of the outgoing and incoming threads,
/// pinned as `sched::schedule` pins its contexts (running, and `Running` with `on_cpu` set),
/// interrupts are masked, and nothing can reap either.
#[inline(always)]
pub(crate) unsafe fn hand_over(prev: *mut FpState, next: *const FpState) {
    // SAFETY: the caller's, forwarded. Both reads are of one `u64` in a pinned allocation.
    let prev_live = unsafe { (*prev).is_live() };
    // SAFETY: the caller's, forwarded; one `u64` read in a pinned allocation, as above.
    let next_live = unsafe { (*next).is_live() };

    if !prev_live && !next_live {
        // Every switch on a machine where nothing uses floating point.
        return;
    }

    // SAFETY: the caller's, and at least one of the two flags is set.
    unsafe { move_the_register_file(prev, next, prev_live, next_live) }
}

/// The half of [`hand_over`] that touches registers, out of line and `#[cold]`.
///
/// Cold because only this module's tests reach it ([`ENABLES`] counts them), and
/// `script/fastpath-footprint` measures the non-cold closure of the IPC roots: inlined into
/// `schedule`, it put riscv64's `ipc_send_receive` 7.5% over that gate's 5% bound.
///
/// # Safety
/// [`hand_over`]'s, plus: at least one of `prev_live` and `next_live` is true, and each describes
/// the `FpState` it is named for.
#[cold]
#[inline(never)]
unsafe fn move_the_register_file(
    prev: *mut FpState,
    next: *const FpState,
    prev_live: bool,
    next_live: bool,
) {
    if prev_live {
        // SAFETY: the caller's. FP is enabled on this core, because this core is running `prev` and
        // `prev.live` is exactly the condition under which the enable was installed.
        unsafe { crate::arch::fp::save(prev) };
    }

    if next_live {
        if !prev_live {
            crate::arch::fp::enable();
        }
        // SAFETY: the caller's, and FP is enabled either way by the line above or by `prev`.
        unsafe { crate::arch::fp::restore(next) };
    } else {
        // `prev_live` here. Scrub before disabling: the trap is not a wall.
        //
        // SAFETY: `INITIAL` is a `'static` const, and FP is enabled (we just saved through it).
        unsafe { crate::arch::fp::restore(&FpState::INITIAL) };
        crate::arch::fp::disable();
    }
}

/// Open the FP unit for the running thread on its first-use trap, and mark it live.
///
/// The body of every architecture's first-use trap. Installs [`FpState::INITIAL`], because before
/// any live thread has run on this core the registers hold whatever reset left. Returns false when
/// the unit cannot open or there is no current thread; the caller must then fault, or the same
/// instruction traps forever. Cold, because RISC-V's trap decoder, which calls it, is measured
/// flat by `script/fastpath-footprint`.
///
/// Context: a synchronous trap from a running thread; takes `IPC_TABLES` through
/// `sched::mark_current_fp_live`, which this core cannot already hold.
#[cold]
#[inline(never)]
pub(crate) fn enable_for_current() -> bool {
    crate::arch::fp::enable();
    if !crate::arch::fp::is_enabled() {
        // No unit: RISC-V hardwires `sstatus.FS` to zero without F/D, so the enable was a no-op.
        // Only reading back after the write answers that.
        return false;
    }
    if !crate::sched::mark_current_fp_live() {
        crate::arch::fp::disable();
        return false;
    }
    // SAFETY: `INITIAL` is a `'static` const and FP is enabled by the lines above.
    unsafe { crate::arch::fp::restore(&FpState::INITIAL) };
    ENABLES.fetch_add(1, Ordering::Relaxed);
    true
}

/// A distinct 64-bit value for register `index` under `seed`, so a test can tell one thread's
/// file from another's and from a scrubbed one. Neighboring seeds and indices land far apart, so
/// a save to the wrong offset fails.
#[cfg(any(test, feature = "system_tests"))]
pub fn register_pattern(seed: u64, index: usize) -> u64 {
    seed.wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(index as u64 + 1)
}

/// Take the first-use trap, then fill every vector register with `seed`'s pattern.
///
/// `touch` provokes the trap; `restore` may not, since its contract says the unit is open.
#[cfg(test)]
pub(crate) fn load_pattern(seed: u64) {
    crate::arch::fp::touch();
    let mut state = FpState::INITIAL;
    state.set_pattern(seed);
    // SAFETY: `state` is a local, aligned by its own `repr`, and `touch` above left the unit open.
    unsafe { crate::arch::fp::restore(&state) };
}

/// Read the live register file and leave the unit as found.
///
/// Not [`enable_for_current`], which would install `INITIAL` over what a test wants to see and
/// mark the thread live. Interrupts are masked throughout so no switch finds the core mid-borrow.
#[cfg(test)]
pub(crate) fn peek_registers() -> FpState {
    let was_enabled_interrupts = crate::arch::interrupts::disable();
    let was_enabled_fp = crate::arch::fp::is_enabled();
    if !was_enabled_fp {
        crate::arch::fp::enable();
    }
    let mut state = FpState::INITIAL;
    // SAFETY: `state` is a local and the unit is open either way by the line above.
    unsafe { crate::arch::fp::save(&mut state) };
    if !was_enabled_fp {
        crate::arch::fp::disable();
    }
    crate::arch::interrupts::restore(was_enabled_interrupts);
    state
}

#[cfg(test)]
mod tests {
    use core::sync::atomic::AtomicUsize;

    use super::*;

    /// Proof that interrupts are masked and the vector registers are borrowed. Only
    /// [`with_borrowed_registers`] builds one, which is what lets [`Borrowed::hand_over`] be safe.
    struct Borrowed(());

    impl Borrowed {
        /// [`hand_over`] between two of a test's own states, and the one place these tests argue
        /// its contract.
        fn hand_over(&self, prev: &mut FpState, next: &FpState) {
            // SAFETY: `&mut` and `&` make the two states live and distinct for the call. A
            // `Borrowed` exists only inside `with_borrowed_registers`' body, where interrupts are
            // masked, and neither state is a thread's, so nothing can switch or reap either.
            unsafe { hand_over(prev, next) }
        }
    }

    /// Run `body` with the vector registers borrowed and handed back exactly as found.
    ///
    /// The calling thread may already be `live`, and leaving the unit shut under it would make the
    /// next switch `save` into a trapping unit. Interrupts stay masked for the whole body.
    fn with_borrowed_registers(body: impl FnOnce(&Borrowed)) {
        let was_enabled_interrupts = crate::arch::interrupts::disable();
        let was_enabled_fp = crate::arch::fp::is_enabled();
        let saved = if was_enabled_fp {
            let mut state = FpState::INITIAL;
            // SAFETY: a local, and the unit is open.
            unsafe { crate::arch::fp::save(&mut state) };
            Some(state)
        } else {
            None
        };

        body(&Borrowed(()));

        match saved {
            Some(state) => {
                crate::arch::fp::enable();
                // SAFETY: a local, and the unit is open by the line above.
                unsafe { crate::arch::fp::restore(&state) };
            }
            None => crate::arch::fp::disable(),
        }
        crate::arch::interrupts::restore(was_enabled_interrupts);
    }

    /// Two threads doing vector work on the same core do not see each other's registers.
    ///
    /// Both on this core with `spawn_on`, because on two cores they would have two register files
    /// and pass with no kernel help. This thread sits between them, so the scrub arm runs too.
    #[test_case]
    fn two_threads_doing_vector_work_do_not_see_each_others_registers() {
        /// Threads that finished their turns without finding a foreign value.
        static INTACT: AtomicUsize = AtomicUsize::new(0);
        /// Threads that found one, counted because an assertion there would panic unwatched.
        static CLOBBERED: AtomicUsize = AtomicUsize::new(0);
        /// Turns per thread: enough for many preemptions, few enough for milliseconds.
        const TURNS: usize = 200;

        INTACT.store(0, Ordering::Relaxed);
        CLOBBERED.store(0, Ordering::Relaxed);

        let here = crate::cpu::id();
        for seed in [0x1234_5678_9abc_def0_u64, 0x0fed_cba9_8765_4321_u64] {
            crate::sched::spawn_on(here, move || {
                load_pattern(seed);
                for _ in 0..TURNS {
                    crate::sched::yield_now();
                    if !peek_registers().has_pattern(seed) {
                        CLOBBERED.fetch_add(1, Ordering::Release);
                        return;
                    }
                }
                INTACT.fetch_add(1, Ordering::Release);
            })
            .expect("could not spawn a vector-arithmetic thread");
        }

        let finished = || INTACT.load(Ordering::Acquire) + CLOBBERED.load(Ordering::Acquire) == 2;
        let deadline = crate::arch::timer::now() + 2 * crate::arch::timer::frequency();
        while crate::arch::timer::now() < deadline && !finished() {
            crate::sched::yield_now();
        }

        assert!(finished(), "the vector-arithmetic threads never finished");
        assert_eq!(
            CLOBBERED.load(Ordering::Acquire),
            0,
            "a thread found another thread's values in its own vector registers",
        );
    }

    /// The first floating-point instruction a fresh thread executes takes a trap that opens the
    /// unit.
    ///
    /// A machine that did not trap would leave every thread un-`live` and share one register file
    /// silently. A fresh thread, because the harness's own may already be live.
    #[test_case]
    fn the_first_floating_point_instruction_takes_a_trap() {
        /// 0 = unfinished, 1 = the trap opened the unit, 2 = the unit was open before the
        /// instruction ran, 3 = the instruction ran and the unit is still shut.
        static VERDICT: AtomicUsize = AtomicUsize::new(0);
        VERDICT.store(0, Ordering::Relaxed);

        crate::sched::spawn_on(crate::cpu::id(), || {
            if crate::arch::fp::is_enabled() {
                VERDICT.store(2, Ordering::Release);
                return;
            }
            crate::arch::fp::touch();
            VERDICT.store(
                if crate::arch::fp::is_enabled() { 1 } else { 3 },
                Ordering::Release,
            );
        })
        .expect("could not spawn a thread to take the first-use trap");

        let deadline = crate::arch::timer::now() + 2 * crate::arch::timer::frequency();
        while crate::arch::timer::now() < deadline && VERDICT.load(Ordering::Acquire) == 0 {
            crate::sched::yield_now();
        }

        match VERDICT.load(Ordering::Acquire) {
            1 => {}
            0 => panic!("the thread taking the first-use trap never finished"),
            2 => panic!(
                "a brand-new thread started with the floating-point unit already open, so the \
                 first-use trap can never fire and no thread will ever be marked live",
            ),
            _ => panic!(
                "a floating-point instruction executed without opening the unit: this machine \
                 does not trap on the control register this kernel relies on",
            ),
        }
    }

    /// Handing the registers to a thread that has none leaves nothing of the previous thread's.
    ///
    /// This arm fails silently, as CVE-2018-3665 did, so it is driven directly on synthetic states
    /// and read back with [`peek_registers`], which skips the enable trap.
    #[test_case]
    fn a_thread_with_no_vector_state_finds_the_registers_scrubbed() {
        with_borrowed_registers(|registers| {
            let mut departing = FpState::INITIAL;
            departing.set_live();
            departing.set_pattern(0xdead_beef_cafe_f00d);
            let arriving = FpState::INITIAL;
            let mut idle = FpState::INITIAL;

            // Give the registers something worth leaking.
            registers.hand_over(&mut idle, &departing);
            assert!(
                peek_registers().has_pattern(0xdead_beef_cafe_f00d),
                "hand_over did not install the incoming thread's registers",
            );

            // And take them away again, to a thread that has never used the unit.
            registers.hand_over(&mut departing, &arriving);
            assert!(
                peek_registers().is_scrubbed(),
                "a thread's vector registers survived into a thread that has none",
            );
            assert!(
                !crate::arch::fp::is_enabled(),
                "the unit was left open for a thread that has never asked for it",
            );
        });
    }

    /// What a thread had in its registers comes back out into its [`FpState`], lane for lane.
    ///
    /// A save to the wrong lane can pass the concurrency test; [`register_pattern`] makes it fail
    /// here.
    #[test_case]
    fn the_whole_register_file_survives_a_save_and_a_restore() {
        const SEED: u64 = 0x5555_aaaa_5555_aaaa;
        with_borrowed_registers(|registers| {
            let mut written = FpState::INITIAL;
            written.set_live();
            written.set_pattern(SEED);
            let mut idle = FpState::INITIAL;
            registers.hand_over(&mut idle, &written);

            let read_back = peek_registers();
            for index in 0..crate::arch::fp::REGISTERS {
                assert_eq!(
                    read_back.lane_low(index),
                    register_pattern(SEED, index),
                    "vector register {index} did not survive the round trip",
                );
            }

            // Put the unit back shut, so `with_borrowed_registers` restores from a known state.
            let arriving = FpState::INITIAL;
            registers.hand_over(&mut written, &arriving);
        });
    }

    /// [`hand_over`] between two threads that never used FP leaves the unit shut.
    ///
    /// Every switch in this tree takes this case, and a regression would show only as a slower
    /// benchmark.
    #[test_case]
    fn two_threads_that_never_used_the_unit_leave_it_shut() {
        with_borrowed_registers(|registers| {
            crate::arch::fp::disable();
            let mut one = FpState::INITIAL;
            let two = FpState::INITIAL;
            assert!(!one.is_live() && !two.is_live());
            registers.hand_over(&mut one, &two);
            assert!(
                !crate::arch::fp::is_enabled(),
                "a switch between two soft-float threads opened the floating-point unit",
            );
        });
    }
}
