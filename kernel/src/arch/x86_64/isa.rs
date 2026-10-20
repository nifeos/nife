//! **What CPU is this, and can it run us**, `x86_64`. The third implementation of the arch contract
//! aarch64 answers from `MIDR_EL1` and RISC-V from the device tree plus `mvendorid`.
//!
//! x86 answers with `CPUID`, which returns structured data about the part, including the vendor's
//! 48-character brand string. The decoding is in [`machine_discovery::x86_64`], host-tested; what
//! can only happen on the machine is here: the `cpuid` instructions, the record, and the refusal.
//!
//! # Where each gate sits in the boot
//!
//! Three features this kernel is built on (milestone 524 (the three `x86_64` boot gates)). **A
//! check belongs at the last moment before the thing it protects**, unless that moment is before
//! there is any way to say what went wrong.
//!
//! | Feature | Checked | Why there |
//! |---|---|---|
//! | NX | `boot.s`, before `EFER.NXE` is written | that write is itself the hazard: setting a reserved `EFER` bit on a part without NX is `#GP`, with no IDT, which QEMU shows as a silent reset |
//! | `syscall` | [`init`], six lines before `arch::init` programs the four MSRs | the console is up, so the refusal can be read |
//! | invariant TSC | [`init`], before `timer::init_frequency` measures a rate | same, and see below; this one warns rather than refuses, and the table row in [`machine_discovery::x86_64`] says why |
//!
//! [`init`] re-checks NX because the assembly gate can only halt, and this one can say what is
//! wrong.
//!

//! # The invariant TSC is a different kind of check
//!
//! NX and `syscall` announce their own absence the moment the kernel uses them. The invariant TSC
//! does not: `arch::timer::init_frequency` measures the TSC's rate once, against the PIT, and on a
//! part without `CPUID.80000007H:EDX[8]` the rate moves with the core's frequency and idle state,
//! so **there is no single rate for that measurement to be the answer to**. The error appears later,
//! as time itself being wrong. A wrong assumption cannot be caught by measuring harder.
//!
//! **It warns rather than refuses** because QEMU's TCG reports the bit as zero and will not be
//! persuaded (`-cpu max,invtsc=on` clears it; it is KVM-only), and every x86 machine this project
//! runs on is TCG, so a refusal would refuse every boot. The warning prints on its own line each
//! boot, and `Gate::Warn` in [`machine_discovery::x86_64`] carries the promotion trigger.
//!
//! # BUGS
//!
//! - **One of the three gates does not refuse**: a part that does not promise a constant TSC rate
//!   boots and is trusted for timekeeping anyway. The promotion trigger is a boot on real silicon
//!   reporting the bit (milestone 87 (the `x86_64` bare-metal machine)), and the change is one token.
//! - **The gates are checked on the boot CPU only.** Secondaries replay the `EFER` write and the
//!   `syscall` MSRs without re-reading `CPUID`. On a hybrid part (Intel's P-cores and E-cores differ
//!   in leaf 7) a per-core check would earn its cost; [`machine_discovery::riscv64`] already solves
//!   the RISC-V twin (the intersection over every hart).
//! - **`RDSEED` is reported and never gated**: it is an entropy source, and a machine without it
//!   boots with a different backend (`kernel/src/user/entropy_service.rs`).

use core::arch::x86_64::CpuidResult;

use machine_discovery::x86_64::{CpuidWords, Isa, TABLE};

use crate::sync::{IrqSafeMutex, rank};
use crate::{print, println};

/// **`CPUID`, spelled so that two toolchains a year apart both accept it**
/// (milestone 304 (`cargo kani -p kernel` only ever compiled one architecture)).
///
/// `core::arch::x86_64::__cpuid` is a *safe* function on the toolchain this tree pins, because the
/// instruction has no precondition on a 64-bit part; every call site here used to read it bare and
/// say so in a comment. It was an `unsafe fn` until upstream made it safe, and **Kani bundles its
/// own rustc**, ten months behind ours (`kani-0.67.0` pins `nightly-2025-11-21`, rustc 1.93.0). So
/// the bare calls were four `E0133`s under the prover, and `arch/x86_64/` did not compile there at
/// all: the whole subtree was out of reach of the model checker for four missing keywords.
///
/// The `unsafe` block satisfies the old toolchain; `allow(unused_unsafe)` satisfies the new one,
/// where the block is redundant and `script/lint`'s `-D warnings` would otherwise reject it. That
/// allow is the **exception this file owes a reader** (AGENTS.md's ladder): it is load-bearing for
/// the prover and a foot gun for anyone who reads it as "this call needs auditing". Delete it when
/// Kani's pinned rustc passes 2026-02-ish, and the bare call comes back.
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

/// **Read what this machine is, and refuse to run on one we cannot.**
///
/// `boot_info_pointer` is ignored: the argument is the portable arch contract's (both other
/// architectures are handed a device tree), and x86 answers from the instruction set itself. Named
/// rather than dropped so the seam stays one shape across three architectures.
///
/// Every leaf is read unconditionally and believed selectively, by
/// [`Isa::decode`](machine_discovery::x86_64::Isa::decode): `CPUID` has no way to report "I do not
/// implement that leaf" other than answering with a different leaf's bits, so the rule for telling
/// those apart is the thing that can be wrong, and it lives where a host test can reach it.
///
/// # Panics
///
/// Deliberately, when the part is missing something the kernel is built on. The same discipline
/// RISC-V's refusal keeps: a kernel that ran without `syscall` would fault on the first system
/// call with nothing to say, and one that ran on a varying TSC would report a time that is simply
/// wrong. The panic prints what is missing, what the kernel uses it for, and where `CPUID` says so.
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
            }
        }
        panic!("required CPU feature is absent");
    }

    // And the third kind of answer, which is neither a refusal nor a clean boot: something the
    // kernel is built on that the part does not promise. It gets its own line rather than a
    // `no-invariant-tsc` token in the feature list below, because a missing word in a list of
    // present ones is not something a reader notices. See `Gate::Warn`.
    let unpromised = cpu.unpromised();
    if !unpromised.is_empty() {
        for row in &TABLE {
            if unpromised.contains(row.bit) {
                println!(
                    "  unpromised  : {} ({} reads 0); {}",
                    row.name, row.cite, row.why
                );
            }
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
/// `None` if [`get`]`().has_random_seed_instruction()` is false (never execute the instruction
/// without checking first: on a part that lacks it, `RDSEED` is `#UD`, and this kernel has no
/// exception recovery path for a probe that was told the answer already) or if the source stayed
/// dry across every attempt.
///
/// The retry count and the `pause` between attempts are Intel's own guidance for `RDSEED`
/// specifically (DRNG Software Implementation Guide rev. 2.2, §5.3.1.2): an "asynchronous
/// application" should give up after "somewhere between 1 and 100" retries. This is a one-shot
/// boot-tour probe rather than a service under load, so the high end costs nothing; see
/// `components/src/entropy.rs::instr` for the identical constant and reasoning on the userspace side,
/// which this kernel-side copy exists only because ring 3 does not exist yet
/// (milestone 161 (the `x86_64` kernel port)).
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

/// `CPUID` leaf `0x15`'s TSC/core-crystal-clock ratio, converted to a TSC rate in Hz, on the
/// parts that report one directly (the "ask the CPU first" half of
/// `arch::x86_64::timer::init_frequency`, whose docs explain why the fallback - calibrating
/// against the PIT - exists at all).
///
/// `None` in two cases, and both are real "this part does not say" answers rather than errors:
/// the part's own maximum leaf (from [`init`]'s leaf 0 read) is below `0x15`, so reading it would
/// misattribute a lower leaf's bits; or the part implements the leaf but leaves `ECX` (the crystal
/// clock's own frequency) at zero, which the SDM defines as "not enumerated" and which would
/// otherwise need a vendor-specific model-to-crystal table this crate does not carry (Linux's
/// `native_calibrate_tsc` has one; this port does not, and falls back to calibration instead).
///
/// Measured empirically under this project's own QEMU invocation (`-cpu max`, TCG): `max_leaf` is
/// `0xd`, so this returns `None` on every boot this kernel currently runs, and
/// `init_frequency`'s PIT-calibration fallback is what every test and every boot print's number
/// actually comes from today. The leaf is read anyway, for real hardware
/// (milestone 87 (the `x86_64` bare-metal machine)) where it may be populated.
///
/// **Whatever answer this gives, the rate it describes is a constant only because [`init`]
/// refused to boot without the invariant TSC.** That gate is what makes "the TSC's rate" a
/// quantity at all, here and in the calibration path both.
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
    println!(
        "  cpu         : x86_64, vendor {vendor}, cpuid leaves 0..{:#x} and {:#x}..{:#x}",
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
