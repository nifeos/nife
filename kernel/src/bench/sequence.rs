//! **The bench boot**: every benchmark, in the fixed order a baseline is read in, then a halt.
//!
//! Name: provisional (lane `maintainer/last-three-over-2000`, 2026-10-12 UTC), for the module and
//! the file.

use super::*;

/// Run every benchmark and halt. Never returns, never semihosts (see the module doc).
///
/// `ipc_stack_depth` runs first, so nothing earlier in the suite is on a stack it reads; it is off
/// in every timing build, and the rows after it are not results with it on.
pub fn run() -> ! {
    println!();
    println!("bench: cntfrq {}", crate::arch::timer::frequency());
    // Which E3 image this is; nothing else on a card says. A probe, never a row.
    #[cfg(feature = "fastpath_pad")]
    {
        let [units, shift] = *core::hint::black_box(&crate::fastpath_pad::BUILD);
        println!("bench-probe: fastpath_pad units {units} shift {shift}");
    }

    #[cfg(feature = "ipc_stack_depth")]
    {
        let bad =
            crate::ipc_stack_depth::kernel_thread_shapes() + crate::ipc_stack_depth::el0_shapes();
        println!("ipc-stack-depth: done ({bad} series were not measurements)");
    }

    yield_switch();
    #[cfg(target_arch = "x86_64")]
    tss_iomap_switch();
    #[cfg(target_arch = "x86_64")]
    tss_iomap_lazy_switch();
    #[cfg(target_arch = "x86_64")]
    tss_iomap_lazy_nop();
    ipc_rtt();
    relay_rtt();
    call_reply();
    broker_rtt();
    spawn_reap();
    map_new();
    #[cfg(target_arch = "riscv64")]
    rfence_self();
    cycles_per_tick();
    coremark_compute();
    null_syscall_el0();
    ctx_switch_el0();
    ipc_rtt_el0();
    #[cfg(any(target_arch = "aarch64", target_arch = "riscv64"))]
    ipc_thread_scaling();
    #[cfg(any(target_arch = "aarch64", target_arch = "riscv64"))]
    app_displacement();
    sink_throughput();
    map_el0();
    spawn_el0();
    smp_throughput();
    fs_read();
    fs_throughput();
    fs_walk();
    fs_walk_bound();
    rg_search();

    println!("bench: done");
    crate::arch::halt(crate::arch::HaltReason::measurement_boot());
}
