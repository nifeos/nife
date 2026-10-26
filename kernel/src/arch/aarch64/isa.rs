//! **Ask the aarch64 machine what it is, once, at boot.**
//!
//! `MIDR_EL1` names the part and the `ID_AA64*` registers say what it can do, so discovery reads
//! registers and parses no firmware. The decoding is in [`machine_discovery::aarch64`],
//! host-tested; what only the machine can do is here: the reads, the record, and the refusal. See
//! notes/aarch64-port.md, "Asking the part what it is".

use core::sync::atomic::{AtomicU8, AtomicU32, Ordering};

use machine_discovery::aarch64::{Conduit, Isa, Psci};

use super::instructions;

use crate::sync::{IrqSafeMutex, rank};
use crate::{print, println};

/// The record, written once by [`init`] and read by the boot print, `mmu::init` and the tests.
///
/// `None` until then, so a read before discovery panics naming this file rather than reporting a
/// plausible all-zero machine.
static ISA: IrqSafeMutex<Option<Isa>> = IrqSafeMutex::new(rank::ISA, None);

/// The `/psci` record, in plain atomics rather than behind the mutex above: `cpu_start` reads it
/// on the bring-up path with interrupts enabled, and it is written once before any secondary exists.
const PSCI_ABSENT: u8 = 0;
const PSCI_NO_METHOD: u8 = 1;
const PSCI_HVC: u8 = 2;
const PSCI_SMC: u8 = 3;
static PSCI_CONDUIT: AtomicU8 = AtomicU8::new(PSCI_ABSENT);
/// The `CPU_ON` function id, or 0 for "the node did not publish one". No PSCI implementation
/// publishes zero (SMCCC reserves the low identifiers and every published `CPU_ON` sets the
/// fast-call bit), so the sentinel cannot collide with a real id.
static PSCI_CPU_ON: AtomicU32 = AtomicU32::new(0);
/// Did the id come from the node's own `cpu_on` property, rather than from the 0.2 standard? Boot
/// line only; see [`psci_record`].
static PSCI_ID_FROM_PROPERTY: AtomicU8 = AtomicU8::new(0);

/// **Discover the machine, and refuse to run on one we cannot.**
///
/// The ISA comes from ID registers. `dtb_ptr` is read for `/psci` only: the conduit depends on what
/// runs above the kernel, not on the part.
///
/// # Panics
///
/// With no 4 KiB granule, or a reserved `ASIDBits` encoding. A machine with no usable `/psci`
/// does not stop the boot: it runs on one core, and bring-up says so.
pub fn init(dtb_ptr: usize) {
    // SAFETY: the pointer firmware handed us, whose magic `memory::init` has already checked (it
    // runs first and panics otherwise), named through the boot map's direct region.
    let dt = unsafe {
        device_tree_blob::DeviceTreeBlob::from_ptr(
            super::mmu::phys_to_virt(dtb_ptr as u64) as *const u8
        )
    }
    .expect("device tree is unreadable");
    match Psci::from_device_tree(&dt).expect("cannot read /psci") {
        None => PSCI_CONDUIT.store(PSCI_ABSENT, Ordering::Relaxed),
        Some(psci) => {
            PSCI_CONDUIT.store(
                match psci.conduit {
                    None => PSCI_NO_METHOD,
                    Some(Conduit::Hvc) => PSCI_HVC,
                    Some(Conduit::Smc) => PSCI_SMC,
                },
                Ordering::Relaxed,
            );
            PSCI_CPU_ON.store(psci.cpu_on.unwrap_or(0), Ordering::Relaxed);
            PSCI_ID_FROM_PROPERTY.store(u8::from(psci.cpu_on_from_property), Ordering::Relaxed);
        }
    }

    let cpu = Isa::decode(
        instructions::read_midr(),
        instructions::read_id_aa64mmfr0(),
        instructions::read_id_aa64mmfr2(),
        instructions::read_id_aa64isar0(),
    );

    let missing = cpu.missing_requirements();
    if missing.any() {
        println!();
        println!("nife cannot run on this machine:");
        if missing.granule_4k {
            println!(
                "  granule     : no 4 KiB stage-1 granule (ID_AA64MMFR0_EL1.TGran4), and every \
                 page table here is 4 KiB"
            );
        }
        if missing.asid_bits {
            println!(
                "  asid        : ID_AA64MMFR0_EL1.ASIDBits is a reserved encoding, so the ASID \
                 width is unknown and crates/address_space_identifier needs at least 8"
            );
        }
        panic!("required hardware facility is absent");
    }

    *ISA.lock() = Some(cpu);
}

/// The record. Panics if read before [`init`].
pub fn get() -> Isa {
    ISA.lock()
        .expect("arch::isa::get() before arch::isa::init()")
}

/// **What a `CPU_ON` call needs**: the conduit to make it on and the function id to make it with,
/// or `None` when this machine stated neither well enough to try. Read by
/// [`super::cpu_start`] on every core it starts.
pub fn psci() -> Option<(Conduit, u32)> {
    let conduit = match PSCI_CONDUIT.load(Ordering::Relaxed) {
        PSCI_HVC => Conduit::Hvc,
        PSCI_SMC => Conduit::Smc,
        _ => return None,
    };
    match PSCI_CPU_ON.load(Ordering::Relaxed) {
        0 => None,
        id => Some((conduit, id)),
    }
}

/// The whole `/psci` answer, for the boot line and the tests: `None` when the tree had no such node,
/// otherwise the record with whatever it did and did not state. [`psci`] is the version the bring-up
/// path wants, which is the two facts or nothing.
pub(crate) fn psci_record() -> Option<Psci> {
    if PSCI_CONDUIT.load(Ordering::Relaxed) == PSCI_ABSENT {
        return None;
    }
    let cpu_on = match PSCI_CPU_ON.load(Ordering::Relaxed) {
        0 => None,
        id => Some(id),
    };
    Some(Psci {
        conduit: match PSCI_CONDUIT.load(Ordering::Relaxed) {
            PSCI_HVC => Some(Conduit::Hvc),
            PSCI_SMC => Some(Conduit::Smc),
            _ => None,
        },
        cpu_on,
        cpu_on_from_property: PSCI_ID_FROM_PROPERTY.load(Ordering::Relaxed) != 0,
        // Derived, not stored: `cpu_on_from_property` already says which source the id came from.
        standard: PSCI_ID_FROM_PROPERTY.load(Ordering::Relaxed) == 0 && cpu_on.is_some(),
    })
}

/// The boot line: who made this part, and the three numbers the kernel's own configuration rests on.
///
/// Its only caller is the banner in `main.rs`, which the test build and the `bench` boot mode both
/// compile out, so it has no caller in exactly those two configurations. The same shape as
/// `mmu::print_summary` beside it. The RISC-V twin has no such exemption, because that boot prints
/// its ISA line before the branch into the test, shell or bench paths.
#[cfg_attr(
    any(test, feature = "system_tests", feature = "bench"),
    allow(dead_code)
)]
pub(crate) fn print_summary() {
    let cpu = get();

    match cpu.implementer_name() {
        Some(name) => print!("  cpu             : {name}"),
        None => print!("  cpu             : implementer {:#04x}", cpu.implementer),
    }
    println!(" part {:#05x} r{}p{}", cpu.part, cpu.variant, cpu.revision);
    println!(
        "                  : {}-bit PA, {}-bit VA available, {}-bit ASIDs, granules {}{}{}",
        cpu.pa_bits(),
        cpu.va_bits,
        cpu.asid_bits,
        if cpu.granules.k4 { "4K " } else { "" },
        if cpu.granules.k16 { "16K " } else { "" },
        if cpu.granules.k64 { "64K" } else { "" },
    );
}

#[cfg(test)]
mod tests {
    //! What the part said, checked on the part.
    //!
    //! The decoding is proved on the host (`crates/machine_discovery/tests`). These are the assertions that need
    //! a real boot: that the record was populated, and that what the CPU says about itself agrees
    //! with what the kernel independently configured on the strength of it.

    use super::*;

    /// **Discovery ran, and the part identified itself.** Reaching this test means `init` did not
    /// refuse; this is the other half, that the record holds a machine rather than a default.
    #[test_case]
    fn the_part_identified_itself() {
        let cpu = get();

        assert!(cpu.granules.k4, "we are running on 4 KiB pages");
        assert!(
            cpu.pa_bits() > 0,
            "a reserved PARange would be reported as zero"
        );
        assert!(!cpu.missing_requirements().any());
    }

    /// **The PSCI conduit and function id came from `/psci`, and match QEMU `virt`'s.**
    ///
    /// A skipped parse leaves `psci()` at `None`. The conduit is keyed on the entry exception
    /// level: entered at EL2, an `hvc` from EL1 would land in that EL2, so the machine states `smc`.
    /// The id is held against the constant QEMU `virt` publishes, so a change says which moved.
    #[test_case]
    fn the_psci_call_was_read_from_the_machine() {
        let (conduit, cpu_on) = super::psci().expect("QEMU virt has a usable /psci node");

        let expected = if super::super::entry_el() == 2 {
            Conduit::Smc
        } else {
            Conduit::Hvc
        };
        assert_eq!(
            conduit,
            expected,
            "the method /psci states, given entry at EL{}",
            super::super::entry_el(),
        );
        assert_eq!(cpu_on, 0xC400_0003, "the id smp.rs used to hold as a const");
        assert_eq!(cpu_on, machine_discovery::aarch64::PSCI_CPU_ON_64);
    }

    /// **`boot.s` recorded the exception level firmware entered at, and it is one this kernel
    /// knows how to start from** (milestone 127, the seL4 machine).
    ///
    /// `running_at_el1` proves where the kernel is; this proves the entry was recorded. A zero
    /// means `.bss` was zeroed after the store, or the store was dropped, and the banner would
    /// claim an entry at EL0.
    #[test_case]
    fn the_entry_exception_level_was_recorded() {
        let entered = super::super::entry_el();
        assert!(
            entered == 1 || entered == 2,
            "boot.s recorded entry at EL{entered}, which no firmware can deliver"
        );
    }

    /// **The ASID width the part reports is enough for the allocator built on it.**
    ///
    /// The RISC-V twin of this has to *measure* the number, because RISC-V permits any width
    /// including zero and publishes nothing. ARM mandates 8 or 16, so here it is a read. Same
    /// assertion either way, and the same consequence if it failed: `crates/address_space_identifier` hands out 255
    /// numbers on the stated assumption that hardware can tell them apart.
    #[test_case]
    fn the_asid_width_supports_the_allocator() {
        assert!(
            get().asid_bits >= 8,
            "crates/address_space_identifier assumes at least 8"
        );
    }

    /// **`TCR_EL1.IPS` holds what the part reported, and nothing else.**
    ///
    /// A change that made `mmu::init` write a constant, or read another field, would leave the live
    /// register disagreeing with the boot line that describes it.
    #[test_case]
    fn the_configured_physical_address_size_is_the_one_the_part_reported() {
        use super::super::{instructions, mmu};

        assert_eq!(
            ((instructions::read_tcr() >> mmu::TCR_IPS_SHIFT) & mmu::TCR_IPS_MASK) as u8,
            get().pa_range,
            "TCR_EL1.IPS is ID_AA64MMFR0_EL1.PARange, straight through"
        );
    }
}
