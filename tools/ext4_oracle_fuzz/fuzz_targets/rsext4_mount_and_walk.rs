//! `rsext4`'s own `mount_and_walk` target (`rsext4_fuzz::targets::mount_and_walk`), in this rotation.
//!
//! Name: provisional, minted 2026-11-26 (UTC) by lane/ext4-fuzz-rotation: `rsext4_` and the target's
//! name in `rsext4`'s fuzz directory.

#![no_main]

libfuzzer_sys::fuzz_target!(|data: &[u8]| rsext4_fuzz::targets::mount_and_walk(data));
