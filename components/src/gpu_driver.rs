//! The GPU driver: virtio-gpu, at EL0, confined (milestone 29 (a display terminal: framebuffer,
//! virtio-gpu, and a foreign component), the display ladder's rung one).
//!
//! It owns the device and the transport and serves [`graphics_contract`] to a client that owns the
//! pixels and no device, so a hostile client can draw nonsense and nothing more. Its DMA region is
//! one `PageFrame` capability for one contiguous run (DECISIONS §102): page 0 holds the rings and
//! the control buffers, pages 1.. the surface the client also maps. The kernel registers the whole
//! run, so the shadow-ring validator and the IOMMU domain bound every device access to it. Fonts,
//! VT state, scrollback and input arrive as clients of the contract, never as code in here. See
//! notes/framebuffer-contract.md and notes/frames.md.

//! Name: ratified 2026-08-27 (calef). Renamed from `display` to `gpu_driver`, matching the
//! `<device>_driver` shape given to `block_driver` and `keyboard_driver` in the same pass. This
//! module's own doc already said it above: it is specifically and only the GPU device driver, and
//! does nothing without a GPU. The broader "manage what's on screen" role belongs to the separate
//! `display_terminal.rs` program, not this one, so `gpu_driver` is more accurate than `display`
//! was, not just more consistent. `display` itself was ratified 2026-07-30 (DECISIONS §39,
//! landed by milestone 46), replacing `gpud` (refused for the `-d` claim); this supersedes that
//! entry with the fuller, consistent name.

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

use graphics_contract as gfx;
use user_mode_runtime::mapped_window::MappedWindow;
use user_mode_runtime::virtio::virtio_ring_barrier;
use user_mode_runtime::{
    exit, interrupt_acknowledge, interrupt_wait, receive_request, reply, send,
};

/// Capability slots, by convention with `kernel/src/user/display_service.rs`. Slot 0, the report
/// endpoint: how this driver tells its spawner it came up, and what it flushed.
const REPORT: u64 = 0;
/// Slot 1, an `Irq`: the device's completion interrupt, arriving as a message.
const IRQ: u64 = 1;
/// Slot 2, a `Virtio`: the confined transport. The device's registers are not mapped here, so this
/// process cannot program a queue address or ring a doorbell; the kernel does both, and validates
/// every descriptor first (notes/dma.md).
const VIRTIO: u64 = 2;
/// Slot 3, the display endpoint this driver receives on: where clients ask for a flush.
const DISPLAY: u64 = 3;
/// Slot 4: the untyped this driver spends on the page tables its DMA region needs.
const BUDGET: u64 = 4;
/// Slot 5: the whole DMA region, one `PageFrame` capability naming the [`DMA_PAGE_FRAMES`]-page run
/// (DECISIONS §102), which this driver maps itself in one call (milestone 108 (the drivers move onto
/// frame capabilities)). One capability per page would be 684 at the 1120x624 scanout, which no
/// sixteen-slot capability table holds.
const DMA_FRAME: u64 = 5;

/// The DMA region, in frames: one for the rings and control buffers, then the surface. Must match
/// `display_service::DMA_PAGE_FRAMES`.
const DMA_PAGE_FRAMES: u64 = 1 + gfx::SURFACE_PAGE_FRAMES as u64;

/// Where this driver puts its DMA region. Its choice, not the kernel's: it holds the frames and maps
/// them (milestone 108). The physical base, which descriptors need, is read out of the region
/// itself (`abi::virtio::DMA_PHYS_OFFSET`; see `_start`).
///
/// **2 MiB-aligned** (milestone 142 (a text display good enough that people use it instead of a
/// GUI), DECISIONS §102 (a Frame names a run of pages)): at the 1120x624 scanout
/// [`DMA_PAGE_FRAMES`] is 684 and the region 2.7 MiB, which an aligned base keeps to two 2 MiB
/// page-table windows and an unaligned one could spread over three.
const DMA_VA: u64 = address_space_map::pair_page(0x0000_0000_0100_0000);

// --- virtio-mmio v2 register offsets. The PCI transport answers this same vocabulary
// (kernel/src/virtio.rs, the transport seam), so this driver reads like every other one here even
// though a virtio-gpu only ever arrives over PCIe on our boards. ---
const MAGIC: u64 = 0x000;
const DEVICE_ID: u64 = 0x008;
const DEVICE_FEATURES: u64 = 0x010;
const DEVICE_FEATURES_SEL: u64 = 0x014;
const DRIVER_FEATURES: u64 = 0x020;
const DRIVER_FEATURES_SEL: u64 = 0x024;
const INTERRUPT_STATUS: u64 = 0x060;
const INTERRUPT_ACK: u64 = 0x064;
const STATUS: u64 = 0x070;

const S_ACKNOWLEDGE: u32 = 1;
const S_DRIVER: u32 = 2;
const S_DRIVER_OK: u32 = 4;
const S_FEATURES_OK: u32 = 8;

/// `VIRTIO_F_VERSION_1` (feature bit 32) and `VIRTIO_F_ACCESS_PLATFORM` (bit 33), both in the high
/// feature word. `ACCESS_PLATFORM` is offered only when the device sits behind an IOMMU, and is
/// acked only when offered, exactly as the disk and net drivers do it: our DMA domain is an identity
/// map, so accepting it changes no address we compute, it only tells the device we know we are
/// translated.
const F_VERSION_1_HI: u32 = 1;
const F_ACCESS_PLATFORM_HI: u32 = 1 << 1;

/// The virtio device type a GPU reports. The kernel's PCI transport answers `DEVICE_ID` with the
/// type recovered from the PCI id, so this check means the same thing on either bus.
const VIRTIO_ID_GPU: u32 = 16;

/// The control virtqueue. A virtio-gpu also has a cursor queue (1); this driver has no cursor, so it
/// never sets that queue up, which keeps the confinement's two-queue ceiling untouched.
const CONTROL_Q: u64 = 0;

/// Our ring size, matching the kernel's `QSIZE`.
const QSIZE: usize = 8;

const VIRTQ_DESC_F_NEXT: u16 = 1;
const VIRTQ_DESC_F_WRITE: u16 = 2;

// --- The DMA region's layout. Page 0 is driver-private; the surface starts at page 1. ---
//
// The ring offsets are the kernel's contract (kernel/src/virtio.rs: queue q's rings sit at
// q * 0x200 + {0x000, 0x080, 0x100}); queue 0 is the only one we use. The request and response
// buffers sit above both queues' ring blocks, the same place the net driver puts its packet buffers.
const OFF_DESC: u64 = 0x000;
const OFF_AVAIL: u64 = 0x080;
const OFF_USED: u64 = 0x100;
/// The control request the device reads. The largest command we send is 56 bytes.
const OFF_REQ: u64 = 0x400;
/// The control response the device writes. `GET_DISPLAY_INFO`'s reply is 408 bytes (a header plus 16
/// scanout descriptions), which is what sizes this and why it is the last thing in page 0.
const OFF_RESP: u64 = 0x600;
/// The surface: page 1 onward, `gfx::SURFACE_PAGE_FRAMES` frames of it, shared with the client.
const OFF_SURFACE: u64 = 0x1000;

// --- virtio-gpu control commands and responses (virtio spec 5.7). ---
const CMD_GET_DISPLAY_INFO: u32 = 0x0100;
const CMD_RESOURCE_CREATE_2D: u32 = 0x0101;
const CMD_SET_SCANOUT: u32 = 0x0103;
const CMD_RESOURCE_FLUSH: u32 = 0x0104;
const CMD_TRANSFER_TO_HOST_2D: u32 = 0x0105;
const CMD_RESOURCE_ATTACH_BACKING: u32 = 0x0106;
const RESP_OK_NODATA: u32 = 0x1100;
const RESP_OK_DISPLAY_INFO: u32 = 0x1101;

/// A control header is 24 bytes: type, flags, fence id, context id, ring index, padding.
const HDR_LEN: u64 = 24;

/// The one resource this driver creates. Rung two will want several (one per client surface); the
/// contract already routes by endpoint, so that is a driver change and not a contract change.
const RESOURCE_ID: u32 = 1;
/// The scanout we drive. QEMU's virtio-gpu offers one output by default.
const SCANOUT_ID: u32 = 0;

/// The entry role that attacks its own confinement instead of serving (see [`run_backing_escape`]).
/// Must match `ROLE_BACKING_ESCAPE` in `kernel/src/user/display_service.rs`.
const ROLE_BACKING_ESCAPE: u64 = 1;

/// Failure codes reported to the spawner, so a bring-up failure names its step instead of hanging.
/// Each is OR'd into a `0xDEAD_...` word the kernel test prints.
const E_NOT_VIRTIO: u64 = 0x01;
const E_NOT_GPU: u64 = 0x02;
const E_FEATURES: u64 = 0x03;
const E_QUEUE: u64 = 0x04;
const E_DISPLAY_INFO: u64 = 0x05;
const E_CREATE_2D: u64 = 0x06;
const E_ATTACH_BACKING: u64 = 0x07;
const E_SET_SCANOUT: u64 = 0x08;
const E_DISPLAY_TOO_SMALL: u64 = 0x09;
const E_DMA_REFUSED: u64 = 0x0A;
const E_NO_COMPLETION: u64 = 0x0B;

fn mr(off: u64) -> u32 {
    user_mode_runtime::virtio::read_register(VIRTIO, off) as u32
}

fn mw(off: u64, v: u32) {
    // Only DMA-*safe* registers reach the device; the queue-address and notify registers are
    // refused by the kernel and go through SETUP_QUEUE / NOTIFY instead.
    user_mode_runtime::virtio::write_register(VIRTIO, off, v as u64);
}

// SAFETY: the `MAP` loop in `_start` maps `DMA_PAGE_FRAMES` frames read/write at `DMA_VA` before any
// command is written or the surface digested (milestone 139 round 4: this is the shared invariant
// `direct_memory_access_write`/`direct_memory_access_read` used to assert by hand at every call
// site, now checked once here and enforced per access rather than trusted at each of the few dozen
// ring/request/response/surface offsets below, `surface_pixel` included). Constructing a window
// touches no memory.
const WINDOW: MappedWindow = unsafe { MappedWindow::new(DMA_VA, DMA_PAGE_FRAMES * 4096) };

fn direct_memory_access_write<T>(off: u64, val: T) {
    WINDOW.write(off, val);
}

fn direct_memory_access_read<T: Copy>(off: u64) -> T {
    WINDOW.read(off)
}

/// Report a bring-up failure and stop. Distinct from every success word, so the kernel test prints
/// the step that failed rather than timing out on a silent driver.
fn die(code: u64) -> ! {
    send(REPORT, 0xDEAD_0000_0000_0000 | code, 0, 0);
    // Exit so the kernel reaps this thread instead of leaving it on a run queue forever; a leaked
    // spinner starves later tests (notes/supervision.md).
    exit();
}

/// Write one 16-byte descriptor into queue 0's table: `{ u64 addr; u32 len; u16 flags; u16 next }`.
fn write_desc(i: u64, addr: u64, len: u32, flags: u16, next: u16) {
    let b = OFF_DESC + i * 16;
    direct_memory_access_write::<u64>(b, addr);
    direct_memory_access_write::<u32>(b + 8, len);
    direct_memory_access_write::<u16>(b + 12, flags);
    direct_memory_access_write::<u16>(b + 14, next);
}

/// Write a control header at `OFF_REQ`: the command type, and zeros for everything else. We use no
/// fences (the used ring is our completion) and no 3D contexts, so every other field is zero.
fn write_hdr(cmd: u32) {
    direct_memory_access_write::<u32>(OFF_REQ, cmd);
    direct_memory_access_write::<u32>(OFF_REQ + 4, 0); // flags: no VIRTIO_GPU_FLAG_FENCE
    direct_memory_access_write::<u64>(OFF_REQ + 8, 0); // fence_id
    direct_memory_access_write::<u32>(OFF_REQ + 16, 0); // ctx_id
    direct_memory_access_write::<u32>(OFF_REQ + 20, 0); // ring_idx + padding
}

/// Write a `virtio_gpu_rect` at `OFF_REQ + at`.
fn write_rect(at: u64, x: u32, y: u32, w: u32, h: u32) {
    direct_memory_access_write::<u32>(OFF_REQ + at, x);
    direct_memory_access_write::<u32>(OFF_REQ + at + 4, y);
    direct_memory_access_write::<u32>(OFF_REQ + at + 8, w);
    direct_memory_access_write::<u32>(OFF_REQ + at + 12, h);
}

/// **Submit the control command sitting at `OFF_REQ` and wait for the device's reply.** Returns the
/// response type the device wrote into `OFF_RESP`.
///
/// Two descriptors: the request, which the device reads, and the response buffer, which it writes.
/// Both are inside our DMA region, so the kernel's validator passes them; a bug that aimed either
/// outside would be refused before the device was ever rung, which is why `E_DMA_REFUSED` is a
/// driver bug and not a device error.
fn submit(direct_memory_access_phys: u64, req_len: u32) -> u32 {
    // Zero the response type first: a stale OK from the previous command must not be readable as
    // this command's answer.
    direct_memory_access_write::<u32>(OFF_RESP, 0);

    write_desc(
        0,
        direct_memory_access_phys + OFF_REQ,
        req_len,
        VIRTQ_DESC_F_NEXT,
        1,
    );
    write_desc(
        1,
        direct_memory_access_phys + OFF_RESP,
        512,
        VIRTQ_DESC_F_WRITE,
        0,
    );

    let used_before: u16 = direct_memory_access_read::<u16>(OFF_USED + 2);
    let idx: u16 = direct_memory_access_read::<u16>(OFF_AVAIL + 2);
    direct_memory_access_write::<u16>(OFF_AVAIL + 4 + (idx as u64 % QSIZE as u64) * 2, 0); // ring[idx] = head 0
    virtio_ring_barrier(); // the descriptors and the ring slot must be visible before idx moves
    direct_memory_access_write::<u16>(OFF_AVAIL + 2, idx.wrapping_add(1));
    virtio_ring_barrier(); // idx must be visible before the kernel rings the device

    // The kernel validates both descriptors against our DMA region, copies them into the shadow
    // ring the device actually reads, and only then rings the doorbell.
    if user_mode_runtime::virtio::notify(VIRTIO, CONTROL_Q) < 0 {
        die(E_DMA_REFUSED);
    }

    // **The completion is the used ring advancing, not the wakeup.** The device's interrupt line is
    // shared across every driver that ever holds this device, and a wakeup can be spurious,
    // coalesced, or a previous operator's leftover, so the used index is what decides (notes/dma.md,
    // the kill-mid-write section). The bound turns "no completion ever" into a legible failure
    // instead of a hang the watchdog has to catch.
    for _ in 0..64 {
        interrupt_wait(IRQ); // blocks until the device raises its line
        let istatus = mr(INTERRUPT_STATUS);
        mw(INTERRUPT_ACK, istatus);
        interrupt_acknowledge(IRQ); // re-enable the line the kernel masked when it fired
        virtio_ring_barrier();
        if direct_memory_access_read::<u16>(OFF_USED + 2) != used_before {
            return direct_memory_access_read::<u32>(OFF_RESP);
        }
    }
    die(E_NO_COMPLETION)
}

/// The virtio handshake, then queue 0. The queue is set up **through the kernel**, which programs
/// the ring addresses to fixed offsets in our region; we never choose them, which is what keeps the
/// device inside our grant.
fn init(direct_memory_access_phys: u64) -> (u32, u32) {
    if mr(MAGIC) != 0x7472_6976 {
        die(E_NOT_VIRTIO); // not "virt": we are not talking to a virtio device at all
    }
    if mr(DEVICE_ID) != VIRTIO_ID_GPU {
        die(E_NOT_GPU); // the kernel handed us something that is not a GPU
    }

    mw(STATUS, 0);
    mw(STATUS, S_ACKNOWLEDGE);
    mw(STATUS, S_ACKNOWLEDGE | S_DRIVER);

    // No low-word gpu features: not VIRGL (no 3D, that is milestone 34), not EDID, not
    // RESOURCE_BLOB. A 2D scanout out of guest pages is the whole of rung one.
    mw(DRIVER_FEATURES_SEL, 0);
    mw(DRIVER_FEATURES, 0);

    mw(DEVICE_FEATURES_SEL, 1);
    let dev_hi = mr(DEVICE_FEATURES);
    let mut ack_hi = F_VERSION_1_HI;
    if dev_hi & F_ACCESS_PLATFORM_HI != 0 {
        ack_hi |= F_ACCESS_PLATFORM_HI; // required when the device is behind the IOMMU
    }
    mw(DRIVER_FEATURES_SEL, 1);
    mw(DRIVER_FEATURES, ack_hi);

    mw(STATUS, S_ACKNOWLEDGE | S_DRIVER | S_FEATURES_OK);
    if mr(STATUS) & S_FEATURES_OK == 0 {
        die(E_FEATURES);
    }

    // The kernel places queue 0's rings at the contract's offsets.
    if user_mode_runtime::virtio::setup_queue(VIRTIO, QSIZE as u64, CONTROL_Q) != 0 {
        die(E_QUEUE);
    }

    mw(
        STATUS,
        S_ACKNOWLEDGE | S_DRIVER | S_FEATURES_OK | S_DRIVER_OK,
    );

    // Ask the device how big the display is. We do not have to obey it (SET_SCANOUT is bounded by
    // the resource, not the display), but a surface larger than the panel means the scanout would be
    // clipped somewhere we cannot see, so refuse loudly rather than draw into the dark.
    write_hdr(CMD_GET_DISPLAY_INFO);
    if submit(direct_memory_access_phys, HDR_LEN as u32) != RESP_OK_DISPLAY_INFO {
        die(E_DISPLAY_INFO);
    }
    // The reply is a header then 16 `virtio_gpu_display_one`, each a rect plus enabled and flags.
    // Scanout 0's rect starts right after the header.
    let dw: u32 = direct_memory_access_read::<u32>(OFF_RESP + HDR_LEN + 8);
    let dh: u32 = direct_memory_access_read::<u32>(OFF_RESP + HDR_LEN + 12);
    if dw < gfx::WIDTH || dh < gfx::HEIGHT {
        die(E_DISPLAY_TOO_SMALL);
    }
    (dw, dh)
}

/// Create the 2D resource, point it at the surface frames, and put it on the scanout. After this the
/// device holds a mapping of exactly the frames the client writes, and nothing else.
fn bring_up_surface(direct_memory_access_phys: u64) {
    // RESOURCE_CREATE_2D: a host-side image of our geometry and format.
    write_hdr(CMD_RESOURCE_CREATE_2D);
    direct_memory_access_write::<u32>(OFF_REQ + HDR_LEN, RESOURCE_ID);
    direct_memory_access_write::<u32>(OFF_REQ + HDR_LEN + 4, gfx::FORMAT);
    direct_memory_access_write::<u32>(OFF_REQ + HDR_LEN + 8, gfx::WIDTH);
    direct_memory_access_write::<u32>(OFF_REQ + HDR_LEN + 12, gfx::HEIGHT);
    if submit(direct_memory_access_phys, (HDR_LEN + 16) as u32) != RESP_OK_NODATA {
        die(E_CREATE_2D);
    }

    // RESOURCE_ATTACH_BACKING: the guest memory the device reads pixels out of. **One entry**,
    // because the surface is a contiguous run of frames inside our DMA region, which is what makes
    // the IOMMU domain cover it: the domain is exactly this region plus the kernel's shadow page, so
    // an entry pointing anywhere else would fail to translate and the device would refuse the
    // command. See notes/framebuffer-contract.md.
    write_hdr(CMD_RESOURCE_ATTACH_BACKING);
    direct_memory_access_write::<u32>(OFF_REQ + HDR_LEN, RESOURCE_ID);
    direct_memory_access_write::<u32>(OFF_REQ + HDR_LEN + 4, 1); // nr_entries
    direct_memory_access_write::<u64>(
        OFF_REQ + HDR_LEN + 8,
        direct_memory_access_phys + OFF_SURFACE,
    ); // entry addr
    direct_memory_access_write::<u32>(OFF_REQ + HDR_LEN + 16, gfx::SURFACE_BYTES); // entry length
    direct_memory_access_write::<u32>(OFF_REQ + HDR_LEN + 20, 0); // entry padding
    if submit(direct_memory_access_phys, (HDR_LEN + 24) as u32) != RESP_OK_NODATA {
        die(E_ATTACH_BACKING);
    }

    // SET_SCANOUT: this resource, whole, is what the display shows.
    write_hdr(CMD_SET_SCANOUT);
    write_rect(HDR_LEN, 0, 0, gfx::WIDTH, gfx::HEIGHT);
    direct_memory_access_write::<u32>(OFF_REQ + HDR_LEN + 16, SCANOUT_ID);
    direct_memory_access_write::<u32>(OFF_REQ + HDR_LEN + 20, RESOURCE_ID);
    if submit(direct_memory_access_phys, (HDR_LEN + 24) as u32) != RESP_OK_NODATA {
        die(E_SET_SCANOUT);
    }
}

/// Serve one flush: copy the damaged rectangle from the surface into the host resource, then put
/// that resource on the screen. Returns 0, or the contract's `EINVAL` for a rectangle that is not
/// wholly inside the surface.
///
/// Two device commands, in this order and for this reason: `TRANSFER_TO_HOST_2D` is what actually
/// reads our pixels (the device DMAs them out of the backing), and `RESOURCE_FLUSH` is what makes
/// the host show the result. A driver that only flushed would put stale pixels on the screen.
fn flush(direct_memory_access_phys: u64, x: u32, y: u32, w: u32, h: u32) -> i64 {
    if !gfx::rect_in_surface(x, y, w, h) {
        return gfx::EINVAL;
    }

    write_hdr(CMD_TRANSFER_TO_HOST_2D);
    write_rect(HDR_LEN, x, y, w, h);
    // The offset of the rectangle's first pixel in the backing. The device walks rows of `stride`
    // from here, so this is the one place the surface's stride reaches the device.
    direct_memory_access_write::<u64>(OFF_REQ + HDR_LEN + 16, gfx::offset_of(x, y) as u64);
    direct_memory_access_write::<u32>(OFF_REQ + HDR_LEN + 24, RESOURCE_ID);
    direct_memory_access_write::<u32>(OFF_REQ + HDR_LEN + 28, 0); // padding
    if submit(direct_memory_access_phys, (HDR_LEN + 32) as u32) != RESP_OK_NODATA {
        return gfx::EINVAL;
    }

    write_hdr(CMD_RESOURCE_FLUSH);
    write_rect(HDR_LEN, x, y, w, h);
    direct_memory_access_write::<u32>(OFF_REQ + HDR_LEN + 16, RESOURCE_ID);
    direct_memory_access_write::<u32>(OFF_REQ + HDR_LEN + 20, 0); // padding
    if submit(direct_memory_access_phys, (HDR_LEN + 24) as u32) != RESP_OK_NODATA {
        return gfx::EINVAL;
    }
    0
}

/// Read pixel `i` of the surface, through our own mapping of the shared frames.
fn surface_pixel(i: usize) -> u32 {
    direct_memory_access_read::<u32>(OFF_SURFACE + (i * 4) as u64)
}

/// Attack this driver's own confinement: bring the device up honestly, then attach `victim`, a
/// frame the kernel left out of this device's IOMMU domain, as the resource's backing, and report
/// the device's response code. `direct_memory_access_phys` is the region's physical base.
///
/// A virtio-gpu's backing addresses ride in a command payload the kernel does not parse (§18 (the
/// PCIe transport: the seam in the kernel) keeps device knowledge out of the transport), so the
/// IOMMU is the barrier, checked by the kernel test from its fault queue: QEMU answers a translation
/// failure with a bounce buffer, so the response code is not the evidence. `victim` comes from the
/// kernel because the test must know the exact address to look for. See
/// notes/framebuffer-contract/iommu-escape-test.md.
fn run_backing_escape(direct_memory_access_phys: u64, victim: u64) -> ! {
    init(direct_memory_access_phys);

    write_hdr(CMD_RESOURCE_CREATE_2D);
    direct_memory_access_write::<u32>(OFF_REQ + HDR_LEN, RESOURCE_ID);
    direct_memory_access_write::<u32>(OFF_REQ + HDR_LEN + 4, gfx::FORMAT);
    direct_memory_access_write::<u32>(OFF_REQ + HDR_LEN + 8, gfx::WIDTH);
    direct_memory_access_write::<u32>(OFF_REQ + HDR_LEN + 12, gfx::HEIGHT);
    if submit(direct_memory_access_phys, (HDR_LEN + 16) as u32) != RESP_OK_NODATA {
        die(E_CREATE_2D);
    }

    // Attaching is itself the access the IOMMU must refuse: the device maps the backing here. Four
    // bytes, not a frame: the RISC-V IOMMU's 128-record fault queue latches its overflow bit and
    // then records nothing, so one translation and one fault ask the same question without a flood
    // (notes/framebuffer-contract/iommu-escape-test.md).
    write_hdr(CMD_RESOURCE_ATTACH_BACKING);
    direct_memory_access_write::<u32>(OFF_REQ + HDR_LEN, RESOURCE_ID);
    direct_memory_access_write::<u32>(OFF_REQ + HDR_LEN + 4, 1); // nr_entries
    direct_memory_access_write::<u64>(OFF_REQ + HDR_LEN + 8, victim); // OUTSIDE the grant
    direct_memory_access_write::<u32>(OFF_REQ + HDR_LEN + 16, 4); // one pixel: see above
    direct_memory_access_write::<u32>(OFF_REQ + HDR_LEN + 20, 0);
    let resp = submit(direct_memory_access_phys, (HDR_LEN + 24) as u32);

    // Report what the device said about the attach. The kernel test decides whether the barrier held,
    // from the IOMMU's fault queue; this role does not get to grade its own attack.
    send(REPORT, gfx::status::BACKING, resp as u64, 0);
    exit();
}

#[unsafe(no_mangle)]
pub extern "C" fn _start(role: u64, _arg1: u64, arg2: u64) -> ! {
    // **The DMA region is ours to place** (milestone 108): one `PageFrame` capability naming the
    // whole [`DMA_PAGE_FRAMES`]-page run (DECISIONS §102), mapped read/write out of our own budget
    // in one `MAP` call. Before either role, because the rings live in the first page of it and the
    // escape attempt writes a descriptor too.
    if !user_mode_runtime::map_page_frame(DMA_FRAME, DMA_VA, true, BUDGET) {
        // Nothing to report: `gfx::status` has no code for "I never reached my own rings", and
        // inventing one would be a protocol change to say what the missing `UP` already says. A
        // spawner that never sees `UP` knows bring-up failed.
        exit();
    }
    // Where the region is, read out of the region (milestone 600 (provisional)): descriptors speak
    // physical addresses, and the progenitor that builds this driver holds the run as a capability
    // and knows none, so whoever built this process wrote the base into the tail of the first page
    // (`abi::virtio::DMA_PHYS_OFFSET`, past `OFF_RESP`'s 408 bytes).
    // SAFETY: the whole run was just mapped read/write at `DMA_VA`, and the offset is inside its
    // first page.
    let direct_memory_access_phys =
        unsafe { core::ptr::read_volatile((DMA_VA + abi::virtio::DMA_PHYS_OFFSET) as *const u64) };

    // Two roles in one binary, for the reason the blk attackers ride with the blk driver: the attack
    // differs from the honest driver by one field, and sharing the bring-up is what makes it a fair
    // test rather than a different program that fails for its own reasons.
    if role == ROLE_BACKING_ESCAPE {
        run_backing_escape(direct_memory_access_phys, arg2);
    }

    let (dw, dh) = init(direct_memory_access_phys);
    bring_up_surface(direct_memory_access_phys);

    // The display is up: resource created, backed, and on the scanout. A spawner that never sees
    // this knows bring-up failed rather than drawing.
    send(
        REPORT,
        gfx::status::UP,
        gfx::WIDTH as u64 | ((gfx::HEIGHT as u64) << 32),
        dw as u64 | ((dh as u64) << 32),
    );

    // Serve the contract. One client today; the loop answers whoever holds the WRITE half of the
    // display endpoint, through the kernel-minted one-shot Reply, so it never names its caller.
    let mut reported_flush = false;
    loop {
        // The opcode and the damage rectangle both ride in `w0` (`gfx::req` packs the rectangle
        // into its low 56 bits); this contract does not use `w1`. A client's SEND_CAP is deleted
        // rather than answered into (milestone 706 (a CALL server can tell a Reply from a
        // delegation)).
        let req = receive_request(DISPLAY);
        let w0 = req.w0;
        let reply_slot = req.delivered.into_reply();
        let r0: i64 = match gfx::operation(w0) {
            gfx::display::INFO => {
                // The runtime half of the geometry contract. The reply's second word carries it, so
                // a client need not have been compiled against our constants.
                0
            }
            gfx::display::FLUSH => {
                let (x, y, w, h) = gfx::unrect(gfx::operand(w0));
                flush(direct_memory_access_phys, x, y, w, h)
            }
            _ => gfx::EINVAL,
        };
        let r1 = match gfx::operation(w0) {
            gfx::display::INFO => gfx::WIDTH as u64 | ((gfx::HEIGHT as u64) << 32),
            _ => 0,
        };
        if let Some(to) = reply_slot {
            reply(to, r0 as u64, r1);
        }

        // The driver-side witness, once, after the first successful flush; see
        // notes/framebuffer-contract.md. This `send` blocks until received and nothing receives on
        // `DISPLAY` meanwhile, so a spawner that never takes it stops every flush after the first
        // (milestone 177 (wire the graphical terminal stack into the real interactive boot)'s hang).
        if !reported_flush && gfx::operation(w0) == gfx::display::FLUSH && r0 == 0 {
            reported_flush = true;
            let digest = gfx::checksum(surface_pixel);
            send(REPORT, gfx::status::FLUSHED, digest, gfx::PIXELS as u64);
        }
    }
}

user_mode_runtime::panic_handler!();
