use super::*;

/// **A userspace program reads the same counter frequency the kernel did**, on every architecture,
/// through whatever mechanism that architecture has.
///
/// The comparison is against the kernel's own number, which comes from the machine (`CNTFRQ_EL0` on
/// aarch64, `/cpus/timebase-frequency` on riscv64, `CPUID` leaf `0x15` or PIT calibration on
/// `x86_64`), because a test that compares against a constant cannot catch a constant. `coremark`
/// carries it because it reports its own `cntfrq` over IPC and is built by `coremark_builder`
/// through `supervision_protocol::build_child_space`, so the number crosses the userspace ELF loader
/// as well as the kernel's mapping. See notes/pmu.md, "What the userspace counter tests assert".
#[test_case]
fn a_userspace_program_reads_the_frequency_the_kernel_measured() {
    let endpoints = crate::memory_region::create(1).expect("no endpoint region");
    let report = crate::sched::create_rendezvous_from(endpoints).expect("no report rendezvous");
    let mut builder = spawn_elf_child_builder(
        initial_ramdisk().expect("no initial ramdisk"),
        "coremark_builder",
        report,
    );
    builder.add_region(endpoints);

    let [_crc, _ticks, freq, _, _] = crate::sched::ipc_receive(report);
    let kernel = crate::arch::timer::frequency();
    assert_eq!(
        freq, kernel,
        "a userspace program reports {freq} Hz and the kernel measured {kernel} Hz; one of them is \
         timing against a number the machine never stated",
    );
    builder.release_or_fail("the counter-frequency test's building budget");
}

/// **A rate the machine could not have is refused at the point it is learned**, rather than
/// believed and divided by.
///
/// A `> 0` check passes a firmware that reports 1 Hz, or a PIT calibration spoiled by a descheduled
/// vCPU, and the bad rate then scales every number derived from it. The unit tests in
/// `counter_frequency_contract` pin the band against typed values; this pins it against a rate a
/// real or emulated machine reported, so a band narrowed until it rejected a machine we run on fails
/// here rather than in a boot nobody watched.
#[test_case]
fn the_machines_own_rate_is_inside_the_plausibility_band() {
    let hz = crate::arch::timer::frequency();
    assert!(
        counter_frequency_contract::is_plausible(hz),
        "this machine's counter runs at {hz} Hz, which the plausibility band refuses; either the \
         band is wrong or the boot should not have got this far",
    );
}
