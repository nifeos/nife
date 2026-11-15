//! A sequence of heap operations, driven the way a nife program's heap drives talc. See
//! notes/talc-fuzzing.md.

#![no_main]

libfuzzer_sys::fuzz_target!(|data: &[u8]| fuzz::run(data));
