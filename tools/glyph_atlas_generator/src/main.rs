//! Write `crates/glyph_atlas`'s coverage tables from the vendored fonts, or with `--check` say
//! whether the checked-in ones are what this tool makes. The gate is `tests/regenerate.rs`, which
//! `cargo xtask test` runs; `--check` is the same comparison for a person at a prompt.
//!
//! ```text
//! cargo run --manifest-path tools/glyph_atlas_generator/Cargo.toml            # regenerate
//! cargo run --manifest-path tools/glyph_atlas_generator/Cargo.toml -- --check # compare only
//! ```
//!
//! Name: provisional, the program's name is the package's. calef names things.

use std::process::ExitCode;

fn main() -> ExitCode {
    let check = match std::env::args().nth(1).as_deref() {
        None => false,
        Some("--check") => true,
        Some(other) => {
            eprintln!("usage: glyph_atlas_generator [--check] (got {other:?})");
            return ExitCode::FAILURE;
        }
    };
    if check {
        return match glyph_atlas_generator::check() {
            None => ExitCode::SUCCESS,
            Some(why) => {
                eprintln!("{why}");
                ExitCode::FAILURE
            }
        };
    }
    for (path, table) in glyph_atlas_generator::outputs() {
        let at = glyph_atlas_generator::root().join(path);
        if let Err(e) = std::fs::write(&at, &table) {
            eprintln!("writing {path}: {e}");
            return ExitCode::FAILURE;
        }
        println!("{path}: {} bytes", table.len());
    }
    ExitCode::SUCCESS
}
