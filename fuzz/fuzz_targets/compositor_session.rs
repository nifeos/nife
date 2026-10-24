//! Fuzz the compositor's **frame handling** with sessions of lying clients, and check the screen
//! after every frame (proposal #1592 part a, rank 5).
//!
//! It runs the decode the compositor binary's `serve_frame` runs for each client whose sequence
//! moved (`compositor::commit_damage`: four untrusted control-page words in, screen damage and a
//! status word out), the union of those, and `compositor::composite` into a host screen. In each
//! frame of a session any client may scribble on its own surface, write any four words as its
//! damage rectangle, and ring or not. notes/fuzzing-the-services.md covers the session targets.

//! # The rules it checks after every frame, beyond "no panic"
//!
//! 1. *A lie re-copies only the liar.* A client's damage, whatever four words it wrote, lands inside
//!    its own window on the screen.
//! 2. *An honest status.* `STATUS_OK` only when the rectangle was inside the client's surface.
//! 3. *Only damage changes.* Every screen pixel outside the frame's damage is what it was.
//! 4. *The damage is right.* Every pixel inside it is what the scene says: the topmost committed
//!    window's own pixel there, or the background. No client's pixel reaches another's window.
//!
//! Name: provisional (`lane/fuzz-service-handlers`, 2026-10-04 UTC); an architect names things.
//! Falsification: replayable `fuzz/falsifications/compositor_session.damage_is_not_clipped_to_the_surface.patch`

#![no_main]

use compositor::proto::ctl;
use compositor::{
    EMPTY, Rect, SCENE, SCREEN_PIXELS, SCREEN_W, background_pixel, commit_damage, composite,
};
use libfuzzer_sys::arbitrary::{Result as ArbResult, Unstructured};
use libfuzzer_sys::fuzz_target;

const MAX_FRAMES: usize = 24;

/// A damage word: mostly near the surface, sometimes anywhere, including the values that turn
/// negative or saturate when the compositor reads them as `i32`.
fn word(u: &mut Unstructured) -> ArbResult<u32> {
    Ok(match u.int_in_range(0u8..=7)? {
        0 => u.arbitrary()?,
        1 => *u.choose(&[
            0,
            1,
            i32::MAX as u32,
            i32::MAX as u32 + 1,
            u32::MAX,
            u32::MAX - 7,
        ])?,
        _ => u.int_in_range(0..=96)?,
    })
}

/// The screen pixel the scene says belongs at `(x, y)`: the topmost committed window's own, or the
/// background. Written here rather than reused, so a defect in the crate's own expectation helper
/// cannot hide one in `composite`.
fn reference(surfaces: &[Vec<u32>], committed: &[bool], x: i32, y: i32) -> u32 {
    for i in (0..SCENE.len()).rev() {
        let w = &SCENE[i];
        if committed[i] && w.rect().contains(x, y) {
            let (sx, sy) = ((x - w.origin_x) as usize, (y - w.origin_y) as usize);
            return surfaces[i][sy * w.w as usize + sx];
        }
    }
    background_pixel(x as u32, y as u32)
}

fn first_paint() -> &'static [u32] {
    static SCREEN: std::sync::OnceLock<Vec<u32>> = std::sync::OnceLock::new();
    SCREEN.get_or_init(|| {
        let mut screen = vec![0xdead_beef_u32; SCREEN_PIXELS];
        let nobody: Vec<&[u32]> = vec![&[]; SCENE.len()];
        composite(&mut screen, &nobody, SCENE.len(), Rect::screen());
        assert!(
            !screen.contains(&0xdead_beef),
            "the first paint left poison"
        );
        screen
    })
}

fn session(u: &mut Unstructured) -> ArbResult<()> {
    let n = SCENE.len();
    let mut surfaces: Vec<Vec<u32>> = SCENE.iter().map(|w| vec![0; w.pixels()]).collect();
    let mut committed = vec![false; n];
    // The compositor's first act is a whole-screen paint with nobody committed. It is the same for
    // every session, so it is painted once (onto poison, so a pixel it should have written and did
    // not is visible) and copied.
    let initial = first_paint();
    let mut screen = initial.to_vec();
    // Rules 3 and 4 are checked per frame over the scene and a margin round it, which is where any
    // damage can legitimately fall; everything else is checked once, at the end, against the first
    // paint. A full-screen scan per frame cost a hundred times the session.
    let scene = SCENE
        .iter()
        .fold(EMPTY, |a, w| a.union(&w.rect()))
        .intersect(&Rect::screen());
    let margin = Rect::new(scene.x - 16, scene.y - 16, scene.w + 32, scene.h + 32)
        .intersect(&Rect::screen());

    for _ in 0..MAX_FRAMES {
        if u.is_empty() {
            break;
        }
        let mut damage = EMPTY;
        for i in 0..n {
            // Scribble: a run of pixels anywhere in the client's own surface.
            if u.ratio(1, 2)? {
                let len = surfaces[i].len();
                let start = u.choose_index(len)?;
                let run = u.int_in_range(1..=64)?.min(len - start);
                let colour: u32 = u.arbitrary()?;
                for (k, p) in surfaces[i][start..start + run].iter_mut().enumerate() {
                    *p = colour.wrapping_add(k as u32 * 0x0101_0101) ^ (i as u32) << 24;
                }
            }
            if !u.ratio(2, 3)? {
                continue; // this client did not ring this frame
            }
            let (x, y, w, h) = (word(u)?, word(u)?, word(u)?, word(u)?);
            let (seen, status) = commit_damage(&SCENE[i], x, y, w, h);
            committed[i] = true;

            // Rule 1.
            let own = SCENE[i].rect().intersect(&Rect::screen());
            assert!(
                seen.is_empty() || seen.intersect(&own) == seen,
                "client {i}'s damage {seen:?} from ({x}, {y}, {w}, {h}) leaves its window {own:?}"
            );
            // Rule 2.
            let asked = Rect::new(x as i32, y as i32, w, h);
            if status == ctl::STATUS_OK {
                assert_eq!(
                    asked.intersect(&SCENE[i].bounds()),
                    asked,
                    "client {i}: STATUS_OK for {asked:?}, outside its surface"
                );
            }
            damage = damage.union(&seen);
        }
        if damage.is_empty() {
            continue;
        }

        let at = |x: i32, y: i32| y as usize * SCREEN_W as usize + x as usize;
        let before: Vec<u32> = (margin.y..margin.bottom())
            .flat_map(|y| (margin.x..margin.right()).map(move |x| (x, y)))
            .map(|(x, y)| screen[at(x, y)])
            .collect();
        let srcs: Vec<&[u32]> = (0..n)
            .map(|i| {
                if committed[i] {
                    &surfaces[i][..]
                } else {
                    &[][..]
                }
            })
            .collect();
        composite(&mut screen, &srcs, n, damage);

        let mut was = before.into_iter();
        for y in margin.y..margin.bottom() {
            for x in margin.x..margin.right() {
                let (now, was) = (screen[at(x, y)], was.next().unwrap());
                if damage.contains(x, y) {
                    // Rule 4.
                    let want = reference(&surfaces, &committed, x, y);
                    assert_eq!(now, want, "pixel ({x}, {y}) inside the damage {damage:?}");
                } else {
                    // Rule 3.
                    assert_eq!(
                        now, was,
                        "pixel ({x}, {y}) outside the damage {damage:?} changed"
                    );
                }
            }
        }
    }
    // Rule 3 for the rest of the screen, once, a row slice at a time.
    let row = SCREEN_W as usize;
    for (y, (now, first)) in screen.chunks(row).zip(initial.chunks(row)).enumerate() {
        let y = y as i32;
        let (a, b) = if y >= margin.y && y < margin.bottom() {
            (margin.x as usize, margin.right() as usize)
        } else {
            (0, 0)
        };
        assert!(
            now[..a] == first[..a] && now[b.max(a)..] == first[b.max(a)..],
            "a pixel in row {y} far from every window changed"
        );
    }
    Ok(())
}

fuzz_target!(|data: &[u8]| {
    let _ = session(&mut Unstructured::new(data));
});
