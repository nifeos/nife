//! **`seeker` at 0.2.0**: the second version's bytes, whose note asks one zone more than the
//! first's (`example.net` beside `nife.test`), so the approval file's entry for `seeker` does not
//! cover it and the subset test refuses the update, naming the extra zone, while the previous
//! version stays installed and running (milestone 911 (scripted installs grant only what the
//! owner's approval file allows), Done-when 2). Rows key on the digest (milestone 614 (two installed versions of one program, each runnable, and a caller granted
//! the one it needs)'s ruling 2),
//! so the note's difference is the whole point; the `as` key packs these bytes under the member
//! name `seeker`, so the package's program is still `seeker`.
//!
//! The package is `packages/seeker-0.2.0.recipe.toml`. Listed in `packaged_only`, so no archive
//! packs it.
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

capability_manifest_note::carry!(MANIFEST, zones = &["example.net", "nife.test"]);

user_mode_runtime::program_entry!(start);

fn start(_a0: u64, _a1: u64, _a2: u64) -> ! {
    say(b"seeker 0.2.0: holding the network and two zones\n");
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
