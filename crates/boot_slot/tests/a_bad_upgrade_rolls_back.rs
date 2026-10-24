//! **What the rollback promise actually says**, written as the sequences a disk goes through
//! rather than as assertions about single functions: *a machine handed a bad upgrade ends up back
//! on the image it came with, in a bounded number of reboots, with nobody typing anything.* Every
//! test below is a loop over boots with no console in it.
//!
//! # EXAMPLES
//!
//! ```console
//! $ cargo test -p boot_slot
//! ```

use boot_slot::{MAX_NIBBLE, SlotHeader, State, select, select_excluding};

/// One boot, the way the chooser does it: pick, spend a try, hand off. Returns which slot was
/// handed to, or `None` if nothing was bootable and the chooser fell back to its own image.
fn boot(slots: &mut [State]) -> Option<usize> {
    let chosen = select(slots)?;
    slots[chosen] = slots[chosen].attempted();
    Some(chosen)
}

#[test]
fn a_trial_image_that_never_comes_up_is_abandoned_after_its_tries() {
    let mut slots = [State::installed(), State::on_trial(3, 3)];

    // Three boots that hang: each one spends a try before handing off, so each one leaves the disk
    // closer to giving up whether or not the machine ever ran a line of userspace.
    for boot_number in 1..=3 {
        assert_eq!(
            boot(&mut slots),
            Some(1),
            "boot {boot_number} should still be trying the upgrade"
        );
    }

    // The fourth boot is the one the promise is about, and nobody did anything to cause it.
    assert_eq!(boot(&mut slots), Some(0));
    assert_eq!(slots[0], State::installed(), "the good slot is untouched");
}

#[test]
fn a_confirmed_upgrade_stays_chosen_forever() {
    let mut slots = [State::installed(), State::on_trial(3, 3)];
    assert_eq!(boot(&mut slots), Some(1));
    slots[1] = slots[1].confirmed();

    // Far more boots than the trial ever had tries for. A confirmed slot spends nothing.
    let before = slots;
    for _ in 0..100 {
        assert_eq!(boot(&mut slots), Some(1));
    }
    assert_eq!(slots, before, "a good boot writes nothing to the disk");
}

#[test]
fn an_image_the_chooser_cannot_read_costs_one_boot_and_not_three() {
    // The failure the chooser sees for itself: spending reboots to re-observe it would be reboots
    // of a machine somebody is waiting on.
    let mut slots = [State::installed(), State::on_trial(3, 3)];
    assert_eq!(select(&slots), Some(1));
    slots[1] = slots[1].unreadable();
    assert_eq!(select(&slots), Some(0));
}

#[test]
fn the_chooser_moves_on_within_one_boot_rather_than_making_somebody_reboot() {
    let slots = [State::installed(), State::on_trial(3, 3)];
    let first = select_excluding(&slots, 0).unwrap();
    assert_eq!(first, 1);
    assert_eq!(select_excluding(&slots, 1 << first), Some(0));
    assert_eq!(select_excluding(&slots, 0b11), None);
}

#[test]
fn nothing_bootable_is_a_state_the_caller_has_to_answer_for() {
    // Both slots spent. `select` says nothing, and `uefi_loader`'s chooser answers by starting the
    // image in its own file: never-booting is a worse outcome than booting something old.
    let slots = [State::on_trial(1, 0), State::on_trial(2, 0)];
    assert_eq!(select(&slots), None);
    assert_eq!(select(&[State::EMPTY, State::EMPTY]), None);
    assert_eq!(select(&[]), None);
}

#[test]
fn priority_zero_beats_every_other_reason_to_boot_a_slot() {
    // The one value that means "never", even on a slot that booted successfully a hundred times.
    let confirmed_but_disabled = State {
        priority: 0,
        tries: 15,
        successful: true,
    };
    assert!(!confirmed_but_disabled.is_bootable());
    assert_eq!(
        select(&[confirmed_but_disabled, State::installed()]),
        Some(1)
    );
}

#[test]
fn a_tie_on_priority_picks_the_same_slot_twice_running() {
    // Two slots written with the same priority is a mistake, and a mistake that produced a
    // different boot each time would be the worst possible way to meet it.
    let slots = [State::on_trial(3, 3), State::on_trial(3, 3)];
    assert_eq!(select(&slots), Some(0));
    assert_eq!(select(&slots), Some(0));
}

#[test]
fn the_bits_outside_this_crate_survive_a_write() {
    // UEFI 2.11 5.3.3: "They must be preserved if Bits 0-47 are modified", which this reads in
    // both directions. Bit 0 is the platform's do-not-touch flag and bit 60 is reserved.
    let others = (1 << 0) | (1 << 2) | (1 << 60) | (1 << 63);
    let written = State::on_trial(7, 5).into_attributes(others);
    assert_eq!(
        written & 0x0000_FFFF_FFFF_FFFF,
        others & 0x0000_FFFF_FFFF_FFFF
    );
    assert_eq!(written & ((1 << 60) | (1 << 63)), (1 << 60) | (1 << 63));

    let back = State::from_attributes(written);
    assert_eq!(back, State::on_trial(7, 5));
}

#[test]
fn the_bit_positions_are_the_ones_cgpt_prints() {
    // Verified against the ChromiumOS disk-format reference on 2026-09-21, not recalled: priority
    // 51-48, tries 55-52, successful 56. A nife disk is readable by `cgpt show` because of this.
    let attributes = State {
        priority: 1,
        tries: 0,
        successful: true,
    }
    .into_attributes(0);
    assert_eq!(attributes, (1 << 48) | (1 << 56));

    let attributes = State {
        priority: 15,
        tries: 15,
        successful: false,
    }
    .into_attributes(0);
    assert_eq!(attributes, 0x00FF_0000_0000_0000);
}

#[test]
fn a_round_trip_through_the_attribute_word_is_the_identity() {
    for priority in 0..=MAX_NIBBLE {
        for tries in 0..=MAX_NIBBLE {
            for successful in [false, true] {
                let state = State {
                    priority,
                    tries,
                    successful,
                };
                assert_eq!(State::from_attributes(state.into_attributes(0)), state);
                assert_eq!(State::from_attributes(state.into_attributes(!0)), state);
            }
        }
    }
}

#[test]
fn on_trial_saturates_rather_than_wrapping_a_priority_into_never_boot() {
    // 16 wrapped into four bits is 0, and 0 is the one value that means the machine will not start
    // this image. Saturating is the difference between a clumsy upgrade and a dead one.
    assert_eq!(State::on_trial(16, 16).priority, MAX_NIBBLE);
    assert_eq!(State::on_trial(200, 200).tries, MAX_NIBBLE);
    assert!(State::on_trial(16, 1).is_bootable());
}

#[test]
fn a_slot_that_was_never_written_is_not_a_zero_length_image() {
    let blank = [0u8; 4096];
    assert_eq!(SlotHeader::decode(&blank), None);
}

#[test]
fn a_header_round_trips_and_a_corrupted_one_does_not_decode() {
    let image = b"an image, or near enough for a checksum".repeat(37);
    let header = SlotHeader::of(&image);

    let mut block = [0u8; 4096];
    assert!(header.encode(&mut block));
    assert_eq!(SlotHeader::decode(&block), Some(header));
    assert!(header.matches(&image));

    // Every single-byte change to the header is caught by the header's own checksum.
    for byte in 0..boot_slot::SLOT_HEADER_BYTES {
        let mut damaged = block;
        damaged[byte] ^= 0x01;
        assert_eq!(
            SlotHeader::decode(&damaged),
            None,
            "a flipped bit at {byte} decoded anyway"
        );
    }
}

#[test]
fn a_truncated_image_is_refused_by_the_header_that_described_the_whole_one() {
    // The failure this catches is an install or an upgrade that died partway through the copy,
    // which leaves a slot holding a prefix of a valid image.
    let image = b"the whole image".repeat(100);
    let header = SlotHeader::of(&image);
    assert!(!header.matches(&image[..image.len() - 1]));

    let mut flipped = image.clone();
    flipped[500] ^= 0x80;
    assert!(!header.matches(&flipped));
}

#[test]
fn a_header_needs_a_buffer_and_says_so_rather_than_writing_a_short_one() {
    let mut too_small = [0u8; boot_slot::SLOT_HEADER_BYTES - 1];
    assert!(!SlotHeader::of(b"x").encode(&mut too_small));
    assert_eq!(SlotHeader::decode(&too_small), None);
}

/// A confirmed slot is returned untouched however many tries it carries, and an unconfirmed one
/// with none left is too: only a slot that is both unconfirmed and still has tries spends one.
#[test]
fn a_try_is_spent_only_by_an_unconfirmed_slot_that_has_one() {
    let confirmed_with_tries = State {
        priority: 3,
        tries: 5,
        successful: true,
    };
    assert_eq!(confirmed_with_tries.attempted(), confirmed_with_tries);
    let spent = State {
        priority: 3,
        tries: 0,
        successful: false,
    };
    assert_eq!(spent.attempted(), spent);
    assert_eq!(State::on_trial(3, 2).attempted(), State::on_trial(3, 1));
}

/// The header is exactly 32 bytes: a buffer of that size is enough to write one and to read it
/// back, and no shorter prefix of a good header is read, however much of it is right.
#[test]
fn a_slot_header_needs_exactly_its_own_length() {
    let h = SlotHeader::of(b"an image");
    let mut buf = [0u8; 32];
    assert!(h.encode(&mut buf));
    assert!(!h.encode(&mut [0u8; 31]));
    assert_eq!(SlotHeader::decode(&buf), Some(h));
    for cut in 0..32 {
        assert_eq!(SlotHeader::decode(&buf[..cut]), None, "{cut} bytes");
    }
}

/// The medium token is written only into a buffer that holds all of it, and a larger buffer is
/// fine; a buffer one short gets nothing, not a truncated word.
#[test]
fn the_medium_token_is_written_whole_or_not_at_all() {
    use boot_slot::medium;
    let mut big = [0u8; medium::MAX_LEN + 8];
    assert_eq!(medium::encode_nvme(&mut big), medium::MAX_LEN);
    assert_eq!(&big[..medium::MAX_LEN], medium::NVME.as_bytes());
    let mut short = [0u8; medium::MAX_LEN - 1];
    assert_eq!(medium::encode_nvme(&mut short), 0);
    assert_eq!(short, [0u8; medium::MAX_LEN - 1]);
}
