//! The block-device roster, the disk surveyor, and the blank disk's partitioner and `mkfs`.
//!
//! The kernel's part is two separate authorities. It writes one roster page (`block_roster`) that
//! lists every block device and is handed out read-only, a listing and not a handle. It hands one
//! device to one process: a block server owns one disk's DMA and transport
//! (`fs_service::spawn_block_server`), and a client holds an endpoint to it and a page they share.
//! The kernel never reads a partition table.
//!
//! Every page is a `PageFrame` capability the program maps itself, out of a budget it also holds;
//! only the stack is wired at spawn. See notes/block-devices.md.

use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use super::*;
use crate::cap::{Rights, memory_region_cap, page_frame_cap, rendezvous_cap};
use crate::sched::RendezvousId;

/// Which mmio block device the surveyor is given. The runners attach the GPT-partitioned image as
/// the FOURTH mmio disk, after nifefs (0), RedoxFS (1) and the crash image (2); see
/// `helpers/qemu-runner-*.sh`, which explain why command-line order is reversed.
const GPT_DISK: usize = 3;

/// Which mmio block device the write half is given: the fifth, a 64 MiB image of zeros the runner
/// regenerates every boot and no other test reads (§27 (the filesystem service)).
const BLANK_DISK: usize = 4;

/// Where the surveyor maps the roster in its own address space. The program chooses it; the
/// kernel names it only so the probe's test can assert its fault landed exactly here.
pub const ROSTER_VA: u64 = address_space_map::service_window(0x5001_0000);

/// The budget, in pages, every program on this path draws its page tables from.
///
/// `PageFrame::MAP` retypes tables out of the caller's untyped. Two mappings 64 KiB apart share an
/// L3, so the cost is one L3 plus the levels above it; eight pages is `page_frame_service`'s number.
/// It is a reservation, like [`MAKER_BUDGET_PAGES`].
const MAP_BUDGET_PAGES: u64 = 8;

// The roles, in `a0`. Must match components/src/disk_surveyor.rs.
const ROLE_SURVEY: u64 = 0;
const ROLE_PROBE: u64 = 1;
const ROLE_HOLDER: u64 = 2;

// The surveyor's capability table, one layout for every role. Must match
// components/src/disk_surveyor.rs. A role that lacks a capability leaves its slot empty.
const SURVEY_SLOT_REPORT: u64 = 0;
const SURVEY_SLOT_BLK: u64 = 1;
const SURVEY_SLOT_BUDGET: u64 = 2;
const SURVEY_SLOT_BLK_PAGE: u64 = 3;
const SURVEY_SLOT_ROSTER: u64 = 4;
const SURVEY_SLOT_RESUME: u64 = 5;

/// Stack pages below the single one `run` maps, for the survey role. With none the surveyor
/// overflowed by about 200 bytes: `GloballyUniqueIdentifierPartitionTable::parse` walks 128
/// entries of 128 bytes each in a debug build, and `check_backup` decodes a second header.
const SURVEY_EXTRA_STACK: usize = 4;

/// What the surveyor was wired with, so a test can take its reports.
pub struct Wiring {
    /// The surveyor's report endpoint: two messages, the roster summary then the table verdict.
    pub report: RendezvousId,
    /// The block server's readiness endpoint, so a hang in device bring-up is distinguishable from
    /// a hang in the first read.
    pub ready: RendezvousId,
    /// The roster's physical frame, so the kernel's own tests can read the same bytes the surveyor
    /// sees without holding a mapping of their own.
    pub roster_phys: u64,
    /// How many devices the kernel put in the roster.
    pub devices: usize,
}

/// Every block device the kernel can see, in roster order, and how many there are.
///
/// mmio first in slot order, then virtio-blk on PCIe, then NVMe controllers. Each `ordinal` is the
/// number the matching finder counts by (`virtio::find_block_device_n`, the walk
/// `pci::find_nvme_device` takes), so a listing and a wiring agree on which disk is which. A device
/// is listed whether or not a server has been wired for it.
fn devices() -> (
    [block_roster::Device; block_roster::capacity_of(FRAME_SIZE as usize)],
    usize,
) {
    let mut out = [block_roster::Device {
        ordinal: 0,
        transport: block_roster::TRANSPORT_VIRTIO_MEMORY_MAPPED_IO,
    }; block_roster::capacity_of(FRAME_SIZE as usize)];
    let mut n = 0;

    for i in 0..crate::virtio::count_block_devices() {
        if n == out.len() {
            break;
        }
        out[n] = block_roster::Device {
            ordinal: i as u32,
            transport: block_roster::TRANSPORT_VIRTIO_MEMORY_MAPPED_IO,
        };
        n += 1;
    }
    for i in 0..crate::pci::count_block_devices() {
        if n == out.len() {
            break;
        }
        out[n] = block_roster::Device {
            ordinal: i as u32,
            transport: block_roster::TRANSPORT_VIRTIO_PCI,
        };
        n += 1;
    }
    for i in 0..crate::pci::count_non_volatile_memory_express_controllers() {
        if n == out.len() {
            break;
        }
        out[n] = block_roster::Device {
            ordinal: i as u32,
            transport: block_roster::TRANSPORT_NON_VOLATILE_MEMORY_EXPRESS,
        };
        n += 1;
    }
    (out, n)
}

/// Allocate and fill the roster page. Returns its physical frame and how many devices it names.
fn roster_page() -> (u64, usize) {
    let phys = crate::memory::alloc()
        .expect("no frame for the block-device roster")
        .addr();
    // SAFETY: a freshly allocated frame, reachable through the direct map, owned by nobody yet.
    let page = unsafe {
        core::slice::from_raw_parts_mut(mmu::phys_to_virt(phys) as *mut u8, FRAME_SIZE as usize)
    };
    page.fill(0);
    let (devs, n) = devices();
    block_roster::write(page, &devs[..n]).expect("the roster page sizes its own array");
    (phys, n)
}

/// Wire and spawn the surveyor over the GPT-partitioned test disk.
///
/// `None` when the machine has no fourth mmio block device, which is every boot the runner did not
/// build the GPT image for. That is a fact about the machine and not a failure: the caller skips.
pub fn start(blk_image: &'static [u8], surveyor_image: &'static [u8]) -> Option<Wiring> {
    let dev = crate::virtio::find_block_device_n(GPT_DISK)?;
    let (blk_ep, ready, blk_shared) = fs_service::spawn_block_server(blk_image, dev);
    let (roster_phys, devices) = roster_page();
    let report = crate::sched::create_rendezvous();
    let budget =
        crate::memory_region::create(MAP_BUDGET_PAGES).expect("no map budget for the surveyor");

    crate::sched::spawn(move || {
        // The stack is wired at spawn: a process needs one before it can make the syscall that
        // would map it.
        let mut stack = [Mapping {
            va: 0,
            phys: 0,
            flags: Flags::user_data(),
        }; SURVEY_EXTRA_STACK];
        for (k, m) in stack.iter_mut().enumerate() {
            m.va = USER_STACK_VA - (k as u64 + 1) * FRAME_SIZE;
            m.phys = crate::memory::alloc()
                .expect("no frame for the surveyor's stack")
                .addr();
        }
        crate::sched::grant_at(SURVEY_SLOT_REPORT, rendezvous_cap(report, Rights::WRITE))
            .expect("surveyor slot 0 was occupied");
        crate::sched::grant_at(SURVEY_SLOT_BLK, rendezvous_cap(blk_ep, Rights::WRITE))
            .expect("surveyor slot 1 was occupied");
        crate::sched::grant_at(SURVEY_SLOT_BUDGET, memory_region_cap(budget))
            .expect("surveyor slot 2 was occupied");
        // `WRITE`, because a read is a round trip through the shared page in both directions.
        crate::sched::grant_at(
            SURVEY_SLOT_BLK_PAGE,
            page_frame_cap(blk_shared, Rights::READ.union(Rights::WRITE)),
        )
        .expect("surveyor slot 3 was occupied");
        // `READ` alone: `PageFrame::MAP` refuses a writable mapping of it ([`start_probe`]).
        crate::sched::grant_at(
            SURVEY_SLOT_ROSTER,
            page_frame_cap(roster_phys, Rights::READ),
        )
        .expect("surveyor slot 4 was occupied");
        run(
            surveyor_image,
            Spawn {
                arg0: ROLE_SURVEY,
                arg1: 0,
                arg2: 0,
                grants: &[], // every one of them is placed above, at its own slot
                maps: &stack,
            },
        )
    })
    .expect("could not spawn the disk surveyor");

    Some(Wiring {
        report,
        ready,
        roster_phys,
        devices,
    })
}

/// Spawn the negative control and return its report endpoint: the same binary and roster frame,
/// held `READ`, and no disk, so the claim is about the page alone.
///
/// It asks for a writable mapping (refused before a page table is written), maps the page
/// read-only instead, announces the address, and writes (a fault). See notes/block-devices.md.
pub fn start_probe(surveyor_image: &'static [u8]) -> RendezvousId {
    let (roster_phys, _) = roster_page();
    let report = crate::sched::create_rendezvous();
    let budget =
        crate::memory_region::create(MAP_BUDGET_PAGES).expect("no map budget for the probe");

    crate::sched::spawn(move || {
        crate::sched::grant_at(SURVEY_SLOT_REPORT, rendezvous_cap(report, Rights::WRITE))
            .expect("probe slot 0 was occupied");
        crate::sched::grant_at(SURVEY_SLOT_BUDGET, memory_region_cap(budget))
            .expect("probe slot 2 was occupied");
        crate::sched::grant_at(
            SURVEY_SLOT_ROSTER,
            page_frame_cap(roster_phys, Rights::READ),
        )
        .expect("probe slot 4 was occupied");
        run(
            surveyor_image,
            Spawn {
                arg0: ROLE_PROBE,
                arg1: 0,
                arg2: 0,
                grants: &[], // every one of them is placed above, at its own slot
                maps: &[],
            },
        )
    })
    .expect("could not spawn the roster probe");

    report
}

/// What the revocation witness was wired with. See [`start_holder`].
pub struct HolderWiring {
    /// Its report endpoint: the word it read, then (only if the kernel is broken) a second one.
    pub report: RendezvousId,
    /// The go-ahead. The kernel revokes the frame and then sends here.
    pub resume: RendezvousId,
    /// The roster's physical frame, so the test can revoke exactly the page the program holds and
    /// can read the same bytes back through the direct map.
    pub roster_phys: u64,
}

/// Spawn a program that holds the roster as a `PageFrame` and is about to lose it to
/// `PageFrame::REVOKE`, which reaches it because the mapping is recorded.
///
/// It gets no disk and no shared block page, as [`start_probe`] does not. See
/// notes/block-devices.md.
pub fn start_holder(surveyor_image: &'static [u8]) -> HolderWiring {
    let (roster_phys, _) = roster_page();
    let report = crate::sched::create_rendezvous();
    let resume = crate::sched::create_rendezvous();
    let budget =
        crate::memory_region::create(MAP_BUDGET_PAGES).expect("no map budget for the holder");

    crate::sched::spawn(move || {
        crate::sched::grant_at(SURVEY_SLOT_REPORT, rendezvous_cap(report, Rights::WRITE))
            .expect("holder slot 0 was occupied");
        crate::sched::grant_at(SURVEY_SLOT_BUDGET, memory_region_cap(budget))
            .expect("holder slot 2 was occupied");
        crate::sched::grant_at(
            SURVEY_SLOT_ROSTER,
            page_frame_cap(roster_phys, Rights::READ),
        )
        .expect("holder slot 4 was occupied");
        crate::sched::grant_at(SURVEY_SLOT_RESUME, rendezvous_cap(resume, Rights::READ))
            .expect("holder slot 5 was occupied");
        run(
            surveyor_image,
            Spawn {
                arg0: ROLE_HOLDER,
                arg1: 0,
                arg2: 0,
                grants: &[], // every one of them is placed above, at its own slot
                maps: &[],
            },
        )
    })
    .expect("could not spawn the roster holder");

    HolderWiring {
        report,
        resume,
        roster_phys,
    }
}

impl HolderWiring {
    /// The roster as the kernel sees it, through the direct map: the witness the test grades the
    /// program's report against, belonging to no process in userspace.
    pub fn first_word(&self) -> u64 {
        // SAFETY: a frame this module allocated and still owns, named through the direct map.
        unsafe { core::ptr::read_volatile(mmu::phys_to_virt(self.roster_phys) as *const u64) }
    }
}

// =============================================================================================
// The write half: partitioning and formatting the blank disk
// =============================================================================================

/// Stack pages below the single one `run` maps, for the partitioner: the surveyor's four, for the
/// same parse, plus room for a 512-byte block of scratch.
const PARTITION_EXTRA_STACK: usize = 4;

// The partitioner's capability table. Must match components/src/disk_partitioner.rs. Slot 2 is the hole the
// entropy experiment turns on; see [`start_partitioner`].
const PARTITION_SLOT_REPORT: u64 = 0;
const PARTITION_SLOT_BLK: u64 = 1;
const PARTITION_SLOT_ENTROPY: u64 = 2;
const PARTITION_SLOT_BUDGET: u64 = 3;
const PARTITION_SLOT_BLK_PAGE: u64 = 4;

// `mkfs`'s capability table. Must match redoxfs_server/src/bin/mkfs.rs. Slots 1 and 2 are the holes; slot 0 is the
// heap budget, which is also what pays for the tables that map slot 4.
const MAKER_SLOT_BUDGET: u64 = 0;
const MAKER_SLOT_BLK: u64 = 1;
const MAKER_SLOT_ENTROPY: u64 = 2;
const MAKER_SLOT_REPORT: u64 = 3;
const MAKER_SLOT_BLK_PAGE: u64 = 4;

/// Stack pages for `mkfs`, sized for RedoxFS, which recurses and commits transactions on the stack.
///
/// A full file-server workload peaks at 34 pages (`fs_service::fs_stack_used`), so 48 is 40%
/// headroom. It is not the server's 96 because a boot runs five of these and never frees their
/// stacks. See notes/block-devices.md, "The kernel's wiring".
const MAKER_STACK_PAGES: u64 = 48;

/// The untyped budget every `mkfs` process draws its heap from, the same for the runs meant to be
/// refused, so a refusal cannot be explained by a smaller budget.
///
/// 1.5 MiB is four times a real mount's 352 KiB high-water (§27). It is far below
/// `fs_service::FS_BUDGET_PAGES` because an untyped is a reservation, and three 8 MiB ones do not
/// fit in a 128 MiB test boot.
const MAKER_BUDGET_PAGES: u64 = 384;

/// The blank disk's block server, wired once per boot, since a second server on the device would
/// reset its queue under the first. Its clients run in sequence, which makes one shared page safe.
static BLANK_WIRED: AtomicBool = AtomicBool::new(false);
static BLANK_BLK_EP: AtomicU64 = AtomicU64::new(0);
static BLANK_SHARED: AtomicU64 = AtomicU64::new(0);

/// The untyped region the last `mkfs` process was given, so the next one can hand it back
/// first. Zero means none yet. See [`start_maker`].
static LAST_BUDGET: AtomicU64 = AtomicU64::new(0);

/// The blank disk's block service: the endpoint clients `CALL`, the page they share with the
/// server, and (only for the caller that wired it) the server's readiness endpoint.
pub struct BlankDisk {
    pub blk_ep: RendezvousId,
    pub blk_shared: u64,
    /// `None` for every caller after the first, because the readiness word is sent once.
    pub blk_ready: Option<RendezvousId>,
}

/// Bring the blank disk up under a block server, or hand back the one already running.
///
/// `None` when the machine has no fifth mmio block device, which is every boot the runner did not
/// build the blank image for. A fact about the machine, so the caller skips.
pub fn blank_disk(blk_image: &'static [u8]) -> Option<BlankDisk> {
    if BLANK_WIRED.load(Ordering::Acquire) {
        return Some(BlankDisk {
            blk_ep: BLANK_BLK_EP.load(Ordering::Relaxed),
            blk_shared: BLANK_SHARED.load(Ordering::Relaxed),
            blk_ready: None,
        });
    }
    let dev = crate::virtio::find_block_device_n(BLANK_DISK)?;
    let (blk_ep, ready, blk_shared) = fs_service::spawn_block_server(blk_image, dev);
    BLANK_BLK_EP.store(blk_ep, Ordering::Relaxed);
    BLANK_SHARED.store(blk_shared, Ordering::Relaxed);
    BLANK_WIRED.store(true, Ordering::Release);
    Some(BlankDisk {
        blk_ep,
        blk_shared,
        blk_ready: Some(ready),
    })
}

/// Spawn the partitioner over the blank disk in `role`, and return its report endpoint.
///
/// `entropy` is the experiment: `Some(ep)` lets the program draw a GPT's unique ids, and `None`
/// leaves slot 2 empty, so its first `CALL` there is a kernel error that
/// `entropy_contract::delivered` reports as "no entropy". Nothing else differs.
pub fn start_partitioner(
    image: &'static [u8],
    disk: &BlankDisk,
    role: u64,
    entropy: Option<RendezvousId>,
) -> RendezvousId {
    let report = crate::sched::create_rendezvous();
    let blk_ep = disk.blk_ep;
    let blk_shared = disk.blk_shared;
    let budget =
        crate::memory_region::create(MAP_BUDGET_PAGES).expect("no map budget for the partitioner");
    let ep = entropy.unwrap_or_default();

    crate::sched::spawn(move || {
        let mut stack = [Mapping {
            va: 0,
            phys: 0,
            flags: Flags::user_data(),
        }; PARTITION_EXTRA_STACK];
        for (k, m) in stack.iter_mut().enumerate() {
            m.va = USER_STACK_VA - (k as u64 + 1) * FRAME_SIZE;
            m.phys = crate::memory::alloc()
                .expect("no frame for the partitioner's stack")
                .addr();
        }
        crate::sched::grant_at(PARTITION_SLOT_REPORT, rendezvous_cap(report, Rights::WRITE))
            .expect("partitioner slot 0 was occupied");
        crate::sched::grant_at(PARTITION_SLOT_BLK, rendezvous_cap(blk_ep, Rights::WRITE))
            .expect("partitioner slot 1 was occupied");
        if entropy.is_some() {
            crate::sched::grant_at(PARTITION_SLOT_ENTROPY, rendezvous_cap(ep, Rights::WRITE))
                .expect("partitioner slot 2 was occupied");
        }
        crate::sched::grant_at(PARTITION_SLOT_BUDGET, memory_region_cap(budget))
            .expect("partitioner slot 3 was occupied");
        crate::sched::grant_at(
            PARTITION_SLOT_BLK_PAGE,
            page_frame_cap(blk_shared, Rights::READ.union(Rights::WRITE)),
        )
        .expect("partitioner slot 4 was occupied");
        run(
            image,
            Spawn {
                arg0: role,
                arg1: 0,
                arg2: 0,
                grants: &[], // every one of them is placed above, at its own slot
                maps: &stack,
            },
        )
    })
    .expect("could not spawn the disk partitioner");

    report
}

/// Spawn `mkfs` over the blank disk in `role`, with or without either half of the pair it needs
/// (`entropy`, `with_disk`), and return its report endpoint.
///
/// The budget, the stack and the shared page are the same in every wiring, so only the missing
/// capability can explain a refusal.
pub fn start_maker(
    image: &'static [u8],
    disk: &BlankDisk,
    role: u64,
    entropy: Option<RendezvousId>,
    with_disk: bool,
) -> RendezvousId {
    let report = crate::sched::create_rendezvous();
    // Give the previous run's reservation back first: five at once cost 7.5 MiB of a 128 MiB
    // machine. That run has exited, so `destroy` revokes nothing live; a region it refuses is left.
    let previous = LAST_BUDGET.swap(0, Ordering::Relaxed);
    if previous != 0 {
        crate::memory_region::destroy(previous);
    }
    let budget = crate::memory_region::create(MAKER_BUDGET_PAGES).expect("no heap budget for mkfs");
    LAST_BUDGET.store(budget, Ordering::Relaxed);
    let blk_ep = disk.blk_ep;
    let blk_shared = disk.blk_shared;
    let has_entropy = entropy.is_some();
    let ep = entropy.unwrap_or_default();
    let mut stack = [0u64; MAKER_STACK_PAGES as usize];
    for f in stack.iter_mut() {
        *f = crate::memory::alloc()
            .expect("no frame for the mkfs stack")
            .addr();
    }

    crate::sched::spawn(move || {
        let mut maps = [Mapping {
            va: 0,
            phys: 0,
            flags: Flags::user_data(),
        }; MAKER_STACK_PAGES as usize];
        for (i, &phys) in stack.iter().enumerate() {
            maps[i] = Mapping {
                va: USER_STACK_VA - (i as u64 + 1) * FRAME_SIZE,
                phys,
                flags: Flags::user_data(),
            };
        }
        // Every slot placed with `grant_at`, so a withheld capability leaves a hole rather than
        // renumbering the slots above it (notes/abi.md, section 4).
        crate::sched::grant_at(MAKER_SLOT_BUDGET, memory_region_cap(budget))
            .expect("mkfs slot 0 was occupied");
        if with_disk {
            crate::sched::grant_at(MAKER_SLOT_BLK, rendezvous_cap(blk_ep, Rights::WRITE))
                .expect("mkfs slot 1 was occupied");
        }
        if has_entropy {
            crate::sched::grant_at(MAKER_SLOT_ENTROPY, rendezvous_cap(ep, Rights::WRITE))
                .expect("mkfs slot 2 was occupied");
        }
        crate::sched::grant_at(MAKER_SLOT_REPORT, rendezvous_cap(report, Rights::WRITE))
            .expect("mkfs slot 3 was occupied");
        // Granted in every run, even those meant to be refused, so a missing page cannot explain a
        // refusal. Its tables come out of slot 0, the program's own heap budget.
        crate::sched::grant_at(
            MAKER_SLOT_BLK_PAGE,
            page_frame_cap(blk_shared, Rights::READ.union(Rights::WRITE)),
        )
        .expect("mkfs slot 4 was occupied");
        run(
            image,
            Spawn {
                arg0: role,
                arg1: 0,
                arg2: 0,
                grants: &[], // every one of them is placed above, at its own slot
                maps: &maps,
            },
        )
    })
    .expect("could not spawn mkfs");

    report
}

impl Wiring {
    /// The roster as the kernel sees it, through the direct map. The surveyor holds a read-only
    /// mapping of the same frame; the layout comes from the one contract crate, so both sides are
    /// reading the same bytes with the same code.
    pub fn roster(&self) -> &'static [u8] {
        // SAFETY: a frame this module allocated and still owns, named through the direct map.
        unsafe {
            core::slice::from_raw_parts(
                mmu::phys_to_virt(self.roster_phys) as *const u8,
                FRAME_SIZE as usize,
            )
        }
    }
}
