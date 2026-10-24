//! **Permission to stop this core for good**, and the reasons that may ask for it.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

/// **Permission to stop this core for good**, and the only way to call [`halt`](crate::arch::halt) (milestone 720
/// (provisional)).
///
/// `halt` is `wfi` (or `hlt`) in a loop. A thread that calls it under a running scheduler stays on
/// the run queue and stops its core each time round robin reaches it; a thread whose work is done
/// leaves with [`crate::sched::exit`] instead, and the idle thread takes the core.
///
/// The field is private; the constructors below are the whole list of who may halt (a panic, a
/// test build, a measurement boot, a boot that fails before the scheduler exists).
///
/// Name: ratified 2026-10-03 (calef). Refused `Terminal`, this type's provisional name (milestone
/// 720's lane), because "terminal" already means the display terminal and the tty across about
/// ninety-eight files. The constructors' names (`panicked`, `before_scheduler`, `test_build`,
/// `measurement_boot`) are still provisional. Prior art, from memory and unverified: Zircon's
/// `platform_halt` takes a reason enum; Linux's `kernel_halt` takes nothing.
pub struct HaltReason(());

impl HaltReason {
    /// The panic handler's. Only the panic handler is handed a `PanicInfo`, so nothing else can
    /// make one of these this way. A test image's panic exits through semihosting instead.
    #[cfg_attr(any(test, feature = "system_tests"), allow(dead_code))]
    pub(crate) fn panicked(_info: &core::panic::PanicInfo<'_>) -> Self {
        HaltReason(())
    }

    /// A boot that cannot continue and has no scheduler yet, so there is no run queue for the
    /// calling thread to be on. **Checked, not trusted**: called once the scheduler is up, this
    /// panics, and the panic is what halts.
    #[cfg_attr(not(target_arch = "x86_64"), allow(dead_code))]
    pub(crate) fn before_scheduler() -> Self {
        assert!(
            !crate::sched::is_running(),
            "HaltReason::before_scheduler with the scheduler running: this thread would halt while \
             runnable; leave with sched::exit instead (milestone 720 (provisional))"
        );
        HaltReason(())
    }

    /// A test image, after its suite has asked the host to end the run. Exists only in one. The
    /// shared boot has no halt after its suite (the runner's exit never returns), so only the
    /// `x86_64` boot calls this, hence the allow.
    #[cfg(any(test, feature = "system_tests"))]
    #[cfg_attr(not(target_arch = "x86_64"), allow(dead_code))]
    pub(crate) fn test_build() -> Self {
        HaltReason(())
    }

    /// A measurement boot (`bench`, `icount`, `soak_test`, `job_mix`, `disk_throughput`,
    /// `network_bench`, `storage_bench`, `tsc_probe`): it replaces the hand-over, its last marker is
    /// the result, and the harness tears QEMU down. Exists only in such a build.
    #[cfg(any(
        feature = "bench",
        feature = "icount",
        feature = "soak_test",
        feature = "job_mix",
        feature = "disk_throughput",
        feature = "network_bench",
        feature = "storage_bench",
        feature = "tsc_probe"
    ))]
    pub(crate) fn measurement_boot() -> Self {
        HaltReason(())
    }
}
