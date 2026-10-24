//! The display terminal: the terminal contract's IPC half (notes/terminal-contract.md) served
//! against a grid and a font, on a screen it does not own (milestone 29 (a display terminal)).
//!
//! One binary, two wirings chosen by `arg0`, neither holding a device: [`MODE_DISPLAY`] holds rung
//! one's display endpoint and the scanout (`painter`'s authority) and presents with `FLUSH`;
//! [`MODE_WINDOW`] holds rung two's doorbell and one window's control page and surface (`window`'s).
//! Its whole world is a report endpoint, one to present through, the one it serves (slot 2), an
//! application's output page and its own pixels: no device, interrupt, DMA authority, physical
//! address, or way to name another client's anything. With a control endpoint it can be replaced
//! live, by `video_terminal::component`'s `OPERATION_QUIESCE`/`START_ABSORB` exchange.

//! # One endpoint, and why input does not ring the doorbell
//!
//! With one wait point (§33 (the compositor's authority is memory, not messages)), output and
//! keystrokes share the served endpoint, told apart by opcode; an application holding it can forge
//! a keystroke into its own terminal and gains nothing. In [`MODE_WINDOW`] the compositor delivers
//! a keystroke by `CALL`ing this process, so ringing the doorbell in answer would deadlock; the
//! input source's own `COMMIT` shows it. `OPERATION_WRITE` does ring, since its blocked caller is
//! the application. See notes/glyphs/terminal-and-proof.md.
//!
//! Name: ratified 2026-08-01 (calef, milestone 63), replacing `vterm`. Refused `vterm` (an
//! abbreviation), `text_console` (`console` is already a program) and `video_terminal` for the
//! program (the crate is named for the protocol it implements and this program for its role, next
//! to `gpu_driver`, the virtio-gpu driver it is a client of, so the display ladder reads straight from
//! the filenames).

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

use compositor::proto::ctl;
use graphics_contract as gfx;
use line_editor::proto;
use user_mode_runtime::mapped_window::MappedWindow;
use user_mode_runtime::{Delivered, call, receive_request, reply, send};
use video_terminal::status::{MODE_DISPLAY, MODE_WINDOW};
use video_terminal::{component, handoff};

/// Capability slots, from `video_terminal::component`, which every builder reads too.
const REPORT: u64 = component::REPORT;
/// The endpoint this terminal presents through: the display driver in [`MODE_DISPLAY`], the
/// compositor's doorbell in [`MODE_WINDOW`].
const PRESENT: u64 = component::PRESENT;
/// The endpoint it serves. **One**, for both classes of sender; see the module note.
const TERM: u64 = component::TERM;
/// The untyped it spends on the page tables its own mappings need. [`MODE_DISPLAY`] only; see the
/// note on [`SURFACE_FRAME`].
const BUDGET: u64 = component::BUDGET;
/// The whole scanout, one `PageFrame` capability naming the run the surface lives in (on
/// virtio-gpu the `gfx::SURFACE_PAGE_FRAMES`-page run; on a firmware screen whatever covers it)
/// (DECISIONS §102), then one more slot for [`OUT_VA`]'s page. **[`MODE_DISPLAY`] only**
/// (milestone 108).
///
/// In [`MODE_WINDOW`] the surface, the output page and the control page arrive as spawn-time
/// mappings from `compositor_service` instead, which is why the `MAP` calls below are inside the
/// `MODE_DISPLAY` arm.
const SURFACE_FRAME: u64 = component::SURFACE;
const OUT_PAGE_FRAME: u64 = component::OUT;
const _: () = assert!(OUT_PAGE_FRAME == SURFACE_FRAME + 1);

/// Where the scanout goes. In [`MODE_DISPLAY`] this process holds the frames and picks the address;
/// in [`MODE_WINDOW`] the compositor's wiring maps it here.
///
/// The scanout run is up to 900 page frames (4 MiB from `SURFACE_VA`, DECISIONS §102 (a Frame
/// names a run of pages)), so
/// `OUT_VA` and `CTL_VA` sit clear of it; one inside it would be refused by `PageFrame::MAP` as
/// already mapped. The 2 MiB alignment keeps the run in as few page-table windows as possible
/// (`display_service::MAP_BUDGET_PAGES`'s comment has the arithmetic).
const SURFACE_VA: u64 = address_space_map::pair_page(0x0000_0000_0060_0000);
/// The page an application writes the bytes of an `OPERATION_WRITE` into. The terminal contract's
/// "control by message, bulk by shared page" split (DECISIONS §10 (process model: capability-based, microkernel. Untyped memory deferred.)), the same one `filesystem_contract` makes.
const OUT_VA: u64 = address_space_map::pair_page(0x0000_0000_0a00_0000);
/// The compositor's per-client control page. [`MODE_WINDOW`] only.
const CTL_VA: u64 = address_space_map::pair_page(0x0000_0000_0a01_0000);

/// Failure codes, in a `0xDEAD_...` word so a failure names its step rather than hanging.
const E_INFO: u64 = 0x01;
const E_GEOMETRY: u64 = 0x02;
const E_PRESENT: u64 = 0x03;
const E_HELLO: u64 = 0x04;
const E_MAGIC: u64 = 0x05;
const E_MODE: u64 = 0x06;
/// A frame this process holds would not map. [`MODE_DISPLAY`] only.
const E_SURFACE: u64 = 0x07;

/// **The grid lives in a `static`, not on the stack.** A user process here gets one 4 KiB page of
/// stack (`kernel/src/user.rs`, `USER_STACK_VA`), and a `Vt` is hundreds of KiB.
/// `video_terminal::Vt::new` is `const` precisely so this can be a `static`. It is `.data`, not
/// `.bss`: a blank cell is not zero bytes, so the grid's 177 pages are in the image file (a 900 KB
/// image, measured 2026-10-19 UTC).
static mut TERMINAL: video_terminal::Vt = video_terminal::Vt::new(1, 1);

/// The one terminal this process owns.
///
/// # Safety
/// This process has exactly one thread and cannot be given a second: it is built without its own
/// process, so nothing holds the `BIND` that adds a thread to it (milestone 812
/// (`std::thread::spawn` runs real threads in one address space)). So there is no second reference
/// and no aliasing question.
fn term() -> &'static mut video_terminal::Vt {
    // A raw pointer first, then one dereference: taking `&mut TERMINAL` directly is what
    // `static_mut_refs` exists to refuse, and it refuses it for a real reason (a second reference
    // would be undefined behaviour, not merely untidy).
    let p = &raw mut TERMINAL;
    // SAFETY: see above; single-threaded by construction, so `p` is the only route to it.
    unsafe { &mut *p }
}

fn rd32(va: u64) -> u32 {
    // SAFETY: inside a page the kernel mapped into this process at spawn.
    unsafe { core::ptr::read_volatile(va as *const u32) }
}

fn wr32(va: u64, v: u32) {
    // SAFETY: as above, and the page is writable.
    unsafe { core::ptr::write_volatile(va as *mut u32, v) }
}

fn out_byte(i: usize) -> u8 {
    // SAFETY: `i` is below FRAME_SIZE, inside the application's output page.
    unsafe { core::ptr::read_volatile((OUT_VA + i as u64) as *const u8) }
}

fn die(code: u64) -> ! {
    send(REPORT, 0xDEAD_0000_0000_0000 | code, 0, 0);
    user_mode_runtime::exit();
}

/// **Paint a rectangle of the surface** from the engine's picture.
///
/// `stride` is the surface's bytes per row, which differs between the two wirings (the scanout's in
/// [`MODE_DISPLAY`], the window's in [`MODE_WINDOW`]) and is therefore *read* rather than assumed.
/// Ordinarily only the damaged rectangle is passed: everything outside it is already correct, and
/// repainting it would make the damage rectangle a decoration rather than a saving. The exception
/// is the first frame, which paints the whole surface; see [`Wiring::present`].
fn paint(
    surface: &MappedWindow,
    x0: u32,
    y0: u32,
    w: u32,
    h: u32,
    stride: u32,
) -> (u32, u32, u32, u32) {
    let t = term();
    for y in y0..y0 + h {
        for x in x0..x0 + w {
            let off = (y * stride) as u64 + (x * 4) as u64;
            // The rectangle is either the engine's damage or the whole surface this process asked
            // the geometry of, and `Vt::pixel` is defined outside its own grid (it is the default
            // background), which is what makes the second case safe as well as correct.
            surface.w32(off, t.pixel(x, y));
        }
    }
    (x0, y0, w, h)
}

/// **Move the picture up `px` pixel rows without rendering anything**: the pixel half of a scroll
/// (`video_terminal::Damage`'s invariant).
///
/// Re-rendering unchanged rows through `Vt::pixel` (a divide, a cell fetch, a glyph lookup and a
/// blend for each of the 1120x624 scanout's 698,880 pixels, at ~3.5x per store under TCG) would be
/// most of the `x86_64` swish leg's time (`notes/benchmarks/icount-tick-scales.md`). So this moves
/// the bytes with one bounds-checked [`MappedWindow::copy_within`], not a checked access per word
/// (about 250 ms a scroll on riscv64 at opt-level 0). Packed rows (`w * 4 == stride`) move as one
/// block; a window narrower than its stride moves row by row.
fn scroll_surface(surface: &MappedWindow, stride: u32, w: u32, h: u32, px: u32) {
    let (stride, row, moved) = (stride as u64, w as u64 * 4, (h - px) as u64);
    let from = px as u64 * stride;
    if row == stride {
        surface.copy_within(from, 0, moved * stride);
        return;
    }
    // Destination rows run top-down and are always above their sources, so nothing is overwritten
    // before it is read.
    for y in 0..moved {
        surface.copy_within(from + y * stride, y * stride, row);
    }
}

/// A whole terminal's worth of state that the two wirings differ on. Small, and gathered here so the
/// serving loop below reads the same in both.
struct Wiring {
    mode: u64,
    stride: u32,
    /// [`MODE_WINDOW`]: damage this process has published but not seen composited yet, unioned so a
    /// second update before the compositor's next scan does not lose the first.
    pending: Option<video_terminal::CellRect>,
    seq: u32,
    /// The surface's size in pixels, which is **not** always a whole number of character cells.
    /// 128 is not a multiple of 7, so a full-scanout terminal owns 18 columns and leaves two
    /// pixels on the right that no cell covers. See [`Wiring::present`].
    surface: (u32, u32),
    /// The bounds-checked window onto the surface frames, sized to this wiring's own geometry
    /// (milestone 139 round 4; see the `SAFETY` comment where `_start` constructs it).
    window: MappedWindow,
    /// Has the whole surface been painted once? Until it has, the strip outside the grid holds
    /// whatever the frames held at boot.
    painted_all: bool,
}

impl Wiring {
    /// **Put whatever changed on the screen.**
    ///
    /// `MODE_DISPLAY` calls rung one's `FLUSH(rect)` on the display endpoint; `MODE_WINDOW` writes
    /// the rectangle into its control page, bumps its sequence, and rings `COMMIT`. A scroll moves
    /// pixels with [`scroll_surface`] and renders only the reported rectangle, then hands onward
    /// the whole surface, because every row on the screen moved.
    ///
    /// `ring` is false for a keystroke: see the module note on the deadlock.
    fn present(&mut self, ring: bool) {
        let Some(damage) = term().take_damage() else {
            return;
        };
        // For the control page in [`MODE_WINDOW`], a scroll is whole-grid damage in the old
        // vocabulary: the compositor re-scans whatever rectangle is named, and after a move that
        // rectangle has to be everything.
        let cells = damage.repaint_rect(term().cols(), term().rows());
        // **The first frame paints the whole surface, not just the grid.** Pixels outside every
        // cell (two columns of them on a 128-pixel scanout) are never damaged, so they would keep
        // whatever the frame held at boot. The grid never writes there, so once is enough.
        let (x, y, w, h) = if !self.painted_all {
            self.painted_all = true;
            paint(
                &self.window,
                0,
                0,
                self.surface.0,
                self.surface.1,
                self.stride,
            )
        } else if damage.scrolled > 0 {
            scroll_surface(
                &self.window,
                self.stride,
                self.surface.0,
                self.surface.1,
                damage.scrolled * video_terminal::CELL_H,
            );
            let (x, y, w, h) = damage.rect.to_pixels();
            paint(&self.window, x, y, w, h, self.stride);
            // Every row on the screen moved, whatever was rendered.
            (0, 0, self.surface.0, self.surface.1)
        } else {
            let (x, y, w, h) = damage.rect.to_pixels();
            paint(&self.window, x, y, w, h, self.stride)
        };

        // The pixels must be visible to whoever reads them next: another address space, and through
        // it a device. A release fence is the portable way to say so (`dmb ish` on aarch64, `fence`
        // on RISC-V), and it belongs in this userspace program rather than in arch code (rule 1).
        //
        // PAIR: whichever of the two paths below runs. `MODE_DISPLAY` pairs with `barrier()` in
        // components/src/gpu_driver.rs, the same as the compositor's `flush`; `MODE_WINDOW` pairs with
        // `serve_frame` in components/src/compositor.rs. Both are also ordered by the `call` each path
        // makes, when it makes one; the second fence below is the case where it does not.
        core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);

        if self.mode == MODE_DISPLAY {
            let (r0, _) = call(
                PRESENT,
                gfx::req(gfx::display::FLUSH, gfx::rect(x, y, w, h)),
                0,
            );
            if r0 as i64 != 0 {
                // The driver refuses a rectangle outside the surface rather than clamping it, so
                // this is our arithmetic being wrong. Fail loudly.
                die(E_PRESENT);
            }
            return;
        }

        // MODE_WINDOW. The compositor publishes the sequence it has composited, so damage that has
        // not been acknowledged is carried forward rather than overwritten.
        if rd32(CTL_VA + ctl::ACKED) == self.seq {
            self.pending = None;
        }
        let d = match self.pending {
            Some(p) => p.union(cells),
            None => cells,
        };
        self.pending = Some(d);
        // The rectangle published is the one painted, which on the first frame is the whole
        // surface rather than the grid, for the reason above.
        let (x, y, w, h) = if self.seq == 0 {
            (x, y, w, h)
        } else {
            d.to_pixels()
        };
        wr32(CTL_VA + ctl::DAMAGE_X, x);
        wr32(CTL_VA + ctl::DAMAGE_Y, y);
        wr32(CTL_VA + ctl::DAMAGE_W, w);
        wr32(CTL_VA + ctl::DAMAGE_H, h);
        self.seq += 1;
        // The sequence must become visible after the pixels and the rectangle that describes them,
        // or the compositor could composite a frame we have not finished writing.
        //
        // PAIR: `serve_frame` in components/src/compositor.rs, which milestone 43's audit found had no
        // fence (finding 7). **Read `ring` before assuming a rendezvous covers this one.** On a
        // keystroke `ring` is false and no `call` follows, so the only edge is the reply this
        // process is about to send to the compositor that `CALL`ed it. That reply does order it. The
        // case the reader's fence is genuinely load-bearing for is a *second* window committing
        // while `serve_frame` rescans every client's page; see the note in fixtures/src/window.rs and
        // notes/memory-ordering.md.
        core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
        wr32(CTL_VA + ctl::SEQ, self.seq);
        if ring {
            let (r0, _) = call(
                PRESENT,
                compositor::proto::req(compositor::proto::COMMIT, 0),
                0,
            );
            if r0 as i64 != 0 {
                die(E_PRESENT);
            }
        }
    }
}

/// **The handoff run, as this instance holds it** (milestone 23's `display_terminal` swap). It
/// comes from the machine's swap reserve, so it is never mapped at build: a supervisor sends it
/// with `CTL_HANDOFF` (`video_terminal::component`'s module note has the exchange).
struct HandoffRun {
    /// A run is mapped at `HANDOFF_ADDRESS`. True from the message that brought it until the reserve it
    /// came from is destroyed, which unmaps it from under this flag: the supervisor sends a plain
    /// `CTL_HANDOFF` only within the swap that sent the run, so the flag is never read stale.
    mapped: bool,
}

impl HandoffRun {
    /// The run, read or written whole: all `HANDOFF_PAGES` of it, not this build's
    /// `HANDOFF_BYTES`, because a predecessor's layout may run longer than this build's longest.
    fn bytes(&mut self) -> &mut [u8] {
        // SAFETY: called only while `mapped`, which means a run of `HANDOFF_PAGES` frames is
        // mapped read/write at `HANDOFF_ADDRESS` in this address space, mapped by `take` below from a
        // capability the supervisor sent. This process has one thread, and `&mut self` makes this
        // the only reference.
        unsafe {
            core::slice::from_raw_parts_mut(
                component::HANDOFF_ADDRESS as *mut u8,
                (video_terminal::handoff::HANDOFF_PAGES * address_space_map::PAGE) as usize,
            )
        }
    }

    /// Map the run a `CTL_HANDOFF` carried, or keep the one already mapped. `false` when no run
    /// came and none is mapped, or it would not map.
    fn take(&mut self, delivered: Delivered) -> bool {
        match delivered {
            Delivered::Delegation(run) => {
                // The capability stays in its slot: an instance is handed at most two runs in its
                // life, the one it absorbs and the one it writes, and each goes stale when its
                // reserve is destroyed.
                self.mapped = user_mode_runtime::map_page_frame(
                    run,
                    component::HANDOFF_ADDRESS,
                    true,
                    BUDGET,
                );
                self.mapped
            }
            Delivered::Nothing => self.mapped,
            Delivered::Reply(_) => false,
        }
    }

    /// **Started with `START_ABSORB`: become the incumbent.** Before the first paint, so nothing
    /// of the blank grid ever reaches the screen. A refusal is said and this instance leaves; the
    /// supervisor collects it and resumes the incumbent.
    fn absorb(&mut self) {
        let req = receive_request(component::CONTROL);
        let verdict = if req.w0 == proto::CTL_HANDOFF && self.take(req.delivered) {
            term().absorb_handoff(self.bytes())
        } else {
            Err(handoff::Refusal::NoBlob)
        };
        match verdict {
            Ok(()) => {
                send(component::SUPERVISOR, proto::NOTE_ABSORBED, 0, 0);
            }
            Err(refusal) => {
                // `line_editor::proto::NOTE_NOT_ABSORBED`'s codes: 0 no blob, 1 malformed (a grid
                // of another shape is one: its cells would land on the wrong rows), otherwise the
                // layout found.
                let code = match refusal {
                    handoff::Refusal::NoBlob => 0,
                    handoff::Refusal::Geometry | handoff::Refusal::Malformed => 1,
                    handoff::Refusal::Layout(l) => u64::from(l),
                };
                send(component::SUPERVISOR, proto::NOTE_NOT_ABSORBED, code, 0);
                user_mode_runtime::exit();
            }
        }
    }

    /// **Quiesced: wait for the supervisor's word.** Write the blob when the run comes, go back to
    /// serving on `CTL_RESUME`, leave on `CTL_QUIT`. Anything else on the control endpoint is
    /// ignored: only the supervisor holds it.
    fn quiesced(&mut self) {
        loop {
            let req = receive_request(component::CONTROL);
            match req.w0 {
                proto::CTL_HANDOFF => {
                    let written = if self.take(req.delivered) {
                        term().write_handoff(self.bytes())
                    } else {
                        None
                    };
                    let (note, len) = match written {
                        Some(len) => (proto::NOTE_WRITTEN, len as u64),
                        None => (proto::NOTE_NOT_WRITTEN, 0),
                    };
                    send(component::SUPERVISOR, note, len, 0);
                }
                proto::CTL_RESUME => return,
                proto::CTL_QUIT => user_mode_runtime::exit(),
                _ => {}
            }
        }
    }
}

/// Map the surface (in [`MODE_DISPLAY`]) and learn its size from whoever owns the screen:
/// `(width, height, stride)` in pixels and bytes.
fn negotiate_geometry(mode: u64) -> (u32, u32, u32) {
    // **The geometry is asked for, never assumed**, in both wirings. Rung one added `INFO` for this
    // and rung two publishes a control page for it, and the reason is the same in both: the process
    // that owns the screen decides how big a surface is, and a client that guessed would paint the
    // wrong shape the first time anything changed.
    match mode {
        MODE_DISPLAY => {
            // The scanout and the application's output page are `PageFrame`s this process holds, and it
            // maps them itself out of its own budget (milestone 108). Before the `INFO` call,
            // because a terminal with nowhere to paint has no use for the geometry. One `MAP` call
            // for the whole scanout run (DECISIONS §102), not one per page.
            if !user_mode_runtime::map_page_frame(SURFACE_FRAME, SURFACE_VA, true, BUDGET) {
                die(E_SURFACE);
            }
            if !user_mode_runtime::map_page_frame(OUT_PAGE_FRAME, OUT_VA, true, BUDGET) {
                die(E_SURFACE);
            }
            let (r0, geometry) = call(PRESENT, gfx::req(gfx::display::INFO, 0), 0);
            if r0 as i64 != 0 {
                die(E_INFO);
            }
            let (w, h) = ((geometry & 0xffff_ffff) as u32, (geometry >> 32) as u32);
            // Four bytes a pixel and no padding, at the width `INFO` answered: on virtio-gpu that
            // is `gfx::STRIDE` exactly, and on a firmware screen the driver's surface was
            // allocated at this width (2026-10-04, when it started filling the screen).
            (w, h, w * gfx::BYTES_PER_PIXEL)
        }
        MODE_WINDOW => {
            // `HELLO` first: its reply cannot arrive until the compositor is serving the doorbell,
            // and the compositor publishes every control page before it starts serving. The
            // rendezvous is the synchronization, so nothing polls for a valid page.
            let (r0, _) = call(
                PRESENT,
                compositor::proto::req(compositor::proto::HELLO, 0),
                0,
            );
            if r0 as i64 != 0 {
                die(E_HELLO);
            }
            if rd32(CTL_VA + ctl::MAGIC) != ctl::MAGIC_VALUE {
                die(E_MAGIC);
            }
            (
                rd32(CTL_VA + ctl::WIDTH),
                rd32(CTL_VA + ctl::HEIGHT),
                rd32(CTL_VA + ctl::STRIDE),
            )
        }
        _ => die(E_MODE),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn _start(mode: u64, control: u64, start: u64) -> ! {
    // Under a supervisor it names the control endpoint's slot, which is fixed; a spawner that
    // supervises nothing passes `0`.
    let supervised = match control {
        0 => false,
        component::CONTROL => true,
        _ => die(E_MODE),
    };
    let (w, h, stride) = negotiate_geometry(mode);

    // A surface too small for one character has nothing to show and is refused. It does **not**
    // have to be a whole number of cells: a compositor window or a firmware screen can leave a
    // strip on the right or the bottom that no cell covers. `present` paints it once with the
    // background the engine says is there, which is why a partial cell is a defined picture
    // rather than a strip of whatever the frames held at boot.
    if w < video_terminal::CELL_W || h < video_terminal::CELL_H {
        die(E_GEOMETRY);
    }
    let (cols, rows) = (w / video_terminal::CELL_W, h / video_terminal::CELL_H);
    if cols as usize > video_terminal::MAX_COLS
        || rows as usize > video_terminal::MAX_ROWS
        || cols as usize * rows as usize > video_terminal::MAX_CELLS
    {
        die(E_GEOMETRY);
    }
    // `reset_to`, not `*term() = Vt::new(cols, rows)` (milestone 142): `cols`/`rows` are runtime
    // values negotiated with the driver or the compositor, so `Vt::new` here would run as an
    // ordinary function call rather than being evaluated at compile time, and its return value (a
    // `Vt` is hundreds of KiB now, see `Vt`'s own doc) would need to exist somewhere at runtime.
    // This process gets one 4 KiB page of stack; `reset_to` mutates the existing `static` in place
    // instead, so no `Vt`-sized value is ever a stack local or a return value here.
    term().reset_to(cols, rows);
    // A replacement becomes its incumbent before it paints, so its first picture is the
    // incumbent's screen rendered from the absorbed state, not a blank one.
    let mut handoff = HandoffRun { mapped: false };
    if supervised && start & proto::START_ABSORB != 0 {
        handoff.absorb();
    }

    // SAFETY: in MODE_DISPLAY this process mapped the whole surface run it was granted at
    // SURFACE_VA itself, above, and the kernel sized that run from the same geometry the driver
    // answers `INFO` with: `gfx::SURFACE_PAGE_FRAMES` on virtio-gpu (where `stride * h` is
    // `gfx::SURFACE_BYTES`), `width * height * 4` rounded up on a firmware screen
    // (`display_service::start_screen_terminal`). The grid check above also bounds `stride * h`
    // under 28 MB, far short of `OUT_VA`, so even a driver that lied could only fault this
    // process, never write its output page. MODE_WINDOW's frames are mapped by the compositor's
    // `spawn_client_term` before `HELLO`'s reply arrived, sized to the same geometry (`stride`,
    // `h`) this process just read off the control page it validated above (milestone 139 (drive
    // the unsafe count down), round 4).
    let window = unsafe { MappedWindow::new(SURFACE_VA, stride as u64 * h as u64) };

    let mut wiring = Wiring {
        mode,
        stride,
        pending: None,
        seq: 0,
        surface: (w, h),
        window,
        painted_all: false,
    };
    // The blank grid is a *defined* picture (spaces on the default background), so presenting it
    // before anyone has written a byte means the screen a spawner sees is black rather than whatever
    // was in those frames. A fresh `Vt` reports its whole grid as damage for exactly this reason.
    wiring.present(true);
    send(
        REPORT,
        video_terminal::status::TERM_UP,
        cols as u64 | ((rows as u64) << 32),
        mode,
    );

    loop {
        // The one wait point. An application's `OPERATION_WRITE` and an input source's `OPERATION_BYTES` both
        // arrive here and are told apart by opcode, because there is no wait-any to tell them apart
        // by endpoint (DECISIONS §33).
        //
        // A CALL's Reply, or nothing to answer (milestone 706 (a `CALL` server can tell a Reply
        // from a delegation)): a SEND_CAP is deleted rather than answered into.
        let req = receive_request(TERM);
        let (w0, w1) = (req.w0, req.w1);
        let reply_slot = req.delivered.into_reply();
        let mut r0: u64 = 0;
        match proto::operation(w0) {
            // The application half: print `len` bytes from the shared output page. The terminal
            // performs no newline translation, deliberately: `line_editor::expand_output` already put
            // `\r\n` on the wire for a Unix `\n`, and a second translation here would move the
            // carriage twice. The engine treats a bare `LF` as a line feed, which is what a VT does.
            proto::OPERATION_WRITE => {
                let n = proto::len(w0).min(4096);
                for i in 0..n {
                    let b = [out_byte(i)];
                    term().feed(&b);
                }
                r0 = n as u64;
                // Ring: nobody else is going to ask for this frame.
                wiring.present(true);
            }
            // The driver half: one to eight raw wire bytes, packed little-endian in the second word.
            // Byte for byte the framing `input.rs` sends and the compositor forwards, so this
            // terminal is a focusable compositor client without either contract changing.
            proto::OPERATION_BYTES => {
                let n = proto::len(w0).min(8);
                for k in 0..n {
                    let b = [((w1 >> (8 * k)) & 0xff) as u8];
                    term().feed(&b);
                }
                // Do NOT ring in MODE_WINDOW: the compositor is blocked in its CALL to us. See the
                // module note. In MODE_DISPLAY there is no such caller, so a flush is safe and
                // necessary.
                wiring.present(wiring.mode == MODE_DISPLAY);
            }
            // A supervisor's quiesce (milestone 23). Every request queued ahead of it has been
            // served, and nothing is parked here to hand back, so the answer is immediate. From
            // here until `CTL_RESUME` this instance reads only its control endpoint.
            proto::OPERATION_QUIESCE if supervised => {
                if let Some(to) = reply_slot {
                    reply(to, proto::QUIESCED, 0);
                }
                handoff.quiesced();
                continue;
            }
            // `OPERATION_READLINE` and `OPERATION_INTRCOUNT` are the line discipline's, and this component is not
            // one: it renders a stream and echoes keystrokes. A client that wants edited lines puts
            // `line_editor` in front of this and prints its echo through `OPERATION_WRITE`, which needs no new
            // protocol because `line_editor`'s echo is exactly a byte stream this engine parses (the
            // `video_terminal` crate's interoperability test proves that on the host). Recorded as a limit in
            // notes/glyphs.md rather than half-implemented.
            _ => r0 = proto::BAD_REQUEST,
        }
        if let Some(to) = reply_slot {
            reply(to, r0, 0);
        }
    }
}

user_mode_runtime::panic_handler!();
