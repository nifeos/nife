//! The compositor contract's arithmetic: one screen, several mutually distrusting clients, a
//! surface each.
//!
//! It holds the scene ([`SCENE`]), the clipping and damage math, the composition and the protocol
//! ([`proto`]), checked on the host and shared by `components/src/compositor.rs`, its clients
//! (`fixtures/src/window.rs`), the kernel-side test and the host scanout check, so none can drift.
//! It holds no authorization code: every per-client fact lives in per-client memory and the reply
//! words carry status only. No GPU acceleration, fonts, window management or alpha blending. See
//! notes/compositor.md, and notes/compositor/arithmetic.md for the scene and the clips.
//!
//! Name: ratified 2026-08-01 (calef), replacing `compose`. Refused `compose` (a verb, where a
//! namespace is a thing), `compositor_proto` (promises a wire definition and delivers an algorithm)
//! and `scene` (undersells the clipping and damage arithmetic). It shares the program's name
//! because it is that program's logic, lifted out to be host-testable.

#![no_std]
// milestone 68's ratchet is workspace-wide (§107); this crate opts out until its 19-item
// worklist (notes/doc-coverage.md) is burned down.
#![allow(missing_docs)]

// ================================================================================================
// The screen.
// ================================================================================================

/// The screen's width. **The screen is rung one's surface**, so this is `graphics_contract`'s geometry rather
/// than a second declaration of it: the compositor composites into the very frames `painter` used to
/// paint, and flushes them through the same `FLUSH(rect)` the GPU driver already honours.
pub const SCREEN_W: u32 = graphics_contract::WIDTH;
/// The screen's height. See [`SCREEN_W`].
pub const SCREEN_H: u32 = graphics_contract::HEIGHT;
/// Pixels in the screen.
pub const SCREEN_PIXELS: usize = graphics_contract::PIXELS;

/// The most windows a scene may hold. Fixed because the compositor has no allocator and builds its
/// window table as an array; four is what [`SCENE`] needs and what the kernel wires.
pub const MAX_WINDOWS: usize = 4;

// ================================================================================================
// Rectangles: the arithmetic that compositors get wrong.
// ================================================================================================

/// A rectangle, with a **signed** origin.
///
/// Signed because a window may hang off the left or top edge of the screen, and that case is where
/// clipping bugs live: the naive `u32` version cannot represent it, so it gets "fixed" by moving the
/// window instead, and the off-by-one is never found. Width and height stay unsigned; a negative
/// extent is not a rectangle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

/// The canonical empty rectangle. Every operation that produces nothing produces *this*, so an empty
/// result compares equal regardless of where the emptiness came from.
pub const EMPTY: Rect = Rect {
    x: 0,
    y: 0,
    w: 0,
    h: 0,
};

impl Rect {
    pub const fn new(x: i32, y: i32, w: u32, h: u32) -> Rect {
        Rect { x, y, w, h }
    }

    /// A rectangle covering the whole screen.
    pub const fn screen() -> Rect {
        Rect::new(0, 0, SCREEN_W, SCREEN_H)
    }

    pub const fn is_empty(&self) -> bool {
        self.w == 0 || self.h == 0
    }

    /// The first column past the rectangle. **Half-open**: a rectangle covers `x .. right()`, and
    /// two rectangles that share this edge do not overlap.
    ///
    /// **Saturating, because a client chooses these numbers** (milestone 719 (compositor
    /// confinement claim 25)): a damage rectangle reaches this before any clip. A plain `+` panics
    /// in a debug build, killing the compositor and every client behind its doorbell; in release it
    /// wraps. Saturating keeps the sum on the number line, so every caller's clip sees a rectangle
    /// that really is as wide as the client claimed rather than one the wrap folded back.
    pub const fn right(&self) -> i32 {
        self.x.saturating_add(Self::extent(self.w))
    }

    /// The first row past the rectangle. See [`right`](Self::right).
    pub const fn bottom(&self) -> i32 {
        self.y.saturating_add(Self::extent(self.h))
    }

    /// An unsigned extent as the signed number it is added to an origin as, clamped rather than
    /// wrapped (see [`right`](Self::right)).
    const fn extent(n: u32) -> i32 {
        if n > i32::MAX as u32 {
            i32::MAX
        } else {
            n as i32
        }
    }

    pub const fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x && x < self.right() && y >= self.y && y < self.bottom()
    }

    pub const fn translate(&self, dx: i32, dy: i32) -> Rect {
        Rect::new(self.x + dx, self.y + dy, self.w, self.h)
    }

    /// The overlap of two rectangles, or [`EMPTY`]. This is the whole of clipping: a damage rectangle
    /// clipped to a surface, a surface clipped to the screen, and a repaint clipped to the damage are
    /// all this one function.
    pub const fn intersect(&self, o: &Rect) -> Rect {
        if self.is_empty() || o.is_empty() {
            return EMPTY;
        }
        let x = if self.x > o.x { self.x } else { o.x };
        let y = if self.y > o.y { self.y } else { o.y };
        let r = if self.right() < o.right() {
            self.right()
        } else {
            o.right()
        };
        let b = if self.bottom() < o.bottom() {
            self.bottom()
        } else {
            o.bottom()
        };
        if r <= x || b <= y {
            return EMPTY;
        }
        Rect::new(x, y, (r - x) as u32, (b - y) as u32)
    }

    /// The bounding box of two rectangles, ignoring empties. The compositor accumulates one damage
    /// rectangle per frame this way: a bounding box costs a few extra pixels of copying and saves
    /// keeping a region list, which is the right trade at this scale and the wrong one at a
    /// desktop's (notes/compositor.md records it as a limit).
    pub const fn union(&self, o: &Rect) -> Rect {
        if self.is_empty() {
            return *o;
        }
        if o.is_empty() {
            return *self;
        }
        let x = if self.x < o.x { self.x } else { o.x };
        let y = if self.y < o.y { self.y } else { o.y };
        let r = if self.right() > o.right() {
            self.right()
        } else {
            o.right()
        };
        let b = if self.bottom() > o.bottom() {
            self.bottom()
        } else {
            o.bottom()
        };
        Rect::new(x, y, (r - x) as u32, (b - y) as u32)
    }

    pub const fn area(&self) -> u64 {
        (self.w as u64) * (self.h as u64)
    }

    /// This rectangle as unsigned screen coordinates, for [`graphics_contract::rect`]. `None` if it is empty
    /// or not wholly on the screen, because the GPU driver **refuses** an out-of-surface flush
    /// rather than clamping it (rung one's rule), so handing it one is a bug we should not paper over.
    pub const fn as_flush_rect(&self) -> Option<(u32, u32, u32, u32)> {
        if self.is_empty() || self.x < 0 || self.y < 0 {
            return None;
        }
        if self.right() > SCREEN_W as i32 || self.bottom() > SCREEN_H as i32 {
            return None;
        }
        Some((self.x as u32, self.y as u32, self.w, self.h))
    }
}

// ================================================================================================
// The scene: which windows exist, and in what order they stack.
// ================================================================================================

/// A window: a client's surface, and where it sits on the screen.
///
/// The surface's own coordinate system starts at `(0, 0)` and is `w` by `h`; `origin` is where that
/// corner lands on the screen and may be negative. A client is never told its origin, and does not
/// need it: it draws in its own coordinates and the compositor places it. That is not a convenience,
/// it is the isolation: a client that does not know where it is on screen cannot reason about what is
/// underneath it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    pub origin_x: i32,
    pub origin_y: i32,
    pub w: u32,
    pub h: u32,
}

impl Window {
    pub const fn new(origin_x: i32, origin_y: i32, w: u32, h: u32) -> Window {
        Window {
            origin_x,
            origin_y,
            w,
            h,
        }
    }

    /// Where this window sits on the screen, in screen coordinates.
    pub const fn rect(&self) -> Rect {
        Rect::new(self.origin_x, self.origin_y, self.w, self.h)
    }

    /// The surface's own bounds, in surface coordinates.
    pub const fn bounds(&self) -> Rect {
        Rect::new(0, 0, self.w, self.h)
    }

    /// Bytes per row of this surface. Tightly packed, like the scanout: no padding, so a stride bug
    /// is a wrong picture rather than a hidden slack byte.
    pub const fn stride(&self) -> u32 {
        self.w * 4
    }

    pub const fn pixels(&self) -> usize {
        (self.w as usize) * (self.h as usize)
    }

    /// How many 4 KiB frames this surface's pixels need. Mapping is frame-granular, so a surface that
    /// does not fill its last frame leaves the tail mapped and unused; that tail is inside the
    /// client's own grant, so it is slack, not a hole.
    pub const fn page_frames(&self) -> u32 {
        (self.w * self.h * 4).div_ceil(4096)
    }
}

/// **The scene, bottom window first.** A window's index in this array is both its id and its
/// stacking order: index 0 is at the back, the last is in front.
///
/// Fixed at compile time, so the composed screen is a value every party can predict and check pixel
/// for pixel. The geometry makes a wrong composite fail: three sizes, overlap, and a window hanging
/// off the left edge. Each surface is at most four page frames, because the kernel maps a surface
/// one `Spawn::maps` entry a page (`kernel/src/user/compositor_service.rs`'s `MAX_SURFACE_PAGES`,
/// asserted at build time). See notes/compositor/arithmetic.md for why each choice.
pub const SCENE: &[Window] = &[
    Window::new(16, 26, 72, 52),
    Window::new(40, 50, 71, 56),
    Window::new(-8, 60, 30, 80),
];

// ================================================================================================
// The pictures: what a client paints, and what the screen must then hold.
// ================================================================================================

/// The most bytes any surface in [`SCENE`] takes, and so the most a client can be asked to paint. A
/// client checks its published geometry against this before it paints: a geometry larger than the
/// grant it was mapped would make it fault on its own surface, and a client should refuse a control
/// page it cannot believe rather than discover that as a crash.
pub const MAX_SURFACE_BYTES: u32 = {
    let mut m = 0;
    let mut i = 0;
    while i < SCENE.len() {
        let b = SCENE[i].w * SCENE[i].h * 4;
        if b > m {
            m = b;
        }
        i += 1;
    }
    m
};

/// **The small damage rectangle**, in surface coordinates, that the damage test's client commits for
/// its second frame. Here rather than in the client because three parties must agree on it: the
/// client that commits it, the kernel test that predicts the flush rectangle it produces, and the host
/// tests that check the arithmetic without a machine.
///
/// Deliberately not aligned to anything and not at the origin: an odd offset and an odd size are what
/// catch a compositor that rounded a damage rectangle out to a tile or a row.
pub const SMALL_DAMAGE: Rect = Rect::new(4, 5, 9, 7);

/// **What window `id` paints at its own `(x, y)`.** A per-coordinate, per-window function, for the
/// reason `graphics_contract::pixel` is one: a fill proves nothing. A blank surface, a surface painted by the
/// wrong client, a surface blitted at the wrong offset, and a transposed surface all have to be
/// visibly wrong, and they are, because no two windows agree on any pixel and no window is constant.
///
/// The red channel is always **odd**, and [`background_pixel`]'s red is always **even**, so no window
/// pixel can be mistaken for background. That is asserted exhaustively over the whole screen in this
/// crate's tests rather than argued here.
pub const fn window_pixel(id: u32, x: u32, y: u32) -> u32 {
    let k = id.wrapping_mul(53).wrapping_add(11);
    let r = (x.wrapping_mul(3).wrapping_add(k) | 1) & 0xff;
    let g = (y
        .wrapping_mul(7)
        .wrapping_add(x >> 1)
        .wrapping_add(id.wrapping_mul(29)))
        & 0xff;
    let b = ((x ^ (y << 1))
        .wrapping_mul(id.wrapping_mul(2).wrapping_add(13))
        .wrapping_add(id.wrapping_mul(7)))
        & 0xff;
    (r << 16) | (g << 8) | b
}

/// **What the screen holds where no window is.** Coarse 8x8 blocks plus a gradient, so a compositor
/// that forgot to paint the background inside a damage rectangle leaves visibly stale pixels rather
/// than plausible ones. Red is always even (see [`window_pixel`]).
pub const fn background_pixel(x: u32, y: u32) -> u32 {
    let block = ((x >> 3) ^ (y >> 3)) & 1;
    let r = (block.wrapping_mul(0x30)) & 0xfe;
    let g = (0x10 + (y >> 1)) & 0xff;
    let b = (0x08 + (x >> 2)) & 0xff;
    (r << 16) | (g << 8) | b
}

/// **What screen pixel `(x, y)` must be** when the first `n` windows of [`SCENE`] are live: the
/// topmost window covering it, or the background.
///
/// The *independent* definition of the composed screen: [`composite`] paints up the stack and this
/// looks down it, so they agree only if both are right. The kernel test, the capturing client and
/// the host-side `screendump` check all compare against this one.
///
/// # Examples
///
/// ```
/// use compositor::{Rect, SCENE, SCREEN_PIXELS, SCREEN_W, background_pixel, composite};
/// use compositor::{expected_screen_pixel, window_pixel};
///
/// let surfaces: Vec<Vec<u32>> = SCENE
///     .iter()
///     .enumerate()
///     .map(|(id, w)| {
///         (0..w.pixels())
///             .map(|i| {
///                 let (sx, sy) = ((i % w.w as usize) as u32, (i / w.w as usize) as u32);
///                 window_pixel(id as u32, sx, sy)
///             })
///             .collect()
///     })
///     .collect();
/// let refs: Vec<&[u32]> = surfaces.iter().map(|s| s.as_slice()).collect();
///
/// let mut screen = vec![0u32; SCREEN_PIXELS];
/// composite(&mut screen, &refs, SCENE.len(), Rect::screen());
///
/// for i in 0..SCREEN_PIXELS {
///     let (x, y) = ((i % SCREEN_W as usize) as u32, (i / SCREEN_W as usize) as u32);
///     assert_eq!(screen[i], expected_screen_pixel(SCENE.len(), x, y), "pixel ({x}, {y})");
/// }
///
/// // Window 1 covers part of window 0, so the topmost one wins.
/// assert_eq!(expected_screen_pixel(2, 45, 56), window_pixel(1, 5, 6));
/// assert_eq!(expected_screen_pixel(1, 45, 56), window_pixel(0, 29, 30));
/// assert_eq!(expected_screen_pixel(0, 45, 56), background_pixel(45, 56));
/// ```
pub fn expected_screen_pixel(n: usize, x: u32, y: u32) -> u32 {
    expected_screen_pixel_with(n, x, y, |i, sx, sy| window_pixel(i as u32, sx, sy))
}

/// **What screen pixel `(x, y)` must be when the windows draw something other than
/// [`window_pixel`]**: the same topmost-window-wins lookup, with the per-window picture supplied by
/// the caller as `content(id, surface_x, surface_y)`.
///
/// For milestone 29 (a display terminal)'s windows, whose content is a character grid. The
/// composition rule is the compositor's and the content the client's, which is why `compositor`
/// does not depend on `video_terminal`.
pub fn expected_screen_pixel_with(
    n: usize,
    x: u32,
    y: u32,
    content: impl Fn(usize, u32, u32) -> u32,
) -> u32 {
    let mut i = n.min(SCENE.len());
    while i > 0 {
        i -= 1;
        let win = &SCENE[i];
        if win.rect().contains(x as i32, y as i32) {
            return content(
                i,
                (x as i32 - win.origin_x) as u32,
                (y as i32 - win.origin_y) as u32,
            );
        }
    }
    background_pixel(x, y)
}

/// The digest of the whole composed screen, using rung one's position-sensitive checksum so the
/// guest-side witnesses and the kernel speak one language about "is the screen right".
pub fn expected_screen_checksum(n: usize) -> u64 {
    graphics_contract::checksum(|i| {
        let x = (i % SCREEN_W as usize) as u32;
        let y = (i / SCREEN_W as usize) as u32;
        expected_screen_pixel(n, x, y)
    })
}

// ================================================================================================
// Composition.
// ================================================================================================

/// Map a client's damage rectangle (in **its own** surface coordinates) to the screen: clip it to the
/// surface, place it, then clip it to the screen. Returns [`EMPTY`] if nothing of it is visible.
///
/// Both clips are needed and each catches a different bug. Without the first, a client that reports
/// damage past its own edge makes the compositor read pixels it does not own. Without the second, a
/// window that hangs off the screen makes the compositor write outside the framebuffer. The client's
/// number is *untrusted input* to the first clip, which is why it is a clip and not an assertion.
///
/// # Examples
///
/// ```
/// use compositor::{EMPTY, Rect, SCREEN_H, SCREEN_W, Window, damage_to_screen};
///
/// // A window at (8, 8), 64 by 32.
/// let win = Window::new(8, 8, 64, 32);
///
/// // Ordinary damage, placed on the screen.
/// assert_eq!(damage_to_screen(&win, Rect::new(4, 5, 9, 7)), Rect::new(12, 13, 9, 7));
///
/// // Damage past the client's OWN edge. Without the first clip the compositor would read pixels the
/// // client does not own, so it is clipped to the surface before it is placed.
/// let clipped = damage_to_screen(&win, Rect::new(60, 30, 100, 100));
/// assert_eq!(clipped, Rect::new(68, 38, 4, 2));
///
/// // A window hanging off the left edge, with damage in the part that is off-screen. Without the
/// // second clip the compositor would write outside the framebuffer.
/// let hanging = Window::new(-8, 30, 40, 40);
/// assert_eq!(damage_to_screen(&hanging, Rect::new(0, 0, 4, 4)), EMPTY);
/// // The part that IS on screen survives, starting at x = 0.
/// assert_eq!(damage_to_screen(&hanging, Rect::new(0, 0, 16, 4)), Rect::new(0, 30, 8, 4));
///
/// // Nothing that comes out of this function can leave the screen.
/// let out = damage_to_screen(&hanging, Rect::new(0, 0, 999, 999));
/// assert!(out.right() <= SCREEN_W as i32 && out.bottom() <= SCREEN_H as i32);
/// ```
pub fn damage_to_screen(win: &Window, damage: Rect) -> Rect {
    let inside = damage.intersect(&win.bounds());
    inside
        .translate(win.origin_x, win.origin_y)
        .intersect(&Rect::screen())
}

/// **One client's commit, as the compositor reads it**: the damage rectangle exactly as the client
/// wrote it into its control page (four untrusted words), mapped to what the screen must redraw, and
/// the status word the compositor writes back into the same page.
///
/// The rectangle is clipped to the client's surface, and the status says so
/// ([`proto::ctl::STATUS_CLIPPED`]). The worst a lie can do is make the compositor re-copy the
/// liar's own pixels: the returned damage is always inside `win` on the screen.
///
/// Name: provisional, minted 2026-10-04 (UTC) by `lane/fuzz-service-handlers`, which lifted it out
/// of the compositor binary's frame loop so the host fuzz target `compositor_session` runs the
/// decode the binary runs.
pub fn commit_damage(win: &Window, x: u32, y: u32, w: u32, h: u32) -> (Rect, u32) {
    let asked = Rect::new(x as i32, y as i32, w, h);
    let inside = asked.intersect(&win.bounds());
    let status = if inside == asked {
        proto::ctl::STATUS_OK
    } else {
        proto::ctl::STATUS_CLIPPED
    };
    (damage_to_screen(win, asked), status)
}

/// **Composite the first `n` windows into `screen`, touching only `damage`.**
///
/// `screen` is `SCREEN_W * SCREEN_H` pixels; `sources[i]` is window `i`'s surface, at least
/// `SCENE[i].pixels()` long, or empty for a window that has never committed (drawn as nothing).
/// `damage` is in screen coordinates and clipped here, so a caller cannot make this write outside
/// the framebuffer. Background first, then each opaque window in stacking order; every pixel
/// outside `damage` is left as it was. See notes/compositor/arithmetic.md.
pub fn composite(screen: &mut [u32], sources: &[&[u32]], n: usize, damage: Rect) {
    let damage = damage.intersect(&Rect::screen());
    if damage.is_empty() {
        return;
    }

    for y in damage.y..damage.bottom() {
        let row = (y as u32 as usize) * SCREEN_W as usize;
        for x in damage.x..damage.right() {
            screen[row + x as usize] = background_pixel(x as u32, y as u32);
        }
    }

    for i in 0..n.min(SCENE.len()).min(sources.len()) {
        let win = &SCENE[i];
        let vis = win.rect().intersect(&damage);
        if vis.is_empty() {
            continue;
        }
        let src = sources[i];
        if src.is_empty() {
            continue; // never committed: not on screen
        }
        for y in vis.y..vis.bottom() {
            let sy = (y - win.origin_y) as u32 as usize;
            let drow = (y as u32 as usize) * SCREEN_W as usize;
            let srow = sy * win.w as usize;
            for x in vis.x..vis.right() {
                let sx = (x - win.origin_x) as u32 as usize;
                screen[drow + x as usize] = src[srow + sx];
            }
        }
    }
}

// ================================================================================================
// The protocol.
// ================================================================================================

/// The IPC the compositor serves, and the shared-memory layouts that carry everything the IPC
/// deliberately does not.
///
/// Request words are packed the way `graphics_contract` and `line_editor::proto` pack theirs (opcode in bits
/// 63:56), and the helpers are re-exported rather than redefined so all three contracts read alike.
pub mod proto {
    pub use graphics_contract::{operand, operation, req};

    /// `CALL(req(HELLO, 0), 0)` -> `(0, 0)`. "I have started." Carries nothing, and its reply
    /// carries nothing: the client's geometry is in its own control page, which the compositor has
    /// published by the time it can answer this at all. The rendezvous *is* the synchronization, so
    /// no client has to poll for a valid control page.
    pub const HELLO: u64 = 1;

    /// `CALL(req(COMMIT, 0), 0)` -> `(0, 0)`. "Look at the surfaces." The compositor reads every
    /// client's control page, composites whatever changed, flushes the damage to the display, and
    /// replies. Content-free on purpose: a shared endpoint has no sender identity, so a request that
    /// named a surface would let any client speak for any other (see the crate note).
    ///
    /// Because the caller is blocked in `CALL` for the whole of composition, a client cannot race the
    /// compositor's read of its own pixels. That is flow control by rendezvous, not by trust.
    pub const COMMIT: u64 = 2;

    // **There is no capture verb, and no enumerate verb**, and their absence is the design rather
    // than a gap. A `CAPTURE` opcode was drafted and dropped: it would have been safe (the pixels
    // would land in a mapping the caller may not hold, so a forging client would learn nothing), but
    // it would also have been a verb that does nothing for anyone who does not already hold the
    // mapping that *is* the authority. Reading the screen is a read-only mapping; enumerating windows
    // is a read-only page. A client holding neither has nothing to call and nowhere to look, which is
    // a stronger statement than a guarded verb and a smaller protocol. The one thing it costs is a
    // consistent snapshot (a live mapping can be read mid-composite); notes/compositor.md records that
    // as a limit and the served-copy path as what would fix it.

    /// The reply to a word whose opcode the compositor does not implement. Negative-is-an-error, the
    /// convention `filesystem_contract` set and `graphics_contract` follows.
    pub const EBADOP: i64 = -22;

    /// **A client's control page**: the per-client channel that carries everything the shared
    /// doorbell cannot. Byte offsets into the one 4 KiB frame the compositor shares with exactly one
    /// client, in the *client's* address space at `CTL_VA` and in the compositor's at its own.
    ///
    /// Direction matters and is stated per field, because the trust flows one way in each: what the
    /// compositor writes is authoritative (it published the surface), and what the client writes is
    /// untrusted input to be clipped.
    pub mod ctl {
        /// u32, compositor -> client. [`MAGIC_VALUE`] once the page is valid.
        pub const MAGIC: u64 = 0x00;
        /// u32, compositor -> client: which window this client is. **A label, not a name**: no
        /// request anywhere in this protocol takes a window id from a client, so a client that lies
        /// about its id can only paint the wrong pattern into its own surface.
        pub const ID: u64 = 0x04;
        /// u32, compositor -> client: the surface's width in pixels. A client is *told* its geometry
        /// rather than assuming it, which is the runtime half of the contract rung one added `INFO`
        /// for; at rung two the compositor really does choose it.
        pub const WIDTH: u64 = 0x08;
        /// u32, compositor -> client: the surface's height in pixels.
        pub const HEIGHT: u64 = 0x0c;
        /// u32, compositor -> client: bytes per row.
        pub const STRIDE: u64 = 0x10;
        /// u32, compositor -> client: [`STATUS_OK`] or [`STATUS_CLIPPED`] for the last commit.
        pub const STATUS: u64 = 0x14;
        /// u32, client -> compositor: bumped when the client has painted something new. The
        /// compositor composites a surface only when this changes, so an idle client costs nothing.
        pub const SEQ: u64 = 0x18;
        /// u32, compositor -> client: the [`SEQ`] value the compositor has composited.
        pub const ACKED: u64 = 0x1c;
        /// i32 x u32 x u32 x u32, client -> compositor: the damaged rectangle in **surface**
        /// coordinates. Untrusted: clipped to the surface, never believed.
        pub const DAMAGE_X: u64 = 0x20;
        pub const DAMAGE_Y: u64 = 0x24;
        pub const DAMAGE_W: u64 = 0x28;
        pub const DAMAGE_H: u64 = 0x2c;

        /// "CMP1". A client that reads anything else has not been published yet, which cannot happen
        /// through the `HELLO` rendezvous but is checked anyway: the alternative to a magic number is
        /// painting into a surface whose size you guessed.
        pub const MAGIC_VALUE: u32 = 0x434d_5031;
        pub const STATUS_OK: u32 = 0;
        /// The last commit's damage rectangle was not wholly inside the surface, so it was clipped.
        ///
        /// Rung one **refuses** an out-of-surface rectangle rather than clamping it, because there the
        /// caller is the only party who can tell a coordinate bug from intent. Here the compositor
        /// scans every surface on one identity-free doorbell, so it cannot attribute a bad rectangle
        /// to a caller in order to refuse *it*; and the worst a lie can do is make the compositor
        /// re-copy the liar's own pixels. So it clips, and says so **in the liar's own control page**,
        /// which is per-client feedback through the only channel that can carry it.
        pub const STATUS_CLIPPED: u32 = 1;
    }

    /// **The window list**: one page the compositor publishes and maps *read-only* into whoever was
    /// granted the enumeration authority.
    ///
    /// There is deliberately **no enumerate verb**. Window enumeration is a page, holding it is the
    /// grant, and a client that was not granted it has no mapping there at all: its attempt to look
    /// is a fault at an unmapped address, which is "you hold no such capability" in the address
    /// space's dialect rather than a permission error from a server that checked a list. Screen
    /// sharing is the same act with the screen's own frames, mapped read-only into a third party.
    pub mod wlist {
        /// u32: how many windows the list holds.
        pub const COUNT: u64 = 0x00;
        /// u32: which window currently has input focus. Live: it changes when the compositor moves
        /// focus, which is what lets a holder (and the kernel test) witness a focus decision it did
        /// not make.
        pub const FOCUSED: u64 = 0x04;
        /// The first record. Each is four `i32`/`u32` (x, y, w, h) in stacking order, bottom first.
        pub const RECORDS: u64 = 0x10;
        /// Bytes per record.
        pub const RECORD_STRIDE: u64 = 0x10;
    }

    /// **The input ring**: one page the input source shares with the compositor, and nobody else.
    ///
    /// Keystrokes arrive here rather than in message words because the doorbell is shared, so a
    /// keystroke in a message would be forgeable. The compositor forwards them to the focused
    /// client as `line_editor::proto::OPERATION_BYTES`, the terminal contract's driver half
    /// verbatim (notes/terminal-contract.md), so a terminal needs no change to be a client.
    pub mod ring {
        /// u32, compositor: bytes consumed.
        pub const HEAD: u64 = 0x00;
        /// u32, source: bytes produced. The source writes bytes, then this, then rings the doorbell.
        pub const TAIL: u64 = 0x04;
        /// The byte buffer.
        pub const BYTES: u64 = 0x08;
        /// How many bytes the ring holds. Small: this is a keyboard, not a pipe.
        pub const CAPACITY: u32 = 256;
    }

    /// The byte that moves focus to the next window: ASCII TAB. A policy decision, made in
    /// userspace, in the compositor, where window policy belongs; the kernel never sees it.
    pub const FOCUS_NEXT: u8 = 0x09;
}

/// What each process reports to whoever spawned it. Status, not contract: a client cannot ask for any
/// of this, and the kernel-side test is the only party that compares the accounts.
pub mod status {
    /// The compositor is up: `send(REPORT, COMP_UP, window count, focused)`. Its control pages are
    /// published and it is serving the doorbell.
    /// `COMP_UP` is the compositor's **only** report, ever, and that is deliberate: a status `SEND`
    /// is a rendezvous, so a compositor that narrated each frame would block until its spawner
    /// listened, and a spawner that stopped listening would deadlock the clients behind it. What was
    /// flushed is observable where it belongs, at the display endpoint (kernel/src/user.rs's display
    /// stand-in reads the rectangle straight off the `FLUSH`).
    pub const COMP_UP: u64 = 0xC33_0001;

    /// A client painted and committed: `send(REPORT, WIN_PAINTED, digest, seq)`, where `digest` is
    /// [`super::surface_checksum`] over its own surface read back after the commit.
    pub const WIN_PAINTED: u64 = 0xC33_0011;
    /// A client asked for something it holds no capability for and was refused:
    /// `send(REPORT, WIN_REFUSED, errno, what)`. `errno` is the kernel's, and the whole point is
    /// which one: `NoSuchSlot` (-1), "there is nothing there", not a permission error.
    pub const WIN_REFUSED: u64 = 0xC33_0012;
    /// A capture-capable client read the screen: `send(REPORT, WIN_CAPTURED, digest, focused | count << 32)`.
    pub const WIN_CAPTURED: u64 = 0xC33_0013;
    /// A client re-read its own surface after a neighbour attacked it:
    /// `send(REPORT, WIN_INTACT, digest, 0)`. The witness that matters for isolation.
    pub const WIN_INTACT: u64 = 0xC33_0014;
    /// A client received input: `send(REPORT, WIN_INPUT, byte, count so far)`.
    pub const WIN_INPUT: u64 = 0xC33_0015;
    /// A client is about to touch memory it was not granted: `send(REPORT, WIN_PROBING, va, 0)`.
    /// Sent *before* the access, because the access is expected to be fatal, and a test that saw no
    /// probe report would otherwise not know whether the probe happened at all.
    pub const WIN_PROBING: u64 = 0xC33_0016;
    /// A client survived an access it should not have: `send(REPORT, WIN_ESCAPED, va, value)`. If a
    /// test ever sees this word, the isolation this milestone exists to prove is broken.
    pub const WIN_ESCAPED: u64 = 0xC33_0017;
    /// A client committed every damage rectangle in its table of lies and the compositor answered
    /// each one: `send(REPORT, WIN_LIED, how many, 0)`. Milestone 719. Never sent if the compositor
    /// stopped answering, which is the whole observation.
    pub const WIN_LIED: u64 = 0xC33_0018;
    /// `send(REPORT, WIN_FORGED, how many rings answered, 0)`. The eighth outsider pass's
    /// control-page forgery: a client that forged its page's identity, geometry and bookkeeping
    /// and committed after each forge. Name: provisional.
    pub const WIN_FORGED: u64 = 0xC33_0019;
}

/// A position-sensitive digest of a `w` by `h` surface, `read(i)` giving pixel `i` in row-major
/// order. Rung one's checksum, reused so every witness in the system digests the same way.
pub fn surface_checksum(w: u32, h: u32, mut read: impl FnMut(usize) -> u32) -> u64 {
    let mut hsh: u64 = 0xcbf2_9ce4_8422_2325;
    for i in 0..(w as usize) * (h as usize) {
        for b in read(i).to_le_bytes() {
            hsh ^= b as u64;
            hsh = hsh.wrapping_mul(0x100_0000_01b3);
        }
    }
    hsh
}

/// The digest window `id` must report for a surface holding exactly its pattern.
pub fn expected_window_checksum(id: usize) -> u64 {
    let win = &SCENE[id];
    surface_checksum(win.w, win.h, |i| {
        let x = (i % win.w as usize) as u32;
        let y = (i / win.w as usize) as u32;
        window_pixel(id as u32, x, y)
    })
}

#[cfg(test)]
mod tests;
