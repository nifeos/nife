//! The crate's host tests, moved out of `lib.rs` unchanged on 2026-10-12 (UTC) by lane
//! maintainer/near-ceiling-splits, so the crate root stays under §266 (a Rust source file stays
//! under 2,000 lines). The SD/MMC test that sat in its own `sdio_tests` module is at the end.

use super::*;

const STGCRG_MAINLINE: &[u8] = include_bytes!("../tests/fixtures/jh7110-stgcrg-mainline.dtb");
const CLKGEN_VENDOR: &[u8] = include_bytes!("../tests/fixtures/jh7110-clkgen-vendor.dtb");
const CLKGEN_VENDOR_UNNAMED: &[u8] =
    include_bytes!("../tests/fixtures/jh7110-clkgen-vendor-unnamed.dtb");
const CLKGEN_VENDOR_MISMATCHED: &[u8] =
    include_bytes!("../tests/fixtures/jh7110-clkgen-vendor-mismatched-names.dtb");
/// The blob `crates/device_tree_blob`'s own tests boot-verify against, so a change to QEMU's
/// `virt` board is caught here rather than surfacing as a mystery at the bench.
const QEMU_RISCV64_VIRT: &[u8] =
    include_bytes!("../../device_tree_blob/tests/fixtures/qemu-riscv64-virt.dtb");

const PMIC_BUS_RADON: &[u8] = include_bytes!("../tests/fixtures/jh7110-pmic-bus-radon.dtb");
const PMIC_BUS_MAINLINE: &[u8] = include_bytes!("../tests/fixtures/jh7110-pmic-bus-mainline.dtb");
const PMIC_BUS_FOREIGN: &[u8] = include_bytes!("../tests/fixtures/jh7110-pmic-bus-foreign.dtb");
const PMIC_BUS_OVERFULL: &[u8] = include_bytes!("../tests/fixtures/jh7110-pmic-bus-overfull.dtb");

#[test]
fn radons_pmic_bus_is_one_real_gate_and_one_reset_and_the_virtual_clock_is_skipped() {
    // The fixture is radon's own U-Boot tree. Its bus names clock 298 first, which is a
    // divide-by-one child of 143 with no register: writing "word 298" would be a store to
    // 0x4a8 of the SYS window, past every clock and reset word. It must be skipped, counted,
    // and the plan must still be the tree's own.
    let tree = device_tree_blob::DeviceTreeBlob::from_bytes(PMIC_BUS_RADON).unwrap();
    let bus = pmic_bus(&tree).unwrap();
    assert_eq!(
        bus.plan(),
        &[
            Step::EnableClock(SYSCLK_I2C5_APB),
            Step::DeassertReset(SYSRST_I2C5_APB)
        ]
    );
    assert!(bus.from_tree);
    assert_eq!(bus.pmic, Some(COMPATIBLE_PMIC_VENDOR));
    assert_eq!(bus.skipped, 1, "clock 298, and nothing else");
    assert!(!bus.truncated);
}

#[test]
fn radons_sys_window_is_the_vendor_nodes_first_entry_found_by_name() {
    let tree = device_tree_blob::DeviceTreeBlob::from_bytes(PMIC_BUS_RADON).unwrap();
    let found = discover_sys(&tree).unwrap();
    assert_eq!((found.base, found.size), (SYS_BASE, SYS_SIZE));
    assert!(found.from_tree);
    assert_eq!(found.compatible, Some(COMPATIBLE_VENDOR_CLKGEN));
    // And STG, from the same node, is still its second window.
    assert_eq!(discover(&tree).unwrap().base, STG_BASE);
}

#[test]
fn mainlines_pmic_bus_gives_the_same_plan_through_syscrg() {
    // Two trees that spell everything differently converge on 143 and 81, which is the
    // agreement that lets the constant fallback exist at all.
    let tree = device_tree_blob::DeviceTreeBlob::from_bytes(PMIC_BUS_MAINLINE).unwrap();
    let bus = pmic_bus(&tree).unwrap();
    assert_eq!(bus.plan(), PMIC_BUS_BRING_UP);
    assert!(bus.from_tree);
    assert_eq!(bus.pmic, Some(COMPATIBLE_PMIC_MAINLINE));
    assert_eq!(bus.skipped, 0);
    let found = discover_sys(&tree).unwrap();
    assert_eq!(found.base, SYS_BASE);
    assert_eq!(found.compatible, Some(COMPATIBLE_SYSCRG));
}

#[test]
fn a_bus_naming_only_foreign_specifiers_falls_back_and_says_so() {
    // Wrong domain, out of bounds twice, and a two-cell provider that makes the rest of its
    // list unreadable (the trailing `<&syscrg 143>` included: refusing is the safe direction).
    // Nothing usable is left, so the constant plan is used, and `from_tree` says it was.
    let tree = device_tree_blob::DeviceTreeBlob::from_bytes(PMIC_BUS_FOREIGN).unwrap();
    let bus = pmic_bus(&tree).unwrap();
    assert_eq!(bus.plan(), PMIC_BUS_BRING_UP);
    assert!(!bus.from_tree);
    assert_eq!(bus.pmic, Some(COMPATIBLE_PMIC_MAINLINE));
    // aoncrg 3, syscrg 190, then 36 - 16 = 20 bytes read as three eight-byte strides, then
    // syscrg 126 in `resets`.
    assert_eq!(bus.skipped, 2 + 3 + 1);
}

/// More usable gates than the plan holds: the first four are kept in order, the rest are
/// counted as cut off and not as foreign, and a bus that names clocks but no resets is still
/// read (the absence of one property is not the absence of the bus).
#[test]
fn a_bus_with_more_gates_than_the_plan_holds_keeps_the_first_and_says_so() {
    let tree = device_tree_blob::DeviceTreeBlob::from_bytes(PMIC_BUS_OVERFULL).unwrap();
    let bus = pmic_bus(&tree).unwrap();
    assert_eq!(
        bus.plan(),
        &[
            Step::EnableClock(140),
            Step::EnableClock(141),
            Step::EnableClock(142),
            Step::EnableClock(143)
        ]
    );
    assert_eq!(bus.plan().len(), MAX_PMIC_BUS_STEPS);
    assert!(bus.truncated);
    assert!(bus.from_tree);
    assert_eq!(bus.skipped, 0);
}

#[test]
fn qemus_virt_board_has_no_pmic_and_gets_the_constant_plan_unclaimed() {
    let tree = device_tree_blob::DeviceTreeBlob::from_bytes(QEMU_RISCV64_VIRT).unwrap();
    let bus = pmic_bus(&tree).unwrap();
    assert_eq!(bus.plan(), PMIC_BUS_BRING_UP);
    assert!(!bus.from_tree);
    assert_eq!(bus.pmic, None);
    assert!(!discover_sys(&tree).unwrap().from_tree);
}

#[test]
fn i2c5s_clock_and_reset_land_where_linux_puts_them() {
    // [mainline-clk] `base + 4 * idx`, and radon's OpenSBI's own `0x13020228 + 5 * 4` for the
    // same gate: two derivations of 0x23c.
    assert_eq!(SYS.clock_offset(SYSCLK_I2C5_APB), Some(0x23c));
    assert_eq!(0x228 + 5 * 4, 0x23c);
    let r = SYS.reset_bit(SYSRST_I2C5_APB).unwrap();
    assert_eq!(
        (r.assert_offset, r.status_offset, r.mask),
        (0x300, 0x310, 1 << 17)
    );
    // Radon's OpenSBI reads the index from the node name; `i2c@...` gives '@' - '0' = 16,
    // which is clock 138 + 16 = 154, JH7110_SYSCLK_UART4_CORE. Written down because it is
    // why the firmware's own re-enable cannot be relied on.
    assert_eq!(0x228 + (u64::from(b'@' - b'0')) * 4, 4 * 154);
}

#[test]
fn the_sys_domains_last_clock_word_does_not_reach_its_reset_words() {
    let last = SYS.clock_offset(SYS.clocks - 1).unwrap();
    assert_eq!(last, 0x2f4);
    assert!(last + 4 <= SYS.reset_assert);
    assert_eq!(SYS.clock_offset(SYS.clocks), None);
    assert_eq!(SYS.clock_offset(298), None, "radon's virtual core clock");
    assert_eq!(SYS.reset_bit(125).unwrap().assert_offset, 0x2f8 + 12);
    assert_eq!(SYS.reset_bit(126), None);
}

#[test]
fn the_pmic_plan_ungates_before_it_releases() {
    assert!(matches!(PMIC_BUS_BRING_UP[0], Step::EnableClock(_)));
    assert!(matches!(
        PMIC_BUS_BRING_UP[PMIC_BUS_BRING_UP.len() - 1],
        Step::DeassertReset(_)
    ));
}

#[test]
fn mainline_stgcrg_is_read_from_its_single_reg() {
    let tree = device_tree_blob::DeviceTreeBlob::from_bytes(STGCRG_MAINLINE).unwrap();
    let found = discover(&tree).unwrap();
    assert_eq!(found.base, 0x1023_0000);
    assert_eq!(found.size, 0x1_0000);
    assert!(found.from_tree);
    assert_eq!(found.compatible, Some(COMPATIBLE_STGCRG));
}

#[test]
fn the_vendor_clkgens_stg_window_is_found_by_name() {
    // The whole point of the vendor arm: `reg[0]` is the SYS domain at 0x13020000, and a
    // driver that took it would enable clock 15 of the wrong controller.
    let tree = device_tree_blob::DeviceTreeBlob::from_bytes(CLKGEN_VENDOR).unwrap();
    let found = discover(&tree).unwrap();
    assert_eq!(found.base, STG_BASE, "the second window, not the first");
    assert_eq!(found.size, 0x1_0000);
    assert!(found.from_tree);
    assert_eq!(found.compatible, Some(COMPATIBLE_VENDOR_CLKGEN));
}

#[test]
fn a_multi_window_node_without_reg_names_falls_back_rather_than_guessing() {
    let tree = device_tree_blob::DeviceTreeBlob::from_bytes(CLKGEN_VENDOR_UNNAMED).unwrap();
    let found = discover(&tree).unwrap();
    assert_eq!(found.base, STG_BASE);
    assert!(
        !found.from_tree,
        "the address is right and nothing in the tree said so; the transcript must say which"
    );
    assert_eq!(found.compatible, None);
}

#[test]
fn qemus_virt_board_has_no_clock_controller_at_all() {
    // This is the path every machine this repository's CI boots takes, and it is why this
    // milestone cannot be gated in an emulator: `virt` has no clock or reset controller to
    // program, so what runs here is the fallback and the arithmetic, never a device.
    let tree = device_tree_blob::DeviceTreeBlob::from_bytes(QEMU_RISCV64_VIRT).unwrap();
    let found = discover(&tree).unwrap();
    assert!(!found.from_tree);
    assert_eq!(found.compatible, None);
    assert_eq!(found.base, STG_BASE);
}

#[test]
fn a_reg_names_entry_past_the_end_of_reg_is_refused_rather_than_read() {
    // `reg-names` lists three names but `reg` has only two windows, with "stg" at index 2: an
    // index `regions` was never filled at. `discover_as`'s `i < n` guard exists for exactly
    // this: a tree this malformed must fall back to the corroborated constant rather than
    // read the zeroed slot an off-by-one would land on. Kills the `i < n -> true` and
    // `< -> <=` mutants, neither of which any other fixture reaches.
    let tree = device_tree_blob::DeviceTreeBlob::from_bytes(CLKGEN_VENDOR_MISMATCHED).unwrap();
    let found = discover(&tree).unwrap();
    assert!(
        !found.from_tree,
        "the named window does not exist; this must not be trusted"
    );
    assert_eq!(found.base, STG_BASE);
    assert_eq!(found.compatible, None);
}

#[test]
fn a_report_of_zeros_is_not_a_report_of_success() {
    // This is radon's 2026-09-04 shape, one level up: a window with nothing behind it accepts
    // every store and reads back zero, so `has_clocks_running` must be false on the *after*
    // words rather than on the fact that the writes returned.
    let dead = Report {
        clocks: 2,
        had_reset: true,
        ..Report::default()
    };
    assert!(!dead.has_clocks_running());
    assert!(!dead.was_already_up());
    assert!(!dead.released);
}

#[test]
fn a_report_with_no_clock_steps_claims_nothing() {
    // `clocks: 0` must not read as "all zero of my clocks are running", which is what a bare
    // `.iter().all()` would say. A plan that enabled nothing has proven nothing.
    assert!(!Report::default().has_clocks_running());
    assert!(!Report::default().was_already_up());
}

#[test]
fn clocks_running_does_not_imply_already_up_while_the_reset_is_still_asserted() {
    // Every existing "clocks true" fixture also has the reset already released, so the
    // `!self.had_reset || ...` clause in `was_already_up` never gets to matter: had_reset is
    // true and the release check is true, and deleting the `!` leaves that OR true either
    // way. Force the reset bit still SET in `reset_assert_before` (not yet deasserted) so the
    // real answer is false and only the reset half of the OR can produce it.
    let clocked_but_held = Report {
        clock_before: [CLOCK_ENABLE, CLOCK_ENABLE, 0, 0],
        clock_after: [CLOCK_ENABLE, CLOCK_ENABLE, 0, 0],
        clocks: 2,
        had_reset: true,
        reset_assert_before: 1 << STGRST_SEC_AHB,
        reset_assert_after: 0,
        ..Report::default()
    };
    assert!(clocked_but_held.has_clocks_running());
    assert!(
        !clocked_but_held.was_already_up(),
        "the reset bit was still asserted before this ran"
    );
}

#[test]
fn an_unrelated_bit_in_the_before_word_does_not_defeat_the_release_check() {
    // Complements the test above: here the target reset bit IS already clear (so the real
    // answer is true), but `reset_assert_before` carries an unrelated bit the mask does not
    // cover. `&` ignores it; `|` does not, which is what this catches.
    let released_with_noise = Report {
        clock_before: [CLOCK_ENABLE, CLOCK_ENABLE, 0, 0],
        clock_after: [CLOCK_ENABLE, CLOCK_ENABLE, 0, 0],
        clocks: 2,
        had_reset: true,
        reset_assert_before: 0x8000,
        reset_assert_after: 0x8000 ^ (1 << STGRST_SEC_AHB),
        ..Report::default()
    };
    assert_eq!(released_with_noise.reset_mask(), 1 << STGRST_SEC_AHB);
    assert!(
        released_with_noise.was_already_up(),
        "the target bit was already clear; the noise bit must not matter"
    );
}

#[test]
fn reset_mask_recovers_the_bit_that_changed_between_the_two_assert_words() {
    let r = Report {
        reset_assert_before: 0b1010,
        reset_assert_after: 0b0010,
        ..Report::default()
    };
    assert_eq!(
        r.reset_mask(),
        0b1000,
        "the bit that differs, not 0 or 1 or their union"
    );
}

#[test]
fn a_reset_word_past_the_first_multiplies_the_word_index_by_four() {
    // STG has only 23 resets, so every id this crate ever asks for lands in word 0 and
    // `* 4`/`/ 4` are indistinguishable there. A larger domain (SYS has 126, per this
    // module's own doc comment) is the shape `reset_bit`'s arithmetic is written for, so
    // construct one and ask for id 35, which is word 1.
    let big = Domain {
        reset_assert: 0x10,
        reset_status: 0x20,
        resets: 128,
        clocks: 0,
    };
    let r = big.reset_bit(35).unwrap();
    assert_eq!(r.assert_offset, 0x10 + 4, "word 1: (35 / 32) * 4 == 4");
    assert_eq!(r.status_offset, 0x20 + 4);
    assert_eq!(r.mask, 1 << 3, "35 % 32 == 3");
}

#[test]
fn the_firmware_having_already_done_it_is_a_distinguishable_outcome() {
    // The one result that would refute this milestone's premise: the clocks were already on
    // and the reset already released before anything here ran, so the TRNG's zeros have some
    // other cause. A bench transcript has to be able to say this, which is why the report
    // keeps `before` words at all.
    let up = Report {
        clock_before: [CLOCK_ENABLE | 4, CLOCK_ENABLE | 4, 0, 0],
        clock_after: [CLOCK_ENABLE | 4, CLOCK_ENABLE | 4, 0, 0],
        clocks: 2,
        had_reset: true,
        reset_assert_before: 0,
        reset_assert_after: 0,
        reset_status_after: 1 << STGRST_SEC_AHB,
        released: true,
        polls: 1,
        ..Report::default()
    };
    assert!(up.has_clocks_running());
    assert!(up.was_already_up());
}

#[test]
fn a_device_this_run_actually_started_is_not_reported_as_already_up() {
    let started = Report {
        clock_before: [0, 0, 0, 0],
        clock_after: [CLOCK_ENABLE, CLOCK_ENABLE, 0, 0],
        clocks: 2,
        had_reset: true,
        reset_assert_before: 1 << STGRST_SEC_AHB,
        reset_assert_after: 0,
        reset_status_after: 1 << STGRST_SEC_AHB,
        released: true,
        polls: 3,
        ..Report::default()
    };
    assert!(started.has_clocks_running());
    assert!(!started.was_already_up(), "this run is what turned it on");
}

#[test]
fn the_trng_plan_enables_both_clocks_before_it_touches_the_reset() {
    // [mainline-rst]'s own comment is the reason this order is a test rather than a comment:
    // "if the associated clock is gated, deasserting might otherwise hang forever". A plan
    // that deasserted first would wedge the boot on hardware and pass every host test.
    let reset_at = TRNG_BRING_UP
        .iter()
        .position(|s| matches!(s, Step::DeassertReset(_)))
        .expect("the plan must deassert something");
    let clocks = TRNG_BRING_UP
        .iter()
        .filter(|s| matches!(s, Step::EnableClock(_)))
        .count();
    assert_eq!(clocks, 2, "hclk and ahb, per [mainline-trng]'s probe");
    assert_eq!(reset_at, 2, "the reset comes last");
}

#[test]
fn the_two_trees_agree_on_the_identifiers() {
    // The rebase arithmetic from [vendor-ids], written out so a reader can check it rather
    // than take the module header's word for it. If StarFive ever renumbers a group, this is
    // the test that fails.
    const VENDOR_STG_CLK_BASE: u32 = 190; // JH7110_HIFI4_CLK_CORE
    const VENDOR_SEC_HCLK: u32 = 205;
    const VENDOR_SEC_MISCAHB_CLK: u32 = 206;
    const VENDOR_STG_RST_BASE: u32 = 128; // RSTN_U0_STG_SYSCON_PRESETN
    const VENDOR_SEC_TOP_HRESETN: u32 = 131;

    assert_eq!(VENDOR_SEC_HCLK - VENDOR_STG_CLK_BASE, STGCLK_SEC_AHB);
    assert_eq!(
        VENDOR_SEC_MISCAHB_CLK - VENDOR_STG_CLK_BASE,
        STGCLK_SEC_MISC_AHB
    );
    assert_eq!(VENDOR_SEC_TOP_HRESETN - VENDOR_STG_RST_BASE, STGRST_SEC_AHB);
}

#[test]
fn clock_words_are_one_per_index_and_bounded() {
    assert_eq!(STG.clock_offset(0), Some(0x00));
    assert_eq!(STG.clock_offset(STGCLK_SEC_AHB), Some(0x3c));
    assert_eq!(STG.clock_offset(STGCLK_SEC_MISC_AHB), Some(0x40));
    // 29 clocks, so 28 is the last. An identifier from another domain gets nothing rather
    // than an offset that lands somewhere plausible.
    assert_eq!(STG.clock_offset(28), Some(0x70));
    assert_eq!(STG.clock_offset(29), None);
    assert_eq!(STG.clock_offset(205), None); // the vendor number, un-rebased
}

#[test]
fn the_last_clock_word_does_not_reach_the_reset_words() {
    // 0x70 is the last clock word and 0x74 is the assert register. They abut, which means an
    // off-by-one in `clocks` would write an enable bit into a reset word: a stuck-on clock
    // would be the *good* outcome, and resetting the STG matrix mid-boot the bad one.
    let last = STG.clock_offset(STG.clocks - 1).unwrap();
    assert!(last + 4 <= STG.reset_assert);
}

#[test]
fn resets_are_thirty_two_to_a_word_and_bounded() {
    let r = STG.reset_bit(STGRST_SEC_AHB).unwrap();
    assert_eq!(r.assert_offset, 0x74);
    assert_eq!(r.status_offset, 0x78);
    assert_eq!(r.mask, 1 << 3);
    assert_eq!(STG.reset_bit(22).unwrap().mask, 1 << 22);
    assert_eq!(STG.reset_bit(23), None);
    assert_eq!(STG.reset_bit(131), None); // the vendor number, un-rebased
}

#[test]
fn a_set_status_bit_means_out_of_reset() {
    let mask = STG.reset_bit(STGRST_SEC_AHB).unwrap().mask;
    assert!(is_deasserted(u32::MAX, mask));
    assert!(!is_deasserted(0, mask));
    assert!(
        !is_deasserted(!mask, mask),
        "every other bit must not count"
    );
}

#[test]
fn an_enabled_clock_reads_its_top_bit_back() {
    assert!(is_clock_enabled(CLOCK_ENABLE));
    assert!(is_clock_enabled(CLOCK_ENABLE | 0x1234));
    assert!(!is_clock_enabled(0x7fff_ffff));
    // Zero is what radon's TRNG window read on 2026-09-04, and it is what a gated clock
    // reads: the same value a window with nothing behind it reads, which is why the tour
    // reports `from_tree` and the before/after words rather than a verdict.
    assert!(!is_clock_enabled(0));
}

#[test]
fn reg_names_index_by_name_not_by_position() {
    assert_eq!(name_index(b"sys\0stg\0aon\0", b"stg"), Some(1));
    assert_eq!(
        name_index(b"syscrg\0stgcrg\0aoncrg\0ispcrg\0voutcrg\0", b"stgcrg"),
        Some(1)
    );
    assert_eq!(name_index(b"sys\0aon\0", b"stg"), None);
    // A trailing NUL must not invent an empty fourth entry.
    assert_eq!(name_index(b"sys\0stg\0aon\0", b"aon"), Some(2));
    assert_eq!(name_index(b"", b"stg"), None);
}

#[test]
fn radons_aon_window_is_the_vendor_nodes_third_entry_found_by_name() {
    let tree = device_tree_blob::DeviceTreeBlob::from_bytes(CLKGEN_VENDOR).unwrap();
    let found = discover_aon(&tree).unwrap();
    assert_eq!((found.base, found.size), (AON_BASE, AON_SIZE));
    assert!(found.from_tree);
    assert_eq!(found.compatible, Some(COMPATIBLE_VENDOR_CLKGEN));
}

#[test]
fn a_tree_naming_no_aon_controller_gets_the_constant_and_says_so() {
    let tree = device_tree_blob::DeviceTreeBlob::from_bytes(STGCRG_MAINLINE).unwrap();
    let found = discover_aon(&tree).unwrap();
    assert_eq!(found.base, AON_BASE);
    assert!(!found.from_tree);
}

#[test]
fn gmac0s_plans_ungate_every_clock_before_either_reset_and_pick_the_parent_while_gated() {
    let first_reset = GMAC0_AON_BRING_UP
        .iter()
        .position(|s| matches!(s, Step::DeassertReset(_)))
        .unwrap();
    assert!(
        GMAC0_AON_BRING_UP[first_reset..]
            .iter()
            .all(|s| matches!(s, Step::DeassertReset(_)))
    );
    assert!(
        GMAC0_SYS_BRING_UP
            .iter()
            .all(|s| matches!(s, Step::EnableClock(_)))
    );
    let mux = GMAC0_AON_BRING_UP
        .iter()
        .position(|s| matches!(s, Step::SelectParent { .. }))
        .unwrap();
    let enable = GMAC0_AON_BRING_UP
        .iter()
        .position(|s| *s == Step::EnableClock(AONCLK_GMAC0_TX))
        .unwrap();
    assert!(mux < enable);
    // Every identifier is inside its domain, so neither plan can be rejected.
    for s in GMAC0_AON_BRING_UP {
        match *s {
            Step::EnableClock(i) | Step::SelectParent { clock: i, .. } => {
                assert!(AON.clock_offset(i).is_some());
            }
            Step::DeassertReset(i) => assert!(AON.reset_bit(i).is_some()),
        }
    }
    for s in GMAC0_SYS_BRING_UP {
        if let Step::EnableClock(i) = *s {
            assert!(SYS.clock_offset(i).is_some());
        }
    }
}

#[test]
fn the_word_offsets_match_the_vendor_u_boots_own_constants() {
    // radon's U-Boot writes `AON_CRG_BASE + 0x14` for gmac0's transmit clock and `0x1c` for
    // its receive clock (`jh7110-regs.h`): indices 5 and 7, one word each.
    assert_eq!(AON.clock_offset(AONCLK_GMAC0_TX), Some(0x14));
    assert_eq!(AON.clock_offset(7), Some(0x1c));
    assert_eq!(AON.reset_bit(AONRST_GMAC0_AHB).unwrap().mask, 1 << 1);
    assert_eq!(SYS.clock_offset(SYSCLK_GMAC0_GTXC), Some(0x1bc));
}

#[test]
fn selecting_a_parent_keeps_the_enable_bit_and_the_divider() {
    let running = CLOCK_ENABLE | 0x0000_0008;
    assert_eq!(with_parent(running, 1), CLOCK_ENABLE | 1 << 24 | 8);
    assert_eq!(with_parent(running | 1 << 24, 0), running);
    // A parent too wide for the four-bit field cannot reach the enable bit.
    assert_eq!(with_parent(0, 0xff) & CLOCK_ENABLE, 0);
}

// ===== Option B: the rate chain, the timing formula, and the two tree facts (milestone 592) =====

#[test]
fn radons_pmic_bus_also_names_the_controller_window_and_the_pmic_address() {
    let tree = device_tree_blob::DeviceTreeBlob::from_bytes(PMIC_BUS_RADON).unwrap();
    let bus = pmic_bus(&tree).unwrap();
    assert_eq!(bus.controller, Some((0x1205_0000, 0x1_0000)));
    assert_eq!(bus.pmic_address, Some(0x36));
}

#[test]
fn the_mainline_tree_names_the_same_controller_and_address() {
    let tree = device_tree_blob::DeviceTreeBlob::from_bytes(PMIC_BUS_MAINLINE).unwrap();
    let bus = pmic_bus(&tree).unwrap();
    assert_eq!(bus.controller, Some((0x1205_0000, 0x1_0000)));
    assert_eq!(bus.pmic_address, Some(0x36));
}

#[test]
fn the_foreign_tree_names_neither() {
    let tree = device_tree_blob::DeviceTreeBlob::from_bytes(PMIC_BUS_FOREIGN).unwrap();
    let bus = pmic_bus(&tree).unwrap();
    assert_eq!(bus.controller, None);
    assert_eq!(bus.pmic_address, None);
}

#[test]
fn a_one_based_divider_field_reading_zero_divides_by_one() {
    assert_eq!(one_based_div(0, 2), 1);
    assert_eq!(one_based_div(0, 4), 1);
    assert_eq!(one_based_div(3, 2), 3);
    assert_eq!(one_based_div(0x1f, 4), 15);
    assert_eq!(one_based_div(u32::MAX, 31), (1u64 << 31) - 1);
}

#[test]
fn the_osc_root_chain_divides_and_the_pll2_chain_multiplies() {
    // bus_root on osc, every divider at 1: the chain is transparent.
    let osc = IcClkWords {
        bus_root: 0,
        axi_cfg0: 1,
        stg_axiahb: 1,
        apb_bus_func: 1,
        pll2_dacpd_dsmpd_fbdiv: 0,
        pll2_postdiv1: 0,
        pll2_prediv: 0,
    };
    assert_eq!(i2c5_ic_clk(&osc), OSC_HZ);

    // bus_root on pll2 in integer mode. The fields are the vendor PLL2 table's 1228.8 MHz row
    // (fbdiv 768, prediv 15, postdiv1 1, dacpd 1, dsmpd 1, from the pll.c this crate cites),
    // laid into the words at the syscon's offsets: 0x2c carries dsmpd<<16 | dacpd<<15 |
    // fbdiv<<17, 0x30 carries postdiv1<<28, 0x34 carries prediv. The dividers are field
    // values, not words: the 2-bit fields hold 2 and 2, the 4-bit field 12, so the chain is
    // 1_228_800_000 / 2 / 2 / 12.
    let pll2 = IcClkWords {
        bus_root: 1 << 24,
        axi_cfg0: 2,
        stg_axiahb: 2,
        apb_bus_func: 12,
        pll2_dacpd_dsmpd_fbdiv: (1 << 15) | (1 << 16) | (768 << 17),
        pll2_postdiv1: 0,
        pll2_prediv: 15,
    };
    assert_eq!(i2c5_ic_clk(&pll2), 1_228_800_000 / 2 / 2 / 12);

    // A zero divider field is treated as divide-by-one rather than a panic, because the words
    // belong to firmware and a print of the computed rate is the transcript's fact.
    let zeros = IcClkWords {
        bus_root: 0,
        axi_cfg0: 0,
        stg_axiahb: 0,
        apb_bus_func: 0,
        pll2_dacpd_dsmpd_fbdiv: 0,
        pll2_postdiv1: 0,
        pll2_prediv: 0,
    };
    assert_eq!(i2c5_ic_clk(&zeros), OSC_HZ);

    // Fractional mode (dacpd 0, dsmpd 0) is the vendor's default rate, not a guess from the
    // fields: 1_188_000_000 divided by the chain.
    let frac = IcClkWords {
        bus_root: 1 << 24,
        axi_cfg0: 1,
        stg_axiahb: 1,
        apb_bus_func: 1,
        pll2_dacpd_dsmpd_fbdiv: 768 << 17,
        pll2_postdiv1: 0,
        pll2_prediv: 15,
    };
    assert_eq!(i2c5_ic_clk(&frac), PLL2_DEFAULT_HZ);
}

#[test]
fn the_standard_mode_timing_is_the_vendor_formula_worked_by_hand() {
    // 50 MHz in, the formula's own numbers: rise = ceil(50e6 * 1000ns / 1e9) = 50 counts,
    // fall = 15, thigh = 200, tlow = 235, period = 500. Then
    // hcnt = 200 - 15 - 7 = 178, lcnt = 235 - 50 + 15 - 1 = 199, tot = 178 + 199 + 7 + 50 + 1
    // = 435 < 500, so diff = 32 and lcnt takes the remainder 1: (210, 232). sda_hold is
    // ceil(300ns / 20ns) = 15.
    let t = standard_mode_100k(50_000_000);
    assert_eq!((t.hcnt, t.lcnt, t.sda_hold), (210, 232, 15));
    assert_eq!(t.con, 0b0110_0011);
    // Every count legal and the mode word exactly the vendor's four init bits.
    assert!(t.hcnt > 0 && t.lcnt > 0);
}

#[test]
fn a_slow_input_clock_still_produces_legal_counts() {
    // 1 MHz: every minimum-time count is 1, the period is 10, and the formula must not wrap.
    let t = standard_mode_100k(1_000_000);
    assert!(t.hcnt > 0 && t.lcnt > 0 && t.sda_hold > 0);
    // Below 100 kHz of input the period clamps to 1 rather than dividing by zero.
    let t = standard_mode_100k(1_000);
    assert!(t.hcnt > 0 && t.lcnt > 0);
}

#[test]
fn reg_cells_parse_in_both_spellings() {
    // The vendor and mainline bus nodes: <0x0 0x12050000 0x0 0x10000>.
    let four = 0x0000_0000u32
        .to_be_bytes()
        .iter()
        .chain(0x1205_0000u32.to_be_bytes().iter())
        .chain(0x0000_0000u32.to_be_bytes().iter())
        .chain(0x0001_0000u32.to_be_bytes().iter())
        .copied()
        .collect::<Vec<u8>>();
    assert_eq!(parse_reg(&four), Some((0x1205_0000, 0x1_0000)));
    // A one-address-cell spelling: <0x12050000 0x10000>.
    let two = 0x1205_0000u32
        .to_be_bytes()
        .iter()
        .chain(0x0001_0000u32.to_be_bytes().iter())
        .copied()
        .collect::<Vec<u8>>();
    assert_eq!(parse_reg(&two), Some((0x1205_0000, 0x1_0000)));
    // A truncated property is not a window.
    assert_eq!(parse_reg(&[0, 0, 0]), None);
    // Four cells carry a high word in both the address and the size, and it lands above bit 31
    // rather than being dropped: <0x1 0x12050000 0x2 0x10000>.
    let high = [1u32, 0x1205_0000, 2, 0x1_0000]
        .iter()
        .flat_map(|c| c.to_be_bytes())
        .collect::<Vec<u8>>();
    assert_eq!(parse_reg(&high), Some((0x1_1205_0000, 0x2_0001_0000)));
}

/// **The PLL2 chain's guards and its post-divider**, each on its own. The vendor table's rows all
/// have postdiv1 at 0 (divide by one), so the field is exercised here with a 1 (divide by two);
/// and integer mode needs both of dacpd and dsmpd and a nonzero fbdiv, not any one of them.
#[test]
fn the_pll2_chain_needs_both_mode_bits_and_honors_its_post_divider() {
    let integer = IcClkWords {
        bus_root: 1 << 24,
        axi_cfg0: 1,
        stg_axiahb: 1,
        apb_bus_func: 1,
        pll2_dacpd_dsmpd_fbdiv: (1 << 15) | (1 << 16) | (768 << 17),
        pll2_postdiv1: 1 << 28,
        pll2_prediv: 15,
    };
    assert_eq!(i2c5_ic_clk(&integer), 1_228_800_000 / 2);
    // dacpd alone is fractional mode, which is the vendor's default rate.
    let dacpd_only = IcClkWords {
        pll2_dacpd_dsmpd_fbdiv: (1 << 15) | (768 << 17),
        ..integer
    };
    assert_eq!(i2c5_ic_clk(&dacpd_only), PLL2_DEFAULT_HZ);
    // Integer mode with fbdiv 0 would be a zero rate; it is the default instead.
    let no_fbdiv = IcClkWords {
        pll2_dacpd_dsmpd_fbdiv: (1 << 15) | (1 << 16),
        ..integer
    };
    assert_eq!(i2c5_ic_clk(&no_fbdiv), PLL2_DEFAULT_HZ);
}

/// `every_reset_released` compares the two counts, so a plan with one reset that read back still
/// asserted is not released, and a plan with no reset step is (vacuously) released.
#[test]
fn every_reset_released_counts_the_released_against_the_planned() {
    assert!(Report::default().every_reset_released());
    let one_of_two = Report {
        resets: 2,
        resets_released: 1,
        ..Report::default()
    };
    assert!(!one_of_two.every_reset_released());
    let both = Report {
        resets: 2,
        resets_released: 2,
        ..Report::default()
    };
    assert!(both.every_reset_released());
}

// The SD/MMC bring-up plans (milestone 53 (the board's own peripherals: network and storage on
// real silicon)), which sat in their own test module beside `SDIO_BRING_UP` until the tests moved
// out of lib.rs.

#[test]
fn each_sdio_plan_ungates_both_clocks_before_its_reset_and_stays_in_the_sys_domain() {
    for plan in SDIO_BRING_UP {
        assert!(matches!(plan.last(), Some(Step::DeassertReset(_))));
        assert_eq!(plan.len(), 3);
        for s in plan {
            if let Step::EnableClock(i) = *s {
                assert!(SYS.clock_offset(i).is_some());
            } else if let Step::DeassertReset(i) = *s {
                assert!(SYS.reset_bit(i).is_some());
            }
        }
    }
    assert_eq!(SYS.clock_offset(SYSCLK_SDIO1_SDCARD), Some(0x178));
    let r = SYS.reset_bit(SYSRST_SDIO1_AHB).unwrap();
    assert_eq!(
        (r.assert_offset, r.status_offset, r.mask),
        (0x300, 0x310, 1 << 1)
    );
    assert_eq!(clock_divider(CLOCK_ENABLE | 8), 8);
}

/// radon's 2026-10-21 words (PLL2 `fbdiv 99 prediv 2 postdiv1 1`, `axi_cfg0` divider 3): 396 MHz
/// into the card clock, so the tree's 50 MHz needs a divider of 8 (49.5 MHz), where the firmware's
/// 2 gives 198 MHz. The storage bench's measured transfer rates matched 198, not 50.
#[test]
fn radon_card_clock_comes_to_the_trees_rate_from_below() {
    let words = IcClkWords {
        bus_root: 1 << 24,
        axi_cfg0: 3,
        stg_axiahb: 2,
        apb_bus_func: 4,
        pll2_dacpd_dsmpd_fbdiv: (1 << 15) | (1 << 16) | (99 << 17),
        pll2_postdiv1: 0,
        pll2_prediv: 2,
    };
    let parent = axi_cfg0_hz(&words);
    assert_eq!(parent, 396_000_000);
    assert_eq!(i2c5_ic_clk(&words), 49_500_000);
    let div = sdcard_divider_for(parent, 50_000_000);
    assert_eq!(div, 8);
    assert!(parent / u64::from(div) <= 50_000_000);
    assert_eq!(sdcard_divider_for(parent, 1), SDCARD_DIVIDER_MAX);
    assert_eq!(sdcard_divider_for(parent, 1_000_000_000), 1);
    assert_eq!(with_divider(0x8000_0002, 8), 0x8000_0008);
}
