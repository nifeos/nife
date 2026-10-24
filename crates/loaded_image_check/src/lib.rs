//! **A program proving its own ELF image was loaded correctly**, using nothing but its own memory.
//!
//! It checks that `.rodata` holds the file's bytes, `.data` its initializer and is writable, `.bss`
//! reads zero, and the stack survives eight frames of recursion. Every byte it reads belongs to the
//! process, so the milestone-7 test spawns it with no authority. It says no by trapping through the
//! caller's `fail`, because a program whose loader is broken has no reliable way to report; the
//! kernel turns the trap into a fault the test counts. See notes/elf/loaded-image-check.md.

//! # Bugs
//!
//! - The markers are `#[unsafe(no_mangle)]` statics, so exactly one copy of this crate may be linked.
//! - It cannot check the executable segment; the `W^X` assertions of DECISIONS §10 (process model:
//!   capability-based, microkernel) in `system_tests/src/user/elf_loader_tests.rs` catch that.

//! Name: ratified 2026-09-14 (calef, working the unratified worklist). Coined, not standard: the
//! published prior art verifies ELF sections loader-side, and `kselftest` and `POST` name a system
//! testing itself, where this program audits someone else's work on it. A crate because two
//! fixtures need it and AGENTS.md rule 7 admits no `#[path]` module. Refused `self_check` (a verb
//! phrase, and `self` names the caller rather than the thing checked). Refused `image_check`
//! ("image" is as often a disk image here: `target/nifefs.img`). Refused putting it in
//! `crates/user_mode_runtime` (a diagnostic nothing in a running system calls does not belong in
//! that surface, and a crate depending on it cannot build for the host).

#![no_std]

use core::sync::atomic::AtomicU64;
use core::sync::atomic::Ordering::Relaxed;

#[unsafe(no_mangle)]
static RODATA_MARKER: [u8; 4] = [0xc0, 0xff, 0xee, 0xd0];
// Atomics rather than `static mut` (milestone 891 (a mutable static becomes an atomic, a lock or a
// recorded cell)). An atomic is interior-mutable, so it is never placed in `.rodata`, and it is laid
// out as a plain `u64`, so a nonzero initializer is `.data` and a zero one is `.bss`. An atomic load
// is not folded to the initializer.
#[unsafe(no_mangle)]
static DATA_MARKER: AtomicU64 = AtomicU64::new(0x0000_c0ff_ee00_d0d0);
#[unsafe(no_mangle)]
static BSS_MARKER: AtomicU64 = AtomicU64::new(0);

/// Check this process's own image, calling `fail` if any part of it is wrong.
///
/// Returns normally when everything held. There is no error value on purpose: see the crate root
/// on how it says no. `fail` diverges, so nothing after a failed check runs.
///
/// # Examples
///
/// The whole of a self-checking program:
///
/// ```ignore
/// #[unsafe(no_mangle)]
/// pub extern "C" fn _start(_a: u64, _b: u64, _c: u64) -> ! {
///     loaded_image_check::verify(user_mode_runtime::trap);
///     user_mode_runtime::exit();
/// }
/// ```
pub fn verify(fail: fn() -> !) {
    let check = |ok: bool| {
        if !ok {
            fail()
        }
    };
    check(RODATA_MARKER == [0xc0, 0xff, 0xee, 0xd0]);
    check(DATA_MARKER.load(Relaxed) == 0x0000_c0ff_ee00_d0d0);
    check(BSS_MARKER.load(Relaxed) == 0); // .bss was zeroed
    BSS_MARKER.store(1, Relaxed);
    check(BSS_MARKER.load(Relaxed) == 1); // .bss is writable
    check(stack_works(7));
}

/// Recurse `n` frames, keeping a live array on each so the compiler cannot flatten the chain.
#[inline(never)]
fn stack_works(n: u64) -> bool {
    let local = [n; 8];
    if n == 0 {
        return local[0] == 0;
    }
    core::hint::black_box(&local);
    stack_works(n - 1)
}

#[cfg(test)]
mod tests {
    /// **The check passes on a correctly loaded image**, which on the host is every image: the
    /// point of running it here is that the *logic* is exercised by an ordinary `cargo test`
    /// rather than only inside an emulator. What it cannot prove on the host is the thing it
    /// exists for, a loader that got a segment wrong, so this is a smoke test and says so.
    #[test]
    fn a_correctly_loaded_image_passes_every_check() {
        super::verify(|| unreachable!("a host binary failed this crate's own image check"));
        // `verify` writes `.bss` to prove it is writable, and it is the only thing here that can
        // be seen from outside: a `verify` that did nothing would pass the line above. This test
        // is the one caller in this binary, so the marker is not raced.
        let marker = super::BSS_MARKER.load(super::Relaxed);
        assert_eq!(marker, 1, "verify did not run its checks");
    }

    /// The recursion really recurses: eight frames deep, and the answer depends on the deepest.
    #[test]
    fn the_stack_check_reaches_the_bottom() {
        assert!(super::stack_works(7));
        assert!(super::stack_works(0));
    }
}
