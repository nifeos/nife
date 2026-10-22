use super::*;

/// One byte past the version-1 structure, so a symbolic length can straddle both size checks.
const N: usize = V1_LEN + 1;

/// **No length of handoff bytes makes the decode read past its end.**
///
/// The structure has two sizes, and which one applies is decided by a field *inside* it: a
/// version-0 handoff stops at offset 40, and only a version of 1 or more puts anything at
/// offsets 40 and 48. So the length check for the larger structure is gated on a `u32` the
/// loader wrote, and the offsets it guards are read four lines below it. Every byte here is the
/// loader's, version included.
///
/// Could plausibly have been false: reading `memmap` and `memmap_entries` unconditionally and
/// zeroing them afterwards is the obvious tidy-up, and it panics on any 40-to-55-byte handoff.
/// The module's own tests supply 40 bytes and 56 bytes and nothing between.
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

/// **A range read out of the memory map never wraps to look empty.**
///
/// [`MemoryEntry::end`] is what a frame allocator subtracts to size a region, and both `addr`
/// and `size` are `u64`s straight out of the map. A plain `+` on a firmware-supplied size near
/// `u64::MAX` wraps to an end *below* the start, which reads as a zero-length or negative range
/// and quietly hands the allocator nothing where the map described everything. The
/// `saturating_add` is the defence; this proves it holds for every pair, and that the index
/// arithmetic that reaches the entry is total at every index.
///
/// Could plausibly have been false: `end` is a two-line `const fn` whose doc is the only thing
/// saying why it saturates, and `Module::end` beside it is a second copy of the same two lines.
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

/// **A feature is never reported from a leaf the part does not answer.**
///
/// `CPUID` has no fault for a leaf a part does not implement. A read above the maximum answers
/// with some *other* leaf's data, so "this bit is clear" and "this leaf is not there" arrive as
/// the same bits on the wire and only the maximum tells them apart. That rule is the whole of
/// what [`Isa::decode`] decides, and it is stated four separate times, once per leaf, with two
/// different maxima: the standard one in `leaf0[0]` and the extended one in `extended_max_leaf`,
/// which are different numbers in different spaces and read the same way in a diff.
///
/// What makes a proof worth more than the host tests beside it is that the consequence is a
/// *refusal path nothing here can take*: milestone 524 (the three `x86_64` boot gates) gates the
/// boot on NX and SYSCALL, and every machine this project runs on reports both, so a decode that
/// invented them from a part's leaf-0 data would be silent on every machine in the building.
///
/// Could plausibly have been false: gating an *extended* leaf on the *standard* maximum is one
/// character of difference and reads as correct, and on any modern part `leaf0[0] >= 7` holds, so
/// the wrong gate and the right one agree everywhere a test is likely to look.
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
