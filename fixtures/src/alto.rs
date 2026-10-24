//! One of two programs in one package, named unlike it (milestone 919 (a package carries several
//! programs)).
//!
//! The package `duet` (`packages/duet.recipe.toml`) carries this and `tenor` as `bin/alto` and
//! `bin/tenor`, so a bare `alto` at the prompt reaches this only if `jig install` wrote a row per
//! program and not one for the package. `fixtures` lists both in `packaged_only`, so no archive
//! packs them. `solo` (`packages/solo.recipe.toml`) carries the same bytes as `bin/alto`: it is
//! refused whole while `duet` is installed, naming `alto` and `duet`, and installs once
//! `jig remove duet` has run. notes/packages/several-programs.md has the transcript.

//! # What this program holds
//!
//! Slot 0, its output, and slot 1, the clock page, because [`MANIFEST`] asks for one. `tenor`'s
//! note does not, so the two programs of one package are endowed differently, each from its own
//! note, and the second line each prints is the proof.
//!
//! # BUGS
//!
//! It proves the path, and nothing about the program: it does not read the clock it holds.
//!
//! Name: provisional, milestone 919's lane, 2026-10-14 (UTC). Two voices in one duet.

#![no_std]
#![allow(missing_docs)]
#![no_main]

use user_mode_runtime::{exit, is_granted, send};

/// Slot 0: where the line goes.
const OUT: u64 = 0;

/// Slot 1: the clock page, where a native child's clock lands when its manifest asks.
const CLOCK: u64 = 1;

/// **What this program asks for**: `uptime`'s output and nothing else, plus the clock.
const MANIFEST: grant_plan::Manifest = grant_plan::Manifest {
    clock: true,
    ..grant_plan::Program::Uptime.manifest()
};

capability_manifest_note::carry!(MANIFEST);

#[unsafe(no_mangle)]
pub extern "C" fn _start(_a0: u64, _a1: u64, _a2: u64) -> ! {
    say(b"alto: one of two programs in the package duet\n");
    say(if is_granted(CLOCK) {
        b"clock: held at slot 1, as its manifest note asked\n"
    } else {
        b"clock: not held; slot 1 is empty\n"
    });
    send(OUT, byte_sink_contract::eof(), 0, 0);
    exit();
}

fn say(mut rest: &[u8]) {
    while !rest.is_empty() {
        let (w0, w1, w2, n) = byte_sink_contract::pack(rest);
        send(OUT, w0, w1, w2);
        rest = &rest[n..];
    }
}

user_mode_runtime::panic_handler!();
