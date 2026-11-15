//! Delivers the link script to every program that links this crate.
//!
//! A program takes the script with `-C link-arg=-Tlink.ld` and no path: this build script copies
//! `link.ld` into `OUT_DIR` and adds that directory to the linker's search path, which cargo hands
//! to every dependent that links. The same mechanism delivers `link.x` in `cortex-m-rt` and
//! `riscv-rt`.

use std::{env, fs, path::PathBuf};

fn main() {
    let out = PathBuf::from(env::var("OUT_DIR").expect("cargo sets OUT_DIR for a build script"));
    fs::copy("link.ld", out.join("link.ld")).expect("user_mode_runtime: cannot copy link.ld");
    println!("cargo::rustc-link-search=native={}", out.display());
    println!("cargo::rerun-if-changed=link.ld");
    println!("cargo::rerun-if-changed=build.rs");
}
