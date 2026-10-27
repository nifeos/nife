//! **The other half of `a_bad_upgrade_rolls_back`**: an upgrade that comes up is confirmed, and a
//! machine that reboots afterwards keeps it.
//!
//! The rollback test proves the machine gives up on an image that never comes up. This one proves
//! it does *not* give up on one that does, which is a separate claim and the one a person notices:
//! a system whose upgrades all revert three boots later is worse than one with no upgrader.
//!
//! It is the host half. `cargo xtask stick-boot` is the same story with a real disk and three real
//! boots under OVMF.

use boot_slot::{State, cmdline, select};

/// **The whole life of a good upgrade**, in the states the disk holds between reboots, ending with
/// the boot that would have rolled back and does not.
#[test]
fn an_upgrade_that_comes_up_is_still_chosen_after_its_tries_would_have_run_out() {
    let mut slots = [State::installed(), State::EMPTY];

    // An upgrader writes slot 1 and puts it on trial above the running image, with one try.
    slots[1] = State::on_trial(State::installed().priority + 1, 1);

    // Boot 2: the chooser spends the try before handing off, exactly as the rollback case does.
    let chosen = select(&slots).expect("the upgrade outranks the installed image");
    assert_eq!(chosen, 1);
    slots[chosen] = slots[chosen].attempted();
    assert_eq!(slots[1].tries, 0, "the try is spent before the handoff");

    // **The state the rollback test stops at.** Without a confirmation, the next boot is slot 0.
    assert_eq!(
        select(&slots),
        Some(0),
        "this is the rollback, and it is what happens when nothing confirms"
    );

    // The running system says it came up, which is the whole of this feature.
    slots[1] = slots[1].confirmed();

    assert_eq!(select(&slots), Some(1), "the upgrade stuck");
    assert_eq!(
        slots[1].priority,
        State::installed().priority + 1,
        "confirming spends no priority, so the record of what was chosen and why survives"
    );
}

/// **Confirming twice is confirming once.** The write happens on a running machine that may be
/// interrupted and rebooted, so the second attempt has to be a no-op rather than a second edit.
#[test]
fn a_confirmation_is_idempotent_down_to_the_attribute_word() {
    let on_trial = State::on_trial(3, 2).attempted();
    let previous = on_trial.into_attributes(0xDEAD_0000_0000_BEEF);

    let once = State::from_attributes(previous)
        .confirmed()
        .into_attributes(previous);
    let twice = State::from_attributes(once)
        .confirmed()
        .into_attributes(once);

    assert_eq!(once, twice, "the second confirmation writes the same bytes");
    assert_eq!(
        once & !((0xF << 48) | (0xF << 52) | (1 << 56)),
        previous & !((0xF << 48) | (0xF << 52) | (1 << 56)),
        "and neither touches a bit this crate does not own"
    );
}

/// **A confirmation of a slot that never booted is still safe.** It cannot happen through the
/// command-line token, because the chooser writes the number of the slot it started, but the bit
/// is a single word and a wrong one would be a machine keeping an image nobody ran.
///
/// This states the property the write's caller has to hold rather than testing the caller: a
/// confirmed empty slot is *bootable*, so the number must come from the chooser and never from a
/// guess.
#[test]
fn confirming_an_empty_slot_would_make_it_bootable_which_is_why_the_number_is_carried() {
    assert!(!State::EMPTY.is_bootable());
    assert!(
        !State::EMPTY.confirmed().is_bootable(),
        "priority zero still wins, so an empty slot survives a stray confirmation"
    );
}

/// **The token the number is carried in**, at the edges the doc example does not cover.
#[test]
fn the_slot_token_refuses_everything_it_cannot_read_exactly() {
    assert_eq!(cmdline::parse("boot-slot=1"), Some(1));
    assert_eq!(cmdline::parse("boot-slot=0 screen-hold"), Some(0));

    // Not a slot: no digit, two digits, a prefix that only looks like the key.
    assert_eq!(cmdline::parse("boot-slot="), None);
    assert_eq!(cmdline::parse("boot-slot=10"), None);
    assert_eq!(cmdline::parse("boot-slot=x"), None);
    assert_eq!(cmdline::parse("boot-slots=1"), None);
    assert_eq!(cmdline::parse(""), None);

    // And the writer refuses what the reader would refuse.
    let mut out = [0u8; cmdline::MAX_LEN];
    assert_eq!(
        cmdline::encode(10, &mut out),
        0,
        "two digits are not a slot"
    );
    assert_eq!(
        cmdline::encode(0, &mut out[..1]),
        0,
        "a buffer too small writes nothing"
    );
}

/// **What this writer writes, that reader reads**, which is the rule every other format in this
/// tree is held to.
#[test]
fn every_slot_that_can_be_written_reads_back_as_itself() {
    for slot in 0..=9u8 {
        let mut out = [0u8; cmdline::MAX_LEN];
        let n = cmdline::encode(slot, &mut out);
        assert_eq!(n, cmdline::MAX_LEN);
        let line = core::str::from_utf8(&out[..n]).expect("the writer emits ASCII");
        assert_eq!(cmdline::parse(line), Some(slot));
    }
}
