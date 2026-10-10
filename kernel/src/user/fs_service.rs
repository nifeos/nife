use super::*;
use crate::cap::{
    Rights, irq_cap, memory_region_cap, page_frame_cap, rendezvous_cap, rendezvous_cap_badged,
    virtio_cap,
};
use crate::sched::RendezvousId;

/// The block server's role in the driver binary (must match `fixtures/src/hello.rs`,
/// `components/src/block_driver.rs` and `crates/virtio`).
const ROLE_BLK_SERVER: u64 = 32;

/// The heap budget the FS server draws RedoxFS's allocations from. RedoxFS keeps a 128 KiB
/// compress buffer, block buffers, and small tree structures for the images phase 2 serves; 8 MiB
/// is comfortable and is the process's hard ceiling (its `HEAP_MAX` matches).
const FS_BUDGET_PAGES: u64 = 2048;

/// **Extra stack pages for the FS server**, below the single page `run` maps, so the process
/// gets `1 + FS_STACK_PAGES` pages in one contiguous run down from [`USER_STACK_TOP`].
///
/// RedoxFS recurses through its tree, htree and transaction code with **8 KiB frames** (one
/// `read_block::<TreeList<..>>` activation carries a whole 4096-byte block plus scratch), so this
/// is not a "generous round number" question, it is a measured one. It used to be 32, and 32 was
/// **528 bytes short** the moment milestone 31 phase 2 added `CREATE` and `TRUNCATE`: the FS
/// server took one more level of tree recursion, ran off the bottom of its stack, and was killed
/// by a data abort mid-request, which left every client blocked on a `CALL` that would never be
/// answered. See [`fs_stack_used`] for the instrument that now measures this instead of guessing,
/// and `the_redoxfs_servers_stack_still_has_headroom` for the test that fails the day it is too small
/// again. notes/fs-server.md carries the story.
///
/// 96 is chosen against the measurement, not above it: the high-water is **135,696 bytes** (and
/// 127,408 as measured by milestone 37, an unattributed 8 KiB lower; notes/fs-server.md carries
/// both and the reasoning), and
/// `1 + 96` pages is 397,312, which leaves room for roughly thirty more 8 KiB activations. That
/// margin is the point, because recursion depth here tracks the *tree* depth, which grows with
/// the image; a size proven on a 16 MiB fixture is not proven on a real disk. 384 KiB of frames
/// once per boot is cheap next to the FS server's 8 MiB heap budget.
const FS_STACK_PAGES: u64 = 96;

/// The pattern every FS-server stack page is filled with before the process starts. Nothing but
/// the FS server's own stack writes ever touch these frames, so a word that still reads as this
/// is a word the stack never reached, and the deepest changed word is the high-water mark. The
/// value is deliberately not 0 and not a plausible pointer, so a poisoned word cannot be mistaken
/// for real data (or vice versa) in a dump.
const STACK_POISON: u64 = 0xC71C_5E57_C71C_5E57;

// The VAs each process expects its mappings at. Each MUST match that program's source.
const DMA_VA: u64 = address_space_map::pair_page(0x0000_0000_0090_0000); // block server DMA region, 1 + BLK_PAGES pages (crates/virtio)
const BLK_PAGE_FS: u64 = 0x5000_0000; // FS server's block region (redoxfs_server.rs BLK_PAGE)
// FS server's file region (redoxfs_server.rs FILE_PAGE): BLK_PAGES pages above BLK_PAGE_FS, so growing
// the block channel (milestone 138 step 4) cannot walk into it.
const FILE_PAGE_FS: u64 = BLK_PAGE_FS + (BLK_PAGES as u64) * FRAME_SIZE;
const FILE_VA_CLIENT: u64 = 0x0000_0000_0060_0000; // client's file page (fs_test_client.rs FILE_VA)

/// A std program's half of the same agreement (notes/abi.md section 4, notes/std.md): the slot the PAL
/// looks for the FS-service endpoint in, and the VA it expects the shared file page at. Read from
/// `std_runtime_protocol`, which the std PAL has generated into it, so the two cannot drift. A std
/// program's slot layout differs from the hand-written client's because std already owes slots 0
/// and 1 to its heap and its stdout, and 2 and 3 to `std::net`.
const FS_DIR_SLOT: u64 = std_runtime_protocol::FS_DIR_SLOT;
const FS_PAGE_STD: u64 = std_runtime_protocol::FS_PAGE;

/// A fresh, zeroed frame, returned by physical address. Zeroed so no stale RAM is ever visible
/// across a share, and (for the DMA frame) so the device never reads a stale descriptor.
fn page_frame() -> u64 {
    crate::memory::alloc_zeroed()
        .expect("no frame for the fs service")
        .addr()
}

/// **How many pages the file channel spans**, straight from the contract, so this wiring cannot
/// disagree with the two programs that speak it.
const FILE_PAGES: usize = filesystem_protocol::fs::TRANSFER_PAGES;

/// **How many pages the blk channel spans** (milestone 138 step 4), straight from the contract, the
/// same reason [`FILE_PAGES`] is.
const BLK_PAGES: usize = filesystem_protocol::blk::TRANSFER_BLOCKS;

/// **The file channel: [`FILE_PAGES`] fresh, zeroed, physically contiguous frames**, returned by
/// the base physical address (milestone 138 step 3).
///
/// Contiguous rather than a list of frames, and that is what keeps this change from spreading. Both
/// halves of the agreement already pass the channel around as one `u64`; a run of frames keeps that
/// signature, and every mapping site becomes a loop over [`FILE_PAGES`] instead of a struct change
/// in nine places. The block server's DMA region is already wired exactly this way (two contiguous
/// pages at `DMA_VA`), so this is the tree's existing shape for a multi-page share rather than a
/// new one.
///
/// It is zeroed for the same reason one frame was: no stale RAM is ever visible across a share.
/// **Every client window at once**: [`CLIENT_WINDOWS`] channels of [`FILE_PAGES`] frames, one
/// contiguous zeroed run (milestone 599), returned by its base. Window 0 is the first
/// [`FILE_PAGES`] of it, so everything that took [`file_channel`]'s base as "the channel" still
/// finds window 0 there.
fn file_pool() -> u64 {
    crate::memory::alloc_contiguous_zeroed(CLIENT_WINDOWS * FILE_PAGES)
        .expect("no contiguous run for the fs service's client windows")
        .addr()
}

/// The pool's length in pages: what the progenitor's run capability over it names.
pub const FILE_POOL_PAGES: u64 = (CLIENT_WINDOWS * FILE_PAGES) as u64;

fn file_channel() -> u64 {
    crate::memory::alloc_contiguous_zeroed(FILE_PAGES)
        .expect("no contiguous run for the fs service's file channel")
        .addr()
}

/// Write the file channel's mappings into `maps`, `va` upward against `phys` upward, and answer how
/// many entries that took. One call per party that shares the channel, so the "map every page of
/// it" rule lives in one place rather than in each of them.
///
/// **`pages` is how much of the channel this party maps**, and it is a parameter rather than always
/// [`FILE_PAGES`] because a client is entitled to map less: a client that only ever moves one page
/// needs one page, and `filesystem_protocol::fs::TRANSFER_PAGES` says so. The FS server is the one party that
/// must map all of it.
///
/// Visible to the rest of `user` because its callers are wired somewhere else entirely
/// (`virtio_service`, since it is a client of the net stack rather than of this module) and shares
/// the same channel with the same FS server. Two spellings of "map every page of it" is exactly the
/// drift the foot gun at `filesystem_protocol::fs::TRANSFER_PAGES` punishes, so there is one.
pub(super) fn map_channel(maps: &mut [Mapping], va: u64, phys: u64, pages: usize) -> usize {
    for (i, m) in maps.iter_mut().take(pages).enumerate() {
        *m = Mapping {
            va: va + i as u64 * FRAME_SIZE,
            phys: phys + i as u64 * FRAME_SIZE,
            flags: Flags::user_data(),
        };
    }
    pages
}

/// How many FS servers one boot can start, and therefore how many banks of poisoned stack pages
/// [`FS_STACK_PHYS`] holds: the ordinary one, and milestone 37's two (the server that is killed
/// mid-transaction, and the one that mounts the disk it left behind).
///
/// They cannot share stack frames. The killed server is still executing its trap when the
/// recovery server starts, so reusing its pages would have one process writing another's stack,
/// and the bug would present as the recovery mount failing for no reason.
const FS_SERVERS: usize = 3;

/// A fresh frame filled with [`STACK_POISON`], for one of the FS server's stack pages, and
/// remembered in [`FS_STACK_PHYS`] so the depth actually reached can be read back afterwards.
fn poisoned_stack_page_frame(server: usize, index: usize) -> u64 {
    let p = crate::memory::alloc()
        .expect("no frame for the fs server's stack")
        .addr();
    // SAFETY: fresh frame, reachable through the direct map, exactly FRAME_SIZE bytes.
    let words = unsafe {
        core::slice::from_raw_parts_mut(mmu::phys_to_virt(p) as *mut u64, FRAME_SIZE as usize / 8)
    };
    words.fill(STACK_POISON);
    FS_STACK_PHYS[server][index].store(p, core::sync::atomic::Ordering::Relaxed);
    p
}

/// The physical frames behind every FS server's extra stack pages: one bank per server, index 0
/// being the page directly below [`USER_STACK_VA`]. Written once at wiring, read by
/// [`fs_stack_used`]. Zero means "this boot never started that server".
#[allow(clippy::declare_interior_mutable_const)] // an atomic array is exactly what this is for
static FS_STACK_PHYS: [[core::sync::atomic::AtomicU64; FS_STACK_PAGES as usize]; FS_SERVERS] =
    [const { [const { core::sync::atomic::AtomicU64::new(0) }; FS_STACK_PAGES as usize] };
        FS_SERVERS];

/// **How deep the FS server's stack actually went**, in bytes below [`USER_STACK_TOP`], and how
/// much it was given. `None` if this boot wired no FS service.
///
/// Read by scanning the poison: the deepest word that is no longer [`STACK_POISON`] is the
/// deepest the process ever wrote. This is a measurement of the whole run so far, not a sample,
/// because nothing ever un-writes a stack word. The base page `run` maps is counted as fully used
/// (it holds the entry frame and cannot be scanned from here), which is true and is why the
/// number starts at one page.
///
/// The point of it is that a stack size is otherwise a number nobody can defend. The one before
/// this was 528 bytes too small, and the way we found out was a mystery hang.
///
/// The high-water is a **maximum over every FS server this boot started**, which is why
/// milestone 37's recovery mount is covered by it too. A mount that has to walk back a
/// generation is the case most likely to recurse further than a clean one, so it is exactly the
/// case this instrument should be watching.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub fn fs_stack_used() -> Option<(u64, u64)> {
    use core::sync::atomic::Ordering;
    let total = (FS_STACK_PAGES + 1) * FRAME_SIZE;
    let mut deepest = FRAME_SIZE; // the base page, always used
    let mut wired = false;
    for bank in FS_STACK_PHYS.iter() {
        for (i, slot) in bank.iter().enumerate() {
            let phys = slot.load(Ordering::Relaxed);
            if phys == 0 {
                continue;
            }
            wired = true;
            // SAFETY: a frame this module allocated and still owns, via the direct map.
            let words = unsafe {
                core::slice::from_raw_parts(
                    mmu::phys_to_virt(phys) as *const u64,
                    FRAME_SIZE as usize / 8,
                )
            };
            // Page `i` spans [USER_STACK_VA - (i+1)*FRAME, USER_STACK_VA - i*FRAME). Word `w` in
            // it sits that far above the page's base, so a touched word means at least this much
            // depth.
            if let Some(w) = words.iter().position(|&x| x != STACK_POISON) {
                let depth = (i as u64 + 2) * FRAME_SIZE - w as u64 * 8;
                deepest = deepest.max(depth);
            }
        }
    }
    wired.then_some((deepest, total))
}

/// **One boot, one FS service**, remembered here.
///
/// The block server owns the RedoxFS device: a second wiring would put a second driver on the
/// same virtio slot and re-bind its interrupt, so the two client tests (the hand-written
/// `fs_test_client` and the std program) share one wired service instead. Whichever runs first pays
/// for the wiring and receives the two readiness endpoints; the other sees `None` for them,
/// because a readiness sentinel is sent once and has already been taken. Plain atomics rather
/// than a lock: the only writer is the boot/test thread that calls these functions.
static WIRED: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);
static FILE_EP: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
static FILE_SHARED: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);

/// **How many client channels this service keeps side by side** (milestone 599, provisional),
/// straight from the contract so this wiring and the server agree. Window 0 is [`FILE_SHARED`], the
/// unbadged default every legacy single-client path maps; windows 1.. are the pool [`claim_window`]
/// hands out, one per additional live client, each its own frame and its own badge.
const CLIENT_WINDOWS: usize = filesystem_protocol::fs::CLIENT_WINDOWS;

/// The physical base of each window's [`FILE_PAGES`]-frame run. `WINDOWS[0]` is [`FILE_SHARED`];
/// the rest are allocated once when the service is wired and mapped into the FS server. Plain
/// atomics for [`WIRED`]'s reason: the only writer is the boot/test thread.
static WINDOWS: [core::sync::atomic::AtomicU64; CLIENT_WINDOWS] =
    [const { core::sync::atomic::AtomicU64::new(0) }; CLIENT_WINDOWS];

/// Which windows are handed out. Bit `w` set means window `w` is claimed; bit 0 is set from wiring,
/// because window 0 is the default rather than a claimable channel. A `u64` bitmap because
/// [`CLIENT_WINDOWS`] is far under 64; [`claim_window`] and [`release_window`] are the only writers.
static WINDOW_TAKEN: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);

/// **Claim a free client-channel window** (milestone 599): returns its index (always `>= 1`, since
/// window 0 is the default) and the physical base of its frame run, or `None` when all
/// [`CLIENT_WINDOWS`] are in use, which is this service's live-client ceiling. The badge a client is
/// granted IS its window index, so the FS server reads the right frame for its request.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
fn claim_window() -> Option<(u64, u64)> {
    use core::sync::atomic::Ordering;
    // Never the last: that one is `login`'s, for its durable sessions ([`durable_window`]).
    let pool = login_protocol::DURABLE_WINDOW as usize;
    for (w, window) in WINDOWS.iter().enumerate().take(pool).skip(1) {
        let bit = 1u64 << w;
        if WINDOW_TAKEN.fetch_or(bit, Ordering::Relaxed) & bit == 0 {
            return Some((w as u64, window.load(Ordering::Relaxed)));
        }
    }
    None
}

/// **The window `login` keeps for its durable sessions** (milestone 152, Fork 8 D):
/// `login_protocol::DURABLE_WINDOW`, claimed once and handed to every `login` this boot starts, the
/// way the progenitor reserves it. The physical base of its frame run, or `None` when the service is
/// not wired.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub fn durable_window() -> Option<u64> {
    use core::sync::atomic::Ordering;
    // [`claim_window`] never hands this one out, so it is always free for `login`.
    let w = login_protocol::DURABLE_WINDOW as usize;
    WINDOW_TAKEN.fetch_or(1u64 << w, Ordering::Relaxed);
    let phys = WINDOWS[w].load(Ordering::Relaxed);
    (phys != 0).then_some(phys)
}

/// [`release_window`] for a test that bound a grant and holds no program (milestone 606).
#[cfg(any(test, feature = "system_tests"))]
pub fn release_window_after_test(w: u64) {
    release_window(w);
}

/// **Return a window to the pool** (milestone 599), the take-back half of [`claim_window`]. The
/// frame is not freed (it stays mapped in the FS server for the life of the service); it is zeroed
/// so no client's staging survives into the next client that claims the slot, which is the same
/// "no stale RAM across a share" rule [`file_channel`] applies at allocation.
///
/// `allow(dead_code)` unconditionally, and it is the marked exception AGENTS.md's ladder allows: the
/// caller is the production progenitor's reap, which is this milestone's outstanding piece (see the
/// block's "What is left"). The harness witness claims windows and does not release them, because
/// its clients run for the length of one test; the take-back is written here, beside its claim half,
/// so the progenitor pool has the whole pool to build against rather than half of it.
#[allow(dead_code)]
fn release_window(w: u64) {
    use core::sync::atomic::Ordering;
    let phys = WINDOWS[w as usize].load(Ordering::Relaxed);
    if phys != 0 {
        // SAFETY: a frame this module allocated and still owns, via the direct map; FILE_PAGES
        // frames of FRAME_SIZE bytes, reachable and mapped only into the FS server and whatever
        // client last held this window (now gone).
        unsafe {
            core::ptr::write_bytes(
                mmu::phys_to_virt(phys) as *mut u8,
                0,
                FILE_PAGES * FRAME_SIZE as usize,
            );
        }
    }
    WINDOW_TAKEN.fetch_and(!(1u64 << w), Ordering::Relaxed);
}

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
fn ensure(blk_image: &'static [u8], fs_server_image: &'static [u8]) -> Option<Service> {
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
/// installer wrote to. The NVMe data plane already serves `filesystem_protocol::blk` from a
/// confined process with the same transfer geometry the FS server expects
/// (`filesystem_protocol::blk::TRANSFER_BLOCKS` pages, multi-block requests included), so the
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
/// by the region's growth, the same compatibility [`filesystem_protocol::blk::TRANSFER_BLOCKS`] documents.
pub(super) fn spawn_block_server(
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
struct FsServer {
    /// Which bank of [`FS_STACK_PHYS`] this process's poisoned stack is recorded in. One per
    /// FS server a boot can start, so the high-water instrument covers all of them.
    slot: usize,
    blk_ep: RendezvousId,
    blk_shared: u64,
    file_ep: RendezvousId,
    file_shared: u64,
    ready: RendezvousId,
    /// The untyped budget this server's heap draws from, in frames. The ordinary server gets
    /// [`FS_BUDGET_PAGES`]; milestone 37's two get a fraction of it, because an untyped is
    /// **reserved** rather than merely capped and three 8 MiB reservations do not fit in this
    /// machine's 128 MiB (the first symptom was the progenitor failing to get its own budget, several
    /// tests later, which is a long way from the cause). The measured high-water of a real mount
    /// under this allocator is 352 KiB (DECISIONS §27), so [`CRASH_BUDGET_PAGES`] is still five
    /// times the number rather than a guess trimmed until it fit.
    budget_pages: u64,
    /// Milestone 37's crash injection, straight into the process's START arguments:
    /// `(which WRITE request to die in, block writes to allow first, bytes of the last one that
    /// reach the platter)`. All zero disables it, which is every FS server but the crash test's
    /// first one. See `redoxfs_server/src/bin/redoxfs_server.rs`.
    crash: (u64, u64, u64),
    /// **How many pool windows beyond window 0 this server maps** (milestone 599). The main service
    /// maps all of [`CLIENT_WINDOWS`] (so `CLIENT_WINDOWS - 1` extra), so a badged client's request
    /// lands in its own frame; the crash-test servers serve one client and map 0 extra, keeping
    /// their footprint and their disks separate from the pool.
    extra_windows: usize,
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
static mut SERVER_MAPS: [[Mapping; SERVER_MAPS_LEN]; FS_SERVERS] = [[Mapping {
    va: 0,
    phys: 0,
    flags: Flags::user_data(),
}; SERVER_MAPS_LEN]; FS_SERVERS];

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
    let lists = &raw mut SERVER_MAPS;
    // SAFETY: `slot` is below `FS_SERVERS` (every caller passes a literal 0, 1 or 2, and indexing
    // checks it), and the claim bit above guarantees this is the only live reference to this
    // slot's list for the life of the boot; the other slots' lists are disjoint elements.
    unsafe { &mut (*lists)[slot] }
}

fn spawn_fs_server(fs_server_image: &'static [u8], cfg: FsServer) {
    let budget =
        crate::memory_region::create(cfg.budget_pages).expect("no heap budget for the FS server");
    let mut stack = [0u64; FS_STACK_PAGES as usize];
    for (i, f) in stack.iter_mut().enumerate() {
        *f = poisoned_stack_page_frame(cfg.slot, i);
    }
    crate::sched::spawn(move || {
        // Build the mapping list: the two shared channels, then the extra stack pages. The FS
        // server maps the whole of both, the one party that must for each: it drives every block
        // the blk channel can carry (up to `filesystem_protocol::blk::TRANSFER_BLOCKS`, milestone 138 step 4)
        // and serves whatever length a client asks for on the file channel, up to
        // `filesystem_protocol::fs::TRANSFER_MAX` (step 3). A client maps only what it uses of the file
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

// =======================================================================================
// Milestone 37: the crash test's own service, on its own disk (DECISIONS §34 condition 1)
// =======================================================================================

/// What [`start_crash`] hands back: the two readiness endpoints, and the endpoint the driver
/// reports its acknowledged write on.
///
/// `fs_ready` carries **two** messages in sequence, and that is the design rather than an
/// accident: the FS server's ordinary readiness sentinel when the mount is up, and then
/// `fixture::crash::CUT` from inside the injector, immediately before it traps. The second one
/// is what tells the test the kill was the injector's doing and not something else going wrong,
/// and it is what gives the recovery mount a defined moment to start at instead of a guess.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub struct CrashRun {
    pub blk_ready: RendezvousId,
    pub fs_ready: RendezvousId,
    pub driver_report: RendezvousId,
}

/// The heap budget each of milestone 37's two FS servers draws from. Smaller than
/// [`FS_BUDGET_PAGES`] on purpose: an untyped is a reservation, and three 8 MiB ones do not fit
/// beside everything else this boot builds in 128 MiB. 2 MiB is five times the 352 KiB
/// high-water a real mount under this allocator actually reaches (DECISIONS §27's measurement),
/// and the crash workload is one open and two 66-byte writes.
const CRASH_BUDGET_PAGES: u64 = 512;

/// The crash disk's block-service endpoint and shared block page, remembered between the two
/// halves of the test: the recovery server is a **different process** on the **same** block
/// server, which is endpoint-only naming doing its job. The block server never learns that its
/// client died and was replaced, because it never knew who its client was.
static CRASH_BLK_EP: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
static CRASH_BLK_SHARED: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);

/// **Phase one: wire a filesystem on its own disk and kill it mid-transaction** (milestone 37).
///
/// Three processes on virtio block device 2, which nothing else in the boot touches. The disk is
/// dedicated on purpose: this test deliberately leaves a filesystem half-written, and pointing it
/// at the shared fixture would couple every later test to whether this one ran first. DECISIONS
/// §27 records what an order-coupled gate costs (three investigations, three incompatible root
/// causes, none of them real), so the fixture is regenerated per run and owned by one test.
///
/// The FS server is spawned **armed**: die one block write into the second `WRITE` request, with
/// that block torn in half. One is the count that cannot miss, because a write transaction always
/// issues at least one block write and a larger count is a server that never dies and a test that
/// hangs.
/// The mmio slot the crash test's dedicated disk arrives on, named once so [`start_crash`] and
/// [`is_crash_disk_present`] cannot drift apart about which device this test owns.
const CRASH_DISK_INDEX: usize = 2;

/// **Is the crash test's own disk attached to this boot?**
///
/// Milestone 214 (design/roadmap/0214-print-and-return-skips.md), on a test that prints
/// "skipping" and returns being counted as passed: the fixture check used to live inside
/// `std_tests::assert_a_kill_mid_transaction_recovers`, which printed "skipping" and returned.
/// A helper cannot skip on a test's behalf, because `skip!()` returns from the function it is
/// written in, so the two callers asked their question one frame too late and were counted as
/// passes. This is the same guard-then-run shape those tests already use for
/// [`NO_FS_SERVER`], and it belongs in this module because the device index does.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub fn is_crash_disk_present() -> bool {
    crate::virtio::find_block_device_n(CRASH_DISK_INDEX).is_some()
}

#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub fn start_crash(
    blk_image: &'static [u8],
    fs_server_image: &'static [u8],
    client_image: &'static [u8],
) -> Option<CrashRun> {
    use core::sync::atomic::Ordering;
    let (blk_ep, blk_ready, blk_shared) = spawn_block_server(
        blk_image,
        crate::virtio::find_block_device_n(CRASH_DISK_INDEX)?,
    );
    CRASH_BLK_EP.store(blk_ep, Ordering::Relaxed);
    CRASH_BLK_SHARED.store(blk_shared, Ordering::Relaxed);

    let file_shared = file_channel();
    let file_ep = crate::sched::create_rendezvous();
    let fs_ready = crate::sched::create_rendezvous();
    spawn_fs_server(
        fs_server_image,
        FsServer {
            slot: 1,
            blk_ep,
            blk_shared,
            file_ep,
            file_shared,
            ready: fs_ready,
            budget_pages: CRASH_BUDGET_PAGES,
            // Die one block write into the SECOND write request, tearing that block at 2048
            // bytes. The first write is acknowledged and must survive; the second is the one the
            // property is about.
            crash: (2, 1, 2048),
            extra_windows: 0, // a single-client crash service maps only window 0 (milestone 599)
        },
    );

    let driver_report = spawn_fs_client(client_image, file_ep, file_shared, 3, 0, 0, 0);
    Some(CrashRun {
        blk_ready,
        fs_ready,
        driver_report,
    })
}

/// **Phase two: mount the disk the dead server left behind** (milestone 37).
///
/// A fresh FS-server process, on the same block server and the same block page, with its own file
/// endpoint, its own file page and its own stack. It carries nothing over from the process that
/// died: what it recovers, it recovers from the platter, through the ordinary `Server::open` every
/// FS server in this system mounts with. Its readiness sentinel arriving at all is the first half
/// of the result, because that open fails outright on an image it cannot make sense of.
///
/// Returns `(ready, report)`.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub fn recover_crash(
    fs_server_image: &'static [u8],
    client_image: &'static [u8],
) -> (RendezvousId, RendezvousId) {
    use core::sync::atomic::Ordering;
    let file_shared = file_channel();
    let file_ep = crate::sched::create_rendezvous();
    let ready = crate::sched::create_rendezvous();
    spawn_fs_server(
        fs_server_image,
        FsServer {
            slot: 2,
            blk_ep: CRASH_BLK_EP.load(Ordering::Relaxed),
            blk_shared: CRASH_BLK_SHARED.load(Ordering::Relaxed),
            file_ep,
            file_shared,
            ready,
            budget_pages: CRASH_BUDGET_PAGES,
            crash: (0, 0, 0), // this one is not armed: it is the one that has to survive
            extra_windows: 0, // single-client crash service (milestone 599)
        },
    );
    let report = spawn_fs_client(client_image, file_ep, file_shared, 4, 0, 0, 0);
    (ready, report)
}

/// The most extra stack pages [`spawn_fs_client`] will map below the one `run` gives every
/// program. Small on purpose: a client that needs more than this is a client whose frames want
/// looking at, not a number that wants raising.
const CLIENT_EXTRA_STACK: usize = 8;

/// Spawn one client holding exactly a file-service endpoint, a report endpoint and its view of
/// the shared page. Returns the report endpoint.
///
/// `extra_stack` is pages **below** the single one `run` maps. The hand-written `fs_test_client` roles
/// need none: they hold a handle and a small buffer. The navigating shell (milestone 47's
/// commands) needs some, and the number is a measurement rather than a guess: with none it
/// overflowed by 192 bytes, which presented as a data abort on its own `sp` and then as the
/// 60-second lost-wakeup watchdog, because the kernel test was still waiting for a report from a
/// process that had died. A shell carries a path stack (eight levels of name), a parsed path,
/// and a listing buffer, all by value, so 4 KiB is genuinely tight for it. Same discipline as
/// the FS server's stack (DECISIONS §27): sized by what it did, not by what looks generous.
///
/// **Milestone 50 moved it again, the same way.** `Endowment` grew a sink and a source, each of
/// which can carry a whole `FileGrant`, and the shell now holds one endowment per pipeline
/// stage; four extra pages overflowed by 48 bytes at the same `sp` and with the same symptom.
/// Recorded because it will happen a third time: the shell's stack is sized by the largest value
/// `grant_plan` hands it, so a field added there is a page needed here.
/// `pub(super)` (rather than private, its shape before milestone 152) so a sibling test module can
/// spawn a client with a nonzero `extra_stack` directly, bypassing [`start`]'s convenience wrapper
/// (which hardcodes `0`): milestone 152's schedule-store roles needed more than the one-page
/// default, found short under `script/test`'s own aarch64 run (a data abort at the stack's guard
/// page). Those roles went with `session_reviver` on 2026-09-27; the granted-directory spawners
/// below still pass their callers' stack pages through it.
pub fn spawn_fs_client(
    client_image: &'static [u8],
    file_ep: RendezvousId,
    file_shared: u64,
    role: u64,
    arg: u64,
    arg2: u64,
    extra_stack: usize,
) -> RendezvousId {
    assert!(
        extra_stack <= CLIENT_EXTRA_STACK,
        "an FS client asked for more stack than this wiring maps",
    );
    let report = crate::sched::create_rendezvous();
    crate::sched::spawn(move || {
        let mut maps = [Mapping {
            va: 0,
            phys: 0,
            flags: Flags::user_data(),
        }; FILE_PAGES + CLIENT_EXTRA_STACK];
        // The whole channel, because this is the spawn the throughput benchmark comes through and
        // a client may not ask for more than it mapped (`filesystem_protocol::fs::TRANSFER_PAGES`). It costs
        // fifteen extra page-table entries against the same frames, not fifteen extra frames.
        let n = map_channel(&mut maps, FILE_VA_CLIENT, file_shared, FILE_PAGES);
        for (k, m) in maps[n..n + extra_stack].iter_mut().enumerate() {
            m.va = USER_STACK_VA - (k as u64 + 1) * FRAME_SIZE;
            m.phys = page_frame();
        }
        run(
            client_image,
            Spawn {
                arg0: role,
                arg1: arg,
                // A third word, because the `rm` program is started the way a **grant** is
                // (`filesystem_protocol::grant`): a spec word and two words of name. Every other client
                // here takes a role and one number and leaves this zero.
                arg2,
                grants: &[
                    rendezvous_cap(file_ep, Rights::WRITE), // slot 0: CALL the FS server
                    rendezvous_cap(report, Rights::WRITE),  // slot 1: report to the kernel
                ],
                maps: &maps[..n + extra_stack],
            },
        )
    })
    .expect("could not spawn the FS client");
    report
}

/// Wire the service (or reuse this boot's) and spawn the hand-written client
/// (`fixtures/src/fs_test_client.rs`): the file-service endpoint, which IS its directory capability, the
/// report endpoint, and its view of the shared file page. It names nothing else in the system.
///
/// Returns `(readiness, report)`: the two readiness endpoints if this call wired the service,
/// and the endpoint the client reports on.
pub fn start(
    blk_image: &'static [u8],
    fs_server_image: &'static [u8],
    client_image: &'static [u8],
    client_role: u64,
) -> Option<(Readiness, RendezvousId)> {
    let (file_ep, file_shared, readiness) = ensure(blk_image, fs_server_image)?;
    // 0 = the end-to-end proof; 1 = the fs_read benchmark loop.
    let report = spawn_fs_client(client_image, file_ep, file_shared, client_role, 0, 0, 0);
    Some((readiness, report))
}

/// **Wire a per-file grant and the program that holds it** (milestone 31 phase 2,
/// notes/grant-expression.md). This is what `run wc report.txt` resolves to, wired by the
/// kernel's test suite instead of by the shell, so the mechanism is gated on both ISAs.
///
/// Three processes, and the shape is the point:
///
/// ```text
///   FS server ──file IPC──► fs_file_caretaker ──narrowed file IPC──► the confined program
///                (a directory)          (one file, one direction)
/// ```
///
/// The caretaker holds the directory capability. The program holds an endpoint to the caretaker
/// and **nothing that names the FS server**, so "it cannot reach a second file" is a statement
/// about its capability table, not about a check it is trusted to pass. The narrowing is an address
/// space, which is why the attacker test below is a witness rather than an assertion.
///
/// One frame is shared by all three. Every request on both hops is a blocking `CALL`, so the
/// client is parked inside its own call for the whole time the caretaker is using the page; a
/// second frame would buy a copy and no isolation, since the client is entitled to the bytes
/// either way.
///
/// The grant, as one value, because its four fields are one decision: which file, in which
/// direction, handed to which program started how. Splitting them across a long argument list
/// invites a caller to get `rights` and `role` the wrong way round, and both are bare integers.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub struct Grant {
    /// The one name the caretaker will answer for. Must fit [`filesystem_protocol::grant::MAX_NAME`].
    pub name: &'static str,
    /// `grant::READ`, or `READ | WRITE`.
    pub rights: u64,
    /// The confined program's `arg0` (its role) and `arg1`.
    pub role: u64,
    pub arg: u64,
}

/// **Drain the FS service's two readiness sentinels**, which is a sequencing act and not only an
/// assertion.
///
/// Both servers announce with a blocking `SEND`, so each is parked inside its own announcement
/// until somebody receives it. Nothing they serve can be answered before that. Every caller that
/// needs the service to be *running* rather than merely spawned has to come through here first,
/// and [`wait_for_caretaker`] documents what that ordering is load-bearing for.
///
/// `None` means an earlier caller in this boot already wired the service and drained these.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub fn wait_for_service(readiness: Readiness) {
    let Some((blk_ready, fs_ready)) = readiness else {
        return;
    };
    // `None` means the block service was already running when this one was wired, so its sentinel
    // has been drained by whoever started it and there is nothing here to wait for.
    if let Some(blk_ready) = blk_ready {
        assert_eq!(
            crate::sched::ipc_receive(blk_ready)[0],
            filesystem_protocol::fixture::READY,
            "the block server did not bring the RedoxFS device up",
        );
    }
    assert_eq!(
        crate::sched::ipc_receive(fs_ready)[0],
        filesystem_protocol::fixture::READY,
        "the FS server did not open the RedoxFS image",
    );
}

/// **Wait for a caretaker's startup request to have been answered, before the program it
/// confines exists at all.**
///
/// This is a correction, and the bug it fixes is worth stating where it was made rather than
/// only in the note. All three processes share **one frame** (the module comment above argues
/// that is sound because every request on both hops is a blocking `CALL`, so the client is
/// parked inside its own call for the whole time the caretaker is using the page). That
/// argument holds once the caretaker is serving. It does not hold at **startup**, where the
/// caretaker stages the granted name in the page and then blocks in a `CALL` to the FS server:
/// a confined program that already exists writes its own first name over that page, and the FS
/// server resolves whatever it finds there.
///
/// It is not even a race in the common case. When this call is the one that wires the service,
/// the FS server is parked inside its readiness `SEND`, so the caretaker's descent cannot be
/// answered until [`wait_for_service`] drains it, and the client has that whole window to
/// clobber the page. The caretaker then dies (it will not serve a hole) and its client blocks
/// forever on a call nobody will answer, which is how this presented: a userspace `ebreak`
/// followed by the 60 s lost-wakeup watchdog. It passed on aarch64 and failed on riscv, which
/// is the shape of a timing bug and was one.
///
/// The fix is ordering, not a second page: drain the service, wait for the caretaker's own
/// sentinel, and only then spawn the confined program.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
fn wait_for_caretaker(caretaker_ready: RendezvousId) {
    assert_eq!(
        crate::sched::ipc_receive(caretaker_ready)[0],
        filesystem_protocol::fixture::READY,
        "the caretaker could not open what it was granted, so there is nothing to attenuate",
    );
}

#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub fn start_granted(
    blk_image: &'static [u8],
    fs_server_image: &'static [u8],
    caretaker_image: &'static [u8],
    client_image: &'static [u8],
    grant: Grant,
) -> Option<RendezvousId> {
    let Grant {
        name,
        rights,
        role: client_role,
        arg: client_arg,
    } = grant;
    assert!(
        filesystem_protocol::grant::fits(name.as_bytes()),
        "a granted name rides in two argument words; this one does not fit",
    );
    let (file_ep, file_shared, readiness) = ensure(blk_image, fs_server_image)?;
    let narrow_ep = crate::sched::create_rendezvous();
    let caretaker_ready = crate::sched::create_rendezvous();

    let (lo, hi) = filesystem_protocol::grant::pack_name(name.as_bytes());
    let spec = filesystem_protocol::grant::spec(name.len(), rights);

    // The caretaker: the directory capability, the narrowed endpoint it serves, and the shared
    // page. The grant itself (the name and the direction) rides in its START arguments, so a
    // per-file grant costs no frame at all.
    crate::sched::spawn(move || {
        run(
            caretaker_image,
            Spawn {
                arg0: lo,
                arg1: hi,
                arg2: spec,
                grants: &[
                    rendezvous_cap(file_ep, Rights::WRITE), // slot 0: CALL the FS server
                    rendezvous_cap(narrow_ep, Rights::READ), // slot 1: serve the narrowed capability
                    rendezvous_cap(caretaker_ready, Rights::WRITE), // slot 2: readiness, once
                ],
                maps: &[Mapping {
                    va: FILE_VA_CLIENT,
                    phys: file_shared,
                    flags: Flags::user_data(),
                }],
            },
        )
    })
    .expect("could not spawn the file caretaker");

    // Both handshakes before the client exists, in this order and for [`wait_for_caretaker`]'s
    // reason: the servers are parked inside their announcements until they are drained, and the
    // caretaker's own request is staged in the page all three share.
    wait_for_service(readiness);
    wait_for_caretaker(caretaker_ready);

    // The confined program. Its slot 0 looks exactly like a directory capability from inside, and
    // is not one: same protocol, same page, a namespace of one name.
    Some(spawn_fs_client(
        client_image,
        narrow_ep,
        file_shared,
        client_role,
        client_arg,
        0,
        0,
    ))
}

/// **Wire a per-directory grant and the program confined to it** (milestone 47,
/// notes/dir-capability.md). The same three-process shape [`start_granted`] wires, one rung up:
///
/// ```text
///   FS server ──file IPC──► fs_subtree_caretaker ──narrowed file IPC──► the confined program
///            (the image root)         (one subtree, one rights set)
/// ```
///
/// The caretaker holds the root directory capability and the confined program holds an endpoint
/// to the caretaker and **nothing that names the FS server**, which is what makes "it cannot
/// reach the parent directory or a sibling" a statement about its capability table rather than about a
/// branch. That argument is `fs_file_caretaker`'s, and it is load-bearing here for an extra
/// reason: the FS server's handle table is per *server*, so a rights-carrying handle on its own
/// would not confine a program that could still name [`filesystem_protocol::fs::ROOT`].
///
/// `rights` is a [`filesystem_protocol::dir`] mask. It is what the caretaker *asks* for; the FS server
/// intersects it with the root's and refuses if the answer is smaller, so a wiring that asked
/// for more than exists fails at the caretaker's first request rather than silently serving
/// less.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub struct DirGrant {
    /// The directory the caretaker descends into, one component under the image root. Must fit
    /// [`filesystem_protocol::grant::MAX_NAME`].
    pub name: &'static str,
    /// The [`filesystem_protocol::dir`] rights the subtree capability is to carry.
    pub rights: u64,
    /// The confined program's `arg0` (its role) and `arg1`.
    pub role: u64,
    pub arg: u64,
    /// Its `arg2`. Zero for every client that takes a role and a number; the `rm` program is
    /// started with a **grant's** three words instead (a spec and two of name), so it uses all
    /// three and its "role" word is the spec.
    pub arg2: u64,
    /// Stack pages beyond the one `run` maps, for a confined program that needs them. The
    /// hand-written attacker needs none; a shell does (see [`spawn_fs_client`]).
    pub stack_pages: usize,
}

/// **The FS server's binary, or `None` because nothing packed one into this archive** (milestone
/// 161; the reason changed under milestone 164 and this comment with it).
///
/// It exists as a named helper rather than an `.expect` somebody reads once because for two
/// months this was a **toolchain** fact rather than a machine one, unlike every other missing
/// fixture in this tree (no disk attached, no virtio-rng on the bus, no second core). The engine
/// this server links pulls in `aes` unconditionally, and `aes` would not codegen for
/// `x86_64-unknown-none` at any optimisation level: that target is `-mmx,-sse,+soft-float`, so
/// there was no 128-bit vector register for LLVM to legalise an AES block into.
///
/// **That is fixed** (milestone 164): `--cfg aes_force_soft` in `.cargo/config.toml` selects the
/// portable software backend `aes` already ships, and all three architectures build the server
/// and pack it. So `None` is once again an ordinary build fact: nothing built the binary before
/// the archive was packed. A test that needs a filesystem still skips with [`NO_FS_SERVER`]
/// rather than panicking on a lookup, and on `x86_64` it now more often skips one step later,
/// inside [`start`], for want of a disk this machine's runner does not attach yet
/// (design/roadmap/0164-x86-64-fs-server-aes.md).
///
/// **The archive entry is `redoxfs_server`, not `fs_server`, on every architecture that has one**
/// (milestone 140 increment zero renamed the crate and its packed name together). A lookup for the
/// old name returns `None` everywhere, which reads exactly like "no fixture on this build" and
/// skips 30 tests silently rather than failing loudly -- caught only because one of those thirty is
/// `kernel::user::tests::a_host_process_connects_to_the_guest_and_is_answered`, whose absence took
/// the mDNS responder and the inbound TCP listener down with it, which is what a
/// bisection of the resulting host-side network-check failures actually found.
pub fn fs_server_image() -> Option<&'static [u8]> {
    program("redoxfs_server")
}

/// The reason a test gives when there is no FS service to talk to. One string, because a dozen
/// files share one cause and a reader comparing two runs should not have to decide whether two
/// wordings mean the same thing.
///
/// **It names two causes, and that is deliberate rather than vague** (milestone 164). Most callers
/// reach it from [`fs_server_image`] being `None`, which is only the first; `rmle_tests` reaches
/// it from `rmle_service::start` returning `None`, which on a machine whose archive *does* carry
/// the binary means the second. Until milestone 164 the first cause was the only one that ever
/// fired on `x86_64` and the string named only it, which would now be a false statement in two
/// tests rather than an imprecise one in none.
pub const NO_FS_SERVER: &str = "no RedoxFS service on this boot: either nothing packed a \
                                redoxfs_server into this archive, or this machine has no RedoxFS \
                                disk for one to serve";

/// **`mkfs`'s binary, or `None`** (milestone 161). Same package as [`fs_server_image`], same
/// vendored engine underneath it, so it is absent for exactly the same reason and on exactly the
/// same targets; a separate accessor only so a skipped test names the program it actually wanted.
pub fn mkfs_image() -> Option<&'static [u8]> {
    program("mkfs")
}

/// The reason a test gives when [`mkfs_image`] is `None`. [`NO_FS_SERVER`]'s cause, one binary over.
pub const NO_MKFS: &str = "no mkfs in this archive (nothing built one for this target before \
                           the archive was packed)";

/// **The binary carrying the block server's role**, which is now the same one everywhere.
///
/// **Three `cfg` arms stood here until milestone 291**, because aarch64 reached this role through
/// the `hello` multiplexer while the other two boards had the dedicated `block_driver`. The two
/// shapes always ran the same code (`crates/virtio` is the driver; both binaries were dispatch
/// tables in front of it), so the disagreement bought nothing and cost a fact every caller of this
/// function had to be routed around. 291 packed `block_driver` into the aarch64 archive too and
/// the arms collapsed.
///
/// It panics rather than returning `None` because a boot archive without it is a build that did
/// not finish, not a machine without a disk; the disk's absence is [`root_directory`]'s `None`.
pub fn blk_server_image() -> &'static [u8] {
    program("block_driver").expect("no block_driver program in the initrd archive")
}

/// **Wire the filesystem and hand back the root directory capability**, for a boot rather than
/// for a test (milestone 50).
///
/// This is the one entry point the interactive boot uses. It brings up the block server and the
/// FS server, drains both readiness sentinels (so the service is *running* and not merely
/// spawned by the time the progenitor exists), and returns `(the file-service endpoint, the physical frame
/// its clients map)`. `None` means no RedoxFS disk is attached to this run, which is the normal
/// case for a plain `cargo xtask run`, and every caller treats it as "this boot has no
/// filesystem" rather than as an error.
///
/// The endpoint **is** the directory capability (DECISIONS §27), rooted at the image root. A
/// boot hands it to the shell unnarrowed on purpose: it is the machine's own prompt, and the
/// interesting confinement claims are about what the shell then hands to the programs it spawns.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub fn root_directory(
    blk_image: &'static [u8],
    fs_server_image: &'static [u8],
) -> Option<(RendezvousId, u64)> {
    let (file_ep, file_shared, readiness) = ensure(blk_image, fs_server_image)?;
    wait_for_service(readiness);
    Some((file_ep, file_shared))
}

/// **Wire a subtree caretaker and hand back the narrowed capability**, without spawning
/// whatever is going to hold it.
///
/// [`start_granted_dir`]'s first half, split out because milestone 50's redirection witness is
/// a shell that holds a directory **and** a terminal, a spawn channel and a budget, so its
/// client half is nothing like [`spawn_fs_client`]'s. What it shares with every other confined
/// program is exactly what is here: a caretaker between it and the FS server, and the page they
/// both map.
///
/// Returns `(narrow_ep, file_shared)`, with both handshakes already drained for
/// [`wait_for_caretaker`]'s reason, so a caller may spawn its client the moment this returns.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub fn narrow_dir(
    blk_image: &'static [u8],
    fs_server_image: &'static [u8],
    caretaker_image: &'static [u8],
    name: &'static str,
    rights: u64,
) -> Option<(RendezvousId, u64)> {
    let narrow_ep = crate::sched::create_rendezvous();
    let caretaker_ready = crate::sched::create_rendezvous();
    let (file_shared, _caretaker) = spawn_caretaker(
        blk_image,
        fs_server_image,
        caretaker_image,
        name,
        rights,
        narrow_ep,
        caretaker_ready,
    )?;
    Some((narrow_ep, file_shared))
}

/// [`narrow_dir`], **with a caretaker the caller can end** (milestone 121 (`ripgrep` on nife:
/// enumeration as a capability)).
///
/// [`narrow_dir`]'s caretaker parks in `RECEIVE` on an endpoint no region owns, so it lives for the
/// boot. That is fine for a test that grants one directory and was a frame-ledger failure for one
/// that grants two more: the aarch64 suite ran out of page frames three tests later. Here both
/// endpoints come out of a region of their own, `display_service`'s shape and for its reason,
/// and the returned [`super::holding::Holding`] reclaims that region, which wakes the caretaker to
/// die.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub fn narrow_dir_held(
    blk_image: &'static [u8],
    fs_server_image: &'static [u8],
    caretaker_image: &'static [u8],
    name: &'static str,
    rights: u64,
) -> Option<(RendezvousId, u64, RendezvousId, super::holding::Holding)> {
    // Three endpoints, a page each, and one spare: the caretaker's two, and the program's stdout.
    // All three live in the region so that one reclaim returns every endpoint this grant made. The
    // kernel's rendezvous registry is a fixed 512 and the suite runs close to it: two endpoints a
    // walk test left live were enough to exhaust it in `uuid_tests` on 2026-09-27.
    let ep_region = crate::memory_region::create(4).expect("no endpoint region for a caretaker");
    let endpoint =
        || crate::sched::create_rendezvous_from(ep_region).expect("no endpoint for a caretaker");
    let narrow_ep = endpoint();
    let caretaker_ready = endpoint();
    let report = endpoint();
    let (file_shared, caretaker) = spawn_caretaker(
        blk_image,
        fs_server_image,
        caretaker_image,
        name,
        rights,
        narrow_ep,
        caretaker_ready,
    )?;
    let mut held = super::holding::Holding::new();
    held.add_thread(caretaker);
    held.add_region(ep_region);
    Some((narrow_ep, file_shared, report, held))
}

/// The caretaker both of the above start, serving `narrow_ep` and reporting once on
/// `caretaker_ready`. Returns the file channel and the caretaker's thread.
#[allow(clippy::too_many_arguments)]
fn spawn_caretaker(
    blk_image: &'static [u8],
    fs_server_image: &'static [u8],
    caretaker_image: &'static [u8],
    name: &'static str,
    rights: u64,
    narrow_ep: RendezvousId,
    caretaker_ready: RendezvousId,
) -> Option<(u64, crate::thread::ThreadId)> {
    assert!(
        filesystem_protocol::grant::fits(name.as_bytes()),
        "a granted name rides in two argument words; this one does not fit",
    );
    let (file_ep, file_shared, readiness) = ensure(blk_image, fs_server_image)?;

    let (lo, hi) = filesystem_protocol::grant::pack_name(name.as_bytes());
    let spec = filesystem_protocol::grant::spec(name.len(), rights);

    let tid = crate::sched::spawn(move || {
        run(
            caretaker_image,
            Spawn {
                arg0: lo,
                arg1: hi,
                arg2: spec,
                grants: &[
                    rendezvous_cap(file_ep, Rights::WRITE), // slot 0: CALL the FS server
                    rendezvous_cap(narrow_ep, Rights::READ), // slot 1: serve the narrowed capability
                    rendezvous_cap(caretaker_ready, Rights::WRITE), // slot 2: readiness, once
                ],
                maps: &[Mapping {
                    va: FILE_VA_CLIENT,
                    phys: file_shared,
                    flags: Flags::user_data(),
                }],
            },
        )
    })
    .expect("could not spawn the subtree caretaker");

    // Both handshakes before the client exists. See [`wait_for_caretaker`]: this is the call
    // that found the bug, because a caretaker whose descent is answered only after the service
    // is drained gives its client an unbounded window to write over the staged name.
    wait_for_service(readiness);
    wait_for_caretaker(caretaker_ready);
    Some((file_shared, tid))
}

#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub fn start_granted_dir(
    blk_image: &'static [u8],
    fs_server_image: &'static [u8],
    caretaker_image: &'static [u8],
    client_image: &'static [u8],
    grant: DirGrant,
) -> Option<RendezvousId> {
    let DirGrant {
        name,
        rights,
        role: client_role,
        arg: client_arg,
        arg2: client_arg2,
        stack_pages,
    } = grant;
    let (narrow_ep, file_shared) =
        narrow_dir(blk_image, fs_server_image, caretaker_image, name, rights)?;

    // The confined program. Its slot 0 looks exactly like a directory capability from inside,
    // and is one: the same protocol, the same page, a namespace of one subtree.
    Some(spawn_fs_client(
        client_image,
        narrow_ep,
        file_shared,
        client_role,
        client_arg,
        client_arg2,
        stack_pages,
    ))
}

/// Where the nameset caretaker expects its read-only name-set page
/// (`components/src/fs_nameset_caretaker.rs`'s `SET_VA`).
const SET_VA_CARETAKER: u64 = 0x0000_0000_0070_0000;

/// **Wire a set grant and the program confined to it** (milestone 47's globbing lane,
/// notes/glob-grant.md). [`start_granted_dir`]'s shape with a narrower namespace:
///
/// ```text
///   FS server ──file IPC──► fs_nameset_caretaker ──narrowed file IPC──► the confined program
///            (the image root)         (one directory, and only the names in the set)
/// ```
///
/// The interesting difference from every other grant here is **where the grant lives**. A name
/// rides in two `START` argument words; a set does not fit in any number of registers, so it is
/// encoded into a **frame of its own and mapped read-only** into the caretaker, which copies it
/// into a local before it does anything else. That is the honest place for `ARG_MAX` to
/// reappear: it is the size of a capability now, not the size of a buffer, and it is bounded by
/// `filesystem_protocol::nameset::MAX_NAMES` at both ends.
///
/// The set is written **before the caretaker is spawned**, into a frame nothing else has ever
/// been handed, which is why it needs none of [`wait_for_caretaker`]'s ordering care: unlike
/// the shared page, no client can reach it at all.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub struct SetGrant<'a> {
    /// The directory the caretaker descends into, one component under the image root. The set's
    /// names are the names *in* it that the grant designates.
    pub dir: &'static str,
    /// The set, as `(name, is_dir)`: what the shell's expansion produced. At most
    /// `filesystem_protocol::nameset::MAX_NAMES` of them, and an over-long set is a panic here because the
    /// shell refuses it at the prompt (`grant_plan::Refusal::TooManyNames`), so one arriving means
    /// the wiring built a grant no command line could have expressed.
    pub names: &'a [(&'a [u8], bool)],
    /// The [`filesystem_protocol::dir`] rights the caretaker asks for on its descent.
    pub rights: u64,
    /// The confined program's three `START` words. `rm` is started with a grant's spec and two
    /// name words rather than a role and a number; see [`DirGrant`].
    pub role: u64,
    pub arg: u64,
    pub arg2: u64,
    /// Stack pages beyond the one `run` maps.
    pub stack_pages: usize,
}

#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub fn start_granted_set(
    blk_image: &'static [u8],
    fs_server_image: &'static [u8],
    caretaker_image: &'static [u8],
    client_image: &'static [u8],
    grant: SetGrant<'_>,
) -> Option<RendezvousId> {
    let SetGrant {
        dir,
        names,
        rights,
        role: client_role,
        arg: client_arg,
        arg2: client_arg2,
        stack_pages,
    } = grant;
    assert!(
        filesystem_protocol::grant::fits(dir.as_bytes()),
        "a granted directory's name rides in two argument words; this one does not fit",
    );
    let (file_ep, file_shared, readiness) = ensure(blk_image, fs_server_image)?;
    let narrow_ep = crate::sched::create_rendezvous();
    let caretaker_ready = crate::sched::create_rendezvous();

    // The set, encoded into its own frame before anything can see it. `encode` refuses rather
    // than truncating, and a truncated set would be a capability nobody planned.
    let set_phys = page_frame();
    let mut encoded = [0u8; filesystem_protocol::nameset::BYTES];
    let n = filesystem_protocol::nameset::encode(names, &mut encoded)
        .expect("this set does not fit one grant, so no command line could have named it");
    // SAFETY: a fresh frame of FRAME_SIZE bytes reachable through the direct map, and `n` is at
    // most `nameset::BYTES`, which is far smaller.
    unsafe {
        core::ptr::copy_nonoverlapping(encoded.as_ptr(), mmu::phys_to_virt(set_phys) as *mut u8, n);
    };

    let (lo, hi) = filesystem_protocol::grant::pack_name(dir.as_bytes());
    let spec = filesystem_protocol::grant::spec(dir.len(), rights);

    crate::sched::spawn(move || {
        run(
            caretaker_image,
            Spawn {
                arg0: lo,
                arg1: hi,
                arg2: spec,
                grants: &[
                    rendezvous_cap(file_ep, Rights::WRITE), // slot 0: CALL the FS server
                    rendezvous_cap(narrow_ep, Rights::READ), // slot 1: serve the narrowed capability
                    rendezvous_cap(caretaker_ready, Rights::WRITE), // slot 2: readiness, once
                ],
                maps: &[
                    Mapping {
                        va: FILE_VA_CLIENT,
                        phys: file_shared,
                        flags: Flags::user_data(),
                    },
                    // **Read-only, and that is not decoration.** The set is the namespace this
                    // process serves; a writable mapping would let the one program that must not
                    // be able to widen its own grant do exactly that.
                    Mapping {
                        va: SET_VA_CARETAKER,
                        phys: set_phys,
                        flags: Flags::user_rodata(),
                    },
                ],
            },
        )
    })
    .expect("could not spawn the nameset caretaker");

    wait_for_service(readiness);
    wait_for_caretaker(caretaker_ready);

    Some(spawn_fs_client(
        client_image,
        narrow_ep,
        file_shared,
        client_role,
        client_arg,
        client_arg2,
        stack_pages,
    ))
}

/// **Wire two live clients on one file service, for the shared-frame witness** (milestone 599 (a
/// frame per filesystem client channel), provisional).
///
/// This is the wiring finding 1 of `notes/shared-page-audit.md` describes: two live clients on one
/// file service, one rewriting the frame the other staged its name in. **After milestone 599 each
/// client claims its own pool window** ([`claim_window`]) and a badged endpoint naming it, so the
/// two map different frames and the file server reads each request from its caller's own window. The
/// attacker's write to its own window can no longer reach the victim's, which is the isolation the
/// witness now asserts. A **sync endpoint** sequences them deterministically so the test does not
/// depend on a race landing.
///
/// Both `fs_test_client` roles (`ROLE_SHARE_VICTIM`, `ROLE_SHARE_ATTACKER`) read one slot layout:
/// FILE at 0 (a badged view of the FS-service endpoint), REPORT at 1, SYNC at 2, and each maps its
/// own window's frame at `FILE_VA_CLIENT`. The victim calls the server; the attacker only writes its
/// frame and hands off, because holding a frame the service maps is the whole of what made a second
/// writer dangerous before the fix.
///
/// `shared` puts both clients on one window, the pre-599 wiring, as the witness's negative control.
///
/// Returns `(readiness, victim_report)`: the service's readiness sentinels if this call wired it,
/// and the endpoint the victim reports its verdict on. The attacker never reports; it signals the
/// victim and exits. `None` when no RedoxFS disk is attached, or when the pool has no free window
/// (which cannot happen with two clients against [`CLIENT_WINDOWS`] of at least three).
///
/// Provisional name (this lane's coinage); an architect names functions.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub fn start_shared_frame_witness(
    blk_image: &'static [u8],
    fs_server_image: &'static [u8],
    client_image: &'static [u8],
    victim_role: u64,
    attacker_role: u64,
    shared: bool,
) -> Option<(Readiness, RendezvousId)> {
    let (file_ep, _file_shared, readiness) = ensure(blk_image, fs_server_image)?;
    let sync = crate::sched::create_rendezvous();
    let victim_report = crate::sched::create_rendezvous();
    let attacker_report = crate::sched::create_rendezvous();

    // The service must be serving before either client calls: the FS server is parked in its
    // readiness SEND until this drains it, so a victim that called first would block forever. The
    // test binds `_readiness` and never drains it again (milestone 599).
    wait_for_service(readiness);
    // A window per client: its own frame, and a badge equal to its window index, so the server
    // reads each request from the caller's own frame.
    //
    // **`shared` is the negative control**: the attacker is put on the victim's own window, which is
    // the wiring every client had before milestone 599. The witness must then report the
    // substitution, and that is what makes its isolated run evidence rather than a test that would
    // pass whatever the wiring did.
    let (victim_win, victim_phys) = claim_window()?;
    let (attacker_win, attacker_phys) = if shared {
        (victim_win, victim_phys)
    } else {
        claim_window()?
    };

    // Spawn a witness client. Slot layout FILE=0 (the FS endpoint), REPORT=1, SYNC=2 (`READ|WRITE`
    // so a role can both `SEND` and `RECEIVE`). It maps its own window's frame at `FILE_VA_CLIENT`.
    //
    // Two shapes of the FILE endpoint, so the test covers both (milestone 599): a client that
    // **mints its own badge** gets an unbadged endpoint with `GRANT` and its window index in `arg1`,
    // and calls `abi::rendezvous::BADGE` itself; one that is **handed a pre-badged endpoint** gets a
    // badged view directly, the shape the progenitor builds for a confined child.
    let spawn_witness = move |role: u64, report: RendezvousId, win: u64, phys: u64, mint: bool| {
        crate::sched::spawn(move || {
            let mut maps = [Mapping {
                va: 0,
                phys: 0,
                flags: Flags::user_data(),
            }; FILE_PAGES];
            let n = map_channel(&mut maps, FILE_VA_CLIENT, phys, FILE_PAGES);
            let file = if mint {
                rendezvous_cap(file_ep, Rights::WRITE.union(Rights::GRANT))
            } else {
                rendezvous_cap_badged(file_ep, Rights::WRITE, win)
            };
            run(
                client_image,
                Spawn {
                    arg0: role,
                    arg1: if mint { win } else { 0 }, // the window a minting client badges for
                    arg2: 0,
                    grants: &[
                        file,                                                    // slot 0: the FS endpoint
                        rendezvous_cap(report, Rights::WRITE), // slot 1: report to the kernel
                        rendezvous_cap(sync, Rights::READ.union(Rights::WRITE)), // slot 2: handshake
                    ],
                    maps: &maps[..n],
                },
            )
        })
        .expect("could not spawn a shared-frame witness client");
    };

    // The victim first, so it is parked in its first `SEND` on the sync endpoint before the attacker
    // runs; the attacker's first act is a `RECEIVE` on the same endpoint, so the order they start in
    // does not change the handshake, but starting the victim first keeps the ordering obvious. The
    // victim mints its own badge (exercising the mint syscall); the attacker is handed a pre-badged
    // endpoint (the progenitor's shape). The attacker never calls the server, so its badge is
    // immaterial; what matters is that it maps a different window and cannot reach the victim's.
    spawn_witness(victim_role, victim_report, victim_win, victim_phys, true);
    spawn_witness(
        attacker_role,
        attacker_report,
        attacker_win,
        attacker_phys,
        false,
    );

    Some((readiness, victim_report))
}

/// **Put a file behind a byte sink** (milestone 50, notes/sink-protocol.md).
///
/// Wires the FS service (or reuses this boot's) and spawns `fixtures/src/file_sink.rs`: it holds
/// the FS-service endpoint, a report endpoint, and the page it shares with the FS server, and it
/// serves one endpoint whose only expressible request is "append these bytes".
///
/// `sink` is the capability a program's output slot gets, and **that endpoint is the whole of
/// what the writer holds**, which is the property the milestone rests on: it is created here
/// and handed out with `WRITE` to whoever is redirected and `READ` to the sink.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub struct FileSink {
    /// The FS service's two readiness endpoints, if this call is the one that wired it.
    pub readiness: Readiness,
    /// The byte sink. Goes into a program's output slot with `WRITE`.
    pub sink: RendezvousId,
    /// Readiness first, then `DONE` and the byte total at end of stream.
    pub report: RendezvousId,
}

#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub fn start_file_sink(
    blk_image: &'static [u8],
    fs_server_image: &'static [u8],
    file_sink_image: &'static [u8],
) -> Option<FileSink> {
    let (file_ep, file_shared, readiness) = ensure(blk_image, fs_server_image)?;
    let sink = crate::sched::create_rendezvous();
    let report = crate::sched::create_rendezvous();
    crate::sched::spawn(move || {
        run(
            file_sink_image,
            Spawn {
                arg0: 0,
                arg1: 0,
                arg2: 0,
                grants: &[
                    rendezvous_cap(sink, Rights::READ), // slot 0: the byte sink it serves
                    rendezvous_cap(file_ep, Rights::WRITE), // slot 1: CALL the FS server
                    rendezvous_cap(report, Rights::WRITE), // slot 2: readiness and the total
                ],
                maps: &[Mapping {
                    va: FILE_VA_CLIENT,
                    phys: file_shared,
                    flags: Flags::user_data(),
                }],
            },
        )
    })
    .expect("could not spawn the file sink");
    Some(FileSink {
        readiness,
        sink,
        report,
    })
}

/// **Read back what the file sink wrote**, in a different process with a different FS session.
///
/// Spawns `fixtures/src/file_source.rs`, and only after the sink has reported that it closed the
/// file, because the two share the FS server's one file page (the [`wait_for_caretaker`] lesson:
/// one page is sound between parties that are never using it at once, and sequencing is what makes
/// that true).
///
/// It streams the file's contents out **over the sink contract**, so the bytes that reach the
/// test arrive in the same sixteen-byte framing a `println!` does. Returns `(out, report)`.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub fn start_file_source(
    blk_image: &'static [u8],
    fs_server_image: &'static [u8],
    file_source_image: &'static [u8],
) -> Option<(RendezvousId, RendezvousId)> {
    let (file_ep, file_shared, _) = ensure(blk_image, fs_server_image)?;
    let out = crate::sched::create_rendezvous();
    let report = crate::sched::create_rendezvous();
    crate::sched::spawn(move || {
        run(
            file_source_image,
            Spawn {
                arg0: 0,
                arg1: 0,
                arg2: 0,
                grants: &[
                    rendezvous_cap(out, Rights::WRITE), // slot 0: where the file's bytes go
                    rendezvous_cap(file_ep, Rights::WRITE), // slot 1: CALL the FS server
                    rendezvous_cap(report, Rights::WRITE), // slot 2: the size it found
                ],
                maps: &[Mapping {
                    va: FILE_VA_CLIENT,
                    phys: file_shared,
                    flags: Flags::user_data(),
                }],
            },
        )
    })
    .expect("could not spawn the file source");
    Some((out, report))
}

/// The `std::fs` client's heap budget and extra stack. Same magnitudes as the networked std
/// program: it is a full std program (formatting, `Vec`, `String`, `read_to_string`), so it
/// needs the generous heap and the deep stack std's machinery wants.
const STD_FS_HEAP_PAGES: u64 = 256;
const STD_FS_STACK_PAGES: u64 = std_runtime_protocol::STACK_PAGES;

/// **Wire the service and endow a std program with a directory capability** (milestone 27 phase
/// two, the FS half).
///
/// This is the one spawn site that makes `std::fs` work: an ordinary std ELF, given the std slot
/// convention (heap untyped at 0, stdout at 1) **plus the FS-service endpoint at slot 4** and
/// the page it shares with the FS server, mapped at the VA the PAL expects
/// (`sys/pal/nife/rt.rs::FS_PAGE`). Slots 2 and 3 are deliberately left EMPTY, which is why
/// the grants go in by explicit slot instead of in order: this program holds a filesystem and no
/// network, and `std::net` must be able to tell.
///
/// The same binary spawned without slot 4 gets `Unsupported` from every `std::fs` call. That is
/// the whole point: the code never chose to have a filesystem, its capability table did.
///
/// Returns `(readiness, stdout)`: the readiness endpoints if this call wired the service, and
/// the program's stdout endpoint for the test to reassemble.
pub fn start_std(
    blk_image: &'static [u8],
    fs_server_image: &'static [u8],
    std_image: &'static [u8],
) -> Option<(Readiness, RendezvousId)> {
    let spawned = start_std_full(blk_image, fs_server_image, std_image)?;
    Some((spawned.readiness, spawned.report))
}

/// What [`start_std_full`] hands back: the service's readiness endpoints if this call wired it, the
/// program's stdout endpoint, the untyped region its heap was drawn from, and the thread it runs as.
pub struct StdSpawn {
    pub readiness: Readiness,
    pub report: RendezvousId,
    pub heap: u64,
    pub thread: crate::thread::ThreadId,
}

/// The same spawn, **also handing back the heap region and the thread**, so a caller that knows the
/// program exits can give the memory back (`user::holding`'s reasoning, milestone 121).
///
/// [`start_std`] deliberately does not: `std_exerciser` is spawned once and its 256 pages are a
/// charge this suite's frame ledger has always carried. A program built outside this tree is
/// different, because it is present only when somebody built it, so leaving its pages spoken for
/// would make the ledger fail for whoever ran the experiment and pass for everyone else. Rather than
/// budget for a test that usually does not run, its caller reclaims.
pub fn start_std_full(
    blk_image: &'static [u8],
    fs_server_image: &'static [u8],
    std_image: &'static [u8],
) -> Option<StdSpawn> {
    let (file_ep, file_shared, readiness) = ensure(blk_image, fs_server_image)?;
    let report = crate::sched::create_rendezvous();
    let (heap, thread, _stack, _args) =
        spawn_std(file_ep, 0, file_shared, report, std_image, None, None);
    Some(StdSpawn {
        readiness,
        report,
        heap,
        thread,
    })
}

/// What [`start_std_narrowed`] hands back: [`StdSpawn`] without the readiness endpoints, because
/// [`narrow_dir`] has already drained them before the program exists.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub struct NarrowedStd {
    pub report: RendezvousId,
    pub heap: u64,
    pub thread: crate::thread::ThreadId,
    /// The stack frames [`spawn_std`] allocated. A [`Mapping`] does not give its frame to the
    /// process, so they outlive it unless somebody frees them; see [`NarrowedStd::release`].
    stack: [u64; STD_FS_STACK_PAGES as usize],
    /// The argument page, if the program was given words. Not the process's either.
    args: Option<u64>,
    /// The caretaker in front of it, which [`NarrowedStd::release`] ends.
    caretaker: super::holding::Holding,
}

#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
impl NarrowedStd {
    /// **Give back everything the program held that the caller can name**: the heap's region and
    /// the stack frames. Only after the thread is gone, which the caller must have seen: a frame
    /// freed under a running program is a use-after-free with the program as the victim.
    ///
    /// And the caretaker, through its [`super::holding::Holding`]. Returns whether everything
    /// came back, which a test should assert: `false` is a service that outlived its holding.
    pub fn release(self) -> bool {
        assert!(
            !crate::sched::is_thread_present(self.thread),
            "released a std program's memory while it was still running",
        );
        let _ = crate::sched::reclaim_region(self.heap);
        for phys in self.stack.into_iter().chain(self.args) {
            crate::memory::free(page_frames::PageFrame::from_addr(phys));
        }
        self.caretaker.release()
    }
}

/// **A std program holding one subtree with a chosen set of rights** (milestone 121 (`ripgrep` on nife: enumeration as a capability)), rather than
/// the image root with all of them.
///
/// [`start_std_full`]'s spawn with a [`narrow_dir`] caretaker in front: the program's slot 4 is
/// the caretaker's narrowed endpoint instead of the FS server's own, and it maps the same file
/// page at the same VA, for the reason [`start_granted_two_dirs`] gives (one thread of control,
/// one `CALL` in flight). So `std::fs` inside it sees `name` as `/`, carrying `rights` and nothing
/// wider. Nothing in the PAL knows the difference, which is the point: a stranger's walker runs
/// against a narrowed grant exactly as it runs against the root.
///
/// Built for milestone 121: `std_exerciser`'s walk under `ENUMERATE | READ | DESCEND` and the same
/// walk refused under a grant lacking `ENUMERATE`, and then `rg` doing both. `line`, when given,
/// is a command line as typed at the prompt (`rg --stats 'walk entry'`): it becomes the program's
/// argv through `grant_plan::argv`, the function the shell uses, so the program hears exactly the
/// words `swish` would send for that line. Milestone 205 (how a foreign program is told what to do)
/// built that page as DECISIONS §170 (how a foreign program is told what to do) ruled.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub fn start_std_narrowed(
    blk_image: &'static [u8],
    fs_server_image: &'static [u8],
    caretaker_image: &'static [u8],
    std_image: &'static [u8],
    name: &'static str,
    rights: u64,
    line: Option<&[u8]>,
) -> Option<NarrowedStd> {
    start_std_narrowed_clocked(
        blk_image,
        fs_server_image,
        caretaker_image,
        std_image,
        name,
        rights,
        line,
        None,
    )
}

/// [`start_std_narrowed`], **and a wall clock**: `clock_page` is a clock service's page
/// (`clock_service::Wiring::page_phys`), granted read-only at `std_runtime_protocol::CLOCK_SLOT`
/// and mapped at `CLOCK_PAGE`, so `SystemTime::now()` answers instead of refusing. Built for
/// milestone 835 (a C library, stage 1: files, clock and memory): SQLite's `speedtest1` times every
/// test with `gettimeofday`, which nife's C library answers from `SystemTime`.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
#[allow(clippy::too_many_arguments)]
pub fn start_std_narrowed_clocked(
    blk_image: &'static [u8],
    fs_server_image: &'static [u8],
    caretaker_image: &'static [u8],
    std_image: &'static [u8],
    name: &'static str,
    rights: u64,
    line: Option<&[u8]>,
    clock_page: Option<u64>,
) -> Option<NarrowedStd> {
    let (narrow_ep, file_shared, report, caretaker) =
        narrow_dir_held(blk_image, fs_server_image, caretaker_image, name, rights)?;
    let (heap, thread, stack, args) = spawn_std(
        narrow_ep,
        0,
        file_shared,
        report,
        std_image,
        line,
        clock_page,
    );
    Some(NarrowedStd {
        report,
        heap,
        thread,
        stack,
        args,
        caretaker,
    })
}

/// What [`start_std_bound`] hands back: [`NarrowedStd`]'s shape, with the badge and window the
/// grant holds in place of a caretaker.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub struct BoundStd {
    pub report: RendezvousId,
    pub heap: u64,
    pub thread: crate::thread::ThreadId,
    stack: [u64; STD_FS_STACK_PAGES as usize],
    /// The FS server's endpoint, which the kernel calls to take the grant back.
    file_ep: RendezvousId,
    /// The client window, which is also the badge the grant is bound to, under §230 (badged
    /// endpoint capabilities).
    pub window: u64,
    /// The region the report endpoint was carved from, so one reclaim returns it.
    ep_region: u64,
}

#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
impl BoundStd {
    /// **Give back everything, and take the grant back first.** `UNBIND` from the kernel's own
    /// unbadged call closes every handle the badge minted and leaves the badge revoked; then the
    /// window returns to the pool, and the heap, stack and endpoint come back as
    /// [`NarrowedStd::release`] returns them. Returns whether the server accepted the `UNBIND`.
    pub fn release(self) -> bool {
        assert!(
            !crate::sched::is_thread_present(self.thread),
            "released a std program's memory while it was still running",
        );
        let unbound = crate::sched::ipc_call(
            self.file_ep,
            [
                filesystem_protocol::fs::req(filesystem_protocol::fs::UNBIND, 0, 0),
                self.window,
            ],
        )[0] == 0;
        release_window(self.window);
        let _ = crate::sched::reclaim_region(self.heap);
        for phys in self.stack {
            crate::memory::free(page_frames::PageFrame::from_addr(phys));
        }
        let _ = crate::sched::reclaim_region(self.ep_region);
        unbound
    }
}

/// **A std program holding one subtree, enforced by the FS server itself** (milestone 606 (a
/// directory walk costs what it does on Linux), calef's ruling D, 2026-09-27): the same grant as
/// [`start_std_narrowed`], with no caretaker in front.
///
/// The kernel claims a client window, opens `name` with `rights` under that window's badge, and
/// `BIND`s the badge to the handle, so from then on a request carrying the badge sees the
/// directory as its root and reaches only what it opens (`subtree_scope` decides both). The
/// program then gets the FS server's endpoint with that badge on it, and the window mapped where
/// the PAL expects its page. Only an eligible server takes `BIND`; `redoxfs_server` is the first.
///
/// Caretakers stay the default: the production choice between the two is the progenitor's, per
/// mount, from what the filesystem's package declares. This is the kernel harness's route, for
/// the tests and the bench boot.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub fn start_std_bound(
    blk_image: &'static [u8],
    fs_server_image: &'static [u8],
    std_image: &'static [u8],
    name: &'static str,
    rights: u64,
) -> Option<BoundStd> {
    let grant = bind_subtree(blk_image, fs_server_image, name, rights)?;
    let ep_region = crate::memory_region::create(1).expect("no endpoint region for a bound grant");
    let report = crate::sched::create_rendezvous_from(ep_region).expect("no report endpoint");
    let (heap, thread, stack, _args) = spawn_std(
        grant.file_ep,
        grant.window,
        grant.phys,
        report,
        std_image,
        None,
        None,
    );
    Some(BoundStd {
        report,
        heap,
        thread,
        stack,
        file_ep: grant.file_ep,
        window: grant.window,
        ep_region,
    })
}

/// A subtree bound to a client window's badge, before anyone holds it: the FS server's endpoint,
/// the window (which is the badge), and the window's physical base.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub struct BoundGrant {
    pub file_ep: RendezvousId,
    pub window: u64,
    pub phys: u64,
    /// Window 0's base, the one an unbadged call stages its name in.
    pub shared: u64,
}

/// **Bind `name` under the image root, with `rights`, to a fresh client window's badge** (ruling
/// D). [`start_std_bound`]'s first half, and what a test uses to speak for the badge itself.
#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub fn bind_subtree(
    blk_image: &'static [u8],
    fs_server_image: &'static [u8],
    name: &'static str,
    rights: u64,
) -> Option<BoundGrant> {
    use filesystem_protocol::fs;
    let (file_ep, shared, readiness) = ensure(blk_image, fs_server_image)?;
    wait_for_service(readiness);
    let (window, phys) = claim_window().expect("no free client window for a bound grant");
    // The binder speaks on its own unbadged channel, as the progenitor would: window 0, badge 0.
    // Never under the grant's badge, which may be a revoked one coming back from the pool (a
    // revoked badge reaches nothing until it is bound again, `subtree_scope::Binding`).
    // SAFETY: window 0's frame, which this module allocated and still owns, via the direct map;
    // the suite runs one test at a time, so no other client is mid-request on it.
    unsafe {
        core::ptr::copy_nonoverlapping(
            name.as_ptr(),
            mmu::phys_to_virt(shared) as *mut u8,
            name.len(),
        );
    }
    let opened = crate::sched::ipc_call(
        file_ep,
        [fs::req(fs::OPENDIR, fs::ROOT, name.len() as u64), rights],
    )[0] as i64;
    assert!(
        opened >= 0,
        "the FS server would not open {name:?} to bind: {opened}"
    );
    let bound =
        crate::sched::ipc_call(file_ep, [fs::req(fs::BIND, opened as u64, 0), window])[0] as i64;
    assert_eq!(bound, 0, "the FS server refused to bind {name:?}");
    Some(BoundGrant {
        file_ep,
        window,
        phys,
        shared,
    })
}

mod std_spawn;
use std_spawn::spawn_std;

/// **Two directory grants to one process** (milestone 154,
/// design/roadmap/0154-multi-directory-namespace.md). The endowment question milestone 47's
/// `bind` and milestone 64's `File::open` fork both independently found unbuilt: nothing before
/// this granted a *second* directory capability to one process. `start_granted_dir` starts one
/// caretaker and hands one endpoint; this starts two, for one confined program:
///
/// ```text
///   FS server ──file IPC──► fs_subtree_caretaker A ──narrowed file IPC──►
///            (the image root)      (subtree A, one rights set)          the confined program
///                       └────────► fs_subtree_caretaker B ──narrowed file IPC──►  (slot 0: A
///                                  (subtree B, one rights set)                     slot 1: B
///                                                                                   slot 2: report)
/// ```
///
/// **The spawn-protocol position that says which directory is which is the capability table slot**: the
/// confined program's slot 0 is always [`TwoDirGrant::a`], slot 1 always [`TwoDirGrant::b`].
/// That is the whole of what this milestone decides about the wire, deliberately: a shell-to-progenitor
/// encoding for a *second* `DIR_BIT` grant (extending `grant_plan::spawnproto`'s `GRANT_WORDS`
/// precedent the way a real interactive `bind` eventually will) is a design fork this milestone's
/// own roadmap block leaves to whoever wires this into the shell.
///
/// Both caretakers share the one FS server this boot has: [`narrow_dir`]'s idempotent `ensure`
/// pays for wiring it once and the second call reuses what the first built. Both narrowed
/// endpoints end up mapping the **same** shared file-channel frame at [`FILE_VA_CLIENT`], and
/// that is safe for the reason [`narrow_dir`]'s own doc gives one level narrower: the confined
/// program is one thread of control with at most one `CALL` in flight, so it is never mid-request
/// on both caretakers at once, whichever it happens to be talking to at a given moment owns the
/// page exclusively for the length of that call.
/// The last pair [`start_granted_two_dirs`] built: both grants as asked for, both narrowed
/// endpoints, and the shared file-channel frame. See the reuse note there.
#[allow(clippy::type_complexity)]
static TWO_DIRS: spin::Mutex<
    Option<(
        (&'static str, u64),
        (&'static str, u64),
        RendezvousId,
        RendezvousId,
        u64,
    )>,
> = spin::Mutex::new(None);

#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub struct TwoDirGrant {
    /// The first grant: the directory (one component under the image root) and the
    /// [`filesystem_protocol::dir`] rights, delivered at the confined program's capability table slot 0.
    pub a: (&'static str, u64),
    /// The second grant, at slot 1.
    pub b: (&'static str, u64),
    /// The confined program's `arg0` (its role) and `arg1`.
    pub role: u64,
    pub arg: u64,
    /// Stack pages beyond the one `run` maps, for a confined program that needs them.
    pub stack_pages: usize,
}

#[cfg_attr(not(any(test, feature = "system_tests")), allow(dead_code))]
pub fn start_granted_two_dirs(
    blk_image: &'static [u8],
    fs_server_image: &'static [u8],
    caretaker_image: &'static [u8],
    client_image: &'static [u8],
    grant: TwoDirGrant,
) -> Option<RendezvousId> {
    let TwoDirGrant {
        a,
        b,
        role,
        arg,
        stack_pages,
    } = grant;
    assert!(
        stack_pages <= CLIENT_EXTRA_STACK,
        "a two-directory client asked for more stack than this wiring maps",
    );

    // Both caretakers, fully up (their own readiness drained inside `narrow_dir`) before the
    // confined program exists, for [`wait_for_caretaker`]'s reason: a client that already existed
    // could clobber the shared page while a caretaker was still staging its own startup name in
    // it.
    //
    // **Built once per pair and reused**, since milestone 154 (a process that holds two directory
    // capabilities) added a second test of the same pair. A caretaker never exits, so every pair
    // this builds holds its regions for the rest of the boot, and the suite's region table is a
    // fixed 256 that it already runs close to. The second two-directory test asked for the same
    // two grants and, built fresh, filled the table: a later, unrelated test failed to load as
    // `Unmappable(OutOfPageFrames)`. A caretaker serves one request at a time and keeps no state
    // between clients, so a second client of the same pair is the same grant, not a shared one.
    let cached = *TWO_DIRS.lock();
    let (narrow_a, narrow_b, file_shared) = match cached {
        Some((ca, cb, na, nb, shared)) if ca == a && cb == b => (na, nb, shared),
        _ => {
            let (narrow_a, file_shared) =
                narrow_dir(blk_image, fs_server_image, caretaker_image, a.0, a.1)?;
            let (narrow_b, file_shared_b) =
                narrow_dir(blk_image, fs_server_image, caretaker_image, b.0, b.1)?;
            debug_assert_eq!(
                file_shared, file_shared_b,
                "one boot has one FS server, so both caretakers must share its one file channel",
            );
            *TWO_DIRS.lock() = Some((a, b, narrow_a, narrow_b, file_shared));
            (narrow_a, narrow_b, file_shared)
        }
    };

    let report = crate::sched::create_rendezvous();
    crate::sched::spawn(move || {
        let mut maps = [Mapping {
            va: 0,
            phys: 0,
            flags: Flags::user_data(),
        }; FILE_PAGES + CLIENT_EXTRA_STACK];
        let n = map_channel(&mut maps, FILE_VA_CLIENT, file_shared, FILE_PAGES);
        for (k, m) in maps[n..n + stack_pages].iter_mut().enumerate() {
            m.va = USER_STACK_VA - (k as u64 + 1) * FRAME_SIZE;
            m.phys = page_frame();
        }
        run(
            client_image,
            Spawn {
                arg0: role,
                arg1: arg,
                arg2: 0,
                grants: &[
                    rendezvous_cap(narrow_a, Rights::WRITE), // slot 0: CALL grant A's caretaker
                    rendezvous_cap(narrow_b, Rights::WRITE), // slot 1: CALL grant B's caretaker
                    rendezvous_cap(report, Rights::WRITE),   // slot 2: report to the kernel
                ],
                maps: &maps[..n + stack_pages],
            },
        )
    })
    .expect("could not spawn the two-directory client");
    Some(report)
}

/// **Write one of the owner's lists at the file service's root** (`kernel::user::login_tests` needs
/// to), as the owner's console would: the run-unvouched list, the suspended list of milestone 152
/// (durable delegation), or the durable-session manifest. `None` removes it.
#[cfg(any(test, feature = "system_tests"))]
pub fn set_root_list(file: &str, list: Option<&[u8]>) {
    set_file(None, file, list);
}

/// **Write `file` inside `identity`'s own subtree** (DECISIONS §117 (a principal's subtree is named
/// by its identity string)), which must already exist: an identity's stored schedule (§122 (the
/// on-disk schedule store)), as that identity's session would have written it. `None` removes it.
#[cfg(any(test, feature = "system_tests"))]
pub fn set_home_file(identity: &[u8], file: &str, contents: Option<&[u8]>) {
    set_file(Some(identity), file, contents);
}

/// [`set_root_list`] and [`set_home_file`]'s one body: `file` at the root, or under the subtree
/// named `dir`.
#[cfg(any(test, feature = "system_tests"))]
fn set_file(dir: Option<&[u8]>, file: &str, contents: Option<&[u8]>) {
    use filesystem_protocol::{dir as rights, fs};
    let (fs_ep, fs_page_frame) = root_directory(
        blk_server_image(),
        crate::user::program("redoxfs_server").expect("no redoxfs_server in the initrd"),
    )
    .expect("wired() already brought the file service up");
    // SAFETY: as in `ensure_home_subtree`: the file service's own shared page, idle between logins.
    let page = unsafe {
        core::slice::from_raw_parts_mut(
            mmu::phys_to_virt(fs_page_frame) as *mut u8,
            filesystem_protocol::PAGE,
        )
    };
    let call = |w: [u64; 2]| crate::sched::ipc_call(fs_ep, w)[0] as i64;
    let at = match dir {
        None => fs::ROOT,
        Some(d) => {
            page[..d.len()].copy_from_slice(d);
            let h = call([fs::req(fs::OPENDIR, fs::ROOT, d.len() as u64), rights::ALL]);
            assert!(
                h >= 0,
                "could not open the subtree {:?} ({h})",
                core::str::from_utf8(d)
            );
            h as u64
        }
    };
    let name = file.as_bytes();
    let named = |page: &mut [u8], verb: u64| {
        page[..name.len()].copy_from_slice(name);
        call([fs::req(verb, at, name.len() as u64), 0])
    };
    if let Some(contents) = contents {
        let mut h = named(page, fs::OPEN);
        if h < 0 {
            h = named(page, fs::CREATE);
        }
        assert!(h >= 0, "could not open or create {file} ({h})");
        let h = h as u64;
        assert_eq!(
            call([fs::req(fs::TRUNCATE, h, 0), 0]),
            0,
            "could not truncate {file}"
        );
        page[..contents.len()].copy_from_slice(contents);
        let wrote = call([fs::req(fs::WRITE, h, contents.len() as u64), 0]);
        assert_eq!(wrote, contents.len() as i64, "short write of {file}");
        call([fs::req(fs::CLOSE, h, 0), 0]);
    } else {
        // `ENOENT` is the state asked for, so the answer is not checked.
        named(page, fs::UNLINK);
    }
    if at != fs::ROOT {
        call([fs::req(fs::CLOSE, at, 0), 0]);
    }
}

/// **Install `bytes` as the one program of package `name` at `version`, as an installer would**
/// (milestone 152 (durable delegation), Fork 8 D, which runs a scheduled job from the store): the
/// bytes at `packages/<name>/<version>/<program>`, and a generation `1` in `activation/` naming them
/// with their digest, made live by `current`. Anything already there is overwritten, and a
/// generation this writes replaces the whole table, which no test here needs more of.
#[cfg(feature = "system_tests")]
pub fn install_for_test(program: &str, name: &str, version: &str, bytes: &[u8]) {
    use filesystem_protocol::{PAGE, dir as rights, fs};
    let (fs_ep, fs_page_frame) = root_directory(
        blk_server_image(),
        crate::user::program("redoxfs_server").expect("no redoxfs_server in the initrd"),
    )
    .expect("wired() already brought the file service up");
    // SAFETY: as in `set_file`: the file service's own shared page, idle between logins.
    let page = unsafe {
        core::slice::from_raw_parts_mut(mmu::phys_to_virt(fs_page_frame) as *mut u8, PAGE)
    };
    let call = |w: [u64; 2]| crate::sched::ipc_call(fs_ep, w)[0] as i64;
    let named = |page: &mut [u8], verb: u64, at: u64, name: &str, w1: u64| {
        page[..name.len()].copy_from_slice(name.as_bytes());
        call([fs::req(verb, at, name.len() as u64), w1])
    };
    // A directory under `at`, made if it is not there.
    let dir = |page: &mut [u8], at: u64, name: &str| {
        let made = named(page, fs::MKDIR, at, name, rights::ALL);
        if made >= 0 {
            return made as u64;
        }
        let h = named(page, fs::OPENDIR, at, name, rights::ALL);
        assert!(h >= 0, "could not make or open {name} ({h})");
        h as u64
    };
    // `contents` as the whole of `file` under `at`, a page at a time.
    let put = |page: &mut [u8], at: u64, file: &str, contents: &[u8]| {
        let mut h = named(page, fs::OPEN, at, file, 0);
        if h < 0 {
            h = named(page, fs::CREATE, at, file, 0);
        }
        assert!(h >= 0, "could not open or create {file} ({h})");
        let h = h as u64;
        assert_eq!(
            call([fs::req(fs::TRUNCATE, h, 0), 0]),
            0,
            "could not truncate {file}"
        );
        for (k, chunk) in contents.chunks(PAGE).enumerate() {
            page[..chunk.len()].copy_from_slice(chunk);
            let wrote = call([fs::req(fs::WRITE, h, chunk.len() as u64), (k * PAGE) as u64]);
            assert_eq!(wrote, chunk.len() as i64, "short write of {file}");
        }
        call([fs::req(fs::CLOSE, h, 0), 0]);
    };
    let packages = dir(page, fs::ROOT, activation_set::PACKAGES);
    let n = dir(page, packages, name);
    let v = dir(page, n, version);
    put(page, v, program, bytes);
    for h in [v, n, packages] {
        call([fs::req(fs::CLOSE, h, 0), 0]);
    }

    // The row and its default pointer (milestone 614 (two installed versions of one program)):
    // the package column is the package's name and the version is its own column, so the bytes'
    // path is read off the row.
    let entry = activation_set::Entry {
        program,
        version,
        package: name,
        digest: measured_boot::sha256(bytes),
    };
    let mut table = [0u8; 512];
    let len = activation_set::with_entry(
        "",
        &entry,
        crate::user::program(program).is_some(),
        &mut table,
    )
    .expect("one entry fits");
    let mut current = [0u8; 16];
    let current_len = activation_set::format_current(1, &mut current).expect("fits");
    let mut digits = [0u8; 10];
    let first = activation_set::generation_name(1, &mut digits);
    let activation = dir(page, fs::ROOT, activation_set::DIRECTORY);
    put(page, activation, first, &table[..len]);
    put(
        page,
        activation,
        activation_set::CURRENT,
        &current[..current_len],
    );
    call([fs::req(fs::CLOSE, activation, 0), 0]);
}

/// **Read one of the owner's files at the file service's root** into `out`, [`set_root_list`]'s
/// other half: the byte count, or `None` when there is no such file.
#[cfg(any(test, feature = "system_tests"))]
pub fn read_root_file(file: &str, out: &mut [u8]) -> Option<usize> {
    use filesystem_protocol::fs;
    let (fs_ep, fs_page_frame) = root_directory(
        blk_server_image(),
        crate::user::program("redoxfs_server").expect("no redoxfs_server in the initrd"),
    )?;
    // SAFETY: as in `set_root_list`: the file service's own shared page, idle between logins.
    let page = unsafe {
        core::slice::from_raw_parts_mut(
            mmu::phys_to_virt(fs_page_frame) as *mut u8,
            filesystem_protocol::PAGE,
        )
    };
    page[..file.len()].copy_from_slice(file.as_bytes());
    let h = crate::sched::ipc_call(fs_ep, [fs::req(fs::OPEN, fs::ROOT, file.len() as u64), 0])[0]
        as i64;
    if h < 0 {
        return None;
    }
    let want = out.len().min(filesystem_protocol::PAGE) as u64;
    let n = crate::sched::ipc_call(fs_ep, [fs::req(fs::READ, h as u64, want), 0])[0] as i64;
    crate::sched::ipc_call(fs_ep, [fs::req(fs::CLOSE, h as u64, 0), 0]);
    let n = usize::try_from(n).ok()?.min(out.len());
    out[..n].copy_from_slice(&page[..n]);
    Some(n)
}
