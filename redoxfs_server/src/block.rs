//! **The block layer beneath RedoxFS**: whole-block transfers ([`BlockIo`], [`BlockDisk`]) and the
//! single-block read cache ([`CachedDisk`]); [`crate::Server`] reaches it only through
//! [`redoxfs::Disk`]. See notes/fs-server/caches-and-io.md. The module name is provisional.

use alloc::vec::Vec;

use redoxfs::Disk;
use syscall::error::Result;

/// One filesystem block, in bytes (RedoxFS's `BLOCK_SIZE`): the unit [`BlockIo`] transfers.
pub const BLOCK: usize = 4096;

/// How many blocks [`CachedDisk`] holds in the EL0 server, here rather than in the binary so a
/// host test measures the cache the device runs. 256 blocks, 1 MiB at most, allocated as they
/// fill: the smallest size that holds the walk fixture's 207-block metadata working set (milestone
/// 606 (a directory walk costs what it does on Linux), `examples/walk_replay.rs`). A larger tree
/// wants more. See notes/fs-server/caches-and-io.md.
pub const CACHE_SLOTS: usize = 256;

/// The wire-to-store assertions, here because this is the one place in the tree that sees both
/// `filesystem_contract` and the store. `const` rather than tests, so they fire in the EL0 build
/// too, where no test harness runs. See notes/fs-server/caches-and-io.md.
const _: () = {
    // `BLOCK` is the transfer unit for a store whose block size is `BLOCK_SIZE`; if they disagree,
    // every chunked read and write in this file is off by a factor.
    assert!(BLOCK as u64 == redoxfs::BLOCK_SIZE);

    // The record-aligned throughput phase reads at multiples of `RECORD`, which means what it
    // claims only while each multiple is a record boundary. See that constant's comment.
    assert!(
        filesystem_fixture::throughput::RECORD
            .is_multiple_of(redoxfs::BLOCK_SIZE << redoxfs::RECORD_LEVEL)
    );

    // A file this build creates must be readable by this build: the creation level may not pass
    // the ceiling the two `BlockTrait::empty` guards enforce, a separate constant.
    assert!(redoxfs::RECORD_LEVEL <= redoxfs::RECORD_LEVEL_MAX);

    // The file channel is a whole number of blocks (milestone 138 (close the read gap) step 3): a
    // channel ending part way through a block would make the last chunk of every full-channel
    // request partial, the repeat-write bug's shape.
    assert!(filesystem_contract::fs::TRANSFER_MAX.is_multiple_of(BLOCK));
    assert!(filesystem_contract::fs::TRANSFER_MAX >= BLOCK);
};

/// A block-granular transport: read or write one whole filesystem block, or report the disk size.
/// The EL0 binary implements this over blk IPC; a host test implements it over a `Vec`.
///
/// This exists so the **chunking** that bridges RedoxFS's byte-addressed, arbitrary-length `Disk`
/// calls to whole-block transfers is written once and host-tested, instead of living only in the EL0
/// binary where no host test can reach it. That gap is what let the repeat-write bug hide.
pub trait BlockIo {
    /// Read filesystem block `block` into `buf` (exactly [`BLOCK`] bytes).
    fn read_block(&mut self, block: u64, buf: &mut [u8; BLOCK]) -> Result<()>;
    /// Write `buf` (exactly [`BLOCK`] bytes) to filesystem block `block`.
    fn write_block(&mut self, block: u64, buf: &[u8; BLOCK]) -> Result<()>;
    /// The disk size, in bytes.
    fn size_bytes(&mut self) -> Result<u64>;
}

/// A RedoxFS [`Disk`] over a [`BlockIo`]. RedoxFS calls `read_at`/`write_at` with a block index and a
/// buffer that may be shorter than a block (a compressed record), exactly a block, or many blocks
/// long (an uncompressed 128 KiB record), so this splits each call into whole-block transfers.
///
/// A write whose final chunk is short is **read-modify-written**: the block is read first, the chunk
/// overwrites its front, and the whole block goes back. That preserves the bytes past the chunk,
/// which is what a byte-addressed disk (`DiskFile`, where `write_at` writes exactly the buffer) does,
/// and RedoxFS relies on it for compressed records.
pub struct BlockDisk<T>(pub T);

impl<T: BlockIo> Disk for BlockDisk<T> {
    unsafe fn read_at(&mut self, block: u64, buffer: &mut [u8]) -> Result<usize> {
        let mut scratch = [0u8; BLOCK];
        for (i, chunk) in buffer.chunks_mut(BLOCK).enumerate() {
            self.0.read_block(block + i as u64, &mut scratch)?;
            chunk.copy_from_slice(&scratch[..chunk.len()]);
        }
        Ok(buffer.len())
    }

    unsafe fn write_at(&mut self, block: u64, buffer: &[u8]) -> Result<usize> {
        let mut scratch = [0u8; BLOCK];
        for (i, chunk) in buffer.chunks(BLOCK).enumerate() {
            let b = block + i as u64;
            if chunk.len() < BLOCK {
                self.0.read_block(b, &mut scratch)?;
            }
            scratch[..chunk.len()].copy_from_slice(chunk);
            self.0.write_block(b, &scratch)?;
        }
        Ok(buffer.len())
    }

    fn size(&mut self) -> Result<u64> {
        self.0.size_bytes()
    }
}

/// A direct-mapped, write-through cache of single-block reads (milestone 138 step 2). Each slot
/// holds at most one `(block number, block bytes)` pair, chosen by `block % capacity`; a collision
/// evicts, which is safe because a miss falls through to the disk. A slot's 4 KiB is allocated the
/// first time something lands in it, so a server pays only for what it touched.
/// Why not an LRU, and the heap sizes, are in notes/fs-server/caches-and-io.md.
struct BlockCache {
    slots: Vec<Option<alloc::boxed::Box<Slot>>>,
}

/// One cached block: its number and its bytes.
type Slot = (u64, [u8; BLOCK]);

impl BlockCache {
    /// `capacity` slots, none of them allocated. `capacity` must be at least 1.
    fn new(capacity: usize) -> Self {
        let mut slots = Vec::with_capacity(capacity);
        slots.resize_with(capacity, || None);
        Self { slots }
    }

    fn slot(&self, block: u64) -> usize {
        (block as usize) % self.slots.len()
    }

    /// The cached bytes for `block`, or `None` on a miss (never cached, or evicted by a collision).
    fn get(&self, block: u64) -> Option<&[u8; BLOCK]> {
        match &self.slots[self.slot(block)] {
            Some(entry) if entry.0 == block => Some(&entry.1),
            _ => None,
        }
    }

    /// Record `block`'s current bytes, replacing whatever the slot held (its own entry to refresh,
    /// or a different block's to evict). `data` must be exactly [`BLOCK`] bytes.
    fn insert(&mut self, block: u64, data: &[u8]) {
        let i = self.slot(block);
        match &mut self.slots[i] {
            Some(entry) => {
                entry.0 = block;
                entry.1.copy_from_slice(data);
            }
            empty => {
                let mut entry = alloc::boxed::Box::new((block, [0u8; BLOCK]));
                entry.1.copy_from_slice(data);
                *empty = Some(entry);
            }
        }
    }

    /// Drop `block`'s entry if it is the one occupying its slot. A no-op if the slot holds a
    /// different block (already evicted) or nothing. The allocation is kept for the next insert.
    fn invalidate(&mut self, block: u64) {
        let i = self.slot(block);
        if let Some(entry) = &mut self.slots[i]
            && entry.0 == block
        {
            // `u64::MAX` is no block this disk has, so the slot now answers every `get` with a miss.
            entry.0 = u64::MAX;
        }
    }
}

/// Whether `block` is one [`CachedDisk`] keeps (milestone 606): everything but the header ring,
/// which is read once at mount and then only written. This server opens images whose filesystem
/// starts at block 0 (`FileSystem::open(disk, None, ..)`), so the ring is blocks
/// `0..HEADER_RING`. See notes/fs-server/caches-and-io.md.
fn cacheable(block: u64) -> bool {
    block >= redoxfs::HEADER_RING
}

/// A [`Disk`] wrapping another, caching single-block reads (milestone 138 step 2): the five
/// single-block reads of `Transaction::read_tree_and_addr`'s tree walk on every
/// `Server::read`/`write`/`fstat` (notes/benchmarks/read-path-block-contract-and-metadata-cache.md).
///
/// Only a `buffer.len() == BLOCK` read consults the cache. A write updates it only after the inner
/// write succeeded, and a short final chunk invalidates its slot. No generation counter is needed:
/// RedoxFS never rewrites a live block in place, and this type's [`Disk::write_at`] is the only
/// path a write reaches the platter through. See notes/fs-server/caches-and-io.md.
pub struct CachedDisk<D> {
    inner: D,
    cache: BlockCache,
}

impl<D> CachedDisk<D> {
    /// The disk this wraps, so a host test can read a counter below the cache.
    #[cfg(feature = "hosttest")]
    pub fn inner(&self) -> &D {
        &self.inner
    }

    /// Wrap `inner`, caching up to `capacity` distinct single-block reads. `capacity` is the
    /// caller's call, against its own heap budget: milestone 37's crash-test FS servers run a
    /// fraction of the ordinary heap and must pick a smaller number than the ordinary server does.
    pub fn new(inner: D, capacity: usize) -> Self {
        Self {
            inner,
            cache: BlockCache::new(capacity),
        }
    }
}

impl<D: Disk> Disk for CachedDisk<D> {
    unsafe fn read_at(&mut self, block: u64, buffer: &mut [u8]) -> Result<usize> {
        if buffer.len() == BLOCK && cacheable(block) {
            if let Some(cached) = self.cache.get(block) {
                buffer.copy_from_slice(cached);
                return Ok(BLOCK);
            }
            // SAFETY: forwarded from this method's own caller, on a buffer of the same shape.
            let n = unsafe { self.inner.read_at(block, buffer)? };
            if n == BLOCK {
                self.cache.insert(block, buffer);
            }
            return Ok(n);
        }
        // A record body or larger: already batched by step 4, and not this cache's target.
        unsafe { self.inner.read_at(block, buffer) }
    }

    unsafe fn write_at(&mut self, block: u64, buffer: &[u8]) -> Result<usize> {
        // SAFETY: forwarded from this method's own caller, on a buffer of the same shape.
        let n = unsafe { self.inner.write_at(block, buffer)? };
        // Only the bytes the inner disk confirmed writing are cached, so a short count cannot be
        // papered over with an entry claiming more landed than did.
        let mut off = 0;
        let mut b = block;
        while off < n {
            let chunk = (n - off).min(BLOCK);
            if !cacheable(b) {
                // A header-ring block: never cached, so there is nothing to keep fresh.
            } else if chunk == BLOCK {
                self.cache.insert(b, &buffer[off..off + BLOCK]);
            } else {
                // A short final chunk: the whole-block bytes that actually landed are a merge
                // (read-modify-write) this type never saw, so it cannot vouch for a cached copy.
                self.cache.invalidate(b);
            }
            off += chunk;
            b += 1;
        }
        Ok(n)
    }

    fn size(&mut self) -> Result<u64> {
        self.inner.size()
    }
}

#[cfg(test)]
mod tests;
