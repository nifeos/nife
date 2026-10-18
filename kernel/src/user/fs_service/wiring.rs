//! **Wiring the service**: the block server and the FS server, spawned once per boot and
//! remembered, and the readiness sentinels a caller drains before either is running.
//!
//! Cut out of `fs_service.rs` unchanged by lane `maintainer/last-three-over-2000`, which left
//! the parent an index (§282 (when a file is split, the parent becomes an index)) and under §266
//! (a Rust source file stays under 2,000 lines).
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

/// **One boot, one FS service**, remembered here.
///
/// The block server owns the RedoxFS device: a second wiring would put a second driver on the
/// same virtio slot and re-bind its interrupt, so the two client tests (the hand-written
/// `fs_test_client` and the std program) share one wired service instead. Whichever runs first pays
/// for the wiring and receives the two readiness endpoints; the other sees `None` for them,
/// because a readiness sentinel is sent once and has already been taken. Plain atomics rather
/// than a lock: the only writer is the boot/test thread that calls these functions.
pub(super) static WIRED: core::sync::atomic::AtomicBool =
    core::sync::atomic::AtomicBool::new(false);
static FILE_EP: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
pub(super) static FILE_SHARED: core::sync::atomic::AtomicU64 =
    core::sync::atomic::AtomicU64::new(0);

/// The wired service: the file-service endpoint clients `CALL`, the physical frame they share
/// with the FS server, and (only on the call that did the wiring) the block server's and FS
/// server's readiness endpoints.
///
/// **The block server's half is itself an `Option`** (milestone 198 (a package manager, and the
/// trivial install that makes a second customer possible), rung 2a). A boot that mounts the NVMe
/// disk does not start a block server at all: it borrows the one
/// `non_volatile_memory_express_service` already runs, whose readiness sentinel is sent once and
/// may already have been taken by whoever wired it. `None` there means "already up, with nothing
/// left to drain", which is a different fact from the outer `None`'s "somebody else wired the whole
/// service" and deserves to be a different shape rather than a flag somewhere.
type Service = (RendezvousId, u64, Readiness);

/// **The two sentinels a caller has to drain before the service is running**, and `None` when an
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
    // **The NVMe arm is for a machine with no virtio block device at all**, which is what an
    // installed system is, and the test for it is device 0 rather than device 1. The two questions
    // are not the same and the difference cost a gate: `cargo xtask uefi-test` attaches one virtio
    // disk and an NVMe controller, so "there is no device 1" was true there and the FS server was
    // handed a disk with no filesystem on it. A boot with a virtio disk and no *second* one still
    // has no RedoxFS service, exactly as it always did.
    let (blk_ep, blk_ready, blk_shared) = match crate::virtio::find_block_device_n(1) {
        Some(dev) => {
            let (ep, ready, shared) = spawn_block_server(blk_image, dev);
            (ep, Some(ready), shared)
        }
        // **Only when the boot came from that disk.** A stick booted on an installed machine
        // would otherwise mount it, and RedoxFS writes on mount: measured as 3 changed blocks by
        // boot 3 of `cargo xtask install-boot` (the live stick proposal's G2; calef ruled the
        // `boot_slot::medium` token that answers it on PR #1652, 2026-10-04 UTC).
        None if crate::virtio::find_block_device_n(0).is_none()
            && crate::memory::booted_from_nvme() =>
        {
            nvme_disk()?
        }
        None => return None,
    };
    // Window 0 is the default channel every legacy single-client path maps; windows 1.. are the
    // pool `claim_window` hands out, one frame each, all mapped into the FS server (milestone 599).
    // **All K windows are one physically contiguous run** (milestone 599, calef's option-4
    // ruling of 2026-09-27), so one `PageFrame` capability can name the whole pool: the
    // progenitor holds it in the slot `fs_page` always held, and slices a window per client
    // (`abi::page_frame::SLICE`). Window `w` starts `w * FILE_PAGES` pages in.
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

/// **The NVMe disk, when this machine has no virtio one** (milestone 198 (a package manager, and
/// the trivial install that makes a second customer possible), rung 2a).
///
/// This is what an **installed** system boots through. A machine with a nife disk on it has no
/// virtio block device at all: the stick is gone, and what is left is the NVMe controller the
/// installer wrote to. The NVMe data plane already serves `filesystem_contract::blk` from a
/// confined process with the same transfer geometry the FS server expects
/// (`filesystem_contract::blk::TRANSFER_BLOCKS` pages, multi-block requests included), so the
/// substitution is the endpoint and the shared region and nothing else.
///
/// **The FS server is given the whole disk, not the nife data partition**, and that is a recorded
/// limitation rather than an oversight. It works because `redoxfs`'s own `FileSystem::open` scans
/// blocks `0..65536` for its header and adopts the block it finds it at as the filesystem's
/// origin, which is why `components/src/system_installer.rs` puts the data partition **first** on the
/// disk. The cost is that this server can address the EFI system partition and the partition table;
/// a partition-bounded mount needs either a base-block field on the `blk` wire or a program in the
/// middle, and the installer's `BUGS` carries the argument.
///
/// `None` when there is no NVMe controller. When the service was already wired this boot (the
/// install offer surveys the disk through it before the progenitor exists) the readiness half comes
/// back `None`, because that sentinel is sent once and has been taken; the service is up either
/// way, which is what the caller actually needs to know.
fn nvme_disk() -> Option<(RendezvousId, Option<RendezvousId>, u64)> {
    let image = crate::user::program("non_volatile_memory_express")?;
    let wiring = crate::user::non_volatile_memory_express_service::ensure(image)?;
    Some((wiring.request, wiring.ready, wiring.transfer_phys))
}

/// **Spawn a block server on one virtio block device.** Extracted from [`wire_servers`] because
/// milestone 37's crash test needs a second one, on its own disk, so that a test which
/// deliberately leaves a filesystem half-written cannot touch the image every other FS test
/// depends on. Returns `(blk_ep, blk_ready, blk_shared)`.
///
/// The DMA region is `1 + BLK_PAGES` contiguous pages: page 0 for the rings, request header and
/// status (block-server-private), and `BLK_PAGES` pages for the data buffer (milestone 138 step 4;
/// it was one page through step 3). Those data pages are ALSO the block region shared with the FS
/// server, so the device DMAs up to `BLK_PAGES` contiguous filesystem blocks straight into the FS
/// server's region in one request, no per-block loop and no copy.
///
/// Bring one virtio block device up under a confined userspace block server, and hand back the
/// three things a client needs: the request endpoint, the readiness endpoint, and the physical
/// base address of the region the transfers land in.
///
/// **It takes a resolved device rather than an mmio slot** (milestone 303), which is the transport
/// seam doing here what it already did for `virtio_service::wire`: the DMA region, the `Irq`
/// routing, the confined `Virtio` capability and the spawn are all bus-agnostic, and the one thing
/// that was not was the type of this argument. On `q35` there is no virtio-mmio bus for it to have
/// named.
///
/// `pub(super)` because milestone 57's `disk_service` wires a fourth disk the same way. The FS
/// server is no longer the only thing that wants "a block device, served over IPC, by a process
/// that owns the DMA and nothing else". Its two clients (`disk_surveyor`, `disk_partitioner`) only
/// ever map the first data page and only ever send single-block requests, so they are unmodified
/// by the region's growth, the same compatibility [`filesystem_contract::blk::TRANSFER_BLOCKS`] documents.
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

/// Everything one FS-server process is, as a value, because eight positional arguments of which
/// four are bare `u64` endpoint ids is a call nobody can read and a caller can silently get the
/// wrong way round.
pub(super) struct FsServer {
    /// Which bank of [`FS_STACK_PHYS`] this process's poisoned stack is recorded in. One per
    /// FS server a boot can start, so the high-water instrument covers all of them.
    pub(super) slot: usize,
    pub(super) blk_ep: RendezvousId,
    pub(super) blk_shared: u64,
    pub(super) file_ep: RendezvousId,
    pub(super) file_shared: u64,
    pub(super) ready: RendezvousId,
    /// The untyped budget this server's heap draws from, in frames. The ordinary server gets
    /// [`FS_BUDGET_PAGES`]; milestone 37's two get a fraction of it, because an untyped is
    /// **reserved** rather than merely capped and three 8 MiB reservations do not fit in this
    /// machine's 128 MiB (the first symptom was the progenitor failing to get its own budget, several
    /// tests later, which is a long way from the cause). The measured high-water of a real mount
    /// under this allocator is 352 KiB (DECISIONS §27), so [`CRASH_BUDGET_PAGES`] is still five
    /// times the number rather than a guess trimmed until it fit.
    pub(super) budget_pages: u64,
    /// Milestone 37's crash injection, straight into the process's START arguments:
    /// `(which WRITE request to die in, block writes to allow first, bytes of the last one that
    /// reach the platter)`. All zero disables it, which is every FS server but the crash test's
    /// first one. See `redoxfs_server/src/bin/redoxfs_server.rs`.
    pub(super) crash: (u64, u64, u64),
    /// **How many pool windows beyond window 0 this server maps** (milestone 599). The main service
    /// maps all of [`CLIENT_WINDOWS`] (so `CLIENT_WINDOWS - 1` extra), so a badged client's request
    /// lands in its own frame; the crash-test servers serve one client and map 0 extra, keeping
    /// their footprint and their disks separate from the pool.
    pub(super) extra_windows: usize,
}

/// Spawn one FS server: a heap budget, the block-service endpoint (client side), the
/// file-service endpoint (server side), and both shared pages. No device, no DMA.
///
/// It also gets a DEEP stack. `run` maps one stack page (enough for the shallow programs), but
/// RedoxFS recurses through its tree and htree and commits transactions on the stack, and one
/// 4 KiB page overflows immediately (the first `open` faults ~4.2 KiB down). So map extra stack
/// pages below `USER_STACK_VA` out of fresh frames. These are shared-style mappings (not freed on
/// death), a one-time cost per FS server a boot starts.
/// How many mapping entries one FS server's list can need: its block channel, every client
/// window, and its stack (milestone 599).
const SERVER_MAPS_LEN: usize = BLK_PAGES + CLIENT_WINDOWS * FILE_PAGES + FS_STACK_PAGES as usize;

/// **Each FS server slot's mapping list, in a static rather than on a stack** (milestone 599).
///
/// With the client windows, a server's list is `SERVER_MAPS_LEN` entries of 24 bytes, about
/// 5.6 KiB, and a kernel frame may not exceed the 4 KiB guard page (`script/stack-frame-check`).
/// The kernel has no heap, so it lives here: one list per slot in [`FS_SERVERS`], 17 KiB of
/// `.bss` in all.
///
/// A `core::cell::SyncUnsafeCell` since milestone 891 (a mutable static becomes an atomic, a lock
/// or a recorded cell), not a lock: a server keeps its list for the life of the boot, so a guard
/// would be held forever. [`SERVER_MAPS_TAKEN`] is the ownership instead. Each slot is handed out
/// once, to one server thread, and the slots are disjoint elements.
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

/// **Hand out slot `slot`'s mapping list, once.** Panics on a second claim of one slot, which is
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

pub(super) fn spawn_fs_server(fs_server_image: &'static [u8], cfg: FsServer) {
    let budget =
        crate::memory_region::create(cfg.budget_pages).expect("no heap budget for the FS server");
    let mut stack = [0u64; FS_STACK_PAGES as usize];
    for (i, f) in stack.iter_mut().enumerate() {
        *f = poisoned_stack_page_frame(cfg.slot, i);
    }
    crate::sched::spawn(move || {
        // Build the mapping list: the two shared channels, then the extra stack pages. The FS
        // server maps the whole of both, the one party that must for each: it drives every block
        // the blk channel can carry (up to `filesystem_contract::blk::TRANSFER_BLOCKS`, milestone 138 (close the read gap: a 4 KiB request must stop moving 128 KiB) step 4)
        // and serves whatever length a client asks for on the file channel, up to
        // `filesystem_contract::fs::TRANSFER_MAX` (step 3). A client maps only what it uses of the file
        // channel; nothing else maps the blk channel at all.
        // The mapping list lives in this server slot's static, not on this thread's stack
        // (milestone 599): K windows of `FILE_PAGES` entries took this closure's frame to 6.7 KiB,
        // over the 4 KiB guard page `script/stack-frame-check` holds every kernel frame under.
        let maps = server_maps(cfg.slot);
        let n0 = map_channel(maps, BLK_PAGE_FS, cfg.blk_shared, BLK_PAGES);
        // Window 0 is `file_shared`; windows 1..=extra_windows come from the pool (milestone 599),
        // each mapped `FILE_PAGES` above the last so the server's `window_base(badge)` addresses
        // them. `extra_windows` is 0 for a single-client server, which maps just window 0 as before.
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
