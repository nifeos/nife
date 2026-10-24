use super::*;

/// One byte past the version-1 structure, so a symbolic length can straddle both size checks.
const N: usize = V1_LEN + 1;

/// **No length of handoff bytes makes the decode read past its end.** Which of the two sizes
/// applies is decided by the version field inside the structure, a `u32` the loader wrote, so the
/// larger length check is gated on it. Reading the memory-map fields unconditionally panics on any
/// 40-to-55-byte handoff, and the tests supply only 40 and 56.
/// Falsification: replayable `crates/machine_discovery/falsifications/x86_64.verification.no_length_of_handoff_bytes_makes_the_decode_read_past_its_end.patch`
#[kani::proof]
fn no_length_of_handoff_bytes_makes_the_decode_read_past_its_end() {
    let bytes: [u8; N] = kani::any();
    let len: usize = kani::any();
    kani::assume(len <= N);
    if let Ok(info) = BootInfo::parse(&bytes[..len]) {
        assert!(len >= V0_LEN);
        // The memory map is reported only when the bytes that carry it were actually there.
        if info.memmap_entries != 0 || info.memmap != 0 {
            assert!(info.version >= 1 && len >= V1_LEN);
        }
        kani::cover!(info.version >= 1, "a version-1 handoff is accepted");
    }
}

/// **A range read out of the memory map never wraps to look empty.** [`MemoryEntry::end`] is what
/// a frame allocator subtracts to size a region, and a plain `+` on a firmware size near
/// `u64::MAX` wraps below the start. Proves the `saturating_add` holds for every pair, and that the
/// index arithmetic reaching an entry is total. `Module::end` is a second copy of the same lines.
/// Falsification: replayable `crates/machine_discovery/falsifications/x86_64.verification.a_range_read_out_of_the_memory_map_never_wraps_to_look_empty.patch`
#[kani::proof]
fn a_range_read_out_of_the_memory_map_never_wraps_to_look_empty() {
    let bytes: [u8; MEMMAP_ENTRY_LEN] = kani::any();
    let index: usize = kani::any();
    if let Some(e) = memory_entry(&bytes, index) {
        assert_eq!(index, 0, "only one entry fits in these bytes");
        assert!(e.end() >= e.addr);
        // Ram is the only kind a frame allocator may hand out, and the module's doc is explicit
        // that AcpiReclaimable is not one of them however reclaimable its name sounds.
        assert_eq!(e.is_usable_ram(), matches!(e.kind, MemoryKind::Ram));
        kani::cover!(e.is_usable_ram(), "some entry is allocatable RAM");
    }
    // The module list is read by the same index arithmetic one structure over, and it is a
    // separate function with a separate stride; a shared proof would prove neither.
    let module_bytes: [u8; MODULE_ENTRY_LEN] = kani::any();
    if let Some(m) = module(&module_bytes, index) {
        assert!(m.end() >= m.addr);
    }
}

/// **A feature is never reported from a leaf the part does not answer.** A `CPUID` read above
/// the maximum answers with another leaf's data, so only the maximum tells "clear" from "absent",
/// and [`Isa::decode`] states that four times against two maxima. Milestone 524 (the three
/// `x86_64` boot gates) refuses the boot on NX and SYSCALL, which every machine here reports, so a
/// wrong gate would be silent; gating an extended leaf on the standard maximum is one character.
/// Falsification: replayable `crates/machine_discovery/falsifications/x86_64.verification.a_feature_is_never_reported_from_a_leaf_the_part_does_not_answer.patch`
#[kani::proof]
fn a_feature_is_never_reported_from_a_leaf_the_part_does_not_answer() {
    let w = CpuidWords {
        leaf0: [kani::any(), 0, 0, 0],
        leaf1_ecx: kani::any(),
        leaf7_0: [0, kani::any(), 0, 0],
        extended_max_leaf: kani::any(),
        extended_leaf1_edx: kani::any(),
        extended_leaf7_edx: kani::any(),
        // One symbolic word repeated. The brand claim below is about whether the bytes were
        // copied at all, so twelve distinct words would buy nothing but solver time.
        brand: [kani::any(); 12],
    };
    let isa = Isa::decode(&w);

    if w.extended_max_leaf < 0x8000_0001 {
        assert!(!isa.features.contains(NX));
        assert!(!isa.features.contains(SYSCALL));
        // The two required rows both live in that leaf, so a part which does not answer it can
        // never clear the gate. This is the claim the boot depends on, rather than a restatement
        // of the branch above it.
        assert!(isa.missing_requirements().any());
    }
    if w.extended_max_leaf < 0x8000_0007 {
        assert!(!isa.features.contains(INVARIANT_TSC));
    }
    if w.extended_max_leaf < 0x8000_0004 {
        // Not `brand_str`, which trims and validates: the claim is that nothing was copied.
        assert_eq!(isa.brand, [0u8; 48]);
    }
    if w.leaf0[0] < 7 {
        assert!(!isa.has_random_seed_instruction());
    }

    if w.leaf0[0] < 1 {
        assert!(!isa.under_hypervisor);
    }

    // `REQUIRED` and `REQUIRED_UNLESS_HYPERVISOR` are folded out of `TABLE` by a `const fn`
    // comparing discriminants, which is the one place a row's gate can be got wrong without any
    // call site changing. So the refusal is tied back to the three features by name: a table edit
    // that moved NX to `Gate::RefuseUnlessHypervisor` would boot a guest whose page tables'
    // no-execute is a comment, and one that moved the invariant TSC to `Gate::Refuse` would
    // refuse every boot under QEMU's TCG.
    assert_eq!(
        isa.missing_requirements().any(),
        !(isa.features.contains(NX)
            && isa.features.contains(SYSCALL)
            && (isa.under_hypervisor || isa.features.contains(INVARIANT_TSC)))
    );
    assert!(!isa.unpromised().contains(NX) && !isa.unpromised().contains(SYSCALL));
    // A row the part does not report is refused or warned, never both, and warned only under a
    // hypervisor.
    assert!(
        isa.missing_requirements()
            .features
            .difference(isa.unpromised())
            == isa.missing_requirements().features
    );
    if !isa.under_hypervisor {
        assert!(isa.unpromised().is_empty());
    }

    kani::cover!(
        !isa.missing_requirements().any(),
        "some part clears the boot gate"
    );
    kani::cover!(isa.missing_requirements().any(), "some part is refused");
    kani::cover!(
        !isa.missing_requirements().any() && !isa.unpromised().is_empty(),
        "some guest boots with a warning"
    );
}
