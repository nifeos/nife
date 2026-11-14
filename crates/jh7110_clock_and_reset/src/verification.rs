//! **Machine-checked proofs (Kani) of the domains and plans**, run with `cargo kani`. The domains
//! and plans are this crate's own constants and the device-tree properties are symbolic bytes, so
//! each harness covers every word and every property value inside its stated bound.
//!
//! Name: provisional, after the `verification` module every other Kani crate here keeps out of
//! line.

use super::*;

/// One of the three domains this crate describes, chosen by the solver.
fn any_domain() -> Domain {
    match kani::any::<u8>() % 3 {
        0 => SYSTEM_TOP_GROUP,
        1 => ALWAYS_ON,
        _ => SYSTEM,
    }
}

/// True when every id in `plan` is inside `domain`, no clock is selected a parent after it was
/// enabled, and no clock step comes after a reset release.
fn ungates_before_releasing(plan: &[Step], domain: &Domain) -> bool {
    let mut released = false;
    let mut i = 0;
    while i < plan.len() {
        match plan[i] {
            Step::EnableClock(clock) => {
                if released || domain.clock_offset(clock).is_none() {
                    return false;
                }
            }
            Step::SelectParent { clock, parent } => {
                if released || domain.clock_offset(clock).is_none() || parent > 0xf {
                    return false;
                }
                let mut j = 0;
                while j < i {
                    if plan[j] == Step::EnableClock(clock) {
                        return false;
                    }
                    j += 1;
                }
            }
            Step::DeassertReset(reset) => {
                if domain.reset_bit(reset).is_none() {
                    return false;
                }
                released = true;
            }
        }
        i += 1;
    }
    true
}

/// The phandle lookup a caller supplies, modeled as a SYS controller with one cell. Every
/// specifier in the bytes the solver chooses therefore reaches the id bounds check, which is
/// the step that makes a store. The solver cannot afford a lookup that answers differently on
/// every call (the plan harnesses then run for over an hour), nor more than three specifiers, nor property lengths other than a whole number of specifiers, so the other answers (another
/// provider, a different cell count, an error) are covered by the host tests, not proved.
fn system_provider(_phandle: u32, name: &[u8]) -> Result<Option<&'static [u8]>, ()> {
    if name == b"compatible" {
        Ok(Some(COMPATIBLE_SYSTEM_CLOCK_AND_RESET_GENERATOR))
    } else {
        Ok(Some(&[0, 0, 0, 1]))
    }
}

/// The phandle lookup for an assigned-clocks list: an even phandle is `aoncrg` and an odd one
/// `syscrg`, both with one cell. It answers by phandle alone, so the solver can pick either
/// provider for every specifier and still afford the proof; the other answers (another
/// provider, a different cell count, an error) are covered by the host tests, not proved.
fn split_provider(phandle: u32, name: &[u8]) -> Result<Option<&'static [u8]>, ()> {
    if name != b"compatible" {
        Ok(Some(&[0, 0, 0, 1]))
    } else if phandle % 2 == 0 {
        Ok(Some(COMPATIBLE_ALWAYS_ON_CLOCK_AND_RESET_GENERATOR))
    } else {
        Ok(Some(COMPATIBLE_SYSTEM_CLOCK_AND_RESET_GENERATOR))
    }
}

/// **`with_parent` changes only the mux field**: for every word and every parent, each bit
/// outside [`CLOCK_MULTIPLEXER_MASK`] is as it was (the enable bit and the divider among them), and
/// the field holds the parent's low four bits, so an over-wide parent never spills.
/// Falsification: replayable `crates/jh7110_clock_and_reset/falsifications/verification.with_parent_changes_only_the_mux_field.patch`
#[kani::proof]
fn with_parent_changes_only_the_mux_field() {
    let word: u32 = kani::any();
    let parent: u32 = kani::any();
    let result = with_parent(word, parent);
    assert_eq!(
        result & !CLOCK_MULTIPLEXER_MASK,
        word & !CLOCK_MULTIPLEXER_MASK
    );
    assert_eq!(result & CLOCK_ENABLE, word & CLOCK_ENABLE);
    assert_eq!((result >> 24) & 0xf, parent & 0xf);
}

/// **A clock offset is one of the domain's own words**: `clock_offset` answers `None` exactly
/// for an index at or past the domain's clock count, and otherwise a word-aligned offset below
/// `4 * clocks`, which ends before the domain's first reset word.
/// Falsification: replayable `crates/jh7110_clock_and_reset/falsifications/verification.a_clock_offset_is_one_of_the_domains_own_words.patch`
#[kani::proof]
fn a_clock_offset_is_one_of_the_domains_own_words() {
    let domain = any_domain();
    let index: u32 = kani::any();
    match domain.clock_offset(index) {
        None => assert!(index >= domain.clocks),
        Some(offset) => {
            assert!(index < domain.clocks);
            assert_eq!(offset % 4, 0);
            assert!(offset < 4 * u64::from(domain.clocks));
            assert!(offset + 4 <= domain.reset_assert);
        }
    }
}

/// **A reset line is one bit of one of the domain's own words**: `reset_bit` answers `None`
/// exactly for an id at or past the domain's reset count, and otherwise word-aligned assert and
/// status offsets that sit the same distance from their first words, a word index of
/// `id / 32` at most, and a mask of exactly one bit, the bit `id % 32`.
/// Falsification: replayable `crates/jh7110_clock_and_reset/falsifications/verification.a_reset_line_is_one_bit_of_one_of_the_domains_own_words.patch`
#[kani::proof]
fn a_reset_line_is_one_bit_of_one_of_the_domains_own_words() {
    let domain = any_domain();
    let id: u32 = kani::any();
    match domain.reset_bit(id) {
        None => assert!(id >= domain.resets),
        Some(bit) => {
            assert!(id < domain.resets);
            assert_eq!(bit.assert_offset % 4, 0);
            assert_eq!(bit.status_offset % 4, 0);
            let word = bit.assert_offset - domain.reset_assert;
            assert_eq!(word, bit.status_offset - domain.reset_status);
            assert_eq!(word, u64::from(id / 32) * 4);
            assert_eq!(bit.mask, 1u32 << (id % 32));
            assert!(bit.mask.is_power_of_two());
        }
    }
}

/// **Every fixed plan ungates its clocks before it releases a reset**: the TRNG's, both halves
/// of the Ethernet controller's, the PMIC bus's and both SD/MMC slots' plans, and the v1.3B
/// transmit parent walked before the Ethernet controller's AON half, hold only ids inside
/// their own domain, put every clock step ahead of every reset release, and choose a clock's
/// parent before enabling that clock.
/// Falsification: replayable `crates/jh7110_clock_and_reset/falsifications/verification.every_fixed_plan_ungates_its_clocks_before_releasing_a_reset.patch`
#[kani::proof]
#[kani::unwind(8)]
fn every_fixed_plan_ungates_its_clocks_before_releasing_a_reset() {
    assert!(ungates_before_releasing(
        ENTROPY_BRING_UP,
        &SYSTEM_TOP_GROUP
    ));
    assert!(ungates_before_releasing(GMAC0_SYSTEM_BRING_UP, &SYSTEM));
    assert!(ungates_before_releasing(
        GMAC0_ALWAYS_ON_BRING_UP,
        &ALWAYS_ON
    ));
    assert!(ungates_before_releasing(
        POWER_MANAGEMENT_CHIP_BUS_BRING_UP,
        &SYSTEM
    ));
    assert!(ungates_before_releasing(SD_IO_BRING_UP[0], &SYSTEM));
    assert!(ungates_before_releasing(SD_IO_BRING_UP[1], &SYSTEM));
    assert!(ungates_before_releasing(
        &with_parent_first(
            GMAC0_TRANSMIT_PARENT_REDUCED_MEDIA_INDEPENDENT_INTERFACE_RECEIVE_TRANSMIT
        ),
        &ALWAYS_ON
    ));
}

/// `parent` followed by [`GMAC0_ALWAYS_ON_BRING_UP`], the order a caller walks them in.
fn with_parent_first(parent: Step) -> [Step; 6] {
    let mut plan = [parent; 6];
    let mut i = 0;
    while i < GMAC0_ALWAYS_ON_BRING_UP.len() {
        plan[i + 1] = GMAC0_ALWAYS_ON_BRING_UP[i];
        i += 1;
    }
    plan
}

/// **A transmit parent read from a tree is one of `gmac0_tx`'s two inputs**: for an
/// `assigned-clocks` of two specifiers and an `assigned-clock-parents` of one or two, of any
/// bytes, answered by `aoncrg` and `syscrg`, an assigned step selects a parent of `gmac0_tx`
/// and no other clock, with mux input 0 or 1, and walked before [`GMAC0_ALWAYS_ON_BRING_UP`] it
/// keeps that plan's order: the parent is chosen while the clock is gated, ahead of every
/// reset release.
/// Falsification: replayable `crates/jh7110_clock_and_reset/falsifications/verification.a_transmit_parent_from_the_tree_is_one_of_the_clocks_two_inputs.patch`
#[kani::proof]
#[kani::unwind(8)]
fn a_transmit_parent_from_the_tree_is_one_of_the_clocks_two_inputs() {
    let clocks: [u8; 16] = kani::any();
    let parents: [u8; 16] = kani::any();
    let parents = if kani::any() {
        &parents[..8]
    } else {
        &parents[..]
    };

    if let Ok(TransmitParent::Assigned(step)) =
        gmac0_transmit_parent(Some(&clocks), Some(parents), split_provider)
    {
        assert!(matches!(
            step,
            Step::SelectParent {
                clock: ALWAYS_ON_CLOCK_GMAC0_TRANSMIT,
                parent: 0 | 1,
            }
        ));
        assert!(ungates_before_releasing(
            &with_parent_first(step),
            &ALWAYS_ON
        ));
    }
}

/// **A plan built from a device's specifiers holds only usable steps, in order**: for two
/// clock specifiers and one reset specifier of any bytes, answered by a SYS controller, the
/// plan has at most `MAXIMUM_DEVICE_STEPS` steps, every id in the SYS domain's bounds, every
/// clock step ahead of every reset release.
/// Falsification: replayable `crates/jh7110_clock_and_reset/falsifications/verification.a_device_plan_from_specifiers_ungates_before_it_releases.patch`
#[kani::proof]
#[kani::unwind(24)]
fn a_device_plan_from_specifiers_ungates_before_it_releases() {
    let clocks: [u8; 16] = kani::any();
    let resets: [u8; 8] = kani::any();
    let clocks = Some(&clocks[..]);
    let resets = Some(&resets[..]);

    if let Ok(device) = DevicePlan::from_specifiers(clocks, resets, system_provider) {
        assert!(device.plan().len() <= MAXIMUM_DEVICE_STEPS);
        assert!(ungates_before_releasing(device.plan(), &SYSTEM));
    }
}

/// **The PMIC bus plan is never empty, never longer than its bound, and ungates before it
/// releases**: from any node properties (two clock and one reset specifier, any
/// `reg` bytes) and a SYS controller, `from_nodes` answers either the tree's own plan or, when the
/// tree gave no usable step, the constant plan, and both keep every id in the SYS domain's
/// bounds with every clock step ahead of every reset release.
/// Falsification: replayable `crates/jh7110_clock_and_reset/falsifications/verification.the_power_management_chip_bus_plan_is_never_empty_and_ungates_before_it_releases.patch`
#[kani::proof]
#[kani::unwind(24)]
fn the_power_management_chip_bus_plan_is_never_empty_and_ungates_before_it_releases() {
    let clocks: [u8; 16] = kani::any();
    let resets: [u8; 8] = kani::any();
    let bus_register_property: [u8; 16] = kani::any();
    let chip_register_property: [u8; 4] = kani::any();
    let clocks = Some(&clocks[..]);
    let resets = Some(&resets[..]);
    let bus_register_property = Some(&bus_register_property[..]);
    let chip_register_property = Some(&chip_register_property[..]);

    if let Ok(bus) = PowerManagementChipBus::from_nodes(
        COMPATIBLE_POWER_MANAGEMENT_CHIP_MAINLINE,
        chip_register_property,
        bus_register_property,
        clocks,
        resets,
        system_provider,
    ) {
        assert!(!bus.plan().is_empty());
        assert!(bus.plan().len() <= MAXIMUM_POWER_MANAGEMENT_CHIP_BUS_STEPS);
        assert!(ungates_before_releasing(bus.plan(), &SYSTEM));
        if !bus.from_tree {
            assert!(bus.plan() == POWER_MANAGEMENT_CHIP_BUS_BRING_UP);
        }
    }
}

/// **`is_deasserted` reads the bit the mask names**: for a one-bit mask and any status word,
/// the answer is that bit, and a status word with the bit clear is never released.
/// Falsification: replayable `crates/jh7110_clock_and_reset/falsifications/verification.is_deasserted_reads_the_bit_the_mask_names.patch`
#[kani::proof]
fn is_deasserted_reads_the_bit_the_mask_names() {
    let status: u32 = kani::any();
    let bit: u32 = kani::any();
    kani::assume(bit < 32);
    let mask = 1u32 << bit;
    assert_eq!(is_deasserted(status, mask), (status >> bit) & 1 == 1);
    assert!(!is_deasserted(status & !mask, mask));
    assert!(is_deasserted(status | mask, mask));
}

/// **`is_clock_enabled` reads bit 31 and nothing else**: for any word the answer is the top
/// bit, whatever the mux and divider fields hold.
/// Falsification: replayable `crates/jh7110_clock_and_reset/falsifications/verification.is_clock_enabled_reads_bit_31_and_nothing_else.patch`
#[kani::proof]
fn is_clock_enabled_reads_bit_31_and_nothing_else() {
    let word: u32 = kani::any();
    assert_eq!(is_clock_enabled(word), word >> 31 == 1);
    assert!(is_clock_enabled(word | CLOCK_ENABLE));
    assert!(!is_clock_enabled(word & !CLOCK_ENABLE));
}

/// **`clock_divider` is the low 24 bits and nothing else**: for any word the answer is the
/// word's bits 23:0, so it never exceeds `0x00ff_ffff`, and neither the enable bit nor the
/// mux field moves it.
/// Falsification: replayable `crates/jh7110_clock_and_reset/falsifications/verification.clock_divider_is_the_low_24_bits_and_nothing_else.patch`
#[kani::proof]
fn clock_divider_is_the_low_24_bits_and_nothing_else() {
    let word: u32 = kani::any();
    assert_eq!(clock_divider(word), (word << 8) >> 8);
    assert!(clock_divider(word) <= 0x00ff_ffff);
    assert_eq!(clock_divider(word | CLOCK_ENABLE), clock_divider(word));
    assert_eq!(
        clock_divider(with_parent(word, kani::any())),
        clock_divider(word)
    );
}
