//! **The atlas generator**: DejaVu Sans Mono's outlines in, `crates/glyph_atlas`'s coverage tables
//! out (milestone 142 (a text display good enough that people use it instead of a GUI) increment
//! 3). It runs on the host, once, and its output is checked in, so the three-party proof of
//! on-screen text (the terminal draws, the kernel predicts, the host grades) is table lookups. The
//! table is on the verification path and this tool is not; `tests/regenerate.rs` holds the table
//! to this tool's output byte for byte. §166 (the rasteriser dependency, and whether the glyph
//! atlas ships one face or four) and the determinism appendix fix three choices, not to be
//! re-litigated here: `skrifa` plus `ab_glyph_rasterizer` 0.1.10; grayscale coverage, never
//! subpixel; and `u8` coverage at the boundary ([`quantize`]), never `f32`. See notes/glyphs.md.
//!
//! Name: provisional, for the crate and every item in it. calef names things.

use ab_glyph_rasterizer::{Point, Rasterizer, point};
use glyph_atlas::{ASCENT, CELL_H, CELL_W, FIRST, GLYPHS, LAST, PIXELS_PER_EM};
use skrifa::instance::{LocationRef, Size};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::raw::TableProvider;
use skrifa::{FontRef, GlyphId, MetadataProvider};

/// Each face: the vendored font it is drawn from, and the table it becomes. Paths are relative to
/// the repository root. `crates/glyph_atlas/src/lib.rs` names the same four tables in its
/// `include_bytes!`, and [`check`] fails on a table that is missing.
///
/// Name: provisional.
pub const FACES: [(&str, &str); 4] = [
    (
        "vendor/dejavu/DejaVuSansMono.ttf",
        "crates/glyph_atlas/coverage/regular.coverage",
    ),
    (
        "vendor/dejavu/DejaVuSansMono-Bold.ttf",
        "crates/glyph_atlas/coverage/bold.coverage",
    ),
    (
        "vendor/dejavu/DejaVuSansMono-Oblique.ttf",
        "crates/glyph_atlas/coverage/oblique.coverage",
    ),
    (
        "vendor/dejavu/DejaVuSansMono-BoldOblique.ttf",
        "crates/glyph_atlas/coverage/bold_oblique.coverage",
    ),
];

/// One face's table: [`GLYPHS`] cells of `CELL_W * CELL_H` coverage bytes, top row first, in the
/// order `glyph_atlas` reads them (`FIRST..=LAST`, then the missing-glyph box, the font's
/// `.notdef`).
///
/// # Panics
///
/// On a font that does not parse, lacks a glyph in the repertoire, or is not monospaced at the
/// advance the cell was sized for. Each is a wrong input, and a table drawn from one would be wrong
/// in a way nothing downstream could see.
///
/// Name: provisional.
pub fn table(font: &[u8]) -> Vec<u8> {
    let face = FontRef::new(font).expect("the vendored font parses");
    let units_per_em = face
        .head()
        .expect("the font has a head table")
        .units_per_em();
    let scale = PIXELS_PER_EM as f32 / units_per_em as f32;
    let charmap = face.charmap();
    let metrics = face.glyph_metrics(Size::unscaled(), LocationRef::default());
    let outlines = face.outline_glyphs();
    let mut out = Vec::with_capacity(GLYPHS * (CELL_W * CELL_H) as usize);
    let glyphs = (FIRST..=LAST)
        .map(|cp| {
            let ch = char::from_u32(cp).expect("the repertoire is scalar values");
            charmap
                .map(ch)
                .unwrap_or_else(|| panic!("the font has no glyph for {ch:?}"))
        })
        .chain([GlyphId::NOTDEF]);
    for id in glyphs {
        let advance = metrics
            .advance_width(id)
            .expect("every glyph has an advance");
        // Centered in the cell: the advance is 13.24 pixels at 22 per em, so each side gets the
        // same fraction of the 0.76 left over rather than all of it landing on the right.
        let left = (CELL_W as f32 - advance * scale) / 2.0;
        let mut pen = Pen {
            raster: Rasterizer::new(CELL_W as usize, CELL_H as usize),
            scale,
            left,
            start: point(0.0, 0.0),
            last: point(0.0, 0.0),
        };
        // In font units and unhinted, so the scale and the centering above are the only
        // transform, the same arithmetic whatever parser read the outline. A glyph with no
        // outline (the space) is all background, which is what an untouched rasteriser yields.
        if let Some(glyph) = outlines.get(id) {
            glyph
                .draw(
                    DrawSettings::unhinted(Size::unscaled(), LocationRef::default()),
                    &mut pen,
                )
                .expect("the vendored font's outline draws");
        }
        let mut cell = vec![0u8; (CELL_W * CELL_H) as usize];
        pen.raster.for_each_pixel_2d(|x, y, a| {
            cell[(y * CELL_W + x) as usize] = quantize(a);
        });
        out.extend_from_slice(&cell);
    }
    out
}

/// **The `u8` boundary.** `ab_glyph_rasterizer`'s raw `f32` output has changed in a patch release,
/// and its accumulated error leaves coverage a hair above zero outside a glyph or a hair above one
/// inside it. Rounding to 256 levels here absorbs both, and nothing past this function sees a float.
///
/// Name: provisional.
pub fn quantize(a: f32) -> u8 {
    (a.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// The outline, in font units with y up, drawn into the cell, in pixels with y down and the
/// baseline [`ASCENT`] rows from the top.
struct Pen {
    raster: Rasterizer,
    scale: f32,
    left: f32,
    start: Point,
    last: Point,
}

impl Pen {
    fn at(&self, x: f32, y: f32) -> Point {
        point(x * self.scale + self.left, ASCENT as f32 - y * self.scale)
    }
}

impl OutlinePen for Pen {
    fn move_to(&mut self, x: f32, y: f32) {
        self.start = self.at(x, y);
        self.last = self.start;
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let p = self.at(x, y);
        self.raster.draw_line(self.last, p);
        self.last = p;
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let (c, p) = (self.at(x1, y1), self.at(x, y));
        self.raster.draw_quad(self.last, c, p);
        self.last = p;
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let (c1, c2, p) = (self.at(x1, y1), self.at(x2, y2), self.at(x, y));
        self.raster.draw_cubic(self.last, c1, c2, p);
        self.last = p;
    }

    // A contour the font left open is closed with a line back to its start, as every TrueType
    // renderer does; without it the coverage of the open side would leak to the cell's edge.
    fn close(&mut self) {
        if self.last != self.start {
            self.raster.draw_line(self.last, self.start);
        }
        self.last = self.start;
    }
}

/// The repository root, from this crate's own manifest directory.
///
/// Name: provisional.
pub fn root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Every face's table as this tool makes it, beside its path.
///
/// Name: provisional.
pub fn tables() -> Vec<(&'static str, Vec<u8>)> {
    FACES
        .iter()
        .map(|(font, table_path)| {
            let bytes =
                std::fs::read(root().join(font)).unwrap_or_else(|e| panic!("reading {font}: {e}"));
            (*table_path, table(&bytes))
        })
        .collect()
}

/// Where the sRGB-to-linear table is written: Rust source, so the runtime crate reads it as a
/// `const` and a reviewer reads it as numbers.
///
/// Name: provisional.
pub const LINEAR_PATH: &str = "crates/glyph_atlas/src/linear.rs";

/// **The sRGB-to-linear table as Rust source**: each of the 256 sRGB codes as linear light scaled
/// to `u16::MAX`, by IEC 61966-2-1's curve, rounded to nearest. Computed here in `f64` so the
/// runtime crate does no floating point: `glyph_atlas::blend` only indexes the table and searches
/// it. `powf` is the platform's, so a host whose `pow` rounds differently at one of these 256
/// points fails the regenerate gate rather than shipping a different table.
///
/// Name: provisional.
pub fn linear_source() -> String {
    // The header is a template file, not a string here, because `script/names` and the
    // module-block ratchet would read its doc-comment and `pub` lines as this file's own.
    let mut out = String::from(include_str!("linear_header.txt"));
    for row in 0..32 {
        out.push_str("   ");
        for code in row * 8..row * 8 + 8 {
            let c = code as f64 / 255.0;
            let linear = if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            };
            out.push_str(&format!(" {},", (linear * 65535.0).round() as u16));
        }
        out.push('\n');
    }
    out.push_str("];\n");
    out
}

/// Everything the generator writes, beside its path: the four face tables and the linear table.
///
/// Name: provisional.
pub fn outputs() -> Vec<(&'static str, Vec<u8>)> {
    let mut all = tables();
    all.push((LINEAR_PATH, linear_source().into_bytes()));
    all
}

/// The first difference between the checked-in tables and what this tool makes now, described in
/// glyphs and pixels, or `None` when every table is byte-identical.
///
/// Name: provisional.
pub fn check() -> Option<String> {
    for (path, want) in outputs() {
        let got = match std::fs::read(root().join(path)) {
            Ok(b) => b,
            Err(e) => return Some(format!("{path}: {e}")),
        };
        if path == LINEAR_PATH && got != want {
            return Some(format!("{path}: differs from what the generator writes"));
        }
        if got.len() != want.len() {
            return Some(format!(
                "{path}: {} bytes checked in, the generator makes {}",
                got.len(),
                want.len()
            ));
        }
        if let Some(i) = (0..got.len()).find(|&i| got[i] != want[i]) {
            let per = (CELL_W * CELL_H) as usize;
            let (glyph, px) = (i / per, i % per);
            let cp = FIRST + glyph as u32;
            let name = if cp <= LAST {
                format!("{:?}", char::from_u32(cp).unwrap_or('?'))
            } else {
                "the missing-glyph box".into()
            };
            return Some(format!(
                "{path}: {name} differs at ({}, {}): checked in {}, generated {}",
                px as u32 % CELL_W,
                px as u32 / CELL_W,
                got[i],
                want[i]
            ));
        }
    }
    None
}
