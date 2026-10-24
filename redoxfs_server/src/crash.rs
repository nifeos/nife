//! Crash injection: the failures a copy-on-write filesystem exists to survive (milestone 37,
//! DECISIONS §34 condition 1).
//!
//! Applies the byte-level damage a dying machine or a lying device does to a disk to a real image,
//! and hands the result to the same [`Server::open`](crate::Server::open) the FS server always
//! mounts with. The seam is [`BlockIo`], the trait the server reaches its disk through on device
//! and host alike, so the property is host-testable in milliseconds. See
//! notes/fs-server/crash-consistency.md.

//! # The model: record once, replay damaged
//!
//! A workload runs once against [`Recording`], whose ordered write log is exactly what the device
//! saw. Each crash is then the pristine image plus a prefix or a mutilation of that log
//! ([`Damage`]). For a device with no reordering that is the crash, not an approximation, and every
//! fault point starts from a byte-identical image (DECISIONS §27 records what a harness that leaves
//! state behind between runs costs).

//! # The negative control lives here too
//!
//! [`only_this_generation`] blanks every header slot but one, so the mount has no older generation
//! to fall back to and the injector's damage shows as a mount that fails. See
//! `the_ring_is_what_saves_it` in the tests.

use alloc::vec;
use alloc::vec::Vec;

use redoxfs::{BLOCK_SIZE, HEADER_RING, Header};
use syscall::error::{EIO, Error, Result};

use crate::{BLOCK, BlockIo};

/// One block write, as the device saw it: which block, and the 4096 bytes offered for it.
pub struct Write {
    pub block: u64,
    pub data: [u8; BLOCK],
}

impl Write {
    /// True if this write targets the header ring, which is to say it is a **commit**. RedoxFS
    /// writes a transaction's data and tree blocks first and its header last, at slot
    /// `generation % HEADER_RING`, so the header write is the instant a transaction becomes
    /// visible; every block in the ring belongs to the ring and nothing else is allocated there.
    pub fn is_commit(&self) -> bool {
        self.block < HEADER_RING
    }
}

/// A [`BlockIo`] over an in-memory image that **records every write in order**.
///
/// The recording is the whole trick. Run the workload once through this and the log is a complete,
/// ordered account of what the platter was asked to do, which is enough to reconstruct the disk as
/// it would stand after a failure at any point, without running the engine again.
pub struct Recording {
    image: Vec<u8>,
    log: Vec<Write>,
}

impl Recording {
    pub fn new(image: Vec<u8>) -> Self {
        Self {
            image,
            log: Vec::new(),
        }
    }

    /// The ordered write log, one entry per block write.
    pub fn log(&self) -> &[Write] {
        &self.log
    }

    /// Take the log, dropping the image.
    pub fn into_log(self) -> Vec<Write> {
        self.log
    }
}

impl BlockIo for Recording {
    fn read_block(&mut self, block: u64, buf: &mut [u8; BLOCK]) -> Result<()> {
        let off = block as usize * BLOCK;
        buf.copy_from_slice(&self.image[off..off + BLOCK]);
        Ok(())
    }

    fn write_block(&mut self, block: u64, buf: &[u8; BLOCK]) -> Result<()> {
        let off = block as usize * BLOCK;
        self.image[off..off + BLOCK].copy_from_slice(buf);
        self.log.push(Write { block, data: *buf });
        Ok(())
    }

    fn size_bytes(&mut self) -> Result<u64> {
        Ok(self.image.len() as u64)
    }
}

/// A plain [`BlockIo`] over an in-memory image, with no recording and no damage: what a recovering
/// mount reads a reconstructed disk through.
///
/// **A block past the end of the disk is `EIO`, not a panic**, because that is what a device does
/// and because a recovering mount really does ask for one. `FileSystem::open`'s ring scan walks
/// `0..65536` looking for a valid header, so an image with no valid header at all (which the
/// negative control deliberately manufactures) runs the scan off the end of an 8 MiB disk. A panic
/// there would report the harness rather than the filesystem.
pub struct MemIo(pub Vec<u8>);

impl MemIo {
    fn range(&self, block: u64) -> Result<core::ops::Range<usize>> {
        let off = (block as usize).checked_mul(BLOCK).ok_or(Error::new(EIO))?;
        let end = off.checked_add(BLOCK).ok_or(Error::new(EIO))?;
        if end > self.0.len() {
            return Err(Error::new(EIO));
        }
        Ok(off..end)
    }
}

impl BlockIo for MemIo {
    fn read_block(&mut self, block: u64, buf: &mut [u8; BLOCK]) -> Result<()> {
        buf.copy_from_slice(&self.0[self.range(block)?]);
        Ok(())
    }

    fn write_block(&mut self, block: u64, buf: &[u8; BLOCK]) -> Result<()> {
        let r = self.range(block)?;
        self.0[r].copy_from_slice(buf);
        Ok(())
    }

    fn size_bytes(&mut self) -> Result<u64> {
        Ok(self.0.len() as u64)
    }
}

/// What went wrong at the device. Every variant is stated in terms of **what reached the platter**,
/// because that is the only thing a filesystem can reason about; whether the write returned success
/// to the caller is a separate question and is exactly what makes [`Damage::Dropped`] interesting.
#[derive(Clone, Copy, Debug)]
pub enum Damage {
    /// A power cut. Writes `1..=landed` reached the platter; nothing after did, and the process is
    /// gone with them.
    Cut { landed: usize },
    /// A power cut that caught a write in flight: writes `1..landed` are whole, write `landed`
    /// landed only its first `bytes` bytes, and the rest of that block still holds the old
    /// contents. Nothing after landed.
    TornCut { landed: usize, bytes: usize },
    /// A device that **acknowledged write `index` and never persisted it**, then carried on
    /// persisting everything after. A volatile write cache that lost one block, or a disk that
    /// lies about ordering.
    Dropped { index: usize },
    /// The same lie with a half-written block: write `index` persisted only its first `bytes`
    /// bytes, and every write after it landed whole.
    Torn { index: usize, bytes: usize },
}

/// Rebuild the image the platter would hold after `damage`, from the pristine image and the log.
///
/// Writes are numbered from 1 in issue order, matching how a person counts them when reading a
/// trace. The pristine image is never modified.
pub fn damaged_image(pristine: &[u8], log: &[Write], damage: Damage) -> Vec<u8> {
    let mut image = pristine.to_vec();
    let mut apply = |w: &Write, bytes: usize| {
        let off = w.block as usize * BLOCK;
        image[off..off + bytes].copy_from_slice(&w.data[..bytes]);
    };
    match damage {
        Damage::Cut { landed } => {
            for w in &log[..landed] {
                apply(w, BLOCK);
            }
        }
        Damage::TornCut { landed, bytes } => {
            for w in &log[..landed - 1] {
                apply(w, BLOCK);
            }
            apply(&log[landed - 1], bytes);
        }
        Damage::Dropped { index } => {
            for (i, w) in log.iter().enumerate() {
                if i + 1 != index {
                    apply(w, BLOCK);
                }
            }
        }
        Damage::Torn { index, bytes } => {
            for (i, w) in log.iter().enumerate() {
                apply(w, if i + 1 == index { bytes } else { BLOCK });
            }
        }
    }
    image
}

/// Take the header ring's history away: blank every generation in the ring except the one at
/// `keep_slot`. The negative control's instrument.
///
/// A mount's recovery is `FileSystem::open`'s scan of all [`HEADER_RING`] slots for the newest
/// whose seahash checks out. With one slot left, damage to the newest commit makes the mount fail
/// instead of stepping back a generation. Blanking is zeroing, which fails `Header::valid()` on the
/// signature, the first thing it checks.
pub fn only_this_generation(image: &mut [u8], keep_slot: u64) {
    for slot in 0..HEADER_RING {
        if slot != keep_slot {
            let off = slot as usize * BLOCK;
            image[off..off + BLOCK].fill(0);
        }
    }
}

/// Read the header sitting in ring slot `slot` of `image`, whether or not it is valid. Used by the
/// control to say *why* a mount failed: a torn commit leaves a header the engine rejects, and
/// showing that directly is better evidence than a mount that merely errored.
pub fn header_at(image: &[u8], slot: u64) -> Header {
    let off = slot as usize * BLOCK;
    let mut header = Header::default();
    let bytes: &mut [u8] = &mut header;
    bytes.copy_from_slice(&image[off..off + BLOCK_SIZE as usize]);
    header
}

/// An all-zero image of `mib` mebibytes, the raw disk a fixture is built on.
pub fn blank_image(mib: usize) -> Vec<u8> {
    vec![0u8; mib * 1024 * 1024]
}
