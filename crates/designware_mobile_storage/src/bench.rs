//! The bench step's two halves, as logic: the read-only probe and the scratch write test that a
//! kernel's storage bench runs on a VisionFive 2, here so their order of operations, which is the
//! safety argument, is tested on a host rather than first tried on the card radon boots from.
//! [`run_read_only_probe`] identifies the card, reads sector 0, parses the MBR, reads the first partition's
//! boot sector and times a sequential read from its start; it sends no command that writes.
//! [`run_scratch_write_test`] writes only inside [`MasterBootRecord::scratch_sector_range`]'s range, after reading what
//! is there: a pattern, read back, then the original, read back too. A card with no gap is refused,
//! not written.
//!
//! Name: ratified 2026-11-03 (UTC), #3135. `bench` is the project's word for the physical test bench
//! (notes/bench-boots.md): the step run on the real board, not on a host.

use crate::host_controller::{HostController, RegisterWindowAndClock};
use crate::internal_direct_memory_access::DirectMemoryAccessRegion;
use crate::partition::MasterBootRecord;
use crate::sd::{self, BLOCK_BYTES, BlockTransferError, Card, InitializationError};

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
        .and_then(|m| m.first_start())
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
/// [`HostController::with_internal_direct_memory_access`] made from the one [`run_read_only_probe`] ran on and reported
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

impl<R: RegisterWindowAndClock> RegisterWindowAndClock for QueueWatch<R> {
    fn read(&mut self, offset: u32) -> u32 {
        let v = self.inner.read(offset);
        if offset == crate::registers::STATUS_REGISTER {
            self.maximum_fill = self.maximum_fill.max(crate::registers::queue_count(v));
            if v & crate::registers::STATUS_FIFO_FULL != 0 {
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

/// One multi-block read of the matrix: its card clock, length, outcome and what the queue did.
///
/// Name: ratified 2026-11-02 (UTC), #3092. One row of the matrix of multi-block reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MatrixRow {
    /// The card clock asked for, in Hz.
    pub asked_hz: u32,
    /// The card clock the divider gave.
    pub card_hz: u32,
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
    /// `STATUS_REGISTER`, `RINTSTS`, `TRANSFERRED_CARD_BYTE_COUNT` and `TRANSFERRED_HOST_BYTE_COUNT` read straight after it.
    pub after: [u32; 4],
}

/// The card clocks the matrix reads at: the transfer rate and two halvings of it.
///
/// Name: ratified 2026-11-02 (UTC), #3092. The card clocks of the matrix; `HZ` is the unit suffix.
pub const MATRIX_HZ: [u32; 3] = [sd::TRANSFER_HZ, sd::TRANSFER_HZ / 2, sd::TRANSFER_HZ / 4];
/// The `CMD18` lengths the matrix reads, in blocks.
///
/// Name: ratified 2026-11-02 (UTC), #3092. The `CMD18` lengths of the matrix, in blocks.
pub const MATRIX_BLOCKS: [u32; 4] = [2, 8, 32, 128];

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
    for &hz in &MATRIX_HZ {
        for &blocks in &MATRIX_BLOCKS {
            let card = sd::identify(host, bus_width)?;
            let card_hz = host
                .set_clock(hz)
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
                watch.inner.read(crate::registers::STATUS_REGISTER),
                watch.inner.read(crate::registers::RINTSTS),
                watch
                    .inner
                    .read(crate::registers::TRANSFERRED_CARD_BYTE_COUNT),
                watch
                    .inner
                    .read(crate::registers::TRANSFERRED_HOST_BYTE_COUNT),
            ];
            row(MatrixRow {
                asked_hz: hz,
                card_hz,
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
