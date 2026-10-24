//! **The regenerate-and-compare gate** (milestone 142 (a text display good enough that people use
//! it instead of a GUI) increment 3).
//!
//! Neither `skrifa` nor `ab_glyph_rasterizer` documents its output as stable, and the rasteriser's
//! raw output has moved in a patch release; two investigations measured it bit-identical across
//! aarch64 and x86_64 hosts, which nothing upstream defends. So on every host that runs `cargo
//! xtask test` the shipped tables must be exactly what the pinned pair makes from the vendored
//! fonts. A pin bump, a font swap or a disagreeing host fails here, naming the glyph and pixel,
//! rather than as a moved pixel that a screen checker reports far less legibly.

#[test]
fn the_checked_in_atlas_is_what_the_generator_makes() {
    if let Some(why) = glyph_atlas_generator::check() {
        panic!(
            "{why}\n\nIf the change is deliberate, regenerate with \
             `cargo run --manifest-path tools/glyph_atlas_generator/Cargo.toml` and commit the \
             tables with the reason."
        );
    }
}
