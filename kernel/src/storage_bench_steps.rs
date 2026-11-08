//! The storage bench's steps that need no hardware of their own: the read-only probe and the
//! scratch write test, written over any `RegisterWindowAndClock`, so the kernel's unit-test image
//! runs them against a fake card on every architecture and their order of operations, which is the
//! safety argument, is tested before it is first tried on the card the board boots from.
//! [`run_read_only_probe`] identifies the card, reads sector 0, parses the MBR, reads the first partition's
//! boot sector and times a sequential read from its start; it sends no command that writes.
//! [`run_scratch_write_test`] writes only inside `MasterBootRecord::scratch_sector_range`'s range, after reading what
//! is there: a pattern, read back, then the original, read back too. A card with no gap is refused,
//! not written. Only `storage_bench.rs` (riscv64, behind the `storage_bench` feature) calls them
//! outside a test; it imports this module as `bench`.
//!
//! Name: provisional. `bench` is the ratified word for the physical test bench (#3135), and the
//! module that holds these steps for the bench is named for that role, `storage_bench_steps`.

use designware_mobile_storage::host_controller::{HostController, RegisterWindowAndClock};
use designware_mobile_storage::internal_direct_memory_access::DirectMemoryAccessRegion;
use designware_mobile_storage::partition::MasterBootRecord;
#[cfg(feature = "storage_read_matrix")]
use designware_mobile_storage::registers::{
    STATUS_REGISTER, TRANSFERRED_CARD_BYTE_COUNT_REGISTER, TRANSFERRED_HOST_BYTE_COUNT_REGISTER,
};
use designware_mobile_storage::sd::{
    self, BLOCK_BYTES, BlockTransferError, Card, InitializationError,
};

#[cfg(feature = "storage_read_matrix")]
/// **Raw Interrupt Status Register** (OpenBSD's `SDMMC_RINTSTS`): write one to clear. The matrix
/// reads it straight after each read, for its transcript.
const RAW_INTERRUPT_STATUS_REGISTER: u32 = 0x044;

#[cfg(feature = "storage_read_matrix")]
/// **FIFO full** (`STATUS_FIFO_FULL`, the databook's `fifo_full`, which `dwmmc-host` keeps): the
/// FIFO is full.
const STATUS_QUEUE_FULL: u32 = 1 << 3;

#[cfg(feature = "storage_read_matrix")]
/// **FIFO count** (OpenBSD's `SDMMC_STATUS_FIFO_COUNT`, the databook's `fifo_count`): how many FIFO
/// words are filled, bits 29:17 of `STATUS_REGISTER`.
const fn queue_count(status: u32) -> u32 {
    (status >> 17) & 0x1fff
}

/// The first used partition's start, or `None` when nothing is partitioned.
fn first_partition_start_of(master_boot_record: MasterBootRecord) -> Option<u32> {
    master_boot_record
        .entries
        .iter()
        .filter(|e| e.is_used())
        .map(|e| e.start)
        .min()
}

/// How many sectors the write test uses at most: 4 KiB, one multi-block command each way.
///
/// Name: ratified 2026-11-03 (UTC), #3141. The most sectors the write test uses; the maximum is spelled out. Renamed from `SCRATCH_SECTORS`.
pub const MAXIMUM_SCRATCH_SECTORS: u32 = 8;

/// What the read-only probe found.
///
/// Name: ratified 2026-11-03 (UTC), #3141. What the read-only probe found, named for the probe that returns it. Renamed from `ReadReport`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadOnlyProbeReport {
    /// The card, identified.
    pub card: Card,
    /// Sector 0, parsed, if it holds an MBR.
    #[doc(alias = "mbr")]
    pub master_boot_record: Option<MasterBootRecord>,
    /// The first partition's first sector ends in `55 aa` (a FAT boot sector does).
    pub partition_boot_signature: bool,
    /// The first partition's first sector's bytes 82..90 (FAT32's `"FAT32   "` file system
    /// type), for the transcript.
    pub file_system_type: [u8; 8],
    /// The block the timed read started at: the first partition's, or 0 without one.
    pub timed_from: u64,
    /// Blocks read in the timed sequential read.
    pub timed_blocks: u64,
    /// Microseconds the timed read took.
    pub timed_microseconds: u64,
}

/// Where the read-only probe stopped.
///
/// Name: ratified 2026-11-03 (UTC), #3141. Where the read-only probe stopped, named for the probe. Renamed from `ReadFailure`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadOnlyProbeFailure {
    /// Identification failed.
    Initialization(InitializationError),
    /// A read failed: the block and the error.
    Read(u64, BlockTransferError),
}

impl core::fmt::Display for ReadOnlyProbeFailure {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Initialization(e) => write!(f, "identification failed: {e}"),
            Self::Read(block, e) => write!(f, "reading block {block} failed: {e}"),
        }
    }
}

impl core::error::Error for ReadOnlyProbeFailure {}

/// **The read-only probe.** `buffer` is the transfer buffer (a whole number of blocks; the timed
/// read moves `buffer.len()` bytes per command), `timed_blocks` how many blocks to time.
///
/// # Errors
///
/// The first step that failed.
///
/// Name: ratified 2026-11-03 (UTC), #3141. The function that runs the read-only probe; a verb phrase says that it acts on the card. Renamed from `read_only`.
pub fn run_read_only_probe<R: RegisterWindowAndClock, D: DirectMemoryAccessRegion>(
    host: &mut HostController<R, D>,
    bus_width: u8,
    buffer: &mut [u8],
    timed_blocks: u64,
) -> Result<ReadOnlyProbeReport, ReadOnlyProbeFailure> {
    let card = sd::identify(host, bus_width).map_err(ReadOnlyProbeFailure::Initialization)?;
    let mut sector = [0u8; BLOCK_BYTES];
    sd::read_blocks(host, &card, 0, &mut sector).map_err(|e| ReadOnlyProbeFailure::Read(0, e))?;
    let master_boot_record = MasterBootRecord::parse(&sector);
    let first = master_boot_record
        .and_then(first_partition_start_of)
        .map_or(0, u64::from);
    let mut partition_boot_signature = false;
    let mut file_system_type = [0u8; 8];
    if first != 0 {
        sd::read_blocks(host, &card, first, &mut sector)
            .map_err(|e| ReadOnlyProbeFailure::Read(first, e))?;
        partition_boot_signature = sector[510..512] == [0x55, 0xaa];
        file_system_type.copy_from_slice(&sector[82..90]);
    }
    let (timed, timed_microseconds) = timed_read(host, &card, first, buffer, timed_blocks)?;
    Ok(ReadOnlyProbeReport {
        card,
        master_boot_record,
        partition_boot_signature,
        file_system_type,
        timed_from: first,
        timed_blocks: timed,
        timed_microseconds,
    })
}

/// **The timed sequential read**: up to `timed_blocks` blocks from `first`, `buffer.len()` bytes a
/// command, stopping before the card's end. Returns the blocks read and the microseconds taken.
fn timed_read<R: RegisterWindowAndClock, D: DirectMemoryAccessRegion>(
    host: &mut HostController<R, D>,
    card: &Card,
    first: u64,
    buffer: &mut [u8],
    timed_blocks: u64,
) -> Result<(u64, u64), ReadOnlyProbeFailure> {
    let per = (buffer.len() / BLOCK_BYTES) as u64;
    let mut timed = 0u64;
    let start = host.registers_mut().now_microseconds();
    if per > 0 {
        while timed < timed_blocks && first + timed + per <= card.blocks {
            sd::read_blocks(host, card, first + timed, buffer)
                .map_err(|e| ReadOnlyProbeFailure::Read(first + timed, e))?;
            timed += per;
        }
    }
    Ok((
        timed,
        host.registers_mut().now_microseconds().wrapping_sub(start),
    ))
}

/// What the IDMAC's half of the read-only probe found.
///
/// Name: ratified 2026-11-02 (UTC), #3092. `idmac` and the names built on it spell out
/// `internal_direct_memory_access`; the databook's `idmac` is kept as `doc(alias = "idmac")`.
#[doc(alias = "idmac")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InternalDirectMemoryAccessReport {
    /// The first transfer's bytes are the polled host's for the same blocks.
    pub matches: bool,
    /// Blocks read in the timed sequential read.
    pub timed_blocks: u64,
    /// Microseconds the timed read took.
    pub timed_microseconds: u64,
}

/// **The IDMAC's half of the read-only probe**, on a host
/// `HostController::with_internal_direct_memory_access` made from the one [`run_read_only_probe`] ran on and reported
/// `report`. `reference` is the polled host's read of `reference.len()` bytes from
/// `report.timed_from`; the same blocks are read through the IDMAC and compared, then
/// [`run_read_only_probe`]'s timed read runs again from there in `buffer.len()`-byte commands, so the two
/// rates differ only in the data path.
///
/// # Errors
///
/// The read that failed.
///
/// Name: ratified 2026-11-02 (UTC), #3092. `idmac` and the names built on it spell out
/// `internal_direct_memory_access`; the databook's `idmac` is kept as `doc(alias = "idmac")`.
#[doc(alias = "idmac")]
pub fn internal_direct_memory_access_read<
    R: RegisterWindowAndClock,
    D: DirectMemoryAccessRegion,
>(
    host: &mut HostController<R, D>,
    report: &ReadOnlyProbeReport,
    reference: &[u8],
    buffer: &mut [u8],
    timed_blocks: u64,
) -> Result<InternalDirectMemoryAccessReport, ReadOnlyProbeFailure> {
    let (card, first) = (&report.card, report.timed_from);
    let n = reference.len().min(buffer.len());
    sd::read_blocks(host, card, first, &mut buffer[..n])
        .map_err(|e| ReadOnlyProbeFailure::Read(first, e))?;
    // An empty reference never gets here: a read of no blocks is refused as `Length`.
    let matches = buffer[..n] == reference[..n];
    let (timed_blocks, timed_microseconds) = timed_read(host, card, first, buffer, timed_blocks)?;
    Ok(InternalDirectMemoryAccessReport {
        matches,
        timed_blocks,
        timed_microseconds,
    })
}

/// The byte the write test puts at offset `i` of the sector at `logical_block_address`: never
/// constant across a sector, different for each sector, so a write that lands one sector off or
/// repeats a block is seen on readback.
///
/// Name: ratified 2026-11-03 (UTC), #3141. The byte of the pattern the write test puts in a scratch sector. Renamed from `pattern`.
#[must_use]
pub const fn scratch_pattern_byte(logical_block_address: u64, i: usize) -> u8 {
    (i as u8).wrapping_mul(31)
        ^ (logical_block_address as u8).wrapping_mul(7)
        ^ 0x5a
        ^ (i >> 8) as u8
}

/// How the scratch write test ended.
///
/// Name: ratified 2026-11-03 (UTC), #3141. The verdict of the scratch write test, named for the test. Renamed from `WriteVerdict`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScratchWriteVerdict {
    /// No MBR, a GPT disk, or no gap before the first partition: nothing was written.
    Refused,
    /// The pattern was written, read back intact, and the original contents written back and read
    /// back intact: `(first sector, count, the original was all zero)`.
    Verified(u32, u32, bool),
    /// The pattern did not read back: the first wrong sector. The original was written back.
    PatternMismatch(u64),
    /// The original did not read back after it was restored: the first wrong sector. This is the
    /// one verdict that leaves the scratch range changed, and it is inside the gap no partition
    /// owns.
    RestoreMismatch(u64),
    /// A command failed: which phase (0 read original, 1 write pattern, 2 read pattern, 3 write
    /// original, 4 read original back) and the error.
    Failed(u8, BlockTransferError),
}

/// **The scratch write test.** `master_boot_record` is what [`run_read_only_probe`] parsed from this card;
/// `original` and `scratch` are two buffers of at least [`MAXIMUM_SCRATCH_SECTORS`] blocks.
///
/// Name: ratified 2026-11-03 (UTC), #3141. The function that runs the scratch write test; a verb phrase says that it acts on the card. Renamed from `scratch_write`.
pub fn run_scratch_write_test<R: RegisterWindowAndClock, D: DirectMemoryAccessRegion>(
    host: &mut HostController<R, D>,
    card: &Card,
    master_boot_record: Option<&MasterBootRecord>,
    original: &mut [u8],
    scratch: &mut [u8],
) -> ScratchWriteVerdict {
    let Some((first, count)) =
        master_boot_record.and_then(|m| m.scratch_sector_range(MAXIMUM_SCRATCH_SECTORS))
    else {
        return ScratchWriteVerdict::Refused;
    };
    let bytes = count as usize * BLOCK_BYTES;
    if original.len() < bytes || scratch.len() < bytes {
        return ScratchWriteVerdict::Failed(0, BlockTransferError::Length);
    }
    let logical_block_address = u64::from(first);
    let (original, scratch) = (&mut original[..bytes], &mut scratch[..bytes]);
    if let Err(e) = sd::read_blocks(host, card, logical_block_address, original) {
        return ScratchWriteVerdict::Failed(0, e);
    }
    let blank = original.iter().all(|&b| b == 0);
    for (s, block) in scratch
        .as_chunks_mut::<BLOCK_BYTES>()
        .0
        .iter_mut()
        .enumerate()
    {
        for (i, b) in block.iter_mut().enumerate() {
            *b = scratch_pattern_byte(logical_block_address + s as u64, i);
        }
    }
    let mut verdict = None;
    if let Err(e) = sd::write_blocks(host, card, logical_block_address, scratch) {
        verdict = Some(ScratchWriteVerdict::Failed(1, e));
    } else {
        let mut back = [0u8; BLOCK_BYTES];
        for s in 0..u64::from(count) {
            if let Err(e) = sd::read_blocks(host, card, logical_block_address + s, &mut back) {
                verdict = Some(ScratchWriteVerdict::Failed(2, e));
                break;
            }
            let at = s as usize * BLOCK_BYTES;
            if back[..] != scratch[at..at + BLOCK_BYTES] {
                verdict = Some(ScratchWriteVerdict::PatternMismatch(
                    logical_block_address + s,
                ));
                break;
            }
        }
    }
    // Whatever happened, put back what was there.
    if let Err(e) = sd::write_blocks(host, card, logical_block_address, original) {
        return ScratchWriteVerdict::Failed(3, e);
    }
    if let Err(e) = sd::read_blocks(host, card, logical_block_address, scratch) {
        return ScratchWriteVerdict::Failed(4, e);
    }
    if let Some(s) = scratch
        .as_chunks::<BLOCK_BYTES>()
        .0
        .iter()
        .zip(original.as_chunks::<BLOCK_BYTES>().0)
        .position(|(a, b)| a != b)
    {
        return ScratchWriteVerdict::RestoreMismatch(logical_block_address + s as u64);
    }
    verdict.unwrap_or(ScratchWriteVerdict::Verified(first, count, blank))
}

#[cfg(feature = "storage_read_matrix")]
/// **A register window that remembers how full the queue got**: every `STATUS_REGISTER` read updates the
/// highest fill seen and counts the reads that found the queue full. It answers the multi-block
/// read diagnosis: whether the polled CPU falls behind the card.
///
/// Name: ratified 2026-11-02 (UTC), #3092. `fifo` becomes `queue` in every name this tree authors;
/// the databook's `fifo` is kept as `doc(alias = "fifo")`.
#[doc(alias = "fifo")]
pub struct QueueWatch<R: RegisterWindowAndClock> {
    inner: R,
    /// The highest `STATUS_REGISTER` queue count seen since the last [`QueueWatch::clear`].
    pub maximum_fill: u32,
    /// `STATUS_REGISTER` reads that found the FIFO-full flag set since the last [`QueueWatch::clear`].
    pub full_reads: u32,
}

#[cfg(feature = "storage_read_matrix")]
impl<R: RegisterWindowAndClock> QueueWatch<R> {
    /// Watch the queue of `inner`.
    ///
    /// Name: ratified 2026-11-02 (UTC), #3092. The constructor of the queue watch.
    pub const fn new(inner: R) -> Self {
        Self {
            inner,
            maximum_fill: 0,
            full_reads: 0,
        }
    }

    /// Start counting again.
    ///
    /// Name: ratified 2026-11-02 (UTC), #3092. The method that resets both counters.
    pub fn clear(&mut self) {
        self.maximum_fill = 0;
        self.full_reads = 0;
    }
}

#[cfg(feature = "storage_read_matrix")]
impl<R: RegisterWindowAndClock> RegisterWindowAndClock for QueueWatch<R> {
    fn read(&mut self, offset: u32) -> u32 {
        let v = self.inner.read(offset);
        if offset == STATUS_REGISTER {
            self.maximum_fill = self.maximum_fill.max(queue_count(v));
            if v & STATUS_QUEUE_FULL != 0 {
                self.full_reads += 1;
            }
        }
        v
    }
    fn write(&mut self, offset: u32, value: u32) {
        self.inner.write(offset, value);
    }
    fn now_microseconds(&mut self) -> u64 {
        self.inner.now_microseconds()
    }
}

#[cfg(feature = "storage_read_matrix")]
/// One multi-block read of the matrix: its card clock, length, outcome and what the queue did.
///
/// Name: ratified 2026-11-02 (UTC), #3092. One row of the matrix of multi-block reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MatrixRow {
    /// The card clock asked for, in hertz.
    ///
    /// Name: ratified 2026-11-05 (UTC), #3276. The unit is spelled out.
    pub asked_hertz: u32,
    /// The card clock the divider gave, in hertz.
    ///
    /// Name: ratified 2026-11-05 (UTC), #3276. The unit is spelled out.
    pub card_hertz: u32,
    /// Blocks in the one `CMD18`.
    pub blocks: u32,
    /// What the read returned.
    pub result: Result<(), BlockTransferError>,
    /// Microseconds the read took.
    pub microseconds: u64,
    /// The highest queue fill seen during it, in words.
    pub maximum_fill: u32,
    /// `STATUS_REGISTER` reads that found the queue full.
    pub full_reads: u32,
    /// `STATUS_REGISTER`, `RINTSTS`, `TRANSFERRED_CARD_BYTE_COUNT_REGISTER` and `TRANSFERRED_HOST_BYTE_COUNT_REGISTER` read straight after it.
    pub after: [u32; 4],
}

#[cfg(feature = "storage_read_matrix")]
/// The card clocks the matrix reads at: the transfer rate and two halvings of it.
///
/// Name: ratified 2026-11-05 (UTC), #3276. The card clocks of the matrix; the unit is spelled out as
/// `HERTZ`.
pub const MATRIX_HERTZ: [u32; 3] = [
    sd::TRANSFER_HERTZ,
    sd::TRANSFER_HERTZ / 2,
    sd::TRANSFER_HERTZ / 4,
];
#[cfg(feature = "storage_read_matrix")]
/// The `CMD18` lengths the matrix reads, in blocks.
///
/// Name: ratified 2026-11-02 (UTC), #3092. The `CMD18` lengths of the matrix, in blocks.
pub const MATRIX_BLOCKS: [u32; 4] = [2, 8, 32, 128];

#[cfg(feature = "storage_read_matrix")]
/// **Read the same blocks at each clock and length, re-identifying the card before every read**,
/// so a read that leaves the card mid-transfer cannot fail the next one. Read-only: `CMD18` from
/// `logical_block_address` and identification, nothing else. `buffer` holds at least the longest
/// read.
///
/// # Errors
///
/// Identification failing, which ends the matrix; a failed read is a row, not an error.
///
/// Name: ratified 2026-11-02 (UTC), #3092. The function that reads the whole matrix.
pub fn read_matrix<R: RegisterWindowAndClock>(
    host: &mut HostController<QueueWatch<R>>,
    bus_width: u8,
    logical_block_address: u64,
    buffer: &mut [u8],
    mut row: impl FnMut(MatrixRow),
) -> Result<(), InitializationError> {
    for &hertz in &MATRIX_HERTZ {
        for &blocks in &MATRIX_BLOCKS {
            let card = sd::identify(host, bus_width)?;
            let card_hertz = host
                .set_clock(hertz)
                .map_err(InitializationError::Controller)?;
            let len = blocks as usize * BLOCK_BYTES;
            let Some(dst) = buffer.get_mut(..len) else {
                continue;
            };
            host.registers_mut().clear();
            let start = host.registers_mut().now_microseconds();
            let result = sd::read_blocks(host, &card, logical_block_address, dst);
            let microseconds = host.registers_mut().now_microseconds().wrapping_sub(start);
            let watch = host.registers_mut();
            let after = [
                watch.inner.read(STATUS_REGISTER),
                watch.inner.read(RAW_INTERRUPT_STATUS_REGISTER),
                watch.inner.read(TRANSFERRED_CARD_BYTE_COUNT_REGISTER),
                watch.inner.read(TRANSFERRED_HOST_BYTE_COUNT_REGISTER),
            ];
            row(MatrixRow {
                asked_hertz: hertz,
                card_hertz,
                blocks,
                result,
                microseconds,
                maximum_fill: watch.maximum_fill,
                full_reads: watch.full_reads,
                after,
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use designware_mobile_storage::card::CardIdentification;
    use designware_mobile_storage::sd::CardKind;

    use super::*;

    #[cfg(feature = "storage_read_matrix")]
    /// `QueueWatch` is what says whether the VisionFive 2's queue filled: its peak and full count must come from
    /// `STATUS_REGISTER` alone, count every full read, and start again at `clear`.
    #[test_case]
    fn the_watch_counts_status_fill_and_nothing_else() {
        /// `STATUS_REGISTER` reads `status`; every other register reads all ones.
        struct Fixed {
            status: u32,
        }
        impl RegisterWindowAndClock for Fixed {
            fn read(&mut self, o: u32) -> u32 {
                if o == STATUS_REGISTER {
                    self.status
                } else {
                    u32::MAX
                }
            }
            fn write(&mut self, _: u32, _: u32) {}
            fn now_microseconds(&mut self) -> u64 {
                0
            }
        }
        // FIFO count in bits 29:17, the full flag in bit 3.
        let mut w = QueueWatch::new(Fixed { status: 5 << 17 });
        w.read(RAW_INTERRUPT_STATUS_REGISTER);
        assert_eq!((w.maximum_fill, w.full_reads), (0, 0));
        w.read(STATUS_REGISTER);
        assert_eq!((w.maximum_fill, w.full_reads), (5, 0));
        let mut w = QueueWatch::new(Fixed {
            status: (128 << 17) | STATUS_QUEUE_FULL,
        });
        w.read(STATUS_REGISTER);
        w.read(STATUS_REGISTER);
        assert_eq!((w.maximum_fill, w.full_reads), (128, 2));
        w.clear();
        assert_eq!((w.maximum_fill, w.full_reads), (0, 0));
    }

    /// The pattern, worked by hand for a few bytes:
    /// `i * 31 ^ logical_block_address * 7 ^ 0x5a ^ i >> 8`, each in eight bits. The `i >> 8` term is
    /// what makes a sector's two halves differ.
    #[test_case]
    fn the_write_pattern_is_the_formula_and_its_halves_differ() {
        assert_eq!(scratch_pattern_byte(0, 0), 0x5a);
        assert_eq!(scratch_pattern_byte(0, 1), 31 ^ 0x5a);
        assert_eq!(scratch_pattern_byte(1, 0), 7 ^ 0x5a);
        assert_eq!(scratch_pattern_byte(3, 5), 155 ^ 21 ^ 0x5a);
        assert_eq!(scratch_pattern_byte(0, 256), 0x5a ^ 1);
        // Where the high term and the rest share a bit, it still XORs rather than ORs.
        assert_eq!(scratch_pattern_byte(0, 257), 31 ^ 0x5a ^ 1);
        assert!(
            (0..256)
                .map(|i| scratch_pattern_byte(9, i))
                .ne((256..512).map(|i| scratch_pattern_byte(9, i)))
        );
    }

    /// The failure prints a sentence naming the block.
    #[test_case]
    fn the_read_failure_names_its_block() {
        use core::fmt::Write;
        struct Text([u8; 96], usize);
        impl Write for Text {
            fn write_str(&mut self, s: &str) -> core::fmt::Result {
                self.0[self.1..self.1 + s.len()].copy_from_slice(s.as_bytes());
                self.1 += s.len();
                Ok(())
            }
        }
        let mut t = Text([0; 96], 0);
        write!(
            t,
            "{}",
            ReadOnlyProbeFailure::Read(7, BlockTransferError::Length)
        )
        .unwrap();
        assert_eq!(
            &t.0[..t.1],
            b"reading block 7 failed: the buffer is empty or not a whole number of blocks"
        );
    }
    /// Sectors on the fake card: a 4 KiB card, so the tests fit a kernel thread's 24 KiB stack. The
    /// partition starts at sector 4, so the scratch range is sectors 1 to 3.
    const FAKE_SECTORS: usize = 8;
    const FAKE_BYTES: usize = FAKE_SECTORS * BLOCK_BYTES;
    const GAP_FIRST: u32 = 1;
    const GAP_COUNT: u32 = 3;
    const GAP_BYTES: usize = GAP_COUNT as usize * BLOCK_BYTES;

    /// What the fake card holds before a test: odd (so never zero) and different for each sector,
    /// except sector 0, which is the master boot record, and the gap of a `blank` card, which is zero.
    fn original_byte(lba: usize, i: usize, blank: bool) -> u8 {
        let gap = (GAP_FIRST as usize..(GAP_FIRST + GAP_COUNT) as usize).contains(&lba);
        match lba {
            0 => table_sector(4)[i],
            _ if gap && blank => 0,
            _ => ((lba * 13 + i * 7) as u8) | 1,
        }
    }

    /// Sector 0 with one FAT32 partition starting at `start`.
    fn table_sector(start: u32) -> [u8; BLOCK_BYTES] {
        let mut s = [0u8; BLOCK_BYTES];
        s[446 + 4] = 0x0b;
        s[446 + 8..446 + 12].copy_from_slice(&start.to_le_bytes());
        s[446 + 12..446 + 16].copy_from_slice(&1000u32.to_le_bytes());
        s[510] = 0x55;
        s[511] = 0xaa;
        s
    }

    /// **A fake controller with a card behind it**: just the registers the polled data path of
    /// `HostController` reads and writes (the command, the response, the raw interrupt status, the
    /// queue), `CMD13`, `CMD17`, `CMD18`, `CMD24` and `CMD25`, and a card that is an array. It is
    /// not the crate's simulator and models no timing, identification or error bits. The faults are
    /// the ones the write test must report: a data command whose response carries an error, a write
    /// the card accepts and drops, and a write that lands with one sector damaged.
    struct FakeCard<'a> {
        sectors: &'a mut [u8; FAKE_BYTES],
        now: u64,
        control: u32,
        raw_interrupt_status: u32,
        byte_count: u32,
        argument: u32,
        response: u32,
        read_at: usize,
        read_end: usize,
        write_at: usize,
        write_length: usize,
        write_received: usize,
        write_buffer: [u8; GAP_BYTES],
        /// Commands the host started, and how many were data commands and how many of those wrote.
        commands: usize,
        data_commands: usize,
        writes: usize,
        /// The lowest and one past the highest sector a write command landed on.
        written: Option<(usize, usize)>,
        /// The data command (counted from 0, reads and writes together) whose response carries an
        /// error in its status.
        fail_data_command: Option<usize>,
        /// The write command (counted from 0) the card accepts and discards.
        drop_write: Option<usize>,
        /// The write command, and the sector it holds, that lands with its first byte inverted.
        corrupt_write: Option<(usize, usize)>,
    }

    impl<'a> FakeCard<'a> {
        const COMMAND_START: u32 = 1 << 31;
        const CONTROL: u32 = 0x000;
        const BYTE_COUNT: u32 = 0x020;
        const ARGUMENT: u32 = 0x028;
        const COMMAND: u32 = 0x02c;
        const RESPONSE: u32 = 0x030;
        const RAW_INTERRUPT_STATUS: u32 = 0x044;
        const VERSION: u32 = 0x06c;
        const HARDWARE_CONFIGURATION: u32 = 0x070;
        /// Where the version in `VERSION` puts the queue.
        const QUEUE: u32 = 0x200;
        const COMMAND_DONE: u32 = 1 << 2;
        const TRANSFER_OVER: u32 = 1 << 3;
        const TRANSMIT_REQUEST: u32 = 1 << 4;
        const RECEIVE_REQUEST: u32 = 1 << 5;
        const AUTOMATIC_STOP_DONE: u32 = 1 << 14;
        /// An error bit in a response, one `sd::read_blocks` and `write_blocks` refuse.
        const STATUS_ERROR: u32 = 1 << 31;

        fn new(sectors: &'a mut [u8; FAKE_BYTES], blank_gap: bool) -> FakeCard<'a> {
            for (at, b) in sectors.iter_mut().enumerate() {
                *b = original_byte(at / BLOCK_BYTES, at % BLOCK_BYTES, blank_gap);
            }
            FakeCard {
                sectors,
                now: 0,
                control: 0,
                raw_interrupt_status: 0,
                byte_count: 0,
                argument: 0,
                response: 0,
                read_at: 0,
                read_end: 0,
                write_at: 0,
                write_length: 0,
                write_received: 0,
                write_buffer: [0; GAP_BYTES],
                commands: 0,
                data_commands: 0,
                writes: 0,
                written: None,
                fail_data_command: None,
                drop_write: None,
                corrupt_write: None,
            }
        }

        /// Every sector is what `new` put there.
        fn is_untouched(&self, blank_gap: bool) -> bool {
            self.sectors
                .iter()
                .enumerate()
                .all(|(at, &b)| b == original_byte(at / BLOCK_BYTES, at % BLOCK_BYTES, blank_gap))
        }

        fn start_command(&mut self, index: u32) {
            self.commands += 1;
            self.raw_interrupt_status = Self::COMMAND_DONE;
            self.response = 0;
            let (n, data) = (self.data_commands, matches!(index, 17 | 18 | 24 | 25));
            if data {
                self.data_commands += 1;
                if self.fail_data_command == Some(n) {
                    self.response = Self::STATUS_ERROR;
                }
            }
            let at = self.argument as usize * BLOCK_BYTES;
            match index {
                17 | 18 => {
                    self.read_at = at;
                    self.read_end = at + self.byte_count as usize;
                }
                24 | 25 => {
                    self.write_at = at;
                    self.write_length = self.byte_count as usize;
                    self.write_received = 0;
                }
                _ => {}
            }
        }

        fn receive_word(&mut self, word: u32) {
            let at = self.write_received;
            self.write_buffer[at..at + 4].copy_from_slice(&word.to_le_bytes());
            self.write_received += 4;
            if self.write_received < self.write_length {
                return;
            }
            let n = self.writes;
            self.writes += 1;
            let first = self.write_at / BLOCK_BYTES;
            let count = self.write_length / BLOCK_BYTES;
            if self.drop_write != Some(n) {
                self.sectors[self.write_at..self.write_at + self.write_length]
                    .copy_from_slice(&self.write_buffer[..self.write_length]);
                if let Some((w, sector)) = self.corrupt_write
                    && w == n
                    && (first..first + count).contains(&sector)
                {
                    self.sectors[sector * BLOCK_BYTES] ^= 0xff;
                }
            }
            self.written = Some(match self.written {
                None => (first, first + count),
                Some((low, high)) => (low.min(first), high.max(first + count)),
            });
            self.raw_interrupt_status |= Self::TRANSFER_OVER | Self::AUTOMATIC_STOP_DONE;
        }
    }

    impl RegisterWindowAndClock for FakeCard<'_> {
        fn read(&mut self, offset: u32) -> u32 {
            let reading = self.read_at < self.read_end;
            let writing = self.write_received < self.write_length;
            match offset {
                Self::CONTROL => self.control,
                Self::RESPONSE => self.response,
                Self::RAW_INTERRUPT_STATUS => {
                    self.raw_interrupt_status
                        | if reading { Self::RECEIVE_REQUEST } else { 0 }
                        | if writing { Self::TRANSMIT_REQUEST } else { 0 }
                }
                // The queue count in bits 29:17: the words left to read, and none held when writing.
                STATUS_REGISTER_OFFSET => (((self.read_end - self.read_at) / 4) as u32) << 17,
                Self::VERSION => 0x240a,
                // A 32-bit queue.
                Self::HARDWARE_CONFIGURATION => 1 << 7,
                Self::QUEUE => {
                    let b = &self.sectors[self.read_at..self.read_at + 4];
                    let word = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
                    self.read_at += 4;
                    if self.read_at == self.read_end {
                        self.raw_interrupt_status |=
                            Self::TRANSFER_OVER | Self::AUTOMATIC_STOP_DONE;
                    }
                    word
                }
                _ => 0,
            }
        }

        fn write(&mut self, offset: u32, value: u32) {
            match offset {
                // The reset bits clear themselves.
                Self::CONTROL => self.control = value & !0b111,
                Self::BYTE_COUNT => self.byte_count = value,
                Self::ARGUMENT => self.argument = value,
                Self::RAW_INTERRUPT_STATUS => self.raw_interrupt_status &= !value,
                Self::COMMAND if value & Self::COMMAND_START != 0 => {
                    self.start_command(value & 0x3f);
                }
                Self::QUEUE => self.receive_word(value),
                _ => {}
            }
        }

        fn now_microseconds(&mut self) -> u64 {
            self.now += 1;
            self.now
        }
    }

    /// `STATUS_REGISTER`, the one register of the fake the crate exports an offset for.
    const STATUS_REGISTER_OFFSET: u32 = designware_mobile_storage::registers::STATUS_REGISTER;

    /// A block-addressed card of `FAKE_SECTORS` sectors, and a controller over `fake` with a
    /// 128-word queue.
    fn host_over(fake: FakeCard<'_>) -> (HostController<FakeCard<'_>>, Card) {
        let (host, _) = HostController::new(fake, 50_000_000, Some(128)).unwrap();
        let card = Card {
            kind: CardKind::SdHighCapacity,
            relative_card_address: 1,
            operation_conditions: 0,
            card_identification_raw: [0; 4],
            card_specific_data_raw: [0; 4],
            card_identification: CardIdentification::from_sd_register(&[0; 4]),
            blocks: FAKE_SECTORS as u64,
            block_addressed: true,
            bus_width: 4,
            clock_hertz: sd::TRANSFER_HERTZ,
        };
        (host, card)
    }

    /// The table of the card `FakeCard::new` makes.
    fn table() -> Option<MasterBootRecord> {
        MasterBootRecord::parse(&table_sector(4))
    }

    /// Run the write test over `fake` with exactly-sized buffers.
    fn write_test(fake: FakeCard<'_>) -> (ScratchWriteVerdict, FakeCard<'_>) {
        let (mut host, card) = host_over(fake);
        let (mut original, mut scratch) = ([0u8; GAP_BYTES], [0u8; GAP_BYTES]);
        let verdict = run_scratch_write_test(
            &mut host,
            &card,
            table().as_ref(),
            &mut original,
            &mut scratch,
        );
        (verdict, host.into_registers())
    }

    /// The test must write nothing when it has no gap: not with no table, not with a first
    /// partition at sector 1, and not with a protective table. It must also send no command at all.
    #[test_case]
    fn a_card_with_no_gap_is_refused_and_not_written() {
        let mut protective = table_sector(2048);
        protective[446 + 4] = 0xee;
        let mut at_one = table_sector(1);
        at_one[446 + 4] = 0x0b;
        let sectors = [[0u8; BLOCK_BYTES], at_one, protective];
        for sector in &sectors {
            let mut store = [0u8; FAKE_BYTES];
            let (mut host, card) = host_over(FakeCard::new(&mut store, false));
            let table = MasterBootRecord::parse(sector);
            let (mut a, mut b) = ([0u8; GAP_BYTES], [0u8; GAP_BYTES]);
            let verdict = run_scratch_write_test(&mut host, &card, table.as_ref(), &mut a, &mut b);
            let fake = host.into_registers();
            assert_eq!(verdict, ScratchWriteVerdict::Refused);
            assert_eq!(fake.commands, 0);
            assert!(fake.is_untouched(false));
        }
    }

    /// The timed read says which block it stopped at: the start of the command that failed.
    #[test_case]
    fn the_timed_read_says_which_block_it_could_not_read() {
        let mut store = [0u8; FAKE_BYTES];
        let mut fake = FakeCard::new(&mut store, false);
        fake.fail_data_command = Some(3);
        let (mut host, card) = host_over(fake);
        let mut buffer = [0u8; 2 * BLOCK_BYTES];
        let failed = timed_read(&mut host, &card, 0, &mut buffer, 16);
        assert!(matches!(
            failed,
            Err(ReadOnlyProbeFailure::Read(6, BlockTransferError::Status(_)))
        ));
        // Nothing is read once a command failed.
        assert_eq!(host.into_registers().data_commands, 4);
    }

    /// The timed read stops before the card's end, and reads nothing with no buffer.
    #[test_case]
    fn the_timed_read_stops_at_the_card_end_and_an_empty_buffer_reads_nothing() {
        let mut store = [0u8; FAKE_BYTES];
        let (mut host, card) = host_over(FakeCard::new(&mut store, false));
        let mut buffer = [0u8; 3 * BLOCK_BYTES];
        // From sector 2, three commands of three sectors would pass the end; two fit.
        let (blocks, _) = timed_read(&mut host, &card, 2, &mut buffer, 1000).unwrap();
        assert_eq!(blocks, 6);
        let (blocks, _) = timed_read(&mut host, &card, 0, &mut [], 1000).unwrap();
        assert_eq!(blocks, 0);
        assert_eq!(host.into_registers().data_commands, 2);
    }

    /// A buffer shorter than the range is refused as phase 0 before any command goes out, for
    /// either buffer; a longer one is fine.
    #[test_case]
    fn the_write_test_refuses_buffers_too_small_for_its_range_before_touching_the_card() {
        let short = BlockTransferError::Length;
        for (original_len, scratch_len) in [(GAP_BYTES - BLOCK_BYTES, GAP_BYTES), (GAP_BYTES, 0)] {
            let mut store = [0u8; FAKE_BYTES];
            let (mut host, card) = host_over(FakeCard::new(&mut store, false));
            let (mut original, mut scratch) = ([0u8; GAP_BYTES], [0u8; GAP_BYTES]);
            let verdict = run_scratch_write_test(
                &mut host,
                &card,
                table().as_ref(),
                &mut original[..original_len],
                &mut scratch[..scratch_len],
            );
            assert_eq!(verdict, ScratchWriteVerdict::Failed(0, short));
            let fake = host.into_registers();
            assert_eq!(fake.commands, 0);
            assert!(fake.is_untouched(false));
        }
    }

    /// Whichever of the five commands fails, the verdict names its phase and the error. The data
    /// commands in order: 0 reads the original, 1 writes the pattern, then one read of each
    /// sector, then the write of the original and its read back.
    #[test_case]
    fn the_write_test_names_the_phase_that_failed() {
        let count = GAP_COUNT as usize;
        let ordinals = [0, 1, 2, 2 + count, 3 + count];
        for (phase, ordinal) in ordinals.into_iter().enumerate() {
            let mut store = [0u8; FAKE_BYTES];
            let mut fake = FakeCard::new(&mut store, false);
            fake.fail_data_command = Some(ordinal);
            let (verdict, fake) = write_test(fake);
            assert!(
                matches!(
                    verdict,
                    ScratchWriteVerdict::Failed(p, BlockTransferError::Status(_)) if usize::from(p) == phase
                ),
                "phase {phase}: {verdict:?}"
            );
            if phase == 0 {
                // The original could not be read, so nothing may be written over it.
                assert_eq!(fake.writes, 0);
            }
            if phase != 3 {
                // Every other failure still puts the original back, or never changed it.
                assert!(fake.is_untouched(false), "phase {phase}");
            }
        }
    }

    /// A card that drops the pattern is a pattern mismatch at the first sector, and the original
    /// is still put back; one that drops the restore is a restore mismatch, and says so.
    #[test_case]
    fn a_card_that_drops_writes_is_a_pattern_mismatch_and_one_that_drops_the_restore_says_so() {
        let mut store = [0u8; FAKE_BYTES];
        let mut fake = FakeCard::new(&mut store, false);
        fake.drop_write = Some(0);
        let (verdict, fake) = write_test(fake);
        assert_eq!(
            verdict,
            ScratchWriteVerdict::PatternMismatch(u64::from(GAP_FIRST))
        );
        assert!(fake.is_untouched(false));

        let mut store = [0u8; FAKE_BYTES];
        let mut fake = FakeCard::new(&mut store, false);
        fake.drop_write = Some(1);
        let (verdict, _) = write_test(fake);
        assert_eq!(
            verdict,
            ScratchWriteVerdict::RestoreMismatch(u64::from(GAP_FIRST))
        );
    }

    /// When the restore is dropped, what the card is left holding is the pattern, and in each
    /// sector it is that sector's own, not one block repeated.
    #[test_case]
    fn the_pattern_left_behind_is_each_sectors_own() {
        let mut store = [0u8; FAKE_BYTES];
        let mut fake = FakeCard::new(&mut store, false);
        fake.drop_write = Some(1);
        let (_, fake) = write_test(fake);
        for s in 0..GAP_COUNT as usize {
            let lba = GAP_FIRST as usize + s;
            let sector = &fake.sectors[lba * BLOCK_BYTES..(lba + 1) * BLOCK_BYTES];
            assert!(
                sector
                    .iter()
                    .enumerate()
                    .all(|(i, &b)| b == scratch_pattern_byte(lba as u64, i)),
                "sector {lba}"
            );
        }
    }

    /// A sector that lands damaged is named by its own address, whether the damage is in the
    /// pattern or in the restore.
    #[test_case]
    fn a_mismatch_names_its_own_sector() {
        let mut store = [0u8; FAKE_BYTES];
        let mut fake = FakeCard::new(&mut store, false);
        fake.corrupt_write = Some((0, 2));
        let (verdict, fake) = write_test(fake);
        assert_eq!(verdict, ScratchWriteVerdict::PatternMismatch(2));
        assert!(fake.is_untouched(false));

        let mut store = [0u8; FAKE_BYTES];
        let mut fake = FakeCard::new(&mut store, false);
        fake.corrupt_write = Some((1, 3));
        let (verdict, _) = write_test(fake);
        assert_eq!(verdict, ScratchWriteVerdict::RestoreMismatch(3));
    }

    /// Longer buffers than the range are used by their first bytes; the verdict reports whether the
    /// gap was all zero; and the writes land only in the gap, which is put back exactly.
    #[test_case]
    fn the_write_test_takes_long_buffers_and_reports_a_blank_gap() {
        for blank in [false, true] {
            let mut store = [0u8; FAKE_BYTES];
            let (mut host, card) = host_over(FakeCard::new(&mut store, blank));
            let (mut original, mut scratch) = (
                [0u8; GAP_BYTES + BLOCK_BYTES],
                [0u8; GAP_BYTES + BLOCK_BYTES],
            );
            let verdict = run_scratch_write_test(
                &mut host,
                &card,
                table().as_ref(),
                &mut original,
                &mut scratch,
            );
            assert_eq!(
                verdict,
                ScratchWriteVerdict::Verified(GAP_FIRST, GAP_COUNT, blank)
            );
            let fake = host.into_registers();
            assert_eq!(fake.written, Some((1, 4)));
            assert!(fake.is_untouched(blank));
        }
    }
}
