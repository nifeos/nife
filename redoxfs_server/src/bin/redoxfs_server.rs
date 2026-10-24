//! The FS-server EL0 binary (milestone 32 phase 2): RedoxFS, confined, served over IPC.
//!
//! The two IO edges of [`redoxfs_server::Server`] and a dedicated binary's runtime. [`IpcDisk`] is
//! the RedoxFS `Disk` as a blk-IPC client, up to `blk::TRANSFER_BLOCKS` blocks per `CALL`
//! (milestone 138 step 4), wrapped in [`redoxfs_server::CachedDisk`] (step 2). [`serve`] answers
//! requests through the one-shot Reply the kernel mints. The heap is untyped-backed, so RedoxFS
//! pays from this process's own budget. It only opens the image, never creates (creation is
//! std-gated and host-side). See notes/fs-server/caches-and-io.md.

//! # Capability contract (notes/fs-server.md, notes/abi.md §4)
//! - **slot 0**: an untyped budget, the heap's (RedoxFS is alloc-heavy; nothing runs without it).
//! - **slot 1**: the block-service endpoint, `WRITE`. The FS server is the block server's client.
//! - **slot 2**: the file-service endpoint, `READ`. Clients `CALL` here; this is the directory
//!   capability, bound in the server to the image's root (phase 2). A client without it opens
//!   nothing.
//! - **[`BLK_PAGE`]**: the base of the block channel shared with the block server, `blk::TRANSFER_MAX`
//!   bytes of contiguous pages (the block buffer).
//! - **[`FILE_PAGE`]**: the base of the file channel shared with the client, `fs::TRANSFER_MAX`
//!   bytes of contiguous pages (a name on open, file bytes on read/write).

//! # BUGS
//!
//! - Window 0 is still shared by the boot's long-lived clients (milestone 599 (a frame per
//!   filesystem client channel)): the shell, `login`, the identity provisioner and the progenitor's
//!   own activation calls. They are kept apart by being blocked, not by mapping: the progenitor
//!   calls only while the shell waits on it, and nothing connects to `login` in the shipped boot. A
//!   job behind a directory grant gets a window of its own. See `notes/page-frame-slice.md`.

#![no_std]
#![no_main]

// **This server enforces subtree grants itself** (milestone 606 (a directory walk costs what it
// does on Linux), calef's rulings D and T1 of 2026-09-27): every path goes through
// `subtree_scope::walk` and every handle through `subtree_scope::admit`, in `redoxfs_server`'s
// core. The note is what the progenitor reads to give this server's clients a bound badge instead of
// a caretaker; `script/lint` checks it against the package declaration and the dependency.
capability_manifest_note::carry_subtree_grants!(capability_manifest_note::Scope::SubtreeScope);

extern crate alloc;

use filesystem_contract::{blk, fs};
use redoxfs::Disk;
use redoxfs_server::{CachedDisk, ServeEdges, Server};
use syscall::error::{EIO, Error, Result};
use user_mode_runtime::{Reply, call, receive_request, send};

/// Capability table slots, by convention with the kernel-side wiring (`kernel/src/user/fs_service.rs`).
const MEMORY_REGION: u64 = 0;
const BLK: u64 = 1;
const FILE: u64 = 2;
/// A readiness endpoint: the server SENDs one word here once the image is open, before it serves.
const READY: u64 = 3;

/// Where the kernel maps the two shared regions: the block channel, a service window on the
/// address-space map (`crates/address_space_map`), above the heap band and below the image band.
///
/// [`BLK_PAGE`] is `blk::TRANSFER_MAX` bytes wide rather than one page (milestone 138 step 4) and
/// [`FILE_PAGE`] is `fs::TRANSFER_MAX` bytes wide rather than one page (step 3); [`FILE_PAGE`] sits
/// above [`BLK_PAGE`] by exactly `blk::TRANSFER_MAX` so growing either region stays inside the 8 MiB
/// nothing else this process maps comes within.
const BLK_PAGE: u64 = address_space_map::service_window(0x5000_0000);
const FILE_PAGE: u64 = BLK_PAGE + blk::TRANSFER_MAX as u64;

/// One filesystem block, in bytes: the unit [`BLK_PAGE`] is carved into. The transfer unit for
/// [`fs::READDIR`] and every other verb whose reply the server itself sizes; a [`fs::READ`] or
/// [`fs::WRITE`] moves up to `fs::TRANSFER_MAX` of these through [`FILE_PAGE`] instead.
const BLOCK: usize = blk::BLOCK_SIZE;

/// The heap cap. RedoxFS keeps a compress buffer sized by `RECORD_SIZE` (128 KiB, still the ceiling after milestone
/// 138 lowered the *created* record level, because the buffer must fit any record this build can
/// rewrite), block buffers, and small
/// tree structures; a few MiB is comfortable for the small images phase 2 serves. The untyped the
/// kernel grants is the real ceiling.
const HEAP_MAX: u64 = 8 * 1024 * 1024;

/// How many single blocks [`redoxfs_server::CachedDisk`] holds (milestone 138 (close the read
/// gap) step 2, resized by milestone 606 (a directory walk costs what it does on Linux)). The number
/// and its measurement live in the library, so the host walk test runs this binary's cache. One
/// constant serves every FS server this build starts, milestone 37's crash-test instances on 2 MiB
/// included (`CRASH_BUDGET_PAGES` in `kernel/src/user/fs_service.rs`), because a slot is allocated
/// only when a block lands in it and the header ring is never cached.
const CACHE_SLOTS: usize = redoxfs_server::CACHE_SLOTS;

#[global_allocator]
static HEAP: memory_region_heap::MemoryRegionHeap = memory_region_heap::MemoryRegionHeap::new();

/// Read every written block straight back and compare (a `fix/redoxfs-repeat-write` diagnostic). Off
/// by default: it doubles the write cost, and its scratch block is 4 KiB of stack inside a call
/// RedoxFS makes from deep recursion, which is enough to overflow this server's stack and produce a
/// *different* failure than the one being chased. Turn it on deliberately, and raise the FS server's
/// stack grant if you do.
const VERIFY_WRITES: bool = false;

/// The crash injector (milestone 37, DECISIONS §34 condition 1), armed by this process's START
/// arguments and inert otherwise. The seam is [`IpcDisk`], and the `Spawn` literal arms it, so a
/// boot that does not ask for a crash cannot get one. See notes/fs-server/crash-consistency.md.
///
/// - `arg0` (`CRASH_AT_WRITE`): which file-service `WRITE` request to die inside, counted from 1.
///   Zero disables the injector, as on every boot but the crash test's.
/// - `arg1` (`CRASH_AFTER_BLOCKS`): block writes into that request to let through before dying.
///   One in the test, because a write transaction always issues at least one.
/// - `arg2` (`CRASH_TEAR_BYTES`): bytes of that last block laid over its old contents.
mod inject {
    use core::sync::atomic::{AtomicU64, Ordering};

    /// Which `WRITE` request to die in, from 1. Zero means never.
    pub static AT_WRITE: AtomicU64 = AtomicU64::new(0);
    /// Block writes to allow inside that request before dying.
    pub static AFTER_BLOCKS: AtomicU64 = AtomicU64::new(0);
    /// Bytes of the final block that reach the platter (0 means the write never happens at all).
    pub static TEAR_BYTES: AtomicU64 = AtomicU64::new(0);
    /// How many `WRITE` requests the serve loop has begun.
    pub static WRITES_SEEN: AtomicU64 = AtomicU64::new(0);
    /// Block writes issued since the armed request began. Only counted once armed.
    pub static BLOCKS: AtomicU64 = AtomicU64::new(0);
    /// Set when the serve loop enters the request the injector names.
    pub static ARMED: AtomicU64 = AtomicU64::new(0);

    /// Called by the serve loop as it begins a `WRITE`. Arms the injector if this is the one.
    pub fn note_write() {
        let at = AT_WRITE.load(Ordering::Relaxed);
        if at != 0 && WRITES_SEEN.fetch_add(1, Ordering::Relaxed) + 1 == at {
            ARMED.store(1, Ordering::Relaxed);
        }
    }

    /// Called by [`super::IpcDisk`] before each block write. `Some(n)` means "this is the one: put
    /// only `n` bytes of it on the platter and then die".
    pub fn tear_now() -> Option<usize> {
        if ARMED.load(Ordering::Relaxed) == 0 {
            return None;
        }
        let n = BLOCKS.fetch_add(1, Ordering::Relaxed) + 1;
        (n == AFTER_BLOCKS.load(Ordering::Relaxed))
            .then(|| TEAR_BYTES.load(Ordering::Relaxed) as usize)
    }
}

/// The RedoxFS `Disk` over blk IPC. Stateless: everything it needs (the endpoint slot, the shared
/// page) is a fixed convention, so it is a zero-sized handle the `Server` owns.
struct IpcDisk;

impl IpcDisk {
    /// One blk `CALL` for `count` contiguous blocks starting at `block` (milestone 138 step 4):
    /// opcode and count pack into the first word ([`filesystem_contract::blk::req`]), the starting block index
    /// is the second. Returns the reply's first word as a signed result (negative is an error, per
    /// the wire convention). The bulk rides in [`BLK_PAGE`], `count * BLOCK` bytes of it.
    fn blk_n(operation_code: u64, block: u64, count: usize) -> i64 {
        // SAFETY: `call` traps to the kernel, which validates the endpoint in slot BLK.
        let (r0, _) = call(BLK, blk::req(operation_code, count), block);
        r0 as i64
    }

    /// [`Self::blk_n`] for exactly one block: every call this file made before milestone 138 step
    /// 4, and still the right shape for [`blk::SIZE`] and [`blk::FLUSH`], which ignore the count.
    fn blk(operation_code: u64, block: u64) -> i64 {
        Self::blk_n(operation_code, block, 1)
    }

    /// Copy `n` bytes out of the shared block region (a completed read landed there, at its start).
    fn from_page(dst: &mut [u8]) {
        // SAFETY: BLK_PAGE is a mapped, writable region of exactly blk::TRANSFER_MAX bytes; `dst`
        // is no larger (every caller passes at most blk::TRANSFER_BLOCKS * BLOCK).
        unsafe {
            core::ptr::copy_nonoverlapping(BLK_PAGE as *const u8, dst.as_mut_ptr(), dst.len())
        }
    }

    /// Copy `src` into the shared block region (to be written), at its start. `src` is at most
    /// `blk::TRANSFER_MAX` bytes.
    fn to_page(src: &[u8]) {
        // SAFETY: as above; `src` is no larger than the region.
        unsafe { core::ptr::copy_nonoverlapping(src.as_ptr(), BLK_PAGE as *mut u8, src.len()) }
    }

    /// **The write path exactly as it stood before milestone 138 step 4**: one CALL per block, so
    /// the crash injector can stop between any two of them, and [`VERIFY_WRITES`] can read every
    /// one straight back. [`Disk::write_at`] falls back to this whenever either might apply; see
    /// that method for why batching cannot serve them.
    fn write_at_unbatched(block: u64, buffer: &[u8]) -> Result<usize> {
        for (i, chunk) in buffer.chunks(BLOCK).enumerate() {
            let b = block + i as u64;
            // The crash injection (milestone 37), and the only line the ordinary path pays for it.
            if let Some(tear) = inject::tear_now() {
                // A real torn write: the first `tear` new bytes over the old block, which is what a
                // drive leaves when the rail collapses mid-block.
                if tear > 0 {
                    if Self::blk(blk::READ, b) < 0 {
                        return Err(Error::new(EIO));
                    }
                    Self::to_page(&chunk[..tear.min(chunk.len())]);
                    if Self::blk(blk::WRITE, b) < 0 {
                        return Err(Error::new(EIO));
                    }
                }
                // Then die with the commit unwritten, announcing it first on the readiness endpoint
                // so the waiting test knows the kill was the injector's.
                send(READY, filesystem_fixture::crash::CUT, 0, 0);
                panic!();
            }
            if chunk.len() < BLOCK {
                // A partial final block would clobber the rest of the block; read-modify-write so
                // only the given bytes change. RedoxFS does not do this today, but a Disk owes it.
                if Self::blk(blk::READ, b) < 0 {
                    return Err(Error::new(EIO));
                }
            }
            Self::to_page(chunk);
            if Self::blk(blk::WRITE, b) < 0 {
                return Err(Error::new(EIO));
            }
            // WRITE-VERIFY: a lossy transport would later make the engine walk a corrupt allocator
            // chain and spin; reading the block back here turns that into a located fault.
            if VERIFY_WRITES {
                let mut echo = [0u8; BLOCK];
                if Self::blk(blk::READ, b) < 0 {
                    return Err(Error::new(EIO));
                }
                Self::from_page(&mut echo);
                if echo[..chunk.len()] != *chunk {
                    // The block did not read back as written: the transport lost or reordered it.
                    panic!();
                }
            }
        }
        Ok(buffer.len())
    }
}

impl Disk for IpcDisk {
    unsafe fn read_at(&mut self, block: u64, buffer: &mut [u8]) -> Result<usize> {
        // Whole blocks, up to blk::TRANSFER_BLOCKS per CALL (milestone 138 step 4); a short final
        // chunk (a compressed record) is read as one whole block with only its bytes copied out.
        // The five tree-spine reads arrive as separate calls, and [`CachedDisk`] answers those.
        let mut off = 0usize;
        let mut b = block;
        while off < buffer.len() {
            let remaining = buffer.len() - off;
            let full_blocks = (remaining / BLOCK).min(blk::TRANSFER_BLOCKS);
            if full_blocks > 0 {
                let n = full_blocks * BLOCK;
                if Self::blk_n(blk::READ, b, full_blocks) < 0 {
                    return Err(Error::new(EIO));
                }
                Self::from_page(&mut buffer[off..off + n]);
                off += n;
                b += full_blocks as u64;
                continue;
            }
            // Only a sub-block tail remains (remaining < BLOCK): one whole-block read, partial copy.
            if Self::blk(blk::READ, b) < 0 {
                return Err(Error::new(EIO));
            }
            Self::from_page(&mut buffer[off..]);
            off = buffer.len();
        }
        Ok(buffer.len())
    }

    unsafe fn write_at(&mut self, block: u64, buffer: &[u8]) -> Result<usize> {
        // The crash injector and `VERIFY_WRITES` need one CALL per block (a batched virtio request
        // completes whole), so either takes the unbatched path; both are off on an ordinary server.
        if VERIFY_WRITES || inject::AT_WRITE.load(core::sync::atomic::Ordering::Relaxed) != 0 {
            return Self::write_at_unbatched(block, buffer);
        }
        let mut off = 0usize;
        let mut b = block;
        while off < buffer.len() {
            let remaining = buffer.len() - off;
            let full_blocks = (remaining / BLOCK).min(blk::TRANSFER_BLOCKS);
            if full_blocks > 0 {
                let n = full_blocks * BLOCK;
                Self::to_page(&buffer[off..off + n]);
                if Self::blk_n(blk::WRITE, b, full_blocks) < 0 {
                    return Err(Error::new(EIO));
                }
                off += n;
                b += full_blocks as u64;
                continue;
            }
            // A short final chunk: read-modify-write one block so the rest of it survives. RedoxFS
            // does not send one today, but a Disk owes the courtesy.
            let tail = &buffer[off..];
            if Self::blk(blk::READ, b) < 0 {
                return Err(Error::new(EIO));
            }
            Self::to_page(tail);
            if Self::blk(blk::WRITE, b) < 0 {
                return Err(Error::new(EIO));
            }
            off = buffer.len();
        }
        Ok(buffer.len())
    }

    fn size(&mut self) -> Result<u64> {
        let n = Self::blk(blk::SIZE, 0);
        if n < 0 {
            return Err(Error::new(EIO));
        }
        Ok(n as u64)
    }
}

impl IpcDisk {
    /// Make the device durable (milestone 55 (time Machine: SMB3 with Apple's extensions, and mDNS)): one `filesystem_contract::blk::FLUSH`, and the block
    /// server's answer handed back untouched.
    ///
    /// Not part of `Disk`, which has no sync method; the engine's transactions commit before this
    /// server replies, so the flush is a fact about the device. A negative errno is returned
    /// unmapped, the one place a block-protocol errno reaches a file-service client: `EOPNOTSUPP`
    /// (no flush) and `EIO` (a refused one) are different facts. See
    /// `filesystem_contract::fs::SYNC`.
    fn sync() -> i64 {
        Self::blk(blk::FLUSH, 0)
    }
}

/// The file-service channel, as a slice. The FS server reads a name (open) or file bytes (write)
/// from [`FILE_PAGE`] and writes read results back into it.
///
/// # Safety
/// `len` must be at most [`fs::TRANSFER_MAX`]. [`FILE_PAGE`] is the base of that many mapped,
/// writable, contiguous bytes shared with the clients bound to this endpoint, and every call site
/// clamps before this runs. The returned slice is `'static` and aliases that region, so no two live
/// slices from here may overlap in a way that outlives one request.
unsafe fn file_page(base: u64, len: usize) -> &'static mut [u8] {
    unsafe { core::slice::from_raw_parts_mut(base as *mut u8, len) }
}

/// The base of the client channel a request's badge names (milestone 599). Window `badge` sits
/// `badge * fs::TRANSFER_MAX` above [`FILE_PAGE`]; badge 0 is window 0, the unbadged default. A
/// badge at or past [`fs::CLIENT_WINDOWS`] would point outside the mapped region, so it is clamped
/// to window 0 rather than trusted: the kernel only ever delivers a badge this server's own wiring
/// stamped, so an out-of-range one is a wiring bug, and folding it onto window 0 fails loudly (two
/// clients would then collide and the witness would catch it) rather than reading unmapped memory. `admit` refuses such a badge before any verb reads the
/// window (milestone 726 (an unknown badge fails closed in subtree_scope)), so the clamp is only the second line.
fn window_base(badge: u64) -> u64 {
    let w = if (badge as usize) < fs::CLIENT_WINDOWS {
        badge
    } else {
        0
    };
    FILE_PAGE + w * fs::TRANSFER_MAX as u64
}

/// Answer a caller through its one-shot Reply capability, then return to serving. `None` is a
/// request nobody is waiting on (a plain `SEND`, or a `SEND_CAP` whose capability
/// `receive_request` deleted), which gets no answer (milestone 706 (a `CALL` server can tell a Reply
/// from a delegation)).
fn reply(to: Option<Reply>, r0: i64, r1: u64) {
    if let Some(to) = to {
        user_mode_runtime::reply(to, r0 as u64, r1);
    }
}

/// [`redoxfs_server::Server::handle`]'s two IO edges, over block IPC: the device flush behind
/// `fs::SYNC`, and the crash injector's count of `WRITE` requests (milestone 37 (prove RedoxFS's crash
/// consistency)).
struct IpcEdges;

impl ServeEdges for IpcEdges {
    fn note_write(&mut self) {
        inject::note_write();
    }

    fn sync(&mut self) -> i64 {
        IpcDisk::sync()
    }
}

/// The serve loop. Blocks on the file-service endpoint, dispatches one request, replies, repeats.
/// The dispatch, and the one place a RedoxFS error becomes a wire value, is
/// [`redoxfs_server::Server::handle`] in the library, so the host fuzz target runs it too; this loop
/// is only the IO around it.
fn serve(server: &mut Server<CachedDisk<IpcDisk>>) -> ! {
    loop {
        // RECEIVE_CAP delivers the first word, the Reply's slot, the second word and the caller's
        // badge. The badge (milestone 599) names the client's own window, which is how two clients
        // are kept apart; a plain `SEND` carries its badge too (milestone 613 (a system log service:
        // the in-memory half), amending §230 (badged endpoint capabilities); `system_log_tests`
        // pins it). The Reply is typed by the kernel's `x4` (milestone 706, DECISIONS §245 (a
        // `CALL` server tells a Reply from a delegation)), so a `SEND_CAP`'d capability is deleted.
        let req = receive_request(FILE);
        let reply_slot = req.delivered.into_reply();
        // SAFETY: `fs::TRANSFER_MAX` is exactly the window's mapped length, and this slice is the
        // only one taken from it until the request is answered.
        let window = unsafe { file_page(window_base(req.badge), fs::TRANSFER_MAX) };
        let (r0, r1) = server.handle(req.badge, req.w0, req.w1, window, &mut IpcEdges);
        reply(reply_slot, r0, r1);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn _start(crash_at_write: u64, crash_after_blocks: u64, crash_tear_bytes: u64) -> ! {
    // The crash injector's arming, straight out of the START arguments (milestone 37). All zero on
    // every boot but the crash test's, and `AT_WRITE == 0` is what makes the injector inert.
    {
        use core::sync::atomic::Ordering;
        inject::AT_WRITE.store(crash_at_write, Ordering::Relaxed);
        inject::AFTER_BLOCKS.store(crash_after_blocks, Ordering::Relaxed);
        inject::TEAR_BYTES.store(crash_tear_bytes, Ordering::Relaxed);
    }
    HEAP.init(MEMORY_REGION, memory_region_heap::DEFAULT_BASE, HEAP_MAX);

    // A bad image, or a block server that never answers correctly, faults here; the server never
    // creates. CachedDisk holds the tree spine's single-block reads (milestone 138 step 2).
    let mut server = match Server::open(CachedDisk::new(IpcDisk, CACHE_SLOTS)) {
        Ok(s) => s,
        Err(_) => panic!(), // not a RedoxFS image, or the disk misbehaved: die legibly.
    };
    // The image is open: signal readiness (so the test can tell an open-path hang from a serve-path
    // one), then serve forever.
    send(READY, filesystem_contract::READY, 0, 0);
    serve(&mut server);
}

// A server fault is a dead server: trap, and the kernel reaps it legibly.
user_mode_runtime::panic_handler!();
