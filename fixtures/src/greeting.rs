//! A program no boot image carries (milestone 198 (a package manager) rung 3a).
//!
//! It prints one line and exits. `fixtures` lists it in `packaged_only`, so no archive packs it
//! (`script/swish-check` reads the archive to make sure), and the only way onto a machine is the
//! package `packages/greeting.recipe.toml` builds. So when it runs at the prompt, the image's
//! catalogue, the fetch, the digest check and the installed-programs table are the only things that
//! could have put it there. notes/packages/fetching.md has the transcript.

//! # What this program holds
//!
//! Slot 0, its output: the sink contract (`crates/byte_sink_contract`). Slot 1, the clock page,
//! because [`MANIFEST`] asks for one, carried as an ELF note (milestone 597 (a program carries its
//! manifest in an ELF note), provisional; DECISIONS §197 (a package is one archive file) option
//! M2). No boot image has a row for this program, so a vouched copy holding slot 1, the second line
//! it prints, proves the note was read. notes/packages/the-test-programs.md has the transcript.

//! # BUGS
//!
//! It proves the path and a manifest's travel, and nothing about the program: a line of text is
//! the least a program can do, and it does not read the clock it holds.
//!

//! Name: provisional (2026-09-26). Says what it prints; GNU `hello`, which exists to demonstrate
//! packaging, would be the prior art, and `hello` is already a fixture here.

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

/// The line. The gate asserts it, so a copy of any program the image carries could not print it.
const LINE: &[u8] = b"hello from a package this image never carried\n";

user_mode_runtime::program_entry!(start);

fn start(_a0: u64, _a1: u64, _a2: u64) -> ! {
    say(LINE);
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
