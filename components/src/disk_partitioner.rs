//! `disk_partitioner`: writes the map `disk_surveyor` reads
//! (notes/globally-unique-identifier-partition-table.md, "Writing a table on the target").
//!
//! The destructive half of the pair, and its endowment is the point: a block-service endpoint for
//! one disk (`filesystem_contract::blk`), the authority to destroy that disk and no other, and an
//! entropy endpoint (`entropy_contract`), which reaches random bytes and not the device. Both are
//! required: a GPT's unique ids must be random, so without entropy it writes nothing rather than
//! fall back to a counter. `a0` is [`ROLE_PARTITION`] (draw four GUIDs, lay out a table, write
//! both copies over blocks read first) or [`ROLE_VERIFY`] (a separate process with no entropy that
//! reads the disk back). The layout, `filesystem_fixture::blank`, is 2048-block aligned.

//! # BUGS
//!
//! - The logical block size is assumed to be 512. One hard-coded layout, with no `mkpart` options.
//! - Not crash-atomic: a kill can leave the copies disagreeing, which `check_backup` reports.
//! - New GUIDs are not checked against an existing table, which is replaced wholesale.
//!
//! Name: ratified 2026-09-13 (calef), the destructive half of `disk_surveyor`: survey and
//! partition. Refused `parted` (it borrows the authority model this contrasts with), `gpt_writer`
//! (names the format, not the act) and `disk_formatter` (formatting makes a filesystem).

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

use filesystem_contract::{blk, req};
use filesystem_fixture::blank;
use globally_unique_identifier_partition_table::entry::{Entry, NAME_UNITS};
use globally_unique_identifier_partition_table::guid::types;
use globally_unique_identifier_partition_table::span::Span;
use globally_unique_identifier_partition_table::{
    ENTRY_ARRAY_BYTES, GloballyUniqueIdentifierPartitionTable, PRIMARY_HEADER_LBA, mbr,
};
use user_mode_runtime::{call, send};
use uuid::Uuid;

/// Slot 0: where the verdict goes. An endpoint with `WRITE`.
const REPORT: u64 = 0;
/// Slot 1: the block service for the one disk this program was handed, `WRITE` (it `CALL`s).
const BLK: u64 = 1;
/// Slot 2: the entropy service, `WRITE` (it `CALL`s). **Empty in the negative control.**
const ENTROPY: u64 = 2;
/// Slot 3: the untyped this program spends on the page tables its own mapping needs.
const BUDGET: u64 = 3;
/// Slot 4: the page shared with the block server, `READ|WRITE` (a transfer goes both ways).
const BLK_PAGE_FRAME: u64 = 4;

/// Where this program puts the page it shares with the block server. **Its choice**: it holds the
/// frame and maps it (milestone 108), so nothing on the kernel side names this address.
const BLK_PAGE: u64 = address_space_map::service_window(0x5000_0000);

/// The transfer unit of the block service: one filesystem block per request.
const TRANSFER: u64 = blk::BLOCK_SIZE as u64;

/// The logical block size a GPT counts in. See BUGS.
const LBA: u64 = blank::LBA;

// The roles, in `a0`. Must match kernel/src/user/disk_service.rs.
const ROLE_PARTITION: u64 = 0;
const ROLE_VERIFY: u64 = 1;

// The partition role's verdict, the report's first word. Must match system_tests/src/user/disk_tests.rs.
/// The table was laid out and both copies were written. ASCII `PARTD`, so a hex dump reads.
pub const R_PARTITIONED: u64 = 0x_50_41_52_54_44;
/// **The entropy endpoint was not there, so nothing was written.** The refusal this program exists
/// to make: a unique id that is not random is not unique, and a made-up one would be worse than no
/// partition at all because it would look right (DECISIONS §42).
pub const R_NO_ENTROPY: u64 = 0x_4E_4F_52_4E_47;
/// The disk refused a read or a write, or the layout did not fit it. The second word says where.
pub(crate) const R_DISK_FAILED: u64 = 0x_44_49_53_4B_45;

// The verify role's flags, the report's first word. Must match system_tests/src/user/disk_tests.rs.
/// LBA 0 holds a well-formed protective MBR covering the whole disk.
pub const F_MBR: u64 = 1 << 0;
/// The primary header and entry array parsed: four CRCs, the geometry, no overlaps.
pub const F_PRIMARY: u64 = 1 << 1;
/// The backup table at the far end of the disk agrees with the primary, field by field.
pub const F_BACKUP: u64 = 1 << 2;
/// A partition of type `NIFE_DATA` is on the disk (DECISIONS §45).
pub const F_NIFE: u64 = 1 << 3;
/// Every partition's name decoded as UTF-8 and matched the layout's.
pub const F_NAMES: u64 = 1 << 4;
/// Every partition's unique GUID is distinct, non-zero, and stamped version 4.
pub(crate) const F_UNIQUE: u64 = 1 << 5;

/// The entry array under construction, and the buffer
/// `GloballyUniqueIdentifierPartitionTable::create` writes into. 16 KiB, so it goes in `.bss`
/// rather than on the one-page stack.
static mut ARRAY: [u8; ENTRY_ARRAY_BYTES] = [0; ENTRY_ARRAY_BYTES];
/// The primary table as read back: LBA 0..33 is 34 logical blocks, which is five transfer blocks.
static mut PRIMARY: [u8; 5 * blk::BLOCK_SIZE] = [0; 5 * blk::BLOCK_SIZE];
/// The backup table as read back. 33 logical blocks ending on the last block of the disk; where
/// that begins inside a transfer block depends on the disk's size, so six is the worst case.
static mut BACKUP: [u8; 6 * blk::BLOCK_SIZE] = [0; 6 * blk::BLOCK_SIZE];

#[unsafe(no_mangle)]
pub extern "C" fn _start(role: u64, _a1: u64, _a2: u64) -> ! {
    // The shared page is a `PageFrame` we hold, mapped here out of our own budget (milestone 108).
    // Before either role, because every `blk` call goes through it.
    if !user_mode_runtime::map_page_frame(BLK_PAGE_FRAME, BLK_PAGE, true, BUDGET) {
        user_mode_runtime::exit()
    }
    match role {
        ROLE_PARTITION => partition(),
        ROLE_VERIFY => verify(),
        _ => panic!(),
    }
}

/// Draw a table's worth of GUIDs, lay the table out, write it. **In that order**, so that a program
/// with no entropy endpoint reports and exits with the disk untouched.
fn partition() -> ! {
    // Four unique ids: the disk's, and one per partition. Drawn first and all together, because the
    // only refusal worth making is one made before anything is written.
    let mut guids = [Uuid::nil(); 4];
    for g in guids.iter_mut() {
        let mut bytes = [0u8; 16];
        if user_mode_runtime::entropy::fill(ENTROPY, &mut bytes).is_none() {
            send(REPORT, R_NO_ENTROPY, 0, 0);
            user_mode_runtime::exit()
        }
        *g = uuid::Builder::from_random_bytes(bytes).into_uuid();
    }

    let layout = [
        (types::EFI_SYSTEM, blank::ESP, blank::NAMES[0]),
        (types::LINUX_FILESYSTEM, blank::LINUX, blank::NAMES[1]),
        (types::NIFE_DATA, blank::DATA, blank::NAMES[2]),
    ];
    let mut parts = [Entry::UNUSED; blank::PARTITIONS];
    for (i, (type_guid, (first, last), name)) in layout.iter().enumerate() {
        let Ok(e) = Entry::new(*type_guid, guids[i + 1], *first, *last).with_name(name) else {
            send(REPORT, R_DISK_FAILED, 1, i as u64);
            user_mode_runtime::exit()
        };
        parts[i] = e;
    }

    // The disk's own size, asked of the disk rather than assumed. A layout that does not fit is a
    // refusal from `GloballyUniqueIdentifierPartitionTable::create`, not a truncated table.
    let size = call(BLK, req(blk::SIZE), 0).0 as i64;
    if size <= 0 || !(size as u64).is_multiple_of(LBA) {
        send(REPORT, R_DISK_FAILED, 2, size as u64);
        user_mode_runtime::exit()
    }
    let block_count = size as u64 / LBA;

    let array = array();
    let Ok(table) = GloballyUniqueIdentifierPartitionTable::create(
        guids[0],
        LBA as usize,
        block_count,
        &parts,
        array,
    ) else {
        send(REPORT, R_DISK_FAILED, 3, block_count);
        user_mode_runtime::exit()
    };

    // One logical block of scratch, built and written one at a time: the MBR, the primary header,
    // and the backup header are each 512 bytes at a known LBA, and the entry array is written
    // straight out of the buffer `GloballyUniqueIdentifierPartitionTable::create` filled.
    let mut block = [0u8; LBA as usize];
    let entries: &[u8] = table.entry_array();

    let steps: [(u64, Option<&[u8]>); 5] = [
        (0, None),                  // the protective MBR
        (PRIMARY_HEADER_LBA, None), // the primary header
        (table.primary_entry_lba(), Some(entries)),
        (table.backup_entry_lba(), Some(entries)),
        (table.backup_header_lba(), None), // the backup header, on the last block
    ];
    for (n, (lba, bytes)) in steps.iter().enumerate() {
        let data: &[u8] = match bytes {
            Some(b) => b,
            None => {
                block.fill(0);
                let built = match n {
                    0 => table.write_protective_mbr(&mut block),
                    1 => table.write_primary_header(&mut block),
                    _ => table.write_backup_header(&mut block),
                };
                if built.is_err() {
                    send(REPORT, R_DISK_FAILED, 4, n as u64);
                    user_mode_runtime::exit()
                }
                &block
            }
        };
        if !write_at(*lba, data) {
            send(REPORT, R_DISK_FAILED, 5, *lba);
            user_mode_runtime::exit()
        }
    }

    send(REPORT, R_PARTITIONED, blank::PARTITIONS as u64, 0);
    user_mode_runtime::exit()
}

/// Read the disk back and say what is on it. No entropy endpoint, no writes.
fn verify() -> ! {
    let mut flags = 0u64;
    let mut partitions = 0u64;
    let mut data_first_lba = 0u64;

    let size = call(BLK, req(blk::SIZE), 0).0 as i64;
    if size <= 0 || !(size as u64).is_multiple_of(LBA) {
        send(REPORT, 0, 0, 0);
        user_mode_runtime::exit()
    }
    let block_count = size as u64 / LBA;

    let head = primary();
    if !read_at(0, &mut head[..]) {
        send(REPORT, 0, 0, 0);
        user_mode_runtime::exit()
    }
    let head: &[u8] = head;

    if mbr::validate(&head[..LBA as usize], block_count).is_ok() {
        flags |= F_MBR;
    }
    let Ok(table) = GloballyUniqueIdentifierPartitionTable::parse(
        &head[LBA as usize..2 * LBA as usize],
        &head[2 * LBA as usize..],
    ) else {
        send(REPORT, flags, 0, 0);
        user_mode_runtime::exit()
    };
    flags |= F_PRIMARY;

    // The backup, read as its own I/O at the far end of the disk, which is what says whether the
    // two copies this program wrote actually agree on the platter.
    let backup_lba = table.backup_entry_lba();
    let backup_blocks = block_count.saturating_sub(backup_lba);
    let tail = backup();
    if backup_blocks > 1
        && let Some(span) = Span::covering(backup_lba * LBA, backup_blocks * LBA, TRANSFER)
        && span.blocks as usize * blk::BLOCK_SIZE <= tail.len()
        && read_span(span, &mut tail[..])
    {
        let array = span.offset;
        let header = array + (backup_blocks as usize - 1) * LBA as usize;
        let tail: &[u8] = tail;
        if table
            .check_backup(&tail[header..header + LBA as usize], &tail[array..header])
            .is_ok()
        {
            flags |= F_BACKUP;
        }
    }

    let mut named = 0u64;
    let mut unique = 0u64;
    let mut seen = [Uuid::nil(); blank::PARTITIONS];
    for (i, part) in table.partitions() {
        if partitions as usize >= blank::PARTITIONS {
            partitions += 1; // more entries than the layout has: the count below will say so
            continue;
        }
        if part.type_guid == types::NIFE_DATA {
            flags |= F_NIFE;
            data_first_lba = part.first_lba;
        }
        let mut label = [0u8; 4 * NAME_UNITS];
        if let Ok(n) = part.name_utf8(&mut label)
            && i < blank::NAMES.len()
            && &label[..n] == blank::NAMES[i].as_bytes()
        {
            named += 1;
        }
        // A unique GUID that is zero, repeated, or not stamped version 4 would mean the entropy
        // never arrived and something invented an id anyway, which is the failure this program's
        // whole shape exists to prevent.
        let g = part.unique_guid;
        let mut text = Uuid::encode_buffer();
        let text = g.hyphenated().encode_upper(&mut text).as_bytes();
        if !g.is_nil() && text[14] == b'4' && !seen[..partitions as usize].contains(&g) {
            unique += 1;
        }
        seen[partitions as usize] = g;
        partitions += 1;
    }
    if partitions == blank::PARTITIONS as u64 {
        if named == partitions {
            flags |= F_NAMES;
        }
        if unique == partitions {
            flags |= F_UNIQUE;
        }
    }

    send(REPORT, flags, partitions, data_first_lba);
    user_mode_runtime::exit()
}

/// Lay `data` over the disk starting at logical block `first_lba`, **preserving everything else in
/// the transfer blocks it touches**: each is read, patched in the shared page, and written back.
fn write_at(first_lba: u64, data: &[u8]) -> bool {
    let mut done = 0usize;
    while done < data.len() {
        let byte_at = first_lba * LBA + done as u64;
        let block = byte_at / TRANSFER;
        let offset = (byte_at % TRANSFER) as usize;
        let n = core::cmp::min(blk::BLOCK_SIZE - offset, data.len() - done);
        if (call(BLK, req(blk::READ), block).0 as i64) < 0 {
            return false;
        }
        // SAFETY: BLK_PAGE is a mapped, writable page of exactly one transfer block, and `offset +
        // n` is bounded by that block above.
        unsafe {
            core::ptr::copy_nonoverlapping(
                data.as_ptr().add(done),
                (BLK_PAGE as *mut u8).add(offset),
                n,
            );
        }
        if (call(BLK, req(blk::WRITE), block).0 as i64) < 0 {
            return false;
        }
        done += n;
    }
    true
}

/// Read `into.len()` bytes starting at logical block `first_lba`. `into` must be a whole number of
/// transfer blocks and `first_lba` must land on one.
fn read_at(first_lba: u64, into: &mut [u8]) -> bool {
    match Span::covering(first_lba * LBA, into.len() as u64, TRANSFER) {
        Some(span) if span.offset == 0 => read_span(span, into),
        _ => false,
    }
}

/// Fetch a [`Span`] of transfer blocks into `into`, one `CALL` per block, copying each out of the
/// shared page before the next request lands in it.
fn read_span(span: Span, into: &mut [u8]) -> bool {
    for i in 0..span.blocks {
        let at = i as usize * blk::BLOCK_SIZE;
        if at + blk::BLOCK_SIZE > into.len() {
            return false;
        }
        if (call(BLK, req(blk::READ), span.first_block + i).0 as i64) < 0 {
            return false;
        }
        // SAFETY: BLK_PAGE is a mapped page of exactly one block, and the destination window was
        // bounds-checked against `into` on the line above.
        unsafe {
            core::ptr::copy_nonoverlapping(
                BLK_PAGE as *const u8,
                into.as_mut_ptr().add(at),
                blk::BLOCK_SIZE,
            );
        }
    }
    true
}

/// The entry-array buffer.
///
/// # Safety
/// This program has one thread and cannot be given a second: it is built without its own process,
/// so nothing holds the `BIND` that adds a thread to it (milestone 812 (`std::thread::spawn` runs
/// real threads in one address space)). So there is no second reference. Taking the raw pointer
/// first is what `static_mut_refs` asks for.
fn array() -> &'static mut [u8] {
    let p = &raw mut ARRAY;
    // SAFETY: see above.
    unsafe { &mut *p }
}

/// The primary-table read buffer. Same reasoning as [`array()`].
fn primary() -> &'static mut [u8] {
    let p = &raw mut PRIMARY;
    // SAFETY: see above.
    unsafe { &mut *p }
}

/// The backup-table read buffer. Same reasoning as [`array()`].
fn backup() -> &'static mut [u8] {
    let p = &raw mut BACKUP;
    // SAFETY: see above.
    unsafe { &mut *p }
}

user_mode_runtime::panic_handler!();
