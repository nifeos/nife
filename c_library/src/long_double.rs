//! `long double` on x86_64, where nife has none (milestone 835 (a C library, stage 1: files, clock
//! and memory)).
//!
//! relibc converts a `long double` in C (`vendor/relibc/c/stdlib.c`), because Rust has no type for
//! one. On x86_64 that file is not compiled: a nife process runs with the x87 off
//! (helpers/c-library-cflags.sh), and clang then refuses `long double` outright, so no C program
//! for this target can declare one or pass one to `printf`. `printf`'s `%Lf` path still names the
//! conversion, so it is defined here. It is never reached by a program that compiled; it answers
//! NaN rather than read 16 bytes that were never passed.

use crate::platform::types::{c_double, c_longdouble};

/// relibc's `long double` to `double` conversion, which `printf` calls for `%Lf`.
///
/// # Safety
/// None needed: the pointer is not read (see the module comment).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn relibc_ldtod(_val: *const c_longdouble) -> c_double {
    c_double::NAN
}
