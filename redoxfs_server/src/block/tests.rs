use redoxfs::{FileSystem, Node, TreePtr};

use super::*;
use crate::Server;
use crate::tests::VecIo;

/// **[`CachedDisk`] tests (milestone 138 step 2).** A [`Disk`] over a `Vec<u8>` that counts every
/// `read_at`/`write_at` call it receives, so a test can assert "the cache answered this one"
/// rather than merely "the answer was right", which every other disk fake here already checks by
/// construction.
struct CountingDisk {
    image: Vec<u8>,
    reads: usize,
    writes: usize,
}

impl CountingDisk {
    fn new(mib: usize) -> Self {
        Self {
            image: vec![0u8; mib * 1024 * 1024],
            reads: 0,
            writes: 0,
        }
    }
}

impl Disk for CountingDisk {
    unsafe fn read_at(&mut self, block: u64, buffer: &mut [u8]) -> Result<usize> {
        self.reads += 1;
        let off = block as usize * BLOCK;
        buffer.copy_from_slice(&self.image[off..off + buffer.len()]);
        Ok(buffer.len())
    }

    unsafe fn write_at(&mut self, block: u64, buffer: &[u8]) -> Result<usize> {
        self.writes += 1;
        let off = block as usize * BLOCK;
        self.image[off..off + buffer.len()].copy_from_slice(buffer);
        Ok(buffer.len())
    }

    fn size(&mut self) -> Result<u64> {
        Ok(self.image.len() as u64)
    }
}

/// [`Disk::read_at`], made safe once for these tests. redoxfs's `# Safety` for it asks nothing of
/// the caller ("Unsafe to discourage use, use filesystem wrappers instead"), so there is no
/// precondition for a call site to argue; these tests call the method directly on purpose.
fn read(disk: &mut impl Disk, block: u64, buffer: &mut [u8]) {
    // SAFETY: the trait's contract has no precondition; see this function's doc.
    unsafe { disk.read_at(block, buffer) }.unwrap();
}

/// [`Disk::write_at`], made safe once, for [`read`]'s reason.
fn write(disk: &mut impl Disk, block: u64, buffer: &[u8]) {
    // SAFETY: as `read`.
    unsafe { disk.write_at(block, buffer) }.unwrap();
}

/// A repeat read of the same block is answered from the cache: the inner disk sees it once.
#[test]
fn a_repeated_single_block_read_hits_the_cache() {
    let mut disk = CachedDisk::new(CountingDisk::new(2), 8);
    let mut buf = [0u8; BLOCK];
    for _ in 0..5 {
        read(&mut disk, 259, &mut buf);
    }
    assert_eq!(
        disk.inner.reads, 1,
        "four of the five reads should have hit"
    );
}

/// **The property the whole type exists for**: a write to a cached block is never served stale
/// afterwards. This is the regression test a caching bug would fail without ever touching the
/// device layer, which is the point of building it host-side.
#[test]
fn a_write_through_the_cache_is_read_back_fresh() {
    let mut disk = CachedDisk::new(CountingDisk::new(2), 8);
    let mut buf = [0u8; BLOCK];
    read(&mut disk, 259, &mut buf); // populate the cache
    assert_eq!(buf, [0u8; BLOCK]);

    let new_bytes = [7u8; BLOCK];
    write(&mut disk, 259, &new_bytes);

    let mut buf2 = [0u8; BLOCK];
    read(&mut disk, 259, &mut buf2);
    assert_eq!(buf2, new_bytes, "a stale cache entry would fail this");
    // And the read that mattered was answered from the cache, not the disk: the write updated
    // the entry rather than merely invalidating it.
    assert_eq!(
        disk.inner.reads, 1,
        "only the first, cold read should have reached the disk"
    );
}

/// A write shorter than a block invalidates rather than caching a guess, because this type never
/// sees the read-modify-write merge that lands underneath it.
#[test]
fn a_short_write_invalidates_rather_than_caching_a_guess() {
    let mut disk = CachedDisk::new(CountingDisk::new(2), 8);
    let mut buf = [0u8; BLOCK];
    read(&mut disk, 259, &mut buf); // populate the cache
    // A short write: the inner disk's contract is to report exactly what it wrote, and a real
    // Disk::write_at that read-modify-writes still reports the full chunk length it was asked
    // for (see BlockDisk::write_at), so simulate the case a Disk implementation reporting a
    // genuinely short count would produce, e.g. a write cut off by the device.
    struct ShortWrite(CountingDisk);
    impl Disk for ShortWrite {
        unsafe fn read_at(&mut self, block: u64, buffer: &mut [u8]) -> Result<usize> {
            unsafe { self.0.read_at(block, buffer) }
        }
        unsafe fn write_at(&mut self, block: u64, buffer: &[u8]) -> Result<usize> {
            unsafe { self.0.write_at(block, &buffer[..BLOCK / 2]) }
        }
        fn size(&mut self) -> Result<u64> {
            self.0.size()
        }
    }
    let mut disk = CachedDisk::new(ShortWrite(CountingDisk::new(2)), 8);
    read(&mut disk, 259, &mut buf);
    let new_bytes = [7u8; BLOCK];
    write(&mut disk, 259, &new_bytes);
    // The entry must be gone: a subsequent read has to reach the disk again rather than answer
    // from a cache that only ever saw half the new bytes.
    let reads_before = disk.inner.0.reads;
    let mut buf2 = [0u8; BLOCK];
    read(&mut disk, 259, &mut buf2);
    assert_eq!(
        disk.inner.0.reads,
        reads_before + 1,
        "a short write must invalidate, not cache a partial guess"
    );
}

/// A collision (two block numbers sharing one slot) evicts rather than corrupting: the newer
/// block reads back correctly and the older one, now a miss, still reads back correctly too
/// (from the inner disk, since the cache no longer has it).
#[test]
fn a_slot_collision_evicts_rather_than_corrupts() {
    let mut disk = CachedDisk::new(CountingDisk::new(2), 4); // capacity 4: blocks 256 and 260 collide
    let mut a = [0u8; BLOCK];
    let mut b = [0u8; BLOCK];
    write(&mut disk, 256, &[1u8; BLOCK]);
    write(&mut disk, 260, &[2u8; BLOCK]); // evicts block 256's entry
    read(&mut disk, 256, &mut a); // a miss now: must come from the disk, not block 260's cached bytes
    read(&mut disk, 260, &mut b);
    assert_eq!(
        a, [1u8; BLOCK],
        "block 256 must read its own bytes, not block 260's"
    );
    assert_eq!(b, [2u8; BLOCK]);
}

/// A multi-block read (a record body) bypasses the cache entirely: it always reaches the disk,
/// and it never populates or consults an entry, so it cannot collide with the single-block
/// working set the cache exists for.
#[test]
fn a_multi_block_read_bypasses_the_cache() {
    let mut disk = CachedDisk::new(CountingDisk::new(2), 8);
    let mut buf = [0u8; BLOCK * 2];
    for _ in 0..3 {
        read(&mut disk, 0, &mut buf);
    }
    assert_eq!(
        disk.inner.reads, 3,
        "every multi-block read must reach the disk; none of them should have been a cache hit"
    );
}

/// [`CachedDisk`] over a real image behaves exactly as the uncached path does: same bytes back,
/// through the ordinary `Server` API rather than through `Disk` directly.
#[test]
fn cached_disk_reads_the_same_bytes_as_uncached() {
    let mut fs = FileSystem::create(BlockDisk(VecIo(vec![0u8; 16 * 1024 * 1024])), None, 0, 0)
        .expect("mkfs through BlockDisk");
    fs.tx(|transaction| {
        let ptr = transaction
            .create_node(TreePtr::root(), "f", Node::MODE_FILE | 0o644, 0, 0)?
            .ptr();
        transaction.write_node(ptr, 0, b"cached-disk-content", 0, 0)?;
        Ok(())
    })
    .expect("populate");
    let image = fs.disk.0.0;

    let mut plain = Server::open(BlockDisk(VecIo(image.clone()))).expect("open, uncached");
    let mut cached =
        Server::open(CachedDisk::new(BlockDisk(VecIo(image)), 8)).expect("open, cached");

    let mut buf_a = [0u8; 32];
    let mut buf_b = [0u8; 32];
    let ha = plain.open_file(&crate::tests::named("f")).unwrap();
    let hb = cached.open_file(&crate::tests::named("f")).unwrap();
    // Read twice through the cached path, so a caching bug would have a chance to show up.
    for _ in 0..2 {
        let na = plain.read(ha, 0, &mut buf_a).unwrap();
        let nb = cached.read(hb, 0, &mut buf_b).unwrap();
        assert_eq!(na, nb);
        assert_eq!(buf_a[..na], buf_b[..nb]);
    }
    assert_eq!(&buf_a[..19], b"cached-disk-content");
}

/// **The header ring is never cached**, so mounting an image does not fill the cache with 256
/// blocks nobody reads again (milestone 606).
#[test]
fn the_header_ring_is_never_cached() {
    let mut disk = CachedDisk::new(CountingDisk::new(2), 8);
    let mut buf = [0u8; BLOCK];
    for _ in 0..2 {
        read(&mut disk, 3, &mut buf);
    }
    assert_eq!(
        disk.inner.reads, 2,
        "a header-ring block was answered from the cache"
    );
}
