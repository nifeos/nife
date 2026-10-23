//! The offer a booted stick makes to install itself onto an NVMe disk not already carrying nife,
//! asked before the capability to destroy the disk exists; a person must type [`CONFIRMATION`] in
//! time. Nothing here writes a byte to a disk. See notes/installing.md.
//!

//! # BUGS
//!
//! - x86_64 only (design/roadmap/0568): the kernel reads the UART to ask, before the input driver.
//! - The disk is named by size, not model (design/roadmap/0569).
//! - It checks for nife and nothing else, so a disk holding Windows looks empty (0570).
//! - A disk already carrying nife, even half-installed, is never offered again (0572).
//! - Not crash-atomic: a power cut between the partitioner and `mkfs` leaves an unmountable disk.

use super::*;
use crate::cap::{Rights, memory_region_cap, page_frame_cap, rendezvous_cap};
use crate::sched::RendezvousId;

/// The word a person types to destroy a disk. Upper case and unabbreviated, so that a stray
/// newline, a `y`, or a terminal echoing something cannot be it.
pub(crate) const CONFIRMATION: &str = "INSTALL";

/// How long the offer waits for an answer before continuing the boot.
///
/// Long enough to read three lines and type seven characters; short enough that a stick left in an
/// unattended machine still reaches a prompt. Measured against the counter, not a spin count.
const PATIENCE: core::time::Duration = core::time::Duration::from_secs(30);

/// Where the kernel maps the boot file, read-only, into the installer. Must match
/// `components/src/system_installer.rs`'s `BOOT_FILE_VA`; the bytes are a run of physical frames
/// the kernel owns, so the program cannot map them itself.
const BOOT_FILE_VA: u64 = address_space_map::runtime_window(0x1000_0000);

/// The installer's capability table. Must match `components/src/system_installer.rs`.
const SLOT_REPORT: u64 = 0;
const SLOT_BLK: u64 = 1;
const SLOT_ENTROPY: u64 = 2;
const SLOT_BUDGET: u64 = 3;
const SLOT_BLK_PAGE: u64 = 4;

/// The untyped the installer spends on the page tables for the one page it maps itself. The same
/// figure `disk_service` gives the partitioner, and for the same reason: it maps one page.
const SYSTEM_INSTALLER_BUDGET_PAGES: u64 = 64;

/// Extra stack pages for the installer, below the single page `run` maps.
///
/// It keeps a 16 KiB entry array in `.bss` rather than on the stack and recurses nowhere, so this
/// is the partitioner's number rather than a measurement of its own; the deepest thing it does is
/// `GloballyUniqueIdentifierPartitionTable::create`.
const SYSTEM_INSTALLER_STACK_PAGES: usize = 8;

/// The report's first word, from `components/src/system_installer.rs`. Duplicated here rather than shared
/// through a crate because it is the kernel's own convention with one program it spawns, the same
/// terms `disk_service` and `disk_tests` already state for `R_PARTITIONED` and `R_MADE`.
const R_INSTALLED: u64 = 0x_49_4E_53_54_44;

/// `mkfs`'s create role, from `redoxfs_server/src/bin/mkfs.rs`. Spelled here for the same reason
/// `disk_tests` spells it: the kernel's convention with a program it spawns.
const ROLE_MAKE: u64 = 0;
/// `mkfs`'s "a filesystem was created and a file written into it" verdict, ASCII `MKFSD`.
const R_MADE: u64 = 0x_4D_4B_46_53_44;

// The installer's roles, in `a0`. Must match `components/src/system_installer.rs`.
const ROLE_INSTALL: u64 = 0;
const ROLE_SURVEY: u64 = 1;
const ROLE_CONFIRM: u64 = 2;
/// The survey's "this disk already carries a nife data partition" verdict, ASCII `HAVIT`.
const R_ALREADY: u64 = 0x_48_41_56_49_54;
/// The confirm role's "that slot is successful" verdict, ASCII `CNFRM`.
const R_CONFIRMED: u64 = 0x_43_4E_46_52_4D;
/// Its "there is no such boot slot on this disk" refusal, ASCII `NOSLT`.
const R_NO_SLOT: u64 = 0x_4E_4F_53_4C_54;

/// Offer to install, and carry it out if a person says so. Returns after either, so the boot
/// continues in every case.
///
/// The three programs it needs are named rather than passed, because a caller on the boot path
/// has no business knowing which binaries an install is made of.
pub(crate) fn offer() {
    let Some((_, boot_file_len)) = crate::memory::boot_file_region() else {
        return; // Not booted from a file: nothing to install.
    };
    let Some(system_installer) = program("system_installer") else {
        crate::println!(
            "  install     : this boot came from a file, but the archive has no system_installer."
        );
        return;
    };
    let Some(nvme) = program("non_volatile_memory_express") else {
        return;
    };
    let Some(disk) = non_volatile_memory_express_service::ensure(nvme) else {
        return; // No NVMe controller: nothing to install onto.
    };
    disk.wait_for_ready();

    // Look before asking, or every installed machine would be asked to wipe itself on every boot.
    // The survey holds no entropy, so it can write no table anything reads back.
    let report = spawn_system_installer(system_installer, &disk, None, ROLE_SURVEY, 0, 0);
    if crate::sched::ipc_receive(report)[0] == R_ALREADY {
        return; // nife is already on this disk. See BUGS: there is no reinstall.
    }

    if !ask_to_confirm(disk.size_bytes) {
        crate::println!("  install     : not confirmed; continuing the boot.");
        return;
    }

    // Entropy from `RDSEED`, so a machine with only a disk can install; without it, nothing is
    // written rather than a table of invented ids.
    let entropy = program("entropy")
        .and_then(|image| entropy_service::ensure(image, entropy_service::Bus::Instruction))
        .map(|w| {
            w.wait_for_ready();
            w.request
        });
    let Some(entropy) = entropy else {
        crate::println!(
            "  install     : this machine has no source of randomness, so a partition table \
             written here could not carry unique ids. Nothing was written."
        );
        return;
    };

    crate::println!("  install     : partitioning and copying the boot file...");
    let report = spawn_system_installer(
        system_installer,
        &disk,
        Some(entropy),
        ROLE_INSTALL,
        boot_file_len,
        boot_file_len,
    );
    let answer = crate::sched::ipc_receive(report);
    if answer[0] != R_INSTALLED {
        crate::println!(
            "  install     : FAILED ({:#x}, {}, {}). The disk may be in any state.",
            answer[0],
            answer[1],
            answer[2]
        );
        return;
    }
    crate::println!(
        "  install     : installed. nife data at LBA {}, EFI system at LBA {}.",
        answer[1],
        answer[2]
    );

    let Some(maker) = program("mkfs") else {
        crate::println!("  install     : the archive has no mkfs, so the data partition is empty.");
        return;
    };
    let target = disk_service::BlankDisk {
        blk_ep: disk.request,
        blk_shared: disk.transfer_phys,
        blk_ready: None,
    };
    let report = disk_service::start_maker(maker, &target, ROLE_MAKE, Some(entropy), true);
    let answer = crate::sched::ipc_receive(report);
    if answer[0] != R_MADE {
        crate::println!(
            "  install     : the partition table is written but mkfs refused ({:#x}).",
            answer[0]
        );
        return;
    }
    crate::println!("  install     : filesystem created.");
    crate::println!("  install     : DONE. Remove the installation medium and reboot.");
}

/// Say that this boot worked, so an upgrade that came up is not rolled back once its tries run out;
/// a no-op on a boot no chooser started. It is called once the filesystem server has mounted the
/// installed disk and the progenitor is built: as late as a machine with no person can check. The
/// write is `system_installer`'s [`ROLE_CONFIRM`], holding the report endpoint, the disk's `blk`
/// endpoint badged with its own transfer window (`non_volatile_memory_express::INSTALLER_WINDOW`),
/// that window, and a map budget; no entropy and no boot file. A `blk` endpoint is the whole disk,
/// as `system_installer`'s `BUGS` records. What it writes is the partition table, which the
/// filesystem server never reads after mounting, so the two never contend for a block. See
/// notes/boot-slots.md.
pub(crate) fn confirm() {
    let Some(slot) = crate::memory::boot_slot() else {
        return; // No chooser started this boot: there is nothing to confirm.
    };
    let Some(system_installer) = program("system_installer") else {
        crate::println!(
            "  boot slot   : slot {slot} cannot be confirmed: the archive has no system_installer."
        );
        return;
    };
    let Some(nvme) = program("non_volatile_memory_express") else {
        return;
    };
    let Some(disk) = non_volatile_memory_express_service::ensure(nvme) else {
        // Say so, rather than leave a silent revert three boots later.
        crate::println!("  boot slot   : slot {slot} cannot be confirmed: no disk.");
        return;
    };
    disk.wait_for_ready();

    let report =
        spawn_system_installer(system_installer, &disk, None, ROLE_CONFIRM, slot as u64, 0);
    let answer = crate::sched::ipc_receive(report);
    match answer[0] {
        R_CONFIRMED if answer[2] == 0 => {
            crate::println!("  boot slot   : slot {slot} was already confirmed; nothing written.");
        }
        R_CONFIRMED => {
            crate::println!(
                "  boot slot   : slot {slot} confirmed. This upgrade will not roll back."
            );
        }
        R_NO_SLOT => {
            // Nothing was written, so this machine rolls back; this line is the only place a
            // person would find out why.
            crate::println!(
                "  boot slot   : slot {slot} is not on this disk ({} boot slots found); NOT confirmed.",
                answer[1]
            );
        }
        other => {
            crate::println!(
                "  boot slot   : slot {slot} NOT confirmed ({other:#x}, step {}). This boot will roll back.",
                answer[1]
            );
        }
    }
}

/// Print what is about to be destroyed and read the answer. `true` only for exactly
/// [`CONFIRMATION`].
///
/// Name: provisional, minted 2026-10-18 (UTC) by maintainer/names-comply-question-shapes, as the
/// yes-or-no worklist recommended.
/// Refused `asked` (a participle that reads as a question, on a function that prompts and reads
/// the reply).
fn ask_to_confirm(size_bytes: u64) -> bool {
    crate::println!("  install     : this system was booted from a file and can install itself.");
    crate::println!(
        "  install     :   TARGET: the NVMe disk attached to this machine, {size_bytes} bytes."
    );
    crate::println!("  install     :   EVERYTHING ON THAT DISK WILL BE DESTROYED.");
    crate::println!(
        "  install     :   Type {CONFIRMATION} and press return to proceed; anything else \
         continues the boot."
    );
    crate::print!("  install     : > ");

    let mut typed = [0u8; 32];
    let Some(n) = crate::console::read_line(&mut typed, PATIENCE) else {
        crate::println!();
        return false;
    };
    &typed[..n] == CONFIRMATION.as_bytes()
}

/// Spawn the installer with the four things it needs and nothing else: the disk, the randomness,
/// the page it shares with the disk's server, and a read-only copy of the file this machine booted
/// from.
fn spawn_system_installer(
    image: &'static [u8],
    disk: &non_volatile_memory_express_service::Wiring,
    entropy: Option<RendezvousId>,
    role: u64,
    // What the role reads out of `a1`: the boot file's length for [`ROLE_INSTALL`], the slot
    // number for [`ROLE_CONFIRM`], nothing for [`ROLE_SURVEY`].
    arg1: u64,
    // How many bytes of the boot file to map, separate from `arg1` because only one role wants
    // both. A role that maps none cannot rewrite the image it is looking at.
    boot_file_len: u64,
) -> RendezvousId {
    let report = crate::sched::create_rendezvous();
    let budget = crate::memory_region::create(SYSTEM_INSTALLER_BUDGET_PAGES)
        .expect("no budget for system_installer");
    // The installer's own window, so a confirm beside a live filesystem server stages in pages
    // that server never reads.
    let blk_capability = disk.client_capability(::non_volatile_memory_express::INSTALLER_WINDOW);
    let blk_shared = disk.window_phys(::non_volatile_memory_express::INSTALLER_WINDOW);
    let ep = entropy.unwrap_or_default();

    crate::sched::spawn(move || {
        let maps = system_installer_maps(boot_file_len);
        crate::sched::grant_at(SLOT_REPORT, rendezvous_cap(report, Rights::WRITE))
            .expect("system_installer slot 0 was occupied");
        crate::sched::grant_at(SLOT_BLK, blk_capability)
            .expect("system_installer slot 1 was occupied");
        // Slot 2 is the only difference between the roles. Without entropy a survey cannot draw a
        // new table's unique ids, which narrows what it would write, not what it can.
        if entropy.is_some() {
            crate::sched::grant_at(SLOT_ENTROPY, rendezvous_cap(ep, Rights::WRITE))
                .expect("system_installer slot 2 was occupied");
        }
        crate::sched::grant_at(SLOT_BUDGET, memory_region_cap(budget))
            .expect("system_installer slot 3 was occupied");
        crate::sched::grant_at(
            SLOT_BLK_PAGE,
            page_frame_cap(blk_shared, Rights::READ.union(Rights::WRITE)),
        )
        .expect("system_installer slot 4 was occupied");
        run(
            image,
            Spawn {
                arg0: role,
                arg1,
                arg2: 0,
                grants: &[], // every one of them is placed above, at its own slot
                maps,
            },
        )
    })
    .expect("could not spawn system_installer");

    report
}

/// Build the installer's extra mappings: its stack, then the first `boot_file_len` bytes of the
/// boot file read-only, one entry per page.
///
/// A ten-megabyte boot file is a few thousand entries, too many for a stack or `.bss`, so they go
/// in frames allocated here and never freed: about sixty kilobytes for the rest of the boot.
fn system_installer_maps(boot_file_len: u64) -> &'static [Mapping] {
    let boot_file_phys = crate::memory::boot_file_region().map_or(0, |(at, _)| at);
    let pages = boot_file_len.div_ceil(FRAME_SIZE) as usize;
    let total = SYSTEM_INSTALLER_STACK_PAGES + pages;

    let bytes = total * core::mem::size_of::<Mapping>();
    let scratch = crate::memory::alloc_contiguous_zeroed(bytes.div_ceil(FRAME_SIZE as usize))
        .expect("no scratch for the installer's mappings")
        .addr();
    // SAFETY: `total` `Mapping`s fit in the run of frames just allocated, which nothing else holds;
    // the direct map reaches them, and `Mapping` is a plain `Copy` struct of integers, so the
    // zeroed frames are already a valid initialised slice of them.
    let maps: &'static mut [Mapping] = unsafe {
        core::slice::from_raw_parts_mut(mmu::phys_to_virt(scratch) as *mut Mapping, total)
    };

    for (k, m) in maps[..SYSTEM_INSTALLER_STACK_PAGES].iter_mut().enumerate() {
        m.va = USER_STACK_VA - (k as u64 + 1) * FRAME_SIZE;
        m.phys = crate::memory::alloc()
            .expect("no frame for the installer's stack")
            .addr();
        m.flags = Flags::user_data();
    }
    for (i, m) in maps[SYSTEM_INSTALLER_STACK_PAGES..].iter_mut().enumerate() {
        m.va = BOOT_FILE_VA + i as u64 * FRAME_SIZE;
        m.phys = boot_file_phys + i as u64 * FRAME_SIZE;
        // Read-only: the whole of what keeps a program holding a disk from rewriting the image.
        m.flags = Flags::user_rodata();
    }
    maps
}
