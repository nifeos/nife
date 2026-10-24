//! `disk_surveyor`: what drives are attached, and how the one we hold is divided up
//! (milestone 57, notes/block-devices.md).
//!
//! It holds a `READ`-only `PageFrame` capability on the roster page (`block_roster`), so no
//! writable mapping of the listing can be obtained (see [`probe`]), and a block-service endpoint
//! for one device (`filesystem_contract::blk`). It was handed one disk: no path to type, and
//! nothing in the roster turns into a handle. It reads LBA 1, parses the GUID Partition Table,
//! checks the backup at the far end against it, and reports. Every byte of judging is
//! `crates/globally_unique_identifier_partition_table`; this file is the I/O.

//! # The roles
//!
//! | `a0` | |
//! |---|---|
//! | [`ROLE_SURVEY`] | enumerate, read the table, report |
//! | [`ROLE_PROBE`] | announce the roster's address, write to it, and die |
//! | [`ROLE_HOLDER`] | map the roster, then fault on it once the kernel revokes the frame |
//!
//! [`ROLE_PROBE`] is the negative control: **the roster is a listing, not a lever**, and the
//! kernel-side test watches the fault land at the address it announced.

//! # BUGS
//!
//! - **The logical block size is assumed to be 512.** The block service does not carry the
//!   device's logical block size on the wire, so a 4Kn disk would be read as though its LBA 1 were
//!   at byte 512: a wrong answer rather than an error. The fix is a field in
//!   `filesystem_contract::blk`, not a change here.
//! - **It does not write.** Writing the table is `components/src/disk_partitioner.rs`.
//! - **No hot plug.** The roster is written once at wiring time and never again.
//!
//! Name: ratified 2026-08-03 (calef, milestone 57 (partitioning and formatting a real drive)). A
//! surveyor maps land they do not own, which is this program's endowment: a read-only roster of
//! what devices exist, plus one device's endpoint.

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

use block_roster::{
    Roster, TRANSPORT_NON_VOLATILE_MEMORY_EXPRESS, TRANSPORT_VIRTIO_MEMORY_MAPPED_IO,
    TRANSPORT_VIRTIO_PCI,
};
use filesystem_contract::{blk, req};
use globally_unique_identifier_partition_table::entry::NAME_UNITS;
use globally_unique_identifier_partition_table::guid::types;
use globally_unique_identifier_partition_table::span::Span;
use globally_unique_identifier_partition_table::{GloballyUniqueIdentifierPartitionTable, mbr};
use user_mode_runtime::mapped_window::MappedWindow;
use user_mode_runtime::{call, send};

/// Slot 0: where the verdict goes. An endpoint with `WRITE`.
const REPORT: u64 = 0;
/// Slot 1: the block service for the one disk this program was handed, `WRITE` (it `CALL`s).
/// Empty in [`ROLE_PROBE`], which holds no disk.
const BLK: u64 = 1;
/// Slot 2: the untyped this program spends on the page tables its own mappings need.
const BUDGET: u64 = 2;
/// Slot 3: the page shared with the block server, `READ|WRITE` (a transfer goes both ways). Empty
/// in [`ROLE_PROBE`].
const BLK_PAGE_FRAME: u64 = 3;
/// Slot 4: the roster page, **`READ` and nothing else**. See [`ROLE_PROBE`].
const ROSTER_PAGE_FRAME: u64 = 4;
/// Slot 5: the go-ahead endpoint, `READ`. Only [`ROLE_HOLDER`] has one.
const RESUME: u64 = 5;

/// Where this program puts the page it shares with the block server. **Its choice, not the
/// kernel's**: it holds the frame and maps it, so the address is a local decision and nothing on
/// the kernel side names it.
const BLK_PAGE: u64 = address_space_map::service_window(0x5000_0000);
/// Where this program puts the roster. `kernel/src/user/disk_service.rs` knows this one, because
/// the probe's fault has to be asserted at an address the test can name.
const ROSTER_VA: u64 = address_space_map::service_window(0x5001_0000);

/// The transfer unit of the block service: one filesystem block per request.
const TRANSFER: u64 = blk::BLOCK_SIZE as u64;

/// The logical block size a GPT counts in. See BUGS: nothing on the wire reports the device's, and
/// every disk this project has met uses this one.
const LBA: u64 = 512;

/// The primary table: LBA 0 through LBA 33, which is the protective MBR, the header and the whole
/// 16 KiB entry array. Five transfer blocks cover it.
const PRIMARY_BYTES: usize = 5 * blk::BLOCK_SIZE;
/// The backup table: 33 logical blocks ending on the last block of the disk. Where that starts
/// inside a transfer block depends on the disk's size, so six blocks is the worst case.
const BACKUP_BYTES: usize = 6 * blk::BLOCK_SIZE;

/// The buffers live in `.bss`. A user process gets one 4 KiB page of stack (`USER_STACK_VA`), and
/// these are forty-four kilobytes between them.
static mut PRIMARY: [u8; PRIMARY_BYTES] = [0; PRIMARY_BYTES];
static mut BACKUP: [u8; BACKUP_BYTES] = [0; BACKUP_BYTES];

// The roles, in `a0`. Must match kernel/src/user/disk_service.rs.
const ROLE_SURVEY: u64 = 0;
const ROLE_PROBE: u64 = 1;
const ROLE_HOLDER: u64 = 2;

// The report's first word: what this program managed to establish. Must match
// system_tests/src/user/disk_tests.rs.
/// The roster page read as a roster (right magic, believable count).
pub const F_ROSTER: u64 = 1 << 0;
/// The block service answered `SIZE` with a size that is a whole number of logical blocks.
pub const F_SIZE: u64 = 1 << 1;
/// LBA 0 holds a well-formed protective MBR covering the whole disk.
pub const F_MBR: u64 = 1 << 2;
/// The primary header and entry array parsed, CRCs and geometry included.
pub const F_PRIMARY: u64 = 1 << 3;
/// The backup table at the far end of the disk agrees with the primary.
pub const F_BACKUP: u64 = 1 << 4;
/// A partition of type `NIFE_DATA` is on the disk (DECISIONS §45).
pub const F_NIFE: u64 = 1 << 5;
/// Every partition's name decoded as UTF-8 (`sgdisk` writes them; macOS does not,
/// notes/globally-unique-identifier-partition-table.md).
pub const F_NAMES: u64 = 1 << 6;

/// The probe's announcement, so a fault that never reached the write is distinguishable from the
/// write being refused. Must match `system_tests/src/user/disk_tests.rs`.
pub const R_PROBING: u64 = 0x50_0B_11_46;

/// The probe's second word: `PageFrame::MAP` refused it a writable window on the roster, one rung
/// before the page permissions would have. Must match `system_tests/src/user/disk_tests.rs`.
pub const P_RW_REFUSED: u64 = 1 << 0;

/// [`ROLE_HOLDER`]'s announcement: it has the roster mapped and here is the first word of it.
/// ASCII `HOLDG`. Must match `system_tests/src/user/disk_tests.rs`.
pub const R_HOLDING: u64 = 0x_48_4F_4C_44_47;

#[unsafe(no_mangle)]
pub extern "C" fn _start(role: u64, _a1: u64, _a2: u64) -> ! {
    match role {
        ROLE_SURVEY => survey(),
        ROLE_PROBE => probe(),
        ROLE_HOLDER => holder(),
        _ => panic!(),
    }
}

/// **Hold the roster, then have it taken away** (milestone 108).
///
/// Maps the frame, reports its first word, waits for a go-ahead, and reads again. Between the two
/// reads the kernel revokes the frame, and a page mapped from a revoked capability goes away, so
/// the second read faults. A page wired in at spawn is in no mapping record and `PageFrame::REVOKE`
/// cannot reach it, which is why the roster is mapped from a capability (notes/frames.md).
fn holder() -> ! {
    if !user_mode_runtime::map_page_frame(ROSTER_PAGE_FRAME, ROSTER_VA, false, BUDGET) {
        user_mode_runtime::exit()
    }
    // SAFETY: ROSTER_VA is mapped read-only from the frame in slot 4, on the line above: the
    // program's own `PageFrame::MAP` succeeded first, which is exactly the case
    // `user_mode_runtime::mapped_window::MappedWindow::new`'s own doc names (milestone 139 round 3).
    let roster = unsafe { MappedWindow::new(ROSTER_VA, filesystem_contract::PAGE as u64) };
    let word = roster.read::<u64>(0);
    send(REPORT, R_HOLDING, word, 0);

    // The kernel revokes the frame while we are parked here.
    user_mode_runtime::receive(RESUME);

    // Not safe any more, and that is the test: the page is gone. This is the one deliberate
    // exception to `new`'s "stays mapped for as long as `self` is used" contract, and it is the
    // whole point of this role (see the module doc above). `MappedWindow`'s own bounds check does
    // not (and cannot) catch it: offset 0 is inside the 4 KiB window `roster` was told about, so
    // the check passes and the real hardware fault happens inside `read`, at the exact volatile
    // access the hand-written version used to make. Nothing about the fault mechanism changed.
    let after = roster.read::<u64>(0);
    // Unreachable in a working kernel. If the read did land, say so, so a silent pass is
    // impossible: the test asserts on the fault, and this message is what it sees instead.
    send(REPORT, R_HOLDING, after, 1);
    user_mode_runtime::exit()
}

/// Enumerate, read the table, report. Two messages: the roster, then the table.
fn survey() -> ! {
    // **Put our own pages in our own address space** (milestone 108). Both are capabilities we
    // hold; the tables to reach them come out of our own budget. A failure here is fatal and
    // silent on purpose: this program's only channel is `REPORT`, and reporting through a page we
    // failed to map is not a thing it can do.
    if !user_mode_runtime::map_page_frame(BLK_PAGE_FRAME, BLK_PAGE, true, BUDGET)
        || !user_mode_runtime::map_page_frame(ROSTER_PAGE_FRAME, ROSTER_VA, false, BUDGET)
    {
        user_mode_runtime::exit()
    }

    // The roster first, because it is the cheaper authority and answers a different question.
    // A page that is not a roster reports as no devices AND no F_ROSTER, so "nobody wired me a
    // roster" never reads as "this machine has no disks" (DECISIONS §42).
    //
    // One count per kind and no total: `SEND` carries three words, and the kernel side compares
    // their sum with what it wrote, so an entry of a kind this program does not count still shows.
    let (roster_ok, mmio, pci, nvme) = match Roster::read(roster_page()) {
        Some(r) => (
            F_ROSTER,
            r.count_on(TRANSPORT_VIRTIO_MEMORY_MAPPED_IO) as u64,
            r.count_on(TRANSPORT_VIRTIO_PCI) as u64,
            r.count_on(TRANSPORT_NON_VOLATILE_MEMORY_EXPRESS) as u64,
        ),
        None => (0, 0, 0, 0),
    };
    send(REPORT, mmio, pci, nvme);

    let mut flags = roster_ok;
    let mut partitions = 0u64;
    let mut nife_first_lba = 0u64;

    // How big is the disk? This is the question only a *holder* can ask: the roster deliberately
    // carries no capacity, because a size is a fact about the device rather than about the machine
    // having one (crates/block_roster).
    let size = call(BLK, req(blk::SIZE), 0).0 as i64;
    if size > 0 && (size as u64).is_multiple_of(LBA) {
        flags |= F_SIZE;
        let block_count = size as u64 / LBA;
        flags |= read_table(block_count, &mut partitions, &mut nife_first_lba);
    }

    send(REPORT, flags, partitions, nife_first_lba);
    user_mode_runtime::exit()
}

/// Read and judge both halves of the table. Returns the flags it established.
fn read_table(block_count: u64, partitions: &mut u64, nife_first_lba: &mut u64) -> u64 {
    let mut flags = 0;

    // LBA 0..=33 in one run: the protective MBR, the primary header, and the whole entry array.
    let Some(head) = Span::covering(0, 34 * LBA, TRANSFER) else {
        return flags;
    };
    let head_buf = primary();
    if !read_span(head, &mut head_buf[..]) {
        return flags;
    }
    // Reborrowed immutably from here on, so this and the backup buffer can be live at once.
    let primary: &[u8] = head_buf;

    if mbr::validate(&primary[..LBA as usize], block_count).is_ok() {
        flags |= F_MBR;
    }
    let Ok(table) = GloballyUniqueIdentifierPartitionTable::parse(
        &primary[LBA as usize..2 * LBA as usize],
        &primary[2 * LBA as usize..],
    ) else {
        return flags;
    };
    flags |= F_PRIMARY;

    // The backup: the entry array plus the header, ending on the last block of the disk. Reading it
    // is a second I/O on purpose (`GloballyUniqueIdentifierPartitionTable::check_backup` is
    // separate for exactly this reason), and it is the check that says whether anything has
    // happened to this disk since it was written.
    let backup_lba = table.backup_entry_lba();
    let backup_blocks = block_count.saturating_sub(backup_lba);
    let tail_buf = backup();
    if backup_blocks > 1
        && let Some(tail) = Span::covering(backup_lba * LBA, backup_blocks * LBA, TRANSFER)
        && tail.blocks as usize * blk::BLOCK_SIZE <= BACKUP_BYTES
        && read_span(tail, &mut tail_buf[..])
    {
        // The backup array first, then the backup header on the very last logical block.
        let array = tail.offset;
        let array_bytes = (backup_blocks as usize - 1) * LBA as usize;
        let header = array + array_bytes;
        let tail_buf: &[u8] = tail_buf;
        if table
            .check_backup(
                &tail_buf[header..header + LBA as usize],
                &tail_buf[array..header],
            )
            .is_ok()
        {
            flags |= F_BACKUP;
        }
    }

    let mut named = 0;
    for (_, part) in table.partitions() {
        *partitions += 1;
        if part.type_guid == types::NIFE_DATA {
            flags |= F_NIFE;
            *nife_first_lba = part.first_lba;
        }
        let mut label = [0u8; 4 * NAME_UNITS];
        if part.name_utf8(&mut label).is_ok() {
            named += 1;
        }
    }
    if named == *partitions && *partitions > 0 {
        flags |= F_NAMES;
    }
    flags
}

/// Fetch a [`Span`] of transfer blocks into `into`, one `CALL` per block. Every block lands in the
/// page shared with the block server and is copied out before the next request.
fn read_span(span: Span, into: &mut [u8]) -> bool {
    for i in 0..span.blocks {
        let at = i as usize * blk::BLOCK_SIZE;
        if at + blk::BLOCK_SIZE > into.len() {
            return false;
        }
        if (call(BLK, req(blk::READ), span.first_block + i).0 as i64) < 0 {
            return false;
        }
        // SAFETY: BLK_PAGE is a page this program mapped from a frame it holds, of exactly one
        // block, and the destination window was bounds-checked against `into` on the line above.
        unsafe {
            core::ptr::copy_nonoverlapping(
                BLK_PAGE as *const u8,
                into.as_mut_ptr().add(at),
                blk::BLOCK_SIZE,
            );
        };
    }
    true
}

/// **The negative control**, at two rungs.
///
/// It asks `PageFrame::MAP` for a writable window on the roster, which the kernel refuses because
/// the frame carries `READ` alone. Then it maps the page read-only, announces the address, and
/// writes there anyway, which faults. Knowing the address buys nothing: the boundary is a
/// capability's rights and then a page's permissions, not a check this program could skip.
fn probe() -> ! {
    // Rung one: a writable mapping, refused by the rights on the capability itself.
    let rw_refused = !user_mode_runtime::map_page_frame(ROSTER_PAGE_FRAME, ROSTER_VA, true, BUDGET);

    // Rung two: the read-only mapping we are entitled to, and a write through it.
    if !user_mode_runtime::map_page_frame(ROSTER_PAGE_FRAME, ROSTER_VA, false, BUDGET) {
        user_mode_runtime::exit()
    }
    send(
        REPORT,
        R_PROBING,
        ROSTER_VA,
        if rw_refused { P_RW_REFUSED } else { 0 },
    );
    // SAFETY: ROSTER_VA is mapped read-only from the frame in slot 4, above (the program's own
    // `PageFrame::MAP` succeeded). Not safe to write through, and that is the test: as in `holder`'s
    // second read, `MappedWindow`'s bounds check passes (offset 0 is inside the window) and the
    // kernel refuses the write itself, at the same volatile access the hand-written version made.
    let roster = unsafe { MappedWindow::new(ROSTER_VA, filesystem_contract::PAGE as u64) };
    roster.write(0u64, 0u64);
    // Unreachable in a working kernel; if we get here the write was allowed and the test's
    // fault-count assertion is what says so.
    user_mode_runtime::exit()
}

/// The roster page, read-only. Its contents are checked by `block_roster::Roster::read`, which
/// refuses a page nobody wrote and a count larger than the page can hold.
fn roster_page() -> &'static [u8] {
    // SAFETY: a page this program mapped read-only from a frame it holds `READ` on, of exactly
    // `filesystem_contract::PAGE` bytes, and never written by anybody after that (the roster is built once at
    // wiring time).
    unsafe { core::slice::from_raw_parts(ROSTER_VA as *const u8, filesystem_contract::PAGE) }
}

/// The primary table buffer.
///
/// # Safety
/// This program has one thread and cannot be given a second: it is built without its own process,
/// so nothing holds the `BIND` that adds a thread to it (milestone 812 (`std::thread::spawn` runs
/// real threads in one address space)). So there is no second reference. The raw pointer first is
/// what `static_mut_refs` asks for, and it asks for a real reason.
fn primary() -> &'static mut [u8] {
    let p = &raw mut PRIMARY;
    // SAFETY: see above.
    unsafe { &mut *p }
}

/// The backup table buffer. Same reasoning as [`primary`].
fn backup() -> &'static mut [u8] {
    let p = &raw mut BACKUP;
    // SAFETY: see above.
    unsafe { &mut *p }
}

user_mode_runtime::panic_handler!();
