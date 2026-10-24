//! **A window: a client of the compositor** (milestone 33, the display ladder's rung two).
//!
//! One program, several roles, because the roles differ by one act each and sharing the honest path
//! is what makes an attack a fair test rather than a different program failing for its own reasons
//! (the same reason `gpu_driver` carries its own escape role).
//!
//! An honest window cannot name another client, another surface, or the screen. No request takes a
//! window id (the protocol has none), so there is nothing to forge, and its neighbours' pixels are
//! not in its address space, so there is nothing to reach. See notes/compositor.md.

//! # What an honest window holds
//!
//! - slot 0, a **report** endpoint: its verdict goes to whoever spawned it;
//! - slot 1, the **doorbell** (WRITE): where it rings the compositor;
//! - slot 2, an **input** endpoint (READ), *only if it was granted one*. Holding it makes a client
//!   eligible for focus; without it nobody can send it a keystroke, and the kernel refuses its
//!   receive there with `NoSuchSlot`;
//! - a **control page** and a **surface**: its own geometry, its own damage rectangle, its own
//!   pixels.

//! # What the roles prove
//!
//! The probes ([`ROLE_PROBE_INPUT`], [`ROLE_PROBE_NEIGHBOUR`], [`ROLE_PROBE_SCREEN`]) each reach
//! for what an honest window was not granted and are refused or killed; [`ROLE_CAPTURE`] is granted
//! the screen read-only and cannot deface it. Each role's constant says what it proves.
//!
//! Name: ratified 2026-07-30 (calef, DECISIONS §39), among the names recorded there as always
//! right.

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

use compositor::proto::{ctl, wlist};
use compositor::status;
use user_mode_runtime::mapped_window::MappedWindow;
use user_mode_runtime::{call, exit, invoke, map_page_frame, receive_request, reply, send};

/// Capability slots, by convention with `kernel/src/user/compositor_service.rs`.
const REPORT: u64 = 0;
const DOORBELL: u64 = 1;
const INPUT: u64 = 2;
/// The screen, one `PageFrame` capability naming the whole run (DECISIONS §102). **[`ROLE_CAPTURE`]
/// only** (milestone 142): before the grid grew, the screen was a handful of `Spawn::maps` entries
/// like everything else here; at the grown scanout that would be hundreds of them, which does not
/// fit in the 1 KiB a spawn closure may capture (`kernel/src/thread.rs`'s own limit) let alone this
/// capability table. A capture client maps it itself, the same way `painter`/`display_terminal`
/// already do for rung one's own surface.
const SCREEN_FRAME: u64 = 3;
/// The untyped [`SCREEN_FRAME`]'s `MAP` draws its page tables from. **[`ROLE_CAPTURE`] only.**
const BUDGET: u64 = 4;

/// Where the kernel maps what this process is given. Must match `compositor_service`. **Every client
/// uses the same addresses**, which is the point: two clients' surfaces are the same virtual address
/// in different address spaces, so "the neighbour's surface" is not somewhere a client can reach by
/// guessing.
const CTL_VA: u64 = address_space_map::pair_page(0x0000_0000_0060_0000);
const SURFACE_VA: u64 = address_space_map::pair_page(0x0000_0000_0061_0000);
/// The screen and the window list, read-only, and **only** for a client granted them.
///
/// `SCREEN_VA` is 2 MiB-aligned (milestone 142 (a text display good enough that people use it
/// instead of a GUI), DECISIONS §102 (a Frame names a run of pages)), and `WLIST_VA` sits clear of
/// the 4 MiB the screen spans at 900 page frames, the scanout's first size: inside that span,
/// `PageFrame::MAP` would refuse the window list as already mapped. At 924x344 (311 page frames,
/// 1.25 MiB, the scanout since 2026-08-27) the screen fits one 2 MiB page-table window;
/// `compositor_service::MAP_BUDGET_PAGES`'s comment has the arithmetic.
const SCREEN_VA: u64 = address_space_map::pair_page(0x0000_0000_0080_0000);
const WLIST_VA: u64 = address_space_map::pair_page(0x0000_0000_0c00_0000);

/// Roles, as a bitmask in `arg0`. A plain window is 0.
const ROLE_INPUT: u64 = 1 << 0;
/// Receive on the input slot it was **not** granted. The refusal is `abi::Error::NoSuchSlot` (-1),
/// "there is nothing there", which is what no-ambient-authority reads like from the inside: not a
/// permission error from a server that consulted a list.
const ROLE_PROBE_INPUT: u64 = 1 << 1;
/// Write at the address where its neighbour's pixels really do live, one page past its own grant.
/// The kernel hands it that address, so it is a perfectly informed attacker; it faults anyway,
/// because the boundary is the mapping and not the layout.
const ROLE_PROBE_NEIGHBOUR: u64 = 1 << 2;
/// Read the composed screen. It drew into that screen and cannot see it: there is no ambient
/// display, and a screenshot is a capability.
const ROLE_PROBE_SCREEN: u64 = 1 << 3;
/// Granted a read-only mapping of the screen and the window list, so it *can* enumerate and
/// screenshot, and it reports a digest the kernel checks against the picture it computed itself.
/// Then it tries to write the screen and dies: a screenshot tool cannot deface what it can read.
const ROLE_CAPTURE: u64 = 1 << 4;
const ROLE_VICTIM: u64 = 1 << 5;
const ROLE_SMALL_DAMAGE: u64 = 1 << 6;
/// With [`ROLE_PROBE_NEIGHBOUR`]: **read** the address instead of writing it (milestone 719 (compositor confinement claim 25)). A write
/// to a read-only page faults at the same address a write to an unmapped one does, so a
/// write-only probe cannot tell "not mapped" from "mapped read-only", and a read-only exposure of a
/// neighbour's pixels is the confidentiality breach.
const ROLE_PROBE_READ: u64 = 1 << 7;
/// Commit every damage rectangle in [`LIES`] (milestone 719), the compositor's one untrusted input.
const ROLE_LIE_DAMAGE: u64 = 1 << 8;
/// Forge the control page's identity, geometry and bookkeeping after `HELLO`, then commit: the
/// eighth outsider pass's addition. The page is the client's to write (it is mapped read/write
/// here), and every field a confused deputy might re-trust is forged at once: `ID`, `WIDTH`,
/// `HEIGHT`, `STRIDE`, `STATUS`, `ACKED` and a `SEQ` that jumps backwards and then to `u32::MAX`.
const ROLE_FORGE_CTL: u64 = 1 << 9;

/// Damage rectangles no honest client sends: extreme origins and extents, including the pairs whose
/// `x + w` leaves `i32`. `(x, y, w, h)` as the control page carries them.
const LIES: [(i32, i32, u32, u32); 9] = [
    (i32::MAX, 0, 1, 1),
    (0, i32::MAX, 1, 1),
    (i32::MIN, i32::MIN, u32::MAX, u32::MAX),
    (-5, -5, 0x8000_0000, 0x8000_0000),
    (1, 1, 0x7fff_ffff, 0x7fff_ffff),
    (0, 0, u32::MAX, u32::MAX),
    (i32::MAX, i32::MAX, u32::MAX, u32::MAX),
    (0, 0, 0, 0),
    (-1, -1, 3, 3),
];

/// What a refusal report is about, in the second word of [`status::WIN_REFUSED`].
const WHAT_INPUT: u64 = 1;

/// Failure codes, in a `0xDEAD_...` word so a failure names its step.
const E_MAGIC: u64 = 0x01;
const E_GEOMETRY: u64 = 0x02;
const E_HELLO: u64 = 0x03;
const E_COMMIT: u64 = 0x04;
/// The spawner refused a report round trip. Defensive: the kernel-side test always replies 0, so this
/// firing means the scenario was wired differently than this role expects.
const E_REPORT: u64 = 0x05;
/// [`ROLE_CAPTURE`]'s own `PageFrame::MAP` of the screen was refused.
const E_SCREEN: u64 = 0x06;

fn rd32(va: u64) -> u32 {
    // SAFETY: inside a page the kernel mapped into this process at spawn.
    unsafe { core::ptr::read_volatile(va as *const u32) }
}

fn wr32(va: u64, v: u32) {
    // SAFETY: as above, and the page is writable (the control page and the surface both are).
    unsafe { core::ptr::write_volatile(va as *mut u32, v) }
}

/// **Not a `const`, unlike `painter.rs`'s twin**: the compositor maps this client's surface at
/// `spawn_client` time to exactly `SCENE[i].frames()` pages (`kernel/src/user/compositor_service.rs`),
/// which differs per client and is not knowable at this program's compile time. The bound this
/// process itself can trust is `bytes`, the `w * h * 4` computed and checked against
/// `compositor::MAX_SURFACE_BYTES` in `_start` below (milestone 43, notes/shared-page-audit.md
/// finding 4) before any pixel is painted, so the window is constructed there, once, right after
/// that check passes, and threaded to every `px_write`/`px_read` call rather than declared as a
/// file-level constant (milestone 139 round 4).
fn px_write(surface: &MappedWindow, i: usize, v: u32) {
    surface.w32((i * 4) as u64, v);
}

fn px_read(surface: &MappedWindow, i: usize) -> u32 {
    surface.r32((i * 4) as u64)
}

fn die(code: u64) -> ! {
    send(REPORT, 0xDEAD_0000_0000_0000 | code, 0, 0);
    exit();
}

/// Ring the doorbell. Content-free by design: the compositor learns what changed from our control
/// page, not from this message, because a shared endpoint's messages cannot be trusted.
fn ring(operation: u64) -> i64 {
    let (r0, _) = call(DOORBELL, compositor::proto::req(operation, 0), 0);
    r0 as i64
}

/// [`ROLE_FORGE_CTL`]: forge the control page's identity, geometry and bookkeeping and commit after
/// each forge, then restore the honest values and commit once more, and report how many rings
/// were answered.
fn forge_control_page(seq: &mut u32) {
    let (id0, w0, h0, s0) = (
        rd32(CTL_VA + ctl::ID),
        rd32(CTL_VA + ctl::WIDTH),
        rd32(CTL_VA + ctl::HEIGHT),
        rd32(CTL_VA + ctl::STRIDE),
    );
    wr32(CTL_VA + ctl::ID, 0xDEAD_BEEF);
    wr32(CTL_VA + ctl::WIDTH, u32::MAX);
    wr32(CTL_VA + ctl::HEIGHT, u32::MAX);
    wr32(CTL_VA + ctl::STRIDE, 0);
    wr32(CTL_VA + ctl::STATUS, 0xFFFF_FFFF);
    wr32(CTL_VA + ctl::ACKED, 0xFFFF_FFFF);
    wr32(CTL_VA + ctl::SEQ, 0);
    commit(seq, compositor::Rect::new(0, 0, 4, 4));

    wr32(CTL_VA + ctl::STRIDE, 1);
    wr32(CTL_VA + ctl::WIDTH, 0x7fff_ffff);
    wr32(CTL_VA + ctl::SEQ, u32::MAX);
    commit(seq, compositor::Rect::new(0, 0, 4, 4));

    wr32(CTL_VA + ctl::ID, id0);
    wr32(CTL_VA + ctl::WIDTH, w0);
    wr32(CTL_VA + ctl::HEIGHT, h0);
    wr32(CTL_VA + ctl::STRIDE, s0);
    wr32(CTL_VA + ctl::STATUS, ctl::STATUS_OK);
    commit(seq, compositor::Rect::new(0, 0, 4, 4));
    send(REPORT, status::WIN_FORGED, 3, 0);
}

/// Set our damage rectangle and bump our sequence, then commit. The compositor reads the rectangle as
/// untrusted input and clips it to our surface; the reply comes after the pixels are on the screen.
fn commit(seq: &mut u32, damage: compositor::Rect) {
    wr32(CTL_VA + ctl::DAMAGE_X, damage.x as u32);
    wr32(CTL_VA + ctl::DAMAGE_Y, damage.y as u32);
    wr32(CTL_VA + ctl::DAMAGE_W, damage.w);
    wr32(CTL_VA + ctl::DAMAGE_H, damage.h);
    *seq += 1;
    // The sequence bump must be visible after the pixels and the rectangle it describes, or the
    // compositor could composite a frame we have not finished writing. A release fence, portable
    // across both ISAs, rather than arch-specific asm in a userspace program.
    //
    // PAIR: `serve_frame` in components/src/compositor.rs, which reads `SEQ` and then the four damage
    // fields. Milestone 43's audit found that reader had no fence at all (finding 7), and **this is
    // the one publish in the subsystem the IPC rendezvous does not cover**: the doorbell is shared,
    // so `serve_frame` rescans *every* client's control page on any client's `COMMIT`, and only the
    // caller is blocked. A second window mid-`commit` on another core is read with nothing ordering
    // the compositor's own loads. See notes/memory-ordering.md.
    core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
    wr32(CTL_VA + ctl::SEQ, *seq);
    if ring(compositor::proto::COMMIT) != 0 {
        die(E_COMMIT);
    }
}

user_mode_runtime::program_entry!(start);

fn start(role: u64, neighbour_va: u64, _arg2: u64) -> ! {
    // `HELLO` first, for one reason: its reply cannot arrive until the compositor is serving the
    // doorbell, and the compositor publishes every control page before it starts serving. So the
    // rendezvous *is* the synchronization, and no client polls for a valid control page.
    if ring(compositor::proto::HELLO) != 0 {
        die(E_HELLO);
    }

    // A capture client maps the screen itself, read-only, out of its own budget (milestone 142,
    // DECISIONS §102): one `PageFrame` capability naming the whole run, one `MAP` call. Every other
    // role has nothing granted at [`SCREEN_FRAME`], so this is a no-op there (`MAP` on an empty
    // slot's own irrelevance: the block never runs).
    if role & ROLE_CAPTURE != 0 && !map_page_frame(SCREEN_FRAME, SCREEN_VA, false, BUDGET) {
        die(E_SCREEN);
    }

    let (id, w, h, bytes) = read_validated_geometry();
    // SAFETY: the compositor's `spawn_client` mapped `bytes` bytes (this client's own `w * h * 4`,
    // published on the control page it just read and validated above) read/write at `SURFACE_VA`
    // before `HELLO`'s reply arrived (see `px_write`/`px_read`'s doc). Constructing the window
    // touches no memory.
    let surface = unsafe { MappedWindow::new(SURFACE_VA, bytes as u64) };

    // Paint. Every pixel is a function of the coordinates and of *which window we are*, so a surface
    // painted by the wrong client, at the wrong offset, or not at all is visibly wrong (crates/compositor
    // asserts those properties on the host).
    for y in 0..h {
        for x in 0..w {
            px_write(
                &surface,
                (y * w + x) as usize,
                compositor::window_pixel(id, x, y),
            );
        }
    }

    let mut seq = 0u32;
    commit(&mut seq, compositor::Rect::new(0, 0, w, h));

    // Read our own surface back and digest it: the client-side witness that what we painted is what
    // is in the frames, taken after the compositor read them.
    let digest = compositor::surface_checksum(w, h, |i| px_read(&surface, i));

    // --- The refusal that needs no attack: ask for something we hold no capability for. ---
    if role & ROLE_PROBE_INPUT != 0 {
        // SAFETY: `svc`/`ecall`. The slot is empty, so the kernel refuses; nothing happens. Left as
        // a raw `invoke` (milestone 139 round 7's own survey of the whole `invoke` cluster): this is
        // the one call site of its kind, deliberately probing the raw negative `abi::Error` a RECEIVE
        // against an empty slot returns, which `user_mode_runtime::receive`'s own contract discards (it assumes
        // success and returns the three data words, not the syscall's own return code). A wrapper
        // exposing the raw code would be a second `receive` for one caller, not a real reduction.
        let r = unsafe { invoke(INPUT, abi::rendezvous::RECEIVE, 0, 0, 0) };
        send(REPORT, status::WIN_REFUSED, r as u64, WHAT_INPUT);
    }

    if role & ROLE_VICTIM != 0 {
        // Report the digest and **wait**: the kernel replies once the attacker beside us has done its
        // worst, and we then re-read our own surface. A pattern that survives an attack is the witness
        // that matters, and taking it from inside the victim's own address space is what makes it a
        // witness rather than an inference.
        let (r0, _) = call(REPORT, status::WIN_PAINTED, digest);
        if r0 as i64 != 0 {
            die(E_REPORT);
        }
        let after = compositor::surface_checksum(w, h, |i| px_read(&surface, i));
        send(REPORT, status::WIN_INTACT, after, digest);
        exit();
    }

    send(REPORT, status::WIN_PAINTED, digest, id as u64);

    if role & ROLE_CAPTURE != 0 {
        // We hold a read-only mapping of the screen and of the window list. That mapping IS the
        // screenshot capability and the enumeration capability; there is no verb for either, which is
        // why this block calls nothing and asks nobody. After our own commit, so what we see includes
        // our own window and is a complete screen rather than a half-composed one.
        let shot = compositor::surface_checksum(compositor::SCREEN_W, compositor::SCREEN_H, |i| {
            // SAFETY: inside the read-only screen mapping the kernel gave this role.
            unsafe { core::ptr::read_volatile((SCREEN_VA + (i * 4) as u64) as *const u32) }
        });
        let count = rd32(WLIST_VA + wlist::COUNT);
        let focused = rd32(WLIST_VA + wlist::FOCUSED);
        send(
            REPORT,
            status::WIN_CAPTURED,
            shot,
            focused as u64 | ((count as u64) << 32),
        );
    }

    if role & ROLE_SMALL_DAMAGE != 0 {
        // A second frame with a *small* damage rectangle, to prove a one-window redraw does not cost a
        // whole screen. The kernel poisons the screen outside this rectangle between the two commits;
        // the poison must survive.
        let (r0, _) = call(REPORT, status::WIN_PAINTED, digest);
        if r0 as i64 != 0 {
            die(E_REPORT);
        }
        commit(&mut seq, compositor::SMALL_DAMAGE);
        send(REPORT, status::WIN_PAINTED, digest, seq as u64);
    }

    // --- The probes that are expected to be fatal. Reported first, so a test knows the attempt
    // happened even though nothing can report afterwards. ---
    if role & ROLE_LIE_DAMAGE != 0 {
        for &(x, y, w, h) in LIES.iter() {
            commit(&mut seq, compositor::Rect::new(x, y, w, h));
        }
        send(REPORT, status::WIN_LIED, LIES.len() as u64, 0);
    }

    if role & ROLE_FORGE_CTL != 0 {
        forge_control_page(&mut seq);
    }

    if role & ROLE_PROBE_NEIGHBOUR != 0 {
        send(REPORT, status::WIN_PROBING, neighbour_va, 0);
        // SAFETY: deliberately not safe. This is the milestone's central claim under test: the
        // address is where a neighbour's pixels physically are, one page past our own grant, and it is
        // not mapped here. The store must fault. If it does not, the report below fires and the test
        // fails loudly rather than passing quietly. With [`ROLE_PROBE_READ`] the access is a load
        // instead, which is the half a read-only mapping would not stop.
        if role & ROLE_PROBE_READ == 0 {
            // SAFETY: deliberately not safe; see above. The store must fault.
            unsafe { core::ptr::write_volatile(neighbour_va as *mut u32, 0xBAD0_BAD0) };
        }
        // SAFETY: as above; only reached if the write did not fault, which is a broken system.
        let seen = unsafe { core::ptr::read_volatile(neighbour_va as *const u32) };
        send(REPORT, status::WIN_ESCAPED, neighbour_va, seen as u64);
    }

    if role & ROLE_PROBE_SCREEN != 0 {
        // Ask for the screen the way a capture client would (milestone 719). With nothing granted at
        // `SCREEN_FRAME` this is refused and the read below faults on an unmapped address; if the
        // spawner leaked the capability it succeeds and the read below returns a pixel, which is the
        // only way the test can see a grant, since a grant alone maps nothing.
        let _ = map_page_frame(SCREEN_FRAME, SCREEN_VA, false, BUDGET);
        send(REPORT, status::WIN_PROBING, SCREEN_VA, 0);
        // SAFETY: deliberately not safe. We painted into this screen and we are not mapped to read it.
        let seen = unsafe { core::ptr::read_volatile(SCREEN_VA as *const u32) };
        send(REPORT, status::WIN_ESCAPED, SCREEN_VA, seen as u64);
    }

    if role & ROLE_CAPTURE != 0 {
        // The screen mapping is read-only, so a screenshot tool cannot deface the screen. Last,
        // because it is fatal.
        send(REPORT, status::WIN_PROBING, SCREEN_VA, 1);
        // SAFETY: deliberately not safe; a write to a read-only mapping must fault.
        unsafe { core::ptr::write_volatile(SCREEN_VA as *mut u32, 0xBAD0_BAD0) };
        send(REPORT, status::WIN_ESCAPED, SCREEN_VA, 1);
    }

    if role & ROLE_INPUT != 0 {
        report_every_keystroke();
    }

    exit();
}

/// Read this window's id and size off its control page, and refuse a size larger than the
/// contract allows. Returns the id, the width and height, and the surface's size in bytes.
fn read_validated_geometry() -> (u32, u32, u32, u32) {
    if rd32(CTL_VA + ctl::MAGIC) != ctl::MAGIC_VALUE {
        die(E_MAGIC);
    }
    let id = rd32(CTL_VA + ctl::ID);
    let w = rd32(CTL_VA + ctl::WIDTH);
    let h = rd32(CTL_VA + ctl::HEIGHT);
    // Told rather than assumed, because the compositor chooses the size; still bounded by the
    // largest surface the contract allows, since a larger claim describes memory we were not
    // granted. Checked arithmetic: `w * h * 4` in `u32` wraps, and `w = h = 0x10000` multiplies to
    // exactly 0, which would pass the bound and let the paint loop write 2^32 pixels past a
    // two-frame surface (milestone 43 (a second security audit), notes/shared-page-audit.md finding
    // 4). `display_terminal.rs` validates the same geometry by division.
    let bytes = w
        .checked_mul(h)
        .and_then(|px| px.checked_mul(4))
        .unwrap_or(u32::MAX);
    if w == 0 || h == 0 || bytes > compositor::MAX_SURFACE_BYTES {
        die(E_GEOMETRY);
    }
    (id, w, h, bytes)
}

/// Answer every input request on [`INPUT`] and report each byte it carried, forever.
fn report_every_keystroke() -> ! {
    // Focus routing: we hold an input endpoint, so keystrokes can reach us. A client without this
    // capability cannot be sent one, however the compositor feels about it.
    let mut count = 0u64;
    loop {
        let req = receive_request(INPUT);
        let (w0, bytes) = (req.w0, req.w1);
        // Answer first: the compositor is blocked in CALL, the terminal contract's driver-half
        // rendezvous, and it is the flow control for a fast source.
        if let Some(to) = req.delivered.into_reply() {
            reply(to, 0, 0);
        }
        // Clamp to the eight bytes one word carries. `proto::len` is a full 32-bit field, so
        // an unclamped count shifts past 63 at `k == 8` and spins up to 2^32 times.
        // `display_terminal.rs` and `line_editor.rs` both clamp here; this was the one that
        // did not (milestone 43, notes/shared-page-audit.md finding 4).
        let n = line_editor::proto::len(w0).min(8);
        for k in 0..n {
            count += 1;
            send(REPORT, status::WIN_INPUT, (bytes >> (8 * k)) & 0xff, count);
        }
    }
}

user_mode_runtime::panic_handler!();
