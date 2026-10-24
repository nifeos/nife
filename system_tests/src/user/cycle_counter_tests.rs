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
/// The program reads `PMCCNTR_EL0` (aarch64) or the `cycle` CSR (riscv64) at EL0. Granted through
/// the test-only `sched::grant_cycle_counter_to_current` (no syscall sets it yet), it reports;
/// ungranted, the closed default of milestone 228 (the cycle counters are closed by assumption)
/// traps the read and `USER_FAULTS` counts the kill. The reported meaning is checked against
/// `expected_cycle_meaning` (milestone 353 (the aarch64 half of 74), calef's ruling B4 of
/// 2026-10-07 UTC), and the delta of the two reads only where `el0_cycle_counter_is_known_to_run`
/// (milestone 74 (cycle counters)). Under QEMU the delta is emulator time. See notes/pmu.md.
#[test_case]
fn a_granted_thread_reads_the_cycle_counter_and_an_ungranted_one_faults() {
    if !crate::arch::timer::is_cycle_counter_grantable() {
        crate::testing::skip!("this core has no user-readable cycle counter to grant");
    }

    // The negative half does not run on x86_64, where `rdtsc` is ambient by DECISIONS 139 part 3
    // (the stated exception to DECISIONS §19 (architectural parity is a tenet)). Nor on an Apple
    // core, which this kernel meets only under a hypervisor (it does not drive AIC): there an EL0
    // read of `PMCCNTR_EL0` with `PMUSERENR_EL0` at zero was answered rather than refused (measured
    // 2026-09-19, milestone 227's first HVF run), so the outcome is the hypervisor's.
    // `arch::timer::set_cycle_counter_grant`'s BUGS carries the measurement. The positive half
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
/// `user_mode_runtime`, so the test is an independent statement of the kernel's promise and a
/// runtime that labeled the TSC as core cycles fails: aarch64 counts core cycles at EL0 and EL1,
/// riscv64 core cycles in every mode, and `x86_64` constant-rate reference cycles.
///
/// On aarch64 the promise rests on the filter: the meaning is only true while `arch::pmu` writes
/// zero to `PMCCFILTR_EL0` (its own test reads the register back), so that is asserted at compile
/// time here, and a change of policy fails the build until the meaning changes with it.
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
///
/// aarch64: when `arch::pmu` reports `Running` on every online core. The thread may sit on any core,
/// and the `yield` between its reads passes through EL1, which the zero `PMCCFILTR_EL0` counts. A
/// `Stuck` core's stopped counter is the honest answer there, not a failed grant. `x86_64`: always,
/// since the TSC is constant-rate and always running. riscv64: never, because `arch::riscv64::pmu`
/// may hold a different counter (`hpmcounter3` on `rva23s64`) from the `cycle` CSR the program reads.
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
/// comes out right.** `coremark_builder` builds the `"coremark"` binary (a CoreMark-derived run:
/// list sort, matrix multiply, state machine, folded into a CRC), starts it, and the workload SENDs
/// the checksum home. Receiving `coremark::PINNED_CRC_64` (`0x7954`, which the host `coremark` test
/// also pins) proves a real workload ran correctly against the native ABI, with the same answer on
/// the kernel's target as on the host: the property a cross-OS comparison rests on.
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
