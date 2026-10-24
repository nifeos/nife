//! **What CPU is this, and can it run us**, `x86_64`: the `cpuid` instructions, the record and
//! the refusal; the decoding is host-tested in [`machine_discovery::x86_64`]. Three features are
//! gated (milestone 524 (the three `x86_64` boot gates)): NX in `boot.s`, and `syscall` and the
//! invariant TSC in [`init`], where a hypervisor turns the TSC's refusal into a warning. See
//! notes/x86-port/cpu-state.md.
//!
//! # BUGS
//!
//! - The gates are checked on the boot CPU only; secondaries do not re-read `CPUID`.
//! - Under a hypervisor a missing invariant TSC only warns; a hidden hypervisor is refused.

use core::arch::x86_64::CpuidResult;

use machine_discovery::x86_64::{CpuidWords, Gate, HYPERVISOR_CITE, Isa, TABLE};

use crate::sync::{IrqSafeMutex, rank};
use crate::{print, println};

/// **`CPUID`, spelled so that two toolchains a year apart both accept it** (milestone 304
/// (`cargo kani -p kernel` only ever compiled one architecture)).
///
/// `__cpuid` is safe on the pinned toolchain and `unsafe` on the older rustc Kani bundles, so the
/// `unsafe` block satisfies the prover and `allow(unused_unsafe)` satisfies `-D warnings`. The
/// allow is load-bearing for the prover, not a sign the call needs auditing; it goes when Kani's
/// rustc passes 2026-02. See notes/x86-port/cpu-state.md.
///
/// Name: provisional (milestone 304 (`cargo kani -p kernel` only ever compiled one architecture)):
/// `cpuid` is the instruction's own mnemonic, which AGENTS.md's naming section keeps rather than
/// expands.
#[allow(unused_unsafe)]
pub(super) fn cpuid(leaf: u32) -> CpuidResult {
    // SAFETY: `CPUID` is architected on every 64-bit x86 part and has no precondition; a leaf the
    // part does not implement answers with another leaf's data rather than faulting, which is why
    // the decoder is handed the maximum leaf alongside the words. Under the pinned toolchain this
    // block is redundant, which is what the allow above is for.
    unsafe { core::arch::x86_64::__cpuid(leaf) }
}

/// `CPUID` with a subleaf. Same toolchain-skew reasoning as [`cpuid`]; see its comment.
#[allow(unused_unsafe)]
pub(super) fn cpuid_count(leaf: u32, sub_leaf: u32) -> CpuidResult {
    // SAFETY: as [`cpuid`].
    unsafe { core::arch::x86_64::__cpuid_count(leaf, sub_leaf) }
}

/// The record, written once by [`init`] and read by the boot print, the entropy service and the
/// tests.
///
/// `None` until then, so a read before discovery is a panic naming this file rather than a
/// plausible part that reports no features. The same discipline the other two architectures'
/// records keep, and the reason this stopped being a `static mut` with a zeroed initializer: the
/// zeroed value decodes as a machine that would have been refused, which is the worst possible
/// thing for a missed initialization to look like.
static ISA: IrqSafeMutex<Option<Isa>> = IrqSafeMutex::new(rank::ISA, None);

/// One `CpuidResult` as the four words the decoder takes.
fn words(r: CpuidResult) -> [u32; 4] {
    [r.eax, r.ebx, r.ecx, r.edx]
}

/// **Read what this machine is, and refuse to run on one we cannot.** `boot_info_pointer` is
/// ignored; it keeps the arch contract one shape across three architectures. Every leaf is read
/// and [`Isa::decode`](machine_discovery::x86_64::Isa::decode) decides which to believe, since
/// `CPUID` answers an unimplemented leaf with another leaf's bits.
///
/// # Panics
///
/// When the part lacks something the kernel is built on, printing what is missing, what it is
/// used for and where `CPUID` says so.
pub fn init(boot_info_pointer: usize) {
    let _ = boot_info_pointer;

    // Leaf 0 needs no maximum-leaf check first, because leaf 0 is what reports the maximum; the
    // same is true of leaf 0x80000000 in the extended space, which has its own maximum.
    let leaf0 = cpuid(0);
    let extended_max_leaf = cpuid(0x8000_0000).eax;

    let mut brand = [0u32; 12];
    for (i, leaf) in [0x8000_0002u32, 0x8000_0003, 0x8000_0004]
        .iter()
        .enumerate()
    {
        brand[i * 4..i * 4 + 4].copy_from_slice(&words(cpuid(*leaf)));
    }

    let cpu = Isa::decode(&CpuidWords {
        leaf0: words(leaf0),
        leaf1_ecx: cpuid(1).ecx,
        leaf7_0: words(cpuid_count(7, 0)),
        extended_max_leaf,
        extended_leaf1_edx: cpuid(0x8000_0001).edx,
        extended_leaf7_edx: cpuid(0x8000_0007).edx,
        brand,
    });

    let missing = cpu.missing_requirements();
    if missing.any() {
        println!();
        println!("nife cannot run on this machine:");
        for row in &TABLE {
            if missing.features.contains(row.bit) {
                println!("  cpu feature : {} is absent ({})", row.name, row.why);
                println!("                {} reads 0", row.cite);
                if row.gate == Gate::RefuseUnlessHypervisor {
                    println!(
                        "                refused on bare metal: {HYPERVISOR_CITE} reads 0, so the part's promise is all there is"
                    );
                }
            }
        }
        panic!("required CPU feature is absent");
    }

    // Unpromised by this hypervisor: its own line, not a `no-` token in the feature list, because a
    // missing word in a list is not something a reader notices. See `Gate::RefuseUnlessHypervisor`.
    let unpromised = cpu.unpromised();
    for row in &TABLE {
        if unpromised.contains(row.bit) {
            println!(
                "  unpromised  : {} ({} reads 0); {}",
                row.name, row.cite, row.why
            );
            println!(
                "                warned, not refused: {HYPERVISOR_CITE} reads 1; a guest's clock is its hypervisor's"
            );
        }
    }

    *ISA.lock() = Some(cpu);
}

/// What [`init`] found. Panics if read before it.
pub fn get() -> Isa {
    ISA.lock()
        .expect("arch::isa::get() before arch::isa::init()")
}

/// **Draw eight bytes with `RDSEED`, retrying a transient "no data this cycle" result.**
///
/// `None` if the part lacks `RDSEED` (where it is `#UD`, so [`get`] is asked first) or if the
/// source stayed dry on every attempt. The 100 retries with a `pause` between are Intel's guidance
/// for `RDSEED` (DRNG Software Implementation Guide rev. 2.2, section 5.3.1.2), as in
/// `components/src/entropy.rs` on the userspace side.
///
/// Name: ratified 2026-09-24 (calef, #1255 review). Refused `draw_rdseed` (the mnemonic is a
/// decoder, not a word).
#[cfg_attr(not(test), allow(dead_code))] // the record-agrees test below; the entropy service draws in userspace
pub fn draw_random_seed() -> Option<u64> {
    if !get().has_random_seed_instruction() {
        return None;
    }
    const RETRIES: u32 = 100;
    for _ in 0..RETRIES {
        let v: u64;
        let ok: u8;
        // SAFETY: `RDSEED` (Intel's "read random seed" instruction) is unprivileged at any ring and
        // touches no memory; `get().has_random_seed_instruction()` above confirmed CPUID leaf 7 EBX
        // bit 18, so the instruction is not `#UD` here.
        unsafe {
            core::arch::asm!(
                "rdseed {v}",
                "setc {ok}",
                v = out(reg) v,
                ok = out(reg_byte) ok,
                options(nomem, nostack),
            );
        }
        if ok != 0 {
            return Some(v);
        }
        // SAFETY: `pause` touches no memory and has no failure mode.
        unsafe { core::arch::asm!("pause", options(nomem, nostack)) };
    }
    None
}

/// `CPUID` leaf `0x15`'s TSC rate in Hz, on parts that report one directly: the "ask the CPU
/// first" half of `timer::init_frequency`, which otherwise calibrates against the PIT.
///
/// `None` when the part's maximum leaf is below `0x15` (reading it would return a lower leaf's
/// bits), or when `ECX`, the crystal frequency, is zero ("not enumerated"; this port keeps no
/// model-to-crystal table). Under QEMU TCG with `-cpu max` the maximum leaf is `0xd`, so every boot
/// today calibrates. The rate is a constant only because [`init`] refuses bare metal without the
/// invariant TSC. See notes/x86-port/cpu-state.md.
pub(crate) fn tsc_crystal_hz() -> Option<u64> {
    if get().max_leaf < 0x15 {
        return None;
    }
    // Leaf 0x15 carries no precondition beyond the max-leaf check just above.
    let leaf = cpuid(0x15);
    if leaf.eax == 0 || leaf.ebx == 0 || leaf.ecx == 0 {
        return None;
    }
    // SDM Vol. 3B §19.7.3: TSC frequency = ECX (crystal Hz) * EBX (numerator) / EAX (denominator).
    Some((leaf.ecx as u64) * (leaf.ebx as u64) / (leaf.eax as u64))
}

/// Print what the machine is, on every boot, beside the other summaries.
///
/// Two lines, the same split the RISC-V summary uses: what the part is, and what it can do. The
/// feature line prints every row of [`TABLE`], present or absent, because "invariant-tsc" missing
/// from a list of things that are present is not something a reader notices.
pub(crate) fn print_summary() {
    let cpu = get();
    let vendor = core::str::from_utf8(&cpu.vendor).unwrap_or("<not utf-8>");
    // The hypervisor bit decides whether a `Gate::RefuseUnlessHypervisor` row refused or warned,
    // so a reader of a boot log needs it as much as the feature line below.
    let host = if cpu.under_hypervisor {
        "under a hypervisor"
    } else {
        "bare metal"
    };
    println!(
        "  cpu         : x86_64, vendor {vendor}, cpuid leaves 0..{:#x} and {:#x}..{:#x}, {host}",
        cpu.max_leaf, 0x8000_0000u32, cpu.extended_max_leaf
    );
    if let Some(brand) = cpu.brand_str() {
        println!("              : {brand}");
    }
    print!("  features    :");
    for row in &TABLE {
        if cpu.features.contains(row.bit) {
            print!(" {}", row.name);
        } else {
            print!(" no-{}", row.name);
        }
    }
    println!();
}

#[cfg(test)]
mod tests {
    //! What the part said, checked on the part.
    //!
    //! The decoding is proved on the host (`crates/machine_discovery/tests/x86_64_cpu_features.rs`,
    //! which is also where the *refusal* is exercised, since no machine this runs on lacks any of
    //! the three). These are the assertions that need a real boot.

    use super::*;

    /// **Discovery ran, and it found a part that can run us.** The requirement check is what
    /// [`init`] would have panicked on, so reaching this test at all is half the assertion; the
    /// other half is that the record is populated rather than defaulted.
    #[test_case]
    fn the_part_described_itself() {
        let cpu = get();

        assert!(cpu.max_leaf >= 1, "leaf 0 reported no leaves at all");
        assert!(
            cpu.extended_max_leaf >= 0x8000_0007,
            "every gate this kernel checks lives at or below extended leaf 7"
        );
        assert!(!cpu.missing_requirements().any());
        assert!(
            core::str::from_utf8(&cpu.vendor).is_ok(),
            "the vendor string is twelve ASCII characters on every part"
        );
    }

    /// **The two the boot refuses without are present**, named one at a time so a failure says
    /// which. Trivially true on every machine this can run on today, which is the point: the day
    /// it is not, this is the test that says so in the suite rather than in a crash.
    #[test_case]
    fn the_part_has_what_the_kernel_refuses_to_run_without() {
        let f = get().features;

        assert!(
            f.contains(machine_discovery::x86_64::NX),
            "boot.s already refused without it; this is the line that says so"
        );
        assert!(f.contains(machine_discovery::x86_64::SYSCALL));
    }

    /// **The third is the one that does not refuse**, and this test records which way round that
    /// is on the machine the suite is running on rather than asserting either answer.
    ///
    /// It is written to pass both ways on purpose. Under QEMU's TCG the bit reads zero (TCG
    /// declines to advertise it at all), so `unpromised` is exactly `INVARIANT_TSC`; on real
    /// silicon it reads one and `unpromised` is empty. Asserting the TCG answer would make this
    /// test fail on the first good machine, which is the wrong direction for a gate to fail in.
    #[test_case]
    fn the_unpromised_set_is_the_invariant_tsc_or_nothing() {
        let cpu = get();
        let unpromised = cpu.unpromised();

        if cpu
            .features
            .contains(machine_discovery::x86_64::INVARIANT_TSC)
        {
            assert!(unpromised.is_empty());
        } else {
            assert_eq!(unpromised, machine_discovery::x86_64::INVARIANT_TSC);
        }
    }

    /// **The record agrees with the instruction that was actually executed.** `draw_random_seed`
    /// branches on the same bit, so a decode that read the wrong leaf would show up here as a
    /// `None` from a part that has `RDSEED`, or as a `#UD` on one that does not.
    #[test_case]
    fn the_rdseed_bit_agrees_with_rdseed() {
        if get().has_random_seed_instruction() {
            assert!(
                draw_random_seed().is_some(),
                "a part reporting RDSEED gave no seed in 100 tries"
            );
        } else {
            assert!(draw_random_seed().is_none());
        }
    }
}
