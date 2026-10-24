//! **Starting the secondary CPUs**: INIT-SIPI-SIPI, the busy-wait it needs, and the line every
//! boot prints about it.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

/// Start a secondary CPU via INIT-SIPI-SIPI. `target_cpu` is a local APIC id (the roster's
/// hardware id), `entry` is the trampoline's physical page and must equal
/// `ap_boot::trampoline_phys()` (debug-asserted), and `context` is the stack top the trampoline
/// hands to `secondary_main`. Returns 0 once the core is online, or -1 if the local APIC is not up
/// or the core did not check in within ten seconds.
///
/// Blocks until then, unlike aarch64 and RISC-V: every `STARTUP` IPI shares one trampoline page,
/// and waiting is what makes the next call's `ap_boot::prepare` safe to overwrite it. See
/// notes/x86-port/smp-bring-up.md.
pub fn cpu_start(target_cpu: u64, entry: u64, context: u64) -> i64 {
    if !irq::is_local_apic_ready() {
        return -1;
    }
    debug_assert_eq!(
        entry,
        ap_boot::trampoline_phys(),
        "cpu_start's entry is not the AP trampoline's own page"
    );
    // SAFETY: this function does not return until the core it is about to start has either come
    // fully online or been given up on (see the wait loop below), which is what keeps two calls
    // from ever overwriting the shared trampoline page while an earlier one is still in use.
    unsafe { ap_boot::prepare(context) };

    let dest = target_cpu as u8;
    // Read once, before the INIT, and compared against by every wait below: a fast core can check
    // in during the settle delays, and a later re-read would miss its increment.
    let before = crate::smp::online_count();

    // The universal INIT-SIPI-SIPI startup algorithm (Intel MP spec, appendix B.4): INIT, a settle
    // delay, then a STARTUP IPI.
    irq::send_init(dest);
    busy_wait_us(10_000);
    let vector = (entry >> 12) as u8;
    irq::send_startup(dest, vector);
    busy_wait_us(200);

    // A second STARTUP only if the first has not visibly worked: a core that has left
    // wait-for-SIPI ignores another (QEMU's `apic_sipi` too), so this is as correct as two.
    if crate::smp::online_count() == before {
        irq::send_startup(dest, vector);
        busy_wait_us(200);
    }

    // Bounded, so a core the firmware declines to start cannot hang bring-up. Ten seconds, because
    // under TCG a loaded host may not schedule the target's vCPU thread for that long. `hlt`
    // between checks (this core's `TICK_HZ` timer wakes it) rather than a spin, which would starve
    // the very vCPU thread this waits on. See notes/x86-port/smp-bring-up.md.
    let budget = 10 * crate::arch::timer::frequency();
    let start = crate::arch::timer::now();
    while crate::smp::online_count() == before {
        if crate::arch::timer::now().wrapping_sub(start) >= budget {
            return -1;
        }
        wait_for_interrupt();
    }
    0
}

/// Busy-wait roughly `us` microseconds, against the calibrated TSC. No `wfi`/`hlt` equivalent
/// here: INIT-SIPI-SIPI's delays are a handful of instructions on real hardware, and parking this
/// core for them would need an interrupt to wake it that nothing is going to send.
pub(super) fn busy_wait_us(us: u64) {
    let ticks = crate::arch::timer::frequency() / 1_000_000 * us;
    let start = crate::arch::timer::now();
    while crate::arch::timer::now().wrapping_sub(start) < ticks {
        core::hint::spin_loop();
    }
}

/// Can this machine start a secondary CPU at all? Yes, once the local APIC is up: INIT-SIPI-SIPI is
/// sent *through* it, unlike PSCI or SBI, which need no device at all before the first call.
pub fn can_start_secondaries() -> bool {
    irq::is_local_apic_ready()
}

/// Print how this machine starts a CPU. One line, on every boot, beside the SMP count.
pub fn print_bring_up_mechanism() {
    if irq::is_local_apic_ready() {
        crate::println!(
            "  smp: init-sipi-sipi via the local apic, trampoline at {:#x}",
            ap_boot::trampoline_phys()
        );
    } else {
        crate::println!(
            "  smp: the local apic is not up, so no core can be started here (init-sipi-sipi needs it)"
        );
    }
}

/// **The physical address `smp::bring_up_secondaries` hands `cpu_start` as `entry`**: the
/// argument itself. On aarch64 and RISC-V `virt_to_phys` converts a high-linked label. Here
/// `secondary_boot` is linked at its fixed low address (`AP_TRAMPOLINE_PHYS`, link-x86_64.ld),
/// because a `STARTUP` IPI can only name a page below 1 MiB, so its address already is that page
/// and converting it again would underflow.
pub fn secondary_boot_entry(secondary_boot_addr: u64) -> u64 {
    secondary_boot_addr
}
