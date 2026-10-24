//! How a builder fails: `must`, `must_ok` and `fail`. A half-built system is not worth limping
//! along, so every failed step traps, and the kernel names the pc.
//!
//! Name: provisional (milestone 843's lane, 2026-10-11 UTC).

/// Unwrap a `Result<_, ()>` or fault: a half-built system is not worth limping along. Generic, so
/// it also unwraps the `Child` `build_child` returns (DECISIONS §142 (what a spawner retains over a
/// child after `START`)).
pub(super) fn must<T>(r: Result<T, ()>) -> T {
    match r {
        Ok(v) => v,
        Err(()) => fail(),
    }
}

/// Fault unless the syscall succeeded.
pub(super) fn must_ok(ok: bool) {
    if !ok {
        fail();
    }
}

/// Trap. The kernel prints the pc and kills the process, which is the legible way for a builder to
/// say that the system it was asked to build cannot exist.
pub(super) fn fail() -> ! {
    supervision_protocol::fail()
}
