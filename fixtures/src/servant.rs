//! **`servant`**: a program whose manifest note asks to run as a service (milestone 911 (scripted
//! installs grant only what the owner's approval file allows), the version 3 ask). `script/swish-check`
//! installs it twice, once interactively so the confirmation lists the ask, and once by a
//! delegated endpoint whose approval allows it, and the grant record is the point: nothing starts
//! a recorded service until milestone 910 (a package declares a service) builds the start, so the
//! body only says it ran.
//!
//! The package is `servant` (`packages/servant.recipe.toml`). Listed in `packaged_only`, so no
//! archive packs it.
//!
//! Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.

#![no_std]
#![allow(missing_docs)]
#![no_main]

use user_mode_runtime::{exit, send};

/// Slot 0: where the line goes.
const OUT: u64 = 0;

const MANIFEST: grant_plan::Manifest = grant_plan::Manifest {
    ..grant_plan::Program::Uptime.manifest()
};

capability_manifest_note::carry!(MANIFEST, service = true);

user_mode_runtime::program_entry!(start);

fn start(_a0: u64, _a1: u64, _a2: u64) -> ! {
    say(b"servant: installed; its note asks to run as a service\n");
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
