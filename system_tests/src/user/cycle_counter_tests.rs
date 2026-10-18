//! `cycle_counter_tests`: the cycle-counter grant, and the CoreMark workload that init runs.
//!
//! Name: provisional, minted 2026-10-12 (UTC) by the lane for milestone 873 (the catch-all
//! userspace test file is split by topic).

use core::sync::atomic::Ordering;

#[cfg(not(target_arch = "x86_64"))]
use super::test_programs::spawn_bare;
use super::*;
use crate::arch::exceptions::USER_FAULTS;
use crate::sched;

/// **The `cycle_counter_reader` program's ELF bytes** (milestone 229): reads the CPU's cycle
/// counter twice across a yield and reports. It was role 42 of the `hello` multiplexer until
/// milestone 291.
fn cycle_counter_image() -> &'static [u8] {
    program("cycle_counter_reader").expect("no cycle_counter_reader program in the initial ramdisk")
}

/// **Milestone 229: a granted thread reads the cycle counter, and an ungranted one is killed for
/// trying.** Both halves, in one test, because each is the other's control.
///
/// A thread runs `hello`'s cycle-counter role at EL0 and reads `PMCCNTR_EL0` (aarch64) or the
/// `cycle` CSR (riscv64). Granted, it reports. Ungranted, the read is an access milestone 228's
/// closed default does not permit, so it traps, the kernel kills the thread, and `USER_FAULTS`
/// counts it.
///
/// **The grant is applied through a `#[cfg(test)]` back door**,
/// `sched::grant_cycle_counter_to_current`: there is no syscall method a loader could set it with
/// yet (a method number is irreversible, and milestone 147 (a profiler that holds exactly the
/// counters it was granted) may subsume it). `sched`'s
/// `a_running_thread_cannot_be_granted_the_cycle_counter` covers the embryo-only rule.
///
/// **What the reads count is checked on every architecture** (milestone 353 (the aarch64 half of 74), calef's ruling B4 of
/// 2026-10-07 UTC). The program reads through `user_mode_runtime::cycle_reading`, which pairs each
/// count with an `abi::cycle_counter::CycleMeaning`, and reports the meaning it was handed. This test
/// states the expected meaning per architecture itself rather than asking the runtime, so a runtime
/// that labeled the TSC as core cycles, or labeled aarch64's count without the kernel's filter
/// policy behind it, fails here: aarch64 must say core cycles at EL0 and EL1 (the `PMCCFILTR_EL0` of
/// zero that `arch::pmu` writes and its own test reads back from the register), riscv64 core cycles
/// in every mode, and `x86_64` constant-rate reference cycles.
///
/// **The difference of two reads is checked where the kernel has said the counter runs, and only
/// there** (milestone 74 (cycle counters)):
///
/// - **aarch64**: when `arch::pmu` reports `Running` on every online core, the second read must be
///   past the first. The thread may be placed on any core and the `yield` between the reads is a
///   trip through EL1, so the claim holds under the `PMCCFILTR_EL0` of zero (EL0 and EL1 counted),
///   and would under any filter that counted either. Where a core refused its counter (`Stuck`) nothing is asserted,
///   because a stopped counter is the honest answer there and not a failure of the grant.
/// - **`x86_64`**: the TSC is always running and constant-rate, so the second read must be past the
///   first, always.
/// - **riscv64**: not checked. The program reads the `cycle` CSR, and `arch::riscv64::pmu` may have
///   been handed a different counter (`hpmcounter3` on `rva23s64`), so the kernel has no statement
///   about the CSR the program reads that a check could lean on.
///
/// None of this is a measurement: under QEMU the delta is emulator time.
#[test_case]
fn a_granted_thread_reads_the_cycle_counter_and_an_ungranted_one_faults() {
    if !crate::arch::timer::is_cycle_counter_grantable() {
        crate::testing::skip!("this core has no user-readable cycle counter to grant");
    }

    // **The negative half, and it does not run on x86_64**, where `rdtsc` is ambient by DECISIONS
    // 139 part 3 and an ungranted read is not an error. Skipping it there is the stated exception
    // to DECISIONS §19 showing up in a test rather than a gap in one.
    //
    // **And it does not run on an Apple core**, which this kernel can only ever meet under a
    // hypervisor (an Apple core's own interrupt controller is AIC, which it does not drive). There
    // the negative half was measured failing on 2026-09-19 (milestone 227's first HVF run): with
    // `PMUSERENR_EL0` at zero an EL0 read of `PMCCNTR_EL0` was answered rather than refused, so the
    // hypervisor, not this kernel, decides the outcome and the assertion would be about QEMU's
    // HVF. `arch::timer::set_cycle_counter_grant`'s BUGS carries the measurement. The positive half
    // below still runs.
    #[cfg(target_arch = "aarch64")]
    let under_apple_hypervisor = crate::arch::isa::get().implementer == 0x61;
    #[cfg(target_arch = "riscv64")]
    let under_apple_hypervisor = false;
    #[cfg(not(target_arch = "x86_64"))]
    if !under_apple_hypervisor {
        let before = USER_FAULTS.load(Ordering::Relaxed);
        spawn_bare(cycle_counter_image(), 0, 0).expect("spawn failed");
        assert!(
            wait_for(|| USER_FAULTS.load(Ordering::Relaxed) > before),
            "an ungranted thread read the cycle counter and was NOT stopped",
        );
    }

    // The positive half. A report arriving at all is most of the assertion: on the two
    // architectures above, the same program without the grant is the fault just counted.
    let endpoints = crate::memory_region::create(1).expect("no endpoint region");
    let result = sched::create_rendezvous_from(endpoints).expect("no result rendezvous");
    let faults = USER_FAULTS.load(Ordering::Relaxed);
    sched::spawn(move || {
        sched::grant_cycle_counter_to_current();
        run(
            cycle_counter_image(),
            Spawn {
                arg0: 0,
                arg1: 0,
                arg2: 0,
                grants: &[crate::cap::rendezvous_cap(
                    result,
                    crate::cap::Rights::WRITE,
                )],
                maps: &[],
            },
        )
    })
    .expect("spawn failed");

    let message = sched::ipc_receive(result);
    assert_eq!(
        message[0],
        capability_witness_contract::CYCLE_COUNTER_WORD,
        "the granted thread did not report: it was killed reading a counter it was granted",
    );
    assert_eq!(
        USER_FAULTS.load(Ordering::Relaxed),
        faults,
        "the granted thread faulted instead of reading cleanly",
    );
    sched::reclaim_region(endpoints).expect("the endpoint region did not come back");

    let (moved, meaning) = (message[1], message[2]);
    assert_eq!(
        abi::cycle_counter::CycleMeaning::from_word(meaning),
        Some(expected_cycle_meaning()),
        "the granted thread's reading said it counts {meaning:#x}, which is not what this \
         architecture's counter counts",
    );
    if el0_cycle_counter_is_known_to_run() {
        assert!(
            moved > 0 && moved < u64::MAX / 2,
            "the granted thread's two cycle-counter reads differ by {moved}: the kernel says the \
             counter runs on every core, so the second should have been past the first",
        );
    }
}

/// **What a granted read must say it counts on this architecture**, stated here and not taken from
/// `user_mode_runtime`, so the test is an independent statement of the kernel's promise.
///
/// On aarch64 the promise rests on the filter: the meaning is only true while `arch::pmu` writes
/// zero, so that is asserted at compile time here, and a change of policy fails the build until the
/// meaning changes with it.
fn expected_cycle_meaning() -> abi::cycle_counter::CycleMeaning {
    use abi::cycle_counter::CycleMeaning;
    #[cfg(target_arch = "aarch64")]
    {
        const _: () = assert!(
            crate::arch::pmu::PMCCFILTR_COUNT_EL0_AND_EL1 == 0,
            "the kernel no longer counts EL0 and EL1 alone, so CoreCyclesUserAndKernel is no \
             longer what a granted aarch64 read counts",
        );
        CycleMeaning::CoreCyclesUserAndKernel
    }
    #[cfg(target_arch = "riscv64")]
    {
        CycleMeaning::CoreCyclesEveryMode
    }
    #[cfg(target_arch = "x86_64")]
    {
        CycleMeaning::ConstantRateReferenceCycles
    }
}

/// **Does the kernel have a statement that the counter `cycle_counter_reader` reads is running?**
/// See the test above for why each architecture answers as it does.
fn el0_cycle_counter_is_known_to_run() -> bool {
    #[cfg(target_arch = "aarch64")]
    {
        crate::smp::online_cpus()
            .all(|cpu| crate::arch::pmu::outcome_on(cpu) == crate::arch::pmu::CycleCounter::Running)
    }
    #[cfg(target_arch = "x86_64")]
    {
        true
    }
    #[cfg(target_arch = "riscv64")]
    {
        false
    }
}

/// **Milestone 19e (run a real workload): a userspace builder runs a real compute workload and it
/// comes out right.** The `least_authority_demo`'s `n*n` proved the mechanism; this proves a
/// *substantial* program. `coremark_builder` builds the `"coremark"` binary (a CoreMark-derived
/// run: list sort, matrix multiply, state machine, folded into a CRC), starts it, and the workload
/// SENDs the run's checksum home. Receiving `coremark::PINNED_CRC_64` (`0x7954`, the value the host
/// `coremark` test also pins) proves a real workload ran correctly against the native ABI, and that
/// the same computation gives the same answer on the kernel's target as on the host, which is the
/// property a cross-OS comparison will rest on.
///
/// Name: provisional (milestone 405 (the nine `INIT` roles)), named for the builder it drives.
///
/// Falsification: unfalsified. Renamed by milestone 405 (the nine `INIT` roles), which ran it green
/// on all three architectures but did not break the code it guards to watch it go red.
#[test_case]
fn the_coremark_builder_runs_the_workload_and_it_checks_out() {
    let report_region =
        crate::memory_region::create(1).expect("no region for the builder's report");
    let report =
        crate::sched::create_rendezvous_from(report_region).expect("no rendezvous from region");
    let builder = spawn_elf_child_builder(
        initial_ramdisk().expect("no initial ramdisk"),
        "coremark_builder",
        report,
    );

    let [crc, ticks, freq, _, _] = crate::sched::ipc_receive(report);
    assert_eq!(
        crc,
        coremark::PINNED_CRC_64 as u64,
        "the CoreMark workload computed the wrong checksum",
    );
    // The workload self-timed via CNTVCT_EL0. Nonzero ticks and a real frequency prove EL0 can
    // read the virtual counter (CNTKCTL_EL1.EL0VCTEN), the foundation the primitive suite needs.
    // (Under TCG the magnitude is icount fiction, but it still advances, so the read works.)
    assert!(
        ticks > 0,
        "the workload's self-timing read a frozen counter"
    );
    assert!(freq > 0, "CNTFRQ_EL0 read as zero at EL0");
    builder.release_or_fail("a builder test's building budget");
    crate::sched::reclaim_region(report_region)
        .expect("the builder's report region would not reclaim");
}
