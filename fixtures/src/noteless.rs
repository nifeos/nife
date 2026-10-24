//! **A packaged program with no manifest note** (milestone 47 (navigation and naming)'s bare-name
//! lane, 2026-09-27).
//!
//! It prints one line and exits. DECISIONS §229 (how a bare name at the prompt reaches an installed
//! program) refuses to install a package whose program has a name the image carries, so
//! `script/swish-check` proves the installed path and the no-note default with this program. Like
//! `greeting` it is listed in `fixtures`' `packaged_only`, so no archive packs it and only its
//! package (`packages/noteless.recipe.toml`) puts it on a machine; unlike `greeting` it carries no
//! ELF note, so a vouched copy is bound and endowed as `grant_plan::NO_NOTE_MANIFEST`, its output
//! alone.

//! # EXAMPLES
//!
//! ```text
//! $ cd downloads
//! $ jig install ./noteless.nifepkg
//!   installed; generation 1 is live
//! $ noteless
//! noteless: installed, and carrying no manifest note
//! ```

//! # What this program holds
//!
//! Slot 0, its output: the sink contract (`crates/byte_sink_contract`). Nothing else, which is the
//! no-note default and the thing the gate's `caps` line checks.

//! # BUGS
//!
//! It proves the path and the default, and nothing about the program.
//!
//! Name: provisional (2026-09-27). Says the one thing that distinguishes it from `greeting`.

#![no_std]
#![allow(missing_docs)]
#![no_main]

use user_mode_runtime::{exit, send};

/// Slot 0: where the line goes.
const OUT: u64 = 0;

/// The line. The gate asserts it, so a copy of any program the image carries could not print it.
const LINE: &[u8] = b"noteless: installed, and carrying no manifest note\n";

user_mode_runtime::program_entry!(start);

fn start(_a0: u64, _a1: u64, _a2: u64) -> ! {
    let mut rest = LINE;
    while !rest.is_empty() {
        let (w0, w1, w2, n) = byte_sink_contract::pack(rest);
        send(OUT, w0, w1, w2);
        rest = &rest[n..];
    }
    send(OUT, byte_sink_contract::eof(), 0, 0);
    exit();
}

user_mode_runtime::panic_handler!();
