//! A program that checks its own image and then exits (milestone 7): the "a real ELF ran and
//! verified itself" fixture.
//!
//! It needs no capabilities and no shared memory, so `kernel::user::tests` spawns it bare; its one
//! syscall, a yield, is authority over itself. The kernel test reads the absence of a fault plus
//! that syscall: `.text` ran, `.rodata` was readable, `.data` was copied, `.bss` was zeroed, and
//! the stack recursed eight frames. The checks are [`loaded_image_check`], shared with
//! `console_test_client`. It exits rather than spinning, because a one-shot fixture that never
//! exits holds a core for the rest of the boot (`no_leaked_threads`).
//!
//! Name: provisional (milestone 291). This was `hello`'s `SELF_CHECK` role, number 0, and 291 made
//! each of that binary's roles its own program. Refused `self_checker`: "self" names the caller
//! rather than what is checked, and this program checks the loaded image specifically. Refused
//! `elf_verifier`: it verifies nothing about ELF parsing, which is the `elf` crate's host tests;
//! it verifies that *this* image arrived intact in memory.

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

#[unsafe(no_mangle)]
pub extern "C" fn _start(_arg0: u64, _arg1: u64, _arg2: u64) -> ! {
    loaded_image_check::verify(user_mode_runtime::trap);

    // One syscall that needs no capability at all, to prove we reached EL0 and can trap back in.
    user_mode_runtime::yield_now();

    user_mode_runtime::exit();
}

user_mode_runtime::panic_handler!();
