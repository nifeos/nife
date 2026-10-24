//! **The rollback gate**: a deliberately doomed upgrade is installed, tried, and abandoned, and the
//! machine comes back on the image it had before, with nobody typing anything (rung 2b of milestone
//! 198 (a package manager, and the trivial install that makes a second customer possible)).
//! `crates/boot_slot`, `components/src/system_installer.rs` and `uefi_loader/src/chooser.rs` are
//! only as good as the three boots below. See notes/boot-slots.md.
//!
//! **Killing the machine at the handoff is the test.** A kernel that wedges, a driver that spins
//! and a power cut are, to the disk, one event: the try was spent and nothing came back. The
//! upgrade is a **byte-for-byte copy of the working image**, so the header checksum cannot catch it
//! and only the spent try decides.

//! # The three boots
//! | | |
//! |---|---|
//! | 1 | the stick installs itself, exactly as `cargo xtask install-boot`'s first boot does |
//! | | *the host writes an upgrade into slot 1 and marks it on trial, above slot 0* |
//! | 2 | the machine picks slot 1, spends its one try, hands off, **and is killed at that instant** |
//! | 3 | the machine picks slot 0 and comes up, and slot 1 is never tried again |
//!
//! QEMU dying one instruction after `StartImage` is the hang and the power cut alike, and the
//! chooser cannot observe it for itself.

//! # What it asserts
//!
//! - Boot 2 says `starting boot slot 1`: **priority chose the upgrade over the running image**.
//! - Boot 3 says `starting boot slot 0` and never mentions slot 1: **the decrement reached the
//!   disk and survived a power cycle**.
//! - Boot 3 reaches a shell and reads the file `mkfs` wrote: the fallback is a working machine.
//! - The disk, read afterwards by this gate, shows slot 1 at `tries=0, successful=0` with its
//!   **priority untouched**.

//! # EXAMPLES
//!
//! ```console
//! $ cargo xtask rollback-boot
//! --- the stick installs itself onto an empty NVMe disk ---
//!   install     : DONE. Remove the installation medium and reboot.
//! rollback-boot: slot 1 written from target/esp-install/EFI/BOOT/BOOTX64.EFI, on trial
//!                priority 3, 1 try; slot 0 stays at priority 2, confirmed
//! --- boot 2 of 3: the machine tries the upgrade, and the upgrade never comes up ---
//! uefi_loader: boot slots on one disk, 2 of them
//! uefi_loader: starting boot slot 1
//! rollback-boot: killed at the handoff, which is what a hang looks like to the disk
//! --- boot 3 of 3: nobody touched anything ---
//! uefi_loader: starting boot slot 0
//! $ wc made-on-target
//!   1 10 57
//! rollback-boot: slot 1 is priority 3, 0 tries, not successful
//! rollback-boot: PASS
//! ```

//! # BUGS
//!
//! - The upgrade is written by this gate, not by nife, through the installer's own crates.
//! - One try, not three; `cargo test -p boot_slot` covers the counting.
//! - It proves nothing about a real firmware, the same caveat `install-boot` carries.
//! - The power cut is at the handoff, not during the table write (`uefi_loader/src/chooser.rs`).
//! - It takes several minutes under TCG, so it is not in `script/test`'s default legs.

use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

use boot_slot::{SlotHeader, State};
use globally_unique_identifier_partition_table as gpt;
use gpt::guid::types;
use gpt::{Entry, GloballyUniqueIdentifierPartitionTable as Table};

use crate::install;

/// The logical block size the installer writes and this gate reads. Its own `BUGS` has the note.
pub(crate) const LBA: u64 = 512;

/// LBA 0 through 33: the protective MBR, the primary header and the whole entry array.
pub(crate) const PRIMARY_BYTES: usize = 34 * LBA as usize;

/// **The priority the staged upgrade is written at**, above the install's own 2 so that the chooser
/// prefers it. Three rather than fifteen for the reason `boot_slot::INSTALLED_PRIORITY` gives: the
/// range is there to be walked up.
pub(crate) const TRIAL_PRIORITY: u8 = 3;

/// **How many tries the staged upgrade gets.** One, so the gate is three boots rather than five;
/// see `BUGS`.
pub(crate) const TRIAL_TRIES: u8 = 1;

/// **The gate.** `true` only if the machine went back to slot 0 on its own.
pub(crate) fn rollback_boot() -> bool {
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
    eprintln!("--- boot 2 of 3: the machine tries the upgrade, and the upgrade never comes up ---");
    let Some(trial) = install::boot(
        &install::empty_esp_dir(),
        true,
        &[],
        // The instant of the handoff. `boot` kills the machine the moment this appears, which from
        // the disk's point of view is indistinguishable from the image hanging: the try is already
        // spent and nothing ever comes back.
        "uefi_loader: starting boot slot 1",
        300,
    ) else {
        eprintln!("rollback-boot: the machine never chose the upgrade, so nothing was tested");
        return false;
    };
    eprintln!();
    eprintln!("rollback-boot: killed at the handoff, which is what a hang looks like to the disk");

    let mut ok = true;
    if !trial.contains("uefi_loader: boot slots on one disk, 2 of them") {
        eprintln!("rollback-boot: boot 2 did not find the boot slots");
        ok = false;
    }

    eprintln!();
    eprintln!("--- boot 3 of 3: nobody touched anything ---");
    let Some(back) = install::boot(
        &install::empty_esp_dir(),
        true,
        &[
            install::LS_AT_THE_PROMPT[0],
            install::LS_AT_THE_PROMPT[1],
            ("made-on-target", "wc made-on-target\r"),
        ],
        "1 10 57",
        300,
    ) else {
        return false;
    };

    for wanted in [
        // The fallback happened, and it happened by itself.
        "uefi_loader: starting boot slot 0",
        "uefi_loader: started from boot slot 0",
        // And what it fell back to is a working machine rather than a different line of output.
        "nife capability shell.",
        "1 10 57",
    ] {
        if !back.contains(wanted) {
            eprintln!("rollback-boot: boot 3's transcript is missing {wanted:?}");
            ok = false;
        }
    }
    // **The whole promise in one assertion.** A machine that tried the dead upgrade again would
    // have spent a try it did not have, which would mean the decrement never reached the disk.
    if back.contains("uefi_loader: starting boot slot 1") {
        eprintln!("rollback-boot: boot 3 tried the upgrade again; the decrement did not stick");
        ok = false;
    }

    ok &= the_disk_agrees(&disk);
    if ok {
        eprintln!("rollback-boot: PASS");
    }
    ok
}

/// **Write an upgrade into slot 1 and mark it on trial**, which is what an upgrader will one day
/// do on a running machine and what this gate does from the host instead.
///
/// The bytes and the bits both go through the same crates the installer uses, so this is the real
/// format and not a second spelling of it.
pub(crate) fn stage_an_upgrade(disk: &Path, boot_file: &Path) -> bool {
    let image = match std::fs::read(boot_file) {
        Ok(bytes) => bytes,
        Err(e) => {
            eprintln!("rollback-boot: could not read {}: {e}", boot_file.display());
            return false;
        }
    };
    let Ok(mut file) = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(disk)
    else {
        eprintln!("rollback-boot: could not open {}", disk.display());
        return false;
    };

    let mut head = [0u8; PRIMARY_BYTES];
    if file.read_exact(&mut head).is_err() {
        eprintln!("rollback-boot: could not read the partition table");
        return false;
    }

    // Everything that has to survive the rebuild, taken before the borrow: the table is rebuilt
    // from its own decoded entries, so the disk's identity and geometry come from the old one.
    let (disk_guid, block_count, mut parts, slots) = {
        let Some((table, parts, slots)) = installed_table(&head) else {
            return false;
        };
        (table.disk_guid(), table.block_count(), parts, slots)
    };

    let [_, second] = slots[..] else {
        eprintln!(
            "rollback-boot: expected two boot slots on the installed disk, found {}",
            slots.len()
        );
        return false;
    };
    if !write_slot_image(&mut file, parts[second].first_lba, &image) {
        return false;
    }

    // And the state: on trial, above the running image.
    parts[second].attributes =
        State::on_trial(TRIAL_PRIORITY, TRIAL_TRIES).into_attributes(parts[second].attributes);

    let mut array = [0u8; gpt::ENTRY_ARRAY_BYTES];
    let Ok(table) = Table::create(disk_guid, LBA as usize, block_count, &parts, &mut array) else {
        eprintln!("rollback-boot: could not rebuild the partition table");
        return false;
    };
    if !write_table_back(&mut file, &table) {
        return false;
    }

    eprintln!();
    eprintln!(
        "rollback-boot: slot 1 written from {}, on trial",
        boot_file.display()
    );
    eprintln!(
        "               priority {TRIAL_PRIORITY}, {TRIAL_TRIES} try; slot 0 stays at priority \
         {}, confirmed",
        State::installed().priority
    );
    true
}

/// **The installed disk's table, its entries, and which of them are boot slots**, from the head of
/// the disk. `None` after saying why the disk is not one this gate can stage onto.
fn installed_table(head: &[u8; PRIMARY_BYTES]) -> Option<(Table<'_>, Vec<Entry>, Vec<usize>)> {
    let Ok(table) = Table::parse(
        &head[LBA as usize..2 * LBA as usize],
        &head[2 * LBA as usize..],
    ) else {
        eprintln!("rollback-boot: the installed disk has no readable partition table");
        return None;
    };
    let mut parts: Vec<Entry> = Vec::new();
    let mut slots: Vec<usize> = Vec::new();
    for (index, part) in table.partitions() {
        if index != parts.len() {
            eprintln!("rollback-boot: the installed disk's entries are not contiguous");
            return None;
        }
        if part.type_guid == types::NIFE_BOOT {
            slots.push(parts.len());
        }
        parts.push(part);
    }
    Some((table, parts, slots))
}

/// **The image into the slot at `first_lba`, header first.** Written in the order the installer
/// writes it so that a failure here looks like the failure there.
fn write_slot_image(file: &mut std::fs::File, first_lba: u64, image: &[u8]) -> bool {
    let mut header = vec![0u8; boot_slot::SLOT_IMAGE_OFFSET as usize];
    if !SlotHeader::of(image).encode(&mut header) {
        eprintln!("rollback-boot: could not encode the slot header");
        return false;
    }
    let written = file
        .seek(SeekFrom::Start(first_lba * LBA))
        .and_then(|_| file.write_all(&header))
        .and_then(|()| file.write_all(image));
    if let Err(e) = written {
        eprintln!("rollback-boot: could not write slot 1: {e}");
        return false;
    }
    true
}

/// **The rebuilt table back onto the disk**: the primary header, both entry arrays, and the
/// backup header, then a flush.
fn write_table_back(file: &mut std::fs::File, table: &Table) -> bool {
    let mut block = [0u8; LBA as usize];
    if table.write_primary_header(&mut block).is_err() {
        eprintln!("rollback-boot: could not build the primary header");
        return false;
    }
    let mut put = |at: u64, bytes: &[u8]| -> bool {
        file.seek(SeekFrom::Start(at * LBA))
            .and_then(|_| file.write_all(bytes))
            .is_ok()
    };
    if !put(gpt::PRIMARY_HEADER_LBA, &block)
        || !put(table.primary_entry_lba(), table.entry_array())
        || !put(table.backup_entry_lba(), table.entry_array())
    {
        eprintln!("rollback-boot: could not write the partition table back");
        return false;
    }
    let mut backup = [0u8; LBA as usize];
    if table.write_backup_header(&mut backup).is_err()
        || !put(table.backup_header_lba(), &backup)
        || file.flush().is_err()
    {
        eprintln!("rollback-boot: could not write the backup header");
        return false;
    }
    true
}

/// **Read the bits back off the disk and say what they are**, which is the assertion a transcript
/// cannot make: the console says what the machine decided, and this says what it wrote down.
fn the_disk_agrees(disk: &Path) -> bool {
    let Ok(mut file) = std::fs::File::open(disk) else {
        eprintln!("rollback-boot: could not reopen {}", disk.display());
        return false;
    };
    let mut head = [0u8; PRIMARY_BYTES];
    if file.read_exact(&mut head).is_err() {
        eprintln!("rollback-boot: could not read the partition table back");
        return false;
    }
    let Ok(table) = Table::parse(
        &head[LBA as usize..2 * LBA as usize],
        &head[2 * LBA as usize..],
    ) else {
        // Worth naming rather than folding into a generic failure: a table this gate wrote and the
        // machine rewrote that no longer parses is the crash-consistency BUGS entry showing up.
        eprintln!("rollback-boot: the disk's table no longer parses after the machine rewrote it");
        return false;
    };

    let states: Vec<State> = slot_states(&table);
    let [zero, one] = states[..] else {
        eprintln!("rollback-boot: the disk no longer has two boot slots");
        return false;
    };
    eprintln!(
        "rollback-boot: slot 1 is priority {}, {} tries, {}successful",
        one.priority,
        one.tries,
        if one.successful { "" } else { "not " }
    );

    let mut ok = true;
    // The try was spent, and nothing else about the slot was.
    if one != State::on_trial(TRIAL_PRIORITY, 0) {
        eprintln!(
            "rollback-boot: slot 1 should be priority {TRIAL_PRIORITY}, 0 tries, not successful"
        );
        ok = false;
    }
    // And the image the machine fell back to was not touched on the way past.
    if zero != State::installed() {
        eprintln!("rollback-boot: slot 0 is no longer the state the install left it in");
        ok = false;
    }
    ok
}

/// **Every boot slot's state, in table order**, which is the order `uefi_loader`'s chooser counts
/// them in and therefore the order a slot number means.
///
/// Shared by this gate and `confirm`'s, which assert opposite things about the same bits.
pub(crate) fn slot_states(table: &Table) -> Vec<State> {
    table
        .partitions()
        .filter(|(_, p)| p.type_guid == types::NIFE_BOOT)
        .map(|(_, p)| State::from_attributes(p.attributes))
        .collect()
}

/// **Read the primary partition table off an installed disk**, for a gate checking what the machine
/// wrote down rather than what it said.
pub(crate) fn read_table(disk: &Path, out: &mut [u8; PRIMARY_BYTES]) -> bool {
    let Ok(mut file) = std::fs::File::open(disk) else {
        eprintln!("could not reopen {}", disk.display());
        return false;
    };
    if file.read_exact(out).is_err() {
        eprintln!("could not read the partition table back");
        return false;
    }
    true
}
