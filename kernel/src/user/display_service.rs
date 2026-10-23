//! The display seam's wiring: a confined virtio-gpu driver, the client that paints through it, and
//! the terminal that takes the client's place.
//!
//! Each program holds its pages as `PageFrame` capabilities and maps them itself, out of a memory
//! region it also holds. A capability names a whole run (§102 (a Frame names a run of pages)), so
//! the [`graphics_contract::SURFACE_PAGE_FRAMES`]-page scanout is one capability and one `MAP` call.
//! The kernel places no page at spawn beyond the one `load` gives every process.
//!
//! See notes/framebuffer-contract.md and notes/framebuffer-contract/kernel-wiring.md.

use core::num::NonZeroU64;

use super::*;
use crate::cap::{
    Rights, irq_cap, memory_region_cap, page_frame_run_cap, page_frame_run_len, rendezvous_cap,
    virtio_cap,
};
use crate::sched::RendezvousId;

/// The DMA region, in frames: one for the rings and control buffers, then the surface.
const DMA_PAGE_FRAMES: u64 = 1 + graphics_contract::SURFACE_PAGE_FRAMES as u64;

/// The three run lengths this file grants.
///
/// They are `const` so that [`crate::cap::page_frame_run_len`]'s refusal of a zero-page run fails
/// the build, not a driver at boot. Name: provisional.
const SURFACE_RUN: NonZeroU64 = page_frame_run_len(graphics_contract::SURFACE_PAGE_FRAMES as u64);
const DMA_RUN: NonZeroU64 = page_frame_run_len(DMA_PAGE_FRAMES);
const ONE_PAGE_RUN: NonZeroU64 = NonZeroU64::MIN;

/// The driver binary's escape-attempt role; must match `components/src/gpu_driver.rs` `ROLE_BACKING_ESCAPE`.
const ROLE_BACKING_ESCAPE: u64 = 1;

/// The budget, in pages, every program on this path draws its page tables from.
///
/// At 1120x624 the 683-page surface spans two 2 MiB windows (two L3 tables) and the terminal's
/// output page a third. Twenty-four pages has also booted a 900-page 1280x720 surface, and is under
/// one percent of the free pool. See notes/frames.md.
const MAP_BUDGET_PAGES: u64 = 24;

// The driver's capability table. Must match components/src/gpu_driver.rs.
const DRIVER_SLOT_REPORT: u64 = 0;
const DRIVER_SLOT_IRQ: u64 = 1;
const DRIVER_SLOT_VIRTIO: u64 = 2;
const DRIVER_SLOT_DISPLAY: u64 = 3;
const DRIVER_SLOT_BUDGET: u64 = 4;
/// The whole DMA region, one capability (§102).
const DRIVER_SLOT_DMA: u64 = 5;

// The painting client's capability table. Must match fixtures/src/painter.rs.
const CLIENT_SLOT_REPORT: u64 = 0;
const CLIENT_SLOT_DISPLAY: u64 = 1;
const CLIENT_SLOT_BUDGET: u64 = 2;
/// The whole scanout, one capability (§102).
const CLIENT_SLOT_SURFACE: u64 = 3;

// The display terminal's capability table. Must match components/src/display_terminal.rs.
const TERM_SLOT_REPORT: u64 = 0;
const TERM_SLOT_DISPLAY: u64 = 1;
const TERM_SLOT_TERM: u64 = 2;
const TERM_SLOT_BUDGET: u64 = 3;
/// The whole scanout, one capability (§102).
const TERM_SLOT_SURFACE: u64 = 4;
/// The page an application writes text into. Still its own single-page capability: it is not part
/// of the scanout's contiguous run.
const TERM_SLOT_OUT: u64 = TERM_SLOT_SURFACE + 1;

// No grant list on this path may reach the fault slot. The supervisor writes the process's fault
// endpoint at `abi::fault::FAULT_EP_SLOT`, and `grant_at` on an empty slot succeeds, so a grant
// there would overwrite it silently: the process would hang on a fault instead of being reaped.
// Each assertion is the highest slot one grant list uses.
const _: () = {
    assert!(DRIVER_SLOT_DMA < abi::fault::FAULT_EP_SLOT);
    assert!(CLIENT_SLOT_SURFACE < abi::fault::FAULT_EP_SLOT);
    assert!(TERM_SLOT_OUT < abi::fault::FAULT_EP_SLOT);
};

/// Grant one capability naming the `count`-frame run at `base`, read/write, at `slot`; the program
/// maps it with one `MAP` call (§102). Panics, naming `what`, if `slot` is occupied.
fn grant_run(slot: u64, base: u64, count: NonZeroU64, what: &str) {
    crate::sched::grant_at(
        slot,
        page_frame_run_cap(base, count, Rights::READ.union(Rights::WRITE)),
    )
    .unwrap_or_else(|_| panic!("{what}: slot {slot} was occupied"));
}

/// Wire and spawn the GPU driver and the painting client.
///
/// Returns `(driver report, client report)`, or `None` if no virtio-gpu function is on the bus.
/// The client holds a report endpoint, the display endpoint, a map budget and the surface, and
/// nothing else. The two share one spawn site because only it knows both halves of the endpoint
/// and the frames that join them.
pub fn start(
    driver_image: &'static [u8],
    client_image: &'static [u8],
) -> Option<(RendezvousId, RendezvousId)> {
    let (driver_report, display_ep, surface) = wire_driver(driver_image, 0, 0)?;

    let client_report = crate::sched::create_rendezvous();
    let budget =
        crate::memory_region::create(MAP_BUDGET_PAGES).expect("no map budget for the client");
    crate::sched::spawn(move || {
        crate::sched::grant_at(
            CLIENT_SLOT_REPORT,
            rendezvous_cap(client_report, Rights::WRITE),
        )
        .expect("client slot 0 was occupied");
        crate::sched::grant_at(
            CLIENT_SLOT_DISPLAY,
            rendezvous_cap(display_ep, Rights::WRITE),
        )
        .expect("client slot 1 was occupied");
        crate::sched::grant_at(CLIENT_SLOT_BUDGET, memory_region_cap(budget))
            .expect("client slot 2 was occupied");
        grant_run(
            CLIENT_SLOT_SURFACE,
            surface,
            SURFACE_RUN,
            "the painting client",
        );
        run(
            client_image,
            Spawn {
                arg0: 0,
                arg1: 0, // no physical address: a client has no business knowing one
                arg2: 0,
                grants: &[], // every one of them is placed above, at its own slot
                maps: &[],
            },
        )
    })
    .expect("could not spawn the painting client");

    Some((driver_report, client_report))
}

/// Spawn a driver that attacks its own confinement (`components/src/gpu_driver.rs`
/// `run_backing_escape`) by asking the device to read a frame outside its grant.
///
/// Returns `(report endpoint, the victim frame's physical address)`, or `None` if no GPU is on the
/// bus. The driver gets the honest driver's world exactly, and no client. The kernel picks the
/// victim frame, passes it in `arg2`, and never frees it, since a device has been told to read it.
/// See notes/framebuffer-contract/kernel-wiring.md.
pub fn start_backing_escape(driver_image: &'static [u8]) -> Option<(RendezvousId, u64)> {
    let victim = crate::memory::alloc()
        .expect("no victim frame for the backing-escape test")
        .addr();
    let (report, _, _) = wire_driver(driver_image, ROLE_BACKING_ESCAPE, victim)?;
    Some((report, victim))
}

/// A virtio-gpu, wired and confined but driven by nobody yet: what [`wire_device`] returns.
///
/// The transport is registered and confined to [`dma`](GpuDevice::dma)'s run, the interrupt routed
/// and enabled, and the region's physical base written into its first page at
/// `abi::virtio::DMA_PHYS_OFFSET`. [`wire_driver`] spawns a driver on it; the progenitor is granted
/// it as capabilities by `kernel::user::boot_progenitor` and builds `gpu_driver` and
/// `display_terminal` itself.
/// Name: provisional.
pub struct GpuDevice {
    /// The `Virtio` capability's id (`crate::virtio::register`'s return value).
    pub vid: usize,
    /// The completion interrupt, routed and enabled.
    pub intid: u32,
    /// The DMA region's physical base: [`DMA_PAGE_FRAMES`] contiguous frames, the rings and
    /// control buffers in the first and the surface after it.
    pub dma: u64,
    /// The surface's physical base, `dma` plus one frame: [`SURFACE_PAGE_FRAMES`] frames, the part
    /// of the region a client also maps.
    ///
    /// [`SURFACE_PAGE_FRAMES`]: graphics_contract::SURFACE_PAGE_FRAMES
    pub surface: u64,
}

/// The run lengths a holder of a [`GpuDevice`] grants, so `kernel::user::boot_progenitor` mints the
/// same two runs this file does rather than recomputing them.
pub(crate) const GPU_DMA_RUN: NonZeroU64 = DMA_RUN;
/// See [`GPU_DMA_RUN`].
pub(crate) const GPU_SURFACE_RUN: NonZeroU64 = SURFACE_RUN;

/// Find the GPU, build its DMA region, route its interrupt and register its confined transport,
/// spawning nothing. `None` if no virtio-gpu function is on the bus.
///
/// The region is one contiguous, zeroed run of [`DMA_PAGE_FRAMES`] frames, registered with
/// `crate::virtio` as one region and granted as one `PageFrame` capability (§102).
pub(crate) fn wire_device() -> Option<GpuDevice> {
    let d = crate::pci::find_gpu_device()?;

    // The DMA region: contiguous, because the surface must be one run of physical frames for the
    // device's backing to be a single memory entry and for the IOMMU domain to cover it as one
    // range. Zeroed, so neither a stale descriptor nor a stale pixel is ever visible to the
    // device or to the client.
    let dma = crate::memory::alloc_contiguous_zeroed(DMA_PAGE_FRAMES as usize)
        .expect("no contiguous DMA region for the GPU driver")
        .addr();
    // The region says where it is, in the tail of its first page, which `gpu_driver`'s rings and
    // control buffers end well short of (`abi::virtio::DMA_PHYS_OFFSET`). A driver the progenitor
    // builds has no other way to learn it: the progenitor holds the run as a capability and knows
    // no physical address either.
    super::write_dma_phys(dma);
    let surface = dma + FRAME_SIZE; // page 1 onward: the frames the client also maps

    // Routed to an endpoint, so the driver's WAIT receives the interrupt as a message.
    let irq_ep = crate::sched::create_rendezvous();
    crate::sched::bind_irq(d.intid, irq_ep);
    crate::arch::irq::enable(d.intid);

    // Register the transport: the kernel keeps the registers and the two DMA-critical powers,
    // and confines the device in hardware to exactly this region plus the shadow page.
    let vid = crate::virtio::register(
        crate::virtio::Transport::pci(&d),
        dma,
        DMA_PAGE_FRAMES * FRAME_SIZE,
        Some(d.rid), // the PCIe requester id the IOMMU keys its tables on
    );
    Some(GpuDevice {
        vid,
        intid: d.intid,
        dma,
        surface,
    })
}

/// The shared half of both spawns: [`wire_device`], then spawn `driver_image` at `role` with
/// `arg2`. Returns `(report endpoint, display endpoint, the surface's physical base)`.
fn wire_driver(
    driver_image: &'static [u8],
    role: u64,
    arg2: u64,
) -> Option<(RendezvousId, RendezvousId, u64)> {
    let GpuDevice {
        vid,
        intid,
        dma,
        surface,
    } = wire_device()?;

    let display_ep = crate::sched::create_rendezvous(); // client WRITE (CALL) -> driver READ
    let driver_report = crate::sched::create_rendezvous();

    let budget =
        crate::memory_region::create(MAP_BUDGET_PAGES).expect("no map budget for the driver");
    crate::sched::spawn(move || {
        crate::sched::grant_at(
            DRIVER_SLOT_REPORT,
            rendezvous_cap(driver_report, Rights::WRITE),
        )
        .expect("driver slot 0 was occupied");
        crate::sched::grant_at(DRIVER_SLOT_IRQ, irq_cap(intid))
            .expect("driver slot 1 was occupied");
        crate::sched::grant_at(DRIVER_SLOT_VIRTIO, virtio_cap(vid))
            .expect("driver slot 2 was occupied");
        crate::sched::grant_at(
            DRIVER_SLOT_DISPLAY,
            rendezvous_cap(display_ep, Rights::READ),
        )
        .expect("driver slot 3 was occupied");
        crate::sched::grant_at(DRIVER_SLOT_BUDGET, memory_region_cap(budget))
            .expect("driver slot 4 was occupied");
        grant_run(DRIVER_SLOT_DMA, dma, DMA_RUN, "the GPU driver");
        run(
            driver_image,
            Spawn {
                arg0: role, // 0 = the GPU driver; 1 = the escape attempt
                // No physical address: the region's first page carries it
                // (`abi::virtio::DMA_PHYS_OFFSET`), the same place the progenitor-built driver
                // reads it from, so the harness and the boot hand the driver one world.
                arg1: 0,
                arg2,        // the escape role's victim frame; unused (0) by the GPU driver
                grants: &[], // every one of them is placed above, at its own slot
                maps: &[],
            },
        )
    })
    .expect("could not spawn the GPU driver");

    Some((driver_report, display_ep, surface))
}

/// Wire and spawn the GPU driver alone, with no client, for the compositor to take `painter`'s
/// place: `(report endpoint, display endpoint, the surface's physical base)`, or `None` if no
/// virtio-gpu is on the bus.
///
/// The caller must receive `FLUSHED` on the report endpoint after the client's first flush, or
/// the driver never serves a second one; see [`start_terminal`].
pub fn start_driver(driver_image: &'static [u8]) -> Option<(RendezvousId, RendezvousId, u64)> {
    wire_driver(driver_image, 0, 0)
}

/// What the kernel keeps after wiring a display terminal onto the scanout.
pub struct TerminalWiring {
    /// The GPU driver's status endpoint.
    pub driver_report: RendezvousId,
    /// The terminal's status endpoint.
    pub term_report: RendezvousId,
    /// The endpoint the terminal serves. The kernel holds WRITE, so it can play **both** classes
    /// of sender: an application (`OPERATION_WRITE`) and an input source (`OPERATION_BYTES`).
    pub term: RendezvousId,
    /// The application's output page, so the kernel can put the bytes of an `OPERATION_WRITE` there.
    pub out: u64,
    /// The scanout frames, so the kernel can read the picture back through the direct map and
    /// grade it against a value it computed itself.
    pub surface: u64,
    /// How many frames [`Self::surface`] is, which a caller that frees them needs: the contract's
    /// [`graphics_contract::SURFACE_PAGE_FRAMES`] on the virtio path, and on a firmware screen
    /// whatever covers that screen (see [`start_screen_terminal`]).
    pub surface_frames: u64,
    /// What a caller hands back to end it: the threads, the region its endpoints live in, and the
    /// map budgets. Filled by [`start_screen_terminal`], whose endpoints come out of a region so a
    /// server parked in `RECEIVE` can be woken to die (`user::holding`'s BUGS). Empty on the virtio
    /// path, which no caller tears down. The surface and the output page are frames, not in it; a
    /// caller that releases this frees them after.
    pub held: super::holding::Holding,
}

/// Wire and spawn the GPU driver with a terminal on the whole scanout. `None` if no virtio-gpu
/// function is on the bus.
///
/// The terminal holds exactly `painter`'s authority (a report endpoint, the display endpoint, the
/// surface) plus the terminal contract's two additions: an endpoint it serves and a page an
/// application writes bytes into (§10).
///
/// The caller must receive three blocking reports, or the screen freezes after one frame: `UP` and
/// then `FLUSHED` on `driver_report`, and `TERM_UP` on `term_report`. The tests read `FLUSHED`'s
/// digest, so this does not swallow it. See notes/framebuffer-contract/kernel-wiring.md.
pub fn start_terminal(
    driver_image: &'static [u8],
    term_image: &'static [u8],
) -> Option<TerminalWiring> {
    // A scanout with no room for a character has nothing to show. It does **not** have to be a
    // whole number of them: a strip no cell covers is painted as background on the terminal's
    // first frame (see `components/src/display_terminal.rs`) and `Vt::pixel` answers for it,
    // which is what keeps the picture a total function of the state.
    const _: () = assert!(
        graphics_contract::WIDTH >= video_terminal::CELL_W
            && graphics_contract::HEIGHT >= video_terminal::CELL_H,
        "the scanout is too small for one character cell",
    );
    // And the script's geometry is the scanout's, checked here rather than trusted, because the
    // script is what three independent parties predict the picture from.
    const _: () = assert!(
        graphics_contract::WIDTH / video_terminal::CELL_W == video_terminal::script::COLS
            && graphics_contract::HEIGHT / video_terminal::CELL_H == video_terminal::script::ROWS,
        "video_terminal::script's geometry and the scanout's have drifted apart",
    );

    let (driver_report, display_ep, surface) = wire_driver(driver_image, 0, 0)?;
    let t = spawn_terminal(
        SURFACE_RUN,
        term_image,
        display_ep,
        surface,
        &crate::sched::create_rendezvous,
    );
    Some(TerminalWiring {
        driver_report,
        term_report: t.term_report,
        term: t.term,
        out: t.out,
        surface,
        surface_frames: graphics_contract::SURFACE_PAGE_FRAMES as u64,
        held: super::holding::Holding::new(),
    })
}

/// **`display_terminal` on a display endpoint and a surface somebody else already serves**: the
/// half of [`start_terminal`] that does not care which driver is on the other side, shared with
/// [`start_screen_terminal`] so the terminal's authority is written down once. Returns
/// `(its report endpoint, the endpoint it serves, its output page)`.
///
/// `endpoint` makes its two endpoints: the kernel's own chunks on the virtio path, a region the
/// caller can reclaim on the firmware-screen path (see [`start_screen_terminal`]). The terminal's
/// thread and its map budget come back beside them, for a caller that tears it down.
fn spawn_terminal(
    surface_run: NonZeroU64,
    term_image: &'static [u8],
    display_ep: RendezvousId,
    surface: u64,
    endpoint: &dyn Fn() -> RendezvousId,
) -> SpawnedTerminal {
    let out = crate::memory::alloc_zeroed()
        .expect("no output-page frame for the display terminal")
        .addr();

    let term_report = endpoint();
    let term = endpoint();
    let budget =
        crate::memory_region::create(MAP_BUDGET_PAGES).expect("no map budget for the terminal");

    let tid = crate::sched::spawn(move || {
        crate::sched::grant_at(TERM_SLOT_REPORT, rendezvous_cap(term_report, Rights::WRITE))
            .expect("terminal slot 0 was occupied");
        crate::sched::grant_at(TERM_SLOT_DISPLAY, rendezvous_cap(display_ep, Rights::WRITE))
            .expect("terminal slot 1 was occupied");
        crate::sched::grant_at(TERM_SLOT_TERM, rendezvous_cap(term, Rights::READ))
            .expect("terminal slot 2 was occupied");
        crate::sched::grant_at(TERM_SLOT_BUDGET, memory_region_cap(budget))
            .expect("terminal slot 3 was occupied");
        grant_run(
            TERM_SLOT_SURFACE,
            surface,
            surface_run,
            "the display terminal",
        );
        grant_run(TERM_SLOT_OUT, out, ONE_PAGE_RUN, "the display terminal");
        run(
            term_image,
            Spawn {
                arg0: video_terminal::status::MODE_DISPLAY,
                arg1: 0, // no physical address: a terminal has no business knowing one
                arg2: 0,
                grants: &[], // every one of them is placed above, at its own slot
                maps: &[],
            },
        )
    })
    .expect("could not spawn the display terminal");

    SpawnedTerminal {
        term_report,
        term,
        out,
        tid,
        budget,
    }
}

/// What [`spawn_terminal`] started, including what a teardown needs.
struct SpawnedTerminal {
    term_report: RendezvousId,
    term: RendezvousId,
    out: u64,
    tid: crate::thread::ThreadId,
    budget: u64,
}

/// Where `framebuffer_driver` finds the covered part of the aperture, a service window on the
/// address-space map. Must match `components/src/framebuffer_driver.rs`'s `APERTURE_VA`.
const SCREEN_APERTURE_VA: u64 = address_space_map::service_window(0x5800_0000);
const _: () = assert!(address_space_map::SERVICE_WINDOWS.holds(
    SCREEN_APERTURE_VA,
    SCREEN_APERTURE_VA + MAX_APERTURE_PAGES * page_frames::FRAME_SIZE
));

/// The most aperture pages this wiring maps: 8 MiB, four 2 MiB page-table windows.
///
/// The tables come out of the driver's own address-space budget (`AS_OVERHEAD`, sixteen pages shared
/// with its image and stack), so this is a bound. xenon's 1920x1080 is 2,025 pages; a larger screen
/// gets a terminal over the rows that fit (see [`start_screen_terminal`]).
const MAX_APERTURE_PAGES: u64 = 2048;

// The firmware screen driver's capability table. Must match components/src/framebuffer_driver.rs.
const SCREEN_SLOT_REPORT: u64 = 0;
const SCREEN_SLOT_DISPLAY: u64 = 1;
const SCREEN_SLOT_BUDGET: u64 = 2;
const SCREEN_SLOT_SURFACE: u64 = 3;
const _: () = assert!(SCREEN_SLOT_SURFACE < abi::fault::FAULT_EP_SLOT);

/// The part of `screen` the terminal covers, and the pages of it the driver maps:
/// [`start_screen_terminal`]'s first step. `None` when the screen cannot show a surface or its
/// covered rows would need more than [`MAX_APERTURE_PAGES`].
///
/// The surface is the screen divided by `screen_console::ScreenConsole::scale_for_cell`'s scale,
/// the boot console's rule, capped at what one terminal grid can lay out. See
/// notes/framebuffer-contract/kernel-wiring.md.
fn screen_terminal_aperture(
    screen: &machine_discovery::framebuffer::Framebuffer,
) -> Option<(screen_console::Aperture, u64)> {
    let mut aperture = screen_console::Aperture::new(
        screen,
        video_terminal::CELL_W,
        video_terminal::MAX_COLS as u32 * video_terminal::CELL_W,
        u32::MAX,
    )?;
    // The rows one `Vt` can hold at that width (`video_terminal::MAX_CELLS` is a cell budget, not
    // a rectangle): the whole screen on xenon and OVMF, the top rows on a taller one.
    let cols = (aperture.size().0 / video_terminal::CELL_W).max(1) as usize;
    let rows = (video_terminal::MAX_CELLS / cols).min(video_terminal::MAX_ROWS) as u32;
    aperture = screen_console::Aperture::new(
        screen,
        video_terminal::CELL_W,
        aperture.size().0,
        rows * video_terminal::CELL_H,
    )?;
    // The aperture need not start on a page: the run is mapped from the page that holds pixel
    // (0, 0), and the driver is told how far into it that pixel is.
    let offset = screen.base % FRAME_SIZE;
    let mut pages = (offset + aperture.span() as u64).div_ceil(FRAME_SIZE);
    if pages > MAX_APERTURE_PAGES {
        // A screen too large to map whole (past 8 MiB, so a 2560x1440 or a 4K panel) gets the
        // shell over the rows that fit rather than no shell at all. Each surface row is `scale`
        // screen rows of `stride` bytes.
        let row_bytes = screen.stride as u64 * aperture.scale() as u64;
        let fit = (MAX_APERTURE_PAGES * FRAME_SIZE - offset) / row_bytes;
        aperture = screen_console::Aperture::new(
            screen,
            video_terminal::CELL_W,
            aperture.size().0,
            fit as u32,
        )?;
        pages = (offset + aperture.span() as u64).div_ceil(FRAME_SIZE);
        crate::println!(
            "  screen    : too large to map whole; the terminal covers its top {} rows",
            aperture.size().1 * aperture.scale()
        );
    }
    if pages > MAX_APERTURE_PAGES {
        return None;
    }
    Some((aperture, pages))
}

/// The terminal on a screen the firmware already set up: `framebuffer_driver` serving the
/// framebuffer contract over `screen`, and `display_terminal` on it as [`start_terminal`] wires it.
/// `None`, with nothing spawned, when the screen cannot show a surface or its covered rows would
/// need more than [`MAX_APERTURE_PAGES`].
///
/// `screen` must already be off the kernel's console (`console::yield_screen`), or two painters
/// share it. The driver holds no interrupt, DMA, transport or physical address, and only the rows
/// its surface reaches, mapped at spawn. See notes/framebuffer-contract/kernel-wiring.md.
pub fn start_screen_terminal(
    driver_image: &'static [u8],
    term_image: &'static [u8],
    screen: machine_discovery::framebuffer::Framebuffer,
) -> Option<TerminalWiring> {
    let (aperture, pages) = screen_terminal_aperture(&screen)?;
    // Where pixel (0, 0) is in the first mapped page (see `screen_terminal_aperture`).
    let offset = screen.base % FRAME_SIZE;

    // RAM at four bytes a pixel and no padding: the terminal paints it at a stride of four times
    // the width `INFO` answers. No ring page, because the driver copies it with the CPU.
    let (sw, sh) = aperture.size();
    let surface_frames = (sw as u64 * sh as u64 * 4).div_ceil(FRAME_SIZE);
    let surface_run = NonZeroU64::new(surface_frames)?;
    let surface = crate::memory::alloc_contiguous_zeroed(surface_frames as usize)
        .expect("no contiguous surface for the framebuffer driver")
        .addr();

    // The endpoints come out of a region of their own: both programs park in `RECEIVE` for good,
    // and reclaiming that region is the only thing that wakes them to die (`user::holding`). One
    // page per endpoint, and one spare.
    let ep_region =
        crate::memory_region::create(5).expect("no endpoint region for the screen terminal");
    let endpoint = || {
        crate::sched::create_rendezvous_from(ep_region)
            .expect("no endpoint for the screen terminal")
    };
    let display_ep = endpoint(); // terminal WRITE (CALL) -> driver READ
    let driver_report = endpoint();
    let budget =
        crate::memory_region::create(MAP_BUDGET_PAGES).expect("no map budget for the driver");
    let (size, layout) = aperture.to_words();
    let device = DeviceRun {
        va: SCREEN_APERTURE_VA,
        phys: screen.base - offset,
        pages,
        // A framebuffer, written and never read (`Aperture::copy_wide` reads the surface, in
        // RAM): the mapping `paging::Flags::user_write_combining` exists for.
        write_combining: true,
    };
    let driver_tid = crate::sched::spawn(move || {
        crate::sched::grant_at(
            SCREEN_SLOT_REPORT,
            rendezvous_cap(driver_report, Rights::WRITE),
        )
        .expect("screen driver slot 0 was occupied");
        crate::sched::grant_at(
            SCREEN_SLOT_DISPLAY,
            rendezvous_cap(display_ep, Rights::READ),
        )
        .expect("screen driver slot 1 was occupied");
        crate::sched::grant_at(SCREEN_SLOT_BUDGET, memory_region_cap(budget))
            .expect("screen driver slot 2 was occupied");
        grant_run(
            SCREEN_SLOT_SURFACE,
            surface,
            surface_run,
            "the framebuffer driver",
        );
        run_with_device_run(
            driver_image,
            Spawn {
                arg0: size,
                arg1: layout,
                arg2: offset, // where pixel (0, 0) is in the first mapped page
                grants: &[],  // every one of them is placed above, at its own slot
                maps: &[],
            },
            device,
        )
    })
    .expect("could not spawn the framebuffer driver");

    let t = spawn_terminal(surface_run, term_image, display_ep, surface, &endpoint);
    let mut held = super::holding::Holding::new();
    held.add_thread(driver_tid);
    held.add_thread(t.tid);
    held.add_region(ep_region);
    held.add_region_after_death(budget);
    held.add_region_after_death(t.budget);
    Some(TerminalWiring {
        driver_report,
        term_report: t.term_report,
        term: t.term,
        out: t.out,
        surface,
        surface_frames,
        held,
    })
}

impl TerminalWiring {
    /// **Play the application**: put `text` in the output page and `OPERATION_WRITE` it.
    ///
    /// Returns when the terminal has drawn it and the GPU driver has put it on the scanout,
    /// because that is what the terminal contract says an `OPERATION_WRITE` reply means (the bytes are
    /// on the console's side). So a test needs no polling and no sleep between writes.
    pub fn print(&self, text: &[u8]) {
        super::term_print(self.out, self.term, text);
    }

    /// **Play the input driver**: `OPERATION_BYTES` these keystrokes, eight to a message.
    ///
    /// Byte for byte the framing `components/src/input.rs` sends and the compositor forwards
    /// (DECISIONS §33), which is the point: the display terminal is fed by the same driver half
    /// as the serial one, so neither contract had to grow anything to carry a keystroke to a
    /// screen.
    pub fn type_bytes(&self, bytes: &[u8]) {
        for chunk in bytes.chunks(8) {
            let mut w1 = 0u64;
            for (k, &b) in chunk.iter().enumerate() {
                w1 |= (b as u64) << (8 * k);
            }
            let w0 =
                line_editor::proto::req(line_editor::proto::OPERATION_BYTES, chunk.len() as u64);
            assert_eq!(
                crate::sched::ipc_call(self.term, [w0, w1])[0],
                0,
                "the terminal refused a keystroke",
            );
        }
    }

    /// A scanout pixel, read by the **kernel** through the direct map: a witness that belongs to
    /// no process in userspace.
    pub(crate) fn screen_pixel(&self, x: u32, y: u32) -> u32 {
        let at = mmu::phys_to_virt(self.surface) + (y * graphics_contract::WIDTH + x) as u64 * 4;
        // SAFETY: inside the scanout frames this kernel allocated.
        unsafe { core::ptr::read_volatile(at as *const u32) }
    }

    /// **The scanout holds exactly the picture `expect` describes.** Compared pixel for pixel
    /// rather than by digest, so a failure names a coordinate.
    pub fn assert_screen_is(&self, expect: &video_terminal::Vt, what: &str) {
        for y in 0..graphics_contract::HEIGHT {
            for x in 0..graphics_contract::WIDTH {
                let (got, want) = (self.screen_pixel(x, y), expect.pixel(x, y));
                assert_eq!(
                    got,
                    want,
                    "{what}: the framebuffer is wrong at ({x},{y}) [cell ({},{})]: {got:#010x}, \
                     expected {want:#010x}",
                    x / video_terminal::CELL_W,
                    y / video_terminal::CELL_H,
                );
            }
        }
    }
}
