//! How a C program starts on nife (milestone 835 (a C library, stage 1: files, clock and memory)).
//!
//! relibc starts a program from `crt0`: it reads `argc`, `argv` and `envp` off the initial stack,
//! sets up TLS through its dynamic linker, runs the init arrays and calls `main`. None of that is
//! seeded. A nife program is entered at `std`'s `_start` (notes/std.md), which sets up `std`'s
//! runtime and calls a Rust `main`; this module is what that Rust `main` calls to become the C
//! program. The words the program was given come from `std::env::args_os`, which reads nife's
//! argument page (milestone 205 (how a foreign program is told what to do)), and the environment
//! from `std::env::vars_os`.
//!
//! The C program's own `main` is renamed at build time (`helpers/build-c-program.sh` passes the
//! object through `llvm-objcopy --redefine-sym main=nife_c_main`), because the Rust program has a
//! `main` of its own: the one `std`'s `_start` calls. The source is not touched.

use alloc::ffi::CString;
use alloc::vec::Vec;

use crate::header::stdlib;
use crate::platform::types::*;
use crate::platform::{self};

/// The C program's `main`, under the name the build gave it.
pub type CMain = unsafe extern "C" fn(c_int, *mut *mut c_char, *mut *mut c_char) -> c_int;

/// A `NUL`-terminated array of owned C strings, kept alive for the rest of the process the way
/// a C `argv` and `environ` are.
fn leak_array(items: impl Iterator<Item = Vec<u8>>) -> *mut *mut c_char {
    let mut ptrs: Vec<*mut c_char> = items
        .map(|mut v| {
            // An interior NUL cannot be passed to C; such a word is cut at it, as `execve` would.
            if let Some(i) = v.iter().position(|&b| b == 0) {
                v.truncate(i);
            }
            CString::new(v).map_or(core::ptr::null_mut(), |c| c.into_raw().cast())
        })
        .collect();
    ptrs.push(core::ptr::null_mut());
    ptrs.leak().as_mut_ptr()
}

/// Run a C program's `main` with this process's words and environment, then `exit` with what it
/// returned, the way `crt0` does: `exit` runs the `atexit` handlers and flushes the C streams.
pub fn run(main: CMain) -> ! {
    let args: Vec<Vec<u8>> = std::env::args_os()
        .map(|a| a.into_encoded_bytes())
        .collect();
    let argc = c_int::try_from(args.len()).unwrap_or(c_int::MAX);
    let argv = leak_array(args.into_iter());
    let envp = leak_array(std::env::vars_os().map(|(k, v)| {
        let mut e = k.into_encoded_bytes();
        e.push(b'=');
        e.extend_from_slice(v.as_encoded_bytes());
        e
    }));
    // SAFETY: one thread, before any C code runs, so nothing reads these while they are written.
    unsafe {
        platform::argv = argv;
        platform::environ = envp;
        if !(*argv).is_null() {
            platform::program_invocation_name = *argv;
            platform::program_invocation_short_name = *argv;
        }
    }
    // SAFETY: `argv` and `envp` are NUL-terminated arrays of NUL-terminated strings that live for
    // the rest of the process, which is what a C `main` may assume.
    let code = unsafe { main(argc, argv, envp) };
    // SAFETY: called once, on the one thread, after `main` returned.
    unsafe { stdlib::exit(code) }
}
