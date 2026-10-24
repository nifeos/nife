//! **The gate `rollback` is the mirror of**: an upgrade that comes up is confirmed by the running
//! system, and the machine keeps it. A rollback mechanism alone reverts every good upgrade a few
//! boots later, so one nobody has watched *not* roll back is as unproven as one nobody has watched
//! roll back.

//! # The boots
//!
//! | | |
//! |---|---|
//! | 1 | the stick installs itself, exactly as `cargo xtask install-boot`'s first boot does |
//! | | *the host writes an upgrade into slot 1 and marks it on trial, above slot 0, with one try* |
//! | 2 | the machine picks slot 1, spends its one try, **comes up**, and says so on the disk |
//! | 3 | the machine picks slot 1 again, on a slot with no tries left |
//!
//! The upgrade is a copy of the working image, staged as `rollback` stages it.

//! # What it asserts
//!
//! Boot 3 is the whole assertion: `boot_slot::State::is_bootable` refuses `tries = 0` unless the
//! successful bit is set, so choosing slot 1 means reading a bit boot 2 wrote, with only the disk
//! between two QEMU processes. One try, so priority alone cannot pass a machine whose confirmation
//! did nothing. Boot 2 must choose slot 1 and be told so (`boot_slot::cmdline`), reach a shell, read
//! the file `mkfs` wrote (`install_service::confirm`'s criterion) and write `slot 1 confirmed`; boot
//! 3 must choose slot 1 without mentioning slot 0 and, the write being idempotent, write nothing.
//! The disk must then show slot 1 at `tries = 0, successful = 1`, priority untouched, slot 0 as
//! installed.

//! # BUGS
//!
//! - The upgrade is staged by this gate, not by nife; the format goes through the installer's
//!   crates, and the program that will stage upgrades does not exist yet.
//! - No power cut during the confirming write; nothing here can cut QEMU's power mid-write.
//! - The two disk clients cannot collide on the boot path only by ordering
//!   (`install_service::confirm` names it a foot gun); a pass shows it held, not that it is enforced.
//! - OVMF is one, generous firmware. x86_64 only, as the chooser is. Several minutes under TCG,
//!   so it is not in `script/test`'s default legs.

use boot_slot::State;
use globally_unique_identifier_partition_table::GloballyUniqueIdentifierPartitionTable as Table;

use crate::install;
use crate::rollback::{
    LBA, PRIMARY_BYTES, TRIAL_PRIORITY, read_table, slot_states, stage_an_upgrade,
};

/// What boot 2's transcript must contain: the chooser's two lines, the confirmation, and a shell
/// that read a file off the installed disk.
const BOOT_TWO_WANTS: [&str; 5] = [
    "uefi_loader: starting boot slot 1",
    "uefi_loader: started from boot slot 1",
    "slot 1 confirmed",
    "nife capability shell.",
    "1 10 57",
];

/// And boot 3's: the same slot chosen with no tries left, and no second write to the table.
const BOOT_THREE_WANTS: [&str; 3] = [
    "uefi_loader: starting boot slot 1",
    "already confirmed; nothing written",
    "1 10 57",
];

/// What a shell is asked to do on a boot that is allowed to come all the way up, and the marker
/// each answer is keyed on. `rollback`'s boot 3 does the same thing for the same reason: reaching a
/// prompt is the claim, and reading a file `mkfs` wrote is what makes it one.
const AT_THE_PROMPT: [(&str, &str); 3] = [
    install::LS_AT_THE_PROMPT[0],
    install::LS_AT_THE_PROMPT[1],
    ("made-on-target", "wc made-on-target\r"),
];

/// **The gate.** `true` only if a good upgrade was confirmed and then chosen again with no tries
/// left.
pub(crate) fn confirm_boot() -> bool {
    if !install::prepare() {
        return false;
    }
    if install::install_once().is_none() {
        return false;
    }

    let disk = install::install_disk_path();
    let boot_file = install::install_esp_dir().join("EFI/BOOT/BOOTX64.EFI");
    if !stage_an_upgrade(&disk, &boot_file) {
        return false;
    }

    eprintln!();
    eprintln!("--- boot 2 of 3: the machine tries the upgrade, and the upgrade comes up ---");
    let Some(trial) = install::boot(
        &install::empty_esp_dir(),
        true,
        &AT_THE_PROMPT,
        "1 10 57",
        300,
    ) else {
        eprintln!("confirm-boot: boot 2 never reached a prompt, so nothing was tested");
        return false;
    };

    let mut ok = true;
    for wanted in BOOT_TWO_WANTS {
        if !trial.contains(wanted) {
            eprintln!("confirm-boot: boot 2's transcript is missing {wanted:?}");
            ok = false;
        }
    }
    // **A confirmation is not a wish.** The one refusal worth naming on its own, because its
    // symptom without this line is a passing gate on a machine that decided the slot number it was
    // handed named nothing.
    if trial.contains("NOT confirmed") {
        eprintln!("confirm-boot: boot 2 refused to confirm the slot it booted from");
        ok = false;
    }

    eprintln!();
    eprintln!("--- boot 3 of 3: the upgrade has no tries left, and is chosen anyway ---");
    let Some(kept) = install::boot(
        &install::empty_esp_dir(),
        true,
        &AT_THE_PROMPT,
        "1 10 57",
        300,
    ) else {
        return false;
    };
    for wanted in BOOT_THREE_WANTS {
        if !kept.contains(wanted) {
            eprintln!("confirm-boot: boot 3's transcript is missing {wanted:?}");
            ok = false;
        }
    }
    // **The whole promise in one assertion**, and it is the exact negative of `rollback`'s. A
    // machine that fell back to slot 0 here read a slot with no tries left as unbootable, which is
    // what it would have done if nothing had written the bit.
    if kept.contains("uefi_loader: starting boot slot 0") {
        eprintln!("confirm-boot: boot 3 fell back to slot 0; the confirmation did not stick");
        ok = false;
    }

    ok &= the_disk_agrees(&disk);
    if ok {
        eprintln!("confirm-boot: PASS");
    }
    ok
}

/// **Read the bits back off the disk and say what they are**, which is the assertion a transcript
/// cannot make: the console says what the machine decided, and this says what it wrote down.
fn the_disk_agrees(disk: &std::path::Path) -> bool {
    let mut head = [0u8; PRIMARY_BYTES];
    if !read_table(disk, &mut head) {
        return false;
    }
    let Ok(table) = Table::parse(
        &head[LBA as usize..2 * LBA as usize],
        &head[2 * LBA as usize..],
    ) else {
        // The crash-consistency case showing up: a table this gate wrote and the machine rewrote
        // that no longer parses. Worth its own sentence rather than a generic failure, because the
        // confirming write is the second thing in this tree that rewrites a live partition table.
        eprintln!("confirm-boot: the disk's table no longer parses after the machine rewrote it");
        return false;
    };

    let states = slot_states(&table);
    let [zero, one] = states[..] else {
        eprintln!("confirm-boot: the disk no longer has two boot slots");
        return false;
    };
    eprintln!(
        "confirm-boot: slot 1 is priority {}, {} tries, {}successful",
        one.priority,
        one.tries,
        if one.successful { "" } else { "not " }
    );

    let mut ok = true;
    // Confirmed, with the try still spent and the priority still saying what was chosen and why.
    // `State::confirmed` zeroes tries rather than restoring them, so this is the same value the
    // rollback gate asserts with one bit different, which is the point.
    if one != State::on_trial(TRIAL_PRIORITY, 0).confirmed() {
        eprintln!("confirm-boot: slot 1 should be priority {TRIAL_PRIORITY}, 0 tries, successful");
        ok = false;
    }
    // And the image the machine no longer needs was not touched on the way past. A confirmation
    // that demoted the previous slot would leave a machine with nothing to fall back to.
    if zero != State::installed() {
        eprintln!("confirm-boot: slot 0 is no longer the state the install left it in");
        ok = false;
    }
    ok
}
