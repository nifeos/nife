use super::*;

/// **A userspace program reads the same counter frequency the kernel did**, on every architecture,
/// through whatever mechanism that architecture has.
///
/// This is the test the tree did not have, and its absence is what let `user_mode_runtime::cntfrq`
/// return a hardcoded `10_000_000` on riscv64 for two months. Under QEMU `virt` that constant is
/// the right answer, so every existing assertion (`freq > 0`, `ticks > 0`) passed and would have
/// gone on passing on radon, where the real rate is 4 MHz and every userspace duration was
/// therefore 2.5x too large. **A test that compares against a constant cannot catch a constant.**
/// So this compares against the kernel's own number, which comes from the machine: `CNTFRQ_EL0` on
/// aarch64, `/cpus/timebase-frequency` on riscv64, `CPUID` leaf `0x15` or PIT calibration on
/// `x86_64`. If any of the three ever hands userspace a different number than it uses itself, this
/// fails, and it fails on the emulator rather than on somebody's board.
///
/// **What the chain under test actually is**, which is why this uses `coremark` rather than a
/// smaller fixture. The kernel spawns `coremark_builder` directly (`spawn_elf_child_builder`, which
/// maps the timebase page on the architectures that need one), and `coremark_builder` then builds
/// `coremark` as a child through `supervision_protocol::build_child_space`, the tree's one
/// *userspace* ELF loader. So the number this asserts on travelled kernel to `coremark_builder` to
/// `coremark`: a generation of
/// propagation, not just a mapping. That second hop is the one that used to lose the number
/// entirely (the child got a zeroed placeholder page and `cntfrq` substituted 1 GHz on `x86_64`).
///
/// `coremark` is also the only program in the tree that reports its own `cntfrq` over IPC, which is
/// the other half of why it is the vehicle: nothing had to be added to a fixture to ask this
/// question.
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
/// Every rate check in this tree used to be `> 0`: aarch64 asserted firmware had written
/// `CNTFRQ_EL0` at all, riscv64 asserted the device tree stated something, and `x86_64`'s calibrated
/// value was checked by nothing whatsoever. A firmware that reports 1 Hz, or a PIT calibration that
/// comes back garbage because the host descheduled the vCPU mid-window, passes all three and then
/// scales every number derived from it, silently, which is the exact failure the rest of this file
/// exists to prevent one layer up.
///
/// This asserts the band accepts *this* machine, which is the half a host test cannot do: the unit
/// tests in `counter_frequency_contract` pin the bounds against values somebody typed, and this pins
/// them against a rate a real (or emulated) machine actually reported. A band narrowed until it
/// rejected a machine we run on would fail here rather than in a boot nobody watched.
#[test_case]
fn the_machines_own_rate_is_inside_the_plausibility_band() {
    let hz = crate::arch::timer::frequency();
    assert!(
        counter_frequency_contract::is_plausible(hz),
        "this machine's counter runs at {hz} Hz, which the plausibility band refuses; either the \
         band is wrong or the boot should not have got this far",
    );
}
