//! The bench step's two halves, as logic: the read-only probe and the scratch write test that
//! a kernel's storage bench runs on a VisionFive 2, here so their order of operations, which is the
//! safety argument, is tested on a host rather than first tried on the card radon boots from.
//! [`read_only`] identifies the card, reads sector 0, parses the MBR, reads the first partition's
//! boot sector and times a sequential read from its start; it sends no command that writes.
//! [`scratch_write`] writes only inside [`Mbr::scratch`]'s range, after reading what is there: a
//! pattern, read back, then the original, read back too. A card with no gap is refused, not
//! written.

use crate::host::{Host, Registers};
use crate::idmac::DirectMemoryAccessRegion;
use crate::partition::Mbr;
use crate::sd::{self, BLOCK, Card, InitError, IoError};

/// How many sectors the write test uses at most: 4 KiB, one multi-block command each way.
pub const SCRATCH_SECTORS: u32 = 8;

/// What the read-only probe found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadReport {
    /// The card, identified.
    pub card: Card,
    /// Sector 0, parsed, if it holds an MBR.
    pub mbr: Option<Mbr>,
    /// The first partition's first sector ends in `55 aa` (a FAT boot sector does).
    pub partition_boot_signature: bool,
    /// The first partition's first sector's bytes 82..90 (FAT32's `"FAT32   "` file system
    /// type), for the transcript.
    pub fs_type: [u8; 8],
    /// The block the timed read started at: the first partition's, or 0 without one.
    pub timed_from: u64,
    /// Blocks read in the timed sequential read.
    pub timed_blocks: u64,
    /// Microseconds the timed read took.
    pub timed_us: u64,
}

/// Where the read-only probe stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadFailure {
    /// Identification failed.
    Init(InitError),
    /// A read failed: the block and the error.
    Read(u64, IoError),
}

/// **The read-only probe.** `buf` is the transfer buffer (a whole number of blocks; the timed read
/// moves `buf.len()` bytes per command), `timed_blocks` how many blocks to time.
///
/// # Errors
///
/// The first step that failed.
pub fn read_only<R: Registers, D: DirectMemoryAccessRegion>(
    host: &mut Host<R, D>,
    bus_width: u8,
    buf: &mut [u8],
    timed_blocks: u64,
) -> Result<ReadReport, ReadFailure> {
    let card = sd::identify(host, bus_width).map_err(ReadFailure::Init)?;
    let mut sector = [0u8; BLOCK];
    sd::read_blocks(host, &card, 0, &mut sector).map_err(|e| ReadFailure::Read(0, e))?;
    let mbr = Mbr::parse(&sector);
    let first = mbr.and_then(|m| m.first_start()).map_or(0, u64::from);
    let mut partition_boot_signature = false;
    let mut fs_type = [0u8; 8];
    if first != 0 {
        sd::read_blocks(host, &card, first, &mut sector)
            .map_err(|e| ReadFailure::Read(first, e))?;
        partition_boot_signature = sector[510..512] == [0x55, 0xaa];
        fs_type.copy_from_slice(&sector[82..90]);
    }
    let (timed, timed_us) = timed_read(host, &card, first, buf, timed_blocks)?;
    Ok(ReadReport {
        card,
        mbr,
        partition_boot_signature,
        fs_type,
        timed_from: first,
        timed_blocks: timed,
        timed_us,
    })
}

/// **The timed sequential read**: up to `timed_blocks` blocks from `first`, `buf.len()` bytes a
/// command, stopping before the card's end. Returns the blocks read and the microseconds taken.
fn timed_read<R: Registers, D: DirectMemoryAccessRegion>(
    host: &mut Host<R, D>,
    card: &Card,
    first: u64,
    buf: &mut [u8],
    timed_blocks: u64,
) -> Result<(u64, u64), ReadFailure> {
    let per = (buf.len() / BLOCK) as u64;
    let mut timed = 0u64;
    let start = host.registers().now_us();
    if per > 0 {
        while timed < timed_blocks && first + timed + per <= card.blocks {
            sd::read_blocks(host, card, first + timed, buf)
                .map_err(|e| ReadFailure::Read(first + timed, e))?;
            timed += per;
        }
    }
    Ok((timed, host.registers().now_us().wrapping_sub(start)))
}

/// What the IDMAC's half of the read-only probe found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IdmacReport {
    /// The first transfer's bytes are the polled host's for the same blocks.
    pub matches: bool,
    /// Blocks read in the timed sequential read.
    pub timed_blocks: u64,
    /// Microseconds the timed read took.
    pub timed_us: u64,
}

/// **The IDMAC's half of the read-only probe**, on a host [`Host::with_idmac`] made from the one
/// [`read_only`] ran on and reported `r`. `reference` is the polled host's read of
/// `reference.len()` bytes from `r.timed_from`; the same blocks are read through the IDMAC and
/// compared, then [`read_only`]'s timed read runs again from there in `buf.len()`-byte commands,
/// so the two rates differ only in the data path.
///
/// # Errors
///
/// The read that failed.
pub fn idmac_read<R: Registers, D: DirectMemoryAccessRegion>(
    host: &mut Host<R, D>,
    r: &ReadReport,
    reference: &[u8],
    buf: &mut [u8],
    timed_blocks: u64,
) -> Result<IdmacReport, ReadFailure> {
    let (card, first) = (&r.card, r.timed_from);
    let n = reference.len().min(buf.len());
    sd::read_blocks(host, card, first, &mut buf[..n]).map_err(|e| ReadFailure::Read(first, e))?;
    // An empty reference never gets here: a read of no blocks is refused as `Length`.
    let matches = buf[..n] == reference[..n];
    let (timed_blocks, timed_us) = timed_read(host, card, first, buf, timed_blocks)?;
    Ok(IdmacReport {
        matches,
        timed_blocks,
        timed_us,
    })
}

/// The byte the write test puts at offset `i` of the sector at `lba`: never constant across a
/// sector, different for each sector, so a write that lands one sector off or repeats a block is
/// seen on readback.
#[must_use]
pub const fn pattern(lba: u64, i: usize) -> u8 {
    (i as u8).wrapping_mul(31) ^ (lba as u8).wrapping_mul(7) ^ 0x5a ^ (i >> 8) as u8
}

/// How the scratch write test ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteVerdict {
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
    Failed(u8, IoError),
}

/// **The scratch write test.** `mbr` is what [`read_only`] parsed from this card; `original` and
/// `scratch` are two buffers of at least [`SCRATCH_SECTORS`] blocks.
pub fn scratch_write<R: Registers, D: DirectMemoryAccessRegion>(
    host: &mut Host<R, D>,
    card: &Card,
    mbr: Option<&Mbr>,
    original: &mut [u8],
    scratch: &mut [u8],
) -> WriteVerdict {
    let Some((first, count)) = mbr.and_then(|m| m.scratch(SCRATCH_SECTORS)) else {
        return WriteVerdict::Refused;
    };
    let bytes = count as usize * BLOCK;
    if original.len() < bytes || scratch.len() < bytes {
        return WriteVerdict::Failed(0, IoError::Length);
    }
    let lba = u64::from(first);
    let (original, scratch) = (&mut original[..bytes], &mut scratch[..bytes]);
    if let Err(e) = sd::read_blocks(host, card, lba, original) {
        return WriteVerdict::Failed(0, e);
    }
    let blank = original.iter().all(|&b| b == 0);
    for (s, block) in scratch.as_chunks_mut::<BLOCK>().0.iter_mut().enumerate() {
        for (i, b) in block.iter_mut().enumerate() {
            *b = pattern(lba + s as u64, i);
        }
    }
    let mut verdict = None;
    if let Err(e) = sd::write_blocks(host, card, lba, scratch) {
        verdict = Some(WriteVerdict::Failed(1, e));
    } else {
        let mut back = [0u8; BLOCK];
        for s in 0..u64::from(count) {
            if let Err(e) = sd::read_blocks(host, card, lba + s, &mut back) {
                verdict = Some(WriteVerdict::Failed(2, e));
                break;
            }
            let at = s as usize * BLOCK;
            if back[..] != scratch[at..at + BLOCK] {
                verdict = Some(WriteVerdict::PatternMismatch(lba + s));
                break;
            }
        }
    }
    // Whatever happened, put back what was there.
    if let Err(e) = sd::write_blocks(host, card, lba, original) {
        return WriteVerdict::Failed(3, e);
    }
    if let Err(e) = sd::read_blocks(host, card, lba, scratch) {
        return WriteVerdict::Failed(4, e);
    }
    if let Some(s) = scratch
        .as_chunks::<BLOCK>()
        .0
        .iter()
        .zip(original.as_chunks::<BLOCK>().0)
        .position(|(a, b)| a != b)
    {
        return WriteVerdict::RestoreMismatch(lba + s as u64);
    }
    verdict.unwrap_or(WriteVerdict::Verified(first, count, blank))
}

/// **A register window that remembers how full the FIFO got**: every `STATUS` read updates the
/// highest fill seen and counts the reads that found the FIFO full. It answers the multi-block
/// read diagnosis: whether the polled CPU falls behind the card.
pub struct Watch<R: Registers> {
    inner: R,
    /// The highest `STATUS` FIFO count seen since the last [`Watch::clear`].
    pub max_fill: u32,
    /// `STATUS` reads that found the FIFO-full flag set since the last [`Watch::clear`].
    pub full_reads: u32,
}

impl<R: Registers> Watch<R> {
    /// Watch `inner`.
    pub const fn new(inner: R) -> Self {
        Self {
            inner,
            max_fill: 0,
            full_reads: 0,
        }
    }

    /// Start counting again.
    pub fn clear(&mut self) {
        self.max_fill = 0;
        self.full_reads = 0;
    }
}

impl<R: Registers> Registers for Watch<R> {
    fn read(&mut self, offset: u32) -> u32 {
        let v = self.inner.read(offset);
        if offset == crate::regs::STATUS {
            self.max_fill = self.max_fill.max(crate::regs::fifo_count(v));
            if v & crate::regs::STATUS_FIFO_FULL != 0 {
                self.full_reads += 1;
            }
        }
        v
    }
    fn write(&mut self, offset: u32, value: u32) {
        self.inner.write(offset, value);
    }
    fn now_us(&mut self) -> u64 {
        self.inner.now_us()
    }
}

/// One multi-block read of the matrix: its card clock, length, outcome and what the FIFO did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MatrixRow {
    /// The card clock asked for, in Hz.
    pub asked_hz: u32,
    /// The card clock the divider gave.
    pub card_hz: u32,
    /// Blocks in the one `CMD18`.
    pub blocks: u32,
    /// What the read returned.
    pub result: Result<(), IoError>,
    /// Microseconds the read took.
    pub us: u64,
    /// The highest FIFO fill seen during it, in words.
    pub max_fill: u32,
    /// `STATUS` reads that found the FIFO full.
    pub full_reads: u32,
    /// `STATUS`, `RINTSTS`, `TCBCNT` and `TBBCNT` read straight after it.
    pub after: [u32; 4],
}

/// The card clocks the matrix reads at: the transfer rate and two halvings of it.
pub const MATRIX_HZ: [u32; 3] = [sd::TRANSFER_HZ, sd::TRANSFER_HZ / 2, sd::TRANSFER_HZ / 4];
/// The `CMD18` lengths the matrix reads, in blocks.
pub const MATRIX_BLOCKS: [u32; 4] = [2, 8, 32, 128];

/// **Read the same blocks at each clock and length, re-identifying the card before every read**,
/// so a read that leaves the card mid-transfer cannot fail the next one. Read-only: `CMD18` from
/// `lba` and identification, nothing else. `buf` holds at least the longest read.
///
/// # Errors
///
/// Identification failing, which ends the matrix; a failed read is a row, not an error.
pub fn read_matrix<R: Registers>(
    host: &mut Host<Watch<R>>,
    bus_width: u8,
    lba: u64,
    buf: &mut [u8],
    mut row: impl FnMut(MatrixRow),
) -> Result<(), InitError> {
    for &hz in &MATRIX_HZ {
        for &blocks in &MATRIX_BLOCKS {
            let card = sd::identify(host, bus_width)?;
            let card_hz = host.set_clock(hz).map_err(InitError::Controller)?;
            let len = blocks as usize * BLOCK;
            let Some(dst) = buf.get_mut(..len) else {
                continue;
            };
            host.registers().clear();
            let start = host.registers().now_us();
            let result = sd::read_blocks(host, &card, lba, dst);
            let us = host.registers().now_us().wrapping_sub(start);
            let r = host.registers();
            let after = [
                r.inner.read(crate::regs::STATUS),
                r.inner.read(crate::regs::RINTSTS),
                r.inner.read(crate::regs::TCBCNT),
                r.inner.read(crate::regs::TBBCNT),
            ];
            row(MatrixRow {
                asked_hz: hz,
                card_hz,
                blocks,
                result,
                us,
                max_fill: r.max_fill,
                full_reads: r.full_reads,
                after,
            });
        }
    }
    Ok(())
}
