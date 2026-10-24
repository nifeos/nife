//! `std::process::id()` on nife (milestone 64): the one function in `std::process` that is nife's.
//!
//! The answer is `0`, a decision rather than a placeholder. A process here is identified by what it
//! holds and no syscall names the caller, so there is no identifier to report; `u32` cannot say
//! "there is none", and `0` is the one number no Unix assigns to a user process. Every reachable
//! call site in milestone 64's measured closures is a *fork* check (`gix-tempfile`'s `forksafe.rs`
//! compares an owning pid), which a constant makes right on a system with no `fork`. `gix-utils`'
//! `rng.rs` mixes the pid only into a seed taken when OS entropy is missing, and nife's is not
//! (milestone 56). Upstream's `unsupported` backend panics here. See notes/std/caveats.md.

//! # BUGS
//!
//! - **Every nife process reports 0**, so a scheme that derives cross-process uniqueness from the
//!   pid collides with nothing reporting it; no measured call site does that. A real per-process
//!   identity is a syscall-surface decision (DECISIONS §10, §16), not a PAL one.
//! - **Only `getpid` is nife's**; every other item in `std::process` comes from the shared
//!   `unsupported` backend, so `Command::spawn` refuses. Spawning by capability is a design fork,
//!   and the measurement records `gix-command` and `gix-credentials` as the crates that will force it.

/// This process's identifier, which does not exist, reported as `0`. See the module docs.
pub fn getpid() -> u32 {
    0
}
