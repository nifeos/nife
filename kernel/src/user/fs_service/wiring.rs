//! Wiring the service: the block server and the FS server, spawned once per boot and
//! remembered, and the readiness sentinels a caller drains before either is running.
//!
//! See notes/fs-server/kernel-wiring.md.
//!
//! Name: provisional (lane `maintainer/last-three-over-2000`, 2026-10-12 UTC), for the module and
//! the file.

use super::*;

/// The block server's role in the driver binary (must match `components/src/block_driver.rs` and
/// `crates/virtio`).
const ROLE_BLK_SERVER: u64 = 32;

/// The heap budget the FS server draws RedoxFS's allocations from. RedoxFS keeps a 128 KiB
/// compress buffer, block buffers, and small tree structures for the images phase 2 serves; 8 MiB
/// is comfortable and is the process's hard ceiling (its `HEAP_MAX` matches).
pub(super) const FS_BUDGET_PAGES: u64 = 2048;

/// One boot, one FS service, remembered here: a second wiring would put a second driver on the
/// same virtio slot and re-bind its interrupt. The first caller receives the two readiness
/// endpoints, later callers `None`. Plain atomics: the only writer is the boot or test thread.
pub(super) static WIRED: core::sync::atomic::AtomicBool =
    core::sync::atomic::AtomicBool::new(false);
static FILE_EP: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
pub(super) static FILE_SHARED: core::sync::atomic::AtomicU64 =
    core::sync::atomic::AtomicU64::new(0);

/// The wired service: the file-service endpoint clients `CALL`, the physical frame they share
/// with the FS server, and (only on the call that did the wiring) the block server's and FS
/// server's readiness endpoints.
///
/// The block server's half is itself an `Option`: a boot that mounts the NVMe disk borrows the
/// server `non_volatile_memory_express_service` runs, whose sentinel may already be taken. `None`
/// there means "already up", a different fact from the outer `None`'s "somebody else wired it".
type Service = (RendezvousId, u64, Readiness);

/// The two sentinels a caller has to drain before the service is running, and `None` when an
/// earlier caller in this boot already drained them.
///
/// The block server's half is itself an `Option` for the reason [`Service`] gives above: a boot
/// that borrows an already-running block service has no sentinel of its own left to wait for.
pub type Readiness = Option<(Option<RendezvousId>, RendezvousId)>;

/// Wire the block server and the FS server if this boot has not already, else hand back what is
/// already running. `None` means no RedoxFS disk is attached.
pub(super) fn ensure(blk_image: &'static [u8], fs_server_image: &'static [u8]) -> Option<Service> {
    use core::sync::atomic::Ordering;

    if WIRED.load(Ordering::Acquire) {
        return Some((
            FILE_EP.load(Ordering::Relaxed),
            FILE_SHARED.load(Ordering::Relaxed),
            None,
        ));
    }
    let (blk_ready, ready, file_ep, file_shared) = wire_servers(blk_image, fs_server_image)?;
    FILE_EP.store(file_ep, Ordering::Relaxed);
    FILE_SHARED.store(file_shared, Ordering::Relaxed);
    WIRED.store(true, Ordering::Release);
    Some((file_ep, file_shared, Some((blk_ready, ready))))
}

/// Wire and spawn the block server and the FS server. `blk_image` is the driver binary carrying
/// the block-server role (hello on aarch64, `block_driver` on riscv); `fs_server_image` is the same on
/// both ISAs. Returns `(blk_ready, ready, file_ep, file_shared)`, where `blk_ready` is `None` when
/// the block service was already running (see [`nvme_disk`]).
fn wire_servers(
    blk_image: &'static [u8],
    fs_server_image: &'static [u8],
) -> Option<(Option<RendezvousId>, RendezvousId, RendezvousId, u64)> {
    // The NVMe arm is for a machine with no virtio block device at all (an installed system), so
    // it tests device 0, not device 1: a boot with one virtio disk and an NVMe controller must not
    // hand the FS server a disk with no filesystem on it.
    let (blk_ep, blk_ready, blk_shared) = match crate::virtio::find_block_device_n(1) {
        Some(dev) => {
            let (ep, ready, shared) = spawn_block_server(blk_image, dev);
            (ep, Some(ready), shared)
        }
        // Only when the boot came from that disk: RedoxFS writes on mount, so a stick booted on an
        // installed machine must not mount it (`boot_slot::medium`, calef, PR #1652, 2026-10-04).
        None if crate::virtio::find_block_device_n(0).is_none()
            && crate::memory::booted_from_nvme() =>
        {
            nvme_disk()?
        }
        None => return None,
    };
    // One contiguous run of every window (calef, 2026-09-27), so one `PageFrame` capability names
    // the pool and the progenitor slices a window per client (`abi::page_frame::SLICE`).
    let file_shared = file_pool();
    for (w, window) in WINDOWS.iter().enumerate() {
        window.store(
            file_shared + (w * FILE_PAGES) as u64 * FRAME_SIZE,
            core::sync::atomic::Ordering::Relaxed,
        );
    }
    // Window 0 is the default, not a claimable channel: mark it taken so `claim_window` skips it.
    WINDOW_TAKEN.store(1, core::sync::atomic::Ordering::Relaxed);
    let file_ep = crate::sched::create_rendezvous(); // client WRITE (CALL) -> FS server READ
    let ready = crate::sched::create_rendezvous(); // FS server WRITE -> the kernel test receives
    spawn_fs_server(
        fs_server_image,
        FsServer {
            slot: 0,
            blk_ep,
            blk_shared,
            file_ep,
            file_shared,
            ready,
            budget_pages: FS_BUDGET_PAGES,
            crash: (0, 0, 0),
            // The main service maps every pool window; a client's badge picks which one it reads.
            extra_windows: CLIENT_WINDOWS - 1,
        },
    );
    Some((blk_ready, ready, file_ep, file_shared))
}

/// The NVMe disk, when this machine has no virtio one: what an installed system boots through.
/// Returns its request endpoint, its readiness endpoint if still undrained, and its transfer
/// region. `None` when there is no NVMe controller.
///
/// BUGS: the FS server is given the whole disk, not the nife data partition, so it can address the
/// EFI system partition and the table. See notes/fs-server/kernel-wiring.md.
fn nvme_disk() -> Option<(RendezvousId, Option<RendezvousId>, u64)> {
    let image = crate::user::program("non_volatile_memory_express")?;
    let wiring = crate::user::non_volatile_memory_express_service::ensure(image)?;
    Some((wiring.request, wiring.ready, wiring.transfer_phys))
}

/// Bring one virtio block device `dev`, on any bus, up under a confined userspace block server.
/// Returns `(blk_ep, blk_ready, blk_shared)`: the request endpoint, the readiness endpoint, and the
/// physical base of the region transfers land in.
///
/// The DMA region is `1 + BLK_PAGES` contiguous pages: page 0 for the rings, request header and
/// status, then the data pages, which are also the region shared with the client, so the device
/// DMAs up to `BLK_PAGES` blocks straight into it. The crash test and `disk_service` start their
/// own. See notes/fs-server/block-server.md.
pub(in crate::user) fn spawn_block_server(
    blk_image: &'static [u8],
    dev: crate::virtio::BlockDevice,
) -> (RendezvousId, RendezvousId, u64) {
    // Zeroed so neither stale descriptors nor stale file bytes are ever visible to the device
    // or the FS server.
    let dma = crate::memory::alloc_contiguous_zeroed(1 + BLK_PAGES)
        .expect("no DMA region for the block server")
        .addr();
    let blk_shared = dma + FRAME_SIZE; // the data pages start right after the rings page

    let blk_ep = crate::sched::create_rendezvous(); // FS server WRITE (CALL) -> block server READ
    let blk_ready = crate::sched::create_rendezvous(); // block server WRITE -> the kernel test receives

    let irq_ep = crate::sched::create_rendezvous();
    crate::sched::bind_irq(dev.intid, irq_ep);
    crate::arch::irq::enable(dev.intid);
    let vid = crate::virtio::register(
        dev.transport,
        dma,
        (1 + BLK_PAGES) as u64 * FRAME_SIZE, // every page: the device may touch the rings AND the data buffer
        dev.rid, // `Some` behind an IOMMU (every PCI function), `None` on virtio-mmio
    );
    crate::sched::spawn(move || {
        // The rings page, then the BLK_PAGES data pages, contiguous at DMA_VA.
        let mut maps = [Mapping {
            va: 0,
            phys: 0,
            flags: Flags::user_data(),
        }; 1 + BLK_PAGES];
        maps[0] = Mapping {
            va: DMA_VA,
            phys: dma,
            flags: Flags::user_data(),
        };
        map_channel(&mut maps[1..], DMA_VA + FRAME_SIZE, blk_shared, BLK_PAGES);
        run(
            blk_image,
            Spawn {
                arg0: ROLE_BLK_SERVER,
                arg1: dma, // the DMA region's physical address
                arg2: 0,
                grants: &[
                    rendezvous_cap(blk_ep, Rights::READ), // slot 0: RECEIVE blk requests
                    irq_cap(dev.intid),                   // slot 1: the device interrupt
                    virtio_cap(vid),                      // slot 2: the confined transport
                    rendezvous_cap(blk_ready, Rights::WRITE), // slot 3: signal readiness once
                ],
                maps: &maps,
            },
        )
    })
    .expect("could not spawn the block server");

    (blk_ep, blk_ready, blk_shared)
}

/// Everything one FS-server process is, as a value, so a caller cannot swap its bare `u64`
/// endpoint ids.
pub(super) struct FsServer {
    /// Which bank of [`FS_STACK_PHYS`] this process's poisoned stack is recorded in. One per
    /// FS server a boot can start, so the high-water instrument covers all of them.
    pub(super) slot: usize,
    pub(super) blk_ep: RendezvousId,
    pub(super) blk_shared: u64,
    pub(super) file_ep: RendezvousId,
    pub(super) file_shared: u64,
    pub(super) ready: RendezvousId,
    /// The untyped budget this server's heap draws from, in frames: [`FS_BUDGET_PAGES`], or
    /// [`CRASH_BUDGET_PAGES`] for the crash test's two, since an untyped is a reservation.
    pub(super) budget_pages: u64,
    /// The crash injection, straight into the process's START arguments:
    /// `(which WRITE request to die in, block writes to allow first, bytes of the last one that
    /// reach the platter)`. All zero disables it, which is every FS server but the crash test's
    /// first one. See `redoxfs_server/src/bin/redoxfs_server.rs`.
    pub(super) crash: (u64, u64, u64),
    /// How many pool windows beyond window 0 this server maps. The main service
    /// maps all of [`CLIENT_WINDOWS`] (so `CLIENT_WINDOWS - 1` extra), so a badged client's request
    /// lands in its own frame; the crash-test servers serve one client and map 0 extra, keeping
    /// their footprint and their disks separate from the pool.
    pub(super) extra_windows: usize,
}

/// How many mapping entries one FS server's list can need: its block channel, every client
/// window, and its stack.
const SERVER_MAPS_LEN: usize = BLK_PAGES + CLIENT_WINDOWS * FILE_PAGES + FS_STACK_PAGES as usize;

/// Each FS server slot's mapping list, in a static rather than on a stack: about 5.6 KiB, over the
/// 4 KiB guard page every kernel frame stays under (`script/stack-frame-check`), and the kernel has
/// no heap.
///
/// A `core::cell::SyncUnsafeCell`, not a lock, since a server keeps its list for the boot and a
/// guard would be held forever. [`SERVER_MAPS_TAKEN`] is the ownership: each slot goes once to one
/// server thread.
static SERVER_MAPS: core::cell::SyncUnsafeCell<[[Mapping; SERVER_MAPS_LEN]; FS_SERVERS]> =
    core::cell::SyncUnsafeCell::new(
        [[Mapping {
            va: 0,
            phys: 0,
            flags: Flags::user_data(),
        }; SERVER_MAPS_LEN]; FS_SERVERS],
    );

/// Which slots' lists have been handed out. A slot's server is spawned once per boot (the main
/// service once, behind [`WIRED`]; each crash-test server once), so a second claim is a bug.
static SERVER_MAPS_TAKEN: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);

/// Hand out slot `slot`'s mapping list, once. Panics on a second claim of one slot, which is
/// what makes the `&mut` below sound: the only thread that can reach a slot's list is the one
/// server thread that claimed it, and it claims it exactly once.
fn server_maps(slot: usize) -> &'static mut [Mapping; SERVER_MAPS_LEN] {
    let bit = 1u64 << slot;
    let prior = SERVER_MAPS_TAKEN.fetch_or(bit, core::sync::atomic::Ordering::AcqRel);
    assert!(
        prior & bit == 0,
        "FS server slot {slot}'s mapping list was claimed twice; one server per slot per boot",
    );
    let lists = SERVER_MAPS.get();
    // SAFETY: `slot` is below `FS_SERVERS` (every caller passes a literal 0, 1 or 2, and indexing
    // checks it), and the claim bit above guarantees this is the only live reference to this
    // slot's list for the life of the boot; the other slots' lists are disjoint elements.
    unsafe { &mut (*lists)[slot] }
}

/// Spawn one FS server described by `cfg`: a heap budget, the block-service endpoint (client
/// side), the file-service endpoint (server side), and both shared pages. No device, no DMA.
///
/// It also gets [`FS_STACK_PAGES`] of poisoned stack below `USER_STACK_VA`, since RedoxFS
/// overflows one page on its first `open`. They are not freed on death: a one-time cost per FS
/// server a boot starts.
pub(super) fn spawn_fs_server(fs_server_image: &'static [u8], cfg: FsServer) {
    let budget =
        crate::memory_region::create(cfg.budget_pages).expect("no heap budget for the FS server");
    let mut stack = [0u64; FS_STACK_PAGES as usize];
    for (i, f) in stack.iter_mut().enumerate() {
        *f = poisoned_stack_page_frame(cfg.slot, i);
    }
    crate::sched::spawn(move || {
        // The FS server maps the whole of both channels, the one party that must: it drives every
        // block the blk channel carries and serves any length a client asks for on the file one.
        let maps = server_maps(cfg.slot);
        let n0 = map_channel(maps, BLK_PAGE_FS, cfg.blk_shared, BLK_PAGES);
        // Window 0 is `file_shared`; windows 1..=extra_windows come from the pool, each mapped
        // `FILE_PAGES` above the last, where the server's `window_base(badge)` expects them.
        let mut n = n0 + map_channel(&mut maps[n0..], FILE_PAGE_FS, cfg.file_shared, FILE_PAGES);
        for (w, window) in WINDOWS.iter().enumerate().skip(1).take(cfg.extra_windows) {
            let phys = window.load(core::sync::atomic::Ordering::Relaxed);
            let va = FILE_PAGE_FS + w as u64 * FILE_PAGES as u64 * FRAME_SIZE;
            n += map_channel(&mut maps[n..], va, phys, FILE_PAGES);
        }
        for (i, &phys) in stack.iter().enumerate() {
            maps[n + i] = Mapping {
                va: super::USER_STACK_VA - (i as u64 + 1) * FRAME_SIZE,
                phys,
                flags: Flags::user_data(),
            };
        }
        // Only the filled prefix: a single-client server leaves the pool's share of the list
        // unused, and an unused entry is a mapping at VA 0 that collides with the next one.
        let used = n + stack.len();
        run(
            fs_server_image,
            Spawn {
                arg0: cfg.crash.0,
                arg1: cfg.crash.1,
                arg2: cfg.crash.2,
                grants: &[
                    memory_region_cap(budget), // slot 0: the heap's untyped budget
                    rendezvous_cap(cfg.blk_ep, Rights::WRITE), // slot 1: CALL the block server
                    rendezvous_cap(cfg.file_ep, Rights::READ), // slot 2: RECEIVE file requests
                    rendezvous_cap(cfg.ready, Rights::WRITE), // slot 3: signal readiness once
                ],
                maps: &maps[..used],
            },
        )
    })
    .expect("could not spawn the FS server");
}
