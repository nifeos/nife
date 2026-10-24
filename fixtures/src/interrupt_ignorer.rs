//! The interrupt ignorer: a runaway that ignores the interrupt entirely (DECISIONS §24).
//!
//! A tight loop that touches nothing and checks nothing: the case the cooperative tier cannot
//! reach, and the reason the second `^C` exists. It never reads the interrupt flag and depends on
//! no frame the shell could revoke, so it is the honest worst case: only the forcible tier ends it,
//! the region owner's `DESTROY` force-killing the resident thread (DECISIONS §16, its milestone 22
//! amendment).
//!
//! It holds nothing: no capabilities, and it does not even map the shared job frame it was granted.
//!
//! Name: ratified 2026-09-13 (calef, working the unratified worklist). Refused `interrupt_spinner`
//! (the prefix parses as an object for `interrupt_heeder` and not for this one, since this program
//! does not spin the interrupt, it spins despite it), `spinner` (the field's ordinary word for a
//! thread that burns cycles rather than blocking, and the better of the old pair on its own; it
//! loses that borrowed recognition so the §24 pair parses one way rather than two). Performed by
//! milestone 175, which read all 177 occurrences of `spinner` by hand rather than sweeping: about
//! forty were this program and the rest are the English word, in `kernel/src/sched.rs`'s scheduler
//! assertions, `crates/virtio`, `kernel/src/testing.rs` and a dozen notes, so a pattern sweep would
//! have rewritten assertion text into nonsense.

#![no_std]
// Program entry points, not the crates/ library surface milestone 68's ratchet tracks
// (DECISIONS §107): each `[[bin]]` is its own crate root with one `_start`, and 58 of them
// documenting an OS-facing ABI entry point is not what the lint is for.
#![allow(missing_docs)]
#![no_main]

#[unsafe(no_mangle)]
pub extern "C" fn _start(_x0: u64, _x1: u64, _x2: u64) -> ! {
    loop {
        core::hint::spin_loop();
    }
}

user_mode_runtime::panic_handler!();
