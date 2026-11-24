//! The second-mount repro, on the host (fix/redoxfs-second-mount).
//!
//! Runs the real engine under the allocator the FS server uses, `memory_region_heap::MemoryRegionHeap`
//! itself, over a host page source, grown incrementally and capped as `redoxfs_server.rs` caps it, and mounts a used image again and again in one process: create the fixture, mount and
//! write, drop the mount without unmounting, then mount again and write. The image lives in a
//! `static`, off the heap, as a disk reached over blk IPC is. A write that fails at the device's
//! 8 MiB cap and succeeds at 64 is heap exhaustion; partsfoundry/record:notes/fs-server/write-path-history.md records
//! what it measured.

//! # EXAMPLES
//!
//! ```text
//! cargo run --manifest-path redoxfs_server/Cargo.toml --bin second_mount            # 8 MiB, the device cap
//! NIFE_HEAP_MIB=64 cargo run --manifest-path redoxfs_server/Cargo.toml --bin second_mount
//! ```

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Mutex, Once};

use memory_region_heap::{MemoryRegionHeap, PageSource, Refusal};
use redoxfs::{FileSystem, Node, TreePtr};
use redoxfs_server::{BLOCK, BlockDisk, BlockIo, Server};

/// The pool the capped heap draws from. Large enough to serve any cap we dial in; the *cap*, not
/// the pool, is what limits the heap. Asked of the host's own allocator on the first allocation and
/// never returned, so it costs nothing in the binary and only touched pages cost memory.
const POOL_SIZE: usize = 512 * 1024 * 1024;

const PAGE: usize = 4096;

/// The heap cap in bytes, from `NIFE_HEAP_MIB`, defaulting to the FS server's 8 MiB
/// (`redoxfs_server.rs::HEAP_MAXIMUM`, which `FS_BUDGET_PAGES` in kernel/src/user.rs matches), and
/// never past the pool.
fn heap_cap() -> usize {
    std::env::var("NIFE_HEAP_MIB")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(8)
        .saturating_mul(1024 * 1024)
        .min(POOL_SIZE)
}

/// Set when an allocation fails once the heap is wired: the cap refused to grow, the signature of
/// exhaustion.
static HIT_CAP: AtomicBool = AtomicBool::new(false);

/// The host's page source: every page of the pool is already mapped and writable, because
/// [`System`] handed over the whole pool before the heap was wired.
struct PoolPages;

impl PageSource for PoolPages {
    fn map_page(&mut self, _virtual_address: u64) -> Result<(), Refusal> {
        Ok(())
    }
}

/// The FS server's heap over the pool, wired on the first allocation.
///
/// Wiring reads `NIFE_HEAP_MIB`, and reading the environment allocates, so the allocations made
/// while `wiring` runs go to [`System`] instead (`during_wiring`), and a block is given back to
/// whichever of the two its address says it came from.
struct CappedHeap {
    heap: MemoryRegionHeap<PoolPages>,
    wiring: Once,
    during_wiring: AtomicBool,
    pool: AtomicUsize,
}

impl CappedHeap {
    /// Read the cap, ask [`System`] for the pool, page-aligned as the device's heap band is, and
    /// wire the heap to it. `System`, not this binary's global allocator: this IS the global
    /// allocator. A refused pool leaves the heap unwired, so every allocation fails.
    fn wire(&self) {
        self.wiring.call_once(|| {
            self.during_wiring.store(true, Ordering::Relaxed);
            let cap = heap_cap();
            self.during_wiring.store(false, Ordering::Relaxed);
            let Ok(layout) = Layout::from_size_align(POOL_SIZE, PAGE) else {
                return;
            };
            // SAFETY: `layout` has a nonzero size. The block is never freed, so the heap's pages
            // stay valid for the life of the process.
            let raw = unsafe { System.alloc(layout) };
            if raw.is_null() {
                return;
            }
            let base = raw.expose_provenance();
            self.pool.store(base, Ordering::Relaxed);
            let _ = self.heap.initialize_source(base as u64, cap as u64);
        });
    }

    /// Whether `ptr` lies in the pool, and so came from the heap rather than from [`System`].
    fn in_pool(&self, ptr: *mut u8) -> bool {
        let base = self.pool.load(Ordering::Relaxed);
        base != 0 && (base..base + POOL_SIZE).contains(&ptr.addr())
    }

    /// The bytes the heap has committed, which only grows, so it is also the high-water mark.
    fn committed(&self) -> usize {
        self.heap.committed_bytes() as usize
    }
}

// SAFETY: forwards the GlobalAlloc contract to `MemoryRegionHeap`, whose pool never moves, or to
// `System` for what wiring itself allocates; `dealloc` returns each block to the allocator whose
// range holds it.
unsafe impl GlobalAlloc for CappedHeap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if self.during_wiring.load(Ordering::Relaxed) {
            // SAFETY: forwarded GlobalAlloc contract.
            return unsafe { System.alloc(layout) };
        }
        self.wire();
        // SAFETY: forwarded GlobalAlloc contract.
        let p = unsafe { self.heap.alloc(layout) };
        if p.is_null() && self.heap.is_initialized() {
            HIT_CAP.store(true, Ordering::Relaxed); // OOM: std's handler aborts, as on device
        }
        p
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if self.in_pool(ptr) {
            // SAFETY: forwarded GlobalAlloc contract; `ptr` came from the heap's `alloc`.
            unsafe { self.heap.dealloc(ptr, layout) };
        } else {
            // SAFETY: forwarded GlobalAlloc contract; `ptr` came from `System` during wiring.
            unsafe { System.dealloc(ptr, layout) };
        }
    }
}

#[global_allocator]
static HEAP: CappedHeap = CappedHeap {
    heap: MemoryRegionHeap::with_source(PoolPages),
    wiring: Once::new(),
    during_wiring: AtomicBool::new(false),
    pool: AtomicUsize::new(0),
};

/// The image, in a `static` and therefore OFF the heap under test, because on device the image is on
/// a disk and never sits in the FS server's heap.
///
/// A `Mutex` since milestone 891 (a mutable static becomes an atomic, a lock or a recorded cell):
/// one thread uses it, so the lock is never contended, and holding it is what lets a block be read
/// and written without `unsafe`.
const IMAGE_SIZE: usize = 16 * 1024 * 1024;
static IMAGE: Mutex<[u8; IMAGE_SIZE]> = Mutex::new([0; IMAGE_SIZE]);

/// A `BlockIo` over the static image. Both mounts address the same bytes, exactly as they would the
/// same disk across two boots.
struct StaticImage;

/// The image, locked. A poisoned lock means a write panicked partway, and the bytes are still the
/// bytes that write left, which is exactly what this repro wants to read back.
fn image() -> std::sync::MutexGuard<'static, [u8; IMAGE_SIZE]> {
    IMAGE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
impl BlockIo for StaticImage {
    fn read_block(&mut self, block: u64, buf: &mut [u8; BLOCK]) -> syscall::error::Result<()> {
        let off = block as usize * BLOCK;
        buf.copy_from_slice(&image()[off..off + BLOCK]);
        Ok(())
    }
    fn write_block(&mut self, block: u64, buf: &[u8; BLOCK]) -> syscall::error::Result<()> {
        let off = block as usize * BLOCK;
        image()[off..off + BLOCK].copy_from_slice(buf);
        Ok(())
    }
    fn size_bytes(&mut self) -> syscall::error::Result<u64> {
        Ok(IMAGE_SIZE as u64)
    }
}

/// Print without allocating, so a report still comes out when the heap is exhausted.
fn mark(s: &str) {
    use std::io::Write;
    let mut e = std::io::stderr();
    let _ = e.write_all(s.as_bytes());
    let _ = e.write_all(b"\n");
    let _ = e.flush();
}

/// The payload every write in this repro uses: same length each time, so a write replaces the last
/// completely (there is no truncate verb), and position-dependent so a stale read cannot match.
fn payload(pass: u8) -> [u8; 61] {
    let mut p = [0u8; 61];
    p[..8].copy_from_slice(b"CRKSND__");
    p[7] = b'0' + pass;
    for (i, b) in p.iter_mut().enumerate().skip(8) {
        *b = (i as u8).wrapping_mul(31) ^ pass;
    }
    p
}

/// Mount the static image, write `pass`'s payload to `scratch`, read it back, and report. Returns the
/// error if any step failed, so the caller can print the errno rather than guess at it.
fn mount_and_write(pass: u8) -> syscall::error::Result<()> {
    let mut srv = Server::open(BlockDisk(StaticImage))?;
    let h = srv.open_file(
        &redoxfs_server::NameCopy::copy_in(("scratch").as_bytes()).expect("a fixed name copies"),
    )?;
    let p = payload(pass);
    let n = srv.write(h, 0, &p)?;
    assert_eq!(n, p.len(), "short write on mount {pass}");
    let mut back = [0u8; 128];
    let m = srv.read(h, 0, &mut back)?;
    assert_eq!(&back[..m], &p[..], "mount {pass} did not read back");
    Ok(())
}

/// How many mount-and-write cycles to run. Two is the reported case; a larger count tests whether the
/// failure is *accumulated* state (each cycle advances the header generation, lengthens the allocator
/// log, and leaves more live tree blocks) rather than merely "not the first".
fn mount_count() -> u8 {
    std::env::var("NIFE_MOUNTS")
        .ok()
        .and_then(|s| s.parse::<u8>().ok())
        .unwrap_or(2)
}

fn main() {
    let cap_mebibytes = heap_cap() / 1024 / 1024;
    let mounts = mount_count();
    mark(&format!(
        "second_mount: heap cap {cap_mebibytes} MiB, {mounts} mount cycles \
         (NIFE_HEAP_MIB / NIFE_MOUNTS override)"
    ));

    // Build the fixture the way the host tool does: an empty filesystem plus the two files.
    let mut fs = FileSystem::create(BlockDisk(StaticImage), None, 0, 0).expect("mkfs");
    fs.tx(|transaction| {
        let motd = transaction
            .create_node(TreePtr::root(), "motd", Node::MODE_FILE | 0o644, 0, 0)?
            .ptr();
        transaction.write_node(motd, 0, b"motd\n", 0, 0)?;
        let scratch = transaction
            .create_node(TreePtr::root(), "scratch", Node::MODE_FILE | 0o644, 0, 0)?
            .ptr();
        transaction.write_node(scratch, 0, b"(placeholder)", 0, 0)?;
        Ok(())
    })
    .expect("populate");
    drop(fs);

    // Mount 1 is the write that always worked, and it is what makes the image "used". Every mount
    // after it is a second mount of a used image, the case that fails on device, and each one leaves
    // the image more used than the last.
    for pass in 1..=mounts {
        match mount_and_write(pass) {
            Ok(()) => mark(&format!(
                "mount {pass}: write OK (heap {} KiB)",
                HEAP.committed() / 1024
            )),
            Err(e) => {
                mark(&format!("mount {pass}: FAILED errno {}", e.errno));
                report_and_exit(1);
            }
        }
    }

    report_and_exit(0);
}

/// Report the heap high-water mark and whether the cap ever refused a growth, then exit.
fn report_and_exit(code: i32) -> ! {
    let hwm = HEAP.committed();
    mark(&format!(
        "heap high-water {} KiB of {} KiB cap; cap refused a growth: {}",
        hwm / 1024,
        heap_cap() / 1024,
        HIT_CAP.load(Ordering::Relaxed)
    ));
    std::process::exit(code);
}
