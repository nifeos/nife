//! **PSCI**: the firmware call that starts a secondary core and the one that resets the machine,
//! and the line every boot prints about how cores start.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

use super::*;

/// One PSCI call, on the conduit the machine stated. The instruction has to be a literal in `asm!`,
/// and the two forms differ in nothing else, so the body is written once here rather than twice.
///
/// Per SMCCC, x0-x3 are results and x4-x17 are scratch, so all are marked clobbered; x18-x30 are
/// preserved by the callee (the firmware).
macro_rules! psci_call {
    ($conduit:literal, $func:expr, $a1:expr, $a2:expr, $a3:expr) => {{
        let ret: i64;
        // SAFETY: a defined firmware call on the conduit `/psci` named. It starts the target core
        // and returns a status in x0; it does not touch our memory.
        unsafe {
            core::arch::asm!(
                $conduit,
                inout("x0") $func => ret,
                inout("x1") $a1 => _,
                inout("x2") $a2 => _,
                inout("x3") $a3 => _,
                lateout("x4") _, lateout("x5") _, lateout("x6") _, lateout("x7") _,
                lateout("x8") _, lateout("x9") _, lateout("x10") _, lateout("x11") _,
                lateout("x12") _, lateout("x13") _, lateout("x14") _, lateout("x15") _,
                lateout("x16") _, lateout("x17") _,
                options(nostack),
            );
        }
        ret
    }};
}

/// What [`cpu_start`] returns when the machine never told us how to make the call. Not a PSCI
/// error code: PSCI's own space runs from -1 to -9, and inventing a tenth would be a lie about who
/// answered. Nothing reaches this in the normal path, because `smp::bring_up_secondaries` asks
/// [`can_start_secondaries`] first.
pub(crate) const PSCI_NOT_DISCOVERED: i64 = i64::MIN;

/// PSCI `CPU_ON`: start a secondary core. Returns 0 on success, a negative error otherwise.
///
/// `target_mpidr` is the core's `MPIDR_EL1[39:0]`, `entry` the physical address it starts at with
/// the MMU off, and `context` arrives in its x0. The conduit and the function id are the ones
/// `isa::init` read from `/psci`. See notes/aarch64-port.md, "PSCI and the entry exception level".
///
/// # BUGS
///
/// - **The `smc` path has never executed.** `crates/machine_discovery`'s host tests decode a real
///   QEMU dump that states `smc` (`virt,virtualization=on`); the `hvc` path runs on every test run.
pub(crate) fn cpu_start(target_mpidr: u64, entry: u64, context: u64) -> i64 {
    let Some((conduit, func)) = isa::psci() else {
        return PSCI_NOT_DISCOVERED;
    };
    let func = func as u64;
    match conduit {
        ::machine_discovery::aarch64::Conduit::Hvc => {
            psci_call!("hvc #0", func, target_mpidr, entry, context)
        }
        ::machine_discovery::aarch64::Conduit::Smc => {
            psci_call!("smc #0", func, target_mpidr, entry, context)
        }
    }
}

/// PSCI `PSCI_VERSION`, the SMC32 id every PSCI 0.2 and later implementation answers. Asked before
/// the reset so the console says which PSCI refused, if one does.
pub(super) const PSCI_VERSION: u64 = 0x8400_0000;

/// PSCI `SYSTEM_RESET` (PSCI 0.2, section 5.11 of ARM DEN 0022): a **cold** reset of the whole
/// system, which "does not return" on success. SMC32, so the same id on every implementation; PSCI
/// 0.1 had no system functions at all, so a 0.1 firmware answers `NOT_SUPPORTED` (-1).
pub(super) const PSCI_SYSTEM_RESET: u64 = 0x8400_0009;

/// **Ask the firmware for a cold reboot through PSCI `SYSTEM_RESET`**, returning only on refusal.
///
/// The arch contract `soak::draw_again` and `kernel::reboot::restart` call: one line per attempt,
/// prefixed with `marker`, drained before the call. One route here, needing only `/psci`'s conduit.
///
/// # BUGS
///
/// - **Proven under QEMU `virt` only, over `hvc`.** argon's TF-A, over `smc`, is milestone 249
///   (the boot lottery is sampled by a person walking to the board)'s.
/// - **The ACPI half (`machine_discovery::acpi::parse_arm_boot`) is not wired**: device tree only.
///
/// Name: provisional (milestone 249): calef names public items.
pub fn reboot(marker: &str) -> abi::Error {
    use ::machine_discovery::aarch64::Conduit;

    let Some(conduit) = isa::psci_record().and_then(|p| p.conduit) else {
        println!(
            "{marker} PSCI SYSTEM_RESET not attempted: the device tree has no /psci node with a \
             usable method, so hvc-versus-smc cannot be chosen and a guess is an undefined \
             instruction"
        );
        return abi::Error::NoResetMechanism;
    };
    let call = |func: u64| -> i64 {
        match conduit {
            Conduit::Hvc => psci_call!("hvc #0", func, 0u64, 0u64, 0u64),
            Conduit::Smc => psci_call!("smc #0", func, 0u64, 0u64, 0u64),
        }
    };
    let version = call(PSCI_VERSION);
    println!(
        "{marker} attempt 1 of 1: PSCI SYSTEM_RESET over {} (PSCI_VERSION answered {}.{}). The next \
         thing this console should show is the firmware's banner.",
        conduit.name(),
        (version >> 16) & 0xffff,
        version & 0xffff,
    );
    let error = {
        // Drain before the call, or the marker line is still in the FIFO when firmware resets.
        crate::console::drain();
        call(PSCI_SYSTEM_RESET)
    };
    println!(
        "{marker} PSCI SYSTEM_RESET refused: returned {error} (-1 is NOT_SUPPORTED, a PSCI 0.1 \
         firmware or one that does not offer system reset)"
    );
    abi::reboot::from_psci(error)
}

/// Can this machine start a secondary core at all? Asked once by `smp::bring_up_secondaries`.
///
/// False on a machine whose device tree has no `/psci` node, or one whose node did not say enough
/// to make the call. Both are real: a uniprocessor board has no reason to carry the node, and a
/// board whose cores are released from a spin-table carries a different mechanism instead.
pub(crate) fn can_start_secondaries() -> bool {
    isa::psci().is_some()
}

/// Print how this machine starts a core, or why it cannot. One line, on every boot, beside the SMP
/// count; the same discipline milestone 60's ISA summary set.
pub(crate) fn print_bring_up_mechanism() {
    let Some(psci) = isa::psci_record() else {
        println!("  smp: the device tree has no /psci node, so no core can be started here");
        return;
    };
    match (psci.conduit, psci.cpu_on) {
        (Some(conduit), Some(cpu_on)) => println!(
            "  smp: psci over {}, CPU_ON {cpu_on:#010x} ({}, from the device tree)",
            conduit.name(),
            if psci.cpu_on_from_property {
                "the node's own id"
            } else {
                "the standard id"
            },
        ),
        (None, _) => {
            println!(
                "  smp: /psci states no usable `method`, and hvc-versus-smc cannot be guessed"
            );
        }
        (_, None) => {
            println!("  smp: /psci is 0.1 and published no CPU_ON id, so there is no call to make");
        }
    }
}
