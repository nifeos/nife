//! A sequence of heap operations, driven the way `memory_region_heap` drives talc.

#![no_main]

libfuzzer_sys::fuzz_target!(|data: &[u8]| fuzz::run(data));
