//! `mkfs`: `mkfs` on the target (milestone 57's write half, notes/nifefs.md).
//!
//! Creates a filesystem with two capabilities and no more: a block-service endpoint for one disk
//! (`filesystem_contract::blk`), the power to destroy that disk, and an entropy endpoint
//! (`entropy_contract`), the power to obtain random bytes. Neither suffices alone: a filesystem
//! carries a unique id, and the vendored RedoxFS has no `no_std` randomness, so the engine takes
//! the uuid as an argument (`vendor/README.md` divergence 4,
//! `patches/redoxfs-no-std-create-uuid.patch`). The kernel-side test runs it with both, with the
//! disk alone (it writes nothing) and with entropy alone. See notes/fs-server/mkfs-on-target.md.

//! # What it does
//!
//! 1. Finds the nife data partition by type GUID, never by name: DECISIONS §45 (a nife partition is
//!    `EC5CC08B-D749-4434-AC38-A274C50385BA`).
//! 2. Draws sixteen random bytes before the first write, so a refusal leaves the partition intact.
//! 3. Creates the filesystem through a [`PartitionDisk`] bounded to the partition, so the engine
//!    cannot address a byte outside it even by arithmetic error.
//! 4. Reopens it through [`redoxfs_server::Server`] and writes one file, so the platter holds a
//!    filesystem with a tree in it rather than only a header.

//! # EXAMPLES
//!
//! ```text
//!   wired with a disk AND an entropy endpoint:    report: R_MADE, 12288 blocks, ctime 0
//!   wired with the disk and NO entropy endpoint:  report: R_NO_ENTROPY (partition untouched)
//!   wired with entropy and NO disk:               report: R_NO_DISK (nothing to write to)
//! ```

//! # BUGS
//!
//! - Every timestamp on the new filesystem is zero: it holds no clock capability, and adding one
//!   would blur the two-capability claim.
//! - It refuses a partition not 4096-aligned at both ends rather than rounding in: blocks
//!   straddling its first byte would be readable only through the offset disk that made them.
//! - No progress, no resume, and no confirmation: it formats one partition of one disk.
//! - Unencrypted (milestone 57 (partitioning and formatting a real drive)): encryption needs a
//!   salt and two keys, and the `no_std` path refuses a password rather than skip it silently.

#![no_std]
#![no_main]

extern crate alloc;

use filesystem_contract::{blk, req};
use filesystem_fixture::blank;
use globally_unique_identifier_partition_table::GloballyUniqueIdentifierPartitionTable;
use globally_unique_identifier_partition_table::guid::types;
use redoxfs::{BLOCK_SIZE, Disk, FileSystem};
use redoxfs_server::{NameCopy, Server};
use syscall::error::{EIO, Error, Result};
use user_mode_runtime::{call, send};

/// Capability table slots, by convention with `kernel/src/user/disk_service.rs`.
const MEMORY_REGION: u64 = 0;
/// The block service for the one disk. **Empty in the no-disk control.**
const BLK: u64 = 1;
/// The entropy service. **Empty in the no-entropy control.**
const ENTROPY: u64 = 2;
/// Where the verdict goes.
const REPORT: u64 = 3;
/// The page shared with the block server, `READ|WRITE`. Granted in **every** wiring, the two
/// controls included, so a refusal can only be explained by the capability that is missing.
const BLK_PAGE_FRAME: u64 = 4;

/// Where this program puts the page it shares with the block server. **Its choice**: it holds the
/// frame and maps it (milestone 108). Clear of the heap, which runs from
/// `memory_region_heap::DEFAULT_BASE` (1 GiB) for [`HEAP_MAX`].
const BLK_PAGE: u64 = address_space_map::service_window(0x5000_0000);

/// One filesystem block / one shared page, in bytes. RedoxFS's `BLOCK_SIZE` and the blk wire's
/// transfer unit are the same number, which is what lets a `Disk` block be a blk block.
const BLOCK: usize = blk::BLOCK_SIZE;

/// The logical block size the partition table counts in (`disk_surveyor`'s BUGS: nothing on the
/// wire reports the device's).
const LBA: u64 = blank::LBA;

/// The heap cap. The engine keeps a compress buffer sized by `RECORD_SIZE` (128 KiB, still the ceiling after milestone
/// 138 lowered the *created* record level, because the buffer must fit any record this build can
/// rewrite) plus block buffers and
/// small tree structures; creation touches fewer of them than a mount does. The untyped the kernel
/// grants is the real ceiling.
const HEAP_MAX: u64 = 8 * 1024 * 1024;

#[global_allocator]
static HEAP: memory_region_heap::MemoryRegionHeap = memory_region_heap::MemoryRegionHeap::new();

// The roles, in `a0`. Must match kernel/src/user/disk_service.rs.
/// Create a filesystem in the nife data partition.
const ROLE_MAKE: u64 = 0;
/// **Read the partition back and say whether there is a filesystem in it.** A process with a disk
/// and no entropy, so it cannot create one however it is spawned, which is what makes it usable
/// both before and after a run that was supposed to create nothing.
const ROLE_CHECK: u64 = 1;

// The report's first word. Must match system_tests/src/user/disk_tests.rs.
/// A filesystem was created and a file written into it. ASCII `MKFSD`, so a hex dump reads.
pub const R_MADE: u64 = 0x_4D_4B_46_53_44;
/// **No entropy endpoint, so nothing was created.** A filesystem id that is not unique is a
/// filesystem two disks can claim to be; refusing beats inventing one (DECISIONS §42).
pub const R_NO_ENTROPY: u64 = 0x_4E_4F_52_4E_47;
/// **No block-service endpoint**, so there is no disk to read a table off or write a filesystem to.
pub const R_NO_DISK: u64 = 0x_4E_4F_44_53_4B;
/// The disk answered, but it carries no nife data partition to format.
pub const R_NO_PARTITION: u64 = 0x_4E_4F_50_54_4E;
/// The partition is there and the engine refused to make a filesystem in it. The second word is a
/// step number, the third the engine's errno.
pub const R_FAILED: u64 = 0x_4D_4B_45_52_52;

// [`ROLE_CHECK`]'s verdicts.
/// The filesystem opened and holds the file this program writes, with the right bytes in it.
pub const R_FOUND: u64 = 0x_46_4F_55_4E_44;
/// **The partition holds no filesystem at all.** What a partition that has only been *described*
/// looks like, and what a refused run must leave behind.
pub const R_NO_FS: u64 = 0x_4E_4F_46_53_00;
/// A filesystem is there and the file is not, or its bytes are wrong.
pub const R_NO_FILE: u64 = 0x_4E_4F_46_4C_45;

user_mode_runtime::program_entry!(start);

fn start(role: u64, _a1: u64, _a2: u64) -> ! {
    HEAP.init(MEMORY_REGION, memory_region_heap::DEFAULT_BASE, HEAP_MAX);

    // The shared page, mapped out of the same budget the heap draws on (milestone 108). It is
    // granted in all three wirings, so a failure here is a broken kernel rather than a control.
    if !user_mode_runtime::map_page_frame(BLK_PAGE_FRAME, BLK_PAGE, true, MEMORY_REGION) {
        user_mode_runtime::exit()
    }

    // The disk first, because "I was given no disk" and "I was given no entropy" must be
    // distinguishable, and the cheapest question decides it.
    let Some((first_block, blocks)) = data_partition() else {
        send(REPORT, R_NO_DISK, 0, 0);
        user_mode_runtime::exit()
    };
    if blocks == 0 {
        send(REPORT, R_NO_PARTITION, 0, 0);
        user_mode_runtime::exit()
    }

    if role == ROLE_CHECK {
        check(first_block, blocks)
    }
    debug_assert!(role == ROLE_MAKE);

    // **Before the first write.** A program that cannot be given randomness must find out while the
    // partition still holds whatever it held.
    let mut uuid = [0u8; 16];
    if user_mode_runtime::entropy::fill(ENTROPY, &mut uuid).is_none() {
        send(REPORT, R_NO_ENTROPY, 0, 0);
        user_mode_runtime::exit()
    }

    let disk = PartitionDisk {
        first_block,
        blocks,
    };
    // ctime zero: this program holds no clock. See BUGS.
    match FileSystem::create_reserved_with_uuid(disk, None, &[], 0, 0, uuid) {
        Ok(fs) => drop(fs),
        Err(e) => {
            send(REPORT, R_FAILED, 1, e.errno as u64);
            user_mode_runtime::exit()
        }
    }

    // Reopen through the FS server's own core and put a file in it. A header that parses proves
    // less than a tree that holds a name: the host tool reads this file back after the run.
    let mut server = match Server::open(PartitionDisk {
        first_block,
        blocks,
    }) {
        Ok(s) => s,
        Err(e) => {
            send(REPORT, R_FAILED, 2, e.errno as u64);
            user_mode_runtime::exit()
        }
    };
    let made = NameCopy::copy_in(blank::MADE_NAME.as_bytes())
        .and_then(|name| server.create_file_at(filesystem_contract::fs::ROOT as u32, &name))
        .and_then(|h| server.write(h, 0, blank::MADE_BODY).map(|n| (h, n)));
    match made {
        Ok((h, n)) if n == blank::MADE_BODY.len() => {
            let _ = server.close(h);
        }
        Ok((_, n)) => {
            send(REPORT, R_FAILED, 3, n as u64);
            user_mode_runtime::exit()
        }
        Err(e) => {
            send(REPORT, R_FAILED, 4, e.errno as u64);
            user_mode_runtime::exit()
        }
    }
    drop(server);

    send(REPORT, R_MADE, blocks, first_block);
    user_mode_runtime::exit()
}

/// **Is there a filesystem in the partition, and does it hold what a creation run puts there?**
///
/// Never creates and never writes: it opens read-only in the sense that matters here (a `Server`
/// mount does tidy allocations, which is why this runs against the disk and not against a fixture
/// somebody else needs). It is the reading that turns "the refused run reported R_NO_ENTROPY" into
/// "the refused run left an unformatted partition".
fn check(first_block: u64, blocks: u64) -> ! {
    // The signature first: on an unformatted partition `FileSystem::open`'s header scan runs until
    // a read fails at the end, 12,288 IPC round trips. Block 0 always holds generation zero's
    // header, so its signature answers "is there a filesystem here" in one read.
    if blk_call(blk::READ, first_block) < 0 {
        send(REPORT, R_NO_DISK, 0, 0);
        user_mode_runtime::exit()
    }
    // SAFETY: BLK_PAGE is a mapped page of BLOCK bytes holding the block just read.
    let signature = unsafe { core::slice::from_raw_parts(BLK_PAGE as *const u8, 8) };
    if signature != redoxfs::SIGNATURE {
        send(REPORT, R_NO_FS, 0, 0);
        user_mode_runtime::exit()
    }

    let mut server = match Server::open(PartitionDisk {
        first_block,
        blocks,
    }) {
        Ok(s) => s,
        // The engine says ENOENT for a partition with no valid header anywhere in the ring, which
        // is what an unformatted one is.
        Err(_) => {
            send(REPORT, R_NO_FS, 0, 0);
            user_mode_runtime::exit()
        }
    };
    let mut buf = [0u8; 128];
    let read = NameCopy::copy_in(blank::MADE_NAME.as_bytes())
        .and_then(|name| server.open_file_at(filesystem_contract::fs::ROOT as u32, &name))
        .and_then(|h| server.read(h, 0, &mut buf[..blank::MADE_BODY.len()]));
    match read {
        Ok(n) if n == blank::MADE_BODY.len() && buf[..n] == *blank::MADE_BODY => {
            send(REPORT, R_FOUND, n as u64, 0)
        }
        _ => send(REPORT, R_NO_FILE, 0, 0),
    };
    user_mode_runtime::exit()
}

/// Find the nife data partition on the disk this process holds: `(first block, block count)`
/// in RedoxFS blocks. `None` when there is no disk at all; `Some((_, 0))` when there is a disk and
/// no such partition.
///
/// The table is read with `crates/globally_unique_identifier_partition_table`, the same parser
/// `disk_surveyor` uses and the same one whose host tests run against tables `sgdisk` and macOS
/// `diskutil` wrote. **The partition is found by type GUID**, never by name: macOS writes no GPT
/// partition names at all (notes/globally-unique-identifier-partition-table.md).
fn data_partition() -> Option<(u64, u64)> {
    let size = blk_call(blk::SIZE, 0);
    if size <= 0 {
        return None; // no endpoint in slot BLK, or a device that cannot say how big it is
    }
    let device_blocks = (size as u64) / LBA;

    // LBA 0..=33: the protective MBR, the header, and the whole 16 KiB entry array. Five transfer
    // blocks cover it. On the heap rather than in `.bss` because this program has one.
    let mut head = alloc::vec![0u8; 5 * BLOCK];
    for i in 0..5u64 {
        if blk_call(blk::READ, i) < 0 {
            return None;
        }
        // SAFETY: BLK_PAGE is a mapped page of exactly BLOCK bytes; the window is in range by
        // construction (five blocks into a five-block buffer).
        unsafe {
            core::ptr::copy_nonoverlapping(
                BLK_PAGE as *const u8,
                head.as_mut_ptr().add(i as usize * BLOCK),
                BLOCK,
            );
        }
    }

    let table = GloballyUniqueIdentifierPartitionTable::parse(
        &head[LBA as usize..2 * LBA as usize],
        &head[2 * LBA as usize..],
    )
    .ok()?;
    for (_, part) in table.partitions() {
        if part.type_guid != types::NIFE_DATA {
            continue;
        }
        let start = part.first_lba * LBA;
        let len = part.blocks()? * LBA;
        // Both ends on a filesystem block, or refuse. See BUGS: a filesystem whose blocks straddle
        // the partition's first byte is readable only through the offset disk that wrote it.
        if !start.is_multiple_of(BLOCK_SIZE) || !len.is_multiple_of(BLOCK_SIZE) {
            return Some((0, 0));
        }
        let first_block = start / BLOCK_SIZE;
        let blocks = len / BLOCK_SIZE;
        // And inside the device the block service is actually serving.
        if first_block + blocks > device_blocks * LBA / BLOCK_SIZE {
            return Some((0, 0));
        }
        return Some((first_block, blocks));
    }
    Some((0, 0))
}

/// One blk `CALL`: the opcode and a block index, with the bulk in [`BLK_PAGE`]. Negative is an
/// error, and "there is no endpoint in that slot" is one of them.
fn blk_call(operation_code: u64, block: u64) -> i64 {
    let (r0, _) = call(BLK, req(operation_code), block);
    r0 as i64
}

/// **The RedoxFS `Disk` for one partition**: block zero is the partition's first block and `size` is
/// the partition's length, so every address the engine computes is inside the partition by
/// construction rather than by the engine's good behaviour.
///
/// This is the FS server's `IpcDisk` plus an origin and a length, and it is deliberately its own
/// copy rather than a shared module: the two differ in what they enforce (this one bounds; that one
/// carries milestone 37's crash injector), and there is one consumer of each.
struct PartitionDisk {
    /// The partition's first block, in RedoxFS blocks, from the start of the device.
    first_block: u64,
    /// How many blocks the partition holds.
    blocks: u64,
}

impl PartitionDisk {
    /// The device block a filesystem block lands on, or `None` if it is past the partition's end.
    fn device_block(&self, block: u64) -> Option<u64> {
        (block < self.blocks).then_some(self.first_block + block)
    }
}

impl Disk for PartitionDisk {
    unsafe fn read_at(&mut self, block: u64, buffer: &mut [u8]) -> Result<usize> {
        for (i, chunk) in buffer.chunks_mut(BLOCK).enumerate() {
            let Some(at) = self.device_block(block + i as u64) else {
                return Err(Error::new(EIO));
            };
            if blk_call(blk::READ, at) < 0 {
                return Err(Error::new(EIO));
            }
            // SAFETY: BLK_PAGE is a mapped page of exactly BLOCK bytes and `chunk` is no larger.
            unsafe {
                core::ptr::copy_nonoverlapping(
                    BLK_PAGE as *const u8,
                    chunk.as_mut_ptr(),
                    chunk.len(),
                )
            };
        }
        Ok(buffer.len())
    }

    unsafe fn write_at(&mut self, block: u64, buffer: &[u8]) -> Result<usize> {
        for (i, chunk) in buffer.chunks(BLOCK).enumerate() {
            let Some(at) = self.device_block(block + i as u64) else {
                return Err(Error::new(EIO));
            };
            if chunk.len() < BLOCK {
                // A partial final block would clobber the rest of the block; read-modify-write so
                // only the given bytes change. The same courtesy `IpcDisk` pays, for the same
                // reason: a `Disk` owes it even if this engine never asks.
                if blk_call(blk::READ, at) < 0 {
                    return Err(Error::new(EIO));
                }
            }
            // SAFETY: as above; `chunk` is no larger than the page.
            unsafe {
                core::ptr::copy_nonoverlapping(chunk.as_ptr(), BLK_PAGE as *mut u8, chunk.len())
            };
            if blk_call(blk::WRITE, at) < 0 {
                return Err(Error::new(EIO));
            }
        }
        Ok(buffer.len())
    }

    fn size(&mut self) -> Result<u64> {
        // **The partition's size, not the device's.** This is the line that keeps the engine inside
        // the partition: it sizes its allocator from this number.
        Ok(self.blocks * BLOCK_SIZE)
    }
}

user_mode_runtime::panic_handler!();
