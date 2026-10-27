use super::*;
extern crate std;
use std::vec;
use std::vec::Vec;

/// **A client's damage rectangle is any 16 bytes it likes** (milestone 719, confinement claim
/// 25's attack on the compositor's one untrusted input). Every combination of extreme origin and
/// extent must neither panic nor produce a rectangle outside the surface it was clipped to, for
/// every window in the scene. Before 719 `Rect::right` was `x + w as i32` and the very first
/// pair here, `(i32::MAX, 0, 1, 1)`, overflowed it.
#[test]
fn a_lying_damage_rectangle_neither_panics_nor_leaves_its_surface() {
    let origins = [i32::MIN, i32::MIN + 1, -1, 0, 1, i32::MAX - 1, i32::MAX];
    let extents = [
        0u32,
        1,
        2,
        i32::MAX as u32,
        i32::MAX as u32 + 1,
        u32::MAX - 1,
        u32::MAX,
    ];
    for win in SCENE {
        for &x in &origins {
            for &y in &origins {
                for &w in &extents {
                    for &h in &extents {
                        let asked = Rect::new(x, y, w, h);
                        let inside = asked.intersect(&win.bounds());
                        let b = win.bounds();
                        assert!(
                            inside.is_empty()
                                || (inside.x >= b.x
                                    && inside.y >= b.y
                                    && inside.right() <= b.right()
                                    && inside.bottom() <= b.bottom()),
                            "{asked:?} clipped to {inside:?}, outside the surface {b:?}",
                        );
                        let on_screen = damage_to_screen(win, asked);
                        let screen = Rect::screen();
                        assert!(
                            on_screen.is_empty()
                                || (on_screen.x >= screen.x
                                    && on_screen.y >= screen.y
                                    && on_screen.right() <= screen.right()
                                    && on_screen.bottom() <= screen.bottom()
                                    && win.rect().intersect(&on_screen) == on_screen),
                            "{asked:?} became {on_screen:?}, outside the window or the screen",
                        );
                    }
                }
            }
        }
    }
}

/// An extent past `i32::MAX` clamps to the far edge of the number line rather than wrapping to
/// a negative width, so a client's oversized rectangle reads as the huge one it claimed.
#[test]
fn an_extent_past_i32_max_clamps_rather_than_wraps() {
    assert_eq!(Rect::new(0, 0, u32::MAX, 1).right(), i32::MAX);
    assert_eq!(Rect::new(0, 0, 1, i32::MAX as u32 + 1).bottom(), i32::MAX);
}

/// **A commit inside the surface is acknowledged plainly, and one past it is reported clipped**,
/// the per-client feedback the status word exists to carry.
#[test]
fn a_commit_says_whether_its_damage_was_clipped() {
    let win = &SCENE[0];
    assert_eq!(
        commit_damage(win, 0, 0, win.w, win.h).1,
        proto::ctl::STATUS_OK
    );
    assert_eq!(
        commit_damage(win, 0, 0, win.w + 1, win.h).1,
        proto::ctl::STATUS_CLIPPED
    );
}

/// `area` on a rectangle that is neither square nor 2 by 2. Every fixture here was square, and
/// `w + h` equals `w * h` at 2 by 2, so `* -> +` survived from the 2026-08-03 run to the
/// 2026-09-19 census. See notes/mutation-testing.md.
#[test]
fn area_multiplies_width_by_height() {
    assert_eq!(Rect::new(0, 0, 3, 5).area(), 15);
    assert_eq!(Rect::new(-4, 7, 1, 9).area(), 9);
    assert_eq!(EMPTY.area(), 0);
}

/// **Half-open intervals, or every clip is off by one.** Two rectangles sharing an edge do not
/// overlap; a rectangle one pixel further in does. This is the bug this whole crate exists to
/// keep out of the compositor, so it is the first test.
#[test]
fn a_shared_edge_is_not_an_overlap() {
    let a = Rect::new(0, 0, 10, 10);
    assert_eq!(
        a.intersect(&Rect::new(10, 0, 10, 10)),
        EMPTY,
        "rectangles that touch at x=10 must not overlap",
    );
    assert_eq!(
        a.intersect(&Rect::new(9, 0, 10, 10)),
        Rect::new(9, 0, 1, 10),
        "a one-pixel overlap must be found",
    );
    assert_eq!(a.intersect(&Rect::new(0, 10, 10, 10)), EMPTY);
    assert_eq!(
        a.intersect(&Rect::new(0, 9, 10, 10)),
        Rect::new(0, 9, 10, 1)
    );
    assert_eq!(a.intersect(&Rect::new(-5, -5, 5, 5)), EMPTY, "up and left");
    assert_eq!(
        a.intersect(&Rect::new(-5, -5, 6, 6)),
        Rect::new(0, 0, 1, 1),
        "a negative origin still intersects correctly",
    );
    assert_eq!(a.intersect(&EMPTY), EMPTY, "nothing overlaps nothing");
    assert_eq!(EMPTY.intersect(&a), EMPTY);
    // Containment both ways round.
    let inner = Rect::new(2, 3, 4, 5);
    assert_eq!(a.intersect(&inner), inner);
    assert_eq!(inner.intersect(&a), inner);
}

/// `contains` agrees with `intersect` on every pixel of a small area, including the edges. Two
/// independent phrasings of "is this pixel in this rectangle" that must not disagree, since
/// `expected_screen_pixel` uses one and `composite` uses the other.
#[test]
fn contains_and_intersect_agree_pixel_by_pixel() {
    let r = Rect::new(-3, 2, 7, 4);
    for y in -6..12 {
        for x in -6..12 {
            let by_contains = r.contains(x, y);
            let by_intersect = !r.intersect(&Rect::new(x, y, 1, 1)).is_empty();
            assert_eq!(by_contains, by_intersect, "disagreement at ({x},{y})");
        }
    }
}

/// A union is a bounding box, and an empty rectangle contributes nothing. The compositor unions
/// the frame's damage this way, so a union that swallowed an empty into `(0,0)` would flush the
/// top-left corner of the screen on every frame for no reason.
#[test]
fn a_union_ignores_empties() {
    let a = Rect::new(4, 4, 2, 2);
    let b = Rect::new(10, 1, 1, 20);
    assert_eq!(a.union(&EMPTY), a);
    assert_eq!(EMPTY.union(&a), a);
    assert_eq!(EMPTY.union(&EMPTY), EMPTY);
    assert_eq!(a.union(&b), Rect::new(4, 1, 7, 20));
    assert_eq!(a.union(&b), b.union(&a), "union is symmetric");
}

/// **A damage rectangle is clipped twice, and each clip catches a different lie.** A client that
/// claims damage past its own edge must not make the compositor read another surface's pixels; a
/// window that hangs off the screen must not make it write outside the framebuffer.
#[test]
fn damage_is_clipped_to_the_surface_and_then_to_the_screen() {
    // A window wholly on screen: an honest rectangle passes through, placed.
    let w = Window::new(8, 8, 64, 32);
    assert_eq!(
        damage_to_screen(&w, Rect::new(2, 3, 4, 5)),
        Rect::new(10, 11, 4, 5),
    );
    // A client claiming the whole world gets its own surface, and no more.
    assert_eq!(
        damage_to_screen(&w, Rect::new(0, 0, 10_000, 10_000)),
        w.rect(),
    );
    // A client claiming a rectangle entirely outside its surface gets nothing.
    assert_eq!(damage_to_screen(&w, Rect::new(64, 0, 10, 10)), EMPTY);
    assert_eq!(damage_to_screen(&w, Rect::new(0, 32, 10, 10)), EMPTY);
    // Exactly the surface's last pixel: the boundary case a `<=` instead of a `<` would lose.
    assert_eq!(
        damage_to_screen(&w, Rect::new(63, 31, 1, 1)),
        Rect::new(71, 39, 1, 1),
    );

    // Off the left and top: the case a u32 origin cannot even represent.
    let nw = Window::new(-8, -6, 40, 40);
    assert_eq!(
        damage_to_screen(&nw, Rect::new(0, 0, 40, 40)),
        Rect::new(0, 0, 32, 34),
        "the parts off the left and top edges must be clipped away",
    );
    assert_eq!(
        damage_to_screen(&nw, Rect::new(0, 0, 4, 4)),
        EMPTY,
        "damage entirely off the screen is nothing to flush",
    );
    // Off the right and bottom.
    let se = Window::new(SCREEN_W as i32 - 8, SCREEN_H as i32 - 4, 32, 32);
    assert_eq!(
        damage_to_screen(&se, Rect::new(0, 0, 32, 32)),
        Rect::new(SCREEN_W as i32 - 8, SCREEN_H as i32 - 4, 8, 4),
    );
    // A window flush against the right edge is NOT clipped: the classic one-pixel error.
    let edge = Window::new(SCREEN_W as i32 - 16, 0, 16, 8);
    assert_eq!(
        damage_to_screen(&edge, Rect::new(0, 0, 16, 8)),
        edge.rect(),
        "a window ending exactly at the screen edge must survive whole",
    );
}

/// The scene is a fair test: every window is visible somewhere, some window is on top of another
/// somewhere, some window is clipped, and the background shows. A scene that failed any of these
/// would let a broken compositor pass.
#[test]
fn the_scene_exercises_overlap_clipping_and_background() {
    let n = SCENE.len();
    let mut visible = vec![0usize; n];
    let mut background = 0;
    for y in 0..SCREEN_H {
        for x in 0..SCREEN_W {
            let mut top = None;
            for (i, win) in SCENE.iter().enumerate() {
                if win.rect().contains(x as i32, y as i32) {
                    top = Some(i);
                }
            }
            match top {
                Some(i) => visible[i] += 1,
                None => background += 1,
            }
        }
    }
    for (i, &count) in visible.iter().enumerate() {
        assert!(count > 0, "window {i} is completely hidden");
    }
    assert!(background > 0, "no background is visible anywhere");

    // Overlap: at least two windows must cover some common pixel, or stacking order is untested.
    let mut overlapping = 0;
    for (i, a) in SCENE.iter().enumerate() {
        for b in &SCENE[i + 1..] {
            overlapping += a.rect().intersect(&b.rect()).area();
        }
    }
    assert!(
        overlapping > 0,
        "no two windows overlap: z-order is untested"
    );

    // Clipping: some window must extend past an edge.
    assert!(
        SCENE
            .iter()
            .any(|w| w.rect().intersect(&Rect::screen()) != w.rect()),
        "every window fits on screen: clipping is untested",
    );
    // And off the LEFT edge specifically. The scene's doc promises a negative origin,
    // which is the case this crate's signed `Rect` exists to represent; a scene whose
    // windows all start on screen would still pass the check above by clipping at the
    // bottom, leaving the negative-origin path exercised only by synthetic rectangles.
    assert!(
        SCENE.iter().any(|w| w.origin_x < 0),
        "no window hangs off the left edge: the live scene never clips a negative origin",
    );
    // And every surface must fit the frames the kernel grants it.
    for (i, win) in SCENE.iter().enumerate() {
        assert!(
            win.pixels() * 4 <= win.page_frames() as usize * 4096,
            "window {i} does not fit its frames",
        );
    }
}

/// **No pixel of the composed screen is ambiguous.** At every screen position, the pattern of the
/// window that should own it differs from every other window's pattern *at that same position* and
/// from the background. Without this, a z-order or offset bug could produce the right value by
/// coincidence and the pixel checks would not see it.
#[test]
fn every_screen_pixel_distinguishes_its_owner() {
    for y in 0..SCREEN_H {
        for x in 0..SCREEN_W {
            let want = expected_screen_pixel(SCENE.len(), x, y);
            let bg = background_pixel(x, y);
            let mut candidates: Vec<u32> = Vec::new();
            for (i, win) in SCENE.iter().enumerate() {
                if win.rect().contains(x as i32, y as i32) {
                    candidates.push(window_pixel(
                        i as u32,
                        (x as i32 - win.origin_x) as u32,
                        (y as i32 - win.origin_y) as u32,
                    ));
                }
            }
            if candidates.is_empty() {
                assert_eq!(want, bg, "({x},{y}) should be background");
                continue;
            }
            assert_ne!(want, bg, "window pixel ({x},{y}) looks like background");
            for (k, &c) in candidates.iter().enumerate() {
                if k + 1 == candidates.len() {
                    assert_eq!(want, c, "({x},{y}) is not the topmost window's pixel");
                } else {
                    assert_ne!(
                        want, c,
                        "({x},{y}): a lower window's pixel is indistinguishable from the top's",
                    );
                }
            }
        }
    }
}

/// A window's pattern is not a fill, is not symmetric, and is nobody else's. The `graphics_contract`
/// pattern tests' properties, per window.
#[test]
fn a_window_pattern_cannot_be_produced_by_accident() {
    for id in 0..MAX_WINDOWS as u32 {
        let first = window_pixel(id, 0, 0);
        assert!(
            (0..64).any(|y| (0..64).any(|x| window_pixel(id, x, y) != first)),
            "window {id}'s pattern is a solid fill",
        );
        for x in 0..63 {
            assert_ne!(
                window_pixel(id, x, 5),
                window_pixel(id, x + 1, 5),
                "window {id} repeats along a row at x={x}",
            );
        }
        for y in 0..63 {
            assert_ne!(
                window_pixel(id, 7, y),
                window_pixel(id, 7, y + 1),
                "window {id} repeats along a column at y={y}",
            );
        }
        assert_eq!(window_pixel(id, 3, 9) >> 24, 0, "the X byte must be zero");
        // Red odd for windows, even for background: the invariant the ambiguity test leans on.
        assert_eq!((window_pixel(id, 11, 13) >> 16) & 1, 1);
    }
    for y in 0..32u32 {
        for x in 0..32u32 {
            assert_eq!(
                (background_pixel(x, y) >> 16) & 1,
                0,
                "background red is odd"
            );
        }
    }
}

/// Paint the surfaces, composite the whole screen, and it must equal the per-pixel definition.
/// Two algorithms, one answer: painter's-algorithm-with-clipping against topmost-window-lookup.
#[test]
fn compositing_the_whole_screen_matches_the_expected_picture() {
    let surfaces = painted_scene();
    let refs: Vec<&[u32]> = surfaces.iter().map(|s| s.as_slice()).collect();
    let mut screen = vec![0u32; SCREEN_PIXELS];
    composite(&mut screen, &refs, SCENE.len(), Rect::screen());
    for y in 0..SCREEN_H {
        for x in 0..SCREEN_W {
            assert_eq!(
                screen[(y * SCREEN_W + x) as usize],
                expected_screen_pixel(SCENE.len(), x, y),
                "composed screen differs at ({x},{y})",
            );
        }
    }
    assert_eq!(
        graphics_contract::checksum(|i| screen[i]),
        expected_screen_checksum(SCENE.len()),
        "the digest the guest reports and the picture the host checks must agree",
    );
}

/// **A damaged composite touches the damage and nothing else.** Poison the screen, composite one
/// window's damage rectangle, and every pixel outside it must still hold the poison while every
/// pixel inside holds the composed picture. This is "a one-window redraw does not cost a full
/// screen", proved on the host in microseconds; the kernel test proves the same thing end to end
/// by watching what the compositor flushes.
#[test]
fn a_damaged_composite_leaves_the_rest_of_the_screen_alone() {
    let surfaces = painted_scene();
    let refs: Vec<&[u32]> = surfaces.iter().map(|s| s.as_slice()).collect();
    const POISON: u32 = 0xDEAD_BEEF;
    let mut screen = vec![POISON; SCREEN_PIXELS];

    let damage = damage_to_screen(&SCENE[1], SMALL_DAMAGE);
    assert_eq!(damage, Rect::new(44, 55, 9, 7));
    assert!(
        damage.area() * 20 < Rect::screen().area(),
        "the 'small' damage rectangle is not small: this test proves nothing about cost",
    );
    composite(&mut screen, &refs, SCENE.len(), damage);

    // 698,880 pixels, each with a call into `expected_screen_pixel`. Natively that is
    // microseconds; under Miri's interpreter it measured **over 44 minutes and had not
    // finished**, which is more than the whole weekly job's budget for one test (milestone
    // 238). So it strides under `cfg(miri)`, the convention `glob` and `network_time_protocol` already use.
    //
    // **The stride never skips a row the damage rectangle touches**, which is the half of this
    // test that has something to prove: the damaged region is 9x7, so the composited-pixel
    // check stays exhaustive and only the poison check outside it is sampled. A stride that
    // thinned both halves would be sampling the claim rather than the work.
    let stride = if cfg!(miri) { 37 } else { 1 };
    let mut checked = 0usize;
    for y in 0..SCREEN_H {
        let in_damage_row = (y as i32) >= damage.y && (y as i32) < damage.bottom();
        if !in_damage_row && y % stride != 0 {
            continue;
        }
        for x in 0..SCREEN_W {
            let got = screen[(y * SCREEN_W + x) as usize];
            if damage.contains(x as i32, y as i32) {
                assert_eq!(
                    got,
                    expected_screen_pixel(SCENE.len(), x, y),
                    "inside the damage, ({x},{y}) was not composited",
                );
            } else {
                assert_eq!(got, POISON, "outside the damage, ({x},{y}) was overwritten");
            }
            checked += 1;
        }
    }
    // The completeness pin, the same one `glob` carries: a stride that silently stopped
    // covering the screen would otherwise still pass.
    assert_eq!(
        checked,
        if cfg!(miri) { 15_708 } else { SCREEN_PIXELS },
        "the sweep did not cover what it claims to",
    );
    // The damaged region is window 1's, and window 0 lies **underneath** it there, so the check
    // above is also a z-order check inside a damage rectangle: a compositor that painted the
    // windows in the wrong order, or stopped at the damaged one, would leave window 0's pixels
    // showing. Asserted rather than assumed, because if the scene ever changes shape this test
    // would quietly stop testing that.
    assert!(
        !SCENE[0].rect().intersect(&damage).is_empty(),
        "no window lies under the damaged one: the z-order half of this test is inert",
    );
}

/// **A window that has not committed is not on screen**, and the screen before any client has run
/// is exactly the background. This is the state the compositor's first paint produces, and the
/// state the kernel test checks the driver's own digest against, so it needs to be a defined
/// picture rather than whatever the surfaces happened to hold.
#[test]
fn an_uncommitted_window_is_not_drawn() {
    let mut screen = vec![0u32; SCREEN_PIXELS];
    let none: [&[u32]; 3] = [&[], &[], &[]];
    composite(&mut screen, &none, SCENE.len(), Rect::screen());
    for y in 0..SCREEN_H {
        for x in 0..SCREEN_W {
            assert_eq!(
                screen[(y * SCREEN_W + x) as usize],
                background_pixel(x, y),
                "({x},{y}) is not background before anyone committed",
            );
        }
    }
    assert_eq!(
        graphics_contract::checksum(|i| screen[i]),
        expected_screen_checksum(0),
        "the empty screen must be the n=0 expected picture",
    );

    // And one committed window among uncommitted ones draws only itself.
    let surfaces = painted_scene();
    let one: [&[u32]; 3] = [&[], surfaces[1].as_slice(), &[]];
    composite(&mut screen, &one, SCENE.len(), Rect::screen());
    for y in 0..SCREEN_H {
        for x in 0..SCREEN_W {
            let inside = SCENE[1].rect().contains(x as i32, y as i32);
            let want = if inside {
                window_pixel(
                    1,
                    (x as i32 - SCENE[1].origin_x) as u32,
                    (y as i32 - SCENE[1].origin_y) as u32,
                )
            } else {
                background_pixel(x, y)
            };
            assert_eq!(screen[(y * SCREEN_W + x) as usize], want, "at ({x},{y})");
        }
    }
}

/// A caller cannot make `composite` write outside the framebuffer, however absurd the rectangle.
/// The compositor clips its own damage before calling this, so this is the belt to that braces: a
/// bug there must not become a wild write here.
#[test]
fn composite_cannot_be_talked_outside_the_framebuffer() {
    let surfaces = painted_scene();
    let refs: Vec<&[u32]> = surfaces.iter().map(|s| s.as_slice()).collect();
    let mut screen = vec![0u32; SCREEN_PIXELS];
    for r in [
        Rect::new(-1000, -1000, 5000, 5000),
        Rect::new(SCREEN_W as i32, 0, 10, 10),
        Rect::new(0, SCREEN_H as i32, 10, 10),
        Rect::new(-10, -10, 5, 5),
        EMPTY,
    ] {
        composite(&mut screen, &refs, SCENE.len(), r); // must not panic or index out of range
    }
}

/// The flush rectangle handed to the GPU driver is always inside the screen, and an empty or
/// off-screen one is refused rather than turned into a bogus flush. Rung one refuses an
/// out-of-surface rectangle, so producing one would be a driver-visible bug.
#[test]
fn only_an_on_screen_rectangle_becomes_a_flush() {
    assert_eq!(
        Rect::screen().as_flush_rect(),
        Some((0, 0, SCREEN_W, SCREEN_H))
    );
    assert_eq!(Rect::new(3, 4, 5, 6).as_flush_rect(), Some((3, 4, 5, 6)));
    assert_eq!(EMPTY.as_flush_rect(), None);
    assert_eq!(Rect::new(-1, 0, 5, 5).as_flush_rect(), None);
    assert_eq!(Rect::new(0, -1, 5, 5).as_flush_rect(), None);
    assert_eq!(
        Rect::new(SCREEN_W as i32 - 4, 0, 5, 5).as_flush_rect(),
        None,
        "one pixel past the right edge is not a flush",
    );
    assert_eq!(
        Rect::new(SCREEN_W as i32 - 5, 0, 5, 5).as_flush_rect(),
        Some((SCREEN_W - 5, 0, 5, 5)),
        "exactly against the right edge is",
    );
    // And what the driver is handed round-trips through its own packing.
    let (x, y, w, h) = Rect::new(9, 8, 7, 6).as_flush_rect().unwrap();
    assert_eq!(
        graphics_contract::unrect(graphics_contract::rect(x, y, w, h)),
        (9, 8, 7, 6)
    );
    assert!(graphics_contract::rect_in_surface(x, y, w, h));
}

/// The control page and the window list fit in one frame each, with no field overlapping another.
/// Two processes read these offsets from different address spaces; an overlap would be a silent
/// corruption rather than a compile error.
#[test]
fn the_shared_page_layouts_are_consistent() {
    use proto::{ctl, ring, wlist};
    let fields = [
        ctl::MAGIC,
        ctl::ID,
        ctl::WIDTH,
        ctl::HEIGHT,
        ctl::STRIDE,
        ctl::STATUS,
        ctl::SEQ,
        ctl::ACKED,
        ctl::DAMAGE_X,
        ctl::DAMAGE_Y,
        ctl::DAMAGE_W,
        ctl::DAMAGE_H,
    ];
    for (i, &a) in fields.iter().enumerate() {
        assert_eq!(a % 4, 0, "field {i} is not 4-byte aligned");
        for &b in &fields[i + 1..] {
            assert_ne!(a, b, "two control fields share an offset");
        }
        assert!(a + 4 <= 4096, "a control field runs off its page");
    }
    assert!(
        wlist::RECORDS + wlist::RECORD_STRIDE * MAX_WINDOWS as u64 <= 4096,
        "the window list does not fit in a page",
    );
    assert!(
        ring::BYTES + ring::CAPACITY as u64 <= 4096,
        "the input ring does not fit in a page",
    );
    assert_eq!(ctl::MAGIC_VALUE, 0x434d_5031);
}

/// A request word round-trips, and an unknown opcode has a defined answer. The compositor's whole
/// message vocabulary is three content-free words, which is the point; this pins them.
#[test]
fn the_request_words_round_trip() {
    for operation in [proto::HELLO, proto::COMMIT] {
        let w = proto::req(operation, 0);
        assert_eq!(proto::operation(w), operation);
        assert_eq!(proto::operand(w), 0, "these verbs carry no operand at all");
    }
    assert_ne!(proto::HELLO, proto::COMMIT);
    assert_eq!(proto::EBADOP, -22);
    // Two verbs, both content-free, is the whole vocabulary. If a third ever arrives, the question
    // to ask first is whether it needs to know who is calling, because this endpoint cannot say.
    assert_eq!(
        proto::COMMIT,
        2,
        "the protocol grew a verb without a decision"
    );
}

/// The per-window digest a client reports is the digest of that window's pattern, and no two
/// windows share one. The kernel compares a client's report against this without trusting it.
#[test]
fn window_digests_are_distinct_and_predictable() {
    let mut seen: Vec<u64> = Vec::new();
    for (id, win) in SCENE.iter().enumerate() {
        let d = expected_window_checksum(id);
        assert!(!seen.contains(&d), "two windows digest the same");
        seen.push(d);
        // A blank surface must not match, which is the failure a client that never painted has.
        let blank = surface_checksum(win.w, win.h, |_| 0);
        assert_ne!(d, blank);
    }
}

/// A rectangle with one zero extent is empty, whatever the other extent says. `is_empty` is
/// the guard on every clip and union in this crate, and "zero width OR zero height" is the
/// half of it the canonical [`EMPTY`] (which zeroes both) can never witness.
#[test]
fn a_degenerate_rectangle_is_empty() {
    assert!(Rect::new(7, 9, 0, 5).is_empty(), "zero width is empty");
    assert!(Rect::new(7, 9, 5, 0).is_empty(), "zero height is empty");
    // The consequence a caller sees: a degenerate rect contributes nothing to the frame's
    // damage union. Were it treated as real, its far-away origin would smear the bounding
    // box and every frame would flush pixels nothing touched.
    let a = Rect::new(1, 2, 3, 4);
    assert_eq!(Rect::new(100, 100, 0, 5).union(&a), a);
    assert_eq!(a.union(&Rect::new(100, 100, 5, 0)), a);
    assert_eq!(Rect::new(3, 4, 0, 5).as_flush_rect(), None);
}

/// The geometry the compositor publishes is exact: stride is width times four with no
/// padding, and [`MAX_SURFACE_BYTES`] is the scene's largest surface, byte for byte. These
/// numbers cross a control page into another process, which paints by them; an
/// off-by-anything here is a fault in the client's own address space.
#[test]
fn stride_and_max_surface_bytes_are_exact() {
    assert_eq!(Window::new(8, 8, 64, 32).stride(), 256);
    assert_eq!(
        Window::new(0, 0, 1, 1).stride(),
        4,
        "one pixel is four bytes"
    );
    // 72*52*4 = 14,976, 71*56*4 = 15,904, 30*80*4 = 9,600: the second window is the largest.
    assert_eq!(MAX_SURFACE_BYTES, 15_904);
    assert_eq!(
        MAX_SURFACE_BYTES,
        SCENE.iter().map(|w| w.w * w.h * 4).max().unwrap(),
        "the const loop and the iterator must find the same maximum",
    );
}

/// **Exact pixel values, hand-computed from the definitions.** The property tests prove
/// the patterns are distinct and asymmetric, but a flipped mask, shift, or bit-op can keep
/// every property while changing every pixel, and the guest writes these exact words into
/// shared memory. Inputs are chosen so `&`, `|`, `^` and both shift directions each give a
/// different answer; the arithmetic for each constant is shown where it is asserted.
#[test]
fn known_pixels_pin_the_pattern_arithmetic() {
    // window_pixel(2, 5, 3): k = 2*53 + 11 = 117;
    //   r = ((5*3 + 117) | 1) & 0xff           = 133 = 0x85
    //   g = (3*7 + (5 >> 1) + 2*29) & 0xff     = 21 + 2 + 58 = 81 = 0x51
    //   b = ((5 ^ (3 << 1)) * (2*2 + 13) + 2*7) & 0xff = 3*17 + 14 = 65 = 0x41
    // x=5 against y<<1=6 pins the xor: 5^6 = 3, 5|6 = 7, 5&6 = 4, all different.
    assert_eq!(window_pixel(2, 5, 3), 0x0085_5141);
    // window_pixel(0, 7, 4): k = 11;
    //   r = ((21 + 11) | 1) & 0xff = 33 = 0x21
    //   g = (28 + 3 + 0) & 0xff    = 31 = 0x1f
    //   b = ((7 ^ 8) * 13 + 0) & 0xff = 195 = 0xc3
    assert_eq!(window_pixel(0, 7, 4), 0x0021_1fc3);

    // background_pixel walks 8x8 blocks; (x>>3, y>>3) of (1,0), (1,1), (2,0) pin the xor
    // and the &1 in turn (block parities 1, 0, and 0-from-an-even-2).
    // (8, 0):  block 1^0 = 1 -> r = 0x30; g = 0x10 + 0 = 0x10; b = 0x08 + 2 = 0x0a.
    assert_eq!(background_pixel(8, 0), 0x0030_100a);
    // (8, 8):  block 1^1 = 0 -> r = 0; g = 0x10 + 4 = 0x14; b = 0x0a.
    assert_eq!(background_pixel(8, 8), 0x0000_140a);
    // (16, 0): block (2^0)&1 = 0 -> r = 0; g = 0x10; b = 0x08 + 4 = 0x0c.
    assert_eq!(background_pixel(16, 0), 0x0000_100c);
}

/// **The digest is FNV-1a 64 over exactly `w * h` pixels, in index order**, pinned against
/// a value computed with an independent FNV-1a implementation. Every other checksum test in
/// this crate compares the function to itself, so a digest that iterated one pixel short,
/// or or-ed where it should xor, would agree with itself everywhere and never be seen.
#[test]
fn the_surface_checksum_is_fnv1a_over_every_pixel() {
    // Six pixels with asymmetric bytes, as a 2x3 surface. FNV-1a 64: start at the offset
    // basis 0xcbf2_9ce4_8422_2325; per little-endian byte, h ^= b, then h *= 0x100_0000_01b3.
    let px: [u32; 6] = [
        0x12b7_45d9,
        0x00e1_073c,
        0x8a5f_3b16,
        0x47c2_d801,
        0x3e99_a47f,
        0x6d18_c5b2,
    ];
    assert_eq!(surface_checksum(2, 3, |i| px[i]), 0x23f8_e2f3_afb0_3cb0);
    // And the pixel count really is w * h, stated directly rather than inferred from the
    // digest: a 2x3 surface is read six times, not five (w + h) and not zero (w / h).
    let mut reads = 0;
    surface_checksum(2, 3, |_| {
        reads += 1;
        0
    });
    assert_eq!(reads, 6);
}

/// A window's expected digest unfolds pixel `i` as `(i % w, i / w)`, row-major. Checked
/// against the same surface built by the nested loops the clients actually paint with, so a
/// transposed or misfolded index cannot agree with it.
#[test]
fn a_window_digest_reads_the_surface_row_major() {
    let surfaces = painted_scene();
    for (id, win) in SCENE.iter().enumerate() {
        let s = &surfaces[id];
        assert_eq!(
            expected_window_checksum(id),
            surface_checksum(win.w, win.h, |i| s[i]),
            "window {id}'s digest does not match its painted surface",
        );
    }
}

/// Every surface painted with its own window's pattern, as the clients paint them.
fn painted_scene() -> Vec<Vec<u32>> {
    let mut surfaces = Vec::new();
    for (i, win) in SCENE.iter().enumerate() {
        let mut s = vec![0u32; win.pixels()];
        for y in 0..win.h {
            for x in 0..win.w {
                s[(y * win.w + x) as usize] = window_pixel(i as u32, x, y);
            }
        }
        surfaces.push(s);
    }
    surfaces
}
