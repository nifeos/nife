//! **The framebuffer driver: a screen the firmware already set up, served behind the framebuffer
//! contract** (the shell on the firmware screen; milestone 198's rung 1b).
//!
//! It serves [`graphics_contract`], as `gpu_driver` does over virtio-gpu, over the linear
//! framebuffer UEFI's `EFI_GRAPHICS_OUTPUT_PROTOCOL` leaves running past `ExitBootServices` (on the
//! boards, U-Boot's `simple-framebuffer`), so `display_terminal` cannot tell the two apart. Its
//! authority is the slot constants below and the covered aperture at [`APERTURE_VA`]; no
//! interrupt, DMA, transport or physical address. A flush copies the changed rectangle from the
//! surface into the aperture as a scale-by-scale block (`screen_console::Aperture::copy_wide`,
//! host-tested), and never reads the aperture.

//! # BUGS
//!
//! - **A full-screen flush is the whole screen.** A scroll (whole-surface damage) rewrites every
//!   visible pixel, 8 MB on a 1080p monitor. Unmeasured on the swish leg; CI's timing is the record.
//! - **Every pixel crosses a device mapping**, write-combining on `x86_64` (the kernel's PAT entry,
//!   `paging::Flags::user_write_combining`), as qword stores: a `u64` is the widest store the
//!   `x86_64` target can legalize (`-mmx,-sse,+soft-float`). A scroll-aware flush contract is the
//!   remaining lever, outside this driver. Not measured on silicon.
//! - **One client, no arbitration**: whoever holds the display endpoint draws; the compositor muxes.
//!
//! Name: ratified 2026-10-06 (calef, in conversation), in the `<device>_driver` shape `gpu_driver`,
//! `block_driver` and `keyboard_driver` take, the device being a linear framebuffer. Considered
//! `screen_driver` (every display driver drives a screen) and `firmware_screen_driver` (names who
//! set it up, and would be wrong on the boards, where U-Boot does).

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

use graphics_contract as gfx;
use screen_console::Aperture;
use user_mode_runtime::mapped_window::MappedWindow;
use user_mode_runtime::{exit, receive_request, reply, send};

/// Capability slots, by convention with `kernel/src/user/display_service.rs`. Slot 0, a report
/// endpoint (`WRITE`): one `UP` message once it is serving.
const REPORT: u64 = 0;
/// Slot 1, the display endpoint (`READ`): where its one client `CALL`s `INFO` and `FLUSH`.
const DISPLAY: u64 = 1;
/// Slot 2, an untyped: the budget its surface mapping's page tables come out of.
const BUDGET: u64 = 2;
/// The surface, one `PageFrame` capability naming the whole run (DECISIONS §102): RAM frames the
/// size of the covered aperture, shared with the client, which maps it itself.
const SURFACE_FRAME: u64 = 3;

/// Where this driver maps the surface. Its choice: it holds the frames. 2 MiB-aligned for the
/// reason `gpu_driver`'s `DMA_VA` gives, so the run spans as few page-table windows as it can.
const SURFACE_VA: u64 = address_space_map::pair_page(0x0000_0000_0100_0000);

/// Where the kernel maps the covered part of the aperture before `_start`. **Must match
/// `kernel/src/user/display_service.rs`'s `SCREEN_APERTURE_VA`.** A service window on the
/// address-space map (milestone 206 (a program image has under 896 KiB)), 8 MiB of it at most, and
/// 2 MiB-aligned for the same reason as above. Device-typed (write-combining on `x86_64`), it
/// covers the rows the surface can reach (`screen_console::Aperture::span`), the whole screen
/// unless that exceeds `display_service::MAX_APERTURE_PAGES`. It is a spawn-time mapping, not a
/// capability, so this driver cannot map the screen again, delegate it, or hand it on. The
/// geometry arrives in the three argument registers (`screen_console::Aperture::to_words`, then
/// the aperture's offset into its first page).
const APERTURE_VA: u64 = address_space_map::service_window(0x5800_0000);

/// Failure codes, sent on the report endpoint in a `0xDEAD_...` word so a failure names its step.
const E_GEOMETRY: u64 = 0x01;
const E_SURFACE: u64 = 0x02;

fn die(code: u64) -> ! {
    send(REPORT, 0xDEAD_0000_0000_0000 | code, 0, 0);
    exit();
}

#[unsafe(no_mangle)]
pub extern "C" fn _start(size: u64, layout: u64, offset: u64) -> ! {
    // The geometry arrived in registers from the kernel; it is checked here rather than believed,
    // because a geometry that does not close would have this process storing past its own mapping.
    let Some(aperture) = Aperture::from_words(size, layout) else {
        die(E_GEOMETRY)
    };
    let (width, height) = aperture.size();
    if offset >= 4096 {
        die(E_GEOMETRY);
    }
    // **The surface is the covered part's size, exactly**: the kernel allocated
    // `width * height` pixels at a stride of `width`, and a flush reads only inside that, because
    // `copy_wide` refuses a rectangle that leaves the covered part.
    let surface_bytes = width as u64 * height as u64 * 4;

    if !user_mode_runtime::map_page_frame(SURFACE_FRAME, SURFACE_VA, true, BUDGET) {
        die(E_SURFACE);
    }
    // SAFETY: the surface run is `surface_bytes` rounded up to whole frames
    // (`display_service::start_screen_terminal` sizes it from the same aperture), mapped read/write
    // at SURFACE_VA by the call just above.
    let surface = unsafe { MappedWindow::new(SURFACE_VA, surface_bytes) };
    // SAFETY: the kernel mapped every page from APERTURE_VA through `offset + span` device-typed
    // (write-combining on `x86_64`) and writable before this program's first instruction
    // (`display_service::start_screen_terminal`), and the geometry that span was computed from is
    // the one `from_words` just validated.
    let screen = unsafe { MappedWindow::new(APERTURE_VA + offset, aperture.span() as u64) };

    send(
        REPORT,
        gfx::status::UP,
        width as u64 | ((height as u64) << 32),
        1,
    );

    loop {
        // A CALL's Reply, or nothing to answer (milestone 706 (a `CALL` server can tell a Reply
        // from a delegation)): a client's SEND_CAP is deleted rather than answered into.
        let req = receive_request(DISPLAY);
        let w0 = req.w0;
        let reply_slot = req.delivered.into_reply();
        let (r0, r1): (i64, u64) = match gfx::operation(w0) {
            // The runtime half of the geometry contract: the surface's size in its own pixels,
            // which is what a client lays its grid out over and paints at four bytes a pixel and
            // no padding. On a scale-two screen it is half the screen in each direction; the
            // scaling is this driver's, and the client never sees it.
            gfx::display::INFO => (0, width as u64 | ((height as u64) << 32)),
            gfx::display::FLUSH => {
                let (x, y, w, h) = gfx::unrect(gfx::operand(w0));
                // The wide copy: qword stores where the aperture's own alignment permits them,
                // word stores at the edges. `offset` is the aperture's distance into its first
                // page from an 8-aligned base, so it is exactly the misalignment `copy_wide`
                // wants. This halves the store count into the uncacheable aperture (the module's
                // BUGS, and `screen_console::Aperture::copy_wide`'s doc for the numbers).
                let copied = aperture.copy_wide(
                    x,
                    y,
                    w,
                    h,
                    (offset % 8) as u32,
                    |px, py| surface.r32((py as u64 * width as u64 + px as u64) * 4),
                    |at, pair| screen.w64(at as u64, pair),
                    |at, word| screen.w32(at as u64, word),
                );
                // The aperture is write-combining on `x86_64`, so the last stores of this flush
                // may sit in a combining buffer. A sequentially consistent fence is `mfence`
                // there, which drains them before the reply tells the client its pixels are on
                // the screen; on the other two it is the ordinary barrier.
                // PAIR: none. Nothing acquires against it: the other party is the display engine
                // scanning the aperture, and the ordering towards the client is the reply below,
                // a blocking IPC rendezvous.
                core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
                (if copied { 0 } else { gfx::EINVAL }, 0)
            }
            _ => (gfx::EINVAL, 0),
        };
        if let Some(to) = reply_slot {
            reply(to, r0 as u64, r1);
        }
    }
}

user_mode_runtime::panic_handler!();
