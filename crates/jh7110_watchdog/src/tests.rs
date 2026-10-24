//! The crate's host tests.
//!
//! Name: provisional, minted 2026-10-20 (UTC) by milestone/593-radon-watchdog.

use jh7110_clock_and_reset::Step;

use super::*;

const RADON: &[u8] = include_bytes!("../tests/fixtures/jh7110-watchdog-radon.dtb");
const MAINLINE: &[u8] = include_bytes!("../tests/fixtures/jh7110-watchdog-mainline.dtb");
const QEMU_RISCV64_VIRT: &[u8] =
    include_bytes!("../../device_tree_blob/tests/fixtures/qemu-riscv64-virt.dtb");

/// The binding radon's firmware uses is not mainline's, and its clocks and resets come from two
/// different provider nodes. Missing either would leave the watchdog unfound or gated.
#[test]
fn radons_vendor_node_gives_the_window_and_all_four_lines_through_clkgen_and_rstgen() {
    let tree = device_tree_blob::DeviceTreeBlob::from_bytes(RADON).unwrap();
    let found = discover(&tree).unwrap().unwrap();
    assert_eq!((found.base, found.size), (0x1307_0000, 0x1_0000));
    assert_eq!(found.compatible, COMPATIBLE_VENDOR);
    assert_eq!(
        found.clocks_and_resets.plan(),
        &[
            Step::EnableClock(123),
            Step::EnableClock(122),
            Step::DeassertReset(109),
            Step::DeassertReset(110),
        ]
    );
    assert_eq!(found.clocks_and_resets.skipped, 0);
    assert!(!found.clocks_and_resets.truncated);
}

#[test]
fn mainlines_node_names_the_same_lines_through_syscrg() {
    let tree = device_tree_blob::DeviceTreeBlob::from_bytes(MAINLINE).unwrap();
    let found = discover(&tree).unwrap().unwrap();
    assert_eq!(found.base, 0x1307_0000);
    assert_eq!(found.compatible, COMPATIBLE_MAINLINE);
    assert_eq!(
        found.clocks_and_resets.plan(),
        &[
            Step::EnableClock(122),
            Step::EnableClock(123),
            Step::DeassertReset(109),
            Step::DeassertReset(110),
        ]
    );
}

/// No constant fallback: the soak must say NOT ARMED here rather than store to 0x1307_0000.
#[test]
fn qemus_virt_board_has_no_watchdog_and_nothing_is_invented() {
    let tree = device_tree_blob::DeviceTreeBlob::from_bytes(QEMU_RISCV64_VIRT).unwrap();
    assert_eq!(discover(&tree).unwrap(), None);
}

/// A write before the unlock is silently dropped by the block, and a sequence that ends
/// unlocked leaves the timer open to any stray store.
#[test]
fn every_sequence_unlocks_first_and_locks_last() {
    for seq in [
        &arm_sequence(7)[..],
        &pet_sequence(7)[..],
        &disarm_sequence()[..],
    ] {
        assert_eq!(seq[0], UNLOCK);
        assert_eq!(seq[seq.len() - 1], RELOCK);
        assert!(
            !seq[1..seq.len() - 1]
                .iter()
                .any(|a| matches!(a, Access::Write { offset: LOCK, .. }))
        );
    }
}

#[test]
fn arming_stops_the_timer_before_loading_and_starts_it_last_with_reset_already_on() {
    let seq = arm_sequence(0x1234);
    let at = |want: Access| seq.iter().position(|&a| a == want).unwrap();
    let stop = at(Access::Clear {
        offset: CONTROL,
        mask: CONTROL_ENABLE,
    });
    let reset_on = at(Access::Set {
        offset: CONTROL,
        mask: CONTROL_RESET_ENABLE,
    });
    let clear = at(CLEAR_INTERRUPT);
    let load = at(Access::Write {
        offset: LOAD,
        value: 0x1234,
    });
    let start = at(Access::Set {
        offset: CONTROL,
        mask: CONTROL_ENABLE,
    });
    // A stale interrupt left set would make the very first zero the resetting one.
    assert!(stop < clear && clear < load && load < start);
    assert!(reset_on < start);
}

#[test]
fn a_pet_clears_the_interrupt_so_a_first_expiry_does_not_carry_into_a_reset() {
    let seq = pet_sequence(9);
    assert_eq!(seq[1], CLEAR_INTERRUPT);
    assert_eq!(
        seq[2],
        Access::Write {
            offset: LOAD,
            value: 9
        }
    );
}

#[test]
fn sixty_seconds_is_two_counts_of_thirty_at_the_oscillator() {
    let load = load_for(60, CORE_CLOCK_HZ).unwrap();
    assert_eq!(load, 720_000_000);
    assert_eq!(reset_after_tenths(load, CORE_CLOCK_HZ), 600);
}

#[test]
fn the_longest_reset_is_about_357_seconds_and_longer_is_refused() {
    assert_eq!(max_reset_after_seconds(CORE_CLOCK_HZ), 357);
    assert!(load_for(357, CORE_CLOCK_HZ).is_some());
    assert_eq!(load_for(358, CORE_CLOCK_HZ), None);
    assert_eq!(load_for(0, CORE_CLOCK_HZ), None);
}

/// The failure the measurement exists for: a gated core clock reads a frozen count, and arming
/// it would print ARMED over a timer that can never fire.
#[test]
fn a_halted_or_reloaded_counter_measures_zero_and_is_refused() {
    assert_eq!(measured_hz(5_000, 5_000, 400_000, 4_000_000), 0);
    assert_eq!(measured_hz(5_000, 6_000, 400_000, 4_000_000), 0);
    assert!(!rate_agrees(0, CORE_CLOCK_HZ));
}

#[test]
fn a_counter_on_the_oscillator_agrees_and_one_on_half_of_it_does_not() {
    // 100 ms of radon's 4 MHz timer.
    let at = |ticks: u32| measured_hz(720_000_000, 720_000_000 - ticks, 400_000, 4_000_000);
    assert_eq!(at(2_400_000), 24_000_000);
    assert!(rate_agrees(at(2_400_000), CORE_CLOCK_HZ));
    assert!(rate_agrees(at(2_350_000), CORE_CLOCK_HZ));
    assert!(!rate_agrees(at(1_200_000), CORE_CLOCK_HZ));
}

/// Comparing a sequence against `RELOCK` cannot catch a `RELOCK` that writes the key itself,
/// which would leave the watchdog unlocked after every sequence.
#[test]
fn relocking_writes_something_other_than_the_key() {
    let Access::Write { offset, value } = disarm_sequence()[2] else {
        panic!("the last access is not a store");
    };
    assert_eq!(offset, LOCK);
    assert_ne!(value, UNLOCK_KEY);
}

/// `STARFIVE_WDT_EN_SHIFT 0` and `STARFIVE_WDT_JH7110_RST_EN_SHIFT 1`. With the reset bit
/// wrong, the arm sequence sets nothing that resets and the wedge is never recovered.
#[test]
fn the_control_bits_are_linuxs_enable_at_bit_0_and_reset_at_bit_1() {
    assert_eq!((CONTROL_ENABLE, CONTROL_RESET_ENABLE), (0b01, 0b10));
}

/// At a 1 Hz clock, `2 * u32::MAX` seconds is exactly a full register.
#[test]
fn the_largest_count_the_register_holds_is_accepted() {
    let full = 2 * u64::from(u32::MAX);
    assert_eq!(load_for(full, 1), Some(u32::MAX));
    assert_eq!(load_for(full + 2, 1), None);
}

/// A counter on a faster clock than the count assumes resets early, before the soak has stalled.
#[test]
fn a_counter_running_fast_is_refused_too() {
    assert!(!rate_agrees(2 * CORE_CLOCK_HZ, CORE_CLOCK_HZ));
    assert!(!rate_agrees(
        CORE_CLOCK_HZ + CORE_CLOCK_HZ / 10,
        CORE_CLOCK_HZ
    ));
}
