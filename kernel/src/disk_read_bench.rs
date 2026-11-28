//! The read-only NVMe bench boot, behind the `disk_read_bench` feature (name provisional).
//!
//! [`run`] replaces the hand-over: it starts the confined EL0 NVMe server, prints what the
//! controller and its namespace say they are, times a fixed number of reads through the disk
//! tree's window, prints one `disk-read-bench:` verdict and halts. It exists for a board that boots
//! from the network and so never mounts a disk tree: the drive is read here, through the same
//! window the disk tree would use, without a file service under it. It serves milestone 163 (the
//! JH7110's PCIe root complex). See nifeos/record:notes/bench-boots.md and
//! nifeos/record:notes/jh7110-pci-express-bench.md.
//!
//! **Nothing here writes the drive.** Every request is a `READ` or a `GET_SIZE_BYTES`, and every
//! one goes through [`DISK_TREE_WINDOW`], whose capability the data plane refuses `WRITE` and
//! `FLUSH` with `EROFS`. So a mistake in this file could only be refused, not written: the read-only
//! property is the capability's, not this code's. No `WRITE` or `FLUSH` is sent at all, not even one
//! expected to be refused, so the transcript of a board run carries no write command of any kind.
//!
//! Name: provisional, minted 2026-11-28 (UTC) by lane/163-nvme-read-bench, after
//! `disk_throughput`: the module names what the boot measures, and `read` is what tells the two
//! apart. calef has not named it.

//! # BUGS
//! - The client is in the kernel, so the figure leaves out a client process's cost.
//! - Sequential only, from block 0, one pass, no warm-up and no repeats.

use driver_process_contract::non_volatile_memory_express::{BringUpError, DISK_TREE_WINDOW};
use file_system_contract::block;

use crate::non_volatile_memory_express::{Absent, BLOCK_SIZE};
use crate::user::non_volatile_memory_express_service::{self as service, Wiring};
use crate::{arch, println};

/// Blocks per timed request: the whole window, 64 KiB, the most one request may move.
const BLOCKS_PER_READ: u64 = block::TRANSFER_BLOCKS as u64;

/// How many requests the timed phase makes: 16 MiB at [`BLOCKS_PER_READ`], or as much of the
/// namespace as there is. Long enough that the phase takes milliseconds on silicon, short enough
/// that TCG finishes it in seconds.
const TIMED_READS: u64 = 256;

/// The two fill bytes the landing check stages before each of its two reads.
const FILLS: [u8; 2] = [0xaa, 0x55];

/// Run the measurement, print the verdict, and never come back.
///
/// Name: provisional, minted 2026-11-28 (UTC) by lane/163-nvme-read-bench, the `run` every
/// measurement boot's module exposes.
pub fn run() -> ! {
    println!();
    println!(
        "disk-read-bench: read-only NVMe bench boot (milestone 163). Reads only, through the \
         disk tree's window."
    );
    println!(
        "disk-read-bench: a skip is not a pass. {} build, {} Hz counter.",
        // A debug build's figures understate the driver, so the photograph says which.
        if cfg!(debug_assertions) {
            "DEBUG"
        } else {
            "release"
        },
        arch::timer::frequency(),
    );
    let verdict = measure();
    println!("disk-read-bench: verdict {verdict}");
    println!("disk-read-bench: done, halting.");
    arch::halt(arch::HaltReason::measurement_boot());
}

/// The verdict line's text, decided in one place.
enum Verdict {
    /// Every read came back, the landing check agreed, and the rate is this many bytes per second.
    ReadAtRate {
        rate: u64,
        reads: u64,
        confined: bool,
        scope: crate::iommu::Scope,
    },
    Skipped(&'static str),
    Failed(&'static str),
}

impl core::fmt::Display for Verdict {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Verdict::ReadAtRate {
                rate,
                reads,
                confined,
                scope,
            } => write!(
                f,
                "READ-AT-RATE: read {rate} B/s over {reads} reads of {} B, EL0 driver, \
                 nothing written; DMA {}: {scope}",
                BLOCKS_PER_READ * BLOCK_SIZE as u64,
                if *confined { "confined" } else { "unconfined" },
            ),
            Verdict::Skipped(why) => write!(f, "SKIPPED: {why}. Not a pass."),
            Verdict::Failed(why) => write!(f, "FAILED: {why}"),
        }
    }
}

/// The NVMe server's image, measured through `trust::require_program` as the progenitor would
/// have; a refusal halts with `MEASURED BOOT REFUSED`. Returns why when the archive has none.
fn measured_server() -> Result<&'static [u8], &'static str> {
    const NAME: &str = "non_volatile_memory_express";
    let image = crate::trust::require_program(NAME)
        .ok_or("no non_volatile_memory_express program in the initial ramdisk")?;
    let hex = measured_boot::hex(&measured_boot::sha256(image));
    println!(
        "disk-read-bench: measured   {NAME} sha256 {}.. matches the table this kernel vouches for",
        core::str::from_utf8(&hex[..16]).unwrap_or("?"),
    );
    Ok(image)
}

fn measure() -> Verdict {
    let image = match measured_server() {
        Ok(image) => image,
        Err(why) => return Verdict::Failed(why),
    };
    let disk = match bring_up(image) {
        Ok(disk) => disk,
        Err(verdict) => return verdict,
    };
    let confined = print_identity(&disk);
    if let Err(verdict) = check_size(&disk) {
        return verdict;
    }
    let total = disk.size_bytes / BLOCK_SIZE as u64;
    if total < BLOCKS_PER_READ {
        return Verdict::Failed("the namespace is smaller than one read");
    }
    if let Err(verdict) = check_landing(&disk) {
        return verdict;
    }
    let reads = TIMED_READS.min(total / BLOCKS_PER_READ);
    match timed_reads(&disk, reads) {
        Ok(ticks) => Verdict::ReadAtRate {
            rate: rate(reads * BLOCKS_PER_READ * BLOCK_SIZE as u64, ticks),
            reads,
            confined,
            scope: disk.scope,
        },
        Err(verdict) => verdict,
    }
}

/// Start the NVMe server and wait for it to come up; a missing, refused or failed controller is
/// the verdict instead.
fn bring_up(image: &'static [u8]) -> Result<Wiring, Verdict> {
    let disk = match service::ensure_or_why(image) {
        Ok(disk) => disk,
        Err(Absent::NoController) => {
            println!("disk-read-bench: controller none: no function on the bus is NVMe class");
            return Err(Verdict::Skipped("no NVMe controller on the bus"));
        }
        Err(Absent::Refused {
            rid,
            why:
                BringUpError::UnsupportedNamespace {
                    logical_block_size_shift: lba_shift,
                    blocks,
                },
        }) => {
            print_scope(rid, crate::iommu::scope_of(rid));
            println!(
                "disk-read-bench: lba size   {}-byte lbas ({blocks} of them): 4096-byte blocks \
                 need 512, 1024, 2048 or 4096 (blocks_per 1..=8)",
                1u64.checked_shl(lba_shift as u32).unwrap_or(0),
            );
            return Err(Verdict::Skipped(
                "the namespace's lba size gives no blocks_per in 1..=8",
            ));
        }
        Err(Absent::Unconfined { rid, why }) => {
            print_scope(rid, crate::iommu::scope_of(rid));
            println!("disk-read-bench: controller {rid:#06x} unconfined: {why}");
            return Err(Verdict::Failed(
                "the controller is present and the IOMMU refused it (line above)",
            ));
        }
        Err(Absent::Refused { rid, why }) => {
            print_scope(rid, crate::iommu::scope_of(rid));
            println!("disk-read-bench: controller {rid:#06x} failed bring-up: {why:?}");
            return Err(Verdict::Failed(
                "the controller is present and failed bring-up (line above)",
            ));
        }
    };
    if let Some(report) = disk.wait_for_ready()
        && report[0] != file_system_contract::READY
    {
        println!(
            "disk-read-bench: the server did not come up: step word {:#x}, first-read status {:#x}",
            report[0], report[2]
        );
        return Err(Verdict::Failed(
            "the EL0 server reported a failed step (line above)",
        ));
    }
    Ok(disk)
}

/// Print the identify facts once the server is up, and return whether the device is confined.
fn print_identity(disk: &Wiring) -> bool {
    let confined = print_scope(disk.rid, disk.scope);
    let lba = BLOCK_SIZE as u64 / disk.blocks_per as u64;
    println!(
        "disk-read-bench: namespace  {} bytes, {lba}-byte lbas, blocks_per {}",
        disk.size_bytes, disk.blocks_per,
    );
    confined
}

/// Print the controller's place on the bus and its DMA scope, and say whether it is confined.
fn print_scope(rid: u32, scope: crate::iommu::Scope) -> bool {
    let confined = scope.is_confining();
    println!(
        "disk-read-bench: controller nvme {:02x}:{:02x}.{} (rid {rid:#06x}), DMA {}: {scope}",
        rid >> 8,
        (rid >> 3) & 0x1f,
        rid & 7,
        if confined { "confined" } else { "unconfined" },
    );
    confined
}

/// One request through the disk tree's window: `operation` over `blocks` blocks from `first`.
/// Returns the server's status word.
fn call(disk: &Wiring, operation: u64, blocks: usize, first: u64) -> i64 {
    crate::sched::ipc_call_badged(
        disk.request,
        [block::request(operation, blocks), first],
        DISK_TREE_WINDOW,
    )[0] as i64
}

/// The disk tree's window, through the direct map.
fn window(disk: &Wiring) -> &'static mut [u8] {
    // SAFETY: the window is `TRANSFER_BLOCKS` blocks of the server's transfer buffer, which the
    // direct map covers, and the server touches it only inside a CALL this thread is not making.
    unsafe {
        core::slice::from_raw_parts_mut(
            crate::arch::mmu::phys_to_virt(disk.window_phys(DISK_TREE_WINDOW)) as *mut u8,
            BLOCKS_PER_READ as usize * BLOCK_SIZE,
        )
    }
}

/// Ask the size through the window and check it against what bring-up recorded.
fn check_size(disk: &Wiring) -> Result<(), Verdict> {
    let size = call(disk, block::GET_SIZE_BYTES, 1, 0);
    let agrees = size as u64 == disk.size_bytes;
    println!(
        "disk-read-bench: size       GET_SIZE_BYTES through the window answers {size}: {}",
        if agrees { "agrees" } else { "DISAGREES" },
    );
    if agrees {
        Ok(())
    } else {
        Err(Verdict::Failed(
            "GET_SIZE_BYTES through the window disagreed with bring-up",
        ))
    }
}

/// Read the first window of blocks twice, the window filled with a different byte before each, and
/// require the two reads to agree: a read whose data never landed would leave each fill in place.
fn check_landing(disk: &Wiring) -> Result<(), Verdict> {
    let mut first = measured_boot::Sha256::new();
    let mut second = measured_boot::Sha256::new();
    for (fill, digest) in FILLS.into_iter().zip([&mut first, &mut second]) {
        window(disk).fill(fill);
        if call(disk, block::READ, BLOCKS_PER_READ as usize, 0) != 0 {
            return Err(Verdict::Failed("the server refused a read of block 0"));
        }
        digest.update(window(disk));
    }
    let (first, second) = (first.finalize(), second.finalize());
    let hex = measured_boot::hex(&first);
    let agrees = first == second;
    println!(
        "disk-read-bench: landing    blocks 0..{BLOCKS_PER_READ} read twice over {:#04x} then \
         {:#04x}: sha256 {}.. {}",
        FILLS[0],
        FILLS[1],
        core::str::from_utf8(&hex[..16]).unwrap_or("?"),
        if agrees { "both times" } else { "DIFFERS" },
    );
    if agrees {
        Ok(())
    } else {
        Err(Verdict::Failed(
            "two reads of the same blocks disagreed, so a read did not land",
        ))
    }
}

/// Time `reads` sequential window-sized reads from block 0.
fn timed_reads(disk: &Wiring, reads: u64) -> Result<u64, Verdict> {
    let start = arch::timer::now();
    for i in 0..reads {
        if call(
            disk,
            block::READ,
            BLOCKS_PER_READ as usize,
            i * BLOCKS_PER_READ,
        ) != 0
        {
            return Err(Verdict::Failed(
                "the server refused a read inside the namespace",
            ));
        }
    }
    let ticks = arch::timer::now().wrapping_sub(start);
    let bytes = reads * BLOCKS_PER_READ * BLOCK_SIZE as u64;
    println!(
        "disk-read-bench: read       {reads} x {} B (blocks 0..{}) in {} us = {} B/s ({} ns per read)",
        BLOCKS_PER_READ * BLOCK_SIZE as u64,
        reads * BLOCKS_PER_READ,
        microseconds(ticks),
        rate(bytes, ticks),
        nanoseconds(ticks) / reads.max(1),
    );
    Ok(ticks)
}

fn microseconds(ticks: u64) -> u64 {
    ticks.saturating_mul(1_000_000) / arch::timer::frequency()
}

fn nanoseconds(ticks: u64) -> u64 {
    // u128 because a 2.7 GHz counter over a minute already overflows `ticks * 1e9` in a u64.
    ((ticks as u128 * 1_000_000_000) / arch::timer::frequency() as u128) as u64
}

/// Bytes per second over `ticks`, or zero for a span too short to measure.
fn rate(bytes: u64, ticks: u64) -> u64 {
    if ticks == 0 {
        return 0;
    }
    ((bytes as u128 * arch::timer::frequency() as u128) / ticks as u128) as u64
}
