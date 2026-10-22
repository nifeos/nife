//! Deciding what a `CPUID` answer means, on the host, including the answers that refuse the boot.
//!
//! **This file exists because the refusal path is unreachable on every machine this project can
//! run on.** Every `x86_64` part made since about 2004 has NX and `syscall`, and every QEMU model
//! reports both, so a boot gate written against those bits is a branch that has never been taken.
//! A gate that has never refused is a gate nobody has tested. Here the feature words are an
//! argument, so the part that does not exist can be handed to the decision and the refusal can be
//! watched happening.
//!
//! (The refusal *has* since been watched happening on a real boot too, by hiding NX from the guest
//! with `NIFE_CPU=max,nx=off`. That is the stronger test and it is recorded in
//! `design/roadmap/0524-the-three-x86-64-boot-gates.md`. It is not a substitute for this file: QEMU
//! can hide a feature, but only from a whole boot, so each case costs a minute of emulator and
//! cannot assert on the decision in isolation.)
//!
//! There is no fixture file, for the same reason `aarch64_id_registers.rs` beside it has none: a
//! `CPUID` leaf *is* the fixture, four 32-bit words.

use machine_discovery::x86_64::*;

/// What QEMU's `-cpu max` reports under TCG, read out of the guest by this kernel's own boot line
/// on 2026-09-21 (`script/test --arch x86_64` is TCG on an Apple Silicon host). This is the
/// machine the `x86_64` suite runs on, so a change in how these decode is a change to what every x86
/// boot of this project prints.
///
/// **The vendor is `AuthenticAMD`**, which is not the guess: QEMU's `max` model under TCG claims
/// to be AMD, and the maximum extended leaf is `0x80000021`, well past Intel's. **And
/// `extended_leaf7_edx` is zero**, which is the whole reason the invariant-TSC row is
/// [`Gate::RefuseUnlessHypervisor`]: TCG refuses to set that bit even when asked with
/// `invtsc=on`, and sets the hypervisor bit, so this machine boots with a warning.
fn qemu_max() -> CpuidWords {
    CpuidWords {
        // EAX = maximum standard leaf; EBX/EDX/ECX = "Auth" "enti" "cAMD", which is the register
        // order nobody guesses (EDX before ECX).
        leaf0: [0xd, 0x6874_7541, 0x444d_4163, 0x6974_6e65],
        // Leaf 1 ECX bit 31, the hypervisor bit. The other bits are not read.
        leaf1_ecx: 1 << 31,
        // Leaf 7 subleaf 0. EBX bit 18 is RDSEED, which this model reports.
        leaf7_0: [0, 1 << 18, 0, 0],
        extended_max_leaf: 0x8000_0021,
        // EDX bit 20 NX, bit 11 SYSCALL, plus the bits every long-mode part sets that this
        // decoder does not read.
        extended_leaf1_edx: (1 << 20) | (1 << 11) | (1 << 29),
        // No invariant TSC. See the doc comment.
        extended_leaf7_edx: 0,
        brand: brand_words("QEMU TCG CPU version 2.5+"),
    }
}

/// xenon, an Intel Core i5-7500T: milestone 87 (the x86_64 bare-metal machine), as its boot line
/// printed it on 2026-10-21 (`cpuid leaves 0..0x16 and 0x80000000..0x80000008`, `features : nx
/// syscall invariant-tsc rdseed`). Only the words the decoder reads are filled in, and the
/// hypervisor bit is clear because nothing sits between this part and the kernel.
fn xenon() -> CpuidWords {
    CpuidWords {
        // "GenuineIntel" in EBX, EDX, ECX order, as above.
        leaf0: [0x16, 0x756e_6547, 0x6c65_746e, 0x4965_6e69],
        leaf1_ecx: 0,
        leaf7_0: [0, 1 << 18, 0, 0],
        extended_max_leaf: 0x8000_0008,
        extended_leaf1_edx: (1 << 20) | (1 << 11) | (1 << 29),
        extended_leaf7_edx: 1 << 8,
        brand: brand_words("Intel(R) Core(TM) i5-7500T CPU @ 2.70GHz"),
    }
}

/// Pack an ASCII brand into the twelve registers leaves `0x80000002` through `0x80000004` return,
/// space-padded the way a vendor writes it. A helper rather than twelve hex constants, so a reader
/// can see which string a test is asserting about.
fn brand_words(s: &str) -> [u32; 12] {
    let mut bytes = [0u8; 48];
    bytes[..s.len()].copy_from_slice(s.as_bytes());
    let mut words = [0u32; 12];
    for (i, w) in words.iter_mut().enumerate() {
        *w = u32::from_le_bytes([
            bytes[i * 4],
            bytes[i * 4 + 1],
            bytes[i * 4 + 2],
            bytes[i * 4 + 3],
        ]);
    }
    words
}

#[test]
fn the_machine_this_project_boots_on_can_run_this_kernel() {
    let cpu = Isa::decode(&qemu_max());

    assert_eq!(&cpu.vendor, b"AuthenticAMD");
    assert_eq!(cpu.brand_str(), Some("QEMU TCG CPU version 2.5+"));
    assert_eq!(cpu.max_leaf, 0xd);
    assert!(cpu.features.contains(REQUIRED));
    assert!(
        cpu.has_random_seed_instruction(),
        "the entropy service's instruction backend"
    );
    assert!(cpu.under_hypervisor);
    assert!(!cpu.missing_requirements().any());
    assert_eq!(
        cpu.unpromised(),
        INVARIANT_TSC,
        "and it boots anyway, saying so, because a hypervisor keeps this clock"
    );
}

/// **Each required feature refuses the boot on its own**, and the refusal names that feature and
/// no other. One test per row rather than one test that clears all three, because the failure
/// this guards is a decode that reads the wrong bit: clearing everything at once passes whether
/// `NX` is bit 20 or bit 11.
#[test]
fn a_part_without_nx_is_refused_and_nothing_else_is_blamed() {
    let mut w = qemu_max();
    w.extended_leaf1_edx &= !(1 << 20);
    let missing = Isa::decode(&w).missing_requirements();

    assert!(missing.any());
    assert_eq!(missing.features, NX);
}

#[test]
fn a_part_without_syscall_is_refused_and_nothing_else_is_blamed() {
    let mut w = qemu_max();
    w.extended_leaf1_edx &= !(1 << 11);
    let missing = Isa::decode(&w).missing_requirements();

    assert!(missing.any());
    assert_eq!(missing.features, SYSCALL);
}

/// **The one whose absence nothing else would catch, and the one gated on who keeps the clock.**
///
/// NX and `syscall` announce themselves the moment they are used (a `#GP` on the `EFER` write, a
/// `#UD` on the first system call); a TSC that is not invariant measures perfectly at boot and is
/// wrong later, when the core changes power state and the stored rate keeps saying what it always
/// said. No amount of measuring at boot finds that, which is why it belongs in this table at all.
///
/// The four tests below are the bit present or absent, on bare metal or under a hypervisor. The
/// bare-metal refusal is the case no machine here can take: xenon reports the bit, and QEMU sets
/// the hypervisor bit. This is where it is exercised.
#[test]
fn bare_metal_without_an_invariant_tsc_is_refused_and_nothing_else_is_blamed() {
    let mut w = xenon();
    w.extended_leaf7_edx = 0;
    let cpu = Isa::decode(&w);

    assert_eq!(cpu.missing_requirements().features, INVARIANT_TSC);
    assert!(
        cpu.unpromised().is_empty(),
        "refused, so not also warned about"
    );
}

#[test]
fn a_guest_without_an_invariant_tsc_boots_and_says_so() {
    let cpu = Isa::decode(&qemu_max());

    assert!(!cpu.missing_requirements().any(), "warned, not refused");
    assert_eq!(cpu.unpromised(), INVARIANT_TSC);
}

#[test]
fn bare_metal_with_an_invariant_tsc_boots_clean() {
    let cpu = Isa::decode(&xenon());

    assert_eq!(&cpu.vendor, b"GenuineIntel");
    assert_eq!(
        cpu.brand_str(),
        Some("Intel(R) Core(TM) i5-7500T CPU @ 2.70GHz")
    );
    assert!(!cpu.under_hypervisor);
    assert!(!cpu.missing_requirements().any());
    assert!(cpu.unpromised().is_empty());
}

#[test]
fn a_guest_with_an_invariant_tsc_boots_clean() {
    let mut w = qemu_max();
    w.extended_leaf7_edx |= 1 << 8;
    let cpu = Isa::decode(&w);

    assert!(!cpu.missing_requirements().any());
    assert!(cpu.unpromised().is_empty());
}

/// **The hypervisor bit is believed only from a leaf the part answers**, the maximum-leaf rule
/// one more time. A part reporting no standard leaf above 0 answers leaf 1 with leaf 0's words,
/// and believing bit 31 of those would turn the bare-metal refusal into a warning.
#[test]
fn a_part_below_leaf_one_is_not_taken_for_a_guest() {
    let mut w = xenon();
    w.leaf0[0] = 0;
    w.leaf1_ecx = 1 << 31;
    w.extended_leaf7_edx = 0;
    let cpu = Isa::decode(&w);

    assert!(!cpu.under_hypervisor);
    assert_eq!(cpu.missing_requirements().features, INVARIANT_TSC);
}

/// **A part that does not answer the extended leaves at all is refused**, and this is the case
/// the maximum-leaf gate exists for. Every bit the kernel needs lives above `0x80000000`, and a
/// part that stops below there answers those leaves with some *other* leaf's data. Believing the
/// words would report whatever the highest implemented leaf happens to hold, which on this
/// fixture reads as a part with NX and `syscall` that has neither.
#[test]
fn a_part_with_no_extended_leaves_reports_no_features_rather_than_another_leaf_bits() {
    let mut w = qemu_max();
    w.extended_max_leaf = 0;
    let cpu = Isa::decode(&w);

    assert_eq!(cpu.features, Features::NONE.union(RDSEED));
    assert_eq!(cpu.missing_requirements().features, REQUIRED);
    assert_eq!(
        cpu.brand_str(),
        None,
        "and the brand string is not invented"
    );
}

/// The same rule one leaf space over: leaf 7 is a *standard* leaf, gated by leaf 0's own maximum,
/// and a part that stops at leaf 1 would otherwise report leaf 1's `EBX` (which carries the
/// initial APIC id in its top byte, and CLFLUSH size and brand index below it) as feature bits.
#[test]
fn a_part_below_leaf_seven_does_not_report_rdseed() {
    let mut w = qemu_max();
    w.leaf0[0] = 1;
    let cpu = Isa::decode(&w);

    assert!(!cpu.has_random_seed_instruction());
    assert!(
        !cpu.missing_requirements().any(),
        "RDSEED is optional; its absence is not a refusal"
    );
}

/// **Neither derived set can pick up a row that did not ask for it.** Both are computed from the
/// table at compile time, so the thing that can go wrong is a row's `Gate`, not the arithmetic; this
/// is the assertion a row flipped by accident has to get past, and it pins today's gates.
#[test]
fn the_derived_sets_are_exactly_the_rows_that_say_so() {
    let mut refuse = Features::NONE;
    let mut unless_hypervisor = Features::NONE;
    for row in &TABLE {
        assert!(!row.why.is_empty(), "{} has no reason beside it", row.name);
        assert!(row.cite.starts_with("CPUID."), "{}", row.name);
        match row.gate {
            Gate::Refuse => refuse = refuse.union(row.bit),
            Gate::RefuseUnlessHypervisor => unless_hypervisor = unless_hypervisor.union(row.bit),
            Gate::Report => {}
        }
    }

    assert_eq!(refuse, REQUIRED);
    assert_eq!(unless_hypervisor, REQUIRED_UNLESS_HYPERVISOR);
    assert!(REQUIRED.contains(NX));
    assert!(REQUIRED.contains(SYSCALL));
    assert!(!REQUIRED.contains(RDSEED));
    assert_eq!(
        REQUIRED_UNLESS_HYPERVISOR, INVARIANT_TSC,
        "Gate::Refuse here would refuse every boot under QEMU's TCG (calef, 2026-10-22, #2534)"
    );
}

/// Every row's bit is distinct. A copy-paste that gave two rows the same bit would make one of
/// them unreportable: the refusal would name whichever row the printer reached first, and the
/// other feature would be absent with nothing said about it.
#[test]
fn no_two_rows_share_a_bit() {
    for (i, a) in TABLE.iter().enumerate() {
        for b in &TABLE[i + 1..] {
            assert!(
                !a.bit.contains(b.bit),
                "{} and {} share a bit",
                a.name,
                b.name
            );
        }
    }
}
