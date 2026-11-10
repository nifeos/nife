//! Wiring for the NVMe disk (§86 (whether an NVMe driver can leave the kernel)): two processes
//! from one image. The admin process gets BAR0's register and doorbell pages, the DMA run's admin
//! pages, and capabilities to the doorbell page and the run's data-plane pages; it brings the
//! controller up and grants the data plane those two. The data plane gets the request endpoint, a
//! readiness endpoint and the endpoint its grants arrive on, and one window per client
//! ([`Wiring::client_capability`]). The kernel's own part is [`find_and_confine`]. See
//! notes/non-volatile-memory-express.md.
//!
//! [`find_and_confine`]: crate::non_volatile_memory_express::find_and_confine

//! # BUGS
//!
//! - The data plane can aim DMA anywhere in the run, the admin rings included.
//! - With `CAP.DSTRD` 0 the doorbell page carries the admin doorbells too.
//! - The data plane cannot read `CSTS`, so a hung controller presents as a timeout, not
//!   `CSTS.CFS`.
//! - One command in flight, so throughput measured against it is a lower bound.
//! - A refused bring-up keeps its DMA run and the admin process's stack frames for the rest of
//!   the boot.
//!
//! Name: recorded (crates/non_volatile_memory_express), renamed 2026-09-18 under DECISIONS §154
//! (the acronym test is whether the phrase is spoken). A `<program>_service` like
//! `entropy_service`, so the name is the program's plus the suffix; the ruling and refusals live
//! in `crates/non_volatile_memory_express`.

use core::num::NonZeroU64;

use driver_process_contract::non_volatile_memory_express::{
    self as contract, ADMINISTRATION_PAGES, AdministrationReport, DATA_PLANE_PAGES, REGISTER_PAGES,
    administration_slots,
};

use super::*;
use crate::capability_object::{
    Rights, device_frame_capability, page_frame_run_capability, rendezvous_capability,
};
use crate::non_volatile_memory_express::{Absent, Found};
use crate::sched::RendezvousId;

/// Where the admin process's BAR0 pages are mapped, device-typed and listed on its startup page.
const REGISTERS_VA: u64 = address_space_map::pair_page(0x0000_0000_0098_0000);

/// Where the admin process's admin pages of the DMA run are mapped, listed on its startup page.
const ADMIN_QUEUES_VA: u64 = address_space_map::pair_page(0x0000_0000_009c_0000);

/// Stack pages the admin process gets beyond the one every loaded program gets. Its bring-up is
/// a call chain carrying 64-byte commands and the IDENTIFY bytes by value, and in a debug build
/// one page overflowed.
const ADMIN_EXTRA_STACK_PAGES: usize = 3;

/// Everything one wiring of the server is, from the outside.
pub struct Wiring {
    /// The server's readiness endpoint, and **only on the call that did the wiring**: the report
    /// is sent once and whoever asked first has taken it.
    pub ready: Option<RendezvousId>,
    /// The request endpoint. **This is the capability a client is given**, with WRITE; the server
    /// holds READ. Nothing about it names the device, the doorbells, or the DMA region.
    pub request: RendezvousId,
    /// The **physical** base of window 0's transfer buffer
    /// (`driver_process_contract::non_volatile_memory_express::FILESYSTEM_WINDOW`), the window an unbadged request stages
    /// in: the filesystem server's, and the kernel's own when it plays a client. The other
    /// windows follow it contiguously; [`Wiring::window_phys`] names one.
    pub transfer_phys: u64,
    /// True when the IOMMU unit this kernel programmed is the one that owns the controller's
    /// requester id ([`crate::iommu::Scope::is_confining`]), and the controller was confined to the
    /// DMA region before any process held BAR0. False means the driver is as unconfined as its
    /// arithmetic.
    pub confined_by_iommu: bool,
    /// Which unit owns the controller, and how: the evidence behind `confined_by_iommu`, printed
    /// by the bench preflight (fatal risk 6's first night-of condition).
    pub scope: crate::iommu::Scope,
    /// The controller's PCIe requester id.
    pub rid: u32,
    /// Logical blocks per 4096-byte filesystem block, as the admin process reported it: always in
    /// `1..=8` for a server that started (fatal risk 6's second night-of condition).
    pub blocks_per: u16,
    /// The namespace's size in bytes as the admin process read it from IDENTIFY, the number its
    /// [`driver_process_contract::non_volatile_memory_express::Handoff`] carried to the data plane.
    /// Tests compare against this, so they hold on QEMU's 8 MiB image and on a 256 GB disk alike.
    pub size_bytes: u64,
}

/// One NVMe server per boot, for the reason the entropy service is wired once: a second
/// bring-up would reset the controller and recreate the queues out from under the first, and the
/// first would then poll forever for a completion nothing was told to make.
static WIRED: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);
static REQUEST: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
static TRANSFER: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
static CONFINED: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);
static SIZE_BYTES: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
static RID: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
static BLOCKS_PER: core::sync::atomic::AtomicU16 = core::sync::atomic::AtomicU16::new(0);

/// Wire the NVMe server if this boot has not already, else hand back what is already running.
/// `None` when there is no NVMe controller on the bus, or when one is present and failed
/// bring-up. For a caller that must tell those two apart (a test, a bench), [`ensure_or_why`].
pub(crate) fn ensure(image: &'static [u8]) -> Option<Wiring> {
    ensure_or_why(image).ok()
}

/// [`ensure`], keeping the reason there is no server: no controller on the bus, or a controller
/// this driver refused and why. The second is never a skip.
pub fn ensure_or_why(image: &'static [u8]) -> Result<Wiring, Absent> {
    use core::sync::atomic::Ordering;

    if WIRED.load(Ordering::Acquire) {
        let rid = RID.load(Ordering::Relaxed);
        return Ok(Wiring {
            ready: None,
            request: REQUEST.load(Ordering::Relaxed),
            transfer_phys: TRANSFER.load(Ordering::Relaxed),
            confined_by_iommu: CONFINED.load(Ordering::Relaxed),
            scope: crate::iommu::scope_of(rid),
            rid,
            blocks_per: BLOCKS_PER.load(Ordering::Relaxed),
            size_bytes: SIZE_BYTES.load(Ordering::Relaxed),
        });
    }
    let w = start(image)?;
    REQUEST.store(w.request, Ordering::Relaxed);
    TRANSFER.store(w.transfer_phys, Ordering::Relaxed);
    CONFINED.store(w.confined_by_iommu, Ordering::Relaxed);
    SIZE_BYTES.store(w.size_bytes, Ordering::Relaxed);
    RID.store(w.rid, Ordering::Relaxed);
    BLOCKS_PER.store(w.blocks_per, Ordering::Relaxed);
    WIRED.store(true, Ordering::Release);
    Ok(w)
}

/// A running admin process, after its report: the controller is up and its one I/O pair exists.
struct Admin {
    /// The endpoint the data plane receives its handoff and grants on; the admin process is
    /// blocked sending there until a data plane takes them.
    grants: RendezvousId,
    /// The endpoint the admin process answers [`contract::RETIRE`] on.
    #[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
    control: RendezvousId,
    /// The admin process's thread, which exits once it has retired the controller.
    #[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
    thread: crate::thread::ThreadId,
    /// What it read from IDENTIFY.
    size_bytes: u64,
    blocks_per: u16,
    /// The extra stack frames its spawn mapped, freed once it is gone.
    #[cfg_attr(not(feature = "system_tests"), allow(dead_code))]
    stack: [u64; ADMIN_EXTRA_STACK_PAGES],
}

/// **Start the admin process on a found and confined controller, and wait for its report.**
///
/// It is handed BAR0's [`REGISTER_PAGES`] pages and the run's first [`ADMINISTRATION_PAGES`] pages as
/// spawn-time mappings, which it holds no name for and so cannot delegate, and two capabilities it
/// can: the doorbell page as a `DeviceFrame` and the run's data-plane pages as one `PageFrame` run.
/// [`Absent::Refused`] carries the phase it reported when it refused the controller.
fn start_admin(image: &'static [u8], found: &Found) -> Result<Admin, Absent> {
    let report = crate::sched::create_rendezvous();
    let grants = crate::sched::create_rendezvous();
    let control = crate::sched::create_rendezvous();
    let (bar0, dma) = (found.bar0, found.dma);
    let doorbell_page = device_frame_capability(
        bar0 + page_frames::FRAME_SIZE,
        Rights::WRITE.union(Rights::GRANT),
    );
    let data_plane = page_frame_run_capability(
        dma + ADMINISTRATION_PAGES * page_frames::FRAME_SIZE,
        NonZeroU64::new(DATA_PLANE_PAGES).expect("the data plane has pages"),
        Rights::READ.union(Rights::WRITE).union(Rights::GRANT),
    );
    let mut stack = [0u64; ADMIN_EXTRA_STACK_PAGES];
    for frame in &mut stack {
        *frame = crate::memory::alloc_zeroed()
            .expect("no frame for the NVMe admin process's stack")
            .addr();
    }
    const MAPS: usize = (REGISTER_PAGES + ADMINISTRATION_PAGES) as usize + ADMIN_EXTRA_STACK_PAGES;
    let thread = crate::sched::spawn(move || {
        let mut maps = [Mapping {
            va: 0,
            phys: 0,
            flags: Flags::user_data(),
        }; MAPS];
        // Device flags for BAR0, because a cached or reordered register write is one the
        // controller never sees.
        for (page, m) in maps.iter_mut().take(REGISTER_PAGES as usize).enumerate() {
            *m = Mapping {
                va: REGISTERS_VA + page as u64 * page_frames::FRAME_SIZE,
                phys: bar0 + page as u64 * page_frames::FRAME_SIZE,
                flags: Flags::user_device(),
            };
        }
        super::fs_service::map_channel(
            &mut maps[REGISTER_PAGES as usize..],
            ADMIN_QUEUES_VA,
            dma,
            ADMINISTRATION_PAGES as usize,
        );
        for (k, &frame) in stack.iter().enumerate() {
            maps[(REGISTER_PAGES + ADMINISTRATION_PAGES) as usize + k] = Mapping {
                va: USER_STACK_VA - (k as u64 + 1) * page_frames::FRAME_SIZE,
                phys: frame,
                flags: Flags::user_data(),
            };
        }
        // In `administration_slots` order: the grants are the process's whole authority.
        let grant_list = [
            rendezvous_capability(report, Rights::WRITE),
            rendezvous_capability(grants, Rights::WRITE),
            doorbell_page,
            data_plane,
            rendezvous_capability(control, Rights::READ),
        ];
        const _: () = assert!(administration_slots::CONTROL == 4);
        run(
            image,
            Spawn {
                arg0: contract::ADMINISTRATION_ROLE,
                arg1: dma, // the run's PHYSICAL base: queue creation names physical pages
                arg2: 0,
                grants: &grant_list,
                maps: &maps,
                windows: &[
                    startup_page_contract::Window {
                        kind: contract::REGISTER_KIND,
                        virtual_address: REGISTERS_VA,
                        pages: REGISTER_PAGES as u32,
                    },
                    startup_page_contract::Window {
                        kind: contract::ADMINISTRATION_QUEUE_KIND,
                        virtual_address: ADMIN_QUEUES_VA,
                        pages: ADMINISTRATION_PAGES as u32,
                    },
                ],
            },
        )
    })
    .expect("could not spawn the NVMe admin process");

    let [w0, w1, w2, ..] = crate::sched::ipc_receive(report);
    match AdministrationReport::unpack([w0, w1, w2]) {
        Some(AdministrationReport::Ready {
            size_bytes,
            blocks_per_transfer,
        }) => Ok(Admin {
            grants,
            control,
            thread,
            size_bytes,
            blocks_per: blocks_per_transfer,
            stack,
        }),
        Some(AdministrationReport::Refused(why)) => {
            crate::println!(
                "  non_volatile_memory_express: controller present but failed bring-up: {why:?}"
            );
            Err(Absent::Refused {
                rid: found.rid,
                why,
            })
        }
        None => panic!("the NVMe admin process sent a report no admin process sends: {w0:#x}"),
    }
}

/// **Spawn a data plane** with `request` and `ready` at its first two slots and the receiving end
/// of the admin process's grant endpoint at the third. It maps what arrives there into its own
/// space, so its spawn budgets the room.
fn spawn_data_plane(
    image: &'static [u8],
    request: crate::capability_object::Capability,
    ready: crate::capability_object::Capability,
    grants: RendezvousId,
) -> crate::thread::ThreadId {
    use contract::data_plane_slots;
    crate::sched::spawn(move || {
        let grant_list = [request, ready, rendezvous_capability(grants, Rights::READ)];
        const _: () = assert!(data_plane_slots::GRANTS == 2 && data_plane_slots::OWN_SPACE == 3);
        run_with_own_space_and_room(
            image,
            Spawn {
                arg0: contract::DATA_PLANE_ROLE,
                arg1: 0,
                arg2: 0,
                grants: &grant_list,
                maps: &[],
                windows: &[],
            },
            data_plane_slots::OWN_SPACE,
            // The doorbell page and the data-plane run, the two grants it maps.
            1 + DATA_PLANE_PAGES,
        )
    })
    .expect("could not spawn the NVMe data plane")
}

/// The physical base of window 0's transfer buffer in the run at `dma`.
fn transfer_phys_of(dma: u64) -> u64 {
    dma + (ADMINISTRATION_PAGES + contract::FIRST_TRANSFER_PAGE) * page_frames::FRAME_SIZE
}

/// Find and confine the controller, start its admin process, then the data plane it grants to.
fn start(image: &'static [u8]) -> Result<Wiring, Absent> {
    let found = crate::non_volatile_memory_express::find_and_confine()?;
    let rid = found.rid;
    let scope = crate::iommu::scope_of(rid);
    let admin = start_admin(image, &found)?;

    let ready = crate::sched::create_rendezvous();
    let request = crate::sched::create_rendezvous();
    spawn_data_plane(
        image,
        rendezvous_capability(request, Rights::READ),
        rendezvous_capability(ready, Rights::WRITE),
        admin.grants,
    );

    Ok(Wiring {
        ready: Some(ready),
        request,
        transfer_phys: transfer_phys_of(found.dma),
        confined_by_iommu: scope.is_confining(),
        scope,
        rid,
        blocks_per: admin.blocks_per,
        size_bytes: admin.size_bytes,
    })
}

/// The byte a confinement-falsification canary is planted with. Distinct from `0x00` (a zeroed
/// frame and the NVMe image's own contents) so "untouched" means "still the canary" rather than
/// "still zero", and a `READ` escape that landed, copying the image's zeros over it, is caught.
#[cfg(feature = "system_tests")]
const CANARY_BYTE: u8 = 0xC1;

/// One DMA-escape attacker, wired and running: what [`start_dma_escape`] hands its test caller.
#[cfg(feature = "system_tests")]
pub struct DmaEscape {
    /// The attacker's readiness endpoint. One message once it has tried both escapes: word 0 the
    /// read-escape status, word 1 the write-escape status (see `run_dma_escape`).
    pub report: RendezvousId,
    /// The physical base of the victim frame, outside the controller's DMA region. The test looks
    /// for this in the IOMMU's fault queue and reads its canary back through the direct map.
    pub victim: u64,
    /// True when the unit this kernel programmed owns the controller's requester id
    /// ([`crate::iommu::Scope::is_confining`]). False means the escape would be refused by nothing,
    /// so the test must not read a pass into it.
    pub confined_by_iommu: bool,
    /// The evidence behind `confined_by_iommu`, for the failure message.
    pub scope: crate::iommu::Scope,
    /// The controller's PCIe requester id, which ties a fault to it when the IOMMU reports no
    /// address (AMD-Vi under QEMU 11.1.1; see `arch/x86_64/iommu.rs`'s `Fault::addr`).
    pub rid: u32,
    /// The kernel thread running the attacker, which [`DmaEscape::release_completed`] waits out.
    thread: crate::thread::ThreadId,
    /// The one-page region `report` lives in, so [`DmaEscape::release_completed`] can destroy it; a kernel
    /// rendezvous (`create_rendezvous`) is never freed.
    ep_region: u64,
    /// The admin process that brought the controller up for the attacker, which
    /// [`DmaEscape::release_completed`] has take it down before the run goes back.
    admin: Admin,
    /// The run's physical base.
    dma: u64,
}

#[cfg(feature = "system_tests")]
impl DmaEscape {
    /// Block for the attacker's report (it sends once, after both escapes).
    pub fn wait(&self) -> [u64; 5] {
        crate::sched::ipc_receive(self.report)
    }

    /// True when every byte of the victim frame is still the canary, i.e. the controller's DMA
    /// never reached it. A `READ` escape that the IOMMU failed to refuse would have copied the
    /// disk's bytes over it, so this goes false exactly when the confinement does.
    ///
    /// Name: provisional, minted 2026-10-18 (UTC) by maintainer/names-comply-question-shapes.
    /// Refused `canary_intact` (an adjective alone reads as a getter).
    pub fn is_canary_intact(&self) -> bool {
        let p = crate::arch::mmu::phys_to_virt(self.victim) as *const u8;
        (0..page_frames::FRAME_SIZE as usize).all(|i| {
            // SAFETY: `victim` is a frame this wiring allocated and frees only in `release_completed`, which
            // consumes `self`, so it is still ours here. The direct map covers all of RAM, and `i`
            // stays inside the one frame. CPU reads bypass the IOMMU, which is the point: only the
            // device's view of this frame is unmapped.
            let byte = unsafe { p.add(i).read_volatile() };
            byte == CANARY_BYTE
        })
    }

    /// Give back the frames this wiring took: the controller's DMA region, the victim and the
    /// report endpoint's region. Returns whether everything came back.
    ///
    /// It waits for the attacker's thread to be gone, has the admin process disable the controller
    /// ([`contract::RETIRE`]) and waits for it to be gone too, since until all three hold something
    /// may still read or DMA into these frames. The IOMMU domain's tables stay, for the reason
    /// [`crate::non_volatile_memory_express::free_retired_run`] gives.
    ///
    /// Name: ratified 2026-10-28 (UTC) (calef, relayed by the maintainer), renamed from
    /// `release`: the question shape asks a `bool` `is_`, `has_`, `can_` or a finite verb, and
    /// this performs the release, so the ruled `is_released` would misstate it.
    pub fn release_completed(self) -> bool {
        // Ten seconds of TCG, far past an `exit` that follows the report this caller already took.
        let gone = |thread| {
            let deadline = crate::arch::timer::now() + 10 * crate::arch::timer::frequency();
            while crate::sched::is_thread_present(thread) && crate::arch::timer::now() < deadline {
                crate::sched::yield_now();
            }
            !crate::sched::is_thread_present(thread)
        };
        if !gone(self.thread) {
            return false;
        }
        if crate::sched::ipc_call(self.admin.control, [contract::RETIRE, 0])[0] != 0
            || !gone(self.admin.thread)
        {
            return false;
        }
        crate::non_volatile_memory_express::free_retired_run(self.dma);
        for &frame in &self.admin.stack {
            crate::memory::free(page_frames::PageFrame::from_addr(frame));
        }
        crate::memory::free(page_frames::PageFrame::from_addr(self.victim));
        crate::sched::reclaim_region(self.ep_region).is_ok()
    }
}

/// Bring the controller up confined, then spawn an EL0 server that aims a PRP outside its region.
/// `None` when no NVMe controller is on the bus.
///
/// The storage twin of `display_service::start_backing_escape`: the attacker gets exactly the
/// honest server's world, from the same admin process, and differs only in the physical address it
/// writes into a PRP. The kernel picks the victim frame and plants its canary, and only
/// [`DmaEscape::release_completed`] frees it, after the controller is disabled. The kernel picks
/// it because a server guessing "the frame past my region" may guess a frame inside the domain;
/// see notes/dma.md for the use-after-free.
#[cfg(feature = "system_tests")]
pub fn start_dma_escape(image: &'static [u8]) -> Option<DmaEscape> {
    // The victim, outside any run find_and_confine will confine the controller to. Planted with
    // the canary before the controller is enabled, so a landed escape is the only thing that could
    // change it.
    let victim = crate::memory::alloc()?.addr();
    // SAFETY: a freshly allocated frame, reached through the direct map; nothing else names it yet.
    unsafe {
        core::ptr::write_bytes(
            crate::arch::mmu::phys_to_virt(victim) as *mut u8,
            CANARY_BYTE,
            page_frames::FRAME_SIZE as usize,
        );
    }

    // Find and confine the controller to its own DMA run. In the shipped tree `ESCAPE_TARGET` is
    // zero, so `victim` is not in the controller's domain and the escape below faults. The
    // replayable falsification inserts a store of `victim` into `ESCAPE_TARGET` right here, which
    // widens the domain to cover it; that one line is the whole patch.
    let found = crate::non_volatile_memory_express::find_and_confine().ok()?;
    let rid = found.rid;
    let scope = crate::iommu::scope_of(rid);
    let transfer_phys = transfer_phys_of(found.dma);

    // `[ESCAPE_MAGIC, victim]` at the start of the transfer buffer, before either process exists.
    // No fence: the `crate::sched::spawn`s below are the happens-before edge to the reader.
    // SAFETY: `transfer_phys` is the data plane's transfer buffer, a frame of the run
    // `find_and_confine` allocated, reached through the direct map.
    unsafe {
        let p = crate::arch::mmu::phys_to_virt(transfer_phys) as *mut u64;
        p.write_volatile(driver_process_contract::non_volatile_memory_express::ESCAPE_MAGIC);
        p.add(1).write_volatile(victim);
    }
    let admin = start_admin(image, &found).ok()?;

    // In a region of its own rather than on the kernel's chunk, so `release` can hand it back.
    let ep_region = crate::memory_region::create(1).expect("no endpoint region for the attacker");
    let report = crate::sched::create_rendezvous_from(ep_region)
        .expect("no report endpoint for the attacker");

    // Slot 0 is unused by the attacker (it never serves a client); it keeps the slot shape the
    // honest server has. Slot 1 is where it reports.
    let thread = spawn_data_plane(
        image,
        rendezvous_capability(report, Rights::READ),
        rendezvous_capability(report, Rights::WRITE),
        admin.grants,
    );

    Some(DmaEscape {
        report,
        victim,
        confined_by_iommu: scope.is_confining(),
        scope,
        rid,
        thread,
        ep_region,
        admin,
        dma: found.dma,
    })
}

impl Wiring {
    /// The physical base of client window `window`'s transfer buffer: the pages a client granted
    /// [`Wiring::client_capability`]`(window)` must be mapped, and no others.
    ///
    /// Name: provisional, minted 2026-10-18 (UTC) by lane/ioping-direct-io.
    pub fn window_phys(&self, window: u64) -> u64 {
        assert!(
            ::driver_process_contract::non_volatile_memory_express::client_window(window).is_some(),
            "no NVMe client was given window {window}",
        );
        self.transfer_phys + window * contract::TRANSFER_PAGES * page_frames::FRAME_SIZE
    }

    /// The request endpoint as client window `window` is granted it: `WRITE`, badged with the
    /// window, so the server stages that client's requests in [`Wiring::window_phys`]`(window)`.
    /// Window 0 is unbadged, as the filesystem server has always held it.
    ///
    /// Name: provisional, minted 2026-10-18 (UTC) by lane/ioping-direct-io.
    pub fn client_capability(&self, window: u64) -> crate::capability_object::Capability {
        assert!(
            ::driver_process_contract::non_volatile_memory_express::client_window(window).is_some()
        );
        crate::capability_object::rendezvous_capability_badged(self.request, Rights::WRITE, window)
    }

    /// Take the startup report, or `None` when this caller was not the one that wired the server.
    /// Word 0 is [`filesystem_contract::READY`] when the first command round-tripped, and
    /// a `0xDEAD_..` word naming the failing step otherwise; word 1 is the namespace size the
    /// server was handed, so a bench transcript can see the geometry reached ring 3 intact.
    pub fn wait_for_ready(&self) -> Option<[u64; 5]> {
        self.ready.map(crate::sched::ipc_receive)
    }

    /// Play a client: one blk `operation` on `block` over the request endpoint, the way any holder
    /// of that endpoint would. Returns the server's status word.
    pub fn blk(&self, operation: u64, block: u64) -> i64 {
        crate::sched::ipc_call(self.request, [filesystem_contract::req(operation), block])[0] as i64
    }

    /// The transfer buffer's first block, through the direct map, for a caller staging a write or
    /// checking a read.
    ///
    /// # Safety
    /// The server must not be mid-transfer, which every caller gets from the blk contract's own
    /// turn-taking: a request is a `CALL`, so the client is blocked exactly while the server runs.
    pub unsafe fn transfer_block(
        &self,
    ) -> &'static mut [u8; crate::non_volatile_memory_express::BLOCK_SIZE] {
        // SAFETY: forwarded from this function's own contract; `transfer_phys` is a frame this
        // wiring allocated and the direct map covers all of RAM.
        unsafe {
            &mut *(crate::arch::mmu::phys_to_virt(self.transfer_phys)
                as *mut [u8; crate::non_volatile_memory_express::BLOCK_SIZE])
        }
    }
}
