//! A C program, on nife (milestone 835 (a C library, stage 1: files, clock and memory)).
//!
//! This is the whole Rust side of one: `std`'s `_start` runs this `main`, which hands the process
//! to the C program's `main` through `c_library::start::run`. The C program is linked in as the
//! archive `helpers/build-c-program.sh` built (`NIFE_C_ARCHIVE`, read by `build.rs`), with its
//! `main` renamed `nife_c_main` so the two do not collide. Without that archive this binary does
//! not link, and the library builds alone with `--lib`.
//!
//! Name: provisional 2026-10-10 (UTC), milestone 835's lane. The output is renamed to the C
//! program's own name (`speedtest1`) when it is installed, so this name is seen only in a build.

unsafe extern "C" {
    fn nife_c_main(
        argc: core::ffi::c_int,
        argv: *mut *mut core::ffi::c_char,
        envp: *mut *mut core::ffi::c_char,
    ) -> core::ffi::c_int;
}

fn main() {
    c_library::start::run(nife_c_main)
}
