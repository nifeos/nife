//! **`seeker`**: a program whose manifest note asks for the network and the zone `nife.test`, the
//! package the approval file's entry allows, at 0.1.0 (milestone 911 (scripted installs grant
//! only what the owner's approval file allows)). `script/swish-check` installs it under the
//! approval, runs it, and then refuses the 0.2.0 package whose note asks one more zone, which is
//! the subset test's shape: the previous version stays installed and running.
//!
//! The package is `seeker` (`packages/seeker.recipe.toml`); 0.2.0's member is
//! [`crate::seeker_v2`]'s bytes (`packages/seeker-0.2.0.recipe.toml`). Listed in `packaged_only`,
//! so no archive packs it.
//!
//! Name: provisional, minted 2026-10-28 (UTC) by milestone/911-scripted-installs-approval-file.

#![no_std]
#![allow(missing_docs)]
#![no_main]

use user_mode_runtime::{exit, send};

/// Slot 0: where the line goes.
const OUT: u64 = 0;

const MANIFEST: grant_plan::Manifest = grant_plan::Manifest {
    network: true,
    ..grant_plan::Program::Uptime.manifest()
};

capability_manifest_note::carry!(MANIFEST, zones = &["nife.test"]);

user_mode_runtime::program_entry!(start);

fn start(_a0: u64, _a1: u64, _a2: u64) -> ! {
    say(b"seeker 0.1.0: holding the network and nife.test\n");
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
